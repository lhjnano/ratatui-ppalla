//! # ratatui-ppalla
//!
//! > High-performance TUI primitives for [Ratatui] — Pretext-inspired prepare/layout separation.
//!
//! `ppalla` (빨라) means **"fast"** in Korean. This crate brings the ergonomics
//! of the [Bubble Tea](https://github.com/charmbracelet/bubbletea) component
//! model to Rust, and adds a **Preparable pattern** that separates expensive
//! one-time work from cheap per-frame work for smooth 60fps multi-pane TUI apps.
//!
//! ## The Preparable pattern
//!
//! The namesake feature, inspired by [Pretext](https://github.com/chenglou/pretext):
//! split expensive one-time [`Preparable::prepare`] (cold path) from cheap
//! per-frame [`Preparable::layout`] (hot path). Because TUI text width is
//! predetermined by Unicode width, the prepare step is cheap and the layout hot
//! path is pure arithmetic over cached widths.
//!
//! ```
//! use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedText};
//!
//! // Cold path: segment into graphemes + cache Unicode widths (once).
//! let prepared = PreparedText::prepare_str("hello world\nppalla is fast");
//! // Hot path: wrap to width + window by scroll/height (every frame).
//! let layout = PreparedText::layout(&prepared, LayoutCtx::new(10, 3));
//! assert_eq!(layout.total_lines, 4);
//! ```
//!
//! ## Modules
//!
//! **Prepared primitives** ([`prepared`]) — the Preparable pattern:
//! - [`PreparedText`](crate::prepared::PreparedText) — grapheme segmentation + Unicode width caching + visible-line wrapping
//! - [`PreparedLayout`](crate::prepared::PreparedLayout) — 1-entry cache of ratatui constraint evaluation
//! - [`PreparedList`](crate::prepared::PreparedList) — filter index + visible-item windowing
//! - [`PreparedTable`](crate::prepared::PreparedTable) — sort permutation + column widths
//! - [`PreparedViewport`](crate::prepared::PreparedViewport) — search-match index + scroll window
//! - [`PreparedBuffer`](crate::prepared::PreparedBuffer) — per-row dirty tracking + merged damage rects
//!
//! **Widgets** — Bubble Tea/Bubbles-style, built on Ratatui:
//! - [`elm`] — Elm architecture (Model/Update/View + Command/Message)
//! - [`list`] — filterable, selectable list
//! - [`viewport`] — scrollable viewport with search
//! - [`text_input`] — multi-line text input with history
//! - [`spinner`] — spinner with built-in styles
//! - [`table`] — sortable, navigable table
//! - [`key_help`] — key binding help display
//! - [`style`] — Lipgloss-style style builder
//!
//! **Runtime** ([`runtime`]) — synchronous crossterm event loop with an
//! injectable backend, testable via [`test_utils`].
//!
//! ## Status
//!
//! v0.0.1 — early development. 392 tests, ~85% coverage. The API may change
//! between 0.0.x releases. See the
//! [benchmark baseline](https://github.com/lhjnano/ratatui-ppalla/blob/main/docs/benchmarks/baseline.md)
//! for measured performance.
//!
//! [Ratatui]: https://ratatui.rs

#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]

pub mod elm;
pub mod list;
pub mod prepared;
pub mod runtime;
pub mod text_input;
pub mod viewport;

pub mod key_help;
pub mod spinner;
pub mod style;
pub mod table;

pub mod test_utils;
