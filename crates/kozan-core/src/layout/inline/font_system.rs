//! Font system — manages font discovery, caching, and measurement.
//!
//! Chrome equivalent: `FontCache` + `SimpleFontData` + `CachingWordShaper`.
//!
//! Wraps Parley's `FontContext` (font discovery + matching) and
//! `LayoutContext` (reusable scratch space for text shaping).
//!
//! # Architecture
//!
//! ```text
//! FontSystem (owned by FrameWidget, one per View)
//!   ├── FontContext     (Parley: font collection, matching, fallback)
//!   │     └── Fontique  (system font enumeration)
//!   │     └── Skrifa    (font file reading, metrics from OS/2 + hhea tables)
//!   │     └── HarfRust  (text shaping, glyph positioning)
//!   └── LayoutContext   (Parley: reusable scratch buffers)
//! ```
//!
//! # Chrome mapping
//!
//! | Chrome | Kozan |
//! |--------|-------|
//! | `FontCache` | `FontSystem` (discovery + cache) |
//! | `SimpleFontData::GetFontMetrics()` | `FontSystem::font_metrics()` |
//! | `CachingWordShaper::Width()` | `FontSystem::measure_text()` |
//! | `HarfBuzzShaper::Shape()` | Parley → `HarfRust` (shaping) |
//! | `FontDescription` | `FontQuery` |
//!
//! # Thread safety
//!
//! `FontSystem` is `!Send` (matches Chrome: font ops are main-thread only).
//! Each View/FrameWidget owns its own `FontSystem`.

use std::cell::RefCell;
use std::sync::Arc;

use parley::fontique::Blob;
use parley::style::{FontStack, FontStyle as ParleyFontStyle, FontWeight as ParleyFontWeight};
use parley::{FontContext, Layout, LayoutContext, StyleProperty};

use super::measurer::{FontMetrics, TextMeasurer, TextMetrics};

/// Wrapper for font data that can be created from multiple sources
/// without unnecessary copies.
///
/// Chrome equivalent: the data source for a `FontFace` — can be a
/// static buffer (bundled font) or dynamically loaded bytes (web font).
pub struct FontBlob(pub(crate) Blob<u8>);

impl From<&'static [u8]> for FontBlob {
    /// Zero-copy: wraps the static pointer in an Arc (no data copy).
    /// Ideal for `include_bytes!()`.
    fn from(data: &'static [u8]) -> Self {
        FontBlob(Blob::new(Arc::new(data)))
    }
}

impl From<Vec<u8>> for FontBlob {
    /// One allocation: Vec → Box<[u8]> → Arc.
    fn from(data: Vec<u8>) -> Self {
        FontBlob(Blob::from(data))
    }
}

impl From<Arc<Vec<u8>>> for FontBlob {
    /// Already shared — wrap directly.
    fn from(data: Arc<Vec<u8>>) -> Self {
        FontBlob(Blob::new(data))
    }
}

/// Query for font lookup — the CSS properties that affect font selection.
///
/// Chrome equivalent: `FontDescription`.
#[derive(Debug, Clone)]
pub struct FontQuery {
    /// Font size in CSS pixels.
    pub font_size: f32,
    /// Font weight (100-900). Chrome: `FontSelectionRequest::weight`.
    pub font_weight: u16,
    /// Font style (normal/italic/oblique). Chrome: `FontSelectionRequest::slope`.
    pub font_style: ParleyFontStyle,
    /// Font family name (comma-separated CSS string).
    pub font_family: String,
    /// CSS `letter-spacing` in px. 0.0 = normal.
    /// Chrome: `FontDescription::LetterSpacing()`.
    pub letter_spacing: f32,
    /// CSS `word-spacing` in px. 0.0 = normal.
    /// Chrome: `FontDescription::WordSpacing()`.
    pub word_spacing: f32,
}

impl FontQuery {
    /// Create a query from full properties.
    #[must_use]
    pub fn new(
        font_size: f32,
        font_weight: u16,
        font_style: ParleyFontStyle,
        font_family: String,
    ) -> Self {
        Self {
            font_size,
            font_weight,
            font_style,
            font_family,
            letter_spacing: 0.0,
            word_spacing: 0.0,
        }
    }

    /// Create a query with just font size (default weight + sans-serif).
    #[must_use]
    pub fn from_size(font_size: f32) -> Self {
        Self {
            font_size,
            font_weight: 400,
            font_style: ParleyFontStyle::Normal,
            font_family: "sans-serif".to_string(),
            letter_spacing: 0.0,
            word_spacing: 0.0,
        }
    }

