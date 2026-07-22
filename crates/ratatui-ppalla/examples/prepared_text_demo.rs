//! PreparedText demo — scroll through wrapped text using the Preparable pattern.
//!
//! Run with: `cargo run --example prepared_text_demo`
//!
//! Keys: `↓`/`↑` scroll · `q`/`Esc` quit
//!
//! This demo shows the full prepare → layout → paint pipeline:
//! - `prepare_str` runs **once** (cold path: grapheme segmentation + width caching)
//! - `layout` runs **every frame** (hot path: pure arithmetic over cached widths)
//! - `TextLayout::paint` bridges the result into a ratatui `Buffer`

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use ratatui_ppalla::elm::{Command, Model};
use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedText, PreparedTextState};
use ratatui_ppalla::runtime::{run, App};

/// A simple scrollable text viewer backed by a prepared text primitive.
struct TextViewer {
    prepared: PreparedTextState,
    scroll: usize,
}

const SAMPLE: &str = "\
ppalla (빨라) means 'fast' in Korean.

This demo shows the Preparable pattern, the namesake feature of ratatui-ppalla:

  1. prepare()   — cold path, runs once when the text changes. It segments
                   the input into grapheme clusters and caches each cluster's
                   Unicode display width.

  2. layout()    — hot path, runs every frame. It wraps the cached segments
                   into display lines using pure integer arithmetic (no
                   Unicode work) and windows the result by scroll/height.

  3. paint()     — render bridge. It walks the visible display lines and
                   writes each segment's grapheme into a ratatui Buffer,
                   advancing by the cached width.

Because the expensive work is isolated in prepare(), the per-frame cost is
just arithmetic + a bounded number of clones for the visible window. On the
benchmark suite, PreparedText::layout(1000 lines x 80 cols) runs in ~134us
— about 0.8% of the 16.67ms 60fps frame budget.

Use the arrow keys to scroll through this text. Press q or Esc to quit.
";

impl Model<()> for TextViewer {
    fn update(&mut self, _msg: ()) -> Command<()> {
        Command::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" PreparedText demo — ↑/↓ scroll, q quit ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Hot path: lay out the prepared text for the current viewport, then
        // paint the result into the frame's buffer.
        let ctx = LayoutCtx::new(inner.width, inner.height).with_scroll(self.scroll);
        let layout = PreparedText::layout(&self.prepared, ctx);
        layout.paint(frame.buffer_mut(), inner);
    }
}

impl App for TextViewer {
    type Msg = ();

    fn init(&mut self) -> Command<()> {
        Command::none()
    }

    fn on_event(&mut self, event: Event) -> Option<()> {
        let Event::Key(KeyEvent { code, .. }) = event else {
            return None;
        };
        match code {
            KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Char('q') | KeyCode::Esc => return Some(()),
            _ => {}
        }
        None
    }

    fn should_quit(&self, _msg: &()) -> bool {
        true
    }
}

fn main() -> std::io::Result<()> {
    let mut viewer = TextViewer {
        prepared: PreparedText::prepare_str(SAMPLE),
        scroll: 0,
    };
    run(&mut viewer)
}
