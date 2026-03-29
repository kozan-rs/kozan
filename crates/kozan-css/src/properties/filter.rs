//! CSS filter, clip-path, and shape-outside parsers.

use cssparser::Parser;
use kozan_style::{
    Atom, Color, FilterList, FilterFunction, ClipPath, ShapeOutside,
    InsetRect, CircleShape, EllipseShape,
};
use kozan_style_macros::css_match;
use crate::Error;

// FilterList

impl crate::Parse for FilterList {
    /// `none | <filter-function>+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(FilterList::None);
        }
        let mut filters = Vec::new();
        while let Ok(f) = input.try_parse(parse_filter_function) {
            filters.push(f);
        }
        if filters.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(FilterList::Filters(filters.into_boxed_slice()))
    }
}

fn parse_filter_function<'i>(input: &mut Parser<'i, '_>) -> Result<FilterFunction, Error<'i>> {
    // url() filter.
    if let Ok(url) = input.try_parse(|i| i.expect_url().map(|u| u.as_ref().to_owned())) {
        return Ok(FilterFunction::Url(Atom::new(&*url)));
    }

    let func = input.expect_function()?.clone();
    input.parse_nested_block(|i| {
        css_match! { &func,
            "blur" => Ok(FilterFunction::Blur(parse_px_value(i)?)),
            "brightness" => Ok(FilterFunction::Brightness(parse_number_or_percentage(i)?)),
            "contrast" => Ok(FilterFunction::Contrast(parse_number_or_percentage(i)?)),
            "grayscale" => Ok(FilterFunction::Grayscale(parse_number_or_percentage(i)?)),
            "hue-rotate" => Ok(FilterFunction::HueRotate(parse_angle_value(i)?)),
            "invert" => Ok(FilterFunction::Invert(parse_number_or_percentage(i)?)),
            "opacity" => Ok(FilterFunction::Opacity(parse_number_or_percentage(i)?)),
            "saturate" => Ok(FilterFunction::Saturate(parse_number_or_percentage(i)?)),
            "sepia" => Ok(FilterFunction::Sepia(parse_number_or_percentage(i)?)),
            "drop-shadow" => {
                let x = i.expect_number()?;
                let y = i.expect_number()?;
                let blur = i.try_parse(|i| i.expect_number()).unwrap_or(0.0);
                let color = i.try_parse(|i| <Color as crate::Parse>::parse(i))
                    .unwrap_or(Color::CurrentColor);
                Ok(FilterFunction::DropShadow { x, y, blur, color })
            },
            _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
        }
    })
}

// ClipPath + ShapeOutside (share basic shapes)

impl crate::Parse for ClipPath {
    /// `none | url() | inset() | circle() | ellipse() | polygon()`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ClipPath::None);
        }
        if let Ok(url) = input.try_parse(|i| i.expect_url().map(|u| u.as_ref().to_owned())) {
            return Ok(ClipPath::Url(Atom::new(&*url)));
        }
        let func = input.expect_function()?.clone();
        input.parse_nested_block(|i| {
            css_match! { &func,
                "inset" => parse_inset(i).map(|s| ClipPath::Inset(Box::new(s))),
                "circle" => parse_circle(i).map(|s| ClipPath::Circle(Box::new(s))),
                "ellipse" => parse_ellipse(i).map(|s| ClipPath::Ellipse(Box::new(s))),
                "polygon" => parse_polygon(i).map(|pts| ClipPath::Polygon(pts.into_boxed_slice())),
                _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
            }
        })
    }
}