    /// Create from Stylo `ComputedValues`.
    #[must_use]
    pub fn from_computed(cv: &style::properties::ComputedValues) -> Self {
        let font = cv.get_font();
        let fs = font.clone_font_size().computed_size().px();
        let fw = font.clone_font_weight().value() as u16;
        let style_val = font.clone_font_style();
        let font_style = if style_val == style::values::computed::font::FontStyle::ITALIC {
            ParleyFontStyle::Italic
        } else if style_val != style::values::computed::font::FontStyle::NORMAL {
            ParleyFontStyle::Oblique(None)
        } else {
            ParleyFontStyle::Normal
        };
        // Extract family name from Stylo's FontFamily.
        use style_traits::ToCss;
        let family = font.clone_font_family().to_css_string();

        // Letter-spacing & word-spacing from inherited text properties.
        // Stylo: `Spacing<CSSPixelLength>` — `Value(px)` or `Normal` (= 0).
        let text = cv.get_inherited_text();
        let zero = style::values::computed::CSSPixelLength::new(0.0);
        let letter_spacing = text
            .clone_letter_spacing()
            .0
            .percentage_relative_to(zero)
            .px();
        let word_spacing = text.clone_word_spacing().percentage_relative_to(zero).px();

        Self {
            font_size: fs,
            font_weight: fw,
            font_style,
            font_family: family,
            letter_spacing,
            word_spacing,
        }
    }
}

/// The font system — owns font discovery, caching, and measurement.
///
/// Chrome equivalent: `FontCache` (singleton per renderer process).
/// In Kozan: one per View (per-thread, matching Chrome's threading model).
///
/// All font metrics and text measurements go through this system.
/// No hardcoded values — everything comes from real font files.
///
/// Uses `RefCell` for interior mutability so the `TextMeasurer` trait
/// (which takes `&self`) can perform real Parley shaping (which needs `&mut`).
/// Safe because layout is single-threaded per View.
pub struct FontSystem {
    font_cx: RefCell<FontContext>,
    layout_cx: RefCell<LayoutContext>,
}

impl FontSystem {
    /// Create a new font system, discovering system fonts.
    ///
    /// Chrome: `FontCache::Create()` → enumerates platform fonts.
    /// Parley/Fontique scans the system font directories.
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_cx: RefCell::new(FontContext::new()),
            layout_cx: RefCell::new(LayoutContext::new()),
        }
    }

    /// Register custom font data (TTF/OTF/TTC bytes) into the font collection.
    ///
    /// Chrome equivalent: `FontFaceCache::Add()` — registers a `@font-face`
    /// source so the font becomes available for CSS `font-family` matching.
    ///
    /// After registration, the font's family name is automatically discovered
    /// from the font's `name` table and becomes usable via `font-family` in CSS.
    ///
    /// Accepts anything convertible to `Arc<dyn AsRef<[u8]> + Send + Sync>`:
    /// - `&'static [u8]` — zero-copy for `include_bytes!()` (Arc wraps the pointer, not the data)
    /// - `Vec<u8>` — for runtime-loaded fonts (one allocation into Arc)
    /// - `Arc<[u8]>` — if you already have shared font data
    ///
    /// Returns the family names that were registered.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Static — zero copy, the bytes live in the binary:
    /// ctx.register_font(include_bytes!("../assets/Cairo.ttf") as &[u8]);
    ///
    /// // Runtime — from a file:
    /// ctx.register_font(std::fs::read("font.ttf").unwrap());
    /// ```
    pub fn register_font(&self, data: impl Into<FontBlob>) -> Vec<String> {
        let mut font_cx = self.font_cx.borrow_mut();
        let blob = data.into().0;
        let registered = font_cx.collection.register_fonts(blob, None);
        registered
            .iter()
            .filter_map(|(fid, _)| font_cx.collection.family_name(*fid).map(|n| n.to_string()))
            .collect()
    }

    /// Measure the advance width of a text run.
    ///
    /// Chrome: `CachingWordShaper::Width()` → `HarfBuzzShaper::Shape()`.
    /// Parley: builds a layout, shapes with `HarfRust`, returns width.
    pub fn shape_text(&self, text: &str, query: &FontQuery) -> TextMetrics {
        if text.is_empty() {
            return TextMetrics { width: 0.0 };
        }

        let mut font_cx = self.font_cx.borrow_mut();
        let mut layout_cx = self.layout_cx.borrow_mut();

        let font_stack = family_to_stack(&query.font_family);
        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(query.font_size));
        builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(
            query.font_weight as f32,
        )));
        builder.push_default(StyleProperty::FontStyle(query.font_style));
        builder.push_default(StyleProperty::FontStack(font_stack));

        let mut layout: Layout<[u8; 4]> = builder.build(text);
        layout.break_all_lines(None);

        TextMetrics {
            width: layout.width(),
        }
    }

    /// Get font metrics (ascent, descent, line-gap) for a given font query.
    ///
    /// Chrome: `SimpleFontData::GetFontMetrics()` → reads from font's
    /// OS/2 and hhea tables via Skia.
    pub fn query_metrics(&self, query: &FontQuery) -> FontMetrics {
        let mut font_cx = self.font_cx.borrow_mut();
        let mut layout_cx = self.layout_cx.borrow_mut();

        let font_stack = family_to_stack(&query.font_family);
        let reference_text = "x";
        let mut builder = layout_cx.ranged_builder(&mut font_cx, reference_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(query.font_size));
        builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(
            query.font_weight as f32,
        )));
        builder.push_default(StyleProperty::FontStyle(query.font_style));
        builder.push_default(StyleProperty::FontStack(font_stack));

        let mut layout: Layout<[u8; 4]> = builder.build(reference_text);
        layout.break_all_lines(None);

        // Read ascent/descent/leading from the first line's real font metrics.
        // Parley always produces at least one line for non-empty text.
        let line = layout
            .lines()
            .next()
            .expect("Parley always produces at least one line for non-empty text");
        let m = line.metrics();
        FontMetrics {
            ascent: m.ascent,
            descent: m.descent,
            line_gap: m.leading,
        }
    }
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Real `TextMeasurer` implementation using Parley font shaping.
///
/// Uses `RefCell` interior mutability to satisfy the `&self` trait
/// while performing real `&mut` Parley operations internally.
impl TextMeasurer for FontSystem {
    fn measure(&self, text: &str, font_size: f32) -> TextMetrics {
        let query = FontQuery::from_size(font_size);
        FontSystem::shape_text(self, text, &query)
    }

