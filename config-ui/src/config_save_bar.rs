//! Floating save bar + review-diff modal.
//!
//! Mounted by `ConfigEditor` whenever the editor diverges from the
//! server snapshot. Owns nothing of its own beyond the diff-modal
//! visibility flag — every signal it touches (`editor_text`,
//! `saved_text`, `EditorStats`) is owned by the editor and threaded in
//! as props.
//!
//! Extracted from `config_editor.rs` to keep that file focused on
//! schema → form rendering. The diff helper at the bottom is naïve
//! (line-aligned with a small look-ahead window) — sufficient for
//! `to_string_pretty`'d JSON; if we ever need word-level diffs the
//! `similar` crate is already a transitive dep.
use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::config_editor::EditorStats;

#[component]
pub fn SaveBar(
    last_saved: Option<String>,
    editor_text: Signal<String>,
    saved_text: Signal<String>,
    on_save: EventHandler<()>,
    on_discard: EventHandler<MouseEvent>,
    stats: EditorStats,
) -> Element {
    let fields = *stats.modified_fields.read();
    let sections = *stats.modified_sections.read();
    let mut diff_open = use_signal(|| false);
    let summary = if fields == 0 {
        t!("config-save-unsaved")
    } else {
        t!("config-save-summary", fields: fields, sections: sections)
    };
    rsx! {
        div { class: "config-save-bar",
            // Pulsing dot — draws the eye when changes are unsaved so
            // the user doesn't think edits are auto-committed. The dot
            // is purely decorative; the bar itself only renders when
            // dirty=true so its presence already implies "draft state".
            span { class: "dot dot-accent animate-pulse" }
            div { class: "min-w-0",
                div { class: "flex items-center gap-2",
                    span { class: "kicker text-brand", {t!("config-save-draft")} }
                }
                div { class: "text-sm font-semibold text-fg-strong", "{summary}" }
                if let Some(t) = last_saved {
                    div { class: "text-[11px] text-fg-faint mt-0.5",
                        {t!("config-editor-last-saved", time: t)}
                    }
                }
            }
            div { class: "h-8 w-px bg-line mx-1" }
            button {
                class: "btn btn-md btn-ghost",
                r#type: "button",
                onclick: move |_| diff_open.set(true),
                {t!("config-save-review-diff")}
            }
            button {
                class: "btn btn-md btn-secondary",
                r#type: "button",
                onclick: on_discard,
                {t!("config-save-discard")}
            }
            button {
                class: "btn btn-md btn-primary",
                r#type: "button",
                onclick: move |_| on_save.call(()),
                {t!("config-editor-save")}
            }
        }
        if *diff_open.read() {
            ReviewDiffModal {
                editor_text,
                saved_text,
                open: diff_open,
            }
        }
    }
}

