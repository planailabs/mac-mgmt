use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;

// ── Server functions ───────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportSourceRow {
    pub id: String,
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
    pub id: String,
    pub source_id: String,
    pub status: String,
    pub skills_imported: i32,
    pub log: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClawHubHit {
    pub slug: String,
    pub display_name: String,
    pub summary: String,
    pub version: Option<String>,
}

#[server]
async fn list_import_sources() -> Result<Vec<ImportSourceRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row { id: uuid::Uuid, name: String, source_type: String, source_config: serde_json::Value, channel: String, auto_sync: bool, last_synced_at: Option<chrono::DateTime<chrono::Utc>>, created_at: chrono::DateTime<chrono::Utc> }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, source_type, source_config, channel, auto_sync, \
         last_synced_at, created_at FROM import_sources ORDER BY created_at DESC",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| ImportSourceRow {
        id: r.id.to_string(), name: r.name, source_type: r.source_type, source_config: r.source_config,
        channel: r.channel, auto_sync: r.auto_sync, last_synced_at: r.last_synced_at, created_at: r.created_at,
    }).collect())
}

#[server]
async fn get_import_source(id: String) -> Result<ImportSourceRow, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row { id: uuid::Uuid, name: String, source_type: String, source_config: serde_json::Value, channel: String, auto_sync: bool, last_synced_at: Option<chrono::DateTime<chrono::Utc>>, created_at: chrono::DateTime<chrono::Utc> }

    let r: Row = sqlx::query_as(
        "SELECT id, name, source_type, source_config, channel, auto_sync, \
         last_synced_at, created_at FROM import_sources WHERE id = $1",
    )
    .bind(uuid).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(ImportSourceRow {
        id: r.id.to_string(), name: r.name, source_type: r.source_type, source_config: r.source_config,
        channel: r.channel, auto_sync: r.auto_sync, last_synced_at: r.last_synced_at, created_at: r.created_at,
    })
}

#[server]
async fn create_import_source(
    name: String, source_type: String, source_config: serde_json::Value, channel: String, auto_sync: bool,
) -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO import_sources (name, source_type, source_config, channel, auto_sync) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&name).bind(&source_type).bind(&source_config).bind(&channel).bind(auto_sync)
    .fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(id.to_string())
}

#[server]
async fn update_import_source(
    source_id: String, name: String, source_config: serde_json::Value, channel: String, auto_sync: bool,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = source_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "UPDATE import_sources SET name = $1, source_config = $2, channel = $3, auto_sync = $4 WHERE id = $5",
    )
    .bind(&name).bind(&source_config).bind(&channel).bind(auto_sync).bind(uuid)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn trigger_sync(source_id: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = source_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let job_id: uuid::Uuid = sqlx::query_scalar("INSERT INTO import_jobs (source_id) VALUES ($1) RETURNING id")
        .bind(uuid).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row { id: uuid::Uuid, name: String, source_type: String, source_config: serde_json::Value, channel: String, auto_sync: bool, last_synced_at: Option<chrono::DateTime<chrono::Utc>>, created_at: chrono::DateTime<chrono::Utc> }

    let source: Row = sqlx::query_as(
        "SELECT id, name, source_type, source_config, channel, auto_sync, last_synced_at, created_at FROM import_sources WHERE id = $1",
    ).bind(uuid).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let pool_clone = pool.clone();
    tokio::spawn(async move {
        let source_row = crate::api::importer::SourceRow {
            id: source.id, name: source.name, source_type: source.source_type,
            source_config: source.source_config, channel: source.channel, auto_sync: source.auto_sync,
            last_synced_at: source.last_synced_at, created_at: source.created_at,
        };
        let result = crate::api::importer::run_sync_public(&pool_clone, &source_row, job_id).await;
        let (status, extra_log) = match result {
            Ok(count) => ("done".to_string(), format!("\nCompleted: {count} skill(s) imported")),
            Err(e) => ("failed".to_string(), format!("\nError: {e}")),
        };
        let _ = sqlx::query("UPDATE import_jobs SET status = $1, log = log || $2, updated_at = now() WHERE id = $3")
            .bind(&status).bind(&extra_log).bind(job_id).execute(&pool_clone).await;
    });
    Ok(())
}

