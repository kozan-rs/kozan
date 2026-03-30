//! CSS animation & transition property parsers.
//!
//! Generated from types.toml: TransitionBehavior, IterationCount (Parse impls),
//! DurationList, TimingFunctionList, TransitionBehaviorList,
//! AnimationIterationCountList, AnimationDirectionList,
//! AnimationFillModeList, AnimationPlayStateList (list Parse impls).
//!
//! Hand-written here: Duration, TimingFunction, TransitionPropertyList, AnimationNameList.

use cssparser::{Parser, Token};
use kozan_style::{
    Atom, AnimationNameList, AnimationTimeline, AnimationTimelineList,
    AnimationRangeValue, ScrollAxis, ScrollScroller, ScrollTimelineNameList,
    ScrollTimelineAxisList, TimelineRangeName, TransitionPropertyList,
    TimingFunction, StepPosition, ViewTimelineNameList, ViewTimelineAxisList,
    ViewTimelineInset, ViewTimelineInsetList,
};
use kozan_style_macros::css_match;
use crate::Parse;
use std::time::Duration;
use crate::Error;

// Duration: `0s`, `200ms`, `1.5s` — hand-written (CSS time token parsing)

impl crate::Parse for Duration {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let location = input.current_source_location();
        match *input.next()? {
            Token::Dimension { value, ref unit, .. } => {
                css_match! { unit,
                    "s" => Ok(Duration::from_secs_f32(value)),
                    "ms" => Ok(Duration::from_secs_f32(value / 1000.0)),
                    _ => Err(location.new_custom_error(crate::CustomError::InvalidValue))
                }
            }
            Token::Number { value, .. } if value == 0.0 => Ok(Duration::ZERO),
            _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
        }
    }
}

// TimingFunction: hand-written (keywords + CSS functions)

impl crate::Parse for TimingFunction {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            return css_match! { &ident,
                "ease" => Ok(TimingFunction::Ease),
                "linear" => Ok(TimingFunction::Linear),
                "ease-in" => Ok(TimingFunction::EaseIn),
                "ease-out" => Ok(TimingFunction::EaseOut),
                "ease-in-out" => Ok(TimingFunction::EaseInOut),
                "step-start" => Ok(TimingFunction::StepStart),
                "step-end" => Ok(TimingFunction::StepEnd),
                _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
            };
        }

        let func = input.expect_function()?.clone();
        input.parse_nested_block(|i| {
            css_match! { &func,
                "cubic-bezier" => {
                    let x1 = i.expect_number()?;
                    i.expect_comma()?;
                    let y1 = i.expect_number()?;
                    i.expect_comma()?;
                    let x2 = i.expect_number()?;
                    i.expect_comma()?;
                    let y2 = i.expect_number()?;
                    Ok(TimingFunction::CubicBezier(x1, y1, x2, y2))
                },
                "steps" => {
                    let count = i.expect_integer()? as u32;
                    let pos = if i.try_parse(|i| i.expect_comma()).is_ok() {
                        let ident = i.expect_ident()?;
                        css_match! { &ident,
                            "start" => StepPosition::Start,
                            "end" => StepPosition::End,
                            "jump-start" => StepPosition::Start,
                            "jump-end" => StepPosition::End,
                            "jump-none" => StepPosition::JumpNone,
                            "jump-both" => StepPosition::JumpBoth,
                            _ => return Err(i.new_custom_error(crate::CustomError::InvalidValue))
                        }
                    } else {
                        StepPosition::End
                    };
                    Ok(TimingFunction::Steps(count, pos))
                },
                _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
            }
        })
    }
}

// TransitionPropertyList: `none | all | <ident>,*` — hand-written (special keywords)

impl crate::Parse for TransitionPropertyList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(TransitionPropertyList::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("all")).is_ok() {
            return Ok(TransitionPropertyList::All);
        }
        let first = { let ident = input.expect_ident()?; Atom::new(&*ident) };
        let mut list = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            let ident = input.expect_ident()?;
            list.push(Atom::new(&*ident));
        }
        Ok(TransitionPropertyList::Properties(list.into_boxed_slice()))
    }
}

// AnimationNameList: `none | <ident>,*` — hand-written (special keyword)

impl crate::Parse for AnimationNameList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(AnimationNameList::None);
        }
        let first = { let ident = input.expect_ident()?; Atom::new(&*ident) };
        let mut list = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            let ident = input.expect_ident()?;
            list.push(Atom::new(&*ident));
        }
        Ok(AnimationNameList::Names(list.into_boxed_slice()))
    }
}

// ---- Scroll-Driven Animation parsers ----

impl crate::Parse for ScrollAxis {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        let ident = input.expect_ident_cloned()?;
        css_match! { &ident,
            "block" => Ok(ScrollAxis::Block),
            "inline" => Ok(ScrollAxis::Inline),
            "x" => Ok(ScrollAxis::X),
            "y" => Ok(ScrollAxis::Y),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        }
    }
}

impl crate::Parse for ScrollScroller {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        let ident = input.expect_ident_cloned()?;
        css_match! { &ident,
            "nearest" => Ok(ScrollScroller::Nearest),
            "root" => Ok(ScrollScroller::Root),
            "self" => Ok(ScrollScroller::SelfElement),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        }
    }
}

