//! Cache for `:has()` selector evaluation results.
//!
//! `:has()` is the most expensive pseudo-class in CSS — it requires searching
//! descendants or siblings of the subject element. During a full restyle,
//! many elements may share the same `:has()` context (e.g., all children of
//! a container matched by `:has(.active)`), leading to redundant subtree walks.
//!
//! This cache stores the boolean result of `:has()` evaluations keyed by the
//! combination of (element identity, selector identity). When the same `:has()`
//! selector is tested against the same element again, the cache returns the
//! previous result in O(1) instead of re-walking the subtree.
//!
//! # Key Design
//!
//! The cache key is `(OpaqueElement, usize)` where:
//! - `OpaqueElement` identifies the subject element
//! - `usize` is the pointer address of the `RelativeSelectorList`, uniquely
//!   identifying the `:has()` selector. This works because selectors are
//!   allocated once at parse time and never move.
//!
//! # Scope
//!
//! The cache is valid for a single restyle pass. Between restyles, the DOM
//! may have changed (children added/removed, classes toggled), invalidating
//! all cached `:has()` results. Call `clear()` between passes.
//!
//! # Comparison with Stylo
//!
//! Stylo uses a more complex `HasSelectorMatchingContext` with bloom filters
//! and traversal state. We start with a simple result cache — if profiling
//! shows `:has()` is a bottleneck, we can add bloom-based pre-filtering.

use crate::fxhash::FxHashMap;
use crate::opaque::OpaqueElement;

/// Cached result of a `:has()` evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasResult {
    /// The `:has()` selector matched this element.
    Matched,
    /// The `:has()` selector did NOT match this element.
    NotMatched,
}

impl HasResult {
    /// Convert to a boolean for matching.
    #[inline]
    pub fn matched(self) -> bool {
        self == Self::Matched
    }
}

impl From<bool> for HasResult {
    #[inline]
    fn from(matched: bool) -> Self {
        if matched { Self::Matched } else { Self::NotMatched }
    }
}

/// Cache for `:has()` selector evaluation results.
///
/// Stores `(element, selector) → matched/not-matched` mappings.
/// Valid for a single restyle pass — call `clear()` between passes.
pub struct HasCache {
    results: FxHashMap<(OpaqueElement, usize), HasResult>,
}

impl HasCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            results: FxHashMap::default(),
        }
    }

    /// Look up a cached `:has()` result.
    ///
    /// `selector_key` should be the pointer address of the `RelativeSelectorList`.
    #[inline]
    pub fn get(&self, element: OpaqueElement, selector_key: usize) -> Option<HasResult> {
        self.results.get(&(element, selector_key)).copied()
    }

    /// Store a `:has()` evaluation result.
    #[inline]
    pub fn insert(&mut self, element: OpaqueElement, selector_key: usize, result: HasResult) {
        self.results.insert((element, selector_key), result);
    }

    /// Get a cached result or compute and cache it.
    #[inline]
    pub fn get_or_insert(
        &mut self,
        element: OpaqueElement,
        selector_key: usize,
        compute: impl FnOnce() -> bool,
    ) -> bool {
        self.results
            .entry((element, selector_key))
            .or_insert_with(|| HasResult::from(compute()))
            .matched()
    }

    /// Clear all cached results. Call between restyle passes.
    pub fn clear(&mut self) {
        self.results.clear();
    }

    /// Number of cached entries (for diagnostics).
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

impl Default for HasCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_and_miss() {
        let mut cache = HasCache::new();
        let el = OpaqueElement::new(1);

        assert!(cache.get(el, 100).is_none());

        cache.insert(el, 100, HasResult::Matched);
        assert_eq!(cache.get(el, 100), Some(HasResult::Matched));

        cache.insert(el, 200, HasResult::NotMatched);
        assert_eq!(cache.get(el, 200), Some(HasResult::NotMatched));
    }

    #[test]
    fn get_or_insert_caches() {
        let mut cache = HasCache::new();
        let el = OpaqueElement::new(1);
        let mut calls = 0;

        let result = cache.get_or_insert(el, 100, || { calls += 1; true });
        assert!(result);
        assert_eq!(calls, 1);

        // Second call should hit cache.
        let result = cache.get_or_insert(el, 100, || { calls += 1; false });
        assert!(result); // Still true from first call.
        assert_eq!(calls, 1);
    }

    #[test]
    fn clear_invalidates() {
        let mut cache = HasCache::new();
        let el = OpaqueElement::new(1);
        cache.insert(el, 100, HasResult::Matched);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
        assert!(cache.get(el, 100).is_none());
    }

    #[test]
    fn different_selectors_independent() {
        let mut cache = HasCache::new();
        let el = OpaqueElement::new(1);
        cache.insert(el, 100, HasResult::Matched);
        cache.insert(el, 200, HasResult::NotMatched);

        assert!(cache.get(el, 100).unwrap().matched());
        assert!(!cache.get(el, 200).unwrap().matched());
    }
}
