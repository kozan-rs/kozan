//! Layer tree builder — fragment tree → LayerTree in a single O(n) pass.
//!
//! Chrome: `PaintArtifactCompositor`.

use kozan_primitives::geometry::Rect;
use kozan_primitives::transform::Transform3D;

use crate::layout::fragment::{Fragment, FragmentKind};
use crate::scroll::scrollbar::Orientation;
use crate::scroll::ScrollOffsets;

use super::content_layer::ContentLayer;
use super::layer::Layer;
use super::layer_tree::LayerTree;
use super::scrollbar_layer::ScrollbarLayer;

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

    pub fn build(mut self, root: &Fragment) -> LayerTree {
        let root_id = self.build_layer(root);
        self.tree.set_root(root_id);
        self.tree
    }

    fn build_layer(
        &mut self,
        fragment: &Fragment,
    ) -> super::layer::LayerId {
        let bounds = Rect::new(0.0, 0.0, fragment.size.width, fragment.size.height);
        let mut layer = Layer::new(fragment.dom_node, bounds, Box::new(ContentLayer));

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

        // Chrome: create scrollbar sibling layers for scrollable containers.
        if is_scrollable {
            if let Some(dom_id) = fragment.dom_node {
                self.create_scrollbar_layers(dom_id, fragment, data, layer_id);
            }
        }

        self.collect_scrollable_children(
            &data.children,
            layer_id,
            kozan_primitives::geometry::Point::ZERO,
        );

        layer_id
    }

    /// Chrome: scrollbar layers are siblings, attached as children of the
    /// scroll container layer so they inherit its position but don't
    /// scroll with content (they have `is_scrollable: false`).
    fn create_scrollbar_layers(
        &mut self,
        dom_id: u32,
        fragment: &Fragment,
        data: &crate::layout::fragment::BoxFragmentData,
        parent_layer_id: super::layer::LayerId,
    ) {
        let container_w =
            (fragment.size.width - data.border.left - data.border.right).max(0.0);
        let container_h =
            (fragment.size.height - data.border.top - data.border.bottom).max(0.0);

        if data.overflow_y.is_user_scrollable() {
            let sb = ScrollbarLayer::new(dom_id, Orientation::Vertical);
            let bounds = Rect::new(0.0, 0.0, container_w, container_h);
            let mut layer = Layer::new(None, bounds, Box::new(sb));
            layer.transform = Transform3D::translate(data.border.left, data.border.top, 0.0);
            let sb_id = self.tree.push(layer);
            self.tree.layer_mut(parent_layer_id).children.push(sb_id);
            self.tree
                .register_scrollbar(dom_id, Orientation::Vertical, sb_id);
        }

        if data.overflow_x.is_user_scrollable() {
            let sb = ScrollbarLayer::new(dom_id, Orientation::Horizontal);
            let bounds = Rect::new(0.0, 0.0, container_w, container_h);
            let mut layer = Layer::new(None, bounds, Box::new(sb));
            layer.transform = Transform3D::translate(data.border.left, data.border.top, 0.0);
            let sb_id = self.tree.push(layer);
            self.tree.layer_mut(parent_layer_id).children.push(sb_id);
            self.tree
                .register_scrollbar(dom_id, Orientation::Horizontal, sb_id);
        }
    }

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
        let root_layer = tree.layer(tree.root().expect("root"));
        assert!(!root_layer.children.is_empty());

        let scroll_child = root_layer.children[0];
        assert!(tree.layer(scroll_child).is_scrollable);
        assert_eq!(tree.layer(scroll_child).dom_node, Some(5));

        // Scrollbar layer is a child of the scrollable layer.
        let scroll_layer = tree.layer(scroll_child);
        assert!(
            !scroll_layer.children.is_empty(),
            "scrollable layer should have scrollbar children"
        );
    }

    #[test]
    fn scrollable_element_gets_scrollbar_layers() {
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

        let tree = LayerTreeBuilder::new(&ScrollOffsets::new()).build(&root);
        let ids = tree.scrollbar_ids(5).expect("scrollbar registered");
        assert!(ids.vertical.is_some());
    }

    #[test]
    fn non_scrollable_has_no_scrollbar() {
        let child = make_box(200.0, 300.0, Some(5), vec![]);
        let root = make_box(
            800.0,
            600.0,
            Some(0),
            vec![ChildFragment {
                offset: Point::ZERO,
                fragment: child,
            }],
        );

        let tree = LayerTreeBuilder::new(&ScrollOffsets::new()).build(&root);
        assert!(tree.scrollbar_ids(5).is_none());
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
        let scroll_child = root_layer.children[0];
        assert_eq!(
            tree.layer(scroll_child).scroll_offset,
            Offset::new(0.0, 150.0)
        );
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
        let scroll_child = root_layer.children[0];

        let p = tree
            .layer(scroll_child)
            .transform
            .transform_point(Point::ZERO);
        assert_eq!(p.x, 30.0);
        assert_eq!(p.y, 40.0);
    }
}
