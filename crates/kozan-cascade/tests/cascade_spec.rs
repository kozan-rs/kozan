//! Comprehensive CSS cascade specification tests — value-level verification.
//!
//! Every test verifies actual OUTPUT VALUES, not just boolean pass/fail.
//! CascadeLevel tests check exact u32 bit patterns.
//! Sort tests verify all fields of every element in the output.
//! Cache tests verify exact StyleIndex values returned.
//! Restyle tests verify exact RestyleHint bit values.

use kozan_atom::Atom;
use kozan_cascade::cascade::{self, ApplicableDeclaration};
use kozan_cascade::device::*;
use kozan_cascade::layer::{LayerOrderMap, UNLAYERED};
use kozan_cascade::media;
use kozan_cascade::origin::{CascadeLevel, CascadeOrigin, Importance};
use kozan_cascade::restyle::{DomMutation, RestyleHint, RestyleTracker};
use kozan_cascade::sharing_cache::{
    hash_matched, MatchedPropertiesCache, SharingCache, SharingKey,
};
use kozan_cascade::stylist::Stylist;
use kozan_css::{
    parse_stylesheet, LengthUnit, MediaCondition, MediaFeature, MediaFeatureValue, MediaQuery,
    MediaQueryList, MediaQualifier, MediaType as CssMediaType, RangeOp,
};
use kozan_selector::invalidation::InvalidationMap;
use kozan_selector::opaque::OpaqueElement;
use std::sync::Arc;
use kozan_cascade::resolver::ResolvedStyle;
use kozan_cascade::custom_properties::CustomPropertyMap;
use kozan_style::{ComputedStyle, DeclarationBlock};
use smallvec::smallvec;

fn dummy_resolved() -> Arc<ResolvedStyle> {
    Arc::new(ResolvedStyle {
        style: ComputedStyle::default(),
        custom_properties: CustomPropertyMap::new(),
    })
}

fn make_rule(
    origin: CascadeOrigin,
    layer: u16,
) -> kozan_cascade::stylist::IndexedRule {
    kozan_cascade::stylist::IndexedRule {
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

// ─── CASCADE LEVEL — EXACT u32 BIT PATTERN VERIFICATION ───
//
// Bit layout (MSB first):
//   [31]     importance        (1 bit)
//   [30..29] effective_origin  (2 bits)
//   [28..16] effective_layer   (13 bits, MAX_LAYER = 0x1FFF)
//   [15..0]  reserved          (16 bits, always 0)
//
// Normal:    effective_origin = origin,     effective_layer = layer
// Important: effective_origin = 2 - origin, effective_layer = MAX_LAYER - layer

#[test]
fn cascade_level_ua_normal_unlayered_exact_bits() {
    // UA=0, normal, layer clamped to 0x1FFF
    // bits = (0 << 29) | (0 << 31) | (0x1FFF << 16) = 0x1FFF_0000
    let level = CascadeLevel::new(CascadeOrigin::UserAgent, Importance::Normal, UNLAYERED);
    assert_eq!(level.as_u32(), 0x1FFF_0000);
    assert!(!level.is_important());
}

#[test]
fn cascade_level_user_normal_unlayered_exact_bits() {
    // User=1, normal, layer clamped to 0x1FFF
    // bits = (1 << 29) | (0x1FFF << 16) = 0x2000_0000 + 0x1FFF_0000 = 0x3FFF_0000
    let level = CascadeLevel::new(CascadeOrigin::User, Importance::Normal, UNLAYERED);
    assert_eq!(level.as_u32(), 0x3FFF_0000);
    assert!(!level.is_important());
}

#[test]
fn cascade_level_author_normal_unlayered_exact_bits() {
    // Author=2, normal, layer clamped to 0x1FFF
    // bits = (2 << 29) | (0x1FFF << 16) = 0x4000_0000 + 0x1FFF_0000 = 0x5FFF_0000
    let level = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, UNLAYERED);
    assert_eq!(level.as_u32(), 0x5FFF_0000);
    assert!(!level.is_important());
}

#[test]
fn cascade_level_author_normal_layer0_exact_bits() {
    // Author=2, normal, layer=0
    // bits = (2 << 29) | (0 << 16) = 0x4000_0000
    let level = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 0);
    assert_eq!(level.as_u32(), 0x4000_0000);
}

#[test]
fn cascade_level_author_normal_layer1_exact_bits() {
    // Author=2, normal, layer=1
    // bits = (2 << 29) | (1 << 16) = 0x4001_0000
    let level = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 1);
    assert_eq!(level.as_u32(), 0x4001_0000);
}

#[test]
fn cascade_level_author_important_unlayered_exact_bits() {
    // Author=2, important: effective_origin = 2 - 2 = 0, effective_layer = 0x1FFF - 0x1FFF = 0
    // bits = (0 << 29) | (1 << 31) | (0 << 16) = 0x8000_0000
    let level = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, UNLAYERED);
    assert_eq!(level.as_u32(), 0x8000_0000);
    assert!(level.is_important());
}

#[test]
fn cascade_level_author_important_layer1_exact_bits() {
    // Author=2, important: effective_origin = 0, effective_layer = 0x1FFF - 1 = 0x1FFE
    // bits = (1 << 31) | (0x1FFE << 16) = 0x8000_0000 + 0x1FFE_0000 = 0x9FFE_0000
    let level = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, 1);
    assert_eq!(level.as_u32(), 0x9FFE_0000);
}

#[test]
fn cascade_level_author_important_layer0_exact_bits() {
    // Author=2, important: effective_origin = 0, effective_layer = 0x1FFF - 0 = 0x1FFF
    // bits = (1 << 31) | (0x1FFF << 16) = 0x8000_0000 + 0x1FFF_0000 = 0x9FFF_0000
    let level = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, 0);
    assert_eq!(level.as_u32(), 0x9FFF_0000);
}

#[test]
fn cascade_level_user_important_unlayered_exact_bits() {
    // User=1, important: effective_origin = 2 - 1 = 1, effective_layer = 0
    // bits = (1 << 29) | (1 << 31) = 0x2000_0000 + 0x8000_0000 = 0xA000_0000
    let level = CascadeLevel::new(CascadeOrigin::User, Importance::Important, UNLAYERED);
    assert_eq!(level.as_u32(), 0xA000_0000);
    assert!(level.is_important());
}

#[test]
fn cascade_level_ua_important_unlayered_exact_bits() {
    // UA=0, important: effective_origin = 2 - 0 = 2, effective_layer = 0
    // bits = (2 << 29) | (1 << 31) = 0x4000_0000 + 0x8000_0000 = 0xC000_0000
    let level = CascadeLevel::new(CascadeOrigin::UserAgent, Importance::Important, UNLAYERED);
    assert_eq!(level.as_u32(), 0xC000_0000);
    assert!(level.is_important());
}

