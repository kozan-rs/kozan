//! Stylesheet-level parsing — turns CSS text into a `Stylesheet` of typed rules.
//!
//! Implements cssparser's `AtRuleParser`, `QualifiedRuleParser`, and
//! `RuleBodyItemParser` traits for full CSS parsing including at-rules,
//! style rules, and CSS Nesting.
//!
//! At-rule keyword dispatch uses `css_match!` for integer-chunk matching.

use cssparser::{
    Parser, ParserInput, ParserState, CowRcStr,
    AtRuleParser, QualifiedRuleParser, DeclarationParser, RuleBodyItemParser,
};
use kozan_atom::Atom;
use kozan_selector::SelectorList;
use kozan_style::{DeclarationBlock, PropertyDeclaration, PropertyId, StyleSetter};
use kozan_style_macros::css_match;
use smallvec::{SmallVec, smallvec};

use crate::declaration::contains_ci;
use crate::rules::*;

// Public API

/// Parse a CSS stylesheet into a `Stylesheet`.
pub fn parse_stylesheet(css: &str) -> Stylesheet {
    parse_stylesheet_impl(css, None)
}

/// Parse a CSS stylesheet with a source URL for error reporting.
pub fn parse_stylesheet_with_url(css: &str, url: &str) -> Stylesheet {
    parse_stylesheet_impl(css, Some(Atom::new(url)))
}

fn parse_stylesheet_impl(css: &str, source_url: Option<Atom>) -> Stylesheet {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);

    // One-time check: does the entire CSS contain var()/env()/attr()?
    // This flag propagates to all style blocks, skipping expensive per-property scans.
    let b = css.as_bytes();
    let may_have_substitutions = contains_ci(b, b"var(") || contains_ci(b, b"env(") || contains_ci(b, b"attr(");
    let mut sheet_parser = SheetParser { may_have_substitutions };

    let mut rules = Vec::with_capacity(128);
    let iter = cssparser::StyleSheetParser::new(&mut parser, &mut sheet_parser);
    for result in iter {
        match result {
            Ok(rule) => rules.push(rule),
            Err(_) => {} // Skip invalid rules (error recovery)
        }
    }

    Stylesheet {
        rules: rules_from_vec(rules),
        source_url,
    }
}

// AtRulePrelude — captures parsed at-rule prelude between parse_prelude → parse_block

enum AtRulePrelude {
    Media(MediaQueryList),
    Keyframes(Atom),
    Layer(LayerPrelude),
    Supports(SupportsCondition, bool),
    Container(Option<Atom>, ContainerCondition),
    FontFace,
    Import(Atom, Option<LayerName>, Option<SupportsCondition>, MediaQueryList),
    Namespace(Option<Atom>, Atom),  // (prefix, url)
    Page(SmallVec<[Atom; 1]>),
    Property(Atom),
    CounterStyle(Atom),
    Scope(Option<SelectorList>, Option<SelectorList>),
    StartingStyle,
    /// `@charset "...";` — silently ignored per spec.
    Charset,
}

enum LayerPrelude {
    Block(Option<LayerName>),
    Statement(SmallVec<[LayerName; 2]>),
}

// SheetParser — top-level stylesheet parser

struct SheetParser {
    may_have_substitutions: bool,
}

