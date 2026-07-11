//! Baked-in action templates (Ansible-like YAML task lists from the repo's
//! `actions/` dir). Read-only: templates change via code review, not the UI.

use dioxus::prelude::*;

use super::ui::{Card, PageHeader, Td, TdMuted, Th};
use crate::api_mcp::endpoints::action_templates::{
    ActionTemplateListInput, ActionTemplateRow, list_action_templates,
};

#[component]
pub fn ActionTemplateList() -> Element {
    let templates =
        use_server_future(move || list_action_templates(ActionTemplateListInput::default()))?;
    let rows: Vec<ActionTemplateRow> = match &*templates.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };

    rsx! {
        PageHeader { "Action Templates" }
        p { class: "mt-2 mb-4 text-sm text-fg-muted",
            "Server-side task lists baked into this build. Run one with parameters; every step is a regular API action executed with your permissions."
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Name" }
                            Th { "Description" }
                            Th { "Inputs" }
                            Th { "Steps" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No action templates in this build" } }
                        }
                        for row in &rows {
                            {
                                let name = row.name.clone();
                                rsx! {
                                    tr { class: "hover:bg-surface-2 cursor-pointer",
                                        onclick: move |_| {
                                            navigator().push(crate::web::app::Route::ActionTemplateDetail { id: name.clone() });
                                        },
                                        Td { class: "font-mono text-brand", "{row.name}" }
                                        TdMuted { "{row.description}" }
                                        TdMuted { "{row.num_inputs}" }
                                        TdMuted { "{row.num_steps}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
