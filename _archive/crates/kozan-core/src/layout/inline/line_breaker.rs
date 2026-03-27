//! Line breaker — splits inline items into lines.
//!
//! Chrome equivalent: `NGLineBreaker` + `NGInlineLayoutAlgorithm`.
//!
//! # Algorithm
//!
//! 1. Initialize the **strut** — the parent block container's font metrics
//!    that set the minimum height for every line (even empty ones).
//!    Chrome: `InlineBoxState::InitializeStrut()`.
//! 2. Walk inline items left to right, accumulating width.
//! 3. When accumulated width exceeds available width:
//!    a. Find the last whitespace break opportunity in the text.
//!    b. Split the text there: first part on current line, remainder on next.
//!    c. If no whitespace fits: overflow the word (CSS `overflow-wrap: normal`).
//! 4. For each line: compute height from tallest item + strut, align baselines.
//! 5. Produce a `Line` for each line box.

use kozan_primitives::geometry::{Point, Size};
use smallvec::SmallVec;
use std::sync::Arc;

use super::item::InlineItem;
use super::measurer::{FontHeight, TextMeasurer};
use crate::layout::fragment::{
    BoxFragmentData, ChildFragment, Fragment, LineFragmentData, TextFragmentData,
};
use style::properties::longhands::text_wrap_mode::computed_value::T as TextWrapMode;
use style::values::computed::box_::AlignmentBaseline;

