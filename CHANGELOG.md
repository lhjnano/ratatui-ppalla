# Changelog

All notable changes to ratatui-bubbles will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial workspace scaffold (`Cargo.toml` workspace + `crates/ratatui-bubbles` member crate).
- Tier 1 modules with functional scaffolding: `elm` (Elm architecture traits), `list` (filterable list), `viewport` (searchable scrollable viewport), `text_input` (multi-line text input with history).
- Tier 2 stub modules with frozen public API: `spinner`, `table`, `key_help`, `style`.
- 24 unit tests covering Tier 1 behavior.
- M2: edge-case unit tests for all Tier 1 modules (list, viewport, text_input, elm) — 22 new tests.
- M3: full implementations of all Tier 2 modules — spinner (8 SpinnerStyle presets with frame arrays), table (sort/navigate/select/render), key_help (render + with_title + len/is_empty), style (StyleBuilder.build() composes Style).
- M4: `runtime` module with synchronous crossterm event loop runner (`App` trait + `run()` function).
- M4: `examples/demo.rs` counter app showcasing the runtime (arrow keys, +/-, r reset, q/Esc quit).
- 4 new unit tests for runtime drain_messages behavior.

### Changed
- `spinner`, `table`, `key_help`, `style` modules upgraded from Tier 2 stubs to full implementations (no more `todo!()` in any module).
- Updated lib.rs module list to remove "Tier 2 — stubbed" annotations and reflect the new `runtime` module.
- Total test count is now ~76 across 6+ suites.

### TODO
- Tier 2 implementations.
- Async `Command`/`Program` event loop.
- Integration tests and a working `examples/demo.rs`.

[Unreleased]: https://github.com/lhjnano/ratatui-bubbles/compare/HEAD
