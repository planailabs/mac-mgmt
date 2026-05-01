use dioxus::prelude::*;

#[derive(Default, Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Danger,
    Warn,
    Ghost,
    Accent,
}

impl ButtonVariant {
    pub fn class(self) -> &'static str {
        match self {
            Self::Primary => "btn-primary",
            Self::Secondary => "btn-secondary",
            Self::Danger => "btn-danger",
            Self::Warn => "btn-warn",
            Self::Ghost => "btn-ghost",
            Self::Accent => "btn-accent",
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum ButtonSize {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl ButtonSize {
    pub fn class(self) -> &'static str {
        match self {
            Self::Xs => "btn-xs",
            Self::Sm => "btn-sm",
            Self::Md => "btn-md",
            Self::Lg => "btn-lg",
        }
    }
}

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(default, into)] class: String,
    #[props(default)] disabled: bool,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    let cls = format!("btn {} {} {}", size.class(), variant.class(), class);
    rsx! {
        button {
            class: "{cls}",
            disabled,
            onclick: move |e| onclick.call(e),
            {children}
        }
    }
}
