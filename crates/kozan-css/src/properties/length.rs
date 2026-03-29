//! CSS `<length>`, `<percentage>`, `<length-percentage>`, and `calc()` parsers.

use cssparser::{Parser, Token};
use kozan_style::calc::{CalcNode, MinMaxOp};
use kozan_style_macros::css_match;
use kozan_style::computed::Percentage;
use kozan_style::specified::{
    Length, AbsoluteLength, FontRelativeLength, ViewportPercentageLength,
    ContainerRelativeLength, LengthPercentage, SpecifiedLeaf,
};
use crate::Error;

impl crate::Parse for Length {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let location = input.current_source_location();
        match *input.next()? {
            Token::Dimension { value, ref unit, .. } => {
                parse_length_unit(value, unit)
                    .ok_or_else(|| location.new_custom_error(crate::CustomError::InvalidValue))
            }
            // CSS spec: unitless 0 is valid as <length>.
            Token::Number { value, .. } if value == 0.0 => {
                Ok(Length::Absolute(AbsoluteLength::Px(0.0)))
            }
            _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
        }
    }
}

impl crate::Parse for Percentage {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        Ok(Percentage::new(input.expect_percentage()?))
    }
}

impl crate::Parse for LengthPercentage {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        // Try math functions first: calc(), min(), max(), clamp()
        if let Ok(func) = input.try_parse(|i| i.expect_function().cloned()) {
            return css_match! { &func,
                "calc" => input.parse_nested_block(|i| {
                    parse_calc_sum(i).map(|node| wrap_calc(node))
                }),
                "min" => input.parse_nested_block(|i| {
                    parse_comma_list(i).map(|args| wrap_calc(CalcNode::MinMax(args.into(), MinMaxOp::Min)))
                }),
                "max" => input.parse_nested_block(|i| {
                    parse_comma_list(i).map(|args| wrap_calc(CalcNode::MinMax(args.into(), MinMaxOp::Max)))
                }),
                "clamp" => input.parse_nested_block(|i| {
                    let min = parse_calc_sum(i)?;
                    i.expect_comma()?;
                    let center = parse_calc_sum(i)?;
                    i.expect_comma()?;
                    let max = parse_calc_sum(i)?;
                    Ok(wrap_calc(CalcNode::Clamp {
                        min: Box::new(min),
                        center: Box::new(center),
                        max: Box::new(max),
                    }))
                }),
                "abs" => input.parse_nested_block(|i| {
                    parse_calc_sum(i).map(|node| wrap_calc(CalcNode::Abs(Box::new(node))))
                }),
                "sign" => input.parse_nested_block(|i| {
                    parse_calc_sum(i).map(|node| wrap_calc(CalcNode::Sign(Box::new(node))))
                }),
                _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
            };
        }

        let location = input.current_source_location();
        match *input.next()? {
            Token::Dimension { value, ref unit, .. } => {
                parse_length_unit(value, unit)
                    .map(LengthPercentage::Length)
                    .ok_or_else(|| location.new_custom_error(crate::CustomError::InvalidValue))
            }
            Token::Percentage { unit_value, .. } => {
                Ok(LengthPercentage::Percentage(Percentage::new(unit_value)))
            }
            Token::Number { value, .. } if value == 0.0 => {
                Ok(LengthPercentage::Length(Length::Absolute(AbsoluteLength::Px(0.0))))
            }
            _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
        }
    }
}

