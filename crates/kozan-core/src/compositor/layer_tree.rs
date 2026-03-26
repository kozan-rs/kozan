//! Layer tree — arena of compositor layers.
//!
//! Chrome: `cc::LayerTreeImpl`.

use std::collections::HashMap;

use kozan_primitives::arena::Storage;

use crate::scroll::Orientation;

use super::layer::{Layer, LayerId};

/// Chrome: `LayerTreeImpl::element_id_to_scrollbar_layer_ids_`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ScrollbarLayerIds {
    pub vertical: Option<LayerId>,
    pub horizontal: Option<LayerId>,
}

/// Chrome: `cc::LayerTreeImpl`.
pub struct LayerTree {
    layers: Storage<Layer>,
    next_id: u32,
    root: Option<LayerId>,
    scrollbar_map: HashMap<u32, ScrollbarLayerIds>,
}

impl Default for LayerTree {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerTree {
    #[must_use]
    pub fn new() -> Self {
        Self {
            layers: Storage::new(),
            next_id: 0,
            root: None,
            scrollbar_map: HashMap::new(),
        }
    }

    pub fn push(&mut self, layer: Layer) -> LayerId {
        let id = LayerId(self.next_id);
        self.layers.set(self.next_id, layer);
        self.next_id += 1;
        id
    }

    #[must_use]
    pub fn root(&self) -> Option<LayerId> {
        self.root
    }

    pub fn set_root(&mut self, id: LayerId) {
        self.root = Some(id);
    }

    #[must_use]
    pub fn layer(&self, id: LayerId) -> &Layer {
        self.layers
            .get(id.0)
            .expect("LayerId points to valid layer")
    }

    pub fn layer_mut(&mut self, id: LayerId) -> &mut Layer {
        self.layers
            .get_mut(id.0)
            .expect("LayerId points to valid layer")
    }

    #[must_use]
    pub fn len(&self) -> u32 {
        self.next_id
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.next_id == 0
    }

    #[must_use]
    pub fn layer_for_dom_node(&self, dom_id: u32) -> Option<LayerId> {
        self.layers
            .iter()
            .find(|(_, l)| l.dom_node == Some(dom_id))
            .map(|(i, _)| LayerId(i))
    }

    /// Chrome: `LayerTreeImpl::RegisterScrollbar()`.
    pub(crate) fn register_scrollbar(
        &mut self,
        scroll_element_id: u32,
        orientation: Orientation,
        layer_id: LayerId,
    ) {
        let entry = self.scrollbar_map.entry(scroll_element_id).or_default();
        match orientation {
            Orientation::Vertical => entry.vertical = Some(layer_id),
            Orientation::Horizontal => entry.horizontal = Some(layer_id),
        }
    }

    /// Chrome: `LayerTreeImpl::ScrollbarsFor()`.
    pub(crate) fn scrollbar_ids(&self, scroll_element_id: u32) -> Option<&ScrollbarLayerIds> {
        self.scrollbar_map.get(&scroll_element_id)
    }

    /// Snapshot for iteration while mutating layers.
    pub(crate) fn scrollbar_entries(&self) -> Vec<(u32, ScrollbarLayerIds)> {
        self.scrollbar_map
            .iter()
            .map(|(&id, &ids)| (id, ids))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::content_layer::ContentLayer;
    use kozan_primitives::geometry::Rect;

    fn content_layer(dom: Option<u32>, w: f32, h: f32) -> Layer {
        Layer::new(dom, Rect::new(0.0, 0.0, w, h), Box::new(ContentLayer))
    }

    #[test]
    fn push_and_retrieve() {
        let mut tree = LayerTree::new();
        let id = tree.push(content_layer(Some(1), 800.0, 600.0));
        tree.set_root(id);

        assert_eq!(tree.root(), Some(id));
        assert_eq!(tree.layer(id).dom_node, Some(1));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn parent_child() {
        let mut tree = LayerTree::new();
        let parent = tree.push(content_layer(Some(1), 800.0, 600.0));
        let child = tree.push(content_layer(Some(5), 200.0, 300.0));
        tree.layer_mut(parent).children.push(child);
        tree.set_root(parent);

        assert_eq!(tree.layer(parent).children.len(), 1);
        assert_eq!(tree.layer(parent).children[0], child);
    }

    #[test]
    fn find_by_dom_node() {
        let mut tree = LayerTree::new();
        tree.push(content_layer(Some(1), 800.0, 600.0));
        let id = tree.push(content_layer(Some(42), 100.0, 100.0));

        assert_eq!(tree.layer_for_dom_node(42), Some(id));
        assert_eq!(tree.layer_for_dom_node(99), None);
    }

    #[test]
    fn scrollbar_registration() {
        let mut tree = LayerTree::new();
        let v = tree.push(content_layer(None, 8.0, 400.0));
        let h = tree.push(content_layer(None, 400.0, 8.0));
        tree.register_scrollbar(5, Orientation::Vertical, v);
        tree.register_scrollbar(5, Orientation::Horizontal, h);

        let ids = tree.scrollbar_ids(5).expect("registered");
        assert_eq!(ids.vertical, Some(v));
        assert_eq!(ids.horizontal, Some(h));
        assert!(tree.scrollbar_ids(99).is_none());
    }
}
