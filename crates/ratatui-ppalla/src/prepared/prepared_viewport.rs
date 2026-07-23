//! # `PreparedViewport` — scrollable viewport with search, prepare/layout separation.
//!
//! Concrete implementation of the [Pretext](https://github.com/0xradical/Pretext)
//! prepare/layout separation for a scrollable line buffer with case-insensitive
//! substring search. It is the prepared-primitive counterpart of the imperative
//! [`Viewport`](crate::viewport::Viewport): the same search/match logic, lifted
//! into the cold/hot-path split.
//!
//! - The cold path ([`PreparedViewport::prepare`] / [`PreparedViewport::append`])
//!   stores the line buffer and the active query, and precomputes the **match
//!   indices** — the line numbers whose plain text contains the query,
//!   case-insensitively.
//! - The hot path ([`PreparedViewport::layout`]) windows the buffer by
//!   scroll/height and flags which visible lines match, using only the cached
//!   match indices (no substring search per frame). It walks **only the visible
//!   window**, so lines outside the view are never touched.
//!
//! # Search model
//!
//! A query of `None` or an empty string means "no search": `match_indices` is
//! empty and no visible line is flagged. A non-empty query matches every line
//! whose lowercased text contains the lowercased query (case-insensitive
//! substring), matching [`Viewport`](crate::viewport::Viewport)'s
//! `recompute_matches`.
//!
//! # Layout inputs
//!
//! `ctx.scroll` selects the top visible row; `ctx.height` bounds the window.
//! `ctx.width` and `ctx.focus` do not affect the layout (lines are stored as
//! plain text without wrapping, and there is no per-row selection) — they are
//! accepted only to satisfy the [`LayoutCtx`](super::LayoutCtx) contract.
//!
//! # Examples
//!
//! ```
//! use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedViewport, ViewportInput};
//!
//! let input = ViewportInput {
//!     lines: vec!["find me".to_string(), "nothing".to_string(), "find me too".to_string()],
//!     query: Some("find".to_string()),
//! };
//! // "find me" and "find me too" both contain "find".
//! let prepared = PreparedViewport::prepare(input);
//! assert_eq!(prepared.match_count(), 2);
//! let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 2));
//! assert_eq!(layout.total, 3);
//! assert_eq!(layout.lines.len(), 2);
//! ```

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};

/// Input for preparing a [`PreparedViewport`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewportInput {
    /// The full line buffer (plain text per line).
    pub lines: Vec<String>,
    /// Search query; `None` or empty means no search.
    pub query: Option<String>,
}

/// Prepared (cold-path) state: the line buffer, the active query, and the cached
/// match indices (line numbers whose text contains the query, case-insensitively).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedViewportState {
    /// The line buffer.
    pub lines: Vec<String>,
    /// Active query (`None` = no search).
    pub query: Option<String>,
    /// Indices into [`lines`](Self::lines) whose text contains `query`
    /// (case-insensitive). Ascending, with no duplicates.
    pub match_indices: Vec<usize>,
}

/// One visible line in a [`ViewportLayout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLine {
    /// Line index into the original buffer.
    pub index: usize,
    /// The line's text.
    pub text: String,
    /// Whether this line matches the active search.
    pub is_match: bool,
}

/// Per-frame layout result: the windowed visible lines plus match info for
/// highlighting and scroll clamping.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewportLayout {
    /// Visible lines, windowed by `ctx.scroll .. ctx.scroll + ctx.height`.
    pub lines: Vec<VisibleLine>,
    /// Total line count (for scroll clamping).
    pub total: usize,
    /// Indices of matches that fall WITHIN the visible window (for highlight).
    pub matches_in_view: Vec<usize>,
}

