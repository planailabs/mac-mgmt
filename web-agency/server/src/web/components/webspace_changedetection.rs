use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td,
    TdMuted, Th,
};
// The change-detection server functions and their DTOs live in the shared
// api_mcp layer.
use crate::api_mcp::endpoints::changedetection::{
    CdConfig, CdGetInput, CdSetInput, NotificationGetInput, NotificationListInput,
    SubUrlCreateInput, SubUrlDeleteInput, SubUrlGetInput, SubUrlListInput, SubUrlUpdateInput,
    TagCondition, TagSettings, create_suburl, delete_suburl, get_cd_config, get_notification,
    get_suburl, list_notifications, list_suburls, set_webspace_changedetection,
    update_suburl_settings,
};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

// ── Server Functions ──────────────────────────────────────────────────

#[server]
async fn list_cd_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| CredOption { id, name })
        .collect())
}

// ── Components ────────────────────────────────────────────────────────

#[component]
pub fn WebspaceChangedetection(id: String) -> Element {
    let wid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid webspace ID" } },
    };

    let config =
        use_server_future(move || async move { get_cd_config(CdGetInput { id: wid }).await })?;
    let cfg = match &*config.read() {
        Some(Ok(c)) => c.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        PageHeader {
            Link {
                to: crate::web::app::Route::WebspaceHostDetail { id: id.clone() },
                class: "text-brand underline",
                "{cfg.webspace_name}"
            }
            " / Change Detection"
        }

        SectionHeading { "Credential" }
        CredentialSection { webspace_id: wid, config: config, current_credential_id: cfg.credential_id, current_credential_name: cfg.credential_name.clone() }

        SectionHeading { class: "mt-6", "Sub-URLs" }
        SubUrlsSection { webspace_id: wid, ws_id_str: id.clone() }

        SectionHeading { class: "mt-6", "Notifications" }
        NotificationsInbox { webspace_id: wid, ws_id_str: id.clone() }
    }
}

