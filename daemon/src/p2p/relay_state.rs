//! Relay connection state machine.
//!
//! Tracks the lifecycle of the daemon's connection to the relay node:
//! `Disabled → Disconnected → Connected → Identified → Registered`.
//!
//! Driven by swarm events. Returns actions for the swarm loop to execute.

use std::time::{Duration, Instant};

use libp2p::{Multiaddr, PeerId};

use super::rpc::RpcStream;

/// Backoff between reconnection attempts.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(30);

/// Periodic re-registration interval (handles relay restarts).
const REREGISTER_INTERVAL: Duration = Duration::from_secs(60);

/// Relay connection lifecycle.
pub enum RelayState {
    /// No relay configured.
    Disabled,
    /// Waiting to connect.
    Disconnected {
        relay_addr: Multiaddr,
        next_dial: Instant,
    },
    /// libp2p connection established, waiting for Identify.
    Connected {
        relay_addr: Multiaddr,
        peer_id: PeerId,
    },
    /// Identify received, proxy_url parsed. Opening RPC stream.
    Identified {
        relay_addr: Multiaddr,
        peer_id: PeerId,
        proxy_url: Option<String>,
    },
    /// Fully operational — RPC stream open, registered.
    Registered {
        relay_addr: Multiaddr,
        peer_id: PeerId,
        proxy_url: Option<String>,
        rpc: RpcStream,
        last_register: Instant,
    },
}

/// Events that drive the state machine.
pub enum RelayEvent {
    /// A libp2p connection was established.
    ConnectionEstablished { peer_id: PeerId },
    /// Identify protocol completed.
    Identified {
        peer_id: PeerId,
        agent_version: String,
    },
    /// The connection to a peer was closed.
    ConnectionClosed { peer_id: PeerId },
    /// Periodic tick (for reconnect / re-register).
    Tick,
    /// An RPC stream was successfully opened to the relay.
    RpcStreamOpened { stream: libp2p::Stream },
    /// RPC stream open failed.
    RpcStreamFailed { error: String },
}

/// Actions the swarm loop should execute after a state transition.
pub enum RelayAction {
    /// Dial the relay multiaddress.
    Dial(Multiaddr),
    /// Open an RPC stream to the relay peer.
    OpenRpcStream(PeerId),
    /// Update the shared relay_proxy_url.
    SetProxyUrl(Option<String>),
    /// Authorize this peer for control requests.
    AuthorizePeer(PeerId),
    /// Remove authorization for this peer.
    DeauthorizePeer(PeerId),
    /// Log a state transition.
    Log(tracing::Level, String),
}

impl RelayState {
    /// Create a new state machine.
    pub fn new(relay_addr: Option<Multiaddr>) -> Self {
        match relay_addr {
            Some(addr) => RelayState::Disconnected {
                relay_addr: addr,
                next_dial: Instant::now(),
            },
            None => RelayState::Disabled,
        }
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        match self {
            RelayState::Connected { peer_id, .. }
            | RelayState::Identified { peer_id, .. }
            | RelayState::Registered { peer_id, .. } => Some(*peer_id),
            _ => None,
        }
    }

    pub fn proxy_url(&self) -> Option<&str> {
        match self {
            RelayState::Identified { proxy_url, .. } | RelayState::Registered { proxy_url, .. } => {
                proxy_url.as_deref()
            }
            _ => None,
        }
    }

    pub fn is_registered(&self) -> bool {
        matches!(self, RelayState::Registered { .. })
    }

    /// Get a mutable reference to the RPC stream (if registered).
    pub fn rpc(&mut self) -> Option<&mut RpcStream> {
        match self {
            RelayState::Registered { rpc, .. } => Some(rpc),
            _ => None,
        }
    }

