//! # ratatui-bubbles
//!
//! Bubble Tea/Bubbles-style TUI components for Ratatui.
//!
//! This crate ports useful patterns and widgets from the Go [Bubble Tea](https://github.com/charmbracelet/bubbletea)
//! ecosystem (Bubble Tea + Bubbles + Lipgloss) to the Rust [Ratatui](https://ratatui.rs) ecosystem.
//!
//! ## Status
//!
//! Early development. Phase 1 in progress — see `docs/ROADMAP.md`.
//!
//! ## Modules
//!
//! - [`elm`] — Elm architecture (Model/Update/View + Command/Message)
//! - [`runtime`] — Synchronous crossterm event loop driver for Elm [`Model`](crate::elm::Model)s
//! - [`list`] — Enhanced List widget (filter/sort/pagination)
//! - [`viewport`] — Scrollable viewport with search
//! - [`text_input`] — Multi-line text input with history
//! - [`spinner`] — Spinner widget with built-in styles
//! - [`table`] — Enhanced Table widget (sort/navigate/select)
//! - [`key_help`] — Key binding help display
//! - [`style`] — Lipgloss-style builder API

#![warn(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]

pub mod elm;
pub mod list;
pub mod runtime;
pub mod text_input;
pub mod viewport;

pub mod key_help;
pub mod spinner;
pub mod style;
pub mod table;

pub mod test_utils;
