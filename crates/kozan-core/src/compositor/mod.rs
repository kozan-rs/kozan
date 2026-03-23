//! Compositor — GPU-thread owner of the layer tree and scroll state.
//!
//! Chrome: `cc::LayerTreeHostImpl` on the compositor thread.
//!
//! The view thread builds frames (DisplayList + LayerTree) and commits them.
//! The compositor owns the committed state and produces `CompositorFrame`
//! for the GPU each vsync. Scroll is handled here — no view thread round-trip.
//!
//! # Scroll ownership
//!
//! The compositor **exclusively owns** scroll offsets. The view thread never
//! sends scroll positions to the compositor. Instead:
//!
//! - Compositor → view: `ScrollSync(offsets)` after each scroll event
//! - View thread paints with those offsets (read-only copy)
//! - Commit sends topology (ScrollTree) but NOT positions (ScrollOffsets)
//!
//! This eliminates the stale-offset problem: no matter how many frames the
//! view thread is behind, the compositor's scroll state is always current.

pub mod frame;
pub(crate) mod layer;
pub(crate) mod layer_builder;
pub mod layer_tree;

use std::sync::Arc;

use kozan_primitives::geometry::{Offset, Point};

use crate::paint::DisplayList;
use crate::scroll::{ScrollController, ScrollOffsets, ScrollTree};

use self::frame::CompositorFrame;
use self::layer_tree::LayerTree;

/// Receives committed frames from the view thread and produces
/// `CompositorFrame` for the GPU.
///
/// Owns a copy of the scroll tree + offsets so it can handle wheel
/// events at vsync rate without waiting for the view thread.
pub struct Compositor {
    display_list: Option<Arc<DisplayList>>,
    layer_tree: Option<LayerTree>,
    scroll_tree: ScrollTree,
    scroll_offsets: ScrollOffsets,
}

impl Compositor {
    pub fn new() -> Self {
        Self {
            display_list: None,
            layer_tree: None,
            scroll_tree: ScrollTree::new(),
            scroll_offsets: ScrollOffsets::new(),
        }
    }

    /// Accept a committed frame from the view thread.
    ///
    /// Chrome: `LayerTreeHostImpl::FinishCommit()`.
    ///
    /// Updates the display list, layer tree, and scroll tree topology.
    /// Scroll offsets are **not** accepted — the compositor owns them
    /// exclusively and syncs them to the view thread via `ScrollSync`.
    pub fn commit(
        &mut self,
        display_list: Arc<DisplayList>,
        layer_tree: LayerTree,
        scroll_tree: ScrollTree,
    ) {
        self.display_list = Some(display_list);
        self.layer_tree = Some(layer_tree);
        self.scroll_tree = scroll_tree;
    }

    /// Handle a scroll event directly — no view thread round-trip.
    ///
    /// Chrome: `InputHandlerProxy::RouteToTypeSpecificHandler()`.
    pub fn try_scroll(&mut self, target: u32, delta: Offset) -> bool {
        !ScrollController::new(&self.scroll_tree, &mut self.scroll_offsets)
            .scroll(target, delta)
            .is_empty()
    }

    /// Produce a frame for the GPU.
    ///
    /// The scroll offsets are passed directly — the renderer uses them
    /// to override tagged scroll transforms. Zero allocation, O(1) lookup.
    pub fn produce_frame(&self) -> Option<CompositorFrame> {
        let display_list = self.display_list.as_ref()?;
        Some(CompositorFrame {
            display_list: Arc::clone(display_list),
            scroll_offsets: self.scroll_offsets.clone(),
        })
    }

    /// Current scroll offsets — for syncing back to the view thread.
    pub fn scroll_offsets(&self) -> &ScrollOffsets {
        &self.scroll_offsets
    }

    pub fn scroll_tree(&self) -> &ScrollTree {
        &self.scroll_tree
    }

    pub fn has_content(&self) -> bool {
        self.display_list.is_some()
    }

