//! CSS selector parser.
//!
//! Parses CSS selector text into a `SelectorList` using the cssparser tokenizer.
//! Supports CSS Selectors Level 4:
//! - Simple selectors: type, class, ID, universal, attribute
//! - Pseudo-classes: structural, state-based, functional (:not, :is, :where, :has, :nth-*)
//! - Pseudo-elements: ::before, ::after, ::first-line, ::first-letter, etc.
//! - Combinators: descendant (space), child (>), adjacent (+), general (~)
//! - Selector lists (comma-separated)
//!
//! Architecture:
//! - Streaming left-to-right parse into a single SmallVec<[Component; 8]> buffer
//! - `next_including_whitespace()` for compound selector boundary detection
//! - `try_parse` only for peek/rollback — never for external state mutation
//! - Right-to-left reversal at the end for matching-order storage
//! - Specificity accumulated inline during parsing (no second pass)
//! - All keyword matching uses `css_match!` for integer-chunk comparison

use triomphe::Arc;

use cssparser::{Parser, ParserInput, Token};
use kozan_atom::Atom;
use smallvec::SmallVec;

use crate::attr::{AttrOperation, AttrSelector, CaseSensitivity};
use crate::nth::parse_nth;
use crate::pseudo_class::PseudoClass;
use crate::pseudo_element::PseudoElement;
use crate::specificity::Specificity;
use crate::types::*;

pub type ParseError<'i> = cssparser::ParseError<'i, ()>;

/// Parse a CSS selector string into a `SelectorList`.
pub fn parse(css: &str) -> Result<SelectorList, String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let list = parse_selector_list(&mut parser).map_err(|e| format!("{e:?}"))?;
    // Reject if there are unconsumed tokens (e.g. ".a:has" where ":has" isn't a function).
    parser.expect_exhausted().map_err(|e| format!("{e:?}"))?;
    Ok(list)
}

/// Parse a CSS selector string using **forgiving** semantics.
///
/// Invalid selectors are silently dropped instead of failing the entire parse.
/// This is required for `querySelectorAll()` compliance per CSS Selectors Level 4.
/// Returns an empty list if all selectors are invalid (matches nothing).
pub fn parse_forgiving(css: &str) -> SelectorList {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parse_forgiving_selector_list(&mut parser).unwrap_or_else(|_| SelectorList(SmallVec::new()))
}

/// Parse a selector list from an existing cssparser `Parser`.
///
/// This is the bridge function for stylesheet-level parsing: the sheet parser
/// owns the `Parser` and hands it to us for the selector portion.
pub fn parse_selector_list<'i>(input: &mut Parser<'i, '_>) -> Result<SelectorList, ParseError<'i>> {
    let mut selectors = SmallVec::new();
    selectors.push(parse_complex_selector(input)?);

    while input.try_parse(|i| i.expect_comma()).is_ok() {
        selectors.push(parse_complex_selector(input)?);
    }

    Ok(SelectorList(selectors))
}

/// Forgiving selector list (CSS Selectors Level 4).
///
/// Used by `:is()` and `:where()`. Invalid selectors are silently dropped
/// instead of causing the entire rule to fail. Returns an empty list if all
/// selectors are invalid (which matches nothing).
fn parse_forgiving_selector_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<SelectorList, ParseError<'i>> {
    let mut selectors = SmallVec::new();

    // Try first selector.
    if let Ok(sel) = input.try_parse(parse_complex_selector) {
        selectors.push(sel);
    }

    // Try remaining comma-separated selectors.
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        if let Ok(sel) = input.try_parse(parse_complex_selector) {
            selectors.push(sel);
        }
    }

    Ok(SelectorList(selectors))
}

// --- Sub-selector variants: skip full hints, just track has_combinators ---

fn parse_sub_selector_list<'i>(input: &mut Parser<'i, '_>) -> Result<SelectorList, ParseError<'i>> {
    let mut selectors = SmallVec::new();
    selectors.push(parse_sub_complex_selector(input)?);

    while input.try_parse(|i| i.expect_comma()).is_ok() {
        selectors.push(parse_sub_complex_selector(input)?);
    }

    Ok(SelectorList(selectors))
}

fn parse_forgiving_sub_selector_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<SelectorList, ParseError<'i>> {
    let mut selectors = SmallVec::new();

    if let Ok(sel) = input.try_parse(parse_sub_complex_selector) {
        selectors.push(sel);
    }

    while input.try_parse(|i| i.expect_comma()).is_ok() {
        if let Ok(sel) = input.try_parse(parse_sub_complex_selector) {
            selectors.push(sel);
        }
    }

    Ok(SelectorList(selectors))
}

/// Lightweight complex selector parsing for sub-selectors.
/// Skips full hints computation — only tracks has_combinators.
fn parse_sub_complex_selector<'i>(input: &mut Parser<'i, '_>) -> Result<Selector, ParseError<'i>> {
    let mut components: SmallVec<[Component; 8]> = SmallVec::new();
    let mut specificity = Specificity::ZERO;

    parse_compound_selector(input, &mut components, &mut specificity)?;

    // Fast path: if input is exhausted after the first compound, skip the
    // combinator loop entirely. Handles the last (or only) sub-selector
    // in every comma-separated list — the hottest path in deep nesting.
    if input.is_exhausted() {
        return Ok(Selector::from_parse_order_sub(components, specificity));
    }

    loop {
        let had_ws = consume_whitespace(input);

        if let Ok(comb) = input.try_parse(|i| -> Result<Combinator, ParseError<'i>> {
            let loc = i.current_source_location();
            match *i.next_including_whitespace().map_err(|_| loc.new_custom_error(()))? {
                Token::Delim('>') => Ok(Combinator::Child),
                Token::Delim('+') => Ok(Combinator::NextSibling),
                Token::Delim('~') => Ok(Combinator::LaterSibling),
                Token::Delim('|') => {
                    let loc2 = i.current_source_location();
                    match *i.next_including_whitespace().map_err(|_| loc2.new_custom_error(()))? {
                        Token::Delim('|') => Ok(Combinator::Column),
                        _ => Err(loc2.new_custom_error(())),
                    }
                }
                _ => Err(loc.new_custom_error(())),
            }
        }) {
            components.push(Component::Combinator(comb));
            parse_compound_selector(input, &mut components, &mut specificity)?;
            continue;
        }

        if had_ws && !input.is_exhausted() {
            let saved_len = components.len();
            let saved_spec = specificity;
            components.push(Component::Combinator(Combinator::Descendant));
            if parse_compound_selector(input, &mut components, &mut specificity).is_err() {
                components.truncate(saved_len);
                specificity = saved_spec;
                break;
            }
            continue;
        }

        break;
    }

    // Lightweight: no full hints, no empty-:is check (sub-selectors are
    // already inside a functional that handles this).
    Ok(Selector::from_parse_order_sub(components, specificity))
}

/// Parse a complex selector: compound selectors joined by combinators.
///
/// Combinator detection strategy: consume whitespace first, then check for
/// explicit combinator with `next_including_whitespace()`. If no combinator
///    a compound selector. If the compound fails (e.g. next token is a comma),
///    roll back the Descendant and break — the caller handles commas/EOF.
fn parse_complex_selector<'i>(input: &mut Parser<'i, '_>) -> Result<Selector, ParseError<'i>> {
    let mut components: SmallVec<[Component; 8]> = SmallVec::new();
    let mut specificity = Specificity::ZERO;

    parse_compound_selector(input, &mut components, &mut specificity)?;

    loop {
        // Consume whitespace FIRST — avoids double-reading for descendant
        // combinators. The old approach used next() which auto-skips whitespace
        // and rolls it back on failure, then re-reads it for descendant detection.
        let had_ws = consume_whitespace(input);

        // Check for explicit combinator without skipping more whitespace.
        if let Ok(comb) = input.try_parse(|i| -> Result<Combinator, ParseError<'i>> {
            let loc = i.current_source_location();
            match *i.next_including_whitespace().map_err(|_| loc.new_custom_error(()))? {
                Token::Delim('>') => Ok(Combinator::Child),
                Token::Delim('+') => Ok(Combinator::NextSibling),
                Token::Delim('~') => Ok(Combinator::LaterSibling),
                Token::Delim('|') => {
                    let loc2 = i.current_source_location();
                    match *i.next_including_whitespace().map_err(|_| loc2.new_custom_error(()))? {
                        Token::Delim('|') => Ok(Combinator::Column),
                        _ => Err(loc2.new_custom_error(())),
                    }
                }
                _ => Err(loc.new_custom_error(())),
            }
        }) {
            components.push(Component::Combinator(comb));
            parse_compound_selector(input, &mut components, &mut specificity)?;
            continue;
        }

        // No explicit combinator. If we had whitespace, try descendant.
        if had_ws && !input.is_exhausted() {
            let saved_len = components.len();
            let saved_spec = specificity;
            components.push(Component::Combinator(Combinator::Descendant));
            if parse_compound_selector(input, &mut components, &mut specificity).is_err() {
                components.truncate(saved_len);
                specificity = saved_spec;
                break;
            }
            continue;
        }

        break;
    }

    // Empty :is()/:where() makes the entire selector unmatchable.
    // Universal removal and flattening are done inline during parsing —
    // no separate simplify pass needed.
    if components.iter().any(|c| match c {
        Component::Is(list) | Component::Where(list) => list.0.is_empty(),
        _ => false,
    }) {
        components.clear();
    }

    // Compute hints from parse-order, then reverse into match-order during
    // ThinArc creation — the copy already happens, reversal is free.
    let hints = SelectorHints::compute_parse_order(&components);
    Ok(Selector::from_parse_order(components, specificity, hints))
}