impl<'i> AtRuleParser<'i> for SheetParser {
    type Prelude = AtRulePrelude;
    type AtRule = CssRule;
    type Error = crate::CustomError;

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let prelude = css_match! { &*name,
            "media" => parse_media_prelude(input)?,
            "keyframes" | "-webkit-keyframes" => parse_keyframes_prelude(input)?,
            "layer" => parse_layer_prelude(input)?,
            "supports" => parse_supports_prelude(input)?,
            "container" => parse_container_prelude(input)?,
            "font-face" => AtRulePrelude::FontFace,
            "import" => parse_import_prelude(input)?,
            "namespace" => parse_namespace_prelude(input)?,
            "page" => parse_page_prelude(input)?,
            "property" => parse_property_prelude(input)?,
            "counter-style" => parse_counter_style_prelude(input)?,
            "scope" => parse_scope_prelude(input)?,
            "starting-style" => AtRulePrelude::StartingStyle,
            "charset" => {
                // @charset must be silently ignored per spec.
                // Consume remainder of prelude (the encoding string).
                while input.next().is_ok() {}
                AtRulePrelude::Charset
            },
            _ => return Err(input.new_custom_error(crate::CustomError::UnknownAtRule)),
        };
        Ok(prelude)
    }

    fn rule_without_block(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
    ) -> Result<Self::AtRule, ()> {
        match prelude {
            // @layer name1, name2; (statement form)
            AtRulePrelude::Layer(LayerPrelude::Statement(names)) => {
                Ok(CssRule::Layer(Box::new(LayerRule::Statement { names })))
            }
            // @import url(...) ...;
            AtRulePrelude::Import(url, layer, supports, media) => {
                Ok(CssRule::Import(Box::new(ImportRule {
                    url,
                    layer,
                    supports,
                    media,
                })))
            }
            // @namespace [prefix] url(...);
            AtRulePrelude::Namespace(prefix, url) => {
                Ok(CssRule::Namespace(NamespaceRule { prefix, url }))
            }
            // @charset — silently ignored, produce no rule.
            AtRulePrelude::Charset => Err(()),
            _ => Err(()),
        }
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, cssparser::ParseError<'i, Self::Error>> {
        match prelude {
            AtRulePrelude::Media(queries) => {
                let rules = parse_nested_rules_with_hint(input, self.may_have_substitutions)?;
                Ok(CssRule::Media(Box::new(MediaRule { queries, rules })))
            }
            AtRulePrelude::Keyframes(name) => {
                let keyframes = parse_keyframe_blocks(input, self.may_have_substitutions)?;
                Ok(CssRule::Keyframes(Box::new(KeyframesRule {
                    name,
                    keyframes,
                })))
            }
            AtRulePrelude::Layer(LayerPrelude::Block(name)) => {
                let rules = parse_nested_rules_with_hint(input, self.may_have_substitutions)?;
                Ok(CssRule::Layer(Box::new(LayerRule::Block { name, rules })))
            }
            AtRulePrelude::Supports(condition, enabled) => {
                let rules = parse_nested_rules_with_hint(input, self.may_have_substitutions)?;
                Ok(CssRule::Supports(Box::new(SupportsRule {
                    condition,
                    enabled,
                    rules,
                })))
            }
            AtRulePrelude::Container(name, condition) => {
                let rules = parse_nested_rules_with_hint(input, self.may_have_substitutions)?;
                Ok(CssRule::Container(Box::new(ContainerRule {
                    name,
                    condition,
                    rules,
                })))
            }
            AtRulePrelude::FontFace => {
                let (declarations, descriptors) = parse_descriptor_and_declaration_block(input);
                Ok(CssRule::FontFace(Box::new(FontFaceRule { declarations, descriptors })))
            }
            AtRulePrelude::Page(selectors) => {
                let declarations = parse_declaration_block(input, self.may_have_substitutions);
                Ok(CssRule::Page(Box::new(PageRule {
                    selectors,
                    declarations,
                })))
            }
            AtRulePrelude::Property(name) => {
                let (syntax, inherits, initial_value) = parse_property_descriptors(input);
                Ok(CssRule::Property(Box::new(PropertyRule {
                    name,
                    syntax,
                    inherits,
                    initial_value,
                })))
            }
            AtRulePrelude::CounterStyle(name) => {
                let (declarations, descriptors) = parse_descriptor_and_declaration_block(input);
                Ok(CssRule::CounterStyle(Box::new(CounterStyleRule {
                    name,
                    declarations,
                    descriptors,
                })))
            }
            AtRulePrelude::Scope(start, end) => {
                let rules = parse_nested_rules_with_hint(input, self.may_have_substitutions)?;
                Ok(CssRule::Scope(Box::new(ScopeRule { start, end, rules })))
            }
            AtRulePrelude::StartingStyle => {
                let rules = parse_nested_rules_with_hint(input, self.may_have_substitutions)?;
                Ok(CssRule::StartingStyle(Box::new(StartingStyleRule { rules })))
            }
            // These are statement-only at-rules, shouldn't have blocks
            AtRulePrelude::Import(..) | AtRulePrelude::Namespace(..)
                | AtRulePrelude::Layer(LayerPrelude::Statement(..))
                | AtRulePrelude::Charset => {
                Err(input.new_custom_error(crate::CustomError::InvalidValue))
            }
        }
    }
}

impl<'i> QualifiedRuleParser<'i> for SheetParser {
    type Prelude = SelectorList;
    type QualifiedRule = CssRule;
    type Error = crate::CustomError;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        kozan_selector::parser::parse_selector_list(input)
            .map_err(|_| input.new_custom_error(crate::CustomError::InvalidSelector))
    }

    fn parse_block<'t>(
        &mut self,
        selectors: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        // Parse declarations (and nested rules for CSS Nesting).
        let (declarations, nested_rules) = parse_style_block_with_hint(input, self.may_have_substitutions);
        Ok(CssRule::Style(Box::new(StyleRule {
            selectors,
            declarations: triomphe::Arc::new(declarations),
            rules: if nested_rules.is_empty() {
                empty_rules()
            } else {
                rules_from_vec(nested_rules)
            },
        })))
    }
}

// Nested rule parsing — recursive for @media, @layer, @supports, @container

fn parse_nested_rules_with_hint<'i>(
    input: &mut Parser<'i, '_>,
    may_have_substitutions: bool,
) -> Result<RuleList, cssparser::ParseError<'i, crate::CustomError>> {
    let mut parser = SheetParser { may_have_substitutions };
    let mut rules = Vec::new();
    let iter = cssparser::RuleBodyParser::new(input, &mut parser);
    for result in iter {
        match result {
            Ok(rule) => rules.push(rule),
            Err(_) => {} // Skip invalid rules
        }
    }
    if rules.is_empty() {
        Ok(empty_rules())
    } else {
        Ok(rules_from_vec(rules))
    }
}

// For nested rule parsing, SheetParser must implement RuleBodyItemParser
impl<'i> DeclarationParser<'i> for SheetParser {
    type Declaration = CssRule;
    type Error = crate::CustomError;

    fn parse_value<'t>(
        &mut self,
        _name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        // In a rule context (not style block), declarations are not expected.
        Err(input.new_custom_error(crate::CustomError::InvalidValue))
    }
}

impl<'i> RuleBodyItemParser<'i, CssRule, crate::CustomError> for SheetParser {
    fn parse_declarations(&self) -> bool { false }
    fn parse_qualified(&self) -> bool { true }
}

// StyleBlockParser — CSS Nesting: declarations + nested rules in a style block

struct StyleBlockParser {
    may_have_substitutions: bool,
}

// Inline up to 4 declarations.
type DeclVec = SmallVec<[(PropertyDeclaration, bool); 4]>;

enum StyleBlockItem {
    Declaration(DeclVec),
    Rule(CssRule),
}

impl<'i> DeclarationParser<'i> for StyleBlockParser {
    type Declaration = StyleBlockItem;
    type Error = crate::CustomError;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        let id = name.parse::<PropertyId>().ok()
            .or_else(|| if name.starts_with("--") { Some(PropertyId::Custom) } else { None })
            .ok_or_else(|| input.new_custom_error(crate::CustomError::UnknownProperty))?;

