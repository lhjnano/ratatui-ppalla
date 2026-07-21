//! Scrollable viewport with optional substring search.
//!
//! A Rust port of the [`Bubbles` `viewport`](https://github.com/charmbracelet/bubbles/viewport)
//! package: a vertically-scrollable buffer of [`ratatui::text::Line`]s with
//! case-insensitive substring search and match navigation.

#![allow(clippy::module_name_repetitions)]

use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// A vertically-scrollable buffer of lines with optional substring search.
///
/// Port of `bubbles/viewport.Model`. See the [module docs](self) for details.
pub struct Viewport {
    /// Every line currently held in the buffer.
    lines: Vec<Line<'static>>,
    /// Top-most visible line index.
    offset: usize,
    /// Desired visible height (in terminal rows).
    height: u16,
    /// Active search query, if any.
    search: Option<String>,
    /// Indices into [`Viewport::lines`] whose plain text contains the query.
    matches: Vec<usize>,
}

impl Viewport {
    /// Create an empty viewport `height` rows tall.
    #[must_use]
    pub fn new(height: u16) -> Self {
        Self {
            lines: Vec::new(),
            offset: 0,
            height,
            search: None,
            matches: Vec::new(),
        }
    }

    /// Return the configured visible height.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Set the visible height, clamping the current offset if needed.
    pub fn set_height(&mut self, h: u16) {
        self.height = h;
        self.clamp_offset();
    }

    /// Append a line to the bottom of the buffer, re-running any active search.
    pub fn append_line(&mut self, line: Line<'static>) {
        self.lines.push(line);
        self.recompute_matches();
        self.clamp_offset();
    }

    /// Replace the entire buffer with `lines`, resetting the scroll offset and
    /// re-running any active search.
    pub fn set_lines(&mut self, lines: Vec<Line<'static>>) {
        self.lines = lines;
        self.offset = 0;
        self.recompute_matches();
    }

    /// Return the total number of lines in the buffer.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Scroll down by `n` rows (clamped to the bottom of the buffer).
    pub fn scroll_down(&mut self, n: usize) {
        self.offset = self.offset.saturating_add(n);
        self.clamp_offset();
    }

    /// Scroll up by `n` rows (clamped at zero).
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    /// Return the current top-most visible line index.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Set the search query, recomputing match positions.
    ///
    /// Passing `None` (or an empty string) clears the search. Matches are found
    /// via a case-insensitive substring search through each line's plain text.
    pub fn set_search(&mut self, query: Option<&str>) {
        self.search = query.filter(|q| !q.is_empty()).map(str::to_owned);
        self.recompute_matches();
    }

    /// Return the number of lines currently matching the active search.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Scroll so the next match (after the current offset) is in view.
    ///
    /// Wraps around to the first match when past the last one. No-op when no
    /// search is active or there are no matches.
    pub fn next_match(&mut self) {
        let Some(target) = self.next_match_target() else {
            return;
        };
        self.scroll_to(target);
    }

    /// Scroll so the previous match (before the current offset) is in view.
    ///
    /// Wraps around to the last match when before the first one. No-op when no
    /// search is active or there are no matches.
    pub fn prev_match(&mut self) {
        let Some(target) = self.prev_match_target() else {
            return;
        };
        self.scroll_to(target);
    }

    /// Render the visible window of lines into `frame`, highlighting any line
    /// that matches the active search with [`Style::default().yellow()`].
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let h = usize::from(self.height);
        let start = self.offset.min(self.lines.len());
        let end = self.offset.saturating_add(h).min(self.lines.len());

        let mut visible: Vec<Line<'static>> = Vec::with_capacity(end.saturating_sub(start));
        for global_idx in start..end {
            let mut line = self.lines[global_idx].clone();
            if self.matches.contains(&global_idx) {
                line.style = Style::default().yellow();
            }
            visible.push(line);
        }

