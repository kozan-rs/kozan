//! Tests for stylesheet-level parsing: style rules, at-rules, CSS Nesting.

use kozan_css::*;

// Helper: count rules of each type

fn count_rules(rules: &RuleList) -> (usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) {
    let slice = &rules.slice;
    let mut style = 0;
    let mut media = 0;
    let mut keyframes = 0;
    let mut layer = 0;
    let mut supports = 0;
    let mut container = 0;
    let mut font_face = 0;
    let mut import = 0;
    let mut namespace = 0;
    let mut page = 0;
    for rule in slice.iter() {
        match rule {
            CssRule::Style(_) => style += 1,
            CssRule::Media(_) => media += 1,
            CssRule::Keyframes(_) => keyframes += 1,
            CssRule::Layer(_) => layer += 1,
            CssRule::Supports(_) => supports += 1,
            CssRule::Container(_) => container += 1,
            CssRule::FontFace(_) => font_face += 1,
            CssRule::Import(_) => import += 1,
            CssRule::Namespace(_) => namespace += 1,
            CssRule::Page(_) => page += 1,
            CssRule::Property(_) => {}
            CssRule::CounterStyle(_) => {}
            CssRule::Scope(_) => {}
            CssRule::StartingStyle(_) => {}
        }
    }
    (style, media, keyframes, layer, supports, container, font_face, import, namespace, page)
}

// Style rules

#[test]
fn parse_simple_style_rule() {
    let sheet = parse_stylesheet(".foo { color: red }");
    let (style, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 1);

    match &sheet.rules.slice[0] {
        CssRule::Style(rule) => {
            assert!(!rule.declarations.entries().is_empty());
        }
        _ => panic!("expected style rule"),
    }
}

