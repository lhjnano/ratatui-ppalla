//! Integration tests for [`ratatui_presto::key_help::KeyHelp`] rendering.
//!
//! Uses ratatui's [`TestBackend`] to render the widget into an in-memory buffer
//! and assert on the visible cell content.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use ratatui_presto::key_help::{KeyBinding, KeyHelp};

fn render_buffer(help: &KeyHelp, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame| help.render(frame, Rect::new(0, 0, width, height)))
        .expect("draw");
    terminal.backend().buffer().clone()
}

fn row_symbols(buf: &Buffer, y: usize, len: usize) -> String {
    (0..len)
        .map(|x| buf[(x as u16, y as u16)].symbol().to_string())
        .collect()
}

#[test]
fn renders_title_in_top_border() {
    let help = KeyHelp::new().with_title("Shortcuts");
    let buf = render_buffer(&help, 30, 5);
    // Title appears in the top border row (row 0)
    let top = row_symbols(&buf, 0, 30);
    assert!(top.contains("Shortcuts"), "top row was: {top:?}");
}

#[test]
fn renders_bindings_as_key_desc_pairs() {
    let mut help = KeyHelp::new();
    help.add(KeyBinding::new("q", "quit"));
    help.add(KeyBinding::new("r", "refresh"));
    let buf = render_buffer(&help, 30, 6);
    // The block has a 1-cell border. Content starts at row 1, col 1.
    // First binding 'q' should be in row 1.
    let row1 = row_symbols(&buf, 1, 15);
    assert!(row1.contains('q'), "row1 was: {row1:?}");
    assert!(row1.contains("quit"), "row1 was: {row1:?}");
}

#[test]
fn excludes_disabled_bindings_from_render() {
    let mut help = KeyHelp::new();
    help.add(KeyBinding::new("q", "quit"));
    help.add(KeyBinding::new("d", "deleted").disabled());
    let buf = render_buffer(&help, 30, 6);
    // Scan all rows — 'deleted' should NOT appear anywhere
    for y in 0..6 {
        let row = row_symbols(&buf, y, 30);
        assert!(
            !row.contains("deleted"),
            "row {y} contained 'deleted': {row:?}"
        );
    }
}
