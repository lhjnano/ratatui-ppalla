//! # `PreparedBlock` — cached border-drawing primitive (Preparable Block).
//!
//! A Preparable version of [`ratatui::widgets::Block`]: the cold path
//! ([`PreparedBlock::prepare`]) stores the block configuration; the hot path
//! ([`PreparedBlock::layout`]) computes the concrete per-cell border + title
//! glyph placement with a 1-entry cache keyed on `(width, height, border_type,
//! borders, title)`; [`BlockLayout::paint`] then writes those cached glyphs
//! into a ratatui [`Buffer`].
//!
//! ## Why it is faster than `Block`
//!
//! A plain `Block::render_widget` recomputes the border glyph positions every
//! frame. In a multi-pane TUI (e.g. N=20 panes + sidebar + footer ≈ 22 blocks
//! per frame) the dimensions and title almost never change between frames, so
//! the work is pure waste. `PreparedBlock` caches the placement: when nothing
//! relevant changed, [`Preparable::layout`] is a cache hit (a single key
//! comparison) and [`BlockLayout::paint`] is just a tight loop of `set_symbol`
//! calls — no recomputation.
//!
//! Crucially, the cache key does **not** include the border/title *style*
//! (foreground/background color). Styles are applied at `paint` time, so
//! toggling a pane's focus color (a very frequent event) does **not** invalidate
//! the layout cache.

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols::border::Set;
use ratatui::text::Line;
use ratatui::widgets::{BorderType, Borders};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// Block configuration: which sides to draw, the corner/edge glyph style, an
/// optional title, and the styles applied to the border glyphs and the title.
/// Cheap to clone.
#[derive(Debug, Clone)]
pub struct BlockSpec {
    /// Which sides to draw (bitflags: `TOP`/`BOTTOM`/`LEFT`/`RIGHT`).
    pub borders: Borders,
    /// Corner/edge glyph style.
    pub border_type: BorderType,
    /// Optional title line (drawn at the top-left of the top border).
    pub title: Option<Line<'static>>,
    /// Style applied to the border glyphs at paint time.
    pub border_style: Style,
    /// Style applied to the title glyphs at paint time.
    pub title_style: Style,
}

impl BlockSpec {
    /// Create a plain block (all borders, plain glyphs) with the given title.
    #[must_use]
    pub fn titled(title: impl Into<Line<'static>>) -> Self {
        Self {
            borders: Borders::ALL,
            border_type: BorderType::Plain,
            title: Some(title.into()),
            border_style: Style::default(),
            title_style: Style::default(),
        }
    }

    /// Create a plain block with all borders and no title.
    #[must_use]
    pub fn bordered() -> Self {
        Self {
            borders: Borders::ALL,
            border_type: BorderType::Plain,
            title: None,
            border_style: Style::default(),
            title_style: Style::default(),
        }
    }

    /// Set the border glyph style (builder).
    #[must_use]
    pub const fn border_type(mut self, border_type: BorderType) -> Self {
        self.border_type = border_type;
        self
    }

    /// Set which sides to draw (builder).
    #[must_use]
    pub const fn borders(mut self, borders: Borders) -> Self {
        self.borders = borders;
        self
    }

    /// Set the border style (builder).
    #[must_use]
    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    /// Set the title style (builder).
    #[must_use]
    pub fn title_style(mut self, style: Style) -> Self {
        self.title_style = style;
        self
    }
}

impl Default for BlockSpec {
    fn default() -> Self {
        Self::bordered()
    }
}

/// One cell to paint: a position relative to the block's area origin, the glyph
/// to write, and whether it is a title cell (so `paint` can apply the title
/// style rather than the border style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderCell {
    /// Column offset within the area (0-based).
    pub x: u16,
    /// Row offset within the area (0-based).
    pub y: u16,
    /// The glyph to write.
    pub symbol: String,
    /// `true` if this cell is part of the title (uses the title style).
    pub is_title: bool,
}

