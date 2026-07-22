//! Testing utilities for ratatui-bubbles.
//!
//! Currently exposes [`ScriptedEventSource`], a test
//! [`EventSource`](crate::runtime::EventSource) that emits events from a
//! pre-loaded queue. Useful for replaying scenarios against widgets without
//! a real terminal — see the `tui-inject` companion binary.

use std::io;
use std::time::Duration;

use crossterm::event::Event;

use crate::runtime::EventSource;

/// A test [`EventSource`] that emits events from a pre-loaded queue.
///
/// Returns `Err(io::ErrorKind::UnexpectedEof)` when the queue is exhausted,
/// which causes [`main_loop`](crate::runtime::main_loop) to exit cleanly via
/// its `?` operator — useful for tests that forget to terminate with a Quit
/// message.
#[derive(Debug, Default)]
pub struct ScriptedEventSource {
    events: Vec<Event>,
    next: usize,
}

impl ScriptedEventSource {
    /// Create a new source pre-loaded with `events`.
    #[must_use]
    pub fn new(events: Vec<Event>) -> Self {
        Self { events, next: 0 }
    }

    /// Create an empty source (immediately exhausted).
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Number of events remaining in the queue.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.events.len().saturating_sub(self.next)
    }
}

impl EventSource for ScriptedEventSource {
    fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
        if self.next < self.events.len() {
            Ok(true)
        } else {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "scripted event queue exhausted",
            ))
        }
    }

    fn read(&mut self) -> io::Result<Event> {
        if self.next < self.events.len() {
            let ev = self.events[self.next].clone();
            self.next += 1;
            Ok(ev)
        } else {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "scripted event queue exhausted",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn empty_source_polls_as_exhausted() {
        let mut s = ScriptedEventSource::empty();
        let err = s.poll(Duration::from_millis(10)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn queue_drains_in_order() {
        let mut s = ScriptedEventSource::new(vec![key(KeyCode::Enter), key(KeyCode::Tab)]);
        assert!(s.poll(Duration::ZERO).unwrap());
        let _ = s.read().unwrap();
        assert!(s.poll(Duration::ZERO).unwrap());
        let _ = s.read().unwrap();
        // Now exhausted
        let err = s.poll(Duration::ZERO).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn remaining_tracks_progress() {
        let mut s = ScriptedEventSource::new(vec![key(KeyCode::Enter), key(KeyCode::Tab)]);
        assert_eq!(s.remaining(), 2);
        let _ = s.read();
        assert_eq!(s.remaining(), 1);
    }
}