/// Map a CSS unit string to a `Length` variant. Returns `None` for unknown units.
fn parse_length_unit(value: f32, unit: &str) -> Option<Length> {
    Some(css_match! { unit,
        // Absolute
        "px" => Length::Absolute(AbsoluteLength::Px(value)),
        "cm" => Length::Absolute(AbsoluteLength::Cm(value)),
        "mm" => Length::Absolute(AbsoluteLength::Mm(value)),
        "q" => Length::Absolute(AbsoluteLength::Q(value)),
        "in" => Length::Absolute(AbsoluteLength::In(value)),
        "pt" => Length::Absolute(AbsoluteLength::Pt(value)),
        "pc" => Length::Absolute(AbsoluteLength::Pc(value)),
        // Font-relative
        "em" => Length::FontRelative(FontRelativeLength::Em(value)),
        "rem" => Length::FontRelative(FontRelativeLength::Rem(value)),
        "ch" => Length::FontRelative(FontRelativeLength::Ch(value)),
        "ex" => Length::FontRelative(FontRelativeLength::Ex(value)),
        "cap" => Length::FontRelative(FontRelativeLength::Cap(value)),
        "ic" => Length::FontRelative(FontRelativeLength::Ic(value)),
        "lh" => Length::FontRelative(FontRelativeLength::Lh(value)),
        "rlh" => Length::FontRelative(FontRelativeLength::Rlh(value)),
        "rcap" => Length::FontRelative(FontRelativeLength::Rcap(value)),
        "rch" => Length::FontRelative(FontRelativeLength::Rch(value)),
        "rex" => Length::FontRelative(FontRelativeLength::Rex(value)),
        "ric" => Length::FontRelative(FontRelativeLength::Ric(value)),
        // Viewport
        "vw" => Length::ViewportPercentage(ViewportPercentageLength::Vw(value)),
        "vh" => Length::ViewportPercentage(ViewportPercentageLength::Vh(value)),
        "vmin" => Length::ViewportPercentage(ViewportPercentageLength::Vmin(value)),
        "vmax" => Length::ViewportPercentage(ViewportPercentageLength::Vmax(value)),
        "vi" => Length::ViewportPercentage(ViewportPercentageLength::Vi(value)),
        "vb" => Length::ViewportPercentage(ViewportPercentageLength::Vb(value)),
        "svw" => Length::ViewportPercentage(ViewportPercentageLength::Svw(value)),
        "svh" => Length::ViewportPercentage(ViewportPercentageLength::Svh(value)),
        "svmin" => Length::ViewportPercentage(ViewportPercentageLength::Svmin(value)),
        "svmax" => Length::ViewportPercentage(ViewportPercentageLength::Svmax(value)),
        "svi" => Length::ViewportPercentage(ViewportPercentageLength::Svi(value)),
        "svb" => Length::ViewportPercentage(ViewportPercentageLength::Svb(value)),
        "lvw" => Length::ViewportPercentage(ViewportPercentageLength::Lvw(value)),
        "lvh" => Length::ViewportPercentage(ViewportPercentageLength::Lvh(value)),
        "lvmin" => Length::ViewportPercentage(ViewportPercentageLength::Lvmin(value)),
        "lvmax" => Length::ViewportPercentage(ViewportPercentageLength::Lvmax(value)),
        "lvi" => Length::ViewportPercentage(ViewportPercentageLength::Lvi(value)),
        "lvb" => Length::ViewportPercentage(ViewportPercentageLength::Lvb(value)),
        "dvw" => Length::ViewportPercentage(ViewportPercentageLength::Dvw(value)),
        "dvh" => Length::ViewportPercentage(ViewportPercentageLength::Dvh(value)),
        "dvmin" => Length::ViewportPercentage(ViewportPercentageLength::Dvmin(value)),
        "dvmax" => Length::ViewportPercentage(ViewportPercentageLength::Dvmax(value)),
        "dvi" => Length::ViewportPercentage(ViewportPercentageLength::Dvi(value)),
        "dvb" => Length::ViewportPercentage(ViewportPercentageLength::Dvb(value)),
        // Container query
        "cqw" => Length::ContainerRelative(ContainerRelativeLength::Cqw(value)),
        "cqh" => Length::ContainerRelative(ContainerRelativeLength::Cqh(value)),
        "cqi" => Length::ContainerRelative(ContainerRelativeLength::Cqi(value)),
        "cqb" => Length::ContainerRelative(ContainerRelativeLength::Cqb(value)),
        "cqmin" => Length::ContainerRelative(ContainerRelativeLength::Cqmin(value)),
        "cqmax" => Length::ContainerRelative(ContainerRelativeLength::Cqmax(value)),
        _ => return None
    })
}

// --- Calc expression parser (recursive descent) ---
// Grammar: calc-sum = calc-product [ ['+' | '-'] calc-product ]*
//          calc-product = calc-value [ ['*' | '/'] calc-value ]*
//          calc-value = <number> | <dimension> | <percentage> | '(' calc-sum ')' | func(...)

/// Parse a `<calc-sum>` — the top-level calc grammar production.
fn parse_calc_sum<'i>(input: &mut Parser<'i, '_>) -> Result<CalcNode<SpecifiedLeaf>, Error<'i>> {
    let mut node = parse_calc_product(input)?;

    loop {
        // CSS calc requires whitespace around + and - operators.
        let start = input.state();
        match input.next_including_whitespace() {
            Ok(&Token::WhiteSpace(_)) => {}
            _ => {
                input.reset(&start);
                break;
            }
        }

        let op_state = input.state();
        match *input.next()? {
            Token::Delim('+') => {
                skip_whitespace(input);
                let rhs = parse_calc_product(input)?;
                node = CalcNode::add(node, rhs);
            }
            Token::Delim('-') => {
                skip_whitespace(input);
                let rhs = parse_calc_product(input)?;
                node = CalcNode::sub(node, rhs);
            }
            _ => {
                input.reset(&op_state);
                break;
            }
        }
    }

    Ok(node)
}

