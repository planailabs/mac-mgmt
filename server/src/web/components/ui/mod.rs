//! Shared UI components.
//!
//! Visual primitives are owned by the `plan-ai-design` crate — this
//! module re-exports them so existing callsites keep working with
//! `use crate::web::components::ui::Button` etc. The CSS source of
//! truth (`assets/input.css`) also lives in that crate and is
//! consumed via the server's tailwind config.
//!
//! When adding a new component:
//!   1. **If it's purely visual (no app types)** — add it to the
//!      `plan-ai-design` crate and re-export here.
//!   2. **If it depends on the server's `Route` enum or other
//!      app-specific types** — add it locally and re-export here.
//!      `breadcrumbs` and `metric::KpiCard` are the existing
//!      precedents.
//!   3. The semantic class for the component goes in
//!      `plan-ai-design/assets/input.css`, never in inline classes
//!      and never with hex literals.
//!
//! See `plan-ai-design/src/components/mod.rs` for the up-to-date
//! list of primitives the crate provides.

#![allow(dead_code, unused_imports)]

pub mod breadcrumbs;
pub mod metric;

pub use plan_ai_design::{
    ActiveSessionCard, ActivityFeed, ActivityItem, Alert, AlertVariant, Badge, BadgeVariant, Bars,
    Button, ButtonKind, ButtonSize, ButtonVariant, Card, ChartColor, Dash, DataTable, Dot,
    ErrorText, FormField, HelpText, Kicker, Mono, PageHeader, PageHero, Pill, PillVariant,
    SectionHeading, SortState, SortableTh, Sparkline, StageItem, StageStatus, StageTimeline,
    StatBlock, SuccessText, TableToolbar, Td, TdMono, TdMuted, Th, TokenCreateForm,
    TokenCreateInput, TokenReveal, TokenRow, TokenTable, TraceStatus, TraceStep, page_window,
};

pub use breadcrumbs::Breadcrumbs;
pub use metric::KpiCard;

pub mod data_table {
    //! Re-export submodule so existing pages can keep using
    //! `crate::web::components::ui::data_table::SortableTh` etc.
    pub use plan_ai_design::{
        Dash, DataTable, SortState, SortableTh, TableToolbar, Td, TdMono, TdMuted, Th,
        page_window,
    };
}
