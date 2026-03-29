//! Matching context — central state for a selector matching pass.
//!
//! During a restyle, the matching engine needs more than just a selector and an
//! element. It needs shared state:
//! - **Bloom filter** — ancestor fast-rejection (shared across the tree walk)
//! - **Scope element** — which element `:scope` refers to
//! - **Visited handling** — `:visited` privacy policy
//! - **Quirks mode** — case-insensitive class matching in HTML quirks mode
//! - **Caches** — nth-index, `:has()` results (shared across all matching)
//! - **Selector flags** — collected during matching for invalidation
//!
//! **Stylo's approach**: A monolithic `MatchingContext<'a, Impl>` generic over
//! `SelectorImpl` with 12+ fields. ALL matching goes through it.
//!
//! **Our approach**: Three API levels with increasing capability:
//!
//! 1. `matches(selector, element)` — Zero context. For tests, simple queries.
//! 2. `matches_with_bloom(selector, element, bloom)` — Just bloom filter.
//! 3. `matches_in_context(selector, element, ctx)` — Full context.
//!
//! This means simple use cases pay zero overhead, while production restyle
//! gets the full optimization stack. Stylo forces all callers through the
//! full context even for trivial matching.

use crate::bloom::AncestorBloom;
use crate::flags::{ElementSelectorFlags, MatchingFlags};
use crate::has_cache::HasCache;
use crate::nth_cache::NthIndexCache;
use crate::opaque::OpaqueElement;

/// How to handle `:visited` and `:link` during matching.
///
/// Browsers must not leak visited-link state via computed styles (timing
/// attacks, getComputedStyle). This enum controls the matching behavior.
///
/// In normal mode, `:visited` matches visited links and `:link` matches
/// unvisited links. In privacy-restricted modes, the engine either treats
/// all links as unvisited or matches both states simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VisitedHandling {
    /// All links treated as unvisited. `:visited` never matches.
    /// Safe default — no information leakage.
    #[default]
    AllLinksUnvisited,

    /// Both `:link` and `:visited` can match. Used during normal styling
    /// where visited state is applied.
    AllLinksVisitedAndUnvisited,

    /// The "relevant link" (the link being styled) is treated as visited;
    /// all other links are unvisited. Used for the limited set of properties
    /// that `:visited` is allowed to change (color, background-color, etc.).
    RelevantLinkVisited,
}

/// HTML document quirks mode — affects case sensitivity of class matching.
///
/// In quirks mode (legacy HTML documents), class names in selectors are
/// matched case-insensitively. In standards mode, they're case-sensitive.
///
/// Most modern documents use standards mode; quirks mode exists for
/// backwards compatibility with pre-HTML5 documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QuirksMode {
    /// Standards-compliant: class names are case-sensitive.
    #[default]
    NoQuirks,

    /// Limited quirks mode (triggered by certain doctypes).
    /// Class matching is still case-sensitive.
    LimitedQuirks,

    /// Full quirks mode (no doctype or old doctypes).
    /// Class names in selectors are matched case-insensitively.
    Quirks,
}

impl QuirksMode {
    /// Whether class name matching should be case-insensitive.
    #[inline]
    pub fn classes_case_insensitive(self) -> bool {
        self == Self::Quirks
    }
}

/// Aggregated caches shared across all matching in a single restyle pass.
///
/// Create once per restyle, pass to `MatchingContext`. After the restyle,
/// either `clear()` for next restyle or keep with `bump_generation()`.
pub struct SelectorCaches {
    /// Cache for `:nth-child` / `:nth-of-type` index computations.
    pub nth: NthIndexCache,
    /// Cache for `:has()` evaluation results.
    pub has: HasCache,
}

impl SelectorCaches {
    pub fn new() -> Self {
        Self {
            nth: NthIndexCache::new(),
            has: HasCache::new(),
        }
    }

    /// Clear all caches. Call between restyle passes.
    pub fn clear(&mut self) {
        self.nth.clear();
        self.has.clear();
    }
}

impl Default for SelectorCaches {
    fn default() -> Self {
        Self::new()
    }
}

/// Central state holder for a selector matching pass.
///
/// Groups all the per-restyle state that the matching engine needs beyond
/// the selector and element themselves.
pub struct MatchingContext<'a> {
    /// Bloom filter for ancestor fast-rejection.
    /// When set, descendant combinator matching checks the bloom filter
    /// before walking ancestors. ~0.3% false positive rate at depth 50.
    pub bloom_filter: Option<&'a AncestorBloom>,

    /// The element that `:scope` matches against.
    /// - In `querySelector()`: the element the query was called on.
    /// - In `@scope` rules: the scoping root element.
    /// - If `None`: `:scope` matches `:root` (spec default).
    pub scope_element: Option<OpaqueElement>,

    /// The element that is the current "relevant link" for `:visited` matching.
    /// Only meaningful when `visited_handling` is `RelevantLinkVisited`.
    pub relevant_link: Option<OpaqueElement>,

    /// `:visited` / `:link` matching policy.
    pub visited_handling: VisitedHandling,

    /// HTML quirks mode — affects class name case sensitivity.
    pub quirks_mode: QuirksMode,

    /// Behavior flags (collect selector flags, invalidation mode, etc.).
    pub flags: MatchingFlags,

    /// Shared caches (nth-index, has-cache).
    /// Mutable because cache lookups may insert computed values.
    pub caches: &'a mut SelectorCaches,

    /// Accumulated selector flags from matching.
    /// After matching, the caller can read these and propagate to the DOM.
    /// Only populated when `flags.contains(COLLECT_SELECTOR_FLAGS)`.
    pub(crate) collected_element_flags: ElementSelectorFlags,
}

