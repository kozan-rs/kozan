//! Layout compliance tests — CSS behavior verified against known-correct output.
//!
//! Each test builds a DOM tree with `LayoutHarness`, runs the full pipeline
//! (style recalc → layout), and asserts computed geometry.

// ── Test harness ──

#[cfg(test)]
mod harness {
    use std::sync::Arc;

    use crate::dom::document::Document;
    use crate::dom::handle::Handle;
    use crate::dom::traits::HasHandle;
    use crate::html::HtmlDivElement;
    use crate::layout::context::LayoutContext;
    use crate::layout::fragment::Fragment;
    use crate::layout::inline::FontSystem;
    use crate::styling::builder::StyleAccess;

    /// A child's resolved position and size after a layout pass.
    #[derive(Debug, Clone, Copy)]
    pub struct ChildLayout {
        pub x: f32,
        pub y: f32,
        pub width: f32,
        pub height: f32,
    }

    /// The result of a layout pass — wraps the root fragment with query helpers.
    pub struct LayoutTestResult {
        pub root_fragment: Arc<Fragment>,
    }

    impl LayoutTestResult {
        /// Get the Nth direct child's position and size.
        pub fn child(&self, index: usize) -> Option<ChildLayout> {
            let box_data = self.root_fragment.try_as_box()?;
            let child = box_data.children.get(index)?;
            Some(ChildLayout {
                x: child.offset.x,
                y: child.offset.y,
                width: child.fragment.size.width,
                height: child.fragment.size.height,
            })
        }

        /// Root fragment size as `(width, height)`.
        pub fn root_size(&self) -> (f32, f32) {
            (self.root_fragment.size.width, self.root_fragment.size.height)
        }

        /// Number of direct children of the root box.
        pub fn child_count(&self) -> usize {
            self.root_fragment
                .try_as_box()
                .map(|b| b.children.len())
                .unwrap_or(0)
        }

    }

    /// An element reference returned from harness builder methods.
    #[derive(Clone, Copy)]
    pub struct HarnessNode {
        pub(crate) handle: Handle,
    }

    /// The layout test harness.
    ///
    /// Owns a `Document` and `FontSystem`. Create nodes with `div()`, `span()`,
    /// then call `layout(root)` to run the full pipeline (style recalc → layout).
    pub struct LayoutTestHarness {
        doc: Document,
        font_system: FontSystem,
        viewport_w: f32,
        viewport_h: f32,
    }

    impl LayoutTestHarness {
        pub fn new(width: f32, height: f32) -> Self {
            Self {
                doc: Document::new(),
                font_system: FontSystem::new(),
                viewport_w: width,
                viewport_h: height,
            }
        }

        pub fn div(&mut self, style_fn: impl FnOnce(&mut StyleAccess), children: &[HarnessNode]) -> HarnessNode {
            let el = self.doc.create::<HtmlDivElement>();
            style_fn(&mut el.handle().style());
            for child in children {
                el.handle().append(child.handle);
            }
            self.doc.root().append(el);
            HarnessNode { handle: el.handle() }
        }

        pub fn layout(&mut self, root: HarnessNode) -> LayoutTestResult {
            self.doc.recalc_styles();

            let root_index = self.doc.root_index();
            let ctx = LayoutContext { text_measurer: &self.font_system };
            let result = self.doc.resolve_layout(
                root_index,
                Some(self.viewport_w),
                Some(self.viewport_h),
                &ctx,
            );
            let doc_root_frag = result.fragment;

            let target_index = root.handle.raw().index();
            let child_frag = doc_root_frag
                .try_as_box()
                .and_then(|b| {
                    b.children
                        .iter()
                        .find(|c| c.fragment.dom_node == Some(target_index))
                })
                .map(|c| Arc::clone(&c.fragment));

            LayoutTestResult { root_fragment: child_frag.unwrap_or(doc_root_frag) }
        }
    }

    pub use crate::styling::units::px;

