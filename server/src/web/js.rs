//! Helpers for building JavaScript source handed to `document::eval`.

/// Encode `s` as a JS string literal (a JSON string: quotes, backslashes and
/// newlines escaped).
///
/// Values interpolated into `document::eval` source — relay proxy URLs, tunnel
/// and service names — come from daemon heartbeats and are stored unvalidated,
/// so a value containing `"}` would break out of a hand-written `"…"` literal
/// and run as script in every operator's browser.
pub fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use super::js_str;

    /// The literal must parse back to the input — every quote, backslash and
    /// newline escaped — and stay on one line.
    fn assert_literal(input: &str) {
        let literal = js_str(input);
        assert_eq!(serde_json::from_str::<String>(&literal).unwrap(), input);
        assert!(!literal.contains('\n') && !literal.contains('\r'));
    }

    #[test]
    fn escapes_quote_brace_breakout() {
        assert_literal(r##""}); alert(1); (""##);
        assert_literal(r#"/api/logs"}"#);
    }

    #[test]
    fn escapes_backslash_and_newline() {
        assert_literal("a\\b\nc\r\td");
    }

    #[test]
    fn plain_url_is_unchanged_apart_from_quotes() {
        assert_eq!(
            js_str("https://abc.relay.example/api/logs"),
            "\"https://abc.relay.example/api/logs\""
        );
    }
}