/// Parse a `<calc-product>`.
fn parse_calc_product<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<CalcNode<SpecifiedLeaf>, Error<'i>> {
    let mut node = parse_calc_value(input)?;

    loop {
        let state = input.state();
        match input.next() {
            Ok(&Token::Delim('*')) => {
                skip_whitespace(input);
                let rhs = parse_calc_value(input)?;
                node = CalcNode::Product(Box::from([node, rhs]));
            }
            Ok(&Token::Delim('/')) => {
                skip_whitespace(input);
                let rhs = parse_calc_value(input)?;
                node = CalcNode::Product(Box::from([node, CalcNode::Invert(Box::new(rhs))]));
            }
            _ => {
                input.reset(&state);
                break;
            }
        }
    }

    Ok(node)
}

/// Parse a `<calc-value>` — a leaf or nested expression.
fn parse_calc_value<'i>(input: &mut Parser<'i, '_>) -> Result<CalcNode<SpecifiedLeaf>, Error<'i>> {
    // Nested math functions.
    if let Ok(func) = input.try_parse(|i| i.expect_function().cloned()) {
        return css_match! { &func,
            "calc" => input.parse_nested_block(parse_calc_sum),
            "min" => input.parse_nested_block(|i| {
                parse_comma_list(i).map(|args| CalcNode::MinMax(args.into(), MinMaxOp::Min))
            }),
            "max" => input.parse_nested_block(|i| {
                parse_comma_list(i).map(|args| CalcNode::MinMax(args.into(), MinMaxOp::Max))
            }),
            "clamp" => input.parse_nested_block(|i| {
                let min = parse_calc_sum(i)?;
                i.expect_comma()?;
                let center = parse_calc_sum(i)?;
                i.expect_comma()?;
                let max = parse_calc_sum(i)?;
                Ok(CalcNode::Clamp {
                    min: Box::new(min),
                    center: Box::new(center),
                    max: Box::new(max),
                })
            }),
            "abs" => input.parse_nested_block(|i| {
                parse_calc_sum(i).map(|n| CalcNode::Abs(Box::new(n)))
            }),
            "sign" => input.parse_nested_block(|i| {
                parse_calc_sum(i).map(|n| CalcNode::Sign(Box::new(n)))
            }),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        };
    }

    // Parenthesized sub-expression.
    if input.try_parse(|i| i.expect_parenthesis_block()).is_ok() {
        return input.parse_nested_block(parse_calc_sum);
    }

    // Leaf token.
    let location = input.current_source_location();
    match *input.next()? {
        Token::Number { value, .. } => {
            Ok(CalcNode::Leaf(SpecifiedLeaf::Number(value)))
        }
        Token::Percentage { unit_value, .. } => {
            Ok(CalcNode::Leaf(SpecifiedLeaf::Percentage(Percentage::new(unit_value))))
        }
        Token::Dimension { value, ref unit, .. } => {
            parse_length_unit(value, unit)
                .map(|l| CalcNode::Leaf(SpecifiedLeaf::Length(l)))
                .ok_or_else(|| location.new_custom_error(crate::CustomError::InvalidValue))
        }
        _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
    }
}

/// Parse a comma-separated list of calc sums (for min/max).
fn parse_comma_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<Vec<CalcNode<SpecifiedLeaf>>, Error<'i>> {
    let first = parse_calc_sum(input)?;
    let mut args = vec![first];
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        args.push(parse_calc_sum(input)?);
    }
    Ok(args)
}

/// Wrap a calc node into `LengthPercentage`, simplifying single-leaf trees.
fn wrap_calc(node: CalcNode<SpecifiedLeaf>) -> LengthPercentage {
    match node {
        CalcNode::Leaf(SpecifiedLeaf::Length(l)) => LengthPercentage::Length(l),
        CalcNode::Leaf(SpecifiedLeaf::Percentage(p)) => LengthPercentage::Percentage(p),
        other => LengthPercentage::Calc(Box::new(other)),
    }
}

fn skip_whitespace(input: &mut Parser<'_, '_>) {
    while input.try_parse(|i| {
        match i.next_including_whitespace() {
            Ok(Token::WhiteSpace(_)) => Ok(()),
            _ => Err(()),
        }
    }).is_ok() {}
}
