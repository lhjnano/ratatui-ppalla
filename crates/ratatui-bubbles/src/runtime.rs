//! Program runtime — a synchronous crossterm event loop that drives an
//! Elm-architecture [`Model`](crate::elm::Model).
//!
//! This is a port of [Bubble Tea's `tea.Program`](https://github.com/charmbracelet/bubbletea)
//! event loop, but synchronous (no async runtime) and built on
//! [`ratatui::Terminal`] with the crossterm backend.
//!
//! Implement the [`App`] trait and pass your type to [`run`] to launch the
//! program. The runner handles:
//!
//! - terminal setup (raw mode, alternate screen, mouse capture) and teardown
//! - crossterm event polling with a small timeout
//! - feeding events through [`App::on_event`] to produce messages
//! - calling [`Model::update`](crate::elm::Model::update) for each message
//! - draining any [`Command::Msg`](crate::elm::Command::Msg) / `Command::Batch` returned by update
//! - calling [`Model::view`](crate::elm::Model::view) to render every iteration
//! - exiting when [`App::should_quit`] returns true

// Terminal/event-loop code returns `io::Result` pervasively; documenting every
// fn with a `# Errors` section adds noise without value here.
#![allow(clippy::missing_errors_doc)]

use std::io::{self, stdout, Stdout};
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::elm::{Command, Model};

/// A terminal-frontend program that can be run by [`run`].
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

/// Run `program` until [`App::should_quit`] returns true or an IO error occurs.
///
/// Takes over the terminal for the duration of the call: enables raw mode,
/// enters the alternate screen, enables mouse capture. Restores the original
/// terminal state on exit (including on error).
pub fn run<P: App>(program: &mut P) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let result = main_loop(program, &mut terminal);
    restore_terminal(&mut terminal)?;
    result
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

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

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

fn main_loop<P: App>(
    program: &mut P,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> io::Result<()> {
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
        if event::poll(poll_timeout)? {
            let ev = event::read()?;
            if let Some(msg) = program.on_event(ev) {
                pending.push(msg);
            }
        }
    }
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
}
