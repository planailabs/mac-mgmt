//! Config migration system for JSON cluster/daemon configs.
//!
//! Migrations transform raw `serde_json::Value` configs *before* they are
//! deserialized into typed structs. Each migration is **idempotent** — safe
//! to run on configs that have already been migrated.
//!
//! ## Adding a new migration
//!
//! 1. Write a function `fn migrate_NNN(config: &mut Value)` that checks
//!    whether the transformation is needed and applies it.
//! 2. Append a call to it at the end of [`migrate`].
//! 3. Add tests covering both the "needs migration" and "already migrated"
//!    cases.

use serde_json::Value;

/// Apply all pending config migrations to a raw JSON config value.
///
/// Called by the server before returning configs to daemons, and by the
/// daemon when loading remote or local configs. Because every migration
/// is idempotent, calling this on an already-current config is a no-op.
pub fn migrate(config: &mut Value) {
    migrate_001_provider_fields(config);
    migrate_002_relay_libp2p(config);
}

// ── Migration 001 ─────────────────────────────────────────────────────
//
// Context: the `enabled` flag and `default_llm`/`default_agent` rename
// (commit 3aeb697).
//
// 1. Rename `global.llm_provider`  → `global.default_llm`
// 2. Rename `global.agent_provider` → `global.default_agent`
// 3. Wrap a bare `cloud` object `{…}` into a single-element array `[{…}]`

fn migrate_001_provider_fields(config: &mut Value) {
    // 1 & 2 — rename global fields
    if let Some(global) = config.get_mut("global").and_then(|g| g.as_object_mut()) {
        if let Some(v) = global.remove("llm_provider") {
            global.entry("default_llm").or_insert(v);
        }
        if let Some(v) = global.remove("agent_provider") {
            global.entry("default_agent").or_insert(v);
        }
    }

    // 3 — cloud: single object → array
    if let Some(cloud) = config.get_mut("cloud") {
        if cloud.is_object() {
            let obj = cloud.take(); // takes the object, leaves Null
            *cloud = Value::Array(vec![obj]);
        }
    }
}

// ── Migration 002 ─────────────────────────────────────────────────────
//
// Context: relay switched from custom WebSocket to libp2p (p2p).
//
// 1. Convert `relay.url` (ws:// or wss:// URL) to `relay.relay_multiaddr`
//    as a libp2p multiaddress.
// 2. Remove the old `relay.url` key.

fn migrate_002_relay_libp2p(config: &mut Value) {
    let Some(relay) = config.get_mut("relay").and_then(|r| r.as_object_mut()) else {
        return;
    };

    // Only migrate if `url` is present and `relay_multiaddr` is not already set.
    let has_multiaddr = relay
        .get("relay_multiaddr")
        .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
    if has_multiaddr {
        // Already migrated — just clean up old key if still present.
        relay.remove("url");
        return;
    }

    if let Some(url_val) = relay.remove("url") {
        if let Some(url) = url_val.as_str() {
            if let Some(multiaddr) = ws_url_to_multiaddr(url) {
                relay.insert(
                    "relay_multiaddr".to_string(),
                    Value::String(multiaddr),
                );
            }
        }
    }
}

