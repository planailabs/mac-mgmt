use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use futures_util::{AsyncRead, AsyncWrite};

#[cfg(feature = "services")]
use crate::managed_service::{FileTunnel, FileTunnelDef};

/// Maximum file size for read/write operations (10 MB).
pub(crate) const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

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

    /// Serialize all file tunnels as JSON for relay advertisement.
    pub fn to_json(&self) -> Vec<serde_json::Value> {
        self.tunnels
            .values()
            .map(|ft| {
                let mut val = serde_json::json!({
                    "name": ft.name(),
                    "service": ft.service,
                    "path": ft.path(),
                    "writable": ft.writable(),
                    "description": ft.description(),
                });
                if let FileTunnelDef::Folder { include, .. } = &ft.def {
                    val.as_object_mut().unwrap().insert("kind".into(), "directory".into());
                    val.as_object_mut().unwrap().insert(
                        "include".into(),
                        serde_json::to_value(include).unwrap_or(serde_json::Value::Null),
                    );
                } else {
                    val.as_object_mut().unwrap().insert("kind".into(), "file".into());
                }
                val
            })
            .collect()
    }
}

// ── Path resolution + security ──────────────────────────────────────────

/// Resolve the requested path within a file tunnel, canonicalize it, and
/// verify it falls within the tunnel's allowed root.
#[cfg(feature = "services")]
pub(crate) fn resolve_path(tunnel: &FileTunnel, relative_path: Option<&str>) -> Result<PathBuf, String> {
    let root = PathBuf::from(tunnel.path());

    // Strip leading slashes — LLMs often hallucinate them in relative paths.
    // Also strip the tunnel's root path prefix if the caller passed a full path
    // (e.g. "/etc/ollama/models.json" when the tunnel root is "/etc/ollama").
    let relative_path = relative_path.map(|p| {
        let p = p.trim_start_matches('/');
        let root_str = tunnel.path().trim_start_matches('/');
        p.strip_prefix(root_str)
            .map(|rest| rest.trim_start_matches('/'))
            .unwrap_or(p)
    });

    let target = match (&tunnel.def, relative_path) {
        (FileTunnelDef::File { .. }, None | Some("")) => root.clone(),
        (FileTunnelDef::File { .. }, Some(name)) => {
            // Allow the file's own name — the list endpoint returns it and
            // agents naturally pass it back as the path to read/write.
            let file_name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if name == file_name {
                root.clone()
            } else {
                return Err("sub-paths not allowed for file tunnels".into());
            }
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
pub(crate) fn matches_include(tunnel: &FileTunnel, filename: &str) -> bool {
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
pub(crate) fn matches_allow_write(tunnel: &FileTunnel, filename: &str) -> bool {
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

/// Find the first [`Validator`] whose pattern matches the file's path
/// relative to the tunnel root.  This ensures patterns like `openclaw.json`
/// only match at the tunnel root, not in subdirectories.
#[cfg(feature = "services")]
fn find_validator<'a>(tunnel: &'a FileTunnel, file_path: &Path) -> Option<&'a crate::validator::Validator> {
    let FileTunnelDef::Folder { validators, .. } = &tunnel.def else {
        return None;
    };
    let root = std::path::Path::new(tunnel.path());
    let rel = file_path.strip_prefix(root).unwrap_or(file_path);
    crate::validator::find_matching(validators, rel)
}

/// Get the mtime of a file as a Unix timestamp (seconds).
pub(crate) fn mtime_secs(path: &Path) -> Option<i64> {
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

// ── Core read/write operations ──────────────────────────────────────────

/// Read a file from a tunnel. Returns `(content, mtime)` on success.
/// Error tuple is `(HTTP status code, error message)`.
#[cfg(feature = "services")]
pub(crate) fn read_file(
    tunnel: &FileTunnel,
    rel_path: Option<&str>,
) -> Result<(Vec<u8>, Option<i64>), (u16, String)> {
    let path = resolve_path(tunnel, rel_path).map_err(|e| (400u16, e))?;

    if matches!(&tunnel.def, FileTunnelDef::Folder { .. }) {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !matches_include(tunnel, filename) {
            return Err((403, "file not included in tunnel filter".into()));
        }
    }

    let meta = std::fs::metadata(&path).map_err(|e| (404u16, format!("file not found: {e}")))?;
    if !meta.is_file() {
        return Err((400, "path is not a file".into()));
    }

    let mtime = mtime_secs(&path);
    let content = std::fs::read(&path).map_err(|e| (500u16, format!("read failed: {e}")))?;
    Ok((content, mtime))
}

/// Write content to a file tunnel. Returns new mtime on success.
/// Performs atomic write with backup + validation + rollback.
/// Error tuple is `(HTTP status code, error message)`.
#[cfg(feature = "services")]
pub(crate) fn write_file(
    tunnel: &FileTunnel,
    rel_path: Option<&str>,
    content: &[u8],
    expected_mtime: Option<i64>,
) -> Result<i64, (u16, String)> {
    if !tunnel.writable() {
        return Err((403, "tunnel is read-only".into()));
    }

    let path = resolve_path(tunnel, rel_path).map_err(|e| (400u16, e))?;

    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if matches!(&tunnel.def, FileTunnelDef::Folder { .. }) {
        if !matches_include(tunnel, filename) {
            return Err((403, "file not included in tunnel filter".into()));
        }
    }

    if !matches_allow_write(tunnel, filename) {
        return Err((403, "file not allowed by write filter".into()));
    }

    if content.len() as u64 > MAX_FILE_SIZE {
        return Err((413, "file too large".into()));
    }

    // Optimistic concurrency check
    if let Some(expected) = expected_mtime {
        if let Some(actual) = mtime_secs(&path) {
            if actual != expected {
                return Err((
                    409,
                    format!("file modified since last read (expected mtime {expected}, actual {actual})"),
                ));
            }
        }
    }

    // Write content to temp file
    let tmp_path = path.with_extension("tmp.file-tunnel");
    std::fs::write(&tmp_path, content)
        .map_err(|e| {
            (500u16, format!("failed to create temp file: {e}"))
        })?;

    // Back up original file (if it exists)
    let backup_path = path.with_extension("bak.file-tunnel");
    let had_original = path.exists();
    if had_original {
        if let Err(e) = std::fs::copy(&path, &backup_path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err((500, format!("backup failed: {e}")));
        }
    }

    // Atomic rename temp → target
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err((500, format!("rename failed: {e}")));
    }

    // Run validator if one matches.
    if let Some(validator) = find_validator(tunnel, &path) {
        if let Err(msg) = validator.validate_file(&path) {
            tracing::warn!("validation failed for {}: {msg}", path.display());
            // Rollback: restore original or remove newly created file.
            if had_original {
                let _ = std::fs::rename(&backup_path, &path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
            return Err((422, format!("validation failed: {msg}")));
        }
    }

    // Clean up backup
    let _ = std::fs::remove_file(&backup_path);

    let new_mtime = mtime_secs(&path).unwrap_or(0);
    tracing::info!(
        "file write completed: {} ({} bytes)",
        path.display(),
        content.len()
    );
    Ok(new_mtime)
}

// ── File read (data session) ────────────────────────────────────────────

/// Handle a file read via a data stream session.
///
/// Protocol (length-prefixed framing):
/// 1. Daemon sends JSON `{ status, size, mtime }` (file metadata)
/// 2. Daemon sends binary chunks (raw file bytes, ≤ STREAM_CHUNK_SIZE)
/// 3. Daemon sends end-of-stream marker
#[cfg(feature = "services")]
pub async fn handle_read_session<S>(
    tunnel: &FileTunnel,
    rel_path: Option<&str>,
    stream: &mut S,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    use mac_mgmt_common::framing as stream_framing;

    let (content, mtime) = match read_file(tunnel, rel_path) {
        Ok(result) => result,
        Err((status, error)) => {
            let msg = serde_json::json!({ "status": status, "error": error });
            let _ = stream_framing::write_json(stream, &msg).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        }
    };

    let size = content.len();
    let header = serde_json::json!({ "status": 200, "size": size, "mtime": mtime.unwrap_or(0) });
    if stream_framing::write_json(stream, &header).await.is_err() {
        return;
    }

    for chunk in content.chunks(STREAM_CHUNK_SIZE) {
        if stream_framing::write_binary(stream, chunk).await.is_err() {
            return;
        }
    }

    let _ = stream_framing::write_end(stream).await;
    tracing::debug!("file read session completed");
}

// ── File write (data session) ───────────────────────────────────────────

/// Handle a file write via a data stream session.
///
/// Protocol (length-prefixed framing):
/// 1. Daemon sends JSON `{ "ready": true }` to signal readiness
/// 2. Client sends binary chunks (file content)
/// 3. Client sends end-of-stream marker
/// 4. Daemon validates + commits, sends JSON `{ status, mtime }` or `{ status, error }`
/// 5. Daemon sends end-of-stream marker
#[cfg(feature = "services")]
pub async fn handle_write_session<S>(
    tunnel: &FileTunnel,
    rel_path: Option<&str>,
    expected_mtime: Option<i64>,
    stream: &mut S,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    use mac_mgmt_common::framing::{self as stream_framing, TaggedFrame};

    macro_rules! send_result {
        ($result:expr) => {{
            let _ = stream_framing::write_json(stream, &$result).await;
            let _ = stream_framing::write_end(stream).await;
            return;
        }};
    }

    // Signal readiness
    if stream_framing::write_json(stream, &serde_json::json!({ "ready": true }))
        .await
        .is_err()
    {
        return;
    }

    // Receive file content into memory
    let mut content = Vec::new();
    loop {
        match stream_framing::read_tagged_frame(stream).await {
            Ok(Some(TaggedFrame::Binary(data))) => {
                content.extend_from_slice(&data);
                if content.len() as u64 > MAX_FILE_SIZE {
                    send_result!(serde_json::json!({ "status": 413, "body": { "error": "file too large" } }));
                }
            }
            Ok(Some(TaggedFrame::End)) | Ok(None) => break,
            Ok(Some(TaggedFrame::Json(_))) => {
                // JSON during data phase = end signal (backwards compat)
                break;
            }
            Err(_) => return,
        }
    }

    // Write using shared core
    match write_file(tunnel, rel_path, &content, expected_mtime) {
        Ok(mtime) => {
            let _ = stream_framing::write_json(
                stream,
                &serde_json::json!({ "status": 200, "body": { "mtime": mtime } }),
            )
            .await;
            let _ = stream_framing::write_end(stream).await;
        }
        Err((status, error)) => {
            send_result!(serde_json::json!({ "status": status, "body": { "error": error } }));
        }
    }
}
