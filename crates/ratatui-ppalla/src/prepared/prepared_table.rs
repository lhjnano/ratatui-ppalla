//! # `PreparedTable` — sortable table with prepare/layout separation.
//!
//! Concrete implementation of the [Pretext](https://github.com/0xradical/Pretext)
//! prepare/layout separation for sortable tables, porting the sort and column
//! logic of the [`crate::table`] widget into the prepared-primitive model. The
//! cold path ([`PreparedTable::prepare`] / [`PreparedTable::append`]) stores the
//! rows and columns and computes a **sort permutation** (`Vec<usize>`) — a
//! mapping from sorted position to original row index — so the source rows are
//! never reordered. The hot path ([`PreparedTable::layout`]) walks the
//! permutation, counts every row for the scroll-clamping `total`, and
//! materialises [`VisibleRow`]s only for the window
//! `[scroll, scroll + height)`.
//!
//! Sorting matches `crate::table::Table::sort_by`: rows are compared by the cell
//! at the sort column using lexicographic `String` ordering (a missing cell is
//! treated as the empty string). The permutation is computed with a **stable**
//! sort, so rows that compare equal keep their insertion order.
//!
//! `ctx.width` and `ctx.focus` are accepted only to satisfy the [`LayoutCtx`]
//! contract; only `ctx.scroll` and `ctx.height` drive row windowing.
//!
//! # Cell truncation
//!
//! Each visible cell is truncated to its column's `width` **by character
//! count** (not Unicode display width), keeping this module logic-only with no
//! extra dependencies. The original (untruncated) cells are always used for
//! sorting. A renderer that needs terminal-width-exact clipping can re-truncate,
//! but for most data the char-count approximation is sufficient.
//!
//! # Examples
//!
//! ```
//! use ratatui_ppalla::prepared::{
//!     LayoutCtx, Preparable, PreparedTable, SortSpec, TableColumn, TableInput,
//! };
//!
//! let input = TableInput {
//!     rows: vec![
//!         vec!["charlie".to_string(), "3".to_string()],
//!         vec!["alice".to_string(), "1".to_string()],
//!         vec!["bob".to_string(), "2".to_string()],
//!     ],
//!     columns: vec![TableColumn::new("name", 10), TableColumn::new("age", 5)],
//!     sort: Some(SortSpec { column: 0, ascending: true }),
//! };
//! let prepared = PreparedTable::prepare(input);
//! let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 10));
//! assert_eq!(layout.total, 3);
//! // Ascending sort on column 0 => first visible row is "alice".
//! assert_eq!(layout.rows[0].cells[0], "alice");
//! ```

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};

/// A column definition: header text plus a fixed width in terminal cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn {
    /// Column header text.
    pub title: String,
    /// Column width in terminal cells.
    pub width: u16,
}

impl TableColumn {
    /// Create a new column with the given title and width.
    #[must_use]
    pub fn new(title: impl Into<String>, width: u16) -> Self {
        Self {
            title: title.into(),
            width,
        }
    }
}

/// Sort specification: which column to sort by and in which direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SortSpec {
    /// Column index to sort by.
    pub column: usize,
    /// `true` = ascending, `false` = descending.
    pub ascending: bool,
}

/// Input for preparing a [`PreparedTable`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableInput {
    /// Rows as cell-strings in column order.
    pub rows: Vec<Vec<String>>,
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Optional sort to apply. `None` keeps insertion order.
    pub sort: Option<SortSpec>,
}

/// Prepared (cold-path) state: rows, columns, active sort, and the cached sort
/// permutation mapping sorted position to original row index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedTableState {
    /// Original rows in insertion order.
    pub rows: Vec<Vec<String>>,
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Active sort (`None` = insertion order).
    pub sort: Option<SortSpec>,
    /// Permutation: the sorted row at position `i` is `rows[permutation[i]]`.
    pub permutation: Vec<usize>,
}

impl PreparedTableState {
    /// Number of original rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// `true` when there are no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// One visible row in a layout result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    /// Index into the original `rows` vector (insertion order).
    pub index: usize,
    /// The row's cells, truncated to their column widths (by character count).
    pub cells: Vec<String>,
}

