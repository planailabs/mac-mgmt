use anyhow::{Context, Result};
use rust_embed::Embed;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Embed)]
#[folder = "os_mgmt/"]
pub struct OsConfigs;

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn sudo_mkdir_p(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if is_root() {
        std::fs::create_dir_all(path)
            .with_context(|| format!("failed to create directory {}", path.display()))?;
    } else {
        let status = Command::new("sudo")
            .args(["mkdir", "-p", &path.to_string_lossy()])
            .status()
            .with_context(|| format!("failed to run sudo mkdir -p {}", path.display()))?;
        if !status.success() {
            anyhow::bail!("sudo mkdir -p {} failed", path.display());
        }
    }
    Ok(())
}

fn sudo_write(path: &Path, contents: &[u8]) -> Result<()> {
    if is_root() {
        std::fs::write(path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        use std::io::Write;
        let mut child = Command::new("sudo")
            .args(["tee", &path.to_string_lossy()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("failed to run sudo tee {}", path.display()))?;
        child.stdin.as_mut().unwrap().write_all(contents)?;
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("sudo tee {} failed", path.display());
        }
    }
    Ok(())
}

fn detect_os() -> Result<String> {
    if cfg!(target_os = "macos") {
        return Ok("macos".to_string());
    }

    let content = std::fs::read_to_string("/etc/os-release")
        .context("failed to read /etc/os-release — cannot detect OS")?;

    for line in content.lines() {
        if let Some(id) = line.strip_prefix("ID=") {
            return Ok(id.trim_matches('"').to_string());
        }
    }

    anyhow::bail!("could not find ID= in /etc/os-release")
}

pub fn configure_os(dry_run: bool) -> Result<()> {
    let os_id = detect_os()?;
    let prefix = format!("{os_id}/");
    let mut count = 0;

    for path in OsConfigs::iter() {
        let Some(rel) = path.strip_prefix(&prefix) else {
            continue;
        };

        let target = PathBuf::from("/").join(rel);
        let file = OsConfigs::get(&path).unwrap();
        let contents = &file.data;

        // Skip if target already has identical contents
        if target.exists() {
            if let Ok(existing) = std::fs::read(&target) {
                if existing == contents.as_ref() {
                    println!("unchanged: {}", target.display());
                    count += 1;
                    continue;
                }
            }
        }

        if dry_run {
            println!("would install: {}", target.display());
        } else {
            if let Some(parent) = target.parent() {
                sudo_mkdir_p(parent)?;
            }
            sudo_write(&target, contents)?;
            println!("installed: {}", target.display());
        }
        count += 1;
    }

    if count == 0 {
        println!("No OS configurations found for {os_id}");
    }

    // Ensure nix.conf has the required settings for the daemon to operate
    // as a trusted user with flakes enabled.
    configure_nix(dry_run)?;

    #[cfg(target_os = "linux")]
    {
        configure_ufw(dry_run)?;
    }

    Ok(())
}

/// Ensure /etc/nix/nix.conf contains `extra-trusted-users` for the daemon user
/// and `extra-experimental-features = nix-command flakes`. Appends missing lines.
fn configure_nix(dry_run: bool) -> Result<()> {
    let nix_conf = Path::new("/etc/nix/nix.conf");
    let user = std::env::var("USER").unwrap_or_else(|_| "mac-mgmt".to_string());

    let existing = if nix_conf.exists() {
        std::fs::read_to_string(nix_conf).unwrap_or_default()
    } else {
        String::new()
    };

    let mut additions = Vec::new();

    // Check for trusted-users / extra-trusted-users containing our user
    let has_trusted = existing.lines().any(|line| {
        let trimmed = line.trim();
        (trimmed.starts_with("trusted-users") || trimmed.starts_with("extra-trusted-users"))
            && trimmed.contains(&user)
    });
    if !has_trusted {
        additions.push(format!("extra-trusted-users = {user}"));
    }

    // Check for experimental-features containing nix-command and flakes
    let has_experimental = existing.lines().any(|line| {
        let trimmed = line.trim();
        (trimmed.starts_with("experimental-features")
            || trimmed.starts_with("extra-experimental-features"))
            && trimmed.contains("nix-command")
            && trimmed.contains("flakes")
    });
    if !has_experimental {
        additions.push("extra-experimental-features = nix-command flakes".to_string());
    }

    if additions.is_empty() {
        println!("nix.conf: already configured");
        return Ok(());
    }

    if dry_run {
        for line in &additions {
            println!("would append to /etc/nix/nix.conf: {line}");
        }
        return Ok(());
    }

    // Ensure /etc/nix exists
    sudo_mkdir_p(Path::new("/etc/nix"))?;

    let append_content = format!("\n{}\n", additions.join("\n"));
    // Use sudo tee --append for non-root
    if is_root() {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(nix_conf)
            .context("failed to open /etc/nix/nix.conf for append")?;
        f.write_all(append_content.as_bytes())?;
    } else {
        use std::io::Write;
        let mut child = Command::new("sudo")
            .args(["tee", "--append", "/etc/nix/nix.conf"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .context("failed to run sudo tee --append /etc/nix/nix.conf")?;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(append_content.as_bytes())?;
        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("sudo tee --append /etc/nix/nix.conf failed");
        }
    }

    for line in &additions {
        println!("appended to /etc/nix/nix.conf: {line}");
    }

    // Restart nix-daemon so it picks up the new config
    restart_nix_daemon();

    Ok(())
}

fn restart_nix_daemon() {
    if cfg!(target_os = "macos") {
        let _ = Command::new("sudo")
            .args(["launchctl", "kickstart", "-k", "system/org.nixos.nix-daemon"])
            .status();
    } else {
        let _ = Command::new("sudo")
            .args(["systemctl", "restart", "nix-daemon"])
            .status();
    }
}

#[cfg(target_os = "linux")]
fn run_ufw(args: &[&str], dry_run: bool) -> Result<()> {
    let display = format!("ufw {}", args.join(" "));
    if dry_run {
        println!("would run: {display}");
        return Ok(());
    }
    let mut cmd = if is_root() {
        let mut c = Command::new("ufw");
        c.args(args);
        c
    } else {
        let mut c = Command::new("sudo");
        c.arg("ufw");
        c.args(args);
        c
    };
    let status = cmd
        .status()
        .with_context(|| format!("failed to run {display}"))?;
    if !status.success() {
        anyhow::bail!("{display} failed");
    }
    println!("ran: {display}");
    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_ufw(dry_run: bool) -> Result<()> {
    // Check if ufw is available
    if Command::new("ufw").arg("status").output().is_err() {
        println!("ufw: not found, skipping firewall configuration");
        return Ok(());
    }

    // Allow SSH before enabling — critical to avoid lockout
    run_ufw(
        &["allow", "22/tcp", "comment", "mac-mgmt: SSH"],
        dry_run,
    )?;

    // Enable UFW (--force skips the interactive confirmation)
    run_ufw(&["--force", "enable"], dry_run)?;

    // Open P2P ports
    run_ufw(
        &["allow", "1122/udp", "comment", "mac-mgmt: P2P QUIC"],
        dry_run,
    )?;
    run_ufw(
        &["allow", "1122/tcp", "comment", "mac-mgmt: P2P WebSocket"],
        dry_run,
    )?;

    Ok(())
}
