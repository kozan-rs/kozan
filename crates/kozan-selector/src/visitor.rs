//! Selector visitor — structural traversal of parsed selectors.
//!
//! The selector engine needs to traverse selector structures for purposes
//! beyond matching:
//! - **Invalidation**: Which atoms (classes, IDs, tags) appear in a selector?
//!   When those atoms change on an element, the selector *might* produce a
//!   different result → schedule a restyle.
//! - **Dependency tracking**: Which selectors reference which pseudo-classes?
//!   Needed for targeted invalidation on state changes.
//! - **Analysis**: Count nesting depth, detect expensive patterns, etc.
//!
//! **Stylo's approach**: `SelectorVisitor` trait with 5 methods and a
//! `SelectorListKind` bitflag. Deeply coupled to their SelectorImpl generics.
//!
//! **Our approach**: Same visitor pattern (it's the right abstraction), but
//! with concrete types (no generics), simpler method signatures, and
//! `SelectorListKind` used as context so visitors can distinguish
//! `:not(.foo)` from `:is(.foo)` without separate methods.

use kozan_atom::Atom;

use crate::types::*;

use bitflags::bitflags;

bitflags! {
    /// Where in the selector tree we currently are.
    ///
    /// Visitors can use this to distinguish context — for example, an
    /// invalidation visitor treats atoms inside `:not()` differently
    /// from atoms inside `:is()`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SelectorListKind: u8 {
        /// Inside `:not(...)`.
        const NEGATION = 1 << 0;
        /// Inside `:is(...)`.
        const IS = 1 << 1;
        /// Inside `:where(...)`.
        const WHERE = 1 << 2;
        /// Inside `:nth-child(... of <selector-list>)`.
        const NTH_OF = 1 << 3;
        /// Inside `:has(...)`.
        const HAS = 1 << 4;
    }
}

/// Visitor for traversing the structure of parsed selectors.
///
/// All methods return `bool` — returning `false` stops traversal early
/// (useful for "does this selector contain X?" queries).
///
/// Default implementations return `true` (continue traversal).
pub trait SelectorVisitor {
    /// Visit an attribute selector component.
    ///
    /// Called for `[attr]`, `[attr=val]`, etc.
    fn visit_attribute_selector(&mut self, _name: &Atom) -> bool {
        true
    }

    /// Visit a simple selector component.
    ///
    /// Called for every non-combinator component: Type, Class, Id,
    /// PseudoClass, PseudoElement, Attribute, etc.
    fn visit_simple_selector(&mut self, _component: &Component) -> bool {
        true
    }

    /// Visit a combinator between compound selectors.
    ///
    /// Called with the combinator that appears to the RIGHT of the
    /// current compound selector (in the original left-to-right CSS).
    /// `None` means we're in the rightmost compound (key selector).
    fn visit_combinator(&mut self, _combinator: Option<Combinator>) -> bool {
        true
    }

    /// Visit a nested selector list.
    ///
    /// Called before recursing into `:not()`, `:is()`, `:where()`,
    /// `:nth-child(... of)`, or `:has()`. The `kind` indicates which.
    /// Return `false` to skip recursion into this list.
    fn visit_selector_list(&mut self, _kind: SelectorListKind, _list: &SelectorList) -> bool {
        true
    }
}

/// Walk a selector, calling visitor methods for each component.
///
/// Components are visited in storage order (right-to-left). The visitor
/// can stop early by returning `false` from any method.
pub fn visit_selector(selector: &Selector, visitor: &mut impl SelectorVisitor) -> bool {
    // Track which combinator is to the right of the current compound.
    // For the first (rightmost) compound, it's None.
    let mut combinator_to_right: Option<Combinator> = None;

    for component in selector.components() {
        match component {
            Component::Combinator(c) => {
                if !visitor.visit_combinator(combinator_to_right) {
                    return false;
                }
                combinator_to_right = Some(*c);
            }
            _ => {
                if !visit_component(component, visitor) {
                    return false;
                }
            }
        }
    }

    // Visit the combinator context for the leftmost compound.
    visitor.visit_combinator(combinator_to_right)
}

/// Walk a selector list, visiting all selectors.
pub fn visit_selector_list(
    list: &SelectorList,
    visitor: &mut impl SelectorVisitor,
) -> bool {
    for selector in &list.0 {
        if !visit_selector(selector, visitor) {
            return false;
        }
    }
    true
}

/// Synthesize a SelectorList from flattened single-component sub-selectors.
/// Used by the visitor to present IsSingle/NotSingle/WhereSingle in the same
/// shape as their Arc-based counterparts, so invalidation analysis works.
fn comps_to_selector_list(comps: &[Component]) -> SelectorList {
    use crate::specificity::Specificity;
    use smallvec::SmallVec;

    let selectors: SmallVec<[Selector; 4]> = comps.iter().map(|c| {
        let mut sv: SmallVec<[Component; 8]> = SmallVec::new();
        sv.push(c.clone());
        Selector::from_parse_order_sub(sv, Specificity::ZERO)
    }).collect();
    SelectorList(selectors)
}