/// Modal showing the JSON diff between the last saved snapshot
/// (`saved_text`) and the in-flight editor state (`editor_text`).
#[component]
fn ReviewDiffModal(
    editor_text: Signal<String>,
    saved_text: Signal<String>,
    open: Signal<bool>,
) -> Element {
    let saved = saved_text.read().clone();
    let draft = editor_text.read().clone();
    let saved_lines: Vec<&str> = saved.lines().collect();
    let draft_lines: Vec<&str> = draft.lines().collect();
    let diff = unified_line_diff(&saved_lines, &draft_lines);

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center p-6 bg-black/60 backdrop-blur-sm",
            onclick: move |_| open.set(false),
            div {
                class: "bg-surface border border-line rounded-xl shadow-pop max-w-4xl w-full max-h-[80vh] flex flex-col overflow-hidden",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "flex items-center justify-between px-4 py-3 border-b border-line",
                    div {
                        div { class: "text-sm font-semibold text-fg-strong", {t!("config-save-review-diff-title")} }
                        div { class: "text-xs text-fg-muted", {t!("config-save-review-diff-help")} }
                    }
                    button {
                        class: "btn btn-sm btn-ghost",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "✕"
                    }
                }
                div { class: "flex-1 overflow-auto font-mono text-xs leading-snug",
                    if diff.is_empty() {
                        div { class: "p-6 text-center text-fg-muted",
                            {t!("config-save-review-diff-empty")}
                        }
                    } else {
                        pre { class: "p-3 whitespace-pre-wrap break-all",
                            for (kind, line) in diff.iter() {
                                {
                                    let cls = match kind {
                                        DiffKind::Add => "block bg-success/10 text-success-strong px-2",
                                        DiffKind::Remove => "block bg-danger/10 text-danger-strong px-2",
                                        DiffKind::Context => "block text-fg-muted px-2",
                                    };
                                    let prefix = match kind {
                                        DiffKind::Add => "+ ",
                                        DiffKind::Remove => "- ",
                                        DiffKind::Context => "  ",
                                    };
                                    rsx! { span { class: cls, "{prefix}{line}" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum DiffKind {
    Add,
    Remove,
    Context,
}

/// Naïve unified diff: walks both line lists in parallel, emitting
/// removed-then-added blocks where they diverge. Good enough for
/// human-readable JSON config diffs (line-aligned, modest size). Not
/// LCS — but the editor's JSON is `to_string_pretty`'d so untouched
/// regions stay byte-identical, which keeps the diff readable.
fn unified_line_diff(saved: &[&str], draft: &[&str]) -> Vec<(DiffKind, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < saved.len() || j < draft.len() {
        match (saved.get(i), draft.get(j)) {
            (Some(a), Some(b)) if a == b => {
                out.push((DiffKind::Context, (*a).to_string()));
                i += 1;
                j += 1;
            }
            (Some(a), Some(b)) => {
                // Look ahead a small window to find the closest re-sync.
                let mut found = None;
                let win = 8usize;
                for k in 1..=win {
                    if let Some(future) = draft.get(j + k) {
                        if future == a {
                            found = Some(("draft_ahead", k));
                            break;
                        }
                    }
                    if let Some(future) = saved.get(i + k) {
                        if future == b {
                            found = Some(("saved_ahead", k));
                            break;
                        }
                    }
                }
                match found {
                    Some(("draft_ahead", k)) => {
                        for n in 0..k {
                            out.push((DiffKind::Add, draft[j + n].to_string()));
                        }
                        j += k;
                    }
                    Some(("saved_ahead", k)) => {
                        for n in 0..k {
                            out.push((DiffKind::Remove, saved[i + n].to_string()));
                        }
                        i += k;
                    }
                    _ => {
                        out.push((DiffKind::Remove, (*a).to_string()));
                        out.push((DiffKind::Add, (*b).to_string()));
                        i += 1;
                        j += 1;
                    }
                }
            }
            (Some(a), None) => {
                out.push((DiffKind::Remove, (*a).to_string()));
                i += 1;
            }
            (None, Some(b)) => {
                out.push((DiffKind::Add, (*b).to_string()));
                j += 1;
            }
            (None, None) => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_produce_empty_diff() {
        assert!(unified_line_diff(&[], &[]).is_empty());
    }

    #[test]
    fn identical_inputs_emit_only_context_lines() {
        let saved = ["a", "b", "c"];
        let draft = ["a", "b", "c"];
        let diff = unified_line_diff(&saved, &draft);
        assert_eq!(diff.len(), 3);
        assert!(diff.iter().all(|(k, _)| matches!(k, DiffKind::Context)));
    }

    #[test]
    fn single_line_change_emits_remove_plus_add() {
        let saved = ["a", "b", "c"];
        let draft = ["a", "B", "c"];
        let diff = unified_line_diff(&saved, &draft);
        let kinds: Vec<DiffKind> = diff.iter().map(|(k, _)| *k).collect();
        assert_eq!(
            kinds,
            vec![
                DiffKind::Context,
                DiffKind::Remove,
                DiffKind::Add,
                DiffKind::Context,
            ]
        );
    }

    #[test]
    fn pure_addition_renders_as_add() {
        let saved = ["a", "c"];
        let draft = ["a", "b", "c"];
        let diff = unified_line_diff(&saved, &draft);
        let added: Vec<&String> = diff
            .iter()
            .filter(|(k, _)| matches!(k, DiffKind::Add))
            .map(|(_, s)| s)
            .collect();
        assert_eq!(added, vec!["b"]);
    }
}
