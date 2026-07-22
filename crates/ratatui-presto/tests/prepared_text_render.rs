//! Integration tests for [`PreparedText`], painting a [`TextLayout`] onto a
//! ratatui [`Buffer`](ratatui::buffer::Buffer).
//!
//! The prepared primitives return layout results (display lines of grapheme
//! segments), not widgets. Each test uses a local render-bridge helper to paint
//! the [`TextLayout`] into an in-memory [`Buffer`](ratatui::buffer::Buffer) and
//! then asserts on the visible cell content — mirroring the `list_render.rs`
//! pattern of `render_buffer()` + `row_symbols()`.

#![allow(clippy::pedantic)]

use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui_presto::prepared::{LayoutCtx, Preparable, PreparedText, TextLayout};

/// Paint a [`TextLayout`] into `buf` within `area`: each visible display line is
/// rendered left-to-right, advancing by every segment's cached Unicode width.
/// Width-2 graphemes clear their continuation cell so the wide character's
/// second column does not read back as a stray space. Unpainted cells keep the
/// default space that [`Buffer::empty`] seeds fresh buffers with.
fn paint_text_layout(buf: &mut Buffer, layout: &TextLayout, area: Rect) {
    for (row_idx, display_line) in layout.lines.iter().enumerate() {
        let Some(y) = area.y.checked_add(row_idx as u16) else {
            break;
        };
        if y >= area.bottom() {
            break;
        }
        let mut x = area.x;
        for seg in &display_line.segments {
            let w = seg.width;
            if w == 0 {
                continue;
            }
            let Some(next) = x.checked_add(w) else {
                break;
            };
            if next > area.right() {
                break;
            }
            buf[(x, y)].set_symbol(&seg.grapheme);
            // A width-2 grapheme occupies two cells; blank the continuation
            // column so it does not read back as a stray space.
            if w == 2 {
                buf[(x + 1, y)].set_symbol("");
            }
            x = next;
        }
    }
}

/// Prepare `text`, lay it out under `ctx`, paint it into a fresh buffer, and
/// return the buffer for assertion.
fn render_with_ctx(text: &str, ctx: LayoutCtx) -> Buffer {
    let prepared = PreparedText::prepare_str(text);
    let layout = PreparedText::layout(&prepared, ctx);
    let area = Rect::new(0, 0, ctx.width, ctx.height);
    let mut buf = Buffer::empty(area);
    paint_text_layout(&mut buf, &layout, area);
    buf
}

/// Prepare `text`, lay it out at `width`x`height`, paint it into a fresh
/// buffer, and return a clone for assertion.
fn render_text(text: &str, width: u16, height: u16) -> Buffer {
    render_with_ctx(text, LayoutCtx::new(width, height))
}

/// Collect the symbols in row `y`, columns `0..len`, as a single `String`.
fn row_symbols(buf: &Buffer, y: usize, len: usize) -> String {
    (0..len)
        .map(|x| buf[(x as u16, y as u16)].symbol().to_string())
        .collect()
}

#[test]
fn paints_ascii_single_line() {
    let buf = render_text("hello", 80, 1);
    assert_eq!(row_symbols(&buf, 0, 5), "hello");
}

#[test]
fn paints_wrapped_lines() {
    // "hello world" wraps to width 5 as ["hello", " worl", "d"].
    let buf = render_text("hello world", 5, 3);
    assert_eq!(row_symbols(&buf, 0, 5), "hello");
    assert_eq!(row_symbols(&buf, 1, 5), " worl");
    assert_eq!(row_symbols(&buf, 2, 1), "d");
}

#[test]
fn scroll_window_hides_top_row() {
    // Four single-char logical lines at 80x2 with scroll=1 window rows 1 and 2.
    let buf = render_with_ctx("a\nb\nc\nd", LayoutCtx::new(80, 2).with_scroll(1));
    assert_eq!(row_symbols(&buf, 0, 1), "b");
    assert_eq!(row_symbols(&buf, 1, 1), "c");
}

#[test]
fn unicode_cjk_occupies_two_cells() {
    let buf = render_text("中日", 4, 1);
    // Each CJK grapheme is width 2: 中 spans columns 0-1, 日 spans columns 2-3.
    assert_eq!(buf[(0, 0)].symbol(), "中");
    assert_eq!(buf[(2, 0)].symbol(), "日");
    // Continuation columns drop out, so the row reads as the two graphemes.
    assert_eq!(row_symbols(&buf, 0, 4), "中日");
    // The display line consumed four cells (two per grapheme).
    let prepared = PreparedText::prepare_str("中日");
    let layout = PreparedText::layout(&prepared, LayoutCtx::new(4, 1));
    assert_eq!(layout.lines[0].width, 4);
}

#[test]
fn wide_grapheme_clipped_at_narrow_width() {
    // A lone width-2 grapheme at width 1 cannot fit and is skipped, leaving an
    // empty (blank) display line.
    let buf = render_text("中", 1, 1);
    assert_eq!(row_symbols(&buf, 0, 1), " ");
}

#[test]
fn empty_text_renders_blank() {
    let buf = render_text("", 10, 2);
    assert_eq!(row_symbols(&buf, 0, 10), "          ");
    assert_eq!(row_symbols(&buf, 1, 10), "          ");
}

#[test]
fn trailing_newline_renders_blank_second_row() {
    let buf = render_text("x\n", 80, 2);
    assert_eq!(row_symbols(&buf, 0, 1), "x");
    assert_eq!(row_symbols(&buf, 1, 10), "          ");
}

#[test]
fn appended_text_appears() {
    let mut prepared = PreparedText::prepare_str("a\nb");
    PreparedText::append(&mut prepared, "c".to_string());
    let layout = PreparedText::layout(&prepared, LayoutCtx::new(80, 3));
    let area = Rect::new(0, 0, 80, 3);
    let mut buf = Buffer::empty(area);
    paint_text_layout(&mut buf, &layout, area);
    assert_eq!(row_symbols(&buf, 0, 1), "a");
    assert_eq!(row_symbols(&buf, 1, 1), "b");
    assert_eq!(row_symbols(&buf, 2, 1), "c");
}