    /// Handle an event and return actions.
    pub fn handle_event(&mut self, event: RelayEvent) -> Vec<RelayAction> {
        let mut actions = Vec::new();

        match event {
            RelayEvent::ConnectionEstablished { peer_id } => {
                // A daemon can discover and connect to non-relay peers via mDNS
                // while it is still trying to dial the configured relay. Do not
                // claim the first arbitrary connection as the relay; wait for
                // Identify and only transition when the peer advertises the
                // `mac-mgmt-relay/` agent string.
                if self.peer_id() == Some(peer_id) {
                    actions.push(RelayAction::Log(
                        tracing::Level::INFO,
                        format!("relay connected: {peer_id}"),
                    ));
                }
            }

            RelayEvent::Identified {
                peer_id,
                agent_version,
            } => {
                // Only process if this is our relay peer.
                let is_relay = agent_version.starts_with("mac-mgmt-relay/");
                let is_our_peer = self.peer_id() == Some(peer_id)
                    || matches!(self, RelayState::Disconnected { .. });

                if is_relay && is_our_peer {
                    let proxy_url = super::parse_relay_proxy_url(&agent_version);
                    let relay_addr = match self {
                        RelayState::Connected { relay_addr, .. } => relay_addr.clone(),
                        RelayState::Disconnected { relay_addr, .. } => relay_addr.clone(),
                        _ => return actions,
                    };

                    actions.push(RelayAction::SetProxyUrl(proxy_url.clone()));
                    actions.push(RelayAction::AuthorizePeer(peer_id));
                    actions.push(RelayAction::OpenRpcStream(peer_id));

                    *self = RelayState::Identified {
                        relay_addr,
                        peer_id,
                        proxy_url,
                    };
                    actions.push(RelayAction::Log(
                        tracing::Level::INFO,
                        format!("relay identified: {peer_id}"),
                    ));
                }
            }

            RelayEvent::RpcStreamOpened { stream } => {
                if let RelayState::Identified {
                    relay_addr,
                    peer_id,
                    proxy_url,
                } = self
                {
                    let addr = relay_addr.clone();
                    let url = proxy_url.clone();
                    let pid = *peer_id;
                    *self = RelayState::Registered {
                        relay_addr: addr,
                        peer_id: pid,
                        proxy_url: url,
                        rpc: RpcStream::new(stream),
                        last_register: Instant::now() - REREGISTER_INTERVAL, // trigger immediate registration
                    };
                    actions.push(RelayAction::Log(
                        tracing::Level::INFO,
                        "RPC stream opened, ready to register".into(),
                    ));
                }
            }

            RelayEvent::RpcStreamFailed { error } => {
                actions.push(RelayAction::Log(
                    tracing::Level::WARN,
                    format!("RPC stream open failed: {error}, will retry"),
                ));
                // Stay in Identified — the tick will retry.
            }

            RelayEvent::ConnectionClosed { peer_id } => {
                if self.peer_id() == Some(peer_id) {
                    let relay_addr = match self {
                        RelayState::Connected { relay_addr, .. }
                        | RelayState::Identified { relay_addr, .. }
                        | RelayState::Registered { relay_addr, .. } => relay_addr.clone(),
                        _ => return actions,
                    };

                    actions.push(RelayAction::DeauthorizePeer(peer_id));
                    actions.push(RelayAction::SetProxyUrl(None));
                    actions.push(RelayAction::Log(
                        tracing::Level::INFO,
                        format!("relay disconnected: {peer_id}"),
                    ));

                    *self = RelayState::Disconnected {
                        relay_addr,
                        next_dial: Instant::now() + RECONNECT_INTERVAL,
                    };
                }
            }

            RelayEvent::Tick => match self {
                RelayState::Disconnected {
                    relay_addr,
                    next_dial,
                } => {
                    if Instant::now() >= *next_dial {
                        actions.push(RelayAction::Dial(relay_addr.clone()));
                        *next_dial = Instant::now() + RECONNECT_INTERVAL;
                    }
                }
                RelayState::Identified { peer_id, .. } => {
                    // Retry opening the RPC stream.
                    actions.push(RelayAction::OpenRpcStream(*peer_id));
                }
                RelayState::Registered { .. } => {
                    // Re-registration happens via the RPC stream, driven by the caller.
                }
                _ => {}
            },
        }

        actions
    }

    /// Check if it's time to re-register (called by the swarm loop).
    pub fn needs_reregister(&self) -> bool {
        matches!(self, RelayState::Registered { last_register, .. }
            if last_register.elapsed() >= REREGISTER_INTERVAL)
    }

    /// Mark that we just registered.
    pub fn mark_registered(&mut self) {
        if let RelayState::Registered { last_register, .. } = self {
            *last_register = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relay_addr() -> Multiaddr {
        "/dns4/relay.plan.ai/tcp/443/wss".parse().unwrap()
    }

    #[test]
    fn non_relay_connection_does_not_capture_relay_state() {
        let non_relay_peer = PeerId::random();
        let relay_peer = PeerId::random();
        let mut state = RelayState::new(Some(relay_addr()));

        let actions = state.handle_event(RelayEvent::ConnectionEstablished {
            peer_id: non_relay_peer,
        });
        assert!(actions.is_empty());
        assert!(matches!(state, RelayState::Disconnected { .. }));

        let actions = state.handle_event(RelayEvent::Identified {
            peer_id: non_relay_peer,
            agent_version: "mac-mgmt/0.1.5/some-instance".to_string(),
        });
        assert!(actions.is_empty());
        assert!(matches!(state, RelayState::Disconnected { .. }));

        let actions = state.handle_event(RelayEvent::Identified {
            peer_id: relay_peer,
            agent_version: "mac-mgmt-relay/0.1.5".to_string(),
        });
        assert!(matches!(state.peer_id(), Some(pid) if pid == relay_peer));
        assert!(matches!(state, RelayState::Identified { .. }));
        assert!(actions.iter().any(|action| matches!(
            action,
            RelayAction::AuthorizePeer(pid) if *pid == relay_peer
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            RelayAction::OpenRpcStream(pid) if *pid == relay_peer
        )));
    }
}
