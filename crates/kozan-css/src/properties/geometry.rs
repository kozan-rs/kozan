//! CSS geometry parsers — `border-radius`, `position`, `border-spacing`, `offset-*`.

use cssparser::{Parser, Token};
use kozan_style::{
    CornerRadius, OffsetPath, OffsetPosition, OffsetRotate, Position2D,
    BorderSpacing, RaySize, TransformOrigin, Url,
};
use kozan_style::specified::LengthPercentage;
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for CornerRadius {
    /// `border-*-radius: <lp> <lp>?` — horizontal [vertical].
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let horizontal = <LengthPercentage as crate::Parse>::parse(input)?;
        let vertical = input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i))
            .unwrap_or_else(|_| horizontal.clone());
        Ok(CornerRadius { horizontal, vertical })
    }
}

impl crate::Parse for Position2D {
    /// Full `<bg-position>` parser — 1, 2, 3, and 4-value forms.
    ///
    /// - 1-value: `center | left | right | top | bottom | <lp>`
    /// - 2-value: `<h-kw-or-lp> <v-kw-or-lp>` (keywords set own axis; lp: first=x second=y)
    /// - 3-value: `<directional> <lp> <axis-kw>` (edge keyword + offset + cross-axis keyword)
    /// - 4-value: `<directional> <lp> <directional> <lp>` (both edges with offsets)
    ///
    /// CSS Backgrounds Level 3 §3.8
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_position(input)
    }
}

// ─── position helpers ─────────────────────────────────────────────────────────

/// Which axis a position keyword targets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis { Horizontal, Vertical, Either }

/// Axis + percentage for a position keyword.
fn kw_info(kw: &str) -> Option<(Axis, f32)> {
    match kw {
        "left"   => Some((Axis::Horizontal, 0.0)),
        "right"  => Some((Axis::Horizontal, 1.0)),
        "center" => Some((Axis::Either,     0.5)),
        "top"    => Some((Axis::Vertical,   0.0)),
        "bottom" => Some((Axis::Vertical,   1.0)),
        _ => None,
    }
}

fn pct_lp(p: f32) -> LengthPercentage {
    LengthPercentage::Percentage(kozan_style::computed::Percentage::new(p))
}

/// Resolve an edge keyword + optional absolute offset into a `LengthPercentage`.
///
/// - `left/top` + offset → offset (measured from that edge)
/// - `right/bottom` + offset → `calc(100% - offset)` (measured from the far edge)
/// - keyword alone → percentage value (0%, 50%, 100%)
fn resolve_edge(edge_pct: f32, offset: Option<LengthPercentage>) -> LengthPercentage {
    use kozan_style::{CalcNode, specified::SpecifiedLeaf};
    use kozan_style::computed::Percentage;

    let Some(off) = offset else {
        return pct_lp(edge_pct);
    };
    if edge_pct == 0.0 {
        // left/top: offset is measured from this edge — use it directly.
        return off;
    }
    // right/bottom: calc(100% - offset)
    let hundred = CalcNode::Leaf(SpecifiedLeaf::Percentage(Percentage::new(1.0)));
    let off_node = match off {
        LengthPercentage::Length(l)      => CalcNode::Leaf(SpecifiedLeaf::Length(l)),
        LengthPercentage::Percentage(p)  => CalcNode::Leaf(SpecifiedLeaf::Percentage(p)),
        LengthPercentage::Calc(node)     => *node,
    };
    LengthPercentage::Calc(Box::new(CalcNode::Sum(Box::from([
        hundred,
        CalcNode::Negate(Box::new(off_node)),
    ]))))
}

/// Try to parse a directional (non-center) keyword → (axis, pct).
fn try_parse_directional<'i>(input: &mut Parser<'i, '_>)
    -> Result<(Axis, f32), Error<'i>>
{
    input.try_parse(|i| {
        let ident = i.expect_ident()?;
        match ident.as_ref() {
            "left"   => Ok((Axis::Horizontal, 0.0)),
            "right"  => Ok((Axis::Horizontal, 1.0)),
            "top"    => Ok((Axis::Vertical,   0.0)),
            "bottom" => Ok((Axis::Vertical,   1.0)),
            _ => Err(i.new_custom_error(crate::CustomError::InvalidValue)),
        }
    })
}

/// Try to parse a position keyword (including center).
fn try_parse_kw<'i>(input: &mut Parser<'i, '_>)
    -> Result<(Axis, f32), Error<'i>>
{
    input.try_parse(|i| {
        let ident = i.expect_ident()?;
        kw_info(ident.as_ref())
            .map(Ok)
            .unwrap_or_else(|| Err(i.new_custom_error(crate::CustomError::InvalidValue)))
    })
}

