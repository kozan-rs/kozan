//! Invalidation maps — atom-to-selector dependency tracking for targeted restyle.
//!
//! # The Problem
//!
//! When a DOM mutation occurs (class toggled, attribute changed, child added),
//! the restyle system must determine which elements need re-matching. Without
//! dependency tracking, EVERY selector must be re-tested against EVERY affected
//! element — O(selectors × elements) work per mutation.
//!
//! # The Solution
//!
//! At stylesheet insertion time, we analyze each selector to extract which atoms
//! (class names, IDs, tag names, attribute names) it references. These are stored
//! in inverted indexes:
//!
//! ```text
//! class_map:  "active"  → [selector_3, selector_17, selector_42]
//!             "hidden"  → [selector_8, selector_23]
//! id_map:     "main"    → [selector_1]
//! type_map:   "div"     → [selector_5, selector_12]
//! attr_map:   "disabled" → [selector_7]
//! ```
//!
//! When class "active" toggles on an element, we look up `class_map["active"]`
//! and re-match only those 3 selectors — not all 500+ in the stylesheet.
//!
//! # What Gets Stored
//!
//! Each entry is a `InvalidationEntry` containing:
//! - The index of the selector in the stylesheet (for lookup)
//! - The specificity (for cascade ordering without re-parsing)
//!
//! # Comparison with Stylo
//!
//! Stylo's `InvalidationMap` is deeply coupled to their `SelectorImpl` generics
//! and stores full selector references with complex lifetime management. Our
//! approach stores lightweight indices and specificities, keeping the map
//! decoupled from selector storage.
//!
//! Stylo also has separate maps for "ID invalidation", "class invalidation",
//! and "other invalidation". We use the same split for O(1) lookup by mutation
//! type: class change → class_map, ID change → id_map, etc.
//!
//! # State-Based Invalidation
//!
//! State pseudo-classes (`:hover`, `:focus`, `:checked`, etc.) are tracked via
//! `ElementState` bitflags. The `state_deps` field accumulates all state flags
//! referenced by any selector. When an element's state changes, the restyle
//! system checks `state_deps & changed_state` — if zero, no selectors care
//! about that state change and the element can be skipped.
//!
//! # Spec Reference
//!
//! Not directly specified by CSS — this is an implementation optimization.
//! Blink calls it "InvalidationSet", Stylo calls it "InvalidationMap".

use kozan_atom::Atom;
use smallvec::SmallVec;

use crate::fxhash::FxHashMap;
use crate::pseudo_class::ElementState;
use crate::specificity::Specificity;
use crate::types::{Component, Selector, SelectorList};
use crate::visitor::{self, SelectorListKind, SelectorVisitor};

/// A reference to a selector in the stylesheet, stored in the invalidation map.
///
/// Lightweight — only the selector index and pre-computed specificity.
/// The actual selector can be looked up from the stylesheet by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidationEntry {
    /// Index of the selector in the stylesheet's selector list.
    pub selector_index: u32,
    /// Pre-computed specificity for cascade ordering.
    pub specificity: Specificity,
}

/// Inverted index from atoms to the selectors that reference them.
///
/// Enables O(1) lookup of "which selectors might be affected?" when a
/// specific class, ID, tag, or attribute changes on an element.
#[derive(Debug)]
pub struct InvalidationMap {
    /// Class name → selectors referencing that class.
    /// Queried when: element gains/loses a class.
    class_map: FxHashMap<Atom, SmallVec<[InvalidationEntry; 4]>>,

    /// ID → selectors referencing that ID.
    /// Queried when: element's ID changes.
    id_map: FxHashMap<Atom, SmallVec<[InvalidationEntry; 2]>>,

    /// Tag name → selectors referencing that tag.
    /// Queried when: element is inserted/removed (type-based selectors).
    type_map: FxHashMap<Atom, SmallVec<[InvalidationEntry; 4]>>,

    /// Attribute name → selectors with attribute selectors on that name.
    /// Queried when: an attribute is set/removed/changed.
    attr_map: FxHashMap<Atom, SmallVec<[InvalidationEntry; 2]>>,

    /// Union of all `ElementState` flags referenced by any selector.
    /// Quick reject: if `state_deps & changed_flags == 0`, no selector cares.
    state_deps: ElementState,

    /// Selectors that reference structural pseudo-classes (:first-child, :empty,
    /// :nth-child, etc.). These must be re-evaluated when children are added/removed.
    structural_entries: SmallVec<[InvalidationEntry; 8]>,

