//! Integration tests for [`ratatui_bubbles::table::Table`] rendering.
//!
//! Uses ratatui's [`TestBackend`] to render the widget into an in-memory buffer
//! and assert on the visible cell content.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui_bubbles::table::{Column, Row, Table};

#[derive(Debug, Clone)]
struct Person {
    name: &'static str,
    age: &'static str,
}
impl Row for Person {
    fn cells(&self) -> Vec<String> {
        vec![self.name.to_string(), self.age.to_string()]
    }
}

fn columns() -> Vec<Column> {
    vec![Column::new("name", 10), Column::new("age", 5)]
}

fn render_buffer(table: &Table<Person>, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame| table.render(frame, Rect::new(0, 0, width, height)))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn row_symbols(buf: &Buffer, y: usize, len: usize) -> String {
    (0..len)
        .map(|x| buf[(x as u16, y as u16)].symbol().to_string())
        .collect()
}

#[test]
fn renders_header_row() {
    let t = Table::<Person>::new(columns());
    let buf = render_buffer(&t, 20, 5);
    // Header row should contain 'name'
    let header = row_symbols(&buf, 0, 10);
    assert!(header.starts_with("name"));
}

#[test]
fn renders_data_rows_below_header() {
    let mut t = Table::<Person>::new(columns());
    t.set_rows(vec![
        Person {
            name: "alice",
            age: "30",
        },
        Person {
            name: "bob",
            age: "25",
        },
    ]);
    let buf = render_buffer(&t, 20, 5);
    // Row 1 should contain 'alice', row 2 'bob' (header is row 0)
    let row1 = row_symbols(&buf, 1, 10);
    let row2 = row_symbols(&buf, 2, 10);
    assert!(row1.starts_with("alice"), "row1 was: {row1:?}");
    assert!(row2.starts_with("bob"), "row2 was: {row2:?}");
}

#[test]
fn renders_no_data_rows_when_empty() {
    let t = Table::<Person>::new(columns());
    let buf = render_buffer(&t, 20, 5);
    // Row 0 has header; row 1 should be empty
    let row1 = row_symbols(&buf, 1, 10);
    assert!(row1.trim().is_empty(), "row1 was: {row1:?}");
}
