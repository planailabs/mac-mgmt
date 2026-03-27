use dioxus::prelude::*;

use crate::models::CustomerConfig;

#[server]
async fn get_current_config(customer_id: String) -> Result<Option<CustomerConfig>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let config = sqlx::query_as::<_, CustomerConfig>(
        "SELECT * FROM customer_configs WHERE customer_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(config)
}

#[server]
async fn save_config(customer_id: String, config_toml: String) -> Result<(), ServerFnError> {
    config_toml
        .parse::<toml::Value>()
        .map_err(|e| ServerFnError::new(format!("invalid TOML: {e}")))?;

    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO customer_configs (customer_id, config_toml) VALUES ($1, $2)")
        .bind(uuid)
        .bind(&config_toml)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn ConfigEditor(customer_id: String) -> Element {
    let cid = customer_id.clone();
    let mut config = use_server_future(move || {
        let cid = cid.clone();
        async move { get_current_config(cid).await }
    })?;

    let mut editor_text = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut initialized = use_signal(|| false);

    if !*initialized.read() {
        if let Some(Ok(Some(cfg))) = &*config.read() {
            editor_text.set(cfg.config_toml.clone());
            initialized.set(true);
        }
    }

    let cid_save = customer_id.clone();
    let on_save = move |evt: FormEvent| {
        evt.prevent_default();
        let cid = cid_save.clone();
        let text = editor_text.read().clone();
        spawn(async move {
            match save_config(cid, text).await {
                Ok(()) => {
                    error.set(None);
                    config.restart();
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 text-sm mb-2", "{err}" }
        }

        form { onsubmit: on_save,
            textarea {
                class: "w-full h-64 font-mono text-sm border border-gray-300 rounded p-2 mb-2",
                placeholder: "Paste TOML config here...",
                value: "{editor_text}",
                oninput: move |evt| editor_text.set(evt.value()),
            }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                r#type: "submit",
                "Save Config"
            }
        }

        {match &*config.read() {
            Some(Ok(Some(cfg))) => {
                let saved_at = cfg.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                rsx! {
                    div { class: "mt-4",
                        p { class: "text-xs text-gray-500", "Last saved: {saved_at}" }
                    }
                }
            }
            Some(Ok(None)) => rsx! {
                p { class: "text-sm text-gray-500 mt-2", "No config saved yet." }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm mt-2", "Error: {e}" } },
            None => rsx! { p { class: "text-sm mt-2", "Loading..." } },
        }}
    }
}
