pub mod bridge;
#[cfg(feature = "client")]
mod connect;
#[cfg(feature = "client")]
mod reconnect;
#[cfg(feature = "client")]
mod stream;

#[cfg(feature = "client")]
pub use connect::{ClientWs, WsConnect};
#[cfg(feature = "client")]
pub use reconnect::{WsClientConfig, spawn_reconnecting};
#[cfg(feature = "client")]
pub use stream::WsStream;

/// Re-export tungstenite for direct access to `Message`, `CloseFrame`, etc.
#[cfg(feature = "client")]
pub use tokio_tungstenite::tungstenite;
