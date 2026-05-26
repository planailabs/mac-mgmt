use std::fmt::Write;

use rocket::http::{ContentType, Header, Status};
use rocket::response::Responder;
use rocket::{Request, Response, State};
use sqlx::PgPool;
use uuid::Uuid;

use super::auth::{MetricsAuth, MetricsScope};

/// Custom responder for Prometheus text exposition format.
pub(crate) struct PrometheusText(String);

impl<'r> Responder<'r, 'static> for PrometheusText {
    fn respond_to(self, _req: &'r Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .header(ContentType::Plain)
            .header(Header::new(
                "Content-Type",
                "text/plain; version=0.0.4; charset=utf-8",
            ))
            .sized_body(self.0.len(), std::io::Cursor::new(self.0))
            .ok()
    }
}

/// Resolve the auth scope into an optional list of cluster UUIDs to filter by.
/// `None` means no filter (admin — all clusters).
async fn resolve_cluster_ids(
    pool: &PgPool,
    scope: &MetricsScope,
) -> Result<Option<Vec<Uuid>>, sqlx::Error> {
    match scope {
        MetricsScope::All => Ok(None),
        MetricsScope::Cluster(id) => Ok(Some(vec![*id])),
        MetricsScope::Organization(org_id) => {
            let ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT cluster_id FROM organization_clusters WHERE organization_id = $1",
            )
            .bind(org_id)
            .fetch_all(pool)
            .await?;
            Ok(Some(ids))
        }
    }
}

/// Escape a label value for Prometheus text format (escape `\`, `"`, `\n`).
fn escape_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ---------------------------------------------------------------------------
// Metric collectors
// ---------------------------------------------------------------------------

/// A. Rollouts by status
async fn collect_rollouts(
    pool: &PgPool,
    cluster_ids: &Option<Vec<Uuid>>,
    out: &mut String,
) -> Result<(), sqlx::Error> {
    let rows: Vec<(String, i64)> = if let Some(ids) = cluster_ids {
        sqlx::query_as(
            "SELECT r.status, COUNT(DISTINCT r.id) \
             FROM rollouts r \
             JOIN rollout_stages rs ON rs.rollout_id = r.id \
             JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
             WHERE rgm.cluster_id = ANY($1) \
             GROUP BY r.status",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as("SELECT status, COUNT(*) FROM rollouts GROUP BY status")
            .fetch_all(pool)
            .await?
    };

    let _ = writeln!(out, "# HELP macmgmt_rollouts Number of rollouts by status.");
    let _ = writeln!(out, "# TYPE macmgmt_rollouts gauge");
    for (status, count) in &rows {
        let _ = writeln!(
            out,
            "macmgmt_rollouts{{status=\"{}\"}} {count}",
            escape_label(status)
        );
    }
    Ok(())
}

/// B. Rollout stage health (latest evaluation per active stage, by group)
async fn collect_rollout_stage_health(
    pool: &PgPool,
    cluster_ids: &Option<Vec<Uuid>>,
    out: &mut String,
) -> Result<(), sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        rollout_id: Uuid,
        group_name: String,
        stage_order: i32,
        passed: bool,
    }

    let rows: Vec<Row> = if let Some(ids) = cluster_ids {
        sqlx::query_as(
            "SELECT DISTINCT ON (rs.id) \
                    r.id AS rollout_id, rg.name AS group_name, rs.stage_order, he.passed \
             FROM rollout_stages rs \
             JOIN rollouts r ON r.id = rs.rollout_id \
             JOIN rollout_groups rg ON rg.id = rs.group_id \
             JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
             JOIN rollout_stage_health_evaluations he ON he.stage_id = rs.id \
             WHERE rs.status = 'rolling' AND r.status = 'rolling' \
               AND rgm.cluster_id = ANY($1) \
             ORDER BY rs.id, he.evaluated_at DESC",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT DISTINCT ON (rs.id) \
                    r.id AS rollout_id, rg.name AS group_name, rs.stage_order, he.passed \
             FROM rollout_stages rs \
             JOIN rollouts r ON r.id = rs.rollout_id \
             JOIN rollout_groups rg ON rg.id = rs.group_id \
             JOIN rollout_stage_health_evaluations he ON he.stage_id = rs.id \
             WHERE rs.status = 'rolling' AND r.status = 'rolling' \
             ORDER BY rs.id, he.evaluated_at DESC",
        )
        .fetch_all(pool)
        .await?
    };

    let _ = writeln!(
        out,
        "# HELP macmgmt_rollout_stage_health Latest health evaluation per active rollout stage."
    );
    let _ = writeln!(out, "# TYPE macmgmt_rollout_stage_health gauge");
    for r in &rows {
        let _ = writeln!(
            out,
            "macmgmt_rollout_stage_health{{rollout_id=\"{}\",group=\"{}\",stage_order=\"{}\",passed=\"{}\"}} {}",
            r.rollout_id,
            escape_label(&r.group_name),
            r.stage_order,
            r.passed,
            if r.passed { 1 } else { 0 }
        );
    }
    Ok(())
}

