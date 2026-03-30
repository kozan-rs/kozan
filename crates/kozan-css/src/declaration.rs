//! CSS declaration parsing — the entry point for property values.
//!
//! Implements cssparser's `DeclarationParser` trait to parse
//! `property: value` pairs into typed `PropertyDeclaration` values.

use cssparser::{
    Parser, ParserInput, CowRcStr,
    DeclarationParser, AtRuleParser, QualifiedRuleParser, RuleBodyItemParser,
};
use kozan_style::{DeclarationBlock, PropertyDeclaration, PropertyId, StyleSetter};
use smallvec::SmallVec;

/// Parses an inline style string into a `DeclarationBlock`.
pub(crate) fn parse_declaration_list(css: &str) -> DeclarationBlock {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut block = DeclarationBlock::new();

    // One-time check: does the entire CSS text contain var()/env()/attr()?
    // If not, skip the expensive per-property token scan entirely.
    let mut decl_parser = DeclParser {
        may_have_substitutions: might_contain_substitution(css),
    };
    let iter = cssparser::RuleBodyParser::new(&mut parser, &mut decl_parser);
    for result in iter {
        if let Ok(decls) = result {
            for (decl, important) in decls {
                if important { block.important(); } else { block.normal(); }
                block.on_set(decl);
            }
        }
    }

    block
}

/// Parses a single property value from CSS text.
///
/// Used by the cascade to re-parse values after `var()`/`env()`/`attr()`
/// substitution. The substituted CSS string is parsed as the target property
/// type, producing a concrete `PropertyDeclaration`.
pub fn parse_single_value(property: PropertyId, css: &str) -> Option<PropertyDeclaration> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    crate::properties::parse_property_value(property, &mut parser).ok()
}

// Inline up to 4 declarations — covers margin/padding shorthands without heap.
type DeclVec = SmallVec<[(PropertyDeclaration, bool); 4]>;

pub(crate) struct DeclParser {
    /// Pre-computed: does the full CSS text contain var()/env()/attr()?
    pub(crate) may_have_substitutions: bool,
}

impl<'i> DeclarationParser<'i> for DeclParser {
    type Declaration = DeclVec;
    type Error = crate::CustomError;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &cssparser::ParserState,
    ) -> Result<DeclVec, cssparser::ParseError<'i, Self::Error>> {
        let id = name.parse::<PropertyId>().ok()
            .or_else(|| if name.starts_with("--") { Some(PropertyId::Custom) } else { None })
            .ok_or_else(|| input.new_custom_error(crate::CustomError::UnknownProperty))?;

        // Custom properties: consume rest as raw value (no need for keyword/var checks).
        if id == PropertyId::Custom {
            let start = input.position();
            while input.next().is_ok() {}
            let raw = input.slice_from(start).trim();
            return Ok(smallvec::smallvec![(
                PropertyDeclaration::Custom {
                    name: kozan_style::Atom::new(&name),
                    value: kozan_style::Atom::new(raw),
                },
                false,
            )]);
        }

        // CSS-wide keywords — peek first token to avoid try_parse overhead on miss.
        // For Tailwind-style CSS (~0% keywords, ~5000 declarations), skipping the
        // full try_parse save/restore cycle saves ~250µs.
        {
            let state = input.state();
            if let Ok(token) = input.next() {
                if let cssparser::Token::Ident(ref ident) = *token {
                    if let Some(kw) = match_css_wide_keyword(ident) {
                        let decls = apply_keyword_to_longhands(id, &kw);
                        let important = input.try_parse(cssparser::parse_important).is_ok();
                        return Ok(decls.into_iter().map(|d| (d, important)).collect());
                    }
                }
                input.reset(&state);
            }
        }

        // Only run the expensive token-level var() scan if the full CSS text
        // actually contains a substitution function substring.
        if self.may_have_substitutions {
            if let Some(unparsed) = crate::var::scan_for_substitutions(input) {
                let important = input.try_parse(cssparser::parse_important).is_ok();
                return Ok(make_unparsed(id, unparsed, &name, important));
            }
        }

        // Shorthand value parsing (generated same-type + hand-written mixed-type).
        if let Some(result) = crate::shorthand::parse_shorthand(id, input) {
            let decls = result?;
            let important = input.try_parse(cssparser::parse_important).is_ok();
            return Ok(decls.into_iter().map(|d| (d, important)).collect());
        }

        // Typed parse via generated dispatch (longhands only).
        let decl = crate::properties::parse_property_value(id, input)?;
        let important = input.try_parse(cssparser::parse_important).is_ok();
        Ok(smallvec::smallvec![(decl, important)])
    }
}

