//! Layout integration — Taffy traits implemented against Document.
//!
//! The DOM IS the layout tree. Taffy reads/writes `LayoutNodeData` directly
//! from Document's parallel storage. No separate `LayoutTree` needed.
//!
//! # Architecture
//!
//! ```text
//! Document                       DocumentLayoutView
//! ────────                       ──────────────────
//! Storage<LayoutNodeData>  ←──   TraversePartialTree (child iteration)
//!   .style                 ←──   LayoutPartialTree   (style access)
//!   .cache                 ←──   CacheTree           (layout caching)
//!   .unrounded_layout      ←──   set_unrounded_layout
//!   .layout_children       ←──   child_ids / child_count
//! ```
//!
//! # Entry point
//!
//! ```ignore
//! // In FrameWidget:
//! doc.resolve_layout(root, Some(vw), Some(vh), &ctx)
//! ```

use core::any::TypeId;

use style::Atom;
use style::properties::ComputedValues;
use taffy::tree::{Cache, Layout, LayoutInput, LayoutOutput, NodeId};
use taffy::{
    CacheTree, LayoutBlockContainer, LayoutFlexboxContainer, LayoutGridContainer,
    LayoutPartialTree, TraversePartialTree, compute_block_layout, compute_cached_layout,
    compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
    compute_root_layout,
};

use kozan_primitives::geometry::{Point, Size};

use kozan_primitives::arena::Storage;

use crate::TextData;
use crate::data_storage::DataStorage;
use crate::dom::document::Document;
use crate::dom::element_data::ElementData;
use crate::dom::node::{NodeMeta, NodeType};
use crate::layout::algo::shared;
use crate::layout::box_model::is_stacking_context;
use crate::layout::context::LayoutContext;
use crate::layout::fragment::{
    BoxFragmentData, ChildFragment, Fragment, OverflowClip, PhysicalInsets, TextFragmentData,
};
use crate::layout::node_data::LayoutNodeData;
use crate::layout::result::{EscapedMargins, LayoutResult};
use crate::tree::{self, TreeData};

// ============================================================
// Document layout methods
// ============================================================

impl Document {
    /// Is this node a text node?
    #[inline]
    pub(crate) fn is_text_node(&self, index: u32) -> bool {
        self.meta
            .get(index)
            .is_some_and(|m| m.flags.node_type() == NodeType::Text)
    }

    /// Get text content by reference (no clone).
    pub(crate) fn text_content_ref(&self, index: u32) -> Option<&str> {
        let meta = self.meta.get(index)?;
        if meta.data_type_id != TypeId::of::<TextData>() {
            return None;
        }
        let data = unsafe { self.data.get::<TextData>(index) };
        Some(&data.content)
    }

    /// Flush `ComputedValues` → `taffy::Style` for all nodes.
    ///
    /// Incremental: `taffy::Style` IS the damage system.
    /// - Style changed → layout-affecting → clear cache + ancestors
    /// - Style unchanged → paint-only (hover color) → skip → 0ms
    /// - Root always cleared (no `ComputedValues` for comparison)
    /// - `force_clear`: true on tree rebuild / resize → clear everything
    pub(crate) fn flush_styles_to_layout(&mut self, index: u32, force_clear: bool) {
        if !self.is_text_node(index) {
            if let Some(cv) = self.computed_style(index) {
                let new_style = shared::computed_to_taffy_item_style(&cv);
                if let Some(data) = self.layout.get_mut(index) {
                    if force_clear {
                        data.style = new_style;
                        data.clear_cache();
                    } else if data.style != new_style {
                        // Layout-affecting change → clear node + ancestors.
                        data.style = new_style;
                        data.clear_cache();
                        self.clear_ancestor_caches(index);
                    }
                    // else: paint-only (e.g., hover color) → skip → 0ms
                }
            } else {
                // Root/document node: always clear (no ComputedValues).
                if let Some(data) = self.layout.get_mut(index) {
                    data.clear_cache();
                }
            }
        } else if force_clear {
            // Text nodes: clear on force (tree rebuild).
            // Normal text changes handled by mark_layout_dirty.
            if let Some(data) = self.layout.get_mut(index) {
                data.clear_cache();
            }
        }

        let dom_children = unsafe { tree::ops::children(&self.tree, index) };
        for child in dom_children {
            self.flush_styles_to_layout(child, force_clear);
        }
    }