    /// Create a `Size` (width/height) from a px value.
    pub fn px_size(v: f32) -> style::values::specified::Size {
        use style::values::specified::{LengthPercentage, Size};
        use style::values::generics::NonNegative;
        Size::LengthPercentage(NonNegative(
            LengthPercentage::Length(
                style::values::specified::length::NoCalcLength::Absolute(
                    style::values::specified::length::AbsoluteLength::Px(v),
                ),
            ),
        ))
    }
}

// ── assert_layout! macro ──

/// Assert a child's layout. Tolerance of 0.5 px for float comparison.
///
/// ```ignore
/// assert_layout!(result, 0 => { x: 0.0, y: 0.0, w: 100.0, h: 50.0 });
/// ```
#[cfg(test)]
macro_rules! assert_layout {
    ($result:expr, $index:expr => { x: $x:expr, y: $y:expr, w: $w:expr, h: $h:expr }) => {{
        let child = $result.child($index).unwrap_or_else(|| {
            panic!(
                "child {} not found (total children: {})",
                $index,
                $result.child_count()
            )
        });
        assert!(
            (child.x - $x).abs() < 0.5,
            "child {} x: expected {}, got {} (diff {})",
            $index, $x, child.x, (child.x - $x).abs()
        );
        assert!(
            (child.y - $y).abs() < 0.5,
            "child {} y: expected {}, got {} (diff {})",
            $index, $y, child.y, (child.y - $y).abs()
        );
        assert!(
            (child.width - $w).abs() < 0.5,
            "child {} width: expected {}, got {} (diff {})",
            $index, $w, child.width, (child.width - $w).abs()
        );
        assert!(
            (child.height - $h).abs() < 0.5,
            "child {} height: expected {}, got {} (diff {})",
            $index, $h, child.height, (child.height - $h).abs()
        );
    }};
}

// ── Block flow ──

#[cfg(test)]
mod block_flow {
    use super::harness::*;

    #[test]
    fn block_children_stack_vertically() {
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let c0 = h.div(|s| { s.width(px_size(200.0)); s.height(px_size(50.0)); }, &[]);
        let c1 = h.div(|s| { s.width(px_size(200.0)); s.height(px_size(80.0)); }, &[]);
        let root = h.div(|s| { s.width(px_size(800.0)); }, &[c0, c1]);
        let result = h.layout(root);

        // Block children stack: first at y=0, second directly below it.
        assert_layout!(result, 0 => { x: 0.0, y: 0.0, w: 200.0, h: 50.0 });
        assert_layout!(result, 1 => { x: 0.0, y: 50.0, w: 200.0, h: 80.0 });
    }

    #[test]
    fn explicit_size_respected() {
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let root = h.div(|s| { s.width(px_size(300.0)); s.height(px_size(200.0)); }, &[]);
        let result = h.layout(root);
        let (w, hv) = result.root_size();
        assert!((w - 300.0).abs() < 0.5, "expected w=300, got {w}");
        assert!((hv - 200.0).abs() < 0.5, "expected h=200, got {hv}");
    }
}

// ── Flexbox row ──

#[cfg(test)]
mod flex_row {
    use super::harness::*;
    use style::values::specified::box_::Display;

