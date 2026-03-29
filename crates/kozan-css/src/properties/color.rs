//! CSS `<color>` parser — all color functions, hex, named, system colors.

use cssparser::{Parser, Token};
use kozan_style::{AbsoluteColor, Color, ColorMix, ColorProperty, ColorSpace, SystemColor};
use kozan_style_macros::css_match;
use super::named_colors;
use crate::Error;

impl crate::Parse for Color {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        // Hex color: #RGB, #RRGGBB, #RGBA, #RRGGBBAA
        if let Ok(color) = input.try_parse(parse_hex) {
            return Ok(Color::Absolute(color));
        }

        // Functions: rgb(), hsl(), oklch(), color(), color-mix(), light-dark()
        if let Ok(func) = input.try_parse(|i| i.expect_function().cloned()) {
            return parse_color_function(input, &func);
        }

        // Keywords: currentcolor, transparent, named colors, system colors
        let location = input.current_source_location();
        let ident = input.expect_ident_cloned()?;
        parse_color_keyword(&ident)
            .ok_or_else(|| location.new_custom_error(crate::CustomError::InvalidValue))
    }
}

impl crate::Parse for ColorProperty {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        <Color as crate::Parse>::parse(input).map(ColorProperty)
    }
}

/// Parse a hex color token.
fn parse_hex<'i>(input: &mut Parser<'i, '_>) -> Result<AbsoluteColor, Error<'i>> {
    let location = input.current_source_location();
    match *input.next()? {
        Token::IDHash(ref hash) | Token::Hash(ref hash) => {
            parse_hex_digits(hash)
                .ok_or_else(|| location.new_custom_error(crate::CustomError::InvalidValue))
        }
        _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
    }
}

/// Parse 3, 4, 6, or 8 hex digits into an AbsoluteColor.
fn parse_hex_digits(hex: &str) -> Option<AbsoluteColor> {
    let bytes = hex.as_bytes();
    match bytes.len() {
        3 => {
            let r = hex_byte(bytes[0])? * 17;
            let g = hex_byte(bytes[1])? * 17;
            let b = hex_byte(bytes[2])? * 17;
            Some(AbsoluteColor::from_u8(r, g, b, 255))
        }
        4 => {
            let r = hex_byte(bytes[0])? * 17;
            let g = hex_byte(bytes[1])? * 17;
            let b = hex_byte(bytes[2])? * 17;
            let a = hex_byte(bytes[3])? * 17;
            Some(AbsoluteColor::from_u8(r, g, b, a))
        }
        6 => {
            let r = hex_pair(bytes[0], bytes[1])?;
            let g = hex_pair(bytes[2], bytes[3])?;
            let b = hex_pair(bytes[4], bytes[5])?;
            Some(AbsoluteColor::from_u8(r, g, b, 255))
        }
        8 => {
            let r = hex_pair(bytes[0], bytes[1])?;
            let g = hex_pair(bytes[2], bytes[3])?;
            let b = hex_pair(bytes[4], bytes[5])?;
            let a = hex_pair(bytes[6], bytes[7])?;
            Some(AbsoluteColor::from_u8(r, g, b, a))
        }
        _ => None,
    }
}

fn hex_byte(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    Some(hex_byte(hi)? * 16 + hex_byte(lo)?)
}

/// Parse a color keyword: currentcolor, transparent, named, or system.
fn parse_color_keyword(ident: &str) -> Option<Color> {
    css_match! { ident,
        "currentcolor" => return Some(Color::CurrentColor),
        "transparent" => return Some(Color::Absolute(AbsoluteColor::TRANSPARENT)),
        _ => {}
    }

    if let Some(c) = named_colors::lookup(ident) {
        return Some(Color::Absolute(c));
    }

    parse_system_color(ident).map(Color::System)
}

/// Parse a CSS system color keyword.
fn parse_system_color(ident: &str) -> Option<SystemColor> {
    Some(css_match! { ident,
        "canvas" => SystemColor::Canvas,
        "canvastext" => SystemColor::CanvasText,
        "linktext" => SystemColor::LinkText,
        "visitedtext" => SystemColor::VisitedText,
        "activetext" => SystemColor::ActiveText,
        "buttonface" => SystemColor::ButtonFace,
        "buttontext" => SystemColor::ButtonText,
        "buttonborder" => SystemColor::ButtonBorder,
        "field" => SystemColor::Field,
        "fieldtext" => SystemColor::FieldText,
        "highlight" => SystemColor::Highlight,
        "highlighttext" => SystemColor::HighlightText,
        "selecteditem" => SystemColor::SelectedItem,
        "selecteditemtext" => SystemColor::SelectedItemText,
        "mark" => SystemColor::Mark,
        "marktext" => SystemColor::MarkText,
        "graytext" => SystemColor::GrayText,
        "accentcolor" => SystemColor::AccentColor,
        "accentcolortext" => SystemColor::AccentColorText,
        _ => return None
    })
}

