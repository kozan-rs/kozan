//! CSS text property parsers — `background-position`, `text-emphasis-style`.

use cssparser::Parser;
use kozan_style::{Atom, PositionComponent, TextEmphasisStyleValue, TextEmphasisFill, TextEmphasisShape};
use kozan_style::specified::LengthPercentage;
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for PositionComponent {
    /// `left | center | right | top | bottom | <length-percentage>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            return css_match! { &ident,
                "left" => Ok(PositionComponent::Left),
                "center" => Ok(PositionComponent::Center),
                "right" => Ok(PositionComponent::Right),
                "top" => Ok(PositionComponent::Top),
                "bottom" => Ok(PositionComponent::Bottom),
                _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
            };
        }
        <LengthPercentage as crate::Parse>::parse(input).map(PositionComponent::Length)
    }
}

impl crate::Parse for TextEmphasisStyleValue {
    /// `none | [ [ filled | open ] || <shape> ] | <string>`
    ///
    /// Per CSS Text Decoration Level 3:
    /// - `filled` alone → `filled circle`
    /// - `open` alone → `open circle`
    /// - `dot` alone → `filled dot`
    /// - `open triangle` or `triangle open` (any order)
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(TextEmphasisStyleValue::None);
        }

        // Try string value.
        if let Ok(s) = input.try_parse(|i| i.expect_string().cloned()) {
            return Ok(TextEmphasisStyleValue::Custom(Atom::new(&*s)));
        }

        let mut fill: Option<TextEmphasisFill> = None;
        let mut shape: Option<TextEmphasisShape> = None;

        // Parse fill and shape in any order (both optional, but at least one required).
        for _ in 0..2 {
            if fill.is_none() {
                if let Ok(f) = input.try_parse(|i| -> Result<_, Error<'i>> {
                    let ident = i.expect_ident().cloned()?;
                    css_match! { &ident,
                        "filled" => Ok(TextEmphasisFill::Filled),
                        "open" => Ok(TextEmphasisFill::Open),
                        _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
                    }
                }) {
                    fill = Some(f);
                    continue;
                }
            }
            if shape.is_none() {
                if let Ok(s) = input.try_parse(|i| -> Result<_, Error<'i>> {
                    let ident = i.expect_ident().cloned()?;
                    css_match! { &ident,
                        "dot" => Ok(TextEmphasisShape::Dot),
                        "circle" => Ok(TextEmphasisShape::Circle),
                        "double-circle" => Ok(TextEmphasisShape::DoubleCircle),
                        "triangle" => Ok(TextEmphasisShape::Triangle),
                        "sesame" => Ok(TextEmphasisShape::Sesame),
                        _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
                    }
                }) {
                    shape = Some(s);
                    continue;
                }
            }
            break;
        }

        // At least one of fill or shape must be present.
        if fill.is_none() && shape.is_none() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }

        // Defaults: fill → filled, shape → circle.
        let fill = fill.unwrap_or(TextEmphasisFill::Filled);
        let shape = shape.unwrap_or(TextEmphasisShape::Circle);

        Ok(TextEmphasisStyleValue::Shape(fill, shape))
    }
}

