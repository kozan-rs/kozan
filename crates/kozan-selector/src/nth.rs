//! An+B notation parsing for `:nth-child()`, `:nth-last-child()`,
//! `:nth-of-type()`, and `:nth-last-of-type()`.
//!
//! Delegates to `cssparser::parse_nth` which handles all edge cases
//! correctly per the CSS Syntax spec (whitespace rules, sign handling, etc.).

use cssparser::Parser;

use crate::parser::ParseError;

/// Parses an `An+B` expression from the token stream.
///
/// Supports all valid forms:
/// - `odd` → (2, 1)
/// - `even` → (2, 0)
/// - `3n+1` → (3, 1)
/// - `-n+3` → (-1, 3)
/// - `5` → (0, 5)
/// - `n` → (1, 0)
pub fn parse_nth<'i>(input: &mut Parser<'i, '_>) -> Result<(i32, i32), ParseError<'i>> {
    cssparser::parse_nth(input).map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> (i32, i32) {
        let mut parser_input = cssparser::ParserInput::new(input);
        let mut parser = cssparser::Parser::new(&mut parser_input);
        parse_nth(&mut parser).unwrap()
    }

    fn parse_fails(input: &str) -> bool {
        let mut parser_input = cssparser::ParserInput::new(input);
        let mut parser = cssparser::Parser::new(&mut parser_input);
        parse_nth(&mut parser).is_err()
    }

    #[test]
    fn keywords() {
        assert_eq!(parse("odd"), (2, 1));
        assert_eq!(parse("even"), (2, 0));
    }

    #[test]
    fn simple_number() {
        assert_eq!(parse("3"), (0, 3));
        assert_eq!(parse("1"), (0, 1));
        assert_eq!(parse("-1"), (0, -1));
    }

    #[test]
    fn an_plus_b() {
        assert_eq!(parse("2n+1"), (2, 1));
        assert_eq!(parse("3n+0"), (3, 0));
        assert_eq!(parse("2n"), (2, 0));
    }

    #[test]
    fn negative_a() {
        assert_eq!(parse("-n+3"), (-1, 3));
    }

    #[test]
    fn bare_n() {
        assert_eq!(parse("n"), (1, 0));
        assert_eq!(parse("n+2"), (1, 2));
    }

    #[test]
    fn n_dash_space_b() {
        // "n- 1" is valid per spec (dash attached to n, space before digit)
        assert_eq!(parse("3n- 1"), (3, -1));
        assert_eq!(parse("n- 1"), (1, -1));
        assert_eq!(parse("-n- 1"), (-1, -1));
    }

    #[test]
    fn plus_n_no_space() {
        // "+n" valid (no space between + and n)
        assert_eq!(parse("+n"), (1, 0));
        assert_eq!(parse("+n+1"), (1, 1));
    }

    #[test]
    fn space_between_plus_and_n_invalid() {
        // "+ n" invalid (space between + and n)
        assert!(parse_fails("+ n"));
    }
}
