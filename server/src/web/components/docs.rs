use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::api_mcp::endpoints::docs::{
    GlossaryGetInput, GlossaryListInput, GlossarySearchInput, glossary_term, glossary_terms,
    search_glossary,
};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Badge, BadgeVariant, ErrorText};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocEntry {
    pub slug: String,
    pub title: String,
    pub audience: String,
    pub ordering_override: Option<i32>,
}

#[cfg(feature = "server")]
pub mod embedded {
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "docs/"]
    #[include = "*.md"]
    pub struct DocsAssets;
}

/// Parse YAML frontmatter from markdown content.
/// Returns (frontmatter_pairs, body_without_frontmatter).
#[cfg(feature = "server")]
pub fn parse_frontmatter(content: &str) -> (Vec<(String, String)>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (vec![], content);
    }
    // Find closing ---
    if let Some(end) = trimmed[3..].find("\n---") {
        let front = &trimmed[3..3 + end];
        let body_start = 3 + end + 4; // skip past "\n---"
        let body = trimmed[body_start..].trim_start_matches('\n');
        let pairs = front
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let (key, val) = line.split_once(':')?;
                Some((key.trim().to_string(), val.trim().to_string()))
            })
            .collect();
        (pairs, body)
    } else {
        (vec![], content)
    }
}

