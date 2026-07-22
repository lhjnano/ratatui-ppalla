//! # `PreparedList` — filterable list with prepare/layout separation.
//!
//! Concrete implementation of the [Pretext](https://github.com/0xradical/Pretext)
//! prepare/layout separation for filterable lists. The cold path
//! ([`PreparedList::prepare`] / [`PreparedList::append`]) owns the full item set
//! (`Vec<String>`) and caches a case-insensitive substring filter index
//! (`filtered_indices`). The hot path ([`PreparedList::layout`]) windows into
//! the filtered set by `ctx.scroll .. ctx.scroll + ctx.height`, cloning an
//! item's `String` only when its filtered position lands inside the visible
//! window.
//!
//! This is the prepare/layout port of the `list` widget's filter/scroll logic,
//! flattened to owned `String`s (the owned semantics
//! [`Preparable`](super::Preparable) requires).
//!
//! # Filter semantics
//!
//! Filtering is case-insensitive substring matching (via
//! [`str::to_lowercase`]). An empty filter matches every item, yielding every
//! index `0..items.len()`. A non-matching filter yields an empty index vector
//! while the full item set is retained.
//!
//! # Windowed cloning
//!
//! Like [`PreparedText`](super::prepared_text::PreparedText), the hot path walks
//! the full `filtered_indices` to report [`ListLayout::total`] (needed for
//! scroll clamping) but defers cloning each item's `String` to the moment its
//! global filtered position falls inside `[ctx.scroll, ctx.scroll + ctx.height)`.
//! Items outside the window are counted but never copied.

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};

/// Input for preparing a [`PreparedList`]: the items and an optional filter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListInput {
    /// The full item set (each item's text is used for both display and filtering).
    pub items: Vec<String>,
    /// Current filter string (empty = no filter; case-insensitive substring match).
    pub filter: String,
}

/// Prepared (cold-path) state: the items, current filter, and cached filter
/// index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreparedListState {
    /// All items in insertion order.
    pub items: Vec<String>,
    /// The filter string (empty matches everything).
    pub filter: String,
    /// Indices into [`items`](Self::items) that pass the filter
    /// (case-insensitive substring).
    pub filtered_indices: Vec<usize>,
}

impl PreparedListState {
    /// Number of items currently passing the filter.
    #[must_use]
    pub fn filtered_len(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Return `true` if there are no items at all (ignoring the filter).
    ///
    /// This checks the total item set, not the filtered set: a non-empty list
    /// whose items are all filtered out still reports `false`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// One visible item in a layout result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleItem {
    /// Index into the original `items` vector.
    pub index: usize,
    /// The item's text.
    pub text: String,
    /// Position within the filtered set (0-based).
    pub filtered_position: usize,
}

/// Per-frame layout result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListLayout {
    /// Visible items, windowed by `ctx.scroll .. ctx.scroll + ctx.height`.
    pub items: Vec<VisibleItem>,
    /// Total filtered count (for scroll clamping).
    pub total: usize,
}

/// Prepared list primitive using the prepare/layout separation.
///
/// Implements [`Preparable`]. The input is a [`ListInput`] (items plus a filter
/// string). [`Preparable::prepare`] caches the full item set and a
/// case-insensitive substring filter index. [`Preparable::layout`] windows the
/// filtered set by scroll/height, cloning only the visible items.
///
/// [`Preparable::append`] extends the item set, optionally adopts a non-empty
/// incoming filter, and recomputes the filter index.
///
/// `ctx.focus` is ignored by list layout (selection/highlighting is the
/// renderer's responsibility); it is accepted only to satisfy the [`LayoutCtx`]
/// contract.
///
/// # Examples
///
/// ```
/// use ratatui_presto::prepared::{LayoutCtx, Preparable, PreparedList, ListInput};
///
/// let input = ListInput {
///     items: vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()],
///     filter: String::new(),
/// };
/// let prepared = PreparedList::prepare(input);
/// let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 2));
/// assert_eq!(layout.total, 3);
/// assert_eq!(layout.items.len(), 2);
/// assert_eq!(layout.items[0].text, "apple");
/// ```
#[derive(Debug, Clone, Default)]
pub struct PreparedList;

