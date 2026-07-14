use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::healer::{
    PingResolveInput, StaffPingRow, StaffPingsListInput, list_all_staff_pings, resolve_ping,
};
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Badge, BadgeVariant, ErrorText, HelpText};

// ── Component ──────────────────────────────────────────────────────────

#[component]
pub fn StaffPings() -> Element {
    use_topbar(t!("staff-pings-title"), None);
    let mut pings = use_signal::<Vec<StaffPingRow>>(Vec::new);
    let mut loaded = use_signal(|| false);
    let mut error_msg = use_signal::<Option<String>>(|| None);

    // Load on mount
    use_future(move || async move {
        match list_all_staff_pings(StaffPingsListInput {}).await {
            Ok(p) => {
                pings.set(p);
                loaded.set(true);
            }
            Err(e) => {
                error_msg.set(Some(e.to_string()));
                loaded.set(true);
            }
        }
    });

    if !*loaded.read() {
        return rsx! { HelpText { {t!("staff-pings-loading")} } };
    }
    if let Some(err) = &*error_msg.read() {
        return rsx! { ErrorText { {t!("error-message", message: err.to_string())} } };
    }

    let all = pings.read();
    let unresolved: Vec<_> = all.iter().filter(|p| !p.resolved).cloned().collect();
    let resolved: Vec<_> = all.iter().filter(|p| p.resolved).cloned().collect();
    let unresolved_count = unresolved.len();
    let resolved_count = resolved.len();

    rsx! {
        h2 { class: "h-page", {t!("staff-pings-title")} }
        p { class: "text-sm text-fg-muted mb-6",
            {t!("staff-pings-description")}
        }

        if !unresolved.is_empty() {
            div { class: "mb-8",
                h3 { class: "h-section flex items-center gap-2",
                    span { class: "inline-block w-2.5 h-2.5 rounded-full bg-danger" }
                    {t!("staff-pings-open", count: unresolved_count)}
                }
                div { class: "space-y-2",
                    for ping in unresolved.iter() {
                        { render_ping_card(ping, pings) }
                    }
                }
            }
        } else {
            div { class: "card mb-8 p-6 text-center text-fg-faint",
                {t!("staff-pings-no-open")}
            }
        }

        if !resolved.is_empty() {
            div {
                h3 { class: "h-section text-fg-muted",
                    {t!("staff-pings-resolved", count: resolved_count)}
                }
                div { class: "space-y-2 opacity-60",
                    for ping in resolved.iter() {
                        { render_ping_card(ping, pings) }
                    }
                }
            }
        }
    }
}

fn render_ping_card(ping: &StaffPingRow, pings: Signal<Vec<StaffPingRow>>) -> Element {
    let cat_variant = category_badge_variant(&ping.category);
    let ping_id = ping.id.clone();
    let session_url = format!("/fleet/{}/healer/{}", ping.instance_id, ping.session_id);
    let is_resolved = ping.resolved;
    let message = ping.message.clone();
    let category = ping.category.clone();
    let cluster_name = ping.cluster_name.clone();
    let instance_id = ping.instance_id.clone();
    let created_at = ping.created_at.clone();
    let resolved_by = ping.resolved_by.clone();

    rsx! {
        div { class: "card p-4",
            div { class: "flex items-start justify-between gap-4",
                div { class: "flex-1 min-w-0",
                    div { class: "flex items-center gap-2 mb-1 flex-wrap",
                        Badge { variant: cat_variant, "{category}" }
                        span { class: "text-xs text-fg-muted", "{cluster_name}" }
                        span { class: "text-xs text-fg-faint", "{instance_id}" }
                        span { class: "text-xs text-fg-faint", "{created_at}" }
                    }
                    {
                        let html = crate::web::components::healer_page::simple_md_to_html(&message);
                        let class = if is_resolved {
                            "text-sm text-fg-strong prose prose-sm dark:prose-invert max-w-none line-through"
                        } else {
                            "text-sm text-fg-strong prose prose-sm dark:prose-invert max-w-none"
                        };
                        rsx! {
                            div { class: "{class}", dangerous_inner_html: "{html}" }
                        }
                    }
                    if let Some(by) = &resolved_by {
                        p { class: "text-xs text-success mt-1", {t!("staff-pings-resolved-by", by: by.clone())} }
                    }
                }
                div { class: "flex items-center gap-2 shrink-0",
                    Link { to: session_url,
                        class: "btn btn-xs btn-info-soft",
                        {t!("staff-pings-view-session")}
                    }
                    if !is_resolved {
                        button { class: "btn btn-xs btn-success-soft",
                            onclick: {
                                let mut pings = pings;
                                move |_| {
                                    let ping_id = ping_id.clone();
                                    async move {
                                        let Ok(id) = ping_id.parse::<uuid::Uuid>() else {
                                            return;
                                        };
                                        if resolve_ping(PingResolveInput { ping_id: id }).await.is_ok() {
                                            pings.with_mut(|list| {
                                                if let Some(p) = list.iter_mut().find(|p| p.id == ping_id) {
                                                    p.resolved = true;
                                                }
                                            });
                                        }
                                    }
                                }
                            },
                            {t!("staff-pings-resolve")}
                        }
                    }
                }
            }
        }
    }
}

fn category_badge_variant(cat: &str) -> BadgeVariant {
    match cat {
        "hardware" | "service_crash" | "security" => BadgeVariant::Danger,
        "network" | "performance" => BadgeVariant::Info,
        "disk_space" | "config_error" => BadgeVariant::Warn,
        "model_issue" | "permission" | "dependency" => BadgeVariant::Accent,
        _ => BadgeVariant::Neutral,
    }
}
