//! Pluggable text measurement — trait + default estimator.
//!
//! Chrome equivalents:
//! - `CachingWordShaper` → cache layer (future)
//! - `HarfBuzzShaper` → shapes text, produces `ShapeResult` (glyph advances)
//! - `SimpleFontData::GetFontMetrics()` → font-level metrics from OS/2 + hhea tables
//! - `FontHeight` → ascent + descent after leading distribution
//!
//! # Architecture
//!
//! ```text
//! TextMeasurer (trait)
//!   ├── measure(text, font_size) → TextMetrics { width }      ← shaper output
//!   └── font_metrics(font_size)  → FontMetrics { ascent, descent, line_gap }
//!                                                               ← font tables
//!
//! FontHeight { ascent, descent }   ← after CSS line-height + leading split
//!   = resolve from FontMetrics + ComputedStyle::line_height()
//! ```
//!
//! The `TextMeasurer` owns ALL font-dependent values. CSS `line-height`
//! resolution uses `FontMetrics::line_spacing()` for the `Normal` case
//! and pure CSS math for `Number`/`Length` cases.

use style::values::computed::font::LineHeight;

// ============================================================
// FontMetrics — raw font table metrics (per-font, per-size)
// ============================================================

/// Raw font metrics from the font's OS/2 and hhea tables.
///
/// Chrome equivalent: `FontMetrics` (the platform-level metrics
/// read from the font file by Skia/FreeType).
///
/// These are per-font, per-size values — the same for all text
/// in the same font at the same size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// Distance from baseline to top of em-square.
    /// Chrome: `FontMetrics::FixedAscent()`.
    pub ascent: f32,
    /// Distance from baseline to bottom of em-square.
    /// Chrome: `FontMetrics::FixedDescent()`.
    pub descent: f32,
    /// Extra space between lines recommended by the font.
    /// Chrome: `FontMetrics::LineGap()`.
    /// Many fonts (e.g., Roboto) set this to 0.
    pub line_gap: f32,
}

impl FontMetrics {
    /// Content height = ascent + descent.
    #[inline]
    #[must_use] 
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Line spacing = ascent + descent + lineGap.
    /// Chrome: `FontMetrics::FixedLineSpacing()`.
    /// This is what CSS `line-height: normal` resolves to.
    #[inline]
    #[must_use] 
    pub fn line_spacing(&self) -> f32 {
        self.ascent + self.descent + self.line_gap
    }
}

// ============================================================
// FontHeight — ascent + descent after leading distribution
// ============================================================

/// Line box contribution: ascent + descent with leading distributed.
///
/// Chrome equivalent: `FontHeight` (ascent + descent as `LayoutUnit`).
///
/// When CSS `line-height` exceeds the font's content height, the extra
/// space (leading) is split equally above and below. This struct holds
/// the result of that distribution.
///
/// Example: font ascent=16, descent=4, line-height=30.
/// - font height = 20, leading = 30 - 20 = 10
/// - half-leading = 5
/// - `FontHeight` { ascent: 16 + 5 = 21, descent: 4 + 5 = 9 }
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontHeight {
    /// Ascent + half-leading above the baseline.
    pub ascent: f32,
    /// Descent + half-leading below the baseline.
    pub descent: f32,
}

impl FontHeight {
    /// Total line height = ascent + descent.
    #[inline]
    #[must_use] 
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    /// Compute `FontHeight` from raw font metrics + resolved CSS line-height.
    ///
    /// Chrome equivalent: `InlineBoxState::ComputeTextMetrics()` →
    /// `CalculateLeadingSpace()` → `AddLeading()`.
    ///
    /// Leading = `line_height` - `font_height`. Split equally above/below.
    #[must_use] 
    pub fn from_metrics_and_line_height(font_metrics: &FontMetrics, line_height: f32) -> Self {
        let font_height = font_metrics.height();
        let leading = (line_height - font_height).max(0.0);
        let half_leading = leading / 2.0;
        FontHeight {
            ascent: font_metrics.ascent + half_leading,
            descent: font_metrics.descent + half_leading,
        }
    }
}

// ============================================================
// TextMetrics — shaper output (per-run)
// ============================================================

/// Metrics for a specific shaped text run.
///
/// Chrome equivalent: `ShapeResult` (contains glyph advances).
/// Width is the sum of all glyph advances after shaping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Total advance width of the text run.
    pub width: f32,
}

/// Metrics for text laid out with a width constraint (line wrapping).
///
/// Chrome equivalent: the result of `NGInlineLayoutAlgorithm` —
/// measures text within an available width, breaking into lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrappedTextMetrics {
    /// Width of the widest line.
    pub width: f32,
    /// Total height (number of lines × line height).
    pub height: f32,
}

// ============================================================
// TextMeasurer trait
// ============================================================

