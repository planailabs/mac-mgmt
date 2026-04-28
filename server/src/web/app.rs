use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, unic_langid::langid};

use super::components::admin_tokens_page::AdminTokens;
use super::components::bundle_detail::BundleDetail;
use super::components::bundle_form::BundleForm;
use super::components::bundle_list::BundleList;
use super::components::cluster_config_page::ClusterConfigPage;
use super::components::cluster_detail::ClusterDetail;
use super::components::cluster_packages::ClusterPackagesPage;
use super::components::cluster_form::ClusterForm;
use super::components::cluster_list::ClusterList;
use super::components::daemon_version_detail::DaemonVersionDetail;
use super::components::daemon_version_list::DaemonVersionList;
use super::components::docs::{DocList, DocPage};
use super::components::easy_access::EasyAccess;
use super::components::skill_center_detail::SkillCenterDetail;
use super::components::skill_center_form::SkillCenterForm;
use super::components::skill_center_list::SkillCenterList;
use super::components::file_editor::FleetFiles;
use super::components::fleet_dashboard::FleetDashboard;
use super::components::fleet_detail::FleetDetail;
use super::components::healer_page::{FleetHealer, FleetHealerSession};
use super::components::layout::Layout;
use super::components::log_viewer::FleetLogs;
use super::components::mcp_bundle_detail::McpBundleDetail;
use super::components::mcp_bundle_form::McpBundleForm;
use super::components::mcp_bundle_list::McpBundleList;
use super::components::mcp_server_detail::{McpServerDetail, McpServerEdit, McpServerForm};
use super::components::mcp_server_list::McpServerList;
use super::components::organization_detail::OrganizationDetail;
use super::components::organization_form::OrganizationForm;
use super::components::organization_list::OrganizationList;
use super::components::profile::Profile;
use super::components::rollout_detail::RolloutDetail;
use super::components::rollout_form::RolloutForm;
use super::components::rollout_group_detail::RolloutGroupDetail;
use super::components::rollout_group_list::RolloutGroupList;
use super::components::rollout_list::RolloutList;
use super::components::shell_commands::FleetShell;
use super::components::skill_detail::SkillDetail;
use super::components::skill_list::SkillList;
use super::components::import_sources::{ImportSourceDetail, ImportSources, ImportSourcesSearch};
use super::components::staff_pings_page::StaffPings;
use super::components::user_detail::UserDetail;
use super::components::user_form::UserForm;
use super::components::user_list::UserList;

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    ClusterList {},
    #[route("/clusters/new")]
    ClusterForm {},
    #[route("/clusters/:id")]
    ClusterDetail { id: String },
    #[route("/clusters/:id/config")]
    ClusterConfigPage { id: String },
    #[route("/clusters/:id/packages")]
    ClusterPackagesPage { id: String },
    #[route("/skills")]
    SkillList {},
    #[route("/skills/:id")]
    SkillDetail { id: String },
    #[route("/bundles")]
    BundleList {},
    #[route("/bundles/new")]
    BundleForm {},
    #[route("/bundles/:id")]
    BundleDetail { id: String },
    #[route("/mcp-servers")]
    McpServerList {},
    #[route("/mcp-servers/new")]
    McpServerForm {},
    #[route("/mcp-servers/:id/edit")]
    McpServerEdit { id: String },
    #[route("/mcp-servers/:id")]
    McpServerDetail { id: String },
    #[route("/mcp-bundles")]
    McpBundleList {},
    #[route("/mcp-bundles/new")]
    McpBundleForm {},
    #[route("/mcp-bundles/:id")]
    McpBundleDetail { id: String },
    #[route("/admin-tokens")]
    AdminTokens {},
    #[route("/rollouts")]
    RolloutList {},
    #[route("/rollouts/new")]
    RolloutForm {},
    #[route("/rollouts/:id")]
    RolloutDetail { id: String },
    #[route("/easy-access")]
    EasyAccess {},
    #[route("/fleet?:stage_id")]
    FleetDashboard { stage_id: Option<String> },
    #[route("/fleet/:instance_id")]
    FleetDetail { instance_id: String },
    #[route("/fleet/:instance_id/files")]
    FleetFiles { instance_id: String },
    #[route("/fleet/:instance_id/shell")]
    FleetShell { instance_id: String },
    #[route("/fleet/:instance_id/logs")]
    FleetLogs { instance_id: String },
    #[route("/fleet/:instance_id/healer")]
    FleetHealer { instance_id: String },
    #[route("/fleet/:instance_id/healer/:session_id")]
    FleetHealerSession { instance_id: String, session_id: String },
    #[route("/staff-pings")]
    StaffPings {},
    #[route("/rollout-groups")]
    RolloutGroupList {},
    #[route("/rollout-groups/:id")]
    RolloutGroupDetail { id: String },
    #[route("/daemon-versions")]
    DaemonVersionList {},
    #[route("/daemon-versions/:version")]
    DaemonVersionDetail { version: String },
    #[route("/organizations")]
    OrganizationList {},
    #[route("/organizations/new")]
    OrganizationForm {},
    #[route("/organizations/:id")]
    OrganizationDetail { id: String },
    #[route("/profile")]
    Profile {},
    #[route("/users")]
    UserList {},
    #[route("/users/new")]
    UserForm {},
    #[route("/users/:id")]
    UserDetail { id: String },
    #[route("/skill-centers")]
    SkillCenterList {},
    #[route("/skill-centers/new")]
    SkillCenterForm {},
    #[route("/skill-centers/:id")]
    SkillCenterDetail { id: String },
    #[route("/import-sources?:prefill_slug&:prefill_name")]
    ImportSources { prefill_slug: Option<String>, prefill_name: Option<String> },
    #[route("/import-sources/search")]
    ImportSourcesSearch {},
    #[route("/import-sources/:id")]
    ImportSourceDetail { id: String },
    #[route("/docs")]
    DocList {},
    #[route("/docs/:slug")]
    DocPage { slug: String },
}