impl<'i> AtRuleParser<'i> for DeclParser {
    type Prelude = ();
    type AtRule = DeclVec;
    type Error = crate::CustomError;
}

impl<'i> QualifiedRuleParser<'i> for DeclParser {
    type Prelude = ();
    type QualifiedRule = DeclVec;
    type Error = crate::CustomError;
}

impl<'i> RuleBodyItemParser<'i, DeclVec, crate::CustomError> for DeclParser {
    fn parse_declarations(&self) -> bool { true }
    fn parse_qualified(&self) -> bool { false }
}

/// Case-insensitive substring search for ASCII needles.
#[inline]
pub(crate) fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w.iter().zip(needle).all(|(h, n)| h.to_ascii_lowercase() == *n))
}

/// Fast check — does the raw CSS text potentially contain var()/env()/attr()?
/// False positives are fine (the full token scan handles them).
#[inline]
fn might_contain_substitution(css: &str) -> bool {
    let b = css.as_bytes();
    contains_ci(b, b"var(") || contains_ci(b, b"env(") || contains_ci(b, b"attr(")
}

pub(crate) use kozan_style::CssWideKeyword;

/// Match a CSS-wide keyword from an already-peeked ident token.
/// Returns `None` for non-keyword idents — no parser state overhead.
#[inline]
pub(crate) fn match_css_wide_keyword(ident: &str) -> Option<CssWideKeyword> {
    use kozan_style_macros::css_match;
    Some(css_match! { ident,
        "inherit" => CssWideKeyword::Inherit,
        "initial" => CssWideKeyword::Initial,
        "unset" => CssWideKeyword::Unset,
        "revert" => CssWideKeyword::Revert,
        "revert-layer" => CssWideKeyword::RevertLayer,
        _ => return None,
    })
}

pub(crate) fn make_unparsed(
    id: PropertyId,
    unparsed: kozan_style::UnparsedValue,
    name: &str,
    important: bool,
) -> DeclVec {
    if id == PropertyId::Custom {
        return smallvec::smallvec![(
            PropertyDeclaration::Custom {
                name: kozan_style::Atom::new(name),
                value: unparsed.css,
            },
            important,
        )];
    }
    apply_to_longhands(id, |pid| {
        crate::properties::make_unparsed_declaration(pid, unparsed.clone())
    })
    .into_iter()
    .map(|d| (d, important))
    .collect()
}

/// Expand a CSS-wide keyword to longhand declarations for the given property.
pub(crate) fn apply_keyword_to_longhands(id: PropertyId, kw: &CssWideKeyword) -> SmallVec<[PropertyDeclaration; 4]> {
    apply_to_longhands(id, |pid| crate::properties::make_keyword_declaration(pid, kw))
}

/// Expand a property (possibly shorthand) to its longhands, applying `f` to each.
fn apply_to_longhands<F>(id: PropertyId, f: F) -> SmallVec<[PropertyDeclaration; 4]>
where
    F: Fn(PropertyId) -> Option<PropertyDeclaration>,
{
    if let Some(longhands) = id.longhands() {
        longhands.iter().filter_map(|&pid| f(pid)).collect()
    } else {
        f(id).into_iter().collect()
    }
}
