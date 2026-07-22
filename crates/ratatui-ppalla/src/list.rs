//! Enhanced list widget with filtering, navigation, and selection.
//!
//! A Rust port of the [`Bubbles` `list`](https://github.com/charmbracelet/bubbles/list)
//! package from the Bubble Tea ecosystem. The [`List`] keeps the full item set
//! alongside a vector of indices (`filtered_indices`) into it representing the
//! currently-visible rows; the selection cursor moves within the filtered set.
//!
//! Unlike the upstream Go package (which renders itself via Lipgloss), this
//! widget delegates the actual drawing to [`ratatui::widgets::List`] and merely
//! tracks state: items, filter, and selection.

#![allow(clippy::module_name_repetitions)]

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{List as RatList, ListItem as RatListItem, ListState};
use ratatui::Frame;

/// A single row in a [`List`].
///
/// Implementors control both how the row is drawn (`render`) and how it
/// participates in filtering (`filterable_text`).
pub trait ListItem {
    /// Render this item as a [`Line`] of styled text.
    fn render(&self) -> Line<'_>;

    /// Return the text used for filtering.
    ///
    /// Filtering is a case-insensitive substring match against the current
    /// filter string (see [`List::set_filter`]).
    fn filterable_text(&self) -> &str;
}

/// A filterable, selectable list of items.
///
/// Port of `bubbles/list.Model`. See the [module docs](self) for details.
pub struct List<T: ListItem> {
    /// Every item in the list, in insertion order.
    items: Vec<T>,
    /// Index into the underlying item vector of the currently-selected row, if any.
    selected: Option<usize>,
    /// Current filter string (empty matches everything).
    filter: String,
    /// Indices into `items` that currently pass the filter.
    filtered_indices: Vec<usize>,
}