/// Trait for measuring text and querying font metrics.
///
/// Chrome equivalent: `CachingWordShaper` (width via `HarfBuzzShaper`)
/// + `SimpleFontData::GetFontMetrics()` (font-level metrics).
///
/// Passed by reference — no lifetime on consumer structs.
pub trait TextMeasurer {
    /// Measure the advance width of a text run (no wrapping).
    ///
    /// Chrome: `CachingWordShaper::Width()` → `HarfBuzzShaper::Shape()`
    /// → sum of glyph advances from `ShapeResult`.
    fn measure(&self, text: &str, font_size: f32) -> TextMetrics;

    /// Get font metrics (ascent, descent, lineGap) for a given font size.
    ///
    /// Chrome: `SimpleFontData::GetFontMetrics()` → reads from the
    /// font's OS/2 and hhea tables via Skia.
    fn font_metrics(&self, font_size: f32) -> FontMetrics;

    /// Measure text width using a full `FontQuery` (correct font family + weight).
    /// Default delegates to `measure()` ignoring family/weight.
    fn shape_text(&self, text: &str, query: &super::font_system::FontQuery) -> TextMetrics {
        self.measure(text, query.font_size)
    }

    /// Get font metrics using a full `FontQuery` (correct font family + weight).
    /// Default delegates to `font_metrics()` ignoring family/weight.
    fn query_metrics(&self, query: &super::font_system::FontQuery) -> FontMetrics {
        self.font_metrics(query.font_size)
    }

    /// Shape text and extract glyph runs for rendering.
    ///
    /// Default: empty (no shaping). `FontSystem` provides real implementation.
    fn shape_glyphs(
        &self,
        _text: &str,
        _query: &super::font_system::FontQuery,
        _color: [u8; 4],
    ) -> Vec<super::font_system::ShapedTextRun> {
        Vec::new()
    }

    /// Measure text with a width constraint, performing line wrapping.
    ///
    /// Chrome: `NGInlineLayoutAlgorithm` → breaks text into lines
    /// within the available width, returns the bounding box.
    ///
    /// `max_width`: `None` = no constraint (single line),
    /// `Some(w)` = wrap lines at `w` pixels.
    ///
    /// Default implementation uses word-level wrapping with `measure()`.
    /// `FontSystem` overrides with Parley's real line breaker.
    fn measure_wrapped(&self, text: &str, font_size: f32, max_width: Option<f32>) -> WrappedTextMetrics {
        let fm = self.font_metrics(font_size);
        let line_h = fm.ascent + fm.descent;
        let max_w = max_width.unwrap_or(f32::INFINITY);

        // Split at word boundaries (whitespace + zero-width space).
        let words: Vec<&str> = text
            .split(|c: char| c.is_whitespace() || c == '\u{200B}')
            .filter(|w| !w.is_empty())
            .collect();

        let mut lines = 1u32;
        let mut current_line_w: f32 = 0.0;
        let mut widest: f32 = 0.0;

        for word in &words {
            let word_w = self.measure(word, font_size).width;
            if current_line_w > 0.0 && current_line_w + word_w > max_w {
                widest = widest.max(current_line_w);
                current_line_w = word_w;
                lines += 1;
            } else {
                current_line_w += word_w;
            }
        }
        widest = widest.max(current_line_w);

        WrappedTextMetrics {
            width: widest,
            height: lines as f32 * line_h,
        }
    }
}

// ============================================================
// CSS line-height resolution
// ============================================================

