//! # `PreparedLayout` — cached ratatui constraint-evaluation primitive.
//!
//! A concrete implementation of the [Pretext](https://github.com/0xradical/Pretext)
//! prepare/layout separation for layout regions. The cold path
//! ([`PreparedLayout::prepare`] / [`PreparedLayout::append`]) stores a
//! [`SplitSpec`] (a list of [`Constraint`]s plus a [`Direction`]) — cheap to
//! clone. The hot path ([`PreparedLayout::layout`]) evaluates the constraints
//! against the per-frame area derived from [`LayoutCtx`], with a **1-entry cache
//! keyed by (constraints, direction, size)**: on a cache hit the previously
//! computed [`Rect`]s are returned without re-invoking ratatui's
//! [`Layout::split`].
//!
//! This is the "cache hit → return cached rects" row of the ROADMAP Direction-1
//! module table.
//!
//! # Interior mutability
//!
//! Because [`Preparable::layout`](super::Preparable::layout) receives the
//! prepared state by shared reference (`&Self::Prepared`), yet caching requires
//! updating the cached rects, the cache is stored behind a
//! [`std::sync::Mutex`]. The mutex is uncontended in the single-threaded render
//! loop (every frame locks once), so the hit path is a single signature
//! comparison with no ratatui solver call.
//!
//! # Examples
//!
//! ```
//! use ratatui::layout::{Constraint, Direction};
//! use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedLayout, SplitSpec};
//!
//! let spec = SplitSpec::new(vec![Constraint::Length(1), Constraint::Min(0)])
//!     .with_direction(Direction::Vertical);
//! let prepared = PreparedLayout::prepare(spec);
//! let ctx = LayoutCtx::new(10, 5);
//! let first = PreparedLayout::layout(&prepared, ctx);
//! assert_eq!(first.rects.len(), 2);
//! let second = PreparedLayout::layout(&prepared, ctx); // same args => cache hit
//! assert!(second.cache_hit);
//! ```

#![allow(clippy::module_name_repetitions)]

use super::{LayoutCtx, Preparable};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::sync::Mutex;

/// Configuration describing how to split an area: a list of [`Constraint`]s and
/// a split [`Direction`] (vertical stacks rows, horizontal places columns side
/// by side). Cheap to clone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitSpec {
    /// Constraints to evaluate, in order.
    pub constraints: Vec<Constraint>,
    /// Split direction (vertical stacks rows, horizontal places columns side by side).
    pub direction: Direction,
}

impl SplitSpec {
    /// Create a new [`SplitSpec`] with the given constraints and the default
    /// direction ([`Direction::Vertical`]).
    #[must_use]
    pub fn new(constraints: Vec<Constraint>) -> Self {
        Self {
            constraints,
            direction: Direction::Vertical,
        }
    }

    /// Set the split direction and return `self` (builder style).
    #[must_use]
    pub const fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }
}

/// The layout result: the list of [`Rect`]s produced by splitting the area.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SplitLayout {
    /// The rects resulting from splitting, in constraint order.
    pub rects: Vec<Rect>,
    /// `true` if this result came from the 1-entry cache (no ratatui `Layout`
    /// call). Useful for testing and benchmarking the hot path.
    pub cache_hit: bool,
}

/// 1-entry cache of the last layout evaluation.
#[derive(Debug, Clone, Default)]
struct LayoutCache {
    /// Signature of the constraints+direction+size that produced `rects`
    /// (`None` when cold).
    key: Option<CacheKey>,
    /// The cached rects (empty when cold).
    rects: Vec<Rect>,
}

/// Cache key: a hashable signature of the constraints and the target area.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CacheKey {
    /// Deterministic signature of the constraints and direction (see
    /// [`constraints_signature`]).
    sig: u64,
    /// The area the cached rects were computed for.
    area: Rect,
}

/// Prepared state: the split spec plus a 1-entry cache of the last evaluation.
///
/// The [`cache`](Self::cache) field uses a [`Mutex`] for interior mutability
/// because [`Preparable::layout`](super::Preparable::layout) receives the
/// prepared state by shared reference, yet caching is a legitimate hot-path
/// optimization. The mutex is uncontended in the single-threaded render loop.
/// `Clone` is implemented manually because [`Mutex`] is not `Clone` — cloning
/// locks the cache and copies its contents into a fresh mutex.
#[derive(Debug, Default)]
pub struct PreparedLayoutState {
    /// The configured split specification.
    pub spec: SplitSpec,
    /// 1-entry cache of the last layout evaluation.
    cache: Mutex<LayoutCache>,
}