impl ViewportLayout {
    /// Paint the visible lines into `buf` within `area`. Lines flagged
    /// `is_match` are drawn with `match_style` (e.g. highlighted); all others
    /// use `normal_style`.
    ///
    /// This is the render bridge for [`PreparedViewport`].
    pub fn paint(
        &self,
        buf: &mut ratatui::buffer::Buffer,
        area: ratatui::layout::Rect,
        normal_style: ratatui::style::Style,
        match_style: ratatui::style::Style,
    ) {
        for (row, line) in self.lines.iter().enumerate() {
            let Ok(y) = u16::try_from(row) else {
                break;
            };
            let Some(y) = area.y.checked_add(y) else {
                break;
            };
            if y >= area.bottom() {
                break;
            }
            let style = if line.is_match {
                match_style
            } else {
                normal_style
            };
            buf.set_string(area.x, y, &line.text, style);
        }
    }
}

impl PreparedViewportState {
    /// Number of lines in the buffer.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Number of lines matching the active search.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.match_indices.len()
    }

    /// Index of the next match strictly after `offset`, wrapping to the first
    /// match when none follows. `None` when there are no matches.
    ///
    /// Ports [`Viewport`](crate::viewport::Viewport)'s `next_match_target`
    /// navigation: find the smallest match index greater than `offset`, else
    /// wrap to the first match.
    #[must_use]
    pub fn next_match_from(&self, offset: usize) -> Option<usize> {
        if let Some(&next) = self.match_indices.iter().find(|&&idx| idx > offset) {
            Some(next)
        } else {
            self.match_indices.first().copied()
        }
    }

    /// Index of the previous match strictly before `offset`, wrapping to the
    /// last match when none precedes. `None` when there are no matches.
    ///
    /// Ports [`Viewport`](crate::viewport::Viewport)'s `prev_match_target`
    /// navigation: find the largest match index less than `offset`, else wrap
    /// to the last match.
    #[must_use]
    pub fn prev_match_from(&self, offset: usize) -> Option<usize> {
        if let Some(&prev) = self.match_indices.iter().rev().find(|&&idx| idx < offset) {
            Some(prev)
        } else {
            self.match_indices.last().copied()
        }
    }
}

/// Prepared scrollable-viewport primitive using the prepare/layout separation.
///
/// Implements [`Preparable`]. The input is a [`ViewportInput`] (lines + query).
/// [`Preparable::prepare`] stores the lines and query and precomputes match
/// indices. [`Preparable::append`] extends the lines, optionally adopts a new
/// query, and recomputes matches. [`Preparable::layout`] windows the buffer by
/// scroll/height and flags matches in view.
#[derive(Debug, Clone, Default)]
pub struct PreparedViewport;

impl Preparable for PreparedViewport {
    type Prepared = PreparedViewportState;
    type Layout = ViewportLayout;
    type Input = ViewportInput;

    fn prepare(input: Self::Input) -> Self::Prepared {
        let query = normalize_query(input.query);
        let match_indices = compute_matches(&input.lines, query.as_deref());
        PreparedViewportState {
            lines: input.lines,
            query,
            match_indices,
        }
    }

    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        // No-op fast path: nothing to extend and no new query.
        if more.lines.is_empty() && more.query.is_none() {
            return;
        }
        prepared.lines.extend(more.lines);
        // Adopt the appended query if one was supplied; an empty query clears
        // the search (None preserves the existing query).
        if let Some(q) = more.query {
            prepared.query = if q.is_empty() { None } else { Some(q) };
        }
        prepared.match_indices = compute_matches(&prepared.lines, prepared.query.as_deref());
    }

    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
        let total = prepared.lines.len();
        // Window exactly as `Viewport::render` does: clamp the start to the
        // buffer length, then take up to `height` rows from there.
        let start = ctx.scroll.min(total);
        let end = start.saturating_add(usize::from(ctx.height)).min(total);

        let mut lines: Vec<VisibleLine> = Vec::with_capacity(end.saturating_sub(start));
        let mut matches_in_view: Vec<usize> = Vec::new();

        for idx in start..end {
            let is_match = prepared.match_indices.contains(&idx);
            lines.push(VisibleLine {
                index: idx,
                text: prepared.lines[idx].clone(),
                is_match,
            });
            if is_match {
                matches_in_view.push(idx);
            }
        }

        ViewportLayout {
            lines,
            total,
            matches_in_view,
        }
    }
}

/// Normalize a query: `None` or an empty string becomes `None` (no search).
/// Any non-empty string is kept as-is.
fn normalize_query(query: Option<String>) -> Option<String> {
    query.filter(|q| !q.is_empty())
}

