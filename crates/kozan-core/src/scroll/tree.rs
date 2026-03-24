//! Scroll tree — topology of scrollable nodes.
//!
//! Chrome: `cc/trees/scroll_tree.h`.
//! Knows parent-child scroll chain. No offsets, no events, no paint.

use kozan_primitives::arena::Storage;
use kozan_primitives::geometry::Size;

use crate::layout::fragment::{Fragment, FragmentKind};

use super::node::ScrollNode;

/// Directed graph of scrollable containers, keyed by DOM node index.
///
/// The parent chain defines how unconsumed scroll delta bubbles up.
/// Rebuilt after each layout pass from the fragment tree.
#[derive(Clone)]
pub struct ScrollTree {
    nodes: Storage<ScrollNode>,
    root: Option<u32>,
}

impl ScrollTree {
    pub fn new() -> Self {
        Self {
            nodes: Storage::new(),
            root: None,
        }
    }

    pub fn set(&mut self, dom_id: u32, node: ScrollNode) {
        if node.parent.is_none() {
            self.root = Some(dom_id);
        }
        self.nodes.set(dom_id, node);
    }

    pub fn get(&self, dom_id: u32) -> Option<&ScrollNode> {
        self.nodes.get(dom_id)
    }

    /// Remove all nodes. Called before re-syncing from a new fragment tree.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
    }

    pub fn contains(&self, dom_id: u32) -> bool {
        self.nodes.get(dom_id).is_some()
    }

    /// The root scroller — cached during `set()` / `sync()`. O(1).
    pub fn root_scroller(&self) -> Option<u32> {
        self.root
    }

    /// Iterate the scroll chain from `start` toward the root scroller.
    pub fn chain(&self, start: u32) -> ScrollChain<'_> {
        ScrollChain {
            nodes: &self.nodes,
            current: Some(start),
        }
    }

    /// Rebuild from the fragment tree. Finds all scrollable boxes and
    /// builds the parent-child scroll chain.
    ///
    /// Chrome: `ScrollTree::UpdateScrollTree()` — runs after layout.
    pub fn sync(&mut self, root: &Fragment) {
        self.clear();
        self.sync_recursive(root, None);
    }

    fn sync_recursive(&mut self, fragment: &Fragment, parent_scroll: Option<u32>) {
        let FragmentKind::Box(ref data) = fragment.kind else {
            return;
        };

        let dom_id = fragment.dom_node;
        let scrollable_x = data.overflow_x.is_user_scrollable();
        let scrollable_y = data.overflow_y.is_user_scrollable();
        let is_scrollable = scrollable_x || scrollable_y;

        // Container = padding box (fragment size minus borders).
        let container = Size::new(
            (fragment.size.width - data.border.left - data.border.right).max(0.0),
            (fragment.size.height - data.border.top - data.border.bottom).max(0.0),
        );

        let next_parent = if is_scrollable {
            if let Some(id) = dom_id {
                // scrollable_overflow is relative to the border box origin,
                // but max_offset = content - container uses the padding box.
                // Shift to padding-box-relative so the math is consistent.
                let content = Size::new(
                    (data.scrollable_overflow.width - data.border.left).max(0.0),
                    (data.scrollable_overflow.height - data.border.top).max(0.0),
                );
                self.set(
                    id,
                    ScrollNode {
                        dom_id: id,
                        parent: parent_scroll,
                        container,
                        content,
                        scrollable_x,
                        scrollable_y,
                    },
                );
                Some(id)
            } else {
                parent_scroll
            }
        } else {
            parent_scroll
        };

        for child in &data.children {
            self.sync_recursive(&child.fragment, next_parent);
        }
    }
}

/// Iterator over the ancestor scroll chain.
pub struct ScrollChain<'a> {
    nodes: &'a Storage<ScrollNode>,
    current: Option<u32>,
}

impl Iterator for ScrollChain<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        let id = self.current?;
        let node = self.nodes.get(id)?;
        self.current = node.parent;
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_primitives::geometry::Size;

    fn scrollable(id: u32, parent: Option<u32>) -> ScrollNode {
        ScrollNode {
            dom_id: id,
            parent,
            container: Size::new(200.0, 400.0),
            content: Size::new(200.0, 1200.0),
            scrollable_x: false,
            scrollable_y: true,
        }
    }

    #[test]
    fn chain_walks_to_root() {
        let mut tree = ScrollTree::new();
        tree.set(1, scrollable(1, None));
        tree.set(5, scrollable(5, Some(1)));
        tree.set(9, scrollable(9, Some(5)));

        let chain: Vec<u32> = tree.chain(9).collect();
        assert_eq!(chain, vec![9, 5, 1]);
    }

    #[test]
    fn chain_single_node() {
        let mut tree = ScrollTree::new();
        tree.set(1, scrollable(1, None));

        let chain: Vec<u32> = tree.chain(1).collect();
        assert_eq!(chain, vec![1]);
    }

    #[test]
    fn root_scroller_returns_parentless_node() {
        let mut tree = ScrollTree::new();
        tree.set(1, scrollable(1, None));
        tree.set(5, scrollable(5, Some(1)));
        assert_eq!(tree.root_scroller(), Some(1));
    }

    #[test]
    fn root_scroller_empty_tree() {
        let tree = ScrollTree::new();
        assert_eq!(tree.root_scroller(), None);
    }

    #[test]
    fn chain_unknown_start_is_empty() {
        let tree = ScrollTree::new();
        let chain: Vec<u32> = tree.chain(99).collect();
        assert!(chain.is_empty());
    }
}
