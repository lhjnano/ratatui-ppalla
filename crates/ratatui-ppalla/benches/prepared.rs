//! # Criterion benchmarks for the prepared primitives.
//!
//! Measures the ROADMAP Direction-2 acceptance criteria for the
//! [`PreparedText`] / [`PreparedLayout`] hot and cold paths:
//!
//! - **Group A** — `PreparedText::prepare_str` cold path (grapheme segmentation +
//!   width caching). Expected to be the slowest single operation.
//! - **Group B** — `PreparedText::layout` hot path (pure arithmetic over cached
//!   widths). Acceptance: **<= 0.5 ms** for 1000 lines x 80 columns.
//! - **Group C** — `PreparedLayout` cache miss vs cache hit. Acceptance:
//!   **cache hit <= 0.01 ms**.
//! - **Group D** — Comparison: ratatui `Paragraph` + `Wrap` vs
//!   `PreparedText::layout`, illustrating the layout-only hot-path advantage.
//! - **Group E** — `PreparedList::layout` hot path (filter-index windowing
//!   over 1000 filtered items).
//! - **Group F** — `PreparedTable::layout` hot path (sort-permutation
//!   windowing over 1000 sorted rows).
//! - **Group G** — `PreparedViewport::layout` hot path (match-flag windowing
//!   over 1000 searchable lines).
//! - **Group H** — `PreparedBuffer::layout` hot path (dirty-row damage-rect
//!   merging), 50%-dirty vs all-dirty.
//!
//! All input is generated deterministically by [`make_lines`] and the sibling
//! `make_*` helpers so results are reproducible. `criterion::black_box` guards
//! every result to prevent dead-code elimination.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ratatui::backend::TestBackend;
use ratatui::layout::Constraint;
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use ratatui_ppalla::prepared::{
    BufferInput, LayoutCtx, ListInput, Preparable, PreparedBuffer, PreparedLayout, PreparedList,
    PreparedTable, PreparedText, PreparedViewport, SortSpec, TableColumn, TableInput,
    ViewportInput,
};

/// Build `n` deterministic lines, each exactly `width` ASCII columns wide.
///
/// Words are repeated to fill the column budget, then truncated to `width`, so
/// the same `(n, width)` always yields byte-identical input. ASCII-only keeps
/// grapheme segmentation trivial and isolates the layout/wrap hot path.
fn make_lines(n: usize, width: usize) -> String {
    const WORDS: &str = "the quick brown fox jumps over the lazy dog ";
    std::iter::repeat_with(|| {
        let mut line = String::with_capacity(width);
        while line.len() < width {
            line.push_str(WORDS);
        }
        line.truncate(width);
        line
    })
    .take(n)
    .collect::<Vec<String>>()
    .join("\n")
}

/// Build `n` deterministic list items (`"item_0000"` .. `"item_{n-1}"`). The
/// same `n` always yields byte-identical input.
fn make_items(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("item_{i:04}")).collect()
}

/// Build `n` rows of `cols` deterministic cells. Cell `(i, j)` is
/// `"r{iiii}c{jjjj}"`, so the same `(n, cols)` always yields byte-identical
/// input.
fn make_rows(n: usize, cols: usize) -> Vec<Vec<String>> {
    (0..n)
        .map(|i| (0..cols).map(|j| format!("r{i:04}c{j:04}")).collect())
        .collect()
}

/// Build `n` deterministic lines, each exactly `width` ASCII columns wide, as
/// a `Vec<String>` (the un-joined counterpart of [`make_lines`]). Used where a
/// line buffer (not a single joined string) is the input shape.
fn make_lines_v(n: usize, width: usize) -> Vec<String> {
    const WORDS: &str = "the quick brown fox jumps over the lazy dog ";
    std::iter::repeat_with(|| {
        let mut line = String::with_capacity(width);
        while line.len() < width {
            line.push_str(WORDS);
        }
        line.truncate(width);
        line
    })
    .take(n)
    .collect()
}

// ===== Group A — PreparedText cold path (prepare) =====

/// Measure `PreparedText::prepare_str`: splitting on `'\n'`, segmenting each
/// line into grapheme clusters, and caching every cluster's Unicode width.
/// This is the one-time cold-path cost paid only when data changes.
fn bench_prepare_text(c: &mut Criterion) {
    let input = make_lines(1000, 80);
    c.bench_function("prepare_text/1000x80", |b| {
        b.iter(|| {
            let state = PreparedText::prepare_str(black_box(&input));
            black_box(state);
        });
    });
}