/// Resolve CSS `line-height` to a pixel value.
///
/// Chrome equivalent: `ComputedStyle::ComputedLineHeight()`.
///
/// Stylo's computed `LineHeight` has three variants:
/// - `Normal` → `FontMetrics::line_spacing()` (ascent + descent + lineGap
///   from the actual font — NOT a hardcoded multiplier).
/// - `Number(n)` → `font_size * n` (unitless multiplier).
/// - `Length(px)` → absolute pixel value (already resolved by Stylo).
#[must_use] 
pub fn resolve_line_height(
    line_height: &LineHeight,
    font_size: f32,
    font_metrics: &FontMetrics,
) -> f32 {
    match line_height {
        LineHeight::Normal => font_metrics.line_spacing(),
        LineHeight::Number(n) => font_size * n.0,
        LineHeight::Length(l) => l.0.px(),
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TextMeasurer tests (real FontSystem) ----

    use crate::layout::inline::FontSystem;

    #[test]
    fn longer_text_is_wider() {
        let m = FontSystem::new();
        let short = m.measure("Hi", 16.0);
        let long = m.measure("Hello World", 16.0);
        assert!(long.width > short.width, "longer text should be wider");
    }

    #[test]
    fn larger_font_is_wider() {
        let m = FontSystem::new();
        let small = m.measure("Hello", 12.0);
        let big = m.measure("Hello", 24.0);
        assert!(big.width > small.width, "larger font should be wider");
    }

    #[test]
    fn empty_string_zero_width() {
        let m = FontSystem::new();
        assert_eq!(m.measure("", 16.0).width, 0.0);
    }

    #[test]
    fn nonzero_width_for_text() {
        let m = FontSystem::new();
        assert!(m.measure("A", 16.0).width > 0.0);
    }

    #[test]
    fn custom_measurer_trait() {
        struct FixedMeasurer;
        impl TextMeasurer for FixedMeasurer {
            fn measure(&self, _text: &str, _font_size: f32) -> TextMetrics {
                TextMetrics { width: 100.0 }
            }
            fn font_metrics(&self, _font_size: f32) -> FontMetrics {
                FontMetrics {
                    ascent: 14.0,
                    descent: 4.0,
                    line_gap: 2.0,
                }
            }
        }
        assert_eq!(FixedMeasurer.measure("x", 0.0).width, 100.0);
        assert_eq!(FixedMeasurer.font_metrics(0.0).line_spacing(), 20.0);
    }

    // ---- FontMetrics tests ----

    #[test]
    fn font_metrics_from_real_font() {
        let m = FontSystem::new();
        let fm = m.font_metrics(16.0);
        assert!(fm.ascent > 0.0, "ascent should be positive");
        assert!(fm.descent > 0.0, "descent should be positive");
        assert!(fm.ascent > fm.descent, "ascent > descent for Latin fonts");
        assert!(fm.height() > 0.0, "height should be positive");
    }

    // ---- FontHeight (leading distribution) tests ----

    #[test]
    fn font_height_no_leading() {
        // line-height == font height → no leading.
        let fm = FontMetrics {
            ascent: 16.0,
            descent: 4.0,
            line_gap: 0.0,
        };
        let fh = FontHeight::from_metrics_and_line_height(&fm, 20.0);
        assert_eq!(fh.ascent, 16.0);
        assert_eq!(fh.descent, 4.0);
        assert_eq!(fh.height(), 20.0);
    }

    #[test]
    fn font_height_with_leading() {
        // line-height=30, font_height=20 → leading=10, half=5.
        let fm = FontMetrics {
            ascent: 16.0,
            descent: 4.0,
            line_gap: 0.0,
        };
        let fh = FontHeight::from_metrics_and_line_height(&fm, 30.0);
        assert_eq!(fh.ascent, 21.0); // 16 + 5
        assert_eq!(fh.descent, 9.0); // 4 + 5
        assert_eq!(fh.height(), 30.0);
    }

    #[test]
    fn font_height_line_height_smaller_than_font() {
        // line-height < font_height → leading clamped to 0.
        let fm = FontMetrics {
            ascent: 16.0,
            descent: 4.0,
            line_gap: 0.0,
        };
        let fh = FontHeight::from_metrics_and_line_height(&fm, 15.0);
        assert_eq!(fh.ascent, 16.0); // no negative leading
        assert_eq!(fh.descent, 4.0);
        assert_eq!(fh.height(), 20.0); // font_height, not 15
    }

    // ---- line-height resolution tests ----

    fn test_font_metrics() -> FontMetrics {
        // Font with lineGap > 0 to verify Normal uses line_spacing.
        FontMetrics {
            ascent: 16.0,
            descent: 4.0,
            line_gap: 2.0,
        }
    }

    #[test]
    fn line_height_normal_uses_font_line_spacing() {
        let fm = test_font_metrics();
        // Normal → ascent + descent + lineGap = 16 + 4 + 2 = 22
        assert_eq!(resolve_line_height(&LineHeight::Normal, 20.0, &fm), 22.0);
    }

    #[test]
    fn line_height_number_multiplier() {
        use style::values::generics::NonNegative;
        let fm = test_font_metrics();
        assert_eq!(
            resolve_line_height(&LineHeight::Number(NonNegative(1.5)), 20.0, &fm),
            30.0
        );
        assert_eq!(
            resolve_line_height(&LineHeight::Number(NonNegative(2.0)), 16.0, &fm),
            32.0
        );
    }

    #[test]
    fn line_height_px() {
        use style::values::computed::CSSPixelLength;
        use style::values::generics::NonNegative;
        let fm = test_font_metrics();
        assert_eq!(
            resolve_line_height(&LineHeight::Length(NonNegative(CSSPixelLength::new(28.0))), 20.0, &fm),
            28.0
        );
    }

    #[test]
    #[ignore] // TODO: Stylo resolves percent/em at computed-value time; LineHeight::Length is always px
    fn line_height_percent() {}

    #[test]
    #[ignore] // TODO: Stylo resolves percent/em at computed-value time; LineHeight::Length is always px
    fn line_height_em() {}

    #[test]
    fn line_height_normal_zero_line_gap() {
        // Roboto and many fonts: lineGap = 0
        let fm = FontMetrics {
            ascent: 16.0,
            descent: 4.0,
            line_gap: 0.0,
        };
        assert_eq!(resolve_line_height(&LineHeight::Normal, 20.0, &fm), 20.0);
    }

    #[test]
    fn line_height_normal_large_line_gap() {
        // Some CJK fonts have significant lineGap.
        let fm = FontMetrics {
            ascent: 16.0,
            descent: 4.0,
            line_gap: 8.0,
        };
        assert_eq!(resolve_line_height(&LineHeight::Normal, 20.0, &fm), 28.0);
    }
}
