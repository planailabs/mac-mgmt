//! Sovereign-AI USB build.
//!
//! When `mac-mgmt` is built with `--features usb`, running it with no
//! subcommand (or an explicit `usb` subcommand) boots a fully self-contained,
//! relocatable AI stack off the flash device the binary lives on:
//!
//!   1. Pin everything to the stick (set `HOME` into the binary's directory).
//!   2. Select a nix runtime — prefer an existing host nix, else bootstrap
//!      nix-portable (Linux) against an ext4 store image mounted at `/nix`
//!      inside a private mount namespace.
//!   3. Install the AI services declared in `config.toml` (substituting from
//!      the on-stick `.nar` cache offline, or xzar online).
//!   4. Run the in-process service supervisor (`mac-mgmt-services`).
//!   5. Serve the memvault web app on its own port.
//!   6. Open the native Dioxus "overview" desktop window (unless `--headless`).
//!
//! The webview event loop must own the process main thread, so this entry is a
//! plain `fn` (not `#[tokio::main]`): it builds a multi-thread runtime, runs the
//! stack on background threads, and runs the desktop event loop on the main
//! thread. See `daemon/src/main.rs` for the early-argv dispatch.

pub mod control;
pub mod nix_portable;
pub mod nixpkgs;
pub mod prefetch;
pub mod runtime;
pub mod store_image;

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Whether the stack may touch the network. `Offline` is fail-loud: any missing
/// dependency is an error, never a silent online fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkMode {
    Online,
    Offline,
}

impl NetworkMode {
    pub fn is_offline(self) -> bool {
        matches!(self, NetworkMode::Offline)
    }
}

/// Parsed `usb` command-line flags.
#[derive(clap::Parser, Debug)]
#[command(name = "mac-mgmt usb", about = "Run the sovereign-AI USB stack")]
pub struct UsbCli {
    /// Run with no network: only the on-stick cache is used, the updater is
    /// skipped, and anything missing is a hard error.
    #[arg(long)]
    pub offline: bool,

    /// Bring up the full stack without opening the desktop overview window.
    /// Used by the VM/CI e2e test and headless hosts.
    #[arg(long)]
    pub headless: bool,

    /// Override the stick/home directory (default: the binary's own directory).
    #[arg(long)]
    pub home: Option<PathBuf>,

    /// Override the config path (default: `<home>/config.toml`).
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Override the pinned nixpkgs revision used for evaluation.
    #[arg(long = "nixpkgs-rev")]
    pub nixpkgs_rev: Option<String>,
}

/// Resolved options for bringing up the stack. Built from [`UsbCli`] plus
/// environment fallbacks (`MAC_MGMT_OFFLINE`).
#[derive(Clone, Debug)]
pub struct StackOpts {
    /// The stick / `HOME` directory. Everything (config, nix state, caches)
    /// lives under here so the build is fully relocatable.
    pub home: PathBuf,
    /// Explicit config path; when `None`, `<home>/config.toml` then the
    /// standard config path are tried.
    pub config_path: Option<PathBuf>,
    pub network: NetworkMode,
    /// Open the desktop overview window. `false` in headless/test mode.
    pub ui: bool,
    pub nixpkgs_rev: Option<String>,
}

impl StackOpts {
    /// Build options from parsed CLI flags, applying environment fallbacks and
    /// resolving the home directory to the binary's directory when unset.
    pub fn from_cli(cli: &UsbCli) -> Result<Self> {
        let home = match &cli.home {
            Some(p) => p.clone(),
            None => default_home_dir()?,
        };
        let offline = cli.offline || env_flag("MAC_MGMT_OFFLINE");
        Ok(StackOpts {
            home,
            config_path: cli.config.clone(),
            network: if offline {
                NetworkMode::Offline
            } else {
                NetworkMode::Online
            },
            ui: !cli.headless,
            nixpkgs_rev: cli.nixpkgs_rev.clone(),
        })
    }
}

/// Set an environment variable only if it is not already set (respect the
/// user's override). Call single-threaded, before the tokio runtime starts.
fn set_env_default(key: &str, val: &str) {
    if std::env::var_os(key).is_none() {
        // SAFETY: called single-threaded during pre-runtime startup.
        unsafe {
            std::env::set_var(key, val);
        }
    }
}

