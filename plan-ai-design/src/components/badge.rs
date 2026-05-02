use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Default)]
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
    fn class(self) -> &'static str {
        match self {
            Self::Neutral => "badge-neutral",
            Self::Info    => "badge-info",
            Self::Warn    => "badge-warn",
            Self::Success => "badge-success",
            Self::Danger  => "badge-danger",
            Self::Accent  => "badge-accent",
        }
    }
}

/// Small inline status pill. Visual is fully owned by `.badge` semantic
/// classes; props express which kind of status this is.
#[component]
pub fn Badge(
    #[props(default)] variant: BadgeVariant,
    #[props(default, into)] title: Option<String>,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    let cls = format!("badge {} {}", variant.class(), class);
    rsx! {
        span { class: "{cls}", title,
            {children}
        }
    }
}
