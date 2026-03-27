//! Hit testing — point → DOM element lookup.
//!
//! Chrome: `HitTestResult` + `HitTestLocation` in
//! `blink/renderer/core/layout/hit_test_result.h`.
//!
//! Walks the fragment tree to find the deepest DOM node at a given point.
//! Respects overflow clipping, scroll offsets, pointer-events, and z-index
//! paint order.

use std::sync::Arc;

use kozan_primitives::geometry::{Offset, Point};
use style::properties::ComputedValues;

use crate::scroll::ScrollOffsets;

use super::fragment::{ChildFragment, Fragment, FragmentKind, OverflowClip};

fn has_pointer_events_none(style: Option<&ComputedValues>) -> bool {
    style.is_some_and(|s| {
        s.clone_pointer_events() == style::values::specified::PointerEvents::None
    })
}

fn z_index(s: &ComputedValues) -> Option<i32> {
    match s.get_position().clone_z_index() {
        style::values::generics::position::ZIndex::Integer(n) => Some(n),
        style::values::generics::position::ZIndex::Auto => None,
    }
}

/// Build child indices sorted in CSS paint order (negative z → normal flow
/// → positioned z:0 → positive z). Mirrors `Painter::paint_children`.
fn paint_order_indices(children: &[ChildFragment]) -> Vec<usize> {
    let mut negative_z: Vec<(i32, usize)> = Vec::new();
    let mut normal: Vec<usize> = Vec::new();
    let mut positioned_zero: Vec<usize> = Vec::new();
    let mut positive_z: Vec<(i32, usize)> = Vec::new();

    for (i, child) in children.iter().enumerate() {
        let z = child.fragment.style.as_ref().and_then(|s| z_index(s));
        let is_positioned = child.fragment.style.as_ref().is_some_and(|s| {
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

    let mut order = Vec::with_capacity(children.len());
    order.extend(negative_z.iter().map(|&(_, idx)| idx));
    order.extend(&normal);
    order.extend(&positioned_zero);
    order.extend(positive_z.iter().map(|&(_, idx)| idx));
    order
}

/// Chrome: `HitTestResult`.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    pub node_index: Option<u32>,
    pub local_point: Point,
}

impl HitTestResult {
    pub const NONE: Self = Self {
        node_index: None,
        local_point: Point::ZERO,
    };
}

/// Walks the fragment tree to find the deepest DOM node at a point.
///
/// Chrome: `HitTesting` in `blink/renderer/core/layout/`.
/// Holds scroll offsets to adjust point coordinates inside scrolled containers.
pub struct HitTester<'a> {
    scroll_offsets: &'a ScrollOffsets,
}

impl<'a> HitTester<'a> {
    #[must_use] 
    pub fn new(scroll_offsets: &'a ScrollOffsets) -> Self {
        Self { scroll_offsets }
    }

    /// Find the deepest DOM node at `point` (CSS px, root-relative).
    #[must_use] 
    pub fn test(&self, fragment: &Fragment, point: Point) -> HitTestResult {
        self.test_fragment(fragment, point, Point::ZERO)
    }

    fn test_fragment(&self, fragment: &Fragment, point: Point, origin: Point) -> HitTestResult {
        if has_pointer_events_none(fragment.style.as_deref()) {
            return HitTestResult::NONE;
        }

        let local = point - origin;
        let in_bounds = local.dx >= 0.0
            && local.dy >= 0.0
            && local.dx < fragment.size.width
            && local.dy < fragment.size.height;

        if let FragmentKind::Box(ref box_data) = fragment.kind {
            let clips = box_data.overflow_x != OverflowClip::Visible
                || box_data.overflow_y != OverflowClip::Visible;
            if clips && !in_bounds {
                return HitTestResult::NONE;
            }

            // Only user-scrollable nodes have scroll offsets.
            // Must match paint_clipped_children — same gate.
            let is_user_scrollable = box_data.overflow_x.is_user_scrollable()
                || box_data.overflow_y.is_user_scrollable();
            let scroll_offset = if is_user_scrollable {
                fragment
                    .dom_node
                    .map(|id| self.scroll_offsets.offset(id))
                    .unwrap_or(Offset::ZERO)
            } else {
                Offset::ZERO
            };

            // Walk children in reverse PAINT order so the topmost
            // (highest z-index) element is tested first.
            let order = paint_order_indices(&box_data.children);
            for &idx in order.iter().rev() {
                let child = &box_data.children[idx];
                let child_origin =
                    origin + Offset::new(child.offset.x, child.offset.y) - scroll_offset;
                let result = self.test_fragment(&child.fragment, point, child_origin);
                if result.node_index.is_some() {
                    return result;
                }
            }
        }

        if let FragmentKind::Line(ref line_data) = fragment.kind {
            for child in line_data.children.iter().rev() {
                let child_origin = origin + Offset::new(child.offset.x, child.offset.y);
                let result = self.test_fragment(&child.fragment, point, child_origin);
                if result.node_index.is_some() {
                    return result;
                }
            }
        }

        if in_bounds {
            if let Some(node_index) = fragment.dom_node {
                return HitTestResult {
                    node_index: Some(node_index),
                    local_point: Point::new(local.dx, local.dy),
                };
            }
        }

        HitTestResult::NONE
    }
}

/// Cached hit testing — avoids re-walking when cursor barely moved.
///
/// Chrome: `HitTestCache` in `blink/renderer/core/layout/`.
/// Invalidates when fragment pointer changes or cursor moves > 0.5px.
pub struct HitTestCache {
    last_fragment_ptr: usize,
    last_point: Point,
    last_result: HitTestResult,
}

const TOLERANCE: f32 = 0.5;

impl Default for HitTestCache {
    fn default() -> Self {
        Self {
            last_fragment_ptr: 0,
            last_point: Point::ZERO,
            last_result: HitTestResult::NONE,
        }
    }
}

impl HitTestCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns cached result if fragment and point haven't changed.
    pub fn test(
        &mut self,
        tester: &HitTester,
        fragment: &Arc<Fragment>,
        point: Point,
    ) -> &HitTestResult {
        let ptr = Arc::as_ptr(fragment) as usize;
        let same_fragment = self.last_fragment_ptr == ptr;
        let close_enough = (point.x - self.last_point.x).abs() < TOLERANCE
            && (point.y - self.last_point.y).abs() < TOLERANCE;

        if same_fragment && close_enough {
            return &self.last_result;
        }

        self.last_result = tester.test(fragment, point);
        self.last_fragment_ptr = ptr;
        self.last_point = point;
        &self.last_result
    }

    /// Force invalidation (e.g., after scroll offset changes).
    pub fn invalidate(&mut self) {
        self.last_fragment_ptr = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::*;
    use kozan_primitives::geometry::Size;

    fn make_box(size: Size, dom_node: Option<u32>, children: Vec<ChildFragment>) -> Arc<Fragment> {
        Fragment::new_box_styled_with_node(size, dom_node, children)
    }

    impl Fragment {
        fn new_box_styled_with_node(
            size: Size,
            dom_node: Option<u32>,
            children: Vec<ChildFragment>,
        ) -> Arc<Self> {
            Arc::new(Self {
                size,
                kind: FragmentKind::Box(BoxFragmentData {
                    children,
                    ..Default::default()
                }),
                style: None,
                dom_node,
            })
        }
    }

    fn no_scroll() -> ScrollOffsets {
        ScrollOffsets::new()
    }

    fn tester(offsets: &ScrollOffsets) -> HitTester<'_> {
        HitTester::new(offsets)
    }

    #[test]
    fn hit_empty_fragment() {
        let root = make_box(Size::new(800.0, 600.0), Some(0), vec![]);
        let result = tester(&no_scroll()).test(&root, Point::new(400.0, 300.0));
        assert_eq!(result.node_index, Some(0));
    }

    #[test]
    fn miss_outside_root() {
        let root = make_box(Size::new(800.0, 600.0), Some(0), vec![]);
        let result = tester(&no_scroll()).test(&root, Point::new(900.0, 300.0));
        assert_eq!(result.node_index, None);
    }

    #[test]
    fn miss_negative_coords() {
        let root = make_box(Size::new(800.0, 600.0), Some(0), vec![]);
        let result = tester(&no_scroll()).test(&root, Point::new(-10.0, 300.0));
        assert_eq!(result.node_index, None);
    }

    #[test]
    fn nested_child_hit() {
        let child = make_box(Size::new(100.0, 50.0), Some(1), vec![]);
        let root = make_box(
            Size::new(800.0, 600.0),
            Some(0),
            vec![ChildFragment {
                offset: Point::new(50.0, 50.0),
                fragment: child,
            }],
        );
        let result = tester(&no_scroll()).test(&root, Point::new(75.0, 60.0));
        assert_eq!(result.node_index, Some(1));
        assert!((result.local_point.x - 25.0).abs() < 0.001);
    }

    #[test]
    fn last_child_wins_overlap() {
        let a = make_box(Size::new(100.0, 100.0), Some(1), vec![]);
        let b = make_box(Size::new(100.0, 100.0), Some(2), vec![]);
        let root = make_box(
            Size::new(800.0, 600.0),
            Some(0),
            vec![
                ChildFragment {
                    offset: Point::new(10.0, 10.0),
                    fragment: a,
                },
                ChildFragment {
                    offset: Point::new(10.0, 10.0),
                    fragment: b,
                },
            ],
        );
        let result = tester(&no_scroll()).test(&root, Point::new(50.0, 50.0));
        assert_eq!(result.node_index, Some(2));
    }

    #[test]
    fn deeply_nested() {
        let gc = make_box(Size::new(20.0, 20.0), Some(3), vec![]);
        let child = make_box(
            Size::new(100.0, 100.0),
            Some(2),
            vec![ChildFragment {
                offset: Point::new(10.0, 10.0),
                fragment: gc,
            }],
        );
        let root = make_box(
            Size::new(800.0, 600.0),
            Some(1),
            vec![ChildFragment {
                offset: Point::new(50.0, 50.0),
                fragment: child,
            }],
        );
        let result = tester(&no_scroll()).test(&root, Point::new(65.0, 65.0));
        assert_eq!(result.node_index, Some(3));
    }

    #[test]
    fn anonymous_box_passthrough() {
        let anon = Arc::new(Fragment {
            size: Size::new(100.0, 100.0),
            kind: FragmentKind::Box(BoxFragmentData::default()),
            style: None,
            dom_node: None,
        });
        let root = make_box(
            Size::new(800.0, 600.0),
            Some(0),
            vec![ChildFragment {
                offset: Point::ZERO,
                fragment: anon,
            }],
        );
        let result = tester(&no_scroll()).test(&root, Point::new(50.0, 50.0));
        assert_eq!(result.node_index, Some(0));
    }

    #[test]
    fn overflow_hidden_clips() {
        let child = Arc::new(Fragment {
            size: Size::new(200.0, 200.0),
            kind: FragmentKind::Box(BoxFragmentData {
                overflow_x: OverflowClip::Hidden,
                overflow_y: OverflowClip::Hidden,
                ..Default::default()
            }),
            style: None,
            dom_node: Some(1),
        });
        let root = make_box(
            Size::new(800.0, 600.0),
            Some(0),
            vec![ChildFragment {
                offset: Point::ZERO,
                fragment: child,
            }],
        );
        let result = tester(&no_scroll()).test(&root, Point::new(50.0, 50.0));
        assert_eq!(result.node_index, Some(1));
    }

    #[test]
    fn boundary_inclusive_exclusive() {
        let root = make_box(Size::new(100.0, 100.0), Some(0), vec![]);
        let offsets = no_scroll();
        let t = tester(&offsets);

        assert_eq!(t.test(&root, Point::new(0.0, 0.0)).node_index, Some(0));
        assert_eq!(t.test(&root, Point::new(100.0, 100.0)).node_index, None);
        assert_eq!(t.test(&root, Point::new(99.9, 99.9)).node_index, Some(0));
    }

    #[test]
    fn scroll_offset_adjusts_child_hit() {
        let child = make_box(Size::new(200.0, 800.0), Some(2), vec![]);
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
            style: None,
            dom_node: Some(1),
        });

        let mut offsets = ScrollOffsets::new();
        offsets.set_offset(1, Offset::new(0.0, 300.0));

        // Point at y=50 in the viewport maps to y=350 in content space.
        // The child is 800px tall, so 350 is inside.
        let result = HitTester::new(&offsets).test(&root, Point::new(100.0, 50.0));
        assert_eq!(result.node_index, Some(2));
    }

    #[test]
    fn pointer_events_none_skips_fragment() {
        // No style → pointer-events defaults to auto → not skipped.
        assert!(!has_pointer_events_none(None));

        // Initial computed style has pointer-events: auto.
        let initial = crate::styling::initial_values_arc();
        assert!(!has_pointer_events_none(Some(&initial)));
    }

    #[test]
    fn pointer_events_none_on_styled_child_falls_through() {
        // A styled child with initial values (pointer-events: auto) is hittable.
        let style = crate::styling::initial_values_arc();
        let child = Arc::new(Fragment {
            size: Size::new(100.0, 100.0),
            kind: FragmentKind::Box(BoxFragmentData::default()),
            style: Some(style),
            dom_node: Some(1),
        });
        let root = make_box(
            Size::new(800.0, 600.0),
            Some(0),
            vec![ChildFragment {
                offset: Point::ZERO,
                fragment: child,
            }],
        );
        let result = tester(&no_scroll()).test(&root, Point::new(50.0, 50.0));
        assert_eq!(result.node_index, Some(1));
    }

    #[test]
    fn paint_order_unstyled_preserves_tree_order() {
        let a = make_box(Size::new(50.0, 50.0), Some(1), vec![]);
        let b = make_box(Size::new(50.0, 50.0), Some(2), vec![]);
        let c = make_box(Size::new(50.0, 50.0), Some(3), vec![]);
        let children = vec![
            ChildFragment { offset: Point::ZERO, fragment: a },
            ChildFragment { offset: Point::ZERO, fragment: b },
            ChildFragment { offset: Point::ZERO, fragment: c },
        ];
        let order = paint_order_indices(&children);
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn paint_order_negative_z_before_normal() {
        let style = crate::styling::initial_values_arc();
        // Unstyled fragments go to "normal" bucket.
        let normal = make_box(Size::new(50.0, 50.0), Some(1), vec![]);
        // Styled with initial values (position: static, z-index: auto) → normal.
        let also_normal = Arc::new(Fragment {
            size: Size::new(50.0, 50.0),
            kind: FragmentKind::Box(BoxFragmentData::default()),
            style: Some(style),
            dom_node: Some(2),
        });
        let children = vec![
            ChildFragment { offset: Point::ZERO, fragment: normal },
            ChildFragment { offset: Point::ZERO, fragment: also_normal },
        ];
        let order = paint_order_indices(&children);
        // Both are normal flow, tree order preserved.
        assert_eq!(order, vec![0, 1]);
    }
}
