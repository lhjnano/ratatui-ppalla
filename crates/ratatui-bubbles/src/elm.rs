//! Elm architecture for Ratatui applications.
//!
//! A Rust port of the [`Bubble Tea`](https://github.com/charmbracelet/bubbletea)
//! `tea.Model` / `tea.Msg` / `tea.Cmd` pattern, adapted for terminal rendering
//! with [`Ratatui`](https://ratatui.rs).
//!
//! The Elm Architecture models a TUI program as a pure [`Model`] that reacts to
//! messages of type `Msg` via [`Model::update`], returning a new state together
//! with a [`Command`] describing the side-effects to run. The view is a pure
//! function of the model ([`Model::view`]).
//!
//! This module provides the core abstractions only. The actual event loop —
//! terminal setup, input polling, command dispatch — is intentionally left to
//! the host application.

use ratatui::layout::Rect;
use ratatui::Frame;

/// A command returned by [`Model::update`].
///
/// Commands describe the side-effects to run after a message is processed.
/// Because a full async runtime is intentionally out of scope for this port,
/// `Command` is modelled as a simple tree of messages rather than as a real
/// future.
#[derive(Debug, Clone, Default)]
pub enum Command<Msg> {
    /// No side-effect.
    #[default]
    None,
    /// Emit a single message synchronously.
    Msg(Msg),
    /// Run several commands together (logically concurrent).
    Batch(Vec<Command<Msg>>),
    /// A placeholder for a timer/tick command (e.g. an animation frame).
    Tick,
}

impl<Msg> Command<Msg> {
    /// Create an empty command (equivalent to [`Command::None`]).
    #[must_use]
    pub const fn none() -> Self {
        Command::None
    }

    /// Wrap a single message into a command.
    #[must_use]
    pub const fn msg(m: Msg) -> Self {
        Command::Msg(m)
    }

    /// Combine several commands into a single [`Command::Batch`].
    #[must_use]
    pub const fn batch(cmds: Vec<Command<Msg>>) -> Self {
        Command::Batch(cmds)
    }

    /// Returns `true` when this command performs no work.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Command::None)
    }
}

/// The core Elm Architecture trait.
///
/// Implementors own the application state (`self`), react to messages of type
/// `Msg` in [`Model::update`], and render themselves in [`Model::view`].
pub trait Model<Msg> {
    /// React to `msg`, mutating internal state and returning a [`Command`]
    /// describing any side-effects to run afterwards.
    fn update(&mut self, msg: Msg) -> Command<Msg>;

    /// Render the current state into `frame` within `area`.
    fn view(&self, frame: &mut Frame<'_>, area: Rect);
}

/// Entry-point abstraction for a runnable Elm program.
///
/// This trait deliberately omits the real event loop — it exists only to anchor
/// the `init` lifecycle that a host runtime would drive. Wiring terminal setup,
/// input polling, and command dispatch is the host application's responsibility.
pub trait Program: Sized {
    /// The message type the program processes.
    type Msg;

    /// Initialise the program and return the first [`Command`] to run.
    fn init(&mut self) -> Command<Self::Msg>;
}

/// Recursively flatten a [`Command`] tree into an ordered list of leaves.
///
/// Every [`Command::Batch`] node is unwrapped one level at a time; the
/// remaining (`None` / `Msg` / `Tick`) leaves are collected in traversal order.
/// The result is never itself a `Batch`.
///
/// # Examples
///
/// ```
/// # use ratatui_bubbles::elm::{flatten, Command};
/// let cmd: Command<()> = Command::Batch(vec![
///     Command::Tick,
///     Command::Batch(vec![Command::None]),
/// ]);
/// assert_eq!(flatten(cmd).len(), 2);
/// ```
#[allow(clippy::missing_const_for_fn)] // recurses + allocates; cannot be `const`
#[must_use]
pub fn flatten<Msg>(cmd: Command<Msg>) -> Vec<Command<Msg>> {
    fn recurse<Msg>(cmd: Command<Msg>, out: &mut Vec<Command<Msg>>) {
        match cmd {
            Command::Batch(cmds) => {
                for c in cmds {
                    recurse(c, out);
                }
            }
            // Any non-Batch variant is a leaf (None / Msg / Tick).
            leaf => out.push(leaf),
        }
    }

    let mut out = Vec::new();
    recurse(cmd, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_unwraps_nested_batches() {
        let cmd: Command<()> =
            Command::Batch(vec![Command::Tick, Command::Batch(vec![Command::None])]);
        // `Tick` and the inner `None` are the two leaves.
        assert_eq!(flatten(cmd).len(), 2);
    }
}
