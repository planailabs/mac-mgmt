//! Shared form-side representation of a `rollout_health::HealthGate`.
//!
//! Rendered by both the rollout-create form and the per-stage edit
//! panel on the rollout detail page. Serialises to the same JSON shape
//! `rollout_stages.health_gate` already stores so the evaluator can
//! round-trip without a translation layer.
//!
//! `probe_thresholds` is a `Vec<(String, u8)>` rather than a HashMap so
//! the UI can render insertion-ordered rows; `to_json()` drops empty
//! service names and clamps percentages to 0..=100 before writing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthGateInput {
    pub enabled: bool,
    pub min_heartbeat_fresh_pct: u8,
    pub heartbeat_freshness_secs: u32,
    pub grace_period_secs: u32,
    pub probe_thresholds: Vec<(String, u8)>,
}

impl Default for HealthGateInput {
    fn default() -> Self {
        Self {
            enabled: true,
            min_heartbeat_fresh_pct: 95,
            heartbeat_freshness_secs: 180,
            grace_period_secs: 600,
            probe_thresholds: vec![
                ("openclaw".into(), 90),
                ("ollama".into(), 90),
            ],
        }
    }
}

impl HealthGateInput {
    /// Convert the form state to the JSON shape stored in
    /// `rollout_stages.health_gate`. Returns `None` when the gate is
    /// toggled off — callers should write SQL NULL to mean "no gate".
    pub fn to_json(&self) -> Option<serde_json::Value> {
        if !self.enabled {
            return None;
        }
        let mut probe_map = serde_json::Map::new();
        for (svc, pct) in &self.probe_thresholds {
            let svc = svc.trim();
            if svc.is_empty() {
                continue;
            }
            probe_map.insert(
                svc.to_string(),
                serde_json::Value::Number((*pct).min(100).into()),
            );
        }
        Some(serde_json::json!({
            "min_heartbeat_fresh_pct": self.min_heartbeat_fresh_pct.min(100),
            "heartbeat_freshness_secs": self.heartbeat_freshness_secs.max(10),
            "min_probe_ok_pct": serde_json::Value::Object(probe_map),
            "grace_period_secs": self.grace_period_secs,
        }))
    }

    /// Parse a `rollout_stages.health_gate` JSON back into form state.
    /// Used by the edit panel to pre-populate with the stage's current
    /// config. Missing fields fall back to `Default`.
    pub fn from_json(v: &serde_json::Value) -> Self {
        let default = Self::default();
        let min_heartbeat_fresh_pct = v
            .get("min_heartbeat_fresh_pct")
            .and_then(|x| x.as_u64())
            .map(|n| n.min(100) as u8)
            .unwrap_or(default.min_heartbeat_fresh_pct);
        let heartbeat_freshness_secs = v
            .get("heartbeat_freshness_secs")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32)
            .unwrap_or(default.heartbeat_freshness_secs);
        let grace_period_secs = v
            .get("grace_period_secs")
            .and_then(|x| x.as_u64())
            .map(|n| n as u32)
            .unwrap_or(default.grace_period_secs);
        // Sort thresholds alphabetically so a reload of the editor
        // doesn't shuffle rows (HashMap → Vec).
        let mut probe_thresholds: Vec<(String, u8)> = v
            .get("min_probe_ok_pct")
            .and_then(|x| x.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, val)| val.as_u64().map(|n| (k.clone(), n.min(100) as u8)))
                    .collect()
            })
            .unwrap_or_default();
        probe_thresholds.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            enabled: true,
            min_heartbeat_fresh_pct,
            heartbeat_freshness_secs,
            grace_period_secs,
            probe_thresholds,
        }
    }
}
