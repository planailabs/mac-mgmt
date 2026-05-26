use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum AlertVariant {
    #[default]
    Info,
    Success,
    Warn,
    Danger,
}

impl AlertVariant {
    fn class(self) -> &'static str {
        match self {
            Self::Info => "alert-info",
            Self::Success => "alert-success",
            Self::Warn => "alert-warn",
            Self::Danger => "alert-danger",
        }
    }
}

/// Boxed status message — a soft-tinted background with matching border
/// and text color. Use `<ErrorText>` / `<SuccessText>` for inline single-
/// line messages; reach for `<Alert>` when the message has structure
/// (heading + body, or a value to copy).
#[component]
pub fn Alert(
    #[props(default)] variant: AlertVariant,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    let cls = format!("alert {} {}", variant.class(), class);
    rsx! {
        div { class: "{cls}",
            {children}
        }
    }
}