/// Per-frame layout result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableLayout {
    /// Visible rows, windowed by `[scroll, scroll + height)` in sorted order.
    pub rows: Vec<VisibleRow>,
    /// Column widths (copied from the prepared columns).
    pub column_widths: Vec<u16>,
    /// Total row count (for scroll clamping).
    pub total: usize,
}

impl TableLayout {
    /// Paint the visible rows into `buf` within `area`, using the cached
    /// `column_widths` to place each cell horizontally. Cells are written via
    /// `Buffer::set_string` and may overflow into the next column if longer
    /// than the width (callers should truncate cell strings before prepare if
    /// strict clipping is required).
    ///
    /// This is the render bridge for [`PreparedTable`].
    pub fn paint(
        &self,
        buf: &mut ratatui::buffer::Buffer,
        area: ratatui::layout::Rect,
        style: ratatui::style::Style,
    ) {
        for (row, vis) in self.rows.iter().enumerate() {
            let Ok(y) = u16::try_from(row) else {
                break;
            };
            let Some(y) = area.y.checked_add(y) else {
                break;
            };
            if y >= area.bottom() {
                break;
            }
            let mut x = area.x;
            for (col, cell) in vis.cells.iter().enumerate() {
                if x >= area.right() {
                    break;
                }
                buf.set_string(x, y, cell, style);
                let w = self.column_widths.get(col).copied().unwrap_or(0);
                x = x.saturating_add(w);
            }
        }
    }
}

/// Prepared sortable-table primitive using the prepare/layout separation.
///
/// Implements [`Preparable`]. The input is a [`TableInput`].
/// [`Preparable::prepare`] computes the sort permutation and caches the rows and
/// columns. [`Preparable::append`] extends the rows (optionally replacing the
/// columns and/or sort) and recomputes the permutation. [`Preparable::layout`]
/// windows the sorted rows by `scroll`/`height`.
///
/// `ctx.width` and `ctx.focus` are accepted only to satisfy the [`LayoutCtx`]
/// contract; they do not affect the table layout.
#[derive(Debug, Clone, Default)]
pub struct PreparedTable;

impl Preparable for PreparedTable {
    type Prepared = PreparedTableState;
    type Layout = TableLayout;
    type Input = TableInput;

    fn prepare(input: Self::Input) -> Self::Prepared {
        let permutation = compute_permutation(&input.rows, input.sort);
        PreparedTableState {
            rows: input.rows,
            columns: input.columns,
            sort: input.sort,
            permutation,
        }
    }

    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        prepared.rows.extend(more.rows);
        if !more.columns.is_empty() {
            prepared.columns = more.columns;
        }
        // A sort present in `more` updates the active sort; otherwise keep the
        // existing one, then recompute the permutation over all rows.
        prepared.sort = more.sort.or(prepared.sort);
        prepared.permutation = compute_permutation(&prepared.rows, prepared.sort);
    }

    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
        let total = prepared.permutation.len();
        let start = ctx.scroll;
        let end = start.saturating_add(usize::from(ctx.height));
        let lo = start.min(total);
        let hi = end.min(total);

        let mut rows = Vec::with_capacity(hi.saturating_sub(lo));
        for sorted in lo..hi {
            let original = prepared.permutation[sorted];
            let cells = row_cells(&prepared.rows[original], &prepared.columns);
            rows.push(VisibleRow {
                index: original,
                cells,
            });
        }

        let column_widths = prepared.columns.iter().map(|c| c.width).collect();

        TableLayout {
            rows,
            column_widths,
            total,
        }
    }
}

/// Compute the sort permutation of `rows` under an optional [`SortSpec`].
///
/// Without a sort the permutation is the identity `0..n`. With a sort, rows are
/// compared by the cell at `spec.column` using lexicographic `String` ordering
/// (a missing cell is the empty string), ascending or descending. The sort is
/// **stable**, so equal rows keep their insertion order.
fn compute_permutation(rows: &[Vec<String>], sort: Option<SortSpec>) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..rows.len()).collect();
    if let Some(spec) = sort {
        perm.sort_by(|&a, &b| {
            let ac = rows[a].get(spec.column).map_or("", String::as_str);
            let bc = rows[b].get(spec.column).map_or("", String::as_str);
            if spec.ascending {
                ac.cmp(bc)
            } else {
                bc.cmp(ac)
            }
        });
    }
    perm
}

