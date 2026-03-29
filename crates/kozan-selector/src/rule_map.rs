//! Rule bucketing for O(1) rule lookup by key selector.
//!
//! Instead of testing every CSS rule against every element (O(rules × elements)),
//! rules are bucketed by their **key selector** — the rightmost simple selector:
//!
//! - `#id` rules → `FxHashMap<Atom, Range>` — lookup by element's ID
//! - `.class` rules → `FxHashMap<Atom, Range>` — lookup by element's classes
//! - `tag` rules → `FxHashMap<Atom, Range>` — lookup by element's tag name
//! - Universal rules → flat range — always tested
//!
//! **Flat storage**: all rule entries live in a single `Vec<RuleEntry>`. Bucket
//! maps store `(start, len)` index ranges into this vec. This eliminates
//! per-bucket `Vec` allocation, improves cache locality, and reduces memory
//! fragmentation compared to `HashMap<Atom, Vec<RuleEntry>>`.
//!
//! **FxHashMap**: Atom keys are pointer-sized integers — FxHash (single multiply)
//! is 2-5x faster than SipHash for these keys.
//!
//! **Pre-sorted buckets**: entries within each bucket are sorted by
//! (specificity DESC, source_order DESC) at build time, so queries return
//! already-sorted results with zero runtime sorting.

use kozan_atom::Atom;
use smallvec::SmallVec;

use crate::context::MatchingContext;
use crate::element::Element;
use crate::fxhash::FxHashMap;
use crate::matching;
use crate::specificity::Specificity;
use crate::types::{KeySelector, Selector};

/// A single rule entry — selector + opaque data index.
///
/// `data` is an opaque index into whatever storage the caller uses
/// (e.g., a `Vec<PropertyDeclaration>`). Keeps the rule map generic.
#[derive(Debug, Clone)]
pub struct RuleEntry {
    pub selector: Selector,
    pub specificity: Specificity,
    pub data: u32,
    pub source_order: u32,
}

/// Index range into the flat `entries` vec.
#[derive(Debug, Clone, Copy)]
struct BucketRange {
    start: u32,
    len: u32,
}

/// Builder for constructing a `RuleMap`. Collects rules, then `build()` sorts
/// and compacts them into the final flat structure.
#[derive(Debug, Default)]
pub struct RuleMapBuilder {
    id_staging: FxHashMap<Atom, Vec<RuleEntry>>,
    class_staging: FxHashMap<Atom, Vec<RuleEntry>>,
    type_staging: FxHashMap<Atom, Vec<RuleEntry>>,
    universal_staging: Vec<RuleEntry>,
    rule_count: u32,
}

impl RuleMapBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule. Source order is auto-assigned.
    pub fn insert(&mut self, selector: Selector, data: u32) {
        let source_order = self.rule_count;
        self.rule_count += 1;
        let specificity = selector.specificity();
        let key = selector.hints().key.clone();

        let entry = RuleEntry { selector, specificity, data, source_order };

        match key {
            KeySelector::Id(atom) => {
                self.id_staging.entry(atom).or_default().push(entry);
            }
            KeySelector::Class(atom) => {
                self.class_staging.entry(atom).or_default().push(entry);
            }
            KeySelector::Type(atom) => {
                self.type_staging.entry(atom).or_default().push(entry);
            }
            KeySelector::Universal => {
                self.universal_staging.push(entry);
            }
        }
    }

    /// Build the final `RuleMap` with flat storage and pre-sorted buckets.
    pub fn build(self) -> RuleMap {
        let capacity = self.rule_count as usize;
        let mut entries = Vec::with_capacity(capacity);
        let mut id_index = FxHashMap::default();
        let mut class_index = FxHashMap::default();
        let mut type_index = FxHashMap::default();

        // Flatten each staging map into entries + index. Takes ownership — zero clones.
        flatten_into(self.id_staging, &mut entries, &mut id_index);
        flatten_into(self.class_staging, &mut entries, &mut class_index);
        flatten_into(self.type_staging, &mut entries, &mut type_index);

        let universal_start = entries.len() as u32;
        let mut universal = self.universal_staging;
        sort_bucket(&mut universal);
        let universal_len = universal.len() as u32;
        entries.extend(universal);
        let universal_range = BucketRange { start: universal_start, len: universal_len };

        RuleMap {
            entries: entries.into_boxed_slice(),
            id_index,
            class_index,
            type_index,
            universal_range,
            rule_count: self.rule_count,
        }
    }
}

/// Sort a bucket by specificity DESC, then source_order DESC (later wins).
#[inline]
fn sort_bucket(bucket: &mut [RuleEntry]) {
    bucket.sort_by(|a, b| {
        b.specificity.cmp(&a.specificity)
            .then(b.source_order.cmp(&a.source_order))
    });
}

/// Flatten a staging HashMap into the flat entries vec and build the index.
/// Takes ownership — zero clones.
fn flatten_into(
    staging: FxHashMap<Atom, Vec<RuleEntry>>,
    entries: &mut Vec<RuleEntry>,
    index: &mut FxHashMap<Atom, BucketRange>,
) {
    for (atom, mut bucket) in staging {
        sort_bucket(&mut bucket);
        let start = entries.len() as u32;
        let len = bucket.len() as u32;
        entries.extend(bucket);
        index.insert(atom, BucketRange { start, len });
    }
}