        if id == PropertyId::Custom {
            let start = input.position();
            while input.next().is_ok() {}
            let raw = input.slice_from(start).trim();
            return Ok(StyleBlockItem::Declaration(smallvec![(
                PropertyDeclaration::Custom {
                    name: kozan_style::Atom::new(&name),
                    value: kozan_style::Atom::new(raw),
                },
                false,
            )]));
        }

        // CSS-wide keywords — peek first token to avoid try_parse overhead on miss.
        {
            let state = input.state();
            if let Ok(token) = input.next() {
                if let cssparser::Token::Ident(ref ident) = *token {
                    if let Some(kw) = crate::declaration::match_css_wide_keyword(ident) {
                        let decls = crate::declaration::apply_keyword_to_longhands(id, &kw);
                        let important = input.try_parse(cssparser::parse_important).is_ok();
                        return Ok(StyleBlockItem::Declaration(
                            decls.into_iter().map(|d| (d, important)).collect(),
                        ));
                    }
                }
                input.reset(&state);
            }
        }

        if self.may_have_substitutions {
            if let Some(unparsed) = crate::var::scan_for_substitutions(input) {
                let important = input.try_parse(cssparser::parse_important).is_ok();
                return Ok(StyleBlockItem::Declaration(
                    crate::declaration::make_unparsed(id, unparsed, &name, important),
                ));
            }
        }

        // Shorthand value parsing (generated same-type + hand-written mixed-type).
        if let Some(result) = crate::shorthand::parse_shorthand(id, input) {
            let decls = result?;
            let important = input.try_parse(cssparser::parse_important).is_ok();
            return Ok(StyleBlockItem::Declaration(
                decls.into_iter().map(|d| (d, important)).collect(),
            ));
        }

        // Typed parse via generated dispatch (longhands only).
        let decl = crate::properties::parse_property_value(id, input)?;
        let important = input.try_parse(cssparser::parse_important).is_ok();
        Ok(StyleBlockItem::Declaration(smallvec![(decl, important)]))
    }
}

impl<'i> AtRuleParser<'i> for StyleBlockParser {
    type Prelude = AtRulePrelude;
    type AtRule = StyleBlockItem;
    type Error = crate::CustomError;

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        // Reuse SheetParser's at-rule prelude parsing (UFCS to disambiguate).
        AtRuleParser::parse_prelude(&mut SheetParser { may_have_substitutions: self.may_have_substitutions }, name, input)
    }

    fn rule_without_block(
        &mut self,
        prelude: Self::Prelude,
        start: &ParserState,
    ) -> Result<Self::AtRule, ()> {
        AtRuleParser::rule_without_block(&mut SheetParser { may_have_substitutions: self.may_have_substitutions }, prelude, start)
            .map(StyleBlockItem::Rule)
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, cssparser::ParseError<'i, Self::Error>> {
        AtRuleParser::parse_block(&mut SheetParser { may_have_substitutions: self.may_have_substitutions }, prelude, start, input)
            .map(StyleBlockItem::Rule)
    }
}

impl<'i> QualifiedRuleParser<'i> for StyleBlockParser {
    type Prelude = SelectorList;
    type QualifiedRule = StyleBlockItem;
    type Error = crate::CustomError;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let mut selectors = kozan_selector::parser::parse_selector_list(input)
            .map_err(|_| input.new_custom_error(crate::CustomError::InvalidSelector))?;
        // CSS Nesting Level 1: selectors inside a style block that don't
        // contain `&` get an implicit `& ` prepended (descendant combinator).
        kozan_selector::ensure_nesting(&mut selectors);
        Ok(selectors)
    }

    fn parse_block<'t>(
        &mut self,
        selectors: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let (declarations, nested_rules) = parse_style_block_with_hint(input, self.may_have_substitutions);
        Ok(StyleBlockItem::Rule(CssRule::Style(Box::new(StyleRule {
            selectors,
            declarations: triomphe::Arc::new(declarations),
            rules: if nested_rules.is_empty() {
                empty_rules()
            } else {
                rules_from_vec(nested_rules)
            },
        }))))
    }
}

impl<'i> RuleBodyItemParser<'i, StyleBlockItem, crate::CustomError> for StyleBlockParser {
    fn parse_declarations(&self) -> bool { true }
    fn parse_qualified(&self) -> bool { true }
}

/// Parse a style block: declarations + nested rules (CSS Nesting).
fn parse_style_block_with_hint(input: &mut Parser<'_, '_>, may_have_substitutions: bool) -> (DeclarationBlock, Vec<CssRule>) {
    let mut block_parser = StyleBlockParser {
        may_have_substitutions,
    };

    let mut block = DeclarationBlock::new();
    let mut nested_rules = Vec::new();

    let iter = cssparser::RuleBodyParser::new(input, &mut block_parser);
    for result in iter {
        match result {
            Ok(StyleBlockItem::Declaration(decls)) => {
                for (decl, important) in decls {
                    if important { block.important(); } else { block.normal(); }
                    block.on_set(decl);
                }
            }
            Ok(StyleBlockItem::Rule(rule)) => {
                nested_rules.push(rule);
            }
            Err(_) => {} // Skip invalid items
        }
    }

    (block, nested_rules)
}