/// Per-frame layout result: the concrete border + title cells to paint, plus the
/// inner (content) rectangle (the area inset by the drawn borders), plus a
/// cache-hit flag.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockLayout {
    /// Cells to paint to render the borders + title.
    pub cells: Vec<BorderCell>,
    /// The rectangle inside the borders (where content goes).
    pub inner: Rect,
    /// `true` if this layout came from the 1-entry cache (no recomputation).
    pub cache_hit: bool,
}

/// Prepared state: the spec plus a 1-entry cache of the last computed layout.
///
/// `Clone` is manual because [`Mutex`] is not `Clone` — cloning locks the cache
/// and copies its contents into a fresh mutex (mirrors `PreparedLayoutState`).
#[derive(Debug, Default)]
pub struct PreparedBlockState {
    /// The configured block specification.
    pub spec: BlockSpec,
    cache: Mutex<BlockCache>,
}

impl Clone for PreparedBlockState {
    fn clone(&self) -> Self {
        Self {
            spec: self.spec.clone(),
            cache: Mutex::new(
                self.cache
                    .lock()
                    .expect("block cache mutex should never be poisoned")
                    .clone(),
            ),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BlockCache {
    key: Option<BlockCacheKey>,
    layout: BlockLayout,
}

/// Cache key: everything that determines the *placement* of border/title glyphs.
/// Note: border/title **styles** are deliberately excluded — they are applied at
/// paint time, so changing focus color does not invalidate the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BlockCacheKey {
    width: u16,
    height: u16,
    /// `BorderType` discriminant + `Borders` bits packed together.
    glyph_sig: u32,
    /// Hash of the title's plain text (0 if no title).
    title_sig: u64,
}

/// `PreparedBlock` — border-drawing primitive using the prepare/layout separation
/// with a 1-entry cache keyed on `(width, height, border_type, borders, title)`.
#[derive(Debug, Clone, Default)]
pub struct PreparedBlock;

impl Preparable for PreparedBlock {
    type Prepared = PreparedBlockState;
    type Layout = BlockLayout;
    type Input = BlockSpec;

    fn prepare(input: Self::Input) -> Self::Prepared {
        PreparedBlockState {
            spec: input,
            cache: Mutex::new(BlockCache::default()),
        }
    }

    /// Replace the spec and invalidate the cache. A block's "more" is a new
    /// configuration (e.g. a new title), so appending replaces and resets.
    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        prepared.spec = more;
        // `&mut` access lets us replace the mutex wholesale without locking.
        prepared.cache = Mutex::new(BlockCache::default());
    }

    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
        let key = BlockCacheKey {
            width: ctx.width,
            height: ctx.height,
            glyph_sig: glyph_signature(prepared.spec.border_type, prepared.spec.borders),
            title_sig: title_signature(prepared.spec.title.as_ref()),
        };

        let mut cache = prepared
            .cache
            .lock()
            .expect("block cache mutex should never be poisoned");

        // Cache hit: same dimensions + glyph config + title → reuse.
        if cache.key == Some(key) {
            let mut layout = cache.layout.clone();
            layout.cache_hit = true;
            return layout;
        }

        // Cache miss: recompute the placement.
        let layout = compute_border_layout(
            &prepared.spec,
            ctx.width,
            ctx.height,
            prepared.spec.border_type.to_border_set(),
        );
        *cache = BlockCache {
            key: Some(key),
            layout: layout.clone(),
        };
        layout
    }
}

impl PreparedBlockState {
    /// Convenience: prepare a plain titled block in one call.
    #[must_use]
    pub fn titled(title: impl Into<Line<'static>>) -> Self {
        PreparedBlock::prepare(BlockSpec::titled(title))
    }
}

