use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

#[server]
async fn get_swagger_url() -> Result<String, ServerFnError> {
    let cfg = crate::config::config();
    let base = cfg.api.external_url.trim_end_matches('/');
    Ok(format!("{base}/api/swagger-ui/"))
}

/// Theme preference: system (follow OS), light (forced), or dark (forced).
/// Cycles: system → light → dark → system
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

/// Inline SVG icon for the current theme state.
#[component]
fn ThemeIcon(mode: ThemeMode) -> Element {
    match mode {
        // Monitor icon — "follow system"
        ThemeMode::System => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.5",
                view_box: "0 0 24 24",
                // Monitor screen
                rect { x: "2", y: "3", width: "20", height: "14", rx: "2", ry: "2" }
                // Stand
                line { x1: "8", y1: "21", x2: "16", y2: "21" }
                line { x1: "12", y1: "17", x2: "12", y2: "21" }
            }
        },
        // Sun icon — "light mode"
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
        // Moon icon — "dark mode"
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

#[component]
pub fn Navbar(is_admin: bool, real_is_admin: bool, display_name: String) -> Element {
    // State for the mobile hamburger menu
    let mut is_open = use_signal(|| false);
    // Theme state
    let mut theme = use_signal(|| ThemeMode::System);

    // Fetch swagger URL from config (only meaningful for admins)
    let swagger_fut = use_server_future(get_swagger_url);
    let swagger_url: Option<String> = match swagger_fut {
        Ok(ref fut) => match &*fut.read() {
            Some(Ok(url)) => Some(url.clone()),
            _ => None,
        },
        Err(_) => None,
    };

    // On mount: read localStorage.theme to sync signal with actual state
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

    // Links visible to all authenticated users
    let common_links: Vec<(Route, &str)> = vec![
        (Route::ClusterList {}, "Clusters"),
        (Route::FleetDashboard {}, "Fleet"),
        (Route::DocList {}, "Docs"),
    ];

    // Links visible only to admins
    let admin_links: Vec<(Route, &str)> = if is_admin {
        vec![
            (Route::SkillList {}, "Skills"),
            (Route::BundleList {}, "Bundles"),
            (Route::McpServerList {}, "MCP Servers"),
            (Route::McpBundleList {}, "MCP Bundles"),
            (Route::AdminTokens {}, "Admin Tokens"),
            (Route::RolloutList {}, "Rollouts"),
            (Route::DaemonVersionList {}, "Daemon Versions"),
            (Route::OrganizationList {}, "Organizations"),
            (Route::UserList {}, "Users"),
        ]
    } else {
        vec![]
    };

    let all_links: Vec<(Route, &str)> = common_links.into_iter().chain(admin_links).collect();

    let current_aria = theme().aria_label();
    let current_theme = theme();

    rsx! {
        nav { class: "bg-white dark:bg-gray-800 shadow dark:shadow-gray-900/30",
            div { class: "max-w-7xl mx-auto px-4 sm:px-6 lg:px-8",
                div { class: "flex justify-between h-16 items-center",
                    Link { to: Route::ClusterList {},
                        h1 { class: "text-xl font-bold text-gray-900 dark:text-white", "mac-mgmt" }
                    }

                    // Desktop menu (visible on xl and larger)
                    div { class: "hidden xl:flex xl:space-x-1 xl:items-center",
                        for (route, label) in all_links.clone() {
                            Link {
                                key: "{label}",
                                to: route,
                                class: "whitespace-nowrap px-3 py-2 rounded-md text-sm font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors",
                                active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                                "{label}"
                            }
                        }
                        // Swagger UI link (admin-only, external)
                        if is_admin {
                            if let Some(ref url) = swagger_url {
                                a {
                                    href: "{url}",
                                    target: "_blank",
                                    class: "whitespace-nowrap px-3 py-2 rounded-md text-sm font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors",
                                    "API Docs"
                                }
                            }
                        }
                        // Impersonation control (only for real admins)
                        if real_is_admin {
                            ImpersonateSelector {}
                        }
                        // Logged-in user display (links to profile)
                        if !display_name.is_empty() {
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
                        // Desktop theme toggle
                        button {
                            onclick: toggle_theme,
                            class: "ml-3 p-2 rounded-md text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors",
                            "aria-label": "{current_aria}",
                            title: "{current_aria}",
                            ThemeIcon { mode: current_theme }
                        }
                    }

                    // Mobile: theme toggle + hamburger (visible below xl)
                    div { class: "flex items-center gap-1 xl:hidden",
                        // Mobile theme toggle
                        button {
                            onclick: toggle_theme,
                            class: "p-2 rounded-md text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors",
                            "aria-label": "{current_aria}",
                            title: "{current_aria}",
                            ThemeIcon { mode: current_theme }
                        }
                        // Hamburger button
                        button {
                            onclick: move |_| is_open.set(!is_open()),
                            class: "inline-flex items-center justify-center p-2 rounded-md text-gray-400 dark:text-gray-300 hover:text-gray-500 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-blue-500",
                            "aria-expanded": "{is_open}",
                            "aria-controls": "mobile-menu",
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

            // Mobile Dropdown Menu
            if *is_open.read() {
                div {
                    id: "mobile-menu",
                    class: "xl:hidden border-t border-gray-100 dark:border-gray-700 bg-white dark:bg-gray-800 shadow-lg",
                    if !display_name.is_empty() {
                        Link {
                            to: Route::Profile {},
                            class: "flex items-center gap-2 mx-2 mt-2 px-3 py-2 rounded-md text-base font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors",
                            active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                            onclick: move |_| is_open.set(false),
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
                    div { class: "px-2 pt-2 pb-3 space-y-1 sm:px-3",
                        for (route, label) in all_links.clone() {
                            Link {
                                key: "{label}",
                                to: route,
                                class: "block px-3 py-2 rounded-md text-base font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors",
                                active_class: "!bg-gray-100 dark:!bg-gray-700 !text-gray-900 dark:!text-white",
                                onclick: move |_| is_open.set(false), // Close menu when navigating
                                "{label}"
                            }
                        }
                        if is_admin {
                            if let Some(ref url) = swagger_url {
                                a {
                                    href: "{url}",
                                    target: "_blank",
                                    class: "block px-3 py-2 rounded-md text-base font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors",
                                    "API Docs"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Impersonation selector ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ImpersonateUser {
    id: String,
    email: String,
}

#[server]
async fn get_impersonation_targets() -> Result<Vec<ImpersonateUser>, ServerFnError> {
    use crate::web::user::current_user;
    let user = current_user().await?;
    if !user.is_admin || user.impersonating_from.is_some() {
        return Ok(vec![]);
    }
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row { id: uuid::Uuid, email: String }

    let rows = sqlx::query_as::<_, Row>("SELECT id, email FROM users ORDER BY email")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| ImpersonateUser { id: r.id.to_string(), email: r.email }).collect())
}

#[component]
fn ImpersonateSelector() -> Element {
    let targets = use_server_future(get_impersonation_targets);

    let users = match targets {
        Ok(ref fut) => match &*fut.read() {
            Some(Ok(list)) if !list.is_empty() => list.clone(),
            _ => return rsx! {},
        },
        Err(_) => return rsx! {},
    };

    rsx! {
        select {
            class: "ml-2 border border-gray-300 dark:border-gray-600 rounded px-1 py-1 text-xs bg-white dark:bg-gray-700 dark:text-white max-w-[160px]",
            onchange: move |e| {
                let val = e.value();
                if val.is_empty() {
                    document::eval(
                        "document.cookie = 'impersonate_user_id=; Path=/; Max-Age=0'; window.location.reload();"
                    );
                } else {
                    let js = format!(
                        "document.cookie = 'impersonate_user_id={val}; Path=/; SameSite=Lax'; window.location.reload();"
                    );
                    document::eval(&js);
                }
            },
            option { value: "", "Impersonate…" }
            for u in &users {
                option { value: "{u.id}", "{u.email}" }
            }
        }
    }
}
