//! CSS border-image property parsers.

use cssparser::Parser;
use kozan_style::{BorderImageSlice, BorderImageRepeat, BorderImageRepeatMode};
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for BorderImageSlice {
    /// `<number-or-percentage>{1,4} [fill]?`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let top = parse_slice_value(input)?;
        let right = input.try_parse(parse_slice_value).unwrap_or(top);
        let bottom = input.try_parse(parse_slice_value).unwrap_or(top);
        let left = input.try_parse(parse_slice_value).unwrap_or(right);
        let fill = input.try_parse(|i| i.expect_ident_matching("fill")).is_ok();
        Ok(BorderImageSlice { top, right, bottom, left, fill })
    }
}

fn parse_slice_value<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    if let Ok(v) = input.try_parse(|i| i.expect_percentage()) {
        return Ok(v * 100.0);
    }
    Ok(input.expect_number()?)
}

fn parse_repeat_mode<'i>(input: &mut Parser<'i, '_>) -> Result<BorderImageRepeatMode, Error<'i>> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    css_match! { &ident,
        "stretch" => Ok(BorderImageRepeatMode::Stretch),
        "repeat" => Ok(BorderImageRepeatMode::Repeat),
        "round" => Ok(BorderImageRepeatMode::Round),
        "space" => Ok(BorderImageRepeatMode::Space),
        _ => Err(location.new_custom_error(crate::CustomError::InvalidValue))
    }
}

impl crate::Parse for BorderImageRepeat {
    /// `<repeat-mode>{1,2}` — horizontal [vertical].
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let horizontal = parse_repeat_mode(input)?;
        let vertical = input.try_parse(parse_repeat_mode).unwrap_or(horizontal);
        Ok(BorderImageRepeat { horizontal, vertical })
    }
}
