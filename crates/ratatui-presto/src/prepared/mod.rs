//! # Prepared primitives — Pretext-inspired prepare/layout separation.
//!
//! Core insight: separate expensive one-time work (`prepare`) from cheap
//! per-frame work (`layout`). Because every TUI text width is predetermined by
//! its Unicode width, the prepare step is extremely cheap. This is the "presto"
//! (fast) value proposition of ratatui-presto.
//!
//! ## Pattern
//!
//! Each [`Preparable`] implementation works in three stages:
//!
//! 1. [`Preparable::prepare`] — called once when data changes (cold path). May
//!    be expensive.
//! 2. [`Preparable::append`] — add incremental data (optional). Updates the
//!    prepared state. The default implementation is a no-op; only implementers
//!    for which incremental update is worthwhile override it.
//! 3. [`Preparable::layout`] — called every frame (hot path). Performs pure
//!    arithmetic over the cached prepared state. Ideally **allocation-free**.
//!
//! The prepared state must not depend on [`LayoutCtx`] — that way a different
//! per-frame context can yield a different layout result without recomputing.
//!
//! Concrete implementations live in child modules:
//!
//! - The flagship text primitive [`PreparedText`] (grapheme segmentation +
//!   Unicode width caching + visible-line wrapping) lives in [`prepared_text`].
//! - The cached layout-region primitive [`PreparedLayout`] (1-entry cache over
//!   ratatui constraint evaluation) lives in [`prepared_layout`].
//! - The damage-tracked cell-grid primitive [`PreparedBuffer`] (per-row dirty
//!   flags merged into damage rects) lives in [`prepared_buffer`].
//! - The filterable list primitive [`PreparedList`] (case-insensitive substring
//!   filter index + visible-item windowing) lives in [`prepared_list`].
//! - The sortable table primitive [`PreparedTable`] (sort permutation +
//!   column widths + visible-row windowing) lives in [`prepared_table`].
//! - The scrollable viewport primitive [`PreparedViewport`] (case-insensitive
//!   substring search + match navigation) lives in [`prepared_viewport`].
//!
//! This module defines the core trait and context.

#![allow(clippy::module_name_repetitions)]

pub mod prepared_buffer;
pub mod prepared_layout;
pub mod prepared_list;
pub mod prepared_table;
pub mod prepared_text;
pub mod prepared_viewport;

pub use prepared_buffer::{
    BufferCell, BufferInput, BufferLayout, PreparedBuffer, PreparedBufferState,
};
pub use prepared_layout::{PreparedLayout, PreparedLayoutState, SplitLayout, SplitSpec};
pub use prepared_list::{ListInput, ListLayout, PreparedList, PreparedListState, VisibleItem};
pub use prepared_table::{
    PreparedTable, PreparedTableState, SortSpec, TableColumn, TableInput, TableLayout, VisibleRow,
};
pub use prepared_text::{
    DisplayLine, LogicalLine, PreparedText, PreparedTextState, TextLayout, TextSegment,
};
pub use prepared_viewport::{
    PreparedViewport, PreparedViewportState, ViewportInput, ViewportLayout, VisibleLine,
};

/// Per-frame layout context — values that change every frame.
///
/// Passed to the hot path (`layout`). The prepared state must not depend on
/// these values (otherwise `prepare` would be meaningless). That is, giving the
/// same prepared state a different [`LayoutCtx`] should produce a different
/// layout result.
///
/// Construct via the builder methods
/// ([`with_scroll`](Self::with_scroll), [`with_focus`](Self::with_focus)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutCtx {
    /// Available width in cells.
    pub width: u16,
    /// Available height in rows.
    pub height: u16,
    /// Vertical scroll offset (rows to skip from the top).
    pub scroll: usize,
    /// Focused row index (used for highlighting, etc.). `None` means no focus.
    pub focus: Option<usize>,
}

