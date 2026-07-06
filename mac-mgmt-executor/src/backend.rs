use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

use crate::types::{ExecOutput, LaunchSpec, OsImage};

#[async_trait]
pub trait IncusBackend: Send + Sync {
    /// Launch an ephemeral container from the given image alias.
    /// Blocks until the container reaches "Running" state.
    async fn launch(&self, image: &str, name: &str) -> Result<()>;

    /// Launch an instance from a full [`LaunchSpec`] (remote/OCI image,
    /// cloud-init config, profiles, instance type). Blocks until Running.
    async fn launch_ext(&self, spec: &LaunchSpec) -> Result<()>;

    /// Execute a shell command inside a running container.
    async fn exec(&self, name: &str, command: &str, timeout: Duration) -> Result<ExecOutput>;

    /// Force-delete a container (stops it first if running).
    async fn delete(&self, name: &str) -> Result<()>;

    /// Get the status of a container (e.g. "Running", "Stopped").
    /// Returns None if the container does not exist.
    async fn status(&self, name: &str) -> Result<Option<String>>;

    /// List available OS images from the image server.
    async fn image_list(&self) -> Result<Vec<OsImage>>;

    /// Write content to a file inside a container.
    async fn file_push(&self, name: &str, path: &str, content: &[u8]) -> Result<()>;

    /// Read a file from inside a container.
    async fn file_pull(&self, name: &str, path: &str) -> Result<String>;
}
