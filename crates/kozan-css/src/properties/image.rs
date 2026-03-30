//! CSS image and gradient parsers, plus background/mask layer list parsers.

use cssparser::Parser;
use kozan_style::{
    Atom, Color, Image, ImageList,
    LinearGradient, RadialGradient, ConicGradient, RadialShape, ColorStop,
    PositionComponentList, BackgroundSizeList, BackgroundRepeatList,
    BackgroundAttachmentList, BackgroundClipList, BackgroundOriginList,
    MaskModeList, MaskClipList, MaskCompositeList, Position2DList,
    BackgroundRepeat, BackgroundAttachment, BackgroundClip, BackgroundOrigin,
    BackgroundSize, MaskMode, MaskClip, MaskComposite,
    specified::LengthPercentage,
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

// ─── BackgroundSize parser ────────────────────────────────────────────────────

impl crate::Parse for BackgroundSize {
    /// `cover | contain | [ <length-percentage> | auto ]{1,2}`
    ///
    /// CSS Backgrounds Level 3 §3.9
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        // Keywords first.
        if let Ok(kw) = input.try_parse(|i| i.expect_ident_cloned()) {
            match kw.as_ref() {
                "cover"   => return Ok(BackgroundSize::Cover),
                "contain" => return Ok(BackgroundSize::Contain),
                "auto"    => {
                    // `auto [auto]?` — try optional second auto/length.
                    let height = parse_bg_size_axis(input).ok().flatten();
                    return Ok(BackgroundSize::Explicit { width: None, height });
                }
                _ => {} // fall through to try as length
            }
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        // Try as length-percentage for width.
        if let Ok(w) = input.try_parse(<LengthPercentage as crate::Parse>::parse) {
            let height = parse_bg_size_axis(input).ok().flatten();
            return Ok(BackgroundSize::Explicit { width: Some(w), height });
        }
        Err(input.new_custom_error(crate::CustomError::InvalidValue))
    }
}

/// Parse one axis of `<bg-size>`: `auto | <length-percentage>`.
/// Returns `None` for `auto` (implicit in the `Option<LengthPercentage>`).
fn parse_bg_size_axis<'i>(input: &mut Parser<'i, '_>) -> Result<Option<LengthPercentage>, Error<'i>> {
    input.try_parse(|i| {
        if i.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(None);
        }
        <LengthPercentage as crate::Parse>::parse(i).map(Some)
    })
}

// ─── Background / mask per-layer list parsers ─────────────────────────────────
//
// Each property is a comma-separated list of per-layer values.
// CSS Backgrounds Level 3 §3, CSS Masking Level 1 §6.

/// Generic helper: parse a comma-separated list of `T` into `Box<[T]>`.
fn parse_comma_list<'i, T, F>(input: &mut Parser<'i, '_>, mut parse_one: F)
    -> Result<Box<[T]>, crate::Error<'i>>
where
    F: FnMut(&mut Parser<'i, '_>) -> Result<T, crate::Error<'i>>,
{
    let first = parse_one(input)?;
    let mut items = vec![first];
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        items.push(parse_one(input)?);
    }
    Ok(items.into_boxed_slice())
}

impl crate::Parse for PositionComponentList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <kozan_style::PositionComponent as crate::Parse>::parse)
            .map(PositionComponentList)
    }
}

impl crate::Parse for BackgroundSizeList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <BackgroundSize as crate::Parse>::parse)
            .map(BackgroundSizeList)
    }
}

impl crate::Parse for BackgroundRepeatList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <BackgroundRepeat as crate::Parse>::parse)
            .map(BackgroundRepeatList)
    }
}

impl crate::Parse for BackgroundAttachmentList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <BackgroundAttachment as crate::Parse>::parse)
            .map(BackgroundAttachmentList)
    }
}

impl crate::Parse for BackgroundClipList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <BackgroundClip as crate::Parse>::parse)
            .map(BackgroundClipList)
    }
}

impl crate::Parse for BackgroundOriginList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <BackgroundOrigin as crate::Parse>::parse)
            .map(BackgroundOriginList)
    }
}

impl crate::Parse for MaskModeList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <MaskMode as crate::Parse>::parse)
            .map(MaskModeList)
    }
}

impl crate::Parse for MaskClipList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <MaskClip as crate::Parse>::parse)
            .map(MaskClipList)
    }
}

impl crate::Parse for MaskCompositeList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <MaskComposite as crate::Parse>::parse)
            .map(MaskCompositeList)
    }
}

impl crate::Parse for Position2DList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_comma_list(input, <kozan_style::Position2D as crate::Parse>::parse)
            .map(Position2DList)
    }
}
