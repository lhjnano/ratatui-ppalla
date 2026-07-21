//! Enhanced Table widget — a port of [Bubbles' `table`](https://github.com/charmbracelet/bubbles/table).
//!
//! Provides a sortable, navigable table widget built on top of
//! [`ratatui::widgets::Table`].

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Cell, Row as RatRow, Table as RatTable, TableState};
use ratatui::Frame;

/// A column definition for a [`Table`].
#[derive(Debug, Clone)]
pub struct Column {
    /// Column header text.
    pub title: String,
    /// Column width constraint in terminal cells.
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

    fn constraint(&self) -> Constraint {
        Constraint::Length(self.width)
    }

    fn header_cell(&self) -> Cell<'_> {
        Cell::from(self.title.as_str())
    }
}

/// A row in a [`Table`]. Implementors expose their cells as `String`s, in
/// column order.
pub trait Row {
    /// The cells of this row, in column order.
    fn cells(&self) -> Vec<String>;
}

/// An enhanced table widget with sort, navigation, and selection.
#[derive(Debug, Clone)]
pub struct Table<R: Row> {
    /// Column definitions.
    pub columns: Vec<Column>,
    /// Row data.
    pub rows: Vec<R>,
    /// Index of the currently-selected row, if any.
    pub selected: Option<usize>,
    /// Active sort column index.
    pub sort_column: Option<usize>,
    /// Sort direction (true = ascending).
    pub sort_ascending: bool,
    /// Vertical scroll offset (rows scrolled past).
    pub scroll_offset: usize,
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

    /// Replace the row set. Resets selection and scroll.
    pub fn set_rows(&mut self, rows: Vec<R>) {
        self.rows = rows;
        self.selected = if self.rows.is_empty() { None } else { Some(0) };
        self.scroll_offset = 0;
    }

    /// Returns the current number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns true when there are no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns the index of the currently-selected row, if any.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Move selection down by one (clamped at last row).
    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = Some(match self.selected {
            None => 0,
            Some(i) => (i + 1).min(self.rows.len() - 1),
        });
    }

    /// Move selection up by one (clamped at first row).
    pub fn select_prev(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = Some(match self.selected {
            None => 0,
            Some(i) => i.saturating_sub(1),
        });
    }

    /// Sort by the given column index.
    ///
    /// If already sorted by that column, toggles the direction.
    /// Otherwise, sets it as the new sort column in ascending order.
    pub fn sort_by(&mut self, column_idx: usize) {
        if self.sort_column == Some(column_idx) {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = Some(column_idx);
            self.sort_ascending = true;
        }
        let dir = self.sort_ascending;
        self.rows.sort_by(move |a, b| {
            let ac = a.cells().get(column_idx).cloned().unwrap_or_default();
            let bc = b.cells().get(column_idx).cloned().unwrap_or_default();
            if dir {
                ac.cmp(&bc)
            } else {
                bc.cmp(&ac)
            }
        });
    }

    /// Renders the table inside `area`.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let header_cells: Vec<Cell<'_>> = self.columns.iter().map(Column::header_cell).collect();
        let header = RatRow::new(header_cells).style(Style::default().add_modifier(Modifier::BOLD));
        let constraints: Vec<Constraint> = self.columns.iter().map(Column::constraint).collect();
        let rat_rows: Vec<RatRow<'_>> = self
            .rows
            .iter()
            .map(|r| {
                let cells: Vec<Cell<'_>> = r
                    .cells()
                    .into_iter()
                    .map(|c| Cell::from(Text::from(c)))
                    .collect();
                RatRow::new(cells)
            })
            .collect();
        let table = RatTable::new(rat_rows, constraints)
            .header(header)
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = TableState::default();
        state.select(self.selected);
        frame.render_stateful_widget(table, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestRow(Vec<String>);
    impl Row for TestRow {
        fn cells(&self) -> Vec<String> {
            self.0.clone()
        }
    }

    fn cols() -> Vec<Column> {
        vec![Column::new("name", 10), Column::new("age", 5)]
    }

    #[test]
    fn select_next_clamps_at_last_row() {
        let mut t = Table::new(cols());
        t.set_rows(vec![
            TestRow(vec!["alice".into(), "30".into()]),
            TestRow(vec!["bob".into(), "25".into()]),
        ]);
        assert_eq!(t.selected(), Some(0));
        t.select_next();
        assert_eq!(t.selected(), Some(1));
        t.select_next(); // would go past end
        assert_eq!(t.selected(), Some(1));
    }

    #[test]
    fn select_prev_clamps_at_zero() {
        let mut t = Table::new(cols());
        t.set_rows(vec![
            TestRow(vec!["alice".into(), "30".into()]),
            TestRow(vec!["bob".into(), "25".into()]),
        ]);
        t.select_next();
        t.select_prev();
        assert_eq!(t.selected(), Some(0));
        t.select_prev(); // would go below zero
        assert_eq!(t.selected(), Some(0));
    }

    #[test]
    fn sort_by_toggles_direction_on_same_column() {
        let mut t = Table::new(cols());
        t.set_rows(vec![
            TestRow(vec!["charlie".into(), "3".into()]),
            TestRow(vec!["alice".into(), "1".into()]),
            TestRow(vec!["bob".into(), "2".into()]),
        ]);
        t.sort_by(0); // ascending
        assert_eq!(t.rows[0].cells()[0], "alice");
        t.sort_by(0); // descending
        assert_eq!(t.rows[0].cells()[0], "charlie");
    }
}