/// Parse a declaration block (for @font-face, @page, keyframes — no nested rules).
fn parse_declaration_block(input: &mut Parser<'_, '_>, may_have_substitutions: bool) -> DeclarationBlock {
    let mut decl_parser = crate::declaration::DeclParser {
        may_have_substitutions,
    };
    let mut block = DeclarationBlock::new();
    let iter = cssparser::RuleBodyParser::new(input, &mut decl_parser);
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

/// Parse a descriptor block for at-rules like @font-face and @counter-style.
///
/// Returns both a `DeclarationBlock` (for descriptors that happen to be valid
/// CSS properties like `font-family`, `font-style`, `font-weight`) and a
/// `Vec<(Atom, Atom)>` of raw descriptor key-value pairs for descriptors that
/// are NOT CSS properties (e.g. `src`, `unicode-range`, `font-display`,
/// `system`, `symbols`, `suffix`, etc.).
fn parse_descriptor_and_declaration_block(
    input: &mut Parser<'_, '_>,
) -> (DeclarationBlock, Vec<(Atom, Atom)>) {
    let mut block = DeclarationBlock::new();
    let mut descriptors: Vec<(Atom, Atom)> = Vec::new();
    let mut desc_parser = DescriptorParser;
    let iter = cssparser::RuleBodyParser::new(input, &mut desc_parser);
    for result in iter {
        match result {
            Ok(DescriptorItem::Declarations(decls)) => {
                for (decl, important) in decls {
                    if important { block.important(); } else { block.normal(); }
                    block.on_set(decl);
                }
            }
            Ok(DescriptorItem::RawDescriptor(name, value)) => {
                descriptors.push((name, value));
            }
            Err(_) => {} // Skip invalid items
        }
    }
    (block, descriptors)
}

/// Items produced by the descriptor block parser.
enum DescriptorItem {
    /// A known CSS property declaration.
    Declarations(SmallVec<[(PropertyDeclaration, bool); 4]>),
    /// An unknown descriptor stored as raw name → value text.
    RawDescriptor(Atom, Atom),
}

/// Parser for @font-face / @counter-style blocks: tries CSS property dispatch
/// first, falls back to raw descriptor capture for unknown names.
struct DescriptorParser;

impl<'i> DeclarationParser<'i> for DescriptorParser {
    type Declaration = DescriptorItem;
    type Error = crate::CustomError;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<Self::Declaration, cssparser::ParseError<'i, Self::Error>> {
        // Try parsing as a known CSS property first.
        let id = name.parse::<PropertyId>().ok();
        if let Some(id) = id {
            match crate::properties::parse_property_value(id, input) {
                Ok(decl) => {
                    let important = input.try_parse(cssparser::parse_important).is_ok();
                    return Ok(DescriptorItem::Declarations(smallvec![(decl, important)]));
                }
                Err(_) => {
                    // Fall through to raw descriptor capture.
                }
            }
        }

        // Not a known CSS property (or parse failed) — capture as raw descriptor.
        let start_pos = input.position();
        while input.next().is_ok() {}
        let raw_value = input.slice_from(start_pos).trim();
        // Strip trailing !important from raw value if present.
        let raw_value = raw_value.strip_suffix("!important")
            .map(|s| s.trim())
            .unwrap_or(raw_value);
        Ok(DescriptorItem::RawDescriptor(
            Atom::new(&*name),
            Atom::new(raw_value),
        ))
    }
}

impl<'i> AtRuleParser<'i> for DescriptorParser {
    type Prelude = ();
    type AtRule = DescriptorItem;
    type Error = crate::CustomError;
}

impl<'i> QualifiedRuleParser<'i> for DescriptorParser {
    type Prelude = ();
    type QualifiedRule = DescriptorItem;
    type Error = crate::CustomError;
}

impl<'i> RuleBodyItemParser<'i, DescriptorItem, crate::CustomError> for DescriptorParser {
    fn parse_declarations(&self) -> bool { true }
    fn parse_qualified(&self) -> bool { false }
}

// At-rule prelude parsers

fn parse_media_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    let queries = parse_media_query_list(input)?;
    Ok(AtRulePrelude::Media(queries))
}

fn parse_keyframes_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // @keyframes name — name can be ident or string.
    let name = match input.next()?.clone() {
        cssparser::Token::Ident(s) => Atom::new(&*s),
        cssparser::Token::QuotedString(s) => Atom::new(&*s),
        t => return Err(input.new_unexpected_token_error(t)),
    };
    Ok(AtRulePrelude::Keyframes(name))
}

fn parse_layer_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // @layer can be:
    // - @layer name { ... }          (block with name)
    // - @layer { ... }               (anonymous block)
    // - @layer name1, name2;         (statement)

    if input.is_exhausted() {
        return Ok(AtRulePrelude::Layer(LayerPrelude::Block(None)));
    }

    let first_name = parse_layer_name(input)?;

    if input.try_parse(|i| i.expect_comma()).is_ok() {
        // Comma → statement form with multiple names
        let mut names = SmallVec::new();
        names.push(first_name);
        names.push(parse_layer_name(input)?);
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            names.push(parse_layer_name(input)?);
        }
        Ok(AtRulePrelude::Layer(LayerPrelude::Statement(names)))
    } else {
        // Single name → block form
        Ok(AtRulePrelude::Layer(LayerPrelude::Block(Some(first_name))))
    }
}

fn parse_layer_name<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<LayerName, cssparser::ParseError<'i, crate::CustomError>> {
    let mut parts = SmallVec::new();
    parts.push(Atom::new(&*input.expect_ident()?));
    while input.try_parse(|i| i.expect_delim('.')).is_ok() {
        parts.push(Atom::new(&*input.expect_ident()?));
    }
    Ok(LayerName(parts))
}

fn parse_supports_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    let condition = parse_supports_condition(input)?;
    // Evaluate at parse time
    let enabled = eval_supports_condition(&condition);
    Ok(AtRulePrelude::Supports(condition, enabled))
}