    /// Total number of selectors indexed.
    count: u32,
}

impl InvalidationMap {
    /// Create a new empty invalidation map.
    pub fn new() -> Self {
        Self {
            class_map: FxHashMap::default(),
            id_map: FxHashMap::default(),
            type_map: FxHashMap::default(),
            attr_map: FxHashMap::default(),
            state_deps: ElementState::empty(),
            structural_entries: SmallVec::new(),
            count: 0,
        }
    }

    /// Index a selector list (typically from a single CSS rule).
    ///
    /// Extracts all referenced atoms and state flags, adding entries to the
    /// appropriate maps. `base_index` is the offset of the first selector
    /// in the stylesheet's global selector numbering.
    pub fn add_selector_list(&mut self, list: &SelectorList, base_index: u32) {
        for (i, selector) in list.0.iter().enumerate() {
            let entry = InvalidationEntry {
                selector_index: base_index + i as u32,
                specificity: selector.specificity(),
            };
            self.add_selector(selector, entry);
            self.count += 1;
        }
    }

    /// Index a single selector.
    fn add_selector(&mut self, selector: &Selector, entry: InvalidationEntry) {
        let mut collector = AtomCollector {
            entry,
            map: self,
            has_structural: false,
        };
        visitor::visit_selector(selector, &mut collector);
        if collector.has_structural {
            self.structural_entries.push(entry);
        }
    }

    /// Look up selectors that reference the given class name.
    #[inline]
    pub fn class_deps(&self, class: &Atom) -> &[InvalidationEntry] {
        self.class_map.get(class).map_or(&[], |v| v.as_slice())
    }

    /// Look up selectors that reference the given ID.
    #[inline]
    pub fn id_deps(&self, id: &Atom) -> &[InvalidationEntry] {
        self.id_map.get(id).map_or(&[], |v| v.as_slice())
    }

    /// Look up selectors that reference the given tag name.
    #[inline]
    pub fn type_deps(&self, tag: &Atom) -> &[InvalidationEntry] {
        self.type_map.get(tag).map_or(&[], |v| v.as_slice())
    }

    /// Look up selectors that reference the given attribute name.
    #[inline]
    pub fn attr_deps(&self, attr: &Atom) -> &[InvalidationEntry] {
        self.attr_map.get(attr).map_or(&[], |v| v.as_slice())
    }

    /// Selectors that use structural pseudo-classes.
    #[inline]
    pub fn structural_deps(&self) -> &[InvalidationEntry] {
        &self.structural_entries
    }

    /// All state flags referenced by any selector.
    ///
    /// If `state_deps() & changed_state` is empty, no selector could possibly
    /// be affected by the state change — skip restyle entirely.
    #[inline]
    pub fn state_deps(&self) -> ElementState {
        self.state_deps
    }

    /// Whether any selector references the given state flags.
    #[inline]
    pub fn depends_on_state(&self, state: ElementState) -> bool {
        self.state_deps.intersects(state)
    }

    /// Total number of indexed selectors.
    #[inline]
    pub fn selector_count(&self) -> u32 {
        self.count
    }

    /// Number of unique class names tracked.
    pub fn class_count(&self) -> usize {
        self.class_map.len()
    }

    /// Number of unique IDs tracked.
    pub fn id_count(&self) -> usize {
        self.id_map.len()
    }

    /// Clear all entries. Call when stylesheets are replaced.
    pub fn clear(&mut self) {
        self.class_map.clear();
        self.id_map.clear();
        self.type_map.clear();
        self.attr_map.clear();
        self.state_deps = ElementState::empty();
        self.structural_entries.clear();
        self.count = 0;
    }
}

/// Visitor that extracts referenced atoms and state flags from a selector.
struct AtomCollector<'a> {
    entry: InvalidationEntry,
    map: &'a mut InvalidationMap,
    has_structural: bool,
}

