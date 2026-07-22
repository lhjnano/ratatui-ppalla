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
/// # use ratatui_ppalla::elm::{flatten, Command};
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

    #[test]
    fn flatten_unwraps_deeply_nested_batches() {
        // Batch(Batch(Batch([Tick]))) — three levels deep, one leaf.
        let cmd: Command<()> = Command::Batch(vec![Command::Batch(vec![Command::Batch(vec![
            Command::Tick,
        ])])]);
        let flat = flatten(cmd);
        assert_eq!(
            flat.len(),
            1,
            "deeply nested batch should flatten to 1 leaf"
        );
        assert!(
            matches!(flat[0], Command::Tick),
            "the single leaf should be Tick"
        );
    }

    #[test]
    fn command_none_is_none_returns_true() {
        assert!(
            Command::<()>::none().is_none(),
            "Command::none() must be None"
        );
        assert!(
            !Command::<()>::Tick.is_none(),
            "Tick must not be considered None"
        );
    }

    #[test]
    fn command_default_is_none() {
        // `Command` does not derive `PartialEq`, so we assert via `is_none()`
        // (which performs the same structural match the prompt's `==` intended).
        assert!(
            Command::<()>::default().is_none(),
            "Default::default() must produce Command::None"
        );
    }

    #[test]
    fn batch_with_empty_vec_still_a_batch() {
        // An empty Vec must still be a Batch variant (not collapsed to None).
        let cmd: Command<()> = Command::batch(Vec::new());
        assert!(
            matches!(cmd, Command::Batch(ref inner) if inner.is_empty()),
            "Command::batch(Vec::new()) must remain Command::Batch([])"
        );
    }

    #[test]
    fn msg_command_carries_payload() {
        let cmd: Command<u8> = Command::msg(42u8);
        assert!(
            matches!(cmd, Command::Msg(42)),
            "Command::msg(42) must carry its payload unchanged"
        );
    }

    #[test]
    fn flatten_of_none_returns_single_element() {
        // `None` is a leaf, so flatten yields a single-element Vec.
        let flat = flatten(Command::<()>::None);
        assert_eq!(
            flat.len(),
            1,
            "flatten(None) should produce exactly one leaf"
        );
        assert!(matches!(flat[0], Command::None));
    }

    #[test]
    fn flatten_of_msg_returns_single_element() {
        let flat = flatten(Command::msg(5i32));
        assert_eq!(
            flat.len(),
            1,
            "flatten(Msg) should produce exactly one leaf"
        );
        assert!(matches!(flat[0], Command::Msg(5)));
    }

    #[test]
    fn flatten_of_tick_returns_single_element() {
        let flat = flatten(Command::<()>::Tick);
        assert_eq!(
            flat.len(),
            1,
            "flatten(Tick) should produce exactly one leaf"
        );
        assert!(matches!(flat[0], Command::Tick));
    }

    #[test]
    fn program_init_can_be_called_via_trait_object() {
        // Define a tiny Program implementation and verify init() is callable.
        struct StubProgram;
        impl Program for StubProgram {
            type Msg = ();
            fn init(&mut self) -> Command<Self::Msg> {
                Command::none()
            }
        }
        let mut p = StubProgram;
        let cmd = Program::init(&mut p);
        assert!(cmd.is_none());
    }
}