fn parse_container_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // Optional container name (ident before the condition).
    let name = input.try_parse(|i| -> Result<Atom, cssparser::ParseError<'i, crate::CustomError>> {
        let ident = i.expect_ident()?.clone();
        // Container name must not be a CSS-wide keyword or 'not'/'and'/'or'.
        let b = ident.as_bytes();
        if b.len() == 3 && (b[0] | 0x20) == b'n' && (b[1] | 0x20) == b'o' && (b[2] | 0x20) == b't' {
            return Err(i.new_custom_error(crate::CustomError::InvalidValue));
        }
        if b.len() == 3 && (b[0] | 0x20) == b'a' && (b[1] | 0x20) == b'n' && (b[2] | 0x20) == b'd' {
            return Err(i.new_custom_error(crate::CustomError::InvalidValue));
        }
        if b.len() == 2 && (b[0] | 0x20) == b'o' && (b[1] | 0x20) == b'r' {
            return Err(i.new_custom_error(crate::CustomError::InvalidValue));
        }
        Ok(Atom::new(&*ident))
    }).ok();

    let condition = parse_container_condition(input)?;
    Ok(AtRulePrelude::Container(name, condition))
}

fn parse_import_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // @import url("...") or @import "..."
    let url = parse_url_or_string(input)?;

    // Optional layer
    let layer = input.try_parse(|i| -> Result<Option<LayerName>, cssparser::ParseError<'i, crate::CustomError>> {
        let ident = i.expect_ident()?.clone();
        if ident.eq_ignore_ascii_case("layer") {
            // @import url("...") layer or @import url("...") layer(name)
            if i.try_parse(|i2| i2.expect_function_matching("layer")).is_ok() {
                let name = i.parse_nested_block(|i2| parse_layer_name(i2))?;
                Ok(Some(name))
            } else {
                Ok(None) // anonymous layer
            }
        } else {
            Err(i.new_custom_error(crate::CustomError::InvalidValue))
        }
    }).ok().flatten();

    // Optional supports()
    let supports = input.try_parse(|i| -> Result<SupportsCondition, cssparser::ParseError<'i, crate::CustomError>> {
        i.expect_function_matching("supports")?;
        i.parse_nested_block(|i2| parse_supports_condition(i2))
    }).ok();

    // Optional trailing media query list
    let media = if !input.is_exhausted() {
        parse_media_query_list(input)?
    } else {
        MediaQueryList::empty()
    };

    Ok(AtRulePrelude::Import(url, layer, supports, media))
}

fn parse_namespace_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // @namespace [prefix] url(...)
    let prefix = input.try_parse(|i| {
        Ok::<_, cssparser::ParseError<'i, crate::CustomError>>(Atom::new(&*i.expect_ident()?))
    }).ok();

    let url = parse_url_or_string(input)?;
    Ok(AtRulePrelude::Namespace(prefix, url))
}

fn parse_page_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    let mut selectors = SmallVec::new();
    while !input.is_exhausted() {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            selectors.push(Atom::new(&*ident));
        } else if input.try_parse(|i| i.expect_colon()).is_ok() {
            let pseudo = input.expect_ident()?;
            selectors.push(Atom::new(&*pseudo));
        } else {
            break;
        }
        let _ = input.try_parse(|i| i.expect_comma());
    }
    Ok(AtRulePrelude::Page(selectors))
}

// URL parsing helper

fn parse_url_or_string<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<Atom, cssparser::ParseError<'i, crate::CustomError>> {
    if let Ok(url) = input.try_parse(|i| i.expect_url_or_string()) {
        Ok(Atom::new(&*url))
    } else {
        Err(input.new_custom_error(crate::CustomError::InvalidValue))
    }
}

// Media query parsing

fn parse_media_query_list<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaQueryList, cssparser::ParseError<'i, crate::CustomError>> {
    let mut queries = SmallVec::new();

    if input.is_exhausted() {
        // Empty → matches all
        return Ok(MediaQueryList(queries));
    }

    queries.push(parse_media_query(input)?);
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        queries.push(parse_media_query(input)?);
    }
    Ok(MediaQueryList(queries))
}

fn parse_media_query<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaQuery, cssparser::ParseError<'i, crate::CustomError>> {
    // Check for qualifier: not/only
    let qualifier = input.try_parse(|i| {
        let ident = i.expect_ident()?;
        Ok::<_, cssparser::ParseError<'i, crate::CustomError>>(css_match! { &**ident,
            "not" => MediaQualifier::Not,
            "only" => MediaQualifier::Only,
            _ => return Err(i.new_custom_error(crate::CustomError::InvalidValue)),
        })
    }).ok();

    // Check for media type
    let media_type = if qualifier.is_some() {
        // After not/only, media type is required
        parse_media_type(input)?
    } else {
        input.try_parse(parse_media_type).unwrap_or(MediaType::All)
    };

    // Check for 'and (condition)'
    let condition = if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        Some(parse_media_condition_without_or(input)?)
    } else if qualifier.is_none() && matches!(media_type, MediaType::All) {
        // No type → condition-only: `(min-width: 768px)`
        input.try_parse(|i| parse_media_condition(i)).ok()
    } else {
        None
    };

    Ok(MediaQuery {
        qualifier,
        media_type,
        condition,
    })
}

fn parse_media_type<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaType, cssparser::ParseError<'i, crate::CustomError>> {
    let ident = input.expect_ident()?;
    Ok(css_match! { &**ident,
        "all" => MediaType::All,
        "screen" => MediaType::Screen,
        "print" => MediaType::Print,
        _ => MediaType::Custom(Atom::new(&*ident)),
    })
}