impl LayoutCtx {
    /// Create a new [`LayoutCtx`]. Defaults to `scroll = 0`, `focus = None`.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            scroll: 0,
            focus: None,
        }
    }

    /// Set the scroll offset and return `self` (builder style).
    #[must_use]
    pub const fn with_scroll(mut self, scroll: usize) -> Self {
        self.scroll = scroll;
        self
    }

    /// Set the focus row and return `self` (builder style).
    #[must_use]
    pub const fn with_focus(mut self, focus: usize) -> Self {
        self.focus = Some(focus);
        self
    }
}

impl Default for LayoutCtx {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Core abstraction: prepare once (cold path), layout many (hot path).
///
/// The Rust expression of the [Pretext](https://github.com/0xradical/Pretext)
/// pattern. Uses associated types to achieve both type safety and zero-cost
/// abstraction (monomorphization) at once. The trait is **not** `dyn`-object-safe,
/// but it is called via static monomorphization, so there is no indirect-call /
/// virtual-dispatch overhead.
///
/// # Workflow
///
/// - [`prepare`](Preparable::prepare) / [`append`](Preparable::append) are
///   called only when data changes (cold path).
/// - [`layout`](Preparable::layout) is called every frame (hot path). It must be
///   fast over the cached prepared state — ideally allocation-free.
///
/// # Type parameters
///
/// - [`Prepared`](Self::Prepared): opaque, cheap-to-clone prepared state.
///   Computed once on the cold path and reused on the hot path.
/// - [`Layout`](Self::Layout): per-frame layout result (hot path).
/// - [`Input`](Self::Input): input that triggers a re-prepare or append.
pub trait Preparable {
    /// Opaque, cheap-to-clone prepared state (computed once on the cold path).
    type Prepared: Clone;
    /// Per-frame layout result (hot path).
    type Layout;
    /// Input that triggers a re-prepare or append.
    type Input;

    /// Compute the prepared state once from the input (cold path).
    ///
    /// Call again to recreate the prepared state whenever the data changes.
    #[must_use]
    fn prepare(input: Self::Input) -> Self::Prepared;

    /// Append incremental data to the prepared state.
    ///
    /// The default implementation is a **no-op**. Only implementers for which
    /// incremental update is meaningful override this method. When incremental
    /// update is more expensive than a full recompute, or is meaningless, leave
    /// the default no-op and call [`prepare`](Preparable::prepare) again on data
    /// change instead.
    fn append(_prepared: &mut Self::Prepared, _more: Self::Input) {}

    /// Compute the layout from the cached prepared state and per-frame context
    /// (hot path).
    ///
    /// Because this method is called every frame, it should perform only pure
    /// arithmetic without allocation. The prepared state must not depend on
    /// [`LayoutCtx`].
    #[must_use]
    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_width_and_height_and_defaults() {
        let ctx = LayoutCtx::new(80, 24);
        assert_eq!(ctx.width, 80);
        assert_eq!(ctx.height, 24);
        assert_eq!(ctx.scroll, 0);
        assert_eq!(ctx.focus, None);
    }

    #[test]
    fn default_is_zeroed() {
        let ctx = LayoutCtx::default();
        assert_eq!(ctx, LayoutCtx::new(0, 0));
        assert_eq!(ctx.width, 0);
        assert_eq!(ctx.height, 0);
    }

    #[test]
    fn with_scroll_sets_scroll() {
        let ctx = LayoutCtx::new(100, 10).with_scroll(7);
        assert_eq!(ctx.scroll, 7);
        // Other fields are preserved.
        assert_eq!(ctx.width, 100);
        assert_eq!(ctx.height, 10);
        assert_eq!(ctx.focus, None);
    }

    #[test]
    fn with_focus_sets_some() {
        let ctx = LayoutCtx::new(40, 5).with_focus(3);
        assert_eq!(ctx.focus, Some(3));
    }

    #[test]
    fn builder_chains_compose() {
        let ctx = LayoutCtx::new(60, 20).with_scroll(2).with_focus(9);
        assert_eq!(ctx.width, 60);
        assert_eq!(ctx.height, 20);
        assert_eq!(ctx.scroll, 2);
        assert_eq!(ctx.focus, Some(9));
    }

