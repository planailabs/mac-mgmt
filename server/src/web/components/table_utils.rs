use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::models::{Bundle, Cluster, McpServer, McpServerBundle, Skill};
use crate::web::app::Route;

// ── Admin list helper ──────────────────────────────────────────────

/// Load a list of admin-only rows with auth + pool boilerplate.
#[cfg(feature = "server")]
pub async fn load_admin_list<T>(query: &str) -> Result<Vec<T>, ServerFnError>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    sqlx::query_as::<_, T>(query)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

// ── Searchable trait ────────────────────────────────────────────────

pub trait Searchable {
    fn matches_search(&self, query: &str) -> bool;
}

impl Searchable for Cluster {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query)
    }
}

impl Searchable for Skill {
    fn matches_search(&self, query: &str) -> bool {
        self.slug.to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }
}

impl Searchable for Bundle {
    fn matches_search(&self, query: &str) -> bool {
        self.slug.to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }
}

impl Searchable for McpServer {
    fn matches_search(&self, query: &str) -> bool {
        self.slug.to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }
}

impl Searchable for McpServerBundle {
    fn matches_search(&self, query: &str) -> bool {
        self.slug.to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }
}

// ── Row impls ───────────────────────────────────────────────────────

impl Row for Cluster {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

impl Row for Skill {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

impl Row for Bundle {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

impl Row for McpServer {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

impl Row for McpServerBundle {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

// ── Shared data types ───────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum ViaKind {
    SkillCenter,
    McpCenter,
}

#[derive(Clone, PartialEq)]
pub struct LinkData {
    pub label: String,
    pub route: Route,
    pub mono: bool,
    /// When set, renders gray text with a "via { source } { kind }" suffix
    /// instead of a clickable link — federated rows are intentionally not
    /// interactive, so the gray signals "informational, not actionable".
    pub remote_source: Option<String>,
    /// Discriminates the suffix label rendered next to `remote_source`
    /// (Skill Center vs MCP Center). Should be `Some` whenever
    /// `remote_source` is.
    pub via_kind: Option<ViaKind>,
}

#[derive(Clone, PartialEq)]
pub struct TextData(pub String);

#[derive(Clone, PartialEq)]
pub struct CreatedAtData(pub String);

// ── GetRowData: Cluster (Link by name + CreatedAt) ─────────────────

impl GetRowData<LinkData> for Cluster {
    fn get(&self) -> LinkData {
        LinkData {
            label: self.name.clone(),
            route: Route::ClusterDetail {
                id: self.id.to_string(),
            },
            mono: false,
            remote_source: None,
            via_kind: None,
        }
    }
}

impl GetRowData<CreatedAtData> for Cluster {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

impl GetRowData<VersionData> for Cluster {
    fn get(&self) -> VersionData {
        VersionData(self.pinned_version.clone())
    }
}

impl GetRowData<NixpkgsCommitData> for Cluster {
    fn get(&self) -> NixpkgsCommitData {
        NixpkgsCommitData(self.nixpkgs_commit.clone())
    }
}

// ── GetRowData: Skill (Link by slug + Name text + CreatedAt) ────────

impl GetRowData<LinkData> for Skill {
    fn get(&self) -> LinkData {
        LinkData {
            label: self.slug.clone(),
            route: Route::SkillDetail {
                id: self.id.to_string(),
            },
            mono: true,
            remote_source: None,
            via_kind: None,
        }
    }
}

impl GetRowData<TextData> for Skill {
    fn get(&self) -> TextData {
        TextData(self.name.clone())
    }
}

impl GetRowData<CreatedAtData> for Skill {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

// ── GetRowData: Bundle ──────────────────────────────────────────────

impl GetRowData<LinkData> for Bundle {
    fn get(&self) -> LinkData {
        LinkData {
            label: self.slug.clone(),
            route: Route::BundleDetail {
                id: self.id.to_string(),
            },
            mono: true,
            remote_source: None,
            via_kind: None,
        }
    }
}

impl GetRowData<TextData> for Bundle {
    fn get(&self) -> TextData {
        TextData(self.name.clone())
    }
}

impl GetRowData<CreatedAtData> for Bundle {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

// ── GetRowData: McpServer ───────────────────────────────────────────

impl GetRowData<LinkData> for McpServer {
    fn get(&self) -> LinkData {
        LinkData {
            label: self.slug.clone(),
            route: Route::McpServerDetail {
                id: self.id.to_string(),
            },
            mono: true,
            remote_source: None,
            via_kind: None,
        }
    }
}

impl GetRowData<TextData> for McpServer {
    fn get(&self) -> TextData {
        TextData(self.name.clone())
    }
}

impl GetRowData<CreatedAtData> for McpServer {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

// ── GetRowData: McpServerBundle ─────────────────────────────────────

impl GetRowData<LinkData> for McpServerBundle {
    fn get(&self) -> LinkData {
        LinkData {
            label: self.slug.clone(),
            route: Route::McpBundleDetail {
                id: self.id.to_string(),
            },
            mono: true,
            remote_source: None,
            via_kind: None,
        }
    }
}

impl GetRowData<TextData> for McpServerBundle {
    fn get(&self) -> TextData {
        TextData(self.name.clone())
    }
}

impl GetRowData<CreatedAtData> for McpServerBundle {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

// ── CatalogEntry: unified type for local + remote catalog items ─────

/// A unified catalog entry for listing pages that can represent either
/// a local item or a remote item from a skill center.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CatalogEntry {
    /// Synthetic UUID — for local items this is the real ID, for remote
    /// items it's a deterministic hash to satisfy the Row trait.
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub hide_from_public_catalog: bool,
    /// Non-None for remote items from skill centers.
    pub skill_center_name: Option<String>,
    /// Route discriminant for local items (e.g. "skill", "mcp_server").
    pub route_kind: String,
}

impl Searchable for CatalogEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.slug.to_lowercase().contains(query)
            || self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
            || self
                .skill_center_name
                .as_deref()
                .map(|s| s.to_lowercase().contains(query))
                .unwrap_or(false)
    }
}

impl Row for CatalogEntry {
    fn key(&self) -> impl Into<String> {
        self.id.to_string()
    }
}

impl GetRowData<LinkData> for CatalogEntry {
    fn get(&self) -> LinkData {
        let (route, via_kind) = match self.route_kind.as_str() {
            "skill" => (
                Route::SkillDetail {
                    id: self.id.to_string(),
                },
                ViaKind::SkillCenter,
            ),
            "bundle" => (
                Route::BundleDetail {
                    id: self.id.to_string(),
                },
                ViaKind::SkillCenter,
            ),
            "mcp_server" => (
                Route::McpServerDetail {
                    id: self.id.to_string(),
                },
                ViaKind::McpCenter,
            ),
            "mcp_bundle" => (
                Route::McpBundleDetail {
                    id: self.id.to_string(),
                },
                ViaKind::McpCenter,
            ),
            _ => (Route::SkillList {}, ViaKind::SkillCenter),
        };
        LinkData {
            label: self.slug.clone(),
            route,
            mono: true,
            via_kind: self.skill_center_name.as_ref().map(|_| via_kind),
            remote_source: self.skill_center_name.clone(),
        }
    }
}

impl GetRowData<TextData> for CatalogEntry {
    fn get(&self) -> TextData {
        TextData(self.name.clone())
    }
}

impl GetRowData<CreatedAtData> for CatalogEntry {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(
            self.created_at
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
        )
    }
}

// ── Shared column types ─────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub struct LinkColumn {
    pub header: &'static str,
}

impl<R: Row + GetRowData<LinkData>> TableColumn<R> for LinkColumn {
    fn column_name(&self) -> String {
        self.header.to_lowercase()
    }

