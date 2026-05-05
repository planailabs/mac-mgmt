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

// Sets `.dark` class on `<html>` before any CSS loads so the right theme is
// in place by the time the stylesheet arrives.
const THEME_INIT_SCRIPT: &str = r#"
(function(){
  var d=document.documentElement.classList,s=localStorage.getItem('theme');
  if(s==='dark'||(s!=='light'&&matchMedia('(prefers-color-scheme:dark)').matches)){
    d.add('dark');document.documentElement.style.backgroundColor='#0f172a';
  } else {
    d.remove('dark');document.documentElement.style.backgroundColor='#fff';
  }
})();
"#;

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

    let css_href = format!("/tailwind.css?v={}", env!("BUILD_TIMESTAMP"));

    rsx! {
        script { dangerous_inner_html: THEME_INIT_SCRIPT }
        document::Link { rel: "stylesheet", href: "{css_href}" }
        Router::<Route> {}
    }
}
