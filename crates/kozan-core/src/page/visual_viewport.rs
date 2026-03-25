//! Visual viewport — post-rendering magnification.
//!
//! Chrome: `VisualViewport` (`blink/core/frame/visual_viewport.h`).
//! Pinch zoom + scroll offset. Purely compositor transform — no layout impact.

/// Chrome: `VisualViewport` — post-rendering magnification.
/// Pinch zoom scale + scroll offset. Does NOT cause layout — purely compositor transform.
pub(crate) struct VisualViewport {
    /// Pinch zoom level (1.0 = no zoom).
    scale: f64,
    /// Horizontal scroll within the layout viewport (CSS pixels).
    offset_x: f64,
    /// Vertical scroll within the layout viewport (CSS pixels).
    offset_y: f64,
}

#[allow(dead_code)] // Platform reads these for compositor pinch-zoom transform.
impl VisualViewport {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    #[inline]
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    #[inline]
    #[must_use]
    pub fn offset(&self) -> (f64, f64) {
        (self.offset_x, self.offset_y)
    }

    pub fn set_offset(&mut self, x: f64, y: f64) {
        self.offset_x = x;
        self.offset_y = y;
    }

    /// Visible width — the layout viewport divided by pinch zoom.
    #[inline]
    #[must_use]
    pub fn width(&self, layout_width: f64) -> f64 {
        layout_width / self.scale
    }

    /// Visible height — the layout viewport divided by pinch zoom.
    #[inline]
    #[must_use]
    pub fn height(&self, layout_height: f64) -> f64 {
        layout_height / self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let vv = VisualViewport::new();
        assert_eq!(vv.scale(), 1.0);
        assert_eq!(vv.offset(), (0.0, 0.0));
    }

    #[test]
    fn pinch_zoom_narrows_visible_area() {
        let vv = VisualViewport { scale: 2.0, ..VisualViewport::new() };
        assert_eq!(vv.width(1920.0), 960.0);
        assert_eq!(vv.height(1080.0), 540.0);
    }

    #[test]
    fn offset_round_trip() {
        let mut vv = VisualViewport::new();
        vv.set_offset(100.0, 200.0);
        assert_eq!(vv.offset(), (100.0, 200.0));
    }
}
