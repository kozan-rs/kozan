//! Compositor layer — GPU-side representation of a painted surface.
//!
//! Chrome: `cc::Layer` + `cc::PictureLayerImpl`.
//!
//! Each layer is an independently compositable surface. The compositor
//! can change a layer's transform, opacity, or clip without repainting
//! its content — enabling vsync-rate scroll and compositor-driven animations.

use kozan_primitives::geometry::{Offset, Rect};
use kozan_primitives::transform::Transform3D;

/// Opaque handle into the layer tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub(super) u32);

/// A compositable surface in the layer tree.
///
/// Chrome: `cc::PictureLayerImpl` — owns rasterized tiles, transform,
/// clip, and opacity. The compositor mutates transform/opacity/scroll
/// per-frame without touching the main thread.
pub struct Layer {
    /// DOM node that owns this layer (for scroll offset lookup).
    pub dom_node: Option<u32>,
    /// Bounds in the parent layer's coordinate space.
    pub bounds: Rect,
    /// Full 4x4 transform relative to parent.
    /// Chrome: part of the property tree. The compositor uses the
    /// inverse for hit testing (screen point → layer local space).
    pub transform: Transform3D,
    /// Current scroll offset — the compositor mutates this directly.
    pub scroll_offset: Offset,
    /// Layer opacity (1.0 = fully opaque). Compositor-animatable.
    pub opacity: f32,
    /// Clip rect in parent coordinates. `None` = no clip.
    pub clip: Option<Rect>,
    /// Child layers (front-to-back order).
    pub children: Vec<LayerId>,
    /// Whether this layer corresponds to a user-scrollable container.
    pub is_scrollable: bool,
}

impl Layer {
    pub(super) fn new(dom_node: Option<u32>, bounds: Rect) -> Self {
        Self {
            dom_node,
            bounds,
            transform: Transform3D::IDENTITY,
            scroll_offset: Offset::ZERO,
            opacity: 1.0,
            clip: None,
            children: Vec::new(),
            is_scrollable: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_primitives::geometry::Rect;

    #[test]
    fn new_layer_defaults() {
        let layer = Layer::new(Some(5), Rect::new(0.0, 0.0, 200.0, 100.0));
        assert_eq!(layer.dom_node, Some(5));
        assert!(layer.transform.is_identity());
        assert_eq!(layer.opacity, 1.0);
        assert!(layer.clip.is_none());
        assert!(layer.children.is_empty());
        assert!(!layer.is_scrollable);
    }
}
