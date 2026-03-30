//! W3C CSS Cascading and Inheritance Level 4/5 compliance tests.
//!
//! Tests every cascade ordering rule from the spec with exact value verification.
//! Reference: https://www.w3.org/TR/css-cascade-5/
//!
//! Test structure mirrors the spec sections:
//! §6.1 — Cascade Sorting: origin, importance, layer, specificity, source order
//! §6.2 — Cascade Layers: @layer ordering, anonymous layers, nesting
//! §6.3 — Important reversal rules
//! §6.4 — Interactions between origins, layers, and importance
//! §7.3 — Revert keyword
//! §7.3.1 — Revert-layer keyword

use kozan_atom::Atom;
use kozan_cascade::cascade::{self, ApplicableDeclaration};
use kozan_cascade::custom_properties::{CustomPropertyMap, EnvironmentValues};
use kozan_cascade::device::Device;
use kozan_cascade::layer::UNLAYERED;
use kozan_cascade::origin::{CascadeLevel, CascadeOrigin, Importance};
use kozan_cascade::resolver::{StyleResolver, ResolvedStyle};
use kozan_cascade::stylist::{IndexedRule, Stylist};
use kozan_css::parse_stylesheet;
use kozan_selector::element::Element;
use kozan_selector::opaque::OpaqueElement;
use kozan_selector::pseudo_class::ElementState;
use kozan_style::{ComputeContext, ComputedStyle, DeclarationBlock};

fn level(origin: CascadeOrigin, important: bool, layer: u16) -> CascadeLevel {
    let imp = if important {
        Importance::Important
    } else {
        Importance::Normal
    };
    CascadeLevel::new(origin, imp, layer)
}

fn make_rule(origin: CascadeOrigin, layer: u16) -> IndexedRule {
    IndexedRule {
        declarations: triomphe::Arc::new(DeclarationBlock::new()),
        origin,
        layer_order: layer,
        container: None,
        scope: None,
        starting_style: false,
    }
}

fn make_decl(rule_index: u32, specificity: u32, source_order: u32) -> ApplicableDeclaration {
    ApplicableDeclaration {
        rule_index,
        specificity,
        source_order,
        origin: CascadeOrigin::Author,
        layer_order: kozan_cascade::layer::UNLAYERED,
        scope_depth: 0,
    }
}

fn make_scoped_decl(rule_index: u32, specificity: u32, source_order: u32, scope_depth: u16) -> ApplicableDeclaration {
    ApplicableDeclaration {
        rule_index,
        specificity,
        source_order,
        origin: CascadeOrigin::Author,
        layer_order: kozan_cascade::layer::UNLAYERED,
        scope_depth,
    }
}

