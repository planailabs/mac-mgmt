use dioxus::prelude::*;

use super::components::bundle_detail::BundleDetail;
use super::components::bundle_form::BundleForm;
use super::components::bundle_list::BundleList;
use super::components::customer_detail::CustomerDetail;
use super::components::customer_form::CustomerForm;
use super::components::customer_list::CustomerList;
use super::components::fleet_dashboard::FleetDashboard;
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

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    CustomerList {},
    #[route("/customers/new")]
    CustomerForm {},
    #[route("/customers/:id")]
    CustomerDetail { id: String },
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
    #[route("/fleet")]
    FleetDashboard {},
    #[route("/rollout-groups")]
    RolloutGroupList {},
    #[route("/rollout-groups/:id")]
    RolloutGroupDetail { id: String },
    #[route("/daemon-versions")]
    DaemonVersionList {},
    #[route("/daemon-versions/:version")]
    DaemonVersionDetail { version: String },
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: "/tailwind.css" }
        Router::<Route> {}
    }
}
