// Based on stylo_taffy (https://github.com/nicoburniske/blitz)
// Licensed under MIT / Apache-2.0 / MPL-2.0
// Modified for Kozan — adapted to stylo 0.14 + taffy 0.9

//! Conversion functions from Stylo computed style types to Taffy equivalents

/// Private module of type aliases so we can refer to stylo types with nicer names
pub(crate) mod stylo {
    pub(crate) use style::Atom;
    pub(crate) use style::properties::ComputedValues;
    pub(crate) use style::properties::generated::longhands::box_sizing::computed_value::T as BoxSizing;
    pub(crate) use style::properties::longhands::aspect_ratio::computed_value::T as AspectRatio;
    pub(crate) use style::properties::longhands::position::computed_value::T as Position;
    pub(crate) use style::values::computed::length_percentage::CalcLengthPercentage;
    pub(crate) use style::values::computed::length_percentage::Unpacked as UnpackedLengthPercentage;
    pub(crate) use style::values::computed::{BorderSideWidth, LengthPercentage, Percentage};
    pub(crate) use style::values::generics::NonNegative;
    pub(crate) use style::values::generics::length::{
        GenericLengthPercentageOrNormal, GenericMargin, GenericMaxSize, GenericSize,
    };
    pub(crate) use style::values::generics::position::{Inset as GenericInset, PreferredRatio};
    pub(crate) use style::values::specified::align::{AlignFlags, ContentDistribution};
    pub(crate) use style::values::specified::border::BorderStyle;
    pub(crate) use style::values::specified::box_::{
        Display, DisplayInside, DisplayOutside, Overflow,
    };
    pub(crate) use style::values::specified::position::GridTemplateAreas;
    pub(crate) use style::values::specified::position::NamedArea;
    pub(crate) use style_atoms::atom;
    pub(crate) type MarginVal = GenericMargin<LengthPercentage>;
    pub(crate) type InsetVal = GenericInset<Percentage, LengthPercentage>;
    pub(crate) type Size = GenericSize<NonNegative<LengthPercentage>>;
    pub(crate) type MaxSize = GenericMaxSize<NonNegative<LengthPercentage>>;

    pub(crate) type Gap = GenericLengthPercentageOrNormal<NonNegative<LengthPercentage>>;

    // Float/Clear available from Stylo but not from taffy 0.9

    pub(crate) use style::{
        computed_values::{flex_direction::T as FlexDirection, flex_wrap::T as FlexWrap},
        values::generics::flex::GenericFlexBasis,
    };

    pub(crate) type FlexBasis = GenericFlexBasis<Size>;

    pub(crate) use style::values::computed::text::TextAlign;

    pub(crate) use style::{
        computed_values::grid_auto_flow::T as GridAutoFlow,
        values::{
            computed::{GridLine, GridTemplateComponent, ImplicitGridTracks},
            generics::grid::{RepeatCount, TrackBreadth, TrackListValue, TrackSize},
            specified::GenericGridTemplateComponent,
        },
    };
}

use stylo::Atom;
use taffy::CompactLength;
use taffy::style_helpers::*;

#[inline]
#[must_use]
pub fn length_percentage(val: &stylo::LengthPercentage) -> taffy::LengthPercentage {
    match val.unpack() {
        stylo::UnpackedLengthPercentage::Calc(calc_ptr) => {
            let val = CompactLength::calc(std::ptr::from_ref::<stylo::CalcLengthPercentage>(
                calc_ptr,
            ) as *const ());
            // SAFETY: calc is a valid value for LengthPercentage
            unsafe { taffy::LengthPercentage::from_raw(val) }
        }
        stylo::UnpackedLengthPercentage::Length(len) => length(len.px()),
        stylo::UnpackedLengthPercentage::Percentage(percentage) => percent(percentage.0),
    }
}