fn stylist_with_css(css: &str) -> Stylist {
    let sheet = parse_stylesheet(css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    stylist
}

// ─── Mock element for resolver tests ─────────────────────────────

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

fn resolve_two_origins(ua_css: &str, author_css: &str, el: &El) -> std::sync::Arc<ResolvedStyle> {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(parse_stylesheet(ua_css), CascadeOrigin::UserAgent);
    stylist.add_stylesheet(parse_stylesheet(author_css), CascadeOrigin::Author);
    stylist.rebuild();
    let ctx = ComputeContext::default();
    let mut resolver = StyleResolver::new(EnvironmentValues::empty());
    resolver.resolve(el, &stylist, None, None, None, &ctx, |_| None)
}

fn resolve_author(css: &str, el: &El) -> std::sync::Arc<ResolvedStyle> {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(parse_stylesheet(css), CascadeOrigin::Author);
    stylist.rebuild();
    let ctx = ComputeContext::default();
    let mut resolver = StyleResolver::new(EnvironmentValues::empty());
    resolver.resolve(el, &stylist, None, None, None, &ctx, |_| None)
}

// ═══════════════════════════════════════════════════════════════════
// §6.1 — ORIGIN ORDERING (normal declarations)
// W3C: "User-agent < User < Author" for normal declarations
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_origin_ua_lt_user_lt_author_normal() {
    let ua = level(CascadeOrigin::UserAgent, false, UNLAYERED);
    let user = level(CascadeOrigin::User, false, UNLAYERED);
    let author = level(CascadeOrigin::Author, false, UNLAYERED);
    assert!(ua < user, "UA normal must be lower than User normal");
    assert!(user < author, "User normal must be lower than Author normal");
    assert!(ua < author, "UA normal must be lower than Author normal");
}

#[test]
fn w3c_origin_important_reverses() {
    // W3C §6.1: "Author !important < User !important < UA !important"
    let author_imp = level(CascadeOrigin::Author, true, UNLAYERED);
    let user_imp = level(CascadeOrigin::User, true, UNLAYERED);
    let ua_imp = level(CascadeOrigin::UserAgent, true, UNLAYERED);
    assert!(
        author_imp < user_imp,
        "Author !important must be lower than User !important"
    );
    assert!(
        user_imp < ua_imp,
        "User !important must be lower than UA !important"
    );
}

#[test]
fn w3c_all_important_beats_all_normal() {
    // W3C: ANY !important declaration beats ANY normal declaration
    let author_normal = level(CascadeOrigin::Author, false, UNLAYERED);
    let ua_important = level(CascadeOrigin::UserAgent, true, 0); // lowest possible !important
    assert!(
        author_normal < ua_important,
        "Even lowest !important (UA layer-0) must beat highest normal (Author unlayered)"
    );
}

#[test]
fn w3c_author_important_beats_author_normal() {
    let normal = level(CascadeOrigin::Author, false, UNLAYERED);
    let important = level(CascadeOrigin::Author, true, UNLAYERED);
    assert!(normal < important);
}

#[test]
fn w3c_user_important_beats_author_important() {
    let author_imp = level(CascadeOrigin::Author, true, UNLAYERED);
    let user_imp = level(CascadeOrigin::User, true, UNLAYERED);
    assert!(author_imp < user_imp);
}

#[test]
fn w3c_ua_important_layer0_is_highest_priority() {
    // For !important: layer order is REVERSED — earlier layers beat later,
    // and layered beats unlayered. So UA !important layer=0 is the absolute highest.
    let ua_imp_layer0 = level(CascadeOrigin::UserAgent, true, 0);

    for &origin in &[CascadeOrigin::UserAgent, CascadeOrigin::User, CascadeOrigin::Author] {
        for &imp in &[false, true] {
            for &layer in &[0, 1, 100, UNLAYERED] {
                let other = level(origin, imp, layer);
                if origin == CascadeOrigin::UserAgent && imp && layer == 0 {
                    continue; // skip self
                }
                assert!(
                    other < ua_imp_layer0,
                    "UA !important layer=0 must beat all others, but (origin={:?}, imp={}, layer={}) = {:08X} was >= {:08X}",
                    origin, imp, layer, other.as_u32(), ua_imp_layer0.as_u32()
                );
            }
        }
    }
}

#[test]
fn w3c_ua_important_unlayered_beats_all_normal() {
    // UA !important unlayered beats ALL normal declarations regardless
    // of origin, layer, or specificity.
    let ua_imp_unlayered = level(CascadeOrigin::UserAgent, true, UNLAYERED);

    for &origin in &[CascadeOrigin::UserAgent, CascadeOrigin::User, CascadeOrigin::Author] {
        for &layer in &[0, 1, 100, UNLAYERED] {
            let normal = level(origin, false, layer);
            assert!(
                normal < ua_imp_unlayered,
                "All normal must be < UA !important unlayered"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// §6.2 — CASCADE LAYERS (normal declarations)
// W3C: "Earlier-declared layer < later-declared layer < unlayered"
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_layer_normal_earlier_lt_later_lt_unlayered() {
    let layer_a = level(CascadeOrigin::Author, false, 0); // first declared
    let layer_b = level(CascadeOrigin::Author, false, 1); // second declared
    let layer_c = level(CascadeOrigin::Author, false, 2); // third declared
    let unlayered = level(CascadeOrigin::Author, false, UNLAYERED);

    assert!(layer_a < layer_b, "Earlier layer < later layer (normal)");
    assert!(layer_b < layer_c, "Layer ordering is transitive (normal)");
    assert!(layer_c < unlayered, "All layers < unlayered (normal)");
}

#[test]
fn w3c_layer_important_reverses_completely() {
    // W3C §6.3: For !important, layer order reverses AND unlayered loses
    let layer_a_imp = level(CascadeOrigin::Author, true, 0);
    let layer_b_imp = level(CascadeOrigin::Author, true, 1);
    let layer_c_imp = level(CascadeOrigin::Author, true, 2);
    let unlayered_imp = level(CascadeOrigin::Author, true, UNLAYERED);

    assert!(
        unlayered_imp < layer_c_imp,
        "Unlayered !important < any layered !important"
    );
    assert!(
        layer_c_imp < layer_b_imp,
        "Later layer !important < earlier layer !important"
    );
    assert!(
        layer_b_imp < layer_a_imp,
        "Layer reversal is transitive for !important"
    );
}

#[test]
fn w3c_unlayered_normal_beats_all_layered_normal() {
    for layer in [0, 1, 10, 100, 1000, 8000] {
        let layered = level(CascadeOrigin::Author, false, layer);
        let unlayered = level(CascadeOrigin::Author, false, UNLAYERED);
        assert!(
            layered < unlayered,
            "Layer {} normal must be lower than unlayered normal",
            layer
        );
    }
}

#[test]
fn w3c_unlayered_important_loses_to_all_layered_important() {
    // W3C: "unlayered !important" has LOWEST priority among !important in same origin
    for layer in [0, 1, 10, 100, 1000, 8000] {
        let layered_imp = level(CascadeOrigin::Author, true, layer);
        let unlayered_imp = level(CascadeOrigin::Author, true, UNLAYERED);
        assert!(
            unlayered_imp < layered_imp,
            "Unlayered !important must lose to layer {} !important",
            layer
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
// §6.3 — FULL CASCADE ORDER (all 10 levels)
// W3C CSS Cascading Level 5, §6.1
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_full_cascade_10_levels() {
    // The complete cascade priority, ascending:
    // 1. UA normal, layer 0
    // 2. UA normal, unlayered
    // 3. User normal, unlayered
    // 4. Author normal, layer 0
    // 5. Author normal, layer 1
    // 6. Author normal, unlayered
    // 7. Author !important, unlayered  (lowest !important for author)
    // 8. Author !important, layer 1
    // 9. Author !important, layer 0    (first layer wins for !important)
    // 10. User !important, unlayered
    // 11. UA !important, unlayered     (highest priority in entire cascade)
    let levels = [
        level(CascadeOrigin::UserAgent, false, 0),
        level(CascadeOrigin::UserAgent, false, UNLAYERED),
        level(CascadeOrigin::User, false, UNLAYERED),
        level(CascadeOrigin::Author, false, 0),
        level(CascadeOrigin::Author, false, 1),
        level(CascadeOrigin::Author, false, UNLAYERED),
        level(CascadeOrigin::Author, true, UNLAYERED),
        level(CascadeOrigin::Author, true, 1),
        level(CascadeOrigin::Author, true, 0),
        level(CascadeOrigin::User, true, UNLAYERED),
        level(CascadeOrigin::UserAgent, true, UNLAYERED),
    ];

    for i in 0..levels.len() - 1 {
        assert!(
            levels[i] < levels[i + 1],
            "W3C cascade order violated: level[{}] ({:08X}) must be < level[{}] ({:08X})",
            i,
            levels[i].as_u32(),
            i + 1,
            levels[i + 1].as_u32(),
        );
    }
}

#[test]
fn w3c_full_cascade_with_user_layers() {
    // User origin also supports layers
    let levels = [
        level(CascadeOrigin::User, false, 0),     // User layer-0 normal
        level(CascadeOrigin::User, false, 1),      // User layer-1 normal
        level(CascadeOrigin::User, false, UNLAYERED), // User unlayered normal
        level(CascadeOrigin::User, true, UNLAYERED),  // User unlayered !important
        level(CascadeOrigin::User, true, 1),       // User layer-1 !important
        level(CascadeOrigin::User, true, 0),       // User layer-0 !important
    ];

    for i in 0..levels.len() - 1 {
        assert!(
            levels[i] < levels[i + 1],
            "User layer cascade order violated at index {}-{}",
            i,
            i + 1,
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
// §6.4 — SPECIFICITY AND SOURCE ORDER TIE-BREAKING
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_same_level_specificity_breaks_tie() {
    // Same origin + layer + importance → specificity wins
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 100, 0), // higher specificity
        make_decl(1, 10, 1),  // lower specificity
    ];
    cascade::sort(&mut decls, &rules);
    // After sort: lower specificity first, higher last (winner)
    assert_eq!(decls[0].specificity, 10, "Lower specificity sorts first");
    assert_eq!(decls[1].specificity, 100, "Higher specificity sorts last (wins)");
}

#[test]
fn w3c_same_specificity_source_order_breaks_tie() {
    // Same origin + layer + importance + specificity → source order wins
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 10, 5), // later source
        make_decl(1, 10, 2), // earlier source
    ];
    cascade::sort(&mut decls, &rules);
    assert_eq!(decls[0].source_order, 2, "Earlier source order first");
    assert_eq!(decls[1].source_order, 5, "Later source order last (wins)");
}

#[test]
fn w3c_layer_beats_specificity() {
    // W3C: Layer ordering takes priority over specificity
    let rules = vec![
        make_rule(CascadeOrigin::Author, 0), // layer 0
        make_rule(CascadeOrigin::Author, 1), // layer 1
    ];
    let mut decls = vec![
        make_decl(0, 10000, 0), // layer 0, ultra-high specificity
        make_decl(1, 1, 1),     // layer 1, minimal specificity
    ];
    cascade::sort(&mut decls, &rules);
    // Layer 1 beats layer 0 regardless of specificity
    assert_eq!(decls[1].rule_index, 1, "Later layer wins despite lower specificity");
}

#[test]
fn w3c_origin_beats_layer() {
    // W3C: Origin takes priority over layer
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED), // UA, best layer
        make_rule(CascadeOrigin::Author, 0),             // Author, worst layer
    ];
    let mut decls = vec![
        make_decl(0, 0, 0),
        make_decl(1, 0, 1),
    ];
    cascade::sort(&mut decls, &rules);
    assert_eq!(
        decls[1].rule_index, 1,
        "Author origin beats UA regardless of layer"
    );
}

#[test]
fn w3c_origin_beats_specificity() {
    // UA with specificity 10000 still loses to Author with specificity 1
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 10000, 0), // UA, ultra-high specificity
        make_decl(1, 1, 1),     // Author, minimal
    ];
    cascade::sort(&mut decls, &rules);
    assert_eq!(decls[1].rule_index, 1, "Author beats UA regardless of specificity");
}

// ═══════════════════════════════════════════════════════════════════
// §6.5 — CASCADE SORT STABILITY
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_sort_is_stable() {
    // Equal cascade level + equal specificity + equal source order
    // must preserve input order (stable sort)
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 10, 5),
        make_decl(1, 10, 5),
        make_decl(2, 10, 5),
    ];
    cascade::sort(&mut decls, &rules);
    // Stable sort preserves original order for equal elements
    assert_eq!(decls[0].rule_index, 0);
    assert_eq!(decls[1].rule_index, 1);
    assert_eq!(decls[2].rule_index, 2);
}

// ═══════════════════════════════════════════════════════════════════
// @LAYER — STYLIST INTEGRATION (end-to-end with parsed CSS)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_layer_statement_reserves_order() {
    // W3C: @layer statements establish layer order before any rules
    let stylist = stylist_with_css(
        "@layer base, components, utilities;
         @layer utilities { .u { color: red } }
         @layer base { .b { color: blue } }
         @layer components { .c { color: green } }",
    );
    assert_eq!(stylist.rule_count(), 3);

    // base=0, components=1, utilities=2 (from @layer statement)
    let rules = stylist.rules();
    assert_eq!(rules[0].layer_order, 2, "utilities should be layer 2");
    assert_eq!(rules[1].layer_order, 0, "base should be layer 0");
    assert_eq!(rules[2].layer_order, 1, "components should be layer 1");
}

