use dioxus::prelude::*;

// ── Server functions ───────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportSourceRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub source_type: String,
    pub source_config: serde_json::Value,
    pub channel: String,
    pub auto_sync: bool,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportJobRow {
    pub id: uuid::Uuid,
    pub source_id: uuid::Uuid,
    pub status: String,
    pub skills_imported: i32,
    pub log: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[server]
async fn list_import_sources() -> Result<Vec<ImportSourceRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
        source_type: String,
        source_config: serde_json::Value,
        channel: String,
        auto_sync: bool,
        last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, source_type, source_config, channel, auto_sync, \
         last_synced_at, created_at FROM import_sources ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ImportSourceRow {
            id: r.id,
            name: r.name,
            source_type: r.source_type,
            source_config: r.source_config,
            channel: r.channel,
            auto_sync: r.auto_sync,
            last_synced_at: r.last_synced_at,
            created_at: r.created_at,
        })
        .collect())
}

#[server]
async fn create_import_source(
    name: String,
    source_type: String,
    source_config: serde_json::Value,
    channel: String,
    auto_sync: bool,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    sqlx::query(
        "INSERT INTO import_sources (name, source_type, source_config, channel, auto_sync) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&name)
    .bind(&source_type)
    .bind(&source_config)
    .bind(&channel)
    .bind(auto_sync)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn trigger_sync(source_id: uuid::Uuid) -> Result<uuid::Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    // Create job
    let job_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO import_jobs (source_id) VALUES ($1) RETURNING id",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fetch source for the background task
    #[derive(sqlx::FromRow)]
    struct SourceRow {
        id: uuid::Uuid,
        name: String,
        source_type: String,
        source_config: serde_json::Value,
        channel: String,
        auto_sync: bool,
        last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
        created_at: chrono::DateTime<chrono::Utc>,
    }
    let source: SourceRow = sqlx::query_as(
        "SELECT id, name, source_type, source_config, channel, auto_sync, \
         last_synced_at, created_at FROM import_sources WHERE id = $1",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Spawn background sync — reuse the API importer's run_sync logic
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let source_row = crate::api::importer::SourceRow {
            id: source.id,
            name: source.name,
            source_type: source.source_type,
            source_config: source.source_config,
            channel: source.channel,
            auto_sync: source.auto_sync,
            last_synced_at: source.last_synced_at,
            created_at: source.created_at,
        };
        let result = crate::api::importer::run_sync_public(&pool_clone, &source_row, job_id).await;
        let (status, extra_log) = match result {
            Ok(count) => ("done".to_string(), format!("\nCompleted: {count} skill(s) imported")),
            Err(e) => ("failed".to_string(), format!("\nError: {e}")),
        };

        let _ = sqlx::query(
            "UPDATE import_jobs SET status = $1, log = log || $2, \
             updated_at = now() WHERE id = $3",
        )
        .bind(&status)
        .bind(&extra_log)
        .bind(job_id)
        .execute(&pool_clone)
        .await;
    });

    Ok(job_id)
}

