use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFilesParams {
    /// File tunnel name (e.g. "ollama-config")
    pub tunnel_name: String,
    /// Optional subdirectory path within the tunnel
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileParams {
    /// File tunnel name
    pub tunnel_name: String,
    /// Path within the tunnel to read
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteFileParams {
    /// File tunnel name
    pub tunnel_name: String,
    /// Path within the tunnel to write
    pub path: String,
    /// File content to write (text)
    pub content: String,
    /// Expected mtime from a previous read (for optimistic concurrency). Optional.
    #[serde(default)]
    pub expected_mtime: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunCommandParams {
    /// Shell command name (must be one of the predefined commands)
    pub command_name: String,
    /// Optional argument to pass to the command
    #[serde(default)]
    pub user_arg: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchLogsParams {
    /// Number of log lines to fetch. Defaults to 200.
    #[serde(default)]
    pub n: Option<usize>,
    /// Filter logs by service name (e.g. "ollama", "openclaw")
    #[serde(default)]
    pub service: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClusterInstanceParams {
    /// Instance ID prefix of another instance in the same cluster
    pub instance_prefix: String,
    /// Number of log lines to fetch. Defaults to 200.
    #[serde(default)]
    pub n: Option<usize>,
    /// Filter logs by service name
    #[serde(default)]
    pub service: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClusterCommandParams {
    /// Instance ID prefix of another instance in the same cluster
    pub instance_prefix: String,
    /// Shell command name
    pub command_name: String,
    /// Optional argument
    #[serde(default)]
    pub user_arg: Option<String>,
}
