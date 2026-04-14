use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

#[server]
async fn get_swagger_url() -> Result<String, ServerFnError> {
    let cfg = crate::config::config();
    let base = cfg.api.external_url.trim_end_matches('/');
    Ok(format!("{base}/api/swagger-ui/"))
}

#[derive(Clone, Copy, PartialEq)]
enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    fn aria_label(self) -> &'static str {
        match self {
            Self::System => "Using system theme. Click for light mode",
            Self::Light => "Using light mode. Click for dark mode",
            Self::Dark => "Using dark mode. Click for system theme",
        }
    }
}

#[component]
fn ThemeIcon(mode: ThemeMode) -> Element {
    match mode {
        ThemeMode::System => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.5",
                view_box: "0 0 24 24",
                rect { x: "2", y: "3", width: "20", height: "14", rx: "2", ry: "2" }
                line { x1: "8", y1: "21", x2: "16", y2: "21" }
                line { x1: "12", y1: "17", x2: "12", y2: "21" }
            }
        },
        ThemeMode::Light => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                view_box: "0 0 24 24",
                circle { cx: "12", cy: "12", r: "5" }
                line { x1: "12", y1: "1", x2: "12", y2: "3" }
                line { x1: "12", y1: "21", x2: "12", y2: "23" }
                line { x1: "4.22", y1: "4.22", x2: "5.64", y2: "5.64" }
                line { x1: "18.36", y1: "18.36", x2: "19.78", y2: "19.78" }
                line { x1: "1", y1: "12", x2: "3", y2: "12" }
                line { x1: "21", y1: "12", x2: "23", y2: "12" }
                line { x1: "4.22", y1: "19.78", x2: "5.64", y2: "18.36" }
                line { x1: "18.36", y1: "5.64", x2: "19.78", y2: "4.22" }
            }
        },
        ThemeMode::Dark => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                view_box: "0 0 24 24",
                path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
            }
        },
    }
}

