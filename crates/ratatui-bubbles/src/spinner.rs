//! Spinner widget — a port of [Bubbles' `spinner`](https://github.com/charmbracelet/bubbles/spinner).
//!
//! Provides an animated spinner widget with named presets mirroring the upstream
//! Bubbles spinner set.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// A named spinner visual style, mirroring the upstream Bubbles presets.
///
/// Each variant corresponds to a fixed sequence of animation frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerStyle {
    /// `|/-\\` rotating bar.
    Line,
    /// Bouncing dots.
    Dot,
    /// Smaller bouncing dots.
    MiniDot,
    /// Jumping dots.
    Jump,
    /// Pulsing line.
    Pulse,
    /// Filling meter `█▉▊▋▌▍▎▏` cycle.
    Meter,
    /// Hamburger-ish `≡` cycling.
    Hamburger,
    /// Three dots cycling.
    Ellipsis,
}

impl SpinnerStyle {
    /// Returns the animation frames (slice of &str) for this style.
    #[must_use]
    pub const fn frames(self) -> &'static [&'static str] {
        match self {
            Self::Line => &["|", "/", "-", "\\"],
            Self::Dot => &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
            Self::MiniDot => &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            Self::Jump => &["⢄", "⢂", "⢁", "⡈", "⡐", "⡠"],
            Self::Pulse => &["█", "▓", "▒", "░"],
            Self::Meter => &["█", "▉", "▊", "▋", "▌", "▍", "▎", "▏"],
            Self::Hamburger => &["☱", "☲", "☴", "☲"],
            Self::Ellipsis => &["  ", ". ", ".. ", "..."],
        }
    }
}

/// A spinner widget.
///
/// Holds the active style, the current frame index, and a target frame rate
/// in frames-per-second. Call [`Spinner::tick`] periodically to advance the
/// animation.
#[derive(Debug, Clone)]
pub struct Spinner {
    style: SpinnerStyle,
    frame: usize,
    fps: u32,
}

impl Spinner {
    /// Create a new spinner with the given style, starting at frame 0.
    /// Default FPS is 10.
    #[must_use]
    pub fn new(style: SpinnerStyle) -> Self {
        Self {
            style,
            frame: 0,
            fps: 10,
        }
    }

    /// Sets the target FPS (frames per second).
    #[must_use]
    pub const fn with_fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }

    /// Current FPS value.
    #[must_use]
    pub const fn fps(&self) -> u32 {
        self.fps
    }

    /// Advance the spinner by one frame, wrapping around.
    pub fn tick(&mut self) {
        let frames = self.style.frames();
        if !frames.is_empty() {
            self.frame = (self.frame + 1) % frames.len();
        }
    }

    /// Returns the text of the current frame.
    #[must_use]
    pub fn current_frame(&self) -> &str {
        let frames = self.style.frames();
        if frames.is_empty() {
            ""
        } else {
            frames[self.frame]
        }
    }

    /// Renders the spinner as a single yellow span in the top-left of `area`.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let span = Span::styled(self.current_frame(), Style::default().fg(Color::Yellow));
        frame.render_widget(Paragraph::new(Line::from(span)), area);
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new(SpinnerStyle::Line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_style_has_four_frames() {
        assert_eq!(SpinnerStyle::Line.frames().len(), 4);
    }

    #[test]
    fn new_spinner_starts_at_frame_zero() {
        let s = Spinner::new(SpinnerStyle::Dot);
        assert_eq!(s.frame, 0);
        assert_eq!(s.current_frame(), "⣾");
    }

    #[test]
    fn tick_advances_and_wraps() {
        let mut s = Spinner::new(SpinnerStyle::Line);
        for expected in ["/", "-", "\\", "|", "/"] {
            s.tick();
            assert_eq!(s.current_frame(), expected);
        }
    }

    #[test]
    fn with_fps_sets_fps() {
        let s = Spinner::new(SpinnerStyle::Dot).with_fps(30);
        assert_eq!(s.fps(), 30);
    }
}