    /// Clear Taffy caches up the DOM ancestor chain.
    fn clear_ancestor_caches(&mut self, index: u32) {
        let mut current = index;
        loop {
            let parent = match self.tree.get(current) {
                Some(td) if td.parent != crate::id::INVALID => td.parent,
                _ => break,
            };
            if let Some(pd) = self.layout.get_mut(parent) {
                pd.clear_cache();
            }
            current = parent;
        }
    }

    /// Build `layout_children` from DOM children, filtering `display:none`.
    ///
    /// Sets `layout_parent` on each child for cache propagation.
    /// Text nodes are always included (they're measured, never hidden).
    pub(crate) fn resolve_layout_children(&mut self, index: u32) {
        if self.is_text_node(index) {
            if let Some(data) = self.layout.get_mut(index) {
                data.layout_children = Some(Vec::new());
            }
            return;
        }

        let dom_children = unsafe { tree::ops::children(&self.tree, index) };
        let mut layout_children = Vec::new();

        for child in dom_children {
            // Text nodes always participate in layout.
            if !self.is_text_node(child) {
                let display = self
                    .layout
                    .get(child)
                    .map_or(taffy::Display::None, |d| d.style.display);
                if display == taffy::Display::None {
                    continue;
                }
            }

            if let Some(data) = self.layout.get_mut(child) {
                data.layout_parent = Some(index);
            }
            layout_children.push(child);
            self.resolve_layout_children(child);
        }

        if let Some(data) = self.layout.get_mut(index) {
            data.layout_children = Some(layout_children);
        }
    }

    /// Set root to fill viewport (100% width + height) — Chrome's ICB.
    ///
    /// Root is a Block container (not Flex) — matches Chrome's initial
    /// containing block. Block children with `width: auto` stretch to
    /// fill the parent (normal block flow). Flex would not stretch them.
    pub(crate) fn set_root_viewport_style(&mut self, root: u32) {
        let lp = crate::styling::taffy_bridge::convert::length_percentage(
            &style::values::computed::LengthPercentage::new_percent(
                style::values::computed::Percentage(1.0),
            ),
        );
        let full: taffy::Dimension = lp.into();
        if let Some(data) = self.layout.get_mut(root) {
            data.style.display = taffy::Display::Block;
            data.style.size.width = full;
            data.style.size.height = full;
        }
    }

    /// Swap left↔right insets for absolute children of RTL parents.
    fn apply_rtl_swap_recursive(&mut self, index: u32) {
        let children = self
            .layout
            .get(index)
            .and_then(|d| d.layout_children.clone())
            .unwrap_or_default();

        for &child in &children {
            let is_abs = self
                .layout
                .get(child)
                .is_some_and(|d| d.style.position == taffy::Position::Absolute);

            if is_abs {
                if let Some(parent_cv) = self.computed_style(index) {
                    let dir = shared::InlineDirection::from_style(&parent_cv);
                    if let Some(data) = self.layout.get_mut(child) {
                        dir.swap_absolute_insets(&mut data.style);
                    }
                }
            }
        }

        for child in children {
            self.apply_rtl_swap_recursive(child);
        }
    }

    /// Full layout resolve: flush styles → build children → compute → fragments.
    ///
    /// Single entry point that replaces the old 4-step pipeline:
    /// 1. `build_layout_tree_from_doc()` → `resolve_layout_children()`
    /// 2. `sync_all_styles()` → `flush_styles_to_layout()`
    /// 3. `compute_layout()` → Taffy runs against Document directly
    /// 4. `build_fragment_recursive()` → reads from Document
    ///
    /// Convenience wrapper — always force-clears caches.
    pub fn resolve_layout(
        &mut self,
        root: u32,
        available_width: Option<f32>,
        available_height: Option<f32>,
        ctx: &LayoutContext,
    ) -> LayoutResult {
        self.resolve_layout_dirty(root, available_width, available_height, ctx, true)
    }