impl crate::Parse for AnimationTimeline {
    /// `auto | none | <custom-ident> | scroll(<scroller>? <axis>?) | view(<axis>?)`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            return css_match! { &ident,
                "auto" => Ok(AnimationTimeline::Auto),
                "none" => Ok(AnimationTimeline::None),
                _ => Ok(AnimationTimeline::Named(Atom::new(&*ident)))
            };
        }
        // scroll() function
        if input.try_parse(|i| -> Result<_, crate::Error<'i>> {
            i.expect_function_matching("scroll")?;
            Ok(())
        }).is_ok() {
            return input.parse_nested_block(|i| {
                let scroller = i.try_parse(ScrollScroller::parse).unwrap_or_default();
                let axis = i.try_parse(ScrollAxis::parse).unwrap_or_default();
                Ok(AnimationTimeline::Scroll(scroller, axis))
            });
        }
        // view() function
        if input.try_parse(|i| -> Result<_, crate::Error<'i>> {
            i.expect_function_matching("view")?;
            Ok(())
        }).is_ok() {
            return input.parse_nested_block(|i| {
                let axis = i.try_parse(ScrollAxis::parse).unwrap_or_default();
                Ok(AnimationTimeline::View(axis))
            });
        }
        Err(input.new_custom_error(crate::CustomError::InvalidValue))
    }
}

impl crate::Parse for AnimationTimelineList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        let first = AnimationTimeline::parse(input)?;
        let mut list = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            list.push(AnimationTimeline::parse(input)?);
        }
        Ok(AnimationTimelineList::Values(list.into_boxed_slice()))
    }
}

impl crate::Parse for TimelineRangeName {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        let ident = input.expect_ident_cloned()?;
        css_match! { &ident,
            "cover" => Ok(TimelineRangeName::Cover),
            "contain" => Ok(TimelineRangeName::Contain),
            "entry" => Ok(TimelineRangeName::Entry),
            "exit" => Ok(TimelineRangeName::Exit),
            "entry-crossing" => Ok(TimelineRangeName::EntryCrossing),
            "exit-crossing" => Ok(TimelineRangeName::ExitCrossing),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        }
    }
}

impl crate::Parse for AnimationRangeValue {
    /// `normal | <length-percentage> | <timeline-range-name> <length-percentage>?`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(AnimationRangeValue::Normal);
        }
        if let Ok(name) = input.try_parse(TimelineRangeName::parse) {
            let lp = input.try_parse(
                <kozan_style::specified::LengthPercentage as crate::Parse>::parse,
            ).ok();
            return Ok(AnimationRangeValue::Named(name, lp));
        }
        let lp = <kozan_style::specified::LengthPercentage as crate::Parse>::parse(input)?;
        Ok(AnimationRangeValue::LengthPercentage(lp))
    }
}

fn parse_comma_idents<'i>(input: &mut Parser<'i, '_>) -> Result<Box<[Atom]>, crate::Error<'i>> {
    let first = input.expect_ident()?;
    let mut list = vec![Atom::new(&*first)];
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        let ident = input.expect_ident()?;
        list.push(Atom::new(&*ident));
    }
    Ok(list.into_boxed_slice())
}

fn parse_comma_axis<'i>(input: &mut Parser<'i, '_>) -> Result<Box<[ScrollAxis]>, crate::Error<'i>> {
    let first = ScrollAxis::parse(input)?;
    let mut list = vec![first];
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        list.push(ScrollAxis::parse(input)?);
    }
    Ok(list.into_boxed_slice())
}

impl crate::Parse for ScrollTimelineNameList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ScrollTimelineNameList::None);
        }
        Ok(ScrollTimelineNameList::Names(parse_comma_idents(input)?))
    }
}

impl crate::Parse for ScrollTimelineAxisList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        Ok(ScrollTimelineAxisList(parse_comma_axis(input)?))
    }
}

impl crate::Parse for ViewTimelineNameList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(ViewTimelineNameList::None);
        }
        Ok(ViewTimelineNameList::Names(parse_comma_idents(input)?))
    }
}

impl crate::Parse for ViewTimelineAxisList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        Ok(ViewTimelineAxisList(parse_comma_axis(input)?))
    }
}

impl crate::Parse for ViewTimelineInset {
    /// `auto | <length-percentage>{1,2}`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        let start = if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            kozan_style::AutoOr::Auto
        } else {
            kozan_style::AutoOr::Value(
                <kozan_style::specified::LengthPercentage as crate::Parse>::parse(input)?,
            )
        };
        let end = if let Ok(v) = input.try_parse(|i| {
            if i.try_parse(|i2| i2.expect_ident_matching("auto")).is_ok() {
                Ok(kozan_style::AutoOr::Auto)
            } else {
                <kozan_style::specified::LengthPercentage as crate::Parse>::parse(i)
                    .map(kozan_style::AutoOr::Value)
            }
        }) {
            v
        } else {
            start.clone()
        };
        Ok(ViewTimelineInset { start, end })
    }
}

impl crate::Parse for ViewTimelineInsetList {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, crate::Error<'i>> {
        let first = ViewTimelineInset::parse(input)?;
        let mut list = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            list.push(ViewTimelineInset::parse(input)?);
        }
        Ok(ViewTimelineInsetList(list.into_boxed_slice()))
    }
}
