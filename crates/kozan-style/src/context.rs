//! Cascade resolution context.
//!
//! `ComputeContext` carries everything needed to resolve specified values
//! to computed values: font metrics, viewport size, container sizes, zoom.

/// Context for resolving specified → computed values.
///
/// Passed to `ToComputedValue::to_computed_value()`. Provides the element's
/// font-size, root font-size, viewport dimensions, and font metrics needed
/// to resolve relative CSS units (em, rem, vw, vh, ch, ex, etc.).
pub struct ComputeContext<'a> {
    /// Current element's computed font-size in px.
    pub font_size: f32,
    /// Root element's computed font-size in px (for `rem`).
    pub root_font_size: f32,
    /// Viewport width in px (for `vw`, `vi`, etc.).
    pub viewport_width: f32,
    /// Viewport height in px (for `vh`, `vb`, etc.).
    pub viewport_height: f32,
    /// Small viewport dimensions (browser UI fully expanded).
    pub small_viewport: ViewportSize,
    /// Large viewport dimensions (browser UI retracted).
    pub large_viewport: ViewportSize,
    /// Dynamic viewport dimensions (current state of browser UI).
    pub dynamic_viewport: ViewportSize,
    /// Font metrics for ch, ex, cap, ic units.
    pub font_metrics: Option<&'a FontMetrics>,
    /// Root element's font metrics for rch, rex, rcap, ric units.
    pub root_font_metrics: Option<&'a FontMetrics>,
    /// Container query sizes for cqw, cqh, cqi, cqb units.
    pub container_size: Option<&'a ContainerSize>,
    /// Current element's line-height in px (for `lh`).
    pub line_height: f32,
    /// Root element's line-height in px (for `rlh`).
    pub root_line_height: f32,
    /// Effective zoom factor.
    pub zoom: f32,
    /// This element's computed `color` value (resolved before other color properties).
    /// Used by `currentColor` in border-color, outline-color, etc.
    pub current_color: crate::AbsoluteColor,
    /// Parent element's computed `color` (for inheriting the `color` property itself).
    pub inherited_color: crate::AbsoluteColor,
    /// Active color scheme (light/dark) — affects system color resolution.
    pub color_scheme: crate::ColorScheme,
    /// Whether the inline axis is horizontal (true for horizontal-tb, false for vertical-*).
    /// Used by `vi`/`vb`/`svi`/`svb`/`lvi`/`lvb`/`dvi`/`dvb` and `cqi`/`cqb` viewport units.
    pub horizontal_writing_mode: bool,
}

/// Viewport dimensions for a specific viewport variant.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportSize {
    pub width: f32,
    pub height: f32,
}

/// Font metrics needed for font-relative CSS units.
#[derive(Clone, Copy, Debug, Default)]
pub struct FontMetrics {
    /// x-height of the font (for `ex`, `rex`).
    pub x_height: f32,
    /// Width of "0" (U+0030) glyph (for `ch`, `rch`).
    pub zero_advance: f32,
    /// Cap height of the font (for `cap`, `rcap`).
    pub cap_height: f32,
    /// Width of CJK water ideograph U+6C34 (for `ic`, `ric`).
    pub ic_width: f32,
}

/// Container query dimensions.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContainerSize {
    pub width: f32,
    pub height: f32,
    pub inline_size: f32,
    pub block_size: f32,
}

impl Default for ComputeContext<'_> {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            small_viewport: ViewportSize { width: 1920.0, height: 1080.0 },
            large_viewport: ViewportSize { width: 1920.0, height: 1080.0 },
            dynamic_viewport: ViewportSize { width: 1920.0, height: 1080.0 },
            font_metrics: None,
            root_font_metrics: None,
            container_size: None,
            line_height: 19.2,   // 1.2 * 16.0
            root_line_height: 19.2,
            zoom: 1.0,
            current_color: crate::AbsoluteColor::BLACK,
            inherited_color: crate::AbsoluteColor::BLACK,
            color_scheme: crate::ColorScheme::Light,
            horizontal_writing_mode: true,
        }
    }
}
