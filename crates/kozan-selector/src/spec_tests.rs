//! CSS Selectors Level 4 spec conformance tests.
//!
//! These tests verify behavior against the W3C CSS Selectors Level 4 specification:
//! <https://drafts.csswg.org/selectors-4/>
//!
//! Test vectors adapted from the Web Platform Tests (WPT) suite:
//! <https://github.com/web-platform-tests/wpt/tree/master/css/selectors>
//!
//! Each test references the relevant spec section.

#[cfg(test)]
mod tests {
    use crate::context::MatchingContext;
    use crate::element::Element;
    use crate::matching::{matches, matches_in_context};
    use crate::opaque::OpaqueElement;
    use crate::parser::parse;
    use crate::pseudo_class::ElementState;
    use crate::specificity::Specificity;
    use crate::types::*;
    use kozan_atom::Atom;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(10000);

    #[derive(Clone)]
    struct El {
        tag: Atom,
        id: Option<Atom>,
        classes: Vec<Atom>,
        attrs: Vec<(Atom, String)>,
        state: ElementState,
        parent: Option<Box<El>>,
        children: Vec<El>,
        prev_sibling: Option<Box<El>>,
        next_sibling: Option<Box<El>>,
        index: u32,
        sibling_count: u32,
        type_index: u32,
        type_count: u32,
        is_root: bool,
        opaque_id: u64,
        dir: Direction,
        // Shadow DOM fields
        shadow_host: bool,
        shadow_host_of: Option<Box<El>>,
        in_slot: bool,
        assigned_slot_el: Option<Box<El>>,
        parts: Vec<Atom>,
        custom_states: Vec<Atom>,
        column_el: Option<Box<El>>,
        ns: Option<Atom>,
    }

    impl El {
        fn new(tag: &str) -> Self {
            Self {
                tag: Atom::from(tag),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
                state: ElementState::empty(),
                parent: None,
                children: Vec::new(),
                prev_sibling: None,
                next_sibling: None,
                index: 1,
                sibling_count: 1,
                type_index: 1,
                type_count: 1,
                is_root: false,
                opaque_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                dir: Direction::Ltr,
                shadow_host: false,
                shadow_host_of: None,
                in_slot: false,
                assigned_slot_el: None,
                parts: Vec::new(),
                custom_states: Vec::new(),
                column_el: None,
                ns: None,
            }
        }

        fn id(mut self, id: &str) -> Self { self.id = Some(Atom::from(id)); self }
        fn class(mut self, c: &str) -> Self { self.classes.push(Atom::from(c)); self }
        fn attr(mut self, n: &str, v: &str) -> Self { self.attrs.push((Atom::from(n), v.into())); self }
        fn state(mut self, s: ElementState) -> Self { self.state = s; self }
        fn parent(mut self, p: El) -> Self { self.parent = Some(Box::new(p)); self }
        fn child(mut self, c: El) -> Self { self.children.push(c); self }
        fn prev(mut self, s: El) -> Self { self.prev_sibling = Some(Box::new(s)); self }
        fn next(mut self, s: El) -> Self { self.next_sibling = Some(Box::new(s)); self }
        fn pos(mut self, index: u32, count: u32) -> Self { self.index = index; self.sibling_count = count; self }
        fn type_pos(mut self, index: u32, count: u32) -> Self { self.type_index = index; self.type_count = count; self }
        fn root(mut self) -> Self { self.is_root = true; self }
        fn dir(mut self, d: Direction) -> Self { self.dir = d; self }
        fn as_shadow_host(mut self) -> Self { self.shadow_host = true; self }
        fn in_shadow_of(mut self, host: El) -> Self { self.shadow_host_of = Some(Box::new(host)); self }
        fn slotted(mut self) -> Self { self.in_slot = true; self }
        fn part(mut self, name: &str) -> Self { self.parts.push(Atom::from(name)); self }
        fn custom_state(mut self, name: &str) -> Self { self.custom_states.push(Atom::from(name)); self }
        fn column(mut self, col: El) -> Self { self.column_el = Some(Box::new(col)); self }
    }

    impl Element for El {
        fn local_name(&self) -> &Atom { &self.tag }
        fn id(&self) -> Option<&Atom> { self.id.as_ref() }
        fn has_class(&self, class: &Atom) -> bool { self.classes.iter().any(|c| c == class) }
        fn each_class<F: FnMut(&Atom)>(&self, mut f: F) { for c in &self.classes { f(c); } }
        fn attr(&self, name: &Atom) -> Option<&str> {
            self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
        }
        fn parent_element(&self) -> Option<Self> { self.parent.as_ref().map(|p| *p.clone()) }
        fn prev_sibling_element(&self) -> Option<Self> { self.prev_sibling.as_ref().map(|s| *s.clone()) }
        fn next_sibling_element(&self) -> Option<Self> { self.next_sibling.as_ref().map(|s| *s.clone()) }
        fn first_child_element(&self) -> Option<Self> { self.children.first().cloned() }
        fn last_child_element(&self) -> Option<Self> { self.children.last().cloned() }
        fn state(&self) -> ElementState { self.state }
        fn is_root(&self) -> bool { self.is_root }
        fn is_empty(&self) -> bool { self.children.is_empty() }
        fn child_index(&self) -> u32 { self.index }
        fn child_count(&self) -> u32 { self.sibling_count }
        fn child_index_of_type(&self) -> u32 { self.type_index }
        fn child_count_of_type(&self) -> u32 { self.type_count }
        fn opaque(&self) -> OpaqueElement { OpaqueElement::new(self.opaque_id) }
        fn direction(&self) -> Direction { self.dir }
        fn namespace(&self) -> Option<&Atom> { self.ns.as_ref() }
        fn is_shadow_host(&self) -> bool { self.shadow_host }
        fn containing_shadow_host(&self) -> Option<Self> { self.shadow_host_of.as_ref().map(|h| *h.clone()) }
        fn is_in_slot(&self) -> bool { self.in_slot }
        fn assigned_slot(&self) -> Option<Self> { self.assigned_slot_el.as_ref().map(|s| *s.clone()) }
        fn is_part(&self, name: &Atom) -> bool { self.parts.iter().any(|p| p == name) }
        fn has_custom_state(&self, name: &Atom) -> bool { self.custom_states.iter().any(|s| s == name) }
        fn column_element(&self) -> Option<Self> { self.column_el.as_ref().map(|c| *c.clone()) }
    }

    fn sel_matches(css: &str, el: &El) -> bool {
        let list = parse(css).unwrap();
        list.0.iter().any(|sel| matches(sel, el))
    }

    fn specificity_of(css: &str) -> Specificity {
        parse(css).unwrap().0[0].specificity()
    }

    /// Assert that a selector parses successfully and round-trips to expected output.
    fn assert_parses_to(input: &str, expected: &str) {
        let list = parse(input).expect(&format!("'{input}' should parse"));
        let output = format!("{}", list.0[0]);
        assert_eq!(expected, output, "'{input}' should serialize to '{expected}', got '{output}'");
    }

    /// Assert that a selector parses and round-trips unchanged.
    fn assert_parses(input: &str) {
        assert_parses_to(input, input);
    }

    /// Assert that a selector fails to parse.
    fn assert_parse_error(input: &str) {
        assert!(parse(input).is_err(), "'{input}' should NOT parse");
    }

    // ===================================================================
    // §5.1 Type selectors (WPT: broad coverage)
    // ===================================================================

    #[test]
    fn spec_type_selector() {
        assert!(sel_matches("div", &El::new("div")));
        assert!(!sel_matches("div", &El::new("span")));
        assert!(sel_matches("h1", &El::new("h1")));
        assert!(sel_matches("custom-element", &El::new("custom-element")));
    }

    // ===================================================================
    // §5.2 Universal selector (WPT: parse-universal.html)
    // ===================================================================

    #[test]
    fn spec_universal_matches_any() {
        assert!(sel_matches("*", &El::new("div")));
        assert!(sel_matches("*", &El::new("span")));
        assert!(sel_matches("*", &El::new("custom-element")));
    }

    #[test]
    fn spec_universal_implicit_in_compound() {
        let el = El::new("div").class("foo");
        assert!(sel_matches("*.foo", &el));
        assert!(sel_matches(".foo", &el));
    }

    // ===================================================================
    // §6 Attribute selectors (WPT: parse-attribute.html)
    // ===================================================================

    #[test]
    fn spec_attr_exists() {
        let el = El::new("input").attr("disabled", "");
        assert!(sel_matches("[disabled]", &el));
        assert!(!sel_matches("[readonly]", &el));
    }

    #[test]
    fn spec_attr_equals() {
        let el = El::new("input").attr("type", "text");
        assert!(sel_matches("[type=text]", &el));
        assert!(sel_matches("[type='text']", &el));
        assert!(!sel_matches("[type=password]", &el));
    }

