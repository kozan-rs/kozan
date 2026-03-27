//! Shadow state for canvas draw operations.
//!
//! Chrome equivalent: shadow fields on `CanvasRenderingContext2DState`.

use kozan_primitives::color::Color;

/// Shadow properties applied to draw operations.
///
/// Baked into `ResolvedPaint` at record time. A shadow is only visible
/// when `color` is non-transparent AND (`blur > 0` OR offsets are non-zero).
#[derive(Clone, Copy, Debug)]
pub struct ShadowState {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub color: Color,
}

impl ShadowState {
    pub fn is_visible(&self) -> bool {
        !self.color.is_transparent()
            && (self.blur > 0.0 || self.offset_x != 0.0 || self.offset_y != 0.0)
    }
}

impl Default for ShadowState {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            blur: 0.0,
            color: Color::TRANSPARENT,
        }
    }
}