    fn font_metrics(&self, font_size: f32) -> FontMetrics {
        let query = FontQuery::from_size(font_size);
        FontSystem::query_metrics(self, &query)
    }

    fn shape_text(&self, text: &str, query: &FontQuery) -> TextMetrics {
        FontSystem::shape_text(self, text, query)
    }

    fn shape_text_min_content(&self, text: &str, query: &FontQuery) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let mut font_cx = self.font_cx.borrow_mut();
        let mut layout_cx = self.layout_cx.borrow_mut();

        let font_stack = family_to_stack(&query.font_family);
        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(query.font_size));
        builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(
            query.font_weight as f32,
        )));
        builder.push_default(StyleProperty::FontStyle(query.font_style));
        builder.push_default(StyleProperty::FontStack(font_stack));

        let mut layout: Layout<[u8; 4]> = builder.build(text);
        // max_width=Some(0) forces a break at every opportunity.
        // The resulting layout.width() is the widest unbreakable segment.
        layout.break_all_lines(Some(0.0));
        layout.width()
    }

    fn query_metrics(&self, query: &FontQuery) -> FontMetrics {
        FontSystem::query_metrics(self, query)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        font_size: f32,
        max_width: Option<f32>,
    ) -> super::measurer::WrappedTextMetrics {
        if text.is_empty() {
            let fm = self.query_metrics(&FontQuery::from_size(font_size));
            return super::measurer::WrappedTextMetrics {
                width: 0.0,
                height: fm.ascent + fm.descent,
            };
        }

        let mut font_cx = self.font_cx.borrow_mut();
        let mut layout_cx = self.layout_cx.borrow_mut();

        let query = FontQuery::from_size(font_size);
        let font_stack = family_to_stack(&query.font_family);

        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(400.0)));
        builder.push_default(StyleProperty::FontStack(font_stack));

        let mut layout: Layout<[u8; 4]> = builder.build(text);
        // Parley's real line breaker — handles Unicode line break rules,
        // word boundaries, hyphenation opportunities.
        layout.break_all_lines(max_width);

        super::measurer::WrappedTextMetrics {
            width: layout.width(),
            height: layout.height(),
        }
    }

    fn shape_glyphs(&self, text: &str, query: &FontQuery, color: [u8; 4]) -> Vec<ShapedTextRun> {
        // Delegates to the inherent method on FontSystem.
        FontSystem::shape_glyphs(self, text, query, color)
    }
}

/// A shaped glyph — glyph ID + position, ready for GPU rendering.
///
/// Chrome equivalent: entry in `ShapeResult::RunInfo::glyph_data`.
/// Extracted from Parley's shaping output, owned (no lifetime ties).
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

