//! `plan-ai-html` — minimal mustache-rendered HTML pages styled with the
//! plan-ai-design system.
//!
//! For standalone server-rendered pages that live **outside** a Dioxus SPA
//! (the basic-auth gate, OIDC login, the relay login page). Pages are
//! **self-contained**: the core plan-ai-design tokens and a handful of semantic
//! classes (`.card`, `.card-pad`, `.btn`/`.btn-primary`/`.btn-secondary`/
//! `.btn-danger`, `.input`, `.label`, `.h-page`, `.err`, `.help`, `.link`) are
//! inlined, so they render correctly on any service without needing a compiled
//! `tailwind.css` to be served. Light/dark follows the OS via a tiny script.
//!
//! Callers compose a page body — with a mustache fragment (see [`render`]) or
//! the small [`components`] helpers — and wrap it in [`Page`].
//!
//! ```no_run
//! let body = plan_ai_html::components::heading("Sign in")
//!     + &plan_ai_html::components::error("Bad password");
//! let html = plan_ai_html::Page::new("Sign in", body).render();
//! ```

pub use mustache;

/// Core plan-ai-design tokens + the component classes these pages use. Values
/// mirror `plan-ai-design/assets/input.css` so standalone pages match the SPA.
const STYLE: &str = r#":root{--c-bg:240 237 228;--c-surface:255 255 255;--c-surface-3:230 226 215;--c-surface-strong:215 209 195;--c-fg:22 26 34;--c-fg-strong:11 15 21;--c-fg-muted:104 106 110;--c-fg-faint:150 150 148;--c-fg-invert:255 255 255;--c-line:224 220 209;--c-brand:234 88 12;--c-brand-strong:194 65 12;--c-danger:220 38 38;--c-danger-strong:185 28 28;--font-sans:ui-sans-serif,-apple-system,BlinkMacSystemFont,"Inter Tight",Inter,system-ui,sans-serif}
.dark{--c-bg:11 15 21;--c-surface:28 39 53;--c-surface-3:36 49 66;--c-surface-strong:48 64 84;--c-fg:232 237 245;--c-fg-strong:255 255 255;--c-fg-muted:148 163 184;--c-fg-faint:100 116 139;--c-fg-invert:255 255 255;--c-line:42 52 66;--c-brand:249 115 22;--c-brand-strong:234 88 12;--c-danger:248 113 113;--c-danger-strong:239 68 68}
*{box-sizing:border-box}
body{background:rgb(var(--c-bg));color:rgb(var(--c-fg));min-height:100vh;margin:0;display:flex;align-items:center;justify-content:center;padding:1.5rem;font-family:var(--font-sans);line-height:1.5}
.card{background:rgb(var(--c-surface));border:1px solid rgb(var(--c-line));border-radius:12px}
.card-pad{padding:1.5rem}
.h-page{font-size:1.5rem;font-weight:700;margin:0 0 1rem;color:rgb(var(--c-fg-strong))}
.label{display:block;font-weight:500;font-size:.875rem;margin-bottom:.25rem;color:rgb(var(--c-fg-strong))}
.input{width:100%;border:1px solid rgb(var(--c-line));border-radius:6px;background:rgb(var(--c-surface));color:rgb(var(--c-fg));padding:.5rem .75rem;font:inherit}
.input::placeholder{color:rgb(var(--c-fg-faint))}
.input:focus{outline:2px solid rgb(var(--c-brand));outline-offset:0;border-color:rgb(var(--c-brand))}
.btn{display:inline-flex;align-items:center;justify-content:center;gap:.4rem;border-radius:6px;font-weight:500;text-decoration:none;cursor:pointer;border:0;padding:.375rem .75rem;font:inherit;transition:background .15s}
.btn-sm{padding:.25rem .75rem;font-size:.875rem}
.btn-lg{padding:.5rem 1rem}
.btn-primary{background:rgb(var(--c-brand));color:rgb(var(--c-fg-invert))}
.btn-primary:hover{background:rgb(var(--c-brand-strong))}
.btn-secondary{background:rgb(var(--c-surface-3));color:rgb(var(--c-fg))}
.btn-secondary:hover{background:rgb(var(--c-surface-strong))}
.btn-danger{background:rgb(var(--c-danger));color:rgb(var(--c-fg-invert))}
.btn-danger:hover{background:rgb(var(--c-danger-strong))}
.err{color:rgb(var(--c-danger));font-size:.875rem}
.help{color:rgb(var(--c-fg-muted));font-size:.875rem}
.link{color:rgb(var(--c-brand));text-decoration:none}
.link:hover{text-decoration:underline}"#;

const LAYOUT: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{{title}}</title>
<script>(function(){try{var d=document.documentElement;var t=localStorage.getItem('theme');var dark=t==='dark'||(!t&&window.matchMedia('(prefers-color-scheme: dark)').matches);d.classList.toggle('dark',dark);d.style.colorScheme=dark?'dark':'light';}catch(e){}})();</script>
<style>{{{style}}}</style>
</head>
<body>
<main class="card card-pad" style="width:100%;max-width:{{max_width}}">{{{body}}}</main>
</body>
</html>"#;

/// A standalone page: the plan-ai-design chrome (inlined CSS, theme script,
/// centered card) wrapped around a pre-rendered, trusted HTML `body`.
pub struct Page<'a> {
    title: &'a str,
    body: String,
    max_width: &'a str,
}

impl<'a> Page<'a> {
    /// `body` must be trusted HTML (already escaped where it interpolates
    /// untrusted values — `render`/`components` do this for you).
    pub fn new(title: &'a str, body: impl Into<String>) -> Self {
        Self {
            title,
            body: body.into(),
            max_width: "24rem",
        }
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
            .insert_str("max_width", self.max_width)
            .insert_str("style", STYLE)
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
        assert!(html.contains("<style>")); // self-contained CSS
        assert!(html.contains(".btn-primary")); // design classes inlined
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
