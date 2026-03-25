//! Fragment tree painter — walks fragments and emits display items.
//!
//! Chrome: `BoxFragmentPainter`, `TextFragmentPainter`.
//!
//! The [`Painter`] struct holds shared state (display list builder,
//! scroll offsets). Methods take only what's unique per call — the
//! fragment and its position.

use kozan_primitives::color::Color;
use kozan_primitives::geometry::{Offset, Point, Rect, Size};
use style::properties::ComputedValues;
use style::values::specified::border::BorderStyle;

use crate::layout::fragment::{
    BoxFragmentData, ChildFragment, Fragment, FragmentKind, LineFragmentData,
};
use crate::scroll::ScrollOffsets;

use super::display_item::{BorderColors, BorderWidths, ClipData, DisplayItem, DrawCommand};
use super::display_list::DisplayListBuilder;

/// Convert a Stylo computed color to our `Color` type.
fn stylo_color_to_color(c: &style::values::computed::Color) -> Color {
    let abs = c.resolve_to_absolute(&style::color::AbsoluteColor::BLACK);
    abs_color_to_color(&abs)
}

fn abs_color_to_color(abs: &style::color::AbsoluteColor) -> Color {
    let c = abs.components;
    Color {
        r: c.0,
        g: c.1,
        b: c.2,
        a: abs.alpha,
    }
}

fn has_border_radius(s: &ComputedValues) -> bool {
    let b = s.get_border();
    let zero = |r: &style::values::computed::BorderCornerRadius| {
        let w = r.0.width.0.to_length().map_or(0.0, |l| l.px());
        let h = r.0.height.0.to_length().map_or(0.0, |l| l.px());
        w == 0.0 && h == 0.0
    };
    !(zero(&b.border_top_left_radius)
        && zero(&b.border_top_right_radius)
        && zero(&b.border_bottom_right_radius)
        && zero(&b.border_bottom_left_radius))
}

fn border_radius_px(r: &style::values::computed::BorderCornerRadius) -> f32 {
    r.0.width.0.to_length().map_or(0.0, |l| l.px())
}

fn border_side_is_visible(width: f32, style: BorderStyle) -> bool {
    width > 0.0 && style != BorderStyle::None && style != BorderStyle::Hidden
}

fn resolve_outline_style(s: &ComputedValues) -> BorderStyle {
    match s.get_outline().outline_style {
        style::values::specified::outline::OutlineStyle::BorderStyle(bs) => bs,
        _ => BorderStyle::Solid,
    }
}

fn compute_inner_radii(
    outer: &super::display_item::BorderRadii,
    bt: f32,
    br: f32,
    bb: f32,
    bl: f32,
) -> super::display_item::BorderRadii {
    super::display_item::BorderRadii {
        top_left: (outer.top_left - bl.max(bt)).max(0.0),
        top_right: (outer.top_right - br.max(bt)).max(0.0),
        bottom_right: (outer.bottom_right - br.max(bb)).max(0.0),
        bottom_left: (outer.bottom_left - bl.max(bb)).max(0.0),
    }
}

fn extract_outer_radii(s: &ComputedValues) -> super::display_item::BorderRadii {
    super::display_item::BorderRadii {
        top_left: border_radius_px(&s.get_border().border_top_left_radius),
        top_right: border_radius_px(&s.get_border().border_top_right_radius),
        bottom_right: border_radius_px(&s.get_border().border_bottom_right_radius),
        bottom_left: border_radius_px(&s.get_border().border_bottom_left_radius),
    }
}

fn z_index(s: &ComputedValues) -> Option<i32> {
    match s.get_position().clone_z_index() {
        style::values::generics::position::ZIndex::Integer(n) => Some(n),
        style::values::generics::position::ZIndex::Auto => None,
    }
}

/// Walks the fragment tree and emits display items.
///
/// Chrome: `BoxFragmentPainter` holds fragment + paint info as a struct.
pub(crate) struct Painter<'a> {
    builder: DisplayListBuilder,
    scroll_offsets: &'a ScrollOffsets,
}