fn parse_media_condition<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaCondition, cssparser::ParseError<'i, crate::CustomError>> {
    // Check for 'not'
    if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
        let inner = parse_media_in_parens(input)?;
        return Ok(MediaCondition::Not(Box::new(inner)));
    }

    let first = parse_media_in_parens(input)?;

    // Check for 'and' or 'or' chains
    if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        let mut conditions: SmallVec<[Box<MediaCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_media_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
            conditions.push(Box::new(parse_media_in_parens(input)?));
        }
        Ok(MediaCondition::And(conditions))
    } else if input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
        let mut conditions: SmallVec<[Box<MediaCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_media_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
            conditions.push(Box::new(parse_media_in_parens(input)?));
        }
        Ok(MediaCondition::Or(conditions))
    } else {
        Ok(first)
    }
}

fn parse_media_condition_without_or<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaCondition, cssparser::ParseError<'i, crate::CustomError>> {
    // After 'type and', only 'and' chains are allowed (no 'or').
    let first = parse_media_in_parens(input)?;

    if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        let mut conditions: SmallVec<[Box<MediaCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_media_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
            conditions.push(Box::new(parse_media_in_parens(input)?));
        }
        Ok(MediaCondition::And(conditions))
    } else {
        Ok(first)
    }
}

fn parse_media_in_parens<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaCondition, cssparser::ParseError<'i, crate::CustomError>> {
    input.expect_parenthesis_block()?;
    input.parse_nested_block(|i| {
        // Try nested condition first (for `not`, `and`, `or` inside parens)
        if let Ok(condition) = i.try_parse(parse_media_condition) {
            return Ok(condition);
        }
        // Otherwise it's a media feature
        let feature = parse_media_feature(i)?;
        Ok(MediaCondition::Feature(feature))
    })
}

fn parse_media_feature<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaFeature, cssparser::ParseError<'i, crate::CustomError>> {
    let name = Atom::new(&*input.expect_ident()?);

    // Boolean feature: (color), (hover)
    if input.is_exhausted() {
        return Ok(MediaFeature::Boolean(name));
    }

    // Check for range operators or colon
    if input.try_parse(|i| i.expect_colon()).is_ok() {
        // Plain feature: (name: value)
        let value = parse_media_feature_value(input)?;
        return Ok(MediaFeature::Plain { name, value });
    }

    // Range syntax: (name op value)
    let op = parse_range_op(input)?;
    let value = parse_media_feature_value(input)?;
    Ok(MediaFeature::Range { name, op, value })
}

fn parse_range_op<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<RangeOp, cssparser::ParseError<'i, crate::CustomError>> {
    let t = input.next()?.clone();
    match t {
        cssparser::Token::Delim('>') => {
            if input.try_parse(|i| i.expect_delim('=')).is_ok() {
                Ok(RangeOp::Ge)
            } else {
                Ok(RangeOp::Gt)
            }
        }
        cssparser::Token::Delim('<') => {
            if input.try_parse(|i| i.expect_delim('=')).is_ok() {
                Ok(RangeOp::Le)
            } else {
                Ok(RangeOp::Lt)
            }
        }
        cssparser::Token::Delim('=') => Ok(RangeOp::Eq),
        t => Err(input.new_unexpected_token_error(t)),
    }
}

fn parse_media_feature_value<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<MediaFeatureValue, cssparser::ParseError<'i, crate::CustomError>> {
    // Try number/dimension first
    if let Ok(value) = input.try_parse(|i| {
        let token = i.next()?.clone();
        match token {
            cssparser::Token::Number { value, int_value, .. } => {
                // Check for ratio: number / number
                if i.try_parse(|i2| i2.expect_delim('/')).is_ok() {
                    let denom = i.expect_integer()? as u32;
                    Ok(MediaFeatureValue::Ratio(value as u32, denom))
                } else if let Some(int) = int_value {
                    Ok(MediaFeatureValue::Integer(int))
                } else {
                    Ok(MediaFeatureValue::Number(value))
                }
            }
            cssparser::Token::Dimension { value, unit, .. } => {
                let unit = parse_length_unit(&unit)?;
                Ok(MediaFeatureValue::Length(value, unit))
            }
            _ => Err(i.new_custom_error(crate::CustomError::InvalidValue)),
        }
    }) {
        return Ok(value);
    }

    // Try ident
    let ident = input.expect_ident()?;
    Ok(MediaFeatureValue::Ident(Atom::new(&*ident)))
}

fn parse_length_unit<'i>(
    unit: &str,
) -> Result<LengthUnit, cssparser::ParseError<'i, crate::CustomError>> {
    use kozan_style_macros::css_match;
    Ok(css_match! { unit,
        "px" => LengthUnit::Px,
        "cm" => LengthUnit::Cm,
        "mm" => LengthUnit::Mm,
        "in" => LengthUnit::In,
        "pt" => LengthUnit::Pt,
        "pc" => LengthUnit::Pc,
        "em" => LengthUnit::Em,
        "rem" => LengthUnit::Rem,
        "ch" => LengthUnit::Ch,
        "ex" => LengthUnit::Ex,
        "vw" => LengthUnit::Vw,
        "vh" => LengthUnit::Vh,
        "vmin" => LengthUnit::Vmin,
        "vmax" => LengthUnit::Vmax,
        "vi" => LengthUnit::Vi,
        "vb" => LengthUnit::Vb,
        "svw" => LengthUnit::Svw,
        "svh" => LengthUnit::Svh,
        "lvw" => LengthUnit::Lvw,
        "lvh" => LengthUnit::Lvh,
        "dvw" => LengthUnit::Dvw,
        "dvh" => LengthUnit::Dvh,
        "cqw" => LengthUnit::Cqw,
        "cqh" => LengthUnit::Cqh,
        "cqi" => LengthUnit::Cqi,
        "cqb" => LengthUnit::Cqb,
        _ => return Err(cssparser::ParseError {
            kind: cssparser::ParseErrorKind::Custom(crate::CustomError::InvalidValue),
            location: cssparser::SourceLocation { line: 0, column: 0 },
        }),
    })
}