impl Preparable for PreparedList {
    type Prepared = PreparedListState;
    type Layout = ListLayout;
    type Input = ListInput;

    fn prepare(input: Self::Input) -> Self::Prepared {
        let ListInput { items, filter } = input;
        let filtered_indices = compute_filtered_indices(&items, &filter);
        PreparedListState {
            items,
            filter,
            filtered_indices,
        }
    }

    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        // Extend the item set first so the recompute sees the new rows.
        prepared.items.extend(more.items);
        // A non-empty incoming filter replaces the current one; an empty
        // incoming filter preserves the existing filter (matching the
        // "empty filter = match everything" prepare semantics).
        if !more.filter.is_empty() {
            prepared.filter = more.filter;
        }
        prepared.filtered_indices = compute_filtered_indices(&prepared.items, &prepared.filter);
    }

    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
        // WHY windowing the clone: the hot path must walk ALL filtered indices
        // to report `total` (needed for scroll clamping), but cloning each
        // item's `String` is the dominant cost. Only items whose filtered
        // position lands inside `[start, end)` are cloned; the rest are counted
        // but skipped, so counting and collecting share one code path.
        let start = ctx.scroll;
        let end = start.saturating_add(usize::from(ctx.height));
        let total = prepared.filtered_indices.len();

        let mut items: Vec<VisibleItem> = Vec::new();
        for (filtered_position, &index) in prepared.filtered_indices.iter().enumerate() {
            if filtered_position >= start && filtered_position < end {
                // The index is valid by construction: `filtered_indices` are
                // produced by our own recompute over `items`, so direct
                // indexing cannot panic for any state we build.
                items.push(VisibleItem {
                    index,
                    text: prepared.items[index].clone(),
                    filtered_position,
                });
            }
        }

        ListLayout { items, total }
    }
}

