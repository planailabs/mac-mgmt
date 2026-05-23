use anyhow::Result;
use russh::keys::{PublicKey, parse_public_key_base64};
use russh::server::{Auth, Handler, Msg, Session};
use russh::{Channel, ChannelId, MethodKind, MethodSet};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Mutex;

use super::pty;

pub struct SshSession {
    authorized_keys: Vec<PublicKey>,
    channels: Arc<Mutex<HashMap<ChannelId, ChannelState>>>,
}

struct ChannelState {
    pty_pair: pty::PtyPair,
}

impl SshSession {
    pub fn new(authorized_keys: Vec<PublicKey>) -> Self {
        Self {
            authorized_keys,
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn reject_with_pubkey() -> Auth {
    Auth::Reject {
        proceed_with_methods: Some(MethodSet::from(&[MethodKind::PublicKey][..])),
        partial_success: false,
    }
}

impl Handler for SshSession {
    type Error = anyhow::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        for key in &self.authorized_keys {
            if key == public_key {
                tracing::info!("public key auth succeeded");
                return Ok(Auth::Accept);
            }
        }
        tracing::warn!("public key auth rejected");
        Ok(reject_with_pubkey())
    }

    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        Ok(reject_with_pubkey())
    }

    async fn auth_password(&mut self, _user: &str, _password: &str) -> Result<Auth, Self::Error> {
        Ok(reject_with_pubkey())
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel_id: ChannelId,
        term: &str,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        tracing::info!("PTY request: {term} {col_width}x{row_height}");
        let (reader, pty_pair) = pty::spawn_shell(col_width, row_height, term)?;

        let channels = Arc::clone(&self.channels);
        channels
            .lock()
            .await
            .insert(channel_id, ChannelState { pty_pair });

        // Spawn PTY → SSH channel reader.
        // The OwnedReadPty owns the read half of the PTY fd — it is closed
        // when this task exits, leaving the write half (in ChannelState) intact.
        let handle = session.handle();
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(reader);
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = bytes::Bytes::copy_from_slice(&buf[..n]);
                        if handle.data(channel_id, data).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::debug!("PTY read error: {e}");
                        break;
                    }
                }
            }
            let _ = handle.close(channel_id).await;
        });

        session.request_success();
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        tracing::info!("shell request on channel {channel_id:?}");
        session.request_success();
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).to_string();
        tracing::info!(
            "exec request on channel {channel_id:?}: <{} bytes>",
            data.len()
        );

        let handle = session.handle();
        tokio::spawn(async move {
            let result = Command::new("bash").args(["-c", &command]).output().await;

            match result {
                Ok(output) => {
                    if !output.stdout.is_empty() {
                        let _ = handle
                            .data(channel_id, bytes::Bytes::from(output.stdout))
                            .await;
                    }
                    if !output.stderr.is_empty() {
                        let _ = handle
                            .extended_data(channel_id, 1, bytes::Bytes::from(output.stderr))
                            .await;
                    }
                    let code = output.status.code().unwrap_or(1) as u32;
                    let _ = handle.exit_status_request(channel_id, code).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                }
                Err(e) => {
                    let msg = format!("exec failed: {e}\n");
                    let _ = handle
                        .extended_data(channel_id, 1, bytes::Bytes::from(msg))
                        .await;
                    let _ = handle.exit_status_request(channel_id, 1).await;
                    let _ = handle.eof(channel_id).await;
                    let _ = handle.close(channel_id).await;
                }
            }
        });

        session.request_success();
        Ok(())
    }

    async fn data(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut channels = self.channels.lock().await;
        if let Some(state) = channels.get_mut(&channel_id) {
            let _ = state.pty_pair.writer.write_all(data).await;
        }
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel_id: ChannelId,
        col_width: u32,
        row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let channels = self.channels.lock().await;
        if let Some(state) = channels.get(&channel_id) {
            pty::resize(&state.pty_pair.writer, col_width, row_height);
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel_id: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let mut channels = self.channels.lock().await;
        if let Some(mut state) = channels.remove(&channel_id) {
            let _ = state.pty_pair.child.kill().await;
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channel_close(channel_id, session).await
    }
}

pub fn load_authorized_keys() -> Vec<PublicKey> {
    let path = super::host_keys::authorized_keys_path();
    if !path.exists() {
        tracing::warn!("no authorized_keys file at {}", path.display());
        return Vec::new();
    }

    let mut keys = Vec::new();
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("failed to read authorized_keys: {e}");
            return Vec::new();
        }
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let b64 = match line.split_whitespace().nth(1) {
            Some(b) => b,
            None => continue,
        };
        match parse_public_key_base64(b64) {
            Ok(key) => keys.push(key),
            Err(e) => tracing::warn!("skipping invalid key line: {e}"),
        }
    }

    tracing::info!("loaded {} authorized keys", keys.len());
    keys
}
