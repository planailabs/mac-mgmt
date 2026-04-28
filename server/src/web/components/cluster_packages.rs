use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Server functions ────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
struct PackageRow {
    id: String,
    package: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct PackageWithSources {
    package: String,
    sources: Vec<String>,
}

#[server]
async fn can_write_cluster(cluster_id: String) -> Result<bool, ServerFnError> {
    let user = current_user().await?;
    if user.is_admin {
        return Ok(true);
    }
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    match user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        Some(ids) => Ok(ids.contains(&uuid)),
        None => Ok(true),
    }
}

#[server]
async fn get_cluster_name(cluster_id: String) -> Result<String, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let name: String = sqlx::query_scalar("SELECT name FROM clusters WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(name)
}

#[server]
async fn list_manual_packages(cluster_id: String) -> Result<Vec<PackageRow>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        package: String,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, package FROM cluster_packages WHERE cluster_id = $1 ORDER BY package",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| PackageRow {
            id: r.id.to_string(),
            package: r.package,
        })
        .collect())
}

#[server]
async fn add_manual_package(
    cluster_id: String,
    package: String,
) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let pkg = package.trim().to_string();
    if pkg.is_empty() {
        return Err(ServerFnError::new("package name is empty"));
    }

    sqlx::query(
        "INSERT INTO cluster_packages (cluster_id, package) VALUES ($1, $2) \
         ON CONFLICT (cluster_id, package) DO NOTHING",
    )
    .bind(uuid)
    .bind(&pkg)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncPackages).await;
    Ok(())
}

#[server]
async fn remove_manual_package(id: String, cluster_id: String) -> Result<(), ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let pkg_id: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    sqlx::query("DELETE FROM cluster_packages WHERE id = $1 AND cluster_id = $2")
        .bind(pkg_id)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncPackages).await;
    Ok(())
}

