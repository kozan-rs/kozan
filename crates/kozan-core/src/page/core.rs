//! Page — the engine's top-level entry point.
//!
//! Chrome: `Page` (`blink/core/page/page.h`) — owns a `LocalFrame`,
//! `FocusController`, and `Viewport`. All platform operations go
//! through `Page`.

use std::sync::Arc;

use crate::compositor::layer_tree::LayerTree;
use crate::dom::document::Document;
use crate::frame::LocalFrame;
use crate::input::InputEvent;
use crate::layout::fragment::Fragment;
use crate::layout::inline::FontSystem;
use crate::lifecycle::LifecycleState;
use crate::paint::DisplayList;
use crate::scroll::{ScrollOffsets, ScrollTree};

use super::FocusController;
use super::Viewport;
use super::VisualViewport;

/// The engine's top-level entry point for a rendering context.
///
/// Chrome: `Page` owns one `LocalFrame` and coordinates window-level
/// concerns (focus, viewport, future: drag, context menu).
///
/// The platform NEVER creates `Document` directly. Everything goes
/// through `Page`.
pub struct Page {
    frame: LocalFrame,
    focus: FocusController,
    viewport: Viewport,
    visual_viewport: VisualViewport,
}

impl Page {
    #[must_use]
    pub fn new() -> Self {
        Self {
            frame: LocalFrame::new(),
            focus: FocusController::new(),
            viewport: Viewport::default(),
            visual_viewport: VisualViewport::new(),
        }
    }

    #[inline]
    pub fn document(&self) -> &Document {
        self.frame.document()
    }

    #[inline]
    pub fn document_mut(&mut self) -> &mut Document {
        self.frame.document_mut()
    }

    #[inline]
    pub fn font_system(&self) -> &FontSystem {
        self.frame.view().font_system()
    }

    #[inline]
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Update physical viewport dimensions.
    ///
    /// Chrome: `LocalFrameView::ViewportSizeChanged()`.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport.resize(width, height);
        let lw = self.viewport.logical_width() as f32;
        let lh = self.viewport.logical_height() as f32;
        self.frame.document_mut().set_viewport(lw, lh);
        self.frame.view_mut().set_viewport_changed();
        self.frame.view_mut().invalidate_layout();
    }

    /// Update DPI scale factor.
    pub fn set_scale_factor(&mut self, factor: f64) {
        self.viewport.set_scale_factor(factor);
        self.frame.view_mut().set_viewport_changed();
        self.frame.view_mut().invalidate_all();
    }

    /// Chrome: Ctrl+/- zoom. Shrinks the layout viewport → content reflows.
    pub fn set_page_zoom(&mut self, factor: f64) {
        self.viewport.set_page_zoom_factor(factor);
        let lw = self.viewport.logical_width() as f32;
        let lh = self.viewport.logical_height() as f32;
        self.frame.document_mut().set_viewport(lw, lh);
        self.frame.view_mut().set_viewport_changed();
        self.frame.view_mut().invalidate_all();
    }

    #[inline]
    #[must_use]
    pub fn page_zoom(&self) -> f64 {
        self.viewport.page_zoom_factor()
    }

    #[inline]
    #[allow(dead_code)] // Platform reads this for pinch-zoom compositor transform.
    pub(crate) fn visual_viewport(&self) -> &VisualViewport {
        &self.visual_viewport
    }

    /// Pinch zoom — compositor-only magnification, no layout reflow.
    pub fn set_pinch_zoom(&mut self, scale: f64) {
        self.visual_viewport.set_scale(scale);
        self.frame.view_mut().invalidate_paint();
    }

    pub fn set_visual_viewport_offset(&mut self, x: f64, y: f64) {
        self.visual_viewport.set_offset(x, y);
    }

    /// Chrome: `WebFrameWidgetImpl::HandleInputEvent()`.
    ///
    /// Returns `(state_changed, scroll_action)`. Scroll actions are returned
    /// to the platform layer for routing to the compositor.
    pub fn handle_input(
        &mut self,
        event: InputEvent,
    ) -> (bool, Option<(u32, kozan_primitives::geometry::Offset)>) {
        self.frame
            .handle_input(event, &mut self.focus, &self.viewport)
    }

    /// Run the rendering lifecycle pipeline (style → layout → paint).
    ///
    /// Chrome: `LocalFrameView::UpdateLifecyclePhases()`.
    pub fn update_lifecycle(&mut self) {
        self.frame.update_lifecycle(&self.viewport);
    }

    /// Force all phases to re-run on the next `update_lifecycle()`.
    pub fn mark_needs_update(&mut self) {
        self.frame.view_mut().invalidate_all();
    }

    #[inline]
    pub fn last_fragment(&self) -> Option<&Arc<Fragment>> {
        self.frame.last_fragment()
    }

    #[inline]
    pub fn last_display_list(&self) -> Option<Arc<DisplayList>> {
        self.frame.last_display_list()
    }

    pub fn take_layer_tree(&mut self) -> Option<LayerTree> {
        self.frame.take_layer_tree()
    }

    pub fn scroll_state_snapshot(&self) -> (ScrollTree, ScrollOffsets) {
        self.frame.scroll_state_snapshot()
    }

    pub fn apply_compositor_scroll(&mut self, offsets: &ScrollOffsets) {
        self.frame.apply_compositor_scroll(offsets);
    }

    #[inline]
    pub fn last_timing(&self) -> kozan_primitives::timing::FrameTiming {
        self.frame.last_timing()
    }

    #[inline]
    pub fn lifecycle(&self) -> LifecycleState {
        self.frame.lifecycle()
    }

    /// Notify that the OS window became foreground/background.
    pub fn set_window_active(&mut self, active: bool) {
        self.focus.set_active(active);
    }

    /// Window focus/blur — clears element focus when the window loses focus.
    pub fn set_window_focused(&mut self, focused: bool) {
        self.focus.set_focused(self.frame.document(), focused);
        if !focused {
            self.frame.view_mut().invalidate_style();
        }
    }

}