/// Best-effort conversion of a WebSocket URL to a libp2p multiaddress.
///
/// Examples:
/// - `wss://relay.example.com`       → `/dns4/relay.example.com/tcp/443/wss`
/// - `wss://relay.example.com:4001`   → `/dns4/relay.example.com/tcp/4001/wss`
/// - `ws://localhost:8080`            → `/dns4/localhost/tcp/8080/ws`
fn ws_url_to_multiaddr(url: &str) -> Option<String> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("wss://") {
        ("wss", r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        ("ws", r)
    } else {
        return None;
    };

    // Strip path (we only care about host:port)
    let host_port = rest.split('/').next().unwrap_or(rest);
    let (host, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
        // Check if p is a valid port number (not part of an IPv6 address)
        if let Ok(port_num) = p.parse::<u16>() {
            (h, port_num)
        } else {
            (host_port, if scheme == "wss" { 443 } else { 80 })
        }
    } else {
        (host_port, if scheme == "wss" { 443 } else { 80 })
    };

    Some(format!("/dns4/{host}/tcp/{port}/{scheme}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn migrate_001_renames_global_fields() {
        let mut cfg = json!({
            "global": {
                "llm_provider": "ollama",
                "agent_provider": "openclaw"
            }
        });
        migrate(&mut cfg);
        assert_eq!(cfg["global"]["default_llm"], "ollama");
        assert_eq!(cfg["global"]["default_agent"], "openclaw");
        assert!(cfg["global"].get("llm_provider").is_none());
        assert!(cfg["global"].get("agent_provider").is_none());
    }

    #[test]
    fn migrate_001_does_not_overwrite_existing_new_fields() {
        let mut cfg = json!({
            "global": {
                "llm_provider": "lms",
                "default_llm": "cloud"
            }
        });
        migrate(&mut cfg);
        // default_llm already set → keep it, drop the old key
        assert_eq!(cfg["global"]["default_llm"], "cloud");
        assert!(cfg["global"].get("llm_provider").is_none());
    }

    #[test]
    fn migrate_001_wraps_cloud_object_in_array() {
        let mut cfg = json!({
            "cloud": {
                "provider": "anthropic",
                "api_key": "sk-test"
            }
        });
        migrate(&mut cfg);
        let cloud = cfg["cloud"].as_array().expect("should be array");
        assert_eq!(cloud.len(), 1);
        assert_eq!(cloud[0]["provider"], "anthropic");
    }

    #[test]
    fn migrate_001_leaves_cloud_array_alone() {
        let mut cfg = json!({
            "cloud": [{
                "provider": "anthropic"
            }]
        });
        let before = cfg.clone();
        migrate(&mut cfg);
        assert_eq!(cfg, before);
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut cfg = json!({
            "global": {
                "llm_provider": "ollama",
                "agent_provider": "none"
            },
            "cloud": {
                "provider": "openai"
            }
        });
        migrate(&mut cfg);
        let after_first = cfg.clone();
        migrate(&mut cfg);
        assert_eq!(cfg, after_first, "second run must be a no-op");
    }

    #[test]
    fn migrate_empty_config() {
        let mut cfg = json!({});
        migrate(&mut cfg);
        assert_eq!(cfg, json!({}));
    }

    #[test]
    fn migrate_no_global_section() {
        let mut cfg = json!({
            "ollama": { "enabled": true }
        });
        let before = cfg.clone();
        migrate(&mut cfg);
        assert_eq!(cfg, before);
    }

    #[test]
    fn migrated_config_parses_as_cluster_config() {
        let mut cfg = json!({
            "global": {
                "llm_provider": "cloud",
                "agent_provider": "openclaw"
            },
            "cloud": {
                "provider": "anthropic",
                "api_key": "sk-test",
                "default_model": "anthropic/claude-sonnet-4-6"
            }
        });
        migrate(&mut cfg);
        let _: crate::ClusterConfig =
            serde_json::from_value(cfg).expect("migrated config must parse as ClusterConfig");
    }

    // ── Migration 002 tests ──────────────────────────────────────────

    #[test]
    fn migrate_002_converts_wss_url_to_multiaddr() {
        let mut cfg = json!({
            "relay": {
                "url": "wss://relay.example.com"
            }
        });
        migrate(&mut cfg);
        assert_eq!(
            cfg["relay"]["relay_multiaddr"],
            "/dns4/relay.example.com/tcp/443/wss"
        );
        assert!(cfg["relay"].get("url").is_none());
    }

    #[test]
    fn migrate_002_converts_wss_url_with_port() {
        let mut cfg = json!({
            "relay": {
                "url": "wss://relay.example.com:4001"
            }
        });
        migrate(&mut cfg);
        assert_eq!(
            cfg["relay"]["relay_multiaddr"],
            "/dns4/relay.example.com/tcp/4001/wss"
        );
    }

    #[test]
    fn migrate_002_converts_ws_url() {
        let mut cfg = json!({
            "relay": {
                "url": "ws://localhost:8080"
            }
        });
        migrate(&mut cfg);
        assert_eq!(
            cfg["relay"]["relay_multiaddr"],
            "/dns4/localhost/tcp/8080/ws"
        );
    }

    #[test]
    fn migrate_002_does_not_overwrite_existing_multiaddr() {
        let mut cfg = json!({
            "relay": {
                "url": "wss://old.example.com",
                "relay_multiaddr": "/dns4/new.example.com/tcp/4001/wss"
            }
        });
        migrate(&mut cfg);
        assert_eq!(
            cfg["relay"]["relay_multiaddr"],
            "/dns4/new.example.com/tcp/4001/wss"
        );
        assert!(cfg["relay"].get("url").is_none());
    }

    #[test]
    fn migrate_002_no_relay_section() {
        let mut cfg = json!({
            "ollama": { "enabled": true }
        });
        let before = cfg.clone();
        migrate(&mut cfg);
        assert_eq!(cfg, before);
    }

    #[test]
    fn migrate_002_idempotent() {
        let mut cfg = json!({
            "relay": {
                "url": "wss://relay.example.com:4001"
            }
        });
        migrate(&mut cfg);
        let after_first = cfg.clone();
        migrate(&mut cfg);
        assert_eq!(cfg, after_first, "second run must be a no-op");
    }

    #[test]
    fn migrate_002_relay_config_parses() {
        let mut cfg = json!({
            "relay": {
                "url": "wss://relay.example.com",
                "remote_ssh_enabled": true,
                "tunnels_enabled": true,
                "fake_origin_local": true
            }
        });
        migrate(&mut cfg);
        let _: crate::ClusterConfig =
            serde_json::from_value(cfg).expect("migrated relay config must parse");
    }
}