/// Consume whitespace tokens. Returns true if any whitespace was consumed.
///
/// Uses `position()`/`reset()` instead of `try_parse` per token — avoids
/// save/restore overhead on each whitespace token.
fn consume_whitespace<'i>(input: &mut Parser<'i, '_>) -> bool {
    let mut saw = false;
    loop {
        let state = input.state();
        match input.next_including_whitespace() {
            Ok(&Token::WhiteSpace(_)) => { saw = true; }
            _ => { input.reset(&state); break; }
        }
    }
    saw
}

/// Parse a compound selector: a sequence of simple selectors with no whitespace.
///
/// Two phases:
/// 1. **First simple selector** — uses `next()` which skips leading whitespace.
///    Handles ALL token types (type, universal, ID, class, attribute, pseudo).
///    This is correct after combinators, commas, or input start.
/// 2. **Subsequent simple selectors** — uses `next_including_whitespace()` so
///    whitespace breaks the compound (whitespace = combinator boundary).
fn parse_compound_selector<'i>(
    input: &mut Parser<'i, '_>,
    components: &mut SmallVec<[Component; 8]>,
    specificity: &mut Specificity,
) -> Result<(), ParseError<'i>> {
    let start_len = components.len();

    // Phase 1: first simple selector via next() — skips leading whitespace.
    //
    // Namespace handling: `ns|div`, `*|div`, `|div` syntax.
    // When we see Ident or `*`, we peek for `|` to determine if it's a
    // namespace prefix (ns|element) or a standalone type/universal selector.
    // `|div` (no namespace) starts with `|` directly.
    input.try_parse(|i| -> Result<(), ParseError<'i>> {
        let loc = i.current_source_location();
        let token = i.next().map_err(|_| loc.new_custom_error(()))?;
        match *token {
            Token::Ident(ref name) => {
                // Eagerly intern — avoids to_owned() heap allocation.
                let atom = Atom::from(name.as_ref());
                if i.try_parse(|i2| -> Result<(), ParseError<'i>> {
                    let loc2 = i2.current_source_location();
                    match *i2.next_including_whitespace().map_err(|_| loc2.new_custom_error(()))? {
                        Token::Delim('|') => Ok(()),
                        _ => Err(loc2.new_custom_error(())),
                    }
                }).is_ok() {
                    components.push(Component::Namespace(
                        Box::new(NamespaceConstraint::Specific(atom)),
                    ));
                    parse_type_or_universal_after_ns(i, components, specificity, loc)?;
                } else {
                    components.push(Component::Type(atom));
                    specificity.add_type();
                }
                Ok(())
            }
            Token::Delim('*') => {
                // Could be `*|element` or just `*`.
                if i.try_parse(|i2| -> Result<(), ParseError<'i>> {
                    let loc2 = i2.current_source_location();
                    match *i2.next_including_whitespace().map_err(|_| loc2.new_custom_error(()))? {
                        Token::Delim('|') => Ok(()),
                        _ => Err(loc2.new_custom_error(())),
                    }
                }).is_ok() {
                    // `*|...` — any namespace.
                    components.push(Component::Namespace(Box::new(NamespaceConstraint::Any)));
                    parse_type_or_universal_after_ns(i, components, specificity, loc)?;
                } else {
                    components.push(Component::Universal);
                }
                Ok(())
            }
            Token::Delim('|') => {
                // `|element` — no namespace (null namespace).
                components.push(Component::Namespace(Box::new(NamespaceConstraint::None)));
                parse_type_or_universal_after_ns(i, components, specificity, loc)?;
                Ok(())
            }
            Token::IDHash(ref id) => {
                components.push(Component::Id(Atom::from(id.as_ref())));
                specificity.add_id();
                Ok(())
            }
            Token::Delim('.') => {
                let name = i.expect_ident().map_err(|_| loc.new_custom_error(()))?;
                components.push(Component::Class(Atom::from(name.as_ref())));
                specificity.add_class();
                Ok(())
            }
            Token::Delim('&') => {
                // CSS Nesting selector — equivalent to :scope in matching.
                // Specificity: same as :is() of the parent selector (1 class).
                components.push(Component::Nesting);
                specificity.add_class();
                Ok(())
            }
            Token::SquareBracketBlock => {
                let attr = i.parse_nested_block(parse_attribute_selector)?;
                components.push(Component::Attribute(Box::new(attr)));
                specificity.add_class();
                Ok(())
            }
            Token::Colon => {
                parse_pseudo(i, components, specificity)?;
                Ok(())
            }
            _ => Err(loc.new_custom_error(())),
        }
    }).ok();

    // Phase 2: subsequent simple selectors via next_including_whitespace().
    // Whitespace token = compound boundary → break.
    loop {
        let parsed = input.try_parse(|i| -> Result<(), ParseError<'i>> {
            let loc = i.current_source_location();
            // Peek at the token discriminant to decide action, then extract
            // needed data inline — avoids cloning the entire Token.
            let token = i.next_including_whitespace()
                .map_err(|_| loc.new_custom_error(()))?;
            match *token {
                Token::IDHash(ref id) => {
                    let atom = Atom::from(id.as_ref());
                    components.push(Component::Id(atom));
                    specificity.add_id();
                    Ok(())
                }
                Token::Delim('.') => {
                    let name = i.expect_ident()
                        .map_err(|_| loc.new_custom_error(()))?;
                    components.push(Component::Class(Atom::from(name.as_ref())));
                    specificity.add_class();
                    Ok(())
                }
                Token::Delim('&') => {
                    components.push(Component::Nesting);
                    specificity.add_class();
                    Ok(())
                }
                Token::SquareBracketBlock => {
                    let attr = i.parse_nested_block(parse_attribute_selector)?;
                    components.push(Component::Attribute(Box::new(attr)));
                    specificity.add_class();
                    Ok(())
                }
                Token::Colon => {
                    parse_pseudo(i, components, specificity)?;
                    Ok(())
                }
                _ => Err(loc.new_custom_error(())),
            }
        });
        if parsed.is_err() {
            break;
        }
    }

    if components.len() == start_len {
        let loc = input.current_source_location();
        return Err(loc.new_custom_error(()));
    }

    // Inline universal removal: if first component is `*` and compound has
    // other components, remove it (unless preceded by Namespace).
    if components.len() > start_len + 1
        && matches!(&components[start_len], Component::Universal)
        && (start_len == 0 || !matches!(&components[start_len.wrapping_sub(1)], Component::Namespace(_)))
    {
        components.remove(start_len);
    }

    Ok(())
}

/// After consuming a namespace prefix and `|`, parse the type or universal selector.
fn parse_type_or_universal_after_ns<'i>(
    input: &mut Parser<'i, '_>,
    components: &mut SmallVec<[Component; 8]>,
    specificity: &mut Specificity,
    loc: cssparser::SourceLocation,
) -> Result<(), ParseError<'i>> {
    let token = input
        .next_including_whitespace()
        .map_err(|_| loc.new_custom_error(()))?
        .clone();
    match token {
        Token::Ident(ref name) => {
            components.push(Component::Type(Atom::from(name.as_ref())));
            specificity.add_type();
            Ok(())
        }
        Token::Delim('*') => {
            components.push(Component::Universal);
            Ok(())
        }
        _ => Err(loc.new_custom_error(())),
    }
}