impl BlockLayout {
    /// Paint the cached border + title cells into `buf` within `area`, applying
    /// `border_style` to border glyphs and `title_style` to title glyphs.
    ///
    /// Each cell is positioned at `(area.x + cell.x, area.y + cell.y)`. Cells
    /// outside the buffer are skipped.
    pub fn paint(&self, buf: &mut Buffer, area: Rect, border_style: Style, title_style: Style) {
        for cell in &self.cells {
            let Some(x) = area.x.checked_add(cell.x) else {
                continue;
            };
            let Some(y) = area.y.checked_add(cell.y) else {
                continue;
            };
            let Some(buf_cell) = buf.cell_mut((x, y)) else {
                continue;
            };
            buf_cell.set_symbol(&cell.symbol);
            buf_cell.set_style(if cell.is_title {
                title_style
            } else {
                border_style
            });
        }
    }
}

/// Compute the border + title cell placement for the given spec and dimensions.
#[allow(clippy::too_many_lines)]
fn compute_border_layout(spec: &BlockSpec, width: u16, height: u16, set: Set) -> BlockLayout {
    let mut cells: Vec<BorderCell> = Vec::new();
    let borders = spec.borders;

    let has_top = borders.contains(Borders::TOP);
    let has_bottom = borders.contains(Borders::BOTTOM);
    let has_left = borders.contains(Borders::LEFT);
    let has_right = borders.contains(Borders::RIGHT);

    // Horizontal edges run between the corners (1..width-1).
    let last_x = width.saturating_sub(1);
    let last_y = height.saturating_sub(1);

    if has_top && height > 0 {
        if has_left && width > 0 {
            cells.push(BorderCell {
                x: 0,
                y: 0,
                symbol: set.top_left.to_string(),
                is_title: false,
            });
        }
        if has_right && width > 0 {
            cells.push(BorderCell {
                x: last_x,
                y: 0,
                symbol: set.top_right.to_string(),
                is_title: false,
            });
        }
        for x in 1..last_x {
            cells.push(BorderCell {
                x,
                y: 0,
                symbol: set.horizontal_top.to_string(),
                is_title: false,
            });
        }
    }
    if has_bottom && height > 0 {
        if has_left && width > 0 {
            cells.push(BorderCell {
                x: 0,
                y: last_y,
                symbol: set.bottom_left.to_string(),
                is_title: false,
            });
        }
        if has_right && width > 0 {
            cells.push(BorderCell {
                x: last_x,
                y: last_y,
                symbol: set.bottom_right.to_string(),
                is_title: false,
            });
        }
        for x in 1..last_x {
            cells.push(BorderCell {
                x,
                y: last_y,
                symbol: set.horizontal_bottom.to_string(),
                is_title: false,
            });
        }
    }
    if has_left {
        for y in 1..last_y {
            cells.push(BorderCell {
                x: 0,
                y,
                symbol: set.vertical_left.to_string(),
                is_title: false,
            });
        }
    }
    if has_right {
        for y in 1..last_y {
            cells.push(BorderCell {
                x: last_x,
                y,
                symbol: set.vertical_right.to_string(),
                is_title: false,
            });
        }
    }

    // Title: drawn on the top border, starting at column 1, truncated to the
    // available interior width (width - 2 when both verticals are present).
    if let Some(title) = &spec.title {
        if has_top && height > 0 && width > 2 {
            let interior_start = u16::from(has_left);
            let interior_end = width - u16::from(has_right);
            let budget = usize::from(interior_end.saturating_sub(interior_start));
            // Flatten the title line to plain text and truncate by char count
            // (a logic-only module; full unicode-width truncation would need
            // grapheme measurement, which PreparedText already covers).
            let plain: String = title.spans.iter().flat_map(|s| s.content.chars()).collect();
            let truncated: String = plain.chars().take(budget).collect();
            for (i, ch) in truncated.chars().enumerate() {
                let Ok(iu16) = u16::try_from(i) else {
                    break;
                };
                cells.push(BorderCell {
                    x: interior_start + iu16,
                    y: 0,
                    symbol: ch.to_string(),
                    is_title: true,
                });
            }
        }
    }

    // Inner rectangle: inset by the drawn borders on each side.
    let inner_x = u16::from(has_left);
    let inner_y = u16::from(has_top);
    let inner_w = width
        .saturating_sub(u16::from(has_left))
        .saturating_sub(u16::from(has_right));
    let inner_h = height
        .saturating_sub(u16::from(has_top))
        .saturating_sub(u16::from(has_bottom));
    let inner = Rect::new(inner_x, inner_y, inner_w, inner_h);

    BlockLayout {
        cells,
        inner,
        cache_hit: false,
    }
}

