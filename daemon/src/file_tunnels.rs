use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use futures_util::{SinkExt, StreamExt};
use mac_mgmt_ws::tungstenite;

#[cfg(feature = "services")]
use crate::managed_service::{FileTunnel, FileTunnelDef};

/// Maximum file size for read/write operations (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Chunk size for streaming file content over WebSocket (1 MB).
const STREAM_CHUNK_SIZE: usize = 1024 * 1024;

// ── Registry ────────────────────────────────────────────────────────────

/// Tracks the current set of file tunnel definitions advertised by services.
#[cfg(feature = "services")]
pub struct FileTunnelRegistry {
    tunnels: HashMap<String, FileTunnel>,
}

/// Stub when services feature is disabled — the relay client still needs the type.
#[cfg(not(feature = "services"))]
pub struct FileTunnelRegistry;

#[cfg(not(feature = "services"))]
impl FileTunnelRegistry {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "services")]
impl FileTunnelRegistry {
    pub fn new() -> Self {
        Self {
            tunnels: HashMap::new(),
        }
    }

    pub fn update(&mut self, defs: Vec<FileTunnel>) {
        self.tunnels.clear();
        for d in defs {
            self.tunnels.insert(d.name().to_string(), d);
        }
    }

    pub fn get(&self, name: &str) -> Option<&FileTunnel> {
        self.tunnels.get(name)
    }
}

// ── Path resolution + security ──────────────────────────────────────────

/// Resolve the requested path within a file tunnel, canonicalize it, and
/// verify it falls within the tunnel's allowed root.
#[cfg(feature = "services")]
fn resolve_path(tunnel: &FileTunnel, relative_path: Option<&str>) -> Result<PathBuf, String> {
    let root = PathBuf::from(tunnel.path());

    let target = match (&tunnel.def, relative_path) {
        (FileTunnelDef::File { .. }, None | Some("")) => root.clone(),
        (FileTunnelDef::File { .. }, Some(_)) => {
            return Err("sub-paths not allowed for file tunnels".into());
        }
        (FileTunnelDef::Folder { .. }, None | Some("")) => root.clone(),
        (FileTunnelDef::Folder { .. }, Some(rel)) => {
            if rel.contains("..") {
                return Err("path traversal not allowed".into());
            }
            root.join(rel)
        }
    };

    let canonical = target
        .canonicalize()
        .map_err(|e| format!("path not found: {e}"))?;

    match &tunnel.def {
        FileTunnelDef::File { .. } => {
            let root_canonical = root
                .canonicalize()
                .map_err(|e| format!("tunnel root error: {e}"))?;
            if canonical != root_canonical {
                return Err("path does not match tunnel file".into());
            }
        }
        FileTunnelDef::Folder { .. } => {
            let root_canonical = root
                .canonicalize()
                .map_err(|e| format!("tunnel root error: {e}"))?;
            if !canonical.starts_with(&root_canonical) {
                return Err("path outside tunnel root".into());
            }
        }
    }

    Ok(canonical)
}

/// Check whether a filename matches the tunnel's include globs.
/// Returns `true` if `include` is `None` (all files allowed) or if the
/// filename matches at least one pattern.
#[cfg(feature = "services")]
fn matches_include(tunnel: &FileTunnel, filename: &str) -> bool {
    let FileTunnelDef::Folder { include, .. } = &tunnel.def else {
        return true;
    };
    let Some(patterns) = include else {
        return true;
    };
    for pat_str in patterns {
        if let Ok(pat) = glob::Pattern::new(pat_str) {
            if pat.matches(filename) {
                return true;
            }
        }
    }
    false
}

#[cfg(feature = "services")]
fn matches_allow_write(tunnel: &FileTunnel, filename: &str) -> bool {
    let FileTunnelDef::Folder { allow_write, .. } = &tunnel.def else {
        return true;
    };
    if allow_write.is_empty() {
        return true;
    }
    for pat_str in allow_write {
        if let Ok(pat) = glob::Pattern::new(pat_str) {
            if pat.matches(filename) {
                return true;
            }
        }
    }
    false
}

