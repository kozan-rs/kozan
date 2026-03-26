//! Layout methods on Document — Taffy pipeline and fragment building.

use core::any::TypeId;

use kozan_primitives::geometry::{Point, Size};

use crate::TextData;
use crate::dom::document::Document;
use crate::dom::node::NodeType;
use crate::layout::algo::shared;
use crate::layout::box_model::is_stacking_context;
use crate::layout::context::LayoutContext;
use crate::layout::document_layout::DocumentLayoutView;
use crate::layout::fragment::{
    BoxFragmentData, ChildFragment, Fragment, OverflowClip, PhysicalInsets, TextFragmentData,
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
}
