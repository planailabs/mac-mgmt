use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, unic_langid::langid};

use super::components::billing_log::BillingLog;
use super::components::contact_form::ContactForm;
use super::components::contact_list::ContactList;
use super::components::credential_form::CredentialForm;
use super::components::credential_list::CredentialList;
use super::components::dashboard::Dashboard;
use super::components::domain_add::DomainAdd;
use super::components::domain_detail::DomainDetail;
use super::components::domain_list::DomainList;
use super::components::layout::Layout;
use super::components::organization_form::OrganizationForm;
use super::components::organization_list::OrganizationList;
use super::components::token_list::TokenList;
use super::components::user_list::UserList;
use super::components::webspace_form::WebspaceForm;
use super::components::webspace_list::WebspaceList;

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    Dashboard {},
    #[route("/credentials")]
    CredentialList {},
    #[route("/credentials/new")]
    CredentialForm {},
    #[route("/domains")]
    DomainList {},
    #[route("/domains/add")]
    DomainAdd {},
    #[route("/domains/:id")]
    DomainDetail { id: String },
    #[route("/webspaces")]
    WebspaceList {},
    #[route("/webspaces/new")]
    WebspaceForm {},
    #[route("/contacts")]
    ContactList {},
    #[route("/contacts/new")]
    ContactForm {},
    #[route("/billing")]
    BillingLog {},
    #[route("/organizations")]
    OrganizationList {},
    #[route("/organizations/new")]
    OrganizationForm {},
    #[route("/users")]
    UserList {},
    #[route("/tokens")]
    TokenList {},
}

#[component]
pub fn App() -> Element {
    use_init_i18n(|| {
        I18nConfig::new(langid!("en-US"))
            .with_locale(Locale::new_static(
                langid!("en-US"),
                plan_ai_design::i18n::EN_US,
            ))
            .with_locale(Locale::new_static(
                langid!("de-DE"),
                plan_ai_design::i18n::DE_DE,
            ))
    });

    rsx! {
        Router::<Route> {}
    }
}