/// True for `1`/`true`/`yes`/`on` (case-insensitive) env values.
fn env_flag(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

/// The default stick/home directory: the directory containing the running
/// executable. A `home/` subdirectory keeps the binary's own directory clean.
pub fn default_home_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to resolve current executable path")?;
    let dir = exe
        .parent()
        .context("executable has no parent directory")?
        .to_path_buf();
    Ok(dir.join("home"))
}

/// Process entry point for usb-run mode. Dispatched from `main.rs` before any
/// async runtime is started, so the desktop event loop can own the main thread.
///
/// `args` is the full process argv (including the program name); a leading
/// `usb` subcommand token is stripped before parsing.
pub fn main(args: Vec<String>) -> ! {
    let code = match run_main(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run_main(mut args: Vec<String>) -> Result<()> {
    use clap::Parser;

    // Strip an optional leading `usb` subcommand token: both `mac-mgmt` (default
    // flip) and `mac-mgmt usb …` route here.
    if args.get(1).map(String::as_str) == Some("usb") {
        args.remove(1);
    }
    let cli = UsbCli::parse_from(args);
    let opts = StackOpts::from_cli(&cli)?;

    init_tracing();

    // Install the rustls crypto provider before any TLS/reqwest client is built
    // (the overview's control-API client, the updater, memvault). The usb entry
    // bypasses daemon_main::main(), which is where the daemon normally does this
    // — without it, reqwest::Client construction panics with "No provider set".
    let _ = rustls::crypto::ring::default_provider().install_default();

    std::fs::create_dir_all(&opts.home)
        .with_context(|| format!("failed to create home dir {}", opts.home.display()))?;

    // Pin everything to the stick: point HOME at the stick dir so the supervisor
    // socket, nix profile, config dir, and nix-portable state all live under it
    // (fully relocatable, never writes the host's real home). Done here,
    // single-threaded, before any runtime/thread is created.
    // SAFETY: single-threaded startup, before the tokio runtime.
    unsafe {
        std::env::set_var("HOME", &opts.home);
    }

    if opts.ui {
        // Harden the GTK/webview environment before the window opens (done here,
        // single-threaded). Loading the system fcitx5 GTK input-method module
        // into the nix-built GTK (different GLib ABI) segfaults the webview, so
        // force the built-in simple IM; skip the a11y bus (HOME is repinned to
        // the stick, so the at-spi socket dir isn't ours); and disable webkit GL
        // compositing, which crashes on some drivers.
        set_env_default("GTK_IM_MODULE", "gtk-im-context-simple");
        set_env_default("NO_AT_BRIDGE", "1");
        set_env_default("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    // Select + prepare the nix runtime *before* building the tokio runtime: the
    // Portable path enters a mount namespace, which only moves the calling
    // thread, so it must happen while we are still single-threaded.
    let rt = runtime::prepare(&opts.home, opts.network.is_offline())
        .context("failed to prepare nix runtime")?;

    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for usb stack")?;

    // Bring the stack up on the runtime's background threads.
    let handle = tokio_rt
        .block_on(async { run_stack(opts.clone(), rt).await })
        .context("failed to start usb stack")?;

    if opts.ui {
        // The overview desktop window owns the main thread. When it closes we
        // tear the stack down. (Implemented in the overview phase.)
        run_overview_ui(&handle, &tokio_rt)?;
    } else {
        // Headless: block until a shutdown signal, then tear down.
        tokio_rt.block_on(handle.wait_for_shutdown());
    }

    tokio_rt.block_on(handle.shutdown());
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Handle to a running stack. Lets the caller wait for and trigger shutdown.
pub struct StackHandle {
    /// Loopback port serving the daemon status + control API.
    pub metrics_port: u16,
    /// Loopback URL of the memvault web app.
    pub memvault_url: Option<String>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl StackHandle {
    /// Resolve once a shutdown has been requested (Ctrl-C or `request_shutdown`).
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.shutdown_tx.subscribe();
        // Also trip on Ctrl-C.
        let ctrl_c = tokio::signal::ctrl_c();
        tokio::select! {
            _ = ctrl_c => {}
            _ = async {
                loop {
                    if *rx.borrow_and_update() {
                        return;
                    }
                    if rx.changed().await.is_err() {
                        return;
                    }
                }
            } => {}
        }
    }

    /// Request shutdown (e.g. when the overview window closes).
    pub fn request_shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Tear the stack down gracefully.
    pub async fn shutdown(&self) {
        self.request_shutdown();
        // Per-component teardown is wired up as the stack is fleshed out.
    }
}

/// Bring up the full stack (config load, in-process supervisor, services, and
/// the loopback control/status API) without the desktop window, against an
/// already-prepared nix `rt`. Returns once everything is running. This is the
/// headless seam the repo tests drive.
pub async fn run_stack(opts: StackOpts, rt: runtime::Runtime) -> Result<StackHandle> {
    use std::sync::Arc;

    tracing::info!(
        home = %opts.home.display(),
        offline = opts.network.is_offline(),
        ui = opts.ui,
        runtime = ?rt,
        "starting sovereign-AI usb stack",
    );

    std::fs::create_dir_all(&opts.home)
        .with_context(|| format!("failed to create home dir {}", opts.home.display()))?;

    // Point nix (substituters, nixpkgs, offline) at the on-stick artifacts.
    configure_nix(&opts);

    // Load config locally — never reach the mac-mgmt server.
    let mut cfg = load_config(&opts).context("failed to load usb config")?;

    // The supervisor runs in-process for the USB build.
    // SAFETY: set before ServiceManager::init reads it; single consumer.
    unsafe {
        std::env::set_var("INPROCESS_SERVICE_MANAGER", "1");
    }

    let dispatcher = Arc::new(crate::notify::Dispatcher::new(
        std::mem::take(&mut cfg.notifications.urls),
        cfg.notifications.events.take(),
    ));
    let log_buf = crate::log_buffer::LogBuffer::new();
    let metrics = Arc::new(crate::metrics::Metrics::new());

    let mut svc_mgr = crate::service_mgmt::ServiceManager::init(
        &mut cfg,
        Arc::clone(&dispatcher),
        log_buf.clone(),
    )
    .context("failed to init service manager")?;
    svc_mgr.register_metrics(&metrics);

    let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
    let (install_tx, mut install_rx) = tokio::sync::mpsc::channel::<control::InstallReq>(8);

    // Loopback control + status server on an ephemeral port.
    let control_state = control::ControlState {
        socket_path: mac_mgmt_services::default_socket_path(),
        offline: opts.network.is_offline(),
        install_tx,
        shutdown_tx: shutdown_tx.clone(),
    };
    let listener = tokio::net::TcpListener::bind(("::1", 0))
        .await
        .context("failed to bind control server")?;
    let control_port = listener.local_addr()?.port();
    let router = control::router(control_state);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!("control server exited: {e}");
        }
    });
    tracing::info!("usb control/status API on http://[::1]:{control_port}");

    // Stack loop: initial service bring-up, then periodic health + install
    // requests, until shutdown.
    let health_interval = humantime::parse_duration(&cfg.daemon.health_interval)
        .unwrap_or_else(|_| std::time::Duration::from_secs(30));
    let metrics_loop = Arc::clone(&metrics);
    let mut loop_shutdown = shutdown_tx.subscribe();
    tokio::spawn(async move {
        // Kick installs + start everything.
        svc_mgr.retry_failed_installs();
        svc_mgr.schedule_restart().await;
        let mut health = tokio::time::interval(health_interval);
        loop {
            tokio::select! {
                _ = health.tick() => {
                    svc_mgr.health_tick(&metrics_loop, false).await;
                }
                Some(req) = install_rx.recv() => {
                    // Coarse but real: re-attempt installs, then restart.
                    svc_mgr.retry_failed_installs();
                    svc_mgr.schedule_restart().await;
                    let _ = req.resp.send(Ok(()));
                }
                _ = loop_shutdown.changed() => {
                    if *loop_shutdown.borrow() {
                        tracing::info!("usb stack loop shutting down");
                        break;
                    }
                }
            }
        }
    });

    // Self-contained binary updater (online only): periodically check GitLab
    // releases and, on a newer build, self-replace + re-exec.
    if !opts.network.is_offline() {
        let mut up_shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
            // Skip the immediate first tick.
            tick.tick().await;
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        match crate::usb_update::check_and_apply().await {
                            Ok(true) => {
                                tracing::info!("usb update applied; re-exec");
                                if let Err(e) = crate::usb_update::reexec_self() {
                                    tracing::error!("re-exec after update failed: {e:#}");
                                }
                            }
                            Ok(false) => {}
                            Err(e) => tracing::warn!("usb update check failed: {e:#}"),
                        }
                    }
                    _ = up_shutdown.changed() => {
                        if *up_shutdown.borrow() { break; }
                    }
                }
            }
        });
    }

    // Best-effort: serve the memvault web app on its own port if configured.
    let memvault_url = maybe_serve_memvault(&opts).await;

    Ok(StackHandle {
        metrics_port: control_port,
        memvault_url,
        shutdown_tx,
    })
}