impl Clone for PreparedLayoutState {
    fn clone(&self) -> Self {
        Self {
            spec: self.spec.clone(),
            cache: Mutex::new(
                self.cache
                    .lock()
                    .expect("layout cache mutex should never be poisoned")
                    .clone(),
            ),
        }
    }
}

/// Prepared layout-region primitive using the prepare/layout separation with a
/// 1-entry cache keyed by (constraints, direction, size).
///
/// Implements [`Preparable`]. The input is a [`SplitSpec`]. [`Preparable::prepare`]
/// stores the spec (cache starts cold). [`Preparable::append`] *replaces* the
/// spec (a layout's "more" is a new configuration) and invalidates the cache.
/// [`Preparable::layout`] evaluates the constraints against the area derived
/// from [`LayoutCtx`], caching the result for a same-args follow-up call.
///
/// `ctx.scroll` and `ctx.focus` do not affect the layout region — only
/// `ctx.width` and `ctx.height` determine the area. They are accepted only to
/// satisfy the [`LayoutCtx`] contract.
///
/// # Examples
///
/// ```
/// use ratatui::layout::Constraint;
/// use ratatui_ppalla::prepared::{LayoutCtx, Preparable, PreparedLayout};
///
/// let prepared = PreparedLayout::prepare_vertical(vec![Constraint::Length(1), Constraint::Min(0)]);
/// let layout = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
/// assert_eq!(layout.rects.len(), 2);
/// assert!(!layout.cache_hit);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PreparedLayout;

impl PreparedLayout {
    /// Convenience wrapper around [`Preparable::prepare`] that builds a
    /// [`SplitSpec`] with the default ([`Direction::Vertical`]) direction.
    #[must_use]
    pub fn prepare_vertical(constraints: Vec<Constraint>) -> PreparedLayoutState {
        Self::prepare(SplitSpec::new(constraints))
    }
}

impl Preparable for PreparedLayout {
    type Prepared = PreparedLayoutState;
    type Layout = SplitLayout;
    type Input = SplitSpec;

    fn prepare(input: Self::Input) -> Self::Prepared {
        // Store the spec; the cache starts cold (no key).
        PreparedLayoutState {
            spec: input,
            cache: Mutex::new(LayoutCache::default()),
        }
    }

    /// Replace the spec and invalidate the cache. A layout's "more" is a new
    /// configuration, so appending replaces rather than extends.
    fn append(prepared: &mut Self::Prepared, more: Self::Input) {
        prepared.spec = more;
        // `&mut` access lets us replace the mutex wholesale without locking.
        prepared.cache = Mutex::new(LayoutCache::default());
    }

    fn layout(prepared: &Self::Prepared, ctx: LayoutCtx) -> Self::Layout {
        let area = area_from_ctx(ctx);
        let key = CacheKey {
            sig: constraints_signature(&prepared.spec.constraints, prepared.spec.direction),
            area,
        };

        let mut cache = prepared
            .cache
            .lock()
            .expect("layout cache mutex should never be poisoned");

        // Cache hit: same spec + same size → return cached rects.
        if cache.key.as_ref() == Some(&key) {
            return SplitLayout {
                rects: cache.rects.clone(),
                cache_hit: true,
            };
        }

        // Cache miss: evaluate via ratatui's layout solver.
        let rects = Layout::new(
            prepared.spec.direction,
            prepared.spec.constraints.iter().copied(),
        )
        .split(area)
        .to_vec();

        *cache = LayoutCache {
            key: Some(key),
            rects: rects.clone(),
        };

        SplitLayout {
            rects,
            cache_hit: false,
        }
    }
}

/// Compute a deterministic `u64` signature from the constraints and direction,
/// so that different specs produce different keys and the same spec always
/// produces the same key.
///
/// Relies on [`Constraint`] and [`Direction`] deriving [`Hash`](std::hash::Hash)
/// in ratatui 0.29. The signature is stable for the lifetime of a process but
/// must not be persisted across versions (the hasher seed is randomized per
/// process).
fn constraints_signature(constraints: &[Constraint], direction: Direction) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    constraints.hash(&mut hasher);
    direction.hash(&mut hasher);
    hasher.finish()
}

