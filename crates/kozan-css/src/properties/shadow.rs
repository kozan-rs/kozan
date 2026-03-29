//! CSS `box-shadow` / `text-shadow` parser.

use cssparser::Parser;
use kozan_style::{Color, Shadow, ShadowList};
use crate::Error;

impl crate::Parse for ShadowList {
    /// `none | <shadow>#`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ShadowList::None);
        }
        let first = parse_shadow(input)?;
        let mut shadows = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            shadows.push(parse_shadow(input)?);
        }
        Ok(ShadowList::Shadows(shadows.into_boxed_slice()))
    }
}

/// Parse a single shadow: `[inset]? <offset-x> <offset-y> [<blur> [<spread>]]? [<color>]?`
///
/// Components can appear in any order for color and inset.
fn parse_shadow<'i>(input: &mut Parser<'i, '_>) -> Result<Shadow, Error<'i>> {
    let mut inset = false;

    // Leading inset / color.
    if input.try_parse(|i| i.expect_ident_matching("inset")).is_ok() {
        inset = true;
    }
    let mut color = input.try_parse(|i| <Color as crate::Parse>::parse(i)).ok();
    if !inset && input.try_parse(|i| i.expect_ident_matching("inset")).is_ok() {
        inset = true;
    }

    // Required: offset-x offset-y.
    let offset_x = input.expect_number()?;
    let offset_y = input.expect_number()?;

    // Optional: blur [spread].
    let blur = input.try_parse(|i| i.expect_number()).unwrap_or(0.0);
    let spread = input.try_parse(|i| i.expect_number()).unwrap_or(0.0);

    // Trailing color / inset.
    if color.is_none() {
        color = input.try_parse(|i| <Color as crate::Parse>::parse(i)).ok();
    }
    if !inset {
        inset = input.try_parse(|i| i.expect_ident_matching("inset")).is_ok();
    }

    Ok(Shadow {
        offset_x,
        offset_y,
        blur,
        spread,
        color: color.unwrap_or(Color::CurrentColor),
        inset,
    })
}