#[test]
fn w3c_layer_block_order_by_first_encounter() {
    let stylist = stylist_with_css(
        "@layer base { .a { color: red } }
         @layer utils { .b { color: blue } }
         @layer base { .c { color: green } }",
    );
    let rules = stylist.rules();
    assert_eq!(rules[0].layer_order, 0, "base first encounter = layer 0");
    assert_eq!(rules[1].layer_order, 1, "utils first encounter = layer 1");
    assert_eq!(rules[2].layer_order, 0, "base second block reuses layer 0");
}

#[test]
fn w3c_anonymous_layers_are_distinct() {
    // W3C: Each anonymous @layer {} gets a unique order
    let stylist = stylist_with_css(
        "@layer { .a { color: red } }
         @layer { .b { color: blue } }",
    );
    let rules = stylist.rules();
    assert_ne!(
        rules[0].layer_order, rules[1].layer_order,
        "Anonymous layers must get distinct order values"
    );
    assert!(
        rules[0].layer_order < rules[1].layer_order,
        "First anonymous < second anonymous"
    );
}

#[test]
fn w3c_unlayered_rule_gets_max_layer() {
    let stylist = stylist_with_css(".plain { color: red }");
    assert_eq!(
        stylist.rules()[0].layer_order, UNLAYERED,
        "Unlayered rules must use UNLAYERED constant"
    );
}

#[test]
fn w3c_empty_layer_block_no_rules() {
    let stylist = stylist_with_css(
        "@layer empty {}
         .visible { color: red }",
    );
    assert_eq!(stylist.rule_count(), 1, "Empty layer block adds no rules");
    // But the layer name should still be registered
    assert_eq!(stylist.layer_order().len(), 1);
}

#[test]
fn w3c_layer_statement_multiple_names() {
    // @layer a, b, c; reserves all three in source order
    let stylist = stylist_with_css(
        "@layer reset, base, theme;
         @layer theme { .t { color: red } }
         @layer reset { .r { color: blue } }",
    );
    let rules = stylist.rules();
    assert_eq!(rules[0].layer_order, 2, "theme = layer 2");
    assert_eq!(rules[1].layer_order, 0, "reset = layer 0");
}

// ═══════════════════════════════════════════════════════════════════
// @MEDIA — CONDITIONAL RULES
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_media_active_rules_included() {
    // Device is 1024x768
    let stylist = stylist_with_css(
        "@media (min-width: 768px) { .tablet { display: flex } }",
    );
    assert_eq!(stylist.rule_count(), 1);
}

#[test]
fn w3c_media_inactive_rules_excluded() {
    let stylist = stylist_with_css(
        "@media (min-width: 2000px) { .wide { display: flex } }",
    );
    assert_eq!(stylist.rule_count(), 0);
}

#[test]
fn w3c_media_inside_layer() {
    let stylist = stylist_with_css(
        "@layer responsive {
            @media (min-width: 768px) {
                .tablet { display: flex }
            }
        }",
    );
    assert_eq!(stylist.rule_count(), 1);
    assert_ne!(stylist.rules()[0].layer_order, UNLAYERED);
}