/// Matched validator with resolved command args.
#[cfg(feature = "services")]
struct MatchedValidator {
    builtin: Option<String>,
    command: Vec<String>,
}

/// Find the first matching validator for a filename.
#[cfg(feature = "services")]
fn find_validator(tunnel: &FileTunnel, file_path: &Path) -> Option<MatchedValidator> {
    let FileTunnelDef::Folder { validators, .. } = &tunnel.def else {
        return None;
    };
    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    for v in validators {
        if let Ok(pat) = glob::Pattern::new(&v.glob) {
            if pat.matches(filename) {
                let path_str = file_path.to_string_lossy();
                let cmd: Vec<String> = v
                    .command
                    .iter()
                    .map(|arg| arg.replace("{}", &path_str))
                    .collect();
                return Some(MatchedValidator {
                    builtin: v.builtin.clone(),
                    command: cmd,
                });
            }
        }
    }
    None
}

/// Get the mtime of a file as a Unix timestamp (seconds).
fn mtime_secs(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

// ── File list (control channel) ─────────────────────────────────────────

/// Handle a file list request. Returns `(status, body_json)`.
#[cfg(feature = "services")]
pub fn handle_list(tunnel: &FileTunnel, rel_path: Option<&str>) -> (u16, serde_json::Value) {
    let path = match resolve_path(tunnel, rel_path) {
        Ok(p) => p,
        Err(e) => return (400, serde_json::json!({ "error": e })),
    };

    if path.is_file() {
        // Single-file info
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => return (500, serde_json::json!({ "error": e.to_string() })),
        };
        let mtime = mtime_secs(&path).unwrap_or(0);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        return (
            200,
            serde_json::json!({
                "entries": [{
                    "name": name,
                    "kind": "file",
                    "size": meta.len(),
                    "mtime": mtime,
                }]
            }),
        );
    }

    if path.is_dir() {
        let entries = match std::fs::read_dir(&path) {
            Ok(rd) => rd,
            Err(e) => return (500, serde_json::json!({ "error": e.to_string() })),
        };
        let mut result = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let kind = if meta.is_dir() { "directory" } else { "file" };
            // Apply include filter for files in directory tunnels
            if kind == "file" && !matches_include(tunnel, &name) {
                continue;
            }
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            result.push(serde_json::json!({
                "name": name,
                "kind": kind,
                "size": meta.len(),
                "mtime": mtime,
            }));
        }
        return (200, serde_json::json!({ "entries": result }));
    }

    (404, serde_json::json!({ "error": "path not found" }))
}

// ── File read (data session) ────────────────────────────────────────────

/// Handle a file read via a dedicated data WebSocket session.
/// Protocol:
/// 1. Daemon sends text `{ status, size, mtime }` (file metadata)
/// 2. Daemon sends binary chunks (raw file bytes, ≤ STREAM_CHUNK_SIZE)
/// 3. Daemon closes WS
#[cfg(feature = "services")]
pub async fn handle_read_session(
    tunnel: &FileTunnel,
    rel_path: Option<&str>,
    ws: mac_mgmt_ws::ClientWs,
) {
    let (mut sink, _stream) = ws.split();

    macro_rules! send_error {
        ($status:expr, $error:expr) => {{
            let msg = serde_json::json!({ "status": $status, "error": $error });
            let _ = sink.send(tungstenite::Message::Text(msg.to_string().into())).await;
            let _ = sink.send(tungstenite::Message::Close(None)).await;
            return;
        }};
    }

    let path = match resolve_path(tunnel, rel_path) {
        Ok(p) => p,
        Err(e) => send_error!(400, e),
    };

    // For directory tunnels, verify the file passes the include filter
    if matches!(&tunnel.def, FileTunnelDef::Folder { .. }) {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !matches_include(tunnel, filename) {
            send_error!(403, "file not included in tunnel filter");
        }
    }

    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) => send_error!(404, format!("file not found: {e}")),
    };

    if !meta.is_file() {
        send_error!(400, "path is not a file");
    }

    let size = meta.len();
    let mtime = mtime_secs(&path).unwrap_or(0);

    // Send metadata header
    let header = serde_json::json!({ "status": 200, "size": size, "mtime": mtime });
    if sink
        .send(tungstenite::Message::Text(header.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    // Stream file content in chunks
    let mut file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("file read session: open failed: {e}");
            let _ = sink.send(tungstenite::Message::Close(None)).await;
            return;
        }
    };

    use std::io::Read;
    let mut buf = vec![0u8; STREAM_CHUNK_SIZE];
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                tracing::warn!("file read session: read failed: {e}");
                break;
            }
        };
        if sink
            .send(tungstenite::Message::Binary(buf[..n].to_vec().into()))
            .await
            .is_err()
        {
            return;
        }
    }

    let _ = sink.send(tungstenite::Message::Close(None)).await;
    tracing::debug!("file read session completed: {}", path.display());
}

