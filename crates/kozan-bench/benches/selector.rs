//! Benchmark: kozan-selector vs Servo's `selectors` crate.
//!
//! Tests parsing speed, matching speed, and rule-map lookup speed with
//! realistic CSS selectors and DOM structures.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

mod kozan {
    use kozan_atom::Atom;
    use kozan_selector::pseudo_class::ElementState;
    use kozan_selector::types::Direction;
    use kozan_selector::opaque::OpaqueElement;
    use kozan_selector::Element;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[derive(Clone)]
    pub struct El {
        pub tag: Atom,
        pub id: Option<Atom>,
        pub classes: Vec<Atom>,
        pub attrs: Vec<(Atom, String)>,
        pub state: ElementState,
        pub parent: Option<Box<El>>,
        pub children: Vec<El>,
        pub prev: Option<Box<El>>,
        pub next: Option<Box<El>>,
        pub index: u32,
        pub count: u32,
        pub is_root: bool,
        pub opaque_id: u64,
    }

    impl El {
        pub fn new(tag: &str) -> Self {
            Self {
                tag: Atom::from(tag),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
                state: ElementState::empty(),
                parent: None,
                children: Vec::new(),
                prev: None,
                next: None,
                index: 1,
                count: 1,
                is_root: false,
                opaque_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            }
        }
        pub fn with_id(mut self, id: &str) -> Self {
            self.id = Some(Atom::from(id));
            self
        }
        pub fn with_class(mut self, c: &str) -> Self {
            self.classes.push(Atom::from(c));
            self
        }
        pub fn with_parent(mut self, p: El) -> Self {
            self.parent = Some(Box::new(p));
            self
        }
        pub fn with_attr(mut self, name: &str, val: &str) -> Self {
            self.attrs.push((Atom::from(name), val.to_string()));
            self
        }
        pub fn prev(mut self, s: El) -> Self {
            self.prev = Some(Box::new(s));
            self
        }
        pub fn at_index(mut self, i: u32, c: u32) -> Self {
            self.index = i;
            self.count = c;
            self
        }
    }

    impl Element for El {
        fn local_name(&self) -> &Atom { &self.tag }
        fn id(&self) -> Option<&Atom> { self.id.as_ref() }
        fn has_class(&self, class: &Atom) -> bool { self.classes.iter().any(|c| c == class) }
        fn each_class<F: FnMut(&Atom)>(&self, mut f: F) {
            for c in &self.classes { f(c); }
        }
        fn attr(&self, name: &Atom) -> Option<&str> {
            self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
        }
        fn parent_element(&self) -> Option<Self> { self.parent.as_ref().map(|p| *p.clone()) }
        fn prev_sibling_element(&self) -> Option<Self> { self.prev.as_ref().map(|s| *s.clone()) }
        fn next_sibling_element(&self) -> Option<Self> { self.next.as_ref().map(|s| *s.clone()) }
        fn first_child_element(&self) -> Option<Self> { self.children.first().cloned() }
        fn last_child_element(&self) -> Option<Self> { self.children.last().cloned() }
        fn state(&self) -> ElementState { self.state }
        fn is_root(&self) -> bool { self.is_root }
        fn is_empty(&self) -> bool { self.children.is_empty() }
        fn child_index(&self) -> u32 { self.index }
        fn child_count(&self) -> u32 { self.count }
        fn child_index_of_type(&self) -> u32 { self.index }
        fn child_count_of_type(&self) -> u32 { self.count }
        fn opaque(&self) -> OpaqueElement { OpaqueElement::new(self.opaque_id) }
        fn direction(&self) -> Direction { Direction::Ltr }
    }
}

mod servo {
    use std::fmt;
    use string_cache::DefaultAtom as SAtom;
    use selectors::attr::{AttrSelectorOperation, NamespaceConstraint, CaseSensitivity};
    use selectors::context::{MatchingContext, MatchingMode, QuirksMode};
    use selectors::context::{NeedsSelectorFlags, MatchingForInvalidation};
    use selectors::matching::ElementSelectorFlags;
    use selectors::SelectorList;

    // -----------------------------------------------------------------------
    // SelectorImpl types — using string_cache::DefaultAtom for O(1) interned
    // pointer-equality comparison, exactly like Servo/Stylo in production.
    // -----------------------------------------------------------------------

    /// Wrapper around `string_cache::DefaultAtom` that adds cssparser::ToCss.
    /// In production Stylo, this role is played by `style::Atom` (also from
    /// string_cache). Comparison is O(1) pointer equality — same as kozan's
    /// `kozan_atom::Atom`.
    #[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
    pub struct Value(pub SAtom);

    impl From<&str> for Value {
        fn from(s: &str) -> Self { Value(SAtom::from(s)) }
    }

    impl AsRef<str> for Value {
        fn as_ref(&self) -> &str { &self.0 }
    }

    impl std::borrow::Borrow<str> for Value {
        fn borrow(&self) -> &str { &self.0 }
    }

