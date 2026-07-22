# tui-inject

CLI tool for testing [ratatui-bubbles](../ratatui-bubbles) widgets via event injection —
the TUI equivalent of `zinject` for ZFS. Inspect, render, snapshot, replay, record,
fuzz, and benchmark any widget without touching a real terminal.

## Install

```sh
cargo install --path crates/tui-inject
# or run from source:
cargo run -p tui-inject -- <command>
```

## Commands

### `list` — enumerate widgets

```sh
tui-inject list
```

```
Available widgets:

  list           — Filterable list of items
  viewport       — Scrollable text viewport with search
  text-input     — Multi-line text input with history
  spinner        — Animated spinner with 8 style presets
  table          — Sortable table with selectable rows
  key-help       — Key binding help display
  style-demo     — StyleBuilder demo showing bold/italic/underline
```

### `render <widget>` — render to stdout

```sh
tui-inject render list --filter "an" --width 30 --height 5
tui-inject render table --width 30 --height 8
tui-inject render spinner --ticks 2
tui-inject render list --format html        # HTML output
```

Flags: `--items N`, `--filter X`, `--ticks N`, `--text S`, `--format text|html`, `--width`, `--height`.

### `snapshot <widget> -o file` — save rendered output to a file

```sh
tui-inject snapshot list -o list.html       # format inferred from extension
tui-inject snapshot spinner -o spin.txt --format text
```

### `replay <scenario.toml>` — replay an event script

```sh
tui-inject replay scenarios/counter-increment.toml
tui-inject replay scenarios/counter-html-snapshot.toml > counter.html
```

Scenarios are TOML files describing a widget configuration, a sequence of events,
and an output format. See [`scenarios/`](scenarios/) for examples.

### `record -o scenario.toml` — capture keyboard events interactively

```sh
tui-inject record -o my-scenario.toml
```

On a real terminal, enters raw mode and records keys until `q`/`Esc`/`Ctrl+C`.
In a headless environment, writes a skeleton scenario for manual editing.

### `fuzz <widget> --events N` — inject random events

```sh
tui-inject fuzz counter --events 1000
```

Generates N random key events, drives a Counter app through them with
`catch_unwind` per event, and reports panics + final state. Currently only
`counter` is fuzzable.

### `bench <widget> --iterations N` — measure render time

```sh
tui-inject bench list --iterations 1000
```

Renders the widget N times via `TestBackend` and reports min/mean/p95/max.

```
bench results for 'list' (100 iterations, 60x16):
  min:  261.19 µs
  mean: 296.61 µs
  p95:  391.39 µs
  max:  480.49 µs
```

## TOML Scenario Format

```toml
[widget]
name = "counter"        # currently the only replayable widget
initial = 0             # optional initial state

[[events]]
key = "+"               # simple form: "enter", "up", "q", "ctrl+c", ...

[[events]]
key = "a"               # full form with modifiers
modifiers = ["shift"]

[[events]]
kind = "down"           # mouse events
column = 5
row = 2

[output]
format = "text"         # or "html"
width = 40
height = 6
```

### Key names

- Special: `enter`, `tab`, `backspace`, `delete`, `esc`, `space`
- Arrows: `up`, `down`, `left`, `right`
- Navigation: `home`, `end`, `pageup`, `pagedown`
- Function: `f1`–`f12`
- Single characters: `a`, `+`, `-`, etc.
- Modifiers (prefix form): `ctrl+c`, `shift+a`, `alt+tab`, `super+x`

## License

MIT OR Apache-2.0, same as ratatui-bubbles.
