//! Frame widget — the engine's entry point for a single rendering context.
//!
//! Chrome: `WebFrameWidgetImpl` + `LocalFrameView` — owns the
//! frame's document, viewport, event handler, and runs the lifecycle pipeline.
//!
//! The platform NEVER creates `Document` directly or calls `recalc_styles()`.
//! Everything goes through `FrameWidget`.

use std::sync::Arc;

use super::event_handler::{EventHandler, InputContext};
use super::viewport::Viewport;
use crate::compositor::layer_builder::LayerTreeBuilder;
use crate::compositor::layer_tree::LayerTree;
use crate::dirty_phases::DirtyPhases;
use crate::dom::document::Document;
use crate::events::ui_event::ScrollEvent;
use crate::input::InputEvent;
use crate::input::default_action::DefaultAction;
use crate::layout::context::LayoutContext;
use crate::layout::fragment::Fragment;
use crate::layout::hit_test::HitTester;
use crate::layout::inline::FontSystem;
use crate::lifecycle::LifecycleState;
use crate::paint::DisplayList;
use crate::paint::Painter;
use crate::scroll::{ScrollController, ScrollOffsets, ScrollTree};

/// The engine's entry point for a single rendering context.
///
/// # Responsibilities
/// - Owns the `Document` (DOM tree)
/// - Owns the `Viewport` (dimensions, scale factor)
/// - Owns the `EventHandler` (input → DOM events)
/// - Owns the scroll subsystems (`ScrollTree`, `ScrollOffsets`)
/// - Runs the lifecycle pipeline: style → layout → paint
///
/// # Chrome mapping
///
/// | Chrome | Kozan |
/// |--------|-------|
/// | `WebFrameWidgetImpl` | Platform bridge (input, resize) |
/// | `LocalFrameView` | Lifecycle orchestration |
/// | `DocumentLifecycle` | `LifecycleState` + `DirtyPhases` |
/// | `LayoutView` | Document root (layout via `DocumentLayoutView`) |
///
/// # Usage (from platform)
///
/// ```ignore
/// let mut widget = FrameWidget::new();
/// widget.resize(1920, 1080);
///
/// // User builds DOM
/// let doc = widget.document_mut();
///
/// // Event loop (called from Scheduler::tick render callback)
/// widget.handle_input(input_event);
/// widget.update_lifecycle();
///
/// // Read paint result
/// if let Some(dl) = widget.last_display_list() {
///     // send to renderer
/// }
/// ```
pub struct FrameWidget {
    doc: Document,
    viewport: Viewport,
    event_handler: EventHandler,

    /// Current lifecycle phase — enforces phase ordering.
    /// Chrome: `DocumentLifecycle`.
    lifecycle: LifecycleState,

    /// Per-phase dirty flags — scroll invalidates paint only, hover invalidates all.
    dirty: DirtyPhases,

    /// Immutable fragment tree from last layout pass.
    last_fragment: Option<Arc<Fragment>>,

    /// Font discovery, caching, and text measurement.
    /// Chrome: `LocalFrameView::GetFontCache()`.
    font_system: FontSystem,

    /// Display list from last paint pass. Arc-wrapped for zero-copy
    /// sharing with the render thread.
    last_display_list: Option<Arc<DisplayList>>,

    /// Fragment that was last painted — paint caching via Arc pointer equality.
    painted_fragment: Option<Arc<Fragment>>,

    /// Viewport changed — re-sync all styles, re-run Taffy.
    needs_full_layout: bool,

    /// Previous frame's pipeline timing.
    last_timing: kozan_primitives::timing::FrameTiming,

    /// Layer tree from last paint — committed to the compositor.
    /// Chrome: `LayerTreeHost::pending_tree_`.
    last_layer_tree: Option<LayerTree>,

    /// Scroll chain topology — rebuilt after each layout pass.
    /// Chrome: `cc/trees/scroll_tree.h`.
    scroll_tree: ScrollTree,

    /// Mutable scroll displacement per node — the only state that
    /// changes during scroll (no relayout needed).
    scroll_offsets: ScrollOffsets,
}

