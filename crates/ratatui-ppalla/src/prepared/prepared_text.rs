//! # `PreparedText` — grapheme-segmented, width-cached text primitive.
//!
//! Flagship concrete implementation of the [Pretext](https://github.com/0xradical/Pretext)
//! prepare/layout separation for text. The cold path
//! ([`PreparedText::prepare`] / [`PreparedText::append`]) splits the input on
//! `'\n'`, segments each line into extended grapheme clusters, and caches every
//! cluster's Unicode display width. The hot path ([`PreparedText::layout`])
//! then wraps those cached segments into display lines using **pure
//! arithmetic** — no Unicode work — and windows the result by scroll/height.
//!
//! Because every grapheme's cell width is fixed and known up front, the hot
//! path is a tight loop of integer comparisons, matching the "ppalla" (빨라,
//! "fast" in Korean) value proposition of ratatui-ppalla.
//!
//! # Grapheme width model
//!
//! A grapheme cluster's width is computed with `unicode_width` and then
//! **capped at 2**. Terminals render a single grapheme cluster in at most two
//! cells; without the cap, a ZWJ emoji sequence such as `"👨‍👩‍👧"` would sum to
//! 6 (three width-2 code points joined by zero-width joiners) instead of the 2
//! cells it actually occupies.

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// One grapheme cluster with its cached Unicode display width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSegment {
    /// The grapheme cluster string.
    pub grapheme: String,
    /// Cached Unicode display width (0, 1, or 2).
    pub width: u16,
}

/// One logical line: the grapheme segments of a single `'\n'`-delimited line,
/// plus its total display width.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogicalLine {
    /// Grapheme segments, left to right.
    pub segments: Vec<TextSegment>,
    /// Sum of segment widths.
    pub total_width: usize,
}

/// Prepared (cold-path) state: logical lines with cached per-grapheme widths.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedTextState {
    /// One entry per `'\n'`-delimited logical line. Empty input yields an
    /// empty vector.
    pub lines: Vec<LogicalLine>,
}

/// One display line produced by wrapping a logical line to `ctx.width`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DisplayLine {
    /// Grapheme segments that fit on this display line.
    pub segments: Vec<TextSegment>,
    /// Display width consumed by this line (sum of segment widths).
    pub width: usize,
}

/// Per-frame layout result: the visible display lines (wrapped + windowed),
/// plus the total wrapped-line count for scroll clamping.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextLayout {
    /// Visible display lines — already wrapped to `ctx.width` and windowed by
    /// `ctx.scroll .. ctx.scroll + ctx.height`.
    pub lines: Vec<DisplayLine>,
    /// Total number of display lines the whole text produces when wrapped to
    /// `ctx.width` (before windowing). Use this to clamp scroll.
    pub total_lines: usize,
}

impl PreparedTextState {
    /// Number of logical lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Total number of grapheme segments across all logical lines.
    #[must_use]
    pub fn grapheme_count(&self) -> usize {
        self.lines.iter().map(|line| line.segments.len()).sum()
    }
}

/// Prepared text primitive using the prepare/layout separation.
///
/// Implements [`Preparable`]. The input is a [`String`] (the full text).
/// [`Preparable::prepare`] splits on `'\n'` into logical lines, segments each
/// into grapheme clusters, and caches each cluster's Unicode width.
/// [`Preparable::layout`] wraps lines to `ctx.width` using only the cached
/// widths (no Unicode work) and windows by scroll/height.
///
/// `ctx.focus` is ignored by text layout (text has no per-row selection); it is
/// accepted only to satisfy the [`LayoutCtx`] contract.
///
/// # Examples
///
/// ```
/// use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedText};
///
/// let prepared = PreparedText::prepare("hello world".to_string());
/// let layout = PreparedText::layout(&prepared, LayoutCtx::new(5, 3));
/// // "hello world" wrapped to width 5 => "hello", " worl", "d" => 3 lines.
/// assert_eq!(layout.total_lines, 3);
/// assert_eq!(layout.lines.len(), 3);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PreparedText;

