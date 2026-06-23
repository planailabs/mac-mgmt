//! `plan-ai-html` — minimal mustache-rendered HTML pages styled with the
//! plan-ai-design system.
//!
//! For standalone server-rendered pages that live **outside** a Dioxus SPA
//! (the basic-auth gate, OIDC login, the relay login page). It emits HTML that
//! links the app's compiled `tailwind.css` and reuses the design system's
//! semantic classes (`.card`, `.card-pad`, `.btn`, `.btn-primary`, `.input`,
//! `.label`, `.h-page`, `.err`, `.link`) plus its CSS variables (`--c-bg`,
//! `--c-fg`, `--font-sans`). The dark-mode preference script matches the SPA.
//!
//! Callers compose a page body — typically with a mustache fragment (see
//! [`render`]) or the small [`components`] helpers — and wrap it in [`Page`].
//!
//! ```no_run
//! let body = plan_ai_html::components::heading("Sign in")
//!     + &plan_ai_html::components::error("Bad password");
//! let html = plan_ai_html::Page::new("Sign in", body).render();
//! ```

pub use mustache;

const LAYOUT: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{{title}}</title>
<link rel="stylesheet" href="{{stylesheet}}">
<script>(function(){try{var d=document.documentElement;var t=localStorage.getItem('theme');var dark=t==='dark'||(!t&&window.matchMedia('(prefers-color-scheme: dark)').matches);d.classList.toggle('dark',dark);d.style.colorScheme=dark?'dark':'light';}catch(e){}})();</script>
</head>
<body style="background:rgb(var(--c-bg));color:rgb(var(--c-fg));min-height:100vh;margin:0;display:flex;align-items:center;justify-content:center;padding:1.5rem;font-family:var(--font-sans)">
<main class="card card-pad" style="width:100%;max-width:{{max_width}}">{{{body}}}</main>
</body>
</html>"#;

/// A standalone page: the plan-ai-design chrome (stylesheet, theme script,
/// centered card) wrapped around a pre-rendered, trusted HTML `body`.
pub struct Page<'a> {
    title: &'a str,
    body: String,
    stylesheet: &'a str,
    max_width: &'a str,
}

impl<'a> Page<'a> {
    /// `body` must be trusted HTML (already escaped where it interpolates
    /// untrusted values — `render`/`components` do this for you).
    pub fn new(title: &'a str, body: impl Into<String>) -> Self {
        Self {
            title,
            body: body.into(),
            stylesheet: "/tailwind.css",
            max_width: "24rem",
        }
    }

    /// Override the compiled stylesheet href (default `/tailwind.css`).
    pub fn stylesheet(mut self, href: &'a str) -> Self {
        self.stylesheet = href;
        self
    }

    /// Override the card max width as a CSS length (default `24rem`).
    pub fn max_width(mut self, w: &'a str) -> Self {
        self.max_width = w;
        self
    }

    /// Render the full HTML document.
    pub fn render(&self) -> String {
        let data = mustache::MapBuilder::new()
            .insert_str("title", self.title)
            .insert_str("stylesheet", self.stylesheet)
            .insert_str("max_width", self.max_width)
            .insert_str("body", &self.body)
            .build();
        render_data(LAYOUT, &data)
    }
}

/// Compile and render a mustache template against `data`. Returns an empty
/// string on a template error (so a malformed template never panics a request).
/// `{{var}}` auto-escapes; use `{{{var}}}` only for already-trusted HTML.
pub fn render_data(template: &str, data: &mustache::Data) -> String {
    let tpl = match mustache::compile_str(template) {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    let mut out = Vec::new();
    if tpl.render_data(&mut out, data).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Render a mustache template against simple string key/value pairs (all
/// auto-escaped). For conditionals/lists, build a [`mustache::Data`] and call
/// [`render_data`] directly.
pub fn render(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut mb = mustache::MapBuilder::new();
    for (k, v) in pairs {
        mb = mb.insert_str(*k, *v);
    }
    render_data(template, &mb.build())
}

/// HTML-escape a string for safe interpolation into trusted HTML you build by
/// hand. (Mustache `{{var}}` already does this; use this only outside a template.)
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Small HTML snippet helpers using plan-ai-design classes. Each returns an
/// HTML string; concatenate them to build a page body.
pub mod components {
    use super::escape;

    /// Page heading (`.h-page`).
    pub fn heading(text: &str) -> String {
        format!("<h1 class=\"h-page\">{}</h1>", escape(text))
    }

    /// Error/danger message (`.err`).
    pub fn error(msg: &str) -> String {
        format!("<div class=\"err\" style=\"margin-top:.6rem\">{}</div>", escape(msg))
    }

    /// Muted helper text (`.help`).
    pub fn muted(text: &str) -> String {
        format!("<p class=\"help\">{}</p>", escape(text))
    }

    /// A link-styled button (`.btn`). `variant` is a btn modifier suffix such
    /// as `primary`, `secondary`, `ghost`, `danger`.
    pub fn link_button(href: &str, label: &str, variant: &str) -> String {
        format!(
            "<a class=\"btn btn-{} btn-lg\" href=\"{}\">{}</a>",
            escape(variant),
            escape(href),
            escape(label),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_wraps_body_and_escapes_title() {
        let html = Page::new("Hi <there>", "<p>ok</p>").render();
        assert!(html.contains("<link rel=\"stylesheet\" href=\"/tailwind.css\">"));
        assert!(html.contains("<title>Hi &lt;there&gt;</title>")); // escaped
        assert!(html.contains("<p>ok</p>")); // body raw
        assert!(html.contains("class=\"card card-pad\""));
    }

    #[test]
    fn render_escapes_values() {
        let out = render("<b>{{x}}</b>", &[("x", "<script>")]);
        assert_eq!(out, "<b>&lt;script&gt;</b>");
    }

    #[test]
    fn components_escape() {
        assert_eq!(
            components::heading("a<b"),
            "<h1 class=\"h-page\">a&lt;b</h1>"
        );
    }
}