impl FrameWidget {
    #[must_use]
    pub fn new() -> Self {
        let mut doc = Document::new();
        doc.init_body();

        Self {
            doc,
            viewport: Viewport::default(),
            event_handler: EventHandler::new(),
            lifecycle: LifecycleState::default(),
            dirty: DirtyPhases::default(),
            last_fragment: None,
            font_system: FontSystem::new(),
            last_display_list: None,
            painted_fragment: None,
            needs_full_layout: false,
            last_timing: kozan_primitives::timing::FrameTiming::default(),
            last_layer_tree: None,
            scroll_tree: ScrollTree::new(),
            scroll_offsets: ScrollOffsets::new(),
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
    pub fn font_system(&self) -> &FontSystem {
        &self.font_system
    }

    #[inline]
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    #[inline]
    pub fn last_fragment(&self) -> Option<&Arc<Fragment>> {
        self.last_fragment.as_ref()
    }

    #[inline]
    pub fn lifecycle(&self) -> LifecycleState {
        self.lifecycle
    }

    /// Handle an input event.
    ///
    /// Performs hit testing against the last fragment tree, dispatches
    /// DOM events, and applies scroll actions via the scroll controller.
    /// Returns `true` if visual state changed (DOM mutation or scroll).
    ///
    /// Chrome: `WebFrameWidgetImpl::HandleInputEvent()`.
    pub fn handle_input(&mut self, event: InputEvent) -> bool {
        let Some(fragment) = &self.last_fragment else {
            return false;
        };
        let fragment = Arc::clone(fragment);

        let hit_tester = HitTester::new(&self.scroll_offsets);
        let viewport_height = self.viewport.logical_height() as f32;
        let ctx = InputContext {
            surface: &self.doc,
            fragment: &fragment,
            hit_tester: &hit_tester,
            viewport_height,
            scroll_tree: &self.scroll_tree,
        };

        let result = self.event_handler.handle_input(event, &ctx);

        if result.state_changed {
            self.dirty.invalidate_style();
        }

        match result.default_action {
            DefaultAction::Scroll { target, delta } => {
                self.apply_scroll(target, delta) || result.state_changed
            }
            DefaultAction::FocusNext | DefaultAction::FocusPrev | DefaultAction::Activate => {
                // Stubs — wired when focus management lands.
                result.state_changed
            }
            DefaultAction::ScrollPrevented | DefaultAction::None => result.state_changed,
        }
    }

    /// Apply a scroll delta and dispatch `ScrollEvent` on affected nodes.
    /// Returns `true` if any node actually scrolled.
    fn apply_scroll(&mut self, target: u32, delta: kozan_primitives::geometry::Offset) -> bool {
        let scrolled = ScrollController::new(&self.scroll_tree, &mut self.scroll_offsets)
            .scroll(target, delta);
        if scrolled.is_empty() {
            return false;
        }
        self.dirty.invalidate_paint();
        self.event_handler.invalidate_hit_cache();
        self.event_handler.suppress_hover();

        for node_id in scrolled.iter() {
            let offset = self.scroll_offsets.offset(node_id);
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
    ///
    /// Called at the start of each frame on the view thread so the next
    /// paint uses the compositor's authoritative scroll positions.
    pub fn apply_compositor_scroll(&mut self, offsets: &ScrollOffsets) {
        for (node_id, offset) in offsets.iter() {
            let current = self.scroll_offsets.offset(node_id);
            if current != *offset {
                self.scroll_offsets.set_offset(node_id, *offset);
                self.dirty.invalidate_paint();
                self.event_handler.invalidate_hit_cache();
            }
        }
    }

    /// Run the rendering lifecycle pipeline.
    ///
    /// Chrome: `LocalFrameView::UpdateLifecyclePhases()`.
    ///
    /// Phases: style recalc → layout → paint.
    /// `DirtyPhases` controls which phases actually run — scroll only
    /// invalidates paint, so style+layout are skipped at 60fps during scroll.
    pub fn update_lifecycle(&mut self) {
        if self.doc.needs_visual_update() || self.needs_full_layout {
            self.dirty.invalidate_all();
        }

        if !self.dirty.needs_update() && self.last_fragment.is_some() {
            return;
        }

        let t0 = std::time::Instant::now();
        let mut style_ms = 0.0;
        let mut layout_ms = 0.0;

        if self.dirty.needs_style() {
            self.lifecycle = LifecycleState::InStyleRecalc;
            let t = std::time::Instant::now();
            self.doc.recalc_styles();
            style_ms = t.elapsed().as_secs_f64() * 1000.0;
            self.lifecycle = LifecycleState::StyleClean;
            self.dirty.clear_style();
        }

        if self.dirty.needs_layout() || self.last_fragment.is_none() {
            self.lifecycle = LifecycleState::InLayout;
            let t = std::time::Instant::now();
            self.layout_pass();
            layout_ms = t.elapsed().as_secs_f64() * 1000.0;
            self.lifecycle = LifecycleState::LayoutClean;
            self.dirty.clear_layout();
        }

        if self.dirty.needs_paint() {
            self.lifecycle = LifecycleState::InPaint;
            let t = std::time::Instant::now();
            self.paint_pass();
            let paint_ms = t.elapsed().as_secs_f64() * 1000.0;
            self.lifecycle = LifecycleState::PaintClean;
            self.dirty.clear_paint();

            self.last_timing = kozan_primitives::timing::FrameTiming {
                style_ms,
                layout_ms,
                paint_ms,
                total_ms: t0.elapsed().as_secs_f64() * 1000.0,
            };
        }
    }

    /// Layout pass — DOM IS the layout tree.
    ///
    /// Taffy's cache handles incrementality automatically.
    /// After layout, rebuilds the scroll tree from the new fragment tree.
    fn layout_pass(&mut self) {
        if self.viewport.width() == 0 || self.viewport.height() == 0 {
            return;
        }

        let vw = self.viewport.logical_width() as f32;
        let vh = self.viewport.logical_height() as f32;

        let layout_dirty = self.doc.take_layout_dirty() || self.needs_full_layout;
        self.needs_full_layout = false;

        let ctx = LayoutContext {
            text_measurer: &self.font_system,
        };
        let root = self.doc.root_index();
        let result = self
            .doc
            .resolve_layout_dirty(root, Some(vw), Some(vh), &ctx, layout_dirty);
        self.last_fragment = Some(result.fragment);

        // Rebuild scroll tree from the new fragment tree.
        if let Some(frag) = &self.last_fragment {
            self.scroll_tree.sync(frag);
        }
    }

    /// Paint pass — generate display list from fragment tree.
    ///
    /// Chrome: `LocalFrameView::PaintTree()`.
    fn paint_pass(&mut self) {
        let Some(fragment) = &self.last_fragment else {
            return;
        };

        // Paint caching: skip if fragment tree hasn't changed AND no
        // scroll offset changed (dirty paint flag handles the latter).
        if let Some(painted) = &self.painted_fragment {
            if Arc::ptr_eq(painted, fragment) && !self.dirty.needs_paint() {
                return;
            }
        }

        let viewport_size = kozan_primitives::geometry::Size::new(
            self.viewport.logical_width() as f32,
            self.viewport.logical_height() as f32,
        );

        let display_list = Painter::new(&self.scroll_offsets).paint(fragment, viewport_size);
        self.last_display_list = Some(Arc::new(display_list));
        self.painted_fragment = Some(Arc::clone(fragment));

        // Build layer tree for the compositor.
        self.last_layer_tree = Some(LayerTreeBuilder::new(&self.scroll_offsets).build(fragment));
    }

    /// The last paint result. The `Arc` is cloned cheaply — no copy of the list.
    #[inline]
    pub fn last_display_list(&self) -> Option<Arc<DisplayList>> {
        self.last_display_list.as_ref().map(Arc::clone)
    }

    /// Take the layer tree for commit to the compositor.
    pub fn take_layer_tree(&mut self) -> Option<LayerTree> {
        self.last_layer_tree.take()
    }

    /// Clone scroll state for the compositor.
    /// Cheap: typically 1-5 nodes × 40 bytes.
    pub fn scroll_state_snapshot(&self) -> (ScrollTree, ScrollOffsets) {
        (self.scroll_tree.clone(), self.scroll_offsets.clone())
    }

    /// Update the viewport dimensions (physical pixels).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport.resize(width, height);
        let lw = self.viewport.logical_width() as f32;
        let lh = self.viewport.logical_height() as f32;
        self.doc.set_viewport(lw, lh);
        // Styles haven't changed — only the available space did.
        // Taffy's cache handles this: nodes whose output depends on the
        // available space will miss the cache automatically. No need to
        // nuke all caches with needs_full_layout.
        self.dirty.invalidate_layout();
    }

    pub fn set_scale_factor(&mut self, factor: f64) {
        self.viewport.set_scale_factor(factor);
        self.dirty.invalidate_all();
        self.needs_full_layout = true;
    }

    /// Force the lifecycle to re-run on the next `update_lifecycle()` call.
    ///
    /// Called by the platform when the scheduler's frame callback fires,
    /// because `set_needs_frame()` means something changed that requires
    /// a new frame.
    pub fn mark_needs_update(&mut self) {
        self.dirty.invalidate_all();
    }

    #[inline]
    pub fn last_timing(&self) -> kozan_primitives::timing::FrameTiming {
        self.last_timing
    }

    pub fn set_focus(&mut self, _focused: bool) {
        // Future: dispatch focus/blur DOM events.
    }
}

impl Default for FrameWidget {
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
        let widget = FrameWidget::new();
        assert_eq!(widget.viewport().width(), 0);
        assert_eq!(widget.viewport().height(), 0);
        assert_eq!(widget.viewport().scale_factor(), 1.0);
        assert_eq!(widget.lifecycle(), LifecycleState::PaintClean);
    }

    #[test]
    fn resize_updates_viewport() {
        let mut widget = FrameWidget::new();
        widget.resize(1920, 1080);
        assert_eq!(widget.viewport().width(), 1920);
        assert_eq!(widget.viewport().height(), 1080);
    }

    #[test]
    fn resize_invalidates_dirty() {
        let mut widget = FrameWidget::new();
        widget.resize(800, 600);
        assert!(widget.dirty.needs_update());
    }

    #[test]
    fn scale_factor() {
        let mut widget = FrameWidget::new();
        widget.set_scale_factor(2.0);
        assert_eq!(widget.viewport().scale_factor(), 2.0);
    }

    #[test]
    fn handle_input_without_fragment() {
        let mut widget = FrameWidget::new();
        let changed = widget.handle_input(InputEvent::MouseMove(RawMouseMoveEvent {
            x: 100.0,
            y: 200.0,
            modifiers: Modifiers::EMPTY,
            timestamp: Instant::now(),
        }));
        assert!(!changed);
    }

    #[test]
    fn update_lifecycle_runs_all_phases() {
        let mut widget = FrameWidget::new();
        widget.resize(800, 600);
        widget.update_lifecycle();

        assert_eq!(widget.lifecycle(), LifecycleState::PaintClean);
        assert!(
            widget.last_fragment().is_some(),
            "layout should produce a fragment after lifecycle"
        );
    }

    #[test]
    fn update_lifecycle_skips_when_clean() {
        let mut widget = FrameWidget::new();
        widget.resize(800, 600);

        widget.update_lifecycle();
        let frag1 = widget.last_fragment().cloned();

        widget.update_lifecycle();
        let frag2 = widget.last_fragment().cloned();

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
        let mut widget = FrameWidget::new();
        widget.resize(800, 600);
        widget.update_lifecycle();
        let frag1 = widget.last_fragment().cloned();

        widget.resize(1024, 768);
        widget.update_lifecycle();
        let frag2 = widget.last_fragment().cloned();

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
        let widget = FrameWidget::new();
        let _doc = widget.document();
    }

    #[test]
    fn no_layout_without_resize() {
        let mut widget = FrameWidget::new();
        widget.update_lifecycle();
        assert!(
            widget.last_fragment().is_none(),
            "should not layout with zero viewport"
        );
    }
}
