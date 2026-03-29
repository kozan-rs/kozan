//! CSS font property parsers — `font-weight`, `font-family`, `font-feature-settings`, etc.

use cssparser::Parser;
use kozan_style::{
    Atom, FontWeight, FontFamily, FontFeatureSettings, FontVariationSettings,
    FamilyEntry, GenericFamily, FontFeature, FontVariation,
};
use kozan_style_macros::css_match;
use crate::Error;

impl crate::Parse for FontWeight {
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident().cloned()) {
            return css_match! { &ident,
                "normal" => Ok(FontWeight(400)),
                "bold" => Ok(FontWeight(700)),
                "lighter" => Ok(FontWeight(100)),
                "bolder" => Ok(FontWeight(700)),
                _ => Err(input.new_custom_error(crate::CustomError::InvalidValue))
            };
        }
        let location = input.current_source_location();
        let value = input.expect_number()?;
        let v = value as u16;
        if !(1..=1000).contains(&v) {
            return Err(location.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(FontWeight(v))
    }
}

impl crate::Parse for FontFamily {
    /// `<family-name>#` — comma-separated list of font families.
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        let first = parse_family_entry(input)?;
        let mut entries = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            entries.push(parse_family_entry(input)?);
        }
        Ok(FontFamily(entries.into_boxed_slice()))
    }
}

fn parse_family_entry<'i>(input: &mut Parser<'i, '_>) -> Result<FamilyEntry, Error<'i>> {
    // Quoted string → named family.
    if let Ok(s) = input.try_parse(|i| i.expect_string().cloned()) {
        return Ok(FamilyEntry::Named(Atom::new(&*s)));
    }
    // Ident — check for generic families first.
    let ident = input.expect_ident()?.clone();
    if let Some(generic) = parse_generic_family(&ident) {
        return Ok(FamilyEntry::Generic(generic));
    }
    // Multi-word unquoted family name: consume consecutive idents.
    let mut name = ident.to_string();
    while let Ok(next) = input.try_parse(|i| i.expect_ident().cloned()) {
        name.push(' ');
        name.push_str(&next);
    }
    Ok(FamilyEntry::Named(Atom::new(&name)))
}

fn parse_generic_family(ident: &str) -> Option<GenericFamily> {
    Some(css_match! { ident,
        "serif" => GenericFamily::Serif,
        "sans-serif" => GenericFamily::SansSerif,
        "monospace" => GenericFamily::Monospace,
        "cursive" => GenericFamily::Cursive,
        "fantasy" => GenericFamily::Fantasy,
        "system-ui" => GenericFamily::SystemUi,
        "ui-serif" => GenericFamily::UiSerif,
        "ui-sans-serif" => GenericFamily::UiSansSerif,
        "ui-monospace" => GenericFamily::UiMonospace,
        "ui-rounded" => GenericFamily::UiRounded,
        "emoji" => GenericFamily::Emoji,
        "math" => GenericFamily::Math,
        "fangsong" => GenericFamily::Fangsong,
        _ => return None
    })
}

impl crate::Parse for FontFeatureSettings {
    /// `normal | <feature-tag-value>#`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(FontFeatureSettings::Normal);
        }
        let first = parse_font_feature(input)?;
        let mut features = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            features.push(parse_font_feature(input)?);
        }
        Ok(FontFeatureSettings::Features(features.into_boxed_slice()))
    }
}

/// `"liga" [<integer>]?` — 4-char tag + optional value (default 1).
fn parse_font_feature<'i>(input: &mut Parser<'i, '_>) -> Result<FontFeature, Error<'i>> {
    let tag = input.expect_string()?.clone();
    let value = input.try_parse(|i| i.expect_integer()).unwrap_or(1) as u32;
    Ok(FontFeature { tag: Atom::new(&*tag), value })
}

impl crate::Parse for FontVariationSettings {
    /// `normal | <variation>#`
    fn parse<'i>(input: &mut Parser<'i, '_>) -> Result<Self, Error<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
            return Ok(FontVariationSettings::Normal);
        }
        let first = parse_font_variation(input)?;
        let mut variations = vec![first];
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            variations.push(parse_font_variation(input)?);
        }
        Ok(FontVariationSettings::Variations(variations.into_boxed_slice()))
    }
}

/// `"wght" 700` — 4-char tag + number.
fn parse_font_variation<'i>(input: &mut Parser<'i, '_>) -> Result<FontVariation, Error<'i>> {
    let tag = input.expect_string()?.clone();
    let value = input.expect_number()?;
    Ok(FontVariation { tag: Atom::new(&*tag), value })
}