#[test]
fn parse_multiple_style_rules() {
    let sheet = parse_stylesheet("
        .a { display: flex }
        .b { color: blue }
        #c { width: 100px }
    ");
    let (style, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 3);
}

#[test]
fn parse_complex_selector() {
    let sheet = parse_stylesheet("div > .container .item:hover { opacity: 0.5 }");
    let (style, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 1);
}

// @media

#[test]
fn parse_media_rule() {
    let sheet = parse_stylesheet("
        @media screen and (min-width: 768px) {
            .container { max-width: 960px }
        }
    ");
    let (_, media, ..) = count_rules(&sheet.rules);
    assert_eq!(media, 1);

    match &sheet.rules.slice[0] {
        CssRule::Media(rule) => {
            assert_eq!(rule.queries.0.len(), 1);
            // Has nested style rule
            assert_eq!(rule.rules.slice.len(), 1);
        }
        _ => panic!("expected media rule"),
    }
}

#[test]
fn parse_media_condition_only() {
    let sheet = parse_stylesheet("
        @media (min-width: 768px) {
            .a { display: block }
        }
    ");
    let (_, media, ..) = count_rules(&sheet.rules);
    assert_eq!(media, 1);
}

#[test]
fn parse_media_multiple_queries() {
    let sheet = parse_stylesheet("
        @media screen, print {
            body { font-size: 14px }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Media(rule) => {
            assert_eq!(rule.queries.0.len(), 2);
        }
        _ => panic!("expected media rule"),
    }
}

#[test]
fn parse_media_not() {
    let sheet = parse_stylesheet("
        @media not print {
            .screen-only { display: block }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Media(rule) => {
            assert_eq!(rule.queries.0[0].qualifier, Some(MediaQualifier::Not));
        }
        _ => panic!("expected media rule"),
    }
}

#[test]
fn parse_nested_media() {
    let sheet = parse_stylesheet("
        @media screen {
            @media (min-width: 768px) {
                .a { display: flex }
            }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Media(outer) => {
            assert_eq!(outer.rules.slice.len(), 1);
            match &outer.rules.slice[0] {
                CssRule::Media(inner) => {
                    assert_eq!(inner.rules.slice.len(), 1);
                }
                _ => panic!("expected nested media rule"),
            }
        }
        _ => panic!("expected media rule"),
    }
}

// @keyframes

#[test]
fn parse_keyframes_rule() {
    let sheet = parse_stylesheet("
        @keyframes fadeIn {
            from { opacity: 0 }
            to { opacity: 1 }
        }
    ");
    let (_, _, keyframes, ..) = count_rules(&sheet.rules);
    assert_eq!(keyframes, 1);

    match &sheet.rules.slice[0] {
        CssRule::Keyframes(rule) => {
            assert_eq!(&*rule.name, "fadeIn");
            assert_eq!(rule.keyframes.len(), 2);
            assert_eq!(rule.keyframes[0].selectors[0], KeyframeSelector::From);
            assert_eq!(rule.keyframes[1].selectors[0], KeyframeSelector::To);
        }
        _ => panic!("expected keyframes rule"),
    }
}

#[test]
fn parse_keyframes_percentages() {
    let sheet = parse_stylesheet("
        @keyframes slide {
            0% { transform: translateX(0) }
            50% { transform: translateX(50px) }
            100% { transform: translateX(100px) }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Keyframes(rule) => {
            assert_eq!(rule.keyframes.len(), 3);
            assert_eq!(rule.keyframes[0].selectors[0].percentage(), 0.0);
            assert_eq!(rule.keyframes[1].selectors[0].percentage(), 0.5);
            assert_eq!(rule.keyframes[2].selectors[0].percentage(), 1.0);
        }
        _ => panic!("expected keyframes rule"),
    }
}

#[test]
fn parse_keyframes_webkit_prefix() {
    let sheet = parse_stylesheet("
        @-webkit-keyframes bounce {
            from { top: 0 }
            to { top: 100px }
        }
    ");
    let (_, _, keyframes, ..) = count_rules(&sheet.rules);
    assert_eq!(keyframes, 1);
}

// @layer

#[test]
fn parse_layer_block() {
    let sheet = parse_stylesheet("
        @layer utilities {
            .flex { display: flex }
        }
    ");
    let (.., layer, _, _, _, _, _, _) = count_rules(&sheet.rules);
    assert_eq!(layer, 1);

    match &sheet.rules.slice[0] {
        CssRule::Layer(rule) => match rule.as_ref() {
            LayerRule::Block { name, rules } => {
                assert!(name.is_some());
                assert_eq!(rules.slice.len(), 1);
            }
            _ => panic!("expected layer block"),
        },
        _ => panic!("expected layer rule"),
    }
}

#[test]
fn parse_layer_statement() {
    let sheet = parse_stylesheet("@layer base, utilities;");
    match &sheet.rules.slice[0] {
        CssRule::Layer(rule) => match rule.as_ref() {
            LayerRule::Statement { names } => {
                assert_eq!(names.len(), 2);
            }
            _ => panic!("expected layer statement"),
        },
        _ => panic!("expected layer rule"),
    }
}

#[test]
fn parse_layer_anonymous() {
    let sheet = parse_stylesheet("
        @layer {
            .a { color: red }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Layer(rule) => match rule.as_ref() {
            LayerRule::Block { name, .. } => {
                assert!(name.is_none());
            }
            _ => panic!("expected anonymous layer block"),
        },
        _ => panic!("expected layer rule"),
    }
}

#[test]
fn parse_layer_dotted_name() {
    let sheet = parse_stylesheet("
        @layer framework.utilities {
            .flex { display: flex }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Layer(rule) => match rule.as_ref() {
            LayerRule::Block { name: Some(name), .. } => {
                assert_eq!(name.0.len(), 2);
                assert_eq!(&*name.0[0], "framework");
                assert_eq!(&*name.0[1], "utilities");
            }
            _ => panic!("expected named layer block"),
        },
        _ => panic!("expected layer rule"),
    }
}

// @supports

#[test]
fn parse_supports_rule() {
    let sheet = parse_stylesheet("
        @supports (display: grid) {
            .grid { display: grid }
        }
    ");
    let (.., supports, _, _, _, _, _) = count_rules(&sheet.rules);
    assert_eq!(supports, 1);

    match &sheet.rules.slice[0] {
        CssRule::Supports(rule) => {
            assert!(rule.enabled); // display: grid is supported
            assert_eq!(rule.rules.slice.len(), 1);
        }
        _ => panic!("expected supports rule"),
    }
}

#[test]
fn parse_supports_not() {
    let sheet = parse_stylesheet("
        @supports not (display: nonexistent-value) {
            .fallback { display: block }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Supports(rule) => {
            assert!(rule.enabled); // not(unsupported) = true
        }
        _ => panic!("expected supports rule"),
    }
}

#[test]
fn parse_supports_and() {
    let sheet = parse_stylesheet("
        @supports (display: flex) and (display: grid) {
            .modern { display: grid }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Supports(rule) => {
            assert!(rule.enabled);
        }
        _ => panic!("expected supports rule"),
    }
}

// @container

#[test]
fn parse_container_rule() {
    let sheet = parse_stylesheet("
        @container (min-width: 400px) {
            .card { display: grid }
        }
    ");
    let (.., container, _, _, _, _) = count_rules(&sheet.rules);
    assert_eq!(container, 1);

    match &sheet.rules.slice[0] {
        CssRule::Container(rule) => {
            assert!(rule.name.is_none());
            assert_eq!(rule.rules.slice.len(), 1);
        }
        _ => panic!("expected container rule"),
    }
}

#[test]
fn parse_container_named() {
    let sheet = parse_stylesheet("
        @container sidebar (min-width: 300px) {
            .nav { display: flex }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Container(rule) => {
            assert_eq!(&*rule.name.as_ref().unwrap(), "sidebar");
        }
        _ => panic!("expected container rule"),
    }
}

// @font-face

#[test]
fn parse_font_face_rule() {
    let sheet = parse_stylesheet("
        @font-face {
            font-family: 'MyFont';
            font-weight: bold;
        }
    ");
    let (.., font_face, _, _, _) = count_rules(&sheet.rules);
    assert_eq!(font_face, 1);

    match &sheet.rules.slice[0] {
        CssRule::FontFace(rule) => {
            assert!(!rule.declarations.entries().is_empty());
        }
        _ => panic!("expected font-face rule"),
    }
}

#[test]
fn parse_font_face_descriptors() {
    let sheet = parse_stylesheet("
        @font-face {
            font-family: 'MyFont';
            src: url('myfont.woff2') format('woff2');
            unicode-range: U+0025-00FF;
            font-display: swap;
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::FontFace(rule) => {
            // font-family goes to declarations (known CSS property)
            assert!(!rule.declarations.entries().is_empty());
            // src, unicode-range, font-display go to descriptors
            assert!(rule.descriptors.iter().any(|(k, _)| &**k == "src"));
            assert!(rule.descriptors.iter().any(|(k, _)| &**k == "unicode-range"));
            assert!(rule.descriptors.iter().any(|(k, _)| &**k == "font-display"));
        }
        _ => panic!("expected font-face rule"),
    }
}

// @charset

#[test]
fn parse_charset_silently_skipped() {
    let sheet = parse_stylesheet("@charset 'utf-8'; .a { color: red }");
    // @charset should be silently dropped, only .a remains
    let (style, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 1, "@charset should not produce a rule");
}

// @import

#[test]
fn parse_import_rule() {
    let sheet = parse_stylesheet("@import url('reset.css');");
    let (.., import, _, _) = count_rules(&sheet.rules);
    assert_eq!(import, 1);

    match &sheet.rules.slice[0] {
        CssRule::Import(rule) => {
            assert_eq!(&*rule.url, "reset.css");
        }
        _ => panic!("expected import rule"),
    }
}

#[test]
fn parse_import_string() {
    let sheet = parse_stylesheet("@import 'styles.css';");
    match &sheet.rules.slice[0] {
        CssRule::Import(rule) => {
            assert_eq!(&*rule.url, "styles.css");
        }
        _ => panic!("expected import rule"),
    }
}

// @namespace

#[test]
fn parse_namespace_rule() {
    let sheet = parse_stylesheet("@namespace url('http://www.w3.org/1999/xhtml');");
    let (.., namespace, _) = count_rules(&sheet.rules);
    assert_eq!(namespace, 1);

    match &sheet.rules.slice[0] {
        CssRule::Namespace(rule) => {
            assert!(rule.prefix.is_none());
            assert_eq!(&*rule.url, "http://www.w3.org/1999/xhtml");
        }
        _ => panic!("expected namespace rule"),
    }
}

#[test]
fn parse_namespace_with_prefix() {
    let sheet = parse_stylesheet("@namespace svg url('http://www.w3.org/2000/svg');");
    match &sheet.rules.slice[0] {
        CssRule::Namespace(rule) => {
            assert_eq!(&*rule.prefix.as_ref().unwrap(), "svg");
        }
        _ => panic!("expected namespace rule"),
    }
}

// @page

#[test]
fn parse_page_rule() {
    let sheet = parse_stylesheet("
        @page {
            margin: 1cm;
        }
    ");
    let (.., page) = count_rules(&sheet.rules);
    assert_eq!(page, 1);
}

#[test]
fn parse_page_named_selector() {
    let sheet = parse_stylesheet("
        @page :first {
            margin-top: 2cm;
        }
    ");
    let (.., page) = count_rules(&sheet.rules);
    assert_eq!(page, 1);
    match &sheet.rules.slice[0] {
        CssRule::Page(rule) => {
            assert!(!rule.selectors.is_empty(), "@page :first should have a selector");
            assert!(!rule.declarations.entries().is_empty());
        }
        _ => panic!("expected page rule"),
    }
}

#[test]
fn parse_page_left_right() {
    let sheet = parse_stylesheet("
        @page :left { margin-left: 3cm }
        @page :right { margin-right: 3cm }
    ");
    let (.., page) = count_rules(&sheet.rules);
    assert_eq!(page, 2);
}

// Mixed stylesheet

#[test]
fn parse_mixed_stylesheet() {
    let sheet = parse_stylesheet("
        @import url('reset.css');
        @layer base, utilities;

        @media screen {
            .container { max-width: 1200px }
        }

        @keyframes fadeIn {
            from { opacity: 0 }
            to { opacity: 1 }
        }

        .hero { display: flex }
        #main { color: black }

        @supports (display: grid) {
            .grid { display: grid }
        }
    ");

    let (style, media, keyframes, layer, supports, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 2);   // .hero, #main
    assert_eq!(media, 1);
    assert_eq!(keyframes, 1);
    assert_eq!(layer, 1);
    assert_eq!(supports, 1);
}

// Error recovery

#[test]
fn parse_recovers_from_invalid_rule() {
    let sheet = parse_stylesheet("
        .valid1 { color: red }
        @invalid-at-rule { something }
        .valid2 { color: blue }
    ");
    // Should recover and parse the valid rules
    let (style, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 2);
}

#[test]
fn parse_recovers_from_invalid_selector() {
    let sheet = parse_stylesheet("
        .valid { color: red }
        [[ invalid { color: blue }
        #also-valid { display: flex }
    ");
    let (style, ..) = count_rules(&sheet.rules);
    assert!(style >= 1); // At least .valid should parse
}

// Source URL

#[test]
fn parse_with_source_url() {
    let sheet = parse_stylesheet_with_url(".a { color: red }", "styles.css");
    assert_eq!(&*sheet.source_url.unwrap(), "styles.css");
}

// CSS Nesting

#[test]
fn parse_nested_style_rules() {
    let sheet = parse_stylesheet("
        .parent {
            color: red;
            .child {
                color: blue;
            }
        }
    ");
    let (style, ..) = count_rules(&sheet.rules);
    assert_eq!(style, 1); // Only the parent at top level

    match &sheet.rules.slice[0] {
        CssRule::Style(rule) => {
            // Should have declarations AND nested rules
            assert!(!rule.declarations.entries().is_empty());
            assert_eq!(rule.rules.slice.len(), 1);
        }
        _ => panic!("expected style rule"),
    }
}

#[test]
fn parse_nested_media_in_style() {
    let sheet = parse_stylesheet("
        .responsive {
            display: block;
            @media (min-width: 768px) {
                display: flex;
            }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Style(rule) => {
            assert_eq!(rule.rules.slice.len(), 1);
            match &rule.rules.slice[0] {
                CssRule::Media(_) => {} // Nested @media inside style rule
                _ => panic!("expected nested media rule"),
            }
        }
        _ => panic!("expected style rule"),
    }
}

// Empty stylesheet

#[test]
fn parse_empty_stylesheet() {
    let sheet = parse_stylesheet("");
    assert_eq!(sheet.rules.slice.len(), 0);
}

#[test]
fn parse_whitespace_only() {
    let sheet = parse_stylesheet("   \n\t  ");
    assert_eq!(sheet.rules.slice.len(), 0);
}

#[test]
fn parse_comments_only() {
    let sheet = parse_stylesheet("/* this is a comment */");
    assert_eq!(sheet.rules.slice.len(), 0);
}

// @property

#[test]
fn parse_property_rule() {
    let sheet = parse_stylesheet("
        @property --gap {
            syntax: '<length>';
            inherits: false;
            initial-value: 0px;
        }
    ");
    assert_eq!(sheet.rules.slice.len(), 1);
    match &sheet.rules.slice[0] {
        CssRule::Property(rule) => {
            assert_eq!(&*rule.name, "--gap");
            assert_eq!(rule.syntax, PropertySyntax::Typed(kozan_atom::Atom::new("<length>")));
            assert!(!rule.inherits);
            assert_eq!(&*rule.initial_value.as_ref().unwrap(), "0px");
        }
        _ => panic!("expected @property rule"),
    }
}

#[test]
fn parse_property_universal_syntax() {
    let sheet = parse_stylesheet("
        @property --theme {
            syntax: '*';
            inherits: true;
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Property(rule) => {
            assert_eq!(&*rule.name, "--theme");
            assert_eq!(rule.syntax, PropertySyntax::Universal);
            assert!(rule.inherits);
            assert!(rule.initial_value.is_none());
        }
        _ => panic!("expected @property rule"),
    }
}

// @counter-style

#[test]
fn parse_counter_style_rule() {
    let sheet = parse_stylesheet("
        @counter-style thumbs {
            system: cyclic;
            suffix: ' ';
        }
    ");
    assert_eq!(sheet.rules.slice.len(), 1);
    match &sheet.rules.slice[0] {
        CssRule::CounterStyle(rule) => {
            assert_eq!(&*rule.name, "thumbs");
            // Counter-style descriptors are captured as raw key-value pairs
            assert!(rule.descriptors.len() >= 2, "should capture system + suffix descriptors");
            assert!(rule.descriptors.iter().any(|(k, _)| &**k == "system"));
            assert!(rule.descriptors.iter().any(|(k, _)| &**k == "suffix"));
        }
        _ => panic!("expected @counter-style rule"),
    }
}

// @scope

#[test]
fn parse_scope_rule() {
    let sheet = parse_stylesheet("
        @scope (.card) to (.card-content) {
            .title { font-weight: bold }
        }
    ");
    assert_eq!(sheet.rules.slice.len(), 1);
    match &sheet.rules.slice[0] {
        CssRule::Scope(rule) => {
            assert!(rule.start.is_some());
            assert!(rule.end.is_some());
            assert_eq!(rule.rules.slice.len(), 1);
        }
        _ => panic!("expected @scope rule"),
    }
}

#[test]
fn parse_scope_no_limit() {
    let sheet = parse_stylesheet("
        @scope (.card) {
            .title { color: red }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Scope(rule) => {
            assert!(rule.start.is_some());
            assert!(rule.end.is_none());
        }
        _ => panic!("expected @scope rule"),
    }
}

#[test]
fn parse_scope_implicit() {
    let sheet = parse_stylesheet("
        @scope {
            .a { color: blue }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::Scope(rule) => {
            assert!(rule.start.is_none());
            assert!(rule.end.is_none());
        }
        _ => panic!("expected @scope rule"),
    }
}

// @starting-style

#[test]
fn parse_starting_style_rule() {
    let sheet = parse_stylesheet("
        @starting-style {
            .fade-in { opacity: 0 }
        }
    ");
    assert_eq!(sheet.rules.slice.len(), 1);
    match &sheet.rules.slice[0] {
        CssRule::StartingStyle(rule) => {
            assert_eq!(rule.rules.slice.len(), 1);
        }
        _ => panic!("expected @starting-style rule"),
    }
}

#[test]
fn parse_starting_style_multiple_rules() {
    let sheet = parse_stylesheet("
        @starting-style {
            .a { opacity: 0 }
            .b { transform: translateY(20px) }
        }
    ");
    match &sheet.rules.slice[0] {
        CssRule::StartingStyle(rule) => {
            assert_eq!(rule.rules.slice.len(), 2);
        }
        _ => panic!("expected @starting-style rule"),
    }
}