    /// Full layout resolve with dirty flag control.
    ///
    /// `layout_dirty`: true = something changed, clear all Taffy caches.
    /// false = nothing changed, Taffy hits cache → ~0ms layout.
    pub fn resolve_layout_dirty(
        &mut self,
        root: u32,
        available_width: Option<f32>,
        available_height: Option<f32>,
        ctx: &LayoutContext,
        layout_dirty: bool,
    ) -> LayoutResult {
        // Step 1: Sync ComputedValues → taffy::Style
        self.flush_styles_to_layout(root, layout_dirty);

        // Step 2: Build layout_children from DOM
        self.resolve_layout_children(root);

        // Step 3: Root fills viewport
        self.set_root_viewport_style(root);

        // Step 4: RTL fixup
        self.apply_rtl_swap_recursive(root);

        // Step 5: Compute layout via Taffy
        let available_space = taffy::Size {
            width: available_width.map_or(
                taffy::AvailableSpace::MaxContent,
                taffy::AvailableSpace::Definite,
            ),
            height: available_height.map_or(
                taffy::AvailableSpace::MaxContent,
                taffy::AvailableSpace::Definite,
            ),
        };
        {
            let mut view = DocumentLayoutView::from_document(self, ctx);
            view.compute_layout(root, available_space);
        }

        // Step 6: Build Fragment tree
        build_fragment_from_document(self, ctx, root)
    }
}

// ============================================================
// DocumentLayoutView — Taffy trait implementations
// ============================================================

/// Layout view — implements Taffy's trait-based API via split borrows.
///
/// Borrows `layout` mutably (Taffy writes results + cache here) and
/// everything else immutably. This allows other systems (scroll, animation)
/// to read Document state concurrently during layout in the future.
///
/// Chrome equivalent: layout algorithms take read-only constraint space;
/// writes go to a separate output buffer (`NGLayoutResult`).
pub(crate) struct DocumentLayoutView<'a> {
    layout: &'a mut Storage<LayoutNodeData>,
    meta: &'a Storage<NodeMeta>,
    tree: &'a Storage<TreeData>,
    element_data: &'a Storage<ElementData>,
    data: &'a DataStorage,
    ctx: &'a LayoutContext<'a>,
}

impl<'a> DocumentLayoutView<'a> {
    fn from_document(doc: &'a mut Document, ctx: &'a LayoutContext<'a>) -> Self {
        Self {
            layout: &mut doc.layout,
            meta: &doc.meta,
            tree: &doc.tree,
            element_data: &doc.element_data,
            data: &doc.data,
            ctx,
        }
    }

    fn compute_layout(&mut self, root: u32, available_space: taffy::Size<taffy::AvailableSpace>) {
        let root_node = NodeId::from(root as u64);
        compute_root_layout(self, root_node, available_space);
    }

    #[inline]
    fn is_text_node(&self, index: u32) -> bool {
        self.meta
            .get(index)
            .is_some_and(|m| m.flags.node_type() == NodeType::Text)
    }

    fn text_content_ref(&self, index: u32) -> Option<&str> {
        let meta = self.meta.get(index)?;
        if meta.data_type_id != std::any::TypeId::of::<TextData>() {
            return None;
        }
        let data = unsafe { self.data.get::<TextData>(index) };
        Some(&data.content)
    }

    fn computed_style(&self, index: u32) -> Option<servo_arc::Arc<ComputedValues>> {
        let ed = self.element_data.get(index)?;
        let data = ed.stylo_data.borrow();
        if data.has_styles() {
            Some(data.styles.primary().clone())
        } else {
            None
        }
    }
}

// ============================================================
// TraversePartialTree — child iteration via layout_children
// ============================================================

pub(crate) struct ChildIter {
    ids: Vec<u32>,
    pos: usize,
}

impl Iterator for ChildIter {
    type Item = NodeId;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.ids.len() {
            let id = self.ids[self.pos];
            self.pos += 1;
            Some(NodeId::from(id as u64))
        } else {
            None
        }
    }
}

impl TraversePartialTree for DocumentLayoutView<'_> {
    type ChildIter<'c>
        = ChildIter
    where
        Self: 'c;

    fn child_ids(&self, parent: NodeId) -> Self::ChildIter<'_> {
        let idx = u64::from(parent) as u32;
        let ids = self
            .layout
            .get(idx)
            .and_then(|d| d.layout_children.as_ref())
            .cloned()
            .unwrap_or_default();
        ChildIter { ids, pos: 0 }
    }

    fn child_count(&self, parent: NodeId) -> usize {
        let idx = u64::from(parent) as u32;
        self.layout
            .get(idx)
            .and_then(|d| d.layout_children.as_ref())
            .map_or(0, |c| c.len())
    }

    fn get_child_id(&self, parent: NodeId, index: usize) -> NodeId {
        let idx = u64::from(parent) as u32;
        let child = self
            .layout
            .get(idx)
            .and_then(|d| d.layout_children.as_ref())
            .and_then(|c| c.get(index))
            .copied()
            .expect("child index out of bounds");
        NodeId::from(child as u64)
    }
}

