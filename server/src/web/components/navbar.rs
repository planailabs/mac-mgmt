//! Sidebar navigation + the mobile drawer that mirrors it.
//!
//! Layout owns:
//!   * `Sidebar`     — desktop-only 220px column. Logo at top, nav
//!                     groups below. Hidden under xl breakpoint.
//!   * `MobileDrawer` — slide-in panel for narrow screens. Same nav
//!                     content as the sidebar; opened by the topbar
//!                     hamburger via a shared `is_open` signal.
//!
//! The pre-redesign `Navbar` (full-width chrome with logo + theme
//! controls) is gone — its responsibilities split between `Topbar`
//! (chrome) and `Sidebar` (logo).

use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;

#[server]
async fn get_swagger_url() -> Result<String, ServerFnError> {
    let cfg = crate::config::config();
    let base = cfg.api.external_url.trim_end_matches('/');
    Ok(format!("{base}/api/swagger-ui/"))
}

// -- Navigation Structure --------------------------------------------------

#[derive(Clone, PartialEq)]
pub enum NavLink {
    Internal(Route, String),
    External(String, String),
}

#[derive(Clone, PartialEq)]
pub struct NavGroup {
    pub title: String,
    pub links: Vec<NavLink>,
}

pub fn get_nav_groups(is_admin: bool, swagger_url: Option<String>) -> Vec<NavGroup> {
    let mut overview_links = vec![
        NavLink::Internal(Route::ClusterList {}, "nav-clusters".to_string()),
    ];
    overview_links.push(NavLink::Internal(
        Route::FleetDashboard { stage_id: None },
        "nav-fleet".to_string(),
    ));
    overview_links.push(NavLink::Internal(Route::EasyAccess {}, "nav-easy-access".to_string()));
    let mut groups = vec![NavGroup {
        title: "nav-overview".to_string(),
        links: overview_links,
    }];

    if is_admin {
        groups.push(NavGroup {
            title: "nav-mcp-skills".to_string(),
            links: vec![
                NavLink::Internal(Route::SkillList {}, "nav-skills".to_string()),
                NavLink::Internal(Route::McpServerList {}, "nav-mcp-servers".to_string()),
                NavLink::Internal(Route::McpBundleList {}, "nav-mcp-bundles".to_string()),
                NavLink::Internal(Route::BundleList {}, "nav-bundles".to_string()),
            ],
        });

        groups.push(NavGroup {
            title: "nav-import".to_string(),
            links: vec![
                NavLink::Internal(
                    Route::ImportSources { prefill_slug: None, prefill_name: None },
                    "nav-import-sources".to_string(),
                ),
            ],
        });

        groups.push(NavGroup {
            title: "nav-admin".to_string(),
            links: vec![
                NavLink::Internal(Route::AdminTokens {}, "nav-admin-tokens".to_string()),
                NavLink::Internal(Route::StaffPings {}, "nav-staff-pings".to_string()),
                NavLink::Internal(Route::OrganizationList {}, "nav-organizations".to_string()),
                NavLink::Internal(Route::UserList {}, "nav-users".to_string()),
                NavLink::Internal(Route::SkillCenterList {}, "nav-skill-centers".to_string()),
            ],
        });

        groups.push(NavGroup {
            title: "nav-version".to_string(),
            links: vec![
                NavLink::Internal(Route::RolloutList {}, "nav-rollouts".to_string()),
                NavLink::Internal(Route::DaemonVersionList {}, "nav-daemon-versions".to_string()),
            ],
        });
    }

    let mut resources_links = vec![NavLink::Internal(Route::DocList {}, "nav-docs".to_string())];
    if let Some(url) = swagger_url {
        resources_links.push(NavLink::External(url, "nav-api-docs".to_string()));
    }

    groups.push(NavGroup {
        title: "nav-resources".to_string(),
        links: resources_links,
    });

    groups
}

// -- Logo ------------------------------------------------------------------

/// Brand mark — orange rounded-square outline + filled inner square,
/// followed by "plan.ai mgmt" wordmark. Used at the top of both the
/// desktop sidebar and the mobile drawer.
#[component]
fn Logo() -> Element {
    rsx! {
        Link {
            to: Route::ClusterList {},
            class: "flex items-center gap-2 text-fg-strong font-semibold text-sm tracking-tight",
            svg {
                width: "20",
                height: "20",
                view_box: "0 0 20 20",
                fill: "none",
                rect {
                    x: "1.5", y: "1.5", width: "17", height: "17", rx: "5",
                    stroke: "rgb(var(--c-brand))",
                    "stroke-width": "1.6",
                }
                rect {
                    x: "6", y: "6", width: "8", height: "8", rx: "1.5",
                    fill: "rgb(var(--c-brand))",
                }
            }
            span {
                {t!("nav-logo")} " "
                span { class: "text-fg-muted font-medium", "mgmt" }
            }
        }
    }
}

// -- Navigation list -------------------------------------------------------

