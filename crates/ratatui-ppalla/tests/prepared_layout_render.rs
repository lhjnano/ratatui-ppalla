//! Integration tests for [`PreparedLayout`], using [`SplitLayout`] rects to
//! partition a ratatui [`Buffer`](ratatui::buffer::Buffer).
//!
//! The prepared layout primitive returns split regions ([`SplitLayout`]), not a
//! widget. Tests drive `prepare` -> `layout` to obtain the rects, then paint
//! labels into the buffer to verify the regions tile the area without overlap —
//! mirroring the `list_render.rs` pattern.

#![allow(clippy::pedantic)]

use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Rect};
use ratatui::style::Style;
use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedLayout, SplitLayout, SplitSpec};

/// Prepare `spec`, lay it out at `width`x`height`, and return a fresh buffer
/// alongside the [`SplitLayout`] (its rects describe how the area is split).
fn render_split(spec: SplitSpec, width: u16, height: u16) -> (Buffer, SplitLayout) {
    let prepared = PreparedLayout::prepare(spec);
    let ctx = LayoutCtx::new(width, height);
    let layout = PreparedLayout::layout(&prepared, ctx);
    let buf = Buffer::empty(Rect::new(0, 0, width, height));
    (buf, layout)
}

/// Write `label` into the top-left cell of `rect`. Zero-area rects are skipped.
fn paint_label_in_rect(buf: &mut Buffer, rect: Rect, label: &str) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    buf.set_string(rect.x, rect.y, label, Style::default());
}

/// AABB overlap test for two rects (u32 arithmetic avoids overflow on bounds).
fn rects_overlap(a: Rect, b: Rect) -> bool {
    let (ax1, ay1) = (u32::from(a.x), u32::from(a.y));
    let (ax2, ay2) = (ax1 + u32::from(a.width), ay1 + u32::from(a.height));
    let (bx1, by1) = (u32::from(b.x), u32::from(b.y));
    let (bx2, by2) = (bx1 + u32::from(b.width), by1 + u32::from(b.height));
    ax1 < bx2 && bx1 < ax2 && ay1 < by2 && by1 < ay2
}

#[test]
fn vertical_split_produces_stacked_rects() {
    let (_buf, layout) = render_split(
        SplitSpec::new(vec![
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(3),
        ]),
        10,
        6,
    );
    assert_eq!(layout.rects.len(), 3);
    assert_eq!(layout.rects[0], Rect::new(0, 0, 10, 1));
    assert_eq!(layout.rects[1], Rect::new(0, 1, 10, 2));
    assert_eq!(layout.rects[2], Rect::new(0, 3, 10, 3));
}

#[test]
fn horizontal_split_produces_side_by_side() {
    let (_buf, layout) = render_split(
        SplitSpec::new(vec![Constraint::Length(5), Constraint::Min(0)])
            .with_direction(Direction::Horizontal),
        10,
        4,
    );
    assert_eq!(layout.rects.len(), 2);
    assert_eq!(layout.rects[0], Rect::new(0, 0, 5, 4));
    assert_eq!(layout.rects[1], Rect::new(5, 0, 5, 4));
}

#[test]
fn rects_fill_area_without_overlap() {
    // Three full-width, single-row rects stacked vertically across a 10x3 area.
    let spec = SplitSpec::new(vec![
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ]);
    let (mut buf, layout) = render_split(spec, 10, 3);
    assert_eq!(layout.rects.len(), 3);

    // Paint a distinct label into each rect's origin cell.
    for (i, label) in ["A", "B", "C"].iter().enumerate() {
        paint_label_in_rect(&mut buf, layout.rects[i], label);
    }
    assert_eq!(buf[(0, 0)].symbol(), "A");
    assert_eq!(buf[(0, 1)].symbol(), "B");
    assert_eq!(buf[(0, 2)].symbol(), "C");

    // Rects are pairwise disjoint: no cell belongs to two regions.
    for i in 0..layout.rects.len() {
        for j in (i + 1)..layout.rects.len() {
            assert!(
                !rects_overlap(layout.rects[i], layout.rects[j]),
                "rects {i} and {j} overlap: {:?} vs {:?}",
                layout.rects[i],
                layout.rects[j]
            );
        }
    }
}

#[test]
fn cache_hit_returns_same_rects() {
    let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
        Constraint::Length(2),
        Constraint::Min(0),
    ]));
    let ctx = LayoutCtx::new(20, 4);
    let first = PreparedLayout::layout(&prepared, ctx);
    assert!(!first.cache_hit);
    let second = PreparedLayout::layout(&prepared, ctx);
    assert!(second.cache_hit);
    assert_eq!(first.rects, second.rects);
}

#[test]
fn single_constraint_is_whole_area() {
    let (_buf, layout) = render_split(SplitSpec::new(vec![Constraint::Min(0)]), 7, 3);
    assert_eq!(layout.rects.len(), 1);
    assert_eq!(layout.rects[0], Rect::new(0, 0, 7, 3));
}

#[test]
fn zero_size_area_no_panic() {
    let (_buf, layout) = render_split(
        SplitSpec::new(vec![Constraint::Length(1), Constraint::Min(0)]),
        0,
        0,
    );
    assert_eq!(layout.rects.len(), 2);
    assert!(layout.rects.iter().all(|r| r.width == 0 && r.height == 0));
}
