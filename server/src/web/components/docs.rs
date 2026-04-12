use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocEntry {
    pub slug: String,
    pub title: String,
}

#[cfg(feature = "server")]
mod embedded {
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "docs/"]
    #[include = "*.md"]
    pub struct DocsAssets;
}

#[server]
async fn list_docs() -> Result<Vec<DocEntry>, ServerFnError> {
    use embedded::DocsAssets;
    use rust_embed::RustEmbed;

    let mut entries: Vec<DocEntry> = DocsAssets::iter()
        .filter_map(|path| {
            let path_str = path.as_ref();
            if !path_str.ends_with(".md") {
                return None;
            }
            let slug = path_str.trim_end_matches(".md").to_string();
            let content = DocsAssets::get(path_str)?;
            let text = std::str::from_utf8(content.data.as_ref()).ok()?;
            // Extract title from first H1 heading, or use slug
            let title = text
                .lines()
                .find(|l| l.starts_with("# "))
                .map(|l| l.trim_start_matches("# ").to_string())
                .unwrap_or_else(|| slug.replace('-', " "));
            Some(DocEntry { slug, title })
        })
        .collect();
    entries.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(entries)
}

#[server]
async fn get_doc(slug: String) -> Result<(String, String), ServerFnError> {
    use embedded::DocsAssets;
    use pulldown_cmark::{Options, Parser, html};
    use rust_embed::RustEmbed;

    let filename = format!("{slug}.md");
    let file = DocsAssets::get(&filename)
        .ok_or_else(|| ServerFnError::new(format!("Document '{slug}' not found")))?;
    let markdown = std::str::from_utf8(file.data.as_ref())
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let title = markdown
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").to_string())
        .unwrap_or_else(|| slug.replace('-', " "));

    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    Ok((title, html_output))
}

#[component]
pub fn DocList() -> Element {
    let docs = use_server_future(list_docs)?;

    rsx! {
        div {
            h2 { class: "text-2xl font-bold mb-4", "Documentation" }
            {match &*docs.read() {
                Some(Ok(entries)) => rsx! {
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 divide-y divide-gray-200 dark:divide-gray-700",
                        for entry in entries {
                            Link {
                                key: "{entry.slug}",
                                to: Route::DocPage { slug: entry.slug.clone() },
                                class: "block px-6 py-4 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors",
                                h3 { class: "text-lg font-medium text-blue-600 dark:text-blue-400", "{entry.title}" }
                                p { class: "text-sm text-gray-500 dark:text-gray-400 mt-1", "{entry.slug}" }
                            }
                        }
                        if entries.is_empty() {
                            p { class: "px-6 py-8 text-gray-500 dark:text-gray-400 text-center",
                                "No documentation pages found. Add "
                                code { ".md" }
                                " files to the "
                                code { "server/docs/" }
                                " directory."
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
                None => rsx! { p { "Loading..." } },
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
                class: "text-sm text-blue-600 dark:text-blue-400 hover:underline mb-4 inline-block",
                "← Back to docs"
            }
            {match &*doc.read() {
                Some(Ok((title, html_content))) => rsx! {
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 px-8 py-6",
                        article {
                            class: "prose dark:prose-invert max-w-none",
                            dangerous_inner_html: "{html_content}",
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
                None => rsx! { p { "Loading..." } },
            }}
        }
    }
}