fn try_parse_lp<'i>(input: &mut Parser<'i, '_>) -> Option<LengthPercentage> {
    input.try_parse(<LengthPercentage as crate::Parse>::parse).ok()
}

// ─── main parser ─────────────────────────────────────────────────────────────

fn parse_position<'i>(input: &mut Parser<'i, '_>) -> Result<Position2D, Error<'i>> {
    // 4-value or 3-value form: directional-kw [offset] directional-kw [offset]
    // First keyword must be left/right/top/bottom (not center).
    if let Ok(pos) = input.try_parse(parse_edge_form) {
        return Ok(pos);
    }
    // 2-value or 1-value fallback.
    parse_1or2_value(input)
}

/// Parse `<directional-kw> [<lp>] <directional-kw> [<lp>]` (3- and 4-value forms).
/// The two keywords must be on different axes (one h, one v).
fn parse_edge_form<'i>(input: &mut Parser<'i, '_>) -> Result<Position2D, Error<'i>> {
    let (axis1, pct1) = try_parse_directional(input)?;
    let off1 = try_parse_lp(input);
    let (axis2, pct2) = try_parse_directional(input)?;

    // Axes must differ.
    if axis1 == axis2 {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }
    // center is not allowed in directional form, already excluded above.

    let off2 = try_parse_lp(input);

    let (x_pct, x_off, y_pct, y_off) = if axis1 == Axis::Horizontal {
        (pct1, off1, pct2, off2)
    } else {
        (pct2, off2, pct1, off1)
    };

    Ok(Position2D {
        x: resolve_edge(x_pct, x_off),
        y: resolve_edge(y_pct, y_off),
    })
}

/// Parse 1-value or 2-value position syntax.
///
/// Rules:
/// - Single value: keyword sets own axis; other defaults to 50%.
///   Non-keyword lp is always x; y defaults to 50%.
/// - Two values: each keyword sets its own axis.
///   If first is a v-keyword and second is h, they're swapped.
///   Mixed keyword+lp: keyword axis is determined by type; lp fills the other.
///   Two lp values: first = x, second = y.
fn parse_1or2_value<'i>(input: &mut Parser<'i, '_>) -> Result<Position2D, Error<'i>> {
    // Parse first token.
    let (first_lp, first_axis) = if let Ok((axis, pct)) = try_parse_kw(input) {
        (pct_lp(pct), axis)
    } else {
        (<LengthPercentage as crate::Parse>::parse(input)?, Axis::Horizontal)
    };

    // Try second token.
    let second = input.try_parse(|i| -> Result<(LengthPercentage, Axis), Error<'i>> {
        if let Ok((axis, pct)) = try_parse_kw(i) {
            Ok((pct_lp(pct), axis))
        } else {
            let lp = <LengthPercentage as crate::Parse>::parse(i)?;
            // lp after a keyword: fills the remaining axis.
            let axis = match first_axis {
                Axis::Horizontal => Axis::Vertical,
                Axis::Vertical   => Axis::Horizontal,
                Axis::Either     => Axis::Vertical, // center + lp → lp is x... treat as y
            };
            Ok((lp, axis))
        }
    });

    let center = pct_lp(0.5);

    match second {
        Ok((second_lp, second_axis)) => {
            // Determine which is x and which is y.
            let (x, y) = assign_axes(
                first_lp, first_axis,
                second_lp, second_axis,
                &center,
            );
            Ok(Position2D { x, y })
        }
        Err(_) => {
            // Single value.
            match first_axis {
                Axis::Horizontal => Ok(Position2D { x: first_lp, y: center }),
                Axis::Vertical   => Ok(Position2D { x: center, y: first_lp }),
                Axis::Either     => Ok(Position2D { x: first_lp.clone(), y: first_lp }), // center center
            }
        }
    }
}

/// Assign two parsed values (each with a known axis hint) to x and y.
fn assign_axes(
    a: LengthPercentage, a_axis: Axis,
    b: LengthPercentage, b_axis: Axis,
    center: &LengthPercentage,
) -> (LengthPercentage, LengthPercentage) {
    match (a_axis, b_axis) {
        (Axis::Horizontal, _) | (Axis::Either, Axis::Vertical) => (a, b),
        (Axis::Vertical, _) | (Axis::Either, Axis::Horizontal) => {
            // First value is y, second is x — or first is center + second is h-kw.
            // If b is h: x=b, y=a. If b is Either (center): x=a(center), y=b? No.
            // center+center → both 50%.
            if b_axis == Axis::Horizontal {
                (b, a)
            } else if a_axis == Axis::Vertical {
                // v + center → x=center, y=v
                (center.clone(), a)
            } else {
                (a, b)
            }
        }
        (Axis::Either, Axis::Either) => (a, b), // center center
    }
}