impl<'a> Painter<'a> {
    pub fn new(scroll_offsets: &'a ScrollOffsets) -> Self {
        Self {
            builder: DisplayListBuilder::new(),
            scroll_offsets,
        }
    }

    /// Paint the full tree. Chrome: `FramePainter::Paint()`.
    /// Consume the painter and produce a display list.
    pub fn paint(mut self, root: &Fragment, viewport: Size) -> super::DisplayList {
        self.emit(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, viewport.width, viewport.height),
            color: Color::WHITE,
        }));
        self.paint_fragment(root, Point::ZERO);
        self.builder.finish()
    }

    fn emit(&mut self, item: DisplayItem) {
        self.builder.push(item);
    }

    fn paint_fragment(&mut self, fragment: &Fragment, origin: Point) {
        match &fragment.kind {
            FragmentKind::Box(data) => self.paint_box(fragment, data, origin),
            FragmentKind::Text(data) => self.paint_text(fragment, data, origin),
            FragmentKind::Line(data) => self.paint_line(data, origin),
        }
    }

    fn paint_child(&mut self, child: &ChildFragment, parent_origin: Point) {
        let origin = parent_origin + Offset::new(child.offset.x, child.offset.y);
        self.paint_fragment(&child.fragment, origin);
    }

    fn paint_box(&mut self, fragment: &Fragment, data: &BoxFragmentData, origin: Point) {
        let border_box = Rect::from_origin_size(origin, fragment.size);
        let style = fragment.style.as_ref();

        if style
            .is_some_and(|s| s.clone_visibility() == style::computed_values::visibility::T::Hidden)
        {
            self.paint_children(&data.children, origin);
            return;
        }

        let needs_transform = style.is_some_and(|s| !s.get_box().clone_transform().0.is_empty());
        if needs_transform {
            self.emit(DisplayItem::PushTransform(
                super::display_item::TransformData {
                    translate_x: 0.0,
                    translate_y: 0.0,
                    scroll_node: None,
                },
            ));
        }

        let opacity_val = style.map_or(1.0, |s| s.get_effects().clone_opacity());
        let needs_opacity = opacity_val < 1.0;
        if needs_opacity {
            self.emit(DisplayItem::PushOpacity(opacity_val));
        }

        if let Some(s) = style {
            self.paint_box_shadows(s, border_box);
            self.paint_background_and_border(s, data, border_box);
            self.paint_clipped_children(fragment, data, origin, s);
            self.paint_outline(s, border_box);
        } else {
            self.paint_unstyled_children(data, origin, fragment.size);
        }

        if needs_opacity {
            self.emit(DisplayItem::PopOpacity);
        }
        if needs_transform {
            self.emit(DisplayItem::PopTransform);
        }
    }

    fn paint_box_shadows(&mut self, s: &ComputedValues, border_box: Rect) {
        let shadows = s.get_effects().clone_box_shadow();
        for shadow in shadows.0.iter().rev() {
            self.emit(DisplayItem::Draw(DrawCommand::BoxShadow {
                rect: border_box,
                offset_x: shadow.base.horizontal.px(),
                offset_y: shadow.base.vertical.px(),
                blur: shadow.base.blur.px(),
                spread: shadow.spread.px(),
                color: stylo_color_to_color(&shadow.base.color),
            }));
        }
    }

    fn paint_background_and_border(
        &mut self,
        s: &ComputedValues,
        data: &BoxFragmentData,
        border_box: Rect,
    ) {
        let bg = stylo_color_to_color(&s.get_background().clone_background_color());
        let bt = data.border.top;
        let br = data.border.right;
        let bb = data.border.bottom;
        let bl = data.border.left;
        let padding_box = border_box.inset(bt, br, bb, bl);

        let border_styles = s.get_border();
        let has_border = border_side_is_visible(bt, border_styles.border_top_style)
            || border_side_is_visible(br, border_styles.border_right_style)
            || border_side_is_visible(bb, border_styles.border_bottom_style)
            || border_side_is_visible(bl, border_styles.border_left_style);

        let rounded = has_border_radius(s);

        if rounded {
            let outer_radii = extract_outer_radii(s);
            let inner_radii = compute_inner_radii(&outer_radii, bt, br, bb, bl);

            if bg != Color::TRANSPARENT {
                self.emit(DisplayItem::Draw(DrawCommand::RoundedRect {
                    rect: padding_box,
                    radii: inner_radii,
                    color: bg,
                }));
            }
            if has_border {
                self.emit(DisplayItem::Draw(DrawCommand::RoundedBorderRing {
                    outer_rect: border_box,
                    outer_radii,
                    inner_rect: padding_box,
                    inner_radii,
                    color: stylo_color_to_color(&border_styles.clone_border_top_color()),
                }));
            }
        } else {
            if bg != Color::TRANSPARENT {
                self.emit(DisplayItem::Draw(DrawCommand::Rect {
                    rect: border_box,
                    color: bg,
                }));
            }
            if has_border {
                self.emit(DisplayItem::Draw(DrawCommand::Border {
                    rect: border_box,
                    widths: BorderWidths {
                        top: bt,
                        right: br,
                        bottom: bb,
                        left: bl,
                    },
                    colors: BorderColors {
                        top: stylo_color_to_color(&border_styles.clone_border_top_color()),
                        right: stylo_color_to_color(&border_styles.clone_border_right_color()),
                        bottom: stylo_color_to_color(&border_styles.clone_border_bottom_color()),
                        left: stylo_color_to_color(&border_styles.clone_border_left_color()),
                    },
                    styles: super::display_item::BorderStyles {
                        top: border_styles.border_top_style,
                        right: border_styles.border_right_style,
                        bottom: border_styles.border_bottom_style,
                        left: border_styles.border_left_style,
                    },
                }));
            }
        }
    }

    /// Push clip → scroll translate → paint children → pop scroll → pop clip.
    fn paint_clipped_children(
        &mut self,
        fragment: &Fragment,
        data: &BoxFragmentData,
        origin: Point,
        s: &ComputedValues,
    ) {
        let bt = data.border.top;
        let br = data.border.right;
        let bb = data.border.bottom;
        let bl = data.border.left;
        let border_box = Rect::from_origin_size(origin, fragment.size);
        let padding_box = border_box.inset(bt, br, bb, bl);

        let needs_clip = data.overflow_x.clips() || data.overflow_y.clips();

        if needs_clip {
            if has_border_radius(s) {
                let outer_radii = extract_outer_radii(s);
                let inner_radii = compute_inner_radii(&outer_radii, bt, br, bb, bl);
                self.emit(DisplayItem::PushRoundedClip(
                    super::display_item::RoundedClipData {
                        rect: padding_box,
                        radii: inner_radii,
                    },
                ));
            } else {
                self.emit(DisplayItem::PushClip(ClipData { rect: padding_box }));
            }
        }

        // User-scrollable nodes always get a tagged scroll transform —
        // even at offset zero — so the compositor can override it without
        // waiting for the view thread to repaint.
        // overflow:hidden clips but never gets a scroll transform.
        let is_user_scrollable =
            data.overflow_x.is_user_scrollable() || data.overflow_y.is_user_scrollable();

        if is_user_scrollable {
            let scroll_offset = fragment
                .dom_node
                .map(|id| self.scroll_offsets.offset(id))
                .unwrap_or(Offset::ZERO);
            self.emit(DisplayItem::PushTransform(
                super::display_item::TransformData {
                    translate_x: -scroll_offset.dx,
                    translate_y: -scroll_offset.dy,
                    scroll_node: fragment.dom_node,
                },
            ));
        }

        self.paint_children(&data.children, origin);

        if is_user_scrollable {
            self.emit(DisplayItem::PopTransform);
        }

        if needs_clip {
            if has_border_radius(s) {
                self.emit(DisplayItem::PopRoundedClip);
            } else {
                self.emit(DisplayItem::PopClip);
            }
        }
    }

    fn paint_unstyled_children(&mut self, data: &BoxFragmentData, origin: Point, size: Size) {
        let needs_clip = data.overflow_x.clips() || data.overflow_y.clips();

        if needs_clip {
            self.emit(DisplayItem::PushClip(ClipData {
                rect: Rect::from_origin_size(origin, size),
            }));
        }

        self.paint_children(&data.children, origin);

        if needs_clip {
            self.emit(DisplayItem::PopClip);
        }
    }

    fn paint_outline(&mut self, s: &ComputedValues, border_box: Rect) {
        let ow = s.get_outline().outline_width.0.to_f32_px();
        let os = resolve_outline_style(s);
        if ow > 0.0 && border_side_is_visible(ow, os) {
            let oc = stylo_color_to_color(&s.get_outline().clone_outline_color());
            let offset = s.get_outline().outline_offset.to_f32_px();
            let radii = extract_outer_radii(s);
            self.emit(DisplayItem::Draw(DrawCommand::Outline {
                rect: border_box,
                radii,
                width: ow,
                offset,
                color: oc,
            }));
        }
    }

    /// Chrome: `BoxFragmentPainter::PaintChildren()` + `PaintLayerPainter`.
    fn paint_children(&mut self, children: &[ChildFragment], parent_origin: Point) {
        let mut negative_z: Vec<(i32, usize)> = Vec::new();
        let mut normal: Vec<usize> = Vec::new();
        let mut positioned_zero: Vec<usize> = Vec::new();
        let mut positive_z: Vec<(i32, usize)> = Vec::new();

        for (i, child) in children.iter().enumerate() {
            let z = child.fragment.style.as_ref().and_then(|s| z_index(s));
            let is_positioned =
                child.fragment.style.as_ref().is_some_and(|s| {
                    s.clone_position() != style::computed_values::position::T::Static
                });

            match z {
                Some(z) if z < 0 => negative_z.push((z, i)),
                Some(z) if z > 0 => positive_z.push((z, i)),
                Some(_) => positioned_zero.push(i),
                None if is_positioned => positioned_zero.push(i),
                None => normal.push(i),
            }
        }

        negative_z.sort_by_key(|&(z, _)| z);
        positive_z.sort_by_key(|&(z, _)| z);

        for &(_, idx) in &negative_z {
            self.paint_child(&children[idx], parent_origin);
        }
        for &idx in &normal {
            self.paint_child(&children[idx], parent_origin);
        }
        for &idx in &positioned_zero {
            self.paint_child(&children[idx], parent_origin);
        }
        for &(_, idx) in &positive_z {
            self.paint_child(&children[idx], parent_origin);
        }
    }

    /// Chrome: `TextFragmentPainter::Paint()`.
    fn paint_text(
        &mut self,
        fragment: &Fragment,
        data: &crate::layout::fragment::TextFragmentData,
        origin: Point,
    ) {
        if data.shaped_runs.is_empty() {
            return;
        }

        self.emit(DisplayItem::Draw(DrawCommand::Text {
            x: origin.x,
            y: origin.y,
            runs: data.shaped_runs.clone(),
        }));

        if let Some(style) = &fragment.style {
            self.paint_text_decorations(style, fragment.size.width, origin, data.baseline);
        }
    }

    fn paint_text_decorations(
        &mut self,
        style: &ComputedValues,
        text_width: f32,
        origin: Point,
        baseline: f32,
    ) {
        use style::values::specified::text::TextDecorationLine;
        let td = style.clone_text_decoration_line();
        if td.is_empty() {
            return;
        }

        let dec_color = {
            let dec_c = style.clone_text_decoration_color();
            let text_c = style.clone_color();
            abs_color_to_color(&dec_c.resolve_to_absolute(&text_c))
        };
        let thickness = 1.0_f32;
        let baseline_y = origin.y + baseline;

        if td.contains(TextDecorationLine::UNDERLINE) {
            self.emit(DisplayItem::Draw(DrawCommand::Line {
                x0: origin.x,
                y0: baseline_y + 2.0,
                x1: origin.x + text_width,
                y1: baseline_y + 2.0,
                width: thickness,
                color: dec_color,
            }));
        }
        if td.contains(TextDecorationLine::OVERLINE) {
            self.emit(DisplayItem::Draw(DrawCommand::Line {
                x0: origin.x,
                y0: origin.y,
                x1: origin.x + text_width,
                y1: origin.y,
                width: thickness,
                color: dec_color,
            }));
        }
        if td.contains(TextDecorationLine::LINE_THROUGH) {
            let mid_y = origin.y + baseline * 0.5;
            self.emit(DisplayItem::Draw(DrawCommand::Line {
                x0: origin.x,
                y0: mid_y,
                x1: origin.x + text_width,
                y1: mid_y,
                width: thickness,
                color: dec_color,
            }));
        }
    }

    fn paint_line(&mut self, data: &LineFragmentData, origin: Point) {
        self.paint_children(&data.children, origin);
    }
}