    fn render_header(&self, context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        let header = self.header;
        let indicator = sort_indicator(context);
        rsx! {
            th { class: "th-sortable",
                onclick: move |_| toggle_sort(context),
                "{header} {indicator}"
            }
        }
    }

    fn render_cell(
        &self,
        _context: ColumnContext,
        row: &R,
        _attributes: Vec<Attribute>,
    ) -> Element {
        let data: LinkData = row.get();
        if let Some(source) = &data.remote_source {
            let mono_class = if data.mono { " font-mono text-sm" } else { "" };
            let source = source.clone();
            let via = match data.via_kind {
                Some(ViaKind::McpCenter) => t!("table-via-mcp-center", source: source),
                _ => t!("table-via-skill-center", source: source),
            };
            rsx! {
                td { class: "td",
                    span { class: "text-fg-muted{mono_class}", "{data.label}" }
                    span { class: "text-xs text-fg-faint ml-2", "{via}" }
                }
            }
        } else {
            let class = if data.mono { "link-mono" } else { "link" };
            rsx! {
                td { class: "td",
                    Link { to: data.route, class, "{data.label}" }
                }
            }
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: LinkData = a.get();
        let b: LinkData = b.get();
        a.label.to_lowercase().cmp(&b.label.to_lowercase())
    }
}

#[derive(Clone, PartialEq)]
pub struct TextColumn {
    pub header: &'static str,
}

impl<R: Row + GetRowData<TextData>> TableColumn<R> for TextColumn {
    fn column_name(&self) -> String {
        self.header.to_lowercase()
    }

    fn render_header(&self, context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        let header = self.header;
        let indicator = sort_indicator(context);
        rsx! {
            th { class: "th-sortable",
                onclick: move |_| toggle_sort(context),
                "{header} {indicator}"
            }
        }
    }