/// Bucketed rule map — flat storage, FxHash lookups, pre-sorted buckets.
///
/// Constructed via `RuleMapBuilder::build()`. Immutable after construction.
#[derive(Debug)]
pub struct RuleMap {
    entries: Box<[RuleEntry]>,
    id_index: FxHashMap<Atom, BucketRange>,
    class_index: FxHashMap<Atom, BucketRange>,
    type_index: FxHashMap<Atom, BucketRange>,
    universal_range: BucketRange,
    rule_count: u32,
}

impl RuleMap {
    /// Create a builder for constructing a RuleMap.
    pub fn builder() -> RuleMapBuilder {
        RuleMapBuilder::new()
    }

    /// Visit all matching rules for an element via callback. **Zero allocation.**
    ///
    /// Rules are visited: ID → classes → type → universal.
    /// Within each bucket, rules are pre-sorted by specificity (highest first).
    pub fn for_each_matching<E, F>(&self, element: &E, mut f: F)
    where
        E: Element,
        F: for<'a> FnMut(&'a RuleEntry),
    {
        if let Some(id) = element.id() {
            if let Some(&range) = self.id_index.get(id) {
                self.match_range(range, element, &mut f);
            }
        }

        element.each_class(|class| {
            if let Some(&range) = self.class_index.get(class) {
                self.match_range(range, element, &mut f);
            }
        });

        if let Some(&range) = self.type_index.get(element.local_name()) {
            self.match_range(range, element, &mut f);
        }

        self.match_range(self.universal_range, element, &mut f);
    }

    /// Collect all matching rules into a SmallVec. Pre-sorted by specificity.
    pub fn find_matching<E: Element>(&self, element: &E) -> SmallVec<[&RuleEntry; 16]> {
        let mut out: SmallVec<[&RuleEntry; 16]> = SmallVec::new();

        if let Some(id) = element.id() {
            if let Some(&range) = self.id_index.get(id) {
                self.collect_range(range, element, &mut out);
            }
        }

        element.each_class(|class| {
            if let Some(&range) = self.class_index.get(class) {
                self.collect_range(range, element, &mut out);
            }
        });

        if let Some(&range) = self.type_index.get(element.local_name()) {
            self.collect_range(range, element, &mut out);
        }

        self.collect_range(self.universal_range, element, &mut out);

        out.sort_by(|a, b| {
            b.specificity.cmp(&a.specificity)
                .then(b.source_order.cmp(&a.source_order))
        });
        out
    }

    #[inline]
    fn collect_range<'a, E: Element>(
        &'a self,
        range: BucketRange,
        element: &E,
        out: &mut SmallVec<[&'a RuleEntry; 16]>,
    ) {
        let start = range.start as usize;
        let end = start + range.len as usize;
        for entry in &self.entries[start..end] {
            if matching::matches(&entry.selector, element) {
                out.push(entry);
            }
        }
    }

    #[inline]
    fn match_range<E: Element, F: FnMut(&RuleEntry)>(
        &self,
        range: BucketRange,
        element: &E,
        f: &mut F,
    ) {
        let start = range.start as usize;
        let end = start + range.len as usize;
        for entry in &self.entries[start..end] {
            if matching::matches(&entry.selector, element) {
                f(entry);
            }
        }
    }

    /// Visit all matching rules using full MatchingContext (bloom, nth cache, flags).
    ///
    /// Use this during restyle — it enables bloom filter pruning, nth caching,
    /// and selector flag collection that `for_each_matching` skips.
    pub fn for_each_matching_in_context<E, F>(
        &self,
        element: &E,
        ctx: &mut MatchingContext,
        mut f: F,
    )
    where
        E: Element,
        F: for<'a> FnMut(&'a RuleEntry),
    {
        if let Some(id) = element.id() {
            if let Some(&range) = self.id_index.get(id) {
                self.match_range_ctx(range, element, ctx, &mut f);
            }
        }

        element.each_class(|class| {
            if let Some(&range) = self.class_index.get(class) {
                self.match_range_ctx(range, element, ctx, &mut f);
            }
        });

        if let Some(&range) = self.type_index.get(element.local_name()) {
            self.match_range_ctx(range, element, ctx, &mut f);
        }

        self.match_range_ctx(self.universal_range, element, ctx, &mut f);
    }

    #[inline]
    fn match_range_ctx<E: Element, F: FnMut(&RuleEntry)>(
        &self,
        range: BucketRange,
        element: &E,
        ctx: &mut MatchingContext,
        f: &mut F,
    ) {
        let start = range.start as usize;
        let end = start + range.len as usize;
        for entry in &self.entries[start..end] {
            if matching::matches_in_context(&entry.selector, element, ctx) {
                f(entry);
            }
        }
    }

    pub fn len(&self) -> u32 {
        self.rule_count
    }

    pub fn is_empty(&self) -> bool {
        self.rule_count == 0
    }

    /// Total memory used by the flat entry storage (bytes).
    pub fn entry_memory(&self) -> usize {
        self.entries.len() * std::mem::size_of::<RuleEntry>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::pseudo_class::ElementState;

    #[derive(Clone)]
    struct MockEl {
        tag: Atom,
        id: Option<Atom>,
        classes: Vec<Atom>,
    }

    impl Element for MockEl {
        fn local_name(&self) -> &Atom { &self.tag }
        fn id(&self) -> Option<&Atom> { self.id.as_ref() }
        fn has_class(&self, class: &Atom) -> bool { self.classes.iter().any(|c| c == class) }
        fn each_class<F: FnMut(&Atom)>(&self, mut f: F) {
            for c in &self.classes { f(c); }
        }
        fn attr(&self, _: &Atom) -> Option<&str> { None }
        fn parent_element(&self) -> Option<Self> { None }
        fn prev_sibling_element(&self) -> Option<Self> { None }
        fn next_sibling_element(&self) -> Option<Self> { None }
        fn first_child_element(&self) -> Option<Self> { None }
        fn last_child_element(&self) -> Option<Self> { None }
        fn state(&self) -> ElementState { ElementState::empty() }
        fn is_root(&self) -> bool { false }
        fn is_empty(&self) -> bool { true }
        fn child_index(&self) -> u32 { 1 }
        fn child_count(&self) -> u32 { 1 }
        fn child_index_of_type(&self) -> u32 { 1 }
        fn child_count_of_type(&self) -> u32 { 1 }
        fn opaque(&self) -> crate::opaque::OpaqueElement {
            crate::opaque::OpaqueElement::from_ptr(self as *const Self)
        }
    }

    fn el(tag: &str) -> MockEl {
        MockEl { tag: Atom::from(tag), id: None, classes: Vec::new() }
    }

    fn el_id(tag: &str, id: &str) -> MockEl {
        MockEl { tag: Atom::from(tag), id: Some(Atom::from(id)), classes: Vec::new() }
    }

    fn el_class(tag: &str, class: &str) -> MockEl {
        MockEl { tag: Atom::from(tag), id: None, classes: vec![Atom::from(class)] }
    }

    fn build_map(rules: &[(&str, u32)]) -> RuleMap {
        let mut builder = RuleMapBuilder::new();
        for &(css, data) in rules {
            let list = parser::parse(css).unwrap();
            for sel in list.0 {
                builder.insert(sel, data);
            }
        }
        builder.build()
    }

    #[test]
    fn bucketing_by_id() {
        let map = build_map(&[("#main", 0), ("#sidebar", 1), (".foo", 2)]);
        let matches = map.find_matching(&el_id("div", "main"));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].data, 0);
    }

    #[test]
    fn bucketing_by_class() {
        let map = build_map(&[(".active", 0), (".hidden", 1), ("div", 2)]);
        let matches = map.find_matching(&el_class("span", "active"));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].data, 0);
    }

    #[test]
    fn bucketing_by_type() {
        let map = build_map(&[("div", 0), ("span", 1)]);
        let matches = map.find_matching(&el("div"));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].data, 0);
    }

    #[test]
    fn universal_always_tested() {
        let map = build_map(&[("*", 0), ("div", 1)]);
        let matches = map.find_matching(&el("div"));
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn sorted_by_specificity() {
        let map = build_map(&[("div", 0), ("div.foo", 1), ("div#bar.foo", 2)]);
        let element = MockEl {
            tag: Atom::from("div"),
            id: Some(Atom::from("bar")),
            classes: vec![Atom::from("foo")],
        };
        let matches = map.find_matching(&element);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].data, 2); // #bar.foo — highest specificity
        assert_eq!(matches[1].data, 1); // .foo
        assert_eq!(matches[2].data, 0); // div
    }

    #[test]
    fn source_order_tiebreak() {
        let map = build_map(&[(".a", 0), (".a", 1)]);
        let matches = map.find_matching(&el_class("div", "a"));
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].data, 1); // later source order wins
        assert_eq!(matches[1].data, 0);
    }

    #[test]
    fn multiple_classes_hit_multiple_buckets() {
        let map = build_map(&[(".a", 0), (".b", 1), (".c", 2)]);
        let element = MockEl {
            tag: Atom::from("div"),
            id: None,
            classes: vec![Atom::from("a"), Atom::from("b")],
        };
        let matches = map.find_matching(&element);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn empty_map() {
        let map = build_map(&[]);
        assert!(map.is_empty());
        assert_eq!(map.find_matching(&el("div")).len(), 0);
    }

    #[test]
    fn visitor_zero_alloc() {
        let map = build_map(&[("div", 0), (".foo", 1), ("*", 2)]);
        let el = el_class("div", "foo");
        let mut count = 0u32;
        map.for_each_matching(&el, |_| count += 1);
        assert_eq!(count, 3);
    }

    #[test]
    fn flat_storage_contiguous() {
        let map = build_map(&[("#a", 0), (".b", 1), ("div", 2), ("*", 3)]);
        // All 4 entries should be in the flat storage.
        assert_eq!(map.len(), 4);
        assert_eq!(map.entries.len(), 4);
    }
}