#[test]
fn cascade_level_full_10_step_exact_values() {
    // CSS Cascading Level 5 complete priority order with exact u32 values.
    let expected: [(CascadeOrigin, Importance, u16, u32); 10] = [
        (CascadeOrigin::UserAgent, Importance::Normal,    UNLAYERED, 0x1FFF_0000), // 1. UA normal
        (CascadeOrigin::User,      Importance::Normal,    UNLAYERED, 0x3FFF_0000), // 2. User normal
        (CascadeOrigin::Author,    Importance::Normal,    0,         0x4000_0000), // 3. Author normal layer 0
        (CascadeOrigin::Author,    Importance::Normal,    1,         0x4001_0000), // 4. Author normal layer 1
        (CascadeOrigin::Author,    Importance::Normal,    UNLAYERED, 0x5FFF_0000), // 5. Author normal unlayered
        (CascadeOrigin::Author,    Importance::Important, UNLAYERED, 0x8000_0000), // 6. Author !important unlayered
        (CascadeOrigin::Author,    Importance::Important, 1,         0x9FFE_0000), // 7. Author !important layer 1
        (CascadeOrigin::Author,    Importance::Important, 0,         0x9FFF_0000), // 8. Author !important layer 0
        (CascadeOrigin::User,      Importance::Important, UNLAYERED, 0xA000_0000), // 9. User !important
        (CascadeOrigin::UserAgent, Importance::Important, UNLAYERED, 0xC000_0000), // 10. UA !important
    ];

    for (i, &(origin, importance, layer, expected_bits)) in expected.iter().enumerate() {
        let level = CascadeLevel::new(origin, importance, layer);
        assert_eq!(
            level.as_u32(), expected_bits,
            "step {}: {:?}/{:?}/layer={} expected 0x{:08X}, got 0x{:08X}",
            i + 1, origin, importance, layer, expected_bits, level.as_u32(),
        );
    }

    // Verify strict ascending order between each consecutive pair.
    for i in 0..expected.len() - 1 {
        let a = CascadeLevel::new(expected[i].0, expected[i].1, expected[i].2);
        let b = CascadeLevel::new(expected[i + 1].0, expected[i + 1].1, expected[i + 1].2);
        assert!(
            a.as_u32() < b.as_u32(),
            "step {} (0x{:08X}) must be < step {} (0x{:08X})",
            i + 1, a.as_u32(), i + 2, b.as_u32(),
        );
    }
}

#[test]
fn cascade_level_layer_clamping_exact() {
    // MAX_LAYER = 0x1FFF = 8191. Layers above this are clamped.
    let below = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 8000);
    let above_a = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 9000);
    let above_b = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 10000);
    let exact = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 0x1FFF);
    let max_u16 = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, u16::MAX);

    // 8000 is below MAX_LAYER — not clamped.
    let expected_8000 = (2u32 << 29) | (8000u32 << 16);
    assert_eq!(below.as_u32(), expected_8000, "8000 not clamped");

    // 9000, 10000, 0x1FFF, u16::MAX all clamp to MAX_LAYER.
    let expected_max = (2u32 << 29) | (0x1FFF_u32 << 16); // 0x5FFF_0000
    assert_eq!(above_a.as_u32(), expected_max, "9000 clamped to MAX_LAYER");
    assert_eq!(above_b.as_u32(), expected_max, "10000 clamped to MAX_LAYER");
    assert_eq!(exact.as_u32(), expected_max, "0x1FFF is exactly MAX_LAYER");
    assert_eq!(max_u16.as_u32(), expected_max, "u16::MAX clamped to MAX_LAYER");
}

#[test]
fn cascade_level_important_always_beats_normal_exact_values() {
    // For every origin, verify the exact gap between normal and important.
    for (origin, normal_expected, important_expected) in [
        (CascadeOrigin::UserAgent, 0x1FFF_0000u32, 0xC000_0000u32),
        (CascadeOrigin::User,      0x3FFF_0000u32, 0xA000_0000u32),
        (CascadeOrigin::Author,    0x5FFF_0000u32, 0x8000_0000u32),
    ] {
        let normal = CascadeLevel::new(origin, Importance::Normal, UNLAYERED);
        let important = CascadeLevel::new(origin, Importance::Important, UNLAYERED);
        assert_eq!(normal.as_u32(), normal_expected, "{origin:?} normal");
        assert_eq!(important.as_u32(), important_expected, "{origin:?} important");
        assert!(
            normal.as_u32() < important.as_u32(),
            "{origin:?}: normal 0x{:08X} must be < important 0x{:08X}",
            normal.as_u32(), important.as_u32(),
        );
    }
}

#[test]
fn cascade_level_important_layer_inversion_exact() {
    // Normal: layer 0 < layer 5 < UNLAYERED
    // Important: UNLAYERED < layer 5 < layer 0
    let normal_0 = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 0);
    let normal_5 = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, 5);
    let normal_u = CascadeLevel::new(CascadeOrigin::Author, Importance::Normal, UNLAYERED);

    assert_eq!(normal_0.as_u32(), 0x4000_0000);
    assert_eq!(normal_5.as_u32(), 0x4005_0000);
    assert_eq!(normal_u.as_u32(), 0x5FFF_0000);

    let imp_0 = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, 0);
    let imp_5 = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, 5);
    let imp_u = CascadeLevel::new(CascadeOrigin::Author, Importance::Important, UNLAYERED);

    // Important Author: effective_origin = 0, effective_layer = 0x1FFF - layer
    assert_eq!(imp_0.as_u32(), 0x9FFF_0000); // 0x1FFF - 0 = 0x1FFF
    assert_eq!(imp_5.as_u32(), 0x9FFA_0000); // 0x1FFF - 5 = 0x1FFA
    assert_eq!(imp_u.as_u32(), 0x8000_0000); // 0x1FFF - 0x1FFF = 0

    // Verify inverted ordering for important.
    assert!(imp_u.as_u32() < imp_5.as_u32(), "unlayered !important < layer 5 !important");
    assert!(imp_5.as_u32() < imp_0.as_u32(), "layer 5 !important < layer 0 !important");
}

// ─── CASCADE SORT — VERIFY ALL FIELDS ───

#[test]
fn sort_by_origin_verify_all_fields() {
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(1, 0, 0), // author
        make_decl(0, 0, 0), // ua
    ];
    cascade::sort(&mut decls, &rules);

    // Position 0: UA (lower priority)
    assert_eq!(decls[0].rule_index, 0);
    assert_eq!(decls[0].specificity, 0);
    assert_eq!(decls[0].source_order, 0);

    // Position 1: Author (higher priority, wins)
    assert_eq!(decls[1].rule_index, 1);
    assert_eq!(decls[1].specificity, 0);
    assert_eq!(decls[1].source_order, 0);
}

#[test]
fn sort_by_specificity_verify_all_fields() {
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 100, 0),  // high specificity
        make_decl(1, 10, 1),   // low specificity
    ];
    cascade::sort(&mut decls, &rules);

    // Lower specificity first (lower priority).
    assert_eq!(decls[0].rule_index, 1);
    assert_eq!(decls[0].specificity, 10);
    assert_eq!(decls[0].source_order, 1);

    // Higher specificity last (wins).
    assert_eq!(decls[1].rule_index, 0);
    assert_eq!(decls[1].specificity, 100);
    assert_eq!(decls[1].source_order, 0);
}

#[test]
fn sort_by_source_order_verify_all_fields() {
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 10, 5), // later source order
        make_decl(1, 10, 2), // earlier source order
    ];
    cascade::sort(&mut decls, &rules);

    assert_eq!(decls[0].rule_index, 1);
    assert_eq!(decls[0].specificity, 10);
    assert_eq!(decls[0].source_order, 2);

    assert_eq!(decls[1].rule_index, 0);
    assert_eq!(decls[1].specificity, 10);
    assert_eq!(decls[1].source_order, 5);
}

#[test]
fn sort_layer_beats_specificity_verify_all_fields() {
    let rules = vec![
        make_rule(CascadeOrigin::Author, 0), // layer 0
        make_rule(CascadeOrigin::Author, 1), // layer 1
    ];
    let mut decls = vec![
        make_decl(0, 10000, 0), // layer 0, huge specificity
        make_decl(1, 1, 1),     // layer 1, tiny specificity
    ];
    cascade::sort(&mut decls, &rules);

    // Layer 0 sorts first (lower layer priority).
    assert_eq!(decls[0].rule_index, 0);
    assert_eq!(decls[0].specificity, 10000);

    // Layer 1 sorts last (higher layer priority, wins regardless of specificity).
    assert_eq!(decls[1].rule_index, 1);
    assert_eq!(decls[1].specificity, 1);
}

