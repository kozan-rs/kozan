//! Font metrics provider for Stylo's Device.
//!
//! Provides font metrics (ascent, descent, x-height, etc.) that Stylo needs
//! for resolving units like `ex`, `ch`, `cap`, `ic`.

use style::device::servo::FontMetricsProvider;
use style::font_metrics::FontMetrics;
use style::properties::style_structs::Font;
use style::values::computed::font::GenericFontFamily;
use style::values::computed::{CSSPixelLength, Length};
use style::values::specified::font::QueryFontMetricsFlags;

/// Kozan's font metrics provider.
/// Uses sensible defaults based on font-size ratios.
/// Will be upgraded to use Parley for real font metrics.
#[derive(Debug)]
pub(crate) struct KozanFontMetricsProvider;

impl KozanFontMetricsProvider {
    pub fn new() -> Self {
        Self
    }
}

impl FontMetricsProvider for KozanFontMetricsProvider {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        font: &Font,
        _base_size: CSSPixelLength,
        _flags: QueryFontMetricsFlags,
    ) -> FontMetrics {
        // Use standard ratios relative to font-size.
        // These are typical for Latin fonts.
        let size = font.font_size.computed_size().px();

        FontMetrics {
            x_height: Some(CSSPixelLength::new(size * 0.53)),
            zero_advance_measure: Some(CSSPixelLength::new(size * 0.6)),
            cap_height: Some(CSSPixelLength::new(size * 0.72)),
            ic_width: Some(CSSPixelLength::new(size)),
            ascent: CSSPixelLength::new(size * 0.8),
            script_percent_scale_down: None,
            script_script_percent_scale_down: None,
        }
    }

    fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
        // 16px is the standard default font size.
        Length::new(16.0)
    }
}
