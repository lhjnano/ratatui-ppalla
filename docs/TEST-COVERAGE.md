# Test Coverage Heatmap

Coverage status of every public API in `ratatui-bubbles`, broken down by test layer.

Each public item was enumerated by reading the source of every module under
`crates/ratatui-bubbles/src/`, and each coverage mark was verified by reading
the in-module `#[cfg(test)] mod tests` block and the integration tests under
`crates/ratatui-bubbles/tests/`. No mark is guessed.

**Test count:** 98 tests across 9 suites (78 unit, 19 integration, 1 doc test).

**Legend:**

- ✅ Unit test (in-module `#[cfg(test)] mod tests`)
- 🟢 Integration test (`tests/*.rs` using `TestBackend`)
- 🔴 No test coverage
- ⚠️ Partial (e.g. only happy path, no edge cases)

**Scope note:** Derived `Default` impls (`Spinner`, `KeyHelp`, `StyleBuilder`,
`Command`, `TextInput`) are part of the public surface but are auto-generated;
their coverage is noted in each struct's row rather than counted as separate
items. `lib.rs` contains only `pub mod` declarations and is listed for
completeness with zero items of its own.

---

## Summary

| Module      | Pub items | ✅ Unit | 🟢 Integration | 🔴 Untested | Coverage % |
|-------------|-----------|---------|-----------------|-------------|------------|
| elm         | 8         | 7       | 5               | 0           | 100%       |
| list        | 12        | 11      | 7               | 0           | 100%       |
| viewport    | 15        | 14      | 7               | 0           | 100%       |
| text_input  | 17        | 16      | 5               | 0           | 100%       |
| spinner     | 9         | 8       | 5               | 0           | 100%       |
| table       | 13        | 12      | 7               | 0           | 100%       |
| key_help    | 11        | 10      | 8               | 0           | 100%       |
| style       | 11        | 11      | 0               | 0           | 100%       |
| runtime     | 3         | 1       | 0               | 2           | 33%        |
| lib         | 0         | —       | —               | —           | n/a        |
| **Total**   | **99**    | **90**  | **44**          | **2**       | **98%**    |

**How the columns are counted:** "✅ Unit" and "🟢 Integration" count items
covered by *at least one* test in that layer (an item may appear in both).
"🔴 Untested" counts items covered by *neither* layer. "Coverage %" is
`(items − untested) / items`. `lib.rs` re-exports modules only and is excluded
from the total.

**Previous state (before Task 3 coverage pass):** 99 items, 78 ✅, 23 🟢,
17 🔴, 83%. The coverage pass added 22 tests (9 integration + 13 unit),
closing 15 of 17 untested items and upgrading 1 ⚠️ partial to full ✅.

---

## elm

| Item             | Kind    | Unit | Integration | Notes |
|------------------|---------|------|-------------|-------|
| `Command<Msg>`   | enum    | ✅   | 🟢          | `None`/`Msg`/`Batch`/`Tick` all exercised; derives `Default` (covered by `command_default_is_none`) |
| `Command::none`  | method  | ✅   | 🟢          | const fn |
| `Command::msg`   | method  | ✅   |             | const fn |
| `Command::batch` | method  | ✅   | 🟢          | const fn |
| `Command::is_none` | method | ✅   |             | const fn |
| `Model<Msg>`     | trait   | 🔴   | 🟢          | no in-module impl; exercised via `Counter` in `tests/elm_render.rs` (`update` + `view`) |
| `Program`        | trait   | ✅   |             | `init` exercised via `StubProgram` in `program_init_can_be_called_via_trait_object` |
| `flatten`        | fn      | ✅   | 🟢          | nested + deeply-nested batches, single-leaf variants all tested |

## list