/// A completed line from the line breaker.
#[derive(Debug)]
pub struct Line {
    pub fragments: Vec<ChildFragment>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

/// Break inline items into lines that fit within `available_width`.
///
/// `strut` is the parent block container's line box contribution —
/// sets the minimum height/baseline for every line, even empty ones.
/// Chrome: `InlineBoxState::InitializeStrut()`.
///
/// `measurer` is needed for re-measuring text substrings when splitting
/// at word boundaries.
pub fn break_into_lines(
    items: &[InlineItem],
    available_width: f32,
    text_wrap_mode: TextWrapMode,
    strut: &FontHeight,
    measurer: &dyn TextMeasurer,
) -> Vec<Line> {
    let allow_wrap = text_wrap_mode == TextWrapMode::Wrap;

    let mut lines: Vec<Line> = Vec::new();
    let mut current_line = LineBuilder::new(strut);

    for item in items {
        match item {
            InlineItem::ForcedBreak => {
                lines.push(current_line.finish());
                current_line = LineBuilder::new(strut);
            }

            InlineItem::Text {
                content,
                style,
                measured_width,
                measured_height,
                baseline,
                ..
            } => {
                let remaining_width = available_width - current_line.width;
                let valign = style.clone_alignment_baseline();

                if !allow_wrap || *measured_width <= remaining_width {
                    // Fits or no-wrap — add whole item.
                    current_line.add_text(
                        content.clone(),
                        *measured_width,
                        *measured_height,
                        *baseline,
                        valign,
                    );
                } else {
                    // Text overflows — try to split at word boundaries.
                    let font_size = style.clone_font_size().computed_size().px();
                    let mut text: &str = content;
                    let height = *measured_height;
                    let bl = *baseline;

                    loop {
                        let space_left = (available_width - current_line.width).max(0.0);

                        if let Some(split) = find_break_point(text, font_size, space_left, measurer)
                        {
                            // Found a break point — add first part to current line.
                            let first = &text[..split];
                            let first_metrics = measurer.measure(first, font_size);
                            if !first.is_empty() {
                                current_line.add_text(
                                    Arc::from(first),
                                    first_metrics.width,
                                    height,
                                    bl,
                                    valign,
                                );
                            }
                            lines.push(current_line.finish());
                            current_line = LineBuilder::new(strut);

                            // Skip the whitespace at the break point.
                            text = text[split..].trim_start();
                            if text.is_empty() {
                                break;
                            }
                            // Continue loop — remainder may need further splitting.
                        } else {
                            // No break point fits.
                            if current_line.width > 0.0 {
                                // Line not empty — break to new line and retry.
                                lines.push(current_line.finish());
                                current_line = LineBuilder::new(strut);
                                // Don't advance text — retry on the fresh line.
                            } else {
                                // Line is empty — overflow the whole text onto this line.
                                // CSS `overflow-wrap: normal`: word doesn't break.
                                let full_metrics = measurer.measure(text, font_size);
                                current_line.add_text(
                                    Arc::from(text),
                                    full_metrics.width,
                                    height,
                                    bl,
                                    valign,
                                );
                                break;
                            }
                        }
                    }
                }
            }

            InlineItem::AtomicInline {
                width,
                height,
                baseline,
                layout_id,
                style,
            } => {
                if allow_wrap
                    && current_line.width + width > available_width
                    && current_line.width > 0.0
                {
                    lines.push(current_line.finish());
                    current_line = LineBuilder::new(strut);
                }

                current_line.add_atomic(
                    *width,
                    *height,
                    *baseline,
                    *layout_id,
                    style.clone_alignment_baseline(),
                );
            }

            InlineItem::OpenTag {
                margin_inline_start,
                border_inline_start,
                padding_inline_start,
                ..
            } => {
                current_line.width +=
                    margin_inline_start + border_inline_start + padding_inline_start;
            }

            InlineItem::CloseTag {
                margin_inline_end,
                border_inline_end,
                padding_inline_end,
            } => {
                current_line.width += margin_inline_end + border_inline_end + padding_inline_end;
            }
        }
    }

    // Don't forget the last line.
    if current_line.width > 0.0 || current_line.items.is_empty() {
        lines.push(current_line.finish());
    }

    lines
}

/// Find the byte offset of the last whitespace break point that fits
/// within `available_width`.
///
/// Chrome equivalent: part of `NGLineBreaker::HandleText()` — scanning
/// for break opportunities using ICU line break rules. We use whitespace
/// as the break opportunity (CSS `word-break: normal`).
///
/// Returns `Some(byte_offset)` if a break point fits, `None` if no
/// whitespace-based break fits within the available width.
fn find_break_point(
    text: &str,
    font_size: f32,
    available_width: f32,
    measurer: &dyn TextMeasurer,
) -> Option<usize> {
    // Walk left to right, measuring each word segment once and accumulating.
    // This is O(n) in character count rather than O(k·n) from re-measuring
    // each prefix from the start on every whitespace encounter.
    let mut last_break: Option<usize> = None;
    let mut segment_start = 0;
    let mut running_width = 0.0_f32;

    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            // Measure only the segment since the last measured point.
            let segment = &text[segment_start..i];
            running_width += measurer.measure(segment, font_size).width;
            if running_width <= available_width {
                last_break = Some(i);
                segment_start = i;
            } else {
                break;
            }
        }
    }

    last_break
}

/// Convert lines to line box fragments.
#[must_use]
pub fn lines_to_fragments(lines: Vec<Line>, available_width: f32) -> Vec<ChildFragment> {
    let mut result = Vec::with_capacity(lines.len());
    let mut block_offset: f32 = 0.0;

    for line in lines {
        let line_fragment = Fragment::new_line(
            Size::new(available_width, line.height),
            LineFragmentData {
                children: line.fragments,
                baseline: line.baseline,
            },
        );

        result.push(ChildFragment {
            offset: Point::new(0.0, block_offset),
            fragment: line_fragment,
        });

        block_offset += line.height;
    }

    result
}

/// The kind of content a line item represents.
///
/// Chrome equivalent: `NGInlineItem::Type` — text vs atomic inline.
enum LineItemKind {
    /// A text run with its string content.
    Text(Arc<str>),
    /// An atomic inline (inline-block, img, etc.) with its layout tree ID.
    Atomic(u32),
}

/// A single item positioned on a line.
///
/// Chrome equivalent: fields on `NGInlineItemResult`.
struct LineItem {
    x: f32,
    width: f32,
    height: f32,
    baseline: f32,
    alignment_baseline: AlignmentBaseline,
    kind: LineItemKind,
}