impl crate::Parse for ShapeOutside {
    /// `none | url() | inset() | circle() | ellipse() | polygon()`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ShapeOutside::None);
        }
        if let Ok(url) = input.try_parse(|i| i.expect_url().map(|u| u.as_ref().to_owned())) {
            return Ok(ShapeOutside::Url(Atom::new(&*url)));
        }
        let func = input.expect_function()?.clone();
        input.parse_nested_block(|i| {
            css_match! { &func,
                "inset" => parse_inset(i).map(|s| ShapeOutside::Inset(Box::new(s))),
                "circle" => parse_circle(i).map(|s| ShapeOutside::Circle(Box::new(s))),
                "ellipse" => parse_ellipse(i).map(|s| ShapeOutside::Ellipse(Box::new(s))),
                "polygon" => parse_polygon(i).map(|pts| ShapeOutside::Polygon(pts.into_boxed_slice())),
                _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
            }
        })
    }
}

// Basic shapes

/// `inset(<top> <right>? <bottom>? <left>? [round <radii>]?)`
fn parse_inset<'i>(input: &mut Parser<'i, '_>) -> Result<InsetRect, Error<'i>> {
    let top = input.expect_number()?;
    let right = input.try_parse(|i| i.expect_number()).unwrap_or(top);
    let bottom = input.try_parse(|i| i.expect_number()).unwrap_or(top);
    let left = input.try_parse(|i| i.expect_number()).unwrap_or(right);
    let mut round = [0.0; 4];
    if input.try_parse(|i| i.expect_ident_matching("round")).is_ok() {
        round[0] = input.expect_number()?;
        round[1] = input.try_parse(|i| i.expect_number()).unwrap_or(round[0]);
        round[2] = input.try_parse(|i| i.expect_number()).unwrap_or(round[0]);
        round[3] = input.try_parse(|i| i.expect_number()).unwrap_or(round[1]);
    }
    Ok(InsetRect { top, right, bottom, left, round })
}

/// `circle(<radius>? [at <cx> <cy>]?)`
fn parse_circle<'i>(input: &mut Parser<'i, '_>) -> Result<CircleShape, Error<'i>> {
    let radius = input.try_parse(|i| i.expect_number()).unwrap_or(50.0);
    let (cx, cy) = parse_at_position(input)?;
    Ok(CircleShape { radius, cx, cy })
}

/// `ellipse(<rx> <ry>? [at <cx> <cy>]?)`
fn parse_ellipse<'i>(input: &mut Parser<'i, '_>) -> Result<EllipseShape, Error<'i>> {
    let rx = input.try_parse(|i| i.expect_number()).unwrap_or(50.0);
    let ry = input.try_parse(|i| i.expect_number()).unwrap_or(rx);
    let (cx, cy) = parse_at_position(input)?;
    Ok(EllipseShape { rx, ry, cx, cy })
}

/// `polygon(<point>,*)` where point = `<x> <y>`.
fn parse_polygon<'i>(input: &mut Parser<'i, '_>) -> Result<Vec<(f32, f32)>, Error<'i>> {
    let mut points = Vec::new();
    let x = input.expect_number()?;
    let y = input.expect_number()?;
    points.push((x, y));
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        let x = input.expect_number()?;
        let y = input.expect_number()?;
        points.push((x, y));
    }
    Ok(points)
}

fn parse_at_position<'i>(input: &mut Parser<'i, '_>) -> Result<(f32, f32), Error<'i>> {
    if input.try_parse(|i| i.expect_ident_matching("at")).is_ok() {
        let cx = input.expect_number()?;
        let cy = input.expect_number()?;
        Ok((cx, cy))
    } else {
        Ok((50.0, 50.0))
    }
}

// Helpers

fn parse_number_or_percentage<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    if let Ok(v) = input.try_parse(|i| i.expect_percentage()) {
        return Ok(v);
    }
    Ok(input.expect_number()?)
}

fn parse_px_value<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    use cssparser::Token;
    let location = input.current_source_location();
    match *input.next()? {
        Token::Dimension { value, ref unit, .. } if unit.eq_ignore_ascii_case("px") => Ok(value),
        Token::Number { value, .. } if value == 0.0 => Ok(0.0),
        _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
    }
}

fn parse_angle_value<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
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
