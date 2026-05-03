pub mod components;

pub use components::*;

/// Shared Fluent translations required by plan-ai-design components.
/// Consuming apps should concatenate these with their own FTL content
/// when initializing `dioxus_i18n`.
pub mod i18n {
    pub const EN_US: &str = include_str!("../i18n/en-US.ftl");
    pub const DE_DE: &str = include_str!("../i18n/de-DE.ftl");
}