    impl cssparser_0_36::ToCss for Value {
        fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
            dest.write_str(&self.0)
        }
    }

    impl precomputed_hash::PrecomputedHash for Value {
        fn precomputed_hash(&self) -> u32 {
            precomputed_hash::PrecomputedHash::precomputed_hash(&self.0)
        }
    }

    impl selectors::parser::NonTSPseudoClass for PseudoClass {
        type Impl = Impl;
        fn is_active_or_hover(&self) -> bool { false }
        fn is_user_action_state(&self) -> bool { false }
    }

    impl selectors::parser::PseudoElement for PseudoElement {
        type Impl = Impl;
    }

    impl selectors::parser::SelectorImpl for Impl {
        type ExtraMatchingData<'a> = ();
        type AttrValue = Value;
        type Identifier = Value;
        type LocalName = Value;
        type NamespaceUrl = Value;
        type NamespacePrefix = Value;
        type BorrowedLocalName = str;
        type BorrowedNamespaceUrl = str;
        type NonTSPseudoClass = PseudoClass;
        type PseudoElement = PseudoElement;
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Impl;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PseudoClass {}

    impl cssparser_0_36::ToCss for PseudoClass {
        fn to_css<W: fmt::Write>(&self, _: &mut W) -> fmt::Result { Ok(()) }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PseudoElement {}

    impl cssparser_0_36::ToCss for PseudoElement {
        fn to_css<W: fmt::Write>(&self, _: &mut W) -> fmt::Result { Ok(()) }
    }

    // -----------------------------------------------------------------------
    // Parser
    // -----------------------------------------------------------------------

    struct Parser;

    impl<'i> selectors::parser::Parser<'i> for Parser {
        type Impl = Impl;
        type Error = selectors::parser::SelectorParseErrorKind<'i>;

        fn parse_is_and_where(&self) -> bool { true }
        fn parse_has(&self) -> bool { true }
    }

    pub fn parse(css: &str) -> Result<SelectorList<Impl>, ()> {
        let mut input = cssparser_0_36::ParserInput::new(css);
        let mut parser = cssparser_0_36::Parser::new(&mut input);
        SelectorList::parse(
            &Parser,
            &mut parser,
            selectors::parser::ParseRelative::No,
        ).map_err(|_| ())
    }

    // -----------------------------------------------------------------------
    // Element impl — uses SAtom (interned) for tag/id/classes/attrs,
    // giving O(1) pointer-equality comparison like production Servo.
    // -----------------------------------------------------------------------

    #[derive(Debug, Clone)]
    pub struct El {
        pub tag: SAtom,
        pub id: Option<SAtom>,
        pub classes: Vec<SAtom>,
        pub attrs: Vec<(SAtom, String)>,
        pub parent: Option<Box<El>>,
        pub prev: Option<Box<El>>,
        pub next: Option<Box<El>>,
        pub children: Vec<El>,
        pub index: usize,
        pub count: usize,
        pub is_root: bool,
        opaque_id: usize,
    }

    static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

    impl El {
        pub fn new(tag: &str) -> Self {
            Self {
                tag: SAtom::from(tag),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
                parent: None,
                prev: None,
                next: None,
                children: Vec::new(),
                index: 1,
                count: 1,
                is_root: false,
                opaque_id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            }
        }
        pub fn with_id(mut self, id: &str) -> Self {
            self.id = Some(SAtom::from(id));
            self
        }
        pub fn with_class(mut self, c: &str) -> Self {
            self.classes.push(SAtom::from(c));
            self
        }
        pub fn with_attr(mut self, name: &str, val: &str) -> Self {
            self.attrs.push((SAtom::from(name), val.to_string()));
            self
        }
        pub fn with_parent(mut self, p: El) -> Self {
            self.parent = Some(Box::new(p));
            self
        }
        pub fn with_prev(mut self, s: El) -> Self {
            self.prev = Some(Box::new(s));
            self
        }
        pub fn at_index(mut self, i: usize, c: usize) -> Self {
            self.index = i;
            self.count = c;
            self
        }
    }

    impl selectors::Element for El {
        type Impl = Impl;

        fn opaque(&self) -> selectors::OpaqueElement {
            selectors::OpaqueElement::new(&self.opaque_id)
        }

        fn parent_element(&self) -> Option<Self> {
            self.parent.as_ref().map(|p| *p.clone())
        }

        fn parent_node_is_shadow_root(&self) -> bool { false }
        fn containing_shadow_host(&self) -> Option<Self> { None }
        fn is_pseudo_element(&self) -> bool { false }

        fn prev_sibling_element(&self) -> Option<Self> {
            self.prev.as_ref().map(|s| *s.clone())
        }

        fn next_sibling_element(&self) -> Option<Self> {
            self.next.as_ref().map(|s| *s.clone())
        }

        fn first_element_child(&self) -> Option<Self> {
            self.children.first().cloned()
        }

        fn is_html_element_in_html_document(&self) -> bool { true }

        fn has_local_name(&self, local_name: &str) -> bool {
            *self.tag == *local_name
        }

        fn has_namespace(&self, _ns: &str) -> bool { true }

        fn is_same_type(&self, other: &Self) -> bool {
            self.tag == other.tag  // O(1) pointer equality
        }

        fn attr_matches(
            &self,
            _ns: &NamespaceConstraint<&Value>,
            local_name: &Value,
            operation: &AttrSelectorOperation<&Value>,
        ) -> bool {
            self.attrs.iter().any(|(name, val)| {
                if *name != local_name.0 {
                    return false;
                }
                match operation {
                    AttrSelectorOperation::Exists => true,
                    AttrSelectorOperation::WithValue {
                        operator,
                        case_sensitivity,
                        value,
                    } => {
                        let v = val.as_str();
                        let expected: &str = &value.0;
                        use selectors::attr::AttrSelectorOperator::*;
                        let matched = match operator {
                            Equal => v == expected,
                            Includes => v.split_whitespace().any(|w| w == expected),
                            DashMatch => v == expected || v.starts_with(&format!("{expected}-")),
                            Prefix => !expected.is_empty() && v.starts_with(expected),
                            Suffix => !expected.is_empty() && v.ends_with(expected),
                            Substring => !expected.is_empty() && v.contains(expected),
                        };
                        match case_sensitivity {
                            CaseSensitivity::CaseSensitive => matched,
                            CaseSensitivity::AsciiCaseInsensitive => {
                                let v_lower = v.to_ascii_lowercase();
                                let exp_lower = expected.to_ascii_lowercase();
                                match operator {
                                    Equal => v_lower == exp_lower,
                                    Includes => v_lower.split_whitespace().any(|w| w == exp_lower),
                                    DashMatch => v_lower == exp_lower || v_lower.starts_with(&format!("{exp_lower}-")),
                                    Prefix => !exp_lower.is_empty() && v_lower.starts_with(&exp_lower),
                                    Suffix => !exp_lower.is_empty() && v_lower.ends_with(&exp_lower),
                                    Substring => !exp_lower.is_empty() && v_lower.contains(&exp_lower),
                                }
                            }
                        }
                    }
                }
            })
        }

        fn match_non_ts_pseudo_class(
            &self,
            _pc: &PseudoClass,
            _context: &mut MatchingContext<Impl>,
        ) -> bool {
            match *_pc {}
        }

        fn match_pseudo_element(
            &self,
            _pe: &PseudoElement,
            _context: &mut MatchingContext<Impl>,
        ) -> bool {
            match *_pe {}
        }

        fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

        fn is_link(&self) -> bool {
            *self.tag == *"a" && self.attrs.iter().any(|(n, _)| *n == *"href")
        }

        fn is_html_slot_element(&self) -> bool { false }

        fn has_id(&self, id: &Value, case_sensitivity: CaseSensitivity) -> bool {
            match &self.id {
                Some(my_id) => case_sensitivity.eq(my_id.as_bytes(), id.0.as_bytes()),
                None => false,
            }
        }

        fn has_class(&self, name: &Value, case_sensitivity: CaseSensitivity) -> bool {
            self.classes.iter().any(|c| case_sensitivity.eq(c.as_bytes(), name.0.as_bytes()))
        }

        fn has_custom_state(&self, _name: &Value) -> bool { false }
        fn imported_part(&self, _name: &Value) -> Option<Value> { None }
        fn is_part(&self, _name: &Value) -> bool { false }

        fn is_empty(&self) -> bool { self.children.is_empty() }
        fn is_root(&self) -> bool { self.is_root }

        fn add_element_unique_hashes(&self, _filter: &mut selectors::bloom::BloomFilter) -> bool {
            false
        }
    }

    pub fn matches_list(list: &SelectorList<Impl>, element: &El) -> bool {
        let mut caches = selectors::context::SelectorCaches::default();
        let mut ctx = MatchingContext::new(
            MatchingMode::Normal,
            None,
            &mut caches,
            QuirksMode::NoQuirks,
            NeedsSelectorFlags::No,
            MatchingForInvalidation::No,
        );
        selectors::matching::matches_selector_list(list, element, &mut ctx)
    }
}

// --- Parsing categories (trivial → extreme) ---

const SIMPLE_SELECTORS: &[&str] = &[
    "div", ".container", "#main", "*",
    "button.primary", "input[type=\"text\"]",
    "a.link", "li.item", "tr.row", "span.text",
];

const COMPLEX_SELECTORS: &[&str] = &[
    "div > .container .item",
    "html body div ul li a span",
    "#app > .header .nav-item.active",
    ".sidebar > ul > li:nth-child(2n+1) > a",
    "main article > p:first-child",
    "table tbody tr:nth-child(even) td:last-child",
    ":not(.hidden):not(.collapsed) > .content",
    "div.wrapper > section.content article.post h2.title",
    "[data-testid] > .content",
    ".a.b.c .d.e.f .g.h.i",
];

const HEAVY_SELECTORS: &[&str] = &[
    ":not(.hidden) > :not(.collapsed) > .target",
    ".l1 > .l2 > .l3 > .l4 > .l5 > .l6 > .l7 > .target",
    "div.a > div.b > div.c > div.d > span.e",
    ".x .y .z .w .v .u .t .s .r .q",
];

const ATTR_SELECTORS: &[&str] = &[
    "[href]",
    "[type=\"text\"]",
    "[class~=\"active\"]",
    "[data-testid=\"submit-btn\"]",
    "input[type=\"email\"][required]",
    "[href^=\"https\"][target=\"_blank\"]",
    "a[href$=\".pdf\"][download]",
    "[data-x][data-y][data-z]",
    "div[class*=\"col-\"][class*=\"md-\"]",
    "[aria-label][role=\"button\"][tabindex=\"0\"]",
];

const FUNCTIONAL_SELECTORS: &[&str] = &[
    ":not(.hidden)",
    ":is(.a, .b, .c, .d, .e)",
    ":where(.x, .y, .z)",
    ":not(:is(.a, .b))",
    ":is(div, span, p, a, button, input, li, ul, ol, table)",
    ":where(h1, h2, h3, h4, h5, h6)",
    ":not(.a):not(.b):not(.c)",
    ":is(.a > .b, .c > .d, .e > .f)",
    ":where(:not(.hidden), :not(.collapsed))",
    ":is(div.a, div.b, div.c, span.d, span.e)",
];

const NTH_SELECTORS: &[&str] = &[
    ":nth-child(2n+1)",
    ":nth-child(even)",
    ":nth-child(odd)",
    ":nth-child(3n)",
    ":nth-last-child(2)",
    ":nth-of-type(2n+1)",
    ":nth-last-of-type(odd)",
    ":nth-child(5n-2)",
    "li:nth-child(2n+1)",
    "tr:nth-child(even) > td:nth-child(3)",
];

const REALWORLD_SELECTORS: &[&str] = &[
    ".btn.btn-primary.btn-lg",
    ".card > .card-header + .card-body",
    ".nav > .nav-item > .nav-link.active",
    ".container > .row > .col-md-6",
    "table.table > thead > tr > th",
    ".form-group > label + .form-control",
    ".modal .modal-dialog .modal-content .modal-body",
    "#app > .layout > .sidebar .menu-item.selected",
    "article.post > .post-content p:first-child",
    ".d-flex.align-items-center.justify-content-between",
];

// --- Matching-specific categories ---

// Trivial: 1-component selectors — measures raw dispatch overhead
const MATCH_TRIVIAL: &[&str] = &[
    "*", "div", ".btn", "#app",
];

// Compound: multiple simple selectors on same element — no tree walk
const MATCH_COMPOUND: &[&str] = &[
    "button.btn",
    "button.btn.btn-primary",
    "button.btn.btn-primary.btn-lg",
    "#submit-btn.btn.btn-primary",
    "button#submit-btn.btn.btn-primary.btn-lg",
    "input[type=\"email\"]",
    "input[type=\"email\"][required]",
    "input[type=\"email\"][required][data-testid=\"email-field\"]",
    ":not(.hidden)",
    ":not(.hidden):not(.disabled)",
    ":is(.btn, .link, .card)",
    ":first-child",
    ":nth-child(3)",
    ":nth-child(2n+1)",
    ":last-child:first-of-type",
];

// Descendant: ancestor chain walking — measures tree traversal cost
const MATCH_DESCENDANT: &[&str] = &[
    // Shallow (1 ancestor hop)
    "div > a",
    ".nav > .link",
    // Medium (2-3 hops)
    "div .wrapper a",
    ".sidebar > ul > li > a",
    "#app > .layout .link",
    // Deep (5+ hops)
    "html body .app main article .post-content a",
    "html body div div div div div div div a",
    // Non-matching (must walk entire chain to fail)
    ".nonexistent a",
    "body > .nonexistent .link",
];

// Sibling: adjacent (+) and general (~) sibling combinators
const MATCH_SIBLING: &[&str] = &[
    "label + input",
    ".card-header + .card-body",
    "h1 + p",
    "h1 ~ p",
    ".tab.active + .tab-panel",
    "dt + dd",
    // Compound + sibling
    "label.required + input.form-control",
    // Chained siblings
    ".a + .b + .c",
];

// Attribute: all 7 operators × varying element attr counts
const MATCH_ATTR: &[&str] = &[
    "[href]",                                // exists
    "[type=\"email\"]",                      // exact
    "[class~=\"active\"]",                   // includes (word)
    "[lang|=\"en\"]",                        // dash-match
    "[href^=\"https://\"]",                  // prefix
    "[href$=\".pdf\"]",                      // suffix
    "[data-info*=\"important\"]",            // substring
    "[type=\"email\" i]",                    // case-insensitive
    "[data-x][data-y][data-z]",              // 3 attrs on one element
    "input[type=\"email\"][required][aria-label][data-testid=\"email\"]", // 4 attrs
];

// Functional: :not / :is / :where with varying complexity
const MATCH_FUNCTIONAL: &[&str] = &[
    ":not(.hidden)",
    ":not(.hidden):not(.disabled):not(.collapsed)",
    ":is(.btn, .link, .card, .badge, .alert)",
    ":where(.flex, .grid, .block, .inline)",
    ":not(:is(.a, .b, .c))",
    ":is(:not(.hidden), :not(.disabled))",
    ":is(div, span, p, a, button, input, li, ul, ol, table, section, article, header, footer, main, nav)",
    ":where(.a, .b, .c, .d, .e, .f, .g, .h, .i, .j)",
    ":not(:is(:not(.visible)))",
    ":is(.a > .b, .c > .d, .e > .f, .g > .h)",
];

// Nth: formulas of varying complexity
const MATCH_NTH: &[&str] = &[
    ":first-child",
    ":last-child",
    ":only-child",
    ":nth-child(1)",
    ":nth-child(odd)",
    ":nth-child(even)",
    ":nth-child(2n+1)",
    ":nth-child(3n-1)",
    ":nth-child(5n+3)",
    ":nth-last-child(2)",
    ":nth-of-type(odd)",
    ":nth-last-of-type(even)",
    ":first-of-type",
    ":last-of-type",
    ":only-of-type",
    "li:nth-child(2n+1):first-of-type",
];

// Real-world: actual selectors from Bootstrap, Tailwind, GitHub, YouTube
const MATCH_REALWORLD: &[&str] = &[
    // Bootstrap components
    ".btn.btn-primary.btn-lg",
    ".card > .card-header + .card-body",
    ".nav > .nav-item > .nav-link.active",
    ".container > .row > .col-md-6",
    "table.table > thead > tr > th",
    ".form-group > label + .form-control",
    ".modal .modal-dialog .modal-content .modal-body",
    // Tailwind utility patterns
    ".d-flex.align-items-center.justify-content-between",
    ".flex.items-center.gap-4",
    // GitHub-style
    "#app > .layout > .sidebar .menu-item.selected",
    "article.post > .post-content p:first-child",
    ".repo-list > .repo-item:nth-child(2n+1) > .repo-name > a",
    // Complex real selectors
    ":not(.hidden):not(.collapsed) > .content",
    ".wrapper > section.main article.entry:first-of-type h2.title",
    ":is(h1, h2, h3, h4, h5, h6):first-child",
    "table > tbody > tr:nth-child(even) > td:nth-child(3)",
];

// Stress: worst-case selectors — deep chains, many classes, nested logic
const MATCH_STRESS: &[&str] = &[
    // 8-deep child combinator chain
    ".l1 > .l2 > .l3 > .l4 > .l5 > .l6 > .l7 > .target",
    // 10-deep descendant chain
    ".x .y .z .w .v .u .t .s .r .target",
    // Many-class compound (tests has_class iteration)
    ".c1.c2.c3.c4.c5.c6.c7.c8",
    // Deeply nested :not/:is
    ":not(:is(:not(.hidden), :not(.disabled)))",
    // Large selector list in :is
    ":is(.a, .b, .c, .d, .e, .f, .g, .h, .i, .j, .k, .l, .m, .n, .o, .p)",
    // Compound + descendant + nth
    "div.wrapper > ul.list > li:nth-child(2n+1).active > a.link",
    // Non-matching deep walk (worst case — walks entire ancestor chain)
    ".nonexistent-class-that-wont-match-anything .target",
    // Attribute-heavy compound
    "[data-a][data-b][data-c][data-d][data-e]",
];

// --- Dimension 1: Functional nesting depth ---
// ONLY nesting depth changes. All use simple class leaves.
// Isolates: recursion overhead, sub-selector allocation traversal.
const MATCH_NEST_DEPTH: &[&str] = &[
    ":not(.hidden)",                                        // depth 1
    ":is(:not(.hidden))",                                   // depth 2
    ":not(:is(:not(.hidden)))",                             // depth 3
    ":is(:not(:is(:not(.hidden))))",                        // depth 4
    ":where(:is(:not(:is(:not(.hidden)))))",                // depth 5
    ":not(:where(:is(:not(:is(:not(.hidden))))))",          // depth 6
];

// --- Dimension 2: Functional list width ---
// ONLY number of sub-selectors changes. All single-class, no nesting.
// Isolates: sub-selector iteration cost, flattening benefit.
const MATCH_LIST_WIDTH: &[&str] = &[
    ":is(.a, .b)",                                                          // 2
    ":is(.a, .b, .c, .d)",                                                 // 4
    ":is(.a, .b, .c, .d, .e, .f, .g, .h)",                                // 8
    ":is(.a, .b, .c, .d, .e, .f, .g, .h, .i, .j, .k, .l, .m, .n, .o, .p)", // 16
];

// --- Dimension 3: Mixed sub-selectors (can't flatten) ---
// Sub-selectors mix class+type+compound. Same width (~4-5 each).
// Isolates: generic matching path vs flattened path.
const MATCH_MIXED_SUBS: &[&str] = &[
    // class + type mixed — can't flatten
    ":is(.btn, div, .card, span)",
    ":not(.hidden, div, .disabled, span)",
    ":where(.active, p, .selected, a)",
    // compound sub-selectors — each sub needs full compound match
    ":is(.btn.active, .card.hidden, .nav.collapsed, div.wrapper)",
    ":not(div.hidden, span.disabled, p.collapsed, a.inactive)",
    ":where(.btn[type], .card[data-x], input[required], a[href])",
];

// --- Dimension 4: Functional + combinators inside ---
// Sub-selectors contain tree-walking combinators.
// Isolates: cost of combinator matching inside functional pseudo-classes.
const MATCH_FUNC_COMBINATORS: &[&str] = &[
    ":is(.a > .b, .c > .d)",
    ":is(.sidebar > .nav, .header > .menu, .footer > .links)",
    ":not(.l1 > .target, .l2 > .target)",
    ":is(.x .y, .z .w, .v .u)",
    ":is(.l1 > .l2 > .l3, .l4 > .l5 > .l6)",
    ":where(div > .layout .sidebar, .header > .nav .link)",
];

// --- Dimension 5: Nesting + width combined ---
// Deep nesting AND wide lists at each level.
// Isolates: multiplicative cost of depth × width.
const MATCH_NEST_WIDE: &[&str] = &[
    ":is(:not(.a, .b), :is(.c, .d))",
    ":is(:not(.a, .b, .c), :where(.d, .e, .f), :is(.g, .h))",
    ":not(:is(:not(.a, .b), :where(.c, .d)), :where(:is(.e, .f)))",
    ":is(:not(:is(.a, .b), :where(.c, .d)), :where(:not(.e, .f), :is(.g, .h)))",
];

// --- Dimension 6: Everything combined (the real torture test) ---
// Each selector combines: deep nesting + wide lists + mixed types +
// combinators + attributes + nth + compound. The worst of everything.
const MATCH_TORTURE: &[&str] = &[
    // Functional nesting + combinator + compound + attr
    ":is(:not(.hidden, .disabled), :where(div.wrapper, span.text)):not(:is(.collapsed))",
    // Deep nesting + nth + compound at leaf
    "div.l1 > :is(:nth-child(2n+1), :not(.hidden)) > span.target[data-a]",
    // Triple functional chain + descendant + compound
    ":is(.btn, div):where(.active, span):not(.disabled) > a.link",
    // Wide :is + nested :not + combinator + attr + nth
    ":is(.a, .b, .c, div, span, p):not(:is(.hidden, .disabled)) > li:nth-child(odd)[data-x]",
    // Deep nesting + all combinator types + compound + attr
    ".l1.x > :not(:is(.nonexistent)) .l3.z + :where(div, .target)[data-a]",
    // Real-world nightmare: framework selector with everything
    "#app > .layout > :is(.sidebar, .main) :where(.nav, .content) > li.item:nth-child(2n+1):not(.hidden) > a.link[href]",
    // Deeply nested functional + wide + structural + compound + descendant
    ":is(:not(:where(.a, .b, .c, .d)), :is(:nth-child(odd), :first-child)) > div.target.s.r[data-a][data-b]",
    // Non-matching torture (must fully evaluate everything before failing)
    ":is(:not(:is(.zz, .yy)), :where(div.nonexistent, span.nope)):not(:is(.also-no)) > .definitely-not-here",
];

fn parse_bench(c: &mut Criterion, name: &str, selectors: &[&str]) {
    // Pre-filter: only benchmark selectors that BOTH engines can parse.
    // This ensures a fair apples-to-apples comparison.
    let shared: Vec<&str> = selectors.iter().copied()
        .filter(|s| kozan_selector::parser::parse(s).is_ok() && servo::parse(s).is_ok())
        .collect();

    let mut group = c.benchmark_group(name);
    group.bench_function("kozan", |b| {
        b.iter(|| {
            for sel in &shared {
                black_box(kozan_selector::parser::parse(sel).unwrap());
            }
        });
    });
    group.bench_function("stylo", |b| {
        b.iter(|| {
            for sel in &shared {
                black_box(servo::parse(sel).unwrap());
            }
        });
    });
    group.finish();
}

/// Build a rich kozan DOM for matching benchmarks.
/// Returns 12 elements at various depths/positions for testing.
fn build_kozan_dom() -> Vec<kozan::El> {
    use kozan_atom::Atom;

    // --- Deep nav tree (8 levels) ---
    // html > body > div#app.dark > div.layout > div.sidebar > ul.nav > li.nav-item.selected > a.nav-link.active
    let html = kozan::El { tag: Atom::from("html"), is_root: true, ..kozan::El::new("html") };
    let body = kozan::El::new("body").with_parent(html);
    let app = kozan::El::new("div").with_id("app").with_class("dark").with_parent(body);
    let layout = kozan::El::new("div").with_class("layout").with_parent(app);
    let sidebar = kozan::El::new("div").with_class("sidebar").with_parent(layout);
    let nav_ul = kozan::El::new("ul").with_class("nav").with_parent(sidebar);
    let nav_item = kozan::El::new("li")
        .with_class("nav-item").with_class("selected")
        .with_parent(nav_ul).at_index(3, 8);
    let nav_link = kozan::El::new("a")
        .with_class("nav-link").with_class("active")
        .with_attr("href", "https://example.com/page.pdf")
        .with_parent(nav_item);

    // --- Grid layout (5 levels) ---
    // div.container > div.row > div.col-md-6 > article.post > div.post-content > p:first-child
    let container = kozan::El::new("div").with_class("container");
    let row = kozan::El::new("div").with_class("row").with_parent(container);
    let col = kozan::El::new("div").with_class("col-md-6").with_parent(row);
    let article = kozan::El::new("article").with_class("post").with_parent(col);
    let post_content = kozan::El::new("div").with_class("post-content").with_parent(article);
    let first_p = kozan::El::new("p").with_parent(post_content).at_index(1, 5);

    // --- Table (4 levels) ---
    // table.table > thead > tr > th:nth-child(3)
    let table = kozan::El::new("table").with_class("table");
    let thead = kozan::El::new("thead").with_parent(table);
    let tr = kozan::El::new("tr").with_parent(thead).at_index(1, 1);
    let th = kozan::El::new("th").with_parent(tr.clone()).at_index(3, 6);

    // --- Form (sibling relationship) ---
    // div.form-group > label.required + input.form-control
    let form_group = kozan::El::new("div").with_class("form-group");
    let label = kozan::El::new("label").with_class("required").with_parent(form_group.clone());
    let input = kozan::El::new("input")
        .with_class("form-control")
        .with_attr("type", "email")
        .with_attr("required", "")
        .with_attr("aria-label", "Email address")
        .with_attr("data-testid", "email")
        .with_parent(form_group)
        .prev(label)
        .at_index(2, 2);

    // --- Modal (4 levels) ---
    // div.modal > div.modal-dialog > div.modal-content > div.modal-body
    let modal = kozan::El::new("div").with_class("modal");
    let dialog = kozan::El::new("div").with_class("modal-dialog").with_parent(modal);
    let modal_content = kozan::El::new("div").with_class("modal-content").with_parent(dialog);
    let modal_body = kozan::El::new("div").with_class("modal-body").with_parent(modal_content);

    // --- Card (sibling: header + body) ---
    // div.card > (div.card-header + div.card-body)
    let card = kozan::El::new("div").with_class("card");
    let card_header = kozan::El::new("div").with_class("card-header").with_parent(card.clone());
    let card_body = kozan::El::new("div")
        .with_class("card-body")
        .with_parent(card)
        .prev(card_header)
        .at_index(2, 2);

    // --- Button (compound, many classes) ---
    let button = kozan::El::new("button")
        .with_class("btn").with_class("btn-primary").with_class("btn-lg")
        .with_id("submit-btn")
        .with_attr("type", "submit");

    // --- Flex utility div (many classes) ---
    let flex = kozan::El::new("div")
        .with_class("d-flex").with_class("align-items-center")
        .with_class("justify-content-between")
        .with_class("flex").with_class("items-center").with_class("gap-4");

    // --- Heading (first-child in article) ---
    let heading = kozan::El::new("h2")
        .with_class("title")
        .with_parent(kozan::El::new("div").with_class("wrapper")
            .with_parent(kozan::El::new("section").with_class("main")
                .with_parent(kozan::El::new("div"))))
        .at_index(1, 4);

    // --- Stress element (8-deep child chain) ---
    let l1 = kozan::El::new("div").with_class("l1").with_class("x");
    let l2 = kozan::El::new("div").with_class("l2").with_class("y").with_parent(l1);
    let l3 = kozan::El::new("div").with_class("l3").with_class("z").with_parent(l2);
    let l4 = kozan::El::new("div").with_class("l4").with_class("w").with_parent(l3);
    let l5 = kozan::El::new("div").with_class("l5").with_class("v").with_parent(l4);
    let l6 = kozan::El::new("div").with_class("l6").with_class("u").with_parent(l5);
    let l7 = kozan::El::new("div").with_class("l7").with_class("t").with_parent(l6);
    let stress_target = kozan::El::new("span")
        .with_class("target").with_class("s").with_class("r")
        .with_class("c1").with_class("c2").with_class("c3")
        .with_class("c4").with_class("c5").with_class("c6")
        .with_class("c7").with_class("c8")
        .with_attr("data-a", "1").with_attr("data-b", "2").with_attr("data-c", "3")
        .with_attr("data-d", "4").with_attr("data-e", "5")
        .with_parent(l7)
        .at_index(1, 1);

    // --- Repo-list item (nth-child in list) ---
    let repo_list = kozan::El::new("div").with_class("repo-list");
    let repo_item = kozan::El::new("div")
        .with_class("repo-item")
        .with_parent(repo_list)
        .at_index(3, 20);
    let repo_name = kozan::El::new("div").with_class("repo-name").with_parent(repo_item);
    let repo_link = kozan::El::new("a").with_parent(repo_name).at_index(1, 1);

    vec![
        nav_link,       // 0: deep nav (8 levels)
        first_p,        // 1: grid article (5 levels)
        th,             // 2: table header (4 levels)
        input,          // 3: form input (sibling + attrs)
        modal_body,     // 4: modal (4 levels)
        card_body,      // 5: card (sibling)
        button,         // 6: button (compound, no ancestors)
        flex,           // 7: flex div (many classes, no ancestors)
        heading,        // 8: heading (3 levels)
        stress_target,  // 9: stress (8 levels, many classes+attrs)
        repo_link,      // 10: repo link (4 levels, nth)
        tr,             // 11: table row
    ]
}

/// Build identical DOM for Servo.
fn build_servo_dom() -> Vec<servo::El> {
    // --- Deep nav tree ---
    let mut html = servo::El::new("html");
    html.is_root = true;
    let body = servo::El::new("body").with_parent(html);
    let app = servo::El::new("div").with_id("app").with_class("dark").with_parent(body);
    let layout = servo::El::new("div").with_class("layout").with_parent(app);
    let sidebar = servo::El::new("div").with_class("sidebar").with_parent(layout);
    let nav_ul = servo::El::new("ul").with_class("nav").with_parent(sidebar);
    let nav_item = servo::El::new("li")
        .with_class("nav-item").with_class("selected")
        .with_parent(nav_ul).at_index(3, 8);
    let nav_link = servo::El::new("a")
        .with_class("nav-link").with_class("active")
        .with_attr("href", "https://example.com/page.pdf")
        .with_parent(nav_item);

    // --- Grid layout ---
    let container = servo::El::new("div").with_class("container");
    let row = servo::El::new("div").with_class("row").with_parent(container);
    let col = servo::El::new("div").with_class("col-md-6").with_parent(row);
    let article = servo::El::new("article").with_class("post").with_parent(col);
    let post_content = servo::El::new("div").with_class("post-content").with_parent(article);
    let first_p = servo::El::new("p").with_parent(post_content).at_index(1, 5);

    // --- Table ---
    let table = servo::El::new("table").with_class("table");
    let thead = servo::El::new("thead").with_parent(table);
    let tr = servo::El::new("tr").with_parent(thead).at_index(1, 1);
    let th = servo::El::new("th").with_parent(tr.clone()).at_index(3, 6);

    // --- Form ---
    let form_group = servo::El::new("div").with_class("form-group");
    let label = servo::El::new("label").with_class("required").with_parent(form_group.clone());
    let input = servo::El::new("input")
        .with_class("form-control")
        .with_attr("type", "email")
        .with_attr("required", "")
        .with_attr("aria-label", "Email address")
        .with_attr("data-testid", "email")
        .with_parent(form_group)
        .with_prev(label)
        .at_index(2, 2);

    // --- Modal ---
    let modal = servo::El::new("div").with_class("modal");
    let dialog = servo::El::new("div").with_class("modal-dialog").with_parent(modal);
    let modal_content = servo::El::new("div").with_class("modal-content").with_parent(dialog);
    let modal_body = servo::El::new("div").with_class("modal-body").with_parent(modal_content);

    // --- Card ---
    let card = servo::El::new("div").with_class("card");
    let card_header = servo::El::new("div").with_class("card-header").with_parent(card.clone());
    let card_body = servo::El::new("div")
        .with_class("card-body")
        .with_parent(card)
        .with_prev(card_header)
        .at_index(2, 2);

    // --- Button ---
    let button = servo::El::new("button")
        .with_class("btn").with_class("btn-primary").with_class("btn-lg")
        .with_id("submit-btn")
        .with_attr("type", "submit");

    // --- Flex ---
    let flex = servo::El::new("div")
        .with_class("d-flex").with_class("align-items-center")
        .with_class("justify-content-between")
        .with_class("flex").with_class("items-center").with_class("gap-4");

    // --- Heading ---
    let heading = servo::El::new("h2")
        .with_class("title")
        .with_parent(servo::El::new("div").with_class("wrapper")
            .with_parent(servo::El::new("section").with_class("main")
                .with_parent(servo::El::new("div"))))
        .at_index(1, 4);

    // --- Stress ---
    let l1 = servo::El::new("div").with_class("l1").with_class("x");
    let l2 = servo::El::new("div").with_class("l2").with_class("y").with_parent(l1);
    let l3 = servo::El::new("div").with_class("l3").with_class("z").with_parent(l2);
    let l4 = servo::El::new("div").with_class("l4").with_class("w").with_parent(l3);
    let l5 = servo::El::new("div").with_class("l5").with_class("v").with_parent(l4);
    let l6 = servo::El::new("div").with_class("l6").with_class("u").with_parent(l5);
    let l7 = servo::El::new("div").with_class("l7").with_class("t").with_parent(l6);
    let stress_target = servo::El::new("span")
        .with_class("target").with_class("s").with_class("r")
        .with_class("c1").with_class("c2").with_class("c3")
        .with_class("c4").with_class("c5").with_class("c6")
        .with_class("c7").with_class("c8")
        .with_attr("data-a", "1").with_attr("data-b", "2").with_attr("data-c", "3")
        .with_attr("data-d", "4").with_attr("data-e", "5")
        .with_parent(l7)
        .at_index(1, 1);

    // --- Repo link ---
    let repo_list = servo::El::new("div").with_class("repo-list");
    let repo_item = servo::El::new("div")
        .with_class("repo-item")
        .with_parent(repo_list)
        .at_index(3, 20);
    let repo_name = servo::El::new("div").with_class("repo-name").with_parent(repo_item);
    let repo_link = servo::El::new("a").with_parent(repo_name).at_index(1, 1);

    vec![
        nav_link, first_p, th, input, modal_body, card_body,
        button, flex, heading, stress_target, repo_link, tr,
    ]
}

fn match_bench(c: &mut Criterion, name: &str, selectors: &[&str]) {
    let mut group = c.benchmark_group(name);

    let kozan_els = build_kozan_dom();
    let servo_els = build_servo_dom();

    // Pre-parse for both engines (only selectors that parse successfully)
    let kozan_parsed: Vec<_> = selectors.iter()
        .filter_map(|s| kozan_selector::parser::parse(s).ok())
        .collect();
    let servo_parsed: Vec<_> = selectors.iter()
        .filter_map(|s| servo::parse(s).ok())
        .collect();

    group.bench_function("kozan", |b| {
        b.iter(|| {
            for list in &kozan_parsed {
                for sel in &list.0 {
                    for el in &kozan_els {
                        black_box(kozan_selector::matching::matches(sel, el));
                    }
                }
            }
        });
    });

    group.bench_function("stylo", |b| {
        b.iter(|| {
            for list in &servo_parsed {
                for el in &servo_els {
                    black_box(servo::matches_list(list, el));
                }
            }
        });
    });

    group.finish();
}

// --- Parsing benchmarks (7 categories) ---

fn bench_parse_simple(c: &mut Criterion) {
    parse_bench(c, "parse/simple", SIMPLE_SELECTORS);
}
fn bench_parse_complex(c: &mut Criterion) {
    parse_bench(c, "parse/complex", COMPLEX_SELECTORS);
}
fn bench_parse_heavy(c: &mut Criterion) {
    parse_bench(c, "parse/heavy", HEAVY_SELECTORS);
}
fn bench_parse_attr(c: &mut Criterion) {
    parse_bench(c, "parse/attr", ATTR_SELECTORS);
}
fn bench_parse_functional(c: &mut Criterion) {
    parse_bench(c, "parse/functional", FUNCTIONAL_SELECTORS);
}
fn bench_parse_nth(c: &mut Criterion) {
    parse_bench(c, "parse/nth", NTH_SELECTORS);
}
fn bench_parse_realworld(c: &mut Criterion) {
    parse_bench(c, "parse/realworld", REALWORLD_SELECTORS);
}
fn bench_parse_nest_depth(c: &mut Criterion) {
    parse_bench(c, "parse/nest_depth", MATCH_NEST_DEPTH);
}
fn bench_parse_mixed_subs(c: &mut Criterion) {
    parse_bench(c, "parse/mixed_subs", MATCH_MIXED_SUBS);
}
fn bench_parse_torture(c: &mut Criterion) {
    parse_bench(c, "parse/torture", MATCH_TORTURE);
}

// --- Matching benchmarks (9 categories × 12 elements × N selectors) ---

fn bench_match_trivial(c: &mut Criterion) {
    match_bench(c, "match/trivial", MATCH_TRIVIAL);
}
fn bench_match_compound(c: &mut Criterion) {
    match_bench(c, "match/compound", MATCH_COMPOUND);
}
fn bench_match_descendant(c: &mut Criterion) {
    match_bench(c, "match/descendant", MATCH_DESCENDANT);
}
fn bench_match_sibling(c: &mut Criterion) {
    match_bench(c, "match/sibling", MATCH_SIBLING);
}
fn bench_match_attr(c: &mut Criterion) {
    match_bench(c, "match/attr", MATCH_ATTR);
}
fn bench_match_functional(c: &mut Criterion) {
    match_bench(c, "match/functional", MATCH_FUNCTIONAL);
}
fn bench_match_nth(c: &mut Criterion) {
    match_bench(c, "match/nth", MATCH_NTH);
}
fn bench_match_realworld(c: &mut Criterion) {
    match_bench(c, "match/realworld", MATCH_REALWORLD);
}
fn bench_match_stress(c: &mut Criterion) {
    match_bench(c, "match/stress", MATCH_STRESS);
}
fn bench_match_nest_depth(c: &mut Criterion) {
    match_bench(c, "match/nest_depth", MATCH_NEST_DEPTH);
}
fn bench_match_list_width(c: &mut Criterion) {
    match_bench(c, "match/list_width", MATCH_LIST_WIDTH);
}
fn bench_match_mixed_subs(c: &mut Criterion) {
    match_bench(c, "match/mixed_subs", MATCH_MIXED_SUBS);
}
fn bench_match_func_combinators(c: &mut Criterion) {
    match_bench(c, "match/func_combinators", MATCH_FUNC_COMBINATORS);
}
fn bench_match_nest_wide(c: &mut Criterion) {
    match_bench(c, "match/nest_wide", MATCH_NEST_WIDE);
}
fn bench_match_torture(c: &mut Criterion) {
    match_bench(c, "match/torture", MATCH_TORTURE);
}

// --- Infrastructure benchmarks ---

fn bench_rule_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("rule_map");

    let rule_selectors: Vec<&str> = vec![
        "#header", "#footer", "#main", "#sidebar", "#nav",
        ".btn", ".btn-primary", ".btn-secondary", ".btn-lg", ".btn-sm",
        ".card", ".card-header", ".card-body", ".card-footer",
        ".nav-item", ".nav-link", ".active", ".disabled", ".hidden",
        ".container", ".row", ".col", ".col-6", ".col-12",
        ".text-center", ".text-left", ".text-right",
        ".d-flex", ".d-block", ".d-none", ".d-inline",
        ".mt-1", ".mt-2", ".mt-3", ".mb-1", ".mb-2", ".mb-3",
        ".p-1", ".p-2", ".p-3", ".px-1", ".py-1",
        "div", "span", "p", "a", "button", "input", "img", "ul", "li", "table",
        "tr", "td", "th", "h1", "h2", "h3", "h4", "section", "article", "main",
        "div.container", "button.btn", "a.nav-link", "li.nav-item",
        "input[type=text]", "button.btn.active", "div.card.hidden",
        ".container > .row > .col", "nav .nav-item > .nav-link",
        ".card > .card-body p", "#main .container .row",
        "*", "::before", "::after",
    ];

    let mut builder = kozan_selector::RuleMap::builder();
    for (i, css) in rule_selectors.iter().enumerate() {
        if let Ok(list) = kozan_selector::parser::parse(css) {
            for sel in list.0 {
                builder.insert(sel, i as u32);
            }
        }
    }
    let map = builder.build();

    let els = build_kozan_dom();

    group.bench_function("lookup_button", |b| {
        b.iter(|| black_box(map.find_matching(&els[6])));
    });

    group.bench_function("lookup_nav_link", |b| {
        b.iter(|| black_box(map.find_matching(&els[0])));
    });

    group.bench_function("lookup_input", |b| {
        b.iter(|| black_box(map.find_matching(&els[3])));
    });

    group.bench_function("lookup_stress", |b| {
        b.iter(|| black_box(map.find_matching(&els[9])));
    });

    group.bench_function("visitor_all", |b| {
        b.iter(|| {
            let mut count = 0u32;
            for el in &els {
                map.for_each_matching(el, |_| count += 1);
            }
            black_box(count);
        });
    });

    group.finish();
}