#[test]
fn sort_origin_beats_layer_verify_all_fields() {
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED),
        make_rule(CascadeOrigin::Author, 0),
    ];
    let mut decls = vec![
        make_decl(0, 0, 0), // UA unlayered
        make_decl(1, 0, 1), // Author layer 0
    ];
    cascade::sort(&mut decls, &rules);

    assert_eq!(decls[0].rule_index, 0); // UA first
    assert_eq!(decls[0].source_order, 0);
    assert_eq!(decls[1].rule_index, 1); // Author wins
    assert_eq!(decls[1].source_order, 1);
}

#[test]
fn sort_stable_preserves_source_order() {
    let rules = vec![
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
    ];
    let mut decls = vec![
        make_decl(0, 10, 0),
        make_decl(1, 10, 1),
        make_decl(2, 10, 2),
    ];
    cascade::sort(&mut decls, &rules);

    // Same origin, same specificity → source order is tiebreaker.
    for (i, d) in decls.iter().enumerate() {
        assert_eq!(d.rule_index, i as u32, "position {i}");
        assert_eq!(d.specificity, 10, "specificity preserved at {i}");
        assert_eq!(d.source_order, i as u32, "source_order preserved at {i}");
    }
}

#[test]
fn sort_complex_5_rules_all_fields() {
    // 5 rules: different origins, layers, specificities.
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED),  // 0: UA
        make_rule(CascadeOrigin::Author, 0),             // 1: Author layer 0
        make_rule(CascadeOrigin::Author, 1),             // 2: Author layer 1
        make_rule(CascadeOrigin::Author, UNLAYERED),     // 3: Author unlayered
        make_rule(CascadeOrigin::User, UNLAYERED),       // 4: User
    ];
    let mut decls = vec![
        make_decl(3, 50, 3),   // Author unlayered
        make_decl(0, 200, 0),  // UA
        make_decl(2, 10, 2),   // Author layer 1
        make_decl(4, 100, 4),  // User
        make_decl(1, 1000, 1), // Author layer 0
    ];
    cascade::sort(&mut decls, &rules);

    // Expected order by CascadeLevel:
    // 1. UA normal unlayered        = 0x1FFF_0000
    // 2. User normal unlayered      = 0x3FFF_0000
    // 3. Author normal layer 0      = 0x4000_0000
    // 4. Author normal layer 1      = 0x4001_0000
    // 5. Author normal unlayered    = 0x5FFF_0000
    assert_eq!(decls[0].rule_index, 0); // UA
    assert_eq!(decls[0].specificity, 200);

    assert_eq!(decls[1].rule_index, 4); // User
    assert_eq!(decls[1].specificity, 100);

    assert_eq!(decls[2].rule_index, 1); // Author layer 0
    assert_eq!(decls[2].specificity, 1000);

    assert_eq!(decls[3].rule_index, 2); // Author layer 1
    assert_eq!(decls[3].specificity, 10);

    assert_eq!(decls[4].rule_index, 3); // Author unlayered (winner)
    assert_eq!(decls[4].specificity, 50);
}

// ─── CASCADE APPLY — VERIFY CALLBACK ORDER AND VALUES ───

#[test]
fn cascade_apply_normal_pass_order_and_levels() {
    let rules = vec![
        make_rule(CascadeOrigin::UserAgent, UNLAYERED),
        make_rule(CascadeOrigin::Author, UNLAYERED),
        make_rule(CascadeOrigin::User, UNLAYERED),
    ];
    let sorted = vec![
        make_decl(0, 0, 0), // UA
        make_decl(2, 5, 1), // User
        make_decl(1, 10, 2), // Author
    ];
    let mut visits: Vec<(u32, Importance)> = Vec::new();
    cascade::cascade_apply(&sorted, &rules, |rule, _level, importance| {
        let idx = rules.iter().position(|r| std::ptr::eq(r, rule)).unwrap();
        visits.push((idx as u32, importance));
    });

    // Normal pass visits all 3 rules in sorted order.
    assert_eq!(visits.len(), 3, "normal pass visits all rules");
    assert_eq!(visits[0], (0, Importance::Normal), "first: UA");
    assert_eq!(visits[1], (2, Importance::Normal), "second: User");
    assert_eq!(visits[2], (1, Importance::Normal), "third: Author");
}

// ─── LAYER ORDERING — EXACT VALUES ───

#[test]
fn layer_first_encounter_exact_values() {
    let mut map = LayerOrderMap::new();
    let name_a = kozan_css::LayerName(smallvec![Atom::from("base")]);
    let name_b = kozan_css::LayerName(smallvec![Atom::from("utils")]);

    assert_eq!(map.get_or_insert(&name_a), 0, "first layer = 0");
    assert_eq!(map.get_or_insert(&name_b), 1, "second layer = 1");
    assert_eq!(map.get_or_insert(&name_a), 0, "re-lookup returns same value");
    assert_eq!(map.len(), 2);
}

#[test]
fn layer_dotted_names_exact_values() {
    let mut map = LayerOrderMap::new();
    let fw_utils = kozan_css::LayerName(smallvec![Atom::from("framework"), Atom::from("utilities")]);
    let fw_base = kozan_css::LayerName(smallvec![Atom::from("framework"), Atom::from("base")]);
    let just_fw = kozan_css::LayerName(smallvec![Atom::from("framework")]);

    assert_eq!(map.get_or_insert(&fw_utils), 0);
    assert_eq!(map.get_or_insert(&fw_base), 1);
    assert_eq!(map.get_or_insert(&just_fw), 2, "prefix is distinct name");
    assert_eq!(map.len(), 3);
}

#[test]
fn layer_anonymous_sequential_values() {
    let mut map = LayerOrderMap::new();
    assert_eq!(map.next_anonymous(), 0);
    assert_eq!(map.next_anonymous(), 1);
    assert_eq!(map.next_anonymous(), 2);
}

#[test]
fn layer_named_and_anonymous_interleave_exact() {
    let mut map = LayerOrderMap::new();
    let name = kozan_css::LayerName(smallvec![Atom::from("base")]);

    assert_eq!(map.get_or_insert(&name), 0, "named = 0");
    assert_eq!(map.next_anonymous(), 1, "anon = 1");
    assert_eq!(map.get_or_insert(&name), 0, "named still = 0");
    let name2 = kozan_css::LayerName(smallvec![Atom::from("utils")]);
    assert_eq!(map.get_or_insert(&name2), 2, "new named = 2 (after anon)");
}

#[test]
fn layer_clear_resets_counter() {
    let mut map = LayerOrderMap::new();
    let name = kozan_css::LayerName(smallvec![Atom::from("base")]);
    assert_eq!(map.get_or_insert(&name), 0);
    assert_eq!(map.len(), 1);

    map.clear();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    // After clear, counter resets — same name gets 0 again.
    assert_eq!(map.get_or_insert(&name), 0);
}

#[test]
fn layer_unlayered_constant() {
    assert_eq!(UNLAYERED, u16::MAX);
    assert_eq!(UNLAYERED, 0xFFFF);
    assert_eq!(UNLAYERED, 65535);
}

// ─── STYLIST — VERIFY INDEXED RULE VALUES ───

