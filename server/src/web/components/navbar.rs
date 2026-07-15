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

/// Newtype wrapping the desktop-sidebar collapsed signal so it lives in
/// the context graph without colliding with other `Signal<bool>` values.
/// Provided by `Layout`; consumed by `Sidebar` (slide-off animation) and
/// the topbar `Logo` (click toggles it).
#[derive(Clone, Copy)]
pub struct SidebarCollapsed(pub Signal<bool>);

/// Newtype wrapping the per-group expanded state for the desktop sidebar.
/// Stored as a list of expanded `nav-*` keys so we can persist as a
/// comma-joined string and skip serde_json in the WASM bundle. Provided
/// by `Layout`; consumed by the per-group toggle in `NavGroupItem`.
#[derive(Clone, Copy)]
pub struct ExpandedGroups(pub Signal<Vec<String>>);

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
        // Command Center first — it's the at-a-glance landing page;
        // Clusters and Fleet are the drill-down details.
        NavLink::Internal(Route::Overview {}, "nav-command-center".to_string()),
        NavLink::Internal(Route::ClusterList {}, "nav-clusters".to_string()),
    ];
    overview_links.push(NavLink::Internal(
        Route::FleetDashboard { stage_id: None },
        "nav-fleet".to_string(),
    ));
    overview_links.push(NavLink::Internal(
        Route::HealerSpend {},
        "nav-healer-spend".to_string(),
    ));
    overview_links.push(NavLink::Internal(
        Route::EasyAccess {},
        "nav-easy-access".to_string(),
    ));
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
            links: vec![NavLink::Internal(
                Route::ImportSources {
                    prefill_slug: None,
                    prefill_name: None,
                },
                "nav-import-sources".to_string(),
            )],
        });

        groups.push(NavGroup {
            title: "nav-admin".to_string(),
            links: vec![
                NavLink::Internal(Route::AdminTokens {}, "nav-admin-tokens".to_string()),
                NavLink::Internal(
                    Route::AdminClientCerts {},
                    "nav-admin-client-certs".to_string(),
                ),
                NavLink::Internal(Route::AdminClientCas {}, "nav-admin-client-cas".to_string()),
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
                NavLink::Internal(
                    Route::DaemonVersionList {},
                    "nav-daemon-versions".to_string(),
                ),
            ],
        });
    }

    let mut resources_links = vec![
        NavLink::Internal(Route::DocList {}, "nav-docs".to_string()),
        NavLink::Internal(Route::GlossaryPage {}, "nav-glossary".to_string()),
    ];
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