/// Build the visible cells for one row: one entry per column, truncated to the
/// column's width by character count. Missing cells become the empty string.
fn row_cells(row: &[String], columns: &[TableColumn]) -> Vec<String> {
    columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            row.get(i)
                .map(|cell| truncate_cell(cell, col.width))
                .unwrap_or_default()
        })
        .collect()
}

/// Truncate a cell to at most `width` characters.
fn truncate_cell(cell: &str, width: u16) -> String {
    cell.chars().take(usize::from(width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Two-column sample used across tests: name + age.
    fn sample_columns() -> Vec<TableColumn> {
        vec![TableColumn::new("name", 10), TableColumn::new("age", 5)]
    }

    /// Three unsorted rows.
    fn sample_rows() -> Vec<Vec<String>> {
        vec![
            vec!["charlie".to_string(), "3".to_string()],
            vec!["alice".to_string(), "1".to_string()],
            vec!["bob".to_string(), "2".to_string()],
        ]
    }

    /// Assert `perm` is a valid permutation of `0..n` (each index exactly once).
    fn assert_valid_permutation(perm: &[usize], n: usize) {
        assert_eq!(perm.len(), n, "permutation length != {n}");
        let mut sorted = perm.to_vec();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            (0..n).collect::<Vec<_>>(),
            "not a permutation of 0..{n}"
        );
    }

    // ---------- prepare ----------

    #[test]
    fn prepare_no_sort_is_identity_permutation() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: None,
        });
        assert_eq!(prepared.permutation, vec![0, 1, 2]);
        assert_eq!(prepared.sort, None);
    }

    #[test]
    fn prepare_ascending_sort_orders_column() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        // charlie, alice, bob -> alice(1), bob(2), charlie(0)
        assert_eq!(prepared.permutation, vec![1, 2, 0]);
    }

    #[test]
    fn prepare_descending_sort_reverses_order() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: false,
            }),
        });
        // descending on names -> charlie(0), bob(2), alice(1)
        assert_eq!(prepared.permutation, vec![0, 2, 1]);
    }

    #[test]
    fn prepare_toggle_direction_flips_order() {
        // Same column, opposite directions => reversed permutations.
        let asc = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 1,
                ascending: true,
            }),
        });
        let desc = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 1,
                ascending: false,
            }),
        });
        // ages 3,1,2 -> asc: 1(1),2(2),3(0); desc: 3(0),2(2),1(1)
        assert_eq!(asc.permutation, vec![1, 2, 0]);
        assert_eq!(desc.permutation, vec![0, 2, 1]);
    }

    #[test]
    fn prepare_sort_with_single_row_is_identity() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: vec![vec!["solo".to_string()]],
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        assert_eq!(prepared.permutation, vec![0]);
    }

    #[test]
    fn prepare_empty_rows_is_empty_permutation() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: Vec::new(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        assert!(prepared.permutation.is_empty());
        assert!(prepared.is_empty());
        assert_eq!(prepared.row_count(), 0);
    }

    #[test]
    fn prepare_sort_missing_cell_treated_as_empty() {
        // Ragged rows: the second row has no cell at the sort column.
        let prepared = PreparedTable::prepare(TableInput {
            rows: vec![
                vec!["b".to_string()],
                vec![], // missing cell -> "" sorts before "b"
                vec!["a".to_string()],
            ],
            columns: vec![TableColumn::new("x", 4)],
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        // "" (idx1) < "a" (idx2) < "b" (idx0)
        assert_eq!(prepared.permutation, vec![1, 2, 0]);
    }

    #[test]
    fn prepare_sort_out_of_range_column_does_not_panic() {
        // Sorting by column 9 when rows have 2 cells: every cell is missing ->
        // all compare equal -> stable identity permutation.
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 9,
                ascending: true,
            }),
        });
        assert_eq!(prepared.permutation, vec![0, 1, 2]);
    }

    #[test]
    fn prepare_stable_sort_preserves_insertion_order_for_ties() {
        // Two rows share the name "alice"; the stable sort keeps insertion order.
        let prepared = PreparedTable::prepare(TableInput {
            rows: vec![
                vec!["alice".to_string(), "2".to_string()],
                vec!["bob".to_string(), "9".to_string()],
                vec!["alice".to_string(), "1".to_string()],
            ],
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        // "alice"(0), "alice"(2), "bob"(1) — tie keeps 0 before 2.
        assert_eq!(prepared.permutation, vec![0, 2, 1]);
    }

    // ---------- layout ----------

    #[test]
    fn layout_returns_all_rows_in_insertion_order_unsorted() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 10));
        assert_eq!(layout.total, 3);
        assert_eq!(layout.rows.len(), 3);
        assert_eq!(layout.rows[0].index, 0);
        assert_eq!(layout.rows[0].cells[0], "charlie");
    }

    #[test]
    fn layout_total_equals_row_count() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 1));
        assert_eq!(layout.total, 3);
        assert_eq!(layout.rows.len(), 1);
    }

    #[test]
    fn layout_windowing_with_scroll() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        // Sorted names: alice(1), bob(2), charlie(0). Show only the 2nd.
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 1).with_scroll(1));
        assert_eq!(layout.total, 3);
        assert_eq!(layout.rows.len(), 1);
        assert_eq!(layout.rows[0].index, 2);
        assert_eq!(layout.rows[0].cells[0], "bob");
    }

    #[test]
    fn layout_visible_never_exceeds_height() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 2));
        assert_eq!(layout.total, 3);
        assert!(layout.rows.len() <= 2);
    }

    #[test]
    fn layout_scroll_beyond_total_is_empty_no_panic() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 10).with_scroll(999));
        assert_eq!(layout.total, 3);
        assert!(layout.rows.is_empty());
    }

    #[test]
    fn layout_height_zero_is_empty() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 0));
        assert_eq!(layout.total, 3);
        assert!(layout.rows.is_empty());
    }

    #[test]
    fn layout_empty_rows_is_empty() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: Vec::new(),
            columns: sample_columns(),
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 10));
        assert_eq!(layout.total, 0);
        assert!(layout.rows.is_empty());
    }

    #[test]
    fn layout_truncates_cells_to_column_width() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: vec![vec!["abcdefghij".to_string()]], // 10 chars
            columns: vec![TableColumn::new("x", 4)],    // width 4
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(layout.rows[0].cells[0], "abcd");
    }

    #[test]
    fn layout_column_widths_match_columns() {
        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: vec![TableColumn::new("a", 7), TableColumn::new("b", 3)],
            sort: None,
        });
        let layout = PreparedTable::layout(&prepared, LayoutCtx::new(80, 10));
        assert_eq!(layout.column_widths, vec![7, 3]);
    }

    // ---------- append ----------

    #[test]
    fn append_extends_rows_and_recomputes_permutation() {
        let mut prepared = PreparedTable::prepare(TableInput {
            rows: vec![vec!["b".to_string()]],
            columns: vec![TableColumn::new("x", 5)],
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        PreparedTable::append(
            &mut prepared,
            TableInput {
                rows: vec![vec!["a".to_string()], vec!["c".to_string()]],
                columns: Vec::new(), // keep existing columns
                sort: None,          // keep existing sort
            },
        );
        assert_eq!(prepared.row_count(), 3);
        // rows: b(0), a(1), c(2); ascending -> a(1), b(0), c(2)
        assert_eq!(prepared.permutation, vec![1, 0, 2]);
    }

    #[test]
    fn append_with_new_sort_re_sorts() {
        let mut prepared = PreparedTable::prepare(TableInput {
            rows: vec![vec!["b".to_string()]],
            columns: vec![TableColumn::new("x", 5)],
            sort: None,
        });
        PreparedTable::append(
            &mut prepared,
            TableInput {
                rows: vec![vec!["a".to_string()], vec!["c".to_string()]],
                columns: Vec::new(),
                sort: Some(SortSpec {
                    column: 0,
                    ascending: false,
                }),
            },
        );
        // rows: b(0), a(1), c(2); descending -> c(2), b(0), a(1)
        assert_eq!(prepared.permutation, vec![2, 0, 1]);
        assert_eq!(
            prepared.sort,
            Some(SortSpec {
                column: 0,
                ascending: false,
            })
        );
    }

    #[test]
    fn append_replaces_columns_when_non_empty() {
        let mut prepared = PreparedTable::prepare(TableInput {
            rows: vec![vec!["x".to_string()]],
            columns: vec![TableColumn::new("old", 1)],
            sort: None,
        });
        PreparedTable::append(
            &mut prepared,
            TableInput {
                rows: Vec::new(),
                columns: vec![TableColumn::new("new", 9)],
                sort: None,
            },
        );
        assert_eq!(prepared.columns.len(), 1);
        assert_eq!(prepared.columns[0].title, "new");
        assert_eq!(prepared.columns[0].width, 9);
    }

    // ---------- helpers ----------

    #[test]
    fn helpers_row_count_and_is_empty() {
        let empty = PreparedTable::prepare(TableInput {
            rows: Vec::new(),
            columns: sample_columns(),
            sort: None,
        });
        assert!(empty.is_empty());
        assert_eq!(empty.row_count(), 0);

        let prepared = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: None,
        });
        assert!(!prepared.is_empty());
        assert_eq!(prepared.row_count(), 3);
    }

    #[test]
    fn tablecolumn_new_builds_title_and_width() {
        let col = TableColumn::new("age", 5);
        assert_eq!(col.title, "age");
        assert_eq!(col.width, 5);
    }

    // ---------- Clone / PartialEq derive checks ----------

    #[test]
    fn state_and_layout_are_clone_and_eq() {
        let state = PreparedTable::prepare(TableInput {
            rows: sample_rows(),
            columns: sample_columns(),
            sort: Some(SortSpec {
                column: 0,
                ascending: true,
            }),
        });
        assert_eq!(state.clone(), state);

        let layout = PreparedTable::layout(&state, LayoutCtx::new(80, 2));
        assert_eq!(layout.clone(), layout);
    }

    #[test]
    fn visible_row_is_clone_and_eq() {
        let a = VisibleRow {
            index: 1,
            cells: vec!["x".to_string()],
        };
        assert_eq!(a.clone(), a);
    }

    #[test]
    fn sortspec_is_copy_and_eq() {
        let s = SortSpec {
            column: 2,
            ascending: true,
        };
        let copied = s; // Copy
        assert_eq!(s, copied);
    }

    // ---------- invariant loops (manual, no proptest macros) ----------

    #[test]
    fn invariant_many_sorts_permutation_valid_and_windowing_bounded() {
        let rows = sample_rows();
        let n = rows.len();
        for col in 0..2usize {
            for ascending in [true, false] {
                let prepared = PreparedTable::prepare(TableInput {
                    rows: rows.clone(),
                    columns: sample_columns(),
                    sort: Some(SortSpec {
                        column: col,
                        ascending,
                    }),
                });
                assert_valid_permutation(&prepared.permutation, n);

                for height in [0u16, 1, 2, 10] {
                    for scroll in [0usize, 1, 5, 100] {
                        let layout = PreparedTable::layout(
                            &prepared,
                            LayoutCtx::new(80, height).with_scroll(scroll),
                        );
                        assert_eq!(layout.total, n, "total mismatch col={col}");
                        assert!(
                            layout.rows.len() <= usize::from(height),
                            "visible > height: col={col} h={height} s={scroll}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn invariant_never_panics_on_edge_inputs() {
        let cases: Vec<TableInput> = vec![
            TableInput::default(),
            TableInput {
                rows: vec![Vec::new()],
                columns: Vec::new(),
                sort: None,
            },
            TableInput {
                rows: vec![vec!["a".to_string()]],
                columns: Vec::new(),
                sort: Some(SortSpec {
                    column: 0,
                    ascending: true,
                }),
            },
            TableInput {
                rows: sample_rows(),
                columns: Vec::new(),
                sort: Some(SortSpec {
                    column: 5,
                    ascending: false,
                }),
            },
        ];
        for input in cases {
            let prepared = PreparedTable::prepare(input);
            for height in [0u16, 1, 5] {
                for scroll in [0usize, 3, 50] {
                    let layout = PreparedTable::layout(
                        &prepared,
                        LayoutCtx::new(0, height).with_scroll(scroll),
                    );
                    assert_eq!(layout.total, prepared.row_count());
                    assert!(layout.rows.len() <= usize::from(height));
                }
            }
        }
    }
}