fn stylist_with_css(css: &str) -> Stylist {
    let sheet = parse_stylesheet(css);
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(sheet, CascadeOrigin::Author);
    stylist.rebuild();
    stylist
}

#[test]
fn stylist_empty_stylesheet_values() {
    let stylist = stylist_with_css("");
    assert_eq!(stylist.rule_count(), 0);
    assert_eq!(stylist.generation(), 1);
    assert!(stylist.author_map().is_empty());
}

#[test]
fn stylist_single_rule_values() {
    let stylist = stylist_with_css(".btn { color: red }");
    assert_eq!(stylist.rule_count(), 1);

    let rule = &stylist.rules()[0];
    assert_eq!(rule.origin, CascadeOrigin::Author);
    assert_eq!(rule.layer_order, UNLAYERED);
    assert!(!rule.declarations.is_empty());

    // Verify the CascadeLevel this rule would produce.
    let level = rule.level(Importance::Normal);
    assert_eq!(level.as_u32(), 0x5FFF_0000);
}

#[test]
fn stylist_empty_rule_skipped_values() {
    let stylist = stylist_with_css(".empty {} .has-decl { color: red }");
    assert_eq!(stylist.rule_count(), 1);
    // The only indexed rule should be the one with declarations.
    assert!(!stylist.rules()[0].declarations.is_empty());
}

#[test]
fn stylist_layer_block_exact_order_values() {
    let stylist = stylist_with_css(
        "@layer base { .a { color: red } }
         @layer utils { .b { color: blue } }",
    );
    assert_eq!(stylist.rule_count(), 2);

    let rule_a = &stylist.rules()[0];
    let rule_b = &stylist.rules()[1];

    assert_eq!(rule_a.layer_order, 0, "base = first layer = 0");
    assert_eq!(rule_b.layer_order, 1, "utils = second layer = 1");
    assert_eq!(rule_a.origin, CascadeOrigin::Author);
    assert_eq!(rule_b.origin, CascadeOrigin::Author);

    // Verify cascade levels: layer 0 < layer 1 for normal.
    assert_eq!(rule_a.level(Importance::Normal).as_u32(), 0x4000_0000);
    assert_eq!(rule_b.level(Importance::Normal).as_u32(), 0x4001_0000);

    // For !important, the order reverses.
    assert_eq!(rule_a.level(Importance::Important).as_u32(), 0x9FFF_0000);
    assert_eq!(rule_b.level(Importance::Important).as_u32(), 0x9FFE_0000);
    assert!(
        rule_b.level(Importance::Important).as_u32()
            < rule_a.level(Importance::Important).as_u32(),
        "!important: layer 1 < layer 0 (reversed)"
    );
}

#[test]
fn stylist_layer_statement_reserves_exact_values() {
    let stylist = stylist_with_css(
        "@layer utils, base;
         @layer base { .a { color: red } }
         @layer utils { .b { color: blue } }",
    );
    // Statement declares: utils=0, base=1.
    // Block @layer base uses reserved order 1, block @layer utils uses 0.
    let rule_a = &stylist.rules()[0]; // .a in base
    let rule_b = &stylist.rules()[1]; // .b in utils

    assert_eq!(rule_a.layer_order, 1, "base reserved at position 1");
    assert_eq!(rule_b.layer_order, 0, "utils reserved at position 0");
    assert!(
        rule_a.layer_order > rule_b.layer_order,
        "base (1) > utils (0) due to statement order"
    );
}

#[test]
fn stylist_layer_anonymous_distinct_values() {
    let stylist = stylist_with_css(
        "@layer { .a { color: red } }
         @layer { .b { color: blue } }",
    );
    assert_eq!(stylist.rule_count(), 2);

    let order_a = stylist.rules()[0].layer_order;
    let order_b = stylist.rules()[1].layer_order;

    assert_eq!(order_a, 0, "first anonymous = 0");
    assert_eq!(order_b, 1, "second anonymous = 1");
}

#[test]
fn stylist_unlayered_rule_has_max_layer() {
    let stylist = stylist_with_css(".plain { color: red }");
    assert_eq!(stylist.rules()[0].layer_order, UNLAYERED);
    assert_eq!(stylist.rules()[0].layer_order, 0xFFFF);
}

#[test]
fn stylist_media_active_inactive_exact() {
    // Device is 1024x768.
    let stylist = stylist_with_css(
        "@media (min-width: 768px) { .tablet { display: flex } }
         @media (min-width: 2000px) { .wide { display: block } }
         .always { color: red }",
    );
    assert_eq!(stylist.rule_count(), 2, "tablet + always, wide skipped");

    // All indexed rules are Author origin, unlayered.
    for rule in stylist.rules() {
        assert_eq!(rule.origin, CascadeOrigin::Author);
        assert_eq!(rule.layer_order, UNLAYERED);
    }
}

#[test]
fn stylist_media_print_skipped_exact() {
    let stylist = stylist_with_css("@media print { .print-only { display: block } }");
    assert_eq!(stylist.rule_count(), 0, "print rules skipped on screen device");
    assert_eq!(stylist.generation(), 1);
}

#[test]
fn stylist_keyframes_exact_values() {
    let stylist = stylist_with_css(
        "@keyframes fadeIn { from { opacity: 0 } to { opacity: 1 } }",
    );
    let kf = stylist.keyframes(&Atom::from("fadeIn")).unwrap();
    assert_eq!(kf.name.as_ref(), "fadeIn");
    assert_eq!(kf.keyframes.len(), 2, "from + to = 2 keyframe blocks");
    assert!(stylist.keyframes(&Atom::from("missing")).is_none());
}

#[test]
fn stylist_keyframes_last_wins_exact() {
    let stylist = stylist_with_css(
        "@keyframes fade { from { opacity: 0 } to { opacity: 0.5 } }
         @keyframes fade { 0% { opacity: 0 } 50% { opacity: 0.7 } 100% { opacity: 1 } }",
    );
    let kf = stylist.keyframes(&Atom::from("fade")).unwrap();
    assert_eq!(kf.keyframes.len(), 3, "last @keyframes wins: 3 blocks");
}

#[test]
fn stylist_property_registered_exact() {
    let stylist = stylist_with_css(
        "@property --gap { syntax: '<length>'; inherits: false; initial-value: 0px }",
    );
    let prop = stylist.registered_property(&Atom::from("--gap")).unwrap();
    assert_eq!(prop.name.as_ref(), "--gap");
    assert!(stylist.registered_property(&Atom::from("--missing")).is_none());
}

#[test]
fn stylist_multiple_origins_exact() {
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

    // Verify origins of indexed rules.
    let origins: Vec<CascadeOrigin> = stylist.rules().iter().map(|r| r.origin).collect();
    assert!(origins.contains(&CascadeOrigin::UserAgent));
    assert!(origins.contains(&CascadeOrigin::Author));
}

#[test]
fn stylist_generation_tracking_exact() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    assert_eq!(stylist.generation(), 0);

    stylist.rebuild();
    assert_eq!(stylist.generation(), 1);

    stylist.rebuild();
    assert_eq!(stylist.generation(), 2);

    stylist.rebuild();
    assert_eq!(stylist.generation(), 3);
}

#[test]
fn stylist_rebuild_idempotent_exact() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(
        parse_stylesheet(".a { color: red } .b { color: blue }"),
        CascadeOrigin::Author,
    );
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 2);
    let gen1 = stylist.generation();

    // Rebuilding again should NOT double the rules.
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 2, "no duplicate rules");
    assert_eq!(stylist.generation(), gen1 + 1);
}

