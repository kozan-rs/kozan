//! CSS transform function parser.

use cssparser::Parser;
use kozan_style::{TransformList, TransformFunction};
use kozan_style::specified::LengthPercentage;
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for TransformList {
    /// `none | <transform-function>+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(TransformList::None);
        }
        let mut funcs = Vec::new();
        while let Ok(f) = input.try_parse(parse_transform_function) {
            funcs.push(f);
        }
        if funcs.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(TransformList::Functions(funcs.into_boxed_slice()))
    }
}

fn parse_lp<'i>(input: &mut Parser<'i, '_>) -> Result<LengthPercentage, Error<'i>> {
    <LengthPercentage as crate::Parse>::parse(input)
}

fn parse_angle<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    use cssparser::Token;
    let location = input.current_source_location();
    match *input.next()? {
        Token::Number { value, .. } => Ok(value),
        Token::Dimension { value, ref unit, .. } => {
            css_match! { unit,
                "deg" => Ok(value),
                "rad" => Ok(value.to_degrees()),
                "grad" => Ok(value * 0.9),
                "turn" => Ok(value * 360.0),
                _ => Err(location.new_custom_error(crate::CustomError::InvalidValue))
            }
        }
        _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
    }
}

fn parse_transform_function<'i>(input: &mut Parser<'i, '_>) -> Result<TransformFunction, Error<'i>> {
    let func = input.expect_function()?.clone();
    input.parse_nested_block(|i| {
        css_match! { &func,
            "translate" => {
                let x = parse_lp(i)?;
                let y = if i.try_parse(|i| i.expect_comma()).is_ok() {
                    parse_lp(i)?
                } else {
                    LengthPercentage::default()
                };
                Ok(TransformFunction::Translate(x, y))
            },
            "translatex" => Ok(TransformFunction::TranslateX(parse_lp(i)?)),
            "translatey" => Ok(TransformFunction::TranslateY(parse_lp(i)?)),
            "translatez" => Ok(TransformFunction::TranslateZ(parse_lp(i)?)),
            "translate3d" => {
                let x = parse_lp(i)?; i.expect_comma()?;
                let y = parse_lp(i)?; i.expect_comma()?;
                let z = parse_lp(i)?;
                Ok(TransformFunction::Translate3d(x, y, z))
            },
            "scale" => {
                let x = i.expect_number()?;
                let y = if i.try_parse(|i| i.expect_comma()).is_ok() {
                    i.expect_number()?
                } else { x };
                Ok(TransformFunction::Scale(x, y))
            },
            "scalex" => Ok(TransformFunction::ScaleX(i.expect_number()?)),
            "scaley" => Ok(TransformFunction::ScaleY(i.expect_number()?)),
            "scalez" => Ok(TransformFunction::ScaleZ(i.expect_number()?)),
            "scale3d" => {
                let x = i.expect_number()?; i.expect_comma()?;
                let y = i.expect_number()?; i.expect_comma()?;
                let z = i.expect_number()?;
                Ok(TransformFunction::Scale3d(x, y, z))
            },
            "rotate" => Ok(TransformFunction::Rotate(parse_angle(i)?)),
            "rotatex" => Ok(TransformFunction::RotateX(parse_angle(i)?)),
            "rotatey" => Ok(TransformFunction::RotateY(parse_angle(i)?)),
            "rotatez" => Ok(TransformFunction::RotateZ(parse_angle(i)?)),
            "rotate3d" => {
                let x = i.expect_number()?; i.expect_comma()?;
                let y = i.expect_number()?; i.expect_comma()?;
                let z = i.expect_number()?; i.expect_comma()?;
                let a = parse_angle(i)?;
                Ok(TransformFunction::Rotate3d(x, y, z, a))
            },
            "skew" => {
                let x = parse_angle(i)?;
                let y = if i.try_parse(|i| i.expect_comma()).is_ok() {
                    parse_angle(i)?
                } else { 0.0 };
                Ok(TransformFunction::Skew(x, y))
            },
            "skewx" => Ok(TransformFunction::SkewX(parse_angle(i)?)),
            "skewy" => Ok(TransformFunction::SkewY(parse_angle(i)?)),
            "perspective" => Ok(TransformFunction::Perspective(parse_lp(i)?)),
            "matrix" => {
                let mut m = [0.0f64; 6];
                m[0] = i.expect_number()? as f64;
                for v in &mut m[1..] {
                    i.expect_comma()?;
                    *v = i.expect_number()? as f64;
                }
                Ok(TransformFunction::Matrix(Box::new(m)))
            },
            "matrix3d" => {
                let mut m = [0.0f64; 16];
                m[0] = i.expect_number()? as f64;
                for v in &mut m[1..] {
                    i.expect_comma()?;
                    *v = i.expect_number()? as f64;
                }
                Ok(TransformFunction::Matrix3d(Box::new(m)))
            },
            _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
        }
    })
}
