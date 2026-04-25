use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::backend::IncusBackend;
use crate::types::{ExecOutput, OsImage};

pub struct CliBackend {
    project: Option<String>,
}

impl CliBackend {
    pub fn new(project: Option<String>) -> Self {
        Self { project }
    }

    fn project_args(&self) -> Vec<String> {
        match &self.project {
            Some(p) => vec!["--project".to_string(), p.clone()],
            None => vec![],
        }
    }
}

#[async_trait]
impl IncusBackend for CliBackend {
    async fn launch(&self, image: &str, name: &str) -> Result<()> {
        let mut args = vec![
            "launch".to_string(),
            format!("images:{image}"),
            name.to_string(),
            "--ephemeral".to_string(),
        ];
        args.extend(self.project_args());

        let output = Command::new("incus")
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus launch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("incus launch failed: {stderr}");
        }

        // Wait for container to be running (up to 60s).
        for _ in 0..60 {
            if let Some(s) = self.status(name).await? {
                if s == "Running" {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        bail!("container {name} did not reach Running state within 60s");
    }

    async fn exec(&self, name: &str, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let mut args = vec!["exec".to_string()];
        args.extend(self.project_args());
        args.extend([
            name.to_string(),
            "--".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ]);

        let result = tokio::time::timeout(
            timeout,
            Command::new("incus")
                .args(&args)
                .stdin(std::process::Stdio::null())
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                Ok(ExecOutput {
                    stdout,
                    stderr,
                    exit_code,
                })
            }
            Ok(Err(e)) => bail!("failed to spawn incus exec: {e}"),
            Err(_) => bail!("command timed out after {}s", timeout.as_secs()),
        }
    }

    async fn delete(&self, name: &str) -> Result<()> {
        let mut args = vec![
            "delete".to_string(),
            name.to_string(),
            "--force".to_string(),
        ];
        args.extend(self.project_args());

        let output = Command::new("incus")
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "not found" errors during cleanup.
            if !stderr.contains("not found") {
                bail!("incus delete failed: {stderr}");
            }
        }
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<Option<String>> {
        let mut args = vec![
            "list".to_string(),
            name.to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        args.extend(self.project_args());

        let output = Command::new("incus")
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus list")?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&stdout).unwrap_or_default();

        for entry in &entries {
            if entry.get("name").and_then(|n| n.as_str()) == Some(name) {
                return Ok(entry
                    .get("status")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()));
            }
        }
        Ok(None)
    }

    async fn image_list(&self) -> Result<Vec<OsImage>> {
        let mut args = vec![
            "image".to_string(),
            "list".to_string(),
            "images:".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        args.extend(self.project_args());

        let output = Command::new("incus")
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus image list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("incus image list failed: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&stdout).context("failed to parse image list JSON")?;

        let current_arch = std::env::consts::ARCH;
        let incus_arch = match current_arch {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };

        let mut images = Vec::new();
        for entry in &entries {
            let props = match entry.get("properties") {
                Some(p) => p,
                None => continue,
            };

            // Filter to current architecture and container type.
            let arch = props
                .get("architecture")
                .and_then(|a| a.as_str())
                .unwrap_or("");
            if arch != incus_arch {
                continue;
            }

            let image_type = entry
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if image_type != "container" {
                continue;
            }

            let aliases = entry.get("aliases").and_then(|a| a.as_array());
            let alias = aliases
                .and_then(|a| {
                    a.iter()
                        .filter_map(|e| e.get("name").and_then(|n| n.as_str()))
                        // Prefer the shortest alias (e.g. "ubuntu/24.04" over "ubuntu/24.04/amd64").
                        .min_by_key(|n| n.len())
                })
                .unwrap_or("");

            if alias.is_empty() {
                continue;
            }

            let description = props
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let os = props
                .get("os")
                .and_then(|o| o.as_str())
                .unwrap_or("")
                .to_string();
            let release = props
                .get("release")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            let variant = props
                .get("variant")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();

            images.push(OsImage {
                alias: alias.to_string(),
                description,
                os,
                release,
                variant,
                image_type: image_type.to_string(),
            });
        }

        // Deduplicate by alias (some images have multiple fingerprints).
        images.sort_by(|a, b| a.alias.cmp(&b.alias));
        images.dedup_by(|a, b| a.alias == b.alias);

        Ok(images)
    }

    async fn file_push(&self, name: &str, path: &str, content: &[u8]) -> Result<()> {
        let mut args = vec!["file".to_string(), "push".to_string(), "-".to_string()];
        args.extend(self.project_args());
        args.push(format!("{name}/{path}"));
        // Create parent directories.
        args.push("--create-dirs".to_string());

        let mut child = Command::new("incus")
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to spawn incus file push")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(content)
                .await
                .context("writing to incus file push stdin")?;
        }

        let output = child
            .wait_with_output()
            .await
            .context("waiting for incus file push")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("incus file push failed: {stderr}");
        }
        Ok(())
    }

    async fn file_pull(&self, name: &str, path: &str) -> Result<String> {
        let mut args = vec![
            "file".to_string(),
            "pull".to_string(),
        ];
        args.extend(self.project_args());
        args.extend([format!("{name}/{path}"), "-".to_string()]);

        let output = Command::new("incus")
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus file pull")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("incus file pull failed: {stderr}");
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
