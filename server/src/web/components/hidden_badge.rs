use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::models::{Bundle, McpServer, McpServerBundle, Skill};
use crate::web::components::table_utils::CatalogEntry;
use crate::web::components::table_utils::{sort_indicator, toggle_sort};

/// Small "Hidden" pill rendered when an entity has `hide_from_public_catalog = true`.
/// Renders nothing when `hidden` is false so callers can drop it in unconditionally.
#[component]
pub fn HiddenBadge(hidden: bool) -> Element {
    if !hidden {
        return rsx! {};
    }
    rsx! {
        span {
            class: "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-200 dark:bg-gray-600 text-gray-700 dark:text-gray-200 border border-gray-300 dark:border-gray-500",
            title: "Hidden from public catalog",
            "Hidden"
        }
    }
}

// ── Table column ────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub struct HiddenData(pub bool);

#[derive(Clone, PartialEq)]
pub struct HiddenColumn;

impl<R: Row + GetRowData<HiddenData>> TableColumn<R> for HiddenColumn {
    fn column_name(&self) -> String {
        "hidden".into()
    }

    fn render_header(&self, context: ColumnContext, _attributes: Vec<Attribute>) -> Element {
        let indicator = sort_indicator(context);
        rsx! {
            th {
                class: "px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase cursor-pointer select-none hover:text-gray-700 dark:hover:text-gray-200",
                onclick: move |_| toggle_sort(context),
                "Visibility {indicator}"
            }
        }
    }

    fn render_cell(
        &self,
        _context: ColumnContext,
        row: &R,
        _attributes: Vec<Attribute>,
    ) -> Element {
        let data: HiddenData = row.get();
        rsx! {
            td { class: "px-6 py-4",
                HiddenBadge { hidden: data.0 }
            }
        }
    }

    fn compare(&self, a: &R, b: &R) -> std::cmp::Ordering {
        let a: HiddenData = a.get();
        let b: HiddenData = b.get();
        a.0.cmp(&b.0)
    }
}

// ── GetRowData impls ────────────────────────────────────────────────

impl GetRowData<HiddenData> for Skill {
    fn get(&self) -> HiddenData {
        HiddenData(self.hide_from_public_catalog)
    }
}

impl GetRowData<HiddenData> for Bundle {
    fn get(&self) -> HiddenData {
        HiddenData(self.hide_from_public_catalog)
    }
}

impl GetRowData<HiddenData> for McpServer {
    fn get(&self) -> HiddenData {
        HiddenData(self.hide_from_public_catalog)
    }
}

impl GetRowData<HiddenData> for McpServerBundle {
    fn get(&self) -> HiddenData {
        HiddenData(self.hide_from_public_catalog)
    }
}

impl GetRowData<HiddenData> for CatalogEntry {
    fn get(&self) -> HiddenData {
        HiddenData(self.hide_from_public_catalog)
    }
}
