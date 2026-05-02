# plan-ai-design

Shared visual primitives and design tokens for plan.ai apps. **This crate
is the source of truth for the design language** — when adding or changing
a button, card, pill, etc., update this crate first; consumers (currently
`mac-mgmt-server`) re-export from here.

## Layout

- `src/components/` — Dioxus components (Alert, Badge, Button, Card,
  Sparkline / Bars, ActivityFeed, FormField, PageHero, PageHeader,
  Pill / Dot, ActiveSessionCard, StageTimeline, Kicker / Mono /
  StatBlock, DataTable / Td / Th / SortableTh / TableToolbar / Dash, …)
- `assets/input.css` — single Tailwind source of truth: tokens
  (`--c-bg`, `--c-fg-muted`, `--c-brand-*`, …) under `:root` and
  `.dark`, plus all `.btn`, `.card`, `.pill`, `.nav-*`, `.breadcrumbs`,
  `.topbar`, etc. semantic classes. Apps run their tailwindcss build
  against this file.

## What stays out

Anything that types the consuming app's domain (`Route`, server-fn
calls, app-specific state). Two precedents the server keeps locally:

- `Breadcrumbs` — walks a `Route` chain.
- `KpiCard` — accepts `to: Option<Route>` for drill-down.

A consuming app re-exports from this crate, then layers its own
Route-coupled wrappers on top. See
`server/src/web/components/ui/mod.rs` for the pattern.

## Consuming the crate

In `Cargo.toml` (under whatever feature you use to gate your web UI):

```toml
plan-ai-design = { path = "../plan-ai-design", optional = true }
```

In `tailwind.config.js` add the crate's source paths so Tailwind's
extractor picks up classes the components emit:

```js
content: ["./src/**/*.rs", "../plan-ai-design/src/**/*.rs"],
```

In `package.json` point Tailwind at the crate's `input.css`:

```json
"tailwind:build": "tailwindcss -i ../plan-ai-design/assets/input.css -o public/tailwind.css --minify"
```

## Source-of-truth contract

If you find yourself editing a `.rs` or class definition under
`server/src/web/components/ui/` for anything other than `Breadcrumbs`
or `KpiCard`, you're working in the wrong place — make the change
here and have the server re-export. Same for `input.css`: edit the
copy under `plan-ai-design/assets/`, never one elsewhere.