/// Builder for a single line.
///
/// Chrome equivalent: part of `NGLineBoxFragmentBuilder`.
struct LineBuilder {
    items: SmallVec<[LineItem; 8]>,
    width: f32,
    max_ascent: f32,
    max_descent: f32,
}

impl LineBuilder {
    fn new(strut: &FontHeight) -> Self {
        Self {
            items: SmallVec::new(),
            width: 0.0,
            max_ascent: strut.ascent,
            max_descent: strut.descent,
        }
    }

    fn add_text(
        &mut self,
        content: Arc<str>,
        width: f32,
        height: f32,
        baseline: f32,
        alignment_baseline: AlignmentBaseline,
    ) {
        self.items.push(LineItem {
            x: self.width,
            width,
            height,
            baseline,
            alignment_baseline,
            kind: LineItemKind::Text(content),
        });
        self.width += width;
        self.update_line_metrics(height, baseline, alignment_baseline);
    }

    fn add_atomic(
        &mut self,
        width: f32,
        height: f32,
        baseline: f32,
        layout_id: u32,
        alignment_baseline: AlignmentBaseline,
    ) {
        self.items.push(LineItem {
            x: self.width,
            width,
            height,
            baseline,
            alignment_baseline,
            kind: LineItemKind::Atomic(layout_id),
        });
        self.width += width;
        self.update_line_metrics(height, baseline, alignment_baseline);
    }

    /// Update ascent/descent tracking for a new item.
    ///
    /// In CSS Inline 3 (Stylo 0.14), the old `vertical-align` is decomposed into:
    /// - `alignment-baseline` (keyword: Baseline, `TextTop`, `TextBottom`, Middle, etc.)
    /// - `baseline-shift` (length offset for super/sub)
    ///
    /// Currently we only handle `alignment-baseline` keywords. Super/Sub shift
    /// is handled via `baseline-shift` which requires separate resolution.
    fn update_line_metrics(&mut self, height: f32, baseline: f32, align: AlignmentBaseline) {
        match align {
            // TextTop/TextBottom-aligned items don't shift the baseline —
            // they are positioned after the line height is known.
            // Note: CSS Inline 3 doesn't have Top/Bottom on alignment-baseline.
            // TextTop and TextBottom are the closest equivalents.
            AlignmentBaseline::TextTop | AlignmentBaseline::TextBottom => {
                // They still contribute to minimum line height.
                let total = self.max_ascent + self.max_descent;
                if height > total {
                    self.max_descent = self.max_descent.max(height - self.max_ascent);
                }
            }
            // Baseline, Middle, and all others contribute normally
            // to the ascent/descent envelope.
            _ => {
                let descent = height - baseline;
                self.max_ascent = self.max_ascent.max(baseline);
                self.max_descent = self.max_descent.max(descent);
            }
        }
    }