#[server]
async fn delete_import_source(source_id: uuid::Uuid, remove_skills: bool) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    if remove_skills {
        let slugs: Vec<String> = sqlx::query_scalar(
            "SELECT skill_slug FROM import_source_skills WHERE source_id = $1",
        )
        .bind(source_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if !slugs.is_empty() {
            sqlx::query("DELETE FROM skills WHERE slug = ANY($1)")
                .bind(&slugs)
                .execute(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

    sqlx::query("DELETE FROM import_sources WHERE id = $1")
        .bind(source_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if remove_skills {
        crate::api::push::notify_federation_global();
    }

    Ok(())
}

#[server]
async fn list_import_jobs(source_id: uuid::Uuid) -> Result<Vec<ImportJobRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        source_id: uuid::Uuid,
        status: String,
        skills_imported: i32,
        log: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, source_id, status, skills_imported, log, created_at, updated_at \
         FROM import_jobs WHERE source_id = $1 ORDER BY created_at DESC LIMIT 10",
    )
    .bind(source_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ImportJobRow {
            id: r.id,
            source_id: r.source_id,
            status: r.status,
            skills_imported: r.skills_imported,
            log: r.log,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
}

// ── Component ──────────────────────────────────────────────────────────

#[component]
pub fn ImportSources() -> Element {
    let mut sources = use_server_future(list_import_sources)?;
    let mut show_form = use_signal(|| false);
    let mut form_error = use_signal(|| None::<String>);
    let mut syncing = use_signal(|| None::<uuid::Uuid>);
    let mut expanded_source = use_signal(|| None::<uuid::Uuid>);

    // Form state
    let mut form_name = use_signal(String::new);
    let mut form_type = use_signal(|| "git".to_string());
    let mut form_repo_url = use_signal(String::new);
    let mut form_branch = use_signal(String::new);
    let mut form_glob = use_signal(|| "skills/*".to_string());
    let mut form_clawhub_slug = use_signal(String::new);
    let mut form_channel = use_signal(|| "stable".to_string());
    let mut form_auto_sync = use_signal(|| false);

    rsx! {
        div { class: "px-6 py-8 max-w-5xl mx-auto",
            div { class: "flex items-center justify-between mb-6",
                h1 { class: "text-2xl font-bold dark:text-white", "Import Sources" }
                button {
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    onclick: move |_| show_form.set(!show_form()),
                    if show_form() { "Cancel" } else { "Add Source" }
                }
            }

            // ── Create form ────────────────────────────────────────
            if show_form() {
                div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6 mb-6 border dark:border-gray-700",
                    h2 { class: "text-lg font-semibold mb-4 dark:text-white", "New Import Source" }

                    if let Some(err) = form_error() {
                        p { class: "text-red-500 text-sm mb-3", "{err}" }
                    }

                    div { class: "space-y-4",
                        div {
                            label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "Name" }
                            input {
                                class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                value: "{form_name}",
                                oninput: move |e| form_name.set(e.value()),
                                placeholder: "My skill source",
                            }
                        }
                        div {
                            label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "Type" }
                            select {
                                class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                value: "{form_type}",
                                onchange: move |e| form_type.set(e.value()),
                                option { value: "git", "Git Repository" }
                                option { value: "clawhub", "ClawHub" }
                            }
                        }

                        if form_type() == "git" {
                            div {
                                label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "Repository URL" }
                                input {
                                    class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                    value: "{form_repo_url}",
                                    oninput: move |e| form_repo_url.set(e.value()),
                                    placeholder: "https://github.com/org/skills.git",
                                }
                            }
                            div {
                                label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "Branch (optional)" }
                                input {
                                    class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                    value: "{form_branch}",
                                    oninput: move |e| form_branch.set(e.value()),
                                    placeholder: "main",
                                }
                            }
                            div {
                                label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "Glob pattern" }
                                input {
                                    class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                    value: "{form_glob}",
                                    oninput: move |e| form_glob.set(e.value()),
                                    placeholder: "skills/*",
                                }
                            }
                        } else {
                            div {
                                label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "ClawHub Skill Slug" }
                                input {
                                    class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                    value: "{form_clawhub_slug}",
                                    oninput: move |e| form_clawhub_slug.set(e.value()),
                                    placeholder: "my-skill",
                                }
                            }
                        }

                        div {
                            label { class: "block text-sm font-medium dark:text-gray-300 mb-1", "Channel" }
                            input {
                                class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                                value: "{form_channel}",
                                oninput: move |e| form_channel.set(e.value()),
                                placeholder: "stable",
                            }
                        }
                        div { class: "flex items-center gap-2",
                            input {
                                r#type: "checkbox",
                                checked: form_auto_sync(),
                                onchange: move |e| form_auto_sync.set(e.checked()),
                            }
                            label { class: "text-sm dark:text-gray-300", "Auto-sync periodically" }
                        }
                        button {
                            class: "bg-green-600 text-white px-4 py-2 rounded text-sm hover:bg-green-700",
                            onclick: move |_| {
                                let name = form_name();
                                let source_type = form_type();
                                let channel = form_channel();
                                let auto_sync = form_auto_sync();

                                let source_config = if source_type == "git" {
                                    let branch = form_branch();
                                    serde_json::json!({
                                        "type": "git",
                                        "repo_url": form_repo_url(),
                                        "branch": if branch.is_empty() { None } else { Some(branch) },
                                        "glob": form_glob(),
                                    })
                                } else {
                                    serde_json::json!({
                                        "type": "clawhub",
                                        "slug": form_clawhub_slug(),
                                    })
                                };

                                spawn(async move {
                                    match create_import_source(name, source_type, source_config, channel, auto_sync).await {
                                        Ok(()) => {
                                            show_form.set(false);
                                            form_error.set(None);
                                            sources.restart();
                                        }
                                        Err(e) => form_error.set(Some(e.to_string())),
                                    }
                                });
                            },
                            "Create"
                        }
                    }
                }
            }

            // ── Sources table ──────────────────────────────────────
            {
                let snapshot = sources.read().clone();
                match snapshot {
                    Some(Ok(list)) => rsx! {
                        if list.is_empty() {
                            p { class: "text-gray-500 dark:text-gray-400", "No import sources configured." }
                        } else {
                            div { class: "space-y-4",
                                for source in list {
                                    {render_source_card(&source, &mut syncing, &mut expanded_source, &mut sources)}
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-red-500", "Error: {e}" } },
                    None => rsx! { p { class: "text-gray-500", "Loading..." } },
                }
            }
        }
    }
}

fn render_source_card(
    source: &ImportSourceRow,
    syncing: &mut Signal<Option<uuid::Uuid>>,
    expanded: &mut Signal<Option<uuid::Uuid>>,
    sources: &mut Resource<Result<Vec<ImportSourceRow>, ServerFnError>>,
) -> Element {
    let source_id = source.id;
    let source_name = source.name.clone();
    let is_syncing = syncing() == Some(source_id);
    let is_expanded = expanded() == Some(source_id);
    let mut syncing = *syncing;
    let mut expanded = *expanded;
    let mut sources = *sources;

    // Build description from source_config
    let desc = match source.source_type.as_str() {
        "git" => {
            let url = source.source_config.get("repo_url")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let glob = source.source_config.get("glob")
                .and_then(|v| v.as_str())
                .unwrap_or("*");
            format!("{url} ({glob})")
        }
        "clawhub" => {
            let slug = source.source_config.get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("clawhub:{slug}")
        }
        _ => "unknown".to_string(),
    };

    let synced_label = source.last_synced_at
        .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| "never".to_string());

    rsx! {
        div { class: "bg-white dark:bg-gray-800 rounded-lg shadow border dark:border-gray-700",
            div { class: "p-4 flex items-center justify-between",
                div {
                    div { class: "flex items-center gap-2",
                        h3 { class: "font-semibold dark:text-white", "{source_name}" }
                        span { class: "text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400",
                            "{source.source_type}"
                        }
                        span { class: "text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
                            "{source.channel}"
                        }
                        if source.auto_sync {
                            span { class: "text-xs px-2 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300",
                                "auto"
                            }
                        }
                    }
                    p { class: "text-sm text-gray-500 dark:text-gray-400 mt-1 font-mono", "{desc}" }
                    p { class: "text-xs text-gray-400 dark:text-gray-500 mt-0.5",
                        "Last synced: {synced_label}"
                    }
                }
                div { class: "flex items-center gap-2",
                    button {
                        class: "text-sm px-3 py-1 rounded border dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 dark:text-gray-300",
                        disabled: is_syncing,
                        onclick: move |_| {
                            expanded.set(if is_expanded { None } else { Some(source_id) });
                        },
                        if is_expanded { "Hide Jobs" } else { "Jobs" }
                    }
                    button {
                        class: "text-sm px-3 py-1 rounded bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50",
                        disabled: is_syncing,
                        onclick: move |_| {
                            syncing.set(Some(source_id));
                            spawn(async move {
                                match trigger_sync(source_id).await {
                                    Ok(_job_id) => {
                                        syncing.set(None);
                                        sources.restart();
                                    }
                                    Err(e) => {
                                        tracing::error!("sync failed: {e}");
                                        syncing.set(None);
                                    }
                                }
                            });
                        },
                        if is_syncing { "Syncing..." } else { "Sync Now" }
                    }
                    button {
                        class: "text-sm px-3 py-1 rounded bg-red-600 text-white hover:bg-red-700",
                        onclick: move |_| {
                            spawn(async move {
                                if let Err(e) = delete_import_source(source_id, true).await {
                                    tracing::error!("delete failed: {e}");
                                }
                                sources.restart();
                            });
                        },
                        "Delete"
                    }
                }
            }

            // ── Expanded: show recent jobs ─────────────────────────
            if is_expanded {
                { render_jobs_panel(source_id) }
            }
        }
    }
}