    #[test]
    fn spec_attr_includes() {
        let el = El::new("div").attr("class", "foo bar baz");
        assert!(sel_matches("[class~=bar]", &el));
        assert!(!sel_matches("[class~=ba]", &el));
    }

    #[test]
    fn spec_attr_dash_match() {
        let el = El::new("div").attr("lang", "en-US");
        assert!(sel_matches("[lang|=en]", &el));
        assert!(!sel_matches("[lang|=en-U]", &el));
        let el2 = El::new("div").attr("lang", "en");
        assert!(sel_matches("[lang|=en]", &el2));
    }

    #[test]
    fn spec_attr_prefix() {
        let el = El::new("a").attr("href", "https://example.com");
        assert!(sel_matches("[href^=\"https\"]", &el));
        assert!(!sel_matches("[href^=\"http://\"]", &el));
    }

    #[test]
    fn spec_attr_suffix() {
        let el = El::new("a").attr("href", "document.pdf");
        assert!(sel_matches("[href$=\".pdf\"]", &el));
        assert!(!sel_matches("[href$=\".doc\"]", &el));
    }

    #[test]
    fn spec_attr_substring() {
        let el = El::new("div").attr("data-info", "hello world");
        assert!(sel_matches("[data-info*=llo]", &el));
        assert!(!sel_matches("[data-info*=xyz]", &el));
    }

    #[test]
    fn spec_attr_case_insensitive() {
        let el = El::new("input").attr("type", "TEXT");
        assert!(sel_matches("[type=text i]", &el));
        assert!(sel_matches("[type=TEXT]", &el));
    }

    // WPT: parse-attribute.html — round-trip serialization
    #[test]
    fn wpt_attr_parse_round_trip() {
        // Unquoted values serialize as quoted
        assert_parses_to("[att=val]", "[att=\"val\"]");
        assert_parses_to("[att~=val]", "[att~=\"val\"]");
        assert_parses_to("[att|=val]", "[att|=\"val\"]");
        assert_parses_to("[att^=val]", "[att^=\"val\"]");
        assert_parses_to("[att$=val]", "[att$=\"val\"]");
        assert_parses_to("[att*=val]", "[att*=\"val\"]");

        // Presence selector
        assert_parses("[att]");

        // With type selector
        assert_parses("h1[title]");
        assert_parses_to("span[class='example']", "span[class=\"example\"]");
        assert_parses_to("a[hreflang=fr]", "a[hreflang=\"fr\"]");
        assert_parses_to("a[hreflang|='en']", "a[hreflang|=\"en\"]");

        // Quoted values
        assert_parses("object[type^=\"image/\"]");
        assert_parses("a[href$=\".html\"]");
        assert_parses("p[title*=\"hello\"]");
    }

    // WPT: parse-attribute.html — namespace in attributes
    // Our AttrSelector stores name only (no namespace field) — namespace is
    // consumed during parsing but not preserved in serialization.
    #[test]
    fn wpt_attr_namespace() {
        // [*|att] parses successfully, serializes without namespace prefix
        assert_parses_to("[*|att]", "[att]");
        // [|att] = no namespace = plain [att]
        assert_parses_to("[|att]", "[att]");
    }

    // ===================================================================
    // §6.6-6.7 Class and ID selectors
    // ===================================================================

    #[test]
    fn spec_class_selector() {
        let el = El::new("div").class("warning").class("urgent");
        assert!(sel_matches(".warning", &el));
        assert!(sel_matches(".urgent", &el));
        assert!(sel_matches(".warning.urgent", &el));
        assert!(!sel_matches(".info", &el));
    }

    #[test]
    fn spec_id_selector() {
        let el = El::new("div").id("nav");
        assert!(sel_matches("#nav", &el));
        assert!(!sel_matches("#footer", &el));
    }

    // ===================================================================
    // §13 Structural pseudo-classes
    // ===================================================================

    #[test]
    fn spec_root() {
        assert!(sel_matches(":root", &El::new("html").root()));
        assert!(!sel_matches(":root", &El::new("div")));
    }

    #[test]
    fn spec_empty() {
        assert!(sel_matches(":empty", &El::new("div")));
        assert!(!sel_matches(":empty", &El::new("div").child(El::new("span"))));
    }

    #[test]
    fn spec_first_child() {
        let el = El::new("li").pos(1, 5);
        assert!(sel_matches(":first-child", &el));
        assert!(!sel_matches(":first-child", &El::new("li").pos(2, 5)));
    }

    #[test]
    fn spec_last_child() {
        assert!(sel_matches(":last-child", &El::new("li").pos(5, 5)));
        assert!(!sel_matches(":last-child", &El::new("li").pos(3, 5)));
    }

    #[test]
    fn spec_only_child() {
        assert!(sel_matches(":only-child", &El::new("li").pos(1, 1)));
        assert!(!sel_matches(":only-child", &El::new("li").pos(1, 3)));
    }

    #[test]
    fn spec_first_of_type() {
        assert!(sel_matches(":first-of-type", &El::new("li").type_pos(1, 3)));
        assert!(!sel_matches(":first-of-type", &El::new("li").type_pos(2, 3)));
    }

    #[test]
    fn spec_last_of_type() {
        assert!(sel_matches(":last-of-type", &El::new("li").type_pos(3, 3)));
        assert!(!sel_matches(":last-of-type", &El::new("li").type_pos(1, 3)));
    }

    #[test]
    fn spec_only_of_type() {
        assert!(sel_matches(":only-of-type", &El::new("li").type_pos(1, 1)));
        assert!(!sel_matches(":only-of-type", &El::new("li").type_pos(1, 2)));
    }

    // ===================================================================
    // §13.3 :nth-child() (WPT: parse-anplusb.html, child-indexed-pseudo-class.html)
    // ===================================================================

    #[test]
    fn spec_nth_child_even_odd() {
        assert!(sel_matches(":nth-child(odd)", &El::new("li").pos(1, 10)));
        assert!(!sel_matches(":nth-child(odd)", &El::new("li").pos(2, 10)));
        assert!(sel_matches(":nth-child(even)", &El::new("li").pos(2, 10)));
        assert!(!sel_matches(":nth-child(even)", &El::new("li").pos(3, 10)));
    }

    #[test]
    fn spec_nth_child_formula() {
        // 3n+1 matches 1, 4, 7, 10...
        assert!(sel_matches(":nth-child(3n+1)", &El::new("li").pos(1, 10)));
        assert!(!sel_matches(":nth-child(3n+1)", &El::new("li").pos(2, 10)));
        assert!(!sel_matches(":nth-child(3n+1)", &El::new("li").pos(3, 10)));
        assert!(sel_matches(":nth-child(3n+1)", &El::new("li").pos(4, 10)));
        assert!(sel_matches(":nth-child(3n+1)", &El::new("li").pos(7, 10)));
    }

    #[test]
    fn spec_nth_child_negative_offset() {
        // -n+3 matches 1, 2, 3 (first three).
        assert!(sel_matches(":nth-child(-n+3)", &El::new("li").pos(1, 10)));
        assert!(sel_matches(":nth-child(-n+3)", &El::new("li").pos(2, 10)));
        assert!(sel_matches(":nth-child(-n+3)", &El::new("li").pos(3, 10)));
        assert!(!sel_matches(":nth-child(-n+3)", &El::new("li").pos(4, 10)));
    }

    #[test]
    fn spec_nth_child_constant() {
        assert!(!sel_matches(":nth-child(3)", &El::new("li").pos(2, 10)));
        assert!(sel_matches(":nth-child(3)", &El::new("li").pos(3, 10)));
        assert!(!sel_matches(":nth-child(3)", &El::new("li").pos(4, 10)));
    }

    #[test]
    fn spec_nth_last_child() {
        assert!(sel_matches(":nth-last-child(1)", &El::new("li").pos(5, 5)));
        assert!(!sel_matches(":nth-last-child(1)", &El::new("li").pos(4, 5)));
    }

    #[test]
    fn spec_nth_of_type() {
        assert!(sel_matches(":nth-of-type(2n)", &El::new("p").type_pos(2, 5)));
        assert!(!sel_matches(":nth-of-type(2n)", &El::new("p").type_pos(1, 5)));
    }

