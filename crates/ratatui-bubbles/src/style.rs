//! Lipgloss-style builder API — a port of [Lipgloss](https://github.com/charmbracelet/lipgloss) for Ratatui.
//!
//! Provides a chained builder for [`ratatui::style::Style`]. Padding, margin,
//! and border (layout concepts in Lipgloss) are accepted by the builder for
//! API parity but are not yet applied to the produced `Style` — they would
//! require wrapping the widget in a `Block` at render time, which is out of
//! scope for this module.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;

/// A chained builder for [`ratatui::style::Style`], inspired by Lipgloss.
#[derive(Debug, Clone)]
pub struct StyleBuilder {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    italic: bool,
    underline: bool,
    padding: Option<(u16, u16)>,
    margin: Option<(u16, u16)>,
    border: Option<BorderType>,
}

impl StyleBuilder {
    /// Start a new style.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            padding: None,
            margin: None,
            border: None,
        }
    }

    /// Set the foreground color.
    #[must_use]
    pub fn foreground(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set the background color.
    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Enable bold.
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Enable italic.
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Enable underline.
    #[must_use]
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Set horizontal and vertical padding (stored but NOT applied to the
    /// produced `Style`; would need a wrapping `Block` at render time).
    #[must_use]
    pub fn padding(mut self, x: u16, y: u16) -> Self {
        self.padding = Some((x, y));
        self
    }

    /// Set horizontal and vertical margin (stored but NOT applied to the
    /// produced `Style`; would need a wrapping `Block` at render time).
    #[must_use]
    pub fn margin(mut self, x: u16, y: u16) -> Self {
        self.margin = Some((x, y));
        self
    }

    /// Set border type (stored but NOT applied to the produced `Style`; would
    /// need a wrapping `Block` at render time).
    #[must_use]
    pub fn border(mut self, border: BorderType) -> Self {
        self.border = Some(border);
        self
    }

    /// Build the final [`ratatui::style::Style`].
    ///
    /// Note: `padding`, `margin`, and `border` are intentionally ignored —
    /// see the module docs.
    #[must_use]
    pub fn build(self) -> Style {
        let mut style = Style::default();
        if let Some(fg) = self.fg {
            style = style.fg(fg);
        }
        if let Some(bg) = self.bg {
            style = style.bg(bg);
        }
        let mut modifier = Modifier::empty();
        if self.bold {
            modifier |= Modifier::BOLD;
        }
        if self.italic {
            modifier |= Modifier::ITALIC;
        }
        if self.underline {
            modifier |= Modifier::UNDERLINED;
        }
        if !modifier.is_empty() {
            style = style.add_modifier(modifier);
        }
        style
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_builder_produces_default_style() {
        let s = StyleBuilder::new().build();
        assert_eq!(s, Style::default());
    }

    #[test]
    fn foreground_sets_fg() {
        let s = StyleBuilder::new().foreground(Color::Red).build();
        assert_eq!(s.fg, Some(Color::Red));
    }

    #[test]
    fn bold_and_italic_compose_into_modifier() {
        let s = StyleBuilder::new().bold().italic().build();
        assert!(s.add_modifier.contains(Modifier::BOLD));
        assert!(s.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn background_sets_bg() {
        let s = StyleBuilder::new().background(Color::Blue).build();
        assert_eq!(s.bg, Some(Color::Blue));
    }

    #[test]
    fn underline_adds_underlined_modifier() {
        let s = StyleBuilder::new().underline().build();
        assert!(s.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn padding_is_stored_without_affecting_build() {
        // padding is accepted by the builder but not applied to Style (documented limitation).
        // Verify build() still produces a valid default-equivalent Style when only padding is set.
        let s = StyleBuilder::new().padding(2, 1).build();
        assert_eq!(s, Style::default());
    }

    #[test]
    fn margin_is_stored_without_affecting_build() {
        let s = StyleBuilder::new().margin(1, 0).build();
        assert_eq!(s, Style::default());
    }

    #[test]
    fn border_is_stored_without_affecting_build() {
        let s = StyleBuilder::new().border(BorderType::Rounded).build();
        assert_eq!(s, Style::default());
    }
}