impl SelectorVisitor for AtomCollector<'_> {
    fn visit_simple_selector(&mut self, component: &Component) -> bool {
        match component {
            Component::Class(atom) => {
                self.map
                    .class_map
                    .entry(atom.clone())
                    .or_default()
                    .push(self.entry);
            }
            Component::Id(atom) => {
                self.map
                    .id_map
                    .entry(atom.clone())
                    .or_default()
                    .push(self.entry);
            }
            Component::Type(atom) => {
                self.map
                    .type_map
                    .entry(atom.clone())
                    .or_default()
                    .push(self.entry);
            }
            Component::PseudoClass(pc) => {
                // Track state dependencies.
                if let Some(flag) = pc.state_flag() {
                    self.map.state_deps |= flag;
                }
                // Track structural pseudo-classes.
                if pc.is_structural() {
                    self.has_structural = true;
                }
            }
            // Flattened functional variants — the visitor synthesizes a
            // SelectorList and recurses into each sub-component, so each
            // Class/Type/Id is visited via the match arms above. No direct
            // extraction needed here.
            Component::IsSingle(_)
            | Component::WhereSingle(_)
            | Component::NotSingle(_) => {}
            // Functional structural pseudo-classes stored as Component variants.
            Component::NthChild(_)
            | Component::NthLastChild(_)
            | Component::NthOfType(_, _)
            | Component::NthLastOfType(_, _) => {
                self.has_structural = true;
            }
            _ => {}
        }
        true
    }

    fn visit_attribute_selector(&mut self, name: &Atom) -> bool {
        self.map
            .attr_map
            .entry(name.clone())
            .or_default()
            .push(self.entry);
        true
    }

    fn visit_selector_list(&mut self, _kind: SelectorListKind, _list: &SelectorList) -> bool {
        // Recurse into nested selector lists (:not, :is, :where, :has, nth-of).
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn make_map(rules: &[&str]) -> InvalidationMap {
        let mut map = InvalidationMap::new();
        for (i, css) in rules.iter().enumerate() {
            let list = parse(css).unwrap();
            map.add_selector_list(&list, i as u32);
        }
        map
    }

    #[test]
    fn class_dependency() {
        let map = make_map(&[".active", ".hidden", "div.active"]);
        let active = Atom::from("active");
        let hidden = Atom::from("hidden");

        assert_eq!(map.class_deps(&active).len(), 2); // .active, div.active
        assert_eq!(map.class_deps(&hidden).len(), 1); // .hidden
        assert_eq!(map.class_deps(&Atom::from("unknown")).len(), 0);
    }

    #[test]
    fn id_dependency() {
        let map = make_map(&["#main", "#sidebar", "div#main"]);
        let main = Atom::from("main");
        assert_eq!(map.id_deps(&main).len(), 2);
    }

    #[test]
    fn type_dependency() {
        let map = make_map(&["div", "span", "div.foo"]);
        let div = Atom::from("div");
        assert_eq!(map.type_deps(&div).len(), 2);
    }

    #[test]
    fn attr_dependency() {
        let map = make_map(&["[disabled]", "[type=text]", "[data-x^=foo]"]);
        let disabled = Atom::from("disabled");
        let typ = Atom::from("type");
        assert_eq!(map.attr_deps(&disabled).len(), 1);
        assert_eq!(map.attr_deps(&typ).len(), 1);
    }

    #[test]
    fn state_dependency() {
        let map = make_map(&[":hover", ":focus", ":first-child"]);
        assert!(map.depends_on_state(ElementState::HOVER));
        assert!(map.depends_on_state(ElementState::FOCUS));
        assert!(!map.depends_on_state(ElementState::ACTIVE));
    }

    #[test]
    fn structural_dependency() {
        let map = make_map(&[":first-child", ":nth-child(2n)", "div.foo"]);
        assert_eq!(map.structural_deps().len(), 2);
    }

    #[test]
    fn nested_selectors_tracked() {
        let map = make_map(&[":not(.hidden)", ":is(.a, .b)", ":has(> .child)"]);
        let hidden = Atom::from("hidden");
        let a = Atom::from("a");
        let b = Atom::from("b");
        let child = Atom::from("child");

        assert_eq!(map.class_deps(&hidden).len(), 1);
        assert_eq!(map.class_deps(&a).len(), 1);
        assert_eq!(map.class_deps(&b).len(), 1);
        assert_eq!(map.class_deps(&child).len(), 1);
    }

    #[test]
    fn clear_resets() {
        let mut map = make_map(&[".foo", "#bar", ":hover"]);
        assert!(map.selector_count() > 0);
        map.clear();
        assert_eq!(map.selector_count(), 0);
        assert_eq!(map.class_count(), 0);
        assert!(map.state_deps().is_empty());
    }

    #[test]
    fn specificity_preserved() {
        let map = make_map(&["#main .active"]);
        let active = Atom::from("active");
        let deps = map.class_deps(&active);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].specificity.components(), (1, 1, 0));
    }
}