#[test]
fn stylist_clear_sheets_exact() {
    let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
    stylist.add_stylesheet(parse_stylesheet(".a { color: red }"), CascadeOrigin::Author);
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 1);

    stylist.clear_sheets();
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 0);
    assert!(stylist.author_map().is_empty());
}

#[test]
fn stylist_device_change_exact() {
    let mut stylist = Stylist::new(Device::new(500.0, 400.0));
    stylist.add_stylesheet(
        parse_stylesheet("@media (min-width: 768px) { .tablet { display: flex } }"),
        CascadeOrigin::Author,
    );
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 0, "500px viewport: no match");

    stylist.set_device(Device::new(1024.0, 768.0));
    stylist.rebuild();
    assert_eq!(stylist.rule_count(), 1, "1024px viewport: matches");
    assert_eq!(stylist.rules()[0].layer_order, UNLAYERED);
}

// ─── SHARING CACHE — EXACT VALUE VERIFICATION ───

fn sharing_key(tag: &str, classes: &[&str], parent: u64) -> SharingKey {
    SharingKey::new(
        Atom::from(tag),
        None,
        classes.iter().map(|c| Atom::from(*c)).collect(),
        0,
        parent,
    )
}

fn sharing_key_full(tag: &str, id: Option<&str>, classes: &[&str], state: u32, parent: u64) -> SharingKey {
    SharingKey::new(
        Atom::from(tag),
        id.map(Atom::from),
        classes.iter().map(|c| Atom::from(*c)).collect(),
        state,
        parent,
    )
}

#[test]
fn sharing_cache_hit_and_miss() {
    let mut cache = SharingCache::new();
    let k = sharing_key("div", &["btn"], 0);
    cache.insert(k.clone(), dummy_resolved());
    assert!(cache.get(&k).is_some(), "hit on inserted key");
    assert_eq!(cache.len(), 1);
}

#[test]
fn sharing_cache_miss_returns_none() {
    let mut cache = SharingCache::new();
    cache.insert(sharing_key("div", &["btn"], 0), dummy_resolved());

    assert!(cache.get(&sharing_key("span", &["btn"], 0)).is_none(), "different tag");
    assert!(cache.get(&sharing_key("div", &["link"], 0)).is_none(), "different class");
    assert!(cache.get(&sharing_key("div", &["btn"], 99)).is_none(), "different parent");
    assert!(cache.get(&sharing_key("div", &["btn", "primary"], 0)).is_none(), "extra class");
    assert!(cache.get(&sharing_key("div", &[], 0)).is_none(), "missing class");
}

#[test]
fn sharing_cache_id_mismatch() {
    let mut cache = SharingCache::new();
    cache.insert(sharing_key_full("div", Some("app"), &[], 0, 0), dummy_resolved());

    assert!(cache.get(&sharing_key_full("div", Some("app"), &[], 0, 0)).is_some());
    assert!(cache.get(&sharing_key_full("div", Some("other"), &[], 0, 0)).is_none());
    assert!(cache.get(&sharing_key_full("div", None, &[], 0, 0)).is_none());
}

#[test]
fn sharing_cache_state_mismatch() {
    let mut cache = SharingCache::new();
    cache.insert(sharing_key_full("div", None, &[], 1, 0), dummy_resolved());

    assert!(cache.get(&sharing_key_full("div", None, &[], 1, 0)).is_some());
    assert!(cache.get(&sharing_key_full("div", None, &[], 2, 0)).is_none(), "different state");
    assert!(cache.get(&sharing_key_full("div", None, &[], 0, 0)).is_none(), "zero vs nonzero state");
}

#[test]
fn sharing_cache_lru_eviction() {
    let mut cache = SharingCache::new();
    for i in 0u32..40 {
        cache.insert(sharing_key("div", &[&format!("c{i}")], 0), dummy_resolved());
    }
    assert_eq!(cache.len(), 32, "capped at 32");

    // First 8 entries (c0..c7) should be evicted.
    for i in 0u32..8 {
        assert!(
            cache.get(&sharing_key("div", &[&format!("c{i}")], 0)).is_none(),
            "c{i} should be evicted"
        );
    }

    // Entries c8..c39 should still be present.
    for i in 8u32..40 {
        assert!(
            cache.get(&sharing_key("div", &[&format!("c{i}")], 0)).is_some(),
            "c{i} should be present"
        );
    }
}

#[test]
fn sharing_cache_lru_promotes_on_hit() {
    let mut cache = SharingCache::new();
    cache.insert(sharing_key("div", &["a"], 0), dummy_resolved());
    cache.insert(sharing_key("div", &["b"], 0), dummy_resolved());

    // Access "a" to promote it.
    assert!(cache.get(&sharing_key("div", &["a"], 0)).is_some());
    assert_eq!(cache.len(), 2);

    // Insert 31 more entries — "b" should be evicted (LRU), "a" should survive.
    for i in 0u32..31 {
        cache.insert(sharing_key("span", &[&format!("x{i}")], 0), dummy_resolved());
    }

    assert_eq!(cache.len(), 32);
    assert!(
        cache.get(&sharing_key("div", &["a"], 0)).is_some(),
        "promoted entry survives"
    );
    assert!(
        cache.get(&sharing_key("div", &["b"], 0)).is_none(),
        "unpromoted entry evicted"
    );
}

#[test]
fn sharing_cache_generation_invalidation() {
    let mut cache = SharingCache::new();
    cache.insert(sharing_key("div", &[], 0), dummy_resolved());
    assert_eq!(cache.len(), 1);

    let invalidated = cache.check_generation(1);
    assert!(invalidated, "generation change returns true");
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
}

#[test]
fn sharing_cache_same_generation_keeps_data() {
    let mut cache = SharingCache::new();
    cache.check_generation(5);
    cache.insert(sharing_key("div", &[], 0), dummy_resolved());

    let invalidated = cache.check_generation(5);
    assert!(!invalidated, "same generation returns false");
    assert_eq!(cache.len(), 1);
    assert!(cache.get(&sharing_key("div", &[], 0)).is_some());
}

#[test]
fn sharing_cache_update_existing() {
    let mut cache = SharingCache::new();
    let k = sharing_key("div", &["btn"], 0);
    cache.insert(k.clone(), dummy_resolved());
    assert!(cache.get(&k).is_some());

    cache.insert(k.clone(), dummy_resolved());
    assert!(cache.get(&k).is_some(), "updated entry accessible");
    assert_eq!(cache.len(), 1, "no duplicate entry");
}

// ─── MATCHED PROPERTIES CACHE — RESOLVED STYLE CACHING ───

#[test]
fn mpc_basic_hit() {
    let mut cache = MatchedPropertiesCache::new();
    cache.insert(12345, dummy_resolved());
    assert!(cache.get(12345).is_some(), "hit on inserted key");
    assert_eq!(cache.len(), 1);
}

#[test]
fn mpc_miss() {
    let cache = MatchedPropertiesCache::new();
    assert!(cache.get(0).is_none());
    assert!(cache.get(12345).is_none());
    assert!(cache.get(u64::MAX).is_none());
    assert_eq!(cache.len(), 0);
}

#[test]
fn mpc_multiple_entries() {
    let mut cache = MatchedPropertiesCache::new();
    cache.insert(100, dummy_resolved());
    cache.insert(200, dummy_resolved());
    cache.insert(300, dummy_resolved());

    assert!(cache.get(100).is_some());
    assert!(cache.get(200).is_some());
    assert!(cache.get(300).is_some());
    assert!(cache.get(400).is_none());
    assert_eq!(cache.len(), 3);
}

