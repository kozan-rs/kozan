//! Element selector flags for incremental restyle and invalidation.
//!
//! During selector matching, the engine discovers which *kinds* of selectors
//! affect each element. These flags are set on elements as a side-effect of
//! matching and later inform the restyle system about what needs invalidation.
//!
//! **Why this matters**: Without these flags, ANY DOM mutation (child added,
//! class changed, attribute toggled) would require a full document restyle.
//! With them, the restyle system knows exactly which elements need re-matching:
//!
//! - Element has `HAS_EDGE_CHILD_SELECTOR`? → Restyle when children added/removed
//!   at the edges (first/last child changed).
//! - Element has `HAS_SLOW_SELECTOR_NTH`? → Restyle all siblings when any
//!   sibling is added/removed (nth indices shift).
//! - Element has `HAS_EMPTY_SELECTOR`? → Restyle when children change (may
//!   become empty or non-empty).
//!
//! **Stylo's approach**: Sets these as side-effects deep inside matching.rs
//! via a `NeedsSelectorFlags` context flag.
//!
//! **Our approach**: Same concept (these flags are essential for performance),
//! but the flag-setting is explicit — `MatchingContext` collects them, and the
//! caller decides when/how to apply them to the DOM. This separates the
//! "what flags are needed" question from the "how to store them" question,
//! making the matching engine DOM-agnostic.

use bitflags::bitflags;

bitflags! {
    /// Flags set on DOM elements during selector matching to enable
    /// targeted invalidation on DOM mutations.
    ///
    /// Set by the matching engine, read by the restyle/invalidation system.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ElementSelectorFlags: u16 {
        /// A selector with a child/descendant combinator matched this element
        /// as an ancestor. Implies: when children are added/removed/reordered,
        /// descendants may need restyling.
        ///
        /// Triggered by: `div > .foo`, `.parent .child`
        const HAS_SLOW_SELECTOR = 1 << 0;

        /// A general sibling combinator (`~`) matched this element's sibling
        /// context. Later siblings must restyle when this element changes.
        ///
        /// Triggered by: `div ~ .foo`
        const HAS_SLOW_SELECTOR_LATER_SIBLINGS = 1 << 1;

        /// An `:nth-child()` or `:nth-last-child()` selector matched.
        /// When siblings are added/removed, ALL siblings' nth-indices change,
        /// so all siblings with this flag need restyling.
        ///
        /// Triggered by: `:nth-child(2n)`, `:nth-last-child(odd)`
        const HAS_SLOW_SELECTOR_NTH = 1 << 2;

        /// An `:nth-child(... of <selector>)` selector matched.
        /// More expensive than plain nth — requires re-evaluating the
        /// filter selector on all siblings.
        ///
        /// Triggered by: `:nth-child(2n of .active)`
        const HAS_SLOW_SELECTOR_NTH_OF = 1 << 3;

        /// A `:first-child`, `:last-child`, or `:only-child` selector matched.
        /// Only need to restyle when children are added/removed at the edges.
        ///
        /// Triggered by: `:first-child`, `:last-child`, `:only-child`
        const HAS_EDGE_CHILD_SELECTOR = 1 << 4;

        /// An `:empty` selector matched. Must restyle when the element
        /// gains or loses all children/text content.
        ///
        /// Triggered by: `:empty`
        const HAS_EMPTY_SELECTOR = 1 << 5;

        /// This element anchors a `:has()` relative selector as the subject.
        /// When descendants/siblings change, the `:has()` condition may flip.
        ///
        /// Triggered by: `:has(.child)`, `:has(> .direct-child)`
        const ANCHORS_RELATIVE_SELECTOR = 1 << 6;

        /// This element is inside a `:has()` scope but is NOT the subject.
        /// It was traversed during `:has()` matching.
        ///
        /// Triggered by: inner elements matched during `:has()` evaluation
        const ANCHORS_RELATIVE_SELECTOR_NON_SUBJECT = 1 << 7;

        /// The `:has()` selector searches siblings (`:has(~ .foo)`).
        const RELATIVE_SELECTOR_SEARCH_SIBLING = 1 << 8;

        /// The `:has()` selector searches ancestors (`:has(.foo)` inside
        /// a descendant combinator context).
        const RELATIVE_SELECTOR_SEARCH_ANCESTOR = 1 << 9;
    }
}

bitflags! {
    /// Flags controlling how the matching engine behaves.
    ///
    /// These are set on `MatchingContext` to control matching behavior
    /// without changing the matching algorithm.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MatchingFlags: u8 {
        /// Collect `ElementSelectorFlags` during matching.
        /// When set, the matching engine records which flag-triggering
        /// selectors matched, so the caller can propagate them to the DOM.
        const COLLECT_SELECTOR_FLAGS = 1 << 0;

        /// We're matching for invalidation — relaxed mode.
        /// Some pseudo-classes that can't be statically evaluated
        /// return `true` (assume they might match) to be conservative.
        const MATCHING_FOR_INVALIDATION = 1 << 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_combining() {
        let flags = ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH
            | ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR;
        assert!(flags.contains(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH));
        assert!(flags.contains(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR));
        assert!(!flags.contains(ElementSelectorFlags::HAS_EMPTY_SELECTOR));
    }

    #[test]
    fn matching_flags() {
        let flags = MatchingFlags::COLLECT_SELECTOR_FLAGS;
        assert!(flags.contains(MatchingFlags::COLLECT_SELECTOR_FLAGS));
        assert!(!flags.contains(MatchingFlags::MATCHING_FOR_INVALIDATION));
    }
}
