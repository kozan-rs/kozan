//! Compositor layer — GPU-side representation of a painted surface.
//!
//! Chrome: `cc::LayerImpl` with virtual `AppendQuads()`.

use std::any::Any;

use kozan_primitives::geometry::{Offset, Point, Rect};
use kozan_primitives::transform::Transform3D;
use smallvec::SmallVec;

use super::frame::FrameQuad;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub(super) u32);

/// Chrome: `SharedQuadState` — accumulated state for quad production.
pub(crate) struct QuadContext {
    pub _origin: Point,
    pub container_rect: Rect,
    /// Page zoom factor — overlay layers use this to convert content-space
    /// geometry to screen-space so they render at fixed device-pixel size.
    pub page_zoom: f32,
}

/// Chrome: `LayerImpl` virtual interface.
pub(crate) trait LayerContent: Send + Sync {
    fn append_quads(&self, ctx: &QuadContext) -> SmallVec<[FrameQuad; 2]>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Chrome: `cc::LayerImpl` base fields.
pub struct Layer {
    pub(crate) dom_node: Option<u32>,
    pub(crate) bounds: Rect,
    pub(crate) transform: Transform3D,
    pub(crate) scroll_offset: Offset,
    pub(crate) opacity: f32,
    pub(crate) clip: Option<Rect>,
    pub(crate) children: Vec<LayerId>,
    pub(crate) is_scrollable: bool,
    /// Stacking contexts block scroll hit-testing — events on a
    /// `position: fixed` overlay must not reach scroll containers behind it.
    /// Chrome: `cc::LayerImpl::is_scroll_blocking`.
    pub(crate) is_stacking_context: bool,
    pub(crate) content: Box<dyn LayerContent>,
}

impl Layer {
    pub(crate) fn new(
        dom_node: Option<u32>,
        bounds: Rect,
        content: Box<dyn LayerContent>,
    ) -> Self {
        Self {
            dom_node,
            bounds,
            transform: Transform3D::IDENTITY,
            scroll_offset: Offset::ZERO,
            opacity: 1.0,
            clip: None,
            children: Vec::new(),
            is_scrollable: false,
            is_stacking_context: false,
            content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compositor::content_layer::ContentLayer;

    #[test]
    fn new_layer_defaults() {
        let layer = Layer::new(
            Some(5),
            Rect::new(0.0, 0.0, 200.0, 100.0),
            Box::new(ContentLayer),
        );
        assert_eq!(layer.dom_node, Some(5));
        assert!(layer.transform.is_identity());
        assert_eq!(layer.opacity, 1.0);
        assert!(layer.clip.is_none());
        assert!(layer.children.is_empty());
        assert!(!layer.is_scrollable);
    }
}