// ===== Group B — PreparedText hot path (layout) =====

/// Measure `PreparedText::layout`: wrapping cached segments into display lines
/// using pure integer arithmetic. The input is prepared **once** outside the
/// timed loop (the cold path is measured separately in Group A).
///
/// ACCEPTANCE: <= 0.5 ms for 1000 lines x 80 columns at an 80x24 viewport.
fn bench_layout_text(c: &mut Criterion) {
    let input = make_lines(1000, 80);
    // Prepare once: the cold path is excluded from this measurement.
    let prepared = PreparedText::prepare_str(&input);
    let ctx = LayoutCtx::new(80, 24);
    c.bench_function("layout_text/1000x80", |b| {
        b.iter(|| {
            let layout = PreparedText::layout(&prepared, black_box(ctx));
            black_box(layout);
        });
    });
}

// ===== Group C — PreparedLayout cache miss vs cache hit =====

/// Compare a cache miss (fresh cold state -> ratatui `Layout::split` solver
/// runs) against a cache hit (same args -> cached rects returned).
///
/// ACCEPTANCE: cache hit <= 0.01 ms. The miss path always re-evaluates the
/// constraints because `iter_batched` builds a fresh, cold state each
/// iteration (setup is untimed).
fn bench_layout_cache(c: &mut Criterion) {
    let constraints = vec![
        Constraint::Length(3),
        Constraint::Percentage(50),
        Constraint::Min(0),
        Constraint::Fill(1),
    ];
    let ctx = LayoutCtx::new(80, 24);

    let mut group = c.benchmark_group("layout_cache");

    // Miss: every iteration recomputes via the ratatui solver.
    group.bench_function("miss", |b| {
        b.iter_batched(
            || PreparedLayout::prepare_vertical(constraints.clone()),
            |state| {
                let out = PreparedLayout::layout(&state, ctx);
                black_box(out);
            },
            BatchSize::SmallInput,
        );
    });

    // Hit: warm the cache once, then every call returns the cached rects.
    let warm = PreparedLayout::prepare_vertical(constraints.clone());
    let _ = PreparedLayout::layout(&warm, ctx); // populate the 1-entry cache
    group.bench_function("hit", |b| {
        b.iter(|| {
            let out = PreparedLayout::layout(&warm, ctx);
            black_box(out);
        });
    });

    group.finish();
}

// ===== Group D — ratatui Paragraph+Wrap vs PreparedText::layout =====

/// Compare ratatui's `Paragraph::new(text).wrap(Wrap { trim: false })` render
/// (wrap + paint) against `PreparedText::layout` (Group B).
///
/// The `Paragraph` is built **once** and rendered by reference each iteration,
/// so the timed loop measures only the per-frame wrap+paint cost — not the
/// string-to-`Text` conversion. Note that `Paragraph` does strictly more work
/// than `PreparedText::layout`: it both wraps AND paints every cell into the
/// buffer, whereas `PreparedText::layout` performs layout only (no painting).
/// The comparison therefore illustrates the advantage of caching the expensive
/// work in the cold path and leaving the hot path as pure arithmetic.
fn bench_comparison(c: &mut Criterion) {
    let text = make_lines(1000, 80);

    // Build once: the string -> Text conversion happens here, outside timing.
    let paragraph = Paragraph::new(text.as_str()).wrap(Wrap { trim: false });
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");

    c.bench_function("paragraph_wrap/1000x80", |b| {
        b.iter(|| {
            terminal
                .draw(|f: &mut Frame<'_>| {
                    f.render_widget(&paragraph, f.area());
                })
                .expect("draw");
            black_box(terminal.backend().buffer());
        });
    });
}

// ===== Group E — PreparedList hot path (layout) =====

