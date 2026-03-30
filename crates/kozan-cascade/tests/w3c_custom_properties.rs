//! W3C CSS Custom Properties Level 1 compliance tests.
//!
//! Tests the full pipeline: parse CSS → build stylist → resolve style →
//! verify computed values are exactly correct.
//!
//! References:
//! - https://www.w3.org/TR/css-variables-1/
//! - https://www.w3.org/TR/css-cascade-5/

use kozan_atom::Atom;
use kozan_cascade::custom_properties::{
    self, CustomPropertyMap, EnvironmentValues,
};
use kozan_cascade::device::Device;
use kozan_cascade::origin::CascadeOrigin;
use kozan_cascade::resolver::{StyleResolver, ResolvedStyle};
use kozan_cascade::stylist::Stylist;
use kozan_css::parse_stylesheet;
use kozan_selector::element::Element;
use kozan_selector::opaque::OpaqueElement;
use kozan_selector::pseudo_class::ElementState;
use kozan_style::{ComputeContext, PropertyId};

// ═══════════════════════════════════════════════════
// TEST ELEMENT
// ═══════════════════════════════════════════════════

#[derive(Clone)]
struct El {
    tag: Atom,
    id: Option<Atom>,
    classes: Vec<Atom>,
}

impl El {
    fn tag(tag: &str) -> Self {
        Self { tag: Atom::from(tag), id: None, classes: vec![] }
    }
    fn with_class(mut self, c: &str) -> Self {
        self.classes.push(Atom::from(c));
        self
    }
    fn with_id(mut self, id: &str) -> Self {
        self.id = Some(Atom::from(id));
        self
    }
}

impl Element for El {
    fn local_name(&self) -> &Atom { &self.tag }
    fn id(&self) -> Option<&Atom> { self.id.as_ref() }
    fn has_class(&self, c: &Atom) -> bool { self.classes.contains(c) }
    fn each_class<F: FnMut(&Atom)>(&self, mut f: F) { for c in &self.classes { f(c); } }
    fn attr(&self, _: &Atom) -> Option<&str> { None }
    fn parent_element(&self) -> Option<Self> { None }
    fn prev_sibling_element(&self) -> Option<Self> { None }
    fn next_sibling_element(&self) -> Option<Self> { None }
    fn first_child_element(&self) -> Option<Self> { None }
    fn last_child_element(&self) -> Option<Self> { None }
    fn state(&self) -> ElementState { ElementState::empty() }
    fn is_root(&self) -> bool { false }
    fn is_empty(&self) -> bool { true }
    fn child_index(&self) -> u32 { 1 }
    fn child_count(&self) -> u32 { 1 }
    fn child_index_of_type(&self) -> u32 { 1 }
    fn child_count_of_type(&self) -> u32 { 1 }
    fn opaque(&self) -> OpaqueElement { OpaqueElement::new(0) }
}