    /// Find the deepest scrollable layer at a screen point.
    ///
    /// Chrome: `InputHandler::HitTestScrollNode()` — compositor-side hit test
    /// against the layer tree to determine which scrollable container should
    /// receive the scroll delta. Returns the DOM node ID of the scroll target.
    ///
    /// Walks the layer tree depth-first, checking bounds. Returns the deepest
    /// scrollable layer whose bounds contain the point. Falls back to
    /// `root_scroller()` if no scrollable layer is hit.
    pub fn hit_test_scroll_target(&self, point: Point) -> Option<u32> {
        let tree = self.layer_tree.as_ref()?;
        let root = tree.root()?;

        let mut best: Option<u32> = None;
        self.hit_test_layer(tree, root, point, &mut best);

        best.or_else(|| self.scroll_tree.root_scroller())
    }

    fn hit_test_layer(
        &self,
        tree: &LayerTree,
        layer_id: layer::LayerId,
        point: Point,
        best: &mut Option<u32>,
    ) {
        let layer = tree.layer(layer_id);

        if !layer.bounds.contains_point(point) {
            return;
        }

        // If this layer is scrollable and has a scroll node, it's a candidate.
        if layer.is_scrollable {
            if let Some(dom_id) = layer.dom_node {
                if self.scroll_tree.contains(dom_id) {
                    *best = Some(dom_id);
                }
            }
        }

        // Check children — deeper layers override (last match wins).
        for &child_id in &layer.children {
            self.hit_test_layer(tree, child_id, point, best);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::display_list::DisplayListBuilder;
    use crate::scroll::node::ScrollNode;
    use kozan_primitives::geometry::Size;

    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}
    #[test]
    fn send_sync_bounds() {
        _assert_send::<Compositor>();
        _assert_send::<CompositorFrame>();
        _assert_send::<LayerTree>();
        _assert_sync::<CompositorFrame>();
    }

    fn empty_display_list() -> Arc<DisplayList> {
        Arc::new(DisplayListBuilder::new().finish())
    }

    fn test_scroll_state() -> (ScrollTree, ScrollOffsets) {
        let mut tree = ScrollTree::new();
        tree.set(1, ScrollNode {
            dom_id: 1, parent: None,
            container: Size::new(800.0, 600.0),
            content: Size::new(800.0, 2000.0),
            scrollable_x: false, scrollable_y: true,
        });
        let mut offsets = ScrollOffsets::new();
        offsets.set_offset(1, Offset::ZERO);
        (tree, offsets)
    }

    #[test]
    fn no_content_before_commit() {
        let c = Compositor::new();
        assert!(!c.has_content());
        assert!(c.produce_frame().is_none());
    }

    #[test]
    fn commit_updates_display_list_and_tree() {
        let mut c = Compositor::new();
        let dl = empty_display_list();
        let (tree, _offsets) = test_scroll_state();
        c.commit(Arc::clone(&dl), LayerTree::new(), tree);
        assert!(c.has_content());
        assert!(Arc::ptr_eq(&c.produce_frame().expect("frame").display_list, &dl));
    }

    #[test]
    fn commit_does_not_overwrite_compositor_scroll_offsets() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree);

        // Compositor scrolls to 120px
        c.try_scroll(1, Offset::new(0.0, 120.0));
        assert_eq!(c.scroll_offsets().offset(1).dy, 120.0);

        // View thread commits again — compositor's 120px must survive
        let (tree2, _offsets2) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree2);
        assert_eq!(c.scroll_offsets().offset(1).dy, 120.0);
    }

    #[test]
    fn try_scroll_updates_offsets() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree);

        assert!(c.try_scroll(1, Offset::new(0.0, 100.0)));
        assert_eq!(c.scroll_offsets().offset(1).dy, 100.0);
    }

    #[test]
    fn try_scroll_clamps() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree);
        c.try_scroll(1, Offset::new(0.0, 99999.0));
        assert_eq!(c.scroll_offsets().offset(1).dy, 1400.0);
    }

    #[test]
    fn try_scroll_unknown_node() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree);
        assert!(!c.try_scroll(99, Offset::new(0.0, 100.0)));
    }

    #[test]
    fn produce_frame_has_scroll_offsets() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree);
        c.try_scroll(1, Offset::new(0.0, 200.0));

        let frame = c.produce_frame().expect("frame");
        assert_eq!(frame.scroll_offsets.offset(1).dy, 200.0);
    }
}