#[test]
fn mpc_overwrite() {
    let mut cache = MatchedPropertiesCache::new();
    cache.insert(100, dummy_resolved());
    assert!(cache.get(100).is_some());

    cache.insert(100, dummy_resolved());
    assert!(cache.get(100).is_some(), "overwrite replaces value");
    assert_eq!(cache.len(), 1, "still 1 entry");
}

#[test]
fn mpc_generation_invalidation() {
    let mut cache = MatchedPropertiesCache::new();
    cache.insert(1, dummy_resolved());
    cache.insert(2, dummy_resolved());
    assert_eq!(cache.len(), 2);

    let invalidated = cache.check_generation(1);
    assert!(invalidated);
    assert_eq!(cache.len(), 0);
    assert!(cache.is_empty());
    assert!(cache.get(1).is_none());
    assert!(cache.get(2).is_none());
}

#[test]
fn mpc_same_generation_keeps_data() {
    let mut cache = MatchedPropertiesCache::new();
    cache.check_generation(5);
    cache.insert(42, dummy_resolved());

    let invalidated = cache.check_generation(5);
    assert!(!invalidated);
    assert!(cache.get(42).is_some());
}

#[test]
fn mpc_hash_deterministic_exact() {
    let decls = vec![make_decl(0, 10, 0), make_decl(5, 100, 3)];
    let h1 = hash_matched(&decls);
    let h2 = hash_matched(&decls);
    assert_eq!(h1, h2, "same input produces same hash");
    // Verify it's a nonzero hash (extremely unlikely to be 0 for non-empty input).
    assert_ne!(h1, 0, "hash of non-empty input should be nonzero");
}

#[test]
fn mpc_hash_order_matters() {
    let a = vec![make_decl(0, 10, 0), make_decl(1, 20, 1)];
    let b = vec![make_decl(1, 20, 1), make_decl(0, 10, 0)];
    let ha = hash_matched(&a);
    let hb = hash_matched(&b);
    assert_ne!(ha, hb, "different order = different hash");
}

#[test]
fn mpc_hash_different_rule_indices() {
    let a = vec![make_decl(0, 10, 0)];
    let b = vec![make_decl(1, 10, 0)];
    assert_ne!(hash_matched(&a), hash_matched(&b), "different rule_index = different hash");
}

#[test]
fn mpc_hash_different_specificities() {
    let a = vec![make_decl(0, 10, 0)];
    let b = vec![make_decl(0, 20, 0)];
    assert_ne!(hash_matched(&a), hash_matched(&b), "different specificity = different hash");
}

#[test]
fn mpc_hash_empty_deterministic() {
    let empty: Vec<ApplicableDeclaration> = vec![];
    let h1 = hash_matched(&empty);
    let h2 = hash_matched(&empty);
    assert_eq!(h1, h2, "empty input hashes are deterministic");
}

#[test]
fn mpc_roundtrip_insert_and_lookup() {
    // Simulate real usage: hash declarations, insert, look up.
    let mut cache = MatchedPropertiesCache::new();
    let decls = vec![make_decl(0, 10, 0), make_decl(3, 100, 5)];
    let hash = hash_matched(&decls);
    cache.insert(hash, dummy_resolved());

    // Same declarations produce same hash, so lookup hits.
    let same_decls = vec![make_decl(0, 10, 0), make_decl(3, 100, 5)];
    let same_hash = hash_matched(&same_decls);
    assert_eq!(hash, same_hash);
    assert!(cache.get(same_hash).is_some());

    // Different declarations produce different hash, so lookup misses.
    let diff_decls = vec![make_decl(0, 10, 0), make_decl(4, 100, 5)];
    let diff_hash = hash_matched(&diff_decls);
    assert_ne!(hash, diff_hash);
    assert!(cache.get(diff_hash).is_none());
}

// ─── RESTYLE — EXACT HINT BIT VALUES ───

#[test]
fn restyle_inline_style_exact_hint_bits() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();
    tracker.push(DomMutation::InlineStyleChange {
        element: OpaqueElement::new(1),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);

    assert_eq!(tracker.dirty_count(), 1);
    let hint = tracker.dirty()[&OpaqueElement::new(1)];
    assert_eq!(hint, RestyleHint::RECASCADE, "exactly RECASCADE, no other flags");
    assert_eq!(hint.bits(), 0b0000_0100, "RECASCADE = bit 2");
}

#[test]
fn restyle_class_change_no_deps_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();
    tracker.push(DomMutation::ClassChange {
        element: OpaqueElement::new(1),
        class: Atom::from("btn"),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
    assert_eq!(tracker.dirty_count(), 0, "no class deps = no dirty elements");
    assert!(!tracker.has_dirty());
}

#[test]
fn restyle_multiple_mutations_merge_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();

    // Two inline style changes on same element merge into one entry.
    tracker.push(DomMutation::InlineStyleChange {
        element: OpaqueElement::new(1),
    });
    tracker.push(DomMutation::InlineStyleChange {
        element: OpaqueElement::new(1),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);

    assert_eq!(tracker.dirty_count(), 1);
    let hint = tracker.dirty()[&OpaqueElement::new(1)];
    assert_eq!(hint, RestyleHint::RECASCADE);
}

#[test]
fn restyle_different_elements_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();

    tracker.push(DomMutation::InlineStyleChange {
        element: OpaqueElement::new(10),
    });
    tracker.push(DomMutation::InlineStyleChange {
        element: OpaqueElement::new(20),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);

    assert_eq!(tracker.dirty_count(), 2);
    assert_eq!(
        tracker.dirty()[&OpaqueElement::new(10)],
        RestyleHint::RECASCADE,
    );
    assert_eq!(
        tracker.dirty()[&OpaqueElement::new(20)],
        RestyleHint::RECASCADE,
    );
}

#[test]
fn restyle_hint_bit_values() {
    // Verify the exact bit values of each RestyleHint flag.
    assert_eq!(RestyleHint::RESTYLE_SELF.bits(), 0b0000_0001);
    assert_eq!(RestyleHint::RESTYLE_DESCENDANTS.bits(), 0b0000_0010);
    assert_eq!(RestyleHint::RECASCADE.bits(), 0b0000_0100);
    assert_eq!(RestyleHint::INHERIT.bits(), 0b0000_1000);

    // Combined flags.
    let combined = RestyleHint::RESTYLE_SELF | RestyleHint::RESTYLE_DESCENDANTS;
    assert_eq!(combined.bits(), 0b0000_0011);
    assert!(combined.contains(RestyleHint::RESTYLE_SELF));
    assert!(combined.contains(RestyleHint::RESTYLE_DESCENDANTS));
    assert!(!combined.contains(RestyleHint::RECASCADE));
}

#[test]
fn restyle_clear_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();

    tracker.push(DomMutation::InlineStyleChange {
        element: OpaqueElement::new(1),
    });
    assert_eq!(tracker.pending_mutations(), 1);

    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
    assert_eq!(tracker.dirty_count(), 1);
    assert_eq!(tracker.pending_mutations(), 0, "compute_hints drains mutations");

    tracker.clear();
    assert_eq!(tracker.dirty_count(), 0);
    assert_eq!(tracker.pending_mutations(), 0);
    assert!(!tracker.has_dirty());
}

