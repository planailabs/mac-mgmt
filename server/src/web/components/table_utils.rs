use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::models::{Bundle, Customer, McpServer, McpServerBundle, Skill};
use crate::web::app::Route;

// ── Searchable trait ────────────────────────────────────────────────

pub trait Searchable {
    fn matches_search(&self, query: &str) -> bool;
}

impl Searchable for Customer {
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

impl Row for Customer {
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

#[derive(Clone, PartialEq)]
pub struct LinkData {
    pub label: String,
    pub route: Route,
    pub mono: bool,
}

#[derive(Clone, PartialEq)]
pub struct TextData(pub String);

#[derive(Clone, PartialEq)]
pub struct CreatedAtData(pub String);

// ── GetRowData: Customer (Link by name + CreatedAt) ─────────────────

impl GetRowData<LinkData> for Customer {
    fn get(&self) -> LinkData {
        LinkData {
            label: self.name.clone(),
            route: Route::CustomerDetail {
                id: self.id.to_string(),
            },
            mono: false,
        }
    }
}

impl GetRowData<CreatedAtData> for Customer {
    fn get(&self) -> CreatedAtData {
        CreatedAtData(self.created_at.format("%Y-%m-%d %H:%M").to_string())
    }
}

impl GetRowData<VersionData> for Customer {
    fn get(&self) -> VersionData {
        VersionData(self.pinned_version.clone())
    }
}

impl GetRowData<NixpkgsCommitData> for Customer {
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
            th {
                class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase cursor-pointer select-none hover:text-gray-700",
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
        let class = if data.mono {
            "text-blue-600 hover:underline font-mono text-sm"
        } else {
            "text-blue-600 hover:underline"
        };
        rsx! {
            td { class: "px-6 py-4",
                Link { to: data.route, class, "{data.label}" }
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
            th {
                class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase cursor-pointer select-none hover:text-gray-700",
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
            td { class: "px-6 py-4", "{data.0}" }
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
            th {
                class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase cursor-pointer select-none hover:text-gray-700",
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
            td { class: "px-6 py-4 text-gray-500", "{data.0}" }
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
            th {
                class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase cursor-pointer select-none hover:text-gray-700",
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
                td { class: "px-6 py-4",
                    span { class: "font-mono text-sm text-gray-700", "v{ver}" }
                }
            },
            None => rsx! {
                td { class: "px-6 py-4 text-gray-400 text-sm", "-" }
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
            th {
                class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase cursor-pointer select-none hover:text-gray-700",
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
                    td { class: "px-6 py-4",
                        span { class: "font-mono text-sm text-gray-700", "{short}" }
                    }
                }
            }
            None => rsx! {
                td { class: "px-6 py-4 text-gray-400 text-sm", "-" }
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

fn sort_indicator(context: ColumnContext) -> &'static str {
    match context.sort_info() {
        Some(info) => match info.direction {
            SortDirection::Ascending => "\u{2191}",
            SortDirection::Descending => "\u{2193}",
        },
        None => "",
    }
}

fn toggle_sort(context: ColumnContext) {
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

// ── TableToolbar component ──────────────────────────────────────────

#[component]
pub fn TableToolbar(
    search: Signal<String>,
    limit: Signal<usize>,
    total: usize,
    filtered: usize,
    shown: usize,
) -> Element {
    rsx! {
        div { class: "flex items-center justify-between mb-3 gap-4",
            div { class: "relative",
                input {
                    class: "border border-gray-300 rounded px-3 py-1.5 text-sm w-64 pl-8",
                    r#type: "text",
                    placeholder: "Search\u{2026}",
                    value: "{search}",
                    oninput: move |evt| search.set(evt.value()),
                }
                svg {
                    class: "absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-gray-400",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    view_box: "0 0 24 24",
                    circle { cx: "11", cy: "11", r: "8" }
                    line { x1: "21", y1: "21", x2: "16.65", y2: "16.65" }
                }
            }
            div { class: "flex items-center gap-3 text-sm text-gray-500",
                if total != filtered {
                    span { "Showing {shown} of {filtered} (filtered from {total})" }
                } else {
                    span { "Showing {shown} of {total}" }
                }
                select {
                    class: "border border-gray-300 rounded px-2 py-1.5 text-sm bg-white",
                    value: "{limit}",
                    onchange: move |evt| {
                        if let Ok(n) = evt.value().parse::<usize>() {
                            limit.set(n);
                        }
                    },
                    option { value: "20", "20 per page" }
                    option { value: "50", "50 per page" }
                    option { value: "100", "100 per page" }
                }
            }
        }
    }
}