    fn render_cell(
        &self,
        _context: ColumnContext,
        row: &R,
        _attributes: Vec<Attribute>,
    ) -> Element {
        let data: TextData = row.get();
        rsx! {
            td { class: "td", "{data.0}" }
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: TextData = a.get();
        let b: TextData = b.get();
        a.0.to_lowercase().cmp(&b.0.to_lowercase())
    }
}

#[derive(Clone, PartialEq)]
pub struct CreatedAtColumn;

impl<R: Row + GetRowData<CreatedAtData>> TableColumn<R> for CreatedAtColumn {
    fn column_name(&self) -> String {
        "created".into()
    }

    fn render_header(&self, context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        let indicator = sort_indicator(context);
        rsx! {
            th { class: "th-sortable",
                onclick: move |_| toggle_sort(context),
                "Created {indicator}"
            }
        }
    }

    fn render_cell(
        &self,
        _context: ColumnContext,
        row: &R,
        _attributes: Vec<Attribute>,
    ) -> Element {
        let data: CreatedAtData = row.get();
        rsx! {
            td { class: "td-muted", "{data.0}" }
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: CreatedAtData = a.get();
        let b: CreatedAtData = b.get();
        a.0.cmp(&b.0)
    }
}

// ── Version column ──────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub struct VersionData(pub Option<String>);

#[derive(Clone, PartialEq)]
pub struct VersionColumn;

impl<R: Row + GetRowData<VersionData>> TableColumn<R> for VersionColumn {
    fn column_name(&self) -> String {
        "version".into()
    }

    fn render_header(&self, context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        let indicator = sort_indicator(context);
        rsx! {
            th { class: "th-sortable",
                onclick: move |_| toggle_sort(context),
                "Version {indicator}"
            }
        }
    }

    fn render_cell(
        &self,
        _context: ColumnContext,
        row: &R,
        _attributes: Vec<Attribute>,
    ) -> Element {
        let data: VersionData = row.get();
        match data.0 {
            Some(ver) => rsx! {
                td { class: "td",
                    span { class: "font-mono text-sm", {t!("table-version-prefix", version: ver)} }
                }
            },
            None => rsx! {
                td { class: "td-muted text-sm", {t!("dash")} }
            },
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: VersionData = a.get();
        let b: VersionData = b.get();
        a.0.cmp(&b.0)
    }
}

// ── Nixpkgs commit column ───────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub struct NixpkgsCommitData(pub Option<String>);

#[derive(Clone, PartialEq)]
pub struct NixpkgsCommitColumn;

impl<R: Row + GetRowData<NixpkgsCommitData>> TableColumn<R> for NixpkgsCommitColumn {
    fn column_name(&self) -> String {
        "nixpkgs".into()
    }

    fn render_header(&self, context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        let indicator = sort_indicator(context);
        rsx! {
            th { class: "th-sortable",
                onclick: move |_| toggle_sort(context),
                "Nixpkgs {indicator}"
            }
        }
    }

    fn render_cell(
        &self,
        _context: ColumnContext,
        row: &R,
        _attributes: Vec<Attribute>,
    ) -> Element {
        let data: NixpkgsCommitData = row.get();
        match data.0 {
            Some(commit) => {
                let short = commit.chars().take(7).collect::<String>();
                rsx! {
                    td { class: "td",
                        span { class: "font-mono text-sm", "{short}" }
                    }
                }
            }
            None => rsx! {
                td { class: "td-muted text-sm", {t!("dash")} }
            },
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: NixpkgsCommitData = a.get();
        let b: NixpkgsCommitData = b.get();
        a.0.cmp(&b.0)
    }
}

// ── Sort helpers ────────────────────────────────────────────────────

pub fn sort_indicator(context: ColumnContext) -> &'static str {
    match context.sort_info() {
        Some(info) => match info.direction {
            SortDirection::Ascending => "\u{2191}",
            SortDirection::Descending => "\u{2193}",
        },
        None => "",
    }
}

pub fn toggle_sort(context: ColumnContext) {
    match context.sort_info() {
        None => context.request_sort(SortGesture::AddFirst(Sort {
            direction: SortDirection::Ascending,
        })),
        Some(info) => match info.direction {
            SortDirection::Ascending => context.request_sort(SortGesture::AddFirst(Sort {
                direction: SortDirection::Descending,
            })),
            SortDirection::Descending => context.request_sort(SortGesture::Cancel),
        },
    }
}

// SortableTh and TableToolbar live in `ui::data_table` now, re-exported
// here so existing list pages don't need to update their imports.
pub use crate::web::components::ui::data_table::{SortableTh, TableToolbar};
