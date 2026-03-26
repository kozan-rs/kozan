//! Compositor frame — what the GPU receives each vsync.
//!
//! Chrome: `viz::CompositorFrame` + `viz::SolidColorDrawQuad`.

use std::sync::Arc;

use kozan_primitives::color::Color;
use kozan_primitives::geometry::Rect;

use crate::paint::DisplayList;
use crate::scroll::ScrollOffsets;

/// Which coordinate space a quad's rect is expressed in.
///
/// Chrome: each `cc::LayerImpl` carries a `screen_space_transform`.
/// Content layers use content_scale (device DPI × page zoom), while
/// compositor overlays (scrollbars) use device_scale only so they
/// stay a fixed physical size regardless of page zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuadSpace {
    /// Content coordinate space — scaled by device DPI × page zoom.
    #[default]
    Content,
    /// Screen coordinate space — scaled by device DPI only.
    /// Used for compositor overlays (scrollbars) that must maintain
    /// a fixed device-pixel size regardless of page zoom.
    Screen,
}

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
    /// Which transform the renderer should apply for this quad.
    pub space: QuadSpace,
}

/// Chrome: `viz::CompositorFrame`.
pub struct CompositorFrame {
    pub display_list: Arc<DisplayList>,
    pub scroll_offsets: ScrollOffsets,
    pub quads: Vec<FrameQuad>,
}