// ============================================================
// LayoutPartialTree — core layout dispatch
// ============================================================

impl LayoutPartialTree for DocumentLayoutView<'_> {
    type CoreContainerStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;
    type CustomIdent = Atom;

    fn get_core_container_style(&self, node: NodeId) -> Self::CoreContainerStyle<'_> {
        let idx = u64::from(node) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }

    fn set_unrounded_layout(&mut self, node: NodeId, layout: &Layout) {
        let idx = u64::from(node) as u32;
        if let Some(data) = self.layout.get_mut(idx) {
            data.unrounded_layout = *layout;
        }
    }

    fn compute_child_layout(&mut self, node: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let idx = u64::from(node) as u32;
        let display = self
            .layout
            .get(idx)
            .map_or(taffy::Display::None, |d| d.style.display);

        if display == taffy::Display::None {
            return compute_hidden_layout(self, node);
        }

        compute_cached_layout(self, node, inputs, |view, node, inputs| {
            view.compute_uncached_child_layout(node, inputs, display)
        })
    }
}

impl DocumentLayoutView<'_> {
    /// Compute layout for a node that missed the cache.
    ///
    /// `display` is the display value already read by the caller to decide
    /// between hidden/cached paths — passed in to avoid re-reading storage.
    fn compute_uncached_child_layout(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        display: taffy::Display,
    ) -> LayoutOutput {
        let idx = u64::from(node) as u32;
        let is_text = self.is_text_node(idx);
        let has_children = self
            .layout
            .get(idx)
            .and_then(|d| d.layout_children.as_ref())
            .is_some_and(|c: &Vec<u32>| !c.is_empty());

        if is_text && !has_children {
            return self.layout_text_leaf(node, inputs, idx);
        }

        let is_replaced = self
            .layout
            .get(idx)
            .is_some_and(|d| d.style.item_is_replaced);

        if is_replaced {
            return self.layout_replaced(node, inputs, idx);
        }

        self.layout_container(node, inputs, display, idx)
    }

    fn layout_text_leaf(&mut self, _node: NodeId, inputs: LayoutInput, idx: u32) -> LayoutOutput {
        let parent_idx = self.tree.get(idx).map_or(crate::id::INVALID, |t| t.parent);
        let parent_style = if parent_idx != crate::id::INVALID {
            self.computed_style(parent_idx)
        } else {
            None
        }
        .unwrap_or_else(|| crate::styling::initial_values_arc().clone());

        let text = self.text_content_ref(idx).map(str::to_string);
        let style = &self.layout.get(idx).expect("missing layout data").style;
        let measurer = self.ctx.text_measurer;

        compute_leaf_layout(
            inputs,
            style,
            |_val, _basis| 0.0,
            |known_dimensions, available_space| {
                compute_text_measure(
                    known_dimensions,
                    available_space,
                    text.as_deref(),
                    measurer,
                    &parent_style,
                )
            },
        )
    }

    fn layout_replaced(&mut self, _node: NodeId, inputs: LayoutInput, idx: u32) -> LayoutOutput {
        let style = &self.layout.get(idx).expect("missing layout data").style;
        compute_leaf_layout(
            inputs,
            style,
            |_val, _basis| 0.0,
            |known_dimensions, _available_space| taffy::Size {
                width: known_dimensions.width.unwrap_or(0.0),
                height: known_dimensions.height.unwrap_or(0.0),
            },
        )
    }

    fn layout_container(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        display: taffy::Display,
        idx: u32,
    ) -> LayoutOutput {
        match display {
            taffy::Display::Flex => compute_flexbox_layout(self, node, inputs),
            taffy::Display::Grid => compute_grid_layout(self, node, inputs),
            taffy::Display::Block => compute_block_layout(self, node, inputs),
            _ => {
                let style = &self.layout.get(idx).expect("missing layout data").style;
                compute_leaf_layout(
                    inputs,
                    style,
                    |_val, _basis| 0.0,
                    |known, _avail| taffy::Size {
                        width: known.width.unwrap_or(0.0),
                        height: known.height.unwrap_or(0.0),
                    },
                )
            }
        }
    }
}

