//! Counter demo for `ratatui_bubbles`.
//!
//! Run with: `cargo run --example demo`
//!
//! Keys:
//! - ←/↑ or + : increment
//! - →/↓ or - : decrement
//! - r        : reset to 0
//! - q / Esc  : quit

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ratatui_bubbles::elm::{Command, Model};
use ratatui_bubbles::runtime::{run, App};

#[derive(Debug, Default)]
struct Counter {
    count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Increment,
    Decrement,
    Reset,
    Quit,
}

impl Model<Msg> for Counter {
    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
            Msg::Reset => self.count = 0,
            Msg::Quit => {}
        }
        Command::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let title = Span::styled(
            " ratatui-bubbles counter demo ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

        let count_line = Line::from(vec![
            Span::raw("  Count: "),
            Span::styled(
                self.count.to_string(),
                Style::default()
                    .fg(if self.count >= 0 {
                        Color::Green
                    } else {
                        Color::Red
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let help = Line::from(vec![
            Span::raw("  "),
            Span::styled("+", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" increment   "),
            Span::styled("-", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" decrement   "),
            Span::styled("r", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" reset   "),
            Span::styled("q/Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" quit"),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center);

        let paragraph = Paragraph::new(vec![
            Line::default(),
            count_line,
            Line::default(),
            Line::default(),
            help,
        ])
        .block(block);

        frame.render_widget(paragraph, area);
    }
}

impl App for Counter {
    type Msg = Msg;

    fn init(&mut self) -> Command<Msg> {
        Command::none()
    }

    fn on_event(&mut self, event: Event) -> Option<Msg> {
        let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = event
        else {
            return None;
        };
        // Ctrl+C always quits.
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            return Some(Msg::Quit);
        }
        match code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('+') => Some(Msg::Increment),
            KeyCode::Right | KeyCode::Down | KeyCode::Char('-') => Some(Msg::Decrement),
            KeyCode::Char('r') => Some(Msg::Reset),
            KeyCode::Char('q') | KeyCode::Esc => Some(Msg::Quit),
            _ => None,
        }
    }

    fn should_quit(&self, msg: &Msg) -> bool {
        matches!(msg, Msg::Quit)
    }
}

fn main() -> std::io::Result<()> {
    let mut counter = Counter::default();
    run(&mut counter)
}
