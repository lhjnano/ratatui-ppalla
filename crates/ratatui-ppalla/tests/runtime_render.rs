//! End-to-end test for [`ratatui_ppalla::runtime::main_loop`] using an
//! injected [`TestBackend`] and a scripted [`EventSource`].
//!
//! Exercises the runtime's full message-drain -> render -> event-poll loop
//! without touching the real terminal.

use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal};
use ratatui_ppalla::elm::{Command, Model};
use ratatui_ppalla::runtime::{main_loop, App};
use ratatui_ppalla::test_utils::ScriptedEventSource;

// ============================================================
// Counter App -- copied from examples/demo.rs (private there),
// simplified to a plain `Count: N` render for easy assertions.
// ============================================================

#[derive(Debug, Default)]
struct Counter {
    count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Increment,
    Decrement,
    Quit,
}

impl Model<Msg> for Counter {
    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
            Msg::Quit => {}
        }
        Command::none()
    }

    fn view(&self, frame: &mut Frame, area: Rect) {
        let text = format!("Count: {}", self.count);
        let block = Block::default().borders(Borders::ALL).title("Counter");
        let paragraph = Paragraph::new(text).block(block);
        frame.render_widget(paragraph, area);
    }
}

impl App for Counter {
    type Msg = Msg;

    fn init(&mut self) -> Command<Msg> {
        Command::none()
    }

    fn on_event(&mut self, event: Event) -> Option<Msg> {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            match code {
                KeyCode::Char('+') | KeyCode::Up => return Some(Msg::Increment),
                KeyCode::Char('-') | KeyCode::Down => return Some(Msg::Decrement),
                KeyCode::Char('q') | KeyCode::Esc => return Some(Msg::Quit),
                _ => {}
            }
        }
        None
    }

    fn should_quit(&self, msg: &Msg) -> bool {
        matches!(msg, Msg::Quit)
    }
}

// ============================================================
// Helpers
// ============================================================

fn key_event(code: KeyCode) -> Event {
    Event::Key(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    })
}

/// Drive `program` through `main_loop` with a TestBackend and a scripted
/// event queue. Returns the final buffer for assertions.
fn run_program(program: &mut Counter, events: Vec<Event>) -> io::Result<Buffer> {
    let backend = TestBackend::new(30, 5);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut source = ScriptedEventSource::new(events);
    main_loop(program, &mut terminal, &mut source)?;
    Ok(terminal.backend().buffer().clone())
}

fn row_symbols(buf: &Buffer, y: usize, len: usize) -> String {
    (0..len)
        .map(|x| buf[(x as u16, y as u16)].symbol().to_string())
        .collect()
}

// ============================================================
// Tests
// ============================================================

#[test]
fn counter_starts_at_zero_and_renders() {
    // A single Quit event: the initial render still shows count = 0 before
    // the Quit message is drained and the loop exits cleanly.
    let mut counter = Counter::default();
    let events = vec![key_event(KeyCode::Char('q'))];
    let buf = run_program(&mut counter, events).expect("ran");

    // The Counter renders "Count: 0" inside a bordered block.
    // Row 1 (inside the border) should contain "Count: 0".
    let row1 = row_symbols(&buf, 1, 30);
    assert!(
        row1.contains("Count: 0"),
        "expected 'Count: 0' in row 1, got: {row1:?}"
    );
}

#[test]
fn increment_events_increase_count() {
    let mut counter = Counter::default();
    // 3 increments then quit.
    let events = vec![
        key_event(KeyCode::Char('+')),
        key_event(KeyCode::Char('+')),
        key_event(KeyCode::Char('+')),
        key_event(KeyCode::Char('q')),
    ];
    let buf = run_program(&mut counter, events).expect("ran");

    let row1 = row_symbols(&buf, 1, 30);
    assert!(
        row1.contains("Count: 3"),
        "expected 'Count: 3' in row 1, got: {row1:?}"
    );
}

#[test]
fn decrement_events_decrease_count() {
    let mut counter = Counter::default();
    // 2 decrements then quit.
    let events = vec![
        key_event(KeyCode::Char('-')),
        key_event(KeyCode::Char('-')),
        key_event(KeyCode::Char('q')),
    ];
    let buf = run_program(&mut counter, events).expect("ran");

    let row1 = row_symbols(&buf, 1, 30);
    assert!(
        row1.contains("Count: -2"),
        "expected 'Count: -2' in row 1, got: {row1:?}"
    );
}

#[test]
fn mixed_events_net_to_correct_count() {
    let mut counter = Counter::default();
    // +5, -2 -> net +3
    let events = vec![
        key_event(KeyCode::Up),
        key_event(KeyCode::Up),
        key_event(KeyCode::Up),
        key_event(KeyCode::Up),
        key_event(KeyCode::Up),
        key_event(KeyCode::Down),
        key_event(KeyCode::Down),
        key_event(KeyCode::Esc),
    ];
    let buf = run_program(&mut counter, events).expect("ran");

    let row1 = row_symbols(&buf, 1, 30);
    assert!(
        row1.contains("Count: 3"),
        "expected 'Count: 3' in row 1, got: {row1:?}"
    );
}

#[test]
fn quit_event_exits_cleanly() {
    let mut counter = Counter::default();
    let events = vec![key_event(KeyCode::Char('q'))];
    let result = run_program(&mut counter, events);
    assert!(result.is_ok(), "main_loop should exit cleanly on Quit");
}

#[test]
fn exhaustion_breaks_loop_without_quit() {
    // No Quit event -- exhaustion should propagate as UnexpectedEof and
    // cause run_program to return Err.
    let mut counter = Counter::default();
    let events = vec![key_event(KeyCode::Char('+'))];
    let result = run_program(&mut counter, events);
    assert!(
        result.is_err(),
        "expected Err on exhaustion, got: {result:?}"
    );
    let err = result.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
}
