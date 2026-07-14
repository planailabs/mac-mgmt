use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::certificates::{
    ClusterCaAddInput, ClusterCaRemoveInput, ClusterCasListInput, add_cluster_ca,
    list_cluster_cas, remove_cluster_ca,
};
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};

#[component]
pub fn ClusterClientCas(cluster_id: String, read_only: bool) -> Element {
    let cid_list = cluster_id.clone();
    let mut cas = use_server_future(move || {
        let cid = cid_list.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_cluster_cas(ClusterCasListInput { cluster_id }).await
        }
    })?;

    let mut pem_input = use_signal(String::new);
    let mut label_input = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    let cid_add = cluster_id.clone();

    rsx! {
        if !read_only {
            if let Some(err) = &*error_msg.read() {
                ErrorText { class: "mb-2", "{err}" }
            }
            form { class: "flex flex-col gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add.clone();
                    let pem = pem_input.read().clone();
                    let lbl = label_input.read().clone();
                    spawn(async move {
                        let cluster_id: uuid::Uuid = match cid.parse() {
                            Ok(id) => id,
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                                return;
                            }
                        };
                        if !pem.trim().is_empty() {
                            match add_cluster_ca(ClusterCaAddInput {
                                cluster_id,
                                certificate_pem: pem,
                                label: lbl,
                            })
                            .await
                            {
                                Ok(()) => {
                                    error_msg.set(None);
                                    pem_input.set(String::new());
                                    label_input.set(String::new());
                                    cas.restart();
                                }
                                Err(e) => error_msg.set(Some(e.to_string())),
                            }
                        }
                    });
                },
                div { class: "flex gap-2",
                    input { class: "input flex-1 w-auto py-1 text-sm",
                        placeholder: "{t!(\"client-certs-label-placeholder\")}",
                        value: "{label_input}",
                        oninput: move |e| label_input.set(e.value()),
                    }
                    Button {
                        kind: ButtonKind::Submit,
                        size: ButtonSize::Sm,
                        class: "self-start",
                        {t!("add")}
                    }
                }
                textarea { class: "input w-full py-1 text-xs font-mono h-20",
                    placeholder: "{t!(\"client-cas-pem-placeholder\")}",
                    value: "{pem_input}",
                    oninput: move |e| pem_input.set(e.value()),
                }
            }
        }
        {match &*cas.read() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                HelpText { {t!("client-cas-no-cas")} }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-line-soft",
                    for ca in list {
                        {
                            let cid = ca.id.to_string();
                            let fp = ca.fingerprint.clone();
                            let label = ca.label.clone();
                            rsx! {
                                li { class: "py-2 flex justify-between items-center",
                                    div {
                                        span { class: "text-sm font-mono", "{fp}" }
                                        if !label.is_empty() {
                                            span { class: "text-xs text-fg-muted ml-2", "{label}" }
                                        }
                                    }
                                    if !read_only {
                                        button { class: "link-danger text-sm",
                                            onclick: move |_| {
                                                let cid = cid.clone();
                                                spawn(async move {
                                                    let Ok(id) = cid.parse::<uuid::Uuid>() else {
                                                        return;
                                                    };
                                                    if remove_cluster_ca(ClusterCaRemoveInput {
                                                        id,
                                                    })
                                                    .await
                                                    .is_ok()
                                                    {
                                                        cas.restart();
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
}
