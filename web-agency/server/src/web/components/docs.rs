//! Built-in markdown knowledgebase.
//!
//! Markdown files in `server/docs/` are embedded at compile time via rust-embed
//! and rendered to HTML via pulldown-cmark. Files support YAML-like frontmatter
//! for audience tags and sort ordering.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::ui::{Badge, BadgeVariant, Card, PageHeader};

// ── Embedded assets ───────────────────────────────────────────────────

#[cfg(feature = "server")]
mod embedded {
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "docs/"]
    #[include = "*.md"]
    pub struct DocsAssets;
}

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocEntry {
    pub slug: String,
    pub title: String,
    pub audience: String,
    pub ordering: i32,
}

// ── Frontmatter parsing ──────────────────────────────────────────────

#[cfg(feature = "server")]
fn parse_frontmatter(content: &str) -> (std::collections::HashMap<String, String>, &str) {
    let mut meta = std::collections::HashMap::new();
    if !content.starts_with("---") {
        return (meta, content);
    }
    if let Some(end) = content[3..].find("---") {
        let fm = &content[3..3 + end];
        for line in fm.lines() {
            if let Some((key, value)) = line.split_once(':') {
                meta.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        let body_start = 3 + end + 3;
        return (meta, &content[body_start..]);
    }
    (meta, content)
}

#[cfg(feature = "server")]
fn extract_title(body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            return title.trim().to_string();
        }
    }
    "Untitled".to_string()
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn list_docs() -> Result<Vec<DocEntry>, ServerFnError> {
    use embedded::DocsAssets;

    let mut entries = Vec::new();
    for path in DocsAssets::iter() {
        let path_str = path.as_ref();
        if !path_str.ends_with(".md") {
            continue;
        }
        let slug = path_str.trim_end_matches(".md").to_string();

        if let Some(file) = DocsAssets::get(path_str) {
            let content = std::str::from_utf8(file.data.as_ref()).unwrap_or("");
            let (meta, body) = parse_frontmatter(content);
            let title = extract_title(body);
            let audience = meta.get("audience").cloned().unwrap_or_default();
            let ordering = meta
                .get("ordering_override")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            entries.push(DocEntry { slug, title, audience, ordering });
        }
    }

    entries.sort_by(|a, b| a.ordering.cmp(&b.ordering).then(a.title.cmp(&b.title)));
    Ok(entries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocContent {
    title: String,
    html: String,
    audience: String,
}

#[server]
async fn get_doc(slug: String) -> Result<DocContent, ServerFnError> {
    use embedded::DocsAssets;
    use pulldown_cmark::{Options, Parser, html};

    let filename = format!("{slug}.md");
    let file = DocsAssets::get(&filename)
        .ok_or_else(|| ServerFnError::new("document not found"))?;

    let content = std::str::from_utf8(file.data.as_ref())
        .map_err(|e| ServerFnError::new(format!("invalid UTF-8: {e}")))?;

    let (meta, body) = parse_frontmatter(content);
    let title = extract_title(body);
    let audience = meta.get("audience").cloned().unwrap_or_default();

    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(body, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    Ok(DocContent { title, html: html_output, audience })
}

// ── Components ────────────────────────────────────────────────────────

#[component]
fn AudienceBadge(audience: String) -> Element {
    match audience.as_str() {
        "admin" => rsx! { Badge { variant: BadgeVariant::Accent, "Admin" } },
        "user" => rsx! { Badge { variant: BadgeVariant::Info, "User" } },
        "developer" => rsx! { Badge { variant: BadgeVariant::Success, "Developer" } },
        _ => rsx! {},
    }
}

#[component]
pub fn DocList() -> Element {
    let docs = use_server_future(list_docs)?;
    let entries = match &*docs.read() {
        Some(Ok(e)) => e.clone(),
        _ => vec![],
    };

    let user_docs: Vec<_> = entries.iter().filter(|e| e.audience == "user").cloned().collect();
    let dev_docs: Vec<_> = entries.iter().filter(|e| e.audience == "developer").cloned().collect();
    let admin_docs: Vec<_> = entries.iter().filter(|e| e.audience == "admin").cloned().collect();
    let other_docs: Vec<_> = entries.iter().filter(|e| e.audience.is_empty() || !["user", "developer", "admin"].contains(&e.audience.as_str())).cloned().collect();

    rsx! {
        PageHeader { "Documentation" }

        if entries.is_empty() {
            Card { div { class: "p-6 text-fg-muted",
                "No documentation pages found. Add " code { class: "font-mono bg-surface-2 px-1 rounded", ".md" }
                " files to " code { class: "font-mono bg-surface-2 px-1 rounded", "server/docs/" }
                " and rebuild."
            }}
        }

        if !user_docs.is_empty() {
            DocGroup { title: "User Guides", docs: user_docs }
        }
        if !dev_docs.is_empty() {
            DocGroup { title: "Developer", docs: dev_docs }
        }
        if !admin_docs.is_empty() {
            DocGroup { title: "Administration", docs: admin_docs }
        }
        if !other_docs.is_empty() {
            DocGroup { title: "Other", docs: other_docs }
        }
    }
}

#[component]
fn DocGroup(title: &'static str, docs: Vec<DocEntry>) -> Element {
    rsx! {
        h3 { class: "h-section mt-4", "{title}" }
        Card { div { class: "divide-y divide-line-soft",
            for doc in docs {
                Link {
                    to: crate::web::app::Route::DocPage { slug: doc.slug.clone() },
                    class: "flex items-center justify-between px-4 py-3 hover:bg-surface-2 transition-colors",
                    span { class: "text-fg", "{doc.title}" }
                    AudienceBadge { audience: doc.audience.clone() }
                }
            }
        }}
    }
}

#[component]
pub fn DocPage(slug: String) -> Element {
    let doc = use_server_future(move || {
        let s = slug.clone();
        async move { get_doc(s).await }
    })?;

    let content = match &*doc.read() {
        Some(Ok(c)) => c.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        div { class: "mb-4",
            Link {
                to: crate::web::app::Route::DocList {},
                class: "text-sm text-brand hover:underline",
                "← Back to docs"
            }
        }

        div { class: "flex items-center gap-3 mb-4",
            PageHeader { "{content.title}" }
            AudienceBadge { audience: content.audience }
        }

        Card { div { class: "p-6 prose dark:prose-invert max-w-none",
            dangerous_inner_html: "{content.html}",
        }}
    }
}