#[server]
async fn list_docs() -> Result<Vec<DocEntry>, ServerFnError> {
    use embedded::DocsAssets;

    let mut entries: Vec<DocEntry> = DocsAssets::iter()
        .filter_map(|path| {
            let path_str = path.as_ref();
            if !path_str.ends_with(".md") {
                return None;
            }
            let slug = path_str.trim_end_matches(".md").to_string();
            let content = DocsAssets::get(path_str)?;
            let text = std::str::from_utf8(content.data.as_ref()).ok()?;
            let (frontmatter, body) = parse_frontmatter(text);
            let audience = frontmatter
                .iter()
                .find(|(k, _)| k == "audience")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let ordering_override = frontmatter
                .iter()
                .find(|(k, _)| k == "ordering_override")
                .and_then(|(_, v)| v.parse::<i32>().ok());
            let title = body
                .lines()
                .find(|l| l.starts_with("# "))
                .map(|l| l.trim_start_matches("# ").to_string())
                .unwrap_or_else(|| slug.replace('-', " "));
            Some(DocEntry {
                slug,
                title,
                audience,
                ordering_override,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        let oa = a.ordering_override.unwrap_or(0);
        let ob = b.ordering_override.unwrap_or(0);
        oa.cmp(&ob).then_with(|| a.title.cmp(&b.title))
    });
    Ok(entries)
}

#[server]
async fn get_doc(slug: String) -> Result<(String, String, String), ServerFnError> {
    use embedded::DocsAssets;

    let filename = format!("{slug}.md");
    let file = DocsAssets::get(&filename)
        .ok_or_else(|| ServerFnError::new(format!("Document '{slug}' not found")))?;
    let markdown =
        std::str::from_utf8(file.data.as_ref()).map_err(|e| ServerFnError::new(e.to_string()))?;

    let (frontmatter, body) = parse_frontmatter(markdown);
    let audience = frontmatter
        .iter()
        .find(|(k, _)| k == "audience")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    let title = body
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").to_string())
        .unwrap_or_else(|| slug.replace('-', " "));

    let html_output = crate::web::components::chat_ui::simple_md_to_html(body);

    Ok((title, html_output, audience))
}

#[component]
fn AudienceBadge(audience: String) -> Element {
    match audience.as_str() {
        "admin" => rsx! {
            Badge { variant: BadgeVariant::Accent, {t!("docs-badge-admin")} }
        },
        "user" => rsx! {
            Badge { variant: BadgeVariant::Success, {t!("docs-badge-user")} }
        },
        _ => rsx! {},
    }
}

#[component]
pub fn DocList() -> Element {
    use_topbar(t!("docs-title"), None);
    let docs = use_server_future(list_docs)?;

    rsx! {
        div {
            h2 { class: "h-page", {t!("docs-title")} }
            {match &*docs.read() {
                Some(Ok(entries)) => {
                    let user_docs: Vec<_> = entries.iter().filter(|e| e.audience == "user").collect();
                    let admin_docs: Vec<_> = entries.iter().filter(|e| e.audience == "admin").collect();
                    let other_docs: Vec<_> = entries.iter().filter(|e| e.audience != "user" && e.audience != "admin").collect();
                    rsx! {
                        if !user_docs.is_empty() {
                            h3 { class: "h-section text-fg mt-6", {t!("docs-user-guides")} }
                            div { class: "card divide-y divide-line-soft mb-6",
                                for entry in &user_docs {
                                    Link {
                                        key: "{entry.slug}",
                                        to: Route::DocPage { slug: entry.slug.clone() },
                                        class: "flex items-center justify-between px-6 py-4 hover:bg-surface-2 transition-colors",
                                        div {
                                            h4 { class: "text-lg font-medium text-brand", "{entry.title}" }
                                            p { class: "text-sm text-fg-muted mt-1", "{entry.slug}" }
                                        }
                                        AudienceBadge { audience: entry.audience.clone() }
                                    }
                                }
                            }
                        }
                        if !admin_docs.is_empty() {
                            h3 { class: "h-section text-fg mt-6", {t!("docs-administration")} }
                            div { class: "card divide-y divide-line-soft mb-6",
                                for entry in &admin_docs {
                                    Link {
                                        key: "{entry.slug}",
                                        to: Route::DocPage { slug: entry.slug.clone() },
                                        class: "flex items-center justify-between px-6 py-4 hover:bg-surface-2 transition-colors",
                                        div {
                                            h4 { class: "text-lg font-medium text-brand", "{entry.title}" }
                                            p { class: "text-sm text-fg-muted mt-1", "{entry.slug}" }
                                        }
                                        AudienceBadge { audience: entry.audience.clone() }
                                    }
                                }
                            }
                        }
                        if !other_docs.is_empty() {
                            div { class: "card divide-y divide-line-soft",
                                for entry in &other_docs {
                                    Link {
                                        key: "{entry.slug}",
                                        to: Route::DocPage { slug: entry.slug.clone() },
                                        class: "flex items-center justify-between px-6 py-4 hover:bg-surface-2 transition-colors",
                                        div {
                                            h4 { class: "text-lg font-medium text-brand", "{entry.title}" }
                                            p { class: "text-sm text-fg-muted mt-1", "{entry.slug}" }
                                        }
                                    }
                                }
                            }
                        }
                        if entries.is_empty() {
                            div { class: "card",
                                p { class: "px-6 py-8 text-fg-muted text-center",
                                    {t!("docs-none-prefix")}
                                    code { {t!("docs-none-md")} }
                                    " "
                                    code { {t!("docs-none-suffix")} }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { p { {t!("loading")} } },
            }}

            // Glossary lives on its own page.
            div { class: "mt-6",
                Link {
                    to: Route::GlossaryPage {},
                    class: "link text-sm",
                    {t!("docs-glossary-link")}
                }
            }
        }
    }
}

/// Searchable glossary of platform terms (server/glossary/*.md).
#[component]
pub fn GlossaryPage() -> Element {
    use_topbar(t!("docs-glossary"), None);
    let terms = use_server_future(|| glossary_terms(GlossaryListInput {}))?;
    let mut query = use_signal(String::new);

    // Server-side content search whenever the query changes (debounce-free:
    // the corpus is tiny). Empty query = full term list.
    let results = use_resource(move || {
        let q = query.read().trim().to_string();
        async move {
            if q.is_empty() {
                return None;
            }
            Some(search_glossary(GlossarySearchInput { query: q }).await)
        }
    });

    rsx! {
        h2 { class: "h-page", {t!("docs-glossary")} }
        input {
            class: "input w-full mb-3",
            placeholder: t!("glossary-search-placeholder").to_string(),
            value: "{query}",
            oninput: move |e| query.set(e.value()),
        }
        {match &*results.read() {
            Some(Some(Ok(hits))) if hits.is_empty() => rsx! {
                p { class: "text-sm text-fg-muted", {t!("glossary-no-results")} }
            },
            Some(Some(Ok(hits))) => rsx! {
                div { class: "card divide-y divide-line-soft mb-6",
                    for hit in hits.iter() {
                        GlossaryTermRow {
                            key: "{hit.term}",
                            term: hit.term.clone(),
                            snippet: Some(hit.snippet.clone()),
                        }
                    }
                }
            },
            Some(Some(Err(e))) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            _ => match &*terms.read() {
                Some(Ok(list)) => rsx! {
                    div { class: "card divide-y divide-line-soft mb-6",
                        for t in list.iter() {
                            GlossaryTermRow { key: "{t.term}", term: t.term.clone(), snippet: None }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { p { {t!("loading")} } },
            },
        }}
    }
}

/// One glossary row; the definition loads and renders on expand.
#[component]
fn GlossaryTermRow(term: String, snippet: Option<String>) -> Element {
    let mut open = use_signal(|| false);
    let term_fetch = term.clone();
    let entry = use_resource(move || {
        let term = term_fetch.clone();
        let load = *open.read();
        async move {
            if !load {
                return None;
            }
            Some(glossary_term(GlossaryGetInput { id: term }).await)
        }
    });

    rsx! {
        div {
            button {
                r#type: "button",
                class: "w-full text-left px-6 py-3 hover:bg-surface-2 transition-colors",
                onclick: move |_| { let v = *open.read(); open.set(!v); },
                h4 { class: "font-medium text-brand", "{term}" }
                if let Some(sn) = &snippet {
                    p { class: "text-sm text-fg-muted mt-1", "{sn}" }
                }
            }
            if *open.read() {
                div { class: "px-6 pb-4",
                    {match &*entry.read() {
                        Some(Some(Ok(e))) => rsx! {
                            div {
                                class: "prose-chat text-sm",
                                dangerous_inner_html: crate::web::components::chat_ui::simple_md_to_html(&e.markdown),
                            }
                        },
                        Some(Some(Err(err))) => rsx! { ErrorText { {t!("error-message", message: err.to_string())} } },
                        _ => rsx! { p { class: "text-sm text-fg-muted", {t!("loading")} } },
                    }}
                }
            }
        }
    }
}

#[component]
pub fn DocPage(slug: String) -> Element {
    let slug_clone = slug.clone();
    let doc = use_server_future(move || get_doc(slug_clone.clone()))?;

    rsx! {
        div {
            Link {
                to: Route::DocList {},
                class: "link text-sm mb-4 inline-block",
                {t!("docs-back")}
            }
            {match &*doc.read() {
                Some(Ok((_, html_content, audience))) => rsx! {
                    div { class: "card px-8 py-6",
                        if !audience.is_empty() {
                            div { class: "mb-4",
                                AudienceBadge { audience: audience.clone() }
                            }
                        }
                        article {
                            class: "prose dark:prose-invert max-w-none",
                            dangerous_inner_html: "{html_content}",
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { p { {t!("loading")} } },
            }}
        }
    }
}
