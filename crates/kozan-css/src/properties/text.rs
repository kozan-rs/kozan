//! CSS text property parsers — `background-position`, `text-emphasis-style`,
//! `font-palette`, `initial-letter`, `hyphenate-character`, `hyphenate-limit-chars`,
//! `image-orientation`.

use cssparser::Parser;
use kozan_style::{
    Atom, FontPalette, HyphenateCharacter, HyphenateLimitChars, HyphenateLimitValue,
    ImageOrientation, InitialLetter, PositionComponent,
    TextEmphasisStyleValue, TextEmphasisFill, TextEmphasisShape,
};
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

impl crate::Parse for FontPalette {
    /// `normal | light | dark | <custom-ident>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let ident = input.expect_ident_cloned()?;
        match ident.as_ref() {
            "normal" => Ok(FontPalette::Normal),
            "light" => Ok(FontPalette::Light),
            "dark" => Ok(FontPalette::Dark),
            other => Ok(FontPalette::Custom(Atom::new(other))),
        }
    }
}

impl crate::Parse for InitialLetter {
    /// `normal | <number> [<integer> | drop | raise]?`
    ///
    /// - `drop` and no keyword → sink = ceil(size)
    /// - `raise` → sink = 1 (raised cap)
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(InitialLetter::Normal);
        }
        let size = input.expect_number()?;
        if size <= 0.0 {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        let sink = if let Ok(kw) = input.try_parse(|i| i.expect_ident_cloned()) {
            match kw.as_ref() {
                "drop" => size.ceil() as u32,
                "raise" => 1,
                _ => return Err(input.new_custom_error(crate::CustomError::InvalidValue)),
            }
        } else if let Ok(n) = input.try_parse(|i| i.expect_integer()) {
            if n < 1 {
                return Err(input.new_custom_error(crate::CustomError::InvalidValue));
            }
            n as u32
        } else {
            size.ceil() as u32
        };
        Ok(InitialLetter::Raised { size, sink })
    }
}

impl crate::Parse for HyphenateCharacter {
    /// `auto | <string>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(HyphenateCharacter::Auto);
        }
        let s = input.expect_string()?;
        Ok(HyphenateCharacter::String(s.as_ref().into()))
    }
}

impl crate::Parse for HyphenateLimitChars {
    /// `auto | <integer> <integer>? <integer>?`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        fn parse_slot<'i>(input: &mut Parser<'i, '_>) -> Option<HyphenateLimitValue> {
            if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
                return Some(HyphenateLimitValue::Auto);
            }
            input.try_parse(|i| i.expect_integer()).ok()
                .filter(|&n| n >= 0)
                .map(|n| HyphenateLimitValue::Integer(n as u32))
        }

        let total = parse_slot(input)
            .ok_or_else(|| input.new_custom_error(crate::CustomError::InvalidValue))?;
        let before = input.try_parse(|i| {
            parse_slot(i).ok_or_else(|| i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue))
        }).unwrap_or(HyphenateLimitValue::Auto);
        let after = input.try_parse(|i| {
            parse_slot(i).ok_or_else(|| i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue))
        }).unwrap_or(HyphenateLimitValue::Auto);

        Ok(HyphenateLimitChars { total, before, after })
    }
}

impl crate::Parse for ImageOrientation {
    /// `from-image | <angle>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| {
            let ident = i.expect_ident()?;
            if ident.eq_ignore_ascii_case("from-image") { Ok(()) }
            else { Err(i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue)) }
        }).is_ok() {
            return Ok(ImageOrientation::FromImage);
        }
        use cssparser::Token;
        let angle_deg = input.try_parse(|i| -> Result<f32, Error<'i>> {
            match i.next()? {
                Token::Dimension { value, unit, .. } => {
                    let deg = match unit.as_ref() {
                        "deg" => *value,
                        "grad" => *value * 360.0 / 400.0,
                        "rad" => *value * 180.0 / std::f32::consts::PI,
                        "turn" => *value * 360.0,
                        _ => return Err(i.new_custom_error(crate::CustomError::InvalidValue)),
                    };
                    Ok(deg)
                }
                _ => Err(i.new_custom_error(crate::CustomError::InvalidValue)),
            }
        })?;
        Ok(ImageOrientation::Angle(angle_deg))
    }
}
