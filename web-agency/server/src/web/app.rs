use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, unic_langid::langid};

use super::components::basic_auth_detail::BasicAuthDetail;
use super::components::basic_auth_form::BasicAuthForm;
use super::components::basic_auth_list::BasicAuthList;
use super::components::billing_log::BillingLog;
use super::components::certificate_list::CertificateList;
use super::components::contact_form::ContactForm;
use super::components::contact_list::ContactList;
use super::components::credential_edit::CredentialEdit;
use super::components::credential_form::CredentialForm;
use super::components::credential_list::CredentialList;
use super::components::dashboard::Dashboard;
use super::components::docs::{DocList, DocPage};
use super::components::domain_add::DomainAdd;
use super::components::domain_detail::DomainDetail;
use super::components::domain_import::DomainImport;
use super::components::domain_list::DomainList;
use super::components::domain_register::DomainRegister;
use super::components::layout::Layout;
use super::components::organization_detail::OrganizationDetail;
use super::components::organization_form::OrganizationForm;
use super::components::organization_list::OrganizationList;
use super::components::token_list::TokenList;
use super::components::user_detail::UserDetail;
use super::components::user_form::UserForm;
use super::components::user_list::UserList;
use super::components::webspace_changedetection::{
    WebspaceChangedetection, WebspaceChangedetectionNotification, WebspaceChangedetectionSuburl,
};
use super::components::webspace_detail::WebspaceDetail;
use super::components::webspace_form::WebspaceForm;
use super::components::webspace_host_detail::WebspaceHostDetail;
use super::components::webspace_host_form::WebspaceHostForm;
use super::components::webspace_host_list::WebspaceHostList;
use super::components::webspace_import::WebspaceImport;
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
    #[route("/credentials/:id/edit")]
    CredentialEdit { id: String },
    #[route("/domains")]
    DomainList {},
    #[route("/domains/add")]
    DomainAdd {},
    #[route("/domains/import")]
    DomainImport {},
    #[route("/domains/register")]
    DomainRegister {},
    #[route("/domains/:id")]
    DomainDetail { id: String },
    #[route("/webspace-hosts")]
    WebspaceHostList {},
    #[route("/webspace-hosts/new")]
    WebspaceHostForm {},
    #[route("/webspace-hosts/:id")]
    WebspaceHostDetail { id: String },
    #[route("/webspace-hosts/:host_id/folders/new")]
    WebspaceForm { host_id: String },
    #[route("/webspace-hosts/:id/changedetection")]
    WebspaceChangedetection { id: String },
    #[route("/webspace-hosts/:id/changedetection/suburls/:suburl_id")]
    WebspaceChangedetectionSuburl { id: String, suburl_id: String },
    #[route("/webspace-hosts/:id/changedetection/:notification_id")]
    WebspaceChangedetectionNotification { id: String, notification_id: String },
    #[route("/webspaces")]
    WebspaceList {},
    #[route("/webspaces/import")]
    WebspaceImport {},
    #[route("/webspaces/:id")]
    WebspaceDetail { id: String },
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
    #[route("/organizations/:id")]
    OrganizationDetail { id: String },
    #[route("/users")]
    UserList {},
    #[route("/users/new")]
    UserForm {},
    #[route("/users/:id")]
    UserDetail { id: String },
    #[route("/tokens")]
    TokenList {},
    #[route("/basic-auth")]
    BasicAuthList {},
    #[route("/basic-auth/new")]
    BasicAuthForm {},
    #[route("/basic-auth/:id")]
    BasicAuthDetail { id: String },
    #[route("/certificates")]
    CertificateList {},
    #[route("/docs")]
    DocList {},
    #[route("/docs/:slug")]
    DocPage { slug: String },
}

#[component]
pub fn App() -> Element {
    let mut i18n = use_init_i18n(|| {
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

    use_effect(|| {
        document::eval("document.getElementById('wasm-loading')?.remove();");
    });

    rsx! {
        script { dangerous_inner_html: plan_ai_design::THEME_INIT_SCRIPT }
        document::Link { rel: "stylesheet", href: "{css_href}" }

        div { id: "wasm-loading",
            style: plan_ai_design::WASM_LOADING_STYLE,
            dangerous_inner_html: plan_ai_design::WASM_LOADING_INNER,
        }

        Router::<Route> {}
    }
}
