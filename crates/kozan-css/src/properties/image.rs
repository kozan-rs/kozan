//! CSS image and gradient parsers.

use cssparser::Parser;
use kozan_style::{
    Atom, Color, Image, ImageList,
    LinearGradient, RadialGradient, ConicGradient, RadialShape, ColorStop,
};
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for Image {
    /// `none | url() | linear-gradient() | radial-gradient() | conic-gradient()`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Image::None);
        }
        if let Ok(url) = input.try_parse(|i| i.expect_url().map(|u| u.as_ref().to_owned())) {
            return Ok(Image::Url(Atom::new(&*url)));
        }
        let func = input.expect_function()?.clone();
        input.parse_nested_block(|i| {
            css_match! { &func,
                "linear-gradient" => parse_linear_gradient(i, false).map(|g| Image::LinearGradient(Box::new(g))),
                "repeating-linear-gradient" => parse_linear_gradient(i, true).map(|g| Image::LinearGradient(Box::new(g))),
                "radial-gradient" => parse_radial_gradient(i).map(|g| Image::RadialGradient(Box::new(g))),
                "repeating-radial-gradient" => parse_radial_gradient(i).map(|g| Image::RadialGradient(Box::new(g))),
                "conic-gradient" => parse_conic_gradient(i).map(|g| Image::ConicGradient(Box::new(g))),
                "repeating-conic-gradient" => parse_conic_gradient(i).map(|g| Image::ConicGradient(Box::new(g))),
                "url" => {
                    let url = i.expect_string()?.clone();
                    Ok(Image::Url(Atom::new(&*url)))
                },
                _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
            }
        })
    }
}

impl crate::Parse for ImageList {
    /// `none | <image>#`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ImageList::None);
        }
        let first = <Image as crate::Parse>::parse(input)?;
        let mut images = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            images.push(<Image as crate::Parse>::parse(input)?);
        }
        Ok(ImageList::Images(images.into_boxed_slice()))
    }
}

// Gradient parsers

/// `linear-gradient([<angle> | to <side>,]? <color-stop-list>)`
fn parse_linear_gradient<'i>(input: &mut Parser<'i, '_>, _repeating: bool) -> Result<LinearGradient, Error<'i>> {
    let angle = input.try_parse(|i| -> Result<f32, Error<'i>> {
        // Try angle value.
        if let Ok(a) = i.try_parse(parse_angle) {
            i.expect_comma()?;
            return Ok(a);
        }
        // Try `to <side>`.
        i.expect_ident_matching("to")?;
        let side = i.expect_ident()?.clone();
        let a = css_match! { &side,
            "top" => 0.0,
            "right" => 90.0,
            "bottom" => 180.0,
            "left" => 270.0,
            _ => return Err(i.new_custom_error(crate::CustomError::InvalidValue))
        };
        i.expect_comma()?;
        Ok(a)
    }).unwrap_or(180.0); // Default: top to bottom.

    let stops = parse_color_stop_list(input)?;
    Ok(LinearGradient { angle, stops: stops.into_boxed_slice() })
}

/// `radial-gradient([<shape>]? [at <position>]?, <color-stop-list>)`
fn parse_radial_gradient<'i>(input: &mut Parser<'i, '_>) -> Result<RadialGradient, Error<'i>> {
    let shape = input.try_parse(|i| -> Result<RadialShape, Error<'i>> {
        let ident = i.expect_ident()?.clone();
        let s = css_match! { &ident,
            "circle" => RadialShape::Circle,
            "ellipse" => RadialShape::Ellipse,
            _ => return Err(i.new_custom_error(crate::CustomError::InvalidValue))
        };
        // Skip optional `at <position>`.
        if i.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
            let _ = i.try_parse(|i| i.expect_number());
            let _ = i.try_parse(|i| i.expect_number());
        }
        i.expect_comma()?;
        Ok(s)
    }).unwrap_or(RadialShape::Ellipse);

    let stops = parse_color_stop_list(input)?;
    Ok(RadialGradient { shape, stops: stops.into_boxed_slice() })
}

/// `conic-gradient([from <angle>]? [at <position>]?, <color-stop-list>)`
fn parse_conic_gradient<'i>(input: &mut Parser<'i, '_>) -> Result<ConicGradient, Error<'i>> {
    let from_angle = input.try_parse(|i| -> Result<f32, Error<'i>> {
        i.expect_ident_matching("from")?;
        let a = parse_angle(i)?;
        // Skip optional `at <position>`.
        if i.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
            let _ = i.try_parse(|i| i.expect_number());
            let _ = i.try_parse(|i| i.expect_number());
        }
        i.expect_comma()?;
        Ok(a)
    }).unwrap_or(0.0);

    let stops = parse_color_stop_list(input)?;
    Ok(ConicGradient { from_angle, stops: stops.into_boxed_slice() })
}

// Color stops

fn parse_color_stop_list<'i>(input: &mut Parser<'i, '_>) -> Result<Vec<ColorStop>, Error<'i>> {
    let first = parse_color_stop(input)?;
    let mut stops = vec![first];
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        stops.push(parse_color_stop(input)?);
    }
    Ok(stops)
}

fn parse_color_stop<'i>(input: &mut Parser<'i, '_>) -> Result<ColorStop, Error<'i>> {
    let color = <Color as crate::Parse>::parse(input)?;
    let position = input.try_parse(|i| i.expect_percentage()).ok();
    Ok(ColorStop { color, position })
}

// Angle helper

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