/// Configure nix env (substituters, nixpkgs tarball, offline) for the stack.
fn configure_nix(opts: &StackOpts) {
    let offline = opts.network.is_offline();

    // Pinned nixpkgs tarball, if cached on the stick.
    let rev = opts
        .nixpkgs_rev
        .clone()
        .unwrap_or_else(|| nixpkgs::DEFAULT_REV.to_string());
    let tarball = nixpkgs::cached_path(&opts.home, &rev);
    if tarball.exists() {
        // SAFETY: single-threaded pre-runtime / startup config.
        unsafe {
            std::env::set_var("MAC_MGMT_NIXPKGS_TARBALL", nixpkgs::flakeref_base(&tarball));
        }
    }

    // Substituters: on-stick .nar cache first, then xzar online.
    let mut caches: Vec<(String, String)> =
        vec![(store_image::nar_cache_substituter(&opts.home), String::new())];
    if !offline {
        caches.push((
            "https://xzar.plan.ai".to_string(),
            "xzar.plan.ai:KUE66pjr6UX5HHCn9kedN1DJ2J5nSlBrKmE7tUjXewE=".to_string(),
        ));
    }
    crate::nix::set_nix_caches(caches);

    if offline {
        unsafe {
            std::env::set_var("MAC_MGMT_NIX_OFFLINE", "1");
        }
    }
}