// ============================================================
// Text measurement
// ============================================================

/// Measure a text node's size given known/available constraints.
///
/// Called by the Taffy leaf-layout measure callback. `text` is `None` for
/// nodes whose content was unexpectedly empty — yields zero size.
fn compute_text_measure(
    known_dimensions: taffy::Size<Option<f32>>,
    available_space: taffy::Size<taffy::AvailableSpace>,
    text: Option<&str>,
    measurer: &dyn crate::layout::inline::measurer::TextMeasurer,
    parent_style: &ComputedValues,
) -> taffy::Size<f32> {
    let Some(text) = text else {
        return taffy::Size {
            width: known_dimensions.width.unwrap_or(0.0),
            height: known_dimensions.height.unwrap_or(0.0),
        };
    };

    use crate::layout::inline::font_system::FontQuery;
    let query = FontQuery::from_computed(parent_style);

    let max_w = match available_space.width {
        taffy::AvailableSpace::Definite(w) => Some(w),
        taffy::AvailableSpace::MaxContent => None,
        taffy::AvailableSpace::MinContent => Some(0.0),
    };

    let text_metrics = measurer.shape_text(text, &query);
    let fm = measurer.query_metrics(&query);

    let text_width = if let Some(mw) = max_w {
        text_metrics.width.min(mw)
    } else {
        text_metrics.width
    };

    let lh = crate::layout::inline::measurer::resolve_line_height(
        &parent_style.clone_line_height(),
        query.font_size,
        &fm,
    );

    taffy::Size {
        width: known_dimensions.width.unwrap_or(text_width),
        height: known_dimensions.height.unwrap_or(lh),
    }
}

// ============================================================
// CacheTree — layout caching
// ============================================================

impl CacheTree for DocumentLayoutView<'_> {
    fn cache_get(
        &self,
        node: NodeId,
        known_dimensions: taffy::Size<Option<f32>>,
        available_space: taffy::Size<taffy::AvailableSpace>,
        run_mode: taffy::tree::RunMode,
    ) -> Option<LayoutOutput> {
        let idx = u64::from(node) as u32;
        self.layout
            .get(idx)?
            .cache
            .get(known_dimensions, available_space, run_mode)
    }

    fn cache_store(
        &mut self,
        node: NodeId,
        known_dimensions: taffy::Size<Option<f32>>,
        available_space: taffy::Size<taffy::AvailableSpace>,
        run_mode: taffy::tree::RunMode,
        layout_output: LayoutOutput,
    ) {
        let idx = u64::from(node) as u32;
        if let Some(data) = self.layout.get_mut(idx) {
            data.cache
                .store(known_dimensions, available_space, run_mode, layout_output);
        }
    }

    fn cache_clear(&mut self, node: NodeId) {
        let idx = u64::from(node) as u32;
        if let Some(data) = self.layout.get_mut(idx) {
            data.cache = Cache::new();
        }
    }
}

// ============================================================
// Container style traits (flex, grid, block)
// ============================================================

impl LayoutFlexboxContainer for DocumentLayoutView<'_> {
    type FlexboxContainerStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;

    fn get_flexbox_container_style(&self, node: NodeId) -> Self::FlexboxContainerStyle<'_> {
        let idx = u64::from(node) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }

    fn get_flexbox_child_style(&self, child: NodeId) -> Self::FlexboxItemStyle<'_> {
        let idx = u64::from(child) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }
}

impl LayoutGridContainer for DocumentLayoutView<'_> {
    type GridContainerStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;

    fn get_grid_container_style(&self, node: NodeId) -> Self::GridContainerStyle<'_> {
        let idx = u64::from(node) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }

    fn get_grid_child_style(&self, child: NodeId) -> Self::GridItemStyle<'_> {
        let idx = u64::from(child) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }
}