/// A shaped text run — font + glyphs, ready for the renderer.
///
/// Chrome equivalent: `ShapeResult::RunInfo` — one run per font/script change.
/// Carries the font data (same type as `peniko::Font` — zero conversion to vello).
#[derive(Debug, Clone)]
pub struct ShapedTextRun {
    /// Font file data — `parley::FontData` = `peniko::Font` (same type).
    pub font: parley::FontData,
    /// Font size in CSS pixels.
    pub font_size: f32,
    /// Pre-positioned glyphs from `HarfRust` shaping.
    pub glyphs: Vec<ShapedGlyph>,
    /// Text color as RGBA u8.
    pub color: [u8; 4],
    /// X offset of this run within the line.
    pub offset: f32,
    /// Baseline Y position.
    pub baseline: f32,
    /// Normalized design-space coordinates for variable font axes (e.g., wght, wdth).
    /// Chrome: `FontVariationSettings` → OpenType `fvar` axis values.
    /// Without these, vello renders the default instance (Regular/400)
    /// even if `HarfRust` shaped at the correct weight.
    pub normalized_coords: Vec<i16>,
}

impl FontSystem {
    /// Shape text and extract owned glyph runs for rendering.
    ///
    /// Chrome: layout shapes text → stores `ShapeResult` → paint reads it.
    /// This is the ONLY place text shaping happens. The renderer just
    /// draws the pre-shaped glyphs — zero font logic in the GPU layer.
    ///
    /// Handles automatically via Parley + `HarfRust`:
    /// - Arabic letter joining (initial/medial/final forms)
    /// - RTL bidi reordering
    /// - Ligatures and kerning
    /// - Font fallback (system fonts via Fontique)
    /// - CJK, Thai, Devanagari — all complex scripts
    pub fn shape_glyphs(
        &self,
        text: &str,
        query: &FontQuery,
        color: [u8; 4],
    ) -> Vec<ShapedTextRun> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut font_cx = self.font_cx.borrow_mut();
        let mut layout_cx = self.layout_cx.borrow_mut();

        let font_stack = family_to_stack(&query.font_family);
        let mut builder = layout_cx.ranged_builder(&mut font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(query.font_size));
        builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(
            query.font_weight as f32,
        )));
        builder.push_default(StyleProperty::FontStyle(query.font_style));
        builder.push_default(StyleProperty::FontStack(font_stack));
        builder.push_default(StyleProperty::Brush(color));

        // CSS letter-spacing / word-spacing → Parley shaping.
        // Chrome: `FontDescription::LetterSpacing()` feeds into HarfBuzz.
        if query.letter_spacing != 0.0 {
            builder.push_default(StyleProperty::LetterSpacing(query.letter_spacing));
        }
        if query.word_spacing != 0.0 {
            builder.push_default(StyleProperty::WordSpacing(query.word_spacing));
        }

        let mut layout: Layout<[u8; 4]> = builder.build(text);
        layout.break_all_lines(None);

        let mut runs = Vec::new();

        for line in layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let font_data = run.font().clone();
                let run_font_size = run.font_size();

                // Parley's glyph.x is an OFFSET within the glyph cell, NOT
                // an absolute position. We must accumulate glyph.advance to
                // compute each glyph's absolute x within the run.
                // Reference: parley/src/tests/utils/renderer.rs lines 336-339.
                let mut cursor_x = 0.0f32;
                let glyphs: Vec<ShapedGlyph> = glyph_run
                    .glyphs()
                    .map(|g| {
                        let shaped = ShapedGlyph {
                            id: g.id,
                            x: cursor_x + g.x,
                            y: g.y,
                        };
                        cursor_x += g.advance;
                        shaped
                    })
                    .collect();

                if glyphs.is_empty() {
                    continue;
                }

                // Variable font axis values — tells vello which weight/width
                // instance to render. Without this, vello draws the default
                // instance (Regular/400) even though HarfRust shaped correctly.
                // Chrome: reads fvar axis values from ComputedStyle.
                let normalized_coords: Vec<i16> = run.normalized_coords().to_vec();

                runs.push(ShapedTextRun {
                    font: font_data,
                    font_size: run_font_size,
                    glyphs,
                    color,
                    offset: glyph_run.offset(),
                    baseline: glyph_run.baseline(),
                    normalized_coords,
                });
            }
        }

        runs
    }
}