fn bench_bloom(c: &mut Criterion) {
    let mut group = c.benchmark_group("bloom");

    use kozan_atom::Atom;
    use kozan_selector::bloom::AncestorBloom;

    let atoms: Vec<Atom> = (0..50)
        .map(|i| Atom::from(format!("element-{i}").as_str()))
        .collect();

    group.bench_function("push_pop_50", |b| {
        b.iter(|| {
            let mut bloom = AncestorBloom::new();
            for atom in &atoms {
                bloom.insert_hash(AncestorBloom::hash_atom(atom));
            }
            for atom in &atoms {
                bloom.remove_hash(AncestorBloom::hash_atom(atom));
            }
            black_box(&bloom);
        });
    });

    group.bench_function("query_50", |b| {
        let mut bloom = AncestorBloom::new();
        for atom in &atoms {
            bloom.insert_hash(AncestorBloom::hash_atom(atom));
        }
        b.iter(|| {
            for atom in &atoms {
                black_box(bloom.might_contain(AncestorBloom::hash_atom(atom)));
            }
        });
    });

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(50).warm_up_time(std::time::Duration::from_millis(500)).measurement_time(std::time::Duration::from_secs(1));
    targets =
    // Parsing (7 categories)
    bench_parse_simple,
    bench_parse_complex,
    bench_parse_heavy,
    bench_parse_attr,
    bench_parse_functional,
    bench_parse_nth,
    bench_parse_realworld,
    bench_parse_nest_depth,
    bench_parse_mixed_subs,
    bench_parse_torture,
    // Matching (9 categories × 12 elements = comprehensive)
    bench_match_trivial,
    bench_match_compound,
    bench_match_descendant,
    bench_match_sibling,
    bench_match_attr,
    bench_match_functional,
    bench_match_nth,
    bench_match_realworld,
    bench_match_stress,
    // Brutal isolation benchmarks (6 dimensions)
    bench_match_nest_depth,
    bench_match_list_width,
    bench_match_mixed_subs,
    bench_match_func_combinators,
    bench_match_nest_wide,
    bench_match_torture,
    // Infrastructure
    bench_rule_map,
    bench_bloom,
);
criterion_main!(benches);
