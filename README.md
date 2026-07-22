# ratatui-ppalla

[![CI](https://img.shields.io/github/actions/workflow/status/lhjnano/ratatui-ppalla/ci.yml?branch=main&logo=github)](https://github.com/lhjnano/ratatui-ppalla/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ratatui-ppalla?logo=rust&label=crates.io)](https://crates.io/crates/ratatui-ppalla)
[![docs.rs](https://img.shields.io/docsrs/ratatui-ppalla?logo=docsdotrs&label=docs.rs)](https://docs.rs/ratatui-ppalla)
[![license](https://img.shields.io/crates/l/ratatui-ppalla?logo=opensourcehardware&label=license)](#license)

> High-performance TUI primitives for [Ratatui] — Pretext-inspired prepare/layout separation.

`ppalla` (빨라) means **"fast"** in Korean — and performance is the point.
`ratatui-ppalla` brings the ergonomics of the [Bubble Tea](https://github.com/charmbracelet/bubbletea)
component model to Rust, and adds a **Preparable pattern** that separates
expensive one-time work from cheap per-frame work, enabling smooth 60fps
multi-pane TUI applications.

## Why ppalla? — the Preparable pattern

The namesake feature is the **Preparable pattern**, inspired by
[Pretext](https://github.com/chenglou/pretext): split expensive one-time
`prepare()` (cold path) from cheap per-frame `layout()` (hot path).

Because every TUI text width is predetermined by its Unicode width, the prepare
step is cheap, and the layout hot path is pure arithmetic over cached widths —
no Unicode work, ideally no allocation.

```rust
/// Core abstraction: prepare once (cold), layout many (hot).
pub trait Preparable {
    type Prepared: Clone;  // opaque, cheap-to-clone cached state
    type Layout;           // per-frame result
    type Input;            // what triggers re-preparation

    fn prepare(input: Self::Input) -> Self::Prepared;          // cold path
    fn append(prepared: &mut Self::Prepared, more: Self::Input); // optional
    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout; // hot path
}
```

The same pattern generalizes across text, lists, tables, viewports, layout
regions, and damage-tracked buffers — see the modules below.

## Performance

Measured with [`criterion`](https://docs.rs/criterion); full methodology and the
optimization journey in [`docs/benchmarks/baseline.md`](docs/benchmarks/baseline.md).

| Primitive | `layout()` hot path | vs 60fps budget (16.67ms) |
|---|---|---|
| `PreparedText` (1000×80 wrap) | **134 µs** | 0.8% |
| `PreparedLayout` (cache hit) | **54 ns** | 0.0003% |
| `PreparedList` (1000 items) | **848 ns** | 0.005% |
| `PreparedTable` (1000 rows) | **2.78 µs** | 0.017% |
| `PreparedViewport` (1000 lines) | **478 ns** | 0.003% |
| `PreparedBuffer` (80×24 damage) | **37 ns** | 0.0002% |

`PreparedText::layout` went **2.83ms → 134µs (21× faster)** via windowed cloning
— the hot path walks all lines for `total_lines` but clones grapheme `String`s
only for the visible `[scroll, scroll+height)` window.

## Quick start

```rust
use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedText};

// Cold path: segment into graphemes + cache Unicode widths (once, on data change).
let prepared = PreparedText::prepare_str("hello world\nppalla is fast");

// Hot path: wrap to width + window by scroll/height (every frame, pure arithmetic).
let layout = PreparedText::layout(&prepared, LayoutCtx::new(10, 3));
for line in &layout.lines {
    for seg in &line.segments {
        // seg.grapheme (String) + seg.width (u16) — ready to paint
    }
}
```

## Modules

**Prepared primitives** (`prepared::`) — the Preparable pattern:

- [`PreparedText`](crates/ratatui-ppalla/src/prepared/prepared_text.rs) — grapheme segmentation + Unicode width caching + visible-line wrapping
- [`PreparedLayout`](crates/ratatui-ppalla/src/prepared/prepared_layout.rs) — 1-entry cache of ratatui constraint evaluation
- [`PreparedList`](crates/ratatui-ppalla/src/prepared/prepared_list.rs) — case-insensitive filter index + visible-item windowing
- [`PreparedTable`](crates/ratatui-ppalla/src/prepared/prepared_table.rs) — sort permutation + column widths + visible-row windowing
- [`PreparedViewport`](crates/ratatui-ppalla/src/prepared/prepared_viewport.rs) — search-match index + scroll window + match flags
- [`PreparedBuffer`](crates/ratatui-ppalla/src/prepared/prepared_buffer.rs) — per-row dirty tracking + merged damage rects

**Widgets** — Bubble Tea/Bubbles-style, built on Ratatui:

- [`elm`](crates/ratatui-ppalla/src/elm.rs) — Elm architecture (Model/Update/View + Command/Message)
- [`list`](crates/ratatui-ppalla/src/list.rs) — filterable, selectable list
- [`viewport`](crates/ratatui-ppalla/src/viewport.rs) — scrollable viewport with search
- [`text_input`](crates/ratatui-ppalla/src/text_input.rs) — multi-line text input with history
- [`spinner`](crates/ratatui-ppalla/src/spinner.rs) — spinner with built-in styles
- [`table`](crates/ratatui-ppalla/src/table.rs) — sortable, navigable table
- [`key_help`](crates/ratatui-ppalla/src/key_help.rs) — key binding help display
- [`style`](crates/ratatui-ppalla/src/style.rs) — Lipgloss-style style builder

**Runtime** — synchronous crossterm event loop with an injectable backend
(testable via `TestBackend`).

## Installation

```sh
cargo add ratatui-ppalla
```

Requires Rust **1.74+** (declared MSRV).

## `tui-inject` — test tool

A companion CLI for inspecting, rendering, snapshotting, replaying, recording,
fuzzing, and benchmarking widgets without a real terminal.

```sh
cargo run -p tui-inject -- list                          # enumerate widgets
cargo run -p tui-inject -- render list --filter "an"     # render with filter
cargo run -p tui-inject -- snapshot list -o list.html    # save snapshot
cargo run -p tui-inject -- replay scenarios/counter-increment.toml
cargo run -p tui-inject -- fuzz counter --events 1000    # random-event stress
cargo run -p tui-inject -- bench list --iterations 1000  # render perf
```

See [`crates/tui-inject/README.md`](crates/tui-inject/README.md) for full docs.

## Status

**v0.0.1 — early development.** 392 tests across 13 suites, ~85% workspace line
coverage. The API may change between `0.0.x` releases. See the
[benchmark baseline](docs/benchmarks/baseline.md) for measured performance.

## Why

The Rust TUI ecosystem has [Ratatui](https://ratatui.rs) as an excellent
rendering layer, but it deliberately stays low-level: you get a `Frame`, a
buffer, and a set of primitive widgets, and you build everything else yourself.
The Go world, by contrast, has Bubble Tea plus its companion libraries — a
cohesive component model, an Elm-style update loop, and a declarative styling
DSL. `ratatui-ppalla` fills that gap and adds the Preparable pattern for
high-frequency rendering.

## Inspiration

- **[Pretext](https://github.com/chenglou/pretext)** — DOM-free text layout engine; the prepare/layout separation this crate generalizes
- **[Bubble Tea](https://github.com/charmbracelet/bubbletea)** — Elm-architecture TUI runtime
- **[Bubbles](https://github.com/charmbracelet/bubbles)** — the component library being ported
- **[Lipgloss](https://github.com/charmbracelet/lipgloss)** — declarative terminal styling
- **[Ratatui](https://ratatui.rs)** — the Rust rendering layer this crate builds on

## License

Dual-licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

## Contributing

Contributions are welcome! See [`CONTRIBUTING.md`](CONTRIBUTING.md) for
development setup, code style, and how to add a new widget port.

[Ratatui]: https://ratatui.rs
