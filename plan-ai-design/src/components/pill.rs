use dioxus::prelude::*;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum PillVariant {
    #[default]
    Muted,
    Ok,
    Warn,
    Bad,
    Info,
    Accent,
}

impl PillVariant {
    pub fn class(self) -> &'static str {
        match self {
            Self::Muted => "pill-muted",
            Self::Ok => "pill-ok",
            Self::Warn => "pill-warn",
            Self::Bad => "pill-bad",
            Self::Info => "pill-info",
            Self::Accent => "pill-accent",
        }
    }
}

#[component]
pub fn Pill(
    #[props(default)] variant: PillVariant,
    #[props(default)] mono: bool,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    let variant_cls = variant.class();
    let mono_cls = if mono { "pill-mono" } else { "" };
    rsx! {
        span { class: "pill {variant_cls} {mono_cls} {class}", {children} }
    }
}

#[component]
pub fn Dot(#[props(default)] variant: PillVariant) -> Element {
    let dot_cls = match variant {
        PillVariant::Muted => "dot-muted",
        PillVariant::Ok => "dot-ok",
        PillVariant::Warn => "dot-warn",
        PillVariant::Bad => "dot-bad",
        PillVariant::Info => "dot-info",
        PillVariant::Accent => "dot-accent",
    };
    rsx! { span { class: "dot {dot_cls}" } }
}
