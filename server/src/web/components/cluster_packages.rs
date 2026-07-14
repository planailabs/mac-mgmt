use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::clusters::{
    AllPackagesInput, ClusterCanWriteInput, ClusterGetInput, ManualPackagesInput, PackageAddInput,
    PackageRemoveInput, add_manual_package, can_write_cluster, get_cluster, list_all_packages,
    list_manual_packages, remove_manual_package,
};
use crate::web::app::Route;
use crate::web::components::ui::{
    Badge, BadgeVariant, Button, ButtonKind, ButtonSize, ErrorText, HelpText, PageHeader,
    SectionHeading,
};

// ── Page component ──────────────────────────────────────────────────────

#[component]
pub fn ClusterPackagesPage(id: String) -> Element {
    let cid_name = id.clone();
    let cluster_name = use_server_future(move || {
        let cid = cid_name.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            let cluster = get_cluster(ClusterGetInput { id }).await?;
            Ok::<String, ServerFnError>(cluster.name)
        }
    })?;

    let cid_write = id.clone();
    let write_check = use_server_future(move || {
        let cid = cid_write.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            can_write_cluster(ClusterCanWriteInput { id }).await
        }
    })?;

    let can_write = matches!(&*write_check.read(), Some(Ok(true)));
    let read_only = !can_write;

    let name = match &*cluster_name.read() {
        Some(Ok(n)) => n.clone(),
        _ => String::new(),
    };

    let cid_manual = id.clone();
    let mut manual_pkgs = use_server_future(move || {
        let cid = cid_manual.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_manual_packages(ManualPackagesInput { id }).await
        }
    })?;

    let cid_all = id.clone();
    let mut all_pkgs = use_server_future(move || {
        let cid = cid_all.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_all_packages(AllPackagesInput { id }).await
        }
    })?;

    let mut new_pkg = use_signal(String::new);
    let mut error: Signal<Option<String>> = use_signal(|| None);
    let id_form = id.clone();
    let id_list = id.clone();

    rsx! {
        div { class: "mb-4",
            Link { to: Route::ClusterDetail { id: id.clone() }, class: "link text-sm",
                "← {t!(\"back\")}"
            }
            if !name.is_empty() {
                PageHeader { class: "mt-1", "{name} — {t!(\"cluster-packages-title\")}" }
            }
        }

        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-4", "{err}" }
        }

        div { class: "space-y-6",
            // Manual packages section
            div {
                SectionHeading { {t!("cluster-packages-manual")} }
                if !read_only {
                    form { class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let cid = id_form.clone();
                            let pkg = new_pkg.read().clone();
                            spawn(async move {
                                let Ok(id) = cid.parse::<uuid::Uuid>() else {
                                    error.set(Some("invalid cluster id".to_string()));
                                    return;
                                };
                                if !pkg.trim().is_empty() {
                                    match add_manual_package(PackageAddInput { id, package: pkg })
                                        .await
                                    {
                                        Ok(()) => {
                                            new_pkg.set(String::new());
                                            error.set(None);
                                            manual_pkgs.restart();
                                            all_pkgs.restart();
                                        }
                                        Err(e) => error.set(Some(e.to_string())),
                                    }
                                }
                            });
                        },
                        input { class: "input flex-1 w-auto py-1 text-sm font-mono",
                            r#type: "text",
                            placeholder: t!("cluster-packages-add-placeholder"),
                            value: "{new_pkg}",
                            oninput: move |e| new_pkg.set(e.value()),
                        }
                        Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                            {t!("add")}
                        }
                    }
                }
                {match &*manual_pkgs.read() {
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        HelpText { {t!("cluster-packages-no-manual")} }
                    },
                    Some(Ok(list)) => rsx! {
                        ul { class: "divide-y divide-line-soft",
                            for pkg in list {
                                {
                                    let pkg_id = pkg.id.clone();
                                    let pkg_name = pkg.package.clone();
                                    let cid = id_list.clone();
                                    rsx! {
                                        li { class: "py-2 flex justify-between items-center",
                                            span { class: "font-mono text-sm", "{pkg_name}" }
                                            if !read_only {
                                                button { class: "link-danger text-xs",
                                                    onclick: move |_| {
                                                        let pid = pkg_id.clone();
                                                        let c = cid.clone();
                                                        spawn(async move {
                                                            let (Ok(id), Ok(package_id)) =
                                                                (c.parse::<uuid::Uuid>(), pid.parse::<uuid::Uuid>())
                                                            else {
                                                                return;
                                                            };
                                                            if remove_manual_package(PackageRemoveInput { id, package_id }).await.is_ok() {
                                                                manual_pkgs.restart();
                                                                all_pkgs.restart();
                                                            }
                                                        });
                                                    },
                                                    {t!("remove")}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                    None => rsx! { HelpText { {t!("loading")} } },
                }}
            }

            // All packages overview
            div {
                SectionHeading { {t!("cluster-packages-all")} }
                {match &*all_pkgs.read() {
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        HelpText { "No packages from any source." }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "overflow-x-auto",
                            table { class: "min-w-full text-sm",
                                thead {
                                    tr { class: "border-b border-line-soft",
                                        th { class: "text-left py-2 pr-4 font-semibold text-fg-strong", "Package" }
                                        th { class: "text-left py-2 font-semibold text-fg-strong", "Sources" }
                                    }
                                }
                                tbody {
                                    for pkg in list {
                                        {
                                            let name = pkg.package.clone();
                                            let sources = pkg.sources.clone();
                                            rsx! {
                                                tr { class: "border-b border-line-soft",
                                                    td { class: "py-2 pr-4 font-mono", "{name}" }
                                                    td { class: "py-2",
                                                        div { class: "flex flex-wrap gap-1",
                                                            for src in &sources {
                                                                {
                                                                    let (variant, label) = source_badge(src);
                                                                    rsx! {
                                                                        Badge { variant, "{label}" }
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
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                    None => rsx! { HelpText { {t!("loading")} } },
                }}
            }
        }
    }
}

fn source_badge(src: &str) -> (BadgeVariant, String) {
    if let Some(slug) = src.strip_prefix("mcp:") {
        (BadgeVariant::Accent, format!("MCP: {slug}"))
    } else if let Some(slug) = src.strip_prefix("skill:") {
        (BadgeVariant::Success, format!("Skill: {slug}"))
    } else if src == "manual" {
        (BadgeVariant::Info, "Manual".to_string())
    } else {
        (BadgeVariant::Neutral, src.to_string())
    }
}
