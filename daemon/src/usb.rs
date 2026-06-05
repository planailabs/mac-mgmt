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

pub mod nix_portable;
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

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for usb stack")?;

    // Bring the stack up on the runtime's background threads.
    let handle = runtime
        .block_on(async { run_stack(opts.clone()).await })
        .context("failed to start usb stack")?;

    if opts.ui {
        // The overview desktop window owns the main thread. When it closes we
        // tear the stack down. (Implemented in the overview phase.)
        run_overview_ui(&handle, &runtime)?;
    } else {
        // Headless: block until a shutdown signal, then tear down.
        runtime.block_on(handle.wait_for_shutdown());
    }

    runtime.block_on(handle.shutdown());
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

/// Bring up the full stack (runtime selection, store, supervisor, services,
/// memvault) without the desktop window. Returns once everything is running.
///
/// This is the headless seam the repo tests drive directly.
pub async fn run_stack(opts: StackOpts) -> Result<StackHandle> {
    tracing::info!(
        home = %opts.home.display(),
        offline = opts.network.is_offline(),
        ui = opts.ui,
        "starting sovereign-AI usb stack",
    );

    std::fs::create_dir_all(&opts.home)
        .with_context(|| format!("failed to create home dir {}", opts.home.display()))?;

    // Subsequent phases fill in: runtime selection + store mount, config load,
    // in-process supervisor, service install, memvault serving.
    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);

    anyhow::bail!(
        "usb stack orchestration is not yet wired up (home={})",
        opts.home.display()
    );
    #[allow(unreachable_code)]
    Ok(StackHandle {
        metrics_port: 0,
        memvault_url: None,
        shutdown_tx,
    })
}

/// Run the native overview desktop window on the main thread. Implemented in
/// the overview phase; until then this is unavailable.
fn run_overview_ui(_handle: &StackHandle, _runtime: &tokio::runtime::Runtime) -> Result<()> {
    anyhow::bail!("overview desktop UI is not yet available — use --headless")
}
