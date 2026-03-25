//! Local frame — owns the document, view, and event handler.
//!
//! Chrome: `LocalFrame` (`blink/core/frame/local_frame.h`).

use std::sync::Arc;

use crate::input_handler::{EventHandler, InputContext};
use super::frame_view::FrameView;
use crate::compositor::layer_tree::LayerTree;
use crate::dom::document::Document;
use crate::events::ui_event::ScrollEvent;
use crate::input::default_action::DefaultAction;
use crate::input::InputEvent;
use crate::layout::fragment::Fragment;
use crate::layout::hit_test::HitTester;
use crate::lifecycle::LifecycleState;
use crate::page::FocusController;
use crate::page::Viewport;
use crate::paint::DisplayList;
use crate::scroll::{ScrollController, ScrollOffsets, ScrollTree};

/// A single frame — document, lifecycle pipeline, and event handler.
///
/// Chrome: `LocalFrame` bridges `Document`, `LocalFrameView`, and
/// `EventHandler`. A `Page` owns one `LocalFrame` (multi-frame
/// support is a future extension).
pub(crate) struct LocalFrame {
    doc: Document,
    view: FrameView,
    event_handler: EventHandler,
}

impl LocalFrame {
    pub fn new() -> Self {
        let mut doc = Document::new();
        doc.init_body();

        Self {
            doc,
            view: FrameView::new(),
            event_handler: EventHandler::new(),
        }
    }

    #[inline]
    pub fn document(&self) -> &Document {
        &self.doc
    }

    #[inline]
    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.doc
    }

    #[inline]
    pub fn view(&self) -> &FrameView {
        &self.view
    }

    #[inline]
    pub fn view_mut(&mut self) -> &mut FrameView {
        &mut self.view
    }

    /// Handle an input event. Returns `true` if visual state changed.
    ///
    /// Chrome: `EventHandler::HandleInputEvent()` dispatches DOM events,
    /// then the frame coordinator applies default actions (scroll, focus).
    pub fn handle_input(
        &mut self,
        event: InputEvent,
        focus: &mut FocusController,
        viewport: &Viewport,
    ) -> bool {
        let Some(fragment) = self.view.last_fragment() else {
            return false;
        };
        let fragment = Arc::clone(fragment);

        let hit_tester = HitTester::new(self.view.scroll_offsets());
        let viewport_height = viewport.logical_height() as f32;
        let ctx = InputContext {
            doc: &self.doc,
            fragment: &fragment,
            hit_tester: &hit_tester,
            viewport_height,
            scroll_tree: self.view.scroll_tree(),
        };

        let result = self.event_handler.handle_input(event, &ctx);

        if result.state_changed {
            self.view.invalidate_style();
        }

        match result.default_action {
            DefaultAction::Scroll { target, delta } => {
                self.apply_scroll(target, delta) || result.state_changed
            }
            DefaultAction::FocusNavigate { forward } => {
                focus.advance(&self.doc, forward);
                true
            }
            DefaultAction::ScrollPrevented | DefaultAction::None => result.state_changed,
        }
    }

    /// Apply a scroll delta and dispatch `ScrollEvent` on affected nodes.
    fn apply_scroll(&mut self, target: u32, delta: kozan_primitives::geometry::Offset) -> bool {
        let (tree, offsets) = self.view.scroll_parts_mut();
        let scrolled = ScrollController::new(tree, offsets).scroll(target, delta);
        if scrolled.is_empty() {
            return false;
        }
        self.view.invalidate_paint();
        self.event_handler.invalidate_hit_cache();
        self.event_handler.suppress_hover();

        for node_id in scrolled.iter() {
            let offset = self.view.scroll_offsets().offset(node_id);
            if let Some(handle) = self.doc.handle_for_index(node_id) {
                handle.dispatch_event(&ScrollEvent {
                    scroll_x: offset.dx,
                    scroll_y: offset.dy,
                });
            }
        }
        true
    }

    /// Apply scroll offsets received from the compositor.
    pub fn apply_compositor_scroll(&mut self, offsets: &ScrollOffsets) {
        for (node_id, offset) in offsets.iter() {
            let current = self.view.scroll_offsets().offset(node_id);
            if current != *offset {
                self.view.scroll_offsets_mut().set_offset(node_id, *offset);
                self.view.invalidate_paint();
                self.event_handler.invalidate_hit_cache();
            }
        }
    }

    /// Run style → layout → paint.
    pub fn update_lifecycle(&mut self, viewport: &Viewport) {
        self.view.update_lifecycle(&mut self.doc, viewport);
    }

    #[inline]
    pub fn last_fragment(&self) -> Option<&Arc<Fragment>> {
        self.view.last_fragment()
    }

    #[inline]
    pub fn last_display_list(&self) -> Option<Arc<DisplayList>> {
        self.view.last_display_list()
    }

    pub fn take_layer_tree(&mut self) -> Option<LayerTree> {
        self.view.take_layer_tree()
    }

    pub fn scroll_state_snapshot(&self) -> (ScrollTree, ScrollOffsets) {
        self.view.scroll_state_snapshot()
    }

    #[inline]
    pub fn last_timing(&self) -> kozan_primitives::timing::FrameTiming {
        self.view.last_timing()
    }

    #[inline]
    pub fn lifecycle(&self) -> LifecycleState {
        self.doc.lifecycle()
    }

}
