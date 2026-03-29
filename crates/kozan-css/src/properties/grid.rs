//! CSS Grid property parsers — track lists, grid lines, template areas.

use cssparser::{Parser, Token};
use kozan_style::{
    Atom, TrackList, TrackSize, TrackEntry, TrackRepeat, RepeatCount,
    GridLine, GridTemplateAreas,
};
use kozan_style::specified::LengthPercentage;
use kozan_style_macros::css_match;
use crate::Error;

// TrackList

impl crate::Parse for TrackList {
    /// `none | <track-entry>+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(TrackList::None);
        }
        let mut entries = Vec::new();
        loop {
            // Line names: [name1 name2]
            if input.try_parse(|i| i.expect_square_bracket_block()).is_ok() {
                input.parse_nested_block(|i| {
                    while let Ok(ident) = i.try_parse(|i| i.expect_ident().cloned()) {
                        entries.push(TrackEntry::LineName(Atom::new(&*ident)));
                    }
                    Ok(())
                })?;
                continue;
            }
            // repeat()
            if let Ok(entry) = input.try_parse(parse_repeat) {
                entries.push(entry);
                continue;
            }
            // Track size
            if let Ok(size) = input.try_parse(parse_track_size) {
                entries.push(TrackEntry::Size(size));
                continue;
            }
            break;
        }
        if entries.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(TrackList::Tracks(entries.into_boxed_slice()))
    }
}

impl crate::Parse for TrackSize {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        parse_track_size(input)
    }
}

fn parse_track_size<'i>(input: &mut Parser<'i, '_>) -> Result<TrackSize, Error<'i>> {
    // Keywords.
    if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
        return css_match! { &ident,
            "auto" => Ok(TrackSize::Auto),
            "min-content" => Ok(TrackSize::MinContent),
            "max-content" => Ok(TrackSize::MaxContent),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        };
    }
    // Fr unit.
    if let Ok(fr) = input.try_parse(parse_fr) {
        return Ok(TrackSize::Fr(fr));
    }
    // Functions: minmax(), fit-content().
    if let Ok(func) = input.try_parse(|i| i.expect_function().cloned()) {
        return input.parse_nested_block(|i| {
            css_match! { &func,
                "minmax" => {
                    let min = parse_track_size(i)?;
                    i.expect_comma()?;
                    let max = parse_track_size(i)?;
                    Ok(TrackSize::MinMax(Box::new(min), Box::new(max)))
                },
                "fit-content" => {
                    let lp = <LengthPercentage as crate::Parse>::parse(i)?;
                    Ok(TrackSize::FitContent(lp))
                },
                _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
            }
        });
    }
    // Length/percentage.
    let lp = <LengthPercentage as crate::Parse>::parse(input)?;
    Ok(TrackSize::Length(lp))
}

fn parse_fr<'i>(input: &mut Parser<'i, '_>) -> Result<f32, Error<'i>> {
    let location = input.current_source_location();
    match *input.next()? {
        Token::Dimension { value, ref unit, .. } if unit.eq_ignore_ascii_case("fr") => Ok(value),
        _ => Err(location.new_custom_error(crate::CustomError::InvalidValue)),
    }
}

/// `repeat(<count>, <track-entry>+)`
fn parse_repeat<'i>(input: &mut Parser<'i, '_>) -> Result<TrackEntry, Error<'i>> {
    input.expect_function_matching("repeat")?;
    input.parse_nested_block(|i| {
        let count = parse_repeat_count(i)?;
        i.expect_comma()?;
        let mut tracks = Vec::new();
        loop {
            if let Ok(size) = i.try_parse(parse_track_size) {
                tracks.push(TrackEntry::Size(size));
                continue;
            }
            if i.try_parse(|i| i.expect_square_bracket_block()).is_ok() {
                i.parse_nested_block(|i| {
                    while let Ok(ident) = i.try_parse(|i| i.expect_ident().cloned()) {
                        tracks.push(TrackEntry::LineName(Atom::new(&*ident)));
                    }
                    Ok(())
                })?;
                continue;
            }
            break;
        }
        Ok(TrackEntry::Repeat(TrackRepeat {
            count,
            tracks: tracks.into_boxed_slice(),
        }))
    })
}

fn parse_repeat_count<'i>(input: &mut Parser<'i, '_>) -> Result<RepeatCount, Error<'i>> {
    if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
        return css_match! { &ident,
            "auto-fill" => Ok(RepeatCount::AutoFill),
            "auto-fit" => Ok(RepeatCount::AutoFit),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        };
    }
    let n = input.expect_integer()? as u32;
    Ok(RepeatCount::Number(n))
}

// GridLine

impl crate::Parse for GridLine {
    /// `auto | <integer> | <ident> | span <integer> | span <ident>`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(GridLine::Auto);
        }
        if input.try_parse(|i| i.expect_ident_matching("span")).is_ok() {
            if let Ok(n) = input.try_parse(|i| i.expect_integer()) {
                return Ok(GridLine::Span(n));
            }
            let ident = input.expect_ident()?;
            return Ok(GridLine::SpanNamed(Atom::new(&*ident)));
        }
        if let Ok(n) = input.try_parse(|i| i.expect_integer()) {
            return Ok(GridLine::Line(n));
        }
        let ident = input.expect_ident()?;
        Ok(GridLine::Named(Atom::new(&*ident)))
    }
}

// GridTemplateAreas

impl crate::Parse for GridTemplateAreas {
    /// `none | "<string>"+` — each string is a row.
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(GridTemplateAreas::None);
        }
        let mut rows = Vec::new();
        while let Ok(s) = input.try_parse(|i| i.expect_string().cloned()) {
            let row: Box<[Option<Atom>]> = s.split_whitespace()
                .map(|cell| {
                    if cell == "." { None } else { Some(Atom::new(cell)) }
                })
                .collect();
            rows.push(row);
        }
        if rows.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(GridTemplateAreas::Areas(rows.into_boxed_slice()))
    }
}
