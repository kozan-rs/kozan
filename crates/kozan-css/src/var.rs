//! Detection of `var()`, `env()`, `attr()` substitution functions.
//!
//! When the parser encounters any substitution function in a property value,
//! it stores the entire value as raw CSS text in `Declared::WithVariables`.
//! The cascade substitutes references, then re-parses the result.

use cssparser::{Parser, Token};
use kozan_style::{SubstitutionRefs, UnparsedValue};

/// Scan a property value for substitution functions.
///
/// If `var()`, `env()`, or `attr()` is found, consumes the entire value
/// and returns an `UnparsedValue` with raw CSS text + reference flags.
/// If none found, resets the parser to its original position and returns `None`.
pub(crate) fn scan_for_substitutions<'i>(
    input: &mut Parser<'i, '_>,
) -> Option<UnparsedValue> {
    let state = input.state();
    let start = input.position();
    let mut refs = SubstitutionRefs::empty();

    consume_all(input, &mut refs);

    if refs.is_empty() {
        input.reset(&state);
        return None;
    }

    Some(UnparsedValue {
        css: kozan_style::Atom::new(input.slice_from(start)),
        references: refs,
    })
}

/// Consume all tokens in the current block, collecting substitution refs.
fn consume_all(input: &mut Parser<'_, '_>, refs: &mut SubstitutionRefs) {
    while let Ok(token) = input.next_including_whitespace_and_comments() {
        match token {
            Token::Function(name) => {
                if name.eq_ignore_ascii_case("var") {
                    *refs = *refs | SubstitutionRefs::VAR;
                } else if name.eq_ignore_ascii_case("env") {
                    *refs = *refs | SubstitutionRefs::ENV;
                } else if name.eq_ignore_ascii_case("attr") {
                    *refs = *refs | SubstitutionRefs::ATTR;
                }
                let _ = input.parse_nested_block(|nested| {
                    consume_all(nested, refs);
                    Ok::<_, cssparser::ParseError<'_, ()>>(())
                });
            }
            Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                let _ = input.parse_nested_block(|nested| {
                    consume_all(nested, refs);
                    Ok::<_, cssparser::ParseError<'_, ()>>(())
                });
            }
            _ => {}
        }
    }
}
