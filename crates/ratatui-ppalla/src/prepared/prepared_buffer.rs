//! # `PreparedBuffer` — cell-grid buffer with per-row dirty tracking.
//!
//! A concrete implementation of the [Pretext](https://github.com/0xradical/Pretext)
//! prepare/layout separation for a damage-tracked cell grid. The cold path
//! ([`PreparedBuffer::prepare`] / [`PreparedBuffer::append`]) builds a
//! `width × height` grid of [`BufferCell`]s plus a parallel per-row `dirty`
//! flag vector. The hot path ([`PreparedBuffer::layout`]) scans the dirty flags
//! and emits the **minimal set of full-width damage rects**: every maximal run
//! of adjacent dirty rows collapses into a single [`Rect`] (full grid width,
//! `y = run start`, `height = run length`).
//!
//! Because damage merging is a single linear scan over a `Vec<bool>`, the hot
//! path performs only integer comparisons plus a handful of rect allocations —
//! matching the "ppalla" (빨라, "fast" in Korean) value proposition of ratatui-ppalla.
//!
//! # Dirty-tracking model
//!
//! - [`PreparedBuffer::prepare`] marks **every** row dirty (a fresh buffer is a
//!   full first paint).
//! - [`PreparedBufferState::set_cell`] mutates one cell and marks exactly its
//!   own row dirty.
//! - [`PreparedBufferState::mark_all_dirty`] /
//!   [`PreparedBufferState::clear_dirty`] flip the whole flag vector (e.g.
//!   before/after a full repaint).
//! - [`Preparable::layout`] never mutates the flags;
//!   consume the damage rects, then call
//!   [`clear_dirty`](PreparedBufferState::clear_dirty).
//!
//! [`LayoutCtx`] is accepted by `layout` only to satisfy the [`Preparable`]
//! contract; the rects are emitted in grid coordinates and `ctx.scroll` /
//! `ctx.focus` are intentionally ignored.
//!
//! # Damage-rect merging algorithm
//!
//! Scan `dirty` top to bottom. For each maximal run of `true` values
//! `[y_start, y_end)`, emit
//! `Rect { x: 0, y: y_start, width: grid_width, height: y_end - y_start }`.
//! Runs of `false` are skipped. This yields the minimal set of full-width bands
//! covering every dirty row with no overlaps.

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};
use ratatui::layout::Rect;

/// A single buffer cell: a grapheme string.
///
/// Kept simple and owned so that [`Eq`] holds for the whole grid (a
/// `Vec<Vec<BufferCell>>`). The canonical "empty" cell is
/// [`BufferCell::blank`] (a single space); note that the derived
/// [`Default`](BufferCell#impl-Default-for-BufferCell) produces an empty string,
/// so prefer [`blank`](BufferCell::blank) when constructing grids.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BufferCell {
    /// The cell's grapheme (default = empty string; [`blank`](BufferCell::blank)
    /// yields a single space).
    pub symbol: String,
}

impl BufferCell {
    /// A blank cell holding a single space.
    #[must_use]
    pub fn blank() -> Self {
        Self {
            symbol: " ".to_string(),
        }
    }
}

/// Input for preparing a [`PreparedBuffer`]: the grid dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BufferInput {
    /// Grid width in cells.
    pub width: u16,
    /// Grid height in rows.
    pub height: u16,
}

/// Prepared state: the cell grid plus a parallel per-row dirty-flag vector.
///
/// `cells[y][x]` addresses row `y`, column `x`. `dirty[y] == true` means row
/// `y` changed since the flags were last cleared. The two vectors always have
/// length `height` (rows) and each `cells[y]` has length `width`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedBufferState {
    /// Grid width in cells.
    pub width: u16,
    /// Grid height in rows.
    pub height: u16,
    /// Row-major cells: `cells[y][x]`.
    pub cells: Vec<Vec<BufferCell>>,
    /// `dirty[y] == true` → row `y` changed since the flags were last cleared.
    pub dirty: Vec<bool>,
}

