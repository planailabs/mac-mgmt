//! Shared config editor used by BOTH mac-mgmt-server and the sovereign-AI USB
//! overview app — the whole editor UI (schema-driven form, schema defaults,
//! section filter chips / hidden sections, review-diff, raw-JSON mode, save
//! bar, extra-config modal), not a clone.
//!
//! Extracted from the server's `web/components/config_editor.rs` and decoupled
//! from its Dioxus-fullstack `#[server]` data layer: the top-level
//! [`ConfigEditor`] takes the JSON Schema + current config + an `on_save`
//! handler as props, so it runs in a plain desktop app too. `cluster_id` is
//! kept as an opaque namespace string (used for per-section localStorage keys).
//!
//! i18n: the editor uses `dioxus_i18n::t!`. Consumers must initialise
//! `dioxus_i18n` with at least [`EN_US`] (the server already bundles a superset;
//! the overview includes [`EN_US`]).

mod config_editor;
mod config_filter_chips;
mod config_save_bar;
mod extra_config_modal;
mod model_select_modal;

pub use config_editor::ConfigEditor;

/// Config-editor Fluent translations (en-US). Consumers concatenate this with
/// their own FTL when initialising `dioxus_i18n` so the editor's `t!` keys
/// resolve. Missing keys fall back to the key name.
pub const EN_US: &str = include_str!("../i18n/en-US.ftl");
/// Config-editor Fluent translations (de-DE).
pub const DE_DE: &str = include_str!("../i18n/de-DE.ftl");

#[cfg(test)]
mod ftl_tests {
    use super::*;

    /// The bundled FTL must parse cleanly — a malformed entry (e.g. a truncated
    /// `{ $n -> … }` select) makes `dioxus_i18n` panic at runtime when the app
    /// builds its bundle, which is invisible at compile time.
    fn assert_parses(name: &str, src: &str) {
        if let Err((_ast, errors)) = fluent_syntax::parser::parse(src) {
            panic!(
                "{name} has {} Fluent parse error(s): {errors:?}",
                errors.len()
            );
        }
    }

    #[test]
    fn en_us_ftl_is_valid() {
        assert_parses("en-US.ftl", EN_US);
    }

    #[test]
    fn de_de_ftl_is_valid() {
        assert_parses("de-DE.ftl", DE_DE);
    }
}