| Item               | Kind    | Unit | Integration | Notes |
|--------------------|---------|------|-------------|-------|
| `ListItem`         | trait   | ✅   | 🟢          | `render` + `filterable_text`; impls in `Row` (unit) and `Task` (`tests/list_render.rs`) |
| `List<T>`          | struct  | ✅   | 🟢          | private fields; constructed via `new` everywhere |
| `List::new`        | method  | ✅   | 🟢          | |
| `List::selected`   | method  | ✅   |             | |
| `List::select_next`| method  | ✅   | 🟢          | |
| `List::select_prev`| method  | ✅   |             | |
| `List::set_filter` | method  | ✅   | 🟢          | case-insensitive, unicode, no-match, survive-filter paths all covered |
| `List::filter`     | method  | ✅   |             | |
| `List::len`        | method  | ✅   |             | |
| `List::is_empty`   | method  | ✅   |             | |
| `List::filtered_len`| method | ✅   | 🟢          | |
| `List::render`     | method  | 🔴   | 🟢          | rendered via `TestBackend` in `tests/list_render.rs` (items, filter, highlight) |

## viewport

| Item                 | Kind   | Unit | Integration | Notes |
|----------------------|--------|------|-------------|-------|
| `Viewport`           | struct | ✅   | 🟢          | private fields; no `Default` |
| `Viewport::new`      | method | ✅   | 🟢          | |
| `Viewport::height`   | method | ✅   |             | covered by `height_returns_configured_value` |
| `Viewport::set_height` | method | ✅ |             | covered by `set_height_updates_value` |
| `Viewport::append_line` | method | ✅ | 🟢          | |
| `Viewport::set_lines`| method | ✅   |             | covered by `set_lines_replaces_content`; resets content + line count |
| `Viewport::line_count` | method | ✅ |             | |
| `Viewport::scroll_down` | method | ✅ | 🟢          | clamp-to-max covered |
| `Viewport::scroll_up` | method | ✅ |             | clamp-at-zero covered |
| `Viewport::offset`   | method | ✅   |             | |
| `Viewport::set_search` | method | ✅ | 🟢          | case-insensitive, clear (`None`), empty-string clear |
| `Viewport::match_count` | method | ✅ | 🟢        | |
| `Viewport::next_match` | method | ✅ |             | advance + wrap covered |
| `Viewport::prev_match` | method | ✅ |             | covered by `prev_match_scrolls_to_earlier_match`; mirrors `next_match` |
| `Viewport::render`   | method | 🔴   | 🟢          | rendered via `TestBackend` in `tests/viewport_render.rs` |

## text_input

| Item                  | Kind   | Unit | Integration | Notes |
|-----------------------|--------|------|-------------|-------|
| `TextInput`           | struct | ✅   | 🟢          | private fields; `Default` impl covered by `default_equals_new` |
| `TextInput::new`      | method | ✅   | 🟢          | |
| `TextInput::value`    | method | ✅   |             | |
| `TextInput::cursor`   | method | ✅   |             | |
| `TextInput::insert_char` | method | ✅ |            | incl. multibyte |
| `TextInput::insert_str` | method | ✅ | 🟢          | embedded newlines |
| `TextInput::backspace`| method | ✅   |             | incl. line-merge at col 0, no-op at origin |
| `TextInput::enter`    | method | ✅   | 🟢          | |
| `TextInput::move_left`| method | ✅   |             | wrap + clamp |
| `TextInput::move_right`| method | ✅ |             | wrap + clamp |
| `TextInput::move_up`  | method | ✅   |             | clamp col |
| `TextInput::move_down`| method | ✅   |             | clamp col |
| `TextInput::clear`    | method | ✅   |             | keeps history |
| `TextInput::submit`   | method | ✅   |             | push + clear |
| `TextInput::history_prev` | method | ✅ |           | walk + no-op on empty |
| `TextInput::history_next` | method | ✅ |           | walk + restore saved buffer |
| `TextInput::render`   | method | 🔴   | 🟢          | rendered via `TestBackend` in `tests/text_input_render.rs` |

## spinner

| Item                    | Kind   | Unit | Integration | Notes |
|-------------------------|--------|------|-------------|-------|
| `SpinnerStyle`          | enum   | ✅   | 🟢          | all 8 variants exercised by `all_spinner_styles_have_non_empty_frames`; `Line`/`Dot` also in integration tests |
| `SpinnerStyle::frames`  | method | ✅   |             | all 8 variants' frame counts + non-empty strings asserted by `all_spinner_styles_have_non_empty_frames` |
| `Spinner`               | struct | ✅   | 🟢          | private fields; `Default` impl (`SpinnerStyle::Line`) not directly tested |
| `Spinner::new`          | method | ✅   | 🟢          | |
| `Spinner::with_fps`     | method | ✅   |             | const fn |
| `Spinner::fps`          | method | ✅   |             | const fn |
| `Spinner::tick`         | method | ✅   | 🟢          | advance + wrap |
| `Spinner::current_frame`| method | ✅   |             | |
| `Spinner::render`       | method | 🔴   | 🟢          | rendered via `TestBackend` in `tests/spinner_render.rs` (Line frame 0, tick advance, Dot first frame) |