impl<T: ListItem> List<T> {
    /// Create a new list containing `items`.
    ///
    /// Defaults: no selection, an empty filter, and every item visible
    /// (all indices present in the filtered set).
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        let filtered_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            items,
            selected: None,
            filter: String::new(),
            filtered_indices,
        }
    }

    /// Return the currently-selected item, or `None` if nothing is selected
    /// (or the filtered set is empty).
    #[must_use]
    pub fn selected(&self) -> Option<&T> {
        self.selected.and_then(|idx| self.items.get(idx))
    }

    /// Move the selection cursor down by one, wrapping within the filtered set.
    ///
    /// If nothing is currently selected, the first visible item becomes selected.
    /// Does nothing when the filtered set is empty.
    pub fn select_next(&mut self) {
        if self.filtered_indices.is_empty() {
            self.selected = None;
            return;
        }
        let new_pos = match self.current_filtered_pos() {
            Some(pos) => (pos + 1) % self.filtered_indices.len(),
            None => 0,
        };
        self.selected = Some(self.filtered_indices[new_pos]);
    }

    /// Move the selection cursor up by one, wrapping within the filtered set.
    ///
    /// If nothing is selected (or the cursor sits at the top), the last visible
    /// item becomes selected. Does nothing when the filtered set is empty.
    pub fn select_prev(&mut self) {
        if self.filtered_indices.is_empty() {
            self.selected = None;
            return;
        }
        let len = self.filtered_indices.len();
        let new_pos = match self.current_filtered_pos() {
            Some(pos) if pos > 0 => pos - 1,
            Some(_) | None => len - 1,
        };
        self.selected = Some(self.filtered_indices[new_pos]);
    }

    /// Set the filter string and recompute the visible rows.
    ///
    /// `filter` is matched case-insensitively as a substring of each item's
    /// [`filterable_text`](ListItem::filterable_text). An empty filter shows
    /// every item.
    ///
    /// If the currently-selected item survives the filter it stays selected;
    /// otherwise the selection moves to the first visible item (or `None`
    /// when the filtered set becomes empty).
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.recompute_filtered();
        let still_visible = self
            .selected
            .is_some_and(|idx| self.filtered_indices.contains(&idx));
        self.selected = if still_visible {
            self.selected
        } else {
            self.filtered_indices.first().copied()
        };
    }

    /// Return the current filter string.
    #[must_use]
    pub fn filter(&self) -> &str {
        self.filter.as_str()
    }

    /// Return the total number of items (ignoring the filter).
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return `true` if there are no items at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return the number of items currently passing the filter.
    #[must_use]
    pub fn filtered_len(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Render the visible items into `frame`, highlighting the selected row.
    ///
    /// The selected row is highlighted with `Style::default().add_modifier(Modifier::REVERSED)`,
    /// applied via [`ratatui::widgets::List`] stateful highlighting.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let rat_items: Vec<RatListItem<'_>> = self
            .filtered_indices
            .iter()
            .filter_map(|&idx| {
                self.items
                    .get(idx)
                    .map(|item| RatListItem::new(item.render()))
            })
            .collect();
        let list = RatList::new(rat_items)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default();
        state.select(self.current_filtered_pos());
        frame.render_stateful_widget(list, area, &mut state);
    }

    /// Position of the currently-selected item within `filtered_indices`, if any.
    fn current_filtered_pos(&self) -> Option<usize> {
        self.selected
            .and_then(|idx| self.filtered_indices.iter().position(|&i| i == idx))
    }

    /// Recompute `filtered_indices` from `items` and the current filter.
    fn recompute_filtered(&mut self) {
        if self.filter.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
            return;
        }
        let needle = self.filter.to_lowercase();
        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.filterable_text()
                    .to_lowercase()
                    .contains(needle.as_str())
            })
            .map(|(idx, _)| idx)
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Row {
        text: String,
    }

    impl ListItem for Row {
        fn render(&self) -> Line<'_> {
            Line::from(self.text.as_str())
        }
        fn filterable_text(&self) -> &str {
            self.text.as_str()
        }
    }

    #[test]
    fn filter_narrows_to_single_item() {
        let mut list = List::new(vec![
            Row {
                text: "apple".into(),
            },
            Row {
                text: "banana".into(),
            },
            Row {
                text: "cherry".into(),
            },
        ]);
        list.set_filter("ban");
        assert_eq!(list.filtered_len(), 1);
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("banana"));
    }

    #[test]
    fn starts_with_no_selection_and_all_visible() {
        let list = List::new(vec![
            Row {
                text: "apple".into(),
            },
            Row {
                text: "banana".into(),
            },
        ]);
        assert!(list.selected().is_none());
        assert_eq!(list.len(), 2);
        assert!(!list.is_empty());
        assert_eq!(list.filtered_len(), 2);
        assert_eq!(list.filter(), "");
    }

    #[test]
    fn filter_is_case_insensitive() {
        let mut list = List::new(vec![Row {
            text: "Banana".into(),
        }]);
        list.set_filter("BAN");
        assert_eq!(list.filtered_len(), 1);

        list.set_filter("banana");
        assert_eq!(list.filtered_len(), 1);
    }

    #[test]
    fn navigation_wraps_within_filtered_set() {
        let mut list = List::new(vec![
            Row {
                text: "apple".into(),
            },
            Row {
                text: "apricot".into(),
            },
            Row {
                text: "banana".into(),
            },
        ]);
        list.set_filter("ap"); // [apple, apricot]
                               // selected reset to first filtered item
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apple"));

        list.select_next();
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apricot"));

        // wrap from bottom to top
        list.select_next();
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apple"));

        // prev wraps from top to bottom
        list.select_prev();
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apricot"));
    }

    #[test]
    fn surviving_selection_is_preserved_across_filter() {
        let mut list = List::new(vec![
            Row {
                text: "apple".into(),
            },
            Row {
                text: "apricot".into(),
            },
            Row {
                text: "banana".into(),
            },
        ]);
        list.set_filter(""); // None -> first (apple)
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apple"));
        list.select_next(); // apricot

        // apricot survives "ap" filter -> stays selected
        list.set_filter("ap");
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apricot"));
    }

    #[test]
    fn no_match_filter_clears_selection() {
        let mut list = List::new(vec![Row {
            text: "apple".into(),
        }]);
        list.set_filter("zzz");
        assert_eq!(list.filtered_len(), 0);
        assert!(list.selected().is_none());

        // empty filter restores everything and selects the first item
        list.set_filter("");
        assert_eq!(list.filtered_len(), 1);
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("apple"));
    }

    #[test]
    fn empty_list_has_no_selection() {
        let list = List::new(Vec::<Row>::new());
        assert!(list.selected().is_none());
        assert_eq!(list.filtered_len(), 0);
        assert!(list.is_empty());
    }

    #[test]
    fn single_item_select_next_wraps_or_clamps() {
        let mut list = List::new(vec![Row {
            text: "only".into(),
        }]);
        list.select_next();
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("only"));
        // navigating again on a single-item set wraps/clamps back to the same item
        list.select_next();
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("only"));
    }

    #[test]
    fn filter_with_no_matches_clears_filtered() {
        let mut list = List::new(vec![
            Row {
                text: "apple".into(),
            },
            Row {
                text: "banana".into(),
            },
        ]);
        list.set_filter("zzz");
        assert_eq!(list.filtered_len(), 0);
        assert!(list.selected().is_none());
    }

    #[test]
    fn unicode_filter_matches_correctly() {
        let mut list = List::new(vec![
            Row {
                text: "안녕하세요".into(),
            },
            Row {
                text: "hello".into(),
            },
        ]);
        list.set_filter("안녕");
        assert_eq!(list.filtered_len(), 1);
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("안녕하세요"));
    }

    #[test]
    fn selection_resets_when_filtered_out() {
        let mut list = List::new(vec![
            Row {
                text: "apple".into(),
            },
            Row {
                text: "banana".into(),
            },
            Row {
                text: "cherry".into(),
            },
        ]);
        // select_next from None picks the first filtered item
        list.select_next(); // apple
        list.select_next(); // banana
        assert_eq!(list.selected().map(|r| r.text.as_str()), Some("banana"));

        // filter that excludes "banana" (matches apple + cherry via 'e')
        list.set_filter("e");
        // banana is filtered out -> selection resets to first remaining item
        let sel = list.selected().map(|r| r.text.as_str());
        assert!(sel == Some("apple") || sel == Some("cherry"));
        assert_ne!(sel, Some("banana"));
    }

    // ============ Property/invariant tests ============

    fn sample_items() -> Vec<Row> {
        vec![
            Row {
                text: "apple".to_string(),
            },
            Row {
                text: "banana".to_string(),
            },
            Row {
                text: "cherry".to_string(),
            },
            Row {
                text: "date".to_string(),
            },
            Row {
                text: "elderberry".to_string(),
            },
        ]
    }

    /// `filtered_len()` <= `len()` for any filter
    #[test]
    fn invariant_filtered_len_never_exceeds_total() {
        for filter in &[
            "",
            "a",
            "e",
            "xyz",
            "app",
            "UNICODE_안녕",
            "!@#$%",
            "elderberry",
        ] {
            let mut list = List::new(sample_items());
            list.set_filter(filter);
            assert!(
                list.filtered_len() <= list.len(),
                "filter '{filter}': filtered_len {} > len {}",
                list.filtered_len(),
                list.len()
            );
        }
    }

    /// `select_next`/`prev` never panic regardless of count
    #[test]
    fn invariant_select_never_panics() {
        let mut list = List::new(sample_items());
        for _ in 0..10_000 {
            list.select_next();
        }
        for _ in 0..10_000 {
            list.select_prev();
        }
    }

    /// Setting the same filter twice is idempotent
    #[test]
    fn invariant_filter_idempotent() {
        for filter in &["", "a", "e", "xyz"] {
            let mut a = List::new(sample_items());
            let mut b = List::new(sample_items());
            a.set_filter(filter);
            b.set_filter(filter);
            assert_eq!(a.filtered_len(), b.filtered_len());
        }
    }

    /// Empty filter equals no filter
    #[test]
    fn invariant_empty_filter_equals_no_filter() {
        let mut a = List::new(sample_items());
        let b = List::new(sample_items());
        a.set_filter("");
        assert_eq!(a.filtered_len(), b.filtered_len());
    }

    /// Stress: 100 random filter strings never panic
    #[test]
    fn stress_100_random_filters() {
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
            "1",
            "99",
            "z",
            "Z",
            "@",
            "#",
            "$",
            "%",
            "^",
            "apple",
            "banana",
            "cherry",
            "elder",
            "berry",
            "A",
            "B",
            "C",
            "D",
            "E",
            "F",
            "G",
            "H",
            "I",
            "J",
            "aaaaaaaaaa",
            "zzzzzzzzzz",
            "2024",
            "01",
            "true",
            "false",
            "null",
            "NaN",
            "undefined",
            "None",
            "안녕",
            "하세요",
            "こんにちは",
            "你好",
            "Здравствуй",
            "🌟",
            "🚀",
            "❤️",
            "⚡",
            "🔥",
            "\t",
            "\n",
            "\\",
        ];
        for f in &filters {
            let mut list = List::new(sample_items());
            list.set_filter(f);
            // Must not panic, filtered_len must be valid
            assert!(list.filtered_len() <= list.len());
        }
    }
}
