//! CSS wrapper type parsers — `Edges`, `ContainIntrinsicSize`.

use cssparser::Parser;
use kozan_style::{ContainIntrinsicSize, Edges};
use kozan_style::specified::LengthPercentage;
use crate::Error;

impl crate::Parse for ContainIntrinsicSize {
    /// `none | <length-percentage> | auto <length-percentage>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ContainIntrinsicSize::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            let lp = <LengthPercentage as crate::Parse>::parse(input)?;
            return Ok(ContainIntrinsicSize::AutoLength(lp));
        }
        let lp = <LengthPercentage as crate::Parse>::parse(input)?;
        Ok(ContainIntrinsicSize::Length(lp))
    }
}

impl crate::Parse for Edges<LengthPercentage> {
    /// `<lp>{1,4}` — CSS shorthand: 1 value = all, 2 = tb rl, 3 = t rl b, 4 = t r b l.
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let top = <LengthPercentage as crate::Parse>::parse(input)?;
        let right = input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i));
        match right {
            Err(_) => Ok(Edges { top: top.clone(), right: top.clone(), bottom: top.clone(), left: top }),
            Ok(right) => {
                let bottom = input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i));
                match bottom {
                    Err(_) => Ok(Edges { top: top.clone(), right: right.clone(), bottom: top, left: right }),
                    Ok(bottom) => {
                        let left = input.try_parse(|i| <LengthPercentage as crate::Parse>::parse(i))
                            .unwrap_or_else(|_| right.clone());
                        Ok(Edges { top, right, bottom, left })
                    }
                }
            }
        }
    }
}