    fn finish(self) -> Line {
        let height = self.max_ascent + self.max_descent;
        let baseline = self.max_ascent;

        let mut fragments = Vec::with_capacity(self.items.len());

        for item in &self.items {
            let offset_y = match item.alignment_baseline {
                AlignmentBaseline::Baseline => (baseline - item.baseline).max(0.0),
                AlignmentBaseline::Middle => ((height - item.height) / 2.0).max(0.0),
                AlignmentBaseline::TextTop => {
                    // Align top of element with top of parent font (strut).
                    0.0
                }
                AlignmentBaseline::TextBottom => {
                    // Align bottom of element with bottom of parent font.
                    (height - item.height).max(0.0)
                }
                // All other alignment-baseline values (Alphabetic, Central,
                // Mathematical, Hanging, Ideographic) — treat as baseline for now.
                _ => (baseline - item.baseline).max(0.0),
            };

            let fragment = match &item.kind {
                LineItemKind::Text(content) => Fragment::new_text(
                    Size::new(item.width, item.height),
                    TextFragmentData {
                        text_range: 0..content.len(),
                        baseline: item.baseline,
                        text: Some(content.clone()),
                        shaped_runs: Vec::new(),
                    },
                ),
                LineItemKind::Atomic(_layout_id) => Fragment::new_box(
                    Size::new(item.width, item.height),
                    BoxFragmentData {
                        scrollable_overflow: Size::new(item.width, item.height),
                        ..Default::default()
                    },
                ),
            };

            fragments.push(ChildFragment {
                offset: Point::new(item.x, offset_y),
                fragment,
            });
        }

        Line {
            fragments,
            width: self.width,
            height,
            baseline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::inline::FontSystem;
    use crate::layout::inline::measurer::resolve_line_height;
    use style::properties::ComputedValues;

    fn initial_style() -> servo_arc::Arc<ComputedValues> {
        crate::styling::initial_values_arc().clone()
    }

    fn default_strut() -> FontHeight {
        let m = FontSystem::new();
        let fm = m.font_metrics(16.0);
        let style = initial_style();
        let lh = resolve_line_height(&style.clone_line_height(), 16.0, &fm);
        FontHeight::from_metrics_and_line_height(&fm, lh)
    }

    fn text_item(text: &str, width: f32) -> InlineItem {
        let m = FontSystem::new();
        let fm = m.font_metrics(16.0);
        let style = initial_style();
        let lh = resolve_line_height(&style.clone_line_height(), 16.0, &fm);
        let fh = FontHeight::from_metrics_and_line_height(&fm, lh);

        InlineItem::Text {
            content: Arc::from(text),
            style,
            measured_width: width,
            measured_height: fh.height(),
            baseline: fh.ascent,
        }
    }

    /// Build a text item with width from the default measurer.
    fn measured_text_item(text: &str) -> InlineItem {
        let m = FontSystem::new();
        let fm = m.font_metrics(16.0);
        let style = initial_style();
        let lh = resolve_line_height(&style.clone_line_height(), 16.0, &fm);
        let fh = FontHeight::from_metrics_and_line_height(&fm, lh);
        let width = m.measure(text, 16.0).width;

        InlineItem::Text {
            content: Arc::from(text),
            style,
            measured_width: width,
            measured_height: fh.height(),
            baseline: fh.ascent,
        }
    }

    fn m() -> FontSystem {
        FontSystem::new()
    }

    #[test]
    fn single_line_fits() {
        let strut = default_strut();
        let items = vec![text_item("Hello", 50.0), text_item("World", 50.0)];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].width, 100.0);
    }

