use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::ClientWs;

/// Adapts a [`ClientWs`] into `AsyncRead + AsyncWrite`.
///
/// Spawns two background tasks that bridge between the poll-based
/// I/O world and the async WebSocket stream/sink API. Binary frames
/// become reads; writes become binary frames.
pub struct WsStream {
    read_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
    write_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    read_buf: Vec<u8>,
    read_pos: usize,
}

impl WsStream {
    pub fn new(ws: ClientWs) -> Self {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let (read_tx, read_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
        let (write_tx, mut write_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let (mut ws_sink, mut ws_stream) = ws.split();

        // WS → read channel
        tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_stream.next().await {
                match msg {
                    Message::Binary(data) => {
                        if read_tx.send(data.to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        });

        // Write channel → WS
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_rx;
            loop {
                tokio::select! {
                    data = write_rx.recv() => {
                        match data {
                            Some(data) => {
                                if ws_sink.send(Message::Binary(data.into())).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = &mut shutdown_rx => {
                        let _ = ws_sink.close().await;
                        break;
                    }
                }
            }
        });

        Self {
            read_rx,
            write_tx,
            shutdown_tx: Some(shutdown_tx),
            read_buf: Vec::new(),
            read_pos: 0,
        }
    }
}

impl AsyncRead for WsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Drain buffered data first
        if self.read_pos < self.read_buf.len() {
            let remaining = &self.read_buf[self.read_pos..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            self.read_pos += n;
            if self.read_pos >= self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        match self.read_rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                let n = data.len().min(buf.remaining());
                buf.put_slice(&data[..n]);
                if n < data.len() {
                    self.read_buf = data[n..].to_vec();
                    self.read_pos = 0;
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Ok(())), // EOF
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for WsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.write_tx.try_send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "WS closed")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        Poll::Ready(Ok(()))
    }
}
