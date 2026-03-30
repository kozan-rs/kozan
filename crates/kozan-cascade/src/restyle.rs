//! Incremental restyle — tracks DOM mutations and computes minimal restyle hints.
//!
//! Instead of restyling the entire tree on every DOM change, the restyle system:
//! 1. Records mutations (class/id/attr/state/child changes)
//! 2. Queries the `InvalidationMap` to find which selectors are affected
//! 3. Marks only affected elements with `RestyleHint` flags
//! 4. The style resolver processes only marked elements
//!
//! This is the key to 60fps style recalculation on dynamic UIs.

use bitflags::bitflags;
use kozan_atom::Atom;
use kozan_selector::fxhash::FxHashMap;
use kozan_selector::invalidation::InvalidationMap;
use kozan_selector::opaque::OpaqueElement;
use kozan_selector::pseudo_class::ElementState;

bitflags! {
    /// What kind of restyle is needed for an element.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct RestyleHint: u8 {
        /// Re-run selector matching and cascade for this element.
        const RESTYLE_SELF         = 1 << 0;
        /// Restyle all descendants (class change on ancestor).
        const RESTYLE_DESCENDANTS  = 1 << 1;
        /// Re-run cascade only (matched rules didn't change, but parent did).
        const RECASCADE            = 1 << 2;
        /// Propagate inherited properties from parent.
        const INHERIT              = 1 << 3;
    }
}

/// Represents a single DOM mutation that may require restyling.
#[derive(Clone, Debug)]
pub enum DomMutation {
    /// A class was added or removed.
    ClassChange {
        element: OpaqueElement,
        class: Atom,
    },
    /// The element's ID changed.
    IdChange {
        element: OpaqueElement,
        old: Option<Atom>,
        new: Option<Atom>,
    },
    /// An attribute changed.
    AttrChange {
        element: OpaqueElement,
        attr: Atom,
    },
    /// Element state changed (hover, focus, active, etc.).
    StateChange {
        element: OpaqueElement,
        /// Which state bits actually changed (e.g., HOVER | FOCUS).
        /// Used to intersect with InvalidationMap::state_deps() so we
        /// only restyle when a selector actually depends on the changed state.
        changed: ElementState,
    },
    /// Children were added or removed (affects structural pseudo-classes).
    ChildChange {
        parent: OpaqueElement,
    },
    /// An inline `style` attribute changed.
    InlineStyleChange {
        element: OpaqueElement,
    },
}

/// Accumulates DOM mutations and computes restyle hints.
///
/// Used in a batch model: record mutations during event handling,
/// then call `compute_hints()` before the next frame to get the
/// minimal set of elements that need restyling.
pub struct RestyleTracker {
    dirty: FxHashMap<OpaqueElement, RestyleHint>,
    mutations: Vec<DomMutation>,
}