/// Renders one group of nav links. Shared between desktop sidebar and
/// mobile drawer so they stay in sync.
#[component]
fn NavGroupList(
    groups: Vec<NavGroup>,
    /// Optional callback fired when an internal link is clicked. The
    /// mobile drawer uses this to close itself; the sidebar passes
    /// `None`.
    #[props(default)] on_navigate: Option<EventHandler<()>>,
) -> Element {
    rsx! {
        nav { class: "flex-1 px-3 py-5 space-y-7",
            for group in groups {
                div { key: "{group.title}",
                    h3 { class: "nav-group-head", {t!(&group.title)} }
                    div { class: "mt-2 space-y-0.5",
                        for link in group.links {
                            match link {
                                NavLink::Internal(route, label) => {
                                    let nav = on_navigate;
                                    rsx! {
                                        Link {
                                            key: "{label}",
                                            to: route.clone(),
                                            class: "nav-link",
                                            active_class: "nav-link-active",
                                            onclick: move |_| if let Some(h) = nav { h.call(()); },
                                            {t!(&label)}
                                        }
                                    }
                                }
                                NavLink::External(url, label) => {
                                    let nav = on_navigate;
                                    rsx! {
                                        a {
                                            key: "{label}",
                                            href: "{url}",
                                            target: "_blank",
                                            class: "nav-link",
                                            onclick: move |_| if let Some(h) = nav { h.call(()); },
                                            {t!(&label)}
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

// -- Sidebar (desktop) -----------------------------------------------------

#[component]
pub fn Sidebar(is_admin: bool) -> Element {
    let swagger_fut = use_server_future(get_swagger_url);
    let swagger_url: Option<String> = match swagger_fut {
        Ok(ref fut) => match &*fut.read() {
            Some(Ok(url)) => Some(url.clone()),
            _ => None,
        },
        Err(_) => None,
    };

    let groups = get_nav_groups(is_admin, swagger_url);

    rsx! {
        aside { class: "nav-side",
            div { class: "px-5 pt-5 pb-4 shrink-0",
                Logo {}
            }
            NavGroupList { groups }
        }
    }
}

// -- Mobile drawer ---------------------------------------------------------

#[component]
pub fn MobileDrawer(
    is_admin: bool,
    display_name: String,
    is_open: Signal<bool>,
) -> Element {
    let open = *is_open.read();

    let swagger_fut = use_server_future(get_swagger_url);
    let swagger_url: Option<String> = match swagger_fut {
        Ok(ref fut) => match &*fut.read() {
            Some(Ok(url)) => Some(url.clone()),
            _ => None,
        },
        Err(_) => None,
    };
    let groups = get_nav_groups(is_admin, swagger_url);

    let backdrop_cls = if open {
        "fixed inset-0 bg-fg-strong/80 backdrop-blur-sm transition-opacity duration-300 z-40 opacity-100 pointer-events-auto"
    } else {
        "fixed inset-0 bg-fg-strong/80 backdrop-blur-sm transition-opacity duration-300 z-40 opacity-0 pointer-events-none"
    };

    let drawer_cls = if open {
        "fixed inset-y-0 right-0 max-w-xs w-full bg-surface shadow-xl overflow-y-auto flex flex-col z-50 transform transition-transform duration-300 ease-in-out border-l border-line translate-x-0 pointer-events-auto"
    } else {
        "fixed inset-y-0 right-0 max-w-xs w-full bg-surface shadow-xl overflow-y-auto flex flex-col z-50 transform transition-transform duration-300 ease-in-out border-l border-line translate-x-full pointer-events-none"
    };

    rsx! {
        div {
            id: "mobile-drawer-container",
            class: "xl:hidden relative z-50",

            // Backdrop — taps close the drawer.
            div {
                class: backdrop_cls,
                "aria-hidden": "true",
                onclick: move |_| is_open.set(false),
            }

            // Drawer panel.
            div {
                class: drawer_cls,
                id: "mobile-drawer",

                // Header: logo + close button. We deliberately repeat
                // the logo here (rather than only in the sidebar) so
                // the drawer is self-contained on phones.
                div { class: "px-5 py-4 border-b border-line bg-surface-2 flex justify-between items-center shrink-0",
                    Logo {}
                    button {
                        onclick: move |_| is_open.set(false),
                        class: "nav-icon-btn",
                        "aria-label": t!("nav-close-menu"),
                        svg {
                            class: "h-5 w-5",
                            fill: "none",
                            stroke: "currentColor",
                            view_box: "0 0 24 24",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                stroke_width: "2",
                                d: "M6 18L18 6M6 6l12 12",
                            }
                        }
                    }
                }

                // User block — only when authenticated.
                if !display_name.is_empty() {
                    div { class: "px-5 py-3 border-b border-line bg-surface-2",
                        div { class: "flex items-center gap-3",
                            svg {
                                class: "h-9 w-9 text-fg-faint bg-surface rounded-full p-1.5 border border-line shrink-0",
                                fill: "none",
                                stroke: "currentColor",
                                view_box: "0 0 24 24",
                                stroke_width: "1.5",
                                path {
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    d: "M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z",
                                }
                            }
                            div { class: "flex flex-col min-w-0",
                                span { class: "text-sm font-medium text-fg-strong truncate", "{display_name}" }
                                div { class: "flex gap-3",
                                    Link {
                                        to: Route::Profile {},
                                        class: "link text-xs",
                                        onclick: move |_| is_open.set(false),
                                        {t!("nav-view-profile")}
                                    }
                                    a {
                                        href: "/auth/logout",
                                        class: "link-danger text-xs",
                                        {t!("nav-sign-out")}
                                    }
                                }
                            }
                        }
                    }
                }

                // Nav groups — clicking a link closes the drawer.
                NavGroupList {
                    groups,
                    on_navigate: move |_| is_open.set(false),
                }
            }
        }
    }
}