// -- Navigation Structure --

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
    let mut groups = vec![
        NavGroup {
            title: "Overview".to_string(),
            links: vec![
                NavLink::Internal(Route::ClusterList {}, "Clusters".to_string()),
                NavLink::Internal(Route::FleetDashboard { stage_id: None }, "Fleet".to_string()),
            ]
        },
    ];

    if is_admin {
        groups.push(NavGroup {
            title: "MCP + Skills".to_string(),
            links: vec![
                NavLink::Internal(Route::SkillList {}, "Skills".to_string()),
                NavLink::Internal(Route::McpServerList {}, "MCP Servers".to_string()),
                NavLink::Internal(Route::McpBundleList {}, "MCP Bundles".to_string()),
                NavLink::Internal(Route::BundleList {}, "Bundles".to_string()),
            ]
        });

        groups.push(NavGroup {
            title: "Admin".to_string(),
            links: vec![
                NavLink::Internal(Route::AdminTokens {}, "Admin Tokens".to_string()),
                NavLink::Internal(Route::OrganizationList {}, "Organizations".to_string()),
                NavLink::Internal(Route::UserList {}, "Users".to_string()),
            ]
        });

        groups.push(NavGroup {
            title: "Version".to_string(),
            links: vec![
                NavLink::Internal(Route::RolloutList {}, "Rollouts".to_string()),
                NavLink::Internal(Route::DaemonVersionList {}, "Daemon Versions".to_string()),
            ]
        });
    }

    let mut resources_links = vec![
        NavLink::Internal(Route::DocList {}, "Docs".to_string()),
    ];
    if let Some(url) = swagger_url {
        resources_links.push(NavLink::External(url, "API Docs".to_string()));
    }

    groups.push(NavGroup {
        title: "Resources".to_string(),
        links: resources_links,
    });

    groups
}

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
        aside { class: "hidden xl:flex xl:flex-col w-64 bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 overflow-y-auto",
            nav { class: "flex-1 px-4 py-6 space-y-8",
                for group in groups {
                    div { key: "{group.title}",
                        h3 { class: "px-3 text-xs font-semibold text-gray-500 uppercase tracking-wider", "{group.title}" }
                        div { class: "mt-2 space-y-1",
                            for link in group.links {
                                match link {
                                    NavLink::Internal(route, label) => rsx! {
                                        Link {
                                            key: "{label}",
                                            to: route.clone(),
                                            class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
                                            active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                                            "{label}"
                                        }
                                    },
                                    NavLink::External(url, label) => rsx! {
                                        a {
                                            key: "{label}",
                                            href: "{url}",
                                            target: "_blank",
                                            class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
                                            "{label}"
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

#[component]
pub fn Navbar(is_admin: bool, display_name: String) -> Element {
    let mut is_open = use_signal(|| false);
    let mut theme = use_signal(|| ThemeMode::System);

    let swagger_fut = use_server_future(get_swagger_url);
    let swagger_url: Option<String> = match swagger_fut {
        Ok(ref fut) => match &*fut.read() {
            Some(Ok(url)) => Some(url.clone()),
            _ => None,
        },
        Err(_) => None,
    };

    use_effect(move || {
        spawn(async move {
            let result = document::eval(r#"
                try {
                    var t = localStorage.getItem('theme');
                    if (t === 'dark') return 'dark';
                    if (t === 'light') return 'light';
                    return 'system';
                } catch(e) { return 'system'; }
            "#).await;
            if let Ok(val) = result {
                if let Some(s) = val.as_str() {
                    match s {
                        "dark" => theme.set(ThemeMode::Dark),
                        "light" => theme.set(ThemeMode::Light),
                        _ => theme.set(ThemeMode::System),
                    }
                }
            }
        });
    });

    let toggle_theme = move |_| {
        let next = theme().next();
        theme.set(next);
        let js = match next {
            ThemeMode::System => r#"
                localStorage.removeItem('theme');
                var d = document.documentElement;
                if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
                    d.classList.add('dark');
                    d.style.colorScheme = 'dark';
                    d.style.backgroundColor = '#111827';
                } else {
                    d.classList.remove('dark');
                    d.style.colorScheme = 'light';
                    d.style.backgroundColor = '#f9fafb';
                }
            "#,
            ThemeMode::Light => r#"
                localStorage.setItem('theme', 'light');
                var d = document.documentElement;
                d.classList.remove('dark');
                d.style.colorScheme = 'light';
                d.style.backgroundColor = '#f9fafb';
            "#,
            ThemeMode::Dark => r#"
                localStorage.setItem('theme', 'dark');
                var d = document.documentElement;
                d.classList.add('dark');
                d.style.colorScheme = 'dark';
                d.style.backgroundColor = '#111827';
            "#,
        };
        document::eval(js);
    };

    let current_aria = theme().aria_label();
    let current_theme = theme();

    rsx! {
        nav { class: "bg-white dark:bg-gray-800 shadow dark:shadow-gray-900/30 shrink-0",
            div { class: "w-full mx-auto px-4 sm:px-6 lg:px-8",
                div { class: "flex justify-between h-16 items-center",
                    // Left side: Logo
                    Link { to: Route::ClusterList {},
                        h1 { class: "text-xl font-bold text-gray-900 dark:text-white", "mac-mgmt" }
                    }

                    // Right side: Profile & Theme (Desktop & Mobile share some parts)
                    div { class: "flex space-x-1 items-center",
                        
                        // Desktop user icon
                        if !display_name.is_empty() {
                            div { class: "hidden xl:flex items-center",
                                Link {
                                    to: Route::Profile {},
                                    class: "ml-3 flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors",
                                    active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                                    svg {
                                        class: "h-5 w-5 shrink-0",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "1.5",
                                        view_box: "0 0 24 24",
                                        path {
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            d: "M17.982 18.725A7.488 7.488 0 0 0 12 15.75a7.488 7.488 0 0 0-5.982 2.975m11.963 0a9 9 0 1 0-11.963 0m11.963 0A8.966 8.966 0 0 1 12 21a8.966 8.966 0 0 1-5.982-2.275M15 9.75a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
                                        }
                                    }
                                    "{display_name}"
                                }
                            }
                        }
                        
                        // Theme Toggle
                        button {
                            onclick: toggle_theme,
                            class: "ml-1 p-2 rounded-md text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors",
                            "aria-label": "{current_aria}",
                            title: "{current_aria}",
                            ThemeIcon { mode: current_theme }
                        }
                        
                        // Hamburger button (Mobile)
                        button {
                            onclick: move |_| is_open.set(!is_open()),
                            class: "xl:hidden ml-1 inline-flex items-center justify-center p-2 rounded-md text-gray-400 dark:text-gray-300 hover:text-gray-500 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-blue-500",
                            "aria-expanded": "{is_open}",
                            "aria-controls": "mobile-drawer",
                            span { class: "sr-only", "Open main menu" }
                            svg {
                                class: "h-6 w-6",
                                fill: "none",
                                stroke: "currentColor",
                                view_box: "0 0 24 24",
                                if *is_open.read() {
                                    path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M6 18L18 6M6 6l12 12" }
                                } else {
                                    path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M4 6h16M4 12h16M4 18h16" }
                                }
                            }
                        }
                    }
                }
            }

            // Mobile Slide-in Drawer
            div {
                id: "mobile-drawer-container",
                class: "xl:hidden relative z-50",
                
                // Backdrop
                div {
                    class: if *is_open.read() {
                        "fixed inset-0 bg-gray-900/80 backdrop-blur-sm transition-opacity duration-300 z-40 opacity-100 pointer-events-auto"
                    } else {
                        "fixed inset-0 bg-gray-900/80 backdrop-blur-sm transition-opacity duration-300 z-40 opacity-0 pointer-events-none"
                    },
                    "aria-hidden": "true",
                    onclick: move |_| is_open.set(false),
                }
                
                // Drawer
                div {
                    class: if *is_open.read() {
                        "fixed inset-y-0 right-0 max-w-xs w-full bg-white dark:bg-gray-800 shadow-xl overflow-y-auto flex flex-col z-50 transform transition-transform duration-300 ease-in-out border-l border-gray-200 dark:border-gray-700 translate-x-0 pointer-events-auto"
                    } else {
                        "fixed inset-y-0 right-0 max-w-xs w-full bg-white dark:bg-gray-800 shadow-xl overflow-y-auto flex flex-col z-50 transform transition-transform duration-300 ease-in-out border-l border-gray-200 dark:border-gray-700 translate-x-full pointer-events-none"
                    },
                    
                    // Header Area with User & Close Button
                    div { class: "p-4 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 flex justify-between items-center",
                        div { class: "flex-1 mr-4 overflow-hidden",
                            if !display_name.is_empty() {
                                div { class: "flex items-center gap-3",
                                    div { class: "flex-shrink-0",
                                        svg { class: "h-10 w-10 text-gray-400 bg-white dark:bg-gray-700 rounded-full p-2 border border-gray-200 dark:border-gray-600", fill: "none", stroke: "currentColor", view_box: "0 0 24 24", stroke_width: "1.5",
                                            path { stroke_linecap: "round", stroke_linejoin: "round", d: "M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z" }
                                        }
                                    }
                                    div { class: "flex flex-col overflow-hidden",
                                        span { class: "text-sm font-medium text-gray-900 dark:text-white truncate block", "{display_name}" }
                                        Link {
                                            to: Route::Profile {},
                                            class: "text-xs text-blue-600 dark:text-blue-400 hover:underline block",
                                            onclick: move |_| is_open.set(false),
                                            "View Profile"
                                        }
                                    }
                                }

                            }
                        }
                        
                        button {
                            onclick: move |_| is_open.set(false),
                            class: "flex-shrink-0 p-2 -mr-2 rounded-md text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700 focus:outline-none transition-colors",
                            "aria-label": "Close menu",
                            svg {
                                class: "h-6 w-6",
                                fill: "none",
                                stroke: "currentColor",
                                view_box: "0 0 24 24",
                                path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M6 18L18 6M6 6l12 12" }
                            }
                        }
                    }
                    
                    // Navigation Groups
                    nav { class: "flex-1 px-4 py-6 space-y-8",
                        for group in get_nav_groups(is_admin, swagger_url.clone()) {
                            div { key: "{group.title}",
                                h3 { class: "px-3 text-xs font-semibold text-gray-500 uppercase tracking-wider", "{group.title}" }
                                div { class: "mt-2 space-y-1",
                                    for link in group.links {
                                        match link {
                                            NavLink::Internal(route, label) => rsx! {
                                                Link {
                                                    key: "{label}",
                                                    to: route.clone(),
                                                    class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
                                                    active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                                                    onclick: move |_| is_open.set(false),
                                                    "{label}"
                                                }
                                            },
                                            NavLink::External(url, label) => rsx! {
                                                a {
                                                    key: "{label}",
                                                    href: "{url}",
                                                    target: "_blank",
                                                    class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
                                                    onclick: move |_| is_open.set(false),
                                                    "{label}"
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
}

