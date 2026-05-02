// plan-ai-design — shared visual primitives.
//
// Components in this module are the single source of truth for the
// app's design language. They have no app-specific dependencies (no
// `Route` enum, no server-fn calls) so any Dioxus app can drop the
// crate in and pick up the same look-and-feel.
//
// Route-coupled wrappers (Breadcrumbs, KpiCard, etc.) live in the
// consuming app — see `server/src/web/components/ui/` for examples.
//
// CSS lives at `plan-ai-design/assets/input.css`. Tailwind builds
// against it (the consuming app points its tailwind config at the
// crate's `assets/input.css` and adds `../plan-ai-design/src/**/*.rs`
// to its `content` paths so class names get extracted).

mod alert;
mod badge;
mod button;
mod card;
mod chart;
mod data_table;
mod feed;
mod form;
mod hero;
mod metric;
mod pill;
mod session;
mod text;
mod timeline;

pub use alert::*;
pub use badge::*;
pub use button::*;
pub use card::*;
pub use chart::*;
pub use data_table::*;
pub use feed::*;
pub use form::*;
pub use hero::*;
pub use metric::*;
pub use pill::*;
pub use session::*;
pub use text::*;
pub use timeline::*;
