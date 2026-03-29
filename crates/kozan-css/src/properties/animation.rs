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
    Atom, AnimationNameList, TransitionPropertyList, TimingFunction, StepPosition,
};
use kozan_style_macros::css_match;
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