/// Visit a single component and recurse into nested lists.
fn visit_component(component: &Component, visitor: &mut impl SelectorVisitor) -> bool {
    if !visitor.visit_simple_selector(component) {
        return false;
    }

    match component {
        Component::Attribute(attr) => {
            visitor.visit_attribute_selector(&attr.name)
        }
        Component::Negation(list) => {
            if visitor.visit_selector_list(SelectorListKind::NEGATION, list) {
                visit_selector_list(list, visitor)
            } else {
                true
            }
        }
        Component::Is(list) => {
            if visitor.visit_selector_list(SelectorListKind::IS, list) {
                visit_selector_list(list, visitor)
            } else {
                true
            }
        }
        Component::Where(list) => {
            if visitor.visit_selector_list(SelectorListKind::WHERE, list) {
                visit_selector_list(list, visitor)
            } else {
                true
            }
        }
        // Flattened variants — synthesize a SelectorList for the visitor
        // so invalidation and dependency analysis still see these.
        Component::IsSingle(comps) => {
            let list = comps_to_selector_list(&comps.slice);
            if visitor.visit_selector_list(SelectorListKind::IS, &list) {
                visit_selector_list(&list, visitor)
            } else {
                true
            }
        }
        Component::NotSingle(comps) => {
            let list = comps_to_selector_list(&comps.slice);
            if visitor.visit_selector_list(SelectorListKind::NEGATION, &list) {
                visit_selector_list(&list, visitor)
            } else {
                true
            }
        }
        Component::WhereSingle(comps) => {
            let list = comps_to_selector_list(&comps.slice);
            if visitor.visit_selector_list(SelectorListKind::WHERE, &list) {
                visit_selector_list(&list, visitor)
            } else {
                true
            }
        }
        Component::Has(rel_list) => {
            let dummy_list = SelectorList(
                rel_list.0.iter().map(|r| r.selector.clone()).collect(),
            );
            if visitor.visit_selector_list(SelectorListKind::HAS, &dummy_list) {
                visit_selector_list(&dummy_list, visitor)
            } else {
                true
            }
        }
        Component::NthChild(nth) | Component::NthLastChild(nth) => {
            if let Some(ref of_sel) = nth.of_selector {
                if visitor.visit_selector_list(SelectorListKind::NTH_OF, of_sel) {
                    visit_selector_list(of_sel, visitor)
                } else {
                    true
                }
            } else {
                true
            }
        }
        Component::HostFunction(list) | Component::HostContext(list) | Component::Slotted(list) => {
            visit_selector_list(list, visitor)
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AtomCollector {
        ids: Vec<Atom>,
        classes: Vec<Atom>,
        types: Vec<Atom>,
    }

    impl AtomCollector {
        fn new() -> Self {
            Self {
                ids: Vec::new(),
                classes: Vec::new(),
                types: Vec::new(),
            }
        }
    }

    impl SelectorVisitor for AtomCollector {
        fn visit_simple_selector(&mut self, component: &Component) -> bool {
            match component {
                Component::Id(atom) => self.ids.push(atom.clone()),
                Component::Class(atom) => self.classes.push(atom.clone()),
                Component::Type(atom) => self.types.push(atom.clone()),
                _ => {}
            }
            true
        }
    }

    #[test]
    fn collect_atoms_simple() {
        let sel = crate::parser::parse("div.foo#bar").unwrap();
        let mut collector = AtomCollector::new();
        visit_selector(&sel.0[0], &mut collector);
        assert_eq!(collector.ids.len(), 1);
        assert_eq!(collector.classes.len(), 1);
        assert_eq!(collector.types.len(), 1);
    }

    #[test]
    fn collect_atoms_nested() {
        let sel = crate::parser::parse(":not(.hidden)").unwrap();
        let mut collector = AtomCollector::new();
        visit_selector(&sel.0[0], &mut collector);
        assert_eq!(collector.classes.len(), 1);
        assert_eq!(collector.classes[0].as_ref(), "hidden");
    }

    #[test]
    fn early_termination() {
        struct StopAtId;
        impl SelectorVisitor for StopAtId {
            fn visit_simple_selector(&mut self, component: &Component) -> bool {
                !matches!(component, Component::Id(_))
            }
        }

        let sel = crate::parser::parse("div#foo.bar").unwrap();
        let mut visitor = StopAtId;
        // Should stop at #foo and not visit .bar
        let completed = visit_selector(&sel.0[0], &mut visitor);
        assert!(!completed);
    }

    struct ListKindCollector {
        kinds: Vec<SelectorListKind>,
    }

    impl SelectorVisitor for ListKindCollector {
        fn visit_selector_list(&mut self, kind: SelectorListKind, _: &SelectorList) -> bool {
            self.kinds.push(kind);
            true
        }
    }

    #[test]
    fn nested_list_kinds() {
        // Use mixed :is() (class+type) so it doesn't get flattened, and single :not()
        let sel = crate::parser::parse(":is(.a, div):not(.b)").unwrap();
        let mut collector = ListKindCollector { kinds: Vec::new() };
        visit_selector(&sel.0[0], &mut collector);
        assert!(collector.kinds.contains(&SelectorListKind::IS));
        assert!(collector.kinds.contains(&SelectorListKind::NEGATION));
    }
}