impl<'a> MatchingContext<'a> {
    /// Create a new matching context with the given caches.
    /// All optional features are off by default.
    pub fn new(caches: &'a mut SelectorCaches) -> Self {
        Self {
            bloom_filter: None,
            scope_element: None,
            relevant_link: None,
            visited_handling: VisitedHandling::default(),
            quirks_mode: QuirksMode::default(),
            flags: MatchingFlags::empty(),
            caches,
            collected_element_flags: ElementSelectorFlags::empty(),
        }
    }

    /// Create a context for a normal restyle pass with bloom filter.
    pub fn for_restyle(
        bloom: &'a AncestorBloom,
        quirks_mode: QuirksMode,
        caches: &'a mut SelectorCaches,
    ) -> Self {
        Self {
            bloom_filter: Some(bloom),
            scope_element: None,
            relevant_link: None,
            visited_handling: VisitedHandling::AllLinksUnvisited,
            quirks_mode,
            flags: MatchingFlags::COLLECT_SELECTOR_FLAGS,
            caches,
            collected_element_flags: ElementSelectorFlags::empty(),
        }
    }

    /// Create a context for invalidation matching (relaxed, conservative).
    pub fn for_invalidation(caches: &'a mut SelectorCaches) -> Self {
        Self {
            bloom_filter: None,
            scope_element: None,
            relevant_link: None,
            visited_handling: VisitedHandling::AllLinksVisitedAndUnvisited,
            quirks_mode: QuirksMode::default(),
            flags: MatchingFlags::MATCHING_FOR_INVALIDATION,
            caches,
            collected_element_flags: ElementSelectorFlags::empty(),
        }
    }

    /// Whether we're matching for invalidation (relaxed mode).
    #[inline]
    pub fn matching_for_invalidation(&self) -> bool {
        self.flags.contains(MatchingFlags::MATCHING_FOR_INVALIDATION)
    }

    /// Whether to collect selector flags during matching.
    #[inline]
    pub fn needs_selector_flags(&self) -> bool {
        self.flags.contains(MatchingFlags::COLLECT_SELECTOR_FLAGS)
    }

    /// Record a selector flag on the current element.
    #[inline]
    pub(crate) fn add_element_flag(&mut self, flag: ElementSelectorFlags) {
        if self.needs_selector_flags() {
            self.collected_element_flags |= flag;
        }
    }

    /// Read and reset the collected element selector flags.
    pub fn take_element_flags(&mut self) -> ElementSelectorFlags {
        let flags = self.collected_element_flags;
        self.collected_element_flags = ElementSelectorFlags::empty();
        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_context() {
        let mut caches = SelectorCaches::new();
        let ctx = MatchingContext::new(&mut caches);
        assert!(ctx.bloom_filter.is_none());
        assert!(ctx.scope_element.is_none());
        assert_eq!(ctx.visited_handling, VisitedHandling::AllLinksUnvisited);
        assert_eq!(ctx.quirks_mode, QuirksMode::NoQuirks);
        assert!(!ctx.matching_for_invalidation());
        assert!(!ctx.needs_selector_flags());
    }

    #[test]
    fn restyle_context() {
        let bloom = AncestorBloom::new();
        let mut caches = SelectorCaches::new();
        let ctx = MatchingContext::for_restyle(&bloom, QuirksMode::NoQuirks, &mut caches);
        assert!(ctx.bloom_filter.is_some());
        assert!(ctx.needs_selector_flags());
        assert!(!ctx.matching_for_invalidation());
    }

    #[test]
    fn invalidation_context() {
        let mut caches = SelectorCaches::new();
        let ctx = MatchingContext::for_invalidation(&mut caches);
        assert!(ctx.matching_for_invalidation());
        assert!(!ctx.needs_selector_flags());
        assert_eq!(ctx.visited_handling, VisitedHandling::AllLinksVisitedAndUnvisited);
    }

    #[test]
    fn collect_flags() {
        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        ctx.flags |= MatchingFlags::COLLECT_SELECTOR_FLAGS;
        ctx.add_element_flag(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH);
        ctx.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
        let flags = ctx.take_element_flags();
        assert!(flags.contains(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH));
        assert!(flags.contains(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR));
        assert!(ctx.collected_element_flags.is_empty());
    }

    #[test]
    fn quirks_mode_classes() {
        assert!(!QuirksMode::NoQuirks.classes_case_insensitive());
        assert!(!QuirksMode::LimitedQuirks.classes_case_insensitive());
        assert!(QuirksMode::Quirks.classes_case_insensitive());
    }
}