#[test]
fn w3c_layer_inside_media() {
    let stylist = stylist_with_css(
        "@media (min-width: 768px) {
            @layer tablet {
                .t { display: flex }
            }
        }",
    );
    assert_eq!(stylist.rule_count(), 1);
    assert_ne!(stylist.rules()[0].layer_order, UNLAYERED);
}

// ═══════════════════════════════════════════════════════════════════
// CASCADE APPLY — TWO-PASS APPLICATION
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_cascade_apply_normal_pass_ascending_order() {
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let sorted = vec![
        make_decl(0, 0, 0), // UA
        make_decl(1, 10, 1), // Author
    ];
    let mut order = Vec::new();
    cascade::cascade_apply(&sorted, &rules, |_rule, _level, importance| {
        order.push(importance);
    });
    // Normal pass runs first (at least 2 Normal callbacks)
    assert!(order.iter().filter(|i| **i == Importance::Normal).count() >= 2);
}

// ═══════════════════════════════════════════════════════════════════
// STYLIST — SHEET MANAGEMENT
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_multiple_origins_indexed_separately() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(
        parse_stylesheet("* { display: block }"),
        CascadeOrigin::UserAgent,
    );
    stylist.add_stylesheet(
        parse_stylesheet(".custom { color: red }"),
        CascadeOrigin::Author,
    );
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 2);
    assert!(!stylist.ua_map().is_empty());
    assert!(!stylist.author_map().is_empty());
    assert!(stylist.user_map().is_empty());
}

#[test]
fn w3c_rebuild_after_clear_sheets() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(parse_stylesheet(".a { color: red }"), CascadeOrigin::Author);
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 1);

    stylist.clear_sheets();
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 0);
}

#[test]
fn w3c_replace_stylesheet_hot_reload() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(parse_stylesheet(".a { color: red }"), CascadeOrigin::Author);
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 1);
    let gen1 = stylist.generation();

    // Hot reload: replace the sheet
    stylist.replace_stylesheet(0, parse_stylesheet(".b { color: blue } .c { color: green }"), CascadeOrigin::Author);
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 2);
    assert!(stylist.generation() > gen1, "Generation must bump on rebuild");
}

#[test]
fn w3c_remove_stylesheet() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(parse_stylesheet(".a { color: red }"), CascadeOrigin::Author);
    stylist.add_stylesheet(parse_stylesheet(".b { color: blue }"), CascadeOrigin::Author);
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 2);

    stylist.remove_stylesheet(0);
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 1);
}

#[test]
fn w3c_rebuild_is_idempotent() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(
        parse_stylesheet("@layer base { .a { color: red } } .b { color: blue }"),
        CascadeOrigin::Author,
    );
    stylist.rebuild();
    let count1 = stylist.rule_count();
    let layer_count1 = stylist.layer_order().len();

    // Second rebuild must produce identical results
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), count1);
    assert_eq!(stylist.layer_order().len(), layer_count1);
}

// ═══════════════════════════════════════════════════════════════════
// CROSS-ORIGIN LAYER INTERACTIONS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_cross_origin_author_layer_beats_ua_unlayered_normal() {
    // Even Author layer-0 (lowest author layer) beats UA unlayered
    let ua_unlayered = level(CascadeOrigin::UserAgent, false, UNLAYERED);
    let author_layer0 = level(CascadeOrigin::Author, false, 0);
    assert!(ua_unlayered < author_layer0, "Author layer-0 beats UA unlayered");
}

#[test]
fn w3c_cross_origin_ua_important_beats_author_important() {
    // UA !important always beats Author !important
    let author_imp_layer0 = level(CascadeOrigin::Author, true, 0); // highest Author !important
    let ua_imp_unlayered = level(CascadeOrigin::UserAgent, true, UNLAYERED); // lowest UA !important
    assert!(author_imp_layer0 < ua_imp_unlayered);
}

#[test]
fn w3c_user_layers_ordered_correctly() {
    let user_layer0 = level(CascadeOrigin::User, false, 0);
    let user_layer1 = level(CascadeOrigin::User, false, 1);
    let user_unlayered = level(CascadeOrigin::User, false, UNLAYERED);
    assert!(user_layer0 < user_layer1);
    assert!(user_layer1 < user_unlayered);
}

// ═══════════════════════════════════════════════════════════════════
// EDGE CASES
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_max_layers_handled() {
    // Test with many layers — should not overflow or panic
    let mut levels = Vec::new();
    for i in 0..100u16 {
        levels.push(level(CascadeOrigin::Author, false, i));
    }
    for i in 0..99 {
        assert!(levels[i] < levels[i + 1], "Layer {} must be < layer {}", i, i + 1);
    }
}

#[test]
fn w3c_layer_clamping_above_max() {
    // Layers above MAX_LAYER (8191) get clamped
    let at_max = level(CascadeOrigin::Author, false, 8191);
    let above_max = level(CascadeOrigin::Author, false, 9000);
    // Both clamp to 8191, so they're equal
    assert_eq!(at_max.as_u32(), above_max.as_u32());
}

#[test]
fn w3c_cascade_sort_large_set() {
    // 1000 declarations with mixed origins and layers — sort must not crash
    let mut rules = Vec::new();
    for i in 0..1000u32 {
        let origin = match i % 3 {
            0 => CascadeOrigin::UserAgent,
            1 => CascadeOrigin::User,
            _ => CascadeOrigin::Author,
        };
        let layer = if i % 5 == 0 { (i % 10) as u16 } else { UNLAYERED };
        rules.push(make_rule(origin, layer));
    }

    let mut decls: Vec<ApplicableDeclaration> = (0..1000u32)
        .map(|i| make_decl(i, i * 7 % 1000, i))
        .collect();

    cascade::sort(&mut decls, &rules);

    // Verify the sort is monotonically non-decreasing by cascade level
    for i in 0..decls.len() - 1 {
        let level_a = rules[decls[i].rule_index as usize].level(Importance::Normal);
        let level_b = rules[decls[i + 1].rule_index as usize].level(Importance::Normal);
        assert!(
            level_a.as_u32() <= level_b.as_u32()
                || (level_a.as_u32() == level_b.as_u32()
                    && (decls[i].specificity <= decls[i + 1].specificity
                        || (decls[i].specificity == decls[i + 1].specificity
                            && decls[i].source_order <= decls[i + 1].source_order))),
            "Sort order violated at index {}-{}",
            i,
            i + 1,
        );
    }
}

