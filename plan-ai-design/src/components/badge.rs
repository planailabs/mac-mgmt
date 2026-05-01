use dioxus::prelude::*;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum BadgeVariant {
    #[default]
    Neutral,
    Info,
    Warn,
    Success,
    Danger,
    Accent,
}

impl BadgeVariant {
    pub fn class(self) -> &'static str {
        match self {
            Self::Neutral => "pill-muted",
            Self::Info => "pill-info",
            Self::Warn => "pill-warn",
            Self::Success => "pill-ok",
            Self::Danger => "pill-bad",
            Self::Accent => "pill-accent",
        }
    }
}

#[component]
pub fn Badge(
    variant: BadgeVariant,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    let variant_cls = variant.class();
    rsx! {
        span { class: "pill {variant_cls} {class}", {children} }
    }
}