    #[test]
    fn multiple_items_overflow() {
        let strut = default_strut();
        // Use measured_text_item for consistent widths with the measurer.
        // Each word width: "Hello" = 5*16*0.5=40, "World"=40, "Foo"=24.
        // Available: 50px. "Hello"(40) fits. "World"(40): 40+40=80 > 50 → break.
        let items = vec![
            measured_text_item("Hello"),
            measured_text_item("World"),
            measured_text_item("Foo"),
        ];
        let lines = break_into_lines(&items, 50.0, TextWrapMode::Wrap, &strut, &m());
        // "Hello" on line 1, "World" on line 2, "Foo" on line 3.
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn no_wrap_single_line() {
        let strut = default_strut();
        let items = vec![text_item("Hello", 200.0), text_item("World", 200.0)];
        let lines = break_into_lines(&items, 100.0, TextWrapMode::Nowrap, &strut, &m());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].width, 400.0);
    }

    #[test]
    fn forced_break() {
        let strut = default_strut();
        let items = vec![
            text_item("Line1", 50.0),
            InlineItem::ForcedBreak,
            text_item("Line2", 50.0),
        ];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn baseline_alignment() {
        let strut = default_strut();
        let items = vec![
            InlineItem::Text {
                content: Arc::from("Big"),
                style: initial_style(),
                measured_width: 50.0,
                measured_height: 24.0,
                baseline: 20.0,
            },
            InlineItem::Text {
                content: Arc::from("Small"),
                style: initial_style(),
                measured_width: 30.0,
                measured_height: 12.0,
                baseline: 10.0,
            },
        ];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1);
        // Line height = max_ascent + max_descent, where both are max of strut
        // and item contributions. The strut (from real font metrics) participates.
        let expected_ascent = strut.ascent.max(20.0).max(10.0);
        let expected_descent = strut.descent.max(4.0).max(2.0);
        assert_eq!(lines[0].height, expected_ascent + expected_descent);
        assert_eq!(lines[0].baseline, expected_ascent);
    }

    #[test]
    fn empty_line_uses_strut() {
        let strut = FontHeight {
            ascent: 14.0,
            descent: 6.0,
        };
        let items = vec![InlineItem::ForcedBreak];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].height, 20.0);
        assert_eq!(lines[0].baseline, 14.0);
    }

    #[test]
    fn strut_sets_minimum_line_height() {
        let strut = FontHeight {
            ascent: 20.0,
            descent: 10.0,
        };
        let items = vec![InlineItem::Text {
            content: Arc::from("tiny"),
            style: initial_style(),
            measured_width: 30.0,
            measured_height: 12.0,
            baseline: 10.0,
        }];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].height, 30.0);
        assert_eq!(lines[0].baseline, 20.0);
    }

    #[test]
    fn lines_to_fragments_stacks_vertically() {
        let strut = default_strut();
        let items = vec![
            text_item("Line1", 50.0),
            InlineItem::ForcedBreak,
            text_item("Line2", 50.0),
        ];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        let line_height = lines[0].height;
        let frags = lines_to_fragments(lines, 200.0);
        assert_eq!(frags.len(), 2);
        assert_eq!(frags[0].offset.y, 0.0);
        assert_eq!(frags[1].offset.y, line_height);
    }

    // ---- Word-boundary breaking tests ----

    #[test]
    fn word_break_splits_at_space() {
        // "Hello World" as a single text item. Available width fits "Hello" but not "Hello World".
        let strut = default_strut();
        let items = vec![measured_text_item("Hello World")];
        // "Hello" and "Hello World" widths come from real font shaping.
        // Available: 50px — fits "Hello" (40) but not "Hello " (48) + "World".
        let lines = break_into_lines(&items, 50.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 2, "should split at space");
        // First line has "Hello", second has "World".
        assert!(lines[0].width > 0.0);
        assert!(lines[1].width > 0.0);
    }

    #[test]
    fn word_break_no_space_overflows() {
        // "Superlongword" — no whitespace, must overflow.
        let strut = default_strut();
        let items = vec![measured_text_item("Superlongword")];
        let lines = break_into_lines(&items, 30.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1, "no break point → overflow on single line");
        assert!(lines[0].width > 30.0);
    }

    #[test]
    fn word_break_multiple_words() {
        // "The quick brown fox" — should break into multiple lines.
        let strut = default_strut();
        let measurer = m();
        let items = vec![measured_text_item("The quick brown fox")];
        // Use a width that fits ~1 word but not the full text.
        let one_word_w = measurer.measure("quick", 16.0).width;
        let lines = break_into_lines(
            &items,
            one_word_w * 1.5,
            TextWrapMode::Wrap,
            &strut,
            &measurer,
        );
        assert!(
            lines.len() >= 2,
            "should break into at least 2 lines, got {}",
            lines.len()
        );
    }

    #[test]
    fn word_break_exact_fit() {
        // "AB CD" where available width exactly fits "AB".
        let strut = default_strut();
        let measurer = m();
        let items = vec![measured_text_item("AB CD")];
        // Use the real measured width of "AB" — no hardcoded values.
        let ab_width = measurer.measure("AB", 16.0).width;
        let lines = break_into_lines(&items, ab_width, TextWrapMode::Wrap, &strut, &measurer);
        assert_eq!(lines.len(), 2);
    }

    // ---- White-space mode tests ----

    #[test]
    fn nowrap_no_wrapping() {
        // TextWrapMode::Nowrap disables wrapping — even overflowing text stays on one line.
        let strut = default_strut();
        let items = vec![measured_text_item(
            "This is a long line that exceeds the width",
        )];
        let lines = break_into_lines(&items, 50.0, TextWrapMode::Nowrap, &strut, &m());
        assert_eq!(
            lines.len(),
            1,
            "nowrap should not wrap, got {} lines",
            lines.len()
        );
    }

    #[test]
    fn wrap_wraps_at_space() {
        // TextWrapMode::Wrap allows wrapping at whitespace.
        let strut = default_strut();
        let items = vec![measured_text_item("Hello World")];
        let lines = break_into_lines(&items, 50.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(
            lines.len(),
            2,
            "wrap should wrap at space, got {} lines",
            lines.len()
        );
    }

    #[test]
    fn wrap_preserves_forced_breaks() {
        // TextWrapMode::Wrap preserves forced breaks.
        let strut = default_strut();
        let items = vec![
            measured_text_item("First"),
            InlineItem::ForcedBreak,
            measured_text_item("Second"),
        ];
        let lines = break_into_lines(&items, 800.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(
            lines.len(),
            2,
            "wrap should preserve forced break, got {} lines",
            lines.len()
        );
    }

    // ---- Alignment-baseline tests ----

    #[test]
    fn vertical_align_baseline_default() {
        // The default style has AlignmentBaseline::Auto (treated as Baseline).
        // An item whose baseline matches the strut baseline needs no vertical shift.
        let strut = default_strut();
        let items = vec![text_item("x", strut.height())];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1);
        // Item baseline matches strut baseline → no vertical shift.
        let frag = &lines[0].fragments[0];
        assert!(
            frag.offset.y.abs() < 0.5,
            "baseline-aligned item should have offset_y ≈ 0, got {}",
            frag.offset.y,
        );
    }

    #[test]
    fn taller_item_increases_line_height() {
        // An item taller than the strut forces the line to grow to accommodate it.
        let strut = default_strut();
        let strut_height = strut.height();
        // Taller item: twice the strut height with matching ascent proportion.
        let tall_height = strut_height * 2.0;
        let tall_baseline = strut.ascent * 2.0;
        let items = vec![InlineItem::Text {
            content: Arc::from("tall"),
            style: initial_style(),
            measured_width: 50.0,
            measured_height: tall_height,
            baseline: tall_baseline,
        }];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].height >= tall_height,
            "line height must accommodate tall item: line={}, item={}",
            lines[0].height,
            tall_height,
        );
    }

    #[test]
    fn mixed_height_items_share_line() {
        // Two items of different heights on the same line align to the tallest baseline.
        let strut = default_strut();
        let small_baseline = strut.ascent * 0.5;
        let large_baseline = strut.ascent * 1.5;
        let items = vec![
            InlineItem::Text {
                content: Arc::from("small"),
                style: initial_style(),
                measured_width: 30.0,
                measured_height: small_baseline + 5.0,
                baseline: small_baseline,
            },
            InlineItem::Text {
                content: Arc::from("large"),
                style: initial_style(),
                measured_width: 30.0,
                measured_height: large_baseline + 8.0,
                baseline: large_baseline,
            },
        ];
        let lines = break_into_lines(&items, 200.0, TextWrapMode::Wrap, &strut, &m());
        assert_eq!(lines.len(), 1, "both items should fit on one line");
        // Line baseline is the max of all item baselines (clamped to strut minimum).
        assert!(
            lines[0].baseline >= large_baseline,
            "line baseline must accommodate the tallest item: {}",
            lines[0].baseline,
        );
    }

    #[test]
    #[ignore] // TODO: needs StyleEngine to construct styled ComputedValues
    fn vertical_align_middle() {}

    #[test]
    #[ignore] // TODO: needs StyleEngine to construct styled ComputedValues
    fn vertical_align_super_shifts_up() {}

    #[test]
    #[ignore] // TODO: needs StyleEngine to construct styled ComputedValues
    fn vertical_align_sub_shifts_down() {}
}