// ── File write (data session) ───────────────────────────────────────────

/// Handle a file write via a dedicated data WebSocket session.
/// Protocol:
/// 1. Daemon sends text `{ "ready": true }` to signal readiness
/// 2. Client sends binary chunks (file content)
/// 3. Client sends text `"end_request"` to signal completion
/// 4. Daemon validates + commits, sends text `{ status, mtime }` or `{ status, error }`
/// 5. Daemon closes WS
#[cfg(feature = "services")]
pub async fn handle_write_session(
    tunnel: &FileTunnel,
    rel_path: Option<&str>,
    expected_mtime: Option<i64>,
    ws: mac_mgmt_ws::ClientWs,
) {
    let (mut sink, mut stream) = ws.split();

    macro_rules! send_result {
        ($result:expr) => {{
            let _ = sink
                .send(tungstenite::Message::Text($result.to_string().into()))
                .await;
            let _ = sink.send(tungstenite::Message::Close(None)).await;
            return;
        }};
    }

    // Pre-flight checks
    if !tunnel.writable() {
        send_result!(serde_json::json!({ "status": 403, "error": "tunnel is read-only" }));
    }

    let path = match resolve_path(tunnel, rel_path) {
        Ok(p) => p,
        Err(e) => send_result!(serde_json::json!({ "status": 400, "error": e })),
    };

    // For directory tunnels, verify the file passes the include filter
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if matches!(&tunnel.def, FileTunnelDef::Folder { .. }) {
        if !matches_include(tunnel, filename) {
            send_result!(
                serde_json::json!({ "status": 403, "error": "file not included in tunnel filter" })
            );
        }
    }

    if !matches_allow_write(tunnel, filename) {
        send_result!(
            serde_json::json!({ "status": 403, "error": "file not allowed by write filter" })
        );
    }

    // Optimistic concurrency check
    if let Some(expected) = expected_mtime {
        if let Some(actual) = mtime_secs(&path) {
            if actual != expected {
                send_result!(serde_json::json!({
                    "status": 409,
                    "error": "file modified since last read",
                    "conflict_mtime": actual,
                }));
            }
        }
    }

    // Signal readiness
    if sink
        .send(tungstenite::Message::Text(
            serde_json::json!({ "ready": true }).to_string().into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    // Receive file content into a temp file
    let tmp_path = path.with_extension("tmp.file-tunnel");
    let mut total_bytes: u64 = 0;
    {
        let mut tmp_file = match std::fs::File::create(&tmp_path) {
            Ok(f) => f,
            Err(e) => {
                send_result!(
                    serde_json::json!({ "status": 500, "error": format!("failed to create temp file: {e}") })
                );
            }
        };

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(tungstenite::Message::Binary(data)) => {
                    total_bytes += data.len() as u64;
                    if total_bytes > MAX_FILE_SIZE {
                        let _ = std::fs::remove_file(&tmp_path);
                        send_result!(
                            serde_json::json!({ "status": 413, "error": "file too large" })
                        );
                    }
                    if let Err(e) = tmp_file.write_all(&data) {
                        let _ = std::fs::remove_file(&tmp_path);
                        send_result!(
                            serde_json::json!({ "status": 500, "error": format!("write failed: {e}") })
                        );
                    }
                }
                Ok(tungstenite::Message::Text(t)) if &*t == "end_request" => break,
                Ok(tungstenite::Message::Close(_)) | Err(_) => {
                    let _ = std::fs::remove_file(&tmp_path);
                    return;
                }
                _ => {}
            }
        }

        if let Err(e) = tmp_file.flush() {
            let _ = std::fs::remove_file(&tmp_path);
            send_result!(
                serde_json::json!({ "status": 500, "error": format!("flush failed: {e}") })
            );
        }
    }

    // Back up original file (if it exists)
    let backup_path = path.with_extension("bak.file-tunnel");
    let had_original = path.exists();
    if had_original {
        if let Err(e) = std::fs::copy(&path, &backup_path) {
            let _ = std::fs::remove_file(&tmp_path);
            send_result!(
                serde_json::json!({ "status": 500, "error": format!("backup failed: {e}") })
            );
        }
    }

    // Atomic rename temp → target
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        let _ = std::fs::remove_file(&tmp_path);
        send_result!(serde_json::json!({ "status": 500, "error": format!("rename failed: {e}") }));
    }

    // Run validator if one matches
    if let Some(validator) = find_validator(tunnel, &path) {
        let rollback = || {
            if had_original {
                let _ = std::fs::rename(&backup_path, &path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        };

        // 1. Run builtin validator first (if configured)
        if let Some(name) = &validator.builtin {
            if let Err(msg) = crate::managed_service::run_builtin_validator(name, &path) {
                tracing::warn!(
                    "builtin validation ({name}) failed for {}: {msg}",
                    path.display()
                );
                rollback();
                send_result!(
                    serde_json::json!({ "status": 422, "error": format!("validation failed: {msg}") })
                );
            }
            tracing::debug!("builtin validation ({name}) passed for {}", path.display());
        }

        // 2. Run external command validator (if configured).
        //    If the binary is missing, log a warning but don't fail — the
        //    builtin validator (if any) already passed.
        if !validator.command.is_empty() {
            let result = std::process::Command::new(&validator.command[0])
                .args(&validator.command[1..])
                .output();
            match result {
                Ok(output) if !output.status.success() => {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    tracing::warn!("command validation failed for {}: {stderr}", path.display());
                    rollback();
                    send_result!(
                        serde_json::json!({ "status": 422, "error": format!("validation failed: {stderr}") })
                    );
                }
                Err(e) if validator.builtin.is_some() => {
                    // Binary missing but builtin already passed — log and continue
                    tracing::warn!(
                        "validation command {:?} not available ({}), builtin passed — accepting write",
                        validator.command[0],
                        e
                    );
                }
                Err(e) => {
                    // No builtin fallback — this is fatal
                    rollback();
                    send_result!(
                        serde_json::json!({ "status": 500, "error": format!("validation command failed to run: {e}") })
                    );
                }
                Ok(_) => {
                    tracing::debug!("command validation passed for {}", path.display());
                }
            }
        }
    }

    // Clean up backup
    let _ = std::fs::remove_file(&backup_path);

    let new_mtime = mtime_secs(&path).unwrap_or(0);
    tracing::info!(
        "file write completed: {} ({total_bytes} bytes)",
        path.display()
    );
    // Success — send result and close (don't use send_result! macro since we don't want to return early)
    let _ = sink
        .send(tungstenite::Message::Text(
            serde_json::json!({ "status": 200, "mtime": new_mtime })
                .to_string()
                .into(),
        ))
        .await;
    let _ = sink.send(tungstenite::Message::Close(None)).await;
}