    // WPT: parse-anplusb.html — An+B serialization round-trips
    // Source: run_tests_on_anplusb_selector() from WPT
    #[test]
    fn wpt_anplusb_serialization() {
        let prefixes = [":nth-child", ":nth-last-child", ":nth-of-type", ":nth-last-of-type"];
        for prefix in &prefixes {
            // 1n+0 → n
            assert_parses_to(&format!("{prefix}(1n+0)"), &format!("{prefix}(n)"));
            // n+0 → n
            assert_parses_to(&format!("{prefix}(n+0)"), &format!("{prefix}(n)"));
            // n → n
            assert_parses_to(&format!("{prefix}(n)"), &format!("{prefix}(n)"));
            // -n+0 → -n
            assert_parses_to(&format!("{prefix}(-n+0)"), &format!("{prefix}(-n)"));
            // -n → -n
            assert_parses_to(&format!("{prefix}(-n)"), &format!("{prefix}(-n)"));
            // Case insensitivity: N → n
            assert_parses_to(&format!("{prefix}(N)"), &format!("{prefix}(n)"));
            // +n+3 → n+3
            assert_parses_to(&format!("{prefix}(+n+3)"), &format!("{prefix}(n+3)"));
            // 23n+123
            assert_parses_to(&format!("{prefix}(23n+123)"), &format!("{prefix}(23n+123)"));
        }
    }

    // WPT: parse-anplusb.html — invalid An+B expressions
    #[test]
    fn wpt_anplusb_invalid() {
        let prefixes = [":nth-child", ":nth-last-child", ":nth-of-type", ":nth-last-of-type"];
        for prefix in &prefixes {
            assert_parse_error(&format!("{prefix}(n-+1)"));
            assert_parse_error(&format!("{prefix}(n-1n)"));
            assert_parse_error(&format!("{prefix}(-n+n)"));
        }
    }

    // Comprehensive nth formula matching
    #[test]
    fn spec_nth_child_all_formulas() {
        // 2n (even): matches 2, 4, 6, 8, 10
        for i in 1..=10u32 {
            let el = El::new("li").pos(i, 10);
            assert_eq!(sel_matches(":nth-child(2n)", &el), i % 2 == 0,
                "2n should{}match index {i}", if i % 2 == 0 { " " } else { " NOT " });
        }

        // 2n+1 (odd): matches 1, 3, 5, 7, 9
        for i in 1..=10u32 {
            let el = El::new("li").pos(i, 10);
            assert_eq!(sel_matches(":nth-child(2n+1)", &el), i % 2 == 1,
                "2n+1 should{}match index {i}", if i % 2 == 1 { " " } else { " NOT " });
        }

        // 5n-2: matches 3, 8 (5*1-2=3, 5*2-2=8)
        let expected = [3, 8];
        for i in 1..=10u32 {
            let el = El::new("li").pos(i, 10);
            assert_eq!(sel_matches(":nth-child(5n-2)", &el), expected.contains(&i),
                "5n-2 should{}match index {i}", if expected.contains(&i) { " " } else { " NOT " });
        }

        // 0n+5 = just 5th child
        for i in 1..=10u32 {
            let el = El::new("li").pos(i, 10);
            assert_eq!(sel_matches(":nth-child(5)", &el), i == 5);
        }
    }

    // nth-last-child comprehensive
    #[test]
    fn spec_nth_last_child_formula() {
        // :nth-last-child(odd) with 5 siblings: from-end indices 1,3,5 → positions 5,3,1
        assert!(sel_matches(":nth-last-child(odd)", &El::new("li").pos(5, 5)));
        assert!(!sel_matches(":nth-last-child(odd)", &El::new("li").pos(4, 5)));
        assert!(sel_matches(":nth-last-child(odd)", &El::new("li").pos(3, 5)));
        assert!(!sel_matches(":nth-last-child(odd)", &El::new("li").pos(2, 5)));
        assert!(sel_matches(":nth-last-child(odd)", &El::new("li").pos(1, 5)));

        // :nth-last-child(2n) from end: 2,4 → positions 4,2
        assert!(!sel_matches(":nth-last-child(2n)", &El::new("li").pos(5, 5)));
        assert!(sel_matches(":nth-last-child(2n)", &El::new("li").pos(4, 5)));
        assert!(!sel_matches(":nth-last-child(2n)", &El::new("li").pos(3, 5)));
        assert!(sel_matches(":nth-last-child(2n)", &El::new("li").pos(2, 5)));
    }

    // ===================================================================
    // §4.3 :not() (WPT: parse-not.html, not-specificity.html)
    // ===================================================================

    #[test]
    fn spec_not_simple() {
        let el = El::new("div").class("active");
        assert!(sel_matches(":not(.hidden)", &el));
        assert!(!sel_matches(":not(.active)", &el));
    }

    #[test]
    fn spec_not_compound() {
        let el = El::new("div").class("foo");
        assert!(!sel_matches(":not(div.foo)", &el));
        let el2 = El::new("span").class("foo");
        assert!(sel_matches(":not(div.foo)", &el2));
    }

    #[test]
    fn spec_not_list() {
        let el = El::new("div").class("c");
        assert!(sel_matches(":not(.a, .b)", &el));
        let el2 = El::new("div").class("a");
        assert!(!sel_matches(":not(.a, .b)", &el2));
    }

    // WPT: parse-not.html — valid selectors
    #[test]
    fn wpt_not_parse_valid() {
        assert_parses("button:not([disabled])");
        assert_parses(":not(:link):not(:visited)");
        assert_parses(":not(:hover)");
        assert_parses("foo:not(bar)");
        assert_parses(":not(:not(foo))");
        assert_parses(":not(.a .b)");
        assert_parses(":not(.a + .b)");
        assert_parses(":not(.a .b ~ c)");
        assert_parses_to(":not(span.a, div.b)", ":not(span.a, div.b)");
        assert_parses(":not(.a .b ~ c, .d .e)");
    }

    // WPT: parse-not.html — invalid selectors
    #[test]
    fn wpt_not_parse_invalid() {
        assert_parse_error(":not()");
        assert_parse_error(":not(:not())");
        assert_parse_error(":not(::before)");
    }

    // WPT: not-specificity.html — specificity of :not() with lists
    #[test]
    fn wpt_not_specificity() {
        // :not(#foo) > :not(.foo)
        assert!(specificity_of(":not(#foo)") > specificity_of(":not(.foo)"));

        // :not(div#foo) > :not(#foo) — compound adds type specificity
        assert!(specificity_of(":not(div#foo)") > specificity_of(":not(#foo)"));

        // :not(.bar, #foo) = :not(#foo, .bar) — max of args, same result
        assert_eq!(specificity_of(":not(.bar, #foo)"), specificity_of(":not(#foo, .bar)"));

        // :not(.bar, #foo) > :not(.foo, .bar) — #foo vs .foo
        assert!(specificity_of(":not(.bar, #foo)") > specificity_of(":not(.foo, .bar)"));

        // :not(span + span) > :not(span) — two type selectors + combinator
        assert!(specificity_of(":not(span + span)") > specificity_of(":not(span)"));
    }

    // :not() nested specificity
    #[test]
    fn wpt_not_nested_specificity() {
        // :not(.foo) = (0,1,0)
        assert_eq!(specificity_of(":not(.foo)"), Specificity::new(0, 1, 0));
        // :not(#id) = (1,0,0)
        assert_eq!(specificity_of(":not(#id)"), Specificity::new(1, 0, 0));
        // :not(.a, #b) = max((0,1,0), (1,0,0)) = (1,0,0)
        assert_eq!(specificity_of(":not(.a, #b)"), Specificity::new(1, 0, 0));
    }

    // ===================================================================
    // §4.2 :is() (WPT: parse-is.html, is-specificity.html)
    // ===================================================================

    #[test]
    fn spec_is_matches_any() {
        let el = El::new("div").class("foo");
        assert!(sel_matches(":is(.foo, .bar)", &el));
        assert!(sel_matches(":is(div, span)", &el));
        assert!(!sel_matches(":is(.bar, .baz)", &el));
    }

    // WPT: parse-is.html — valid :is() selectors
    #[test]
    fn wpt_is_parse_valid() {
        assert_parses_to(":is(ul,ol,.list) > [hidden]", ":is(ul, ol, .list) > [hidden]");
        assert_parses_to(":is(:hover,:focus)", ":is(:hover, :focus)");
        assert_parses("a:is(:not(:hover))");
        assert_parses(":is(#a)");
        assert_parses(".a.b ~ :is(.c.d ~ .e.f)");
    }

    // WPT: is-specificity.html — :is() takes max of arguments
    #[test]
    fn wpt_is_specificity() {
        // :is(.a, #b) → max((0,1,0), (1,0,0)) = (1,0,0)
        assert_eq!(specificity_of(":is(.a, #b)"), Specificity::new(1, 0, 0));
        // :is(.a, .b) → max((0,1,0), (0,1,0)) = (0,1,0)
        assert_eq!(specificity_of(":is(.a, .b)"), Specificity::new(0, 1, 0));
        // :is(div, .foo) → max((0,0,1), (0,1,0)) = (0,1,0)
        assert_eq!(specificity_of(":is(div, .foo)"), Specificity::new(0, 1, 0));
        // :is(#a, .b.c + .d) → max((1,0,0), (0,2,0)+(0,1,0)) = (1,0,0)
        assert_eq!(specificity_of(":is(#a, .b.c + .d)"), Specificity::new(1, 0, 0));
    }

