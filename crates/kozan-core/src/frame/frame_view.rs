//! Frame view — lifecycle pipeline and paint output cache.
//!
//! Chrome: `LocalFrameView` (`blink/core/frame/local_frame_view.h`).

use std::sync::Arc;

use crate::compositor::layer_builder::LayerTreeBuilder;
use crate::compositor::layer_tree::LayerTree;
use crate::dirty_phases::DirtyPhases;
use crate::dom::document::Document;
use crate::layout::context::LayoutContext;
use crate::layout::fragment::Fragment;
use crate::layout::inline::FontSystem;
use crate::lifecycle::LifecycleState;
use crate::page::Viewport;
use crate::paint::{DisplayList, Painter};
use crate::scroll::{ScrollOffsets, ScrollTree};

/// Lifecycle pipeline and paint output cache for a single frame.
///
/// Drives style → layout → paint. Caches immutable outputs
/// (`Arc<Fragment>`, `Arc<DisplayList>`) for zero-copy sharing
/// with the render thread.
pub(crate) struct FrameView {
    dirty: DirtyPhases,
    font_system: FontSystem,
    scroll_tree: ScrollTree,
    scroll_offsets: ScrollOffsets,
    last_fragment: Option<Arc<Fragment>>,
    last_display_list: Option<Arc<DisplayList>>,
    painted_fragment: Option<Arc<Fragment>>,
    last_layer_tree: Option<LayerTree>,
    last_timing: kozan_primitives::timing::FrameTiming,
    viewport_changed: bool,
}

impl FrameView {
    pub fn new() -> Self {
        Self {
            dirty: DirtyPhases::default(),
            font_system: FontSystem::new(),
            scroll_tree: ScrollTree::new(),
            scroll_offsets: ScrollOffsets::new(),
            last_fragment: None,
            last_display_list: None,
            painted_fragment: None,
            last_layer_tree: None,
            last_timing: kozan_primitives::timing::FrameTiming::default(),
            viewport_changed: false,
        }
    }

