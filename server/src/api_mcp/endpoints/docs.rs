//! Built-in platform documentation (server/docs/*.md) exposed as read-only
//! tools, so agents can look things up when the platform behavior is unclear.

#[cfg(feature = "server")]
pub fn register(reg: &mut plan_ai_api_mcp::Registry<sqlx::PgPool>) {
    // NOTE: path segment must not be "docs" — /api/v1/docs is the Swagger UI
    // mount and axum panics on the route collision at startup.
    let mut d = reg.resource("documentation", "doc", "Documentation");
    d.list(
        "List the built-in platform documentation topics (slug + title + audience). \
         Use doc_get to read one.",
        |_pool: sqlx::PgPool, _p, _input: DocListInput| async move { doc_list().await },
    );
    d.get(
        "Read a documentation page as raw markdown by its slug (from doc_list).",
        |_pool: sqlx::PgPool, _p, input: DocGetInput| async move { doc_get(input).await },
    );
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DocListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DocGetInput {
    /// Documentation slug, e.g. "configuration-reference".
    pub id: String,
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
async fn doc_list() -> Result<Vec<DocInfo>, plan_ai_api_mcp::ApiError> {
    use crate::web::components::docs::{embedded::DocsAssets, parse_frontmatter};

    let mut out: Vec<DocInfo> = DocsAssets::iter()
        .filter_map(|path| {
            let path_str = path.as_ref();
            let slug = path_str.strip_suffix(".md")?.to_string();
            let content = DocsAssets::get(path_str)?;
            let text = std::str::from_utf8(content.data.as_ref()).ok()?;
            let (frontmatter, body) = parse_frontmatter(text);
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
    use crate::web::components::docs::{embedded::DocsAssets, parse_frontmatter};

    let path = format!("{}.md", input.id);
    let content = DocsAssets::get(&path)
        .ok_or_else(|| plan_ai_api_mcp::ApiError::not_found("unknown documentation slug"))?;
    let text = std::str::from_utf8(content.data.as_ref())
        .map_err(|e| plan_ai_api_mcp::ApiError::internal(e.to_string()))?;
    let (_, body) = parse_frontmatter(text);
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