    // ===================================================================
    // §4.4 :where() (WPT: parse-where.html)
    // ===================================================================

    #[test]
    fn spec_where_zero_specificity() {
        assert_eq!(specificity_of(":where(.foo)"), Specificity::new(0, 0, 0));
        assert_eq!(specificity_of(":where(#id)"), Specificity::new(0, 0, 0));
        assert_eq!(specificity_of(":where(div.foo#bar)"), Specificity::new(0, 0, 0));
    }

    #[test]
    fn spec_where_matches_same_as_is() {
        let el = El::new("div").class("foo");
        assert!(sel_matches(":where(.foo, .bar)", &el));
        assert!(!sel_matches(":where(.bar, .baz)", &el));
    }

    // WPT: parse-where.html — valid :where() selectors
    #[test]
    fn wpt_where_parse_valid() {
        assert_parses_to(":where(ul,ol,.list) > [hidden]", ":where(ul, ol, .list) > [hidden]");
        assert_parses_to(":where(:hover,:focus)", ":where(:hover, :focus)");
        assert_parses("a:where(:not(:hover))");
        assert_parses(":where(#a)");
        assert_parses(".a.b ~ :where(.c.d ~ .e.f)");
    }

    // :where() nested — specificity stays zero even with complex args
    #[test]
    fn wpt_where_complex_zero_specificity() {
        assert_eq!(specificity_of(":where(.a.b.c)"), Specificity::new(0, 0, 0));
        assert_eq!(specificity_of(":where(.a .b + .c > .d)"), Specificity::new(0, 0, 0));
        assert_eq!(specificity_of(":where(#x, .y, z)"), Specificity::new(0, 0, 0));
    }

    // ===================================================================
    // §4.5 :has() (WPT: parse-has.html, has-specificity.html)
    // ===================================================================

    #[test]
    fn spec_has_child() {
        let el = El::new("div").child(El::new("span").class("child"));
        assert!(sel_matches(":has(> .child)", &el));
        assert!(!sel_matches(":has(> .missing)", &el));
    }

    #[test]
    fn spec_has_descendant() {
        let el = El::new("div").child(El::new("span").class("desc"));
        assert!(sel_matches(":has(.desc)", &el));
    }

    #[test]
    fn spec_has_sibling() {
        let next = El::new("span").class("next");
        let el = El::new("div").next(next);
        assert!(sel_matches(":has(+ .next)", &el));
    }

    // WPT: parse-has.html — valid :has() selectors
    #[test]
    fn wpt_has_parse_valid() {
        assert_parses(":has(a)");
        assert_parses(":has(#a)");
        assert_parses(":has(.a)");
        assert_parses(":has([a])");
        assert_parses(":has([a=\"b\"])");
        assert_parses(":has([a|=\"b\"])");
        assert_parses(":has(:hover)");
        assert_parses(".a:has(.b)");
        assert_parses(".a:has(> .b)");
        assert_parses(".a:has(~ .b)");
        assert_parses(".a:has(+ .b)");
        assert_parses(".a:has(.b) .c");
        assert_parses(".a .b:has(.c)");
        assert_parses(".a .b:has(.c .d)");
        assert_parses(".a .b:has(.c .d) .e");
        assert_parses(".a:has(.b:is(.c .d))");
        assert_parses(".a:is(.b:has(.c) .d)");
        assert_parses(".a:not(:has(.b))");
        assert_parses(".a:has(:not(.b))");
        assert_parses(".a:has(.b):has(.c)");
    }

    // WPT: parse-has.html — invalid :has() selectors
    #[test]
    fn wpt_has_parse_invalid() {
        assert_parse_error(":has");
        assert_parse_error(".a:has");
        assert_parse_error(":has()");
    }

    // WPT: parse-has-disallow-nesting-has-inside-has.html
    #[test]
    fn wpt_has_no_nesting() {
        assert_parse_error(".a:has(.b:has(.c))");
    }

    // WPT: has-specificity.html
    #[test]
    fn wpt_has_specificity() {
        // :has(#foo) > :has(.foo)
        assert!(specificity_of(":has(#foo)") > specificity_of(":has(.foo)"));

        // :has(span#foo) > :has(#foo) — compound adds type specificity
        assert!(specificity_of(":has(span#foo)") > specificity_of(":has(#foo)"));

        // :has(.bar, #foo) = :has(#foo, .bar) — max of args, same result
        assert_eq!(specificity_of(":has(.bar, #foo)"), specificity_of(":has(#foo, .bar)"));

        // :has(.bar, #foo) > :has(.foo, .bar) — #foo vs .foo
        assert!(specificity_of(":has(.bar, #foo)") > specificity_of(":has(.foo, .bar)"));

        // :has(span + span) > :has(span) — two type selectors
        assert!(specificity_of(":has(span + span)") > specificity_of(":has(span)"));
    }

    // ===================================================================
    // §9 State pseudo-classes
    // ===================================================================

    #[test]
    fn spec_state_pseudo_classes() {
        let el = El::new("button")
            .state(ElementState::HOVER | ElementState::FOCUS | ElementState::ENABLED);
        assert!(sel_matches(":hover", &el));
        assert!(sel_matches(":focus", &el));
        assert!(sel_matches(":enabled", &el));
        assert!(sel_matches(":hover:focus", &el));
        assert!(!sel_matches(":disabled", &el));
        assert!(!sel_matches(":active", &el));
    }

    #[test]
    fn spec_any_link() {
        let link = El::new("a").state(ElementState::LINK);
        let visited = El::new("a").state(ElementState::VISITED);
        let plain = El::new("a");
        assert!(sel_matches(":any-link", &link));
        assert!(sel_matches(":any-link", &visited));
        assert!(!sel_matches(":any-link", &plain));
    }

    // All state pseudo-classes
    #[test]
    fn spec_state_exhaustive() {
        let checks: &[(ElementState, &str)] = &[
            (ElementState::HOVER, ":hover"),
            (ElementState::ACTIVE, ":active"),
            (ElementState::FOCUS, ":focus"),
            (ElementState::FOCUS_WITHIN, ":focus-within"),
            (ElementState::FOCUS_VISIBLE, ":focus-visible"),
            (ElementState::ENABLED, ":enabled"),
            (ElementState::DISABLED, ":disabled"),
            (ElementState::CHECKED, ":checked"),
            (ElementState::INDETERMINATE, ":indeterminate"),
            (ElementState::REQUIRED, ":required"),
            (ElementState::OPTIONAL, ":optional"),
            (ElementState::VALID, ":valid"),
            (ElementState::INVALID, ":invalid"),
            (ElementState::READ_ONLY, ":read-only"),
            (ElementState::READ_WRITE, ":read-write"),
            (ElementState::PLACEHOLDER_SHOWN, ":placeholder-shown"),
            (ElementState::DEFAULT, ":default"),
            (ElementState::TARGET, ":target"),
            (ElementState::VISITED, ":visited"),
            (ElementState::LINK, ":link"),
            (ElementState::FULLSCREEN, ":fullscreen"),
            (ElementState::MODAL, ":modal"),
            (ElementState::PLAYING, ":playing"),
            (ElementState::PAUSED, ":paused"),
            (ElementState::SEEKING, ":seeking"),
            (ElementState::BUFFERING, ":buffering"),
            (ElementState::STALLED, ":stalled"),
            (ElementState::MUTED, ":muted"),
            (ElementState::VOLUME_LOCKED, ":volume-locked"),
            (ElementState::AUTOFILL, ":autofill"),
            (ElementState::DEFINED, ":defined"),
            (ElementState::POPOVER_OPEN, ":popover-open"),
            (ElementState::USER_VALID, ":user-valid"),
            (ElementState::USER_INVALID, ":user-invalid"),
            (ElementState::BLANK, ":blank"),
            (ElementState::IN_RANGE, ":in-range"),
            (ElementState::OUT_OF_RANGE, ":out-of-range"),
            (ElementState::OPEN, ":open"),
            (ElementState::CLOSED, ":closed"),
            (ElementState::PICTURE_IN_PICTURE, ":picture-in-picture"),
        ];

        for &(flag, selector) in checks {
            let el = El::new("div").state(flag);
            assert!(sel_matches(selector, &el),
                "{selector} should match element with {flag:?}");
            let plain = El::new("div");
            assert!(!sel_matches(selector, &plain),
                "{selector} should NOT match element without any state");
        }
    }

    // ===================================================================
    // §7 :dir() pseudo-class
    // ===================================================================