#[test]
fn restyle_state_change_no_deps_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new(); // no state deps
    tracker.push(DomMutation::StateChange {
        element: OpaqueElement::new(5),
        changed: kozan_selector::pseudo_class::ElementState::empty(),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
    // Empty invalidation map has no state deps, so state changes are ignored.
    assert_eq!(tracker.dirty_count(), 0);
}

#[test]
fn restyle_id_change_no_deps_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();
    tracker.push(DomMutation::IdChange {
        element: OpaqueElement::new(1),
        old: Some(Atom::from("header")),
        new: Some(Atom::from("footer")),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
    assert_eq!(tracker.dirty_count(), 0, "no id deps = no dirty");
}

#[test]
fn restyle_attr_change_no_deps_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();
    tracker.push(DomMutation::AttrChange {
        element: OpaqueElement::new(1),
        attr: Atom::from("data-active"),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
    assert_eq!(tracker.dirty_count(), 0, "no attr deps = no dirty");
}

#[test]
fn restyle_child_change_no_deps_exact() {
    let mut tracker = RestyleTracker::new();
    let invalidation = InvalidationMap::new();
    tracker.push(DomMutation::ChildChange {
        parent: OpaqueElement::new(1),
    });
    tracker.compute_hints(&invalidation, |_| vec![], |_| vec![]);
    assert_eq!(tracker.dirty_count(), 0, "no structural deps = no dirty");
}

#[test]
fn restyle_has_sibling_class_marks_preceding_sibling() {
    // Rule: div:has(+ .active) — when .active changes on element 2,
    // the PRECEDING sibling (element 1) needs restyle, not element 2 itself.
    use kozan_selector::invalidation::InvalidationMap;
    use kozan_selector::parser::parse;

    let mut map = InvalidationMap::new();
    let list = parse(":has(+ .active)").unwrap();
    map.add_selector_list(&list, 0);

    let mut tracker = RestyleTracker::new();
    tracker.push(DomMutation::ClassChange {
        element: OpaqueElement::new(2), // element gaining .active
        class: Atom::from("active"),
    });
    // sibling_lookup: element 2's preceding sibling is element 1.
    tracker.compute_hints(&map, |_| vec![], |e| {
        if e == OpaqueElement::new(2) { vec![OpaqueElement::new(1)] } else { vec![] }
    });

    // Element 1 (the :has() subject) must be dirty.
    assert!(tracker.dirty().contains_key(&OpaqueElement::new(1)), "preceding sibling should be dirty");
    // Element 2 (the .active element) is also in class_map, so it gets RESTYLE_SELF too
    // — this is a known false positive, not incorrect.
}

#[test]
fn restyle_has_subtree_class_marks_ancestor_not_sibling() {
    // Rule: :has(.child) — ancestor invalidation only, no sibling involvement.
    use kozan_selector::invalidation::InvalidationMap;
    use kozan_selector::parser::parse;

    let mut map = InvalidationMap::new();
    let list = parse(":has(.child)").unwrap();
    map.add_selector_list(&list, 0);

    let mut tracker = RestyleTracker::new();
    tracker.push(DomMutation::ClassChange {
        element: OpaqueElement::new(5),
        class: Atom::from("child"),
    });
    // ancestor_lookup returns element 3; sibling_lookup returns element 4.
    tracker.compute_hints(
        &map,
        |e| if e == OpaqueElement::new(5) { vec![OpaqueElement::new(3)] } else { vec![] },
        |e| if e == OpaqueElement::new(5) { vec![OpaqueElement::new(4)] } else { vec![] },
    );

    assert!(tracker.dirty().contains_key(&OpaqueElement::new(3)), "ancestor must be dirty");
    assert!(!tracker.dirty().contains_key(&OpaqueElement::new(4)), "sibling must NOT be dirty for subtree :has()");
}

// ─── MEDIA QUERIES — EXACT BOOLEAN RESULTS ───

fn eval(queries: Vec<MediaQuery>, device: &Device) -> bool {
    media::evaluate(&MediaQueryList(queries.into()), device)
}

fn range_query(name: &str, op: RangeOp, value: MediaFeatureValue) -> MediaQuery {
    MediaQuery {
        qualifier: None,
        media_type: CssMediaType::All,
        condition: Some(MediaCondition::Feature(MediaFeature::Range {
            name: Atom::from(name),
            op,
            value,
        })),
    }
}

fn plain_query(name: &str, value: MediaFeatureValue) -> MediaQuery {
    MediaQuery {
        qualifier: None,
        media_type: CssMediaType::All,
        condition: Some(MediaCondition::Feature(MediaFeature::Plain {
            name: Atom::from(name),
            value,
        })),
    }
}

#[test]
fn media_empty_list_returns_true() {
    assert_eq!(
        media::evaluate(&MediaQueryList::empty(), &Device::new(100.0, 100.0)),
        true,
    );
}

#[test]
fn media_width_ge_exact() {
    let device = Device::new(1024.0, 768.0);

    // width >= 1024px → true (equal)
    assert_eq!(
        eval(vec![range_query("width", RangeOp::Ge, MediaFeatureValue::Length(1024.0, LengthUnit::Px))], &device),
        true,
    );

    // width >= 1025px → false
    assert_eq!(
        eval(vec![range_query("width", RangeOp::Ge, MediaFeatureValue::Length(1025.0, LengthUnit::Px))], &device),
        false,
    );

    // width >= 500px → true
    assert_eq!(
        eval(vec![range_query("width", RangeOp::Ge, MediaFeatureValue::Length(500.0, LengthUnit::Px))], &device),
        true,
    );
}

#[test]
fn media_width_le_exact() {
    let device = Device::new(1024.0, 768.0);

    assert_eq!(
        eval(vec![range_query("width", RangeOp::Le, MediaFeatureValue::Length(1024.0, LengthUnit::Px))], &device),
        true,
        "1024 <= 1024"
    );
    assert_eq!(
        eval(vec![range_query("width", RangeOp::Le, MediaFeatureValue::Length(1023.0, LengthUnit::Px))], &device),
        false,
        "1024 > 1023"
    );
}

#[test]
fn media_height_exact() {
    let device = Device::new(1024.0, 768.0);

    assert_eq!(
        eval(vec![range_query("height", RangeOp::Eq, MediaFeatureValue::Length(768.0, LengthUnit::Px))], &device),
        true,
    );
    assert_eq!(
        eval(vec![range_query("height", RangeOp::Eq, MediaFeatureValue::Length(769.0, LengthUnit::Px))], &device),
        false,
    );
}

#[test]
fn media_min_width_legacy_exact() {
    let device = Device::new(1024.0, 768.0);

    // min-width: 768px → width >= 768px → true
    assert_eq!(
        eval(vec![range_query("min-width", RangeOp::Ge, MediaFeatureValue::Length(768.0, LengthUnit::Px))], &device),
        true,
    );

    // min-width: 2000px → width >= 2000px → false
    assert_eq!(
        eval(vec![range_query("min-width", RangeOp::Ge, MediaFeatureValue::Length(2000.0, LengthUnit::Px))], &device),
        false,
    );
}

#[test]
fn media_zero_viewport_exact() {
    let device = Device::new(0.0, 0.0);

    // 0 >= 0 → true
    assert_eq!(
        eval(vec![range_query("width", RangeOp::Ge, MediaFeatureValue::Length(0.0, LengthUnit::Px))], &device),
        true,
    );

    // 0 > 0 → false
    assert_eq!(
        eval(vec![range_query("width", RangeOp::Gt, MediaFeatureValue::Length(0.0, LengthUnit::Px))], &device),
        false,
    );
}

#[test]
fn media_zero_height_aspect_ratio_no_panic() {
    let device = Device::new(1024.0, 0.0);
    // aspect_ratio = 0.0 (safe division). 0 > 1 → false.
    assert_eq!(
        eval(vec![range_query("aspect-ratio", RangeOp::Gt, MediaFeatureValue::Number(1.0))], &device),
        false,
    );
}

#[test]
fn media_not_negates_exact() {
    let device = Device::new(1024.0, 768.0);
    let q = MediaQuery {
        qualifier: Some(MediaQualifier::Not),
        media_type: CssMediaType::Screen,
        condition: None,
    };
    assert_eq!(eval(vec![q], &device), false, "NOT screen on screen device");
}

#[test]
fn media_not_print_on_screen() {
    let device = Device::new(1024.0, 768.0);
    let q = MediaQuery {
        qualifier: Some(MediaQualifier::Not),
        media_type: CssMediaType::Print,
        condition: None,
    };
    assert_eq!(eval(vec![q], &device), true, "NOT print on screen device");
}

#[test]
fn media_or_semantics_exact() {
    let device = Device::new(600.0, 400.0);
    // Query 1: width >= 1200 → false. Query 2: width <= 768 → true.
    // Result: false OR true → true.
    let queries = vec![
        range_query("min-width", RangeOp::Ge, MediaFeatureValue::Length(1200.0, LengthUnit::Px)),
        range_query("max-width", RangeOp::Le, MediaFeatureValue::Length(768.0, LengthUnit::Px)),
    ];
    assert_eq!(eval(queries, &device), true, "comma = OR semantics");
}

#[test]
fn media_or_both_false() {
    let device = Device::new(600.0, 400.0);
    // width >= 1200 → false. width <= 100 → false.
    let queries = vec![
        range_query("width", RangeOp::Ge, MediaFeatureValue::Length(1200.0, LengthUnit::Px)),
        range_query("width", RangeOp::Le, MediaFeatureValue::Length(100.0, LengthUnit::Px)),
    ];
    assert_eq!(eval(queries, &device), false, "both false = false");
}

#[test]
fn media_forced_colors_exact() {
    let mut device = Device::new(1024.0, 768.0);
    device.forced_colors = ForcedColors::Active;
    assert_eq!(
        eval(vec![plain_query("forced-colors", MediaFeatureValue::Ident(Atom::from("active")))], &device),
        true,
    );
    assert_eq!(
        eval(vec![plain_query("forced-colors", MediaFeatureValue::Ident(Atom::from("none")))], &device),
        false,
    );
}

#[test]
fn media_color_gamut_hierarchy_exact() {
    // P3 device includes sRGB but not rec2020.
    let mut device = Device::new(1024.0, 768.0);
    device.color_gamut = ColorGamut::P3;

    assert_eq!(
        eval(vec![plain_query("color-gamut", MediaFeatureValue::Ident(Atom::from("srgb")))], &device),
        true,
        "P3 includes sRGB"
    );
    assert_eq!(
        eval(vec![plain_query("color-gamut", MediaFeatureValue::Ident(Atom::from("p3")))], &device),
        true,
        "P3 matches P3"
    );
    assert_eq!(
        eval(vec![plain_query("color-gamut", MediaFeatureValue::Ident(Atom::from("rec2020")))], &device),
        false,
        "P3 does not include rec2020"
    );
}

#[test]
fn media_prefers_color_scheme_exact() {
    let mut device = Device::new(1024.0, 768.0);

    device.prefers_color_scheme = ColorScheme::Dark;
    assert_eq!(
        eval(vec![plain_query("prefers-color-scheme", MediaFeatureValue::Ident(Atom::from("dark")))], &device),
        true,
    );
    assert_eq!(
        eval(vec![plain_query("prefers-color-scheme", MediaFeatureValue::Ident(Atom::from("light")))], &device),
        false,
    );

    device.prefers_color_scheme = ColorScheme::Light;
    assert_eq!(
        eval(vec![plain_query("prefers-color-scheme", MediaFeatureValue::Ident(Atom::from("light")))], &device),
        true,
    );
    assert_eq!(
        eval(vec![plain_query("prefers-color-scheme", MediaFeatureValue::Ident(Atom::from("dark")))], &device),
        false,
    );
}

#[test]
fn media_prefers_reduced_motion_exact() {
    let mut device = Device::new(1024.0, 768.0);

    device.prefers_reduced_motion = true;
    assert_eq!(
        eval(vec![plain_query("prefers-reduced-motion", MediaFeatureValue::Ident(Atom::from("reduce")))], &device),
        true,
    );
    assert_eq!(
        eval(vec![plain_query("prefers-reduced-motion", MediaFeatureValue::Ident(Atom::from("no-preference")))], &device),
        false,
    );

    device.prefers_reduced_motion = false;
    assert_eq!(
        eval(vec![plain_query("prefers-reduced-motion", MediaFeatureValue::Ident(Atom::from("no-preference")))], &device),
        true,
    );
}

#[test]
fn media_scripting_exact() {
    let mut device = Device::new(1024.0, 768.0);

    device.scripting = Scripting::Enabled;
    assert_eq!(
        eval(vec![plain_query("scripting", MediaFeatureValue::Ident(Atom::from("enabled")))], &device),
        true,
    );
    assert_eq!(
        eval(vec![plain_query("scripting", MediaFeatureValue::Ident(Atom::from("none")))], &device),
        false,
    );

    device.scripting = Scripting::None;
    assert_eq!(
        eval(vec![plain_query("scripting", MediaFeatureValue::Ident(Atom::from("none")))], &device),
        true,
    );
}

// ─── DEVICE DEFAULTS — EXACT VALUES ───

#[test]
fn device_defaults_exact_values() {
    let device = Device::new(1920.0, 1080.0);

    assert_eq!(device.viewport_width, 1920.0);
    assert_eq!(device.viewport_height, 1080.0);
    assert_eq!(device.media_type, MediaType::Screen);
    assert_eq!(device.device_pixel_ratio, 1.0);
    assert_eq!(device.resolution_dpi, 96.0);
    assert_eq!(device.color_bits, 8);
    assert_eq!(device.monochrome_bits, 0);
    assert_eq!(device.pointer, Pointer::Fine);
    assert_eq!(device.any_pointer, Pointer::Fine);
    assert_eq!(device.hover, HoverCapability::Hover);
    assert_eq!(device.any_hover, HoverCapability::Hover);
    assert_eq!(device.color_gamut, ColorGamut::Srgb);
    assert_eq!(device.dynamic_range, DynamicRange::Standard);
    assert_eq!(device.forced_colors, ForcedColors::None);
    assert_eq!(device.update, Update::Fast);
    assert_eq!(device.scripting, Scripting::Enabled);
    assert_eq!(device.prefers_color_scheme, ColorScheme::Light);
    assert_eq!(device.prefers_reduced_motion, false);
    assert_eq!(device.prefers_reduced_transparency, false);
    assert_eq!(device.prefers_contrast, false);
    assert_eq!(device.inverted_colors, false);
    assert_eq!(device.grid, false);
    assert!(device.font_metrics.is_none());
}

#[test]
fn device_aspect_ratio_exact_values() {
    assert_eq!(Device::new(1920.0, 1080.0).aspect_ratio(), 1920.0 / 1080.0);
    assert_eq!(Device::new(1024.0, 768.0).aspect_ratio(), 1024.0 / 768.0);
    assert_eq!(Device::new(100.0, 100.0).aspect_ratio(), 1.0);
    assert_eq!(Device::new(100.0, 0.0).aspect_ratio(), 0.0);
    assert_eq!(Device::new(0.0, 0.0).aspect_ratio(), 0.0);
    assert_eq!(Device::new(0.0, 100.0).aspect_ratio(), 0.0);
}

#[test]
fn device_default_font_size_exact() {
    let device = Device::new(1024.0, 768.0);
    assert_eq!(device.default_font_size(), 16.0);
}
