//! CSS UI interaction value types.

use crate::{Atom, Color};
use kozan_style_macros::ToComputedValue;

/// CSS `will-change`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum WillChange {
    Auto,
    Properties(Box<[Atom]>),
}

impl Default for WillChange {
    fn default() -> Self { Self::Auto }
}

/// CSS `touch-action` computed value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TouchActionValue {
    Auto,
    None,
    Manipulation,
    Flags(crate::TouchAction),
}

impl Default for TouchActionValue {
    fn default() -> Self { Self::Auto }
}

/// CSS `scrollbar-color`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ScrollbarColor {
    Auto,
    Colors { thumb: Color, track: Color },
}

impl Default for ScrollbarColor {
    fn default() -> Self { Self::Auto }
}

/// CSS `scroll-snap-type`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ScrollSnapType {
    None,
    Snap {
        axis: crate::ScrollSnapAxis,
        strictness: crate::ScrollSnapStrictness,
    },
}

impl Default for ScrollSnapType {
    fn default() -> Self { Self::None }
}