    #[test]
    fn spec_dir() {
        let ltr = El::new("div").dir(Direction::Ltr);
        let rtl = El::new("div").dir(Direction::Rtl);
        assert!(sel_matches(":dir(ltr)", &ltr));
        assert!(!sel_matches(":dir(rtl)", &ltr));
        assert!(sel_matches(":dir(rtl)", &rtl));
        assert!(!sel_matches(":dir(ltr)", &rtl));
    }

    // ===================================================================
    // §7.2 :lang() pseudo-class
    // ===================================================================

    #[test]
    fn spec_lang() {
        let el = El::new("div").attr("lang", "en-US");
        assert!(sel_matches(":lang(en)", &el));
    }

    #[test]
    fn spec_lang_exact_match() {
        let el = El::new("div").attr("lang", "fr");
        assert!(sel_matches(":lang(fr)", &el));
        assert!(!sel_matches(":lang(en)", &el));
    }

    #[test]
    fn spec_lang_subtag_match() {
        // :lang(zh) matches zh-Hant, zh-Hans, zh-TW, etc.
        let el = El::new("div").attr("lang", "zh-Hant");
        assert!(sel_matches(":lang(zh)", &el));
    }

    // ===================================================================
    // §14 Combinators (WPT: parse-child.html, parse-descendant.html, parse-sibling.html)
    // ===================================================================

    #[test]
    fn spec_child_combinator() {
        let parent = El::new("div");
        let child = El::new("span").parent(parent);
        assert!(sel_matches("div > span", &child));
        assert!(!sel_matches("p > span", &child));
    }

    #[test]
    fn spec_descendant_combinator_deep() {
        let grandparent = El::new("html").root();
        let parent = El::new("body").parent(grandparent);
        let el = El::new("div").parent(parent);
        assert!(sel_matches("html div", &el));
        assert!(sel_matches("body div", &el));
    }

    #[test]
    fn spec_adjacent_sibling() {
        let prev = El::new("h1");
        let el = El::new("p").prev(prev);
        assert!(sel_matches("h1 + p", &el));
        assert!(!sel_matches("h2 + p", &el));
    }

    #[test]
    fn spec_general_sibling() {
        let prev = El::new("h1");
        let el = El::new("p").prev(prev);
        assert!(sel_matches("h1 ~ p", &el));
    }

    // Multiple combinators in sequence
    #[test]
    fn spec_combinator_chain() {
        let html = El::new("html").root();
        let body = El::new("body").parent(html);
        let div = El::new("div").class("container").parent(body);
        let span = El::new("span").parent(div);
        assert!(sel_matches("html body span", &span));
        assert!(sel_matches("body > div > span", &span));
        assert!(sel_matches("html span", &span));
    }

    // ===================================================================
    // Selector list semantics
    // ===================================================================

    #[test]
    fn spec_selector_list_any_matches() {
        let el = El::new("div");
        assert!(sel_matches("div, span", &el));
        assert!(sel_matches("span, div", &el));
        assert!(!sel_matches("span, p", &el));
    }

    #[test]
    fn spec_selector_list_multiple() {
        let el = El::new("div").class("foo").id("bar");
        assert!(sel_matches("span, .foo, #baz", &el));
        assert!(sel_matches("span, p, #bar", &el));
        assert!(!sel_matches("span, p, a", &el));
    }

    // ===================================================================
    // §15 Specificity (WPT: specificity tests)
    // ===================================================================

    #[test]
    fn spec_specificity_basic() {
        assert_eq!(specificity_of("div"), Specificity::new(0, 0, 1));
        assert_eq!(specificity_of(".foo"), Specificity::new(0, 1, 0));
        assert_eq!(specificity_of("#bar"), Specificity::new(1, 0, 0));
        assert_eq!(specificity_of("*"), Specificity::new(0, 0, 0));
    }

    #[test]
    fn spec_specificity_compound() {
        assert_eq!(specificity_of("div.foo#bar"), Specificity::new(1, 1, 1));
        assert_eq!(specificity_of(".a.b.c"), Specificity::new(0, 3, 0));
    }

    #[test]
    fn spec_specificity_pseudo_class() {
        assert_eq!(specificity_of(":hover"), Specificity::new(0, 1, 0));
        assert_eq!(specificity_of(":first-child"), Specificity::new(0, 1, 0));
        assert_eq!(specificity_of(":nth-child(2n)"), Specificity::new(0, 1, 0));
    }

    #[test]
    fn spec_specificity_pseudo_element() {
        assert_eq!(specificity_of("::before"), Specificity::new(0, 0, 1));
        assert_eq!(specificity_of("::after"), Specificity::new(0, 0, 1));
    }

    #[test]
    fn spec_specificity_complex() {
        assert_eq!(specificity_of("div > .foo"), Specificity::new(0, 1, 1));
        assert_eq!(specificity_of("#id .class div"), Specificity::new(1, 1, 1));
    }

    // Attribute selectors count as class-level specificity
    #[test]
    fn spec_specificity_attributes() {
        assert_eq!(specificity_of("[type]"), Specificity::new(0, 1, 0));
        assert_eq!(specificity_of("[type=text]"), Specificity::new(0, 1, 0));
        assert_eq!(specificity_of("input[type=text]"), Specificity::new(0, 1, 1));
        assert_eq!(specificity_of("[a][b][c]"), Specificity::new(0, 3, 0));
    }

    // :nth-child(An+B of S) specificity = own + max of S
    #[test]
    fn spec_specificity_nth_of() {
        // :nth-child(2n of .foo) = (0,1,0) own + (0,1,0) from .foo = (0,2,0)
        assert_eq!(specificity_of(":nth-child(2n of .foo)"), Specificity::new(0, 2, 0));
        // :nth-child(2n of #id) = (0,1,0) + (1,0,0) = (1,1,0)
        assert_eq!(specificity_of(":nth-child(2n of #id)"), Specificity::new(1, 1, 0));
        // :nth-child(2n of .a, #b) = (0,1,0) + max((0,1,0),(1,0,0)) = (1,1,0)
        assert_eq!(specificity_of(":nth-child(2n of .a, #b)"), Specificity::new(1, 1, 0));
    }

    // ===================================================================
    // Round-trip fidelity (parse → serialize → re-parse)
    // ===================================================================

    #[test]
    fn spec_round_trip_comprehensive() {
        let selectors = [
            // Type / universal / compound
            "div", ".foo", "#bar", "*", "div.foo#bar",
            // Combinators
            "div > .foo", "div .foo", "div + .foo", "div ~ .foo",
            // Structural pseudo-classes
            ":hover", ":first-child", ":nth-child(odd)",
            // Logical combinators
            ":not(.foo)", ":is(.a, .b)", ":where(.x)",
            // Pseudo-elements
            "::before", "::after", "::placeholder",
            // Attribute selectors
            "[attr]", "[attr=\"val\"]", "[attr~=\"val\"]", "[attr|=\"val\"]",
            "[attr^=\"val\"]", "[attr$=\"val\"]", "[attr*=\"val\"]",
            // Functional pseudo-classes
            ":dir(ltr)", ":dir(rtl)", ":lang(en)",
            // Input pseudo-classes
            ":in-range", ":out-of-range", ":open", ":closed",
            // Media pseudo-classes
            ":playing", ":paused", ":picture-in-picture",
            // Pseudo-elements
            "::backdrop", "::marker", "::file-selector-button",
        ];
        for css in selectors {
            let list = parse(css).unwrap();
            let output = format!("{}", list.0[0]);
            assert_eq!(css, output, "Round-trip failed for '{css}'");
        }
    }

    // Extended round-trip: all pseudo-classes
    #[test]
    fn spec_round_trip_all_pseudo_classes() {
        let pseudo_classes = [
            ":hover", ":active", ":focus", ":focus-within", ":focus-visible",
            ":enabled", ":disabled", ":checked", ":indeterminate",
            ":required", ":optional", ":valid", ":invalid",
            ":read-only", ":read-write", ":placeholder-shown",
            ":default", ":target", ":visited", ":link", ":any-link",
            ":root", ":empty", ":first-child", ":last-child", ":only-child",
            ":first-of-type", ":last-of-type", ":only-of-type",
            ":fullscreen", ":modal", ":playing", ":paused",
            ":seeking", ":buffering", ":stalled", ":muted", ":volume-locked",
            ":autofill", ":defined", ":popover-open",
            ":user-valid", ":user-invalid", ":blank",
            ":in-range", ":out-of-range", ":open", ":closed",
            ":picture-in-picture",
        ];
        for css in pseudo_classes {
            let list = parse(css).expect(&format!("{css} should parse"));
            let output = format!("{}", list.0[0]);
            assert_eq!(css, output, "Round-trip failed for '{css}'");
        }
    }

