# Roadmap — Bubble Tea → Ratatui Migration

## Overview

Phase 1 ports the most useful features from the Go
[Bubble Tea](https://github.com/charmbracelet/bubbletea) / [Bubbles](https://github.com/charmbracelet/bubbles)
/ [Lipgloss](https://github.com/charmbracelet/lipgloss) ecosystem into the Rust
[Ratatui](https://ratatui.rs) ecosystem. Work is organized into three tiers:

- **Tier 1** — foundation. The widgets and abstractions that everything else builds on; these must
  work and be tested first.
- **Tier 2** — the next wave. Public API is frozen so downstream code can depend on the shape, but
  method bodies are `todo!()` stubs pending implementation.
- **Tier 3** — stretch goals. Noted here so the design space is visible, but not started.

The guiding principle throughout is **idiomatic Rust over faithful Go parity**: we borrow the mental
model and widget vocabulary, not the exact type signatures.

## Tier table

| Component | Bubble Tea source | Rust target module | Status | Priority |
| --- | --- | --- | --- | --- |
| Elm architecture | [bubbletea](https://github.com/charmbracelet/bubbletea) | `elm` | ✅ Scaffold | P0 |
| Filterable list | [bubbles/list](https://github.com/charmbracelet/bubbles/list) | `list` | ✅ Scaffold | P0 |
| Scrollable viewport | [bubbles/viewport](https://github.com/charmbracelet/bubbles/viewport) | `viewport` | ✅ Scaffold | P0 |
| Multi-line text input | [bubbles/textarea](https://github.com/charmbracelet/bubbles/textarea) | `text_input` | ✅ Scaffold | P0 |
| Spinner | [bubbles/spinner](https://github.com/charmbracelet/bubbles/spinner) | `spinner` | 🚧 Stub | P1 |
| Sortable table | [bubbles/table](https://github.com/charmbracelet/bubbles/table) | `table` | 🚧 Stub | P1 |
| Key help | [bubbles/key](https://github.com/charmbracelet/bubbles/key) | `key_help` | 🚧 Stub | P1 |
| Lipgloss-style builder | [lipgloss](https://github.com/charmbracelet/lipgloss) | `style` | 🚧 Stub | P1 |
| File picker | [bubbles/filepicker](https://github.com/charmbracelet/bubbles/filepicker) | (new) `filepicker` | ⏳ Not started | P2 |
| Progress bar | [bubbles/progress](https://github.com/charmbracelet/bubbles/progress) | (new) `progress` | ⏳ Not started | P2 |
| Timer | [bubbles/timer](https://github.com/charmbracelet/bubbles/timer) | (new) `timer` | ⏳ Not started | P2 |

## Milestones

- **M1** — Workspace + Tier 1 scaffolds (DONE, this commit)
- **M2** — Tier 1 full implementations with edge-case tests
- **M3** — Tier 2 implementations
- **M4** — Real async `Command`/`Program` event loop with crossterm
- **M5** — Tier 3 stretch components

## Non-goals

- Modifying Ratatui's rendering engine itself — we build *on top of* `Frame` and its buffer.
- Replacing crossterm as the default backend.
- Perfect 1:1 API parity with Bubble Tea — idiomatic Rust wins when the two conflict.

## Open questions

- Should the async `Command` use [tokio](https://tokio.rs), [async-std](https://async.rs), or a
  runtime-agnostic trait? The workspace already declares a `tokio` dependency, but the decision is
  not final.
- Should we depend on existing community crates like
  [`tui-textarea`](https://crates.io/crates/tui-textarea) for `text_input`, or roll our own to keep
  the API aligned with the rest of this crate?
- How do we handle terminal size-change events (SIGWINCH / `Resize`) idiomatically under the Elm
  `Program` loop — as a `Message`, a side-effecting `Command`, or a separate event channel?
