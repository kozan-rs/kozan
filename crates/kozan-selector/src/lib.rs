//! CSS selector parsing and matching for the Kozan UI platform.
//!
//! A high-performance selector engine with three API levels:
//!
//! 1. **Simple**: `matches(selector, element)` — zero context, for tests
//! 2. **Bloom**: `matches_with_bloom(selector, element, bloom)` — ancestor pruning
//! 3. **Full**: `matches_in_context(selector, element, ctx)` — bloom, scope,
//!    visited handling, quirks mode, nth caching, selector flags
//!
//! Performance design:
//! - Atom-based comparison (O(1) pointer equality for tag/class/ID)
//! - ElementState bitflags (1 AND instruction for pseudo-class matching)
//! - Integer-chunk keyword matching via `css_match!` (2-5x faster than PHF)
//! - Bloom filter ancestor pruning (~0.3% false positive at depth 50)
//! - NthIndexCache for O(1) repeated nth lookups during restyle
//! - SelectorVisitor for invalidation and dependency analysis
//! - ElementSelectorFlags for incremental restyle

pub mod attr;
pub mod bloom;
pub mod context;
pub mod element;
pub mod flags;
pub mod fxhash;
pub mod has_cache;
pub mod invalidation;
pub mod kleene;
pub mod matching;
pub mod nth;
pub mod nth_cache;
pub mod opaque;
pub mod parser;
pub mod pseudo_class;
pub mod pseudo_element;
pub mod rule_map;
pub mod specificity;
pub mod types;
pub mod visitor;

#[cfg(test)]
mod spec_tests;

pub use attr::{AttrOperation, AttrSelector, CaseSensitivity};
pub use context::{MatchingContext, QuirksMode, SelectorCaches, VisitedHandling};
pub use element::Element;
pub use flags::{ElementSelectorFlags, MatchingFlags};
pub use has_cache::{HasCache, HasResult};
pub use invalidation::{InvalidationEntry, InvalidationMap};
pub use kleene::KleeneValue;
pub use opaque::OpaqueElement;
pub use pseudo_class::{ElementState, PseudoClass};
pub use pseudo_element::PseudoElement;
pub use rule_map::{RuleEntry, RuleMap};
pub use specificity::Specificity;
pub use types::{
    Combinator, Component, Direction, KeySelector, NamespaceConstraint, NthData,
    HasTraversal, RelativeSelector, RelativeSelectorList, Selector, SelectorDeps, SelectorHints,
    SelectorList,
};
pub use visitor::{SelectorListKind, SelectorVisitor};

/// Parse an `An+B` expression from CSS text (e.g. `"2n+1"`, `"odd"`, `"3"`).
/// Returns `(a, b)` tuple on success, `None` on parse failure.
/// Requires the entire input to be consumed (no trailing tokens).
pub fn parse_anb(css: &str) -> Option<(i32, i32)> {
    let mut input = cssparser::ParserInput::new(css);
    let mut parser = cssparser::Parser::new(&mut input);
    let result = nth::parse_nth(&mut parser).ok()?;
    // Ensure entire input was consumed
    if parser.expect_exhausted().is_err() {
        return None;
    }
    Some(result)
}