impl LayoutBlockContainer for DocumentLayoutView<'_> {
    type BlockContainerStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;
    type BlockItemStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;

    fn get_block_container_style(&self, node: NodeId) -> Self::BlockContainerStyle<'_> {
        let idx = u64::from(node) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }

    fn get_block_child_style(&self, child: NodeId) -> Self::BlockItemStyle<'_> {
        let idx = u64::from(child) as u32;
        &self.layout.get(idx).expect("missing layout data").style
    }
}

// ============================================================
// Fragment building from Document layout results
// ============================================================

/// Build the immutable Fragment tree from Document's layout results.
///
/// Reads `LayoutNodeData.unrounded_layout` for positions/sizes,
/// `ComputedValues` for paint properties, and shapes text.
pub(crate) fn build_fragment_from_document(
    doc: &Document,
    ctx: &LayoutContext,
    root: u32,
) -> LayoutResult {
    build_fragment_recursive(doc, ctx, root)
}

/// Map Stylo's computed overflow value to our fragment-level `OverflowClip`.
fn overflow_clip_from_style(overflow: style::computed_values::overflow_x::T) -> OverflowClip {
    use style::computed_values::overflow_x::T;
    match overflow {
        T::Visible => OverflowClip::Visible,
        T::Hidden | T::Clip => OverflowClip::Hidden,
        T::Scroll => OverflowClip::Scroll,
        T::Auto => OverflowClip::Auto,
    }
}