impl crate::Parse for BorderSpacing {
    /// `border-spacing: <lp> <lp>?` — horizontal [vertical].
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let horizontal = <LengthPercentage as crate::Parse>::parse(input)?;
        let vertical = input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i))
            .unwrap_or_else(|_| horizontal.clone());
        Ok(BorderSpacing { horizontal, vertical })
    }
}

impl crate::Parse for TransformOrigin {
    /// `transform-origin: <lp> <lp> <lp>?` — x y [z].
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let x = <LengthPercentage as crate::Parse>::parse(input)?;
        let y = <LengthPercentage as crate::Parse>::parse(input)?;
        let z = input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i))
            .unwrap_or_default();
        Ok(TransformOrigin { x, y, z })
    }
}

// ---- Motion Path parsers ----

impl crate::Parse for RaySize {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let ident = input.expect_ident_cloned()?;
        css_match! { &ident,
            "closest-side" => Ok(RaySize::ClosestSide),
            "closest-corner" => Ok(RaySize::ClosestCorner),
            "farthest-side" => Ok(RaySize::FarthestSide),
            "farthest-corner" => Ok(RaySize::FarthestCorner),
            "sides" => Ok(RaySize::Sides),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        }
    }
}

/// Parse an angle value and return degrees.
fn parse_angle<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    match input.next()? {
        Token::Dimension { value, unit, .. } => {
            let deg = match unit.as_ref() {
                "deg" => *value,
                "grad" => *value * 360.0 / 400.0,
                "rad" => *value * 180.0 / std::f32::consts::PI,
                "turn" => *value * 360.0,
                _ => return Err(input.new_custom_error(crate::CustomError::InvalidValue)),
            };
            Ok(deg)
        }
        _ => Err(input.new_custom_error(crate::CustomError::InvalidValue)),
    }
}

impl crate::Parse for OffsetPath {
    /// `none | path(<string>) | ray(<angle> <size>? contain?) | url(<url>)`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(OffsetPath::None);
        }
        // path()
        if input.try_parse(|i| -> Result<_, Error<'i>> {
            i.expect_function_matching("path")?; Ok(())
        }).is_ok() {
            return input.parse_nested_block(|i| {
                let s = i.expect_string()?;
                Ok(OffsetPath::Path(s.as_ref().into()))
            });
        }
        // ray()
        if input.try_parse(|i| -> Result<_, Error<'i>> {
            i.expect_function_matching("ray")?; Ok(())
        }).is_ok() {
            return input.parse_nested_block(|i| {
                let angle = parse_angle(i)?;
                let size = i.try_parse(RaySize::parse).unwrap_or_default();
                let contain = i.try_parse(|i2| i2.expect_ident_matching("contain")).is_ok();
                Ok(OffsetPath::Ray { angle, size, contain })
            });
        }
        // url()
        let url = <Url as crate::Parse>::parse(input)?;
        Ok(OffsetPath::Url(url))
    }
}

impl crate::Parse for OffsetRotate {
    /// `auto | reverse | <angle> | auto <angle> | reverse <angle>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            match ident.as_ref() {
                "auto" => {
                    if let Ok(a) = input.try_parse(parse_angle) {
                        return Ok(OffsetRotate::AutoAngle(a));
                    }
                    return Ok(OffsetRotate::Auto);
                }
                "reverse" => {
                    if let Ok(a) = input.try_parse(parse_angle) {
                        return Ok(OffsetRotate::ReverseAngle(a));
                    }
                    return Ok(OffsetRotate::Reverse);
                }
                _ => return Err(input.new_custom_error(crate::CustomError::InvalidValue)),
            }
        }
        let a = parse_angle(input)?;
        Ok(OffsetRotate::Angle(a))
    }
}

impl crate::Parse for OffsetPosition {
    /// `normal | auto | <position>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(OffsetPosition::Normal);
        }
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(OffsetPosition::Auto);
        }
        let pos = <Position2D as crate::Parse>::parse(input)?;
        Ok(OffsetPosition::Position(pos))
    }
}
