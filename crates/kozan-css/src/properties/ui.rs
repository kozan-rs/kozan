//! CSS UI property parsers — `scroll-snap-type`, `scrollbar-color`, `scrollbar-gutter`, `touch-action`, `will-change`.

use cssparser::Parser;
use kozan_style::{
    Atom, Color, ScrollSnapType, ScrollbarColor, ScrollbarGutter, TouchActionValue, WillChange,
    ScrollSnapAxis, ScrollSnapStrictness, Zoom,
};
use crate::Error;

impl crate::Parse for ScrollbarGutter {
    /// `auto | stable [both-edges]?`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let ident = input.expect_ident_cloned()?;
        match ident.as_ref() {
            "auto" => Ok(ScrollbarGutter::Auto),
            "stable" => {
                // Optionally followed by `both-edges`.
                if input.try_parse(|i| i.expect_ident_matching("both-edges")).is_ok() {
                    Ok(ScrollbarGutter::StableBothEdges)
                } else {
                    Ok(ScrollbarGutter::Stable)
                }
            }
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue)),
        }
    }
}

impl crate::Parse for ScrollSnapType {
    /// `none | <axis> [mandatory | proximity]?`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ScrollSnapType::None);
        }
        let axis = <ScrollSnapAxis as crate::Parse>::parse(input)?;
        let strictness = input.try_parse(|i| <ScrollSnapStrictness as crate::Parse>::parse(i))
            .unwrap_or(ScrollSnapStrictness::default());
        Ok(ScrollSnapType::Snap { axis, strictness })
    }
}

impl crate::Parse for ScrollbarColor {
    /// `auto | <color>{2}` (thumb track).
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(ScrollbarColor::Auto);
        }
        let thumb = <Color as crate::Parse>::parse(input)?;
        let track = <Color as crate::Parse>::parse(input)?;
        Ok(ScrollbarColor::Colors { thumb, track })
    }
}

impl crate::Parse for TouchActionValue {
    /// `auto | none | manipulation | <touch-action-flags>+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(TouchActionValue::Auto);
        }
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(TouchActionValue::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("manipulation")).is_ok() {
            return Ok(TouchActionValue::Manipulation);
        }
        let flags = <kozan_style::TouchAction as crate::Parse>::parse(input)?;
        Ok(TouchActionValue::Flags(flags))
    }
}

impl crate::Parse for Zoom {
    /// `normal | reset | <number> | <percentage>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            return match ident.as_ref() {
                "normal" => Ok(Zoom::Normal),
                "reset" => Ok(Zoom::Reset),
                _ => Err(input.new_custom_error(crate::CustomError::InvalidValue)),
            };
        }
        if let Ok(pct) = input.try_parse(|i| i.expect_percentage()) {
            return Ok(Zoom::Number(pct));
        }
        let num = input.expect_number()?;
        Ok(Zoom::Number(num))
    }
}

impl crate::Parse for WillChange {
    /// `auto | <ident>#`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(WillChange::Auto);
        }
        let first = input.expect_ident()?;
        let mut props = vec![Atom::new(&*first)];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            let ident = input.expect_ident()?;
            props.push(Atom::new(&*ident));
        }
        Ok(WillChange::Properties(props.into_boxed_slice()))
    }
}
