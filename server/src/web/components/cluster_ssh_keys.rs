use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::certificates::{
    SshKeyAddInput, SshKeyRemoveInput, SshKeysListInput, add_ssh_key, list_ssh_keys,
    remove_ssh_key,
};
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};

#[component]
pub fn ClusterSshKeys(cluster_id: String, read_only: bool) -> Element {
    let cid_list = cluster_id.clone();
    let mut keys = use_server_future(move || {
        let cid = cid_list.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_ssh_keys(SshKeysListInput { cluster_id }).await
        }
    })?;

    let mut key_input = use_signal(String::new);
    let mut error_msg = use_signal(|| None::<String>);

    let cid_add = cluster_id.clone();

    rsx! {
        if !read_only {
            if let Some(err) = &*error_msg.read() {
                ErrorText { class: "mb-2", "{err}" }
            }
            form { class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add.clone();
                    let pk = key_input.read().clone();
                    spawn(async move {
                        let cluster_id: uuid::Uuid = match cid.parse() {
                            Ok(id) => id,
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                                return;
                            }
                        };
                        if !pk.trim().is_empty() {
                            match add_ssh_key(SshKeyAddInput { cluster_id, public_key: pk }).await {
                                Ok(()) => {
                                    error_msg.set(None);
                                    key_input.set(String::new());
                                    keys.restart();
                                }
                                Err(e) => {
                                    error_msg.set(Some(e.to_string()));
                                }
                            }
                        }
                    });
                },
                textarea { class: "input flex-1 w-auto py-1 text-sm font-mono",
                    rows: 2,
                    placeholder: t!("ssh-keys-placeholder"),
                    value: "{key_input}",
                    oninput: move |e| key_input.set(e.value()),
                }
                Button {
                    kind: ButtonKind::Submit,
                    size: ButtonSize::Sm,
                    class: "self-start",
                    {t!("add")}
                }
            }
        }
        {match &*keys.read() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                HelpText { {t!("ssh-keys-no-keys")} }
            },
            Some(Ok(list)) => rsx! {
                ul { class: "divide-y divide-line-soft",
                    for key in list {
                        {
                            let kid = key.id.to_string();
                            let fp = key.fingerprint.clone();
                            let comment = key.comment.clone();
                            rsx! {
                                li { class: "py-2 flex justify-between items-center",
                                    div {
                                        span { class: "text-sm font-mono", "{fp}" }
                                        if !comment.is_empty() {
                                            span { class: "text-xs text-fg-muted ml-2", "{comment}" }
                                        }
                                    }
                                    if !read_only {
                                        button { class: "link-danger text-sm",
                                            onclick: move |_| {
                                                let kid = kid.clone();
                                                spawn(async move {
                                                    let Ok(id) = kid.parse::<uuid::Uuid>() else {
                                                        return;
                                                    };
                                                    if remove_ssh_key(SshKeyRemoveInput { id })
                                                        .await
                                                        .is_ok()
                                                    {
                                                        keys.restart();
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