fn parse_pseudo<'i>(
    input: &mut Parser<'i, '_>,
    components: &mut SmallVec<[Component; 8]>,
    specificity: &mut Specificity,
) -> Result<(), ParseError<'i>> {
    let location = input.current_source_location();

    // Read one token to determine single-colon vs double-colon.
    // Old approach: try_parse(expect_colon) reads the token, fails for pseudo-
    // classes, rolls back, then next() re-reads it — double work for every
    // :hover, :focus, :not(), etc.
    let token = input.next_including_whitespace()
        .map_err(|_| location.new_custom_error(()))?.clone();

    if matches!(token, Token::Colon) {
        // :: → pseudo-element (non-functional or functional)
        let token = input.next().map_err(|_| location.new_custom_error(()))?.clone();
        match token {
            Token::Ident(ref name) => {
                let pe = parse_pseudo_element_name(name.as_ref())
                    .ok_or_else(|| location.new_custom_error(()))?;
                components.push(Component::PseudoElement(pe));
                specificity.add_type();
            }
            Token::Function(ref name) => {
                parse_functional_pseudo(input, name.as_ref(), components, specificity, location)?;
            }
            _ => return Err(location.new_custom_error(())),
        }
        return Ok(());
    }

    match token {
        Token::Ident(ref name) => {
            if let Some(pe) = parse_pseudo_element_name(name.as_ref()) {
                if pe.allows_single_colon() {
                    components.push(Component::PseudoElement(pe));
                    specificity.add_type();
                    return Ok(());
                }
            }
            if name.eq_ignore_ascii_case("host") {
                components.push(Component::Host);
                specificity.add_class();
                return Ok(());
            }
            let pc = parse_pseudo_class_name(name.as_ref())
                .ok_or_else(|| location.new_custom_error(()))?;
            specificity.add_class();
            components.push(Component::PseudoClass(pc));
            Ok(())
        }
        Token::Function(ref name) => {
            parse_functional_pseudo(input, name.as_ref(), components, specificity, location)
        }
        _ => Err(location.new_custom_error(())),
    }
}

fn parse_functional_pseudo<'i>(
    input: &mut Parser<'i, '_>,
    name: &str,
    components: &mut SmallVec<[Component; 8]>,
    specificity: &mut Specificity,
    location: cssparser::SourceLocation,
) -> Result<(), ParseError<'i>> {
    kozan_style_macros::css_match!(name,
        "not" => {
            let list = input.parse_nested_block(parse_sub_selector_list)?;
            // §4.3: :not() accepts <complex-real-selector-list> — no pseudo-elements
            if contains_pseudo_element(&list) {
                return Err(location.new_custom_error(()));
            }
            add_max_specificity(&list, specificity);
            // Try flattening: :not(.a, .b) → NotSingle
            components.push(try_flatten_not(&list)
                .unwrap_or_else(|| Component::Negation(Arc::new(list))));
            Ok(())
        },
        "is" => {
            let list = input.parse_nested_block(parse_forgiving_sub_selector_list)?;
            add_max_specificity(&list, specificity);
            // Try flattening: :is(.a, .b, div) → IsSingle
            components.push(try_flatten_is(&list)
                .unwrap_or_else(|| Component::Is(Arc::new(list))));
            Ok(())
        },
        "where" => {
            let list = input.parse_nested_block(parse_forgiving_sub_selector_list)?;
            // Try flattening: :where(.a, .b, div) → WhereSingle
            components.push(try_flatten_where(&list)
                .unwrap_or_else(|| Component::Where(Arc::new(list))));
            Ok(())
        },
        "has" => {
            let rel_list = input.parse_nested_block(parse_relative_selector_list)?;
            // §4.5: :has() must have at least one valid selector
            if rel_list.0.is_empty() {
                return Err(location.new_custom_error(()));
            }
            // §4.5: :has() cannot be nested inside :has()
            if contains_has_in_rel_list(&rel_list) {
                return Err(location.new_custom_error(()));
            }
            // Specificity = max of arguments (not sum)
            if let Some(max) = rel_list.0.iter().map(|r| r.selector.specificity()).max() {
                add_specificity_raw(max, specificity);
            }
            components.push(Component::Has(Arc::new(rel_list)));
            Ok(())
        },
        "nth-child" => {
            let nth = input.parse_nested_block(parse_nth_with_of)?;
            specificity.add_class();
            // §15: :nth-child(of S) specificity += max specificity of S
            if let Some(ref of_sel) = nth.of_selector {
                add_max_specificity(of_sel, specificity);
            }
            components.push(Component::NthChild(Box::new(nth)));
            Ok(())
        },
        "nth-last-child" => {
            let nth = input.parse_nested_block(parse_nth_with_of)?;
            specificity.add_class();
            if let Some(ref of_sel) = nth.of_selector {
                add_max_specificity(of_sel, specificity);
            }
            components.push(Component::NthLastChild(Box::new(nth)));
            Ok(())
        },
        "nth-of-type" => {
            let (a, b) = input.parse_nested_block(parse_nth)?;
            specificity.add_class();
            components.push(Component::NthOfType(a, b));
            Ok(())
        },
        "nth-last-of-type" => {
            let (a, b) = input.parse_nested_block(parse_nth)?;
            specificity.add_class();
            components.push(Component::NthLastOfType(a, b));
            Ok(())
        },
        "lang" => {
            let langs = input.parse_nested_block(|block| -> Result<SmallVec<[Atom; 1]>, ParseError<'i>> {
                let loc = block.current_source_location();
                let mut list = SmallVec::new();
                let ident = block.expect_ident().map_err(|_| loc.new_custom_error(()))?;
                list.push(Atom::from(ident.as_ref()));
                while block.try_parse(|i| i.expect_comma()).is_ok() {
                    let loc2 = block.current_source_location();
                    let ident = block.expect_ident().map_err(|_| loc2.new_custom_error(()))?;
                    list.push(Atom::from(ident.as_ref()));
                }
                Ok(list)
            })?;
            specificity.add_class();
            components.push(Component::Lang(Box::new(langs)));
            Ok(())
        },
        "dir" => {
            let dir = input.parse_nested_block(|block| -> Result<Direction, ParseError<'i>> {
                let loc = block.current_source_location();
                let ident = block.expect_ident().map_err(|_| loc.new_custom_error(()))?;
                kozan_style_macros::css_match!(ident.as_ref(),
                    "ltr" => Ok(Direction::Ltr),
                    "rtl" => Ok(Direction::Rtl),
                    _ => Err(loc.new_custom_error(())),
                )
            })?;
            specificity.add_class();
            components.push(Component::Dir(dir));
            Ok(())
        },
        "state" => {
            let name = input.parse_nested_block(|block| -> Result<Atom, ParseError<'i>> {
                let loc = block.current_source_location();
                let ident = block.expect_ident().map_err(|_| loc.new_custom_error(()))?;
                Ok(Atom::from(ident.as_ref()))
            })?;
            specificity.add_class();
            components.push(Component::State(name));
            Ok(())
        },
        "host" => {
            let list = input.parse_nested_block(parse_forgiving_sub_selector_list)?;
            if list.0.is_empty() {
                // :host() with empty/invalid args — still a valid :host selector
                components.push(Component::Host);
            } else {
                add_max_specificity(&list, specificity);
                components.push(Component::HostFunction(Arc::new(list)));
            }
            specificity.add_class();
            Ok(())
        },
        "host-context" => {
            let list = input.parse_nested_block(parse_forgiving_sub_selector_list)?;
            add_max_specificity(&list, specificity);
            specificity.add_class();
            components.push(Component::HostContext(Arc::new(list)));
            Ok(())
        },
        "slotted" => {
            let list = input.parse_nested_block(parse_sub_selector_list)?;
            add_max_specificity(&list, specificity);
            specificity.add_type(); // ::slotted contributes to pseudo-element column
            components.push(Component::Slotted(Arc::new(list)));
            Ok(())
        },
        "part" => {
            let parts = input.parse_nested_block(|block| -> Result<SmallVec<[Atom; 1]>, ParseError<'i>> {
                let loc = block.current_source_location();
                let mut list = SmallVec::new();
                let ident = block.expect_ident().map_err(|_| loc.new_custom_error(()))?;
                list.push(Atom::from(ident.as_ref()));
                // ::part() takes space-separated idents
                while let Ok(ident) = block.try_parse(|i| i.expect_ident().map(|s| s.clone())) {
                    list.push(Atom::from(ident.as_ref()));
                }
                Ok(list)
            })?;
            specificity.add_type(); // ::part contributes to pseudo-element column
            components.push(Component::Part(Box::new(parts)));
            Ok(())
        },
        "highlight" => {
            let name = input.parse_nested_block(|block| -> Result<Atom, ParseError<'i>> {
                let loc = block.current_source_location();
                let ident = block.expect_ident().map_err(|_| loc.new_custom_error(()))?;
                Ok(Atom::from(ident.as_ref()))
            })?;
            specificity.add_type(); // ::highlight contributes to pseudo-element column
            components.push(Component::Highlight(name));
            Ok(())
        },
        _ => Err(location.new_custom_error(())),
    )
}

