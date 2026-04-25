use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::backend::IncusBackend;
use crate::types::{ExecOutput, OsImage};

/// Backend that shells out to the `incus` CLI binary.
///
/// Only used for `exec` and `file push/pull` operations that are awkward
/// over the REST API (exec requires websockets). Instance lifecycle and
/// image listing go through [`crate::incus_unix::UnixBackend`] instead.
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

    fn cmd(&self) -> Command {
        let mut cmd = Command::new("incus");
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd
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

        let output = self
            .cmd()
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

        let result = tokio::time::timeout(timeout, self.cmd().args(&args).output()).await;

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

        let output = self
            .cmd()
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("not found") {
                bail!("incus delete failed: {stderr}");
            }
        }
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<Option<String>> {
        let mut args = vec![
            "list".to_string(),
            format!("^{name}$"),
            "--format".to_string(),
            "json".to_string(),
        ];
        args.extend(self.project_args());

        let output = self
            .cmd()
            .args(&args)
            .output()
            .await
            .context("failed to spawn incus list")?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap_or_default();

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

        let output = self
            .cmd()
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

        Ok(crate::incus_common::parse_image_list(&entries))
    }

    async fn file_push(&self, name: &str, path: &str, content: &[u8]) -> Result<()> {
        let mut args = vec!["file".to_string(), "push".to_string(), "-".to_string()];
        args.extend(self.project_args());
        args.push(format!("{name}/{path}"));
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
        let mut args = vec!["file".to_string(), "pull".to_string()];
        args.extend(self.project_args());
        args.extend([format!("{name}/{path}"), "-".to_string()]);

        let output = self
            .cmd()
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
