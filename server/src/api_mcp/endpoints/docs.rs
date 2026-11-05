//! Built-in platform documentation (server/docs/<lang>/*.md) exposed as read-only
//! tools, so agents can look things up when the platform behavior is unclear.

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    // NOTE: path segment must not be "docs" — /api/v1/docs is the Swagger UI
    // mount and axum panics on the route collision at startup.
    let mut d = reg.resource("documentation", "doc", "Documentation");
    d.list(
        "List the built-in platform documentation topics (slug + title + audience). \
         Use doc_get to read one. Optional lang for translated titles.",
        |_pool: sqlx::PgPool, _p, input: DocListInput| async move { doc_list(input).await },
    );
    d.get(
        "Read a documentation page as raw markdown by its slug (from doc_list). \
         Optional lang for a translated version (falls back to English).",
        |_pool: sqlx::PgPool, _p, input: DocGetInput| async move { doc_get(input).await },
    );

    let mut g = reg.resource("glossary", "glossary", "Glossary");
    g.list(
        "List all platform glossary terms (Cluster, Daemon, Skill, Healer, ...).          Use glossary_get to read a definition.",
        |pool: sqlx::PgPool, p, input: GlossaryListInput| async move {
            glossary_list(&pool, &p, input).await
        },
    );
    g.get(
        "Read the definition of one glossary term (case-insensitive).",
        |pool: sqlx::PgPool, p, input: GlossaryGetInput| async move {
            glossary_get(&pool, &p, input).await
        },
    );
    g.custom(
        "search",
        plan_ai_api_mcp::Risk::ReadOnly,
        plan_ai_api_mcp::OnItem::No,
        "Search glossary term names and definitions (case-insensitive substring);          returns matching terms with a snippet.",
        |pool: sqlx::PgPool, p, input: GlossarySearchInput| async move {
            glossary_search(&pool, &p, input).await
        },
    );
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DocListInput {
    /// Language tag ("en-US", "de-DE", "de"); titles come from the
    /// translated docs when available. Default: English.
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DocGetInput {
    /// Documentation slug, e.g. "configuration-reference".
    pub id: String,
    /// Language tag ("en-US", "de-DE", "de"); falls back to English when
    /// no translation exists. Default: English.
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DocInfo {
    pub slug: String,
    pub title: String,
    pub audience: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DocPage {
    pub slug: String,
    pub title: String,
    /// Raw markdown body (frontmatter stripped).
    pub markdown: String,
}

#[cfg(feature = "server")]
async fn doc_list(input: DocListInput) -> Result<Vec<DocInfo>, plan_ai_api_mcp::ApiError> {
    use crate::web::components::docs::{
        DOC_CANONICAL_LANG, embedded::DocsAssets, load_doc_markdown, parse_frontmatter,
    };

    let lang = input.lang.unwrap_or_default();
    let mut out: Vec<DocInfo> = DocsAssets::iter()
        .filter_map(|path| {
            let path_str = path.as_ref();
            // The canonical language directory defines the doc list.
            let name = path_str.strip_prefix(DOC_CANONICAL_LANG)?.strip_prefix('/')?;
            let slug = name.strip_suffix(".md")?.to_string();
            if slug.contains('/') {
                return None;
            }
            let content = DocsAssets::get(path_str)?;
            let text = std::str::from_utf8(content.data.as_ref()).ok()?;
            let (frontmatter, body) = parse_frontmatter(text);
            let audience = frontmatter
                .iter()
                .find(|(k, _)| k == "audience")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let translated = load_doc_markdown(&slug, &lang);
            let title_body = match &translated {
                Some(t) => parse_frontmatter(t).1,
                None => body,
            };
            let title = title_body
                .lines()
                .find(|l| l.starts_with("# "))
                .map(|l| l.trim_start_matches("# ").to_string())
                .unwrap_or_else(|| slug.replace('-', " "));
            Some(DocInfo {
                slug,
                title,
                audience,
            })
        })
        .collect();
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

#[cfg(feature = "server")]
async fn doc_get(input: DocGetInput) -> Result<DocPage, plan_ai_api_mcp::ApiError> {
    use crate::web::components::docs::{load_doc_markdown, parse_frontmatter};

    let lang = input.lang.unwrap_or_default();
    let text = load_doc_markdown(&input.id, &lang)
        .ok_or_else(|| plan_ai_api_mcp::ApiError::not_found("unknown documentation slug"))?;
    let (_, body) = parse_frontmatter(&text);
    let title = body
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").to_string())
        .unwrap_or_else(|| input.id.replace('-', " "));
    Ok(DocPage {
        slug: input.id,
        title,
        markdown: body.to_string(),
    })
}

// ── Glossary ────────────────────────────────────────────────────────────
//
// server/glossary/*.md — one file per term (filename = term), embedded like
// the docs. Exposed as tools AND as the #[server] wrappers the docs UI uses.

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

#[cfg(feature = "server")]
mod glossary_embedded {
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "glossary/"]
    #[include = "*.md"]
    pub struct GlossaryAssets;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlossaryListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlossaryGetInput {
    /// Term name, e.g. "Healer" (case-insensitive; from glossary_list).
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlossarySearchInput {
    /// Case-insensitive substring searched in term names and definitions.
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlossaryTermInfo {
    pub term: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlossaryEntry {
    pub term: String,
    /// Definition as raw markdown.
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GlossaryHit {
    pub term: String,
    /// The matching line, trimmed.
    pub snippet: String,
}

#[cfg(feature = "server")]
fn glossary_read(term_file: &str) -> Option<String> {
    let content = glossary_embedded::GlossaryAssets::get(term_file)?;
    std::str::from_utf8(content.data.as_ref())
        .ok()
        .map(String::from)
}

/// List all glossary terms.
#[api_mcp_dioxus_server(server = "glossary_terms")]
pub async fn glossary_list(
    _pool: &sqlx::PgPool,
    _p: &Principal,
    _input: GlossaryListInput,
) -> Result<Vec<GlossaryTermInfo>, ApiError> {
    let mut terms: Vec<GlossaryTermInfo> = glossary_embedded::GlossaryAssets::iter()
        .filter_map(|path| {
            let term = path.as_ref().strip_suffix(".md")?.to_string();
            Some(GlossaryTermInfo { term })
        })
        .collect();
    terms.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));
    Ok(terms)
}

/// Read one glossary term (case-insensitive lookup).
#[api_mcp_dioxus_server(server = "glossary_term")]
pub async fn glossary_get(
    _pool: &sqlx::PgPool,
    _p: &Principal,
    input: GlossaryGetInput,
) -> Result<GlossaryEntry, ApiError> {
    let wanted = input.id.to_lowercase();
    let path = glossary_embedded::GlossaryAssets::iter()
        .find(|p| {
            p.as_ref()
                .strip_suffix(".md")
                .is_some_and(|t| t.to_lowercase() == wanted)
        })
        .ok_or_else(|| ApiError::not_found("unknown glossary term"))?;
    let term = path.as_ref().trim_end_matches(".md").to_string();
    let markdown = glossary_read(path.as_ref())
        .ok_or_else(|| ApiError::internal("glossary entry unreadable"))?;
    Ok(GlossaryEntry { term, markdown })
}

/// Search term names and definitions.
#[api_mcp_dioxus_server(server = "search_glossary")]
pub async fn glossary_search(
    _pool: &sqlx::PgPool,
    _p: &Principal,
    input: GlossarySearchInput,
) -> Result<Vec<GlossaryHit>, ApiError> {
    let q = input.query.trim().to_lowercase();
    if q.is_empty() {
        return Err(ApiError::bad_request("query is empty"));
    }
    let mut hits = Vec::new();
    for path in glossary_embedded::GlossaryAssets::iter() {
        let Some(term) = path.as_ref().strip_suffix(".md").map(String::from) else {
            continue;
        };
        let Some(text) = glossary_read(path.as_ref()) else {
            continue;
        };
        if term.to_lowercase().contains(&q) {
            let snippet = text
                .lines()
                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .unwrap_or("")
                .trim()
                .to_string();
            hits.push(GlossaryHit { term, snippet });
            continue;
        }
        if let Some(line) = text
            .lines()
            .find(|l| l.to_lowercase().contains(&q) && !l.starts_with('#'))
        {
            hits.push(GlossaryHit {
                term,
                snippet: line.trim().to_string(),
            });
        }
    }
    hits.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));
    Ok(hits)
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn principal() -> Principal {
        Principal::admin("test")
    }

    fn pool() -> sqlx::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .expect("lazy pool")
    }

    #[tokio::test]
    async fn glossary_search_is_case_insensitive() {
        for q in ["hermes", "HERMES", "Hermes", "openclaw", "gateway"] {
            let hits = glossary_search(
                &pool(),
                &principal(),
                GlossarySearchInput {
                    query: q.to_string(),
                },
            )
            .await
            .expect("search");
            assert!(!hits.is_empty(), "no hits for query '{q}'");
        }
    }

    #[tokio::test]
    async fn glossary_get_is_case_insensitive() {
        for id in ["Hermes", "hermes", "HERMES", "openclaw"] {
            let entry = glossary_get(
                &pool(),
                &principal(),
                GlossaryGetInput { id: id.to_string() },
            )
            .await
            .expect("get");
            assert!(!entry.markdown.is_empty());
        }
    }
}
