//! Viewport — the rendering surface dimensions and scale factor.
//!
//! Chrome equivalent: `ScreenInfo::device_scale_factor` + `VisualProperties`
//! (`visible_viewport_size`). Updated by `FrameWidget` on resize and
//! scale-factor-change. Read by layout for viewport units (vw, vh).

/// The rendering viewport — physical dimensions and DPI scale factor.
///
/// Physical pixels = logical pixels × `scale_factor`.
///
/// Used by:
/// - Layout engine: viewport units (`vw`, `vh`, `vmin`, `vmax`)
/// - Style resolution: `ResolveContext` gets `viewport_width`/`viewport_height`
/// - Media queries: `@media (min-width: ...)`
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    /// Width in physical pixels.
    width: u32,
    /// Height in physical pixels.
    height: u32,
    /// DPI scale factor (1.0 = standard, 2.0 = Retina/HiDPI).
    scale_factor: f64,
    /// Chrome: `PageScaleConstraints::page_zoom_factor` — Ctrl+/- zoom.
    /// Shrinks the layout viewport so content reflows at the new size.
    page_zoom_factor: f64,
}

impl Viewport {
    /// Create a viewport with the given dimensions and scale factor.
    #[must_use]
    pub fn new(width: u32, height: u32, scale_factor: f64) -> Self {
        Self {
            width,
            height,
            scale_factor,
            page_zoom_factor: 1.0,
        }
    }

    /// Width in physical pixels.
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in physical pixels.
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// DPI scale factor.
    #[inline]
    #[must_use]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Width in logical (CSS-like) pixels.
    ///
    /// This is what layout uses for viewport units.
    /// Page zoom shrinks the layout viewport, causing content reflow.
    #[inline]
    #[must_use]
    pub fn logical_width(&self) -> f64 {
        self.width as f64 / (self.scale_factor * self.page_zoom_factor)
    }

    /// Height in logical (CSS-like) pixels.
    #[inline]
    #[must_use]
    pub fn logical_height(&self) -> f64 {
        self.height as f64 / (self.scale_factor * self.page_zoom_factor)
    }

    /// Combined scale for physical ↔ logical conversion.
    #[inline]
    #[must_use]
    pub fn effective_scale(&self) -> f64 {
        self.scale_factor * self.page_zoom_factor
    }

    /// Chrome: `PageScaleConstraints::page_zoom_factor`.
    #[inline]
    #[must_use]
    pub fn page_zoom_factor(&self) -> f64 {
        self.page_zoom_factor
    }

    /// Update the page zoom factor (Ctrl+/- zoom).
    pub fn set_page_zoom_factor(&mut self, factor: f64) {
        self.page_zoom_factor = factor;
    }

    /// Update the physical dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Update the DPI scale factor.
    pub fn set_scale_factor(&mut self, factor: f64) {
        self.scale_factor = factor;
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            scale_factor: 1.0,
            page_zoom_factor: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_viewport() {
        let vp = Viewport::default();
        assert_eq!(vp.width(), 0);
        assert_eq!(vp.height(), 0);
        assert_eq!(vp.scale_factor(), 1.0);
        assert_eq!(vp.page_zoom_factor(), 1.0);
    }

    #[test]
    fn construction_and_accessors() {
        let vp = Viewport::new(1920, 1080, 2.0);
        assert_eq!(vp.width(), 1920);
        assert_eq!(vp.height(), 1080);
        assert_eq!(vp.scale_factor(), 2.0);
    }

    #[test]
    fn logical_dimensions() {
        let vp = Viewport::new(3840, 2160, 2.0);
        assert_eq!(vp.logical_width(), 1920.0);
        assert_eq!(vp.logical_height(), 1080.0);
    }

    #[test]
    fn logical_dimensions_at_1x() {
        let vp = Viewport::new(1920, 1080, 1.0);
        assert_eq!(vp.logical_width(), 1920.0);
        assert_eq!(vp.logical_height(), 1080.0);
    }

    #[test]
    fn resize() {
        let mut vp = Viewport::new(800, 600, 1.0);
        vp.resize(1920, 1080);
        assert_eq!(vp.width(), 1920);
        assert_eq!(vp.height(), 1080);
    }

    #[test]
    fn set_scale_factor() {
        let mut vp = Viewport::new(1920, 1080, 1.0);
        vp.set_scale_factor(1.5);
        assert_eq!(vp.scale_factor(), 1.5);
        assert!((vp.logical_width() - 1280.0).abs() < 0.01);
    }

    #[test]
    fn page_zoom_shrinks_logical_viewport() {
        let mut vp = Viewport::new(1920, 1080, 1.0);
        vp.set_page_zoom_factor(2.0);
        assert_eq!(vp.page_zoom_factor(), 2.0);
        assert_eq!(vp.logical_width(), 960.0);
        assert_eq!(vp.logical_height(), 540.0);
    }

    #[test]
    fn effective_scale_combines_dpi_and_page_zoom() {
        let mut vp = Viewport::new(3840, 2160, 2.0);
        vp.set_page_zoom_factor(1.5);
        assert_eq!(vp.effective_scale(), 3.0);
        assert_eq!(vp.logical_width(), 1280.0);
        assert_eq!(vp.logical_height(), 720.0);
    }
}
