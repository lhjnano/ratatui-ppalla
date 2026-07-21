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

### TODO
- Tier 2 implementations.
- Async `Command`/`Program` event loop.
- Integration tests and a working `examples/demo.rs`.

[Unreleased]: https://github.com/lhjnano/ratatui-bubbles/compare/HEAD