#[cfg(test)]
fn paint_no_scroll(root: &Fragment, viewport: Size) -> super::DisplayList {
    Painter::new(&ScrollOffsets::new()).paint(root, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::{BoxFragmentData, Fragment, OverflowClip, PhysicalInsets};
    use std::sync::Arc;

    fn make_unstyled_box(w: f32, h: f32) -> Arc<Fragment> {
        Fragment::new_box(Size::new(w, h), BoxFragmentData::default())
    }

    fn make_styled_box_initial(w: f32, h: f32) -> Arc<Fragment> {
        Fragment::new_box_styled(
            Size::new(w, h),
            BoxFragmentData::default(),
            crate::styling::initial_values_arc().clone(),
            None,
        )
    }

    fn viewport() -> Size {
        Size::new(800.0, 600.0)
    }

    #[test]
    fn empty_fragment_tree_produces_viewport_bg() {
        let root = Fragment::new_box(Size::new(800.0, 600.0), BoxFragmentData::default());
        let list = paint_no_scroll(&root, viewport());
        assert!(!list.is_empty());
        assert!(list.items()[0].is_draw());
    }

    #[test]
    fn styled_box_with_initial_values() {
        let root = make_styled_box_initial(800.0, 600.0);
        let list = paint_no_scroll(&root, viewport());
        assert!(!list.is_empty());
    }

    #[test]
    fn unstyled_box_only_viewport_bg() {
        let root = make_unstyled_box(800.0, 600.0);
        let list = paint_no_scroll(&root, viewport());
        let draw_count = list.items().iter().filter(|i| i.is_draw()).count();
        assert_eq!(
            draw_count, 1,
            "unstyled box should only emit viewport background"
        );
    }

    #[test]
    fn border_not_emitted_when_style_is_none() {
        let root = Fragment::new_box_styled(
            Size::new(200.0, 100.0),
            BoxFragmentData {
                border: PhysicalInsets {
                    top: 2.0,
                    right: 0.0,
                    bottom: 0.0,
                    left: 0.0,
                },
                ..Default::default()
            },
            crate::styling::initial_values_arc(),
            None,
        );
        let _list = paint_no_scroll(&root, viewport());
    }

    #[test]
    fn overflow_hidden_emits_clip() {
        let root = Arc::new(Fragment {
            size: Size::new(200.0, 100.0),
            kind: FragmentKind::Box(BoxFragmentData {
                overflow_x: OverflowClip::Hidden,
                overflow_y: OverflowClip::Hidden,
                ..Default::default()
            }),
            style: None,
            dom_node: None,
        });
        let list = paint_no_scroll(&root, viewport());
        assert!(
            list.items()
                .iter()
                .any(|item| matches!(item, DisplayItem::PushClip(_)))
        );
        assert!(
            list.items()
                .iter()
                .any(|item| matches!(item, DisplayItem::PopClip))
        );
    }

    #[test]
    fn opacity_not_emitted_when_one() {
        let root = make_styled_box_initial(200.0, 100.0);
        let list = paint_no_scroll(&root, viewport());
        assert!(
            !list
                .items()
                .iter()
                .any(|item| matches!(item, DisplayItem::PushOpacity(_)))
        );
    }

    #[test]
    fn nested_children_paint_without_panic() {
        use crate::layout::fragment::ChildFragment;

        let child = make_styled_box_initial(100.0, 50.0);
        let root = Arc::new(Fragment {
            size: Size::new(400.0, 300.0),
            kind: FragmentKind::Box(BoxFragmentData {
                children: vec![ChildFragment {
                    offset: Point::new(20.0, 30.0),
                    fragment: child,
                }],
                ..Default::default()
            }),
            style: None,
            dom_node: None,
        });
        let _list = paint_no_scroll(&root, viewport());
    }

    #[test]
    fn text_with_empty_runs_does_not_emit() {
        use crate::layout::fragment::{ChildFragment, TextFragmentData};

        let text_frag = Arc::new(Fragment {
            size: Size::new(80.0, 20.0),
            kind: FragmentKind::Text(TextFragmentData {
                text_range: 0..5,
                baseline: 16.0,
                text: Some(Arc::from("Hello")),
                shaped_runs: Vec::new(),
            }),
            style: Some(crate::styling::initial_values_arc().clone()),
            dom_node: None,
        });
        let root = Arc::new(Fragment {
            size: Size::new(200.0, 20.0),
            kind: FragmentKind::Box(BoxFragmentData {
                children: vec![ChildFragment {
                    offset: Point::ZERO,
                    fragment: text_frag,
                }],
                ..Default::default()
            }),
            style: None,
            dom_node: None,
        });
        let list = paint_no_scroll(&root, viewport());
        assert!(
            !list
                .items()
                .iter()
                .any(|item| matches!(item, DisplayItem::Draw(DrawCommand::Text { .. })))
        );
    }

    #[test]
    fn multiple_children_paint_without_panic() {
        use crate::layout::fragment::ChildFragment;

        let a = make_unstyled_box(50.0, 50.0);
        let b = make_unstyled_box(50.0, 50.0);
        let c = make_unstyled_box(50.0, 50.0);
        let root = Arc::new(Fragment {
            size: Size::new(200.0, 200.0),
            kind: FragmentKind::Box(BoxFragmentData {
                children: vec![
                    ChildFragment {
                        offset: Point::new(0.0, 0.0),
                        fragment: a,
                    },
                    ChildFragment {
                        offset: Point::new(50.0, 0.0),
                        fragment: b,
                    },
                    ChildFragment {
                        offset: Point::new(100.0, 0.0),
                        fragment: c,
                    },
                ],
                ..Default::default()
            }),
            style: None,
            dom_node: None,
        });
        let list = paint_no_scroll(&root, viewport());
        assert!(!list.is_empty());
    }

    #[test]
    fn scroll_offset_emits_transform() {
        use crate::layout::fragment::ChildFragment;

        let child = make_unstyled_box(100.0, 800.0);
        let root = Arc::new(Fragment {
            size: Size::new(200.0, 200.0),
            kind: FragmentKind::Box(BoxFragmentData {
                children: vec![ChildFragment {
                    offset: Point::ZERO,
                    fragment: child,
                }],
                overflow_y: OverflowClip::Scroll,
                ..Default::default()
            }),
            style: Some(crate::styling::initial_values_arc().clone()),
            dom_node: Some(5),
        });

        let mut offsets = ScrollOffsets::new();
        offsets.set_offset(5, Offset::new(0.0, 150.0));

        let list = Painter::new(&offsets).paint(&root, viewport());
        let has_transform = list
            .items()
            .iter()
            .any(|item| matches!(item, DisplayItem::PushTransform(t) if t.translate_y == -150.0));
        assert!(
            has_transform,
            "scrolled node should emit negative translate"
        );
    }
}