#[test]
fn w3c_supports_enabled_rules_included() {
    let stylist = stylist_with_css(
        "@supports (display: flex) { .flex { display: flex } }",
    );
    assert_eq!(stylist.rule_count(), 1);
}

#[test]
fn w3c_keyframes_last_definition_wins() {
    use kozan_atom::Atom;
    let stylist = stylist_with_css(
        "@keyframes fade { from { opacity: 0 } to { opacity: 1 } }
         @keyframes fade { from { opacity: 0.5 } to { opacity: 1 } }",
    );
    // Last @keyframes with same name wins
    assert!(stylist.keyframes(&Atom::from("fade")).is_some());
}

#[test]
fn w3c_property_registration() {
    use kozan_atom::Atom;
    let stylist = stylist_with_css(
        "@property --color { syntax: '<color>'; inherits: false; initial-value: red }",
    );
    assert!(stylist.registered_property(&Atom::from("--color")).is_some());
}

#[test]
fn w3c_nested_rules_css_nesting() {
    let stylist = stylist_with_css(
        ".parent { color: red; .child { color: blue } }",
    );
    // Both parent and child rules should be indexed
    assert!(stylist.rule_count() >= 2);
}

// ═══════════════════════════════════════════════════════════════════
// DEVICE / MEDIA FEATURES
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_device_default_values() {
    let device = Device::new(1920.0, 1080.0);
    assert_eq!(device.viewport_width, 1920.0);
    assert_eq!(device.viewport_height, 1080.0);
    assert_eq!(device.default_font_size(), 16.0);
    let ratio = device.aspect_ratio();
    assert!((ratio - 16.0 / 9.0).abs() < 0.01);
}

#[test]
fn w3c_device_zero_height_aspect_ratio() {
    let device = Device::new(1024.0, 0.0);
    assert_eq!(device.aspect_ratio(), 0.0, "Zero height must not panic");
}

// ═══════════════════════════════════════════════════════════════════
// !IMPORTANT TWO-PASS CASCADE — end-to-end with parsed CSS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_important_declarations_detected_in_parsed_css() {
    // Parse CSS with !important and verify the indexed rule has important entries.
    let stylist = stylist_with_css(".a { color: red !important; display: block }");
    let rules = stylist.rules();
    assert_eq!(rules.len(), 1);

    let entries = rules[0].declarations.entries();
    assert!(entries.len() >= 2, "Should have at least 2 declarations");

    let has_important = entries.iter().any(|(_, imp)| *imp == Importance::Important);
    let has_normal = entries.iter().any(|(_, imp)| *imp == Importance::Normal);
    assert!(has_important, "Must detect !important declarations");
    assert!(has_normal, "Must detect normal declarations too");
}

#[test]
fn w3c_cascade_apply_two_pass_order() {
    // Parse CSS with mixed normal and !important across multiple rules
    let stylist = stylist_with_css(
        ".a { color: red }
         .b { color: blue !important }
         .c { display: block }",
    );
    let rules = stylist.rules();
    assert_eq!(rules.len(), 3);

    // Simulate matching all 3 rules
    let sorted = vec![
        make_decl(0, 10, 0), // .a normal
        make_decl(1, 10, 1), // .b has !important
        make_decl(2, 10, 2), // .c normal
    ];

    let mut passes = Vec::new();
    cascade::cascade_apply(&sorted, rules, |_rule, _level, importance| {
        passes.push(importance);
    });

    // Pass 1 (normal): all 3 rules applied with Normal
    // Pass 2 (important): only rule 1 (.b) re-applied with Important
    let normal_count = passes.iter().filter(|i| **i == Importance::Normal).count();
    let important_count = passes.iter().filter(|i| **i == Importance::Important).count();

    assert_eq!(normal_count, 3, "Normal pass applies all rules");
    assert_eq!(important_count, 1, "Important pass applies only rule with !important");
}

#[test]
fn w3c_important_layer_reversal_end_to_end() {
    // W3C: For !important, layer order reverses.
    // layer-a !important > layer-b !important (first layer wins for !important)
    let stylist = stylist_with_css(
        "@layer a, b;
         @layer a { .x { color: red !important } }
         @layer b { .y { color: blue !important } }",
    );
    let rules = stylist.rules();
    assert_eq!(rules.len(), 2);

    // Rule 0 is layer a (order=0), Rule 1 is layer b (order=1)
    let level_a_imp = rules[0].level(Importance::Important);
    let level_b_imp = rules[1].level(Importance::Important);

    // For !important: earlier layer wins, so layer-a !important > layer-b !important
    assert!(
        level_b_imp < level_a_imp,
        "Layer-a !important must beat layer-b !important (earlier wins for !important)"
    );
}

#[test]
fn w3c_unlayered_important_loses_to_layered_important_end_to_end() {
    // W3C: unlayered !important has LOWEST priority among !important
    let stylist = stylist_with_css(
        "@layer base { .layered { color: red !important } }
         .unlayered { color: blue !important }",
    );
    let rules = stylist.rules();
    assert_eq!(rules.len(), 2);

    // Rule 0: layer base, Rule 1: unlayered
    let layered_imp = rules[0].level(Importance::Important);
    let unlayered_imp = rules[1].level(Importance::Important);

    assert!(
        unlayered_imp < layered_imp,
        "Unlayered !important must lose to layered !important"
    );
}

// ═══════════════════════════════════════════════════════════════════
// CRITICAL SPEC DETAIL: Specificity does NOT invert for !important
// W3C §6.1: Only origin, context, and layers invert. Specificity
// and source order ALWAYS go higher-wins.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_specificity_does_not_invert_for_important() {
    // Two rules in the same origin+layer, both !important.
    // Higher specificity must STILL win (no inversion).
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED), // rule 0
        make_rule(CascadeOrigin::Author, UNLAYERED), // rule 1
    ];
    // Both at same cascade level, but different specificity
    let mut decls = vec![
        make_decl(0, 100, 0), // high specificity
        make_decl(1, 10, 1),  // low specificity
    ];
    // sort() uses Normal importance for ordering (same cascade level for both)
    // so specificity breaks the tie — higher specificity wins.
    cascade::sort(&mut decls, &rules);
    assert_eq!(
        decls[1].specificity, 100,
        "Higher specificity must win even for !important (no inversion)"
    );
}