/// Compute the indices of `items` that pass a case-insensitive substring
/// `filter`. An empty filter matches every item (yields `0..items.len()`).
///
/// This is the prepare/layout port of `list`'s `recompute_filtered`, flattened
/// from the `ListItem` trait to plain `String`s.
fn compute_filtered_indices(items: &[String], filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..items.len()).collect();
    }
    let needle = filter.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.to_lowercase().contains(needle.as_str()))
        .map(|(idx, _)| idx)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ---------- prepare ----------

    #[test]
    fn prepare_empty_filter_shows_all_items() {
        let input = ListInput {
            items: vec![
                "apple".to_string(),
                "banana".to_string(),
                "cherry".to_string(),
            ],
            filter: String::new(),
        };
        let state = PreparedList::prepare(input);
        assert_eq!(state.items.len(), 3);
        assert_eq!(state.filter, "");
        assert_eq!(state.filtered_indices, vec![0, 1, 2]);
        assert_eq!(state.filtered_len(), 3);
        assert!(!state.is_empty());
    }

    #[test]
    fn prepare_filter_narrows_to_matching_items() {
        let input = ListInput {
            items: vec![
                "apple".to_string(),
                "banana".to_string(),
                "cherry".to_string(),
            ],
            filter: "an".to_string(),
        };
        let state = PreparedList::prepare(input);
        assert_eq!(state.filtered_indices, vec![1]);
        assert_eq!(state.filtered_len(), 1);
    }

    #[test]
    fn prepare_filter_is_case_insensitive() {
        let upper = PreparedList::prepare(ListInput {
            items: vec!["Banana".to_string()],
            filter: "BAN".to_string(),
        });
        assert_eq!(upper.filtered_indices, vec![0]);

        let lower = PreparedList::prepare(ListInput {
            items: vec!["Banana".to_string()],
            filter: "banana".to_string(),
        });
        assert_eq!(lower.filtered_indices, vec![0]);
    }

    #[test]
    fn prepare_no_match_yields_empty_filtered() {
        let input = ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: "zzz".to_string(),
        };
        let state = PreparedList::prepare(input);
        assert!(state.filtered_indices.is_empty());
        assert_eq!(state.filtered_len(), 0);
        // Total items are still present; only the index is empty.
        assert_eq!(state.items.len(), 2);
        assert!(!state.is_empty());
    }

    #[test]
    fn prepare_unicode_filter_matches() {
        let input = ListInput {
            items: vec!["안녕하세요".to_string(), "hello".to_string()],
            filter: "안녕".to_string(),
        };
        let state = PreparedList::prepare(input);
        assert_eq!(state.filtered_indices, vec![0]);
    }

    #[test]
    fn prepare_empty_items_is_empty() {
        let state = PreparedList::prepare(ListInput::default());
        assert!(state.is_empty());
        assert!(state.filtered_indices.is_empty());
        assert_eq!(state.filtered_len(), 0);
    }

    #[test]
    fn prepare_preserves_items_and_filter() {
        let items = vec!["apple".to_string(), "banana".to_string()];
        let input = ListInput {
            items: items.clone(),
            filter: "ap".to_string(),
        };
        let state = PreparedList::prepare(input);
        assert_eq!(state.items, items);
        assert_eq!(state.filter, "ap");
    }

    // ---------- layout ----------

    #[test]
    fn layout_window_clips_to_height() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            filter: String::new(),
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 2));
        assert_eq!(layout.total, 5);
        assert_eq!(layout.items.len(), 2);
        assert_eq!(layout.items[0].text, "a");
        assert_eq!(layout.items[0].index, 0);
        assert_eq!(layout.items[0].filtered_position, 0);
        assert_eq!(layout.items[1].text, "b");
        assert_eq!(layout.items[1].filtered_position, 1);
    }

    #[test]
    fn layout_window_with_scroll() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            filter: String::new(),
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 2).with_scroll(1));
        assert_eq!(layout.total, 5);
        assert_eq!(layout.items.len(), 2);
        // Window [1, 3) -> filtered positions 1 and 2.
        assert_eq!(layout.items[0].text, "b");
        assert_eq!(layout.items[0].filtered_position, 1);
        assert_eq!(layout.items[1].text, "c");
        assert_eq!(layout.items[1].filtered_position, 2);
    }

    #[test]
    fn layout_scroll_near_end_clips() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            filter: String::new(),
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 2).with_scroll(4));
        assert_eq!(layout.total, 5);
        assert_eq!(layout.items.len(), 1);
        assert_eq!(layout.items[0].text, "e");
        assert_eq!(layout.items[0].filtered_position, 4);
    }

    #[test]
    fn layout_scroll_beyond_total_is_empty_no_panic() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec!["a".to_string(), "b".to_string()],
            filter: String::new(),
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 10).with_scroll(999));
        assert_eq!(layout.total, 2);
        assert!(layout.items.is_empty());
    }

    #[test]
    fn layout_height_zero_collects_nothing() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec!["a".to_string(), "b".to_string()],
            filter: String::new(),
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 0));
        assert_eq!(layout.total, 2);
        assert!(layout.items.is_empty());
    }

    #[test]
    fn layout_total_equals_filtered_count() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec![
                "apple".to_string(),
                "banana".to_string(),
                "cherry".to_string(),
            ],
            filter: "a".to_string(),
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 10));
        // "a" matches apple + banana.
        assert_eq!(prepared.filtered_len(), 2);
        assert_eq!(layout.total, 2);
        assert_eq!(layout.items.len(), 2);
    }

    #[test]
    fn layout_index_maps_back_to_original_items() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec![
                "apple".to_string(),
                "banana".to_string(),
                "cherry".to_string(),
            ],
            filter: "rr".to_string(), // only cherry
        });
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(layout.items.len(), 1);
        assert_eq!(layout.items[0].index, 2);
        assert_eq!(layout.items[0].text, "cherry");
        assert_eq!(layout.items[0].filtered_position, 0);
    }

    #[test]
    fn layout_focus_is_ignored() {
        let prepared = PreparedList::prepare(ListInput {
            items: vec!["a".to_string(), "b".to_string()],
            filter: String::new(),
        });
        let without = PreparedList::layout(&prepared, LayoutCtx::new(80, 5));
        let with_focus = PreparedList::layout(&prepared, LayoutCtx::new(80, 5).with_focus(0));
        assert_eq!(without, with_focus);
    }

    #[test]
    fn layout_empty_input_is_empty() {
        let prepared = PreparedList::prepare(ListInput::default());
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 24));
        assert!(layout.items.is_empty());
        assert_eq!(layout.total, 0);
    }

    // ---------- append ----------

    #[test]
    fn append_extends_items_and_recomputes_filter() {
        let mut prepared = PreparedList::prepare(ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: String::new(),
        });
        assert_eq!(prepared.filtered_len(), 2);

        PreparedList::append(
            &mut prepared,
            ListInput {
                items: vec!["apricot".to_string(), "cherry".to_string()],
                filter: String::new(),
            },
        );
        assert_eq!(prepared.items.len(), 4);
        // No filter change -> all items visible.
        assert_eq!(prepared.filtered_len(), 4);
        assert_eq!(prepared.filtered_indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn append_adopts_non_empty_filter() {
        let mut prepared = PreparedList::prepare(ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: String::new(),
        });
        PreparedList::append(
            &mut prepared,
            ListInput {
                items: vec!["apricot".to_string(), "cherry".to_string()],
                filter: "ap".to_string(),
            },
        );
        assert_eq!(prepared.filter, "ap");
        // apple + apricot match "ap" (indices 0 and 2).
        assert_eq!(prepared.filtered_indices, vec![0, 2]);
        assert_eq!(prepared.filtered_len(), 2);
    }

    #[test]
    fn append_preserves_existing_filter_when_incoming_empty() {
        let mut prepared = PreparedList::prepare(ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: "an".to_string(),
        });
        assert_eq!(prepared.filtered_indices, vec![1]);
        PreparedList::append(
            &mut prepared,
            ListInput {
                items: vec!["cherry".to_string()],
                filter: String::new(),
            },
        );
        // Filter unchanged ("an"); new item "cherry" does not match.
        assert_eq!(prepared.filter, "an");
        assert_eq!(prepared.filtered_indices, vec![1]);
    }

    #[test]
    fn append_empty_input_is_unchanged() {
        let mut prepared = PreparedList::prepare(ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: "an".to_string(),
        });
        let before = prepared.clone();
        PreparedList::append(&mut prepared, ListInput::default());
        assert_eq!(prepared, before);
    }

    #[test]
    fn preparable_workflow_prepare_layout_append_relayout() {
        let mut prepared = PreparedList::prepare(ListInput {
            items: vec!["a".to_string(), "b".to_string()],
            filter: String::new(),
        });
        let first = PreparedList::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(first.total, 2);
        assert_eq!(first.items.len(), 2);

        PreparedList::append(
            &mut prepared,
            ListInput {
                items: vec!["c".to_string(), "d".to_string()],
                filter: String::new(),
            },
        );
        let second = PreparedList::layout(&prepared, LayoutCtx::new(80, 5));
        assert_eq!(second.total, 4);
        assert_eq!(second.items.len(), 4);
    }

    // ---------- convenience / helpers ----------

    #[test]
    fn helpers_filtered_len_and_is_empty() {
        let empty = PreparedList::prepare(ListInput::default());
        assert!(empty.is_empty());
        assert_eq!(empty.filtered_len(), 0);

        let none_match = PreparedList::prepare(ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: "zzz".to_string(),
        });
        // is_empty checks total items, not filtered.
        assert!(!none_match.is_empty());
        assert_eq!(none_match.filtered_len(), 0);
    }

    // ---------- Clone / PartialEq derives ----------

    #[test]
    fn clone_equality_for_state_and_layout() {
        let state = PreparedList::prepare(ListInput {
            items: vec!["apple".to_string(), "banana".to_string()],
            filter: "ap".to_string(),
        });
        assert_eq!(state.clone(), state);

        let layout = PreparedList::layout(&state, LayoutCtx::new(80, 1));
        assert_eq!(layout.clone(), layout);
    }

    #[test]
    fn listinput_and_visibleitem_clone_eq() {
        let input = ListInput {
            items: vec!["a".to_string()],
            filter: String::new(),
        };
        assert_eq!(input.clone(), input);

        let prepared = PreparedList::prepare(input);
        let layout = PreparedList::layout(&prepared, LayoutCtx::new(80, 1));
        let vi = layout.items[0].clone();
        assert_eq!(vi.clone(), vi);
    }

    // ---------- invariant loops (manual, no proptest macros) ----------

    #[test]
    fn invariant_filtered_len_never_exceeds_total_items() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
            "date".to_string(),
            "elderberry".to_string(),
        ];
        for filter in &[
            "",
            "a",
            "e",
            "xyz",
            "app",
            "안녕",
            "!@#$%",
            "elderberry",
            "BERRY",
        ] {
            let state = PreparedList::prepare(ListInput {
                items: items.clone(),
                filter: (*filter).to_string(),
            });
            assert!(
                state.filtered_len() <= state.items.len(),
                "filter '{filter}': filtered_len {} > items.len() {}",
                state.filtered_len(),
                state.items.len()
            );
        }
    }

    #[test]
    fn invariant_layout_never_panics_and_visible_le_height() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
            "date".to_string(),
            "elderberry".to_string(),
        ];
        for filter in &["", "a", "e", "xyz", "안녕", "elderberry"] {
            let prepared = PreparedList::prepare(ListInput {
                items: items.clone(),
                filter: (*filter).to_string(),
            });
            for height in [0u16, 1, 3, 100] {
                for scroll in [0usize, 1, 3, 100] {
                    let layout = PreparedList::layout(
                        &prepared,
                        LayoutCtx::new(80, height).with_scroll(scroll),
                    );
                    assert!(
                        layout.items.len() <= usize::from(height),
                        "filter='{filter}' height={height} scroll={scroll}: \
                         visible {} > height",
                        layout.items.len()
                    );
                    assert_eq!(layout.total, prepared.filtered_len());
                    assert!(layout.total <= prepared.items.len());
                }
            }
        }
    }

    #[test]
    fn invariant_stress_many_filters_never_panic() {
        let items = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
            "date".to_string(),
            "elderberry".to_string(),
        ];
        let filters = [
            "a",
            "b",
            "ab",
            "xyz",
            "",
            " ",
            "  ",
            "AAP",
            "E",
            "e",
            "0",
            "z",
            "Z",
            "@",
            "#",
            "apple",
            "banana",
            "cherry",
            "elder",
            "berry",
            "안녕",
            "하세요",
            "こんにちは",
            "你好",
            "🌟",
            "\t",
            "\n",
            "aaaaaaaaaa",
        ];
        for f in &filters {
            let state = PreparedList::prepare(ListInput {
                items: items.clone(),
                filter: (*f).to_string(),
            });
            assert!(state.filtered_len() <= state.items.len());
            // Full pipeline (prepare -> layout) must not panic.
            let _layout = PreparedList::layout(&state, LayoutCtx::new(80, 2));
        }
    }
}
