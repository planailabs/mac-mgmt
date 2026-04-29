use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::models::{Bundle, McpServer, McpServerBundle, Skill};
use crate::web::components::table_utils::CatalogEntry;
use crate::web::components::table_utils::{sort_indicator, toggle_sort};
use crate::web::components::ui::{Badge, BadgeVariant};

/// Small "Hidden" pill rendered when an entity has `hide_from_public_catalog = true`.
/// Renders nothing when `hidden` is false so callers can drop it in unconditionally.
#[component]
pub fn HiddenBadge(hidden: bool) -> Element {
    if !hidden {
        return rsx! {};
    }
    rsx! {
        Badge { variant: BadgeVariant::Neutral, title: "Hidden from public catalog",
            {t!("hidden-badge")}
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
            th { class: "th-sortable",
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
            td { class: "td",
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