## table

| Item              | Kind    | Unit | Integration | Notes |
|-------------------|---------|------|-------------|-------|
| `Column`          | struct  | ✅   | 🟢          | pub fields `title`, `width`; constructed via `new` |
| `Column::new`     | method  | ✅   | 🟢          | |
| `Row`             | trait   | ✅   | 🟢          | `cells()` via `TestRow` impl (unit) and `Person` (`tests/table_render.rs`) |
| `Table<R>`        | struct  | ✅   | 🟢          | pub fields `columns`/`rows`/`selected`/`sort_column`/`sort_ascending`/`scroll_offset` |
| `Table::new`      | method  | ✅   | 🟢          | |
| `Table::set_rows` | method  | ✅   | 🟢          | resets selection + scroll |
| `Table::len`      | method  | ✅   |             | covered by `len_returns_row_count` |
| `Table::is_empty` | method  | ✅   |             | covered by `is_empty_after_construction` |
| `Table::selected` | method  | ✅   |             | |
| `Table::select_next` | method | ✅  |             | clamp at last row |
| `Table::select_prev` | method | ✅  |             | clamp at first row |
| `Table::sort_by`  | method  | ✅   |             | toggle direction on same column |
| `Table::render`   | method  | 🔴   | 🟢          | rendered via `TestBackend` in `tests/table_render.rs` (header, data rows, empty) |

## key_help

| Item                | Kind    | Unit | Integration | Notes |
|---------------------|---------|------|-------------|-------|
| `KeyBinding`        | struct  | ✅   | 🟢          | pub fields `key`/`desc`/`disabled`; constructed via `new` |
| `KeyBinding::new`   | method  | ✅   | 🟢          | |
| `KeyBinding::disabled` | method | ✅  | 🟢          | builder; excluded from rendered output |
| `KeyHelp`           | struct  | ✅   | 🟢          | private fields; `Default` derive not directly tested |
| `KeyHelp::new`      | method  | ✅   | 🟢          | |
| `KeyHelp::with_title` | method | ✅   | 🟢          | |
| `KeyHelp::add`      | method  | ✅   | 🟢          | |
| `KeyHelp::len`      | method  | ✅   |             | counts enabled only |
| `KeyHelp::is_empty` | method  | ✅   |             | |
| `KeyHelp::full_help`| method  | ✅   |             | |
| `KeyHelp::render`   | method  | 🔴   | 🟢          | rendered via `TestBackend` in `tests/key_help_render.rs` (title, bindings, disabled exclusion) |

## style

| Item                     | Kind   | Unit | Integration | Notes |
|--------------------------|--------|------|-------------|-------|
| `StyleBuilder`           | struct | ✅   |             | private fields; `Default` impl not directly tested |
| `StyleBuilder::new`      | method | ✅   |             | |
| `StyleBuilder::foreground` | method | ✅ |            | |
| `StyleBuilder::background` | method | ✅ |            | covered by `background_sets_bg` |
| `StyleBuilder::bold`     | method | ✅   |             | |
| `StyleBuilder::italic`   | method | ✅   |             | |
| `StyleBuilder::underline`| method | ✅   |             | covered by `underline_adds_underlined_modifier` |
| `StyleBuilder::padding`  | method | ✅   |             | stored but not applied by `build`; `padding_is_stored_without_affecting_build` documents the contract |
| `StyleBuilder::margin`   | method | ✅   |             | stored but not applied; `margin_is_stored_without_affecting_build` documents the contract |
| `StyleBuilder::border`   | method | ✅   |             | stored but not applied; `border_is_stored_without_affecting_build` documents the contract |
| `StyleBuilder::build`    | method | ✅   |             | ignores padding/margin/border by design |

