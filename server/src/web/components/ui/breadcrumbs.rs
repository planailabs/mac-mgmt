//! Page breadcrumbs.
//!
//! Rendered by `Layout` at the top of every page. Each chain leads with a
//! non-clickable category label (the sidebar group the page belongs to),
//! continues through clickable parent crumbs, and ends at the current page
//! (also non-clickable). The dynamic last-crumb label is taken from the
//! `TopbarMeta` context that pages populate via `use_topbar`; if the page
//! hasn't set a title we fall back to the static route label so the chain
//! never trails a separator.
//!
//! Chains are derived directly from the sidebar grouping in `navbar.rs`
//! and the route family used by `route_active`. Any new route must be
//! added to `chain_keys` below — the match is exhaustive on `Route`.

use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;
use crate::web::components::topbar::TopbarMeta;

/// One entry in the breadcrumb chain.
struct Entry {
    /// Fluent key for the static label. Translated by the component.
    label_key: &'static str,
    /// `Some` for clickable parent crumbs, `None` for the leading category
    /// and the trailing current-page crumb.
    link: Option<Route>,
    /// When `true`, the component substitutes `TopbarMeta.title` for the
    /// label if the page has set one — used for entity-detail pages.
    dynamic: bool,
}

const fn cat(key: &'static str) -> Entry {
    Entry { label_key: key, link: None, dynamic: false }
}

fn parent(key: &'static str, route: Route) -> Entry {
    Entry { label_key: key, link: Some(route), dynamic: false }
}

const fn cur(key: &'static str) -> Entry {
    Entry { label_key: key, link: None, dynamic: false }
}

const fn cur_dyn(fallback: &'static str) -> Entry {
    Entry { label_key: fallback, link: None, dynamic: true }
}

/// Clickable parent crumb whose label resolves from `TopbarMeta.title`
/// at render time. Used when an ancestor route is an entity-detail page
/// (e.g. a specific cluster) and we want the entity's name in the chain
/// rather than the static category label "Clusters" repeated twice.
fn parent_dyn(fallback: &'static str, route: Route) -> Entry {
    Entry { label_key: fallback, link: Some(route), dynamic: true }
}