impl PreparedText {
    /// Convenience wrapper around [`Preparable::prepare`] that borrows a
    /// `&str`, avoiding the caller's `.to_string()`.
    #[must_use]
    pub fn prepare_str(text: &str) -> PreparedTextState {
        Self::prepare(text.to_string())
    }
}

impl Preparable for PreparedText {
    type Prepared = PreparedTextState;
    type Layout = TextLayout;
    type Input = String;

    fn prepare(input: Self::Input) -> Self::Prepared {
        PreparedTextState {
            lines: segment_text(&input),
        }
    }

    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        if more.is_empty() {
            return;
        }
        prepared.lines.extend(segment_text(&more));
    }

    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
        // WHY windowing the clone: the hot path must walk ALL logical lines to
        // compute `total_lines` (needed for scroll clamping), but cloning each
        // grapheme `String` is the dominant cost — ~80 000 heap allocations per
        // frame for a full document, when only the visible window (e.g. 24 rows)
        // is rendered.  Instead of cloning into an owned `Vec<TextSegment>` for
        // every display line, we accumulate cheap `&TextSegment` borrows and
        // defer the clone to `observe`, which materialises owned segments only
        // when a display line's global index lands inside `[start, end)`.  Lines
        // outside the window still increment `total_lines` but skip all
        // allocation, so counting and collecting share one code path and the
        // global display index can never desync.
        let effective = usize::from(ctx.width).max(1);
        let start = ctx.scroll;
        let end = start.saturating_add(usize::from(ctx.height));

        let mut visible: Vec<DisplayLine> = Vec::new();
        let mut total = 0usize;
        // Borrow-only scratch buffer, reused across logical lines via `clear()`
        // (no per-line reallocation).  Stores references into `prepared` so the
        // walk is allocation-free; cloning happens only in `observe`.
        let mut current: Vec<&TextSegment> = Vec::new();
        let mut current_width: usize;

        for line in &prepared.lines {
            current.clear();
            current_width = 0;
            let mut emitted = false;

            if line.segments.is_empty() {
                // An empty logical line ("") contributes exactly one empty
                // display line.
                observe(
                    &mut total,
                    &mut visible,
                    start,
                    end,
                    &current,
                    current_width,
                );
                continue;
            }

            for seg in &line.segments {
                let w = usize::from(seg.width);
                if w == 0 || current_width + w <= effective {
                    // Zero-width segments always attach without overflowing;
                    // positive-width segments attach while they still fit.
                    if w != 0 {
                        current_width += w;
                    }
                    current.push(seg);
                } else if current.is_empty() {
                    // Over-wide grapheme on a fresh line (w > effective): it
                    // can never fit, so skip it to avoid an infinite loop.
                } else {
                    // The current line is full: flush it, then try to place
                    // the segment on a fresh line.
                    observe(
                        &mut total,
                        &mut visible,
                        start,
                        end,
                        &current,
                        current_width,
                    );
                    emitted = true;
                    current.clear();
                    current_width = 0;
                    if w <= effective {
                        current.push(seg);
                        current_width = w;
                    }
                    // else: over-wide on a fresh line — skip.
                }
            }

            if !current.is_empty() {
                observe(
                    &mut total,
                    &mut visible,
                    start,
                    end,
                    &current,
                    current_width,
                );
                emitted = true;
            }

            if !emitted {
                // Every segment was skipped (e.g. a lone wide grapheme at a
                // width-1 column): the logical line still counts as exactly one
                // (empty) display line.
                observe(
                    &mut total,
                    &mut visible,
                    start,
                    end,
                    &current,
                    current_width,
                );
            }
        }

        TextLayout {
            lines: visible,
            total_lines: total,
        }
    }
}

/// Segment a string of text into logical lines (split on `'\n'`), each line
/// into grapheme clusters, caching each cluster's width.
fn segment_text(text: &str) -> Vec<LogicalLine> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n').map(segment_line).collect()
}