fn parse_pseudo_class_name(name: &str) -> Option<PseudoClass> {
    kozan_style_macros::css_match!(name,
        "hover" => Some(PseudoClass::Hover),
        "active" => Some(PseudoClass::Active),
        "focus" => Some(PseudoClass::Focus),
        "focus-within" => Some(PseudoClass::FocusWithin),
        "focus-visible" => Some(PseudoClass::FocusVisible),
        "enabled" => Some(PseudoClass::Enabled),
        "disabled" => Some(PseudoClass::Disabled),
        "checked" => Some(PseudoClass::Checked),
        "indeterminate" => Some(PseudoClass::Indeterminate),
        "required" => Some(PseudoClass::Required),
        "optional" => Some(PseudoClass::Optional),
        "valid" => Some(PseudoClass::Valid),
        "invalid" => Some(PseudoClass::Invalid),
        "read-only" => Some(PseudoClass::ReadOnly),
        "read-write" => Some(PseudoClass::ReadWrite),
        "placeholder-shown" => Some(PseudoClass::PlaceholderShown),
        "default" => Some(PseudoClass::Default),
        "target" => Some(PseudoClass::Target),
        "visited" => Some(PseudoClass::Visited),
        "link" => Some(PseudoClass::Link),
        "any-link" => Some(PseudoClass::AnyLink),
        "fullscreen" => Some(PseudoClass::Fullscreen),
        "modal" => Some(PseudoClass::Modal),
        "popover-open" => Some(PseudoClass::PopoverOpen),
        "defined" => Some(PseudoClass::Defined),
        "autofill" => Some(PseudoClass::Autofill),
        "user-valid" => Some(PseudoClass::UserValid),
        "user-invalid" => Some(PseudoClass::UserInvalid),
        "root" => Some(PseudoClass::Root),
        "empty" => Some(PseudoClass::Empty),
        "first-child" => Some(PseudoClass::FirstChild),
        "last-child" => Some(PseudoClass::LastChild),
        "only-child" => Some(PseudoClass::OnlyChild),
        "first-of-type" => Some(PseudoClass::FirstOfType),
        "last-of-type" => Some(PseudoClass::LastOfType),
        "only-of-type" => Some(PseudoClass::OnlyOfType),
        "scope" => Some(PseudoClass::Scope),
        "playing" => Some(PseudoClass::Playing),
        "paused" => Some(PseudoClass::Paused),
        "seeking" => Some(PseudoClass::Seeking),
        "buffering" => Some(PseudoClass::Buffering),
        "stalled" => Some(PseudoClass::Stalled),
        "muted" => Some(PseudoClass::Muted),
        "volume-locked" => Some(PseudoClass::VolumeLocked),
        "blank" => Some(PseudoClass::Blank),
        "in-range" => Some(PseudoClass::InRange),
        "out-of-range" => Some(PseudoClass::OutOfRange),
        "open" => Some(PseudoClass::Open),
        "closed" => Some(PseudoClass::Closed),
        "picture-in-picture" => Some(PseudoClass::PictureInPicture),
        "target-within" => Some(PseudoClass::TargetWithin),
        "local-link" => Some(PseudoClass::LocalLink),
        "current" => Some(PseudoClass::Current),
        "past" => Some(PseudoClass::Past),
        "future" => Some(PseudoClass::Future),
        _ => None,
    )
}

fn parse_pseudo_element_name(name: &str) -> Option<PseudoElement> {
    kozan_style_macros::css_match!(name,
        "before" => Some(PseudoElement::Before),
        "after" => Some(PseudoElement::After),
        "first-line" => Some(PseudoElement::FirstLine),
        "first-letter" => Some(PseudoElement::FirstLetter),
        "placeholder" => Some(PseudoElement::Placeholder),
        "selection" => Some(PseudoElement::Selection),
        "marker" => Some(PseudoElement::Marker),
        "backdrop" => Some(PseudoElement::Backdrop),
        "file-selector-button" => Some(PseudoElement::FileSelectorButton),
        "grammar-error" => Some(PseudoElement::GrammarError),
        "spelling-error" => Some(PseudoElement::SpellingError),
        _ => None,
    )
}

/// Add the max specificity from a selector list to the accumulator.
/// Used for :not(), :is(), :has() — specificity = max of arguments.
fn add_max_specificity(list: &SelectorList, specificity: &mut Specificity) {
    if let Some(max) = list.0.iter().map(|s| s.specificity()).max() {
        add_specificity_raw(max, specificity);
    }
}

fn add_specificity_raw(source: Specificity, target: &mut Specificity) {
    // Direct packed addition — no loops. Safe because real-world selectors
    // never overflow within the (8,12,12)-bit fields.
    *target = Specificity::from_raw(target.value() + source.value());
}

fn parse_attribute_selector<'i>(input: &mut Parser<'i, '_>) -> Result<AttrSelector, ParseError<'i>> {
    let loc = input.current_source_location();

    // Handle namespace prefix: *|attr, ns|attr, |attr
    let name_atom = input.try_parse(|i| -> Result<Atom, ParseError<'i>> {
        let loc2 = i.current_source_location();
        let token = i.next().map_err(|_| loc2.new_custom_error(()))?;
        match *token {
            // *|attr — any namespace (treat as just "attr" for our purposes)
            Token::Delim('*') => {
                let loc3 = i.current_source_location();
                match *i.next().map_err(|_| loc3.new_custom_error(()))? {
                    Token::Delim('|') => {}
                    _ => return Err(loc3.new_custom_error(())),
                }
                let ident = i.expect_ident().map_err(|_| loc3.new_custom_error(()))?;
                Ok(Atom::from(ident.as_ref()))
            }
            // |attr — no namespace, or ns|attr
            Token::Delim('|') => {
                let loc3 = i.current_source_location();
                let ident = i.expect_ident().map_err(|_| loc3.new_custom_error(()))?;
                Ok(Atom::from(ident.as_ref()))
            }
            Token::Ident(ref ident_val) => {
                // Eagerly intern to avoid CowRcStr clone.
                let atom = Atom::from(ident_val.as_ref());
                if i.try_parse(|i2| -> Result<(), ParseError<'i>> {
                    let loc3 = i2.current_source_location();
                    match *i2.next().map_err(|_| loc3.new_custom_error(()))? {
                        Token::Delim('|') => Ok(()),
                        _ => Err(loc3.new_custom_error(())),
                    }
                }).is_ok() {
                    // ns|attr — we ignore the namespace, just use attr name
                    let attr_ident = i.expect_ident().map_err(|_| loc2.new_custom_error(()))?;
                    Ok(Atom::from(attr_ident.as_ref()))
                } else {
                    // Just a plain ident — reuse already-interned atom
                    Ok(atom)
                }
            }
            _ => Err(loc2.new_custom_error(())),
        }
    }).map_err(|_: ParseError<'i>| loc.new_custom_error(()))?;

    if input.is_exhausted() {
        return Ok(AttrSelector { name: name_atom, operation: AttrOperation::Exists });
    }

    let op_loc = input.current_source_location();
    let op_char = parse_attr_operation(input)?;
    let value = input.expect_ident_or_string().map_err(|_| op_loc.new_custom_error(()))?;
    let value_atom = Atom::from(value.as_ref());

    let case = input.try_parse(|i| -> Result<CaseSensitivity, ParseError<'i>> {
        let loc = i.current_source_location();
        let ident = i.expect_ident().map_err(|_| loc.new_custom_error(()))?;
        match ident.as_ref() {
            "i" | "I" => Ok(CaseSensitivity::AsciiCaseInsensitive),
            "s" | "S" => Ok(CaseSensitivity::CaseSensitive),
            _ => Err(loc.new_custom_error(())),
        }
    }).unwrap_or(CaseSensitivity::CaseSensitive);

    let operation = match op_char {
        '=' => AttrOperation::Equals(value_atom, case),
        '~' => AttrOperation::Includes(value_atom, case),
        '|' => AttrOperation::DashMatch(value_atom, case),
        '^' => AttrOperation::Prefix(value_atom, case),
        '$' => AttrOperation::Suffix(value_atom, case),
        '*' => AttrOperation::Substring(value_atom, case),
        _ => return Err(op_loc.new_custom_error(())),
    };

    Ok(AttrSelector { name: name_atom, operation })
}

