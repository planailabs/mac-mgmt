use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, t, unic_langid::langid};

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

/// Available locales with their native display names.
const LOCALES: &[(&str, &str)] = &[("en-US", "English"), ("de-DE", "Deutsch")];

#[component]
fn LanguagePicker() -> Element {
    let mut i18n = i18n();
    let current = i18n.language();
    let current_tag = current.to_string();

    let on_change = move |evt: Event<FormData>| {
        let val = evt.value();
        if val == "de-DE" {
            let _ = i18n.set_language(langid!("de-DE"));
        } else {
            let _ = i18n.set_language(langid!("en-US"));
        }
        document::eval(&format!(
            "try {{ localStorage.setItem('lang', '{}'); }} catch(e) {{}}",
            val
        ));
    };

    rsx! {
        div { class: "relative ml-1",
            label { class: "sr-only", r#for: "lang-picker", {t!("language-picker-label")} }
            select {
                id: "lang-picker",
                class: "appearance-none bg-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm rounded-md px-2 py-2 pr-6 cursor-pointer focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors",
                value: "{current_tag}",
                onchange: on_change,
                for &(tag, label) in LOCALES.iter() {
                    option {
                        key: "{tag}",
                        value: "{tag}",
                        selected: tag == current_tag,
                        "{label}"
                    }
                }
            }
            // Dropdown chevron
            svg {
                class: "pointer-events-none absolute right-1 top-1/2 -translate-y-1/2 h-3 w-3 text-gray-400",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                view_box: "0 0 24 24",
                path { stroke_linecap: "round", stroke_linejoin: "round", d: "M19 9l-7 7-7-7" }
            }
        }
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
                NavLink::Internal(Route::ImportSources {}, "nav-import-sources".to_string()),
            ],
        });

        {
            let admin_links = vec![
                NavLink::Internal(Route::AdminTokens {}, "nav-admin-tokens".to_string()),
                NavLink::Internal(Route::StaffPings {}, "nav-staff-pings".to_string()),
                NavLink::Internal(Route::OrganizationList {}, "nav-organizations".to_string()),
                NavLink::Internal(Route::UserList {}, "nav-users".to_string()),
                NavLink::Internal(Route::SkillCenterList {}, "nav-skill-centers".to_string()),
            ];
            groups.push(NavGroup {
                title: "nav-admin".to_string(),
                links: admin_links,
            });

            groups.push(NavGroup {
                title: "nav-version".to_string(),
                links: vec![
                    NavLink::Internal(Route::RolloutList {}, "nav-rollouts".to_string()),
                    NavLink::Internal(Route::DaemonVersionList {}, "nav-daemon-versions".to_string()),
                ],
            });
        }
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
                        h3 { class: "px-3 text-xs font-semibold text-gray-500 uppercase tracking-wider", {t!(&group.title)} }
                        div { class: "mt-2 space-y-1",
                            for link in group.links {
                                match link {
                                    NavLink::Internal(route, label) => rsx! {
                                        Link {
                                            key: "{label}",
                                            to: route.clone(),
                                            class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
                                            active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                                            {t!(&label)}
                                        }
                                    },
                                    NavLink::External(url, label) => rsx! {
                                        a {
                                            key: "{label}",
                                            href: "{url}",
                                            target: "_blank",
                                            class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
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
            let result = document::eval(
                r#"
                try {
                    var t = localStorage.getItem('theme');
                    if (t === 'dark') return 'dark';
                    if (t === 'light') return 'light';
                    return 'system';
                } catch(e) { return 'system'; }
            "#,
            )
            .await;
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
            ThemeMode::System => {
                r#"
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
            "#
            }
            ThemeMode::Light => {
                r#"
                localStorage.setItem('theme', 'light');
                var d = document.documentElement;
                d.classList.remove('dark');
                d.style.colorScheme = 'light';
                d.style.backgroundColor = '#f9fafb';
            "#
            }
            ThemeMode::Dark => {
                r#"
                localStorage.setItem('theme', 'dark');
                var d = document.documentElement;
                d.classList.add('dark');
                d.style.colorScheme = 'dark';
                d.style.backgroundColor = '#111827';
            "#
            }
        };
        document::eval(js);
    };

    let current_aria = match theme() {
        ThemeMode::System => t!("theme-system"),
        ThemeMode::Light => t!("theme-light"),
        ThemeMode::Dark => t!("theme-dark"),
    };
    let current_theme = theme();

    rsx! {
        nav { class: "bg-white dark:bg-gray-800 shadow dark:shadow-gray-900/30 shrink-0",
            div { class: "w-full mx-auto px-4 sm:px-6 lg:px-8",
                div { class: "flex justify-between h-16 items-center",
                    // Left side: Logo
                    Link { to: Route::ClusterList {},
                        h1 { class: "text-xl font-bold text-gray-900 dark:text-white", {t!("nav-logo")} }
                    }

                    // Right side: Profile & Theme (Desktop & Mobile share some parts)
                    div { class: "flex space-x-1 items-center",

                        // Desktop user icon + logout
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
                                a {
                                    href: "/auth/logout",
                                    class: "ml-1 p-2 rounded-md text-gray-500 dark:text-gray-400 hover:text-red-600 dark:hover:text-red-400 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors",
                                    title: t!("nav-sign-out"),
                                    svg {
                                        class: "h-5 w-5",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "1.5",
                                        view_box: "0 0 24 24",
                                        path {
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            d: "M15.75 9V5.25A2.25 2.25 0 0 0 13.5 3h-6a2.25 2.25 0 0 0-2.25 2.25v13.5A2.25 2.25 0 0 0 7.5 21h6a2.25 2.25 0 0 0 2.25-2.25V15m3-3h-9m9 0-3-3m3 3-3 3",
                                        }
                                    }
                                }
                            }
                        }

                        // Language Picker
                        LanguagePicker {}

                        // Theme Toggle
                        button {
                            onclick: toggle_theme,
                            class: "ml-1 p-2 rounded-md text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors",
                            "aria-label": current_aria.clone(),
                            title: current_aria,
                            ThemeIcon { mode: current_theme }
                        }

                        // Hamburger button (Mobile)
                        button {
                            onclick: move |_| is_open.set(!is_open()),
                            class: "xl:hidden ml-1 inline-flex items-center justify-center p-2 rounded-md text-gray-400 dark:text-gray-300 hover:text-gray-500 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-blue-500",
                            "aria-expanded": "{is_open}",
                            "aria-controls": "mobile-drawer",
                            span { class: "sr-only", {t!("nav-open-main-menu")} }
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
                                        div { class: "flex gap-3",
                                            Link {
                                                to: Route::Profile {},
                                                class: "text-xs text-blue-600 dark:text-blue-400 hover:underline block",
                                                onclick: move |_| is_open.set(false),
                                                {t!("nav-view-profile")}
                                            }
                                            a {
                                                href: "/auth/logout",
                                                class: "text-xs text-red-600 dark:text-red-400 hover:underline block",
                                                {t!("nav-sign-out")}
                                            }
                                        }
                                    }
                                }

                            }
                        }

                        button {
                            onclick: move |_| is_open.set(false),
                            class: "flex-shrink-0 p-2 -mr-2 rounded-md text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700 focus:outline-none transition-colors",
                            "aria-label": t!("nav-close-menu"),
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
                                h3 { class: "px-3 text-xs font-semibold text-gray-500 uppercase tracking-wider", {t!(&group.title)} }
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
                                                    {t!(&label)}
                                                }
                                            },
                                            NavLink::External(url, label) => rsx! {
                                                a {
                                                    key: "{label}",
                                                    href: "{url}",
                                                    target: "_blank",
                                                    class: "group flex items-center px-3 py-2 text-sm font-medium rounded-md text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 hover:text-gray-900 dark:hover:text-white transition-colors",
                                                    onclick: move |_| is_open.set(false),
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
    }
}
