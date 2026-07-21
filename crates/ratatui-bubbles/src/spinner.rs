//! Spinner widget — a port of [Bubbles' `spinner`](https://github.com/charmbracelet/bubbles/spinner).
//!
//! # Status
//!
//! **Tier 2 — not yet implemented.** This is a stub module that defines the
//! intended public API. All non-trivial methods panic with [`todo!()`].

#![allow(dead_code)]
#![allow(clippy::missing_panics_doc)]

/// A named spinner visual style, mirroring the upstream Bubbles presets.
///
/// Tier 2 stub — see module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerStyle {
    /// `|/-\` rotating bar.
    Line,
    /// Bouncing dots.
    Dot,
    /// Smaller bouncing dots.
    MiniDot,
    /// Jumping dots.
    Jump,
    /// Pulsing line.
    Pulse,
    /// Filling meter.
    Meter,
    /// Hamburger-ish.
    Hamburger,
    /// Three dots.
    Ellipsis,
}

/// A spinner widget.
///
/// Tier 2 stub — see module docs. All methods panic via `todo!()`.
#[derive(Debug, Clone)]
pub struct Spinner {
    style: SpinnerStyle,
    frame: usize,
    fps: u32,
}

impl Spinner {
    /// Create a new spinner with the given style.
    ///
    /// # Panics
    ///
    /// Tier 2 — not implemented yet.
    #[must_use]
    pub fn new(style: SpinnerStyle) -> Self {
        let _ = style;
        todo!("Tier 2: Spinner::new")
    }

    /// Advance the spinner by one frame.
    ///
    /// # Panics
    ///
    /// Tier 2 — not implemented yet.
    pub fn tick(&mut self) {
        todo!("Tier 2: Spinner::tick")
    }

    /// Current frame text.
    ///
    /// # Panics
    ///
    /// Tier 2 — not implemented yet.
    #[must_use]
    pub fn current_frame(&self) -> &str {
        todo!("Tier 2: Spinner::current_frame")
    }
}
