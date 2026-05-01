use dioxus::prelude::*;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum AlertVariant {
    #[default]
    Info,
    Success,
    Warn,
    Danger,
}

impl AlertVariant {
    pub fn class(self) -> &'static str {
        match self {
            Self::Info => "alert-info",
            Self::Success => "alert-ok",
            Self::Warn => "alert-warn",
            Self::Danger => "alert-bad",
        }
    }
}

#[component]
pub fn Alert(
    #[props(default)] variant: AlertVariant,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    let variant_cls = variant.class();
    rsx! {
        div { class: "alert {variant_cls} {class}", {children} }
    }
}
