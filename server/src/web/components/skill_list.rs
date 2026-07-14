use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::api_mcp::endpoints::skills::{
    CatalogEntryDto, SkillListInput, SkillSyncInput, list_skills, sync_from_xzar,
};
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonVariant, DataTable, ErrorText, HelpText, PageHeader, SuccessText, page_window,
};

/// Map the API DTO to the table row type shared by the catalog pages.
fn to_catalog_entry(e: &CatalogEntryDto) -> CatalogEntry {
    CatalogEntry {
        id: e.id,
        slug: e.slug.clone(),
        name: e.name.clone(),
        description: e.description.clone(),
        created_at: e.created_at,
        hide_from_public_catalog: e.hide_from_public_catalog,
        skill_center_name: e.skill_center_name.clone(),
        route_kind: e.route_kind.clone(),
    }
}

#[component]
pub fn SkillList() -> Element {
    use_topbar(t!("skill-list-title"), None);
    let mut skills = use_server_future(|| async move { list_skills(SkillListInput {}).await })?;
    let mut syncing = use_signal(|| false);
    let mut sync_msg = use_signal(|| None::<String>);
    let mut sync_err = use_signal(|| None::<String>);

    rsx! {
        // Stack on mobile so the action buttons don't overflow.
        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4",
            PageHeader { class: "mb-0", {t!("skill-list-title")} }
            div { class: "flex items-center gap-2 flex-wrap",
                {match &*skills.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter()
                            .filter(|e| e.skill_center_name.is_none())
                            .map(|s| GenerateAllItem {
                                id: s.id.to_string(),
                                name: s.name.clone(),
                                description: s.description.clone(),
                                context: GenerateContext::Skill { skill_id: s.id.to_string() },
                                entity_kind: EntityKind::Skill,
                            }).collect();
                        rsx! {
                            GenerateAllButton {
                                items: gen_items,
                                on_complete: move |_| { skills.restart(); },
                            }
                        }
                    },
                    _ => rsx! {},
                }}
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: *syncing.read(),
                    onclick: move |_| {
                        syncing.set(true);
                        sync_msg.set(None);
                        sync_err.set(None);
                        spawn(async move {
                            match sync_from_xzar(SkillSyncInput {}).await {
                                Ok(result) => {
                                    sync_msg.set(Some(format!(
                                        "Synced: +{} skills, +{} channels, -{} channels, -{} skills",
                                        result.created_skills, result.created_channels,
                                        result.removed_channels, result.removed_skills,
                                    )));
                                    skills.restart();
                                }
                                Err(e) => {
                                    sync_err.set(Some(e.to_string()));
                                }
                            }
                            syncing.set(false);
                        });
                    },
                    if *syncing.read() { {t!("skill-list-syncing")} } else { {t!("skill-list-sync")} }
                }
            }
        }
        if let Some(msg) = &*sync_msg.read() {
            SuccessText { class: "mb-4", "{msg}" }
        }
        if let Some(err) = &*sync_err.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        {match &*skills.read() {
            Some(Ok(list)) => rsx! { CatalogTable { list: list.iter().map(to_catalog_entry).collect::<Vec<_>>() } },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn CatalogTable(list: Vec<CatalogEntry>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let page = use_signal(|| 0usize);

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|s| s.matches_search(&q))
                .cloned()
                .collect()
        }
    });

    let total = list.len();
    let data = use_tabular(
        (
            LinkColumn { header: "Slug" },
            TextColumn { header: "Name" },
            HiddenColumn,
            CreatedAtColumn,
        ),
        filtered.into(),
    );
    let all_rows: Vec<_> = data.rows().collect();
    let filtered_count = all_rows.len();
    let limit_val = *limit.read();
    let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

    rsx! {
        DataTable {
            search, limit, page, total, filtered: filtered_count, shown,
            headers: rsx! { TableHeaders { data } },
            body: rsx! {
                for row in all_rows.into_iter().skip(start).take(limit_val) {
                    tr { key: "{row.key()}", TableCells { row } }
                }
            },
        }
    }
}
