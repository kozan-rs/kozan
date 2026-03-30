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

// ---- Scroll-Driven Animation types ----

/// Axis for scroll/view timelines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum ScrollAxis {
    Block,
    Inline,
    X,
    Y,
}

impl Default for ScrollAxis {
    fn default() -> Self { Self::Block }
}

/// CSS `animation-timeline` single value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum AnimationTimeline {
    Auto,
    None,
    Named(Atom),
    /// `scroll(<scroller>? <axis>?)`
    Scroll(ScrollScroller, ScrollAxis),
    /// `view(<axis>?)`
    View(ScrollAxis),
}

impl Default for AnimationTimeline {
    fn default() -> Self { Self::Auto }
}

/// CSS `animation-timeline` list (comma-separated for multi-animation).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum AnimationTimelineList {
    Values(Box<[AnimationTimeline]>),
}

impl Default for AnimationTimelineList {
    fn default() -> Self { Self::Values(Box::new([AnimationTimeline::Auto])) }
}

/// Scroller argument for `scroll()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum ScrollScroller {
    Nearest,
    Root,
    #[allow(dead_code)]
    SelfElement,
}

impl Default for ScrollScroller {
    fn default() -> Self { Self::Nearest }
}

/// CSS `view-timeline-inset` — `auto | <length-percentage>{1,2}`.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ViewTimelineInset {
    pub start: crate::AutoOr<crate::specified::LengthPercentage>,
    pub end: crate::AutoOr<crate::specified::LengthPercentage>,
}

impl Default for ViewTimelineInset {
    fn default() -> Self {
        Self {
            start: crate::AutoOr::Auto,
            end: crate::AutoOr::Auto,
        }
    }
}

/// Timeline range name for `animation-range`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ToComputedValue)]
pub enum TimelineRangeName {
    Cover,
    Contain,
    Entry,
    Exit,
    EntryCrossing,
    ExitCrossing,
}

/// CSS `animation-range-start` / `animation-range-end` value.
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum AnimationRangeValue {
    Normal,
    LengthPercentage(crate::specified::LengthPercentage),
    Named(TimelineRangeName, Option<crate::specified::LengthPercentage>),
}

impl Default for AnimationRangeValue {
    fn default() -> Self { Self::Normal }
}

/// CSS `scroll-timeline-name` list (comma-separated).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ScrollTimelineNameList {
    None,
    Names(Box<[Atom]>),
}

impl Default for ScrollTimelineNameList {
    fn default() -> Self { Self::None }
}

/// CSS `scroll-timeline-axis` list (comma-separated).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ScrollTimelineAxisList(pub Box<[ScrollAxis]>);

impl Default for ScrollTimelineAxisList {
    fn default() -> Self { Self(Box::new([ScrollAxis::Block])) }
}

/// CSS `view-timeline-name` list (comma-separated).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub enum ViewTimelineNameList {
    None,
    Names(Box<[Atom]>),
}

impl Default for ViewTimelineNameList {
    fn default() -> Self { Self::None }
}

/// CSS `view-timeline-axis` list (comma-separated).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ViewTimelineAxisList(pub Box<[ScrollAxis]>);

impl Default for ViewTimelineAxisList {
    fn default() -> Self { Self(Box::new([ScrollAxis::Block])) }
}

/// CSS `view-timeline-inset` list (comma-separated).
#[derive(Clone, Debug, PartialEq, ToComputedValue)]
pub struct ViewTimelineInsetList(pub Box<[ViewTimelineInset]>);

impl Default for ViewTimelineInsetList {
    fn default() -> Self { Self(Box::new([ViewTimelineInset::default()])) }
}