/// SVG + wordmark. Used inside both the desktop button and the mobile
/// link variants of `Logo`. Always full-size — the brand mark stays
/// visible whether the sidebar is open or collapsed.
#[component]
pub fn LogoMark() -> Element {
    rsx! {
        svg {
            class: "shrink-0",
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
        // Wordmark is brand chrome, not translatable copy — hard-code so
        // we don't accidentally render "<i18n value> mgmt" twice when
        // the i18n key already contains the full brand string.
        span { class: "whitespace-nowrap",
            "plan.ai "
            span { class: "text-fg-muted font-medium", "mgmt" }
        }
    }
}

/// Brand mark in the topbar's left edge.
///
/// * **Mobile** — renders as a `<Link to=Overview>` so a tap brings the
///   user home. The sidebar collapse signal is desktop-only.
/// * **Desktop** — renders as a `<button>` that toggles
///   `SidebarCollapsed`, sliding the sidebar away (and back) per the
///   user's preference. Persistence is handled in `Layout`.
#[component]
pub fn Logo() -> Element {
    let SidebarCollapsed(mut collapsed) = use_context::<SidebarCollapsed>();

    rsx! {
        // Desktop variant — sidebar toggle.
        button {
            class: "hidden xl:flex items-center gap-2 text-fg-strong font-semibold text-sm tracking-tight cursor-pointer rounded-md px-1 py-1 -mx-1 hover:bg-surface-3 focus:outline-none focus:ring-2 focus:ring-info transition-colors",
            "aria-label": t!("nav-toggle-sidebar"),
            onclick: move |_| {
                let cur = *collapsed.read();
                collapsed.set(!cur);
            },
            LogoMark {}
        }
        // Mobile variant — navigates home.
        Link {
            to: Route::Overview {},
            class: "xl:hidden flex items-center gap-2 text-fg-strong font-semibold text-sm tracking-tight",
            LogoMark {}
        }
    }
}

// -- Active-route matching -------------------------------------------------

/// Whether the link target should highlight when the user is on `current`.
///
/// `Link`'s built-in `active_class` does an exact route comparison — that
/// fails for our query-param routes (`#[route("/fleet?:stage_id")]`)
/// because the link's target route (`stage_id: None`) doesn't match a
/// current route carrying any other field state, even when a user clicks
/// over from another page. We match by variant family so e.g. the Fleet
/// link stays lit on `/fleet`, `/fleet?stage_id=...`, the instance-detail
/// page, and its tunnels/files/shell sub-routes — the same intuition the
/// user has when they think "I'm in the Fleet area".
fn route_active(link: &Route, current: &Route) -> bool {
    use Route::*;
    match (link, current) {
        // Fleet "area" — dashboard + every per-instance sub-page.
        (
            FleetDashboard { .. },
            FleetDashboard { .. }
            | FleetDetail { .. }
            | FleetFiles { .. }
            | FleetShell { .. }
            | FleetLogs { .. }
            | FleetHealer { .. }
            | FleetHealerSession { .. },
        ) => true,
        // Clusters area — list + detail + sub-pages + form.
        (
            ClusterList { .. },
            ClusterList { .. }
            | ClusterDetail { .. }
            | ClusterConfigPage { .. }
            | ClusterPackagesPage { .. }
            | ClusterForm { .. },
        ) => true,
        // Skills, MCP, Bundles, Rollouts, Orgs, Users, Skill Centers,
        // Daemon Versions, Import Sources, Docs all light their list
        // entry when on a list/detail/form page in that family.
        (SkillList { .. }, SkillList { .. } | SkillDetail { .. }) => true,
        (
            McpServerList { .. },
            McpServerList { .. }
            | McpServerDetail { .. }
            | McpServerEdit { .. }
            | McpServerForm { .. },
        ) => true,
        (
            McpBundleList { .. },
            McpBundleList { .. } | McpBundleDetail { .. } | McpBundleForm { .. },
        ) => true,
        (BundleList { .. }, BundleList { .. } | BundleDetail { .. } | BundleForm { .. }) => true,
        (
            RolloutList { .. },
            RolloutList { .. }
            | RolloutDetail { .. }
            | RolloutForm { .. }
            | RolloutGroupList { .. }
            | RolloutGroupDetail { .. },
        ) => true,
        (
            OrganizationList { .. },
            OrganizationList { .. } | OrganizationDetail { .. } | OrganizationForm { .. },
        ) => true,
        (UserList { .. }, UserList { .. } | UserDetail { .. } | UserForm { .. }) => true,
        (
            SkillCenterList { .. },
            SkillCenterList { .. } | SkillCenterDetail { .. } | SkillCenterForm { .. },
        ) => true,
        (DaemonVersionList { .. }, DaemonVersionList { .. } | DaemonVersionDetail { .. }) => true,
        (
            ImportSources { .. },
            ImportSources { .. }
            | ImportSourcesSearch { .. }
            | ImportSourceDetail { .. }
            | ImportSourceEdit { .. },
        ) => true,
        (DocList { .. }, DocList { .. } | DocPage { .. }) => true,
        // Single-page entries — match exact variant.
        (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
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
    #[props(default)]
    on_navigate: Option<EventHandler<()>>,
    /// When true (desktop sidebar only), each group header is a
    /// collapse toggle. The mobile drawer passes `false` so users
    /// always see every link — matches the always-expanded mobile
    /// pattern of most chrome.
    #[props(default = false)]
    collapsible: bool,
) -> Element {
    rsx! {
        // `min-h-0` is the flexbox escape hatch that lets this child
        // shrink below its content size — without it, `overflow-y-auto`
        // would never trigger and the sidebar would clip its bottom
        // links on short viewports. Extra bottom padding leaves room
        // below the last group so it doesn't kiss the viewport edge.
        nav { class: "flex-1 min-h-0 overflow-y-auto pl-5 pr-3 py-5 pb-8 space-y-2",
            for group in groups {
                NavGroupItem {
                    key: "{group.title}",
                    group,
                    on_navigate,
                    collapsible,
                }
            }
        }
    }
}

#[component]
fn NavGroupItem(
    group: NavGroup,
    on_navigate: Option<EventHandler<()>>,
    collapsible: bool,
) -> Element {
    let current_route = use_route::<Route>();
    let key = group.title.clone();

    // The mobile drawer skips the context lookup entirely; only the
    // desktop sidebar reads/writes group-collapse state.
    let (is_open, toggle_handler) = if collapsible {
        let ExpandedGroups(mut sig) = use_context::<ExpandedGroups>();
        let open = sig.read().contains(&key);
        let key_for_click = key.clone();
        let onclick = move |_| {
            let mut v = sig.read().clone();
            if v.iter().any(|k| k == &key_for_click) {
                v.retain(|k| k != &key_for_click);
            } else {
                v.push(key_for_click.clone());
            }
            sig.set(v);
        };
        (open, Some(onclick))
    } else {
        (true, None)
    };

    let body_class = if is_open {
        "nav-group-body nav-group-body-open"
    } else {
        "nav-group-body"
    };
    let marker_class = if is_open {
        "nav-group-marker nav-group-marker-open"
    } else {
        "nav-group-marker"
    };

    rsx! {
        div {
            // Header row — `<button>` when collapsible (desktop), plain
            // `<h3>` otherwise (mobile drawer).
            if let Some(handler) = toggle_handler {
                button {
                    class: "nav-group-toggle",
                    "aria-expanded": "{is_open}",
                    onclick: handler,
                    svg {
                        class: "{marker_class}",
                        view_box: "0 0 10 10",
                        fill: "currentColor",
                        "aria-hidden": "true",
                        polygon { points: "2,1 9,5 2,9" }
                    }
                    span { class: "nav-group-head", {t!(&group.title)} }
                }
            } else {
                h3 { class: "nav-group-head px-3", {t!(&group.title)} }
            }
            div { class: body_class,
                div { class: "min-h-0 overflow-hidden",
                    div { class: "mt-2 space-y-0.5 pb-1",
                        for link in group.links {
                            match link {
                                NavLink::Internal(route, label) => {
                                    let nav = on_navigate;
                                    let cls = if route_active(&route, &current_route) {
                                        "nav-link nav-link-active"
                                    } else {
                                        "nav-link"
                                    };
                                    rsx! {
                                        Link {
                                            key: "{label}",
                                            to: route.clone(),
                                            class: cls,
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
    let SidebarCollapsed(collapsed) = use_context::<SidebarCollapsed>();
    let outer_class = if *collapsed.read() {
        "nav-side nav-side-collapsed"
    } else {
        "nav-side"
    };

    rsx! {
        aside { class: outer_class,
            // Logo lives in the topbar's logo-pad now. The sidebar
            // contains only nav groups, which slide off entirely when
            // the user collapses (width 220 → 0).
            div { class: "nav-side-inner",
                NavGroupList { groups, collapsible: true }
            }
        }
    }
}

// -- Mobile drawer ---------------------------------------------------------

#[component]
pub fn MobileDrawer(is_admin: bool, display_name: String, is_open: Signal<bool>) -> Element {
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

    // Backdrop tints the canvas (matches the page theme rather than
    // contrasting it) so dark mode gets a dark scrim and light mode a
    // light one — the inverse of the previous fg-strong-based scrim,
    // which made dark-mode users see a flash of white.
    let backdrop_cls = if open {
        "fixed inset-0 bg-bg/80 backdrop-blur-sm transition-opacity duration-300 z-40 opacity-100 pointer-events-auto"
    } else {
        "fixed inset-0 bg-bg/80 backdrop-blur-sm transition-opacity duration-300 z-40 opacity-0 pointer-events-none"
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

                // Combined drawer header: user identity on the left,
                // close button on the right. Logo lives in the topbar
                // (still visible above the open drawer), so we don't
                // need a second brand row inside the drawer.
                div { class: "px-5 py-4 bg-surface-2 border-b border-line flex items-center gap-3 shrink-0",
                    if !display_name.is_empty() {
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
                        div { class: "flex flex-col min-w-0 flex-1",
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
                    } else {
                        // Spacer so the close button still anchors right
                        // when the user is unauthenticated.
                        div { class: "flex-1" }
                    }
                    button {
                        onclick: move |_| is_open.set(false),
                        class: "nav-icon-btn shrink-0",
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

                // Nav groups — clicking a link closes the drawer.
                NavGroupList {
                    groups,
                    on_navigate: move |_| is_open.set(false),
                }
            }
        }
    }
}