#[inline]
#[must_use]
pub fn dimension(val: &stylo::Size) -> taffy::Dimension {
    match val {
        stylo::Size::LengthPercentage(val) => length_percentage(&val.0).into(),
        stylo::Size::Auto => taffy::Dimension::AUTO,

        // TODO: implement other values in Taffy
        stylo::Size::MaxContent => taffy::Dimension::AUTO,
        stylo::Size::MinContent => taffy::Dimension::AUTO,
        stylo::Size::FitContent => taffy::Dimension::AUTO,
        stylo::Size::FitContentFunction(_) => taffy::Dimension::AUTO,
        stylo::Size::Stretch => taffy::Dimension::AUTO,
        stylo::Size::WebkitFillAvailable => taffy::Dimension::AUTO,

        stylo::Size::AnchorSizeFunction(_) | stylo::Size::AnchorContainingCalcFunction(_) => {
            taffy::Dimension::AUTO
        }
    }
}

#[inline]
#[must_use]
pub fn max_size_dimension(val: &stylo::MaxSize) -> taffy::Dimension {
    match val {
        stylo::MaxSize::LengthPercentage(val) => length_percentage(&val.0).into(),
        stylo::MaxSize::None => taffy::Dimension::AUTO,

        // TODO: implement other values in Taffy
        stylo::MaxSize::MaxContent => taffy::Dimension::AUTO,
        stylo::MaxSize::MinContent => taffy::Dimension::AUTO,
        stylo::MaxSize::FitContent => taffy::Dimension::AUTO,
        stylo::MaxSize::FitContentFunction(_) => taffy::Dimension::AUTO,
        stylo::MaxSize::Stretch => taffy::Dimension::AUTO,
        stylo::MaxSize::WebkitFillAvailable => taffy::Dimension::AUTO,

        stylo::MaxSize::AnchorSizeFunction(_) | stylo::MaxSize::AnchorContainingCalcFunction(_) => {
            taffy::Dimension::AUTO
        }
    }
}

#[inline]
#[must_use]
pub fn margin(val: &stylo::MarginVal) -> taffy::LengthPercentageAuto {
    match val {
        stylo::MarginVal::Auto => taffy::LengthPercentageAuto::AUTO,
        stylo::MarginVal::LengthPercentage(val) => length_percentage(val).into(),

        stylo::MarginVal::AnchorSizeFunction(_)
        | stylo::MarginVal::AnchorContainingCalcFunction(_) => taffy::LengthPercentageAuto::AUTO,
    }
}

#[inline]
#[must_use]
pub fn border(
    width: &stylo::BorderSideWidth,
    style: stylo::BorderStyle,
) -> taffy::LengthPercentage {
    if style.none_or_hidden() {
        return taffy::style_helpers::zero();
    }
    taffy::style_helpers::length(width.0.to_f32_px())
}

#[inline]
#[must_use]
pub fn inset(val: &stylo::InsetVal) -> taffy::LengthPercentageAuto {
    match val {
        stylo::InsetVal::Auto => taffy::LengthPercentageAuto::AUTO,
        stylo::InsetVal::LengthPercentage(val) => length_percentage(val).into(),

        stylo::InsetVal::AnchorSizeFunction(_)
        | stylo::InsetVal::AnchorFunction(_)
        | stylo::InsetVal::AnchorContainingCalcFunction(_) => taffy::LengthPercentageAuto::AUTO,
    }
}

#[inline]
#[must_use]
pub fn is_block(input: stylo::Display) -> bool {
    matches!(input.outside(), stylo::DisplayOutside::Block)
        && matches!(
            input.inside(),
            stylo::DisplayInside::Flow | stylo::DisplayInside::FlowRoot
        )
}

#[inline]
#[must_use]
pub fn is_table(input: stylo::Display) -> bool {
    matches!(input.inside(), stylo::DisplayInside::Table)
}