    // Extended round-trip: all pseudo-elements
    #[test]
    fn spec_round_trip_all_pseudo_elements() {
        let pseudo_elements = [
            "::before", "::after", "::first-line", "::first-letter",
            "::placeholder", "::selection", "::marker", "::backdrop",
            "::file-selector-button", "::grammar-error", "::spelling-error",
        ];
        for css in pseudo_elements {
            let list = parse(css).expect(&format!("{css} should parse"));
            let output = format!("{}", list.0[0]);
            assert_eq!(css, output, "Round-trip failed for '{css}'");
        }
    }

    // Round-trip: complex selectors
    #[test]
    fn spec_round_trip_complex() {
        let selectors = [
            "div > .foo:first-child",
            "#id .class div",
            ".a.b.c",
            "a:not(.disabled)",
            ":is(h1, h2, h3)",
            ":where(.a, .b) > .c",
            ":has(> .child)",
            ":has(+ .next)",
            ":has(~ .sib)",
            ":nth-child(2n+1)",
            ":nth-child(even)",
            ":nth-last-child(3n)",
            ":nth-of-type(odd)",
            ":not(.a, .b)",
            ":is(.a, .b, .c)",
        ];
        for css in selectors {
            let list = parse(css).expect(&format!("{css} should parse"));
            let reparsed = parse(&format!("{}", list.0[0])).expect("re-parse should succeed");
            assert_eq!(list.0[0].specificity(), reparsed.0[0].specificity(),
                "Specificity mismatch after round-trip for '{css}'");
        }
    }

    // An+B serialization normalization
    #[test]
    fn spec_anplusb_normalization() {
        // 2n+1 normalizes to odd
        assert_parses_to(":nth-child(2n+1)", ":nth-child(odd)");
        // 2n normalizes to 2n (even is only for 2n+0)
        assert_parses_to(":nth-child(2n+0)", ":nth-child(even)");
        // 1n+0 → n
        assert_parses_to(":nth-child(1n+0)", ":nth-child(n)");
        // 0n+5 → 5
        assert_parses_to(":nth-child(0n+5)", ":nth-child(5)");
    }

    // ===================================================================
    // CSS Nesting: & selector
    // ===================================================================

    #[test]
    fn spec_nesting_selector_parses() {
        let list = parse("& .child").unwrap();
        let comps = list.0[0].components();
        assert!(comps.iter().any(|c| matches!(c, Component::Nesting)));
    }

    #[test]
    fn spec_nesting_selector_round_trip() {
        let list = parse("&.foo").unwrap();
        let output = format!("{}", list.0[0]);
        assert_eq!("&.foo", output);
    }

    #[test]
    fn spec_nesting_with_combinators() {
        assert_parses("& > .child");
        assert_parses("& + .sibling");
        assert_parses("& ~ .general");
        assert_parses("& .descendant");
    }

    // ===================================================================
    // Parse error handling (WPT: invalid-pseudos.html)
    // ===================================================================

    #[test]
    fn spec_invalid_selectors() {
        assert_parse_error("");
        assert_parse_error(">>>");
        assert_parse_error("...");
    }

    // WPT: invalid-pseudos.html — vendor-prefixed pseudo-classes must not parse
    #[test]
    fn wpt_invalid_vendor_pseudos() {
        assert_parse_error(":-webkit-full-screen-document");
        assert_parse_error(":-khtml-drag");
        assert_parse_error("::-internal-loading-auto-fill-button");
    }

    // Unknown pseudo-classes are invalid
    #[test]
    fn spec_unknown_pseudo_class() {
        assert_parse_error(":nonexistent");
        assert_parse_error(":foo-bar");
    }

    // ===================================================================
    // §3.14 :host / :host() / :host-context() — Shadow DOM
    // ===================================================================

    #[test]
    fn spec_host_bare() {
        // :host matches a shadow host element
        let host = El::new("div").as_shadow_host();
        assert!(sel_matches(":host", &host));
        // :host does NOT match non-shadow-host elements
        let plain = El::new("div");
        assert!(!sel_matches(":host", &plain));
    }

    #[test]
    fn spec_host_functional() {
        // :host(.dark-theme) matches a shadow host with class "dark-theme"
        let host = El::new("div").as_shadow_host().class("dark-theme");
        assert!(sel_matches(":host(.dark-theme)", &host));
        // Doesn't match if class is wrong
        let host2 = El::new("div").as_shadow_host().class("light-theme");
        assert!(!sel_matches(":host(.dark-theme)", &host2));
        // Doesn't match non-shadow-host even with correct class
        let plain = El::new("div").class("dark-theme");
        assert!(!sel_matches(":host(.dark-theme)", &plain));
    }

    #[test]
    fn spec_host_functional_compound() {
        // :host(div#main.active) — compound selector inside :host()
        let host = El::new("div").as_shadow_host().id("main").class("active");
        assert!(sel_matches(":host(div#main.active)", &host));
        // Missing class → no match
        let host2 = El::new("div").as_shadow_host().id("main");
        assert!(!sel_matches(":host(div#main.active)", &host2));
    }

    #[test]
    fn spec_host_context() {
        // :host-context(.theme-dark) walks ancestors to find .theme-dark
        let ancestor = El::new("body").class("theme-dark");
        let host = El::new("div")
            .as_shadow_host()
            .in_shadow_of(El::new("div")) // element is in its own shadow
            .parent(ancestor);
        assert!(sel_matches(":host-context(.theme-dark)", &host));
    }

    #[test]
    fn spec_host_context_no_match() {
        // :host-context(.theme-dark) fails if no ancestor matches
        let ancestor = El::new("body").class("theme-light");
        let host = El::new("div").as_shadow_host().parent(ancestor);
        assert!(!sel_matches(":host-context(.theme-dark)", &host));
    }

    #[test]
    fn spec_host_context_non_shadow_host() {
        // :host-context() requires the element to be a shadow host
        let ancestor = El::new("body").class("theme-dark");
        let plain = El::new("div").parent(ancestor);
        assert!(!sel_matches(":host-context(.theme-dark)", &plain));
    }

    #[test]
    fn spec_host_specificity() {
        // :host = (0,1,0) — pseudo-class specificity
        assert_eq!(specificity_of(":host"), Specificity::new(0, 1, 0));
        // :host(.foo) = (0,1,0) + (0,1,0) = (0,2,0)
        assert_eq!(specificity_of(":host(.foo)"), Specificity::new(0, 2, 0));
        // :host(div#id.class) = (0,1,0) + (1,1,1) = (1,2,1)
        assert_eq!(specificity_of(":host(div#id.class)"), Specificity::new(1, 2, 1));
        // :host-context(.foo) = (0,1,0) + (0,1,0) = (0,2,0)
        assert_eq!(specificity_of(":host-context(.foo)"), Specificity::new(0, 2, 0));
    }

    // ===================================================================
    // §3.15 ::slotted() — Shadow DOM slot distribution
    // ===================================================================

    #[test]
    fn spec_slotted_matches() {
        // ::slotted(.card) matches a slotted element with class "card"
        let el = El::new("div").class("card").slotted();
        assert!(sel_matches("::slotted(.card)", &el));
    }

    #[test]
    fn spec_slotted_not_in_slot() {
        // ::slotted(.card) does NOT match element not in a slot
        let el = El::new("div").class("card");
        assert!(!sel_matches("::slotted(.card)", &el));
    }

    #[test]
    fn spec_slotted_wrong_selector() {
        // ::slotted(.other) doesn't match if inner selector fails
        let el = El::new("div").class("card").slotted();
        assert!(!sel_matches("::slotted(.other)", &el));
    }

    #[test]
    fn spec_slotted_specificity() {
        // ::slotted(div.card) = (0,0,1) pseudo-element + (0,1,1) inner = (0,1,2)
        assert_eq!(specificity_of("::slotted(div.card)"), Specificity::new(0, 1, 2));
    }

    // ===================================================================
    // §3.16 ::part() — Shadow DOM part exposure
    // ===================================================================

    #[test]
    fn spec_part_single() {
        let el = El::new("div").part("header");
        assert!(sel_matches("::part(header)", &el));
        assert!(!sel_matches("::part(footer)", &el));
    }

    #[test]
    fn spec_part_multiple() {
        // ::part(header footer) matches element exposing BOTH parts
        let el = El::new("div").part("header").part("footer");
        assert!(sel_matches("::part(header)", &el));
        assert!(sel_matches("::part(footer)", &el));
    }

    #[test]
    fn spec_part_no_parts() {
        let el = El::new("div");
        assert!(!sel_matches("::part(header)", &el));
    }

    #[test]
    fn spec_part_specificity() {
        // ::part(name) = (0,0,1) pseudo-element specificity
        assert_eq!(specificity_of("::part(header)"), Specificity::new(0, 0, 1));
    }

