//! Compositor frame — what the GPU receives each vsync.
//!
//! Chrome: `viz::CompositorFrame` + `viz::SolidColorDrawQuad`.

use std::sync::Arc;

use kozan_primitives::color::Color;
use kozan_primitives::geometry::Rect;

use crate::paint::DisplayList;
use crate::scroll::ScrollOffsets;

/// Chrome: `viz::SolidColorDrawQuad` — a rendering primitive.
///
/// Layer types produce these generically. The renderer draws them
/// without knowing what layer type created them.
#[derive(Debug, Clone, Copy)]
pub struct FrameQuad {
    pub rect: Rect,
    pub clip: Option<Rect>,
    pub color: Color,
    pub radius: f32,
    pub opacity: f32,
}

/// Chrome: `viz::CompositorFrame`.
pub struct CompositorFrame {
    pub display_list: Arc<DisplayList>,
    pub scroll_offsets: ScrollOffsets,
    pub quads: Vec<FrameQuad>,
}