#[component]
fn CredentialSection(
    webspace_id: Uuid,
    config: Resource<Result<CdConfig, ServerFnError>>,
    current_credential_id: Option<Uuid>,
    current_credential_name: Option<String>,
) -> Element {
    let cd_creds = use_server_future(list_cd_creds)?;
    let cred_list: Vec<CredOption> = match &*cd_creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut selected_cred = use_signal(move || {
        current_credential_id
            .map(|id| id.to_string())
            .unwrap_or_default()
    });
    let mut saving = use_signal(|| false);
    let mut result_msg = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-center gap-3",
                    span { class: "text-sm text-fg-muted", "Current:" }
                    if let Some(ref name) = current_credential_name {
                        Badge { variant: BadgeVariant::Info, "{name}" }
                    } else {
                        span { class: "text-fg-muted text-sm", "Not configured" }
                    }
                }

                if cred_list.is_empty() {
                    div { class: "text-sm text-fg-muted",
                        "No ChangeDetection.io credentials available. "
                        Link {
                            to: crate::web::app::Route::CredentialForm {},
                            class: "text-brand underline",
                            "Add one"
                        }
                    }
                } else {
                    div { class: "flex items-end gap-3",
                        FormField { label: "Credential",
                            select {
                                class: "input w-64",
                                value: "{selected_cred}",
                                oninput: move |evt| selected_cred.set(evt.value()),
                                option { value: "", "None" }
                                for c in &cred_list {
                                    option { value: "{c.id}", "{c.name}" }
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *saving.read(),
                            onclick: {
                                let wid = webspace_id;
                                move |_| {
                                    let cred_str = selected_cred.read().clone();
                                    saving.set(true);
                                    result_msg.set(None);
                                    spawn(async move {
                                        let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                        match set_webspace_changedetection(CdSetInput {
                                            id: wid,
                                            credential_id: cid,
                                        })
                                        .await
                                        {
                                            Ok(()) => {
                                                result_msg.set(Some("Saved".into()));
                                                config.restart();
                                            }
                                            Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                        }
                                        saving.set(false);
                                    });
                                }
                            },
                            if *saving.read() { "Saving..." } else { "Save" }
                        }
                    }
                }

                if let Some(ref msg) = *result_msg.read() {
                    div { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}

// ── Sub-URLs Section ────────────────────────────────────────────────

#[component]
fn SubUrlsSection(webspace_id: Uuid, ws_id_str: String) -> Element {
    let mut suburls = use_server_future(move || async move {
        list_suburls(SubUrlListInput { id: webspace_id }).await
    })?;
    let rows = match &*suburls.read() {
        Some(Ok(r)) => r.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let mut new_path = use_signal(|| String::new());
    let mut creating = use_signal(|| false);
    let mut create_error = use_signal(|| None::<String>);
    let mut deleting_id = use_signal(|| None::<Uuid>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                if rows.is_empty() {
                    div { class: "text-sm text-fg-muted", "No sub-URLs configured. Add one to start monitoring." }
                } else {
                    table { class: "w-full",
                        thead {
                            tr {
                                Th { "Path" }
                                Th { "Created" }
                                Th { "" }
                            }
                        }
                        tbody {
                            for row in &rows {
                                tr {
                                    class: "hover:bg-bg-hover",
                                    Td {
                                        Link {
                                            to: crate::web::app::Route::WebspaceChangedetectionSuburl {
                                                id: ws_id_str.clone(),
                                                suburl_id: row.id.to_string(),
                                            },
                                            class: "text-brand underline",
                                            "{row.path}"
                                        }
                                    }
                                    TdMuted { "{row.created_at}" }
                                    Td {
                                        Button {
                                            variant: ButtonVariant::Danger,
                                            disabled: *deleting_id.read() == Some(row.id),
                                            onclick: {
                                                let sid = row.id;
                                                let wid = webspace_id;
                                                move |_| {
                                                    deleting_id.set(Some(sid));
                                                    spawn(async move {
                                                        match delete_suburl(SubUrlDeleteInput {
                                                            id: wid,
                                                            suburl_id: sid,
                                                        })
                                                        .await
                                                        {
                                                            Ok(()) => suburls.restart(),
                                                            Err(e) => {
                                                                create_error.set(Some(format!("{e}")));
                                                            }
                                                        }
                                                        deleting_id.set(None);
                                                    });
                                                }
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Add sub-URL form.
                div { class: "flex items-end gap-3 pt-2",
                    FormField { label: "Path",
                        input {
                            class: "input w-64",
                            r#type: "text",
                            placeholder: "/about",
                            value: "{new_path}",
                            oninput: move |evt| new_path.set(evt.value()),
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *creating.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let p = new_path.read().clone();
                                if p.is_empty() {
                                    create_error.set(Some("Path cannot be empty".into()));
                                    return;
                                }
                                creating.set(true);
                                create_error.set(None);
                                spawn(async move {
                                    match create_suburl(SubUrlCreateInput { id: wid, path: p }).await {
                                        Ok(_) => {
                                            new_path.set(String::new());
                                            suburls.restart();
                                        }
                                        Err(e) => create_error.set(Some(format!("{e}"))),
                                    }
                                    creating.set(false);
                                });
                            }
                        },
                        if *creating.read() { "Adding..." } else { "Add Sub-URL" }
                    }
                }
                if let Some(ref msg) = *create_error.read() {
                    div { class: "text-sm text-danger", "{msg}" }
                }
            }
        }
    }
}

// ── Notifications ───────────────────────────────────────────────────

#[component]
fn NotificationsInbox(webspace_id: Uuid, ws_id_str: String) -> Element {
    let notifications = use_server_future(move || async move {
        list_notifications(NotificationListInput { id: webspace_id }).await
    })?;
    let rows = match &*notifications.read() {
        Some(Ok(r)) => r.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        Card {
            if rows.is_empty() {
                div { class: "p-6 text-fg-muted text-sm", "No notifications yet." }
            } else {
                table { class: "w-full",
                    thead {
                        tr {
                            Th { "Title" }
                            Th { "Path" }
                            Th { "Preview" }
                            Th { "Date" }
                        }
                    }
                    tbody {
                        for row in &rows {
                            tr {
                                class: "cursor-pointer hover:bg-bg-hover",
                                onclick: {
                                    let nid = row.id.to_string();
                                    let wsid = ws_id_str.clone();
                                    move |_| {
                                        navigator().push(
                                            crate::web::app::Route::WebspaceChangedetectionNotification {
                                                id: wsid.clone(),
                                                notification_id: nid.clone(),
                                            },
                                        );
                                    }
                                },
                                Td { "{row.title}" }
                                TdMuted { "{row.path}" }
                                TdMuted { "{row.body_preview}" }
                                TdMuted { "{row.created_at}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Notification Detail ───────────────────────────────────────────────

#[component]
pub fn WebspaceChangedetectionNotification(id: String, notification_id: String) -> Element {
    let ws_id = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid webspace ID" } },
    };
    let notif_id = match Uuid::parse_str(&notification_id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid notification ID" } },
    };

    let detail = use_server_future(move || async move {
        get_notification(NotificationGetInput {
            id: ws_id,
            notification_id: notif_id,
        })
        .await
    })?;
    let d = match &*detail.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        PageHeader {
            Link {
                to: crate::web::app::Route::WebspaceChangedetection { id: id.clone() },
                class: "text-brand underline",
                "{d.webspace_name} / Change Detection"
            }
            " / Notification"
        }

        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-center gap-3",
                    h2 { class: "text-lg font-semibold text-fg", "{d.title}" }
                    Badge { variant: BadgeVariant::Neutral, "{d.path}" }
                    Badge { variant: BadgeVariant::Neutral, "{d.created_at}" }
                }
                pre { class: "whitespace-pre-wrap text-sm text-fg font-mono bg-bg-inset p-4 rounded overflow-x-auto",
                    "{d.body}"
                }
            }
        }
    }
}

// ── Sub-URL Settings Page ────────────────────────────────────────────

#[component]
pub fn WebspaceChangedetectionSuburl(id: String, suburl_id: String) -> Element {
    let ws_id = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid webspace ID" } },
    };
    let sid = match Uuid::parse_str(&suburl_id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid sub-URL ID" } },
    };

    let detail = use_server_future(move || async move {
        get_suburl(SubUrlGetInput {
            id: ws_id,
            suburl_id: sid,
        })
        .await
    })?;
    let d = match &*detail.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let ws_name_res =
        use_server_future(move || async move { get_cd_config(CdGetInput { id: ws_id }).await })?;
    let ws_name = match &*ws_name_res.read() {
        Some(Ok(c)) => c.webspace_name.clone(),
        _ => "Webspace".to_string(),
    };

    rsx! {
        PageHeader {
            Link {
                to: crate::web::app::Route::WebspaceChangedetection { id: id.clone() },
                class: "text-brand underline",
                "{ws_name} / Change Detection"
            }
            " / {d.path}"
        }

        SubUrlSettingsForm { webspace_id: ws_id, suburl_id: sid, initial: d.tag_settings }
    }
}

/// Helper: split newline-separated text into a Vec, filtering empties.
fn lines_to_vec(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Helper: join a Vec into newline-separated text.
fn vec_to_lines(v: &[String]) -> String {
    v.join("\n")
}

#[component]
fn SubUrlSettingsForm(webspace_id: Uuid, suburl_id: Uuid, initial: TagSettings) -> Element {
    // Text-list fields as multiline text.
    let mut extract_text = use_signal(move || vec_to_lines(&initial.extract_text));
    let mut extract_lines = use_signal(move || vec_to_lines(&initial.extract_lines_containing));
    let mut text_not_present =
        use_signal(move || vec_to_lines(&initial.text_should_not_be_present));
    let mut include_filters = use_signal(move || vec_to_lines(&initial.include_filters));
    let mut subtractive = use_signal(move || vec_to_lines(&initial.subtractive_selectors));
    let mut ignore_text = use_signal(move || vec_to_lines(&initial.ignore_text));
    let mut trigger_text = use_signal(move || vec_to_lines(&initial.trigger_text));

    // Boolean toggles.
    let mut trim_ws = use_signal(move || initial.trim_text_whitespace.unwrap_or(false));
    let mut sort_alpha = use_signal(move || initial.sort_text_alphabetically.unwrap_or(false));
    let mut dedup = use_signal(move || initial.remove_duplicate_lines.unwrap_or(false));
    let mut unique = use_signal(move || initial.check_unique_lines.unwrap_or(false));
    let mut muted = use_signal(move || initial.notification_muted.unwrap_or(false));
    let mut screenshot = use_signal(move || initial.notification_screenshot.unwrap_or(false));

    // Match logic.
    let mut match_logic = use_signal(move || {
        initial
            .conditions_match_logic
            .clone()
            .unwrap_or_else(|| "ALL".to_string())
    });

    // Conditions as a dynamic list.
    let mut conditions = use_signal(move || initial.conditions.clone());

    // Notification fields.
    let mut notif_title =
        use_signal(move || initial.notification_title.clone().unwrap_or_default());
    let mut notif_body = use_signal(move || initial.notification_body.clone().unwrap_or_default());
    let mut notif_format =
        use_signal(move || initial.notification_format.clone().unwrap_or_default());

    let mut saving = use_signal(|| false);
    let mut result_msg = use_signal(|| None::<String>);

    let build_settings = move || -> TagSettings {
        TagSettings {
            conditions: conditions.read().clone(),
            conditions_match_logic: {
                let v = match_logic.read().clone();
                if v == "ALL" { None } else { Some(v) }
            },
            extract_text: lines_to_vec(&extract_text.read()),
            extract_lines_containing: lines_to_vec(&extract_lines.read()),
            text_should_not_be_present: lines_to_vec(&text_not_present.read()),
            include_filters: lines_to_vec(&include_filters.read()),
            subtractive_selectors: lines_to_vec(&subtractive.read()),
            ignore_text: lines_to_vec(&ignore_text.read()),
            trigger_text: lines_to_vec(&trigger_text.read()),
            trim_text_whitespace: if *trim_ws.read() { Some(true) } else { None },
            sort_text_alphabetically: if *sort_alpha.read() { Some(true) } else { None },
            remove_duplicate_lines: if *dedup.read() { Some(true) } else { None },
            check_unique_lines: if *unique.read() { Some(true) } else { None },
            notification_title: {
                let v = notif_title.read().clone();
                if v.is_empty() { None } else { Some(v) }
            },
            notification_body: {
                let v = notif_body.read().clone();
                if v.is_empty() { None } else { Some(v) }
            },
            notification_format: {
                let v = notif_format.read().clone();
                if v.is_empty() { None } else { Some(v) }
            },
            notification_muted: if *muted.read() { Some(true) } else { None },
            notification_screenshot: if *screenshot.read() { Some(true) } else { None },
        }
    };

    rsx! {
        Card {
            div { class: "p-6 space-y-6",

                // ── Conditions ───────────────────────────────────────
                SectionHeading { "Conditions" }
                div { class: "space-y-2",
                    div { class: "flex items-center gap-3 mb-2",
                        span { class: "text-sm text-fg-muted", "Match logic:" }
                        select {
                            class: "input w-32",
                            value: "{match_logic}",
                            oninput: move |evt| match_logic.set(evt.value()),
                            option { value: "ALL", "ALL" }
                            option { value: "ANY", "ANY" }
                        }
                    }

                    for (idx, _cond) in conditions.read().iter().enumerate() {
                        {
                            let conds = conditions.read().clone();
                            let cond = &conds[idx];
                            rsx! {
                                div { class: "flex items-center gap-2",
                                    input {
                                        class: "input w-40",
                                        r#type: "text",
                                        placeholder: "field",
                                        value: "{cond.field}",
                                        oninput: {
                                            move |evt: Event<FormData>| {
                                                let mut c = conditions.write();
                                                c[idx].field = evt.value();
                                            }
                                        },
                                    }
                                    input {
                                        class: "input w-40",
                                        r#type: "text",
                                        placeholder: "operator",
                                        value: "{cond.operator}",
                                        oninput: {
                                            move |evt: Event<FormData>| {
                                                let mut c = conditions.write();
                                                c[idx].operator = evt.value();
                                            }
                                        },
                                    }
                                    input {
                                        class: "input flex-1",
                                        r#type: "text",
                                        placeholder: "value",
                                        value: "{cond.value}",
                                        oninput: {
                                            move |evt: Event<FormData>| {
                                                let mut c = conditions.write();
                                                c[idx].value = evt.value();
                                            }
                                        },
                                    }
                                    Button {
                                        variant: ButtonVariant::Danger,
                                        onclick: {
                                            move |_| {
                                                let mut c = conditions.write();
                                                c.remove(idx);
                                            }
                                        },
                                        "X"
                                    }
                                }
                            }
                        }
                    }

                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            conditions.write().push(TagCondition::default());
                        },
                        "+ Add Condition"
                    }
                }

                // ── Text Filters ─────────────────────────────────────
                SectionHeading { "Text Filters" }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Include Filters (CSS/XPath, one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{include_filters}",
                            oninput: move |evt| include_filters.set(evt.value()),
                        }
                    }
                    FormField { label: "Subtractive Selectors (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{subtractive}",
                            oninput: move |evt| subtractive.set(evt.value()),
                        }
                    }
                    FormField { label: "Extract Text (regex, one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{extract_text}",
                            oninput: move |evt| extract_text.set(evt.value()),
                        }
                    }
                    FormField { label: "Extract Lines Containing (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{extract_lines}",
                            oninput: move |evt| extract_lines.set(evt.value()),
                        }
                    }
                    FormField { label: "Text Should Not Be Present (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{text_not_present}",
                            oninput: move |evt| text_not_present.set(evt.value()),
                        }
                    }
                    FormField { label: "Ignore Text (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{ignore_text}",
                            oninput: move |evt| ignore_text.set(evt.value()),
                        }
                    }
                    FormField { label: "Trigger Text (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{trigger_text}",
                            oninput: move |evt| trigger_text.set(evt.value()),
                        }
                    }
                }

                // ── Text Processing ──────────────────────────────────
                SectionHeading { "Text Processing" }
                div { class: "flex flex-wrap gap-6",
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *trim_ws.read(),
                            oninput: move |evt| trim_ws.set(evt.checked()),
                        }
                        "Trim whitespace"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *sort_alpha.read(),
                            oninput: move |evt| sort_alpha.set(evt.checked()),
                        }
                        "Sort alphabetically"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *dedup.read(),
                            oninput: move |evt| dedup.set(evt.checked()),
                        }
                        "Remove duplicate lines"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *unique.read(),
                            oninput: move |evt| unique.set(evt.checked()),
                        }
                        "Check unique lines"
                    }
                }

                // ── Notifications ────────────────────────────────────
                SectionHeading { "Notification Settings" }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Notification Title",
                        input {
                            class: "input w-full",
                            r#type: "text",
                            value: "{notif_title}",
                            oninput: move |evt| notif_title.set(evt.value()),
                        }
                    }
                    FormField { label: "Notification Format",
                        select {
                            class: "input w-full",
                            value: "{notif_format}",
                            oninput: move |evt| notif_format.set(evt.value()),
                            option { value: "", "Default" }
                            option { value: "text", "Text" }
                            option { value: "html", "HTML" }
                            option { value: "markdown", "Markdown" }
                        }
                    }
                }
                FormField { label: "Notification Body",
                    textarea {
                        class: "input w-full h-24 font-mono text-sm",
                        value: "{notif_body}",
                        oninput: move |evt| notif_body.set(evt.value()),
                    }
                }
                div { class: "flex flex-wrap gap-6",
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *muted.read(),
                            oninput: move |evt| muted.set(evt.checked()),
                        }
                        "Mute notifications"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *screenshot.read(),
                            oninput: move |evt| screenshot.set(evt.checked()),
                        }
                        "Include screenshot"
                    }
                }

                // ── Save Button ──────────────────────────────────────
                div { class: "flex items-center gap-3 pt-4",
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: move |_| {
                            let settings = build_settings();
                            saving.set(true);
                            result_msg.set(None);
                            spawn(async move {
                                match update_suburl_settings(SubUrlUpdateInput {
                                    id: webspace_id,
                                    suburl_id,
                                    settings,
                                })
                                .await
                                {
                                    Ok(()) => result_msg.set(Some("Saved".into())),
                                    Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                }
                                saving.set(false);
                            });
                        },
                        if *saving.read() { "Saving..." } else { "Save Settings" }
                    }
                    if let Some(ref msg) = *result_msg.read() {
                        div { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }
        }
    }
}
