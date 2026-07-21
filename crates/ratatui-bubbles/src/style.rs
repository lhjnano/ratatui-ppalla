//! Lipgloss-style builder API — a port of [Lipgloss](https://github.com/charmbracelet/lipgloss) for Ratatui.
//!
//! # Status
//!
//! **Tier 2 — not yet implemented.** Construction works; chained style methods
//! and `build()` panic with [`todo!()`].

#![allow(clippy::missing_panics_doc)]

use ratatui::style::Color;
use ratatui::widgets::BorderType;

/// A chained builder for [`ratatui::style::Style`], inspired by Lipgloss.
///
/// Tier 2 stub — see module docs.
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

    /// Set horizontal and vertical padding.
    #[must_use]
    pub fn padding(mut self, x: u16, y: u16) -> Self {
        self.padding = Some((x, y));
        self
    }

    /// Set horizontal and vertical margin.
    #[must_use]
    pub fn margin(mut self, x: u16, y: u16) -> Self {
        self.margin = Some((x, y));
        self
    }

    /// Set border type.
    #[must_use]
    pub fn border(mut self, border: BorderType) -> Self {
        self.border = Some(border);
        self
    }

    /// Build the final [`ratatui::style::Style`].
    ///
    /// # Panics
    ///
    /// Tier 2 — not implemented yet.
    #[must_use]
    pub fn build(self) -> ratatui::style::Style {
        let _ = (
            self.fg,
            self.bg,
            self.bold,
            self.italic,
            self.underline,
            self.padding,
            self.margin,
            self.border,
        );
        todo!("Tier 2: StyleBuilder::build")
    }
}

impl Default for StyleBuilder {
    fn default() -> Self {
        Self::new()
    }
}