impl RestyleTracker {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            dirty: FxHashMap::default(),
            mutations: Vec::new(),
        }
    }

    /// Record a DOM mutation for later processing.
    pub fn push(&mut self, mutation: DomMutation) {
        self.mutations.push(mutation);
    }

    /// Process all recorded mutations against the invalidation map
    /// and compute restyle hints for affected elements.
    ///
    /// `ancestor_lookup`: given an element, returns ancestors (parent, grandparent, …).
    /// Used for `:has(.foo)` — when `.foo` changes, ancestors that `:has()` anchors restyle.
    ///
    /// `sibling_lookup`: given an element, returns its preceding siblings.
    /// Used for `:has(+ .foo)` / `:has(~ .foo)` — when `.foo` changes, the preceding
    /// sibling(s) that are `:has()` subjects need restyle.
    ///
    /// Pass `|_| vec![]` for either when that traversal direction is not needed.
    ///
    /// After this call, `dirty()` returns the set of elements that need
    /// restyling, and `mutations` is cleared.
    pub fn compute_hints(
        &mut self,
        invalidation: &InvalidationMap,
        ancestor_lookup: impl Fn(OpaqueElement) -> Vec<OpaqueElement>,
        sibling_lookup: impl Fn(OpaqueElement) -> Vec<OpaqueElement>,
    ) {
        let mutations = std::mem::take(&mut self.mutations);
        for mutation in &mutations {
            match mutation {
                DomMutation::ClassChange { element, class } => {
                    if !invalidation.class_deps(class).is_empty() {
                        self.mark(*element, RestyleHint::RESTYLE_SELF);
                    }
                    // :has(.foo) — ancestors of the changed element are the subjects.
                    if !invalidation.has_class_deps(class).is_empty() {
                        for ancestor in ancestor_lookup(*element) {
                            self.mark(ancestor, RestyleHint::RESTYLE_SELF);
                        }
                    }
                    // :has(+ .foo) / :has(~ .foo) — preceding siblings are the subjects.
                    if !invalidation.has_sibling_class_deps(class).is_empty() {
                        for sib in sibling_lookup(*element) {
                            self.mark(sib, RestyleHint::RESTYLE_SELF);
                        }
                    }
                }
                DomMutation::IdChange { element, old, new } => {
                    let affected = old
                        .as_ref()
                        .is_some_and(|id| !invalidation.id_deps(id).is_empty())
                        || new
                            .as_ref()
                            .is_some_and(|id| !invalidation.id_deps(id).is_empty());
                    if affected {
                        self.mark(*element, RestyleHint::RESTYLE_SELF);
                    }
                    // :has() ancestor invalidation for ID changes.
                    let has_affected = old
                        .as_ref()
                        .is_some_and(|id| !invalidation.has_id_deps(id).is_empty())
                        || new
                            .as_ref()
                            .is_some_and(|id| !invalidation.has_id_deps(id).is_empty());
                    if has_affected {
                        for ancestor in ancestor_lookup(*element) {
                            self.mark(ancestor, RestyleHint::RESTYLE_SELF);
                        }
                    }
                    // :has(+ #id) / :has(~ #id) — preceding siblings.
                    let has_sib_affected = old
                        .as_ref()
                        .is_some_and(|id| !invalidation.has_sibling_id_deps(id).is_empty())
                        || new
                            .as_ref()
                            .is_some_and(|id| !invalidation.has_sibling_id_deps(id).is_empty());
                    if has_sib_affected {
                        for sib in sibling_lookup(*element) {
                            self.mark(sib, RestyleHint::RESTYLE_SELF);
                        }
                    }
                }
                DomMutation::AttrChange { element, attr } => {
                    if !invalidation.attr_deps(attr).is_empty() {
                        self.mark(*element, RestyleHint::RESTYLE_SELF);
                    }
                    if !invalidation.has_attr_deps(attr).is_empty() {
                        for ancestor in ancestor_lookup(*element) {
                            self.mark(ancestor, RestyleHint::RESTYLE_SELF);
                        }
                    }
                    // :has(+ [attr]) / :has(~ [attr]) — preceding siblings.
                    if !invalidation.has_sibling_attr_deps(attr).is_empty() {
                        for sib in sibling_lookup(*element) {
                            self.mark(sib, RestyleHint::RESTYLE_SELF);
                        }
                    }
                }
                DomMutation::StateChange { element, changed } => {
                    if invalidation.state_deps().intersects(*changed) {
                        self.mark(*element, RestyleHint::RESTYLE_SELF);
                    }
                }
                DomMutation::ChildChange { parent } => {
                    if !invalidation.structural_deps().is_empty() {
                        self.mark(*parent, RestyleHint::RESTYLE_DESCENDANTS);
                    }
                    // :has() with structural pseudo-classes (descendant traversal):
                    // child add/remove may affect ancestor :has(:first-child) etc.
                    if invalidation.has_structural_deps() {
                        for ancestor in ancestor_lookup(*parent) {
                            self.mark(ancestor, RestyleHint::RESTYLE_SELF);
                        }
                    }
                    // :has(+ :first-child) — sibling-structural: preceding siblings restyle.
                    if invalidation.has_sibling_structural_deps() {
                        for sib in sibling_lookup(*parent) {
                            self.mark(sib, RestyleHint::RESTYLE_SELF);
                        }
                    }
                }
                DomMutation::InlineStyleChange { element } => {
                    self.mark(*element, RestyleHint::RECASCADE);
                }
            }
        }
    }

    /// Mark an element as needing restyle.
    fn mark(&mut self, element: OpaqueElement, hint: RestyleHint) {
        self.dirty
            .entry(element)
            .and_modify(|h| *h |= hint)
            .or_insert(hint);
    }

    /// The set of elements that need restyling after `compute_hints()`.
    #[must_use] 
    pub fn dirty(&self) -> &FxHashMap<OpaqueElement, RestyleHint> {
        &self.dirty
    }

    /// Whether any elements need restyling.
    #[must_use] 
    pub fn has_dirty(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Number of elements that need restyling.
    #[must_use] 
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }

    /// Clear all restyle hints (after processing).
    pub fn clear(&mut self) {
        self.dirty.clear();
        self.mutations.clear();
    }

    /// Number of pending mutations (not yet processed).
    #[must_use] 
    pub fn pending_mutations(&self) -> usize {
        self.mutations.len()
    }
}

impl Default for RestyleTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_selector::invalidation::InvalidationMap;

    fn element(id: u64) -> OpaqueElement {
        OpaqueElement::new(id)
    }

    #[test]
    fn empty_tracker() {
        let tracker = RestyleTracker::new();
        assert!(!tracker.has_dirty());
        assert_eq!(tracker.dirty_count(), 0);
    }

    #[test]
    fn inline_style_marks_recascade() {
        let mut tracker = RestyleTracker::new();
        let invalidation = InvalidationMap::new();

        tracker.push(DomMutation::InlineStyleChange {
            element: element(1),
        });
        tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);

        assert!(tracker.has_dirty());
        let hint = tracker.dirty()[&element(1)];
        assert!(hint.contains(RestyleHint::RECASCADE));
    }

    #[test]
    fn mutations_merged() {
        let mut tracker = RestyleTracker::new();
        let invalidation = InvalidationMap::new();

        tracker.push(DomMutation::InlineStyleChange {
            element: element(1),
        });
        // State change with no state deps — no restyle needed for state,
        // but inline style still marks RECASCADE.
        tracker.push(DomMutation::StateChange {
            element: element(1),
            changed: ElementState::empty(),
        });
        tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);

        assert_eq!(tracker.dirty_count(), 1);
    }

    #[test]
    fn clear_resets() {
        let mut tracker = RestyleTracker::new();
        let invalidation = InvalidationMap::new();

        tracker.push(DomMutation::InlineStyleChange {
            element: element(1),
        });
        tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
        assert!(tracker.has_dirty());

        tracker.clear();
        assert!(!tracker.has_dirty());
    }
}