/// Pack `BorderType` discriminant + `Borders` bits into a single `u32` glyph
/// signature. Stable because `BorderType` and `Borders` have fixed reprs.
fn glyph_signature(border_type: BorderType, borders: Borders) -> u32 {
    let bt = match border_type {
        BorderType::Plain => 0u32,
        BorderType::Rounded => 1,
        BorderType::Double => 2,
        BorderType::Thick => 3,
        BorderType::QuadrantInside => 4,
        BorderType::QuadrantOutside => 5,
    };
    let bs = u32::from(borders.bits());
    (bt << 8) | bs
}

/// Hash the title's plain text into a stable `u64` (0 when there is no title).
fn title_signature(title: Option<&Line<'_>>) -> u64 {
    let Some(line) = title else {
        return 0;
    };
    let mut hasher = DefaultHasher::new();
    for span in &line.spans {
        span.content.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use ratatui::style::Color;

    fn plain_spec() -> BlockSpec {
        BlockSpec::bordered()
    }

    #[test]
    fn prepare_stores_spec_and_cache_is_cold() {
        let state = PreparedBlock::prepare(plain_spec());
        assert_eq!(state.spec.borders, Borders::ALL);
        assert!(state.cache.lock().unwrap().key.is_none());
    }

    #[test]
    fn layout_all_borders_places_four_corners() {
        let state = PreparedBlock::prepare(plain_spec());
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        assert!(!layout.cache_hit);
        // Corners at (0,0), (9,0), (0,4), (9,4).
        let has = |x, y| layout.cells.iter().any(|c| c.x == x && c.y == y);
        assert!(has(0, 0), "top-left corner");
        assert!(has(9, 0), "top-right corner");
        assert!(has(0, 4), "bottom-left corner");
        assert!(has(9, 4), "bottom-right corner");
        // Inner is inset by 1 on each side.
        assert_eq!(layout.inner, Rect::new(1, 1, 8, 3));
    }

    #[test]
    fn layout_partial_borders_only_drawn_sides() {
        let spec = BlockSpec::bordered().borders(Borders::LEFT | Borders::TOP);
        let state = PreparedBlock::prepare(spec);
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        // No bottom or right edges.
        assert!(!layout.cells.iter().any(|c| c.y == 4), "no bottom border");
        assert!(
            !layout.cells.iter().any(|c| c.x == 9 && c.y != 0),
            "no right edge"
        );
    }

    #[test]
    fn layout_title_appears_on_top_border() {
        let state = PreparedBlockState::titled("hi");
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(10, 3));
        let title_cells: Vec<_> = layout.cells.iter().filter(|c| c.is_title).collect();
        assert_eq!(title_cells.len(), 2);
        assert_eq!(title_cells[0].symbol, "h");
        assert_eq!(title_cells[1].symbol, "i");
        assert_eq!(title_cells[0].y, 0);
    }

    #[test]
    fn layout_zero_size_no_panic() {
        let state = PreparedBlock::prepare(plain_spec());
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(0, 0));
        assert!(layout.cells.is_empty());
    }

    #[test]
    fn cache_hit_on_same_ctx() {
        let state = PreparedBlock::prepare(plain_spec());
        let first = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        assert!(!first.cache_hit);
        let second = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        assert!(second.cache_hit);
        assert_eq!(first.cells, second.cells);
    }

    #[test]
    fn cache_miss_on_dimension_change() {
        let state = PreparedBlock::prepare(plain_spec());
        let _ = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        let second = PreparedBlock::layout(&state, LayoutCtx::new(20, 5));
        assert!(!second.cache_hit);
    }

    #[test]
    fn cache_hit_ignores_style_change() {
        // Changing only the border_style must NOT invalidate the layout cache,
        // because styles are applied at paint time, not layout time.
        let mut state = PreparedBlock::prepare(plain_spec());
        let _ = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        // Mutate the spec's border_style in place via append-equivalent.
        state.spec = state
            .spec
            .clone()
            .border_style(Style::default().fg(Color::Red));
        state.cache = Mutex::new(BlockCache::default()); // append would do this; here simulate a style-only change WITHOUT reset
                                                         // Re-prepare to keep cache and just change style on the stored spec:
        let mut kept = PreparedBlock::prepare(plain_spec());
        let _ = PreparedBlock::layout(&kept, LayoutCtx::new(10, 5));
        kept.spec.border_style = Style::default().fg(Color::Red);
        let again = PreparedBlock::layout(&kept, LayoutCtx::new(10, 5));
        assert!(again.cache_hit, "style-only change should stay a cache hit");
    }

    #[test]
    fn cache_miss_on_title_change() {
        let mut state = PreparedBlockState::titled("a");
        let _ = PreparedBlock::layout(&state, LayoutCtx::new(10, 3));
        PreparedBlock::append(&mut state, BlockSpec::titled("b"));
        let after = PreparedBlock::layout(&state, LayoutCtx::new(10, 3));
        assert!(!after.cache_hit);
    }

    #[test]
    fn paint_draws_corners_into_buffer() {
        let state = PreparedBlock::prepare(plain_spec());
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);
        layout.paint(&mut buf, area, Style::default(), Style::default());
        // top-left corner of PLAIN set is '┌' (box-drawing).
        assert_eq!(buf[(0, 0)].symbol(), "┌");
        // a horizontal edge cell
        assert_eq!(buf[(5, 0)].symbol(), "─");
        // vertical edge
        assert_eq!(buf[(0, 2)].symbol(), "│");
    }

    #[test]
    fn paint_applies_styles() {
        let spec = BlockSpec::titled("hi").border_style(Style::default().fg(Color::Red));
        let state = PreparedBlock::prepare(spec);
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(10, 3));
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        layout.paint(
            &mut buf,
            area,
            Style::default().fg(Color::Red),
            Style::default().fg(Color::Blue),
        );
        // A border cell should carry the red fg; a title cell the blue fg.
        let border_cell = &buf[(5, 0)]; // horizontal edge (not title)
        assert_eq!(border_cell.style().fg, Some(Color::Red));
        let title_cell = &buf[(1, 0)]; // 'h'
        assert_eq!(title_cell.style().fg, Some(Color::Blue));
    }

    #[test]
    fn paint_inner_cells_untouched() {
        let state = PreparedBlock::prepare(plain_spec());
        let layout = PreparedBlock::layout(&state, LayoutCtx::new(10, 5));
        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);
        layout.paint(&mut buf, area, Style::default(), Style::default());
        // Inner cell (5,2) should still be a space (Buffer::empty default).
        assert_eq!(buf[(5, 2)].symbol(), " ");
    }

    #[test]
    fn preparedblockstate_is_clone() {
        let state = PreparedBlockState::titled("x");
        let cloned = state.clone();
        let a = PreparedBlock::layout(&state, LayoutCtx::new(10, 3));
        let b = PreparedBlock::layout(&cloned, LayoutCtx::new(10, 3));
        assert_eq!(a.cells, b.cells);
    }
}
