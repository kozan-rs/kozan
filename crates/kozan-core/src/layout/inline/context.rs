//! Inline formatting context — collects inline items and manages layout.
//!
//! Chrome equivalent: `NGInlineNode::PrepareLayout()` + `CollectInlines`.

use smallvec::SmallVec;

use super::item::InlineItem;
use style::properties::ComputedValues;

/// The inline formatting context for a block container.
///
/// Collects all inline-level content into a flat item list,
/// then delegates to the line breaker for layout.
#[derive(Debug, Clone)]
pub struct InlineFormattingContext {
    /// Flat list of inline items (text runs, open/close tags, atomics).
    /// `SmallVec` avoids heap allocation for the common case of ≤16 items.
    pub items: SmallVec<[InlineItem; 16]>,
}

impl InlineFormattingContext {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            items: SmallVec::new(),
        }
    }

    /// Add a text run, measuring it via the provided `TextMeasurer`.
    ///
    /// Chrome equivalent: `InlineBoxState::ComputeTextMetrics()`.
    ///
    /// 1. `measure()` → `ShapeResult` (width from glyph advances)
    /// 2. `font_metrics()` → `FontMetrics` (ascent/descent/lineGap from font)
    /// 3. `resolve_line_height()` → CSS line-height → pixel value
    /// 4. `FontHeight::from_metrics_and_line_height()` → leading split
    pub fn add_text(
        &mut self,
        content: std::sync::Arc<str>,
        style: servo_arc::Arc<ComputedValues>,
        measurer: &dyn super::measurer::TextMeasurer,
    ) {
        let font_size = style.clone_font_size().computed_size().px();
        let text_metrics = measurer.measure(&content, font_size);
        let font_metrics = measurer.font_metrics(font_size);

        // Resolve CSS line-height using the font's actual metrics.
        let line_height =
            super::measurer::resolve_line_height(&style.clone_line_height(), font_size, &font_metrics);

        // Distribute leading equally above and below the baseline.
        // Chrome: CalculateLeadingSpace() → AddLeading().
        let font_height =
            super::measurer::FontHeight::from_metrics_and_line_height(&font_metrics, line_height);

        self.items.push(InlineItem::Text {
            content,
            style,
            measured_width: text_metrics.width,
            measured_height: font_height.height(),
            baseline: font_height.ascent,
        });
    }

    /// Add a forced line break.
    pub fn add_break(&mut self) {
        self.items.push(InlineItem::ForcedBreak);
    }

    /// Add an atomic inline (inline-block, image, etc.).
    pub fn add_atomic(
        &mut self,
        width: f32,
        height: f32,
        baseline: f32,
        layout_id: u32,
        style: servo_arc::Arc<ComputedValues>,
    ) {
        self.items.push(InlineItem::AtomicInline {
            width,
            height,
            baseline,
            layout_id,
            style,
        });
    }

    /// Add an open tag for an inline element.
    pub fn add_open_tag(&mut self, style: servo_arc::Arc<ComputedValues>) {
        // Extract inline-direction margin/border/padding.
        // TODO: support RTL (swap left↔right based on direction)
        let margin_start = resolve_stylo_margin(&style.get_margin().margin_left);
        let border_start = style.get_border().border_left_width.0.to_f32_px();
        let padding_start = resolve_stylo_lp(&style.get_padding().padding_left.0);

        self.items.push(InlineItem::OpenTag {
            style,
            margin_inline_start: margin_start,
            border_inline_start: border_start,
            padding_inline_start: padding_start,
        });
    }

    /// Add a close tag for an inline element.
    pub fn add_close_tag(&mut self, style: &ComputedValues) {
        // TODO: support RTL (swap left↔right based on direction)
        let margin_end = resolve_stylo_margin(&style.get_margin().margin_right);
        let border_end = style.get_border().border_right_width.0.to_f32_px();
        let padding_end = resolve_stylo_lp(&style.get_padding().padding_right.0);

        self.items.push(InlineItem::CloseTag {
            margin_inline_end: margin_end,
            border_inline_end: border_end,
            padding_inline_end: padding_end,
        });
    }

    /// Whether this context has any items.
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Resolve a Stylo margin value to px (auto → 0).
fn resolve_stylo_margin(value: &style::values::generics::length::GenericMargin<style::values::computed::LengthPercentage>) -> f32 {
    use style::values::generics::length::GenericMargin;
    match value {
        GenericMargin::LengthPercentage(lp) => resolve_stylo_lp(lp),
        GenericMargin::Auto => 0.0,
        _ => 0.0, // AnchorSizeFunction etc.
    }
}

/// Resolve a Stylo `LengthPercentage` to px (percentages resolved against 0 for now).
fn resolve_stylo_lp(value: &style::values::computed::LengthPercentage) -> f32 {
    value.percentage_relative_to(style::values::computed::CSSPixelLength::new(0.0)).px()
}

impl Default for InlineFormattingContext {
    fn default() -> Self {
        Self::new()
    }
}
