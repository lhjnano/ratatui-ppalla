//! Integration tests for [`ratatui_ppalla::viewport::Viewport`] rendering.
//!
//! Uses ratatui's [`TestBackend`](ratatui::backend::TestBackend) to render the
//! viewport into an in-memory buffer and assert on the visible content.

use pretty_assertions::assert_eq;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::{Frame, Terminal};
use ratatui_ppalla::viewport::Viewport;

/// Render `viewport` into a fresh `width`x`height` TestBackend buffer.
fn render_buffer(viewport: &Viewport, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame| viewport.render(frame, Rect::new(0, 0, width, height)))
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
fn renders_appended_lines() {
    let mut vp = Viewport::new(10);
    vp.append_line(Line::from("one"));
    vp.append_line(Line::from("two"));
    vp.append_line(Line::from("three"));
    vp.append_line(Line::from("four"));
    let buf = render_buffer(&vp, 20, 6);

    assert_eq!(row_symbols(&buf, 0, 3), "one");
    assert_eq!(row_symbols(&buf, 1, 3), "two");
    assert_eq!(row_symbols(&buf, 2, 5), "three");
    assert_eq!(row_symbols(&buf, 3, 4), "four");
}

#[test]
fn respects_scroll_offset() {
    let mut vp = Viewport::new(4);
    for word in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"] {
        vp.append_line(Line::from(word));
    }
    vp.scroll_down(3);
    // First visible row should be line index 3 ("d"), not index 0 ("a").
    let buf = render_buffer(&vp, 5, 4);
    assert_eq!(row_symbols(&buf, 0, 1), "d");
    assert_eq!(row_symbols(&buf, 1, 1), "e");
}

#[test]
fn search_finds_matching_lines() {
    let mut vp = Viewport::new(10);
    vp.append_line(Line::from("hello foo world"));
    vp.append_line(Line::from("no match here"));
    vp.append_line(Line::from("another foo"));
    vp.append_line(Line::from("nothing"));

    vp.set_search(Some("foo"));
    assert_eq!(vp.match_count(), 2);

    // Rendering should still show all lines; match styling is applied via
    // cell style and verified separately in the unit tests.
    let buf = render_buffer(&vp, 30, 6);
    let row0 = row_symbols(&buf, 0, 15);
    assert!(row0.starts_with("hello foo world"));
}