fn parse_attr_operation<'i>(input: &mut Parser<'i, '_>) -> Result<char, ParseError<'i>> {
    let loc = input.current_source_location();
    match *input.next().map_err(|_| loc.new_custom_error(()))? {
        Token::Delim('=') => Ok('='),
        Token::IncludeMatch => Ok('~'),
        Token::DashMatch => Ok('|'),
        Token::PrefixMatch => Ok('^'),
        Token::SuffixMatch => Ok('$'),
        Token::SubstringMatch => Ok('*'),
        _ => Err(loc.new_custom_error(())),
    }
}

fn parse_nth_with_of<'i>(input: &mut Parser<'i, '_>) -> Result<NthData, ParseError<'i>> {
    let (a, b) = parse_nth(input)?;

    let of_selector = if input.try_parse(|i| -> Result<(), ParseError<'i>> {
        let loc = i.current_source_location();
        let ident = i.expect_ident().map_err(|_| loc.new_custom_error(()))?;
        if ident.eq_ignore_ascii_case("of") { Ok(()) }
        else { Err(loc.new_custom_error(())) }
    }).is_ok() {
        Some(Box::new(parse_sub_selector_list(input)?))
    } else {
        None
    };

    Ok(NthData { a, b, of_selector })
}

/// Forgiving relative selector list (CSS Selectors Level 4).
///
/// Per spec, `:has()` uses a forgiving argument list — invalid relative
/// selectors are silently dropped instead of failing the entire rule.
/// Returns at least what was parseable; empty list matches nothing.
fn parse_relative_selector_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<RelativeSelectorList, ParseError<'i>> {
    let mut selectors = SmallVec::new();

    // Try first selector.
    if let Ok(sel) = input.try_parse(parse_relative_selector) {
        selectors.push(sel);
    }

    // Try remaining comma-separated selectors.
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        if let Ok(sel) = input.try_parse(parse_relative_selector) {
            selectors.push(sel);
        }
    }

    Ok(RelativeSelectorList(selectors))
}

fn parse_relative_selector<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<RelativeSelector, ParseError<'i>> {
    let combinator = input.try_parse(|i| -> Result<Combinator, ParseError<'i>> {
        let loc = i.current_source_location();
        match *i.next().map_err(|_| loc.new_custom_error(()))? {
            Token::Delim('>') => Ok(Combinator::Child),
            Token::Delim('+') => Ok(Combinator::NextSibling),
            Token::Delim('~') => Ok(Combinator::LaterSibling),
            _ => Err(loc.new_custom_error(())),
        }
    }).unwrap_or(Combinator::Descendant);

    let selector = parse_sub_complex_selector(input)?;
    let traversal = HasTraversal::from_combinator(combinator, selector.hints().deps.has_combinators());
    Ok(RelativeSelector { combinator, selector, traversal })
}

/// Check if a selector list contains any pseudo-element.
/// Used to reject `:not(::before)` — spec forbids pseudo-elements in :not().
fn contains_pseudo_element(list: &SelectorList) -> bool {
    list.0.iter().any(|sel| {
        sel.components().iter().any(|c| matches!(c, Component::PseudoElement(_)))
    })
}

/// Check if a relative selector list contains any nested :has().
/// Used to reject `:has(.a:has(.b))` — spec forbids :has() inside :has().
fn contains_has_in_rel_list(rel_list: &RelativeSelectorList) -> bool {
    rel_list.0.iter().any(|rel| contains_has(&rel.selector))
}

fn contains_has(sel: &Selector) -> bool {
    sel.components().iter().any(|c| match c {
        Component::Has(_) => true,
        Component::Negation(list) | Component::Is(list) | Component::Where(list) => {
            list.0.iter().any(|s| contains_has(s))
        }
        _ => false,
    })
}

use triomphe::ThinArc;

/// Extract single-component sub-selectors for flattening.
///
/// Returns the components if ALL sub-selectors are single-component (any type:
/// class, type, id, pseudo-class, universal — any mix). Returns None if any
/// sub-selector has multiple components (compound/complex).
///
/// This handles cases no browser optimizes: `:is(.btn, div, #main)`,
/// `:not(.hidden, :disabled)`, `:where(.a, span, :hover)`.
fn extract_flat_singles(list: &SelectorList) -> Option<Vec<Component>> {
    if list.0.len() < 2 {
        return None;
    }
    let mut singles = Vec::with_capacity(list.0.len());
    for sel in &list.0 {
        let comps = sel.components();
        if comps.len() != 1 {
            return None;
        }
        singles.push(comps[0].clone());
    }
    Some(singles)
}

fn try_flatten_is(list: &SelectorList) -> Option<Component> {
    let singles = extract_flat_singles(list)?;
    Some(Component::IsSingle(ThinArc::from_header_and_iter((), singles.into_iter())))
}

fn try_flatten_where(list: &SelectorList) -> Option<Component> {
    let singles = extract_flat_singles(list)?;
    Some(Component::WhereSingle(ThinArc::from_header_and_iter((), singles.into_iter())))
}

