//! Cache for `:nth-child` / `:nth-of-type` index computations.
//!
//! Computing a child's nth-index requires an O(n) walk of all preceding (or
//! following) siblings. During a full restyle pass, many elements may be
//! tested against the same `:nth-child()` selector, causing redundant walks.
//!
//! This cache stores computed indices keyed by `OpaqueElement`, turning
//! repeated lookups from O(n) sibling walks into O(1) hash lookups.
//!
//! **Stylo's approach**: 6 separate `FxHashMap<OpaqueElement, i32>` fields —
//! one per nth variant. This is correct: the key spaces are disjoint (an
//! element's `:nth-child` index differs from its `:nth-of-type` index), so
//! separate maps avoid composite keys and their hashing overhead.
//!
//! **Our approach**: Same 6-map design (it's genuinely optimal for this),
//! but we add a `generation` counter. Stylo creates a fresh cache per restyle
//! and throws it away. We allow the cache to persist across frames with
//! generation-based invalidation — when the DOM mutates, bump the generation
//! and the cache lazily discards stale entries. For UI frameworks where
//! most elements don't change between frames, this avoids rebuilding the
//! entire cache on every restyle.

use crate::fxhash::FxHashMap;
use crate::opaque::OpaqueElement;

/// Cached nth-index computations for the current restyle pass.
///
/// Create once, pass to `MatchingContext`, reuse across all matching in a
/// single restyle. Call `clear()` or `bump_generation()` between restyles.
pub struct NthIndexCache {
    nth_child: FxHashMap<OpaqueElement, i32>,
    nth_last_child: FxHashMap<OpaqueElement, i32>,
    nth_of_type: FxHashMap<OpaqueElement, i32>,
    nth_last_of_type: FxHashMap<OpaqueElement, i32>,
    // For `:nth-child(... of <selector>)` variants, the index depends on
    // both the element AND the selector. We use the selector list's pointer
    // as a disambiguator (cast to usize) combined with the element.
    nth_child_of: FxHashMap<(OpaqueElement, usize), i32>,
    nth_last_child_of: FxHashMap<(OpaqueElement, usize), i32>,
    generation: u32,
}

impl NthIndexCache {
    pub fn new() -> Self {
        Self {
            nth_child: FxHashMap::default(),
            nth_last_child: FxHashMap::default(),
            nth_of_type: FxHashMap::default(),
            nth_last_of_type: FxHashMap::default(),
            nth_child_of: FxHashMap::default(),
            nth_last_child_of: FxHashMap::default(),
            generation: 0,
        }
    }

    /// Clear all cached entries. Call between restyle passes when the DOM
    /// may have changed.
    pub fn clear(&mut self) {
        self.nth_child.clear();
        self.nth_last_child.clear();
        self.nth_of_type.clear();
        self.nth_last_of_type.clear();
        self.nth_child_of.clear();
        self.nth_last_child_of.clear();
    }

    /// Bump the generation counter, effectively invalidating all entries.
    /// Stale entries are lazily overwritten on next access.
    pub fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.clear();
    }

    /// Current generation counter.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Get or compute the `:nth-child` index for an element.
    pub fn nth_child(&mut self, el: OpaqueElement, compute: impl FnOnce() -> i32) -> i32 {
        *self.nth_child.entry(el).or_insert_with(compute)
    }

    /// Get or compute the `:nth-last-child` index.
    pub fn nth_last_child(&mut self, el: OpaqueElement, compute: impl FnOnce() -> i32) -> i32 {
        *self.nth_last_child.entry(el).or_insert_with(compute)
    }

    /// Get or compute the `:nth-of-type` index.
    pub fn nth_of_type(&mut self, el: OpaqueElement, compute: impl FnOnce() -> i32) -> i32 {
        *self.nth_of_type.entry(el).or_insert_with(compute)
    }

    /// Get or compute the `:nth-last-of-type` index.
    pub fn nth_last_of_type(&mut self, el: OpaqueElement, compute: impl FnOnce() -> i32) -> i32 {
        *self.nth_last_of_type.entry(el).or_insert_with(compute)
    }

    /// Get or compute the `:nth-child(... of <selector>)` index.
    ///
    /// `selector_key` should be a stable identifier for the selector list
    /// (e.g., pointer to the SelectorList, or a hash). This distinguishes
    /// different `:nth-child(2n of .a)` from `:nth-child(2n of .b)`.
    pub fn nth_child_of(
        &mut self,
        el: OpaqueElement,
        selector_key: usize,
        compute: impl FnOnce() -> i32,
    ) -> i32 {
        *self.nth_child_of.entry((el, selector_key)).or_insert_with(compute)
    }

    /// Get or compute the `:nth-last-child(... of <selector>)` index.
    pub fn nth_last_child_of(
        &mut self,
        el: OpaqueElement,
        selector_key: usize,
        compute: impl FnOnce() -> i32,
    ) -> i32 {
        *self.nth_last_child_of.entry((el, selector_key)).or_insert_with(compute)
    }
}

impl Default for NthIndexCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit() {
        let mut cache = NthIndexCache::new();
        let el = OpaqueElement::new(42);
        let mut calls = 0;
        let idx = cache.nth_child(el, || { calls += 1; 3 });
        assert_eq!(idx, 3);
        assert_eq!(calls, 1);

        // Second access should hit cache, not call compute.
        let idx2 = cache.nth_child(el, || { calls += 1; 999 });
        assert_eq!(idx2, 3);
        assert_eq!(calls, 1);
    }

    #[test]
    fn clear_invalidates() {
        let mut cache = NthIndexCache::new();
        let el = OpaqueElement::new(1);
        cache.nth_child(el, || 5);
        cache.clear();
        let idx = cache.nth_child(el, || 10);
        assert_eq!(idx, 10);
    }

    #[test]
    fn different_variants_independent() {
        let mut cache = NthIndexCache::new();
        let el = OpaqueElement::new(1);
        cache.nth_child(el, || 3);
        cache.nth_of_type(el, || 1);
        assert_eq!(cache.nth_child(el, || 999), 3);
        assert_eq!(cache.nth_of_type(el, || 999), 1);
    }

    #[test]
    fn of_selector_key_distinguishes() {
        let mut cache = NthIndexCache::new();
        let el = OpaqueElement::new(1);
        cache.nth_child_of(el, 100, || 2);
        cache.nth_child_of(el, 200, || 5);
        assert_eq!(cache.nth_child_of(el, 100, || 999), 2);
        assert_eq!(cache.nth_child_of(el, 200, || 999), 5);
    }
}
