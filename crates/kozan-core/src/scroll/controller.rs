//! Scroll controller — applies scroll deltas along the chain.
//!
//! Chrome: `cc/input/scroll_controller.h`.
//! The ONLY code that mutates [`ScrollOffsets`].

use kozan_primitives::geometry::Offset;

use super::node::ScrollNode;
use super::offsets::ScrollOffsets;
use super::tree::ScrollTree;

/// DOM node IDs that consumed scroll delta in one `scroll()` call.
/// Typically 1 (inner container consumed all) or 2 (bubbled to parent).
pub(crate) struct ScrolledNodes {
    buf: [u32; 4],
    len: u8,
}

impl ScrolledNodes {
    fn new() -> Self {
        Self {
            buf: [0; 4],
            len: 0,
        }
    }

    fn push(&mut self, id: u32) {
        if (self.len as usize) < self.buf.len() {
            self.buf[self.len as usize] = id;
            self.len += 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.buf[..self.len as usize].iter().copied()
    }
}

/// Applies scroll deltas by walking the scroll chain with clamping.
///
/// Chrome: `ScrollController` — owns references to tree + offsets,
/// methods take only the per-event data (start node, delta).
pub(crate) struct ScrollController<'a> {
    tree: &'a ScrollTree,
    offsets: &'a mut ScrollOffsets,
}

impl<'a> ScrollController<'a> {
    pub fn new(tree: &'a ScrollTree, offsets: &'a mut ScrollOffsets) -> Self {
        Self { tree, offsets }
    }

    /// Distribute delta along the scroll chain from `start` toward root.
    /// Returns the DOM node IDs that actually consumed scroll delta
    /// (empty if nothing scrolled).
    pub fn scroll(&mut self, start: u32, mut delta: Offset) -> ScrolledNodes {
        let mut nodes = ScrolledNodes::new();

        for node_id in self.tree.chain(start) {
            if delta.dx == 0.0 && delta.dy == 0.0 {
                break;
            }
            let Some(node) = self.tree.get(node_id) else {
                break;
            };
            let consumed = self.apply_delta(node, delta);

            if consumed.dx != 0.0 || consumed.dy != 0.0 {
                nodes.push(node_id);
            }
            delta = Offset::new(delta.dx - consumed.dx, delta.dy - consumed.dy);
        }

        nodes
    }

    /// Clamp and apply delta to one node. Returns consumed portion.
    fn apply_delta(&mut self, node: &ScrollNode, delta: Offset) -> Offset {
        let old = self.offsets.offset(node.dom_id);

        let new_dx = if node.can_scroll_x() {
            (old.dx + delta.dx).clamp(0.0, node.max_offset_x())
        } else {
            old.dx
        };
        let new_dy = if node.can_scroll_y() {
            (old.dy + delta.dy).clamp(0.0, node.max_offset_y())
        } else {
            old.dy
        };

        self.offsets
            .set_offset(node.dom_id, Offset::new(new_dx, new_dy));
        Offset::new(new_dx - old.dx, new_dy - old.dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_primitives::geometry::Size;

    fn build_chain() -> (ScrollTree, ScrollOffsets) {
        let mut tree = ScrollTree::new();
        let mut offsets = ScrollOffsets::new();

        tree.set(
            1,
            ScrollNode {
                dom_id: 1,
                parent: None,
                container: Size::new(800.0, 600.0),
                content: Size::new(800.0, 2000.0),
                scrollable_x: false,
                scrollable_y: true,
            },
        );
        tree.set(
            5,
            ScrollNode {
                dom_id: 5,
                parent: Some(1),
                container: Size::new(300.0, 200.0),
                content: Size::new(300.0, 800.0),
                scrollable_x: false,
                scrollable_y: true,
            },
        );

        offsets.set_offset(1, Offset::ZERO);
        offsets.set_offset(5, Offset::ZERO);
        (tree, offsets)
    }

    #[test]
    fn inner_node_consumes_delta() {
        let (tree, mut offsets) = build_chain();
        let result = ScrollController::new(&tree, &mut offsets).scroll(5, Offset::new(0.0, 100.0));

        assert!(!result.is_empty());
        assert_eq!(offsets.offset(5).dy, 100.0);
        assert_eq!(offsets.offset(1).dy, 0.0);
    }

    #[test]
    fn overflow_bubbles_to_parent() {
        let (tree, mut offsets) = build_chain();
        let result = ScrollController::new(&tree, &mut offsets).scroll(5, Offset::new(0.0, 700.0));

        assert_eq!(result.iter().collect::<Vec<_>>(), vec![5, 1]);
        assert_eq!(offsets.offset(5).dy, 600.0);
        assert_eq!(offsets.offset(1).dy, 100.0);
    }

    #[test]
    fn clamps_at_zero() {
        let (tree, mut offsets) = build_chain();
        let result = ScrollController::new(&tree, &mut offsets).scroll(5, Offset::new(0.0, -50.0));
        assert!(result.is_empty());
        assert_eq!(offsets.offset(5).dy, 0.0);
    }

    #[test]
    fn clamps_at_max() {
        let (tree, mut offsets) = build_chain();
        ScrollController::new(&tree, &mut offsets).scroll(5, Offset::new(0.0, 9999.0));
        assert_eq!(offsets.offset(5).dy, 600.0);
        assert_eq!(offsets.offset(1).dy, 1400.0);
    }

    #[test]
    fn disabled_axis_passes_through() {
        let (tree, mut offsets) = build_chain();
        let result = ScrollController::new(&tree, &mut offsets).scroll(5, Offset::new(100.0, 0.0));
        assert!(result.is_empty());
    }

    #[test]
    fn zero_delta_is_noop() {
        let (tree, mut offsets) = build_chain();
        let result = ScrollController::new(&tree, &mut offsets).scroll(5, Offset::ZERO);
        assert!(result.is_empty());
    }
}
