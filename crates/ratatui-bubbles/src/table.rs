//! Enhanced Table widget — a port of [Bubbles' `table`](https://github.com/charmbracelet/bubbles/table).
//!
//! # Status
//!
//! **Tier 2 — not yet implemented.** Stub only. Non-trivial methods panic
//! with [`todo!()`].

#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]

/// A column definition for a [`Table`].
#[derive(Debug, Clone)]
pub struct Column {
    /// Column header text.
    pub title: String,
    /// Column width in terminal cells.
    pub width: u16,
}

impl Column {
    /// Create a new column with the given title and width.
    #[must_use]
    pub fn new(title: impl Into<String>, width: u16) -> Self {
        Self {
            title: title.into(),
            width,
        }
    }
}

/// A row in a [`Table`].
///
/// Implementors expose their cells as `String`s, in column order.
pub trait Row {
    /// The cells of this row, in column order.
    fn cells(&self) -> Vec<String>;
}

/// An enhanced table widget with sort, filter, and virtual scroll.
///
/// Tier 2 stub — see module docs. Most methods panic via `todo!()`.
#[derive(Debug, Clone)]
pub struct Table<R: Row> {
    columns: Vec<Column>,
    rows: Vec<R>,
    selected: Option<usize>,
    sort_column: Option<usize>,
    sort_ascending: bool,
    scroll_offset: usize,
}

impl<R: Row> Table<R> {
    /// Create a new table with the given columns and no rows.
    #[must_use]
    pub fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
            selected: None,
            sort_column: None,
            sort_ascending: true,
            scroll_offset: 0,
        }
    }

    /// Replace the row set.
    pub fn set_rows(&mut self, rows: Vec<R>) {
        self.rows = rows;
        self.selected = if self.rows.is_empty() { None } else { Some(0) };
        self.scroll_offset = 0;
    }

    /// Move selection down by one (clamped).
    pub fn select_next(&mut self) {
        todo!("Tier 2: Table::select_next")
    }

    /// Move selection up by one (clamped).
    pub fn select_prev(&mut self) {
        todo!("Tier 2: Table::select_prev")
    }

    /// Sort by the given column index; toggles direction if already sorted by it.
    pub fn sort_by(&mut self, column_idx: usize) {
        let _ = column_idx;
        todo!("Tier 2: Table::sort_by")
    }
}
