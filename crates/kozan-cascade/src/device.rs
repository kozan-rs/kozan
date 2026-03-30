//! Device context — viewport dimensions, media features, and font metrics.
//!
//! Used by the `Stylist` to evaluate `@media` queries at stylesheet index time.
//! Covers Media Queries Level 4 + Level 5 features.

/// Pointer type capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Fine,
    Coarse,
    None,
}

/// Hover capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverCapability {
    Hover,
    None,
}

/// Color scheme preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// Media type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaType {
    All,
    Screen,
    Print,
}

/// Color gamut capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorGamut {
    Srgb,
    P3,
    Rec2020,
}

/// Dynamic range capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicRange {
    Standard,
    High,
}

/// Forced colors mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForcedColors {
    None,
    Active,
}

/// Display update frequency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    None,
    Slow,
    Fast,
}

/// Scripting availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scripting {
    None,
    InitialOnly,
    Enabled,
}

/// Font metrics provider trait — DOM layer implements this to supply
/// font measurements needed for unit resolution (em, ex, ch, ic, cap).
pub trait FontMetricsProvider: Send + Sync {
    fn x_height(&self, family: &str, size: f32) -> f32;
    fn zero_advance_width(&self, family: &str, size: f32) -> f32;
    fn cap_height(&self, family: &str, size: f32) -> f32;
    fn ic_width(&self, family: &str, size: f32) -> f32;
}

/// Device capabilities and viewport dimensions for `@media` evaluation.
///
/// Covers Media Queries Level 4 + Level 5 features used by real-world CSS.
pub struct Device {
    pub media_type: MediaType,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub device_pixel_ratio: f32,
    pub resolution_dpi: f32,
    pub color_bits: u8,
    pub monochrome_bits: u8,
    pub pointer: Pointer,
    pub any_pointer: Pointer,
    pub hover: HoverCapability,
    pub any_hover: HoverCapability,
    pub color_gamut: ColorGamut,
    pub dynamic_range: DynamicRange,
    pub forced_colors: ForcedColors,
    pub update: Update,
    pub scripting: Scripting,
    pub prefers_color_scheme: ColorScheme,
    pub prefers_reduced_motion: bool,
    pub prefers_reduced_transparency: bool,
    pub prefers_contrast: bool,
    pub inverted_colors: bool,
    pub grid: bool,
    pub font_metrics: Option<Box<dyn FontMetricsProvider>>,
    /// Safe area insets for `env(safe-area-inset-*)`.
    /// Set by the platform on devices with notches/cutouts. Default 0.0.
    pub safe_area_inset_top: f32,
    pub safe_area_inset_right: f32,
    pub safe_area_inset_bottom: f32,
    pub safe_area_inset_left: f32,
}

impl Device {
    /// Create a device for a typical desktop screen.
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            media_type: MediaType::Screen,
            viewport_width: width,
            viewport_height: height,
            device_pixel_ratio: 1.0,
            resolution_dpi: 96.0,
            color_bits: 8,
            monochrome_bits: 0,
            pointer: Pointer::Fine,
            any_pointer: Pointer::Fine,
            hover: HoverCapability::Hover,
            any_hover: HoverCapability::Hover,
            color_gamut: ColorGamut::Srgb,
            dynamic_range: DynamicRange::Standard,
            forced_colors: ForcedColors::None,
            update: Update::Fast,
            scripting: Scripting::Enabled,
            prefers_color_scheme: ColorScheme::Light,
            prefers_reduced_motion: false,
            prefers_reduced_transparency: false,
            prefers_contrast: false,
            inverted_colors: false,
            grid: false,
            font_metrics: None,
            safe_area_inset_top: 0.0,
            safe_area_inset_right: 0.0,
            safe_area_inset_bottom: 0.0,
            safe_area_inset_left: 0.0,
        }
    }

    /// Default font size for `em`/`rem` resolution in media queries.
    /// Media queries use the initial value (16px), not any element's font size.
    #[inline]
    #[must_use]
    pub fn default_font_size(&self) -> f32 {
        16.0
    }

    /// Aspect ratio, safe against zero-height viewports.
    #[inline]
    #[must_use]
    pub fn aspect_ratio(&self) -> f32 {
        if self.viewport_height == 0.0 {
            0.0
        } else {
            self.viewport_width / self.viewport_height
        }
    }
}