/// Derive the target [`Rect`] from a [`LayoutCtx`]: a zero-origin rectangle of
/// the given width and height. [`LayoutCtx::scroll`] and [`LayoutCtx::focus`] do
/// not affect the layout region.
fn area_from_ctx(ctx: LayoutCtx) -> Rect {
    Rect::new(0, 0, ctx.width, ctx.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // ---------- prepare ----------

    #[test]
    fn prepare_stores_spec_and_cache_is_cold() {
        let spec = SplitSpec::new(vec![Constraint::Length(3), Constraint::Min(0)]);
        let prepared = PreparedLayout::prepare(spec.clone());
        assert_eq!(prepared.spec, spec);
        // Cache starts cold: no key recorded yet.
        assert!(prepared.cache.lock().unwrap().key.is_none());
    }

    #[test]
    fn prepare_vertical_defaults_to_vertical_direction() {
        let prepared = PreparedLayout::prepare_vertical(vec![Constraint::Length(1)]);
        assert_eq!(prepared.spec.direction, Direction::Vertical);
        assert_eq!(prepared.spec.constraints.len(), 1);
    }

    // ---------- layout: vertical ----------

    #[test]
    fn layout_vertical_three_length_constraints_stack() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(3),
        ]));
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 6));
        assert_eq!(out.rects.len(), 3);
        // Stacked vertically: full width each, heights 1/2/3, advancing y.
        assert_eq!(out.rects[0], Rect::new(0, 0, 10, 1));
        assert_eq!(out.rects[1], Rect::new(0, 1, 10, 2));
        assert_eq!(out.rects[2], Rect::new(0, 3, 10, 3));
        assert!(!out.cache_hit);
    }

    // ---------- layout: horizontal ----------

    #[test]
    fn layout_horizontal_two_constraints_side_by_side() {
        let prepared = PreparedLayout::prepare(
            SplitSpec::new(vec![Constraint::Length(5), Constraint::Min(0)])
                .with_direction(Direction::Horizontal),
        );
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 4));
        assert_eq!(out.rects.len(), 2);
        // Side by side: full height each, widths 5/5, advancing x.
        assert_eq!(out.rects[0], Rect::new(0, 0, 5, 4));
        assert_eq!(out.rects[1], Rect::new(5, 0, 5, 4));
    }

    // ---------- cache: hit ----------

    #[test]
    fn cache_hit_same_args_returns_cached_rects() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(2),
            Constraint::Min(0),
        ]));
        let ctx = LayoutCtx::new(20, 8);
        let first = PreparedLayout::layout(&prepared, ctx);
        assert!(!first.cache_hit);
        let second = PreparedLayout::layout(&prepared, ctx);
        assert!(second.cache_hit);
        assert_eq!(first.rects, second.rects);
    }

    // ---------- cache: miss on size change ----------

    #[test]
    fn cache_miss_on_size_change() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]));
        let wide = PreparedLayout::layout(&prepared, LayoutCtx::new(80, 10));
        assert!(!wide.cache_hit);
        let narrow = PreparedLayout::layout(&prepared, LayoutCtx::new(40, 10));
        assert!(!narrow.cache_hit);
        assert_ne!(wide.rects, narrow.rects);
    }

    // ---------- cache: miss on constraint change ----------

    #[test]
    fn cache_miss_after_append_different_spec() {
        let mut prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(2),
            Constraint::Min(0),
        ]));
        let first = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
        assert!(!first.cache_hit);

        PreparedLayout::append(&mut prepared, SplitSpec::new(vec![Constraint::Min(0)]));
        let second = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
        // Append replaces spec + invalidates cache -> guaranteed miss.
        assert!(!second.cache_hit);
        assert_ne!(first.rects.len(), second.rects.len());
    }

    // ---------- append replaces spec + invalidates cache ----------

    #[test]
    fn append_replaces_spec_and_invalidates_cache() {
        let mut prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(3),
            Constraint::Min(0),
        ]));
        // Warm the cache.
        let _ = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));

        let new_spec = SplitSpec::new(vec![Constraint::Min(0)]);
        PreparedLayout::append(&mut prepared, new_spec.clone());
        assert_eq!(prepared.spec, new_spec);
        // Cache reset: next layout is a miss.
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
        assert!(!out.cache_hit);
    }

    #[test]
    fn append_same_spec_still_invalidates_cache() {
        // By design, append ALWAYS resets the cache (a new "more" is treated as
        // a fresh configuration even if equal), so the next layout is a miss.
        let spec = SplitSpec::new(vec![Constraint::Length(2), Constraint::Min(0)]);
        let mut prepared = PreparedLayout::prepare(spec.clone());
        let _ = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5)); // warm
        PreparedLayout::append(&mut prepared, spec);
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
        assert!(!out.cache_hit);
    }

    // ---------- constraint variety ----------

    #[test]
    fn percentage_min_fill_produce_nonempty_rects() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Percentage(50),
            Constraint::Min(1),
            Constraint::Fill(1),
        ]));
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(20, 4));
        assert_eq!(out.rects.len(), 3);
        assert!(out.rects.iter().all(|r| r.area() > 0));
    }

    #[test]
    fn single_constraint_is_whole_area() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![Constraint::Min(0)]));
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(7, 3));
        assert_eq!(out.rects.len(), 1);
        assert_eq!(out.rects[0], Rect::new(0, 0, 7, 3));
    }

    #[test]
    fn zero_size_area_does_not_panic() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(1),
            Constraint::Min(0),
        ]));
        let out = PreparedLayout::layout(&prepared, LayoutCtx::new(0, 0));
        // No panic; rects may be zero-sized.
        assert_eq!(out.rects.len(), 2);
        assert!(out.rects.iter().all(|r| r.width == 0 && r.height == 0));
    }

    // ---------- constraints_signature determinism ----------

    #[test]
    fn constraints_signature_is_deterministic_and_distinct() {
        let a = vec![Constraint::Length(5), Constraint::Min(0)];
        let b = vec![Constraint::Length(5), Constraint::Min(0)];
        let c = vec![Constraint::Length(6), Constraint::Min(0)];
        let d = vec![Constraint::Length(5), Constraint::Min(0)];

        // Same constraints + same direction => same signature.
        assert_eq!(
            constraints_signature(&a, Direction::Vertical),
            constraints_signature(&b, Direction::Vertical)
        );
        // Different constraint value => different signature.
        assert_ne!(
            constraints_signature(&a, Direction::Vertical),
            constraints_signature(&c, Direction::Vertical)
        );
        // Same constraints but different direction => different signature.
        assert_ne!(
            constraints_signature(&a, Direction::Vertical),
            constraints_signature(&d, Direction::Horizontal)
        );
        // Empty constraints still produce a stable signature.
        assert_eq!(
            constraints_signature(&[], Direction::Vertical),
            constraints_signature(&[], Direction::Vertical)
        );
    }

    // ---------- focus / scroll do not affect area ----------

    #[test]
    fn focus_and_scroll_do_not_affect_layout() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(2),
            Constraint::Min(0),
        ]));
        let plain = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
        let with_extras = PreparedLayout::layout(
            &prepared,
            LayoutCtx::new(10, 5).with_scroll(3).with_focus(1),
        );
        // scroll/focus do not change the area, so rects are identical. (The
        // second call is also a cache hit because the area signature is equal.)
        assert_eq!(plain.rects, with_extras.rects);
        assert!(with_extras.cache_hit);
    }

    // ---------- invariant loops (manual, no proptest macros) ----------

    #[test]
    fn invariant_rect_count_and_containment() {
        let specs: Vec<SplitSpec> = vec![
            SplitSpec::new(vec![Constraint::Length(2), Constraint::Min(0)]),
            SplitSpec::new(vec![Constraint::Percentage(50), Constraint::Percentage(50)]),
            SplitSpec::new(vec![
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ]),
            SplitSpec::new(vec![Constraint::Min(0)]).with_direction(Direction::Horizontal),
        ];
        for spec in &specs {
            for (w, h) in [(1u16, 1), (10, 5), (40, 12), (80, 24)] {
                let prepared = PreparedLayout::prepare(spec.clone());
                let out = PreparedLayout::layout(&prepared, LayoutCtx::new(w, h));
                assert_eq!(
                    out.rects.len(),
                    spec.constraints.len(),
                    "rect count mismatch at w={w} h={h}"
                );
                let area = Rect::new(0, 0, w, h);
                for r in &out.rects {
                    // Every rect is contained within the source area (cast to
                    // u32 to avoid overflow in the bound check).
                    assert!(
                        u32::from(r.x) + u32::from(r.width) <= u32::from(area.width),
                        "rect {r:?} exceeds area width"
                    );
                    assert!(
                        u32::from(r.y) + u32::from(r.height) <= u32::from(area.height),
                        "rect {r:?} exceeds area height"
                    );
                }
            }
        }
    }

    #[test]
    fn invariant_many_specs_and_sizes_never_panic() {
        let constraint_sets: Vec<Vec<Constraint>> = vec![
            vec![],
            vec![Constraint::Min(0)],
            vec![Constraint::Length(0), Constraint::Length(0)],
            vec![Constraint::Percentage(100)],
            vec![Constraint::Fill(1), Constraint::Fill(2)],
        ];
        for cs in &constraint_sets {
            for direction in [Direction::Vertical, Direction::Horizontal] {
                let prepared =
                    PreparedLayout::prepare(SplitSpec::new(cs.clone()).with_direction(direction));
                for (w, h) in [(0u16, 0), (1, 0), (0, 1), (10, 10)] {
                    let _ = PreparedLayout::layout(&prepared, LayoutCtx::new(w, h));
                }
            }
        }
    }

    // ---------- cache hit returns equal rects to fresh compute ----------

    #[test]
    fn cache_hit_returns_equal_rects_to_fresh_compute() {
        let spec = SplitSpec::new(vec![Constraint::Percentage(30), Constraint::Percentage(70)]);
        let ctx = LayoutCtx::new(100, 10);

        // Fresh state: first call computes (miss).
        let fresh = PreparedLayout::prepare(spec.clone());
        let first = PreparedLayout::layout(&fresh, ctx);
        assert!(!first.cache_hit);

        // Second call on the same state: cache hit, identical rects.
        let second = PreparedLayout::layout(&fresh, ctx);
        assert!(second.cache_hit);
        assert_eq!(first.rects, second.rects);

        // Independently-prepared state (cold) computes the same rects.
        let other = PreparedLayout::prepare(spec);
        let independent = PreparedLayout::layout(&other, ctx);
        assert!(!independent.cache_hit);
        assert_eq!(first.rects, independent.rects);
    }

    // ---------- derive / clone behavior ----------

    #[test]
    fn splitlayout_clone_partial_eq_eq_and_default() {
        let layout = SplitLayout {
            rects: vec![Rect::new(0, 0, 5, 5)],
            cache_hit: false,
        };
        assert_eq!(layout.clone(), layout);
        assert_eq!(layout, layout);

        let default = SplitLayout::default();
        assert!(default.rects.is_empty());
        assert!(!default.cache_hit);
    }

    #[test]
    fn preparedlayoutstate_is_clone_and_produces_equal_layouts() {
        let prepared = PreparedLayout::prepare(SplitSpec::new(vec![
            Constraint::Length(1),
            Constraint::Min(0),
        ]));
        let cloned = prepared.clone();
        // Identical specs; both compute the same rects. The clone's cache is
        // cold (independent of the original), so both are misses.
        let a = PreparedLayout::layout(&prepared, LayoutCtx::new(10, 5));
        let b = PreparedLayout::layout(&cloned, LayoutCtx::new(10, 5));
        assert_eq!(a.rects, b.rects);
        assert!(!a.cache_hit);
        assert!(!b.cache_hit);
    }

    // ---------- SplitSpec builder ----------

    #[test]
    fn split_spec_new_defaults_to_vertical() {
        let spec = SplitSpec::new(vec![Constraint::Min(0)]);
        assert_eq!(spec.direction, Direction::Vertical);
        assert_eq!(spec.constraints, vec![Constraint::Min(0)]);
    }

    #[test]
    fn split_spec_with_direction_overrides() {
        let spec = SplitSpec::new(vec![Constraint::Min(0)]).with_direction(Direction::Horizontal);
        assert_eq!(spec.direction, Direction::Horizontal);
    }
}
