//! Layout methods on Document — Taffy pipeline and fragment building.

use core::any::TypeId;
use std::sync::Arc;

use kozan_primitives::geometry::{Point, Size};

use crate::TextData;
use crate::dom::document::Document;
use crate::dom::node::NodeType;
use crate::html::html_canvas_element::{CanvasContent, CanvasData};
use crate::layout::algo::shared;
use crate::layout::box_model::is_stacking_context;
use crate::layout::context::LayoutContext;
use crate::layout::document_layout::DocumentLayoutView;
use crate::layout::fragment::{
    BoxFragmentData, ChildFragment, Fragment, OverflowClip, OverscrollBehavior, PhysicalInsets,
    TextFragmentData,
};
use crate::layout::result::{EscapedMargins, LayoutResult};
use crate::tree;

impl Document {
    #[inline]
    pub(crate) fn is_text_node(&self, index: u32) -> bool {
        self.meta
            .get(index)
            .is_some_and(|m| m.flags().node_type() == NodeType::Text)
    }

    pub(crate) fn text_content_ref(&self, index: u32) -> Option<&str> {
        let meta = self.meta.get(index)?;
        if meta.data_type_id() != TypeId::of::<TextData>() {
            return None;
        }
        let data = unsafe { self.data.get::<TextData>(index) };
        Some(&data.content)
    }

    /// Read a canvas element's committed recording from the arena.
    /// Returns None if the node isn't a canvas or has no recording.
    fn canvas_replaced_content(
        &self,
        index: u32,
    ) -> Option<Arc<dyn crate::layout::fragment::ReplacedContent>> {
        let meta = self.meta.get(index)?;
        if meta.data_type_id() != TypeId::of::<CanvasData>() {
            return None;
        }
        let data = unsafe { self.data.get::<CanvasData>(index) };
        let recording = data.committed.as_ref()?;
        Some(Arc::new(CanvasContent {
            recording: Arc::clone(recording),
            canvas_width: data.canvas_width,
            canvas_height: data.canvas_height,
        }))
    }

    /// Flush `ComputedValues` -> `taffy::Style` for all nodes.
    ///
    /// Incremental: `taffy::Style` IS the damage system.
    /// - Style changed -> layout-affecting -> clear cache + ancestors
    /// - Style unchanged -> paint-only (hover color) -> skip -> 0ms
    /// - Root always cleared (no `ComputedValues` for comparison)
    /// - `force_clear`: true on tree rebuild / resize -> clear everything
    pub(crate) fn flush_styles_to_layout(&mut self, index: u32, force_clear: bool) {
        if !self.is_text_node(index) {
            if let Some(cv) = self.computed_style(index) {
                let new_style = shared::computed_to_taffy_item_style(&cv);
                if let Some(data) = self.layout.get_mut(index) {
                    if force_clear {
                        data.style = new_style;
                        data.clear_cache();
                    } else if data.style != new_style {
                        data.style = new_style;
                        data.clear_cache();
                        self.clear_ancestor_caches(index);
                    }
                }
            } else if let Some(data) = self.layout.get_mut(index) {
                data.clear_cache();
            }
        } else if force_clear {
            if let Some(data) = self.layout.get_mut(index) {
                data.clear_cache();
            }
        }

        let dom_children = unsafe { tree::ops::children(&self.tree, index) };
        for child in dom_children {
            self.flush_styles_to_layout(child, force_clear);
        }
    }

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

    /// Set root to fill viewport (100% width + height) -- Chrome's ICB.
    ///
    /// Root is a Block container (not Flex) -- matches Chrome's initial
    /// containing block. Block children with `width: auto` stretch to
    /// fill the parent (normal block flow). Flex would not stretch them.
    ///
    /// Also resolves viewport overflow per CSS Overflow 3 §3.3.
    pub(crate) fn set_root_viewport_style(&mut self, root: u32) {
        let lp = crate::styling::taffy_bridge::convert::length_percentage(
            &style::values::computed::LengthPercentage::new_percent(
                style::values::computed::Percentage(1.0),
            ),
        );
        let full: taffy::Dimension = lp.into();

        // CSS Overflow 3 §3.3 — viewport overflow propagation.
        let (taffy_ox, taffy_oy) = self.resolve_viewport_overflow(root);

        if let Some(data) = self.layout.get_mut(root) {
            data.style.display = taffy::Display::Block;
            data.style.size.width = full;
            data.style.size.height = full;
            data.style.overflow.x = taffy_ox;
            data.style.overflow.y = taffy_oy;
        }
    }

