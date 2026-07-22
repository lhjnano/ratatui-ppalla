//! Integration tests for [`ratatui_bubbles::spinner::Spinner`] rendering.
//!
//! Uses ratatui's [`TestBackend`] to render the widget into an in-memory buffer
//! and assert on the visible cell content.

use pretty_assertions::assert_eq;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::{Frame, Terminal};
use ratatui_bubbles::spinner::{Spinner, SpinnerStyle};

/// Render `spinner` into a fresh `width`x`height` TestBackend buffer.
fn render_buffer(spinner: &Spinner, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame: &mut Frame| spinner.render(frame, Rect::new(0, 0, width, height)))
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
fn renders_line_style_frame_zero() {
    let spinner = Spinner::new(SpinnerStyle::Line);
    let buf = render_buffer(&spinner, 5, 1);
    // Line frame 0 is "|"
    assert_eq!(row_symbols(&buf, 0, 1), "|");
}

#[test]
fn renders_after_tick_advances_frame() {
    let mut spinner = Spinner::new(SpinnerStyle::Line);
    let buf_before = render_buffer(&spinner, 5, 1);
    assert_eq!(row_symbols(&buf_before, 0, 1), "|");
    spinner.tick();
    let buf_after = render_buffer(&spinner, 5, 1);
    assert_eq!(row_symbols(&buf_after, 0, 1), "/");
}

#[test]
fn renders_dot_style_first_frame() {
    let spinner = Spinner::new(SpinnerStyle::Dot);
    let buf = render_buffer(&spinner, 5, 1);
    // Dot frame 0 is "⣾" (multi-byte unicode)
    let s = row_symbols(&buf, 0, 1);
    assert_eq!(s, "⣾");
}