/// Measure `PreparedList::layout`: walking the cached filter index and cloning
/// only the items that land in the `[scroll, scroll + height)` window. The
/// input (1000 items, filter matching all) is prepared **once** outside timing;
/// the hot path reports `total` (1000) but copies only the `height` visible
/// items.
fn bench_layout_list(c: &mut Criterion) {
    let prepared = PreparedList::prepare(ListInput {
        items: make_items(1000),
        // "item" is a substring of every generated item -> all 1000 visible.
        filter: "item".to_string(),
    });
    let ctx = LayoutCtx::new(80, 24);
    c.bench_function("layout_list/1000x24", |b| {
        b.iter(|| {
            let layout = PreparedList::layout(&prepared, black_box(ctx));
            black_box(layout);
        });
    });
}

// ===== Group F — PreparedTable hot path (layout) =====

/// Measure `PreparedTable::layout`: walking the cached sort permutation and
/// materialising only the visible rows (cell truncation by char count). The
/// input (1000 rows x 3 columns, ascending sort on column 0) is prepared
/// **once** outside timing; the O(n log n) sort runs in the cold path, so the
/// hot path only windows `[scroll, scroll + height)` of the permutation.
fn bench_layout_table(c: &mut Criterion) {
    let prepared = PreparedTable::prepare(TableInput {
        rows: make_rows(1000, 3),
        columns: vec![
            TableColumn::new("a", 10),
            TableColumn::new("b", 10),
            TableColumn::new("c", 10),
        ],
        sort: Some(SortSpec {
            column: 0,
            ascending: true,
        }),
    });
    let ctx = LayoutCtx::new(80, 24);
    c.bench_function("layout_table/1000x24", |b| {
        b.iter(|| {
            let layout = PreparedTable::layout(&prepared, black_box(ctx));
            black_box(layout);
        });
    });
}

// ===== Group G — PreparedViewport hot path (layout) =====

/// Measure `PreparedViewport::layout`: windowing the line buffer by
/// scroll/height and flagging matches in view via the cached match indices.
/// The input (1000 lines, query "the" matching all of them) is prepared
/// **once** outside timing. The hot path walks only the visible window but
/// performs a membership test against the full match-index vector per visible
/// line, so the cost also reflects the total match count.
fn bench_layout_viewport(c: &mut Criterion) {
    let prepared = PreparedViewport::prepare(ViewportInput {
        lines: make_lines_v(1000, 80),
        // "the" appears in every generated line -> all 1000 match.
        query: Some("the".to_string()),
    });
    let ctx = LayoutCtx::new(80, 24);
    c.bench_function("layout_viewport/1000x24", |b| {
        b.iter(|| {
            let layout = PreparedViewport::layout(&prepared, black_box(ctx));
            black_box(layout);
        });
    });
}

// ===== Group H — PreparedBuffer hot path (layout / damage-rect merging) =====

/// Measure `PreparedBuffer::layout`: scanning the per-row dirty flags and
/// merging adjacent dirty rows into full-width damage rects.
///
/// Two sub-cases contrast the merge cost:
/// - **50% dirty**: 12 of 24 contiguous rows dirty -> one merged band.
/// - **all dirty**: every row dirty (the post-`prepare` state) -> one full
///   band.
///
/// Both grids are prepared **once** outside timing. `layout` takes the
/// prepared state by shared reference and never mutates the dirty flags, so
/// every iteration measures the same pure linear scan over the flag vector.
fn bench_layout_buffer(c: &mut Criterion) {
    let ctx = LayoutCtx::new(80, 24);

    // 50% dirty: clear the post-prepare flags, then dirty the first 12 rows.
    let mut half = PreparedBuffer::prepare(BufferInput {
        width: 80,
        height: 24,
    });
    half.clear_dirty();
    for y in 0..12u16 {
        half.set_cell(0, y, "x");
    }

    // all dirty: the state straight out of `prepare` flags every row.
    let all = PreparedBuffer::prepare(BufferInput {
        width: 80,
        height: 24,
    });

    let mut group = c.benchmark_group("layout_buffer");
    group.bench_function("80x24_50pct_dirty", |b| {
        b.iter(|| {
            let layout = PreparedBuffer::layout(&half, black_box(ctx));
            black_box(layout);
        });
    });
    group.bench_function("80x24_all_dirty", |b| {
        b.iter(|| {
            let layout = PreparedBuffer::layout(&all, black_box(ctx));
            black_box(layout);
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_prepare_text,
    bench_layout_text,
    bench_layout_cache,
    bench_comparison,
    bench_layout_list,
    bench_layout_table,
    bench_layout_viewport,
    bench_layout_buffer,
);
criterion_main!(benches);
