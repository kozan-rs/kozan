//! CSS SVG property parsers — `stroke-dasharray`, `paint-order`.

use cssparser::Parser;
use kozan_style::{Atom, Color, PaintOrder, PaintTarget, StrokeDasharray, SvgPaint};
use kozan_style::specified::LengthPercentage;
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for StrokeDasharray {
    /// `none | <length-percentage>+#` (comma-separated list).
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(StrokeDasharray::None);
        }
        let first = <LengthPercentage as crate::Parse>::parse(input)?;
        let mut values = vec![first];
        loop {
            // Accept both space-separated and comma-separated.
            let _ = input.try_parse(|i| i.expect_comma());
            match input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i)) {
                Ok(v) => values.push(v),
                Err(_) => break,
            }
        }
        Ok(StrokeDasharray::Values(values.into_boxed_slice()))
    }
}

impl crate::Parse for PaintOrder {
    /// `normal | [fill || stroke || markers]+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(PaintOrder::Normal);
        }
        let mut targets = Vec::with_capacity(3);
        while let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            let target = css_match! { &ident,
                "fill" => PaintTarget::Fill,
                "stroke" => PaintTarget::Stroke,
                "markers" => PaintTarget::Markers,
                _ => return Err(input.new_custom_error(crate::CustomError::InvalidValue))
            };
            targets.push(target);
        }
        if targets.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(PaintOrder::Custom(targets.into_boxed_slice()))
    }
}

impl crate::Parse for SvgPaint {
    /// `none | <color> | url() [<color>]? | context-fill | context-stroke`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(SvgPaint::None);
        }
        if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            return css_match! { &ident,
                "context-fill" => Ok(SvgPaint::ContextFill),
                "context-stroke" => Ok(SvgPaint::ContextStroke),
                _ => {
                    // Could be a named color — re-parse the full color.
                    Err(input.new_custom_error(crate::CustomError::InvalidValue))
                }
            };
        }
        if let Ok(url) = input.try_parse(|i| i.expect_url().map(|u| u.as_ref().to_owned())) {
            let fallback = input.try_parse(|i| <Color as crate::Parse>::parse(i)).ok();
            return Ok(SvgPaint::Url(Atom::new(&*url), fallback));
        }
        let color = <Color as crate::Parse>::parse(input)?;
        Ok(SvgPaint::Color(color))
    }
}