## runtime

| Item             | Kind   | Unit | Integration | Notes |
|------------------|--------|------|-------------|-------|
| `App`            | trait  | 🔴   |             | `init`/`on_event`/`should_quit` never exercised; only drivable through `run` |
| `run`            | fn     | 🔴   |             | takes over the terminal (raw mode, alt screen, mouse capture) — needs a PTY or mock-backend refactor |
| `drain_messages` | fn     | ✅   |             | `None`, `Msg`, nested `Batch` all covered |

## lib

| Item | Kind | Unit | Integration | Notes |
|------|------|------|-------------|-------|
| —    | —    | —    | —           | `pub mod` declarations only; no items of its own |

---

## Gap Analysis

Items still 🔴 untested after the coverage pass, classified by the reason they're deferred and the harness that would be needed to test them.

### Deferred — needs PTY / real terminal

These items take over the terminal (raw mode, alternate screen) and cannot be exercised via TestBackend alone. A PTY-based harness like [`expectrl`](https://crates.io/crates/expectrl) or refactoring to inject a trait-object Backend would unblock them.

| Item | Module | Why deferred | Required harness |
|------|--------|-------------|------------------|
| `runtime::run` | runtime | takes over stdout via crossterm raw mode + alt screen | `expectrl` (spawn binary, send keys, read PTY output) or refactor `run` to accept `&mut Terminal<dyn Backend>` |
| `runtime::App` trait | runtime | only meaningful when driven by `run` | same — once `run` is testable, App is exercised end-to-end |
| `runtime::main_loop` | runtime | private helper of `run`, owns the event poll loop | refactor to take an event-source trait, then test with a scripted event iterator |
| `runtime::setup_terminal` / `restore_terminal` | runtime | side-effectful IO on stdout | extract IO behind a trait, or accept as integration test only |
| `examples/demo.rs` Counter app | example | full TUI app lifecycle | `expectrl` — run `cargo run --example demo`, send arrow keys, assert visible count changes, send 'q' to quit |

### Deferred — needs async runtime

(None currently — runtime is synchronous, no tokio dependency in production code.)

### Acceptable — not worth testing

These items are intentionally not unit-tested because they're trivial pass-throughs, derived impls, or would be redundant.

| Item | Module | Why acceptable |
|------|--------|---------------|
| `Default` impls (Spinner, KeyHelp, StyleBuilder, TextInput, Command) | various | thin wrappers around `new()`; the constructor is already tested |
| `Column::constraint`, `Column::header_cell` | table | private helpers used only by `Table::render` which is integration-tested |
| `drain_messages` recursion edge cases beyond current 3 tests | runtime | current tests cover None/Msg/Batch including nested — further cases are diminishing returns |

### Recommended next steps (priority order)

1. **Highest ROI**: introduce `expectrl` as a dev-dependency and write ONE end-to-end test of `examples/demo.rs` — exercises Counter Model + App trait + runtime::run + crossterm event loop + cleanup all in one shot. If this passes, 4 of the 5 deferred items above get covered transitively.
2. **Medium ROI**: refactor `runtime::run` to accept `&mut Terminal<B>` where `B: Backend`, so a TestBackend can be injected. Then a "headless" smoke test of main_loop becomes possible.
3. **Low ROI**: snapshot testing via `insta` for complex layouts (list+viewport+text_input together) — already a dev-dependency, just unused.

### How to regenerate this document

This heatmap is maintained by hand. When you add or remove public API, update both the per-module tables AND the Summary counts. Run:

```bash
# Verify the matrix is accurate by cross-checking test counts:
cargo test -p ratatui-bubbles 2>&1 | tail -5
# Per-module:
for m in elm list viewport text_input spinner table key_help style runtime; do
  cargo test -p ratatui-bubbles --lib "$m::" 2>&1 | grep -oE '[0-9]+ passed' | head -1 | xargs -I{} echo "src/$m.rs: {}"
done
# Integration:
for t in list_render viewport_render text_input_render elm_render spinner_render table_render key_help_render; do
  cargo test -p ratatui-bubbles --test "$t" 2>&1 | grep -oE '[0-9]+ passed' | head -1 | xargs -I{} echo "tests/$t.rs: {}"
done
```
