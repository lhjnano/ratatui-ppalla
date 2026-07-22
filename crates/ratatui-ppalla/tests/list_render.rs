//! Integration tests for [`ratatui_ppalla::list::List`] rendering.
//!
//! Uses ratatui's [`TestBackend`](ratatui::backend::TestBackend) to render the
//! widget into an in-memory buffer and assert on the visible cell content.

#![allow(clippy::needless_pass_by_value)]

use pretty_assertions::assert_eq;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Line;
use ratatui::{Frame, Terminal};
use ratatui_ppalla::list::{List, ListItem};

struct Task(&'static str);

impl ListItem for Task {
    fn render(&self) -> Line<'_> {
        Line::from(self.0)
    }
    fn filterable_text(&self) -> &str {
        self.0
    }
}

/// Render `list` into a fresh `width`x`height` TestBackend buffer.
fn render_buffer(list: &List<Task>, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame| list.render(frame, Rect::new(0, 0, width, height)))
        .expect("draw");
    terminal.backend().buffer().clone()
}

/// Collect symbols in row `y`, columns `0..len` as a single String.
fn row_symbols(buf: &Buffer, y: usize, len: usize) -> String {
    (0..len)
        .map(|x| buf[(x as u16, y as u16)].symbol().to_string())
        .collect()
}

#[test]
fn renders_all_items_when_no_filter() {
    let list = List::new(vec![Task("alpha"), Task("beta"), Task("gamma")]);
    let buf = render_buffer(&list, 20, 5);

    assert_eq!(row_symbols(&buf, 0, 5), "alpha");
    assert_eq!(row_symbols(&buf, 1, 4), "beta");
    assert_eq!(row_symbols(&buf, 2, 5), "gamma");
}

#[test]
fn renders_only_filtered_items() {
    let mut list = List::new(vec![Task("alpha"), Task("beta"), Task("gamma")]);
    list.set_filter("am");
    let buf = render_buffer(&list, 20, 5);

    // Only "gamma" contains "am".
    assert_eq!(list.filtered_len(), 1);
    assert_eq!(row_symbols(&buf, 0, 5), "gamma");
    // Row 1 should be empty — the filtered set only has one row.
    assert!(row_symbols(&buf, 1, 20).trim().is_empty());
}

#[test]
fn highlighted_style_applied_to_selected_item() {
    let mut list = List::new(vec![Task("alpha"), Task("beta")]);
    list.select_next(); // None -> first (alpha, row 0)
    list.select_next(); // first -> second (beta, row 1)
    let buf = render_buffer(&list, 20, 5);

    // The selected item should be on row 1 ("beta") with REVERSED modifier.
    // We check only the modifier bits because ratatui's stateful rendering
    // also stamps fg/bg/underline_color = Reset onto the cell, making a full
    // Style equality assertion brittle across ratatui versions.
    let selected = &buf[(0, 1)];
    assert!(
        selected.style().add_modifier.contains(Modifier::REVERSED),
        "selected row should have REVERSED highlight, got {:?}",
        selected.style()
    );

    // Sanity check: the non-selected row must NOT carry REVERSED, otherwise
    // the assertion above would be vacuous.
    let unselected = &buf[(0, 0)];
    assert!(
        !unselected.style().add_modifier.contains(Modifier::REVERSED),
        "non-selected row should not have REVERSED, got {:?}",
        unselected.style()
    );
}
