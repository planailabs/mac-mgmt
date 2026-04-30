use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::ui::{Badge, BadgeVariant, ErrorText};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocEntry {
    pub slug: String,
    pub title: String,
    pub audience: String,
    pub ordering_override: Option<i32>,
}

#[cfg(feature = "server")]
mod embedded {
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "docs/"]
    #[include = "*.md"]
    pub struct DocsAssets;
}

/// Parse YAML frontmatter from markdown content.
/// Returns (frontmatter_pairs, body_without_frontmatter).
#[cfg(feature = "server")]
fn parse_frontmatter(content: &str) -> (Vec<(String, String)>, &str) {
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
    use pulldown_cmark::{Options, Parser, html};

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

    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(body, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

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
