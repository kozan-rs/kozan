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

use style::Atom;
use style::properties::ComputedValues;
use taffy::tree::{Cache, Layout, LayoutInput, LayoutOutput, NodeId};
use taffy::{
    CacheTree, LayoutBlockContainer, LayoutFlexboxContainer, LayoutGridContainer,
    LayoutPartialTree, TraversePartialTree, compute_block_layout, compute_cached_layout,
    compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
    compute_root_layout,
};

use kozan_primitives::arena::Storage;

use crate::TextData;
use crate::data_storage::DataStorage;
use crate::dom::element_data::ElementData;
use crate::dom::node::{NodeMeta, NodeType};
use crate::layout::context::LayoutContext;
use crate::layout::node_data::LayoutNodeData;
use crate::tree::TreeData;


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
    pub(crate) fn new(
        layout: &'a mut Storage<LayoutNodeData>,
        meta: &'a Storage<NodeMeta>,
        tree: &'a Storage<TreeData>,
        element_data: &'a Storage<ElementData>,
        data: &'a DataStorage,
        ctx: &'a LayoutContext<'a>,
    ) -> Self {
        Self { layout, meta, tree, element_data, data, ctx }
    }

    pub(crate) fn compute_layout(&mut self, root: u32, available_space: taffy::Size<taffy::AvailableSpace>) {
        let root_node = NodeId::from(root as u64);
        compute_root_layout(self, root_node, available_space);
    }

    #[inline]
    fn is_text_node(&self, index: u32) -> bool {
        self.meta
            .get(index)
            .is_some_and(|m| m.flags().node_type() == NodeType::Text)
    }

    fn text_content_ref(&self, index: u32) -> Option<&str> {
        let meta = self.meta.get(index)?;
        if meta.data_type_id() != std::any::TypeId::of::<TextData>() {
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

    let fm = measurer.query_metrics(&query);

    // Chrome: NGInlineNode::ComputeMinMaxSizes() — min-content is the
    // widest unbreakable word, not zero.
    let text_width = match available_space.width {
        taffy::AvailableSpace::Definite(w) => {
            measurer.shape_text(text, &query).width.min(w)
        }
        taffy::AvailableSpace::MaxContent => measurer.shape_text(text, &query).width,
        taffy::AvailableSpace::MinContent => measurer.shape_text_min_content(text, &query),
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


