//! CSS `content`, `quotes`, and `counter-reset/increment/set` parsers.

use cssparser::Parser;
use kozan_style::{Atom, Content, ContentItem, Quotes, CounterList, CounterEntry};
use kozan_style_macros::css_match;
use crate::Error;

// Content

impl crate::Parse for Content {
    /// `normal | none | <content-item>+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(Content::Normal);
        }
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Content::None);
        }
        let mut items = Vec::new();
        loop {
            if let Ok(item) = input.try_parse(parse_content_item) {
                items.push(item);
            } else {
                break;
            }
        }
        if items.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(Content::Items(items.into_boxed_slice()))
    }
}

fn parse_content_item<'i>(input: &mut Parser<'i, '_>) -> Result<ContentItem, Error<'i>> {
    // String literal.
    if let Ok(s) = input.try_parse(|i| i.expect_string().cloned()) {
        return Ok(ContentItem::String(Atom::new(&*s)));
    }
    // url()
    if let Ok(url) = input.try_parse(|i| i.expect_url().map(|u| u.as_ref().to_owned())) {
        return Ok(ContentItem::Url(Atom::new(&*url)));
    }
    // Keywords.
    if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
        return css_match! { &ident,
            "open-quote" => Ok(ContentItem::OpenQuote),
            "close-quote" => Ok(ContentItem::CloseQuote),
            "no-open-quote" => Ok(ContentItem::NoOpenQuote),
            "no-close-quote" => Ok(ContentItem::NoCloseQuote),
            _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
        };
    }
    // Functions: counter(), counters(), attr()
    let func = input.expect_function()?.clone();
    input.parse_nested_block(|i| {
        css_match! { &func,
            "counter" => {
                let name = i.expect_ident()?.clone();
                let style = if i.try_parse(|i| i.expect_comma()).is_ok() {
                    Some(<kozan_style::ListStyleType as crate::Parse>::parse(i)?)
                } else { None };
                Ok(ContentItem::Counter(Atom::new(&*name), style))
            },
            "counters" => {
                let name = i.expect_ident()?.clone();
                i.expect_comma()?;
                let separator = i.expect_string()?.clone();
                let style = if i.try_parse(|i| i.expect_comma()).is_ok() {
                    Some(<kozan_style::ListStyleType as crate::Parse>::parse(i)?)
                } else { None };
                Ok(ContentItem::Counters(Atom::new(&*name), Atom::new(&*separator), style))
            },
            "attr" => {
                let name = i.expect_ident()?.clone();
                Ok(ContentItem::Attr(Atom::new(&*name)))
            },
            "url" => {
                let url = i.expect_string()?.clone();
                Ok(ContentItem::Url(Atom::new(&*url)))
            },
            _ => Err(i.new_custom_error(crate::CustomError::InvalidValue))
        }
    })
}

// Quotes

impl crate::Parse for Quotes {
    /// `auto | none | [<string> <string>]+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Quotes::Auto);
        }
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Quotes::None);
        }
        let mut pairs = Vec::new();
        while let Ok(open) = input.try_parse(|i| i.expect_string().cloned()) {
            let close = input.expect_string()?.clone();
            pairs.push((Atom::new(&*open), Atom::new(&*close)));
        }
        if pairs.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(Quotes::Pairs(pairs.into_boxed_slice()))
    }
}

// CounterList

impl crate::Parse for CounterList {
    /// `none | [<ident> <integer>?]+`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(CounterList::None);
        }
        let mut counters = Vec::new();
        while let Ok(name) = input.try_parse(|i| i.expect_ident().cloned()) {
            let value = input.try_parse(|i| i.expect_integer()).unwrap_or(0);
            counters.push(CounterEntry { name: Atom::new(&*name), value });
        }
        if counters.is_empty() {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(CounterList::Counters(counters.into_boxed_slice()))
    }
}