/// Dispatch a color function: rgb(), hsl(), oklch(), color(), etc.
fn parse_color_function<'i>(input: &mut Parser<'i, '_>, name: &str) -> Result<Color, Error<'i>> {
    input.parse_nested_block(|i| {
        let color = css_match! { name,
            "rgb" | "rgba" => parse_rgb(i)?,
            "hsl" | "hsla" => parse_hsl(i)?,
            "oklch" => parse_components(i, ColorSpace::Oklch)?,
            "oklab" => parse_components(i, ColorSpace::Oklab)?,
            "lab" => parse_components(i, ColorSpace::Lab)?,
            "lch" => parse_components(i, ColorSpace::Lch)?,
            "hwb" => parse_components(i, ColorSpace::Hwb)?,
            "color" => parse_color_space_function(i)?,
            "color-mix" => return parse_color_mix(i).map(|m| Color::ColorMix(Box::new(m))),
            "light-dark" => return parse_light_dark(i),
            _ => return Err(i.new_custom_error(crate::CustomError::InvalidValue))
        };
        Ok(Color::Absolute(color))
    })
}

/// `rgb(r g b)` or `rgb(r g b / a)` — modern space-separated syntax.
/// Also handles legacy `rgb(r, g, b)` and `rgb(r, g, b, a)`.
///
/// CSS Color 3 forbids mixing numbers and percentages in legacy syntax.
/// CSS Color 4 modern syntax also requires consistency.
fn parse_rgb<'i>(input: &mut Parser<'i, '_>) -> Result<AbsoluteColor, Error<'i>> {
    // Parse first component and detect if it's a percentage.
    let (r, is_pct) = parse_rgb_component(input)?;
    // Detect legacy comma syntax.
    let legacy = input.try_parse(|i| i.expect_comma()).is_ok();
    let (g, g_pct) = parse_rgb_component(input)?;
    if g_pct != is_pct {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }
    if legacy { input.expect_comma()?; }
    let (b, b_pct) = parse_rgb_component(input)?;
    if b_pct != is_pct {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }
    let a = parse_alpha(input, legacy)?;
    // Normalize to 0..1: numbers are 0-255 range, percentages are already 0-1.
    let (r, g, b) = if is_pct {
        (r, g, b)
    } else {
        (r / 255.0, g / 255.0, b / 255.0)
    };
    Ok(AbsoluteColor::srgb(r, g, b, a))
}

/// Parse a single rgb component, returning (value, is_percentage).
fn parse_rgb_component<'i>(input: &mut Parser<'i, '_>) -> Result<(f32, bool), Error<'i>> {
    if let Ok(v) = input.try_parse(|i| i.expect_percentage()) {
        return Ok((v, true));
    }
    Ok((input.expect_number()?, false))
}

/// `hsl(h s l)` or `hsl(h s l / a)`.
///
/// Saturation and lightness MUST be percentages per CSS Color 3/4 spec.
fn parse_hsl<'i>(input: &mut Parser<'i, '_>) -> Result<AbsoluteColor, Error<'i>> {
    let h = parse_angle_or_number(input)?;
    let legacy = input.try_parse(|i| i.expect_comma()).is_ok();
    let s = input.expect_percentage()?;
    if legacy { input.expect_comma()?; }
    let l = input.expect_percentage()?;
    let a = parse_alpha(input, legacy)?;
    Ok(AbsoluteColor::hsl(h, s, l, a))
}

/// Parse 3 space-separated components + optional alpha for oklch/oklab/lab/lch/hwb.
///
/// Component interpretation per color space:
/// - HWB: `hue whiteness blackness` (hue = angle)
/// - Lab: `L a b` (L = percentage 0-100, a/b = numbers or percentages)
/// - LCH: `L C hue` (hue = angle)
/// - OKLab: `L a b` (L = percentage 0-1, a/b = numbers or percentages)
/// - OKLCH: `L C hue` (hue = angle)
fn parse_components<'i>(
    input: &mut Parser<'i, '_>,
    space: ColorSpace,
) -> Result<AbsoluteColor, Error<'i>> {
    let (c0, c1, c2) = match space {
        ColorSpace::Hwb => {
            // hue whiteness blackness
            let h = parse_angle_or_number(input)?;
            let w = parse_number_or_percentage(input)?;
            let b = parse_number_or_percentage(input)?;
            (h, w, b)
        }
        ColorSpace::Lch => {
            // L C hue
            let l = parse_number_or_percentage(input)?;
            let c = parse_number_or_percentage(input)?;
            let h = parse_angle_or_number(input)?;
            (l, c, h)
        }
        ColorSpace::Oklch => {
            // L C hue
            let l = parse_number_or_percentage(input)?;
            let c = parse_number_or_percentage(input)?;
            let h = parse_angle_or_number(input)?;
            (l, c, h)
        }
        _ => {
            // Lab, OKLab, and others: 3 number-or-percentage components
            let c0 = parse_number_or_percentage(input)?;
            let c1 = parse_number_or_percentage(input)?;
            let c2 = parse_number_or_percentage(input)?;
            (c0, c1, c2)
        }
    };
    let a = parse_alpha(input, false)?;
    Ok(AbsoluteColor::new(c0, c1, c2, a, space))
}