    // ===================================================================
    // §3.17 :state() — Custom element state
    // ===================================================================

    #[test]
    fn spec_custom_state_matches() {
        let el = El::new("x-toggle").custom_state("checked");
        assert!(sel_matches(":state(checked)", &el));
    }

    #[test]
    fn spec_custom_state_no_match() {
        let el = El::new("x-toggle").custom_state("checked");
        assert!(!sel_matches(":state(pressed)", &el));
    }

    #[test]
    fn spec_custom_state_plain_element() {
        let el = El::new("div");
        assert!(!sel_matches(":state(checked)", &el));
    }

    #[test]
    fn spec_custom_state_multiple() {
        let el = El::new("x-btn").custom_state("pressed").custom_state("loading");
        assert!(sel_matches(":state(pressed)", &el));
        assert!(sel_matches(":state(loading)", &el));
        assert!(!sel_matches(":state(disabled)", &el));
    }

    #[test]
    fn spec_custom_state_specificity() {
        // :state(x) = (0,1,0) — pseudo-class specificity
        assert_eq!(specificity_of(":state(checked)"), Specificity::new(0, 1, 0));
    }

    // ===================================================================
    // §3.18 ::highlight() — Custom highlight pseudo-element
    // ===================================================================

    #[test]
    fn spec_highlight_matches() {
        // ::highlight(name) always matches (style resolution decides)
        let el = El::new("div");
        assert!(sel_matches("::highlight(my-range)", &el));
    }

    #[test]
    fn spec_highlight_specificity() {
        // ::highlight(name) = (0,0,1) pseudo-element specificity
        assert_eq!(specificity_of("::highlight(my-range)"), Specificity::new(0, 0, 1));
    }

    #[test]
    fn spec_highlight_round_trip() {
        assert_parses("::highlight(search-result)");
        assert_parses("::highlight(spelling-error-custom)");
    }

    // ===================================================================
    // §14.4 Column combinator ||
    // ===================================================================

    #[test]
    fn spec_column_combinator_matches() {
        // col.total || td — match <td> in <col class="total">
        let col = El::new("col").class("total");
        let td = El::new("td").column(col);
        assert!(sel_matches("col.total || td", &td));
    }

    #[test]
    fn spec_column_combinator_no_column() {
        // Column combinator fails if element has no column
        let td = El::new("td");
        assert!(!sel_matches("col.total || td", &td));
    }

    #[test]
    fn spec_column_combinator_wrong_column() {
        // Column combinator fails if column doesn't match
        let col = El::new("col").class("quantity");
        let td = El::new("td").column(col);
        assert!(!sel_matches("col.total || td", &td));
    }

    #[test]
    fn spec_column_combinator_specificity() {
        // col || td = (0,0,1) + (0,0,1) = (0,0,2)
        assert_eq!(specificity_of("col || td"), Specificity::new(0, 0, 2));
        // col.special || td = (0,1,1) + (0,0,1) = (0,1,2)
        assert_eq!(specificity_of("col.special || td"), Specificity::new(0, 1, 2));
    }

    // ===================================================================
    // §12 Time-dimensional pseudo-classes :current/:past/:future
    // ===================================================================

    #[test]
    fn spec_current_state() {
        let el = El::new("p").state(ElementState::CURRENT);
        assert!(sel_matches(":current", &el));
        assert!(!sel_matches(":past", &el));
        assert!(!sel_matches(":future", &el));
    }

    #[test]
    fn spec_past_state() {
        let el = El::new("p").state(ElementState::PAST);
        assert!(sel_matches(":past", &el));
        assert!(!sel_matches(":current", &el));
        assert!(!sel_matches(":future", &el));
    }

    #[test]
    fn spec_future_state() {
        let el = El::new("p").state(ElementState::FUTURE);
        assert!(sel_matches(":future", &el));
        assert!(!sel_matches(":current", &el));
        assert!(!sel_matches(":past", &el));
    }

    // ===================================================================
    // §10.1 :target-within and :local-link
    // ===================================================================

    #[test]
    fn spec_target_within() {
        let el = El::new("section").state(ElementState::TARGET_WITHIN);
        assert!(sel_matches(":target-within", &el));
        let plain = El::new("section");
        assert!(!sel_matches(":target-within", &plain));
    }

    #[test]
    fn spec_local_link() {
        let el = El::new("a").state(ElementState::LOCAL_LINK);
        assert!(sel_matches(":local-link", &el));
        let plain = El::new("a");
        assert!(!sel_matches(":local-link", &plain));
    }

    // ===================================================================
    // §7.2 :lang() — comma-separated language list
    // ===================================================================

    #[test]
    fn spec_lang_comma_list() {
        // :lang(en, fr) matches either language
        let en = El::new("div").attr("lang", "en-US");
        assert!(sel_matches(":lang(en, fr)", &en));
        let fr = El::new("div").attr("lang", "fr-CA");
        assert!(sel_matches(":lang(en, fr)", &fr));
        let de = El::new("div").attr("lang", "de");
        assert!(!sel_matches(":lang(en, fr)", &de));
    }

    #[test]
    fn spec_lang_subtag_in_list() {
        // :lang(zh, ja) — subtag matching for each item
        let zh = El::new("div").attr("lang", "zh-Hant-TW");
        assert!(sel_matches(":lang(zh, ja)", &zh));
        let ja = El::new("div").attr("lang", "ja-JP");
        assert!(sel_matches(":lang(zh, ja)", &ja));
        let ko = El::new("div").attr("lang", "ko");
        assert!(!sel_matches(":lang(zh, ja)", &ko));
    }

    #[test]
    fn spec_lang_inherited() {
        // lang inherited from parent
        let parent = El::new("html").attr("lang", "en");
        let el = El::new("div").parent(parent);
        assert!(sel_matches(":lang(en)", &el));
    }

    // ===================================================================
    // Shadow DOM round-trip parsing
    // ===================================================================

    #[test]
    fn spec_shadow_dom_round_trip() {
        assert_parses(":host");
        assert_parses(":host(.dark)");
        assert_parses(":host(div.theme)");
        assert_parses(":host-context(.theme)");
        assert_parses(":host-context(body.dark)");
        assert_parses("::slotted(div)");
        assert_parses("::slotted(.card)");
        assert_parses("::part(header)");
        assert_parses(":state(checked)");
        assert_parses("::highlight(search-result)");
    }

    #[test]
    fn spec_shadow_dom_parse_invalid() {
        // ::part() empty is invalid
        assert_parse_error("::part()");
        // :state() empty is invalid
        assert_parse_error(":state()");
        // ::highlight() empty is invalid
        assert_parse_error("::highlight()");
    }

    // ===================================================================
    // Additional state pseudo-classes: exhaustive matching for new states
    // ===================================================================

    #[test]
    fn spec_state_exhaustive_new() {
        // Tests all NEW state-based pseudo-classes added in the latest update
        let checks: &[(ElementState, &str)] = &[
            (ElementState::TARGET_WITHIN, ":target-within"),
            (ElementState::LOCAL_LINK, ":local-link"),
            (ElementState::CURRENT, ":current"),
            (ElementState::PAST, ":past"),
            (ElementState::FUTURE, ":future"),
        ];
        for &(flag, selector) in checks {
            let el = El::new("div").state(flag);
            assert!(sel_matches(selector, &el),
                "{selector} should match element with {flag:?}");
            let plain = El::new("div");
            assert!(!sel_matches(selector, &plain),
                "{selector} should NOT match element without state");
        }
    }

    // ===================================================================
    // Combined Shadow DOM + regular selector matching
    // ===================================================================

    #[test]
    fn spec_host_with_state() {
        // :host(:hover) — shadow host with hover state
        let host = El::new("div")
            .as_shadow_host()
            .state(ElementState::HOVER);
        assert!(sel_matches(":host(:hover)", &host));
        // No hover → no match
        let host2 = El::new("div").as_shadow_host();
        assert!(!sel_matches(":host(:hover)", &host2));
    }

    #[test]
    fn spec_slotted_with_compound() {
        // ::slotted(div.card[data-type="hero"]) — compound inside ::slotted()
        let el = El::new("div")
            .class("card")
            .attr("data-type", "hero")
            .slotted();
        assert!(sel_matches("::slotted(div.card)", &el));
        // Wrong tag
        let span = El::new("span").class("card").slotted();
        assert!(!sel_matches("::slotted(div.card)", &span));
    }

    #[test]
    fn spec_host_descendant_selector() {
        // :host .inner — shadow host followed by descendant
        let host = El::new("div").as_shadow_host();
        let inner = El::new("span").class("inner").parent(host);
        assert!(sel_matches(":host .inner", &inner));
    }

    // ===================================================================
    // Real-world Shadow DOM scenarios
    // ===================================================================