/// Segment a single (already `'\n'`-split) line into grapheme-width segments.
fn segment_line(line: &str) -> LogicalLine {
    let mut total_width = 0usize;
    let segments = line
        .graphemes(true)
        .map(|grapheme| {
            let width = grapheme_width(grapheme);
            total_width += usize::from(width);
            TextSegment {
                grapheme: grapheme.to_string(),
                width,
            }
        })
        .collect();
    LogicalLine {
        segments,
        total_width,
    }
}

/// Compute a single grapheme cluster's terminal display width, capped at 2.
fn grapheme_width(grapheme: &str) -> u16 {
    let raw = UnicodeWidthStr::width(grapheme);
    u16::try_from(raw.min(2)).unwrap_or(0)
}

/// Account for one display line: always bump the running `total`, and — only
/// when the line's global index falls inside the visible window `[start, end)`
/// — clone the borrowed segments into an owned [`DisplayLine`].  Outside the
/// window no segment is ever cloned, which is the key allocation saving.
fn observe(
    total: &mut usize,
    visible: &mut Vec<DisplayLine>,
    start: usize,
    end: usize,
    refs: &[&TextSegment],
    width: usize,
) {
    let idx = *total;
    *total += 1;
    if idx >= start && idx < end {
        visible.push(DisplayLine {
            segments: refs.iter().copied().cloned().collect(),
            width,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ---------- prepare ----------

    #[test]
    fn prepare_single_ascii_line() {
        let state = PreparedText::prepare("hello".to_string());
        assert_eq!(state.line_count(), 1);
        let line = &state.lines[0];
        assert_eq!(line.segments.len(), 5);
        assert!(line.segments.iter().all(|s| s.width == 1));
        assert_eq!(line.total_width, 5);
        assert_eq!(line.segments[0].grapheme, "h");
        assert_eq!(line.segments[4].grapheme, "o");
    }

    #[test]
    fn prepare_multi_line_widths() {
        let state = PreparedText::prepare("a\nbb\nccc".to_string());
        assert_eq!(state.line_count(), 3);
        assert_eq!(state.lines[0].total_width, 1);
        assert_eq!(state.lines[1].total_width, 2);
        assert_eq!(state.lines[2].total_width, 3);
        assert_eq!(state.lines[0].segments.len(), 1);
        assert_eq!(state.lines[1].segments.len(), 2);
        assert_eq!(state.lines[2].segments.len(), 3);
    }

    #[test]
    fn prepare_empty_string_is_zero_lines() {
        let state = PreparedText::prepare(String::new());
        assert!(state.lines.is_empty());
        assert_eq!(state.line_count(), 0);
        assert_eq!(state.grapheme_count(), 0);
    }

    #[test]
    fn prepare_trailing_newline_keeps_empty_line() {
        let state = PreparedText::prepare("a\n".to_string());
        assert_eq!(state.line_count(), 2);
        assert_eq!(state.lines[0].segments.len(), 1);
        assert!(state.lines[1].segments.is_empty());
        assert_eq!(state.lines[1].total_width, 0);
    }

    #[test]
    fn prepare_combining_acute_is_single_segment_width_one() {
        // "e" + combining acute => one grapheme cluster, width 1.
        let state = PreparedText::prepare("e\u{0301}".to_string());
        assert_eq!(state.line_count(), 1);
        let line = &state.lines[0];
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].width, 1);
        assert_eq!(line.segments[0].grapheme, "e\u{0301}");
        assert_eq!(line.total_width, 1);
    }

    #[test]
    fn prepare_family_emoji_zwj_is_single_segment_width_two() {
        // ZWJ family sequence => one grapheme cluster, capped to width 2.
        let state = PreparedText::prepare("👨‍👩‍👧".to_string());
        assert_eq!(state.line_count(), 1);
        let line = &state.lines[0];
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].width, 2);
        assert_eq!(line.total_width, 2);
    }

    #[test]
    fn prepare_cjk_width_two() {
        let state = PreparedText::prepare("中".to_string());
        let line = &state.lines[0];
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].width, 2);
        assert_eq!(line.total_width, 2);
    }

    #[test]
    fn prepare_emoji_width_two() {
        let state = PreparedText::prepare("🌟".to_string());
        let line = &state.lines[0];
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].width, 2);
    }

    #[test]
    fn prepare_ascii_width_one() {
        let state = PreparedText::prepare("A".to_string());
        assert_eq!(state.lines[0].segments[0].width, 1);
    }

    #[test]
    fn prepare_standalone_zwj_is_zero_width() {
        let state = PreparedText::prepare("\u{200d}".to_string());
        let line = &state.lines[0];
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].width, 0);
        assert_eq!(line.total_width, 0);
    }

    // ---------- layout ----------

    #[test]
    fn layout_hello_world_wraps_to_three_lines() {
        let prepared = PreparedText::prepare("hello world".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(5, 3));
        assert_eq!(layout.total_lines, 3);
        assert_eq!(layout.lines.len(), 3);
        // "hello"
        assert_eq!(layout.lines[0].width, 5);
        assert_eq!(layout.lines[0].segments.len(), 5);
        // " worl"
        assert_eq!(layout.lines[1].width, 5);
        assert_eq!(layout.lines[1].segments.len(), 5);
        assert_eq!(layout.lines[1].segments[0].grapheme, " ");
        // "d"
        assert_eq!(layout.lines[2].width, 1);
        assert_eq!(layout.lines[2].segments.len(), 1);
        // No line exceeds the effective width.
        assert!(layout.lines.iter().all(|l| l.width <= 5));
    }

    #[test]
    fn layout_windowing_with_scroll() {
        let prepared = PreparedText::prepare("hello world".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(5, 1).with_scroll(1));
        assert_eq!(layout.total_lines, 3);
        assert_eq!(layout.lines.len(), 1);
        // The 2nd display line: " worl".
        assert_eq!(layout.lines[0].width, 5);
        assert_eq!(layout.lines[0].segments[0].grapheme, " ");
    }

    #[test]
    fn layout_height_clipping() {
        let prepared = PreparedText::prepare("hello world".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(5, 2));
        assert_eq!(layout.total_lines, 3);
        assert_eq!(layout.lines.len(), 2);
    }

    #[test]
    fn layout_scroll_beyond_total_is_empty_no_panic() {
        let prepared = PreparedText::prepare("hello world".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(5, 10).with_scroll(999));
        assert_eq!(layout.total_lines, 3);
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn layout_height_zero_collects_nothing() {
        let prepared = PreparedText::prepare("hello world".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(5, 0));
        assert_eq!(layout.total_lines, 3);
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn layout_width_zero_does_not_panic() {
        let prepared = PreparedText::prepare("abc".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(0, 10));
        // Effective width coerces to 1; each ASCII grapheme fits one per line.
        assert_eq!(layout.total_lines, 3);
        assert!(layout.lines.iter().all(|l| l.width <= 1));
    }

    #[test]
    fn layout_wide_grapheme_at_width_one_is_skipped() {
        // A lone width-2 grapheme at effective width 1 can never fit, so it is
        // skipped; the logical line still counts as one (empty) display line.
        let prepared = PreparedText::prepare("中".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(1, 10));
        assert_eq!(layout.total_lines, 1);
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.lines[0].segments.is_empty());
        assert_eq!(layout.lines[0].width, 0);
    }

    #[test]
    fn layout_focus_is_ignored() {
        let prepared = PreparedText::prepare("hello\nworld".to_string());
        let without = PreparedText::layout(&prepared, LayoutCtx::new(80, 5));
        let with_focus = PreparedText::layout(&prepared, LayoutCtx::new(80, 5).with_focus(0));
        assert_eq!(without, with_focus);
    }

    #[test]
    fn layout_empty_input_is_empty() {
        let prepared = PreparedText::prepare(String::new());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(80, 24));
        assert!(layout.lines.is_empty());
        assert_eq!(layout.total_lines, 0);
    }

    #[test]
    fn layout_empty_logical_line_produces_one_display_line() {
        let prepared = PreparedText::prepare("a\n\nb".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(layout.total_lines, 3);
        // Middle line is empty.
        assert!(layout.lines[1].segments.is_empty());
        assert_eq!(layout.lines[1].width, 0);
    }

    #[test]
    fn layout_zero_width_segment_attaches_without_overflow() {
        // Combine a base char with a standalone zero-width segment on a line.
        let prepared = PreparedText::prepare("a\u{200d}b".to_string());
        let layout = PreparedText::layout(&prepared, LayoutCtx::new(2, 5));
        // All three graphemes fit within width 2 (zero-width adds nothing).
        assert_eq!(layout.total_lines, 1);
        assert_eq!(layout.lines[0].width, 2);
    }

    // ---------- append ----------

    #[test]
    fn append_extends_lines() {
        let mut prepared = PreparedText::prepare("a\nb".to_string());
        assert_eq!(prepared.line_count(), 2);
        PreparedText::append(&mut prepared, "c\nd".to_string());
        assert_eq!(prepared.line_count(), 4);
        assert_eq!(prepared.lines[2].segments[0].grapheme, "c");
        assert_eq!(prepared.lines[3].segments[0].grapheme, "d");
    }

    #[test]
    fn append_empty_is_unchanged() {
        let mut prepared = PreparedText::prepare("a\nb".to_string());
        let before = prepared.clone();
        PreparedText::append(&mut prepared, String::new());
        assert_eq!(prepared, before);
    }

    #[test]
    fn preparable_workflow_prepare_layout_append_relayout() {
        let mut prepared = PreparedText::prepare("a\nb".to_string());
        let first = PreparedText::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(first.total_lines, 2);

        PreparedText::append(&mut prepared, "c\nd".to_string());
        let second = PreparedText::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(second.total_lines, 4);
    }

    // ---------- convenience / helpers ----------

    #[test]
    fn prepare_str_equals_prepare() {
        let via_str = PreparedText::prepare_str("hi\nthere");
        let via_string = PreparedText::prepare("hi\nthere".to_string());
        assert_eq!(via_str, via_string);
    }

    #[test]
    fn helpers_line_count_and_grapheme_count() {
        let state = PreparedText::prepare_str("ab\ncde");
        assert_eq!(state.line_count(), 2);
        assert_eq!(state.grapheme_count(), 5);
    }

    #[test]
    fn clone_equality_for_state_and_layout() {
        let state = PreparedText::prepare_str("hello");
        assert_eq!(state.clone(), state);

        let layout = PreparedText::layout(&state, LayoutCtx::new(3, 2));
        assert_eq!(layout.clone(), layout);
    }

    // ---------- invariant loops (manual, no proptest macros) ----------

    #[test]
    fn invariant_total_lines_ge_logical_lines_and_widths_bounded() {
        let sample = "Hello\n世界\n🌟\ne\u{0301}b\n";
        let prepared = PreparedText::prepare(sample.to_string());
        let logical_lines = prepared.line_count();

        for width in 1..=10u16 {
            let layout = PreparedText::layout(&prepared, LayoutCtx::new(width, 100));
            assert!(
                layout.total_lines >= logical_lines,
                "width={width}: total_lines {} < logical_lines {logical_lines}",
                layout.total_lines
            );
            assert!(
                layout.lines.iter().all(|l| l.width <= usize::from(width)),
                "width={width}: a display line exceeds the effective width"
            );
        }
    }

    #[test]
    fn invariant_many_inputs_never_panic() {
        let inputs = [
            "",
            "a",
            "hello world",
            "line1\nline2\nline3",
            "中",
            "🌟",
            "👨‍👩‍👧",
            "e\u{0301}",
            "\u{200d}",
            "a\n",
            "\n",
            "mixed 中🌟 text\n世界",
            "long line without any newlines that should wrap many times",
        ];
        for input in inputs {
            let prepared = PreparedText::prepare(input.to_string());
            for width in [1, 5, 10, 40] {
                for height in [0, 1, 5] {
                    for scroll in [0, 1, 100] {
                        let layout = PreparedText::layout(
                            &prepared,
                            LayoutCtx::new(width, height).with_scroll(scroll),
                        );
                        // Sanity: visible window cannot exceed the height cap.
                        assert!(layout.lines.len() <= usize::from(height));
                        // total_lines is consistent with the input shape.
                        assert!(layout.total_lines >= prepared.line_count());
                    }
                }
            }
        }
    }

    // ---------- differential correctness ----------

    /// Brute-force reference layout: materialise EVERY display line (always
    /// cloning, no windowing).  The differential test then slices
    /// `[scroll, scroll + height)` and compares against the optimised
    /// (windowed-clone) `layout`.  Deliberately simple so it is obviously
    /// correct — it is the ground truth the optimised path is checked against.
    fn reference_layout(prepared: &PreparedTextState, width: u16) -> Vec<DisplayLine> {
        let effective = usize::from(width).max(1);
        let mut all: Vec<DisplayLine> = Vec::new();
        for line in &prepared.lines {
            let mut current: Vec<TextSegment> = Vec::new();
            let mut current_width = 0usize;
            let mut emitted = false;

            if line.segments.is_empty() {
                all.push(DisplayLine::default());
                continue;
            }

            for seg in &line.segments {
                let w = usize::from(seg.width);
                if w == 0 || current_width + w <= effective {
                    if w != 0 {
                        current_width += w;
                    }
                    current.push(seg.clone());
                } else if current.is_empty() {
                    // Over-wide grapheme on a fresh line — skip.
                } else {
                    let flushed = std::mem::take(&mut current);
                    all.push(DisplayLine {
                        segments: flushed,
                        width: current_width,
                    });
                    emitted = true;
                    current_width = 0;
                    if w <= effective {
                        current.push(seg.clone());
                        current_width = w;
                    }
                }
            }

            if !current.is_empty() {
                let flushed = std::mem::take(&mut current);
                all.push(DisplayLine {
                    segments: flushed,
                    width: current_width,
                });
                emitted = true;
            }

            if !emitted {
                all.push(DisplayLine::default());
            }
        }
        all
    }

    /// Differential test: the windowed-clone `layout` must produce byte-
    /// identical output (both `total_lines` and every visible `DisplayLine`)
    /// compared to the brute-force reference over a broad matrix of inputs,
    /// widths, scrolls, and heights — including the long-line case where the
    /// visible window splits a single logical line mid-wrap.
    #[test]
    fn differential_layout_matches_reference_over_matrix() {
        let inputs: Vec<String> = vec![
            "hello world".to_string(),
            "a".repeat(200),
            "世界🌟中".to_string(),
            "👨‍👩‍👧".to_string(),
            "a\u{200d}b".to_string(),
            "中".to_string(),
            String::new(),
            "x\n".to_string(),
            "a\nbb\nccc".to_string(),
        ];
        for input in &inputs {
            let prepared = PreparedText::prepare_str(input);
            for &width in &[1u16, 2, 3, 5, 10, 80] {
                let reference = reference_layout(&prepared, width);
                for &scroll in &[0usize, 1, 3, 100] {
                    for &height in &[0u16, 1, 3, 5, 1000] {
                        let layout = PreparedText::layout(
                            &prepared,
                            LayoutCtx::new(width, height).with_scroll(scroll),
                        );
                        assert_eq!(
                            layout.total_lines,
                            reference.len(),
                            "total_lines mismatch: input={input:?} width={width} \
                             scroll={scroll} height={height}"
                        );
                        let lo = scroll.min(reference.len());
                        let hi = (scroll + usize::from(height)).min(reference.len());
                        assert_eq!(
                            &layout.lines[..],
                            &reference[lo..hi],
                            "visible mismatch: input={input:?} width={width} \
                             scroll={scroll} height={height}"
                        );
                    }
                }
            }
        }
    }
}
