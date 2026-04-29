//! Shared UI components.
//!
//! Each component emits semantic classes (`btn`, `input`, `card`, `h-page`,
//! …) defined in `server/input.css`. Theme — color, typography, spacing
//! tokens — lives in CSS. Components describe structure, accessibility,
//! and behavior; views compose them.
//!
//! When adding a new component, prefer:
//!   1. Add the semantic classes to `input.css` (under `@layer components`).
//!   2. Add the Rust component here that emits those classes.
//!   3. Don't put hex literals or palette utilities (`bg-blue-600`, `dark:…`)
//!      anywhere in this module — those would defeat the theme contract.

// Phase 0: only some of these components have consumers yet. Phase 1+ will
// migrate the remaining pages and the dead-code warnings will resolve.
#![allow(dead_code, unused_imports)]

pub mod badge;
pub mod button;
pub mod card;
pub mod data_table;
pub mod form;
pub mod text;

pub use badge::{Badge, BadgeVariant};
pub use button::{Button, ButtonKind, ButtonSize, ButtonVariant};
pub use card::Card;
pub use data_table::{
    Dash, DataTable, SortState, SortableTh, TableToolbar, Td, TdMono, TdMuted, Th,
};
pub use form::FormField;
pub use text::{ErrorText, HelpText, PageHeader, SectionHeading, SuccessText};