#[server]
async fn list_all_packages(
    cluster_id: String,
) -> Result<Vec<PackageWithSources>, ServerFnError> {
    use std::collections::HashMap;
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let mut packages: HashMap<String, Vec<String>> = HashMap::new();

    // MCP server packages (direct + bundle)
    #[derive(sqlx::FromRow)]
    struct PkgRow {
        slug: String,
        nix_packages: Vec<String>,
    }
    let mcp_rows: Vec<PkgRow> = sqlx::query_as(
        "SELECT ms.slug, ms.nix_packages \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 AND ms.nix_packages != '{}' \
         UNION ALL \
         SELECT ms.slug, ms.nix_packages \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.cluster_id = $1 AND ms.nix_packages != '{}'",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for row in &mcp_rows {
        for pkg in &row.nix_packages {
            packages
                .entry(pkg.clone())
                .or_default()
                .push(format!("mcp:{}", row.slug));
        }
    }

    // Skill channel packages
    #[derive(sqlx::FromRow)]
    struct SkillRow {
        skill_slug: String,
        nix_packages: Vec<String>,
    }
    let skill_rows: Vec<SkillRow> = sqlx::query_as(
        "SELECT s.slug AS skill_slug, sc.nix_packages \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1 AND sc.nix_packages != '{}' \
         UNION ALL \
         SELECT s.slug AS skill_slug, sc.nix_packages \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.cluster_id = $1 AND sc.nix_packages != '{}'",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for row in &skill_rows {
        for pkg in &row.nix_packages {
            packages
                .entry(pkg.clone())
                .or_default()
                .push(format!("skill:{}", row.skill_slug));
        }
    }

    // Manual packages
    let manual: Vec<String> = sqlx::query_scalar(
        "SELECT package FROM cluster_packages WHERE cluster_id = $1",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for pkg in manual {
        packages.entry(pkg).or_default().push("manual".to_string());
    }

    // Deduplicate and sort
    let mut result: Vec<PackageWithSources> = packages
        .into_iter()
        .map(|(package, mut sources)| {
            sources.sort();
            sources.dedup();
            PackageWithSources { package, sources }
        })
        .collect();
    result.sort_by(|a, b| a.package.cmp(&b.package));

    Ok(result)
}

// ── Page component ──────────────────────────────────────────────────────

#[component]
pub fn ClusterPackagesPage(id: String) -> Element {
    let cid_name = id.clone();
    let cluster_name = use_server_future(move || {
        let cid = cid_name.clone();
        async move { get_cluster_name(cid).await }
    })?;

    let cid_write = id.clone();
    let write_check = use_server_future(move || {
        let cid = cid_write.clone();
        async move { can_write_cluster(cid).await }
    })?;

    let can_write = matches!(&*write_check.read(), Some(Ok(true)));
    let read_only = !can_write;

    let name = match &*cluster_name.read() {
        Some(Ok(n)) => n.clone(),
        _ => String::new(),
    };

    let cid_manual = id.clone();
    let mut manual_pkgs = use_server_future(move || {
        let cid = cid_manual.clone();
        async move { list_manual_packages(cid).await }
    })?;

    let cid_all = id.clone();
    let mut all_pkgs = use_server_future(move || {
        let cid = cid_all.clone();
        async move { list_all_packages(cid).await }
    })?;

    let mut new_pkg = use_signal(String::new);
    let mut error: Signal<Option<String>> = use_signal(|| None);
    let id_form = id.clone();
    let id_list = id.clone();

    rsx! {
        div { class: "mb-4",
            Link {
                to: Route::ClusterDetail { id: id.clone() },
                class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                "← {t!(\"back\")}"
            }
            if !name.is_empty() {
                h2 { class: "text-2xl font-bold mt-1", "{name} — {t!(\"cluster-packages-title\")}" }
            }
        }

        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 mb-4", "{err}" }
        }

        div { class: "space-y-6",
            // Manual packages section
            div {
                h3 { class: "text-lg font-semibold mb-3", {t!("cluster-packages-manual")} }
                if !read_only {
                    form {
                        class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let cid = id_form.clone();
                            let pkg = new_pkg.read().clone();
                            spawn(async move {
                                if !pkg.trim().is_empty() {
                                    match add_manual_package(cid, pkg).await {
                                        Ok(()) => {
                                            new_pkg.set(String::new());
                                            error.set(None);
                                            manual_pkgs.restart();
                                            all_pkgs.restart();
                                        }
                                        Err(e) => error.set(Some(e.to_string())),
                                    }
                                }
                            });
                        },
                        input {
                            class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm font-mono dark:bg-gray-700 dark:text-white",
                            r#type: "text",
                            placeholder: t!("cluster-packages-add-placeholder"),
                            value: "{new_pkg}",
                            oninput: move |e| new_pkg.set(e.value()),
                        }
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                            r#type: "submit",
                            {t!("add")}
                        }
                    }
                }
                {match &*manual_pkgs.read() {
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        p { class: "text-sm text-gray-400 dark:text-gray-500", {t!("cluster-packages-no-manual")} }
                    },
                    Some(Ok(list)) => rsx! {
                        ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                            for pkg in list {
                                {
                                    let pkg_id = pkg.id.clone();
                                    let pkg_name = pkg.package.clone();
                                    let cid = id_list.clone();
                                    rsx! {
                                        li { class: "py-2 flex justify-between items-center",
                                            span { class: "font-mono text-sm", "{pkg_name}" }
                                            if !read_only {
                                                button {
                                                    class: "text-xs text-red-600 dark:text-red-400 hover:underline",
                                                    onclick: move |_| {
                                                        let pid = pkg_id.clone();
                                                        let c = cid.clone();
                                                        spawn(async move {
                                                            if remove_manual_package(pid, c).await.is_ok() {
                                                                manual_pkgs.restart();
                                                                all_pkgs.restart();
                                                            }
                                                        });
                                                    },
                                                    {t!("remove")}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", {t!("error-message", message: e.to_string())} } },
                    None => rsx! { p { class: "text-sm", {t!("loading")} } },
                }}
            }

            // All packages overview
            div {
                h3 { class: "text-lg font-semibold mb-3", {t!("cluster-packages-all")} }
                {match &*all_pkgs.read() {
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        p { class: "text-sm text-gray-400 dark:text-gray-500", "No packages from any source." }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "overflow-x-auto",
                            table { class: "min-w-full text-sm",
                                thead {
                                    tr { class: "border-b border-gray-200 dark:border-gray-700",
                                        th { class: "text-left py-2 pr-4 font-semibold", "Package" }
                                        th { class: "text-left py-2 font-semibold", "Sources" }
                                    }
                                }
                                tbody {
                                    for pkg in list {
                                        {
                                            let name = pkg.package.clone();
                                            let sources = pkg.sources.clone();
                                            rsx! {
                                                tr { class: "border-b border-gray-100 dark:border-gray-800",
                                                    td { class: "py-2 pr-4 font-mono", "{name}" }
                                                    td { class: "py-2",
                                                        div { class: "flex flex-wrap gap-1",
                                                            for src in &sources {
                                                                {
                                                                    let (color, label) = source_badge(src);
                                                                    rsx! {
                                                                        span {
                                                                            class: "inline-block px-2 py-0.5 rounded text-xs font-medium {color}",
                                                                            "{label}"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", {t!("error-message", message: e.to_string())} } },
                    None => rsx! { p { class: "text-sm", {t!("loading")} } },
                }}
            }
        }
    }
}

fn source_badge(src: &str) -> (&'static str, String) {
    if let Some(slug) = src.strip_prefix("mcp:") {
        (
            "bg-purple-100 dark:bg-purple-900 text-purple-800 dark:text-purple-200",
            format!("MCP: {slug}"),
        )
    } else if let Some(slug) = src.strip_prefix("skill:") {
        (
            "bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-200",
            format!("Skill: {slug}"),
        )
    } else if src == "manual" {
        (
            "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
            "Manual".to_string(),
        )
    } else {
        (
            "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
            src.to_string(),
        )
    }
}