    #[test]
    fn flex_row_places_children_horizontally() {
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let c0 = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(50.0)); }, &[]);
        let c1 = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(50.0)); }, &[]);
        let c2 = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(50.0)); }, &[]);
        let root = h.div(
            |s| {
                s.display(Display::Flex);
                s.width(px_size(300.0));
                s.height(px_size(50.0));
            },
            &[c0, c1, c2],
        );
        let result = h.layout(root);

        // Flex row: children are placed side by side along the main axis.
        assert_layout!(result, 0 => { x: 0.0,   y: 0.0, w: 100.0, h: 50.0 });
        assert_layout!(result, 1 => { x: 100.0, y: 0.0, w: 100.0, h: 50.0 });
        assert_layout!(result, 2 => { x: 200.0, y: 0.0, w: 100.0, h: 50.0 });
    }

    #[test]
    fn flex_row_child_count() {
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let c0 = h.div(|s| { s.width(px_size(50.0)); s.height(px_size(50.0)); }, &[]);
        let c1 = h.div(|s| { s.width(px_size(50.0)); s.height(px_size(50.0)); }, &[]);
        let root = h.div(|s| { s.display(Display::Flex); s.width(px_size(200.0)); }, &[c0, c1]);
        let result = h.layout(root);
        assert_eq!(result.child_count(), 2);
    }

    #[test]
    fn flex_row_stretch_height() {
        // align-items: stretch (the default) makes children fill the container height.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let c0 = h.div(|s| { s.width(px_size(100.0)); }, &[]);
        let c1 = h.div(|s| { s.width(px_size(100.0)); }, &[]);
        let root = h.div(
            |s| {
                s.display(Display::Flex);
                s.width(px_size(300.0));
                s.height(px_size(80.0));
                s.align_items_stretch();
            },
            &[c0, c1],
        );
        let result = h.layout(root);

        assert_layout!(result, 0 => { x: 0.0,   y: 0.0, w: 100.0, h: 80.0 });
        assert_layout!(result, 1 => { x: 100.0, y: 0.0, w: 100.0, h: 80.0 });
    }

    #[test]
    fn flex_row_justify_center() {
        // justify-content: center shifts children to the middle of the main axis.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let c0 = h.div(|s| { s.width(px_size(50.0)); s.height(px_size(50.0)); }, &[]);
        let root = h.div(
            |s| {
                s.display(Display::Flex);
                s.width(px_size(200.0));
                s.height(px_size(50.0));
                s.justify_content_center();
            },
            &[c0],
        );
        let result = h.layout(root);

        // One 50px child inside a 200px container centered → x = 75.
        let c = result.child(0).expect("child 0 must exist");
        assert!(
            (c.x - 75.0).abs() < 0.5,
            "expected child centered at x=75, got x={}",
            c.x,
        );
    }

    #[test]
    fn flex_row_justify_space_between() {
        // justify-content: space-between pushes first to start, last to end.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let c0 = h.div(|s| { s.width(px_size(50.0)); s.height(px_size(50.0)); }, &[]);
        let c1 = h.div(|s| { s.width(px_size(50.0)); s.height(px_size(50.0)); }, &[]);
        let root = h.div(
            |s| {
                s.display(Display::Flex);
                s.width(px_size(200.0));
                s.height(px_size(50.0));
                s.justify_content_between();
            },
            &[c0, c1],
        );
        let result = h.layout(root);

        assert_layout!(result, 0 => { x: 0.0,   y: 0.0, w: 50.0, h: 50.0 });
        assert_layout!(result, 1 => { x: 150.0, y: 0.0, w: 50.0, h: 50.0 });
    }

    #[test]
    fn flex_row_align_items_center() {
        // align-items: center places children at the cross-axis midpoint.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let c0 = h.div(|s| { s.width(px_size(50.0)); s.height(px_size(20.0)); }, &[]);
        let root = h.div(
            |s| {
                s.display(Display::Flex);
                s.width(px_size(200.0));
                s.height(px_size(100.0));
                s.align_items_center();
            },
            &[c0],
        );
        let result = h.layout(root);

        // 20px child in 100px container: y = (100 - 20) / 2 = 40.
        let c = result.child(0).expect("child 0 must exist");
        assert!(
            (c.y - 40.0).abs() < 0.5,
            "expected child centered at y=40, got y={}",
            c.y,
        );
    }

    #[test]
    fn flex_row_mixed_grow_and_fixed() {
        // One fixed child (100px) and one flex-grow:1 child split remaining space.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let fixed = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(50.0)); }, &[]);
        let grow = h.div(|s| { s.flex_grow(1.0); s.height(px_size(50.0)); }, &[]);
        let root = h.div(
            |s| {
                s.display(Display::Flex);
                s.width(px_size(300.0));
                s.height(px_size(50.0));
            },
            &[fixed, grow],
        );
        let result = h.layout(root);

        // Fixed child stays at 100px; growing child fills the remaining 200px.
        assert_layout!(result, 0 => { x: 0.0,   y: 0.0, w: 100.0, h: 50.0 });
        assert_layout!(result, 1 => { x: 100.0, y: 0.0, w: 200.0, h: 50.0 });
    }
}

