//! Multi-line text input with command history.
//!
//! A Rust port of the [`Bubbles` `textarea`](https://github.com/charmbracelet/bubbles/textarea)
//! package, implemented as a multi-line editor with cursor movement and an
//! optional submission history (similar to a shell readline).
//!
//! Unlike the original Go component — a fully styled textarea — this port
//! focuses on the buffer model, cursor motion, and a submit-driven history
//! cycle suitable for prompt-style inputs.

#![allow(clippy::module_name_repetitions)]

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// A multi-line text input widget.
///
/// Port of `bubbles/textarea.Model`, focused on the editable buffer and a
/// submit-driven history (readline-style). See the [module docs](self).
pub struct TextInput {
    /// The buffer, one [`String`] per line. Always non-empty.
    lines: Vec<String>,
    /// Cursor row (0-indexed into [`TextInput::lines`]).
    cursor_line: usize,
    /// Cursor column measured in characters (0-indexed; may equal the line's
    /// character count when at end-of-line).
    cursor_col: usize,
    /// Previously submitted values, oldest first.
    history: Vec<String>,
    /// Current position within [`TextInput::history`] during navigation, or
    /// `None` when editing the live buffer.
    history_cursor: Option<usize>,
    /// The live buffer captured when navigation enters [`TextInput::history`],
    /// restored when the user browses past the newest entry.
    saved_buffer: Option<String>,
}

