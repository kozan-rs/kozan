//! CSS property value parsers.
//!
//! The `Parse` trait is the counterpart of `ToCss` — every type that can
//! appear in a CSS property value implements it. Keyword enums get
//! auto-generated impls from TOML. Complex types are hand-written.

pub mod length;
pub mod color;
pub mod animation;
pub mod border;
pub mod content;
pub mod filter;
pub mod font;
pub mod geometry;
pub mod grid;
pub mod ident;
pub mod image;
pub mod shadow;
pub mod svg;
pub mod text;
pub mod transform;
pub mod ui;
pub mod wrappers;
pub(crate) mod named_colors;

use cssparser::Parser;
use crate::Error;

/// Parse a CSS value from the token stream.
///
/// Mirror of `kozan_style::ToCss` — one serializes, the other parses.
pub trait Parse: Sized {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>>;
}

// --- Primitive impls ---

impl Parse for f32 {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        Ok(input.expect_number()?)
    }
}

impl Parse for i32 {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        Ok(input.expect_integer()?)
    }
}

impl Parse for u32 {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let v = input.expect_integer()?;
        if v < 0 {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(v as u32)
    }
}

impl Parse for u16 {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let v = input.expect_integer()?;
        if v < 0 || v > i32::from(u16::MAX) {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(v as u16)
    }
}

impl Parse for bool {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        match &**ident {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
        }
    }
}

// --- Generic wrapper impls ---
// These are generic over the inner type, so they work for any T: Parse.

impl<T: Parse> Parse for kozan_style::AutoOr<T> {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        T::parse(input).map(|v| Self::Value(v))
    }
}

impl<T: Parse> Parse for kozan_style::NoneOr<T> {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        T::parse(input).map(|v| Self::Value(v))
    }
}

impl<T: Parse> Parse for kozan_style::NormalOr<T> {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(Self::Normal);
        }
        T::parse(input).map(|v| Self::Value(v))
    }
}

impl<LP: Parse> Parse for kozan_style::generics::Size<LP> {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        if input.try_parse(|i| i.expect_function_matching("fit-content")).is_ok() {
            return input.parse_nested_block(|i| {
                LP::parse(i).map(|v| Self::FitContentFunction(v))
            });
        }
        if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            match &*ident {
                "min-content" => return Ok(Self::MinContent),
                "max-content" => return Ok(Self::MaxContent),
                _ => {}
            }
        }
        LP::parse(input).map(|v| Self::LengthPercentage(v))
    }
}

impl<LP: Parse> Parse for kozan_style::generics::MaxSize<LP> {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if input.try_parse(|i| i.expect_function_matching("fit-content")).is_ok() {
            return input.parse_nested_block(|i| {
                LP::parse(i).map(|v| Self::FitContentFunction(v))
            });
        }
        if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            match &*ident {
                "min-content" => return Ok(Self::MinContent),
                "max-content" => return Ok(Self::MaxContent),
                _ => {}
            }
        }
        LP::parse(input).map(|v| Self::LengthPercentage(v))
    }
}

impl<LP: Parse> Parse for kozan_style::generics::Margin<LP> {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        LP::parse(input).map(|v| Self::LengthPercentage(v))
    }
}

impl<LP: Parse> Parse for kozan_style::generics::LengthPercentageOrAuto<LP> {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        LP::parse(input).map(|v| Self::LengthPercentage(v))
    }
}

impl<LP: Parse> Parse for kozan_style::generics::LengthPercentageOrNormal<LP> {
    #[inline]
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(Self::Normal);
        }
        LP::parse(input).map(|v| Self::LengthPercentage(v))
    }
}

// These are used by the generated code.
use kozan_style::*;
#[allow(unused_imports)]
use kozan_style::{specified, computed, generics};

include!(concat!(env!("OUT_DIR"), "/generated_parsers.rs"));