/// Compute match indices: ascending line indices whose lowercased text contains
/// the lowercased `query`. Returns an empty vector when `query` is `None` or
/// empty. Ports `Viewport::recompute_matches` for plain-text lines.
fn compute_matches(lines: &[String], query: Option<&str>) -> Vec<usize> {
    let Some(query) = query else {
        return Vec::new();
    };
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(&needle))
        .map(|(idx, _)| idx)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn input(lines: &[&str], query: Option<&str>) -> ViewportInput {
        ViewportInput {
            lines: lines.iter().map(|s| (*s).to_string()).collect(),
            query: query.map(str::to_owned),
        }
    }

    // ---------- prepare: search/match ----------

    #[test]
    fn prepare_no_query_has_no_matches() {
        let state = PreparedViewport::prepare(input(&["hello", "world"], None));
        assert_eq!(state.query, None);
        assert!(state.match_indices.is_empty());
        assert_eq!(state.match_count(), 0);
    }

    #[test]
    fn prepare_empty_query_has_no_matches() {
        let state = PreparedViewport::prepare(input(&["hello", "world"], Some("")));
        // An empty query normalizes to None (no search).
        assert_eq!(state.query, None);
        assert!(state.match_indices.is_empty());
    }

    #[test]
    fn prepare_matches_lines_containing_query() {
        // "foo bar" (1) and "footer here" (3) both contain "foo".
        let state = PreparedViewport::prepare(input(
            &["hello world", "foo bar", "baz qux", "footer here"],
            Some("foo"),
        ));
        assert_eq!(state.match_indices, vec![1, 3]);
        assert_eq!(state.match_count(), 2);
    }

    #[test]
    fn prepare_case_insensitive() {
        let state = PreparedViewport::prepare(input(&["Foo Bar", "nothing here"], Some("fOO")));
        assert_eq!(state.match_indices, vec![0]);
        assert_eq!(state.match_count(), 1);
    }

    #[test]
    fn prepare_no_match_returns_empty() {
        let state = PreparedViewport::prepare(input(&["alpha", "beta", "gamma"], Some("zzz")));
        assert!(state.match_indices.is_empty());
    }

    #[test]
    fn prepare_unicode_query() {
        let state =
            PreparedViewport::prepare(input(&["hello 世界", "plain", "世界 again"], Some("世界")));
        assert_eq!(state.match_indices, vec![0, 2]);
    }

    #[test]
    fn prepare_unicode_case_insensitive() {
        // Accented Latin letters lower-case via `to_lowercase`: É -> é, so an
        // uppercase-accented line matches an all-lowercase accented query. (Note:
        // this does NOT hold for ß<->ss equivalence, which needs full case
        // folding that byte-level `contains` does not perform.)
        let state = PreparedViewport::prepare(input(&["ÉCLAIR"], Some("éclair")));
        assert_eq!(state.match_indices, vec![0]);
    }

    #[test]
    fn prepare_query_spanning_multiple_lines() {
        let state = PreparedViewport::prepare(input(
            &["match one", "match two", "match three"],
            Some("match"),
        ));
        assert_eq!(state.match_indices, vec![0, 1, 2]);
        assert_eq!(state.match_count(), 3);
    }

    #[test]
    fn prepare_empty_lines_no_matches() {
        let state = PreparedViewport::prepare(input(&[], Some("anything")));
        assert!(state.match_indices.is_empty());
        assert_eq!(state.line_count(), 0);
    }

    #[test]
    fn prepare_stores_lines_verbatim() {
        let state = PreparedViewport::prepare(input(&["a", "b", "c"], None));
        assert_eq!(
            state.lines,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn prepare_query_preserved_in_state() {
        let state = PreparedViewport::prepare(input(&["x"], Some("query")));
        assert_eq!(state.query.as_deref(), Some("query"));
    }

    // ---------- layout: windowing + match flags ----------

    #[test]
    fn layout_windows_by_scroll_and_height() {
        let prepared = PreparedViewport::prepare(input(&["a", "b", "c", "d", "e"], None));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 2).with_scroll(1));
        assert_eq!(layout.total, 5);
        assert_eq!(layout.lines.len(), 2);
        assert_eq!(layout.lines[0].index, 1);
        assert_eq!(layout.lines[0].text, "b");
        assert_eq!(layout.lines[1].index, 2);
        assert_eq!(layout.lines[1].text, "c");
    }

    #[test]
    fn layout_is_match_flags_correct_in_window() {
        // matches at 1 and 3.
        let prepared =
            PreparedViewport::prepare(input(&["hello", "foo bar", "baz", "footer"], Some("foo")));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 4));
        assert_eq!(layout.lines.len(), 4);
        assert!(!layout.lines[0].is_match);
        assert!(layout.lines[1].is_match);
        assert!(!layout.lines[2].is_match);
        assert!(layout.lines[3].is_match);
    }

    #[test]
    fn layout_matches_in_view_only_contains_window_matches() {
        // matches at 1 and 3; window [1, 3) shows only line 1.
        let prepared =
            PreparedViewport::prepare(input(&["hello", "foo bar", "baz", "footer"], Some("foo")));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 2).with_scroll(1));
        assert_eq!(layout.matches_in_view, vec![1]);
    }

    #[test]
    fn layout_matches_in_view_empty_when_no_match_in_window() {
        // matches at 1 and 3; window [2, 3) shows only line 2 (no match).
        let prepared =
            PreparedViewport::prepare(input(&["hello", "foo bar", "baz", "footer"], Some("foo")));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 1).with_scroll(2));
        assert!(layout.matches_in_view.is_empty());
        assert!(!layout.lines[0].is_match);
    }

    #[test]
    fn layout_total_is_line_count() {
        let prepared = PreparedViewport::prepare(input(&["a", "b", "c"], None));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 10));
        assert_eq!(layout.total, 3);
    }

    #[test]
    fn layout_scroll_beyond_total_is_empty() {
        let prepared = PreparedViewport::prepare(input(&["a", "b"], None));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 10).with_scroll(999));
        assert_eq!(layout.total, 2);
        assert!(layout.lines.is_empty());
        assert!(layout.matches_in_view.is_empty());
    }

    #[test]
    fn layout_height_zero_is_empty() {
        let prepared = PreparedViewport::prepare(input(&["a", "b"], None));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 0));
        assert_eq!(layout.total, 2);
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn layout_full_view_shows_all() {
        let prepared = PreparedViewport::prepare(input(&["a", "b", "c"], Some("b")));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 100));
        assert_eq!(layout.lines.len(), 3);
        assert_eq!(layout.matches_in_view, vec![1]);
    }

    #[test]
    fn layout_no_query_no_matches_in_view() {
        let prepared = PreparedViewport::prepare(input(&["a", "b", "c"], None));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 3));
        assert!(layout.matches_in_view.is_empty());
        assert!(layout.lines.iter().all(|vl| !vl.is_match));
    }

    #[test]
    fn layout_focus_is_ignored() {
        let prepared = PreparedViewport::prepare(input(&["a", "b"], Some("a")));
        let plain = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 2));
        let with_focus = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 2).with_focus(1));
        assert_eq!(plain, with_focus);
    }

    #[test]
    fn layout_width_does_not_truncate_text() {
        let prepared = PreparedViewport::prepare(input(&["a very long line of text"], None));
        // Width 1 must not truncate the stored text.
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(1, 1));
        assert_eq!(layout.lines[0].text, "a very long line of text");
    }

    #[test]
    fn layout_empty_buffer_is_empty() {
        let prepared = PreparedViewport::prepare(input(&[], Some("x")));
        let layout = PreparedViewport::layout(&prepared, LayoutCtx::new(80, 24));
        assert!(layout.lines.is_empty());
        assert_eq!(layout.total, 0);
    }

    // ---------- next/prev match navigation ----------

    fn matches_at_1_and_3() -> PreparedViewportState {
        PreparedViewport::prepare(input(
            &["hello", "foo bar", "baz", "footer here"],
            Some("foo"),
        ))
    }

    #[test]
    fn next_match_from_advances_and_wraps() {
        let state = matches_at_1_and_3(); // matches [1, 3]
        assert_eq!(state.next_match_from(0), Some(1));
        assert_eq!(state.next_match_from(1), Some(3));
        // Past the last match: wrap to the first.
        assert_eq!(state.next_match_from(3), Some(1));
        assert_eq!(state.next_match_from(5), Some(1));
    }

    #[test]
    fn prev_match_from_goes_back_and_wraps() {
        let state = matches_at_1_and_3(); // matches [1, 3]
                                          // Before the first match: wrap to the last.
        assert_eq!(state.prev_match_from(0), Some(3));
        // At the first match: nothing precedes -> wrap to last.
        assert_eq!(state.prev_match_from(1), Some(3));
        assert_eq!(state.prev_match_from(3), Some(1));
        assert_eq!(state.prev_match_from(4), Some(3));
    }

    #[test]
    fn next_match_from_empty_matches_is_none() {
        let state = PreparedViewport::prepare(input(&["a", "b"], Some("zzz")));
        assert_eq!(state.next_match_from(0), None);
    }

    #[test]
    fn prev_match_from_empty_matches_is_none() {
        let state = PreparedViewport::prepare(input(&["a", "b"], Some("zzz")));
        assert_eq!(state.prev_match_from(0), None);
    }

    #[test]
    fn next_match_from_single_match_wraps_to_self() {
        let state = PreparedViewport::prepare(input(&["foo", "bar"], Some("foo"))); // [0]
        assert_eq!(state.next_match_from(0), Some(0));
        assert_eq!(state.next_match_from(1), Some(0));
    }

    #[test]
    fn prev_match_from_single_match_wraps_to_self() {
        let state = PreparedViewport::prepare(input(&["foo", "bar"], Some("foo"))); // [0]
        assert_eq!(state.prev_match_from(0), Some(0));
        assert_eq!(state.prev_match_from(1), Some(0));
    }

    // ---------- append ----------

    #[test]
    fn append_extends_lines_and_recomputes_matches() {
        let mut prepared =
            PreparedViewport::prepare(input(&["foo one", "no match here"], Some("foo")));
        assert_eq!(prepared.match_count(), 1);
        PreparedViewport::append(&mut prepared, input(&["foo two"], None));
        assert_eq!(prepared.line_count(), 3);
        assert_eq!(prepared.match_indices, vec![0, 2]);
        assert_eq!(prepared.match_count(), 2);
    }

    #[test]
    fn append_updates_query() {
        let mut prepared = PreparedViewport::prepare(input(&["foo", "bar"], None));
        assert_eq!(prepared.match_count(), 0);
        PreparedViewport::append(&mut prepared, input(&[], Some("foo")));
        assert_eq!(prepared.query.as_deref(), Some("foo"));
        assert_eq!(prepared.match_indices, vec![0]);
    }

    #[test]
    fn append_preserves_query_when_none() {
        let mut prepared = PreparedViewport::prepare(input(&["foo one", "bar"], Some("foo")));
        assert_eq!(prepared.match_count(), 1);
        // more.query is None -> existing query is preserved, matches recomputed
        // over the extended buffer.
        PreparedViewport::append(&mut prepared, input(&["foo two"], None));
        assert_eq!(prepared.query.as_deref(), Some("foo"));
        assert_eq!(prepared.match_indices, vec![0, 2]);
    }

    #[test]
    fn append_empty_query_clears_search() {
        let mut prepared = PreparedViewport::prepare(input(&["foo", "bar"], Some("foo")));
        assert_eq!(prepared.match_count(), 1);
        PreparedViewport::append(&mut prepared, input(&[], Some("")));
        assert_eq!(prepared.query, None);
        assert!(prepared.match_indices.is_empty());
    }

    #[test]
    fn append_empty_is_unchanged() {
        let mut prepared = PreparedViewport::prepare(input(&["a", "b"], Some("a")));
        let before = prepared.clone();
        PreparedViewport::append(&mut prepared, ViewportInput::default());
        assert_eq!(prepared, before);
    }

    #[test]
    fn append_all_new_lines_matching_extends_match_list() {
        let mut prepared = PreparedViewport::prepare(input(&["x"], Some("match")));
        assert_eq!(prepared.match_count(), 0);
        // "other" does NOT contain the substring "match" (unlike "nomatch").
        PreparedViewport::append(&mut prepared, input(&["match a", "other", "match b"], None));
        assert_eq!(prepared.match_indices, vec![1, 3]);
    }

    // ---------- helpers ----------

    #[test]
    fn helpers_line_count_and_match_count() {
        let state = PreparedViewport::prepare(input(&["foo", "bar", "foobar"], Some("foo")));
        assert_eq!(state.line_count(), 3);
        assert_eq!(state.match_count(), 2);
    }

    #[test]
    fn clone_equality_for_state_and_layout() {
        let state = PreparedViewport::prepare(input(&["hello", "world"], Some("world")));
        assert_eq!(state.clone(), state);

        let layout = PreparedViewport::layout(&state, LayoutCtx::new(80, 2));
        assert_eq!(layout.clone(), layout);
    }

    // ---------- invariant loops (manual, no proptest macros) ----------

    #[test]
    fn invariant_matches_never_exceed_lines_and_visible_never_exceeds_height() {
        let lines = vec![
            "the quick brown fox".to_string(),
            "jumps over the lazy dog".to_string(),
            "THE QUICK BROWN FOX".to_string(),
            "世界 🌟 unicode".to_string(),
            String::new(),
        ];
        let queries: [Option<&str>; 8] = [
            None,
            Some(""),
            Some("the"),
            Some("THE"),
            Some("fox"),
            Some("世界"),
            Some("xyz"),
            Some(" "),
        ];
        for query in queries {
            let prepared = PreparedViewport::prepare(ViewportInput {
                lines: lines.clone(),
                query: query.map(str::to_owned),
            });
            assert!(
                prepared.match_count() <= prepared.line_count(),
                "query={query:?}: match_count > line_count"
            );
            for &height in &[0u16, 1, 2, 5, 100] {
                for &scroll in &[0usize, 1, 3, 100] {
                    let layout = PreparedViewport::layout(
                        &prepared,
                        LayoutCtx::new(80, height).with_scroll(scroll),
                    );
                    assert!(
                        layout.lines.len() <= usize::from(height),
                        "query={query:?} height={height} scroll={scroll}: visible > height"
                    );
                    assert_eq!(layout.total, lines.len());
                    assert!(layout.matches_in_view.len() <= prepared.match_count());
                    // Every matches_in_view entry is actually present in the
                    // visible window.
                    for &m in &layout.matches_in_view {
                        assert!(
                            layout.lines.iter().any(|vl| vl.index == m),
                            "query={query:?}: match {m} flagged but not in view"
                        );
                    }
                    // matches_in_view is ascending (sorted, no dups).
                    for w in layout.matches_in_view.windows(2) {
                        assert!(
                            w[0] < w[1],
                            "query={query:?}: matches_in_view not ascending"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn invariant_many_inputs_never_panic() {
        let inputs: [ViewportInput; 6] = [
            ViewportInput {
                lines: vec![],
                query: None,
            },
            ViewportInput {
                lines: vec![],
                query: Some("x".to_string()),
            },
            ViewportInput {
                lines: vec!["a".to_string()],
                query: None,
            },
            ViewportInput {
                lines: vec!["a".to_string()],
                query: Some("a".to_string()),
            },
            ViewportInput {
                lines: (0..50).map(|i| format!("line-{i}")).collect(),
                query: Some("line".to_string()),
            },
            ViewportInput {
                lines: (0..50).map(|i| format!("row-{i}")).collect(),
                query: Some("line".to_string()),
            },
        ];
        for input in &inputs {
            let prepared = PreparedViewport::prepare(input.clone());
            for &height in &[0u16, 1, 10] {
                for &scroll in &[0usize, 1, 10, 100] {
                    let _ = PreparedViewport::layout(
                        &prepared,
                        LayoutCtx::new(40, height).with_scroll(scroll),
                    );
                }
            }
        }
    }
}
