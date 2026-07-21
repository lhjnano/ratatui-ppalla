//! Key binding help widget — a port of [Bubbles' `key`](https://github.com/charmbracelet/bubbles/key) help formatting.
//!
//! Renders a list of key bindings as "<key>  <desc>" pairs inside a titled block.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// A single key binding descriptor.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// The key sequence, e.g. "ctrl+c" or "enter".
    pub key: String,
    /// Human-readable description.
    pub desc: String,
    /// Whether the binding is currently available (false = greyed out).
    pub disabled: bool,
}

impl KeyBinding {
    /// Create an enabled binding.
    #[must_use]
    pub fn new(key: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            desc: desc.into(),
            disabled: false,
        }
    }

    /// Mark this binding as disabled (will be excluded from rendered help).
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

/// A registry of key bindings for displaying a help footer / overlay.
#[derive(Debug, Clone, Default)]
pub struct KeyHelp {
    bindings: Vec<KeyBinding>,
    title: String,
}

impl KeyHelp {
    /// Create an empty help registry with default title "Keys".
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            title: "Keys".to_string(),
        }
    }

    /// Set a custom title for the help block.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Append a binding.
    pub fn add(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Returns the count of enabled bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.iter().filter(|b| !b.disabled).count()
    }

    /// Returns true when there are no enabled bindings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return (key, desc) pairs for all enabled bindings.
    #[must_use]
    pub fn full_help(&self) -> Vec<(String, String)> {
        self.bindings
            .iter()
            .filter(|b| !b.disabled)
            .map(|b| (b.key.clone(), b.desc.clone()))
            .collect()
    }

    /// Renders the help registry as a Paragraph inside a titled Block.
    ///
    /// Each enabled binding is rendered as "<key>  <desc>" on its own line.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let lines: Vec<Line<'_>> = self
            .bindings
            .iter()
            .filter(|b| !b.disabled)
            .map(|b| {
                Line::from(vec![
                    Span::styled(
                        b.key.as_str().to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(b.desc.as_str().to_string()),
                ])
            })
            .collect();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title.as_str())
            .title_alignment(Alignment::Left);
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let h = KeyHelp::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn add_includes_enabled_bindings_only() {
        let mut h = KeyHelp::new();
        h.add(KeyBinding::new("q", "quit"));
        h.add(KeyBinding::new("d", "deleted").disabled());
        h.add(KeyBinding::new("r", "refresh"));
        assert_eq!(h.len(), 2);
        let pairs = h.full_help();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("q".to_string(), "quit".to_string()));
        assert_eq!(pairs[1], ("r".to_string(), "refresh".to_string()));
    }

    #[test]
    fn with_title_sets_custom_title() {
        let h = KeyHelp::new().with_title("Shortcuts");
        assert_eq!(h.title, "Shortcuts");
    }
}