// ── Flexbox column ──

#[cfg(test)]
mod flex_column {
    use super::harness::*;

    #[test]
    fn flex_column_places_children_vertically() {
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let c0 = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(40.0)); }, &[]);
        let c1 = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(60.0)); }, &[]);
        let root = h.div(
            |s| {
                s.flex_col();
                s.width(px_size(200.0));
                s.height(px_size(200.0));
            },
            &[c0, c1],
        );
        let result = h.layout(root);

        // Flex column: children stack vertically along the main axis.
        assert_layout!(result, 0 => { x: 0.0, y: 0.0,  w: 100.0, h: 40.0 });
        assert_layout!(result, 1 => { x: 0.0, y: 40.0, w: 100.0, h: 60.0 });
    }

    #[test]
    fn flex_column_stretch_width() {
        // align-items: stretch (the default in a column) fills the container width.
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let c0 = h.div(|s| { s.height(px_size(50.0)); }, &[]);
        let c1 = h.div(|s| { s.height(px_size(50.0)); }, &[]);
        let root = h.div(
            |s| {
                s.flex_col();
                s.align_items_stretch();
                s.width(px_size(300.0));
                s.height(px_size(200.0));
            },
            &[c0, c1],
        );
        let result = h.layout(root);

        // Children stretch to fill the 300px container width.
        assert_layout!(result, 0 => { x: 0.0, y: 0.0,  w: 300.0, h: 50.0 });
        assert_layout!(result, 1 => { x: 0.0, y: 50.0, w: 300.0, h: 50.0 });
    }

    #[test]
    fn flex_column_grow_fills_height() {
        // flex-grow: 1 on a child causes it to consume remaining column height.
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let fixed = h.div(|s| { s.height(px_size(60.0)); s.width(px_size(100.0)); }, &[]);
        let grow = h.div(|s| { s.flex_grow(1.0); s.width(px_size(100.0)); }, &[]);
        let root = h.div(
            |s| {
                s.flex_col();
                s.width(px_size(200.0));
                s.height(px_size(200.0));
            },
            &[fixed, grow],
        );
        let result = h.layout(root);

        // Fixed child: 60px tall; growing child fills remaining 140px.
        assert_layout!(result, 0 => { x: 0.0, y: 0.0,  w: 100.0, h: 60.0  });
        assert_layout!(result, 1 => { x: 0.0, y: 60.0, w: 100.0, h: 140.0 });
    }

    #[test]
    fn flex_column_percentage_height() {
        // A child with percentage height resolves against the definite container height.
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        use style::values::specified::{LengthPercentage, Size};
        use style::values::generics::NonNegative;
        use style::values::computed::Percentage;
        let half_height = Size::LengthPercentage(NonNegative(
            LengthPercentage::Percentage(Percentage(0.5)),
        ));
        let c0 = h.div(|s| { s.height(half_height); s.width(px_size(100.0)); }, &[]);
        let root = h.div(
            |s| {
                s.flex_col();
                s.width(px_size(200.0));
                s.height(px_size(200.0));
            },
            &[c0],
        );
        let result = h.layout(root);

        // 50% of 200px = 100px.
        assert_layout!(result, 0 => { x: 0.0, y: 0.0, w: 100.0, h: 100.0 });
    }
}

// ── Grid ──

#[cfg(test)]
mod grid {
    use super::harness::*;
    use style::values::specified::box_::Display;

    #[test]
    fn grid_two_children_both_in_container() {
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let c0 = h.div(|s| { s.height(px_size(50.0)); }, &[]);
        let c1 = h.div(|s| { s.height(px_size(50.0)); }, &[]);
        // Without explicit column template, grid auto-places each item in its own row.
        let root = h.div(
            |s| {
                s.display(Display::Grid);
                s.width(px_size(400.0));
                s.height(px_size(200.0));
            },
            &[c0, c1],
        );
        let result = h.layout(root);
        assert_eq!(result.child_count(), 2);
        let (w, _) = result.root_size();
        assert!((w - 400.0).abs() < 0.5, "container width: {w}");
    }
}

// ── Margin auto ──

