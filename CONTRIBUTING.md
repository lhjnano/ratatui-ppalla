# Contributing to ratatui-ppalla

Thanks for your interest in `ratatui-ppalla`! This crate is an early-stage port of the
[Bubble Tea](https://github.com/charmbracelet/bubbletea) / [Bubbles](https://github.com/charmbracelet/bubbles)
component library to [Ratatui](https://ratatui.rs), and there is plenty of room to help — new widget
ports, better docs, edge-case tests, and the eventual async `Command`/`Program` loop all need hands.

This document explains how to get a local build running and how to contribute a new widget port.

## Getting Started

You'll need a recent stable Rust toolchain (the workspace currently targets edition 2021).

```sh
git clone https://github.com/lhjnano/ratatui-ppalla
cd ratatui-ppalla
cargo build
cargo test
```

The workspace contains a single member crate at `crates/ratatui-ppalla`. There is a placeholder
example at the workspace root in `examples/demo.rs` — it will not be fully wired up until the async
runtime lands.

## Code Style

The crate is compiled with `#![warn(missing_docs)]` and `#![warn(clippy::all, clippy::pedantic)]`,
so two gates must pass before any contribution can merge:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
```

- Every public item **must** carry a `///` doc comment. Module-level docs (`//!`) are required at
  the top of each `src/*.rs` file — they are what `docs.rs` renders.
- Prefer idiomatic Rust: traits over enums-of-closures, `#[must_use]` on constructors that return
  owned data, `&str` over `String` in argument positions.
- Follow the existing naming: free functions for helpers, methods on the main type for behavior.

## Adding a widget

The workflow for porting a new [Bubbles](https://github.com/charmbracelet/bubbles) component is
deliberately incremental so that the public API can be reviewed before any behavior is written:

1. **Read the source.** Open the upstream Go package (e.g.
   [`bubbles/spinner`](https://github.com/charmbracelet/bubbles/tree/master/spinner)) and understand
   its public API surface, state, and update messages. Note where Lipgloss rendering happens — in
   this crate we delegate drawing to Ratatui primitives instead.
2. **Freeze the public API.** Add the module under `crates/ratatui-ppalla/src/<name>.rs`, declare it
   in `src/lib.rs`, and write out the public types, traits, and method signatures. Bodies can be
   `todo!()` at this stage. The point is to agree on the shape before writing logic.
3. **Stub with `todo!()`.** Each unimplemented method returns `todo!()`. This keeps the crate
   compiling (with `cargo build -p ratatui-ppalla`) while signaling that work remains. Tier 2
   modules (`spinner`, `table`, `key_help`, `style`) are currently in this state.
4. **Implement with tests.** Replace `todo!()` bodies one method at a time, adding a unit test per
   behavior in the module's `#[cfg(test)] mod tests`. Filter/navigation/wrap logic especially
   benefits from property-style tests. See `src/list.rs` for the established pattern.

When in doubt about idiomatic Rust vs. faithful Go parity, **idiomatic Rust wins.** The Phase 1 plan
calls this out as an explicit non-goal.

## Testing

- Every module ships at least one unit test in its own `#[cfg(test)] mod tests` block. The Tier 1
  modules (`elm`, `list`, `viewport`, `text_input`) currently account for 24 tests — use them as a
  template for coverage expectations.
- Integration tests that drive a real terminal session belong in `tests/` at the workspace root, but
  those are blocked on the async `Command`/`Program` loop landing first.
- Run the whole suite with `cargo test`, or a single module with
  `cargo test -p ratatui-ppalla --lib list`.

## Commit style

This repository follows [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — a new widget or capability
- `fix:` — a bug fix
- `docs:` — documentation only (README, doc comments, Phase 1 plan)
- `test:` — adding or correcting tests
- `refactor:` — code change that neither fixes a bug nor adds a feature

Keep commits focused: one logical change per commit, and prefer smaller PRs that are easy to review.

## Licensing

By contributing, you agree that your work will be licensed under the same dual license as the rest of
the crate — **MIT OR Apache-2.0** — as described in the [README](README.md#license). If your
contribution is substantial, you may add yourself to the authors list, but no separate contributor
license agreement is required.