#[inline]
#[must_use]
pub fn display(input: stylo::Display) -> taffy::Display {
    let mut display = match input.inside() {
        stylo::DisplayInside::None => taffy::Display::None,

        stylo::DisplayInside::Flex => taffy::Display::Flex,

        stylo::DisplayInside::Grid => taffy::Display::Grid,

        stylo::DisplayInside::Flow => taffy::Display::Block,

        stylo::DisplayInside::FlowRoot => taffy::Display::Block,

        stylo::DisplayInside::TableCell => taffy::Display::Block,
        // TODO(M7): display: contents — Chrome: LayoutNGBlockNode::IsDisplayContents() (no box, children promoted).
        // TODO(M9): display: table / table-cell / table-row — Chrome: LayoutNGTable (full table layout algorithm).
        stylo::DisplayInside::Table => taffy::Display::Grid,
        _ => {
            // println!("FALLBACK {:?} {:?}", input.inside(), input.outside());
            taffy::Display::DEFAULT
        }
    };

    match input.outside() {
        // This is probably redundant as I suspect display.inside() is always None
        // when display.outside() is None.
        stylo::DisplayOutside::None => display = taffy::Display::None,

        // TODO: Support flow and table layout
        stylo::DisplayOutside::Inline => {}
        stylo::DisplayOutside::Block => {}
        stylo::DisplayOutside::TableCaption => {}
        stylo::DisplayOutside::InternalTable => {}
    }

    display
}

#[inline]
#[must_use]
pub fn box_generation_mode(input: stylo::Display) -> taffy::BoxGenerationMode {
    match input.inside() {
        stylo::DisplayInside::None => taffy::BoxGenerationMode::None,
        // stylo::DisplayInside::Contents => display = taffy::BoxGenerationMode::Contents,
        _ => taffy::BoxGenerationMode::Normal,
    }
}

#[inline]
#[must_use]
pub fn box_sizing(input: stylo::BoxSizing) -> taffy::BoxSizing {
    match input {
        stylo::BoxSizing::BorderBox => taffy::BoxSizing::BorderBox,
        stylo::BoxSizing::ContentBox => taffy::BoxSizing::ContentBox,
    }
}

#[inline]
#[must_use]
pub fn position(input: stylo::Position) -> taffy::Position {
    match input {
        // TODO: support position:static
        stylo::Position::Relative => taffy::Position::Relative,
        stylo::Position::Static => taffy::Position::Relative,

        // TODO(M6): position: fixed — Chrome: PaintLayer + compositor (viewport-anchored layer).
        // TODO(M8): position: sticky — Chrome: StickyPositionScrollingConstraints + compositor.
        stylo::Position::Absolute => taffy::Position::Absolute,
        stylo::Position::Fixed => taffy::Position::Absolute,
        stylo::Position::Sticky => taffy::Position::Relative,
    }
}

#[inline]
#[must_use]
pub fn overflow(input: stylo::Overflow) -> taffy::Overflow {
    match input {
        stylo::Overflow::Visible => taffy::Overflow::Visible,
        stylo::Overflow::Clip => taffy::Overflow::Clip,
        stylo::Overflow::Hidden => taffy::Overflow::Hidden,
        stylo::Overflow::Scroll => taffy::Overflow::Scroll,
        // TODO: Support Overflow::Auto in Taffy
        stylo::Overflow::Auto => taffy::Overflow::Scroll,
    }
}

// direction removed — taffy 0.9 handles direction internally

#[inline]
#[must_use]
pub fn aspect_ratio(input: stylo::AspectRatio) -> Option<f32> {
    match input.ratio {
        stylo::PreferredRatio::None => None,
        stylo::PreferredRatio::Ratio(val) => Some(val.0.0 / val.1.0),
    }
}

