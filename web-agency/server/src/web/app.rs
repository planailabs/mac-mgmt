use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, unic_langid::langid};

use super::components::dashboard::Dashboard;
use super::components::layout::Layout;

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[layout(Layout)]
    #[route("/")]
    Dashboard {},
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