/// C. Rollout updated nodes vs total nodes per active rollout group
async fn collect_rollout_nodes(
    pool: &PgPool,
    cluster_ids: &Option<Vec<Uuid>>,
    out: &mut String,
) -> Result<(), sqlx::Error> {
    // Get active rolling rollouts with their target version and stages
    #[derive(sqlx::FromRow)]
    struct StageRow {
        rollout_id: Uuid,
        target_version: Option<String>,
        group_id: Uuid,
        group_name: String,
    }

    let stages: Vec<StageRow> = if let Some(ids) = cluster_ids {
        sqlx::query_as(
            "SELECT r.id AS rollout_id, r.target_version, rs.group_id, rg.name AS group_name \
             FROM rollout_stages rs \
             JOIN rollouts r ON r.id = rs.rollout_id \
             JOIN rollout_groups rg ON rg.id = rs.group_id \
             JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
             WHERE rs.status = 'rolling' AND r.status = 'rolling' \
               AND rgm.cluster_id = ANY($1) \
             GROUP BY r.id, r.target_version, rs.group_id, rg.name",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT r.id AS rollout_id, r.target_version, rs.group_id, rg.name AS group_name \
             FROM rollout_stages rs \
             JOIN rollouts r ON r.id = rs.rollout_id \
             JOIN rollout_groups rg ON rg.id = rs.group_id \
             WHERE rs.status = 'rolling' AND r.status = 'rolling'",
        )
        .fetch_all(pool)
        .await?
    };

    let _ = writeln!(
        out,
        "# HELP macmgmt_rollout_updated_nodes Instances on the rollout target version."
    );
    let _ = writeln!(out, "# TYPE macmgmt_rollout_updated_nodes gauge");
    let _ = writeln!(
        out,
        "# HELP macmgmt_rollout_total_nodes Total instances in the rollout group."
    );
    let _ = writeln!(out, "# TYPE macmgmt_rollout_total_nodes gauge");

    for stage in &stages {
        // Get cluster_ids in this group (optionally intersected with scope)
        let group_clusters: Vec<Uuid> = if let Some(ids) = cluster_ids {
            sqlx::query_scalar(
                "SELECT cluster_id FROM rollout_group_members \
                 WHERE group_id = $1 AND cluster_id = ANY($2)",
            )
            .bind(stage.group_id)
            .bind(ids)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_scalar("SELECT cluster_id FROM rollout_group_members WHERE group_id = $1")
                .bind(stage.group_id)
                .fetch_all(pool)
                .await?
        };

        if group_clusters.is_empty() {
            continue;
        }

        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM daemon_heartbeats WHERE cluster_id = ANY($1)")
                .bind(&group_clusters)
                .fetch_one(pool)
                .await?;

        let updated: i64 = if let Some(ver) = &stage.target_version {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM daemon_heartbeats \
                 WHERE cluster_id = ANY($1) AND version = $2",
            )
            .bind(&group_clusters)
            .bind(ver)
            .fetch_one(pool)
            .await?
        } else {
            total
        };

        let labels = format!(
            "rollout_id=\"{}\",group=\"{}\"",
            stage.rollout_id,
            escape_label(&stage.group_name)
        );
        let _ = writeln!(out, "macmgmt_rollout_updated_nodes{{{labels}}} {updated}");
        let _ = writeln!(out, "macmgmt_rollout_total_nodes{{{labels}}} {total}");
    }
    Ok(())
}

/// D. Staff pings (total, unresolved, by category)
async fn collect_staff_pings(
    pool: &PgPool,
    cluster_ids: &Option<Vec<Uuid>>,
    out: &mut String,
) -> Result<(), sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        category: String,
        resolved: bool,
        count: i64,
    }

    let rows: Vec<Row> = if let Some(ids) = cluster_ids {
        sqlx::query_as(
            "SELECT category, resolved, COUNT(*) AS count \
             FROM healer_staff_pings WHERE cluster_id = ANY($1) \
             GROUP BY category, resolved",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT category, resolved, COUNT(*) AS count \
             FROM healer_staff_pings GROUP BY category, resolved",
        )
        .fetch_all(pool)
        .await?
    };

    let total: i64 = rows.iter().map(|r| r.count).sum();
    let unresolved: i64 = rows.iter().filter(|r| !r.resolved).map(|r| r.count).sum();

    let _ = writeln!(out, "# HELP macmgmt_staff_pings_total Total staff pings.");
    let _ = writeln!(out, "# TYPE macmgmt_staff_pings_total gauge");
    let _ = writeln!(out, "macmgmt_staff_pings_total {total}");
    let _ = writeln!(
        out,
        "# HELP macmgmt_staff_pings_unresolved Unresolved staff pings."
    );
    let _ = writeln!(out, "# TYPE macmgmt_staff_pings_unresolved gauge");
    let _ = writeln!(out, "macmgmt_staff_pings_unresolved {unresolved}");
    let _ = writeln!(
        out,
        "# HELP macmgmt_staff_pings Staff pings by category and resolved status."
    );
    let _ = writeln!(out, "# TYPE macmgmt_staff_pings gauge");
    for r in &rows {
        let _ = writeln!(
            out,
            "macmgmt_staff_pings{{category=\"{}\",resolved=\"{}\"}} {}",
            escape_label(&r.category),
            r.resolved,
            r.count
        );
    }
    Ok(())
}

