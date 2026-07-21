//! Integration tests for [`ratatui_bubbles::text_input::TextInput`] rendering.

use pretty_assertions::assert_eq;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect};
use ratatui::{Frame, Terminal};
use ratatui_bubbles::text_input::TextInput;

fn render_input(ti: &TextInput, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame| ti.render(frame, Rect::new(0, 0, width, height)))
        .expect("draw");
    terminal.backend().buffer().clone()
}

/// Collect the symbols in `buf` at row `y`, columns `0..len`, joined into a
/// single `String`.
fn row_symbols(buf: &Buffer, y: u16, len: u16) -> String {
    (0..len)
        .map(|x| {
            buf.cell(Position { x, y })
                .map(Cell::symbol)
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

#[test]
fn renders_single_line_content() {
    let mut ti = TextInput::new();
    ti.insert_str("hello world");
    let buf = render_input(&ti, 30, 3);
    // "hello world" (11 chars) should render at row 0, column 0.
    assert_eq!(row_symbols(&buf, 0, 11), "hello world");
}

#[test]
fn renders_multiline_content() {
    let mut ti = TextInput::new();
    ti.insert_str("line1");
    ti.enter();
    ti.insert_str("line2");
    let buf = render_input(&ti, 20, 3);
    // "line1" on row 0, "line2" on row 1.
    assert_eq!(row_symbols(&buf, 0, 5), "line1");
    assert_eq!(row_symbols(&buf, 1, 5), "line2");
}