#[inline]
#[must_use]
pub fn content_alignment(input: stylo::ContentDistribution) -> Option<taffy::AlignContent> {
    match input.primary().value() {
        stylo::AlignFlags::NORMAL => None,
        stylo::AlignFlags::AUTO => None,
        stylo::AlignFlags::START => Some(taffy::AlignContent::Start),
        stylo::AlignFlags::END => Some(taffy::AlignContent::End),
        stylo::AlignFlags::LEFT => Some(taffy::AlignContent::Start),
        stylo::AlignFlags::RIGHT => Some(taffy::AlignContent::End),
        stylo::AlignFlags::FLEX_START => Some(taffy::AlignContent::FlexStart),
        stylo::AlignFlags::STRETCH => Some(taffy::AlignContent::Stretch),
        stylo::AlignFlags::FLEX_END => Some(taffy::AlignContent::FlexEnd),
        stylo::AlignFlags::CENTER => Some(taffy::AlignContent::Center),
        stylo::AlignFlags::SPACE_BETWEEN => Some(taffy::AlignContent::SpaceBetween),
        stylo::AlignFlags::SPACE_AROUND => Some(taffy::AlignContent::SpaceAround),
        stylo::AlignFlags::SPACE_EVENLY => Some(taffy::AlignContent::SpaceEvenly),
        // Should never be hit. But no real reason to panic here.
        _ => None,
    }
}

#[inline]
#[must_use]
pub fn item_alignment(input: stylo::AlignFlags) -> Option<taffy::AlignItems> {
    match input.value() {
        stylo::AlignFlags::AUTO => None,
        stylo::AlignFlags::NORMAL => Some(taffy::AlignItems::Stretch),
        stylo::AlignFlags::STRETCH => Some(taffy::AlignItems::Stretch),
        stylo::AlignFlags::FLEX_START => Some(taffy::AlignItems::FlexStart),
        stylo::AlignFlags::FLEX_END => Some(taffy::AlignItems::FlexEnd),
        stylo::AlignFlags::SELF_START => Some(taffy::AlignItems::Start),
        stylo::AlignFlags::SELF_END => Some(taffy::AlignItems::End),
        stylo::AlignFlags::START => Some(taffy::AlignItems::Start),
        stylo::AlignFlags::END => Some(taffy::AlignItems::End),
        stylo::AlignFlags::LEFT => Some(taffy::AlignItems::Start),
        stylo::AlignFlags::RIGHT => Some(taffy::AlignItems::End),
        stylo::AlignFlags::CENTER => Some(taffy::AlignItems::Center),
        stylo::AlignFlags::BASELINE => Some(taffy::AlignItems::Baseline),
        // Should never be hit. But no real reason to panic here.
        _ => None,
    }
}

#[inline]
#[must_use]
pub fn gap(input: &stylo::Gap) -> taffy::LengthPercentage {
    match input {
        // For Flexbox and CSS Grid the "normal" value is 0px. This may need to be updated
        // if we ever implement multi-column layout.
        stylo::Gap::Normal => taffy::LengthPercentage::ZERO,
        stylo::Gap::LengthPercentage(val) => length_percentage(&val.0),
    }
}

#[inline]
pub(crate) fn text_align(input: stylo::TextAlign) -> taffy::TextAlign {
    match input {
        stylo::TextAlign::MozLeft => taffy::TextAlign::LegacyLeft,
        stylo::TextAlign::MozRight => taffy::TextAlign::LegacyRight,
        stylo::TextAlign::MozCenter => taffy::TextAlign::LegacyCenter,
        _ => taffy::TextAlign::Auto,
    }
}

#[inline]
#[must_use]
pub fn flex_basis(input: &stylo::FlexBasis) -> taffy::Dimension {
    // TODO: Support flex-basis: content in Taffy
    match input {
        stylo::FlexBasis::Content => taffy::Dimension::AUTO,
        stylo::FlexBasis::Size(size) => dimension(size),
    }
}

#[inline]
#[must_use]
pub fn flex_direction(input: stylo::FlexDirection) -> taffy::FlexDirection {
    match input {
        stylo::FlexDirection::Row => taffy::FlexDirection::Row,
        stylo::FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
        stylo::FlexDirection::Column => taffy::FlexDirection::Column,
        stylo::FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
    }
}

#[inline]
#[must_use]
pub fn flex_wrap(input: stylo::FlexWrap) -> taffy::FlexWrap {
    match input {
        stylo::FlexWrap::Wrap => taffy::FlexWrap::Wrap,
        stylo::FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        stylo::FlexWrap::Nowrap => taffy::FlexWrap::NoWrap,
    }
}

