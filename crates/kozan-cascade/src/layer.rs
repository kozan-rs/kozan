//! Cascade layer ordering — maps `@layer` names to integer priority.
//!
//! Layers are ordered by first encounter in source order. The first `@layer`
//! declaration (whether block or statement) reserves the position.
//!
//! Unlayered rules use `UNLAYERED` (= `u16::MAX`), which naturally sorts
//! highest in normal cascade (unlayered beats all layers). For `!important`,
//! `CascadeLevel` inverts the layer bits, so `u16::MAX` becomes 0 — unlayered
//! important has lowest layer priority, matching the CSS spec.

use kozan_atom::Atom;
use kozan_selector::fxhash::FxHashMap;
use smallvec::SmallVec;

/// Dotted layer path used as hash key: `framework.utilities` → `[framework, utilities]`.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct LayerKey(SmallVec<[Atom; 2]>);

impl LayerKey {
    fn from_name(name: &kozan_css::LayerName) -> Self {
        Self(name.0.clone())
    }
}

/// Maps cascade layer names to integer order values.
///
/// Order values are assigned by first encounter in source order.
/// Anonymous layers get unique auto-incremented values.
pub struct LayerOrderMap {
    named: FxHashMap<LayerKey, u16>,
    next_order: u16,
}

/// Layer order for unlayered (non-`@layer`) rules.
///
/// `u16::MAX` — sorts highest in normal cascade (unlayered wins over all layers).
/// In `!important`, `CascadeLevel` inverts to 0 (unlayered loses to all layers).
pub const UNLAYERED: u16 = u16::MAX;

impl LayerOrderMap {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            named: FxHashMap::default(),
            next_order: 0,
        }
    }

    /// Get or assign order for a named layer. First encounter wins the position.
    pub fn get_or_insert(&mut self, name: &kozan_css::LayerName) -> u16 {
        let key = LayerKey::from_name(name);
        if let Some(&order) = self.named.get(&key) {
            return order;
        }
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        self.named.insert(key, order);
        order
    }

    /// Allocate order for an anonymous layer (always new, always next).
    pub fn next_anonymous(&mut self) -> u16 {
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        order
    }

    /// Number of named layers registered.
    #[must_use] 
    pub fn len(&self) -> usize {
        self.named.len()
    }

    /// Whether no named layers have been registered.
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }

    /// Clear all layer assignments.
    pub fn clear(&mut self) {
        self.named.clear();
        self.next_order = 0;
    }
}

impl Default for LayerOrderMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_css::LayerName;

    fn name(parts: &[&str]) -> LayerName {
        LayerName(parts.iter().map(|s| Atom::from(*s)).collect())
    }

    #[test]
    fn first_encounter_wins() {
        let mut map = LayerOrderMap::new();
        let order_a = map.get_or_insert(&name(&["base"]));
        let order_b = map.get_or_insert(&name(&["utils"]));
        let order_a2 = map.get_or_insert(&name(&["base"]));
        assert_eq!(order_a, 0);
        assert_eq!(order_b, 1);
        assert_eq!(order_a2, 0); // same as first encounter
    }

    #[test]
    fn dotted_names() {
        let mut map = LayerOrderMap::new();
        let order = map.get_or_insert(&name(&["framework", "utilities"]));
        let order2 = map.get_or_insert(&name(&["framework", "base"]));
        assert_eq!(order, 0);
        assert_eq!(order2, 1);
        // Same dotted name returns same order
        assert_eq!(map.get_or_insert(&name(&["framework", "utilities"])), 0);
    }

    #[test]
    fn anonymous_always_new() {
        let mut map = LayerOrderMap::new();
        let a = map.next_anonymous();
        let b = map.next_anonymous();
        assert_ne!(a, b);
    }

    #[test]
    fn named_and_anonymous_interleave() {
        let mut map = LayerOrderMap::new();
        let named = map.get_or_insert(&name(&["base"]));
        let anon = map.next_anonymous();
        let named2 = map.get_or_insert(&name(&["utils"]));
        assert_eq!(named, 0);
        assert_eq!(anon, 1);
        assert_eq!(named2, 2);
    }

    #[test]
    fn unlayered_is_max() {
        assert_eq!(UNLAYERED, u16::MAX);
    }
}
