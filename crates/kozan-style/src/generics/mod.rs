//! Generic CSS value types parameterized over `LengthPercentage`.
//!
//! These types work for BOTH specified and computed levels — the
//! `LP` type parameter is either `specified::LengthPercentage` or
//! `computed::LengthPercentage`.

mod length;

pub use length::{
    LengthPercentageOrAuto, LengthPercentageOrNormal, Margin, MaxSize, Size,
};