    /// CSS Overflow 3 §3.3 — resolve viewport overflow propagation.
    ///
    /// Algorithm:
    /// 1. If root has `overflow: visible` in both axes AND `<body>` exists
    ///    → propagate body's overflow to viewport, body gets used `visible`.
    /// 2. Otherwise → use root's overflow for viewport.
    /// 3. Viewport special case: `visible` → `auto` (browsers always allow
    ///    viewport scrolling when content overflows).
    ///
    /// Stores results in `self.viewport_overflow` and `self.body_overflow_propagated`
    /// for use by the fragment builder.
    fn resolve_viewport_overflow(&mut self, root: u32) -> (taffy::Overflow, taffy::Overflow) {
        use style::computed_values::overflow_x::T as OT;

        let root_cv = self.computed_style(root);
        let (root_ox, root_oy) = root_cv
            .as_ref()
            .map(|s| (s.clone_overflow_x(), s.clone_overflow_y()))
            .unwrap_or((OT::Visible, OT::Visible));

        // §3.3: When root is <html> with overflow:visible in both axes
        // and a <body> child exists, propagate body's overflow instead.
        let (resolved_ox, resolved_oy, from_body) =
            if root_ox == OT::Visible && root_oy == OT::Visible && self.body != 0 {
                let body_cv = self.computed_style(self.body);
                let (bx, by) = body_cv
                    .as_ref()
                    .map(|s| (s.clone_overflow_x(), s.clone_overflow_y()))
                    .unwrap_or((OT::Visible, OT::Visible));

                // "The element from which the value is propagated must then
                //  have a used overflow value of visible." — reset body's
                //  Taffy overflow so layout treats body as non-clipping.
                if let Some(body_data) = self.layout.get_mut(self.body) {
                    body_data.style.overflow.x = taffy::Overflow::Visible;
                    body_data.style.overflow.y = taffy::Overflow::Visible;
                }

                (bx, by, true)
            } else {
                (root_ox, root_oy, false)
            };

        // CSS 2.1 §11.1.1 + CSS Overflow 3: when the resolved viewport
        // overflow is `visible`, UAs treat it as `auto` — the viewport
        // always allows scrolling when content overflows.
        let final_ox = if resolved_ox == OT::Visible { OT::Auto } else { resolved_ox };
        let final_oy = if resolved_oy == OT::Visible { OT::Auto } else { resolved_oy };

        // Store for fragment builder.
        self.viewport_overflow = [
            overflow_clip_from_style(final_ox),
            overflow_clip_from_style(final_oy),
        ];
        self.body_overflow_propagated = from_body;

        (
            crate::styling::taffy_bridge::convert::overflow(final_ox),
            crate::styling::taffy_bridge::convert::overflow(final_oy),
        )
    }

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

    /// Full layout resolve: flush styles, build children, compute, build fragments.
    ///
    /// Convenience wrapper -- always force-clears caches.
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
    /// false = nothing changed, Taffy hits cache.
    pub fn resolve_layout_dirty(
        &mut self,
        root: u32,
        available_width: Option<f32>,
        available_height: Option<f32>,
        ctx: &LayoutContext,
        layout_dirty: bool,
    ) -> LayoutResult {
        self.flush_styles_to_layout(root, layout_dirty);
        self.resolve_layout_children(root);
        self.set_root_viewport_style(root);
        self.apply_rtl_swap_recursive(root);

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
            let mut view = self.layout_view(ctx);
            view.compute_layout(root, available_space);
        }