/// E. Healer sessions by state
async fn collect_healer_sessions(
    pool: &PgPool,
    cluster_ids: &Option<Vec<Uuid>>,
    out: &mut String,
) -> Result<(), sqlx::Error> {
    let rows: Vec<(String, i64)> = if let Some(ids) = cluster_ids {
        sqlx::query_as(
            "SELECT state, COUNT(*) FROM healer_sessions \
             WHERE cluster_id = ANY($1) GROUP BY state",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as("SELECT state, COUNT(*) FROM healer_sessions GROUP BY state")
            .fetch_all(pool)
            .await?
    };

    let _ = writeln!(
        out,
        "# HELP macmgmt_healer_sessions Healer sessions by state."
    );
    let _ = writeln!(out, "# TYPE macmgmt_healer_sessions gauge");
    for (state, count) in &rows {
        let _ = writeln!(
            out,
            "macmgmt_healer_sessions{{state=\"{}\"}} {count}",
            escape_label(state)
        );
    }
    Ok(())
}

/// F. Healer token usage by provider and model
async fn collect_healer_tokens(
    pool: &PgPool,
    cluster_ids: &Option<Vec<Uuid>>,
    out: &mut String,
) -> Result<(), sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        provider: String,
        model: String,
        total_input: i64,
        total_output: i64,
    }

    let rows: Vec<Row> = if let Some(ids) = cluster_ids {
        sqlx::query_as(
            "SELECT te.provider, te.model, \
                    SUM(te.input_tokens)::bigint AS total_input, \
                    SUM(te.output_tokens)::bigint AS total_output \
             FROM healer_token_events te \
             JOIN healer_sessions hs ON hs.id = te.session_id \
             WHERE hs.cluster_id = ANY($1) \
             GROUP BY te.provider, te.model",
        )
        .bind(ids)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT provider, model, \
                    SUM(input_tokens)::bigint AS total_input, \
                    SUM(output_tokens)::bigint AS total_output \
             FROM healer_token_events GROUP BY provider, model",
        )
        .fetch_all(pool)
        .await?
    };

    let _ = writeln!(
        out,
        "# HELP macmgmt_healer_tokens_total Total tokens used by healer (input + output)."
    );
    let _ = writeln!(out, "# TYPE macmgmt_healer_tokens_total counter");
    let _ = writeln!(
        out,
        "# HELP macmgmt_healer_tokens_input Input tokens used by healer."
    );
    let _ = writeln!(out, "# TYPE macmgmt_healer_tokens_input counter");
    let _ = writeln!(
        out,
        "# HELP macmgmt_healer_tokens_output Output tokens used by healer."
    );
    let _ = writeln!(out, "# TYPE macmgmt_healer_tokens_output counter");
    for r in &rows {
        let labels = format!(
            "provider=\"{}\",model=\"{}\"",
            escape_label(&r.provider),
            escape_label(&r.model)
        );
        let total = r.total_input + r.total_output;
        let _ = writeln!(out, "macmgmt_healer_tokens_total{{{labels}}} {total}");
        let _ = writeln!(
            out,
            "macmgmt_healer_tokens_input{{{labels}}} {}",
            r.total_input
        );
        let _ = writeln!(
            out,
            "macmgmt_healer_tokens_output{{{labels}}} {}",
            r.total_output
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Route
// ---------------------------------------------------------------------------

#[rocket::get("/metrics")]
pub async fn get_metrics(
    auth: MetricsAuth,
    pool: &State<PgPool>,
) -> Result<PrometheusText, Status> {
    let cluster_ids = resolve_cluster_ids(pool.inner(), &auth.scope)
        .await
        .map_err(|_| Status::InternalServerError)?;

    let mut out = String::with_capacity(4096);

    // Collect all metric sections; log errors but don't fail the whole response.
    if let Err(e) = collect_rollouts(pool.inner(), &cluster_ids, &mut out).await {
        tracing::warn!("metrics: rollouts query failed: {e}");
    }
    if let Err(e) = collect_rollout_stage_health(pool.inner(), &cluster_ids, &mut out).await {
        tracing::warn!("metrics: rollout stage health query failed: {e}");
    }
    if let Err(e) = collect_rollout_nodes(pool.inner(), &cluster_ids, &mut out).await {
        tracing::warn!("metrics: rollout nodes query failed: {e}");
    }
    if let Err(e) = collect_staff_pings(pool.inner(), &cluster_ids, &mut out).await {
        tracing::warn!("metrics: staff pings query failed: {e}");
    }
    if let Err(e) = collect_healer_sessions(pool.inner(), &cluster_ids, &mut out).await {
        tracing::warn!("metrics: healer sessions query failed: {e}");
    }
    if let Err(e) = collect_healer_tokens(pool.inner(), &cluster_ids, &mut out).await {
        tracing::warn!("metrics: healer tokens query failed: {e}");
    }

    Ok(PrometheusText(out))
}
