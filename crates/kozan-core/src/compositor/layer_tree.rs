//! Layer tree — arena of compositor layers.
//!
//! Chrome: `cc::LayerTreeImpl` — the compositor's view of the page.
//! Rebuilt from the fragment tree after each paint on the view thread,
//! then committed to the compositor on the main thread.

use kozan_primitives::arena::Storage;

use super::layer::{Layer, LayerId};

/// Arena of layers indexed by LayerId.
///
/// Chrome: `LayerTreeImpl` owns all layers. The compositor reads/mutates
/// layer properties (transform, opacity, scroll offset) without touching
/// the view thread.
pub struct LayerTree {
    layers: Storage<Layer>,
    next_id: u32,
    root: Option<LayerId>,
}

impl LayerTree {
    pub fn new() -> Self {
        Self {
            layers: Storage::new(),
            next_id: 0,
            root: None,
        }
    }

    pub fn push(&mut self, layer: Layer) -> LayerId {
        let id = LayerId(self.next_id);
        self.layers.set(self.next_id, layer);
        self.next_id += 1;
        id
    }

    pub fn root(&self) -> Option<LayerId> {
        self.root
    }

    pub fn set_root(&mut self, id: LayerId) {
        self.root = Some(id);
    }

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

    pub fn len(&self) -> u32 {
        self.next_id
    }

    pub fn layer_for_dom_node(&self, dom_id: u32) -> Option<LayerId> {
        self.layers
            .iter()
            .find(|(_, l)| l.dom_node == Some(dom_id))
            .map(|(i, _)| LayerId(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_primitives::geometry::Rect;

    #[test]
    fn push_and_retrieve() {
        let mut tree = LayerTree::new();
        let id = tree.push(Layer::new(Some(1), Rect::new(0.0, 0.0, 800.0, 600.0)));
        tree.set_root(id);

        assert_eq!(tree.root(), Some(id));
        assert_eq!(tree.layer(id).dom_node, Some(1));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn parent_child() {
        let mut tree = LayerTree::new();
        let parent = tree.push(Layer::new(Some(1), Rect::new(0.0, 0.0, 800.0, 600.0)));
        let child = tree.push(Layer::new(Some(5), Rect::new(10.0, 10.0, 200.0, 300.0)));
        tree.layer_mut(parent).children.push(child);
        tree.set_root(parent);

        assert_eq!(tree.layer(parent).children.len(), 1);
        assert_eq!(tree.layer(parent).children[0], child);
    }

    #[test]
    fn find_by_dom_node() {
        let mut tree = LayerTree::new();
        tree.push(Layer::new(Some(1), Rect::new(0.0, 0.0, 800.0, 600.0)));
        let id = tree.push(Layer::new(Some(42), Rect::new(0.0, 0.0, 100.0, 100.0)));

        assert_eq!(tree.layer_for_dom_node(42), Some(id));
        assert_eq!(tree.layer_for_dom_node(99), None);
    }
}
