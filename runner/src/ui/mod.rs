//! HTML dashboard renderer.
//!
//! Templates live under `templates/` and are embedded at compile time
//! via `include_str!`. CSS and JS are passed in as raw sections so the
//! mustache template stays focused on structure.
//!
//! The view types map from [`crate::orchestrator`] snapshots to a
//! template-friendly shape (pre-formatted timestamps, pre-split
//! boolean health markers for mustache's no-else sections, etc).

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::orchestrator::{CellStatus, StatusSnapshot};

const INDEX_TEMPLATE: &str = include_str!("templates/index.mustache");
const STYLES: &str = include_str!("templates/styles.css");
const SCRIPT: &str = include_str!("templates/dashboard.js");

#[derive(Serialize)]
struct IndexView<'a> {
    styles: &'a str,
    script: &'a str,
    total_cells: usize,
    running: usize,
    paused: bool,
    cluster_sizes: String,
    agents: String,
    llms: String,
    cloud_providers_configured: usize,
    ollama_model: String,
    lms_model: String,
    runner_version: String,
    runner_git_sha_short: String,
    runner_git_sha: String,
    cells: Vec<CellView>,
}

#[derive(Serialize)]
struct CellView {
    key: String,
    stage: String,
    node_count: u32,
    cluster_id: Option<String>,
    instances: Vec<InstanceView>,
    has_instances: bool,
    /// Mustache uses `{{#truthy}}…{{/truthy}}` sections and has no
    /// if/else, so split healthy into three mutually-exclusive flags.
    healthy_yes: bool,
    healthy_no: bool,
    healthy_unknown: bool,
    detail: Option<String>,
    launching_since: Option<String>,
    deploy_failures: u32,
    fails_class: &'static str,
}

#[derive(Serialize)]
struct InstanceView {
    instance_name: String,
    instance_id: String,
    instance_id_short: String,
    stopped: bool,
    stopped_since: Option<String>,
}

pub fn render_index(snap: &StatusSnapshot) -> String {
    let short: String = snap.runner_git_sha.chars().take(12).collect();
    let view = IndexView {
        styles: STYLES,
        script: SCRIPT,
        total_cells: snap.total_cells,
        running: snap.running,
        paused: snap.paused,
        cluster_sizes: join_ints(&snap.matrix_axes.cluster_sizes),
        agents: snap.matrix_axes.agents.join(", "),
        llms: snap.matrix_axes.llms.join(", "),
        cloud_providers_configured: snap.matrix_axes.cloud_providers_configured,
        ollama_model: snap.matrix_axes.ollama_model.clone(),
        lms_model: snap.matrix_axes.lms_model.clone(),
        runner_version: snap.runner_version.clone(),
        runner_git_sha_short: short,
        runner_git_sha: snap.runner_git_sha.clone(),
        cells: snap.cells.iter().map(to_cell_view).collect(),
    };
    let template = match mustache::compile_str(INDEX_TEMPLATE) {
        Ok(t) => t,
        Err(e) => return format!("<pre>template compile failed: {e}</pre>"),
    };
    template
        .render_to_string(&view)
        .unwrap_or_else(|e| format!("<pre>template render failed: {e}</pre>"))
}

fn to_cell_view(c: &CellStatus) -> CellView {
    let instances: Vec<InstanceView> = c
        .instances
        .iter()
        .map(|i| {
            let short: String = i.instance_id.chars().take(12).collect();
            InstanceView {
                instance_name: i.instance_name.clone(),
                instance_id: i.instance_id.clone(),
                instance_id_short: short,
                stopped: i.stopped_at.is_some(),
                stopped_since: i.stopped_at.as_ref().map(format_utc),
            }
        })
        .collect();
    let has_instances = !instances.is_empty();
    let (yes, no, unknown) = match c.healthy {
        Some(true) => (true, false, false),
        Some(false) => (false, true, false),
        None => (false, false, true),
    };
    CellView {
        key: c.key.clone(),
        stage: c.stage.clone(),
        node_count: c.node_count,
        cluster_id: c.cluster_id.map(|u| u.to_string()),
        instances,
        has_instances,
        healthy_yes: yes,
        healthy_no: no,
        healthy_unknown: unknown,
        detail: c.detail.clone().filter(|s| !s.is_empty()),
        launching_since: c.launching_since.as_ref().map(format_utc),
        deploy_failures: c.deploy_failures,
        fails_class: if c.deploy_failures > 0 {
            "fails-nz mono"
        } else {
            "fails-zero mono"
        },
    }
}