/// Layout result: the merged damage rects (contiguous dirty-row bands, each
/// full-width) plus the total dirty-row count.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BufferLayout {
    /// Damage rects: each is a full-width band covering one contiguous run of
    /// dirty rows.
    pub damage_rects: Vec<Rect>,
    /// Number of dirty rows total.
    pub dirty_row_count: usize,
}

/// Prepared buffer primitive: a damage-tracked cell grid.
///
/// Implements [`Preparable`]. The input is a [`BufferInput`] (grid dimensions).
/// [`Preparable::prepare`] builds a blank grid and marks every row dirty (first
/// paint). [`Preparable::append`] rebuilds the grid to new dimensions and marks
/// every row dirty again (a resize is a full repaint). [`Preparable::layout`]
/// merges adjacent dirty rows into contiguous full-width [`Rect`]s.
///
/// `ctx.scroll` and `ctx.focus` do not affect the buffer — only the prepared
/// dirty flags determine the damage rects. They are accepted only to satisfy the
/// [`LayoutCtx`] contract.
///
/// # Examples
///
/// ```
/// use ratatui_ppalla::prepared::{BufferInput, LayoutCtx, Preparable, PreparedBuffer};
///
/// let mut prepared = PreparedBuffer::prepare(BufferInput { width: 4, height: 4 });
/// prepared.clear_dirty();            // start with a clean grid
/// prepared.set_cell(0, 0, "A");      // row 0 dirty
/// prepared.set_cell(0, 2, "C");      // row 2 dirty (row 1 clean)
/// let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(4, 4));
/// // Two non-adjacent dirty rows => two separate full-width damage rects.
/// assert_eq!(layout.damage_rects.len(), 2);
/// assert_eq!(layout.dirty_row_count, 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PreparedBuffer;

impl Preparable for PreparedBuffer {
    type Prepared = PreparedBufferState;
    type Layout = BufferLayout;
    type Input = BufferInput;

    fn prepare(input: Self::Input) -> Self::Prepared {
        let (width, height) = (input.width, input.height);
        PreparedBufferState {
            width,
            height,
            cells: build_blank_grid(width, height),
            // Every row is dirty on the first paint.
            dirty: vec![true; usize::from(height)],
        }
    }

    /// Rebuild the grid to the new dimensions and mark every row dirty. A
    /// resize is a full repaint, so previous cell content is discarded.
    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        *prepared = Self::prepare(more);
    }

    fn layout(prepared: &Self::Prepared, _ctx: LayoutCtx) -> Self::Layout {
        let width = prepared.width;
        let row_count = prepared.dirty.len();
        let mut damage_rects: Vec<Rect> = Vec::new();
        let mut dirty_row_count = 0usize;
        let mut run_start: Option<usize> = None;

        for (y, &is_dirty) in prepared.dirty.iter().enumerate() {
            if is_dirty {
                dirty_row_count += 1;
                // Extend (or open) the current run. A run is left open until a
                // clean row closes it, so adjacent dirty rows merge.
                run_start.get_or_insert(y);
            } else if let Some(start) = run_start.take() {
                damage_rects.push(band_rect(width, start, y));
            }
        }
        // Flush a trailing run that reaches the bottom of the grid.
        if let Some(start) = run_start.take() {
            damage_rects.push(band_rect(width, start, row_count));
        }

        BufferLayout {
            damage_rects,
            dirty_row_count,
        }
    }
}

impl PreparedBufferState {
    /// Set a cell's symbol, marking its row dirty. No-op (no panic) if either
    /// coordinate is out of bounds.
    pub fn set_cell(&mut self, x: u16, y: u16, symbol: impl Into<String>) {
        let (xi, yi) = (usize::from(x), usize::from(y));
        if let Some(row) = self.cells.get_mut(yi) {
            if let Some(cell) = row.get_mut(xi) {
                *cell = BufferCell {
                    symbol: symbol.into(),
                };
                if let Some(flag) = self.dirty.get_mut(yi) {
                    *flag = true;
                }
            }
        }
    }

    /// Mark every row dirty (e.g. before a full repaint).
    pub fn mark_all_dirty(&mut self) {
        for flag in &mut self.dirty {
            *flag = true;
        }
    }