const WASM_LOADING_INNER: &str = r#"<style>@media(prefers-color-scheme:dark){#wasm-loading{background:#1a1f2e!important;color:#7b9fe0!important;border-bottom-color:#2a3040!important}}#wasm-loading svg{animation:wasm-spin 1s linear infinite;width:16px;height:16px}@keyframes wasm-spin{from{transform:rotate(0deg)}to{transform:rotate(360deg)}}</style><svg viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" opacity="0.25"/><path d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" fill="currentColor" opacity="0.75"/></svg>Loading&hellip;"#;

const THEME_INIT_SCRIPT: &str = r#"
(function(){
    try {
        var d = document.documentElement;
        var t = localStorage.getItem('theme');
        var dark = t === 'dark' || (!t && window.matchMedia('(prefers-color-scheme: dark)').matches);
        if (dark) {
            d.classList.add('dark');
            d.style.colorScheme = 'dark';
            d.style.backgroundColor = '#111827';
        } else {
            d.classList.remove('dark');
            d.style.colorScheme = 'light';
            d.style.backgroundColor = '#f9fafb';
        }
    } catch(e){}
})();
"#;

#[component]
pub fn App() -> Element {
    let mut i18n = use_init_i18n(|| {
        I18nConfig::new(langid!("en-US"))
            .with_locale(Locale::new_static(
                langid!("en-US"),
                include_str!("./en-US.ftl"),
            ))
            .with_locale(Locale::new_static(
                langid!("de-DE"),
                include_str!("./de-DE.ftl"),
            ))
    });

    // Restore language preference from localStorage on first load.
    use_effect(move || {
        spawn(async move {
            let result = document::eval(
                "try { return localStorage.getItem('lang') || ''; } catch(e) { return ''; }",
            )
            .await;
            if let Ok(val) = result {
                if let Some(lang) = val.as_str() {
                    if lang == "de-DE" {
                        let _ = i18n.set_language(langid!("de-DE"));
                    }
                }
            }
        });
    });

    let css_href = format!("/tailwind.css?v={}", env!("BUILD_TIMESTAMP"));
    // Remove the pre-hydration loading banner once WASM has hydrated.
    use_effect(|| {
        document::eval("document.getElementById('wasm-loading')?.remove();");
    });

    rsx! {
        // Script FIRST: sets .dark class + inline bg before CSS even loads
        script { dangerous_inner_html: THEME_INIT_SCRIPT }
        document::Link { rel: "stylesheet", href: "{css_href}" }

        // SSR-rendered loading banner — visible until WASM hydrates, then removed
        // by use_effect above.
        div { id: "wasm-loading",
            style: "position:fixed;top:0;left:0;right:0;display:flex;align-items:center;justify-content:center;gap:8px;padding:10px;background:#f0f4ff;color:#3b5998;font-family:system-ui,-apple-system,sans-serif;font-size:13px;z-index:9999;border-bottom:1px solid #d0d8e8",
            dangerous_inner_html: WASM_LOADING_INNER,
        }

        Router::<Route> {}
    }
}