// Float/Clear: taffy 0.9 doesn't support these yet.
// Stylo computes them — we'll handle float layout in our layout engine directly.
// The computed values are available via ComputedValues::clone_float() / clone_clear().

// CSS Grid styles
// ===============

#[inline]
#[must_use]
pub fn grid_auto_flow(input: stylo::GridAutoFlow) -> taffy::GridAutoFlow {
    let is_row = input.contains(stylo::GridAutoFlow::ROW);
    let is_dense = input.contains(stylo::GridAutoFlow::DENSE);

    match (is_row, is_dense) {
        (true, false) => taffy::GridAutoFlow::Row,
        (true, true) => taffy::GridAutoFlow::RowDense,
        (false, false) => taffy::GridAutoFlow::Column,
        (false, true) => taffy::GridAutoFlow::ColumnDense,
    }
}

#[inline]
#[must_use]
pub fn grid_line(input: &stylo::GridLine) -> taffy::GridPlacement<Atom> {
    if input.is_auto() {
        taffy::GridPlacement::Auto
    } else if input.is_span {
        if input.ident.0 != stylo::atom!("") {
            taffy::GridPlacement::NamedSpan(
                input.ident.0.clone(),
                input.line_num.try_into().unwrap_or(u16::MAX),
            )
        } else {
            // CSS grid span values are small positive integers; clamp to u16 range.
            let span: u16 = input.line_num.try_into().unwrap_or(u16::MAX);
            taffy::GridPlacement::Span(span)
        }
    } else if input.ident.0 != stylo::atom!("") {
        // CSS grid named line numbers fit in i16 for any practical stylesheet.
        let line: i16 = input.line_num.try_into().unwrap_or(if input.line_num > 0 {
            i16::MAX
        } else {
            i16::MIN
        });
        taffy::GridPlacement::NamedLine(input.ident.0.clone(), line)
    } else if input.line_num != 0 {
        // CSS grid line numbers fit in i16 for any practical stylesheet.
        let line: i16 = input.line_num.try_into().unwrap_or(if input.line_num > 0 {
            i16::MAX
        } else {
            i16::MIN
        });
        taffy::style_helpers::line(line)
    } else {
        taffy::GridPlacement::Auto
    }
}

