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

pub(crate) mod content_layer;
pub mod frame;
pub(crate) mod layer;
pub(crate) mod layer_builder;
pub mod layer_tree;
pub(crate) mod scrollbar_animation;
pub(crate) mod scrollbar_controller;
pub(crate) mod scrollbar_layer;
pub(crate) mod scrollbar_theme;

use std::sync::Arc;

use kozan_primitives::geometry::{Offset, Point, Rect};

use crate::paint::DisplayList;
use crate::scroll::{ScrollController, ScrollOffsets, ScrollTree};

use kozan_primitives::arena::Storage;

use self::frame::{CompositorFrame, FrameQuad};
use self::layer::QuadContext;
use self::layer_tree::LayerTree;
use self::scrollbar_animation::ScrollbarAnimation;
use self::scrollbar_controller::{ScrollbarAction, ScrollbarController};
use self::scrollbar_layer::ScrollbarLayer;

struct ScrollbarHit {
    scrollbar: ScrollbarLayer,
    local_point: Point,
}

/// Chrome: `cc::LayerTreeHostImpl`.
pub struct Compositor {
    display_list: Option<Arc<DisplayList>>,
    layer_tree: Option<LayerTree>,
    scroll_tree: ScrollTree,
    scroll_offsets: ScrollOffsets,
    scrollbar_controller: ScrollbarController,
    scrollbar_animations: Storage<ScrollbarAnimation>,
    last_hovered_scrollbar: Option<u32>,
    page_zoom: f32,
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compositor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            display_list: None,
            layer_tree: None,
            scroll_tree: ScrollTree::new(),
            scroll_offsets: ScrollOffsets::new(),
            scrollbar_controller: ScrollbarController::new(),
            scrollbar_animations: Storage::new(),
            last_hovered_scrollbar: None,
            page_zoom: 1.0,
        }
    }

    /// Chrome: `LayerTreeHostImpl::FinishCommit()`.
    pub fn commit(
        &mut self,
        display_list: Arc<DisplayList>,
        layer_tree: LayerTree,
        scroll_tree: ScrollTree,
        page_zoom: f32,
    ) {
        self.display_list = Some(display_list);
        self.layer_tree = Some(layer_tree);
        self.scroll_tree = scroll_tree;
        self.page_zoom = page_zoom;
    }

    /// Chrome: `InputHandlerProxy::RouteToTypeSpecificHandler()`.
    pub fn try_scroll(&mut self, target: u32, delta: Offset) -> bool {
        let result = ScrollController::new(&self.scroll_tree, &mut self.scroll_offsets)
            .scroll(target, delta);

        // Chrome: `DidScrollUpdate` triggers scrollbar fade-in.
        for node_id in result.iter() {
            if self.scrollbar_animations.get(node_id).is_none() {
                self.scrollbar_animations.set(node_id, ScrollbarAnimation::new());
            }
            if let Some(anim) = self.scrollbar_animations.get_mut(node_id) {
                anim.on_scroll();
            }
        }

        !result.is_empty()
    }

    /// Chrome: `LayerTreeHostImpl::PrepareToDraw()` + `CalculateRenderPasses()`.
    #[must_use]
    pub fn produce_frame(&mut self) -> Option<CompositorFrame> {
        let display_list = Arc::clone(self.display_list.as_ref()?);
        self.update_scrollbar_geometries();
        let quads = self.collect_quads();
        Some(CompositorFrame {
            display_list,
            scroll_offsets: self.scroll_offsets.clone(),
            quads,
        })
    }

    /// Chrome: `LayerTreeImpl::UpdateScrollbarGeometries()` + `ScrollbarAnimationController::Animate()`.
    fn update_scrollbar_geometries(&mut self) {
        let tree = match self.layer_tree.as_mut() {
            Some(t) => t,
            None => return,
        };

        let entries = tree.scrollbar_entries();
        for (scroll_element_id, sb_ids) in entries {
            let Some(node) = self.scroll_tree.get(scroll_element_id) else {
                continue;
            };
            let offset = self.scroll_offsets.offset(scroll_element_id);
            let theme = scrollbar_theme::ScrollbarTheme::get();
            let opacity = self
                .scrollbar_animations
                .get(scroll_element_id)
                .map_or(0.0, |a| a.opacity(theme));

            if let Some(v_id) = sb_ids.vertical {
                let layer = tree.layer_mut(v_id);
                if let Some(sb) = layer.content.as_any_mut().downcast_mut::<ScrollbarLayer>() {
                    sb.update_geometry(
                        offset.dy,
                        node.container.height,
                        node.content.height,
                        node.container.width,
                    );
                    sb.opacity = opacity;
                }
            }
            if let Some(h_id) = sb_ids.horizontal {
                let layer = tree.layer_mut(h_id);
                if let Some(sb) = layer.content.as_any_mut().downcast_mut::<ScrollbarLayer>() {
                    sb.update_geometry(
                        offset.dx,
                        node.container.width,
                        node.content.width,
                        node.container.height,
                    );
                    sb.opacity = opacity;
                }
            }
        }
    }

    /// Chrome: `CalculateRenderPasses()` calls `AppendQuads()` on every layer.
    fn collect_quads(&self) -> Vec<FrameQuad> {
        let tree = match self.layer_tree.as_ref() {
            Some(t) => t,
            None => return Vec::new(),
        };
        let Some(root) = tree.root() else {
            return Vec::new();
        };
        let mut quads = Vec::new();
        self.collect_quads_recursive(tree, root, Point::ZERO, &mut quads);
        quads
    }

    fn collect_quads_recursive(
        &self,
        tree: &LayerTree,
        layer_id: layer::LayerId,
        parent_origin: Point,
        out: &mut Vec<FrameQuad>,
    ) {
        let layer = tree.layer(layer_id);
        let origin = layer.transform.transform_point(parent_origin);
        let ctx = QuadContext {
            _origin: origin,
            page_zoom: self.page_zoom,
            container_rect: Rect::new(
                origin.x,
                origin.y,
                layer.bounds.width(),
                layer.bounds.height(),
            ),
        };

        out.extend(layer.content.append_quads(&ctx));

        for &child_id in &layer.children {
            self.collect_quads_recursive(tree, child_id, origin, out);
        }
    }

    /// Chrome: `InputHandlerProxy::HandlePointerDown` side-effect.
    pub fn handle_mouse_down(&mut self, point: Point) -> bool {
        let Some(snapshot) = self.snapshot_scrollbar_at(point) else {
            return false;
        };
        let element_id = snapshot.scrollbar.scroll_element_id;
        let action = self.scrollbar_controller.handle_pointer_down(
            &snapshot.scrollbar,
            snapshot.local_point,
        );
        let scrolled = self.apply_scrollbar_action(action);

        // Set active state on the pressed scrollbar.
        self.set_scrollbar_element_state(
            element_id,
            scrollbar_layer::ScrollbarState::Active,
        );

        scrolled
    }

    /// Chrome: `InputHandlerProxy::HandlePointerMove` side-effect.
    pub fn handle_mouse_move(&mut self, point: Point) -> bool {
        self.update_scrollbar_hover(point);

        if !self.scrollbar_controller.is_dragging() {
            return false;
        }
        let action = self.scrollbar_controller.handle_pointer_move(point);
        self.apply_scrollbar_action(action)
    }

    /// Chrome: `InputHandlerProxy::HandlePointerUp` side-effect.
    pub fn handle_mouse_up(&mut self) {
        self.scrollbar_controller.handle_pointer_up();
    }

    fn snapshot_scrollbar_at(&self, point: Point) -> Option<ScrollbarHit> {
        let tree = self.layer_tree.as_ref()?;
        let root = tree.root()?;
        let mut best = None;
        Self::find_scrollbar(tree, root, point, &mut best);
        best
    }

    fn find_scrollbar(
        tree: &LayerTree,
        layer_id: layer::LayerId,
        point: Point,
        best: &mut Option<ScrollbarHit>,
    ) {
        let layer = tree.layer(layer_id);
        if !layer.bounds.contains_point(point) {
            return;
        }

        if let Some(sb) = layer.content.as_any().downcast_ref::<ScrollbarLayer>() {
            if sb.can_scroll() && sb.identify_part(point) != scrollbar_layer::ScrollbarPart::NoPart
            {
                *best = Some(ScrollbarHit {
                    scrollbar: sb.snapshot(),
                    local_point: point,
                });
            }
        }

        for &child_id in &layer.children {
            let child = tree.layer(child_id);
            let local = child
                .transform
                .inverse()
                .map(|inv| inv.transform_point(point))
                .unwrap_or(point);
            Self::find_scrollbar(tree, child_id, local, best);
        }
    }

    fn set_scrollbar_element_state(
        &mut self,
        element_id: u32,
        state: scrollbar_layer::ScrollbarState,
    ) {
        let tree = match self.layer_tree.as_mut() {
            Some(t) => t,
            None => return,
        };
        if let Some(sb_ids) = tree.scrollbar_ids(element_id).copied() {
            for layer_id in [sb_ids.vertical, sb_ids.horizontal].into_iter().flatten() {
                let layer = tree.layer_mut(layer_id);
                if let Some(sb) = layer.content.as_any_mut().downcast_mut::<ScrollbarLayer>() {
                    sb.set_state(state);
                }
            }
        }
    }

    fn update_scrollbar_hover(&mut self, point: Point) {
        use scrollbar_layer::ScrollbarState;

        let hovered_id = self
            .snapshot_scrollbar_at(point)
            .map(|h| h.scrollbar.scroll_element_id);

        self.last_hovered_scrollbar = hovered_id;

        let tree = match self.layer_tree.as_mut() {
            Some(t) => t,
            None => return,
        };

        let entries = tree.scrollbar_entries();
        for (_, sb_ids) in &entries {
            for layer_id in [sb_ids.vertical, sb_ids.horizontal].into_iter().flatten() {
                let layer = tree.layer_mut(layer_id);
                if let Some(sb) = layer.content.as_any_mut().downcast_mut::<ScrollbarLayer>() {
                    let eid = sb.scroll_element_id;
                    let is_dragged = self.scrollbar_controller.dragged_element() == Some(eid);
                    let is_hovered = hovered_id == Some(eid);
                    let new_state = if is_dragged {
                        ScrollbarState::Active
                    } else if is_hovered {
                        ScrollbarState::Hovered
                    } else {
                        ScrollbarState::Idle
                    };
                    sb.set_state(new_state);

                    if self.scrollbar_animations.get(eid).is_none() {
                        self.scrollbar_animations.set(eid, ScrollbarAnimation::new());
                    }
                    if let Some(anim) = self.scrollbar_animations.get_mut(eid) {
                        anim.set_state(new_state);
                    }
                }
            }
        }
    }

    fn apply_scrollbar_action(&mut self, action: ScrollbarAction) -> bool {
        match action {
            ScrollbarAction::ScrollTo { element_id, offset } => {
                self.scroll_offsets.set_offset(element_id, offset);
                true
            }
            ScrollbarAction::None => false,
        }
    }

    #[must_use]
    pub fn scroll_offsets(&self) -> &ScrollOffsets {
        &self.scroll_offsets
    }

    #[must_use]
    pub fn scroll_tree(&self) -> &ScrollTree {
        &self.scroll_tree
    }

    #[must_use]
    pub fn has_content(&self) -> bool {
        self.display_list.is_some()
    }

    /// Chrome: `SetNeedsAnimateForScrollbarAnimation()`.
    #[must_use]
    pub fn needs_animate(&self) -> bool {
        self.scrollbar_animations
            .iter()
            .any(|(_, anim)| anim.is_animating())
    }

    /// Chrome: `InputHandler::HitTestScrollNode()`.
    #[must_use]
    pub fn hit_test_scroll_target(&self, point: Point) -> Option<u32> {
        let tree = self.layer_tree.as_ref()?;
        let root = tree.root()?;

        let mut best: Option<u32> = None;
        self.hit_test_layer(tree, root, point, &mut best);

        best
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

        if layer.is_scrollable {
            if let Some(dom_id) = layer.dom_node {
                if self.scroll_tree.contains(dom_id) {
                    *best = Some(dom_id);
                }
            }
        }

        // Scrollbar layers: route to their scroll element.
        if let Some(sb) = layer.content.as_any().downcast_ref::<ScrollbarLayer>() {
            if self.scroll_tree.contains(sb.scroll_element_id) {
                *best = Some(sb.scroll_element_id);
            }
        }

        for &child_id in &layer.children {
            let child = tree.layer(child_id);
            let local_point = child
                .transform
                .inverse()
                .map(|inv| inv.transform_point(point))
                .unwrap_or(point);
            self.hit_test_layer(tree, child_id, local_point, best);
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
        let mut offsets = ScrollOffsets::new();
        offsets.set_offset(1, Offset::ZERO);
        (tree, offsets)
    }

    #[test]
    fn no_content_before_commit() {
        let mut c = Compositor::new();
        assert!(!c.has_content());
        assert!(c.produce_frame().is_none());
    }

    #[test]
    fn commit_updates_display_list_and_tree() {
        let mut c = Compositor::new();
        let dl = empty_display_list();
        let (tree, _offsets) = test_scroll_state();
        c.commit(Arc::clone(&dl), LayerTree::new(), tree, 1.0);
        assert!(c.has_content());
        assert!(Arc::ptr_eq(
            &c.produce_frame().expect("frame").display_list,
            &dl
        ));
    }

    #[test]
    fn commit_does_not_overwrite_compositor_scroll_offsets() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree, 1.0);

        c.try_scroll(1, Offset::new(0.0, 120.0));
        assert_eq!(c.scroll_offsets().offset(1).dy, 120.0);

        let (tree2, _offsets2) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree2, 1.0);
        assert_eq!(c.scroll_offsets().offset(1).dy, 120.0);
    }

    #[test]
    fn try_scroll_updates_offsets() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree, 1.0);

        assert!(c.try_scroll(1, Offset::new(0.0, 100.0)));
        assert_eq!(c.scroll_offsets().offset(1).dy, 100.0);
    }

    #[test]
    fn try_scroll_clamps() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree, 1.0);
        c.try_scroll(1, Offset::new(0.0, 99999.0));
        assert_eq!(c.scroll_offsets().offset(1).dy, 1400.0);
    }

    #[test]
    fn try_scroll_unknown_node() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree, 1.0);
        assert!(!c.try_scroll(99, Offset::new(0.0, 100.0)));
    }

    #[test]
    fn produce_frame_has_scroll_offsets() {
        let mut c = Compositor::new();
        let (tree, _offsets) = test_scroll_state();
        c.commit(empty_display_list(), LayerTree::new(), tree, 1.0);
        c.try_scroll(1, Offset::new(0.0, 200.0));

        let frame = c.produce_frame().expect("frame");
        assert_eq!(frame.scroll_offsets.offset(1).dy, 200.0);
    }
}