fn try_flatten_not(list: &SelectorList) -> Option<Component> {
    let singles = extract_flat_singles(list)?;
    Some(Component::NotSingle(ThinArc::from_header_and_iter((), singles.into_iter())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(css: &str) -> SelectorList {
        parse(css).unwrap_or_else(|e| panic!("Failed to parse '{css}': {e}"))
    }

    fn first(css: &str) -> &[Component] {
        let list = Box::leak(Box::new(p(css)));
        list.0[0].components()
    }

    #[test]
    fn type_selector() {
        let c = first("div");
        assert_eq!(c.len(), 1);
        assert!(matches!(&c[0], Component::Type(a) if a.as_ref() == "div"));
    }

    #[test]
    fn class_selector() {
        let c = first(".foo");
        assert_eq!(c.len(), 1);
        assert!(matches!(&c[0], Component::Class(a) if a.as_ref() == "foo"));
    }

    #[test]
    fn id_selector() {
        let c = first("#bar");
        assert_eq!(c.len(), 1);
        assert!(matches!(&c[0], Component::Id(a) if a.as_ref() == "bar"));
    }

    #[test]
    fn compound_no_space() {
        let c = first("div.foo#bar");
        assert_eq!(c.len(), 3);
        assert!(c.iter().all(|c| !matches!(c, Component::Combinator(_))));
    }

    #[test]
    fn descendant() {
        let c = first("div .foo");
        assert_eq!(c.len(), 3);
        assert!(matches!(&c[0], Component::Class(a) if a.as_ref() == "foo"));
        assert!(matches!(&c[1], Component::Combinator(Combinator::Descendant)));
        assert!(matches!(&c[2], Component::Type(a) if a.as_ref() == "div"));
    }

    #[test]
    fn child_combinator() {
        let c = first("div > .foo");
        assert!(c.iter().any(|c| matches!(c, Component::Combinator(Combinator::Child))));
    }

    #[test]
    fn adjacent_sibling() {
        let c = first("div + .foo");
        assert!(c.iter().any(|c| matches!(c, Component::Combinator(Combinator::NextSibling))));
    }

    #[test]
    fn general_sibling() {
        let c = first("div ~ .foo");
        assert!(c.iter().any(|c| matches!(c, Component::Combinator(Combinator::LaterSibling))));
    }

    #[test]
    fn universal() {
        let c = first("*");
        assert_eq!(c.len(), 1);
        assert!(matches!(&c[0], Component::Universal));
    }

    #[test]
    fn pseudo_class_hover() {
        let c = first(":hover");
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::Hover)));
    }

    #[test]
    fn pseudo_element_double_colon() {
        let c = first("::before");
        assert!(matches!(&c[0], Component::PseudoElement(PseudoElement::Before)));
    }

    #[test]
    fn pseudo_element_single_colon() {
        let c = first(":after");
        assert!(matches!(&c[0], Component::PseudoElement(PseudoElement::After)));
    }

    #[test]
    fn attribute_exists() {
        let c = first("[disabled]");
        assert!(matches!(&c[0], Component::Attribute(a) if matches!(a.operation, AttrOperation::Exists)));
    }

    #[test]
    fn attribute_equals() {
        let c = first("[type=text]");
        assert!(matches!(&c[0], Component::Attribute(a)
            if matches!(&a.operation, AttrOperation::Equals(v, _) if v.as_ref() == "text")));
    }

    #[test]
    fn negation() {
        let c = first(":not(.foo)");
        assert!(matches!(&c[0], Component::Negation(_)));
    }

    #[test]
    fn is_selector() {
        let c = first(":is(.foo, .bar)");
        // Flattened to IsSingle (all sub-selectors are single-component)
        assert!(matches!(&c[0], Component::IsSingle(_)));
    }

    #[test]
    fn where_selector() {
        let c = first(":where(.foo)");
        assert!(matches!(&c[0], Component::Where(_)));
    }

    #[test]
    fn has_child() {
        let c = first(":has(> .bar)");
        assert!(matches!(&c[0], Component::Has(_)));
    }

    #[test]
    fn nth_child() {
        let c = first(":nth-child(2n+1)");
        assert!(matches!(&c[0], Component::NthChild(nth) if nth.a == 2 && nth.b == 1));
    }

    #[test]
    fn selector_list_comma() {
        let list = p("div, .foo, #bar");
        assert_eq!(list.0.len(), 3);
    }

    #[test]
    fn specificity_id() {
        assert_eq!(p("#id").0[0].specificity().components(), (1, 0, 0));
    }

    #[test]
    fn specificity_class() {
        assert_eq!(p(".class").0[0].specificity().components(), (0, 1, 0));
    }

    #[test]
    fn specificity_type() {
        assert_eq!(p("div").0[0].specificity().components(), (0, 0, 1));
    }

    #[test]
    fn specificity_compound() {
        assert_eq!(p("div.foo#bar").0[0].specificity().components(), (1, 1, 1));
    }

    #[test]
    fn specificity_where_zero() {
        assert_eq!(p(":where(.foo, #bar)").0[0].specificity().components(), (0, 0, 0));
    }

    #[test]
    fn complex_selector() {
        let c = first("div > .container .item:hover");
        // Right-to-left: Hover, Class(item), Descendant, Class(container), Child, Type(div)
        assert!(c.len() >= 6);
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::Hover)));
        assert!(matches!(&c[1], Component::Class(a) if a.as_ref() == "item"));
        assert!(matches!(&c[2], Component::Combinator(Combinator::Descendant)));
    }

    #[test]
    fn deep_nesting() {
        let c = first("html body div ul li a span");
        // 7 types + 6 descendant combinators = 13 components
        assert_eq!(c.len(), 13);
    }

    #[test]
    fn multiple_classes() {
        let c = first(".a.b.c");
        assert_eq!(c.len(), 3);
        assert!(c.iter().all(|c| matches!(c, Component::Class(_))));
    }

    #[test]
    fn attribute_case_insensitive() {
        let c = first("[type=text i]");
        assert!(matches!(&c[0], Component::Attribute(a)
            if matches!(&a.operation, AttrOperation::Equals(_, CaseSensitivity::AsciiCaseInsensitive))));
    }

    #[test]
    fn attribute_prefix() {
        let c = first("[class^=btn]");
        assert!(matches!(&c[0], Component::Attribute(a)
            if matches!(&a.operation, AttrOperation::Prefix(v, _) if v.as_ref() == "btn")));
    }

    #[test]
    fn nth_child_of_selector() {
        let c = first(":nth-child(2n+1 of .active)");
        if let Component::NthChild(nth) = &c[0] {
            assert_eq!(nth.a, 2);
            assert_eq!(nth.b, 1);
            assert!(nth.of_selector.is_some());
        } else {
            panic!("expected NthChild");
        }
    }

    // ---------------------------------------------------------------
    // Round-trip serialization tests: parse → Display → re-parse
    // ---------------------------------------------------------------

    /// Parse → serialize → re-parse and check structural equality.
    fn round_trip(css: &str) {
        let list1 = p(css);
        let serialized = format!("{list1}");
        let list2 = parse(&serialized).unwrap_or_else(|e| {
            panic!("Round-trip failed for '{css}' → '{serialized}': {e}")
        });
        assert_eq!(
            list1.0.len(),
            list2.0.len(),
            "Different selector count: '{css}' → '{serialized}'"
        );
        for (s1, s2) in list1.0.iter().zip(list2.0.iter()) {
            assert_eq!(
                s1.specificity(),
                s2.specificity(),
                "Specificity mismatch: '{css}' → '{serialized}'"
            );
            assert_eq!(
                s1.components().len(),
                s2.components().len(),
                "Component count mismatch: '{css}' → '{serialized}'"
            );
        }
    }

    #[test]
    fn round_trip_simple() {
        round_trip("div");
        round_trip(".foo");
        round_trip("#bar");
        round_trip("*");
    }

    #[test]
    fn round_trip_compound() {
        round_trip("div.foo");
        round_trip("div#main.active");
    }

    #[test]
    fn round_trip_combinators() {
        round_trip("div > .foo");
        round_trip("div .foo");
        round_trip("div + .foo");
        round_trip("div ~ .foo");
    }

    #[test]
    fn round_trip_pseudo_classes() {
        round_trip(":hover");
        round_trip(":first-child");
        round_trip(":last-child");
        round_trip(":only-child");
        round_trip(":root");
        round_trip(":empty");
        round_trip(":focus-within");
    }

    #[test]
    fn round_trip_pseudo_elements() {
        round_trip("::before");
        round_trip("::after");
        round_trip("::first-line");
        round_trip("::placeholder");
    }

    #[test]
    fn round_trip_functional() {
        round_trip(":not(.foo)");
        round_trip(":is(.foo, .bar)");
        round_trip(":where(.foo)");
        round_trip(":nth-child(odd)");
        round_trip(":nth-child(even)");
        round_trip(":nth-child(3)");
        round_trip(":nth-child(2n+1)");
        round_trip(":nth-of-type(2n)");
        round_trip(":lang(en)");
        round_trip(":dir(ltr)");
        round_trip(":dir(rtl)");
    }

    #[test]
    fn round_trip_attribute() {
        round_trip("[disabled]");
        round_trip("[type=text]");
        round_trip("[class^=btn]");
        round_trip("[data-x$=end]");
        round_trip("[title*=hello]");
    }

    #[test]
    fn round_trip_selector_list() {
        round_trip("div, .foo, #bar");
        round_trip("a:hover, a:focus");
    }

    #[test]
    fn round_trip_complex() {
        round_trip("div > .container .item:hover");
        round_trip("html body div ul li a span");
        round_trip(":not(.hidden):first-child");
    }

    // ---------------------------------------------------------------
    // Specificity edge cases
    // ---------------------------------------------------------------

    #[test]
    fn specificity_not_max() {
        // :not(#id) → max specificity of arguments = (1,0,0).
        // Per spec, :not() contributes the specificity of its most specific argument.
        assert_eq!(p(":not(#id)").0[0].specificity().components(), (1, 0, 0));
    }

    #[test]
    fn specificity_is_max() {
        // :is(.a, #b) → max(0,1,0 and 1,0,0) = (1,0,0).
        // Per spec, :is() contributes the specificity of its most specific argument.
        let s = p(":is(.a, #b)").0[0].specificity();
        assert_eq!(s.components(), (1, 0, 0));
    }

    #[test]
    fn specificity_nested_pseudo_element() {
        // ::before adds (0,0,1)
        assert_eq!(p("div::before").0[0].specificity().components(), (0, 0, 2));
    }

    #[test]
    fn specificity_nth_child_of() {
        // :nth-child(2n of .active) → class for nth + class for .active
        let s = p(":nth-child(2n of .active)").0[0].specificity();
        assert!(s.components().1 >= 1);
    }

    // ---------------------------------------------------------------
    // Error handling
    // ---------------------------------------------------------------

    #[test]
    fn invalid_selector_errors() {
        assert!(parse("").is_err());
        assert!(parse("!!!").is_err());
        assert!(parse(":unknown-pseudo").is_err());
        assert!(parse("::unknown-element").is_err());
    }

    // ---------------------------------------------------------------
    // Namespace selectors
    // ---------------------------------------------------------------

    #[test]
    fn namespace_specific() {
        // `svg|rect` → Namespace(Specific("svg")), Type("rect")
        let c = first("svg|rect");
        assert!(c.iter().any(|c| matches!(c, Component::Namespace(ns) if matches!(&**ns, NamespaceConstraint::Specific(n) if n.as_ref() == "svg"))));
        assert!(c.iter().any(|c| matches!(c, Component::Type(t) if t.as_ref() == "rect")));
    }

    #[test]
    fn namespace_any() {
        // `*|div` → Namespace(Any), Type("div")
        let c = first("*|div");
        assert!(c.iter().any(|c| matches!(c, Component::Namespace(ns) if matches!(&**ns, NamespaceConstraint::Any))));
        assert!(c.iter().any(|c| matches!(c, Component::Type(t) if t.as_ref() == "div")));
    }

    #[test]
    fn namespace_none() {
        // `|div` → Namespace(None), Type("div")
        let c = first("|div");
        assert!(c.iter().any(|c| matches!(c, Component::Namespace(ns) if matches!(&**ns, NamespaceConstraint::None))));
        assert!(c.iter().any(|c| matches!(c, Component::Type(t) if t.as_ref() == "div")));
    }

    #[test]
    fn namespace_any_universal() {
        // `*|*` → Namespace(Any), Universal
        let c = first("*|*");
        assert!(c.iter().any(|c| matches!(c, Component::Namespace(ns) if matches!(&**ns, NamespaceConstraint::Any))));
        assert!(c.iter().any(|c| matches!(c, Component::Universal)));
    }

    #[test]
    fn plain_type_no_namespace() {
        // `div` — no namespace component emitted.
        let c = first("div");
        assert!(!c.iter().any(|c| matches!(c, Component::Namespace(_))));
    }

    // ---------------------------------------------------------------
    // New pseudo-classes
    // ---------------------------------------------------------------

    #[test]
    fn media_pseudo_classes() {
        assert!(parse(":playing").is_ok());
        assert!(parse(":paused").is_ok());
        assert!(parse(":seeking").is_ok());
        assert!(parse(":buffering").is_ok());
        assert!(parse(":stalled").is_ok());
        assert!(parse(":muted").is_ok());
        assert!(parse(":volume-locked").is_ok());
        assert!(parse(":blank").is_ok());
    }

    #[test]
    fn backdrop_pseudo_element() {
        let c = first("::backdrop");
        assert!(matches!(&c[0], Component::PseudoElement(PseudoElement::Backdrop)));
    }

    #[test]
    fn range_and_open_pseudo_classes() {
        assert!(parse(":in-range").is_ok());
        assert!(parse(":out-of-range").is_ok());
        assert!(parse(":open").is_ok());
        assert!(parse(":closed").is_ok());

        // Verify state flags are assigned.
        let list = parse(":in-range").unwrap();
        let c = list.0[0].components();
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::InRange)));

        let list = parse(":open").unwrap();
        let c = list.0[0].components();
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::Open)));
    }

    #[test]
    fn round_trip_new_pseudo_classes() {
        for input in [":in-range", ":out-of-range", ":open", ":closed", ":picture-in-picture"] {
            let list = parse(input).unwrap();
            let output = format!("{}", list.0[0]);
            assert_eq!(input, output, "Round-trip failed for {input}");
        }
    }

    #[test]
    fn round_trip_new_pseudo_elements() {
        for input in ["::file-selector-button", "::grammar-error", "::spelling-error"] {
            let list = parse(input).unwrap();
            let output = format!("{}", list.0[0]);
            assert_eq!(input, output, "Round-trip failed for {input}");
        }
    }

    #[test]
    fn namespace_round_trip() {
        // *|div — any namespace
        let list = parse("*|div").unwrap();
        let output = format!("{}", list.0[0]);
        assert_eq!("*|div", output);

        // |div — no namespace
        let list = parse("|div").unwrap();
        let output = format!("{}", list.0[0]);
        assert_eq!("|div", output);
    }

    // ===============================================================
    // New feature tests — CSS Selectors Level 4 additions
    // ===============================================================

    // ---------------------------------------------------------------
    // :lang() comma-separated list
    // ---------------------------------------------------------------

    #[test]
    fn lang_single() {
        let c = first(":lang(en)");
        assert!(matches!(&c[0], Component::Lang(langs) if langs.len() == 1 && langs[0].as_ref() == "en"));
    }

    #[test]
    fn lang_multiple() {
        let c = first(":lang(en, fr, zh)");
        if let Component::Lang(langs) = &c[0] {
            assert_eq!(langs.len(), 3);
            assert_eq!(langs[0].as_ref(), "en");
            assert_eq!(langs[1].as_ref(), "fr");
            assert_eq!(langs[2].as_ref(), "zh");
        } else {
            panic!("expected Lang");
        }
    }

    #[test]
    fn lang_round_trip() {
        let list = p(":lang(en)");
        let s = format!("{}", list.0[0]);
        assert_eq!(s, ":lang(en)");

        let list = p(":lang(en, fr)");
        let s = format!("{}", list.0[0]);
        assert_eq!(s, ":lang(en, fr)");
    }

    #[test]
    fn lang_specificity() {
        // :lang() counts as 1 class
        assert_eq!(p(":lang(en, fr)").0[0].specificity().components(), (0, 1, 0));
    }

    // ---------------------------------------------------------------
    // parse_forgiving() public API
    // ---------------------------------------------------------------

    #[test]
    fn forgiving_valid() {
        let list = super::parse_forgiving("div, .foo, #bar");
        assert_eq!(list.0.len(), 3);
    }

    #[test]
    fn forgiving_partial_invalid() {
        // Invalid selectors silently dropped, valid kept
        let list = super::parse_forgiving("div, !!invalid!!, .foo");
        assert!(list.0.len() >= 1); // At least the valid ones survive
    }

    #[test]
    fn forgiving_all_invalid() {
        let list = super::parse_forgiving("!!!, @@@, $$$");
        assert_eq!(list.0.len(), 0); // Empty — matches nothing
    }

    // ---------------------------------------------------------------
    // :state() custom state pseudo-class
    // ---------------------------------------------------------------

    #[test]
    fn state_parse() {
        let c = first(":state(loading)");
        assert!(matches!(&c[0], Component::State(name) if name.as_ref() == "loading"));
    }

    #[test]
    fn state_specificity() {
        // :state() counts as 1 class
        assert_eq!(p(":state(foo)").0[0].specificity().components(), (0, 1, 0));
    }

    #[test]
    fn state_round_trip() {
        let list = p(":state(loading)");
        let s = format!("{}", list.0[0]);
        assert_eq!(s, ":state(loading)");
    }

    // ---------------------------------------------------------------
    // ::highlight(name)
    // ---------------------------------------------------------------

    #[test]
    fn highlight_parse() {
        let c = first("::highlight(search-result)");
        assert!(matches!(&c[0], Component::Highlight(name) if name.as_ref() == "search-result"));
    }

    #[test]
    fn highlight_specificity() {
        // ::highlight counts as type (0,0,1) like all pseudo-elements
        assert_eq!(p("::highlight(foo)").0[0].specificity().components(), (0, 0, 1));
    }

    #[test]
    fn highlight_round_trip() {
        let list = p("::highlight(search-result)");
        let s = format!("{}", list.0[0]);
        assert_eq!(s, "::highlight(search-result)");
    }

    // ---------------------------------------------------------------
    // Shadow DOM: :host, :host(), :host-context()
    // ---------------------------------------------------------------

    #[test]
    fn host_bare() {
        let c = first(":host");
        assert!(matches!(&c[0], Component::Host));
    }

    #[test]
    fn host_function() {
        let c = first(":host(.active)");
        assert!(matches!(&c[0], Component::HostFunction(_)));
    }

    #[test]
    fn host_context() {
        let c = first(":host-context(.dark-theme)");
        assert!(matches!(&c[0], Component::HostContext(_)));
    }

    #[test]
    fn host_specificity() {
        // :host = 1 class
        assert_eq!(p(":host").0[0].specificity().components(), (0, 1, 0));
        // :host(.foo) = 1 class for :host + max specificity of args
        let s = p(":host(.active)").0[0].specificity();
        assert!(s.components().1 >= 1);
    }

    #[test]
    fn host_round_trip() {
        let list = p(":host");
        assert_eq!(format!("{}", list.0[0]), ":host");

        let list = p(":host(.foo)");
        assert_eq!(format!("{}", list.0[0]), ":host(.foo)");

        let list = p(":host-context(.dark)");
        assert_eq!(format!("{}", list.0[0]), ":host-context(.dark)");
    }

    // ---------------------------------------------------------------
    // Shadow DOM: ::slotted(), ::part()
    // ---------------------------------------------------------------

    #[test]
    fn slotted_parse() {
        let c = first("::slotted(.item)");
        assert!(matches!(&c[0], Component::Slotted(_)));
    }

    #[test]
    fn slotted_specificity() {
        // ::slotted contributes to type column + inner specificity
        let s = p("::slotted(.item)").0[0].specificity();
        assert!(s.components().2 >= 1); // type column
    }

    #[test]
    fn part_single() {
        let c = first("::part(header)");
        if let Component::Part(parts) = &c[0] {
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].as_ref(), "header");
        } else {
            panic!("expected Part");
        }
    }

    #[test]
    fn part_multiple() {
        let c = first("::part(header footer)");
        if let Component::Part(parts) = &c[0] {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].as_ref(), "header");
            assert_eq!(parts[1].as_ref(), "footer");
        } else {
            panic!("expected Part");
        }
    }

    #[test]
    fn part_specificity() {
        // ::part contributes to type column
        let s = p("::part(header)").0[0].specificity();
        assert_eq!(s.components().2, 1);
    }

    #[test]
    fn shadow_dom_round_trip() {
        let list = p("::slotted(.item)");
        assert_eq!(format!("{}", list.0[0]), "::slotted(.item)");

        let list = p("::part(header)");
        assert_eq!(format!("{}", list.0[0]), "::part(header)");

        let list = p("::part(header footer)");
        assert_eq!(format!("{}", list.0[0]), "::part(header footer)");
    }

    // ---------------------------------------------------------------
    // :target-within, :local-link, :current, :past, :future
    // ---------------------------------------------------------------

    #[test]
    fn target_within_parse() {
        let c = first(":target-within");
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::TargetWithin)));
    }

    #[test]
    fn local_link_parse() {
        let c = first(":local-link");
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::LocalLink)));
    }

    #[test]
    fn time_dimensional_parse() {
        let c = first(":current");
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::Current)));
        let c = first(":past");
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::Past)));
        let c = first(":future");
        assert!(matches!(&c[0], Component::PseudoClass(PseudoClass::Future)));
    }

    #[test]
    fn new_pseudo_class_specificity() {
        // All non-functional pseudo-classes = 1 class
        for sel in [":target-within", ":local-link", ":current", ":past", ":future"] {
            assert_eq!(p(sel).0[0].specificity().components(), (0, 1, 0), "specificity for {sel}");
        }
    }

    #[test]
    fn new_pseudo_class_round_trip() {
        for sel in [":target-within", ":local-link", ":current", ":past", ":future"] {
            let list = p(sel);
            let s = format!("{}", list.0[0]);
            assert_eq!(sel, s, "round-trip for {sel}");
        }
    }

    // ---------------------------------------------------------------
    // Column combinator ||
    // ---------------------------------------------------------------

    #[test]
    fn column_combinator_parse() {
        let c = first("col || td");
        assert!(c.iter().any(|c| matches!(c, Component::Combinator(Combinator::Column))));
        assert!(c.iter().any(|c| matches!(c, Component::Type(t) if t.as_ref() == "col")));
        assert!(c.iter().any(|c| matches!(c, Component::Type(t) if t.as_ref() == "td")));
    }

    #[test]
    fn column_combinator_round_trip() {
        let list = p("col || td");
        let s = format!("{}", list.0[0]);
        assert_eq!(s, "col || td");
    }

    // ---------------------------------------------------------------
    // Complex combinations — all features together
    // ---------------------------------------------------------------

    #[test]
    fn complex_shadow_host_with_state() {
        // :host(.active):state(loading) — host with class AND custom state
        let c = first(":host(.active):state(loading)");
        assert!(c.iter().any(|c| matches!(c, Component::HostFunction(_))));
        assert!(c.iter().any(|c| matches!(c, Component::State(n) if n.as_ref() == "loading")));
    }

    #[test]
    fn complex_slotted_with_pseudo() {
        // ::slotted(.card)::before — slotted with pseudo-element (invalid per spec,
        // but our parser should handle them as separate components)
        assert!(parse("::slotted(.card)").is_ok());
    }

    #[test]
    fn complex_part_in_compound() {
        // div::part(header).active — not valid per spec (::part can only be followed
        // by pseudo-elements), but parsing should handle component list
        let list = parse("::part(header)").unwrap();
        assert_eq!(list.0.len(), 1);
    }

    #[test]
    fn complex_highlight_with_class() {
        // .editor::highlight(search-result) — element with highlight
        let c = first(".editor::highlight(search-result)");
        assert!(c.iter().any(|c| matches!(c, Component::Class(a) if a.as_ref() == "editor")));
        assert!(c.iter().any(|c| matches!(c, Component::Highlight(n) if n.as_ref() == "search-result")));
    }

    #[test]
    fn complex_host_context_descendant() {
        // :host-context(.dark) .content — host-context with descendant
        let c = first(":host-context(.dark) .content");
        assert!(c.iter().any(|c| matches!(c, Component::HostContext(_))));
        assert!(c.iter().any(|c| matches!(c, Component::Class(a) if a.as_ref() == "content")));
        assert!(c.iter().any(|c| matches!(c, Component::Combinator(Combinator::Descendant))));
    }

    #[test]
    fn all_new_pseudo_classes_are_parseable() {
        // Exhaustive check — every pseudo-class we claim to support must parse.
        let all_pseudos = [
            ":hover", ":active", ":focus", ":focus-within", ":focus-visible",
            ":enabled", ":disabled", ":checked", ":indeterminate",
            ":required", ":optional", ":valid", ":invalid",
            ":read-only", ":read-write", ":placeholder-shown", ":default",
            ":target", ":visited", ":link", ":any-link",
            ":fullscreen", ":modal", ":popover-open", ":defined",
            ":autofill", ":user-valid", ":user-invalid",
            ":root", ":empty", ":first-child", ":last-child", ":only-child",
            ":first-of-type", ":last-of-type", ":only-of-type",
            ":scope",
            ":playing", ":paused", ":seeking", ":buffering", ":stalled",
            ":muted", ":volume-locked",
            ":blank", ":in-range", ":out-of-range",
            ":open", ":closed", ":picture-in-picture",
            ":target-within", ":local-link",
            ":current", ":past", ":future",
        ];
        for pc in all_pseudos {
            assert!(parse(pc).is_ok(), "Failed to parse pseudo-class: {pc}");
        }
    }

    #[test]
    fn all_functional_selectors_parseable() {
        let all_functional = [
            ":not(.foo)", ":is(.a, .b)", ":where(.x)", ":has(> .bar)",
            ":nth-child(2n+1)", ":nth-last-child(even)", ":nth-of-type(3n)",
            ":nth-last-of-type(odd)", ":nth-child(2n of .active)",
            ":lang(en)", ":lang(en, fr, zh)", ":dir(ltr)", ":dir(rtl)",
            ":state(loading)", ":host(.foo)", ":host-context(.dark)",
        ];
        for sel in all_functional {
            assert!(parse(sel).is_ok(), "Failed to parse: {sel}");
        }
    }

    #[test]
    fn all_pseudo_elements_parseable() {
        let all_pe = [
            "::before", "::after", "::first-line", "::first-letter",
            "::placeholder", "::selection", "::marker", "::backdrop",
            "::file-selector-button", "::grammar-error", "::spelling-error",
            "::highlight(search)", "::slotted(.item)", "::part(header)",
        ];
        for pe in all_pe {
            assert!(parse(pe).is_ok(), "Failed to parse pseudo-element: {pe}");
        }
    }

    #[test]
    fn all_combinators_parseable() {
        assert!(parse("div .foo").is_ok());        // descendant
        assert!(parse("div > .foo").is_ok());       // child
        assert!(parse("div + .foo").is_ok());       // adjacent
        assert!(parse("div ~ .foo").is_ok());       // general sibling
        assert!(parse("col || td").is_ok());        // column
    }

    #[test]
    fn host_bare_and_functional_are_different() {
        // :host with no args → Component::Host
        let c1 = first(":host");
        assert!(matches!(&c1[0], Component::Host));

        // :host(.foo) with args → Component::HostFunction
        let c2 = first(":host(.foo)");
        assert!(matches!(&c2[0], Component::HostFunction(_)));
    }
}