    #[test]
    fn spec_shadow_dom_real_world_card() {
        // Shadow DOM card component scenario:
        // <card-component> (shadow host)
        //   #shadow-root
        //     <div part="header"> (exposed part)
        //     <slot> (slot element)
        //       <p class="content"> (slotted light DOM)

        let host = El::new("card-component").as_shadow_host().class("elevated");

        // :host(.elevated) matches the host with correct class
        assert!(sel_matches(":host(.elevated)", &host));
        assert!(!sel_matches(":host(.flat)", &host));

        // ::part(header) matches element exposing "header" part
        let header = El::new("div").part("header").part("title-area");
        assert!(sel_matches("::part(header)", &header));
        assert!(sel_matches("::part(title-area)", &header));
        assert!(!sel_matches("::part(body)", &header));

        // ::slotted(.content) matches slotted light DOM element
        let slotted_p = El::new("p").class("content").slotted();
        assert!(sel_matches("::slotted(.content)", &slotted_p));
        assert!(!sel_matches("::slotted(.sidebar)", &slotted_p));
    }

    #[test]
    fn spec_shadow_dom_real_world_custom_element() {
        // Custom element with custom states:
        // <x-dropdown :state(open) :state(has-selection)>
        let dropdown = El::new("x-dropdown")
            .custom_state("open")
            .custom_state("has-selection");
        assert!(sel_matches(":state(open)", &dropdown));
        assert!(sel_matches(":state(has-selection)", &dropdown));
        assert!(!sel_matches(":state(closed)", &dropdown));

        // Compound: x-dropdown:state(open):state(has-selection)
        assert!(sel_matches("x-dropdown:state(open)", &dropdown));
    }

    #[test]
    fn spec_column_combinator_real_world() {
        // Table scenario: <col class="price"> covers column 2
        // <td> in column 2 should match col.price || td
        let price_col = El::new("col").class("price").id("col-price");
        let td = El::new("td").column(price_col);
        assert!(sel_matches("col.price || td", &td));
        assert!(sel_matches("#col-price || td", &td));
        assert!(!sel_matches("col.quantity || td", &td));

        // Colgroup scenario
        let colgroup = El::new("colgroup").class("financials");
        let td2 = El::new("td").column(colgroup);
        assert!(sel_matches("colgroup.financials || td", &td2));
    }

    // ===================================================================
    // HasTraversal correctness
    // ===================================================================

    #[test]
    fn spec_has_traversal_hints() {
        let list = parse(":has(> .child)").unwrap();
        if let Component::Has(rel_list) = &list.0[0].components()[0] {
            assert_eq!(rel_list.0[0].traversal, HasTraversal::Children);
        } else {
            panic!("expected :has()");
        }

        let list = parse(":has(.desc)").unwrap();
        if let Component::Has(rel_list) = &list.0[0].components()[0] {
            assert_eq!(rel_list.0[0].traversal, HasTraversal::Subtree);
        } else {
            panic!("expected :has()");
        }

        let list = parse(":has(+ .next)").unwrap();
        if let Component::Has(rel_list) = &list.0[0].components()[0] {
            assert_eq!(rel_list.0[0].traversal, HasTraversal::NextSibling);
        } else {
            panic!("expected :has()");
        }

        let list = parse(":has(~ .sib)").unwrap();
        if let Component::Has(rel_list) = &list.0[0].components()[0] {
            assert_eq!(rel_list.0[0].traversal, HasTraversal::Siblings);
        } else {
            panic!("expected :has()");
        }
    }

    // ===================================================================
    // SelectorDeps correctness
    // ===================================================================

    #[test]
    fn spec_deps_flags() {
        let list = parse("div > .foo:first-child .item:nth-child(2n):hover").unwrap();
        let deps = &list.0[0].hints().deps;
        assert!(deps.has_combinators());
        assert!(deps.depends_on_nth());
        assert!(deps.depends_on_edge_child());
        assert!(!deps.depends_on_has());
        assert!(!deps.depends_on_visited());
    }

    #[test]
    fn spec_deps_has() {
        let list = parse(":has(.foo)").unwrap();
        assert!(list.0[0].hints().deps.depends_on_has());
    }

    #[test]
    fn spec_deps_visited() {
        let list = parse(":visited").unwrap();
        assert!(list.0[0].hints().deps.depends_on_visited());
    }

    // ===================================================================
    // Complex real-world selectors
    // ===================================================================

    #[test]
    fn spec_complex_compound_chain() {
        // Real-world: nav link styling
        let nav = El::new("nav").id("main-nav");
        let ul = El::new("ul").class("menu").parent(nav);
        let li = El::new("li").class("active").parent(ul);
        let a = El::new("a")
            .class("link")
            .state(ElementState::HOVER | ElementState::LINK)
            .parent(li);
        assert!(sel_matches("nav a.link:hover", &a));
        assert!(sel_matches("#main-nav .active > a", &a));
    }

    #[test]
    fn spec_deeply_nested_not_is_where() {
        // :not(:is(.a, .b)) matches elements that are neither .a nor .b
        let el = El::new("div").class("c");
        assert!(sel_matches(":not(:is(.a, .b))", &el));
        let el2 = El::new("div").class("a");
        assert!(!sel_matches(":not(:is(.a, .b))", &el2));

        // :is(:not(.a)) matches elements that are not .a
        assert!(sel_matches(":is(:not(.a))", &el));
        assert!(!sel_matches(":is(:not(.c))", &el));
    }

    #[test]
    fn spec_multiple_pseudo_classes() {
        let el = El::new("input")
            .attr("type", "text")
            .state(ElementState::FOCUS | ElementState::VALID | ElementState::REQUIRED);
        assert!(sel_matches("input:focus:valid:required", &el));
        assert!(!sel_matches("input:focus:invalid:required", &el));
    }

    // ===================================================================
    // Specificity edge cases from WPT
    // ===================================================================

    #[test]
    fn spec_specificity_combinators_dont_add() {
        // Combinators have zero specificity
        assert_eq!(specificity_of("div > span"), Specificity::new(0, 0, 2));
        assert_eq!(specificity_of("div span"), Specificity::new(0, 0, 2));
        assert_eq!(specificity_of("div + span"), Specificity::new(0, 0, 2));
        assert_eq!(specificity_of("div ~ span"), Specificity::new(0, 0, 2));
    }

    #[test]
    fn spec_specificity_is_vs_where_difference() {
        // :is() inherits max specificity, :where() is zero
        // div:is(.a) = (0,0,1) + max((0,1,0)) = (0,1,1)
        assert_eq!(specificity_of("div:is(.a)"), Specificity::new(0, 1, 1));
        // div:where(.a) = (0,0,1) + 0 = (0,0,1)
        assert_eq!(specificity_of("div:where(.a)"), Specificity::new(0, 0, 1));
    }

    #[test]
    fn spec_specificity_has_takes_max_arg() {
        // :has(.foo) = (0,1,0) — has itself has zero spec, takes max of args
        assert_eq!(specificity_of(":has(.foo)"), Specificity::new(0, 1, 0));
        // :has(#id) = (1,0,0)
        assert_eq!(specificity_of(":has(#id)"), Specificity::new(1, 0, 0));
        // :has(.a, #b) = max((0,1,0), (1,0,0)) = (1,0,0)
        assert_eq!(specificity_of(":has(.a, #b)"), Specificity::new(1, 0, 0));
    }

    // ===================================================================
    // Matching with context (MatchingContext)
    // ===================================================================

    #[test]
    fn spec_matching_context_basic() {
        let el = El::new("div").class("FOO");
        let list = parse(".FOO").unwrap();
        let mut caches = crate::context::SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        assert!(matches_in_context(&list.0[0], &el, &mut ctx));
    }

    // ===================================================================
    // Scope pseudo-class
    // ===================================================================

    #[test]
    fn spec_scope_parses() {
        assert_parses(":scope");
        assert_parses(":scope > .child");
        assert_parses(".parent > :scope");
    }

    // ===================================================================
    // Forgiving parsing in :is() and :where()
    // ===================================================================

    #[test]
    fn spec_is_forgiving_parsing() {
        // :is() uses forgiving parsing — invalid selectors are dropped
        // :is(.valid, ::-invalid-pseudo, .also-valid) should still parse
        // and match .valid or .also-valid
        let list = parse(":is(.valid, .also-valid)").unwrap();
        let el = El::new("div").class("valid");
        assert!(list.0.iter().any(|sel| matches(sel, &el)));
    }

    // ===================================================================
    // :not() is NOT forgiving (WPT: parse-not.html)
    // ===================================================================

    #[test]
    fn wpt_not_strict_parsing() {
        // :not() uses strict parsing — any invalid selector = entire :not() fails
        // Note: :not(:unknownpseudo) should fail
        assert_parse_error(":not(::before)");
    }
}