// @keyframes block parsing

fn parse_keyframe_blocks<'i>(
    input: &mut Parser<'i, '_>,
    may_have_substitutions: bool,
) -> Result<Box<[KeyframeBlock]>, cssparser::ParseError<'i, crate::CustomError>> {
    let mut blocks = Vec::new();

    while !input.is_exhausted() {
        if let Ok(block) = input.try_parse(|i| parse_one_keyframe_block(i, may_have_substitutions)) {
            blocks.push(block);
        }
    }

    Ok(blocks.into_boxed_slice())
}

fn parse_one_keyframe_block<'i>(
    input: &mut Parser<'i, '_>,
    may_have_substitutions: bool,
) -> Result<KeyframeBlock, cssparser::ParseError<'i, crate::CustomError>> {
    // Parse keyframe selectors: from, to, or percentages
    let mut selectors = SmallVec::new();
    selectors.push(parse_keyframe_selector(input)?);
    while input.try_parse(|i| i.expect_comma()).is_ok() {
        selectors.push(parse_keyframe_selector(input)?);
    }

    // Parse the declaration block
    input.expect_curly_bracket_block()?;
    let declarations = input.parse_nested_block(|i| {
        Ok::<_, cssparser::ParseError<'i, crate::CustomError>>(parse_declaration_block(i, may_have_substitutions))
    })?;

    Ok(KeyframeBlock {
        selectors,
        declarations,
    })
}

fn parse_keyframe_selector<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<KeyframeSelector, cssparser::ParseError<'i, crate::CustomError>> {
    if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
        return Ok(css_match! { &*ident,
            "from" => KeyframeSelector::From,
            "to" => KeyframeSelector::To,
            _ => return Err(input.new_custom_error(crate::CustomError::InvalidValue)),
        });
    }
    // Percentage
    let pct = input.expect_percentage()?;
    Ok(KeyframeSelector::Percentage(pct))
}

// @supports condition parsing

fn parse_supports_condition<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<SupportsCondition, cssparser::ParseError<'i, crate::CustomError>> {
    if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
        let inner = parse_supports_in_parens(input)?;
        return Ok(SupportsCondition::Not(Box::new(inner)));
    }

    let first = parse_supports_in_parens(input)?;

    if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        let mut conditions: SmallVec<[Box<SupportsCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_supports_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
            conditions.push(Box::new(parse_supports_in_parens(input)?));
        }
        Ok(SupportsCondition::And(conditions))
    } else if input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
        let mut conditions: SmallVec<[Box<SupportsCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_supports_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
            conditions.push(Box::new(parse_supports_in_parens(input)?));
        }
        Ok(SupportsCondition::Or(conditions))
    } else {
        Ok(first)
    }
}

fn parse_supports_in_parens<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<SupportsCondition, cssparser::ParseError<'i, crate::CustomError>> {
    // Check for selector() function
    if input.try_parse(|i| i.expect_function_matching("selector")).is_ok() {
        return input.parse_nested_block(|i| {
            let start = i.position();
            while i.next().is_ok() {}
            let raw = i.slice_from(start).trim();
            Ok(SupportsCondition::Selector(raw.into()))
        });
    }

    input.expect_parenthesis_block()?;
    input.parse_nested_block(|i| {
        // Try nested condition
        if let Ok(cond) = i.try_parse(parse_supports_condition) {
            return Ok(cond);
        }

        // Otherwise it's a declaration test: (property: value)
        let start = i.position();
        while i.next().is_ok() {}
        let raw = i.slice_from(start).trim();
        Ok(SupportsCondition::Declaration(raw.into()))
    })
}

/// Evaluate @supports condition at parse time.
fn eval_supports_condition(condition: &SupportsCondition) -> bool {
    match condition {
        SupportsCondition::Declaration(raw) => {
            // Try parsing the declaration — if it succeeds, the property is supported.
            if let Some(colon_pos) = raw.find(':') {
                let prop_name = raw[..colon_pos].trim();
                let value = raw[colon_pos + 1..].trim();
                if let Ok(id) = prop_name.parse::<PropertyId>() {
                    let mut input = ParserInput::new(value);
                    let mut parser = Parser::new(&mut input);
                    return crate::properties::parse_property_value(id, &mut parser).is_ok();
                }
            }
            false
        }
        SupportsCondition::Not(inner) => !eval_supports_condition(inner),
        SupportsCondition::And(conditions) => {
            conditions.iter().all(|c| eval_supports_condition(c))
        }
        SupportsCondition::Or(conditions) => {
            conditions.iter().any(|c| eval_supports_condition(c))
        }
        SupportsCondition::Selector(raw) => {
            // Try parsing the selector
            kozan_selector::parser::parse(&raw).is_ok()
        }
    }
}

// @container condition parsing

fn parse_container_condition<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<ContainerCondition, cssparser::ParseError<'i, crate::CustomError>> {
    if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
        let inner = parse_container_in_parens(input)?;
        return Ok(ContainerCondition::Not(Box::new(inner)));
    }

    let first = parse_container_in_parens(input)?;

    if input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
        let mut conditions: SmallVec<[Box<ContainerCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_container_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("and")).is_ok() {
            conditions.push(Box::new(parse_container_in_parens(input)?));
        }
        Ok(ContainerCondition::And(conditions))
    } else if input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
        let mut conditions: SmallVec<[Box<ContainerCondition>; 2]> = SmallVec::new();
        conditions.push(Box::new(first));
        conditions.push(Box::new(parse_container_in_parens(input)?));
        while input.try_parse(|i| i.expect_ident_matching("or")).is_ok() {
            conditions.push(Box::new(parse_container_in_parens(input)?));
        }
        Ok(ContainerCondition::Or(conditions))
    } else {
        Ok(first)
    }
}