fn format_utc(t: &DateTime<Utc>) -> String {
    t.format("%Y-%m-%d %H:%M:%SZ").to_string()
}

fn join_ints(v: &[u32]) -> String {
    v.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{CellInstance, CellStatus, MatrixAxes, StatusSnapshot};
    use uuid::Uuid;

    fn sample() -> StatusSnapshot {
        StatusSnapshot {
            total_cells: 2,
            running: 1,
            paused: false,
            matrix_axes: MatrixAxes {
                cluster_sizes: vec![1, 2],
                agents: vec!["openclaw".into(), "none".into()],
                llms: vec!["ollama".into(), "lms".into()],
                cloud_providers_configured: 0,
                ollama_model: "smollm2:1.7b".into(),
                lms_model: "smollm2-1.7b-instruct".into(),
            },
            runner_version: "0.1.0".into(),
            runner_git_sha: "deadbeef12345".into(),
            cells: vec![
                CellStatus {
                    key: "openclaw-ollama".into(),
                    stage: "running".into(),
                    cluster_id: Some(Uuid::nil()),
                    node_count: 1,
                    instances: vec![CellInstance {
                        instance_name: "mmr-openclaw-ollama".into(),
                        instance_id: "abc123def456".into(),
                        stopped_at: None,
                    }],
                    launching_since: None,
                    deploy_failures: 0,
                    healthy: Some(true),
                    detail: Some("v0.1.5 ✔".into()),
                },
                CellStatus {
                    key: "none-lms-n2".into(),
                    stage: "launching".into(),
                    cluster_id: Some(Uuid::nil()),
                    node_count: 2,
                    instances: vec![
                        CellInstance {
                            instance_name: "mmr-none-lms-n2-1".into(),
                            instance_id: "deadbeef0001".into(),
                            stopped_at: None,
                        },
                        CellInstance {
                            instance_name: "mmr-none-lms-n2-2".into(),
                            instance_id: "deadbeef0002".into(),
                            stopped_at: Some(Utc::now()),
                        },
                    ],
                    launching_since: Some(Utc::now()),
                    deploy_failures: 2,
                    healthy: None,
                    detail: Some("launching (waited 30s, 2 nodes)".into()),
                },
            ],
        }
    }

    /// Template must compile and render every branch (has_instances /
    /// empty-instances, healthy yes/no/unknown, optional cluster_id,
    /// launching_since present/absent, fails zero/non-zero).
    #[test]
    fn render_index_produces_plausible_html() {
        let html = render_index(&sample());
        assert!(html.starts_with("<!doctype html>"), "got: {}", &html[..80]);
        assert!(html.contains("mac-mgmt-runner"));
        assert!(html.contains("openclaw-ollama"));
        assert!(html.contains("mmr-openclaw-ollama"));
        assert!(html.contains("abc123def456"), "full instance_id present");
        assert!(html.contains("abc123def456"), "or in title attr");
        assert!(html.contains("healthy"));
        assert!(html.contains("unhealthy") || html.contains("health-unknown"));
        assert!(html.contains("stage running"));
        assert!(html.contains("stage launching"));
        // Neon theme markers
        assert!(html.contains("--neon"));
        // Templates must not leave raw mustache placeholders
        assert!(!html.contains("{{"), "unrendered placeholder in: {html}");
    }
}