impl Default for Page {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{InputEvent, Modifiers, mouse::MouseMoveEvent as RawMouseMoveEvent};
    use std::time::Instant;

    #[test]
    fn construction() {
        let page = Page::new();
        assert_eq!(page.viewport().width(), 0);
        assert_eq!(page.viewport().height(), 0);
        assert_eq!(page.viewport().scale_factor(), 1.0);
        assert_eq!(page.lifecycle(), LifecycleState::PaintClean);
    }

    #[test]
    fn resize_updates_viewport() {
        let mut page = Page::new();
        page.resize(1920, 1080);
        assert_eq!(page.viewport().width(), 1920);
        assert_eq!(page.viewport().height(), 1080);
    }

    #[test]
    fn scale_factor() {
        let mut page = Page::new();
        page.set_scale_factor(2.0);
        assert_eq!(page.viewport().scale_factor(), 2.0);
    }

    #[test]
    fn handle_input_without_fragment() {
        let mut page = Page::new();
        let (changed, _scroll) = page.handle_input(InputEvent::MouseMove(RawMouseMoveEvent {
            x: 100.0,
            y: 200.0,
            modifiers: Modifiers::EMPTY,
            timestamp: Instant::now(),
        }));
        assert!(!changed);
    }

    #[test]
    fn update_lifecycle_runs_all_phases() {
        let mut page = Page::new();
        page.resize(800, 600);
        page.update_lifecycle();

        assert_eq!(page.lifecycle(), LifecycleState::PaintClean);
        assert!(
            page.last_fragment().is_some(),
            "layout should produce a fragment after lifecycle"
        );
    }

    #[test]
    fn update_lifecycle_skips_when_clean() {
        let mut page = Page::new();
        page.resize(800, 600);

        page.update_lifecycle();
        let frag1 = page.last_fragment().cloned();

        page.update_lifecycle();
        let frag2 = page.last_fragment().cloned();

        assert!(
            Arc::ptr_eq(
                frag1.as_ref().expect("frag1"),
                frag2.as_ref().expect("frag2")
            ),
            "clean lifecycle should not re-layout"
        );
    }

    #[test]
    fn resize_triggers_relayout() {
        let mut page = Page::new();
        page.resize(800, 600);
        page.update_lifecycle();
        let frag1 = page.last_fragment().cloned();

        page.resize(1024, 768);
        page.update_lifecycle();
        let frag2 = page.last_fragment().cloned();

        assert!(
            !Arc::ptr_eq(
                frag1.as_ref().expect("frag1"),
                frag2.as_ref().expect("frag2")
            ),
            "resize should trigger relayout"
        );
    }

    #[test]
    fn document_access() {
        let page = Page::new();
        let _doc = page.document();
    }

    #[test]
    fn no_layout_without_resize() {
        let mut page = Page::new();
        page.update_lifecycle();
        assert!(
            page.last_fragment().is_none(),
            "should not layout with zero viewport"
        );
    }

    #[test]
    fn page_zoom_changes_logical_dimensions() {
        let mut page = Page::new();
        page.resize(1920, 1080);
        page.set_page_zoom(2.0);

        assert_eq!(page.page_zoom(), 2.0);
        assert_eq!(page.viewport().logical_width(), 960.0);
        assert_eq!(page.viewport().logical_height(), 540.0);
    }

    #[test]
    fn pinch_zoom_does_not_change_logical_dimensions() {
        let mut page = Page::new();
        page.resize(1920, 1080);
        let w_before = page.viewport().logical_width();
        let h_before = page.viewport().logical_height();

        page.set_pinch_zoom(3.0);

        assert_eq!(page.visual_viewport().scale(), 3.0);
        assert_eq!(page.viewport().logical_width(), w_before);
        assert_eq!(page.viewport().logical_height(), h_before);
    }
}