#[server]
async fn delete_import_source(source_id: String, remove_skills: bool) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = source_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    if remove_skills {
        let slugs: Vec<String> = sqlx::query_scalar("SELECT skill_slug FROM import_source_skills WHERE source_id = $1")
            .bind(uuid).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        if !slugs.is_empty() {
            sqlx::query("DELETE FROM skills WHERE slug = ANY($1)").bind(&slugs)
                .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }
    sqlx::query("DELETE FROM import_sources WHERE id = $1").bind(uuid)
        .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    if remove_skills { crate::api::push::notify_federation_global(); }
    Ok(())
}

#[server]
async fn list_import_jobs(source_id: String) -> Result<Vec<ImportJobRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = source_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row { id: uuid::Uuid, source_id: uuid::Uuid, status: String, skills_imported: i32, log: String, created_at: chrono::DateTime<chrono::Utc>, updated_at: chrono::DateTime<chrono::Utc> }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, source_id, status, skills_imported, log, created_at, updated_at \
         FROM import_jobs WHERE source_id = $1 ORDER BY created_at DESC LIMIT 20",
    ).bind(uuid).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| ImportJobRow {
        id: r.id.to_string(), source_id: r.source_id.to_string(), status: r.status,
        skills_imported: r.skills_imported, log: r.log, created_at: r.created_at, updated_at: r.updated_at,
    }).collect())
}

#[server]
async fn search_clawhub(query: String) -> Result<Vec<ClawHubHit>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let cfg = crate::config::config();
    let importer = cfg.importer.as_ref().ok_or_else(|| ServerFnError::new("importer not configured"))?;
    let client = crate::clawhub_client::ClawHubClient::new(&importer.clawhub_url);
    let results = client.search(&query, 20).await.map_err(|e| ServerFnError::new(e))?;
    Ok(results.into_iter().map(|r| ClawHubHit {
        slug: r.slug, display_name: r.display_name, summary: r.summary, version: r.version,
    }).collect())
}

// ══════════════════════════════════════════════════════════════════════
// List page: /import-sources
// ══════════════════════════════════════════════════════════════════════

