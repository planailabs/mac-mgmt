//! Control protocol types for the persistent RPC stream between
//! daemon and relay. Proxy, metrics, file, and shell requests now
//! use tunnel substreams — only registration and tunnel advertisement
//! remain on the RPC stream.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlRequest {
    /// Register with the relay node.
    Register {
        instance_id: String,
        hostname: Option<String>,
        agent_name: Option<String>,
    },
    /// Advertise tunnel definitions to the relay.
    TunnelAdvertisement {
        tunnels: serde_json::Value,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlResponse {
    Ok,
    Error { message: String },
}
