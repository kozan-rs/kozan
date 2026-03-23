//! Per-node layout data — stored in Document's parallel storage.
//!
//! The DOM IS the layout tree. Each node stores its own Taffy style, cache,
//! and layout results. No separate `LayoutTree` needed.
//!
//! Chrome equivalent: `LayoutObject`'s per-node layout state, but stored
//! as a parallel column rather than a separate tree.
//!
//! # Lifecycle
//!
//! - Created in `Document::alloc_node()` with defaults.
//! - Style synced from `ComputedValues` → `taffy::Style` after restyle.
//! - Cache cleared on style/content changes, propagated to ancestors.
//! - Cleared in `Document::destroy_node()`.

use taffy::tree::{Cache, Layout};
use style::Atom;

/// Per-node layout data stored directly on the DOM node.
///
/// This replaces the separate `LayoutTree` + `TaffyLayoutData` with data
/// co-located on each DOM node. Taffy's trait implementations read/write
/// this data directly through Document.
///
/// Chrome equivalent: `LayoutObject`'s inline style/cache/geometry fields.
#[non_exhaustive]
pub struct LayoutNodeData {
    /// Taffy style — converted from Stylo's `ComputedValues`.
    ///
    /// Updated when `RestyleDamage` indicates layout-affecting changes.
    /// Conversion: `ComputedValues` → `taffy::Style` via `computed_to_taffy_item_style()`.
    pub(crate) style: taffy::Style<Atom>,

    /// Taffy's layout cache — persistent across layout passes.
    ///
    /// Cleared when: style changes, content changes, or ancestor propagation.
    /// Taffy's `compute_cached_layout()` checks this before recomputing.
    pub(crate) cache: Cache,

    /// Raw layout output from Taffy (sub-pixel positions).
    ///
    /// Written by `set_unrounded_layout()` during Taffy's tree traversal.
    pub(crate) unrounded_layout: Layout,

    /// Layout children — may differ from DOM children.
    ///
    /// Reasons for divergence:
    /// - `display: none` nodes are excluded
    /// - Anonymous blocks inserted for inline/block mixing
    /// - `display: contents` promotes children to grandparent
    ///
    /// `None` = not yet constructed (needs rebuild).
    /// `Some(vec)` = constructed (may be empty for leaf nodes).
    pub(crate) layout_children: Option<Vec<u32>>,

    /// Layout parent — may differ from DOM parent.
    ///
    /// Set during layout tree construction. Used for cache invalidation
    /// propagation (clear ancestor caches when a child changes).
    pub(crate) layout_parent: Option<u32>,
}

impl LayoutNodeData {
    /// Create default layout data for a new node.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            style: taffy::Style::<Atom>::default(),
            cache: Cache::new(),
            unrounded_layout: Layout::new(),
            layout_children: None,
            layout_parent: None,
        }
    }

    /// Clear the Taffy cache, forcing re-layout on next pass.
    #[inline]
    pub fn clear_cache(&mut self) {
        self.cache = Cache::new();
    }

    /// Returns the layout parent index for ancestor propagation.
    #[inline]
    #[must_use]
    pub fn layout_parent(&self) -> Option<u32> {
        self.layout_parent
    }

    /// Mark layout children as needing reconstruction.
    #[inline]
    pub fn invalidate_layout_children(&mut self) {
        self.layout_children = None;
    }

    /// Whether layout children have been constructed.
    #[inline]
    #[must_use] 
    pub fn has_layout_children(&self) -> bool {
        self.layout_children.is_some()
    }
}

impl Default for LayoutNodeData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_default_values() {
        let data = LayoutNodeData::new();
        assert!(data.layout_children.is_none());
        assert!(data.layout_parent.is_none());
        assert_eq!(data.unrounded_layout.size.width, 0.0);
        assert_eq!(data.unrounded_layout.size.height, 0.0);
    }

    #[test]
    fn default_matches_new() {
        let a = LayoutNodeData::new();
        let b = LayoutNodeData::default();
        assert_eq!(a.unrounded_layout.size.width, b.unrounded_layout.size.width);
        assert_eq!(a.layout_children.is_none(), b.layout_children.is_none());
    }

    #[test]
    fn clear_cache_resets() {
        let mut data = LayoutNodeData::new();
        // After clear, cache should have no entries.
        data.clear_cache();
        let result = data.cache.get(
            taffy::Size { width: None, height: None },
            taffy::Size {
                width: taffy::AvailableSpace::MaxContent,
                height: taffy::AvailableSpace::MaxContent,
            },
            taffy::tree::RunMode::PerformLayout,
        );
        assert!(result.is_none());
    }

    #[test]
    fn layout_parent_returns_parent() {
        let mut data = LayoutNodeData::new();
        data.layout_parent = Some(5);
        assert_eq!(data.layout_parent(), Some(5));
    }

    #[test]
    fn layout_parent_returns_none_when_root() {
        let data = LayoutNodeData::new();
        assert_eq!(data.layout_parent(), None);
    }

    #[test]
    fn invalidate_layout_children_clears() {
        let mut data = LayoutNodeData::new();
        data.layout_children = Some(vec![1, 2, 3]);
        assert!(data.has_layout_children());

        data.invalidate_layout_children();
        assert!(!data.has_layout_children());
        assert!(data.layout_children.is_none());
    }

    #[test]
    fn empty_children_is_constructed() {
        let mut data = LayoutNodeData::new();
        data.layout_children = Some(Vec::new());
        assert!(data.has_layout_children());
    }

    #[test]
    fn style_starts_as_default() {
        let data = LayoutNodeData::new();
        assert_eq!(data.style.display, taffy::Display::default());
    }

    #[test]
    fn layout_starts_at_zero() {
        let data = LayoutNodeData::new();
        assert_eq!(data.unrounded_layout.location.x, 0.0);
        assert_eq!(data.unrounded_layout.location.y, 0.0);
        assert_eq!(data.unrounded_layout.size.width, 0.0);
        assert_eq!(data.unrounded_layout.size.height, 0.0);
    }
}
