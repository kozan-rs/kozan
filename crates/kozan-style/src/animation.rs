//! CSS animation and transition value types.
//!
//! Generated from types.toml: TransitionBehavior, IterationCount,
//! DurationList, TimingFunctionList, TransitionBehaviorList,
//! AnimationIterationCountList, AnimationDirectionList,
//! AnimationFillModeList, AnimationPlayStateList.

use crate::Atom;
use kozan_style_macros::ToComputedValue;

/// CSS `transition-property` — `all`, `none`, or list of property names.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TransitionPropertyList {
    None,
    All,
    Properties(Box<[Atom]>),
}

impl Default for TransitionPropertyList {
    fn default() -> Self { Self::All }
}

/// CSS timing function (easing).
/// Hand-written: keywords + CSS functions (`cubic-bezier`, `steps`).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum TimingFunction {
    Ease,
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    StepStart,
    StepEnd,
    Steps(u32, StepPosition),
    CubicBezier(f32, f32, f32, f32),
}

/// Step position for CSS `steps()` timing function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum StepPosition { Start, End, JumpNone, JumpBoth }

/// CSS `animation-name` — `none` or list of keyframe names.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum AnimationNameList {
    None,
    Names(Box<[Atom]>),
}

impl Default for AnimationNameList {
    fn default() -> Self { Self::None }
}