/// `color(display-p3 r g b)` or `color(display-p3 r g b / a)`.
fn parse_color_space_function<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AbsoluteColor, Error<'i>> {
    let location = input.current_source_location();
    let space_ident = input.expect_ident_cloned()?;
    let space = css_match! { &space_ident,
        "srgb" => ColorSpace::Srgb,
        "srgb-linear" => ColorSpace::SrgbLinear,
        "display-p3" => ColorSpace::DisplayP3,
        "a98-rgb" => ColorSpace::A98Rgb,
        "prophoto-rgb" => ColorSpace::ProphotoRgb,
        "rec2020" => ColorSpace::Rec2020,
        "xyz-d50" | "xyz" => ColorSpace::XyzD50,
        "xyz-d65" => ColorSpace::XyzD65,
        _ => return Err(location.new_custom_error(crate::CustomError::InvalidValue))
    };
    parse_components(input, space)
}

/// `color-mix(in oklch, red 60%, blue 40%)`.
fn parse_color_mix<'i>(input: &mut Parser<'i, '_>) -> Result<ColorMix, Error<'i>> {
    let location = input.current_source_location();
    // "in"
    let in_kw = input.expect_ident_cloned()?;
    if !in_kw.eq_ignore_ascii_case("in") {
        return Err(location.new_custom_error(crate::CustomError::InvalidValue));
    }
    // Color space
    let space_ident = input.expect_ident_cloned()?;
    let space = parse_interpolation_space(&space_ident)
        .ok_or_else(|| location.new_custom_error(crate::CustomError::InvalidValue))?;
    input.expect_comma()?;
    // Left color + optional percentage
    let left = <Color as crate::Parse>::parse(input)?;
    let left_pct = input.try_parse(|i| i.expect_percentage()).ok();
    input.expect_comma()?;
    // Right color + optional percentage
    let right = <Color as crate::Parse>::parse(input)?;
    let right_pct = input.try_parse(|i| i.expect_percentage()).ok();
    // Normalize percentages (CSS Color 5 rules).
    let (lp, rp) = normalize_mix_percentages(left_pct, right_pct);
    Ok(ColorMix { space, left, left_pct: lp, right, right_pct: rp })
}

/// `light-dark(light-color, dark-color)`.
fn parse_light_dark<'i>(input: &mut Parser<'i, '_>) -> Result<Color, Error<'i>> {
    let light = <Color as crate::Parse>::parse(input)?;
    input.expect_comma()?;
    let dark = <Color as crate::Parse>::parse(input)?;
    Ok(Color::light_dark(light, dark))
}

fn parse_interpolation_space(ident: &str) -> Option<ColorSpace> {
    Some(css_match! { ident,
        "srgb" => ColorSpace::Srgb,
        "srgb-linear" => ColorSpace::SrgbLinear,
        "lab" => ColorSpace::Lab,
        "oklab" => ColorSpace::Oklab,
        "oklch" => ColorSpace::Oklch,
        "xyz" | "xyz-d50" => ColorSpace::XyzD50,
        "xyz-d65" => ColorSpace::XyzD65,
        "hsl" => ColorSpace::Hsl,
        "hwb" => ColorSpace::Hwb,
        "lch" => ColorSpace::Lch,
        "display-p3" => ColorSpace::DisplayP3,
        _ => return None
    })
}

fn normalize_mix_percentages(left: Option<f32>, right: Option<f32>) -> (f32, f32) {
    match (left, right) {
        (Some(l), Some(r)) => (l, r),
        (Some(l), None) => (l, 1.0 - l),
        (None, Some(r)) => (1.0 - r, r),
        (None, None) => (0.5, 0.5),
    }
}

/// Parse a number or percentage (percentage returns 0..1 range).
fn parse_number_or_percentage<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    if let Ok(v) = input.try_parse(|i| i.expect_percentage()) {
        return Ok(v);
    }
    Ok(input.expect_number()?)
}

/// Parse an angle (degrees) or plain number for hue values.
fn parse_angle_or_number<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
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

/// Parse optional `/ alpha` or `, alpha` suffix.
fn parse_alpha<'i>(input: &mut Parser<'i, '_>, legacy_comma: bool) -> Result<f32, Error<'i>> {
    if legacy_comma {
        if input.try_parse(|i| i.expect_comma()).is_ok() {
            return parse_number_or_percentage(input);
        }
    } else if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        return parse_number_or_percentage(input);
    }
    Ok(1.0)
}