/// Convert Kozan's `FontFamily` to Parley's `FontStack`.
///
/// Chrome: `FontDescription` → Fontique query.
/// Builds a comma-separated font list string that Parley understands.
fn family_to_stack(family: &str) -> FontStack<'_> {
    // Parley's FontStack::Source accepts "font1, font2, generic" format.
    FontStack::Source(family.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_system_creates_successfully() {
        let _fs = FontSystem::new();
    }

    #[test]
    fn measure_empty_text() {
        let fs = FontSystem::new();
        let query = FontQuery::from_size(16.0);
        let metrics = fs.shape_text("", &query);
        assert_eq!(metrics.width, 0.0);
    }

    #[test]
    fn measure_text_has_nonzero_width() {
        let fs = FontSystem::new();
        let query = FontQuery::from_size(16.0);
        let metrics = fs.shape_text("Hello", &query);
        assert!(
            metrics.width > 0.0,
            "shaped text should have positive width, got {}",
            metrics.width
        );
    }

    #[test]
    fn longer_text_is_wider() {
        let fs = FontSystem::new();
        let query = FontQuery::from_size(16.0);
        let short = fs.shape_text("Hi", &query);
        let long = fs.shape_text("Hello World", &query);
        assert!(long.width > short.width, "longer text should be wider");
    }

    #[test]
    fn larger_font_is_wider() {
        let fs = FontSystem::new();
        let small = FontQuery::from_size(12.0);
        let big = FontQuery::from_size(24.0);
        let w_small = fs.shape_text("Hello", &small);
        let w_big = fs.shape_text("Hello", &big);
        assert!(
            w_big.width > w_small.width,
            "larger font should produce wider text"
        );
    }

    #[test]
    fn font_metrics_from_real_font() {
        let fs = FontSystem::new();
        let query = FontQuery::from_size(16.0);
        let metrics = fs.query_metrics(&query);

        // Real font metrics should have positive ascent and descent.
        assert!(
            metrics.ascent > 0.0,
            "ascent should be positive, got {}",
            metrics.ascent
        );
        assert!(
            metrics.descent > 0.0,
            "descent should be positive, got {}",
            metrics.descent
        );
        assert!(metrics.line_gap >= 0.0, "line_gap should be non-negative");

        // Ascent should be larger than descent for Latin fonts.
        assert!(
            metrics.ascent > metrics.descent,
            "ascent ({}) should be > descent ({}) for Latin fonts",
            metrics.ascent,
            metrics.descent
        );
    }

    #[test]
    fn font_metrics_scale_with_size() {
        let fs = FontSystem::new();
        let small = FontQuery::from_size(12.0);
        let big = FontQuery::from_size(24.0);
        let m_small = fs.query_metrics(&small);
        let m_big = fs.query_metrics(&big);

        // Metrics should scale roughly proportionally.
        assert!(
            m_big.ascent > m_small.ascent,
            "bigger font should have larger ascent"
        );
        assert!(
            m_big.descent > m_small.descent,
            "bigger font should have larger descent"
        );
    }

    #[test]
    fn bold_text_may_differ_in_width() {
        let fs = FontSystem::new();
        let family = "sans-serif".to_string();
        let regular = FontQuery::new(16.0, 400, ParleyFontStyle::Normal, family.clone());
        let bold = FontQuery::new(16.0, 700, ParleyFontStyle::Normal, family);
        let w_regular = fs.shape_text("Hello", &regular);
        let w_bold = fs.shape_text("Hello", &bold);

        // Both should have positive width.
        assert!(w_regular.width > 0.0);
        assert!(w_bold.width > 0.0);
    }

    #[test]
    fn monospace_characters_equal_width() {
        let fs = FontSystem::new();
        let family = "monospace".to_string();
        let query = FontQuery::new(16.0, 400, ParleyFontStyle::Normal, family);
        let w_i = fs.shape_text("iiiii", &query);
        let w_m = fs.shape_text("mmmmm", &query);

        // In a monospace font, all characters should have equal advance.
        assert!(
            (w_i.width - w_m.width).abs() < 1.0,
            "monospace: 'iiiii' ({:.1}) should equal 'mmmmm' ({:.1})",
            w_i.width,
            w_m.width
        );
    }

    #[test]
    fn font_query_from_size_defaults() {
        let query = FontQuery::from_size(20.0);
        assert_eq!(query.font_size, 20.0);
        assert_eq!(query.font_weight, 400);
        assert_eq!(query.font_family, "sans-serif");
    }

    #[test]
    fn named_font_family_query() {
        let family = "Roboto, Arial, sans-serif".to_string();
        let query = FontQuery::new(16.0, 400, ParleyFontStyle::Normal, family);
        // Font family is now a plain CSS string.
        assert!(query.font_family.contains("Roboto"));
        assert!(query.font_family.contains("sans-serif"));
    }
}