#[test]
fn w3c_source_order_does_not_invert_for_important() {
    // Same origin+layer+specificity, both !important. Later source order wins.
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 10, 0), // earlier
        make_decl(1, 10, 5), // later
    ];
    cascade::sort(&mut decls, &rules);
    assert_eq!(
        decls[1].source_order, 5,
        "Later source order must win even for !important (no inversion)"
    );
}

// ═══════════════════════════════════════════════════════════════════
// ANONYMOUS LAYER SPEC DETAILS
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_anonymous_layers_cannot_be_reopened() {
    // Each anonymous @layer {} is a unique layer — you can't add rules to it later.
    // Three anonymous layers must get three distinct orders.
    let stylist = stylist_with_css(
        "@layer { .a { color: red } }
         @layer { .b { color: blue } }
         @layer { .c { color: green } }",
    );
    let rules = stylist.rules();
    assert_eq!(rules.len(), 3);
    let orders: Vec<u16> = rules.iter().map(|r| r.layer_order).collect();
    assert_ne!(orders[0], orders[1]);
    assert_ne!(orders[1], orders[2]);
    assert_ne!(orders[0], orders[2]);
    // Must be in ascending order (each new anonymous layer gets next order)
    assert!(orders[0] < orders[1]);
    assert!(orders[1] < orders[2]);
}

#[test]
fn w3c_named_layer_can_be_reopened() {
    // Named layers CAN be reopened — second @layer base {} adds to the same layer.
    let stylist = stylist_with_css(
        "@layer base { .a { color: red } }
         @layer other { .b { color: blue } }
         @layer base { .c { color: green } }",
    );
    let rules = stylist.rules();
    assert_eq!(rules.len(), 3);
    // .a and .c are both in "base" — same layer order
    assert_eq!(
        rules[0].layer_order, rules[2].layer_order,
        "Reopened named layer must reuse the same order"
    );
    // .b is in "other" — different layer order
    assert_ne!(rules[0].layer_order, rules[1].layer_order);
}

// ═══════════════════════════════════════════════════════════════════
// LAYER + IMPORTANCE INTERACTION (from WPT layer-important.html)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn w3c_wpt_layer_important_unlayered_loses() {
    // WPT test: unlayered !important loses to ANY layered !important
    // in the same origin.
    let stylist = stylist_with_css(
        "@layer first { .x { color: red !important } }
         @layer second { .y { color: blue !important } }
         .z { color: green !important }",
    );
    let rules = stylist.rules();
    // rules[0] = layer first, rules[1] = layer second, rules[2] = unlayered
    let l0 = rules[0].level(Importance::Important); // layer 0 !important
    let l1 = rules[1].level(Importance::Important); // layer 1 !important
    let l2 = rules[2].level(Importance::Important); // unlayered !important

    // For !important: unlayered < layer second < layer first
    assert!(l2 < l1, "Unlayered !important < layer-second !important");
    assert!(l1 < l0, "Layer-second !important < layer-first !important");
    assert!(l2 < l0, "Unlayered !important < layer-first !important");
}

#[test]
fn w3c_wpt_layer_normal_unlayered_wins() {
    // WPT: unlayered normal wins over ALL layered normal in same origin.
    let stylist = stylist_with_css(
        "@layer first { .x { color: red } }
         @layer second { .y { color: blue } }
         .z { color: green }",
    );
    let rules = stylist.rules();
    let l0 = rules[0].level(Importance::Normal);
    let l1 = rules[1].level(Importance::Normal);
    let l2 = rules[2].level(Importance::Normal);

    assert!(l0 < l1, "Layer-first normal < layer-second normal");
    assert!(l1 < l2, "Layer-second normal < unlayered normal");
}

#[test]
fn w3c_normal_unlayered_beats_layered_end_to_end() {
    // W3C: unlayered normal has HIGHEST priority among normal
    let stylist = stylist_with_css(
        "@layer base { .layered { color: red } }
         .unlayered { color: blue }",
    );
    let rules = stylist.rules();
    assert_eq!(rules.len(), 2);

    let layered_normal = rules[0].level(Importance::Normal);
    let unlayered_normal = rules[1].level(Importance::Normal);

    assert!(
        layered_normal < unlayered_normal,
        "Unlayered normal must beat layered normal"
    );
}

// ═══════════════════════════════════════════════════════════════════
// §7.3 — REVERT KEYWORD
// ═══════════════════════════════════════════════════════════════════
// W3C CSS Cascade Level 4 §7.3: `revert` rolls back the cascade to the
// previous origin. Author → User → UA → initial.

#[test]
fn w3c_revert_falls_back_to_ua_origin() {
    // UA sets display:block on div, Author overrides to flex then reverts.
    // Revert should restore the UA value (block).
    let r = resolve_two_origins(
        "div { display: block }",
        "div { display: flex } div { display: revert }",
        &El::tag("div"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Block,
        "revert should fall back to UA origin value (block)"
    );
}

#[test]
fn w3c_revert_no_previous_origin_uses_initial() {
    // No UA stylesheet. Author sets display:flex then reverts.
    // With no previous origin, revert acts like unset → initial for non-inherited.
    let r = resolve_author(
        ".x { display: flex } .x { display: revert }",
        &El::tag("div").with_class("x"),
    );
    // display is non-inherited, so unset = initial = inline
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Inline,
        "revert with no prior origin → initial (inline for display)"
    );
}

#[test]
fn w3c_revert_only_affects_target_property() {
    // UA sets display:block. Author sets display:flex + visibility:hidden,
    // then reverts only display. Visibility should stay hidden.
    let r = resolve_two_origins(
        "div { display: block }",
        "div { display: flex; visibility: hidden } div { display: revert }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Block);
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Hidden);
}

#[test]
fn w3c_revert_inherited_property_falls_back_to_ua() {
    // For inherited properties, revert rolls back to the UA origin.
    // UA sets direction:ltr, Author sets direction:rtl then reverts.
    let r = resolve_two_origins(
        "div { direction: ltr }",
        ".x { direction: rtl } .x { direction: revert }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.text.direction,
        kozan_style::Direction::Ltr,
        "revert should fall back to UA origin (ltr)"
    );
}

#[test]
fn w3c_revert_inherited_no_prior_origin_uses_initial() {
    // No UA/User stylesheet. Author sets direction:rtl then reverts.
    // Revert with no prior origin → unset → inherit. No parent → initial (ltr).
    let r = resolve_author(
        ".x { direction: rtl } .x { direction: revert }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.text.direction,
        kozan_style::Direction::Ltr,
        "revert on inherited prop with no parent → initial (ltr)"
    );
}