#[cfg(test)]
mod margin_auto {
    use super::harness::*;
    use crate::styling::units::auto;

    #[test]
    fn margin_auto_centers_horizontally() {
        // margin-left: auto; margin-right: auto on a block with explicit width centers it.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let child = h.div(
            |s| {
                s.width(px_size(200.0));
                s.height(px_size(50.0));
                s.margin_left(auto());
                s.margin_right(auto());
            },
            &[],
        );
        let root = h.div(|s| { s.width(px_size(400.0)); }, &[child]);
        let result = h.layout(root);

        // 200px child in 400px parent: left margin = (400 - 200) / 2 = 100.
        let c = result.child(0).expect("child 0 must exist");
        assert!(
            (c.x - 100.0).abs() < 0.5,
            "expected child centered at x=100, got x={}",
            c.x,
        );
    }

    #[test]
    fn margin_left_auto_pushes_right() {
        // margin-left: auto on a block with explicit width pushes it to the right edge.
        let mut h = LayoutTestHarness::new(1280.0, 800.0);
        let child = h.div(
            |s| {
                s.width(px_size(200.0));
                s.height(px_size(50.0));
                s.margin_left(auto());
            },
            &[],
        );
        let root = h.div(|s| { s.width(px_size(400.0)); }, &[child]);
        let result = h.layout(root);

        // All free space (200px) collapses into the left margin → child at x=200.
        let c = result.child(0).expect("child 0 must exist");
        assert!(
            (c.x - 200.0).abs() < 0.5,
            "expected child pushed to x=200, got x={}",
            c.x,
        );
    }

}

// ── Nested blocks ──

#[cfg(test)]
mod nested_blocks {
    use super::harness::*;

    #[test]
    fn nested_blocks_respect_explicit_sizes() {
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let inner = h.div(|s| { s.width(px_size(80.0)); s.height(px_size(50.0)); }, &[]);
        let root = h.div(|s| { s.width(px_size(200.0)); s.height(px_size(150.0)); }, &[inner]);
        let result = h.layout(root);

        let (root_w, _) = result.root_size();
        assert!((root_w - 200.0).abs() < 0.5, "outer width: expected 200, got {root_w}");

        let inner_c = result.child(0).expect("inner child must exist");
        assert!((inner_c.width - 80.0).abs() < 0.5, "inner width: expected 80, got {}", inner_c.width);
    }
}

// ── Padding ──

#[cfg(test)]
mod padding {
    use super::harness::*;

    #[test]
    fn padding_adds_to_content_box() {
        // Content-box sizing (default): padding is added on top of declared width.
        // width: 100px; padding: 10px → fragment width = 100 + 10 + 10 = 120px.
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let root = h.div(
            |s| {
                s.width(px_size(100.0));
                s.height(px_size(50.0));
                s.padding(px(10.0));
            },
            &[],
        );
        let result = h.layout(root);
        let (w, hv) = result.root_size();
        assert!(
            (w - 120.0).abs() < 0.5,
            "padding must add to fragment width: expected 120, got {w}",
        );
        assert!(
            (hv - 70.0).abs() < 0.5,
            "padding must add to fragment height: expected 70, got {hv}",
        );
    }
}

// ── Margins between siblings ──

#[cfg(test)]
mod margins_siblings {
    use super::harness::*;

    #[test]
    fn margin_bottom_pushes_next_sibling() {
        let mut h = LayoutTestHarness::new(800.0, 600.0);
        let c0 = h.div(
            |s| {
                s.width(px_size(100.0));
                s.height(px_size(40.0));
                s.margin_bottom(px(20.0));
            },
            &[],
        );
        let c1 = h.div(|s| { s.width(px_size(100.0)); s.height(px_size(30.0)); }, &[]);
        let root = h.div(|s| { s.width(px_size(200.0)); }, &[c0, c1]);
        let result = h.layout(root);

        assert_layout!(result, 0 => { x: 0.0, y: 0.0, w: 100.0, h: 40.0 });
        assert_layout!(result, 1 => { x: 0.0, y: 60.0, w: 100.0, h: 30.0 });
    }
}
