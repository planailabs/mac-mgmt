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
        let _: crate::ClusterConfig = serde_json::from_value(cfg)
            .expect("migrated config must parse as ClusterConfig");
    }
}