fn render_jobs_panel(source_id: uuid::Uuid) -> Element {
    let jobs = use_server_future(move || list_import_jobs(source_id))?;

    rsx! {
        div { class: "border-t dark:border-gray-700 p-4",
            h4 { class: "text-sm font-semibold dark:text-gray-300 mb-2", "Recent Jobs" }
            match &*jobs.read() {
                Some(Ok(list)) => rsx! {
                    if list.is_empty() {
                        p { class: "text-sm text-gray-500", "No jobs yet." }
                    } else {
                        div { class: "space-y-2",
                            for job in list {
                                div { class: "text-sm border dark:border-gray-700 rounded p-3",
                                    div { class: "flex items-center justify-between mb-1",
                                        {
                                            let ts = job.created_at.format("%Y-%m-%d %H:%M").to_string();
                                            rsx! {
                                                span { class: "font-mono text-xs text-gray-500 dark:text-gray-400",
                                                    "{ts}"
                                                }
                                            }
                                        }
                                        span {
                                            class: match job.status.as_str() {
                                                "done" => "text-xs px-2 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300",
                                                "failed" => "text-xs px-2 py-0.5 rounded bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-300",
                                                "running" => "text-xs px-2 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-300",
                                                _ => "text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400",
                                            },
                                            "{job.status}"
                                        }
                                    }
                                    if job.skills_imported > 0 {
                                        p { class: "text-xs text-gray-600 dark:text-gray-400",
                                            "{job.skills_imported} skill(s) imported"
                                        }
                                    }
                                    if !job.log.is_empty() {
                                        pre { class: "text-xs mt-1 p-2 bg-gray-50 dark:bg-gray-900 rounded overflow-x-auto max-h-40 text-gray-700 dark:text-gray-300",
                                            "{job.log}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-sm text-red-500", "Error: {e}" } },
                None => rsx! { p { class: "text-sm text-gray-500", "Loading..." } },
            }
        }
    }
}