/// Load the daemon config from (in order) the explicit `--config`,
/// `<home>/config.toml`, or the standard config path. Never fetches remote.
fn load_config(opts: &StackOpts) -> Result<crate::config::Config> {
    let candidates: Vec<std::path::PathBuf> = opts
        .config_path
        .clone()
        .into_iter()
        .chain(std::iter::once(opts.home.join("config.toml")))
        .chain(std::iter::once(crate::config::config_path()))
        .collect();
    for path in candidates {
        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let cfg: crate::config::Config = toml::from_str(&contents)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            tracing::info!("loaded usb config from {}", path.display());
            return Ok(cfg);
        }
    }
    tracing::warn!("no config.toml found; using defaults");
    Ok(crate::config::Config::default())
}

/// Best-effort: spawn the memvault web app on its own loopback port. Returns
/// its URL when started. Memvault remains its own Dioxus web app (unchanged);
/// the overview links to it.
async fn maybe_serve_memvault(_opts: &StackOpts) -> Option<String> {
    // Memvault serving reuses the daemon's existing memvault bridge when a
    // [memvault] config + data dir are present. Left as an opt-in follow-up so
    // a minimal stick (no memvault) still boots cleanly.
    None
}

/// Run the native overview desktop window on the main thread (it owns the wry
/// event loop). Requires the `usb-ui` feature; otherwise the caller must use
/// `--headless`.
#[cfg(feature = "usb-ui")]
fn run_overview_ui(handle: &StackHandle, _runtime: &tokio::runtime::Runtime) -> Result<()> {
    mac_mgmt_overview::launch(handle.metrics_port, handle.memvault_url.clone());
    // The window has closed — ask the stack to shut down.
    handle.request_shutdown();
    Ok(())
}

#[cfg(not(feature = "usb-ui"))]
fn run_overview_ui(_handle: &StackHandle, _runtime: &tokio::runtime::Runtime) -> Result<()> {
    anyhow::bail!(
        "this build has no desktop overview (built without the `usb-ui` feature) — use --headless"
    )
}
