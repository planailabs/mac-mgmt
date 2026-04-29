//! Transport stack: QUIC (primary) + WSS (for relay connections).

use anyhow::{Context, Result};
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::core::upgrade::Version;
use libp2p::identity::Keypair;
use libp2p::{PeerId, Transport, noise, yamux};

/// Build the daemon's transport: QUIC for direct peer connections,
/// plus WSS over TCP+DNS for connecting to the relay node (provides
/// TLS domain verification).
pub fn build(keypair: &Keypair) -> Result<Boxed<(PeerId, StreamMuxerBox)>> {
    // QUIC transport — primary, low-latency, multiplexed, encrypted.
    let quic_config = libp2p::quic::Config::new(keypair);
    let quic = libp2p::quic::tokio::Transport::new(quic_config);

    // TCP + DNS resolution (needed for WSS)
    let tcp = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default().nodelay(true));
    let dns_tcp = libp2p::dns::tokio::Transport::system(tcp)
        .context("failed to create DNS transport")?;

    // WSS over TCP+DNS — Noise + Yamux on top of WebSocket
    let wss = libp2p::websocket::Config::new(dns_tcp)
        .upgrade(Version::V1)
        .authenticate(noise::Config::new(keypair).context("noise config")?)
        .multiplex(yamux::Config::default())
        .map(|(peer, muxer), _| (peer, StreamMuxerBox::new(muxer)));

    // Combine: try QUIC first, fall back to WSS
    let transport = quic
        .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
        .or_transport(wss)
        .map(|either, _| either.into_inner())
        .boxed();

    Ok(transport)
}