    #[test]
    fn partialeq_and_eq_hold() {
        let a = LayoutCtx::new(10, 3).with_scroll(1);
        let b = LayoutCtx::new(10, 3).with_scroll(1);
        assert_eq!(a, b);
        let c = LayoutCtx::new(10, 3).with_scroll(2);
        assert_ne!(a, c);
    }

    #[test]
    fn layoutctx_is_copy() {
        let ctx = LayoutCtx::new(5, 5).with_focus(1);
        let copied = ctx; // Copy — move is cheap, original still usable
        assert_eq!(ctx, copied);
    }

    /// Dummy implementation: keeps a running sum as the prepared state.
    /// `layout` returns whether the sum exceeds `ctx.width`. Overrides `append`
    /// to verify incremental summation.
    #[derive(Clone)]
    struct SumPreparer;

    impl Preparable for SumPreparer {
        type Prepared = u64;
        type Layout = bool;
        type Input = Vec<u32>;

        fn prepare(input: Self::Input) -> Self::Prepared {
            input.into_iter().map(u64::from).sum()
        }

        fn append(prepared: &mut Self::Prepared, more: Self::Input) {
            for v in more {
                *prepared += u64::from(v);
            }
        }

        fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
            *prepared > u64::from(ctx.width)
        }
    }

    #[test]
    fn preparable_prepare_produces_initial_state() {
        let prepared = SumPreparer::prepare(vec![1, 2, 3, 4]);
        assert_eq!(prepared, 10);
    }

    #[test]
    fn preparable_prepare_handles_empty_input() {
        let prepared = SumPreparer::prepare(Vec::new());
        assert_eq!(prepared, 0);
    }

    #[test]
    fn preparable_append_updates_state_incrementally() {
        let mut prepared = SumPreparer::prepare(vec![10]);
        SumPreparer::append(&mut prepared, vec![5, 5, 5]);
        assert_eq!(prepared, 25);
    }

    #[test]
    fn preparable_append_with_empty_input_is_no_change() {
        let mut prepared = SumPreparer::prepare(vec![7]);
        SumPreparer::append(&mut prepared, Vec::new());
        assert_eq!(prepared, 7);
    }

    #[test]
    fn preparable_layout_uses_prepared_and_ctx() {
        // sum = 30, width = 50 -> does not exceed -> false
        let prepared = SumPreparer::prepare(vec![10, 20]);
        assert!(!SumPreparer::layout(&prepared, LayoutCtx::new(50, 1)));

        // width = 25 -> exceeds -> true
        assert!(SumPreparer::layout(&prepared, LayoutCtx::new(25, 1)));
    }

    #[test]
    fn preparable_layout_does_not_mutate_prepared() {
        let mut prepared = SumPreparer::prepare(vec![100]);
        let _ = SumPreparer::layout(&prepared, LayoutCtx::new(10, 1));
        // Unchanged — layout does not mutate prepared.
        assert_eq!(prepared, 100);
        // After append, incremental update works independently of layout.
        SumPreparer::append(&mut prepared, vec![1]);
        assert_eq!(prepared, 101);
    }

    /// Implementation that relies on the default `append` (no-op). Types that do
    /// not need incremental update simply do not override `append`.
    #[derive(Clone)]
    struct ConstPreparer;

    impl Preparable for ConstPreparer {
        type Prepared = u8;
        type Layout = u8;
        type Input = u8;

        fn prepare(input: Self::Input) -> Self::Prepared {
            input
        }

        // append is not overridden -> uses the default no-op.

        fn layout(prepared: &Self::Prepared, _ctx: LayoutCtx) -> Self::Layout {
            *prepared
        }
    }

    #[test]
    fn default_append_is_noop() {
        let mut prepared = ConstPreparer::prepare(42);
        // The default append does not change prepared.
        ConstPreparer::append(&mut prepared, 99);
        assert_eq!(prepared, 42);
    }

    #[test]
    fn const_preparer_layout_echoes_prepared() {
        let prepared = ConstPreparer::prepare(7);
        assert_eq!(ConstPreparer::layout(&prepared, LayoutCtx::new(0, 0)), 7);
    }
}