#[inline]
#[must_use]
pub fn grid_template_tracks(
    input: &stylo::GridTemplateComponent,
) -> Vec<taffy::GridTemplateComponent<Atom>> {
    match input {
        stylo::GenericGridTemplateComponent::None => Vec::new(),
        stylo::GenericGridTemplateComponent::TrackList(list) => list
            .values
            .iter()
            .map(|track| match track {
                stylo::TrackListValue::TrackSize(size) => {
                    taffy::GridTemplateComponent::Single(track_size(size))
                }
                stylo::TrackListValue::TrackRepeat(repeat) => {
                    taffy::GridTemplateComponent::Repeat(taffy::GridTemplateRepetition {
                        count: track_repeat(repeat.count),
                        tracks: repeat.track_sizes.iter().map(track_size).collect(),
                        line_names: repeat
                            .line_names
                            .iter()
                            .map(|line_name_set| {
                                line_name_set
                                    .iter()
                                    .map(|ident| ident.0.clone())
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>(),
                    })
                }
            })
            .collect(),

        // TODO: Implement subgrid and masonry
        stylo::GenericGridTemplateComponent::Subgrid(_) => Vec::new(),
        stylo::GenericGridTemplateComponent::Masonry => Vec::new(),
    }
}

#[inline]
#[must_use]
pub fn grid_template_line_names(
    input: &stylo::GridTemplateComponent,
) -> Option<super::wrapper::StyloLineNameIter<'_>> {
    match input {
        stylo::GenericGridTemplateComponent::None => None,
        stylo::GenericGridTemplateComponent::TrackList(list) => {
            Some(super::wrapper::StyloLineNameIter::new(&list.line_names))
        }

        // TODO: Implement subgrid and masonry
        stylo::GenericGridTemplateComponent::Subgrid(_) => None,
        stylo::GenericGridTemplateComponent::Masonry => None,
    }
}

#[inline]
#[must_use]
pub fn grid_template_area(input: &stylo::NamedArea) -> taffy::GridTemplateArea<Atom> {
    // Grid template area indices are small non-negative integers derived from the
    // number of rows/columns in the template. Clamp to u16 range for safety.
    taffy::GridTemplateArea {
        name: input.name.clone(),
        row_start: input.rows.start.try_into().unwrap_or(u16::MAX),
        row_end: input.rows.end.try_into().unwrap_or(u16::MAX),
        column_start: input.columns.start.try_into().unwrap_or(u16::MAX),
        column_end: input.columns.end.try_into().unwrap_or(u16::MAX),
    }
}

#[inline]
fn grid_template_areas(input: &stylo::GridTemplateAreas) -> Vec<taffy::GridTemplateArea<Atom>> {
    match input {
        stylo::GridTemplateAreas::None => Vec::new(),
        stylo::GridTemplateAreas::Areas(template_areas_arc) => {
            super::wrapper::GridAreaWrapper(&template_areas_arc.0.areas)
                .into_iter()
                .collect()
        }
    }
}

#[inline]
pub fn grid_auto_tracks(input: &stylo::ImplicitGridTracks) -> Vec<taffy::TrackSizingFunction> {
    input.0.iter().map(track_size).collect()
}

#[inline]
#[must_use]
pub fn track_repeat(input: stylo::RepeatCount<i32>) -> taffy::RepetitionCount {
    match input {
        stylo::RepeatCount::Number(val) => {
            taffy::RepetitionCount::Count(val.try_into().unwrap_or(u16::MAX))
        }
        stylo::RepeatCount::AutoFill => taffy::RepetitionCount::AutoFill,
        stylo::RepeatCount::AutoFit => taffy::RepetitionCount::AutoFit,
    }
}

#[inline]
#[must_use]
pub fn track_size(input: &stylo::TrackSize<stylo::LengthPercentage>) -> taffy::TrackSizingFunction {
    use taffy::MaxTrackSizingFunction;

    match input {
        stylo::TrackSize::Breadth(breadth) => taffy::MinMax {
            min: min_track(breadth),
            max: max_track(breadth),
        },
        stylo::TrackSize::Minmax(min, max) => taffy::MinMax {
            min: min_track(min),
            max: max_track(max),
        },
        stylo::TrackSize::FitContent(limit) => taffy::MinMax {
            min: taffy::MinTrackSizingFunction::AUTO,
            max: match limit {
                stylo::TrackBreadth::Breadth(lp) => {
                    MaxTrackSizingFunction::fit_content(length_percentage(lp))
                }

                stylo::TrackBreadth::Fr(_)
                | stylo::TrackBreadth::Auto
                | stylo::TrackBreadth::MinContent
                | stylo::TrackBreadth::MaxContent => MaxTrackSizingFunction::AUTO,
            },
        },
    }
}

#[inline]
#[must_use]
pub fn min_track(
    input: &stylo::TrackBreadth<stylo::LengthPercentage>,
) -> taffy::MinTrackSizingFunction {
    use taffy::prelude::*;
    match input {
        stylo::TrackBreadth::Breadth(lp) => {
            taffy::MinTrackSizingFunction::from(length_percentage(lp))
        }
        stylo::TrackBreadth::Fr(_) => taffy::MinTrackSizingFunction::AUTO,
        stylo::TrackBreadth::Auto => taffy::MinTrackSizingFunction::AUTO,
        stylo::TrackBreadth::MinContent => taffy::MinTrackSizingFunction::MIN_CONTENT,
        stylo::TrackBreadth::MaxContent => taffy::MinTrackSizingFunction::MAX_CONTENT,
    }
}

#[inline]
#[must_use]
pub fn max_track(
    input: &stylo::TrackBreadth<stylo::LengthPercentage>,
) -> taffy::MaxTrackSizingFunction {
    use taffy::prelude::*;

    match input {
        stylo::TrackBreadth::Breadth(lp) => {
            taffy::MaxTrackSizingFunction::from(length_percentage(lp))
        }
        stylo::TrackBreadth::Fr(val) => taffy::MaxTrackSizingFunction::from_fr(*val),
        stylo::TrackBreadth::Auto => taffy::MaxTrackSizingFunction::AUTO,
        stylo::TrackBreadth::MinContent => taffy::MaxTrackSizingFunction::MIN_CONTENT,
        stylo::TrackBreadth::MaxContent => taffy::MaxTrackSizingFunction::MAX_CONTENT,
    }
}

#[cfg(test)]
mod tests {
    use style::values::CustomIdent;
    use style_atoms::atom;

    use super::stylo::RepeatCount;
    use super::*;

    fn make_grid_line(is_span: bool, line_num: i32, atom: style::Atom) -> stylo::GridLine {
        stylo::GridLine {
            ident: CustomIdent(atom),
            line_num,
            is_span,
        }
    }

    // ── grid_line ──

    #[test]
    fn grid_line_span_numeric() {
        let gl = make_grid_line(true, 3, atom!(""));
        assert_eq!(grid_line(&gl), taffy::GridPlacement::Span(3));
    }

    #[test]
    fn grid_line_span_large_clamps_to_u16_max() {
        let gl = make_grid_line(true, 70_000, atom!(""));
        assert_eq!(grid_line(&gl), taffy::GridPlacement::Span(u16::MAX));
    }

    #[test]
    fn grid_line_named_span() {
        let gl = make_grid_line(true, 2, style::Atom::from("sidebar"));
        assert_eq!(
            grid_line(&gl),
            taffy::GridPlacement::NamedSpan(style::Atom::from("sidebar"), 2),
        );
    }

    #[test]
    fn grid_line_auto_when_default() {
        let gl = stylo::GridLine::auto();
        assert_eq!(grid_line(&gl), taffy::GridPlacement::Auto);
    }

    #[test]
    fn grid_line_numeric_line_number() {
        let gl = make_grid_line(false, 4, atom!(""));
        // line() returns a GridPlacement<S> — turbofish specifies the full return type.
        let expected: taffy::GridPlacement<style::Atom> = taffy::style_helpers::line(4);
        assert_eq!(grid_line(&gl), expected);
    }

    #[test]
    fn grid_line_named_line() {
        let gl = make_grid_line(false, 1, style::Atom::from("main-start"));
        assert_eq!(
            grid_line(&gl),
            taffy::GridPlacement::NamedLine(style::Atom::from("main-start"), 1),
        );
    }

    // ── track_repeat ──

    #[test]
    fn track_repeat_auto_fit() {
        assert_eq!(
            track_repeat(RepeatCount::AutoFit),
            taffy::RepetitionCount::AutoFit,
        );
    }

    #[test]
    fn track_repeat_auto_fill() {
        assert_eq!(
            track_repeat(RepeatCount::AutoFill),
            taffy::RepetitionCount::AutoFill,
        );
    }

    #[test]
    fn track_repeat_count() {
        assert_eq!(
            track_repeat(RepeatCount::Number(5)),
            taffy::RepetitionCount::Count(5),
        );
    }

    #[test]
    fn track_repeat_count_large_clamps_to_u16_max() {
        assert_eq!(
            track_repeat(RepeatCount::Number(70_000)),
            taffy::RepetitionCount::Count(u16::MAX),
        );
    }
}

/// Eagerly convert an entire [`stylo::ComputedValues`] into a [`taffy::Style`]
#[must_use]
pub fn to_taffy_style(style: &stylo::ComputedValues) -> taffy::Style<Atom> {
    let display = style.clone_display();
    let pos = style.get_position();
    let margin = style.get_margin();
    let padding = style.get_padding();
    let border = style.get_border();

    taffy::Style {
        dummy: core::marker::PhantomData,
        display: self::display(display),
        box_sizing: self::box_sizing(style.clone_box_sizing()),
        item_is_table: display.inside() == stylo::DisplayInside::Table,
        item_is_replaced: false,
        position: self::position(style.clone_position()),
        overflow: taffy::Point {
            x: self::overflow(style.clone_overflow_x()),
            y: self::overflow(style.clone_overflow_y()),
        },
        // direction handled by layout engine, not taffy style
        scrollbar_width: 0.0,

        size: taffy::Size {
            width: self::dimension(&pos.width),
            height: self::dimension(&pos.height),
        },
        min_size: taffy::Size {
            width: self::dimension(&pos.min_width),
            height: self::dimension(&pos.min_height),
        },
        max_size: taffy::Size {
            width: self::max_size_dimension(&pos.max_width),
            height: self::max_size_dimension(&pos.max_height),
        },
        aspect_ratio: self::aspect_ratio(pos.aspect_ratio),

        inset: taffy::Rect {
            left: self::inset(&pos.left),
            right: self::inset(&pos.right),
            top: self::inset(&pos.top),
            bottom: self::inset(&pos.bottom),
        },
        margin: taffy::Rect {
            left: self::margin(&margin.margin_left),
            right: self::margin(&margin.margin_right),
            top: self::margin(&margin.margin_top),
            bottom: self::margin(&margin.margin_bottom),
        },
        padding: taffy::Rect {
            left: self::length_percentage(&padding.padding_left.0),
            right: self::length_percentage(&padding.padding_right.0),
            top: self::length_percentage(&padding.padding_top.0),
            bottom: self::length_percentage(&padding.padding_bottom.0),
        },
        border: taffy::Rect {
            left: self::border(&border.border_left_width, border.border_left_style),
            right: self::border(&border.border_right_width, border.border_right_style),
            top: self::border(&border.border_top_width, border.border_top_style),
            bottom: self::border(&border.border_bottom_width, border.border_bottom_style),
        },

        // Gap
        gap: taffy::Size {
            width: self::gap(&pos.column_gap),
            height: self::gap(&pos.row_gap),
        },

        // Alignment
        align_content: self::content_alignment(pos.align_content),
        justify_content: self::content_alignment(pos.justify_content),
        align_items: self::item_alignment(pos.align_items.0),
        align_self: self::item_alignment(pos.align_self.0),
        justify_items: self::item_alignment((pos.justify_items.computed.0).0),
        justify_self: self::item_alignment(pos.justify_self.0),
        text_align: self::text_align(style.clone_text_align()),

        // Flexbox
        flex_direction: self::flex_direction(pos.flex_direction),
        flex_wrap: self::flex_wrap(pos.flex_wrap),
        flex_grow: pos.flex_grow.0,
        flex_shrink: pos.flex_shrink.0,
        flex_basis: self::flex_basis(&pos.flex_basis),

        // Grid
        grid_auto_flow: self::grid_auto_flow(pos.grid_auto_flow),
        grid_template_rows: self::grid_template_tracks(&pos.grid_template_rows),
        grid_template_columns: self::grid_template_tracks(&pos.grid_template_columns),
        grid_template_row_names: match self::grid_template_line_names(&pos.grid_template_rows) {
            Some(iter) => iter
                .map(|line_name_set| line_name_set.cloned().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            None => Vec::new(),
        },
        grid_template_column_names: match self::grid_template_line_names(&pos.grid_template_columns)
        {
            Some(iter) => iter
                .map(|line_name_set| line_name_set.cloned().collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            None => Vec::new(),
        },
        grid_template_areas: self::grid_template_areas(&pos.grid_template_areas),
        grid_auto_rows: self::grid_auto_tracks(&pos.grid_auto_rows),
        grid_auto_columns: self::grid_auto_tracks(&pos.grid_auto_columns),
        grid_row: taffy::Line {
            start: self::grid_line(&pos.grid_row_start),
            end: self::grid_line(&pos.grid_row_end),
        },
        grid_column: taffy::Line {
            start: self::grid_line(&pos.grid_column_start),
            end: self::grid_line(&pos.grid_column_end),
        },
    }
}