fn parse_container_in_parens<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<ContainerCondition, cssparser::ParseError<'i, crate::CustomError>> {
    input.expect_parenthesis_block()?;
    input.parse_nested_block(|i| {
        // Try nested condition
        if let Ok(cond) = i.try_parse(parse_container_condition) {
            return Ok(cond);
        }

        // Container size feature: (width >= 768px)
        let name = Atom::new(&*i.expect_ident()?);

        // Check for colon (plain syntax) or range op
        if i.try_parse(|i2| i2.expect_colon()).is_ok() {
            let value = parse_media_feature_value(i)?;
            Ok(ContainerCondition::Feature(ContainerSizeFeature {
                name,
                op: RangeOp::Eq,
                value,
            }))
        } else {
            let op = parse_range_op(i)?;
            let value = parse_media_feature_value(i)?;
            Ok(ContainerCondition::Feature(ContainerSizeFeature {
                name,
                op,
                value,
            }))
        }
    })
}

// @property prelude + descriptors

fn parse_property_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // @property --name
    let name_token = input.expect_ident()?.clone();
    if !name_token.starts_with("--") {
        return Err(input.new_custom_error(crate::CustomError::InvalidCustomPropertyName));
    }
    Ok(AtRulePrelude::Property(Atom::new(&*name_token)))
}

/// Parse @property descriptors: syntax, inherits, initial-value.
/// Uses manual token parsing since these aren't standard CSS properties.
fn parse_property_descriptors(input: &mut Parser<'_, '_>) -> (PropertySyntax, bool, Option<Atom>) {
    let mut syntax = PropertySyntax::Universal;
    let mut inherits = false;
    let mut initial_value = None;

    // Parse descriptor declarations manually.
    // cssparser's parse_until_after handles semicolon-delimited declarations correctly.
    loop {
        if input.is_exhausted() { break; }

        let result = input.try_parse(|i| -> Result<(), cssparser::ParseError<'_, crate::CustomError>> {
            let name = i.expect_ident()?.clone();
            i.expect_colon()?;

            let b = name.as_bytes();
            // "syntax"
            if b.len() == 6 && (b[0] | 0x20) == b's' && (b[5] | 0x20) == b'x' {
                let s = i.expect_string()?.clone();
                syntax = if &*s == "*" {
                    PropertySyntax::Universal
                } else {
                    PropertySyntax::Typed(Atom::new(&*s))
                };
            }
            // "inherits"
            else if b.len() == 8 && (b[0] | 0x20) == b'i' && (b[7] | 0x20) == b's' {
                let val = i.expect_ident()?.clone();
                inherits = val.eq_ignore_ascii_case("true");
            }
            // "initial-value" — collect everything up to the semicolon
            else if b.len() == 13 && (b[0] | 0x20) == b'i' && (b[8] | 0x20) == b'v' {
                let start = i.position();
                // Consume tokens but stop before semicolon — use parse_until_before
                // We can't use parse_until_before here since we're inside try_parse,
                // so manually peek for semicolons.
                loop {
                    let _state = i.state();
                    match i.next() {
                        Ok(t) => {
                            if matches!(t, cssparser::Token::Semicolon) {
                                // We consumed the semicolon — get slice before it
                                let raw = i.slice_from(start);
                                // Trim the trailing semicolon and whitespace
                                let raw = raw.trim_end().trim_end_matches(';').trim();
                                if !raw.is_empty() {
                                    initial_value = Some(Atom::new(raw));
                                }
                                return Ok(());
                            }
                        }
                        Err(_) => {
                            // End of input — no semicolon
                            let raw = i.slice_from(start).trim();
                            if !raw.is_empty() {
                                initial_value = Some(Atom::new(raw));
                            }
                            return Ok(());
                        }
                    }
                }
            }

            i.expect_semicolon().or_else(|_| {
                if i.is_exhausted() { Ok(()) } else { Err(i.new_custom_error(crate::CustomError::InvalidValue)) }
            })?;
            Ok(())
        });

        // If try_parse failed, skip one token to avoid infinite loop
        if result.is_err() && !input.is_exhausted() {
            let _ = input.next();
        }
    }

    (syntax, inherits, initial_value)
}

// @counter-style prelude

fn parse_counter_style_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    let name = input.expect_ident()?.clone();
    Ok(AtRulePrelude::CounterStyle(Atom::new(&*name)))
}

// @scope prelude

fn parse_scope_prelude<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<AtRulePrelude, cssparser::ParseError<'i, crate::CustomError>> {
    // @scope [(start-selector)]? [to (end-selector)]?
    let start = if !input.is_exhausted() && !input.try_parse(|i| i.expect_ident_matching("to")).is_ok() {
        input.try_parse(|i| {
            i.expect_parenthesis_block()?;
            i.parse_nested_block(|i2| {
                kozan_selector::parser::parse_selector_list(i2)
                    .map_err(|_| i2.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidSelector))
            })
        }).ok()
    } else {
        None
    };

    let end = if input.try_parse(|i| i.expect_ident_matching("to")).is_ok() {
        input.try_parse(|i| {
            i.expect_parenthesis_block()?;
            i.parse_nested_block(|i2| {
                kozan_selector::parser::parse_selector_list(i2)
                    .map_err(|_| i2.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidSelector))
            })
        }).ok()
    } else {
        None
    };

    Ok(AtRulePrelude::Scope(start, end))
}
