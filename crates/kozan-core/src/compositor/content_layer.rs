//! Content layer — rasterized page content.
//!
//! Chrome: `cc::PictureLayerImpl`.

use std::any::Any;

use smallvec::SmallVec;

use super::frame::FrameQuad;
use super::layer::{LayerContent, QuadContext};

/// Content is in the display list, not produced by the layer.
pub(crate) struct ContentLayer;

impl LayerContent for ContentLayer {
    fn append_quads(&self, _ctx: &QuadContext) -> SmallVec<[FrameQuad; 2]> {
        SmallVec::new()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
