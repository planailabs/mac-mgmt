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

    Ok(())
}