        self.build_fragments(ctx, root)
    }

    /// Construct the Taffy layout view from this document's fields.
    pub(crate) fn layout_view<'a>(&'a mut self, ctx: &'a LayoutContext<'a>) -> DocumentLayoutView<'a> {
        DocumentLayoutView::new(
            &mut self.layout,
            &self.meta,
            &self.tree,
            &self.element_data,
            &self.data,
            ctx,
        )
    }

    #[cfg(test)]
    pub(crate) fn layout_children_count(&self, index: u32) -> usize {
        self.layout
            .get(index)
            .and_then(|d| d.layout_children.as_ref())
            .map_or(0, |c| c.len())
    }

    pub(crate) fn build_fragments(&self, ctx: &LayoutContext, root: u32) -> LayoutResult {
        self.build_fragment_recursive(ctx, root)
    }

    fn build_fragment_recursive(&self, ctx: &LayoutContext, index: u32) -> LayoutResult {
        let layout_data = self.layout.get(index).expect("missing layout data");
        let layout = &layout_data.unrounded_layout;
        let is_text = self.is_text_node(index);

        let style = if is_text {
            let parent_idx = self.tree.get(index).map_or(crate::id::INVALID, |t| t.parent);
            if parent_idx != crate::id::INVALID {
                self.computed_style(parent_idx)
                    .unwrap_or_else(|| crate::styling::initial_values_arc().clone())
            } else {
                crate::styling::initial_values_arc().clone()
            }
        } else {
            self.computed_style(index)
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

        let children_ids = layout_data.layout_children.clone().unwrap_or_default();
        let mut children = Vec::with_capacity(children_ids.len());

        for &child_id in &children_ids {
            let child_layout = &self
                .layout
                .get(child_id)
                .expect("missing layout data")
                .unrounded_layout;
            let child_result = self.build_fragment_recursive(ctx, child_id);

            children.push(ChildFragment {
                offset: Point::new(child_layout.location.x, child_layout.location.y),
                fragment: child_result.fragment,
            });
        }

        let size = Size::new(layout.size.width, layout.size.height);

        let scrollable_overflow = compute_scrollable_overflow(&children);

        let display = layout_data.style.display;
        let dir = shared::InlineDirection::from_style(&style);
        let child_positions: Vec<_> = children_ids
            .iter()
            .map(|&id| {
                self.layout
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

        let has_children = !children.is_empty();

        let fragment = if is_text && !has_children {
            let text: Option<std::sync::Arc<str>> = self.text_content_ref(index).map(Into::into);
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
            // CSS Overflow 3 §3.3 — viewport overflow propagation.
            // Root uses resolved viewport overflow. Body uses `visible`
            // when its overflow was propagated to the viewport.
            let (overflow_x, overflow_y) = if index == self.root {
                (self.viewport_overflow[0], self.viewport_overflow[1])
            } else if index == self.body && self.body_overflow_propagated {
                (OverflowClip::Visible, OverflowClip::Visible)
            } else {
                (
                    overflow_clip_from_style(style.clone_overflow_x()),
                    overflow_clip_from_style(style.clone_overflow_y()),
                )
            };

            let (overscroll_x, overscroll_y) = (
                overscroll_from_style(style.clone_overscroll_behavior_x()),
                overscroll_from_style(style.clone_overscroll_behavior_y()),
            );

            let replaced_content = self.canvas_replaced_content(index);
            Fragment::new_box_styled(
                size,
                BoxFragmentData {
                    children,
                    padding,
                    border,
                    scrollable_overflow,
                    is_stacking_context: is_stacking_context(&style),
                    overflow_x,
                    overflow_y,
                    replaced_content,
                    overscroll_x,
                    overscroll_y,
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
}

/// CSS Overflow Module Level 3 §2.1 — scrollable overflow region.
///
/// Chrome: `NGPhysicalBoxFragment::ComputeScrollableOverflow()`.
///
/// Recursive: a child contributes its `scrollable_overflow` if it doesn't
/// clip on that axis, otherwise just its border box. Since fragments are
/// built bottom-up, each child's `scrollable_overflow` already includes
/// its own descendants.
fn compute_scrollable_overflow(children: &[ChildFragment]) -> Size {
    let mut max_w = 0.0_f32;
    let mut max_h = 0.0_f32;
    for child in children {
        let (w, h) = child.fragment.overflow_extent();
        max_w = max_w.max(child.offset.x + w);
        max_h = max_h.max(child.offset.y + h);
    }
    Size::new(max_w, max_h)
}

fn overflow_clip_from_style(overflow: style::computed_values::overflow_x::T) -> OverflowClip {
    use style::computed_values::overflow_x::T;
    match overflow {
        T::Visible => OverflowClip::Visible,
        T::Hidden | T::Clip => OverflowClip::Hidden,
        T::Scroll => OverflowClip::Scroll,
        T::Auto => OverflowClip::Auto,
    }
}

fn overscroll_from_style(
    v: style::values::computed::OverscrollBehavior,
) -> OverscrollBehavior {
    use style::values::computed::OverscrollBehavior as OB;
    match v {
        OB::Auto => OverscrollBehavior::Auto,
        OB::Contain => OverscrollBehavior::Contain,
        OB::None => OverscrollBehavior::None,
    }
}


#[cfg(test)]
mod tests {
    use crate::dom::document::Document;
    use crate::dom::traits::{Element, HasHandle};
    use crate::layout::context::LayoutContext;
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

        doc.add_stylesheet(".hidden { display: none; }");

        let div = doc.create::<crate::HtmlDivElement>();
        div.set_class_name("hidden");
        doc.body().append(div.handle());
        doc.recalc_styles();

        let body_idx = doc.body().raw().index();
        doc.flush_styles_to_layout(doc.root_index(), false);
        doc.resolve_layout_children(doc.root_index());

        let body_children = doc.layout_children_count(body_idx);
        assert_eq!(body_children, 0);
    }

    #[test]
    fn is_text_node_works() {
        let doc = Document::new();
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

        let text_children = doc.layout_children_count(text_idx);
        assert_eq!(text_children, 0);
    }

    /// CSS Overflow 3 §3.3 — root fragment gets `overflow: auto` by default.
    ///
    /// When neither `<html>` nor `<body>` sets explicit overflow, the viewport
    /// should behave as `overflow: auto` so content can scroll when it overflows.
    #[test]
    fn viewport_overflow_defaults_to_auto() {
        use crate::layout::fragment::{FragmentKind, OverflowClip};

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

        let FragmentKind::Box(ref data) = result.fragment.kind else {
            panic!("root should be a box fragment");
        };
        assert_eq!(data.overflow_x, OverflowClip::Auto, "viewport X should be Auto");
        assert_eq!(data.overflow_y, OverflowClip::Auto, "viewport Y should be Auto");
    }

    /// CSS Overflow 3 §3.3 — body's overflow propagates to viewport.
    ///
    /// When body has `overflow: hidden` and root is `visible`, the viewport
    /// should get `hidden` and body should get `visible`.
    #[test]
    fn body_overflow_propagates_to_viewport() {
        use crate::layout::fragment::{FragmentKind, OverflowClip};

        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);
        doc.add_stylesheet("body { overflow: hidden; }");
        doc.recalc_styles();

        let measurer = FontSystem::new();
        let ctx = LayoutContext {
            text_measurer: &measurer,
        };

        let root = doc.root_index();
        let result = doc.resolve_layout(root, Some(800.0), Some(600.0), &ctx);

        // Root (viewport) should get body's overflow: hidden.
        let FragmentKind::Box(ref root_data) = result.fragment.kind else {
            panic!("root should be a box fragment");
        };
        assert_eq!(root_data.overflow_x, OverflowClip::Hidden, "viewport should get body's hidden");
        assert_eq!(root_data.overflow_y, OverflowClip::Hidden, "viewport should get body's hidden");

        // Body fragment should have used overflow: visible (propagation source).
        let body_frag = &root_data.children[0];
        let FragmentKind::Box(ref body_data) = body_frag.fragment.kind else {
            panic!("body should be a box fragment");
        };
        assert_eq!(body_data.overflow_x, OverflowClip::Visible, "body should be visible after propagation");
        assert_eq!(body_data.overflow_y, OverflowClip::Visible, "body should be visible after propagation");
    }

    /// Viewport units (vh) must resolve using the actual viewport dimensions.
    /// `max-height: 75vh` at viewport 800×600 should resolve to 450px,
    /// constraining the element's height.
    #[test]
    fn vh_units_resolve_to_viewport_height() {
        use crate::dom::traits::ContainerNode;
        use crate::html::HtmlDivElement;

        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);

        // Create a tall child inside a max-height-constrained container.
        doc.add_stylesheet(
            ".container { max-height: 75vh; overflow-y: auto; } \
             .tall { height: 1000px; }",
        );

        let container = doc.create::<HtmlDivElement>();
        container.class_add("container");
        let tall = doc.create::<HtmlDivElement>();
        tall.class_add("tall");
        container.append(tall);
        doc.body().append(container);

        doc.recalc_styles();

        let measurer = FontSystem::new();
        let ctx = LayoutContext {
            text_measurer: &measurer,
        };

        let root = doc.root_index();
        let result = doc.resolve_layout(root, Some(800.0), Some(600.0), &ctx);

        // Find the container fragment (first child of body, which is first child of root).
        let root_data = result.fragment.try_as_box().unwrap();
        let body_frag = &root_data.children[0];
        let body_data = body_frag.fragment.try_as_box().unwrap();
        let container_frag = &body_data.children[0];

        // 75vh at 600px viewport = 450px. Container should be at most 450px, not 1000px.
        let container_h = container_frag.fragment.size.height;
        assert!(
            container_h <= 460.0,
            "max-height: 75vh should constrain to ~450px, got {container_h}",
        );
        assert!(
            container_h > 100.0,
            "container should have content, got {container_h}",
        );
    }

    /// Devtools-like layout: flex column + gap + max-height: 75vh + overflow-y: auto.
    /// This pattern must work identically to a browser — content constrained to
    /// 75vh, scrollbar only when content overflows that constraint.
    #[test]
    fn flex_column_max_height_vh_constrains_overflow() {
        use crate::dom::traits::ContainerNode;
        use crate::html::HtmlDivElement;
        use crate::layout::fragment::OverflowClip;

        let mut doc = Document::new();
        doc.init_body();
        doc.set_viewport(800.0, 600.0);

        doc.add_stylesheet(
            ".scroll-body { \
                display: flex; flex-direction: column; \
                gap: 10px; padding: 12px; \
                max-height: 75vh; overflow-y: auto; \
             } \
             .item { height: 80px; flex-shrink: 0; } \
             .tall-item { height: 200px; flex-shrink: 0; }",
        );

        let scroll_body = doc.create::<HtmlDivElement>();
        scroll_body.class_add("scroll-body");

        // 6 items: 5×80 + 1×200 = 600px content + 5×10 gap = 650px + 24px padding = 674px
        // 75vh = 450px. Content > container → overflow + scrollbar.
        for i in 0..6 {
            let item = doc.create::<HtmlDivElement>();
            if i == 5 { item.class_add("tall-item"); } else { item.class_add("item"); }
            scroll_body.append(item);
        }
        doc.body().append(scroll_body);

        doc.recalc_styles();

        let measurer = FontSystem::new();
        let ctx = LayoutContext { text_measurer: &measurer };
        let root = doc.root_index();
        let result = doc.resolve_layout(root, Some(800.0), Some(600.0), &ctx);

        let root_data = result.fragment.try_as_box().unwrap();
        let body_frag = &root_data.children[0];
        let body_data = body_frag.fragment.try_as_box().unwrap();
        let container_frag = &body_data.children[0];
        let container_data = container_frag.fragment.try_as_box().unwrap();

        let container_h = container_frag.fragment.size.height;
        // 75vh = 450px content-box + 24px padding (12px×2) = 474px total.
        // box-sizing: content-box (default) means max-height constrains content only.
        assert!(
            container_h <= 480.0,
            "flex column with max-height: 75vh should be ≤ ~474px, got {container_h}",
        );

        // Overflow should be Auto on Y axis
        assert_eq!(container_data.overflow_y, OverflowClip::Auto);

        // Scrollable overflow should be > container height (content overflows)
        let scroll_overflow_h = container_data.scrollable_overflow.height;
        assert!(
            scroll_overflow_h > container_h,
            "scrollable overflow ({scroll_overflow_h}) should exceed container ({container_h})",
        );
    }
}
