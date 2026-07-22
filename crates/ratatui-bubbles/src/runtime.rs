//! Program runtime — a synchronous crossterm event loop that drives an
//! Elm-architecture [`Model`](crate::elm::Model).
//!
//! # Testing
//!
//! The production entry point [`run`] takes over the real terminal and cannot
//! be exercised under `cargo-tarpaulin` without a PTY. For testing, use
//! [`run_with`] which accepts an injected [`Terminal`] backend and
//! [`EventSource`], allowing [`main_loop`] to run against a [`TestBackend`]
//! and a scripted event source.
//!
//! [`TestBackend`]: ratatui::backend::TestBackend

#![allow(clippy::missing_errors_doc)]

use std::io::{self, stdout, Stdout};
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;

use crate::elm::{Command, Model};

/// A source of terminal events, abstracting [`crossterm::event::poll`] and
/// [`crossterm::event::read`].
///
/// Object-safe so it can be passed as `&mut dyn EventSource`. The production
/// implementation is [`StdinEventSource`]; tests provide their own impls.
pub trait EventSource {
    /// Wait up to `timeout` for an event to become available. Returns
    /// `Ok(true)` if an event is ready to read, `Ok(false)` on timeout.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;

    /// Read and return the next pending event. Only call after `poll`
    /// returned `Ok(true)`.
    fn read(&mut self) -> io::Result<Event>;
}

/// Production [`EventSource`] reading from stdin via crossterm.
#[derive(Debug, Default)]
pub struct StdinEventSource;

impl EventSource for StdinEventSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }
}

/// A terminal-frontend program that can be run by [`run`] or [`run_with`].
///
/// Composes [`Model`] with two extra hooks needed for an interactive event
/// loop: translating terminal events into messages, and deciding when to exit.
pub trait App: Model<<Self as App>::Msg> {
    /// The message type driven through the loop.
    type Msg;

    /// Returns the initial [`Command`] to run before the first event poll.
    fn init(&mut self) -> Command<Self::Msg>;

    /// Translate a crossterm [`Event`] into a message, or `None` to ignore.
    fn on_event(&mut self, event: Event) -> Option<Self::Msg>;

    /// Whether the given message should cause the program to exit cleanly.
    fn should_quit(&self, msg: &Self::Msg) -> bool;
}

/// Run `program` against the real terminal (raw mode + alt screen + mouse
/// capture via crossterm on stdout).
///
/// Restores the original terminal state on exit (including on error).
/// For testing, use [`run_with`] with an injected backend and event source.
pub fn run<P: App>(program: &mut P) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = main_loop(program, &mut terminal, &mut StdinEventSource);
    restore_terminal(&mut terminal)?;
    result
}

/// Run `program` against an injected [`Terminal`] backend and [`EventSource`].
///
/// This is the testable entry point: pass a `Terminal<TestBackend>` and a
/// scripted event source to exercise [`main_loop`] without touching the real
/// terminal.
pub fn run_with<P, B, E>(
    program: &mut P,
    terminal: &mut Terminal<B>,
    events: &mut E,
) -> io::Result<()>
where
    P: App,
    B: Backend,
    E: EventSource,
{
    main_loop(program, terminal, events)
}

/// Recursively collect every [`Command::Msg`] payload inside `cmd` into `out`.
pub fn drain_messages<Msg>(cmd: Command<Msg>, out: &mut Vec<Msg>) {
    match cmd {
        Command::None | Command::Tick => {}
        Command::Msg(m) => out.push(m),
        Command::Batch(cmds) => {
            for c in cmds {
                drain_messages(c, out);
            }
        }
    }
}

/// The core event loop. Generic over backend `B` and event source `E`.
///
/// Loops: drain pending messages → render → poll for next event → feed event
/// through `App::on_event` → push produced message to pending. Exits when
/// `App::should_quit` returns true.
pub fn main_loop<P, B, E>(
    program: &mut P,
    terminal: &mut Terminal<B>,
    events: &mut E,
) -> io::Result<()>
where
    P: App,
    B: Backend,
    E: EventSource,
{
    let mut pending: Vec<P::Msg> = Vec::new();
    let init_cmd = program.init();
    drain_messages(init_cmd, &mut pending);

    let poll_timeout = Duration::from_millis(20);

    loop {
        // Drain any pending messages produced by previous updates.
        while let Some(msg) = pending.pop() {
            let quit = program.should_quit(&msg);
            let cmd = program.update(msg);
            if quit {
                return Ok(());
            }
            drain_messages(cmd, &mut pending);
        }

        // Render after every update batch.
        terminal.draw(|frame| program.view(frame, frame.area()))?;

        // Poll for the next event with a short timeout so pending messages
        // get serviced promptly.
        if events.poll(poll_timeout)? {
            let ev = events.read()?;
            if let Some(msg) = program.on_event(ev) {
                pending.push(msg);
            }
        }
    }
}

/// Enter raw mode, switch to the alternate screen, enable mouse capture.
///
/// # Requires TTY
///
/// Calls [`enable_raw_mode`] which needs a controlling terminal. Will fail
/// under `cargo-tarpaulin` and other headless test runners — exercise via
/// [`run_with`] + [`TestBackend`] in tests instead.
///
/// [`TestBackend`]: ratatui::backend::TestBackend
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Disable raw mode, leave the alternate screen, disable mouse capture.
///
/// # Requires TTY
///
/// Same constraint as [`setup_terminal`].
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_none_yields_nothing() {
        let mut out = Vec::<u8>::new();
        drain_messages(Command::<u8>::None, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn drain_msg_pushes_payload() {
        let mut out = Vec::new();
        drain_messages(Command::msg(7u8), &mut out);
        assert_eq!(out, vec![7]);
    }

    #[test]
    fn drain_batch_flattens_recursively() {
        let mut out = Vec::new();
        let cmd = Command::Batch(vec![
            Command::Msg(1u8),
            Command::Batch(vec![Command::Msg(2), Command::Msg(3)]),
            Command::None,
        ]);
        drain_messages(cmd, &mut out);
        assert_eq!(out, vec![1, 2, 3]);
    }

    #[test]
    fn stdin_event_source_is_default_constructible() {
        // Smoke test: StdinEventSource compiles and implements Default.
        // Uses a trait-bound assertion rather than constructing the value,
        // since clippy flags both `::default()` on a unit struct and an unused
        // `let _ =` binding under `-D warnings`.
        fn requires_default<T: Default>() {}
        requires_default::<StdinEventSource>();
    }
}
