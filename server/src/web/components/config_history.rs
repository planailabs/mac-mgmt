use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::clusters::{
    ConfigDiffInput, ConfigHistoryInput, DiffLine, get_config_diff, get_config_history,
};
use crate::web::components::ui::{Button, ButtonSize, ErrorText, HelpText};

#[component]
pub fn ConfigHistory(cluster_id: String) -> Element {
    let cid = cluster_id.clone();
    let history = use_server_future(move || {
        let id = cid.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_config_history(ConfigHistoryInput { id }).await
        }
    })?;

    let mut left_id = use_signal(|| Option::<String>::None);
    let mut right_id = use_signal(|| Option::<String>::None);
    let mut diff_lines = use_signal(|| Option::<Vec<DiffLine>>::None);
    let mut diff_loading = use_signal(|| false);
    let mut diff_error = use_signal(|| Option::<String>::None);

    match &*history.read() {
        Some(Ok(versions)) => {
            if versions.is_empty() {
                return rsx! { HelpText { {t!("config-history-no-history")} } };
            }

            let versions_left = versions.clone();
            let versions_right = versions.clone();

            rsx! {
                div { class: "mt-4",
                    div { class: "flex gap-4 mb-3 items-end",
                        div {
                            label { class: "label", {t!("config-history-left")} }
                            select { class: "input input-sm",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        left_id.set(None);
                                    } else {
                                        left_id.set(Some(val));
                                    }
                                },
                                option { value: "", {t!("config-history-select")} }
                                for v in &versions_left {
                                    {
                                        let vid = v.id.to_string();
                                        let label = v.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        rsx! { option { value: "{vid}", "{label}" } }
                                    }
                                }
                            }
                        }
                        div {
                            label { class: "label", {t!("config-history-right")} }
                            select { class: "input input-sm",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        right_id.set(None);
                                    } else {
                                        right_id.set(Some(val));
                                    }
                                },
                                option { value: "", {t!("config-history-select")} }
                                for v in &versions_right {
                                    {
                                        let vid = v.id.to_string();
                                        let label = v.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        rsx! { option { value: "{vid}", "{label}" } }
                                    }
                                }
                            }
                        }
                        Button { size: ButtonSize::Sm,
                            disabled: left_id.read().is_none()
                                || right_id.read().is_none()
                                || *diff_loading.read(),
                            onclick: move |_| {
                                let l = left_id.read().clone();
                                let r = right_id.read().clone();
                                async move {
                                    if let (Some(l), Some(r)) = (l, r) {
                                        diff_loading.set(true);
                                        diff_error.set(None);
                                        let result = async {
                                            let left_id: uuid::Uuid =
                                                l.parse().map_err(|e: uuid::Error| e.to_string())?;
                                            let right_id: uuid::Uuid =
                                                r.parse().map_err(|e: uuid::Error| e.to_string())?;
                                            get_config_diff(ConfigDiffInput { left_id, right_id })
                                                .await
                                                .map_err(|e| e.to_string())
                                        }
                                        .await;
                                        match result {
                                            Ok(lines) => { diff_lines.set(Some(lines)); }
                                            Err(e) => { diff_error.set(Some(e)); }
                                        }
                                        diff_loading.set(false);
                                    }
                                }
                            },
                            if *diff_loading.read() {
                                {t!("loading")}
                            } else {
                                {t!("config-history-compare")}
                            }
                        }
                    }

                    if let Some(error) = &*diff_error.read() {
                        ErrorText { "{error}" }
                    }

                    if let Some(lines) = &*diff_lines.read() {
                        div { class: "border border-line-soft rounded overflow-auto max-h-96",
                            pre { class: "text-xs font-mono p-2",
                                for line in lines {
                                    {
                                        let (cls, prefix) = match line.tag.as_str() {
                                            "insert" => ("bg-success-soft text-success", "+ "),
                                            "delete" => ("bg-danger-soft text-danger", "- "),
                                            _ => ("", "  "),
                                        };
                                        rsx! {
                                            span { class: "block {cls}", "{prefix}{line.content}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
