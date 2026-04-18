use dioxus::prelude::*;

use super::components::bundle_detail::BundleDetail;
use super::components::bundle_form::BundleForm;
use super::components::bundle_list::BundleList;
use super::components::cluster_detail::ClusterDetail;
use super::components::cluster_form::ClusterForm;
use super::components::cluster_list::ClusterList;
use super::components::file_editor::FleetFiles;
use super::components::fleet_dashboard::FleetDashboard;
use super::components::fleet_detail::FleetDetail;
use super::components::healer_page::FleetHealer;
use super::components::log_viewer::FleetLogs;
use super::components::shell_commands::FleetShell;
use super::components::layout::Layout;
use super::components::mcp_bundle_detail::McpBundleDetail;
use super::components::mcp_bundle_form::McpBundleForm;
use super::components::mcp_bundle_list::McpBundleList;
use super::components::admin_tokens_page::AdminTokens;
use super::components::mcp_server_detail::{McpServerDetail, McpServerEdit, McpServerForm};
use super::components::mcp_server_list::McpServerList;
use super::components::rollout_detail::RolloutDetail;
use super::components::rollout_form::RolloutForm;
use super::components::rollout_group_detail::RolloutGroupDetail;
use super::components::rollout_group_list::RolloutGroupList;
use super::components::rollout_list::RolloutList;
use super::components::skill_detail::SkillDetail;
use super::components::skill_list::SkillList;
use super::components::daemon_version_list::DaemonVersionList;
use super::components::daemon_version_detail::DaemonVersionDetail;
use super::components::organization_detail::OrganizationDetail;
use super::components::organization_form::OrganizationForm;
use super::components::organization_list::OrganizationList;
use super::components::docs::{DocList, DocPage};
use super::components::profile::Profile;
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
    #[route("/docs")]
    DocList {},
    #[route("/docs/:slug")]
    DocPage { slug: String },
}

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
    let css_href = format!("/tailwind.css?v={}", env!("BUILD_TIMESTAMP"));
    rsx! {
        // Script FIRST: sets .dark class + inline bg before CSS even loads
        script { dangerous_inner_html: THEME_INIT_SCRIPT }
        document::Link { rel: "stylesheet", href: "{css_href}" }
        Router::<Route> {}
    }
}
