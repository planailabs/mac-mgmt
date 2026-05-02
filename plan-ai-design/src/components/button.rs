use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Default)]
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
    fn class(self) -> &'static str {
        match self {
            Self::Primary   => "btn-primary",
            Self::Secondary => "btn-secondary",
            Self::Danger    => "btn-danger",
            Self::Warn      => "btn-warn",
            Self::Ghost     => "btn-ghost",
            Self::Accent    => "btn-accent",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonSize {
    Xs,
    Sm,
    #[default]
    Md,
    Lg,
}

impl ButtonSize {
    fn class(self) -> &'static str {
        match self {
            Self::Xs => "btn-xs",
            Self::Sm => "btn-sm",
            Self::Md => "btn-md",
            Self::Lg => "btn-lg",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonKind {
    #[default]
    Button,
    Submit,
    Reset,
}

impl ButtonKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Submit => "submit",
            Self::Reset  => "reset",
        }
    }
}

/// Standard button. Visual is fully owned by the `.btn` semantic classes
/// in `input.css`; props describe intent and behavior.
///
/// `class` is for layout-only modifiers (`mt-2`, `w-full`, `self-start`).
/// Color and typography utilities should not appear there — use a different
/// `variant` instead.
#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] size: ButtonSize,
    #[props(default)] kind: ButtonKind,
    #[props(default)] disabled: bool,
    #[props(default, into)] class: String,
    #[props(default, into)] title: Option<String>,
    /// Optional click handler. Buttons of `kind: Submit` rely on the
    /// surrounding `<form onsubmit=...>` and don't need this.
    #[props(default)] onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    let cls = format!("btn {} {} {}", size.class(), variant.class(), class);
    rsx! {
        button {
            class: "{cls}",
            r#type: kind.as_str(),
            disabled,
            title,
            onclick: move |evt| onclick.call(evt),
            {children}
        }
    }
}
