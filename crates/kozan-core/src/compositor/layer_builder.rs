//! Layer tree builder — fragment tree → LayerTree in a single O(n) pass.
//!
//! Chrome: `PaintArtifactCompositor` decides which paint chunks become
//! separate layers. Same struct-with-shared-state pattern as `Painter`.

use kozan_primitives::geometry::Rect;
use kozan_primitives::transform::Transform3D;

use crate::layout::fragment::{Fragment, FragmentKind};
use crate::scroll::ScrollOffsets;

use super::layer::Layer;
use super::layer_tree::LayerTree;

/// Walks the fragment tree and builds a LayerTree.
///
/// Chrome: `PaintArtifactCompositor`. Holds shared state (the tree
/// being built, scroll offsets). Methods take only what varies per call.
pub(crate) struct LayerTreeBuilder<'a> {
    tree: LayerTree,
    scroll_offsets: &'a ScrollOffsets,
}

impl<'a> LayerTreeBuilder<'a> {
    pub fn new(scroll_offsets: &'a ScrollOffsets) -> Self {
        Self {
            tree: LayerTree::new(),
            scroll_offsets,
        }
    }

    /// Build the full layer tree. Consumes the builder.
    pub fn build(mut self, root: &Fragment) -> LayerTree {
        let root_id = self.build_layer(root);
        self.tree.set_root(root_id);
        self.tree
    }

    fn build_layer(&mut self, fragment: &Fragment) -> super::layer::LayerId {
        let bounds = Rect::new(0.0, 0.0, fragment.size.width, fragment.size.height);
        let mut layer = Layer::new(fragment.dom_node, bounds);

        let FragmentKind::Box(ref data) = fragment.kind else {
            return self.tree.push(layer);
        };

        let is_scrollable =
            data.overflow_x.is_user_scrollable() || data.overflow_y.is_user_scrollable();
        layer.is_scrollable = is_scrollable;

        if is_scrollable {
            if let Some(dom_id) = fragment.dom_node {
                layer.scroll_offset = self.scroll_offsets.offset(dom_id);
            }
            if data.overflow_x.clips() || data.overflow_y.clips() {
                layer.clip = Some(Rect::new(
                    data.border.left,
                    data.border.top,
                    (fragment.size.width - data.border.left - data.border.right).max(0.0),
                    (fragment.size.height - data.border.top - data.border.bottom).max(0.0),
                ));
            }
        }

        if let Some(style) = &fragment.style {
            layer.opacity = style.get_effects().clone_opacity();
        }

        let layer_id = self.tree.push(layer);
        self.collect_scrollable_children(
            &data.children,
            layer_id,
            kozan_primitives::geometry::Point::ZERO,
        );

        layer_id
    }

    /// Promote scrollable descendants to child layers, skipping through
    /// non-scrollable boxes with accumulated offset.
    fn collect_scrollable_children(
        &mut self,
        children: &[crate::layout::fragment::ChildFragment],
        parent_layer: super::layer::LayerId,
        parent_offset: kozan_primitives::geometry::Point,
    ) {
        for child in children {
            let child_pos = kozan_primitives::geometry::Point::new(
                parent_offset.x + child.offset.x,
                parent_offset.y + child.offset.y,
            );

            let is_scrollable = matches!(
                child.fragment.kind,
                FragmentKind::Box(ref d) if d.overflow_x.is_user_scrollable() || d.overflow_y.is_user_scrollable()
            );

            if is_scrollable {
                let child_id = self.build_layer(&child.fragment);
                self.tree.layer_mut(child_id).transform =
                    Transform3D::translate(child_pos.x, child_pos.y, 0.0);
                self.tree.layer_mut(parent_layer).children.push(child_id);
            } else if let FragmentKind::Box(ref data) = child.fragment.kind {
                self.collect_scrollable_children(&data.children, parent_layer, child_pos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::*;
    use kozan_primitives::geometry::{Offset, Point, Size};
    use std::sync::Arc;

    fn make_box(w: f32, h: f32, dom: Option<u32>, children: Vec<ChildFragment>) -> Arc<Fragment> {
        Arc::new(Fragment {
            size: Size::new(w, h),
            kind: FragmentKind::Box(BoxFragmentData {
                children,
                ..Default::default()
            }),
            style: None,
            dom_node: dom,
        })
    }

    fn make_scrollable(w: f32, h: f32, dom: u32, children: Vec<ChildFragment>) -> Arc<Fragment> {
        Arc::new(Fragment {
            size: Size::new(w, h),
            kind: FragmentKind::Box(BoxFragmentData {
                children,
                overflow_y: OverflowClip::Scroll,
                ..Default::default()
            }),
            style: None,
            dom_node: Some(dom),
        })
    }

    #[test]
    fn root_only() {
        let root = make_box(800.0, 600.0, Some(0), vec![]);
        let tree = LayerTreeBuilder::new(&ScrollOffsets::new()).build(&root);
        assert_eq!(tree.len(), 1);
        assert!(tree.root().is_some());
    }

    #[test]
    fn scrollable_child_gets_own_layer() {
        let child = make_scrollable(200.0, 300.0, 5, vec![]);
        let root = make_box(
            800.0,
            600.0,
            Some(0),
            vec![ChildFragment {
                offset: Point::new(50.0, 50.0),
                fragment: child,
            }],
        );

        let tree = LayerTreeBuilder::new(&ScrollOffsets::new()).build(&root);
        assert_eq!(tree.len(), 2);

        let root_layer = tree.layer(tree.root().expect("root"));
        assert_eq!(root_layer.children.len(), 1);

        let child_layer = tree.layer(root_layer.children[0]);
        assert!(child_layer.is_scrollable);
        assert_eq!(child_layer.dom_node, Some(5));
    }

    #[test]
    fn non_scrollable_child_stays_in_parent() {
        let child = make_box(200.0, 300.0, Some(5), vec![]);
        let root = make_box(
            800.0,
            600.0,
            Some(0),
            vec![ChildFragment {
                offset: Point::new(50.0, 50.0),
                fragment: child,
            }],
        );

        let tree = LayerTreeBuilder::new(&ScrollOffsets::new()).build(&root);
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn scroll_offset_transferred_to_layer() {
        let child = make_scrollable(200.0, 300.0, 5, vec![]);
        let root = make_box(
            800.0,
            600.0,
            Some(0),
            vec![ChildFragment {
                offset: Point::ZERO,
                fragment: child,
            }],
        );

        let mut offsets = ScrollOffsets::new();
        offsets.set_offset(5, Offset::new(0.0, 150.0));

        let tree = LayerTreeBuilder::new(&offsets).build(&root);
        let root_layer = tree.layer(tree.root().expect("root"));
        let child_layer = tree.layer(root_layer.children[0]);
        assert_eq!(child_layer.scroll_offset, Offset::new(0.0, 150.0));
    }

    #[test]
    fn child_layer_transform_has_offset() {
        let child = make_scrollable(200.0, 300.0, 5, vec![]);
        let root = make_box(
            800.0,
            600.0,
            Some(0),
            vec![ChildFragment {
                offset: Point::new(30.0, 40.0),
                fragment: child,
            }],
        );

        let tree = LayerTreeBuilder::new(&ScrollOffsets::new()).build(&root);
        let root_layer = tree.layer(tree.root().expect("root"));
        let child_layer = tree.layer(root_layer.children[0]);

        let p = child_layer.transform.transform_point(Point::ZERO);
        assert_eq!(p.x, 30.0);
        assert_eq!(p.y, 40.0);
    }
}