impl TextInput {
    /// Create an empty text input (one empty line, cursor at `0,0`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            history: Vec::new(),
            history_cursor: None,
            saved_buffer: None,
        }
    }

    /// Return the full buffer joined by newlines.
    #[must_use]
    pub fn value(&self) -> String {
        self.lines.join("\n")
    }

    /// Return the cursor position as `(line, column)`.
    #[must_use]
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    /// Insert a single character at the cursor, advancing the column by one.
    pub fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.cursor_line];
        let off = char_byte_offset(line, self.cursor_col);
        line.insert(off, c);
        self.cursor_col += 1;
    }

    /// Insert a string at the cursor.
    ///
    /// Embedded newlines split the buffer into additional lines, mirroring
    /// [`TextInput::enter`].
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.enter();
            } else {
                self.insert_char(c);
            }
        }
    }

    /// Delete the character before the cursor.
    ///
    /// At column `0` of a non-first line the current line is merged into the
    /// previous one and the cursor moves to the join.
    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let off = char_byte_offset(line, self.cursor_col - 1);
            line.remove(off);
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            let current = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let join_col = self.lines[self.cursor_line].chars().count();
            self.lines[self.cursor_line].push_str(&current);
            self.cursor_col = join_col;
        }
    }

    /// Split the current line at the cursor, moving the cursor to the start of
    /// the new line below.
    pub fn enter(&mut self) {
        let current = self.lines[self.cursor_line].clone();
        let off = char_byte_offset(&current, self.cursor_col);
        let (before, after) = current.split_at(off);
        self.lines[self.cursor_line] = before.to_string();
        self.lines.insert(self.cursor_line + 1, after.to_string());
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    /// Move the cursor left, wrapping to the end of the previous line.
    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].chars().count();
        }
    }

    /// Move the cursor right, wrapping to the start of the next line.
    pub fn move_right(&mut self) {
        let cur_len = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < cur_len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    /// Move the cursor up one line, clamping the column to the target line.
    pub fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.clamp_col();
        }
    }

    /// Move the cursor down one line, clamping the column to the target line.
    pub fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.clamp_col();
        }
    }

    /// Reset the buffer to a single empty line and abandon any in-progress
    /// history navigation. Submitted [`TextInput::history`] is preserved.
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.history_cursor = None;
        self.saved_buffer = None;
    }

    /// Submit the current buffer: push it onto [`TextInput::history`], clear
    /// the buffer, and return the submitted value.
    pub fn submit(&mut self) -> String {
        let value = self.value();
        self.history.push(value.clone());
        self.clear();
        value
    }

    /// Navigate to the previous (older) history entry, replacing the buffer.
    ///
    /// The first call captures the live buffer so [`TextInput::history_next`]
    /// can restore it later. No-op when [`TextInput::history`] is empty or the
    /// cursor already sits at the oldest entry.
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_cursor {
            None => {
                self.saved_buffer = Some(self.value());
                self.history.len() - 1
            }
            Some(i) if i > 0 => i - 1,
            Some(_) => return, // already at the oldest entry
        };
        self.history_cursor = Some(idx);
        let entry = self.history[idx].clone();
        self.load_string(&entry);
    }

    /// Navigate to the next (newer) history entry.
    ///
    /// Moving past the newest entry exits history navigation and restores the
    /// buffer captured by [`TextInput::history_prev`].
    pub fn history_next(&mut self) {
        let Some(i) = self.history_cursor else {
            return; // already editing the live buffer
        };
        if i + 1 < self.history.len() {
            let idx = i + 1;
            self.history_cursor = Some(idx);
            let entry = self.history[idx].clone();
            self.load_string(&entry);
        } else {
            // past newest -> restore the saved live buffer
            self.history_cursor = None;
            let saved = self.saved_buffer.take().unwrap_or_default();
            self.load_string(&saved);
        }
    }

    /// Render the buffer into `frame` inside `area`.
    ///
    /// The cursor is indicated (best-effort) by drawing the character under it
    /// in reverse video; an inverted space is drawn at end-of-line. Ratatui
    /// 0.29 has no widget-level cursor positioning, so this is a visual hint
    /// rather than a real terminal cursor.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let lines: Vec<Line<'static>> = self
            .lines
            .iter()
            .enumerate()
            .map(|(li, l)| {
                if li == self.cursor_line {
                    render_line_with_cursor(l, self.cursor_col)
                } else {
                    Line::from(l.clone())
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), area);
    }

    /// Clamp [`TextInput::cursor_col`] to the current line's character count.
    fn clamp_col(&mut self) {
        let len = self.lines[self.cursor_line].chars().count();
        if self.cursor_col > len {
            self.cursor_col = len;
        }
    }

    /// Replace the buffer with `s` (split on newlines) and place the cursor at
    /// the end of the buffer.
    fn load_string(&mut self, s: &str) {
        let mut lines: Vec<String> = s.split('\n').map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        self.cursor_line = lines.len() - 1;
        self.cursor_col = lines[self.cursor_line].chars().count();
        self.lines = lines;
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Return the byte offset of the `col`-th character of `s`, or `s.len()` when
/// `col` is at or past the end of the string.
fn char_byte_offset(s: &str, col: usize) -> usize {
    s.char_indices()
        .nth(col)
        .map_or_else(|| s.len(), |(b, _)| b)
}

/// Build a [`Line`] whose character at `col` is shown in reverse video.
///
/// When `col` sits past the last character an inverted space is drawn so the
/// cursor remains visible at end-of-line.
fn render_line_with_cursor(line: &str, col: usize) -> Line<'static> {
    let cursor_style = Style::new().add_modifier(Modifier::REVERSED);
    let off = char_byte_offset(line, col);

    let before = &line[..off];
    let rest = &line[off..];

    let mut spans: Vec<Span<'static>> = Vec::new();
    if !before.is_empty() {
        spans.push(Span::raw(before.to_string()));
    }
    let cur = rest
        .chars()
        .next()
        .map_or_else(|| String::from(' '), |c| c.to_string());
    spans.push(Span::styled(cur, cursor_style));

    let after_off = off + rest.chars().next().map_or(0, char::len_utf8);
    if after_off < line.len() {
        spans.push(Span::raw(line[after_off..].to_string()));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_enter_insert_produces_two_lines() {
        let mut ti = TextInput::new();
        ti.insert_char('a');
        ti.enter();
        ti.insert_char('b');
        assert_eq!(ti.value(), "a\nb");
    }

    #[test]
    fn new_starts_empty_at_origin() {
        let ti = TextInput::new();
        assert_eq!(ti.value(), "");
        assert_eq!(ti.cursor(), (0, 0));
        assert!(ti.history.is_empty());
    }

    #[test]
    fn default_equals_new() {
        assert_eq!(TextInput::default().value(), TextInput::new().value());
    }

    #[test]
    fn insert_str_handles_newlines() {
        let mut ti = TextInput::new();
        ti.insert_str("ab\ncd\nef");
        assert_eq!(ti.value(), "ab\ncd\nef");
        assert_eq!(ti.cursor(), (2, 2)); // end of "ef"
    }

    #[test]
    fn backspace_merges_lines_at_column_zero() {
        let mut ti = TextInput::new();
        ti.insert_str("foo\nbar");
        // cursor at end of "bar": (1, 3)
        ti.backspace();
        ti.backspace();
        ti.backspace();
        // now at col 0 of line 1; one more backspace merges into "foo"
        ti.backspace();
        assert_eq!(ti.value(), "foo");
        assert_eq!(ti.cursor(), (0, 3));
    }

    #[test]
    fn submit_clears_buffer_and_records_history() {
        let mut ti = TextInput::new();
        ti.insert_str("first");
        assert_eq!(ti.submit(), "first");
        assert_eq!(ti.value(), ""); // buffer cleared
        assert_eq!(ti.cursor(), (0, 0));

        ti.insert_str("second");
        assert_eq!(ti.submit(), "second");
        assert_eq!(ti.history, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn history_prev_next_walk_and_restore_saved_buffer() {
        let mut ti = TextInput::new();
        ti.insert_str("a");
        let _ = ti.submit();
        ti.insert_str("b");
        let _ = ti.submit();
        // history: ["a", "b"]; live buffer is now empty
        ti.insert_str("draft"); // live buffer captured on first history_prev

        ti.history_prev();
        assert_eq!(ti.value(), "b"); // newest
        ti.history_prev();
        assert_eq!(ti.value(), "a"); // oldest
        ti.history_prev(); // already at oldest -> no-op
        assert_eq!(ti.value(), "a");

        ti.history_next();
        assert_eq!(ti.value(), "b");
        ti.history_next(); // past newest -> restore the saved "draft"
        assert_eq!(ti.value(), "draft");
        ti.history_next(); // already live -> no-op
        assert_eq!(ti.value(), "draft");
    }

    #[test]
    fn history_prev_is_noop_when_empty() {
        let mut ti = TextInput::new();
        ti.insert_str("x");
        ti.history_prev(); // no history -> no-op
        assert_eq!(ti.value(), "x");
        assert!(ti.history_cursor.is_none());
    }

    #[test]
    fn clear_resets_buffer_but_keeps_history() {
        let mut ti = TextInput::new();
        ti.insert_str("keep me");
        let _ = ti.submit();
        ti.insert_str("scratch");
        ti.clear();
        assert_eq!(ti.value(), "");
        assert_eq!(ti.cursor(), (0, 0));
        assert_eq!(ti.history, vec!["keep me".to_string()]);
    }

    #[test]
    fn cursor_movement_wraps_and_clamps() {
        let mut ti = TextInput::new();
        ti.insert_str("abc\nde");
        // cursor at end of "de": (1, 2)
        ti.move_right(); // no-op (end of buffer)
        assert_eq!(ti.cursor(), (1, 2));
        ti.move_up();
        assert_eq!(ti.cursor(), (0, 2)); // clamped to len("abc")
        ti.move_left();
        ti.move_left();
        ti.move_left(); // (0, -1) -> wraps to end of... no, still line 0
        assert_eq!(ti.cursor(), (0, 0));
        ti.move_left(); // at (0,0) -> no-op
        assert_eq!(ti.cursor(), (0, 0));
        ti.move_down();
        assert_eq!(ti.cursor(), (1, 0)); // clamped down keeps col within "de"
    }

    #[test]
    fn handles_multibyte_characters_by_char_column() {
        let mut ti = TextInput::new();
        ti.insert_str("한글"); // 2 chars, 6 bytes
        assert_eq!(ti.cursor(), (0, 2));
        ti.move_left();
        ti.insert_char('x'); // insert between the two Hangul syllables
        assert_eq!(ti.value(), "한x글");
        ti.backspace(); // delete 'x'
        assert_eq!(ti.value(), "한글");
    }
}
