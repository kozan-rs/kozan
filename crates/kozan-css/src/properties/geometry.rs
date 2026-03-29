//! CSS geometry parsers — `border-radius`, `position`, `border-spacing`.

use cssparser::Parser;
use kozan_style::{CornerRadius, Position2D, BorderSpacing, TransformOrigin};
use kozan_style::specified::LengthPercentage;
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
    /// `<lp> <lp>` — x y position.
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let x = <LengthPercentage as crate::Parse>::parse(input)?;
        let y = <LengthPercentage as crate::Parse>::parse(input)?;
        Ok(Position2D { x, y })
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