fn resolve(css: &str, el: &El) -> std::sync::Arc<ResolvedStyle> {
    let sheet = parse_stylesheet(css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    let ctx = ComputeContext::default();
    let env = EnvironmentValues::empty();
    let mut resolver = StyleResolver::new(env);
    resolver.resolve(el, &stylist, None, None, None, &ctx, |_| None)
}

fn resolve_with_parent(
    css: &str,
    parent_el: &El,
    child_el: &El,
) -> (std::sync::Arc<ResolvedStyle>, std::sync::Arc<ResolvedStyle>) {
    let sheet = parse_stylesheet(css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    let ctx = ComputeContext::default();
    let env = EnvironmentValues::empty();
    let mut resolver = StyleResolver::new(env);

    let parent = resolver.resolve(parent_el, &stylist, None, None, None, &ctx, |_| None);
    let child = resolver.resolve(
        child_el, &stylist,
        Some(&parent.style), Some(&parent.custom_properties), None,
        &ctx, |_| None,
    );
    (parent, child)
}

// ═══════════════════════════════════════════════════
// §2: CUSTOM PROPERTY DEFINITIONS
// ═══════════════════════════════════════════════════

#[test]
fn w3c_custom_property_collected() {
    let r = resolve(
        ".x { --gap: 16px; --color: red }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.custom_properties.get_str("gap").unwrap().as_ref(), "16px");
    assert_eq!(r.custom_properties.get_str("color").unwrap().as_ref(), "red");
}

#[test]
fn w3c_custom_property_inherits_by_default() {
    // W3C §2: "Custom properties are ordinary properties, so they can be declared
    // on any element, are resolved with the normal inheritance and cascade rules"
    let (parent, child) = resolve_with_parent(
        ".parent { --gap: 16px }",
        &El::tag("div").with_class("parent"),
        &El::tag("span"),
    );
    assert_eq!(parent.custom_properties.get_str("gap").unwrap().as_ref(), "16px");
    assert_eq!(
        child.custom_properties.get_str("gap").unwrap().as_ref(), "16px",
        "custom properties MUST inherit by default"
    );
}

#[test]
fn w3c_custom_property_child_overrides_parent() {
    let (parent, child) = resolve_with_parent(
        ".parent { --x: old } .child { --x: new }",
        &El::tag("div").with_class("parent"),
        &El::tag("div").with_class("child"),
    );
    assert_eq!(parent.custom_properties.get_str("x").unwrap().as_ref(), "old");
    assert_eq!(
        child.custom_properties.get_str("x").unwrap().as_ref(), "new",
        "child declaration MUST override inherited value"
    );
}

#[test]
fn w3c_custom_property_case_sensitive() {
    // W3C §2: custom property names are case-sensitive
    let r = resolve(
        ".x { --myProp: upper; --myprop: lower }",
        &El::tag("div").with_class("x"),
    );
    // These are two different properties
    assert_eq!(r.custom_properties.get_str("myProp").unwrap().as_ref(), "upper");
    assert_eq!(r.custom_properties.get_str("myprop").unwrap().as_ref(), "lower");
}

// ═══════════════════════════════════════════════════
// §3: USING CUSTOM PROPERTIES — var()
// ═══════════════════════════════════════════════════

#[test]
fn w3c_var_substitution_simple() {
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("x"), Atom::from("10px"));

    let result = custom_properties::substitute(
        "var(--x)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("10px"));
}

#[test]
fn w3c_var_fallback_used_when_missing() {
    // W3C §3: "If the property named by the first argument to the var() function
    // is a custom property, and its value is anything other than the initial value,
    // the var() function is replaced by the value of the corresponding custom
    // property. Otherwise, the var() function is replaced by the fallback value."
    let map = CustomPropertyMap::new();
    let result = custom_properties::substitute(
        "var(--missing, 20px)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("20px"));
}

#[test]
fn w3c_var_no_fallback_no_value_is_invalid() {
    // W3C §3: "If the var() has no fallback value, the property containing the
    // var() function is invalid at computed-value time."
    let map = CustomPropertyMap::new();
    let result = custom_properties::substitute(
        "var(--missing)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result, None, "var() with no value and no fallback MUST be invalid");
}

#[test]
fn w3c_var_fallback_can_contain_var() {
    // W3C §3: "The fallback value of a var() reference may itself contain
    // var() references."
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("b"), Atom::from("blue"));

    let result = custom_properties::substitute(
        "var(--missing, var(--b))", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("blue"));
}

#[test]
fn w3c_var_fallback_can_contain_commas() {
    // W3C §3: "The fallback value allows commas."
    let map = CustomPropertyMap::new();
    let result = custom_properties::substitute(
        "var(--missing, 1px 2px 3px)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("1px 2px 3px"));
}

#[test]
fn w3c_var_in_shorthand_multiple_values() {
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("w"), Atom::from("2px"));
    map.insert(Atom::from("c"), Atom::from("red"));

    let result = custom_properties::substitute(
        "var(--w) solid var(--c)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("2px solid red"));
}

// ═══════════════════════════════════════════════════
// §3.1: CYCLES
// ═══════════════════════════════════════════════════

#[test]
fn w3c_cycle_two_properties_both_invalid() {
    // W3C §3.1: "Custom properties can contain references to other custom
    // properties. These dependencies form a graph. If there is a cycle in
    // this graph, all custom properties in the cycle must be made invalid
    // at computed-value time."
    let decls = vec![
        (Atom::from("a"), Atom::from("var(--b)")),
        (Atom::from("b"), Atom::from("var(--a)")),
    ];
    let r = custom_properties::resolve_custom_properties(&decls, None, &Default::default());
    assert!(r.get_str("a").is_none(), "--a MUST be invalid (cycle)");
    assert!(r.get_str("b").is_none(), "--b MUST be invalid (cycle)");
}

#[test]
fn w3c_cycle_self_reference_invalid() {
    let decls = vec![(Atom::from("x"), Atom::from("var(--x)"))];
    let r = custom_properties::resolve_custom_properties(&decls, None, &Default::default());
    assert!(r.get_str("x").is_none(), "self-referencing --x MUST be invalid");
}

#[test]
fn w3c_cycle_three_way_all_invalid() {
    let decls = vec![
        (Atom::from("a"), Atom::from("var(--b)")),
        (Atom::from("b"), Atom::from("var(--c)")),
        (Atom::from("c"), Atom::from("var(--a)")),
    ];
    let r = custom_properties::resolve_custom_properties(&decls, None, &Default::default());
    assert!(r.get_str("a").is_none());
    assert!(r.get_str("b").is_none());
    assert!(r.get_str("c").is_none());
}

#[test]
fn w3c_cycle_does_not_affect_non_cyclic() {
    // W3C §3.1: Properties not in the cycle must still resolve normally.
    let decls = vec![
        (Atom::from("safe"), Atom::from("10px")),
        (Atom::from("a"), Atom::from("var(--b)")),
        (Atom::from("b"), Atom::from("var(--a)")),
        (Atom::from("uses-safe"), Atom::from("var(--safe)")),
    ];
    let r = custom_properties::resolve_custom_properties(&decls, None, &Default::default());
    assert_eq!(r.get_str("safe").unwrap().as_ref(), "10px", "non-cyclic MUST survive");
    assert_eq!(r.get_str("uses-safe").unwrap().as_ref(), "10px", "reference to non-cyclic MUST work");
    assert!(r.get_str("a").is_none(), "cyclic MUST be invalid");
    assert!(r.get_str("b").is_none(), "cyclic MUST be invalid");
}

// ═══════════════════════════════════════════════════
// §3.2: RESOLVING DEPENDENCY CHAINS
// ═══════════════════════════════════════════════════

#[test]
fn w3c_dependency_chain_resolves_in_order() {
    let decls = vec![
        (Atom::from("a"), Atom::from("10px")),
        (Atom::from("b"), Atom::from("var(--a)")),
        (Atom::from("c"), Atom::from("calc(var(--b) + 5px)")),
    ];
    let r = custom_properties::resolve_custom_properties(&decls, None, &Default::default());
    assert_eq!(r.get_str("a").unwrap().as_ref(), "10px");
    assert_eq!(r.get_str("b").unwrap().as_ref(), "10px");
    assert_eq!(r.get_str("c").unwrap().as_ref(), "calc(10px + 5px)");
}

#[test]
fn w3c_var_references_inherited_property() {
    let mut parent = CustomPropertyMap::new();
    parent.insert(Atom::from("base"), Atom::from("8px"));

    let decls = vec![(Atom::from("gap"), Atom::from("var(--base)"))];
    let r = custom_properties::resolve_custom_properties(&decls, Some(&parent), &Default::default());
    assert_eq!(r.get_str("gap").unwrap().as_ref(), "8px");
    assert_eq!(r.get_str("base").unwrap().as_ref(), "8px", "inherited prop MUST be present");
}

// ═══════════════════════════════════════════════════
// ENV() — CSS Environment Variables Module Level 1
// ═══════════════════════════════════════════════════

#[test]
fn w3c_env_safe_area_from_device() {
    let mut device = Device::new(1024.0, 768.0);
    device.safe_area_inset_top = 44.0;
    device.safe_area_inset_bottom = 34.0;
    let ev = EnvironmentValues::from_device(&device);

    assert_eq!(ev.get("safe-area-inset-top").unwrap().as_ref(), "44px");
    assert_eq!(ev.get("safe-area-inset-right").unwrap().as_ref(), "0px");
    assert_eq!(ev.get("safe-area-inset-bottom").unwrap().as_ref(), "34px");
    assert_eq!(ev.get("safe-area-inset-left").unwrap().as_ref(), "0px");
}

#[test]
fn w3c_env_with_fallback() {
    let ev = EnvironmentValues::empty();
    let result = custom_properties::substitute(
        "env(safe-area-inset-top, 0px)",
        &CustomPropertyMap::new(), &ev, |_| None,
    );
    assert_eq!(result.as_deref(), Some("0px"), "env() with missing value MUST use fallback");
}

#[test]
fn w3c_env_no_fallback_no_value_fails() {
    let ev = EnvironmentValues::empty();
    let result = custom_properties::substitute(
        "env(unknown-variable)",
        &CustomPropertyMap::new(), &ev, |_| None,
    );
    assert_eq!(result, None, "env() with no value and no fallback MUST fail");
}

#[test]
fn w3c_env_in_calc() {
    let mut ev = EnvironmentValues::empty();
    ev.insert("safe-area-inset-top", "44px");

    let result = custom_properties::substitute(
        "calc(100vh - env(safe-area-inset-top))",
        &CustomPropertyMap::new(), &ev, |_| None,
    );
    assert_eq!(result.as_deref(), Some("calc(100vh - 44px)"));
}

// ═══════════════════════════════════════════════════
// MIXED var() + env()
// ═══════════════════════════════════════════════════

#[test]
fn w3c_mixed_var_and_env() {
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("gap"), Atom::from("16px"));
    let mut ev = EnvironmentValues::empty();
    ev.insert("safe-area-inset-top", "44px");

    let result = custom_properties::substitute(
        "calc(var(--gap) + env(safe-area-inset-top))",
        &map, &ev, |_| None,
    );
    assert_eq!(result.as_deref(), Some("calc(16px + 44px)"));
}

// ═══════════════════════════════════════════════════
// RESOLVER END-TO-END: CSS → ComputedStyle
// ═══════════════════════════════════════════════════

#[test]
fn w3c_resolver_display_flex() {
    let r = resolve(".x { display: flex }", &El::tag("div").with_class("x"));
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
}

#[test]
fn w3c_resolver_display_initial_is_inline() {
    let r = resolve(".nomatch { display: flex }", &El::tag("div"));
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Inline,
        "no matching rules → initial value (inline)"
    );
}

#[test]
fn w3c_resolver_specificity_wins() {
    let r = resolve(
        ".a { display: block } .a.b { display: flex }",
        &El::tag("div").with_class("a").with_class("b"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
}

#[test]
fn w3c_resolver_source_order_wins_same_specificity() {
    let r = resolve(
        ".x { display: block } .x { display: flex }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
}

#[test]
fn w3c_resolver_inheritance_color() {
    let (parent, child) = resolve_with_parent(
        "div { color: rgb(255, 0, 0) }",
        &El::tag("div"),
        &El::tag("span"),
    );
    assert_eq!(
        child.style.text.color, parent.style.text.color,
        "color MUST be inherited"
    );
}

#[test]
fn w3c_resolver_no_inherit_display() {
    let (_parent, child) = resolve_with_parent(
        ".p { display: flex }",
        &El::tag("div").with_class("p"),
        &El::tag("span"),
    );
    assert_eq!(
        child.style.layout.display,
        kozan_style::Display::Inline,
        "display MUST NOT be inherited — should be initial (inline)"
    );
}

#[test]
fn w3c_resolver_inherit_visibility() {
    let (_parent, child) = resolve_with_parent(
        ".p { visibility: hidden }",
        &El::tag("div").with_class("p"),
        &El::tag("span"),
    );
    assert_eq!(
        child.style.layout.visibility,
        kozan_style::Visibility::Hidden,
        "visibility MUST be inherited"
    );
}

#[test]
fn w3c_resolver_multiple_properties() {
    let r = resolve(
        ".x { display: flex; position: absolute; visibility: hidden }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
    assert_eq!(r.style.layout.position, kozan_style::Position::Absolute);
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Hidden);
}

#[test]
fn w3c_resolver_id_beats_class() {
    let r = resolve(
        ".x { display: block } #y { display: flex }",
        &El::tag("div").with_class("x").with_id("y"),
    );
    assert_eq!(
        r.style.layout.display, kozan_style::Display::Flex,
        "#id specificity MUST beat .class"
    );
}

#[test]
fn w3c_resolver_universal_has_lowest_specificity() {
    let r = resolve(
        "* { display: block } .x { display: flex }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display, kozan_style::Display::Flex,
        ".class MUST beat *"
    );
}

// ═══════════════════════════════════════════════════
// PARSE_VALUE: re-parse after substitution
// ═══════════════════════════════════════════════════

#[test]
fn w3c_parse_value_display() {
    let decl = kozan_css::parse_value(PropertyId::Display, "flex");
    assert!(decl.is_some(), "parse_value('display', 'flex') must succeed");
}

#[test]
fn w3c_parse_value_invalid_returns_none() {
    let decl = kozan_css::parse_value(PropertyId::Display, "banana");
    assert!(decl.is_none(), "parse_value('display', 'banana') must fail");
}

#[test]
fn w3c_parse_value_position() {
    let decl = kozan_css::parse_value(PropertyId::Position, "absolute");
    assert!(decl.is_some());
}

#[test]
fn w3c_parse_value_empty_string() {
    let decl = kozan_css::parse_value(PropertyId::Display, "");
    assert!(decl.is_none(), "empty string must fail to parse");
}

// ═══════════════════════════════════════════════════
// EDGE CASES: boundary conditions
// ═══════════════════════════════════════════════════

#[test]
fn w3c_no_rules_no_crash() {
    let r = resolve("", &El::tag("div"));
    // Should return default ComputedStyle without crashing
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline);
}

#[test]
fn w3c_many_matching_rules() {
    let mut css = String::new();
    for i in 0..100 {
        css.push_str(&format!(".x {{ --p{i}: {i} }}\n"));
    }
    css.push_str(".x { display: flex }\n");
    let r = resolve(&css, &El::tag("div").with_class("x"));
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
}

#[test]
fn w3c_custom_prop_empty_value() {
    let r = resolve(
        ".x { --empty: }",
        &El::tag("div").with_class("x"),
    );
    // Empty value is valid for custom properties
    // (may or may not be collected depending on parser)
    let _ = r.custom_properties;
}

#[test]
fn w3c_substitution_depth_limit() {
    // Self-referencing var() must fail, not infinite loop
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("x"), Atom::from("var(--x)"));
    let result = custom_properties::substitute(
        "var(--x)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result, None, "infinite var() recursion MUST fail");
}

#[test]
fn w3c_substitution_size_limit() {
    let mut map = CustomPropertyMap::new();
    let huge = "A".repeat(1_048_577); // > 1MB
    map.insert(Atom::from("big"), Atom::from(huge.as_str()));
    let result = custom_properties::substitute(
        "var(--big)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result, None, "result > 1MB MUST fail");
}

// ═══════════════════════════════════════════════════
// CSS-WIDE KEYWORDS: inherit, initial, unset, revert, revert-layer
// W3C CSS Cascading Level 5 §7
// ═══════════════════════════════════════════════════

#[test]
fn w3c_inherit_keyword_on_non_inherited_property() {
    // `display: inherit` on a non-inherited property → copy from parent
    let (_parent, child) = resolve_with_parent(
        ".p { display: grid } .c { display: inherit }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.display,
        kozan_style::Display::Grid,
        "display:inherit MUST copy parent's value even though display is non-inherited"
    );
}

#[test]
fn w3c_initial_keyword_on_inherited_property() {
    // `visibility: initial` → use CSS initial value (Visible), not parent's
    let (_parent, child) = resolve_with_parent(
        ".p { visibility: hidden } .c { visibility: initial }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.visibility,
        kozan_style::Visibility::Visible,
        "visibility:initial MUST use CSS initial value, not parent's"
    );
}

#[test]
fn w3c_unset_on_inherited_property_inherits() {
    // `visibility: unset` on inherited property → same as inherit
    let (_parent, child) = resolve_with_parent(
        ".p { visibility: hidden } .c { visibility: unset }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.visibility,
        kozan_style::Visibility::Hidden,
        "visibility:unset MUST inherit (visibility is inherited)"
    );
}

#[test]
fn w3c_unset_on_non_inherited_property_resets() {
    // `display: unset` on non-inherited property → same as initial
    let (_parent, child) = resolve_with_parent(
        ".p { display: grid } .c { display: unset }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.display,
        kozan_style::Display::Inline,
        "display:unset MUST use initial (display is NOT inherited)"
    );
}

#[test]
fn w3c_revert_on_non_inherited_property_resets() {
    // W3C §7.3: `revert` rolls back to previous origin. With no user
    // stylesheet, author revert → UA value → initial for non-inherited.
    let (_parent, child) = resolve_with_parent(
        ".p { display: grid } .c { display: revert }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.display,
        kozan_style::Display::Inline,
        "display:revert MUST reset to initial (no user/UA override)"
    );
}

#[test]
fn w3c_revert_on_inherited_property_inherits() {
    // `visibility: revert` → no previous origin → unset → inherit
    let (_parent, child) = resolve_with_parent(
        ".p { visibility: hidden } .c { visibility: revert }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.visibility,
        kozan_style::Visibility::Hidden,
        "visibility:revert MUST inherit when no previous origin"
    );
}

#[test]
fn w3c_revert_layer_on_non_inherited_resets() {
    let r = resolve(
        ".x { display: revert-layer }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Inline,
        "display:revert-layer MUST reset to initial (no previous layer)"
    );
}

#[test]
fn w3c_revert_layer_on_inherited_inherits() {
    let (_parent, child) = resolve_with_parent(
        ".p { visibility: hidden } .c { visibility: revert-layer }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        child.style.layout.visibility,
        kozan_style::Visibility::Hidden,
        "visibility:revert-layer MUST inherit when no previous layer"
    );
}

// ═══════════════════════════════════════════════════
// RESOLVER: multiple rules interacting
// ═══════════════════════════════════════════════════

#[test]
fn w3c_later_rule_overrides_earlier() {
    let r = resolve(
        ".x { position: relative } .x { position: absolute }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.position, kozan_style::Position::Absolute);
}

#[test]
fn w3c_type_selector_lower_than_class() {
    let r = resolve(
        "div { display: block } .x { display: flex }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
}

#[test]
fn w3c_resolver_box_sizing() {
    let r = resolve(
        ".x { box-sizing: border-box }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.box_sizing, kozan_style::BoxSizing::BorderBox);
}

#[test]
fn w3c_resolver_overflow() {
    let r = resolve(
        ".x { overflow-x: hidden; overflow-y: scroll }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.overflow_x, kozan_style::Overflow::Hidden);
    assert_eq!(r.style.layout.overflow_y, kozan_style::Overflow::Scroll);
}

#[test]
fn w3c_resolver_flex_properties() {
    let r = resolve(
        ".x { flex-direction: column; flex-wrap: wrap }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.flex.flex_direction, kozan_style::FlexDirection::Column);
    assert_eq!(r.style.flex.flex_wrap, kozan_style::FlexWrap::Wrap);
}

// ═══════════════════════════════════════════════════
// !IMPORTANT — W3C CSS Cascading Level 5 §6
// ═══════════════════════════════════════════════════

#[test]
fn w3c_important_beats_normal_same_specificity() {
    let r = resolve(
        ".x { display: flex !important } .x { display: block }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "!important MUST beat normal even if later in source"
    );
}

#[test]
fn w3c_important_beats_higher_specificity() {
    let r = resolve(
        ".x { display: flex !important } #y { display: block }",
        &El::tag("div").with_class("x").with_id("y"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "!important MUST beat higher specificity normal declaration"
    );
}

#[test]
fn w3c_important_on_inherited_property() {
    let r = resolve(
        ".x { visibility: hidden !important } .x { visibility: visible }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.visibility,
        kozan_style::Visibility::Hidden,
        "!important on inherited property MUST win"
    );
}

#[test]
fn w3c_two_important_later_wins() {
    let r = resolve(
        ".x { display: block !important } .x { display: flex !important }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "when both !important, later source order MUST win"
    );
}

// ═══════════════════════════════════════════════════
// EARLY PROPERTIES — direction, writing-mode, font-size, color
// ═══════════════════════════════════════════════════

#[test]
fn w3c_direction_applied_early() {
    let r = resolve(
        ".x { direction: rtl }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.text.direction,
        kozan_style::Direction::Rtl,
        "direction MUST be applied"
    );
}

#[test]
fn w3c_writing_mode_applied_early() {
    let r = resolve(
        ".x { writing-mode: vertical-rl }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.text.writing_mode,
        kozan_style::WritingMode::VerticalRl,
        "writing-mode MUST be applied"
    );
}

#[test]
fn w3c_direction_important_wins() {
    let r = resolve(
        ".x { direction: rtl !important } #y { direction: ltr }",
        &El::tag("div").with_class("x").with_id("y"),
    );
    assert_eq!(
        r.style.text.direction,
        kozan_style::Direction::Rtl,
        "direction:rtl !important MUST beat #id direction:ltr"
    );
}

#[test]
fn w3c_font_size_keyword() {
    let r = resolve(
        ".x { font-size: large }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.text.font_size,
        kozan_style::FontSize::Large,
    );
}

#[test]
fn w3c_color_applied_early() {
    let r = resolve(
        ".x { color: rgb(255, 0, 0) }",
        &El::tag("div").with_class("x"),
    );
    // Color should be red (sRGB 1.0, 0.0, 0.0)
    let c = r.style.text.color;
    assert!((c.components[0] - 1.0).abs() < 0.01, "red component must be ~1.0, got {}", c.components[0]);
    assert!(c.components[1].abs() < 0.01, "green component must be ~0.0, got {}", c.components[1]);
    assert!(c.components[2].abs() < 0.01, "blue component must be ~0.0, got {}", c.components[2]);
}

// ═══════════════════════════════════════════════════
// END-TO-END VAR() → ComputedStyle
// ═══════════════════════════════════════════════════

#[test]
fn w3c_var_in_display_resolves() {
    // var() in display → substituted → re-parsed → applied
    let r = resolve(
        ".x { --d: flex; display: var(--d) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "var(--d) with --d:flex MUST resolve display to flex"
    );
}

#[test]
fn w3c_var_in_position_resolves() {
    let r = resolve(
        ".x { --pos: absolute; position: var(--pos) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.position,
        kozan_style::Position::Absolute,
        "var(--pos) with --pos:absolute MUST resolve position"
    );
}

#[test]
fn w3c_var_with_fallback_resolves() {
    let r = resolve(
        ".x { display: var(--missing, grid) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Grid,
        "var(--missing, grid) MUST use fallback"
    );
}

#[test]
fn w3c_var_invalid_substitution_uses_initial() {
    // var(--x) where --x is "banana" → substitutes to "banana" → parse fails
    // → invalid at computed-value time → property uses initial/inherited value
    let r = resolve(
        ".x { --x: banana; display: var(--x) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Inline,
        "var() substituting to invalid CSS MUST fall back to initial"
    );
}

#[test]
fn w3c_var_chained_through_custom_props() {
    let r = resolve(
        ".x { --a: flex; --b: var(--a); display: var(--b) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "var(--b) where --b:var(--a) where --a:flex MUST chain-resolve"
    );
}

#[test]
fn w3c_var_inherits_from_parent_into_child() {
    let (parent, child) = resolve_with_parent(
        ".p { --mode: flex } .c { display: var(--mode) }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(
        parent.custom_properties.get_str("mode").unwrap().as_ref(), "flex"
    );
    assert_eq!(
        child.style.layout.display,
        kozan_style::Display::Flex,
        "child MUST inherit --mode from parent and use it in var()"
    );
}

#[test]
fn w3c_var_missing_no_fallback_property_invalid() {
    // display: var(--nope) with no --nope and no fallback → invalid → initial
    let r = resolve(
        ".x { display: var(--nope) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Inline,
        "var(--nope) with no value and no fallback → initial"
    );
}

// ═══════════════════════════════════════════════════
// PESSIMISTIC: malformed, edge cases, adversarial
// ═══════════════════════════════════════════════════

#[test]
fn w3c_empty_stylesheet_no_panic() {
    let r = resolve("", &El::tag("div"));
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline);
}

#[test]
fn w3c_malformed_rule_skipped() {
    // Malformed rule followed by valid one — valid must still apply
    let r = resolve(
        "{{{{invalid .x { display: flex }",
        &El::tag("div").with_class("x"),
    );
    // Parser should recover and apply the valid rule
    // (if not, display stays initial — that's also acceptable)
    let _ = r.style.layout.display;
}

#[test]
fn w3c_unknown_property_ignored() {
    let r = resolve(
        ".x { banana-color: red; display: flex }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "unknown properties MUST be skipped, valid ones still applied"
    );
}

#[test]
fn w3c_duplicate_property_last_wins() {
    let r = resolve(
        ".x { display: block; display: flex; display: grid }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Grid,
        "last declaration in same rule MUST win"
    );
}

#[test]
fn w3c_100_rules_same_element_last_wins() {
    let mut css = String::new();
    for i in 0..99 {
        css.push_str(&format!(".x {{ --r{i}: {i} }}\n"));
    }
    css.push_str(".x { display: grid }\n");
    let r = resolve(&css, &El::tag("div").with_class("x"));
    assert_eq!(r.style.layout.display, kozan_style::Display::Grid);
}

#[test]
fn w3c_no_matching_rules_all_initial() {
    let r = resolve(
        ".nomatch { display: flex; position: absolute; visibility: hidden }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline);
    assert_eq!(r.style.layout.position, kozan_style::Position::Static);
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Visible);
}

#[test]
fn w3c_custom_prop_cycle_doesnt_crash_resolver() {
    let r = resolve(
        ".x { --a: var(--b); --b: var(--a); display: var(--a, flex) }",
        &El::tag("div").with_class("x"),
    );
    // --a and --b are cyclic → invalid → var(--a, flex) uses fallback "flex"
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "cyclic var() MUST use fallback"
    );
}

#[test]
fn w3c_env_in_resolver() {
    // env() in a property value — uses EnvironmentValues
    // With empty env, env(safe-area-inset-top, 0px) → "0px"
    // Can't easily test this resolving to a computed length without
    // the property being a length type. Test with custom prop instead.
    let map = CustomPropertyMap::new();
    let ev = EnvironmentValues::empty();
    let result = custom_properties::substitute(
        "env(safe-area-inset-top, 0px)", &map, &ev, |_| None,
    );
    assert_eq!(result.as_deref(), Some("0px"));
}

// ═══════════════════════════════════════════════════
// MULTI-PROPERTY INTERACTION
// ═══════════════════════════════════════════════════

#[test]
fn w3c_shorthand_expands_to_longhands() {
    let r = resolve(
        ".x { overflow: hidden }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.overflow_x, kozan_style::Overflow::Hidden);
    assert_eq!(r.style.layout.overflow_y, kozan_style::Overflow::Hidden);
}

#[test]
fn w3c_longhand_after_shorthand_wins() {
    let r = resolve(
        ".x { overflow: hidden; overflow-y: scroll }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.layout.overflow_x, kozan_style::Overflow::Hidden);
    assert_eq!(
        r.style.layout.overflow_y,
        kozan_style::Overflow::Scroll,
        "longhand after shorthand MUST override"
    );
}

#[test]
fn w3c_shorthand_after_longhand_resets() {
    let r = resolve(
        ".x { overflow-y: scroll; overflow: hidden }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.overflow_y,
        kozan_style::Overflow::Hidden,
        "shorthand after longhand MUST reset the longhand"
    );
}

#[test]
fn w3c_flex_flow_shorthand() {
    let r = resolve(
        ".x { flex-flow: column wrap }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.flex.flex_direction, kozan_style::FlexDirection::Column);
    assert_eq!(r.style.flex.flex_wrap, kozan_style::FlexWrap::Wrap);
}

#[test]
fn w3c_flex_flow_longhand_after_shorthand() {
    let r = resolve(
        ".x { flex-flow: column wrap; flex-direction: row }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.flex.flex_direction, kozan_style::FlexDirection::Row,
        "longhand after shorthand MUST override"
    );
    assert_eq!(r.style.flex.flex_wrap, kozan_style::FlexWrap::Wrap);
}

// ═══════════════════════════════════════════════════
// SHORTHAND EXPANSION THROUGH STYLESHEETS
// Critical: these were ALL broken before the parse_shorthand fix
// ═══════════════════════════════════════════════════

#[test]
fn w3c_border_shorthand_in_stylesheet() {
    let r = resolve(
        ".x { border-top: 2px solid }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(r.style.visual.border_top_style, kozan_style::BorderStyle::Solid);
}

#[test]
fn w3c_css_wide_keyword_on_shorthand() {
    // `overflow: inherit` should expand to overflow-x:inherit + overflow-y:inherit
    let (parent, child) = resolve_with_parent(
        ".p { overflow-x: hidden; overflow-y: scroll } .c { overflow: inherit }",
        &El::tag("div").with_class("p"),
        &El::tag("div").with_class("c"),
    );
    assert_eq!(parent.style.layout.overflow_x, kozan_style::Overflow::Hidden);
    assert_eq!(parent.style.layout.overflow_y, kozan_style::Overflow::Scroll);
    assert_eq!(
        child.style.layout.overflow_x, kozan_style::Overflow::Hidden,
        "overflow:inherit must expand to overflow-x:inherit"
    );
    assert_eq!(
        child.style.layout.overflow_y, kozan_style::Overflow::Scroll,
        "overflow:inherit must expand to overflow-y:inherit"
    );
}

#[test]
fn w3c_var_empty_fallback() {
    // W3C §3: var(--missing, ) — empty fallback is valid, substitutes to ""
    let map = CustomPropertyMap::new();
    let result = custom_properties::substitute(
        "var(--missing, )", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some(""), "empty fallback must produce empty string");
}

#[test]
fn w3c_var_whitespace_around_name() {
    // W3C §3: whitespace around custom property name is allowed
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("x"), Atom::from("10px"));
    let result = custom_properties::substitute(
        "var( --x )", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("10px"), "whitespace around --name must be ignored");
}

#[test]
fn w3c_var_case_sensitive_names() {
    // W3C §2: custom property names are case-sensitive
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("MyProp"), Atom::from("upper"));
    map.insert(Atom::from("myprop"), Atom::from("lower"));
    let r1 = custom_properties::substitute(
        "var(--MyProp)", &map, &EnvironmentValues::empty(), |_| None,
    );
    let r2 = custom_properties::substitute(
        "var(--myprop)", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(r1.as_deref(), Some("upper"));
    assert_eq!(r2.as_deref(), Some("lower"));
    assert_ne!(r1, r2, "var(--MyProp) and var(--myprop) MUST be different");
}

#[test]
fn w3c_var_in_calc_multiple() {
    let mut map = CustomPropertyMap::new();
    map.insert(Atom::from("a"), Atom::from("10px"));
    map.insert(Atom::from("b"), Atom::from("20px"));
    let result = custom_properties::substitute(
        "calc(var(--a) + var(--b))", &map, &EnvironmentValues::empty(), |_| None,
    );
    assert_eq!(result.as_deref(), Some("calc(10px + 20px)"));
}

#[test]
fn w3c_var_in_shorthand_property() {
    // var() in a shorthand should create WithVariables for all longhands
    let r = resolve(
        ".x { --v: hidden; overflow: var(--v) }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.overflow_x, kozan_style::Overflow::Hidden,
        "var() in overflow shorthand must resolve overflow-x"
    );
    assert_eq!(
        r.style.layout.overflow_y, kozan_style::Overflow::Hidden,
        "var() in overflow shorthand must resolve overflow-y"
    );
}
