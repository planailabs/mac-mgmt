/// Bridge bidirectional data between two tokio AsyncRead+AsyncWrite streams.
/// Used for TCP-to-libp2p substream bridging.
#[allow(dead_code)] // Infrastructure for SSH tunnel bridging
pub async fn bridge_bidir<A, B>(a: A, b: B)
where
    A: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    B: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut ar, mut aw) = tokio::io::split(a);
    let (mut br, mut bw) = tokio::io::split(b);

    let a_to_b = tokio::io::copy(&mut ar, &mut bw);
    let b_to_a = tokio::io::copy(&mut br, &mut aw);

    tokio::select! {
        r = a_to_b => {
            if let Err(e) = r {
                tracing::debug!("bridge a→b ended: {e}");
            }
        }
        r = b_to_a => {
            if let Err(e) = r {
                tracing::debug!("bridge b→a ended: {e}");
            }
        }
    }
}
