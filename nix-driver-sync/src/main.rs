use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn};

#[derive(Parser)]
#[command(about = "Sync /run/opengl-driver into Incus containers")]
struct Cli {
    /// Incus project(s) to sync
    #[arg(long = "project", required = true)]
    projects: Vec<String>,

    /// Path to the JSON state file tracking per-container store paths
    #[arg(long)]
    state_file: PathBuf,
}

#[derive(Deserialize)]
struct IncusContainer {
    name: String,
    status: String,
}

type State = HashMap<String, String>;

fn run(cmd: &mut Command) -> Result<String> {
    let out = cmd.output().context("failed to spawn command")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!(
            "{:?} failed ({}): {}",
            cmd.get_program(),
            out.status,
            stderr.trim()
        );
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn load_state(path: &PathBuf) -> State {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(path: &PathBuf, state: &State) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)?;
    fs::write(path, json)?;
    Ok(())
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    // 1. Add /run/opengl-driver to the nix store
    let store_path = run(Command::new("nix-store").args(["--add", "/run/opengl-driver"]))
        .context("nix-store --add failed")?;
    info!(store_path, "current opengl-driver store path");

    // 2. Load per-container state
    let mut state = load_state(&cli.state_file);

    // 3. Export NAR with closure to a temp file
    let closure = run(Command::new("nix-store").args(["-qR", &store_path]))
        .context("nix-store -qR failed")?;
    let closure_paths: Vec<&str> = closure.lines().collect();

    let mut nar_file = tempfile::NamedTempFile::new().context("failed to create temp file")?;
    {
        let mut export = Command::new("nix-store")
            .arg("--export")
            .args(&closure_paths)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .context("nix-store --export failed to start")?;

        let mut stdout = export.stdout.take().unwrap();
        std::io::copy(&mut stdout, &mut nar_file).context("failed to write NAR")?;

        let status = export.wait()?;
        if !status.success() {
            bail!("nix-store --export failed: {}", status);
        }
    }
    let nar_path = nar_file.path().to_path_buf();
    info!(nar = %nar_path.display(), "exported NAR closure");

    // 4. Process each project
    for project in &cli.projects {
        let json = run(Command::new("incus").args([
            "list",
            "--project",
            project,
            "--format",
            "json",
        ]))
        .with_context(|| format!("incus list --project {project} failed"))?;

        let containers: Vec<IncusContainer> =
            serde_json::from_str(&json).context("failed to parse incus list output")?;

        for ct in &containers {
            if ct.status != "Running" {
                continue;
            }

            let key = format!("{}/{}", project, ct.name);

            // 5a. Mount /run/opengl-driver (idempotent)
            let devices = run(Command::new("incus").args([
                "config",
                "device",
                "show",
                &ct.name,
                "--project",
                project,
            ]))
            .unwrap_or_default();

            if !devices.contains("opengl-driver") {
                info!(container = %key, "adding opengl-driver disk device");
                if let Err(e) = run(Command::new("incus").args([
                    "config",
                    "device",
                    "add",
                    &ct.name,
                    "opengl-driver",
                    "disk",
                    "source=/run/opengl-driver",
                    "path=/run/opengl-driver",
                    "--project",
                    project,
                ])) {
                    warn!(container = %key, error = %e, "failed to add disk device");
                    continue;
                }
            }

            // 5b. Check if NAR import needed
            if state.get(&key).map(|s| s.as_str()) == Some(&store_path) {
                info!(container = %key, "already synced, skipping");
                continue;
            }

            // 5c. Push NAR and import
            info!(container = %key, "syncing NAR closure");

            if let Err(e) = run(Command::new("incus").args([
                "file",
                "push",
                nar_path.to_str().unwrap(),
                &format!("{}/tmp/opengl-driver.nar", ct.name),
                "--project",
                project,
            ])) {
                warn!(container = %key, error = %e, "failed to push NAR");
                continue;
            }

            if let Err(e) = run(Command::new("incus").args([
                "exec",
                &ct.name,
                "--project",
                project,
                "--",
                "nix-store",
                "--import",
                "/tmp/opengl-driver.nar",
            ])) {
                warn!(container = %key, error = %e, "failed to import NAR");
                // still try cleanup
            } else {
                state.insert(key.clone(), store_path.clone());
            }

            // cleanup
            let _ = run(Command::new("incus").args([
                "exec",
                &ct.name,
                "--project",
                project,
                "--",
                "rm",
                "-f",
                "/tmp/opengl-driver.nar",
            ]));
        }
    }

    // 6. Save state
    save_state(&cli.state_file, &state)?;
    info!("done");

    Ok(())
}
