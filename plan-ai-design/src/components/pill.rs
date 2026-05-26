use dioxus::prelude::*;

/// Status color shared between `Pill` and `Dot`. Names mirror the
/// design language: ok/warn/bad/info/accent are status semantics,
/// `Muted` is the chrome variant (used for environment tags, version
/// numbers, anything where the pill is decorative rather than load-
/// bearing).
#[derive(Clone, Copy, PartialEq, Default)]
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
    fn pill_class(self) -> &'static str {
        match self {
            Self::Muted => "pill-muted",
            Self::Ok => "pill-ok",
            Self::Warn => "pill-warn",
            Self::Bad => "pill-bad",
            Self::Info => "pill-info",
            Self::Accent => "pill-accent",
        }
    }

    fn dot_class(self) -> &'static str {
        match self {
            Self::Muted => "dot-muted",
            Self::Ok => "dot-ok",
            Self::Warn => "dot-warn",
            Self::Bad => "dot-bad",
            Self::Info => "dot-info",
            Self::Accent => "dot-accent",
        }
    }
}

/// Status pill — rounded-full chip with one of six color variants
/// (principle 8: pills, not badges). Pass `mono = true` for identifier
/// pills (versions, hashes, model names) — those tighten typography
/// and switch to mono per principle 4.
///
/// Composing a pill with a leading dot:
/// ```ignore
/// Pill { variant: PillVariant::Ok,
///     Dot { variant: PillVariant::Ok }
///     "online"
/// }
/// ```
#[component]
pub fn Pill(
    #[props(default)] variant: PillVariant,
    #[props(default)] mono: bool,
    #[props(default, into)] title: Option<String>,
    #[props(default, into)] class: String,
    /// Custom color (CSS value). Overrides variant styling with inline color + tinted background.
    #[props(default, into)]
    color: Option<String>,
    children: Element,
) -> Element {
    let mono_cls = if mono { "pill-mono" } else { "" };
    if let Some(ref c) = color {
        let cls = format!("pill {} {}", mono_cls, class);
        rsx! {
            span {
                class: "{cls}",
                style: "background: {c}20; color: {c}; border-color: {c}40;",
                title,
                {children}
            }
        }
    } else {
        let cls = format!("pill {} {} {}", variant.pill_class(), mono_cls, class);
        rsx! {
            span { class: "{cls}", title,
                {children}
            }
        }
    }
}

/// Status dot — 6×6 colored disc with a 3px soft ring, used inside
/// pills and as leading bullets in activity feeds. Visual is owned by
/// `.dot-*` semantic classes.
#[component]
pub fn Dot(
    #[props(default)] variant: PillVariant,
    #[props(default, into)] class: String,
) -> Element {
    let cls = format!("dot {} {}", variant.dot_class(), class);
    rsx! {
        span { class: "{cls}" }
    }
}