    /// Run the rendering lifecycle pipeline.
    ///
    /// Chrome: `LocalFrameView::UpdateLifecyclePhases()`.
    /// Phases: style recalc → layout → paint.
    /// `DirtyPhases` controls which phases run — scroll only invalidates
    /// paint, so style+layout are skipped at 60fps during scroll.
    pub fn update_lifecycle(&mut self, doc: &mut Document, viewport: &Viewport) {
        if doc.needs_visual_update() {
            self.dirty.invalidate_style();
        }
        if self.viewport_changed {
            self.dirty.invalidate_layout();
        }

        if !self.dirty.needs_update() && self.last_fragment.is_some() {
            return;
        }

        let t0 = std::time::Instant::now();
        let mut style_ms = 0.0;
        let mut layout_ms = 0.0;

        if self.dirty.needs_style() {
            doc.advance_lifecycle(LifecycleState::InStyleRecalc);
            let t = std::time::Instant::now();
            doc.recalc_styles();
            style_ms = t.elapsed().as_secs_f64() * 1000.0;
            doc.advance_lifecycle(LifecycleState::StyleClean);
            self.dirty.clear_style();
        }

        if self.dirty.needs_layout() || self.last_fragment.is_none() {
            doc.advance_lifecycle(LifecycleState::InLayout);
            let t = std::time::Instant::now();
            self.layout_pass(doc, viewport);
            layout_ms = t.elapsed().as_secs_f64() * 1000.0;
            doc.advance_lifecycle(LifecycleState::LayoutClean);
            self.dirty.clear_layout();
        }

        if self.dirty.needs_paint() {
            doc.advance_lifecycle(LifecycleState::InPaint);
            let t = std::time::Instant::now();
            self.paint_pass(viewport);
            let paint_ms = t.elapsed().as_secs_f64() * 1000.0;
            doc.advance_lifecycle(LifecycleState::PaintClean);
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
    fn layout_pass(&mut self, doc: &mut Document, viewport: &Viewport) {
        if viewport.width() == 0 || viewport.height() == 0 {
            return;
        }

        let vw = viewport.logical_width() as f32;
        let vh = viewport.logical_height() as f32;

        // Full Taffy cache clear when tree structure changed (nodes
        // added/removed → layout_children stale) OR viewport dimensions
        // changed (every % / vw / vh may resolve differently).
        let tree_changed = doc.take_needs_full_layout_clear();
        let force_clear = tree_changed || self.viewport_changed;
        self.viewport_changed = false;

        let ctx = LayoutContext {
            text_measurer: &self.font_system,
        };
        let root = doc.root_index();
        let result = doc.resolve_layout_dirty(root, Some(vw), Some(vh), &ctx, force_clear);
        self.last_fragment = Some(result.fragment);

        if let Some(frag) = &self.last_fragment {
            self.scroll_tree.sync(frag);
        }
    }

    /// Paint pass — generate display list from fragment tree.
    ///
    /// Chrome: `LocalFrameView::PaintTree()`.
    fn paint_pass(&mut self, viewport: &Viewport) {
        let Some(fragment) = &self.last_fragment else {
            return;
        };

        if let Some(painted) = &self.painted_fragment {
            if Arc::ptr_eq(painted, fragment) && !self.dirty.needs_paint() {
                return;
            }
        }

        let viewport_size = kozan_primitives::geometry::Size::new(
            viewport.logical_width() as f32,
            viewport.logical_height() as f32,
        );

        let display_list = Painter::new(&self.scroll_offsets).paint(fragment, viewport_size);
        self.last_display_list = Some(Arc::new(display_list));
        self.painted_fragment = Some(Arc::clone(fragment));

        self.last_layer_tree = Some(LayerTreeBuilder::new(&self.scroll_offsets).build(fragment));
    }

    // ── Invalidation ──

    pub fn invalidate_style(&mut self) {
        self.dirty.invalidate_style();
    }

    pub fn invalidate_layout(&mut self) {
        self.dirty.invalidate_layout();
    }

    pub fn invalidate_paint(&mut self) {
        self.dirty.invalidate_paint();
    }

    pub fn invalidate_all(&mut self) {
        self.dirty.invalidate_all();
    }

    pub fn set_viewport_changed(&mut self) {
        self.viewport_changed = true;
    }


    // ── Output ──

    #[inline]
    pub fn last_fragment(&self) -> Option<&Arc<Fragment>> {
        self.last_fragment.as_ref()
    }

    #[inline]
    pub fn last_display_list(&self) -> Option<Arc<DisplayList>> {
        self.last_display_list.as_ref().map(Arc::clone)
    }

    pub fn take_layer_tree(&mut self) -> Option<LayerTree> {
        self.last_layer_tree.take()
    }

    #[inline]
    pub fn last_timing(&self) -> kozan_primitives::timing::FrameTiming {
        self.last_timing
    }

    // ── Font system ──

    #[inline]
    pub fn font_system(&self) -> &FontSystem {
        &self.font_system
    }

    // ── Scroll ──

    #[inline]
    pub fn scroll_tree(&self) -> &ScrollTree {
        &self.scroll_tree
    }

    #[inline]
    pub fn scroll_offsets(&self) -> &ScrollOffsets {
        &self.scroll_offsets
    }

    #[inline]
    pub fn scroll_offsets_mut(&mut self) -> &mut ScrollOffsets {
        &mut self.scroll_offsets
    }

    pub fn scroll_state_snapshot(&self) -> (ScrollTree, ScrollOffsets) {
        (self.scroll_tree.clone(), self.scroll_offsets.clone())
    }

    /// Borrow scroll tree and offsets simultaneously — avoids split borrow issues.
    pub fn scroll_parts_mut(&mut self) -> (&ScrollTree, &mut ScrollOffsets) {
        (&self.scroll_tree, &mut self.scroll_offsets)
    }
}
