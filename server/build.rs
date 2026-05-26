use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    build_tailwind();

    // Guard: views in src/web/ must use the semantic tokens defined in
    // input.css (text-fg-muted, btn-primary, badge-info, …) rather than
    // raw tailwind palette literals. The contract is that re-theming is
    // a CSS-only change; an inline `bg-blue-600` defeats that.
    check_no_tailwind_palette_literals(Path::new("src/web"));
}

/// Build tailwind.css from input.css, or fail if tailwindcss is unavailable.
fn build_tailwind() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest = Path::new(&manifest_dir);

    let status = Command::new("npm")
        .args(["run", "tailwind:build"])
        .current_dir(manifest)
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => panic!("npm run tailwind:build exited with {s}"),
        Err(e) => panic!("failed to run npm: {e}"),
    }
}

/// Tailwind palette names that should never appear in views. Any `bg-`,
/// `text-`, `border-`, `hover:*`, or `dark:*` utility paired with one of
/// these names + a numeric shade is rejected.
const PALETTE_NAMES: &[&str] = &[
    "blue", "red", "gray", "green", "yellow", "purple", "orange", "amber", "indigo", "emerald",
    "slate", "zinc", "neutral", "stone", "sky", "cyan", "teal", "lime", "rose", "pink", "fuchsia",
    "violet",
];

/// Class prefixes we consider visual. `hover:bg-`, `dark:hover:bg-`, etc.
/// are all anchored on these stems.
const VISUAL_PREFIXES: &[&str] = &[
    "bg-",
    "text-",
    "border-",
    "divide-",
    "ring-",
    "from-",
    "to-",
    "via-",
    "fill-",
    "stroke-",
    "placeholder-",
    "caret-",
    "outline-",
    "shadow-",
    "decoration-",
    "accent-",
];

fn check_no_tailwind_palette_literals(root: &Path) {
    let mut offenders: Vec<String> = Vec::new();
    walk(root, &mut |path| {
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            return;
        }
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        for (lineno, line) in text.lines().enumerate() {
            if let Some(hit) = find_palette_literal(line) {
                offenders.push(format!("  {}:{}: {}", path.display(), lineno + 1, hit,));
            }
        }
    });

    if !offenders.is_empty() {
        let mut msg = String::from(
            "tailwind palette literals found in src/web/. \
             Use semantic tokens from input.css instead \
             (e.g. `text-fg-muted`, `btn-primary`, `badge-info`):\n",
        );
        for line in &offenders {
            msg.push_str(line);
            msg.push('\n');
        }
        panic!("{msg}");
    }
}

/// Returns the first offending substring on this line, or None.
fn find_palette_literal(line: &str) -> Option<String> {
    // Cheap pre-filter: a literal must contain a hyphen and a digit.
    if !line.contains('-') {
        return None;
    }

    for name in PALETTE_NAMES {
        // Locate every occurrence of "{name}-" in the line, then verify
        // it sits inside a visual utility and has a numeric shade.
        let mut search_start = 0;
        while let Some(rel) = line[search_start..].find(name) {
            let pos = search_start + rel;
            search_start = pos + name.len();

            // Must be followed by `-` then a digit.
            let after_name = &line[pos + name.len()..];
            let mut chars = after_name.chars();
            if chars.next() != Some('-') {
                continue;
            }
            let next = chars.next();
            if !next.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                continue;
            }

            // Walk back to the start of this class token. Class tokens
            // are separated by whitespace, quotes, or parens.
            let before = &line[..pos];
            let stem_start = before
                .rfind(|c: char| c == ' ' || c == '"' || c == '(' || c == '\t' || c == '\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let stem = &line[stem_start..pos];

            // Must be preceded by one of our visual prefixes after
            // stripping variants like `dark:`, `hover:`, `xl:hover:`,
            // and the `!` important marker.
            let bare = stem.trim_start_matches('!');
            let bare = bare.rsplit(':').next().unwrap_or(bare);
            if !VISUAL_PREFIXES.iter().any(|p| bare == *p) {
                continue;
            }

            // Capture up to the next whitespace/quote for the report.
            let end = line[pos..]
                .find(|c: char| c == ' ' || c == '"' || c == '\t')
                .map(|e| pos + e)
                .unwrap_or(line.len());
            return Some(line[stem_start..end].to_string());
        }
    }
    None
}

fn walk(dir: &Path, cb: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, cb);
        } else {
            cb(&path);
        }
    }
}