#[component]
pub fn ImportSources(
    prefill_slug: Option<String>,
    prefill_name: Option<String>,
) -> Element {
    let navigator = navigator();
    let has_prefill = prefill_slug.as_ref().is_some_and(|s| !s.is_empty());

    let mut sources = use_server_future(list_import_sources)?;
    let mut show_form = use_signal(|| false);
    let mut form_error = use_signal(|| None::<String>);

    // Form state
    let mut form_name = use_signal(String::new);
    let mut form_type = use_signal(|| "git".to_string());
    let mut form_repo_url = use_signal(String::new);
    let mut form_branch = use_signal(String::new);
    let mut form_glob = use_signal(|| "skills/*".to_string());
    let mut form_clawhub_slug = use_signal(String::new);
    let mut form_channel = use_signal(|| "stable".to_string());
    let mut form_auto_sync = use_signal(|| false);

    // Prefill from query params (ClawHub search → import flow)
    let mut prefilled = use_signal(|| false);
    if !prefilled() {
        if let Some(slug) = &prefill_slug {
            if !slug.is_empty() {
                form_clawhub_slug.set(slug.clone());
                form_type.set("clawhub".to_string());
                form_name.set(prefill_name.clone().unwrap_or_else(|| slug.clone()));
                show_form.set(true);
            }
        }
        prefilled.set(true);
    }

    let clear_prefill = {
        let navigator = navigator.clone();
        move || {
            if has_prefill {
                navigator.replace(Route::ImportSources { prefill_slug: None, prefill_name: None });
            }
        }
    };

    let mut reset_form = move || {
        form_name.set(String::new());
        form_type.set("git".to_string());
        form_repo_url.set(String::new());
        form_branch.set(String::new());
        form_glob.set("skills/*".to_string());
        form_clawhub_slug.set(String::new());
        form_channel.set("stable".to_string());
        form_auto_sync.set(false);
        form_error.set(None);
    };

    rsx! {
        div { class: "px-6 py-8 max-w-5xl mx-auto",
            div { class: "flex items-center justify-between mb-6",
                h1 { class: "text-2xl font-bold dark:text-white", {t!("import-title")} }
                div { class: "flex items-center gap-2",
                    Link {
                        to: Route::ImportSourcesSearch {},
                        class: "bg-purple-600 text-white px-4 py-2 rounded text-sm hover:bg-purple-700",
                        {t!("import-search-clawhub")}
                    }
                    button {
                        class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                        onclick: move |_| {
                            if show_form() {
                                reset_form();
                                show_form.set(false);
                                clear_prefill();
                            } else {
                                reset_form();
                                show_form.set(true);
                            }
                        },
                        if show_form() { {t!("cancel")} } else { {t!("import-add-source")} }
                    }
                }
            }

            if show_form() {
                div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6 mb-6 border dark:border-gray-700",
                    h2 { class: "text-lg font-semibold mb-4 dark:text-white", {t!("import-new-source")} }
                    if let Some(err) = form_error() {
                        p { class: "text-red-500 text-sm mb-3", "{err}" }
                    }
                    { render_source_form(&form_name, &form_type, &form_repo_url, &form_branch, &form_glob, &form_clawhub_slug, &form_channel, &form_auto_sync, false) }
                    button {
                        class: "bg-green-600 text-white px-4 py-2 rounded text-sm hover:bg-green-700 mt-4",
                        onclick: move |_| {
                            let name = form_name();
                            let source_type = form_type();
                            let channel = form_channel();
                            let auto_sync = form_auto_sync();
                            let source_config = build_source_config(&source_type, &form_repo_url(), &form_branch(), &form_glob(), &form_clawhub_slug());
                            spawn(async move {
                                match create_import_source(name, source_type, source_config, channel, auto_sync).await {
                                    Ok(new_id) => {
                                        show_form.set(false);
                                        form_error.set(None);
                                        sources.restart();
                                        clear_prefill();
                                        // Navigate to the new source's detail page
                                        navigator.push(Route::ImportSourceDetail { id: new_id });
                                    }
                                    Err(e) => form_error.set(Some(e.to_string())),
                                }
                            });
                        },
                        {t!("import-create")}
                    }
                }
            }

            // Source list — click name to go to detail
            {
                let snapshot = sources.read().clone();
                match snapshot {
                    Some(Ok(list)) => rsx! {
                        if list.is_empty() {
                            p { class: "text-gray-500 dark:text-gray-400", {t!("import-no-sources")} }
                        } else {
                            div { class: "space-y-3",
                                for source in list {
                                    {
                                        let sid = source.id.clone();
                                        let source_name = source.name.clone();
                                        let desc = source_description(&source);
                                        let synced_label = source.last_synced_at
                                            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                                            .unwrap_or_else(|| t!("import-never-synced").to_string());
                                        rsx! {
                                            Link {
                                                to: Route::ImportSourceDetail { id: sid },
                                                class: "block bg-white dark:bg-gray-800 rounded-lg shadow border dark:border-gray-700 p-4 hover:border-blue-400 dark:hover:border-blue-500 transition-colors",
                                                div { class: "flex items-center gap-2 mb-1",
                                                    h3 { class: "font-semibold dark:text-white", "{source_name}" }
                                                    span { class: "text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400",
                                                        "{source.source_type}"
                                                    }
                                                    span { class: "text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
                                                        "{source.channel}"
                                                    }
                                                    if source.auto_sync {
                                                        span { class: "text-xs px-2 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300",
                                                            {t!("import-badge-auto")}
                                                        }
                                                    }
                                                }
                                                p { class: "text-sm text-gray-500 dark:text-gray-400 font-mono", "{desc}" }
                                                p { class: "text-xs text-gray-400 dark:text-gray-500 mt-0.5",
                                                    {t!("import-last-synced", time: synced_label)}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-red-500", {t!("error-message", message: e.to_string())} } },
                    None => rsx! { p { class: "text-gray-500", {t!("loading")} } },
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// Detail page: /import-sources/:id
// ══════════════════════════════════════════════════════════════════════

#[component]
pub fn ImportSourceDetail(id: String) -> Element {
    let navigator = navigator();
    let id_fetch = id.clone();
    let mut source = use_server_future(move || {
        let sid = id_fetch.clone();
        async move { get_import_source(sid).await }
    })?;

    let id_jobs = id.clone();
    let mut jobs = use_server_future(move || {
        let sid = id_jobs.clone();
        async move { list_import_jobs(sid).await }
    })?;

    let mut form_error = use_signal(|| None::<String>);
    let mut syncing = use_signal(|| false);
    let mut editing = use_signal(|| false);

    // Form signals — populated from source data when entering edit mode
    let mut form_name = use_signal(String::new);
    let mut form_type = use_signal(String::new);
    let mut form_repo_url = use_signal(String::new);
    let mut form_branch = use_signal(String::new);
    let mut form_glob = use_signal(|| "skills/*".to_string());
    let mut form_clawhub_slug = use_signal(String::new);
    let mut form_channel = use_signal(String::new);
    let mut form_auto_sync = use_signal(|| false);

    match &*source.read() {
        Some(Ok(s)) => {
            let source_id = s.id.clone();
            let source_name = s.name.clone();
            let source_type = s.source_type.clone();
            let desc = source_description(s);
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let synced_label = s.last_synced_at
                .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| t!("import-never-synced").to_string());

            let sid_sync = source_id.clone();
            let sid_del = source_id.clone();
            let sid_save = source_id.clone();

            // Clone source data for populating form on edit click
            let edit_source = s.clone();

            rsx! {
                div { class: "px-6 py-8 max-w-5xl mx-auto",
                    // Header
                    div { class: "mb-6",
                        Link {
                            to: Route::ImportSources { prefill_slug: None, prefill_name: None },
                            class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                            "← {t!(\"import-title\")}"
                        }
                        div { class: "flex items-center justify-between mt-1",
                            div {
                                div { class: "flex items-center gap-3",
                                    h2 { class: "text-2xl font-bold dark:text-white", "{source_name}" }
                                    span { class: "text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-400",
                                        "{source_type}"
                                    }
                                    span { class: "text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
                                        "{s.channel}"
                                    }
                                    if s.auto_sync {
                                        span { class: "text-xs px-2 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300",
                                            {t!("import-badge-auto")}
                                        }
                                    }
                                    if !editing() {
                                        button {
                                            class: "text-gray-400 hover:text-gray-600 dark:text-gray-400 dark:hover:text-gray-300",
                                            onclick: move |_| {
                                                // Populate form from current source data
                                                form_name.set(edit_source.name.clone());
                                                form_type.set(edit_source.source_type.clone());
                                                form_channel.set(edit_source.channel.clone());
                                                form_auto_sync.set(edit_source.auto_sync);
                                                match edit_source.source_type.as_str() {
                                                    "git" => {
                                                        form_repo_url.set(edit_source.source_config.get("repo_url").and_then(|v| v.as_str()).unwrap_or("").to_string());
                                                        form_branch.set(edit_source.source_config.get("branch").and_then(|v| v.as_str()).unwrap_or("").to_string());
                                                        form_glob.set(edit_source.source_config.get("glob").and_then(|v| v.as_str()).unwrap_or("skills/*").to_string());
                                                    }
                                                    "clawhub" => {
                                                        form_clawhub_slug.set(edit_source.source_config.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string());
                                                    }
                                                    _ => {}
                                                }
                                                editing.set(true);
                                                form_error.set(None);
                                            },
                                            {t!("edit")}
                                        }
                                    }
                                }
                                p { class: "text-sm text-gray-500 dark:text-gray-400 font-mono mt-1", "{desc}" }
                                p { class: "text-sm text-gray-500 dark:text-gray-400 mt-1",
                                    {t!("cluster-detail-created", date: created)}
                                    " · "
                                    {t!("import-last-synced", time: synced_label)}
                                }
                            }
                            div { class: "flex items-center gap-2",
                                button {
                                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                                    disabled: syncing(),
                                    onclick: move |_| {
                                        let sid = sid_sync.clone();
                                        syncing.set(true);
                                        spawn(async move {
                                            match trigger_sync(sid).await {
                                                Ok(()) => {
                                                    syncing.set(false);
                                                    source.restart();
                                                    jobs.restart();
                                                }
                                                Err(e) => {
                                                    form_error.set(Some(e.to_string()));
                                                    syncing.set(false);
                                                }
                                            }
                                        });
                                    },
                                    if syncing() { {t!("import-syncing")} } else { {t!("import-sync-now")} }
                                }
                                button {
                                    class: "bg-red-600 text-white px-4 py-2 rounded text-sm hover:bg-red-700",
                                    onclick: move |_| {
                                        let sid = sid_del.clone();
                                        let nav = navigator.clone();
                                        spawn(async move {
                                            if delete_import_source(sid, true).await.is_ok() {
                                                nav.push(Route::ImportSources { prefill_slug: None, prefill_name: None });
                                            }
                                        });
                                    },
                                    {t!("delete")}
                                }
                            }
                        }
                    }

                    if let Some(err) = form_error() {
                        p { class: "text-red-500 text-sm mb-4", "{err}" }
                    }

                    // Edit form (hidden until edit button clicked)
                    if editing() {
                        div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6 mb-6 border dark:border-gray-700",
                            h3 { class: "text-lg font-semibold mb-4 dark:text-white", {t!("import-edit-source")} }
                            { render_source_form(&form_name, &form_type, &form_repo_url, &form_branch, &form_glob, &form_clawhub_slug, &form_channel, &form_auto_sync, true) }
                            div { class: "flex items-center gap-2 mt-4",
                                button {
                                    class: "bg-green-600 text-white px-4 py-2 rounded text-sm hover:bg-green-700",
                                    onclick: move |_| {
                                        let sid = sid_save.clone();
                                        let name = form_name();
                                        let stype = form_type();
                                        let channel = form_channel();
                                        let auto_sync = form_auto_sync();
                                        let source_config = build_source_config(&stype, &form_repo_url(), &form_branch(), &form_glob(), &form_clawhub_slug());
                                        spawn(async move {
                                            match update_import_source(sid, name, source_config, channel, auto_sync).await {
                                                Ok(()) => {
                                                    form_error.set(None);
                                                    editing.set(false);
                                                    source.restart();
                                                }
                                                Err(e) => form_error.set(Some(e.to_string())),
                                            }
                                        });
                                    },
                                    {t!("save")}
                                }
                                button {
                                    class: "text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 px-4 py-2 rounded text-sm",
                                    onclick: move |_| editing.set(false),
                                    {t!("cancel")}
                                }
                            }
                        }
                    }

                    // Jobs list
                    div { class: "bg-white dark:bg-gray-800 rounded-lg shadow p-6 border dark:border-gray-700",
                        h3 { class: "text-lg font-semibold mb-4 dark:text-white", {t!("import-recent-jobs")} }
                        { render_jobs(&jobs) }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 p-6", {t!("error-message", message: e.to_string())} } },
        None => rsx! { p { class: "p-6", {t!("loading")} } },
    }
}

// ══════════════════════════════════════════════════════════════════════
// ClawHub Search page: /import-sources/search
// ══════════════════════════════════════════════════════════════════════

#[component]
pub fn ImportSourcesSearch() -> Element {
    let mut query = use_signal(String::new);
    let mut results = use_signal(|| None::<Vec<ClawHubHit>>);
    let mut searching = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        div { class: "px-6 py-8 max-w-5xl mx-auto",
            div { class: "mb-6",
                Link {
                    to: Route::ImportSources { prefill_slug: None, prefill_name: None },
                    class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                    "← {t!(\"import-title\")}"
                }
                h1 { class: "text-2xl font-bold dark:text-white mt-1", {t!("import-search-clawhub")} }
            }

            form {
                class: "flex gap-2 mb-6",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let q = query();
                    if q.trim().is_empty() { return; }
                    searching.set(true);
                    error.set(None);
                    spawn(async move {
                        match search_clawhub(q).await {
                            Ok(hits) => { results.set(Some(hits)); searching.set(false); }
                            Err(e) => { error.set(Some(e.to_string())); searching.set(false); }
                        }
                    });
                },
                input {
                    class: "flex-1 border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                    r#type: "text",
                    value: "{query}",
                    oninput: move |e| query.set(e.value()),
                    placeholder: t!("import-search-placeholder"),
                }
                button {
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
                    r#type: "submit",
                    disabled: searching(),
                    if searching() { {t!("import-searching")} } else { {t!("import-search-button")} }
                }
            }

            if let Some(err) = error() {
                p { class: "text-red-500 text-sm mb-4", "{err}" }
            }

            match results() {
                Some(hits) if hits.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400", {t!("import-search-no-results")} }
                },
                Some(hits) => rsx! {
                    div { class: "space-y-3",
                        for hit in hits {
                            {
                                let slug = hit.slug.clone();
                                let name = hit.display_name.clone();
                                let slug_link = slug.clone();
                                let name_link = name.clone();
                                rsx! {
                                    div { class: "bg-white dark:bg-gray-800 rounded-lg shadow border dark:border-gray-700 p-4 flex items-center justify-between",
                                        div {
                                            div { class: "flex items-center gap-2",
                                                h3 { class: "font-semibold dark:text-white", "{name}" }
                                                span { class: "text-xs font-mono text-gray-500 dark:text-gray-400", "{slug}" }
                                                if let Some(v) = &hit.version {
                                                    span { class: "text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300",
                                                        "v{v}"
                                                    }
                                                }
                                            }
                                            if !hit.summary.is_empty() {
                                                p { class: "text-sm text-gray-500 dark:text-gray-400 mt-1", "{hit.summary}" }
                                            }
                                        }
                                        Link {
                                            to: Route::ImportSources { prefill_slug: Some(slug_link), prefill_name: Some(name_link) },
                                            class: "bg-green-600 text-white px-3 py-1 rounded text-sm hover:bg-green-700 whitespace-nowrap",
                                            {t!("import-search-import")}
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                None => rsx! {},
            }
        }
    }
}

// ── Shared helpers ────────────────────────────────────────────────────

fn source_description(source: &ImportSourceRow) -> String {
    match source.source_type.as_str() {
        "git" => {
            let url = source.source_config.get("repo_url").and_then(|v| v.as_str()).unwrap_or("?");
            let glob = source.source_config.get("glob").and_then(|v| v.as_str()).unwrap_or("*");
            format!("{url} ({glob})")
        }
        "clawhub" => {
            let slug = source.source_config.get("slug").and_then(|v| v.as_str()).unwrap_or("?");
            format!("clawhub:{slug}")
        }
        _ => "unknown".to_string(),
    }
}

fn build_source_config(source_type: &str, repo_url: &str, branch: &str, glob: &str, clawhub_slug: &str) -> serde_json::Value {
    if source_type == "git" {
        serde_json::json!({
            "type": "git",
            "repo_url": repo_url,
            "branch": if branch.is_empty() { None } else { Some(branch) },
            "glob": glob,
        })
    } else {
        serde_json::json!({
            "type": "clawhub",
            "slug": clawhub_slug,
        })
    }
}

fn render_source_form(
    form_name: &Signal<String>,
    form_type: &Signal<String>,
    form_repo_url: &Signal<String>,
    form_branch: &Signal<String>,
    form_glob: &Signal<String>,
    form_clawhub_slug: &Signal<String>,
    form_channel: &Signal<String>,
    form_auto_sync: &Signal<bool>,
    is_edit: bool,
) -> Element {
    let mut form_name = *form_name;
    let mut form_type = *form_type;
    let mut form_repo_url = *form_repo_url;
    let mut form_branch = *form_branch;
    let mut form_glob = *form_glob;
    let mut form_clawhub_slug = *form_clawhub_slug;
    let mut form_channel = *form_channel;
    let mut form_auto_sync = *form_auto_sync;

    rsx! {
        div { class: "space-y-4",
            div {
                label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-name")} }
                input {
                    class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                    value: "{form_name}",
                    oninput: move |e| form_name.set(e.value()),
                    placeholder: t!("import-name-placeholder"),
                }
            }
            if !is_edit {
                div {
                    label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-type")} }
                    select {
                        class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                        value: "{form_type}",
                        onchange: move |e| form_type.set(e.value()),
                        option { value: "git", {t!("import-type-git")} }
                        option { value: "clawhub", {t!("import-type-clawhub")} }
                    }
                }
            }
            if form_type() == "git" {
                div {
                    label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-repo-url")} }
                    input {
                        class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                        value: "{form_repo_url}",
                        oninput: move |e| form_repo_url.set(e.value()),
                        placeholder: "https://github.com/org/skills.git",
                    }
                }
                div {
                    label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-branch")} }
                    input {
                        class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                        value: "{form_branch}",
                        oninput: move |e| form_branch.set(e.value()),
                        placeholder: "main",
                    }
                }
                div {
                    label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-glob")} }
                    input {
                        class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                        value: "{form_glob}",
                        oninput: move |e| form_glob.set(e.value()),
                        placeholder: "skills/*",
                    }
                }
            } else {
                div {
                    label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-clawhub-slug")} }
                    input {
                        class: "w-full border rounded px-3 py-2 text-sm dark:bg-gray-700 dark:border-gray-600 dark:text-white",
                        value: "{form_clawhub_slug}",
                        oninput: move |e| form_clawhub_slug.set(e.value()),
                        placeholder: "my-skill",
                    }
                }
            }
            div {
                label { class: "block text-sm font-medium dark:text-gray-300 mb-1", {t!("import-field-channel")} }
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
                label { class: "text-sm dark:text-gray-300", {t!("import-auto-sync")} }
            }
        }
    }
}

fn render_jobs(jobs: &Resource<Result<Vec<ImportJobRow>, ServerFnError>>) -> Element {
    match &*jobs.read() {
        Some(Ok(list)) if list.is_empty() => rsx! {
            p { class: "text-sm text-gray-500 dark:text-gray-400", {t!("import-no-jobs")} }
        },
        Some(Ok(list)) => rsx! {
            div { class: "space-y-2",
                for job in list {
                    div { class: "text-sm border dark:border-gray-700 rounded p-3",
                        div { class: "flex items-center justify-between mb-1",
                            {
                                let ts = job.created_at.format("%Y-%m-%d %H:%M").to_string();
                                rsx! {
                                    span { class: "font-mono text-xs text-gray-500 dark:text-gray-400", "{ts}" }
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
                                {t!("import-skills-imported", count: job.skills_imported)}
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
        },
        Some(Err(e)) => rsx! { p { class: "text-sm text-red-500", {t!("error-message", message: e.to_string())} } },
        None => rsx! { p { class: "text-sm text-gray-500", {t!("loading")} } },
    }
}