    /// Clear every dirty flag (call after the damage rects have been consumed).
    pub fn clear_dirty(&mut self) {
        for flag in &mut self.dirty {
            *flag = false;
        }
    }

    /// Count the dirty rows.
    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.dirty.iter().filter(|&&flag| flag).count()
    }
}

/// Build a `width × height` grid of blank cells (`cells[y][x]`).
fn build_blank_grid(width: u16, height: u16) -> Vec<Vec<BufferCell>> {
    let row = vec![BufferCell::blank(); usize::from(width)];
    vec![row; usize::from(height)]
}

/// Build a full-width damage [`Rect`] covering rows `[start, end)`.
///
/// `start` and `end` are row indices; `end >= start` is assumed. Conversions to
/// `u16` are saturating-by-`try_from` and never truncate for valid grid sizes
/// (`start`, `end <= height <= u16::MAX`).
fn band_rect(width: u16, start: usize, end: usize) -> Rect {
    Rect::new(
        0,
        u16::try_from(start).unwrap_or(u16::MAX),
        width,
        u16::try_from(end - start).unwrap_or(u16::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ---------- prepare ----------

    #[test]
    fn prepare_builds_blank_grid_all_dirty() {
        let state = PreparedBuffer::prepare(BufferInput {
            width: 3,
            height: 2,
        });
        assert_eq!(state.width, 3);
        assert_eq!(state.height, 2);
        assert_eq!(state.cells.len(), 2);
        assert_eq!(state.cells[0].len(), 3);
        // Every cell is blank (single space).
        for row in &state.cells {
            for cell in row {
                assert_eq!(*cell, BufferCell::blank());
            }
        }
        // Every row is dirty on the first paint.
        assert_eq!(state.dirty.len(), 2);
        assert!(state.dirty.iter().all(|&flag| flag));
        assert_eq!(state.dirty_count(), 2);
    }

    #[test]
    fn prepare_zero_sized_grid_is_empty() {
        let state = PreparedBuffer::prepare(BufferInput {
            width: 0,
            height: 0,
        });
        assert_eq!(state.width, 0);
        assert_eq!(state.height, 0);
        assert!(state.cells.is_empty());
        assert!(state.dirty.is_empty());
        assert_eq!(state.dirty_count(), 0);
    }

    #[test]
    fn prepare_zero_width_keeps_row_count() {
        let state = PreparedBuffer::prepare(BufferInput {
            width: 0,
            height: 3,
        });
        assert_eq!(state.cells.len(), 3);
        // Each row is empty (width 0) but exists.
        assert!(state.cells.iter().all(Vec::is_empty));
        assert_eq!(state.dirty_count(), 3);
    }

    // ---------- set_cell ----------

    #[test]
    fn set_cell_updates_symbol_and_marks_only_its_row_dirty() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 3,
            height: 3,
        });
        state.clear_dirty();
        state.set_cell(2, 1, "X");
        assert_eq!(state.cells[1][2].symbol, "X");
        assert_eq!(state.dirty, vec![false, true, false]);
        assert_eq!(state.dirty_count(), 1);
    }

    #[test]
    fn set_cell_out_of_bounds_is_noop_no_panic() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 2,
            height: 2,
        });
        state.clear_dirty();
        state.set_cell(5, 0, "Q"); // x out of bounds
        state.set_cell(0, 5, "Q"); // y out of bounds
        state.set_cell(9, 9, "Q"); // both out of bounds
        assert_eq!(state.dirty_count(), 0);
        assert!(state.cells[0].iter().all(|c| c == &BufferCell::blank()));
        assert!(state.cells[1].iter().all(|c| c == &BufferCell::blank()));
    }

    #[test]
    fn set_cell_accepts_str_and_string() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 2,
            height: 1,
        });
        state.set_cell(0, 0, "a"); // &str
        state.set_cell(1, 0, String::from("b")); // String
        assert_eq!(state.cells[0][0].symbol, "a");
        assert_eq!(state.cells[0][1].symbol, "b");
    }

    #[test]
    fn set_cell_re_marking_dirty_row_does_not_double_count() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 2,
            height: 2,
        });
        state.clear_dirty();
        state.set_cell(0, 0, "a");
        assert_eq!(state.dirty_count(), 1);
        state.set_cell(1, 0, "b"); // same row, already dirty
        assert_eq!(state.dirty_count(), 1);
    }

    // ---------- mark_all_dirty / clear_dirty / dirty_count ----------

    #[test]
    fn mark_all_dirty_sets_every_flag() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 1,
            height: 3,
        });
        state.clear_dirty();
        assert_eq!(state.dirty_count(), 0);
        state.mark_all_dirty();
        assert_eq!(state.dirty, vec![true, true, true]);
        assert_eq!(state.dirty_count(), 3);
    }

    #[test]
    fn clear_dirty_resets_every_flag() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 1,
            height: 3,
        });
        // prepare marks all dirty.
        assert_eq!(state.dirty_count(), 3);
        state.clear_dirty();
        assert_eq!(state.dirty, vec![false, false, false]);
        assert_eq!(state.dirty_count(), 0);
    }

    #[test]
    fn dirty_count_tracks_incremental_mutations() {
        let mut state = PreparedBuffer::prepare(BufferInput {
            width: 2,
            height: 4,
        });
        state.clear_dirty();
        assert_eq!(state.dirty_count(), 0);
        state.set_cell(0, 0, "a");
        assert_eq!(state.dirty_count(), 1);
        state.set_cell(1, 3, "b");
        assert_eq!(state.dirty_count(), 2);
        state.set_cell(0, 0, "c"); // re-mark same row
        assert_eq!(state.dirty_count(), 2);
    }

    // ---------- layout: damage-rect merging ----------

    #[test]
    fn layout_no_dirty_rows_is_empty() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 4,
            height: 3,
        });
        prepared.clear_dirty();
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(4, 3));
        assert!(layout.damage_rects.is_empty());
        assert_eq!(layout.dirty_row_count, 0);
    }

    #[test]
    fn layout_single_dirty_row_is_one_rect() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 5,
            height: 4,
        });
        prepared.clear_dirty();
        prepared.set_cell(0, 2, "x");
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(5, 4));
        assert_eq!(layout.damage_rects, vec![Rect::new(0, 2, 5, 1)]);
        assert_eq!(layout.dirty_row_count, 1);
    }

    #[test]
    fn layout_adjacent_dirty_rows_merge_into_one_rect() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 6,
            height: 5,
        });
        prepared.clear_dirty();
        prepared.set_cell(0, 1, "a");
        prepared.set_cell(0, 2, "b");
        prepared.set_cell(0, 3, "c");
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(6, 5));
        assert_eq!(layout.damage_rects, vec![Rect::new(0, 1, 6, 3)]);
        assert_eq!(layout.dirty_row_count, 3);
    }

    #[test]
    fn layout_non_adjacent_dirty_rows_produce_two_rects() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 4,
            height: 6,
        });
        prepared.clear_dirty();
        prepared.set_cell(0, 0, "a");
        prepared.set_cell(0, 4, "b");
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(4, 6));
        assert_eq!(
            layout.damage_rects,
            vec![Rect::new(0, 0, 4, 1), Rect::new(0, 4, 4, 1)]
        );
        assert_eq!(layout.dirty_row_count, 2);
    }

    #[test]
    fn layout_all_dirty_is_one_full_height_rect() {
        let prepared = PreparedBuffer::prepare(BufferInput {
            width: 7,
            height: 4,
        });
        // prepare marks all dirty.
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(7, 4));
        assert_eq!(layout.damage_rects, vec![Rect::new(0, 0, 7, 4)]);
        assert_eq!(layout.dirty_row_count, 4);
    }

    #[test]
    fn layout_alternating_dirty_rows_produce_one_rect_per_run() {
        // 6 rows: dirty, clean, dirty, clean, dirty, clean => 3 runs of length 1.
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 4,
            height: 6,
        });
        prepared.clear_dirty();
        for y in [0u16, 2, 4] {
            prepared.set_cell(0, y, "x");
        }
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(4, 6));
        assert_eq!(layout.damage_rects.len(), 3);
        assert_eq!(layout.dirty_row_count, 3);
        assert_eq!(
            layout.damage_rects,
            vec![
                Rect::new(0, 0, 4, 1),
                Rect::new(0, 2, 4, 1),
                Rect::new(0, 4, 4, 1),
            ]
        );
    }

    #[test]
    fn layout_dirty_row_count_matches_prepared_dirty_count() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 3,
            height: 8,
        });
        prepared.clear_dirty();
        prepared.set_cell(0, 0, "a");
        prepared.set_cell(0, 1, "b");
        prepared.set_cell(0, 5, "c");
        prepared.set_cell(0, 7, "d");
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(3, 8));
        assert_eq!(layout.dirty_row_count, prepared.dirty_count());
    }

    #[test]
    fn layout_does_not_mutate_dirty_flags() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 3,
            height: 3,
        });
        prepared.clear_dirty();
        prepared.set_cell(0, 1, "x");
        let before = prepared.dirty.clone();
        let _ = PreparedBuffer::layout(&prepared, LayoutCtx::new(3, 3));
        assert_eq!(prepared.dirty, before);
    }

    #[test]
    fn layout_empty_grid_is_empty() {
        let prepared = PreparedBuffer::prepare(BufferInput {
            width: 0,
            height: 0,
        });
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(0, 0));
        assert!(layout.damage_rects.is_empty());
        assert_eq!(layout.dirty_row_count, 0);
    }

    // ---------- append (resize) ----------

    #[test]
    fn append_resizes_grid_and_marks_all_dirty() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 2,
            height: 2,
        });
        prepared.clear_dirty();
        PreparedBuffer::append(
            &mut prepared,
            BufferInput {
                width: 4,
                height: 3,
            },
        );
        assert_eq!(prepared.width, 4);
        assert_eq!(prepared.height, 3);
        assert_eq!(prepared.cells.len(), 3);
        assert!(prepared.cells.iter().all(|row| row.len() == 4));
        assert!(prepared.dirty.iter().all(|&flag| flag));
        assert_eq!(prepared.dirty_count(), 3);
    }

    #[test]
    fn append_discards_previous_content() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 4,
            height: 2,
        });
        prepared.set_cell(0, 0, "X");
        PreparedBuffer::append(
            &mut prepared,
            BufferInput {
                width: 4,
                height: 2,
            },
        );
        // Rebuilt grid => every cell is blank again.
        assert_eq!(prepared.cells[0][0], BufferCell::blank());
    }

    #[test]
    fn append_shrinking_grid_rebuilds_clean() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 10,
            height: 10,
        });
        PreparedBuffer::append(
            &mut prepared,
            BufferInput {
                width: 1,
                height: 1,
            },
        );
        assert_eq!(prepared.cells.len(), 1);
        assert_eq!(prepared.cells[0].len(), 1);
        assert_eq!(prepared.dirty_count(), 1);
    }

    // ---------- full workflow ----------

    #[test]
    fn workflow_prepare_mutate_layout_clear_relayout() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 8,
            height: 4,
        });
        // First paint: everything dirty.
        let first = PreparedBuffer::layout(&prepared, LayoutCtx::new(8, 4));
        assert_eq!(first.damage_rects, vec![Rect::new(0, 0, 8, 4)]);

        prepared.clear_dirty();
        prepared.set_cell(0, 0, "a");
        prepared.set_cell(0, 1, "b");
        let second = PreparedBuffer::layout(&prepared, LayoutCtx::new(8, 4));
        // Rows 0-1 merged into one band.
        assert_eq!(second.damage_rects, vec![Rect::new(0, 0, 8, 2)]);
        assert_eq!(second.dirty_row_count, 2);

        prepared.clear_dirty();
        let third = PreparedBuffer::layout(&prepared, LayoutCtx::new(8, 4));
        assert!(third.damage_rects.is_empty());
    }

    // ---------- invariant loops (manual, no proptest macros) ----------

    #[test]
    fn invariant_damage_rects_cover_dirty_rows_exactly_no_overlap() {
        let mut prepared = PreparedBuffer::prepare(BufferInput {
            width: 8,
            height: 12,
        });
        prepared.clear_dirty();
        // Deterministic non-trivial dirty pattern: runs and gaps.
        let dirty_rows = [1usize, 2, 5, 8, 9, 10];
        for &y in &dirty_rows {
            prepared.set_cell(0, u16::try_from(y).unwrap(), "x");
        }
        let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(8, 12));

        // dirty_row_count matches the number of dirty rows.
        assert_eq!(layout.dirty_row_count, dirty_rows.len());

        // Sum of rect heights == dirty row count (no double counting).
        let total_height: u16 = layout.damage_rects.iter().map(|r| r.height).sum();
        assert_eq!(usize::from(total_height), dirty_rows.len());

        // Rects are sorted, non-overlapping, full-width, within bounds.
        let mut prev_end = 0u16;
        for r in &layout.damage_rects {
            assert_eq!(r.x, 0, "rect {r:?} does not start at x=0");
            assert_eq!(r.width, 8, "rect {r:?} is not full-width");
            assert!(
                r.y >= prev_end,
                "rect {r:?} overlaps the previous one (prev_end={prev_end})"
            );
            assert!(r.y + r.height <= 12, "rect {r:?} exceeds grid height");
            prev_end = r.y + r.height;
        }

        // The union of rects covers exactly the dirty rows.
        let mut covered: Vec<usize> = Vec::new();
        for r in &layout.damage_rects {
            for y in r.y..r.y + r.height {
                covered.push(usize::from(y));
            }
        }
        let mut expected: Vec<usize> = dirty_rows.to_vec();
        expected.sort_unstable();
        covered.sort_unstable();
        assert_eq!(covered, expected);
    }

    #[test]
    fn invariant_many_set_cell_patterns_never_panic() {
        for &height in &[0u16, 1, 4, 10] {
            for &width in &[0u16, 1, 5] {
                let mut prepared = PreparedBuffer::prepare(BufferInput { width, height });
                prepared.clear_dirty();
                // Set cells at and beyond bounds — must never panic.
                for y in 0..=height + 2 {
                    for x in 0..=width + 2 {
                        prepared.set_cell(x, y, "z");
                    }
                }
                let layout = PreparedBuffer::layout(&prepared, LayoutCtx::new(width, height));
                // dirty_row_count cannot exceed the grid height.
                assert!(
                    layout.dirty_row_count <= usize::from(height),
                    "h={height} w={width}: dirty_row_count {} > height",
                    layout.dirty_row_count
                );
                let total_height: u16 = layout.damage_rects.iter().map(|r| r.height).sum();
                assert_eq!(
                    usize::from(total_height),
                    layout.dirty_row_count,
                    "h={height} w={width}: rect heights != dirty_row_count"
                );
            }
        }
    }

    // ---------- derive / clone behavior ----------

    #[test]
    fn clone_and_partial_eq_hold_for_state_and_layout() {
        let state = PreparedBuffer::prepare(BufferInput {
            width: 3,
            height: 2,
        });
        assert_eq!(state.clone(), state);

        let layout = PreparedBuffer::layout(&state, LayoutCtx::new(3, 2));
        assert_eq!(layout.clone(), layout);
    }

    #[test]
    fn buffer_cell_blank_is_space_default_is_empty() {
        let blank = BufferCell::blank();
        assert_eq!(blank.symbol, " ");
        // Documented distinction: the derived Default yields an empty string.
        let default = BufferCell::default();
        assert_eq!(default.symbol, "");
        assert_ne!(blank, default);
    }

    #[test]
    fn buffer_input_is_copy_clone_eq() {
        let a = BufferInput {
            width: 10,
            height: 5,
        };
        let b = a; // Copy (Copy implies Clone)
        assert_eq!(a, b);
        let d = BufferInput {
            width: 9,
            height: 5,
        };
        assert_ne!(a, d);
    }

    #[test]
    fn buffer_layout_default_is_empty() {
        let layout = BufferLayout::default();
        assert!(layout.damage_rects.is_empty());
        assert_eq!(layout.dirty_row_count, 0);
    }
}