fn build_fragment_recursive(doc: &Document, ctx: &LayoutContext, index: u32) -> LayoutResult {
    let layout_data = doc.layout.get(index).expect("missing layout data");
    let layout = &layout_data.unrounded_layout;
    let is_text = doc.is_text_node(index);

    // Style: text nodes use parent's, elements use their own.
    let style = if is_text {
        let parent_idx = doc.tree.get(index).map_or(crate::id::INVALID, |t| t.parent);
        if parent_idx != crate::id::INVALID {
            doc.computed_style(parent_idx)
                .unwrap_or_else(|| crate::styling::initial_values_arc().clone())
        } else {
            crate::styling::initial_values_arc().clone()
        }
    } else {
        doc.computed_style(index)
            .unwrap_or_else(|| crate::styling::initial_values_arc().clone())
    };

    let dom_node = Some(index);

    let border = PhysicalInsets {
        top: layout.border.top,
        right: layout.border.right,
        bottom: layout.border.bottom,
        left: layout.border.left,
    };
    let padding = PhysicalInsets {
        top: layout.padding.top,
        right: layout.padding.right,
        bottom: layout.padding.bottom,
        left: layout.padding.left,
    };

    // Build children fragments recursively.
    let children_ids = layout_data.layout_children.clone().unwrap_or_default();
    let mut children = Vec::with_capacity(children_ids.len());

    for &child_id in &children_ids {
        let child_layout = &doc
            .layout
            .get(child_id)
            .expect("missing layout data")
            .unrounded_layout;
        let child_result = build_fragment_recursive(doc, ctx, child_id);

        children.push(ChildFragment {
            offset: Point::new(child_layout.location.x, child_layout.location.y),
            fragment: child_result.fragment,
        });
    }

    let size = Size::new(layout.size.width, layout.size.height);

    // Scrollable overflow = bounding box of all children's positioned extents.
    // Chrome: `NGPhysicalBoxFragment::ComputeScrollableOverflow()`.
    let scrollable_overflow = {
        let mut max_w = 0.0_f32;
        let mut max_h = 0.0_f32;
        for child in &children {
            max_w = max_w.max(child.offset.x + child.fragment.size.width);
            max_h = max_h.max(child.offset.y + child.fragment.size.height);
        }
        Size::new(max_w, max_h)
    };

    // RTL post-layout fixup.
    let display = layout_data.style.display;
    let dir = shared::InlineDirection::from_style(&style);
    let child_positions: Vec<_> = children_ids
        .iter()
        .map(|&id| {
            doc.layout
                .get(id)
                .expect("missing layout data for child during RTL fixup")
                .style
                .position
        })
        .collect();
    dir.fixup_children(
        display,
        &mut children,
        &child_positions,
        border.left,
        border.right,
        padding.left,
        padding.right,
        size.width,
    );

    // Text → TextFragment, Box → BoxFragment.
    let has_children = !children.is_empty();

    let fragment = if is_text && !has_children {
        let text: Option<std::sync::Arc<str>> = doc.text_content_ref(index).map(Into::into);
        let text_len = text.as_ref().map_or(0, |t| t.len());

        use crate::layout::inline::font_system::FontQuery;
        let query = FontQuery::from_computed(&style);
        let color_abs = style.clone_color();
        let color_rgba = [
            (color_abs.components.0 * 255.0).round().clamp(0.0, 255.0) as u8,
            (color_abs.components.1 * 255.0).round().clamp(0.0, 255.0) as u8,
            (color_abs.components.2 * 255.0).round().clamp(0.0, 255.0) as u8,
            (color_abs.alpha * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let shaped_runs = if let Some(ref t) = text {
            ctx.text_measurer.shape_glyphs(t, &query, color_rgba)
        } else {
            Vec::new()
        };

        let metrics = ctx.text_measurer.font_metrics(query.font_size);
        let lh = crate::layout::inline::measurer::resolve_line_height(
            &style.clone_line_height(),
            query.font_size,
            &metrics,
        );
        let font_height = metrics.ascent + metrics.descent;
        let half_leading = ((lh - font_height) / 2.0).max(0.0);
        let baseline = metrics.ascent + half_leading;

        Fragment::new_text_styled(
            size,
            TextFragmentData {
                text_range: 0..text_len,
                baseline,
                text,
                shaped_runs,
            },
            style,
            dom_node,
        )
    } else {
        Fragment::new_box_styled(
            size,
            BoxFragmentData {
                children,
                padding,
                border,
                scrollable_overflow,
                is_stacking_context: is_stacking_context(&style),
                overflow_x: overflow_clip_from_style(style.clone_overflow_x()),
                overflow_y: overflow_clip_from_style(style.clone_overflow_y()),
            },
            style,
            dom_node,
        )
    };

    LayoutResult {
        fragment,
        intrinsic_sizes: None,
        escaped_margins: EscapedMargins::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::traits::{Element, HasHandle};
    use crate::layout::inline::FontSystem;

    #[test]
    fn resolve_layout_produces_fragment() {
        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);
        doc.recalc_styles();

        let measurer = FontSystem::new();
        let ctx = LayoutContext {
            text_measurer: &measurer,
        };

        let root = doc.root_index();
        let result = doc.resolve_layout(root, Some(800.0), Some(600.0), &ctx);
        assert!(result.fragment.size.width > 0.0);
        assert!(result.fragment.size.height > 0.0);
    }

    #[test]
    fn text_node_gets_layout() {
        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);

        let text = doc.create_text("Hello");
        doc.body().append(text.handle());
        doc.recalc_styles();

        let measurer = FontSystem::new();
        let ctx = LayoutContext {
            text_measurer: &measurer,
        };

        let root = doc.root_index();
        let result = doc.resolve_layout(root, Some(800.0), Some(600.0), &ctx);
        assert!(result.fragment.size.width > 0.0);
    }

    #[test]
    fn display_none_excluded_from_layout() {
        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);

        // Add stylesheet that hides elements with class "hidden"
        doc.add_stylesheet(".hidden { display: none; }");

        let div = doc.create::<crate::HtmlDivElement>();
        div.set_class_name("hidden");
        doc.body().append(div.handle());
        doc.recalc_styles();

        let body_idx = doc.body;
        doc.flush_styles_to_layout(doc.root_index(), false);
        doc.resolve_layout_children(doc.root_index());

        // Body's layout_children should not include the hidden div.
        let body_children = doc
            .layout
            .get(body_idx)
            .and_then(|d| d.layout_children.as_ref())
            .map(|c| c.len())
            .unwrap_or(0);
        assert_eq!(body_children, 0);
    }

    #[test]
    fn is_text_node_works() {
        let doc = Document::new();
        // Root is not a text node.
        assert!(!doc.is_text_node(doc.root_index()));
    }

    #[test]
    fn layout_children_set_for_text() {
        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);

        let text = doc.create_text("Hello");
        let text_idx = text.handle().raw().index();
        doc.body().append(text.handle());
        doc.recalc_styles();

        doc.flush_styles_to_layout(doc.root_index(), false);
        doc.resolve_layout_children(doc.root_index());

        // Text nodes are leaves — empty layout_children.
        let text_children = doc
            .layout
            .get(text_idx)
            .and_then(|d| d.layout_children.as_ref())
            .map(|c| c.len());
        assert_eq!(text_children, Some(0));
    }
}