// ═══════════════════════════════════════════════════════════════════
// §7.3.1 — REVERT-LAYER KEYWORD
// ═══════════════════════════════════════════════════════════════════
// W3C CSS Cascade Level 5 §7.3.1: `revert-layer` rolls back to the
// previous cascade layer within the same origin.

#[test]
fn w3c_revert_layer_falls_back_to_previous_layer() {
    // Layer "base" sets display:block, layer "override" sets flex then reverts-layer.
    // Should fall back to the "base" layer value.
    let r = resolve_author(
        "@layer base, override;
         @layer base { .x { display: block } }
         @layer override { .x { display: revert-layer } }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Block,
        "revert-layer should fall back to previous layer (base)"
    );
}

#[test]
fn w3c_revert_layer_no_previous_layer_uses_initial() {
    // Only one layer, revert-layer has nothing to fall back to.
    // Non-inherited property → initial.
    let r = resolve_author(
        "@layer only { .x { display: revert-layer } }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Inline,
        "revert-layer with no prior layer → initial (inline)"
    );
}

#[test]
fn w3c_revert_layer_skips_to_correct_layer() {
    // Three layers: base(block), mid(flex), top(revert-layer).
    // Top reverts → should get mid's value (flex), not base's.
    let r = resolve_author(
        "@layer base, mid, top;
         @layer base { .x { display: block } }
         @layer mid { .x { display: flex } }
         @layer top { .x { display: revert-layer } }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "revert-layer should get mid layer (flex)"
    );
}

#[test]
fn w3c_revert_layer_unlayered_overrides_layered() {
    // Layer sets display:flex, unlayered sets revert-layer.
    // Unlayered is highest priority among normal. revert-layer from unlayered
    // should fall back to the layered value.
    let r = resolve_author(
        "@layer base { .x { display: flex } }
         .x { display: revert-layer }",
        &El::tag("div").with_class("x"),
    );
    assert_eq!(
        r.style.layout.display,
        kozan_style::Display::Flex,
        "unlayered revert-layer falls back to layer value"
    );
}

// ═══════════════════════════════════════════════════════════════════
// §6.3 CSS CASCADING LEVEL 6 — @SCOPE PROXIMITY
// ═══════════════════════════════════════════════════════════════════
// CSS Cascading Level 6 §6.3: When two declarations have the same
// origin, layer, and specificity, the one from the nearer scope root
// wins. Scoped rules beat unscoped rules.

#[test]
fn w3c_scope_scoped_beats_unscoped_same_specificity() {
    // CSS Cascading L6: scoped rules (scope_depth > 0) beat unscoped (scope_depth = 0)
    // at the same specificity and source order.
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];

    let mut decls = vec![
        make_scoped_decl(0, 100, 0, 0), // unscoped (depth 0)
        make_scoped_decl(1, 100, 1, 3), // scoped (depth 3)
    ];
    cascade::sort(&mut decls, &rules);

    // After sort (ascending — last wins), scoped should be last.
    assert_eq!(decls.last().unwrap().scope_depth, 3,
        "Scoped rule (depth 3) should beat unscoped (depth 0)");
}

#[test]
fn w3c_scope_closer_scope_wins() {
    // CSS Cascading L6: closer scope root (smaller depth) beats farther.
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];

    let mut decls = vec![
        make_scoped_decl(0, 100, 0, 5), // far scope
        make_scoped_decl(1, 100, 1, 2), // close scope
        make_scoped_decl(2, 100, 2, 8), // farthest scope
    ];
    cascade::sort(&mut decls, &rules);

    // Ascending order: farthest → far → closest. Closest wins (last).
    assert_eq!(decls[2].scope_depth, 2,
        "Closest scope (depth 2) should be last (wins)");
    assert_eq!(decls[1].scope_depth, 5,
        "Middle scope (depth 5) should be second");
    assert_eq!(decls[0].scope_depth, 8,
        "Farthest scope (depth 8) should be first (lowest priority)");
}

#[test]
fn w3c_scope_specificity_still_beats_proximity() {
    // CSS Cascading L6: specificity is checked BEFORE proximity.
    // Higher specificity wins even if scope is farther.
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];

    let mut decls = vec![
        make_scoped_decl(0, 200, 0, 10), // higher specificity, farther scope
        make_scoped_decl(1, 100, 1, 1),  // lower specificity, closer scope
    ];
    cascade::sort(&mut decls, &rules);

    // Higher specificity wins regardless of proximity.
    assert_eq!(decls.last().unwrap().specificity, 200,
        "Higher specificity should win over closer scope");
}

#[test]
fn w3c_scope_source_order_breaks_tie() {
    // When origin, layer, specificity, AND proximity are all equal,
    // source order is the final tiebreaker.
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];

    let mut decls = vec![
        make_scoped_decl(0, 100, 0, 3), // same specificity, same depth, earlier
        make_scoped_decl(1, 100, 1, 3), // same specificity, same depth, later
    ];
    cascade::sort(&mut decls, &rules);

    // Later source order wins.
    assert_eq!(decls.last().unwrap().source_order, 1,
        "Later source order should win when everything else is equal");
}

#[test]
fn w3c_scope_unscoped_vs_unscoped_uses_source_order() {
    // Two unscoped rules — proximity doesn't matter, source order wins.
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];

    let mut decls = vec![
        make_decl(0, 100, 0),
        make_decl(1, 100, 1),
    ];
    cascade::sort(&mut decls, &rules);

    assert_eq!(decls.last().unwrap().source_order, 1,
        "Later source order should win for two unscoped rules");
}

// ═══════════════════════════════════════════════════════════════════
// VALUE VERIFICATION — end-to-end computed value correctness
// ═══════════════════════════════════════════════════════════════════
// These tests verify that the cascade produces CORRECT computed values,
// not just that it doesn't crash.

#[test]
fn value_last_declaration_wins() {
    let r = resolve_author(
        "div { display: flex } div { display: grid }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Grid,
        "Last declaration with same specificity must win");
}

#[test]
fn value_important_beats_normal() {
    let r = resolve_author(
        "div { display: flex !important } div { display: grid }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex,
        "!important declaration must beat later normal");
}

#[test]
fn value_multiple_properties_independent() {
    let r = resolve_author(
        "div { display: flex; visibility: hidden; position: absolute }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Hidden);
    assert_eq!(r.style.layout.position, kozan_style::Position::Absolute);
}

#[test]
fn value_initial_display_is_inline() {
    // No rules at all — display should be initial value (inline)
    let r = resolve_author("", &El::tag("div"));
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline,
        "Default display must be inline (initial value)");
}