        frame.render_widget(Paragraph::new(visible), area);
    }

    /// Index of the next match strictly after the current offset, wrapping to
    /// the first match when none follows. `None` when there are no matches.
    fn next_match_target(&self) -> Option<usize> {
        if let Some(&next) = self.matches.iter().find(|&&idx| idx > self.offset) {
            Some(next)
        } else {
            self.matches.first().copied()
        }
    }

    /// Index of the previous match strictly before the current offset, wrapping
    /// to the last match when none precedes. `None` when there are no matches.
    fn prev_match_target(&self) -> Option<usize> {
        if let Some(&prev) = self.matches.iter().rev().find(|&&idx| idx < self.offset) {
            Some(prev)
        } else {
            self.matches.last().copied()
        }
    }

    /// Set the offset so `line_idx` is the top visible row, clamped to bounds.
    fn scroll_to(&mut self, line_idx: usize) {
        self.offset = line_idx;
        self.clamp_offset();
    }

    /// Clamp [`Viewport::offset`] to the largest legal top-row index.
    fn clamp_offset(&mut self) {
        let max = self.max_offset();
        if self.offset > max {
            self.offset = max;
        }
    }

    /// Largest legal top-row index given the current height.
    fn max_offset(&self) -> usize {
        self.lines.len().saturating_sub(usize::from(self.height))
    }

    /// Recompute [`Viewport::matches`] from the current query via a
    /// case-insensitive substring search through each line's plain text.
    fn recompute_matches(&mut self) {
        self.matches.clear();
        let Some(query) = &self.search else {
            return;
        };
        let needle = query.to_lowercase();
        for (idx, line) in self.lines.iter().enumerate() {
            // `Span.content` is `Cow<'_, str>`; `&*` derefs to `&str`.
            // (`s.content.as_str()` would resolve to the unstable `str::as_str`.)
            let plain: String = line.spans.iter().map(|s| &*s.content).collect();
            if plain.to_lowercase().contains(&needle) {
                self.matches.push(idx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp_with_lines() -> Viewport {
        let mut vp = Viewport::new(10);
        vp.append_line(Line::from("hello world"));
        vp.append_line(Line::from("foo bar"));
        vp.append_line(Line::from("baz qux"));
        vp.append_line(Line::from("footer here"));
        vp
    }

    #[test]
    fn set_search_finds_two_matching_line_indices() {
        let mut vp = vp_with_lines();
        vp.set_search(Some("foo"));
        // Lines "foo bar" (idx 1) and "footer here" (idx 3) both contain "foo".
        assert_eq!(vp.match_count(), 2);
    }

    #[test]
    fn search_is_case_insensitive() {
        let mut vp = Viewport::new(10);
        vp.append_line(Line::from("Foo Bar"));
        vp.append_line(Line::from("nothing here"));
        vp.set_search(Some("fOO"));
        assert_eq!(vp.match_count(), 1);
    }

    #[test]
    fn clearing_search_empties_matches() {
        let mut vp = vp_with_lines();
        vp.set_search(Some("foo"));
        assert_eq!(vp.match_count(), 2);
        vp.set_search(None);
        assert_eq!(vp.match_count(), 0);
    }

    #[test]
    fn next_match_advances_offset_and_wraps() {
        // height 1 => max_offset 3, so offsets 1 and 3 are valid scroll targets.
        let mut vp = Viewport::new(1);
        vp.append_line(Line::from("hello world"));
        vp.append_line(Line::from("foo bar"));
        vp.append_line(Line::from("baz qux"));
        vp.append_line(Line::from("footer here"));
        vp.set_search(Some("foo")); // matches: [1, 3]
        assert_eq!(vp.offset(), 0);
        vp.next_match();
        assert_eq!(vp.offset(), 1);
        vp.next_match();
        assert_eq!(vp.offset(), 3);
        // wrap around back to the first match
        vp.next_match();
        assert_eq!(vp.offset(), 1);
    }

    #[test]
    fn scroll_clamps_to_max_offset() {
        let mut vp = Viewport::new(2);
        vp.append_line(Line::from("a"));
        vp.append_line(Line::from("b"));
        vp.append_line(Line::from("c"));
        // 3 lines, height 2 => max offset = 1
        vp.scroll_down(10);
        assert_eq!(vp.offset(), 1);
        vp.scroll_up(5);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn empty_viewport_has_zero_lines() {
        let vp = Viewport::new(10);
        assert_eq!(vp.line_count(), 0);
        assert_eq!(vp.match_count(), 0);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn scroll_up_beyond_zero_clamps() {
        let mut vp = Viewport::new(10);
        vp.append_line(Line::from("a"));
        vp.append_line(Line::from("b"));
        vp.append_line(Line::from("c"));
        vp.scroll_up(100);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn scroll_down_beyond_end_clamps() {
        // 3 lines, height 5 => max_offset = 3.saturating_sub(5) = 0
        let mut vp = Viewport::new(5);
        vp.append_line(Line::from("a"));
        vp.append_line(Line::from("b"));
        vp.append_line(Line::from("c"));
        vp.scroll_down(100);
        assert_eq!(vp.offset(), 0);
    }

    #[test]
    fn search_with_no_matches_returns_zero() {
        let mut vp = Viewport::new(10);
        vp.append_line(Line::from("alpha"));
        vp.append_line(Line::from("beta"));
        vp.append_line(Line::from("gamma"));
        vp.set_search(Some("foo"));
        assert_eq!(vp.match_count(), 0);
    }

    #[test]
    fn search_recomputes_after_append() {
        let mut vp = Viewport::new(10);
        vp.append_line(Line::from("foo one"));
        vp.append_line(Line::from("no match here"));
        vp.set_search(Some("foo"));
        assert_eq!(vp.match_count(), 1);
        vp.append_line(Line::from("foo two"));
        assert_eq!(vp.match_count(), 2);
    }

    #[test]
    fn clear_search_returns_all_lines_visible() {
        let mut vp = vp_with_lines();
        vp.set_search(Some("foo"));
        assert_eq!(vp.match_count(), 2);
        vp.set_search(None);
        assert_eq!(vp.match_count(), 0);
        // Search only affects highlighting; the buffer is unchanged.
        assert_eq!(vp.line_count(), 4);
    }
}