/// Build the chain for a given route. Categories use the existing
/// `nav-*` Fluent keys so translations stay in sync with the sidebar.
fn chain_keys(route: &Route) -> Vec<Entry> {
    use Route::*;
    match route {
        // ── Overview category (clusters / fleet / easy-access / command center)
        Overview {} => vec![cat("nav-overview"), cur("nav-command-center")],
        ClusterList {} => vec![cat("nav-overview"), cur("nav-clusters")],
        ClusterForm {} => vec![
            cat("nav-overview"),
            parent("nav-clusters", ClusterList {}),
            cur("breadcrumb-new"),
        ],
        ClusterDetail { .. } => vec![
            cat("nav-overview"),
            parent("nav-clusters", ClusterList {}),
            cur_dyn("nav-clusters"),
        ],
        ClusterConfigPage { id } => vec![
            cat("nav-overview"),
            parent("nav-clusters", ClusterList {}),
            parent_dyn("nav-clusters", ClusterDetail { id: id.clone() }),
            cur("breadcrumb-config"),
        ],
        ClusterPackagesPage { id } => vec![
            cat("nav-overview"),
            parent("nav-clusters", ClusterList {}),
            parent_dyn("nav-clusters", ClusterDetail { id: id.clone() }),
            cur("breadcrumb-packages"),
        ],
        FleetDashboard { .. } => vec![cat("nav-overview"), cur("nav-fleet")],
        FleetDetail { .. }
        | FleetFiles { .. }
        | FleetShell { .. }
        | FleetLogs { .. }
        | FleetHealer { .. }
        | FleetHealerSession { .. } => vec![
            cat("nav-overview"),
            parent("nav-fleet", FleetDashboard { stage_id: None }),
            cur_dyn("nav-fleet"),
        ],
        EasyAccess {} => vec![cat("nav-overview"), cur("nav-easy-access")],

        // ── MCP + Skills
        SkillList {} => vec![cat("nav-mcp-skills"), cur("nav-skills")],
        SkillDetail { .. } => vec![
            cat("nav-mcp-skills"),
            parent("nav-skills", SkillList {}),
            cur_dyn("nav-skills"),
        ],
        McpServerList {} => vec![cat("nav-mcp-skills"), cur("nav-mcp-servers")],
        McpServerForm {} => vec![
            cat("nav-mcp-skills"),
            parent("nav-mcp-servers", McpServerList {}),
            cur("breadcrumb-new"),
        ],
        McpServerEdit { id } => vec![
            cat("nav-mcp-skills"),
            parent("nav-mcp-servers", McpServerList {}),
            parent("nav-mcp-servers", McpServerDetail { id: id.clone() }),
            cur("breadcrumb-edit"),
        ],
        McpServerDetail { .. } => vec![
            cat("nav-mcp-skills"),
            parent("nav-mcp-servers", McpServerList {}),
            cur_dyn("nav-mcp-servers"),
        ],
        McpBundleList {} => vec![cat("nav-mcp-skills"), cur("nav-mcp-bundles")],
        McpBundleForm {} => vec![
            cat("nav-mcp-skills"),
            parent("nav-mcp-bundles", McpBundleList {}),
            cur("breadcrumb-new"),
        ],
        McpBundleDetail { .. } => vec![
            cat("nav-mcp-skills"),
            parent("nav-mcp-bundles", McpBundleList {}),
            cur_dyn("nav-mcp-bundles"),
        ],
        BundleList {} => vec![cat("nav-mcp-skills"), cur("nav-bundles")],
        BundleForm {} => vec![
            cat("nav-mcp-skills"),
            parent("nav-bundles", BundleList {}),
            cur("breadcrumb-new"),
        ],
        BundleDetail { .. } => vec![
            cat("nav-mcp-skills"),
            parent("nav-bundles", BundleList {}),
            cur_dyn("nav-bundles"),
        ],

        // ── Import
        ImportSources { .. } => vec![cat("nav-import"), cur("nav-import-sources")],
        ImportSourcesSearch {} => vec![
            cat("nav-import"),
            parent("nav-import-sources", ImportSources { prefill_slug: None, prefill_name: None }),
            cur("breadcrumb-search"),
        ],
        ImportSourceEdit { id } => vec![
            cat("nav-import"),
            parent("nav-import-sources", ImportSources { prefill_slug: None, prefill_name: None }),
            parent("nav-import-sources", ImportSourceDetail { id: id.clone() }),
            cur("breadcrumb-edit"),
        ],
        ImportSourceDetail { .. } => vec![
            cat("nav-import"),
            parent("nav-import-sources", ImportSources { prefill_slug: None, prefill_name: None }),
            cur_dyn("nav-import-sources"),
        ],

        // ── Admin
        AdminTokens {} => vec![cat("nav-admin"), cur("nav-admin-tokens")],
        AdminClientCerts {} => vec![cat("nav-admin"), cur("nav-admin-client-certs")],
        AdminClientCas {} => vec![cat("nav-admin"), cur("nav-admin-client-cas")],
        StaffPings {} => vec![cat("nav-admin"), cur("nav-staff-pings")],
        OrganizationList {} => vec![cat("nav-admin"), cur("nav-organizations")],
        OrganizationForm {} => vec![
            cat("nav-admin"),
            parent("nav-organizations", OrganizationList {}),
            cur("breadcrumb-new"),
        ],
        OrganizationDetail { .. } => vec![
            cat("nav-admin"),
            parent("nav-organizations", OrganizationList {}),
            cur_dyn("nav-organizations"),
        ],
        UserList {} => vec![cat("nav-admin"), cur("nav-users")],
        UserForm {} => vec![
            cat("nav-admin"),
            parent("nav-users", UserList {}),
            cur("breadcrumb-new"),
        ],
        UserDetail { .. } => vec![
            cat("nav-admin"),
            parent("nav-users", UserList {}),
            cur_dyn("nav-users"),
        ],
        SkillCenterList {} => vec![cat("nav-admin"), cur("nav-skill-centers")],
        SkillCenterForm {} => vec![
            cat("nav-admin"),
            parent("nav-skill-centers", SkillCenterList {}),
            cur("breadcrumb-new"),
        ],
        SkillCenterDetail { .. } => vec![
            cat("nav-admin"),
            parent("nav-skill-centers", SkillCenterList {}),
            cur_dyn("nav-skill-centers"),
        ],

        // ── Version
        RolloutList {} => vec![cat("nav-version"), cur("nav-rollouts")],
        RolloutForm {} => vec![
            cat("nav-version"),
            parent("nav-rollouts", RolloutList {}),
            cur("breadcrumb-new"),
        ],
        RolloutDetail { .. } => vec![
            cat("nav-version"),
            parent("nav-rollouts", RolloutList {}),
            cur_dyn("nav-rollouts"),
        ],
        RolloutGroupList {} => vec![cat("nav-version"), cur("nav-rollout-groups")],
        RolloutGroupDetail { .. } => vec![
            cat("nav-version"),
            parent("nav-rollout-groups", RolloutGroupList {}),
            cur_dyn("nav-rollout-groups"),
        ],
        DaemonVersionList {} => vec![cat("nav-version"), cur("nav-daemon-versions")],
        DaemonVersionDetail { .. } => vec![
            cat("nav-version"),
            parent("nav-daemon-versions", DaemonVersionList {}),
            cur_dyn("nav-daemon-versions"),
        ],

        // ── Resources
        DocList {} => vec![cat("nav-resources"), cur("nav-docs")],
        DocPage { .. } => vec![
            cat("nav-resources"),
            parent("nav-docs", DocList {}),
            cur_dyn("nav-docs"),
        ],

        // ── Standalone
        Profile {} => vec![cur("breadcrumb-profile")],
    }
}

#[component]
pub fn Breadcrumbs() -> Element {
    let route = use_route::<Route>();
    let meta = use_context::<Signal<TopbarMeta>>();
    let dynamic_label = meta.read().title.clone();

    let entries = chain_keys(&route);
    if entries.is_empty() {
        return rsx! {};
    }

    rsx! {
        nav {
            class: "breadcrumbs",
            "aria-label": t!("breadcrumb-aria"),
            for (idx, entry) in entries.into_iter().enumerate() {
                if idx > 0 {
                    span { class: "breadcrumb-sep", "›" }
                }
                {
                    let label = if entry.dynamic && !dynamic_label.is_empty() {
                        dynamic_label.clone()
                    } else {
                        t!(entry.label_key)
                    };
                    match entry.link {
                        Some(target) => rsx! {
                            Link { to: target, class: "breadcrumb-link", "{label}" }
                        },
                        None if idx == 0 => rsx! {
                            span { class: "breadcrumb-category", "{label}" }
                        },
                        None => rsx! {
                            span { class: "breadcrumb-current", "{label}" }
                        },
                    }
                }
            }
        }
    }
}