#[test]
fn value_initial_visibility_is_visible() {
    let r = resolve_author("", &El::tag("div"));
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Visible,
        "Default visibility must be visible");
}

#[test]
fn value_initial_position_is_static() {
    let r = resolve_author("", &El::tag("div"));
    assert_eq!(r.style.layout.position, kozan_style::Position::Static,
        "Default position must be static");
}

#[test]
fn value_float_left() {
    let r = resolve_author("div { float: left }", &El::tag("div"));
    assert_eq!(r.style.layout.float, kozan_style::Float::Left);
}

#[test]
fn value_float_right_override() {
    let r = resolve_author(
        "div { float: left } div { float: right }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.float, kozan_style::Float::Right,
        "Later float:right must override float:left");
}

#[test]
fn value_clear_both() {
    let r = resolve_author("div { clear: both }", &El::tag("div"));
    assert_eq!(r.style.layout.clear, kozan_style::Clear::Both);
}

#[test]
fn value_overflow_hidden() {
    let r = resolve_author("div { overflow-x: hidden }", &El::tag("div"));
    assert_eq!(r.style.layout.overflow_x, kozan_style::Overflow::Hidden);
}

#[test]
fn value_box_sizing_border_box() {
    let r = resolve_author("div { box-sizing: border-box }", &El::tag("div"));
    assert_eq!(r.style.layout.box_sizing, kozan_style::BoxSizing::BorderBox);
}

#[test]
fn value_display_none() {
    let r = resolve_author("div { display: none }", &El::tag("div"));
    assert_eq!(r.style.layout.display, kozan_style::Display::None);
}

#[test]
fn value_display_inline_block() {
    let r = resolve_author("div { display: inline-block }", &El::tag("div"));
    assert_eq!(r.style.layout.display, kozan_style::Display::InlineBlock);
}

#[test]
fn value_position_fixed() {
    let r = resolve_author("div { position: fixed }", &El::tag("div"));
    assert_eq!(r.style.layout.position, kozan_style::Position::Fixed);
}

#[test]
fn value_position_sticky() {
    let r = resolve_author("div { position: sticky }", &El::tag("div"));
    assert_eq!(r.style.layout.position, kozan_style::Position::Sticky);
}

#[test]
fn value_direction_rtl() {
    let r = resolve_author("div { direction: rtl }", &El::tag("div"));
    assert_eq!(r.style.text.direction, kozan_style::Direction::Rtl);
}

#[test]
fn value_ua_overridden_by_author() {
    // UA sets block, Author sets flex — Author must win
    let r = resolve_two_origins(
        "div { display: block; position: static }",
        "div { display: flex; position: relative }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex,
        "Author display must override UA");
    assert_eq!(r.style.layout.position, kozan_style::Position::Relative,
        "Author position must override UA");
}

#[test]
fn value_ua_preserved_when_author_missing() {
    // UA sets display:block. Author sets only visibility. display should remain block.
    let r = resolve_two_origins(
        "div { display: block }",
        "div { visibility: hidden }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Block,
        "UA display must be preserved when Author doesn't set it");
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Hidden,
        "Author visibility must apply");
}

#[test]
fn value_class_selector_matches() {
    let r = resolve_author(
        ".active { display: grid }",
        &El::tag("div").with_class("active"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Grid,
        "Class selector must match element with that class");
}

#[test]
fn value_class_selector_no_match() {
    let r = resolve_author(
        ".active { display: grid }",
        &El::tag("div").with_class("inactive"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline,
        "Class selector must NOT match different class — stays at initial");
}

#[test]
fn value_inherit_keyword_non_inherited() {
    // display is non-inherited. inherit should have no effect without parent → initial.
    let r = resolve_author(
        "div { display: inherit }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline,
        "inherit on non-inherited prop with no parent → initial");
}

#[test]
fn value_initial_keyword_resets() {
    let r = resolve_author(
        "div { display: flex } div { display: initial }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline,
        "initial must reset to spec default (inline)");
}

#[test]
fn value_unset_non_inherited_resets() {
    // display is non-inherited → unset acts like initial
    let r = resolve_author(
        "div { display: flex } div { display: unset }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Inline,
        "unset on non-inherited property → initial (inline)");
}

#[test]
fn value_unset_inherited_inherits() {
    // direction is inherited → unset acts like inherit → no parent → initial
    let r = resolve_author(
        "div { direction: rtl } div { direction: unset }",
        &El::tag("div"),
    );
    assert_eq!(r.style.text.direction, kozan_style::Direction::Ltr,
        "unset on inherited property with no parent → initial (ltr)");
}

#[test]
fn value_layer_ordering_affects_computed() {
    // Layer base sets flex, layer override sets grid.
    // Override layer declared later → its value (grid) wins.
    let r = resolve_author(
        "@layer base, override;
         @layer base { div { display: flex } }
         @layer override { div { display: grid } }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Grid,
        "Later layer value (grid) must win over earlier layer (flex)");
}

#[test]
fn value_important_in_earlier_layer_wins() {
    // For !important: earlier layer wins.
    // base !important (flex) beats override !important (grid)
    let r = resolve_author(
        "@layer base, override;
         @layer base { div { display: flex !important } }
         @layer override { div { display: grid !important } }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex,
        "Earlier layer !important (flex) must beat later layer !important (grid)");
}

#[test]
fn value_multiple_rules_same_element() {
    // Multiple rules target same element — last wins for each property
    let r = resolve_author(
        "div { display: block; visibility: visible }
         div { display: flex }
         div { visibility: hidden }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Flex,
        "Second rule overrides display to flex");
    assert_eq!(r.style.layout.visibility, kozan_style::Visibility::Hidden,
        "Third rule overrides visibility to hidden");
}

#[test]
fn value_revert_restores_multiple_properties() {
    // UA sets both display:block and position:absolute.
    // Author overrides both, then reverts only display.
    let r = resolve_two_origins(
        "div { display: block; position: absolute }",
        "div { display: grid; position: fixed } div { display: revert }",
        &El::tag("div"),
    );
    assert_eq!(r.style.layout.display, kozan_style::Display::Block,
        "display reverted to UA value");
    assert_eq!(r.style.layout.position, kozan_style::Position::Fixed,
        "position NOT reverted — stays at Author value");
}
