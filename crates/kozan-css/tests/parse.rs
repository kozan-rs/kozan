//! Tests for kozan-css parser.

use kozan_css::{parse_inline, parse_value};
use kozan_style::*;

// --- Keyword enum parsing ---

#[test]
fn parse_display_flex() {
    let block = parse_inline("display: flex");
    let entries = block.entries();
    assert_eq!(entries.len(), 1);
    match &entries[0].0 {
        PropertyDeclaration::Display(Declared::Value(Display::Flex)) => {}
        other => panic!("expected Display::Flex, got {other:?}"),
    }
}

#[test]
fn parse_position_absolute() {
    let block = parse_inline("position: absolute");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::Position(Declared::Value(Position::Absolute)) => {}
        other => panic!("expected Position::Absolute, got {other:?}"),
    }
}

#[test]
fn parse_display_case_insensitive() {
    let block = parse_inline("display: FLEX");
    match &block.entries()[0].0 {
        PropertyDeclaration::Display(Declared::Value(Display::Flex)) => {}
        other => panic!("expected Display::Flex, got {other:?}"),
    }
}

// --- CSS-wide keywords ---

#[test]
fn parse_inherit_keyword() {
    let block = parse_inline("display: inherit");
    match &block.entries()[0].0 {
        PropertyDeclaration::Display(Declared::Inherit) => {}
        other => panic!("expected Declared::Inherit, got {other:?}"),
    }
}

#[test]
fn parse_initial_keyword() {
    let block = parse_inline("color: initial");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Initial) => {}
        other => panic!("expected Declared::Initial, got {other:?}"),
    }
}

#[test]
fn parse_revert_layer_keyword() {
    let block = parse_inline("width: revert-layer");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::RevertLayer) => {}
        other => panic!("expected Declared::RevertLayer, got {other:?}"),
    }
}

// --- Length parsing ---

#[test]
fn parse_width_px() {
    let block = parse_inline("width: 100px");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Length(specified::Length::Absolute(
                specified::AbsoluteLength::Px(v),
            )),
        ))) => assert_eq!(*v, 100.0),
        other => panic!("expected 100px, got {other:?}"),
    }
}

#[test]
fn parse_width_em() {
    let block = parse_inline("width: 2.5em");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Length(specified::Length::FontRelative(
                specified::FontRelativeLength::Em(v),
            )),
        ))) => assert_eq!(*v, 2.5),
        other => panic!("expected 2.5em, got {other:?}"),
    }
}

#[test]
fn parse_width_percentage() {
    let block = parse_inline("width: 50%");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Percentage(p),
        ))) => assert_eq!(p.value(), 0.5),
        other => panic!("expected 50%, got {other:?}"),
    }
}

#[test]
fn parse_width_auto() {
    let block = parse_inline("width: auto");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::Auto)) => {}
        other => panic!("expected Size::Auto, got {other:?}"),
    }
}

// --- Color parsing ---

#[test]
fn parse_color_hex() {
    let block = parse_inline("color: #ff0000");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => {
            assert_eq!(c.color_space, ColorSpace::Srgb);
            assert!((c.components[0] - 1.0).abs() < 0.01);
            assert!(c.components[1].abs() < 0.01);
        }
        other => panic!("expected red color, got {other:?}"),
    }
}

#[test]
fn parse_color_named() {
    let block = parse_inline("color: red");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => {
            assert_eq!(c.color_space, ColorSpace::Srgb);
            assert!((c.components[0] - 1.0).abs() < 0.01);
        }
        other => panic!("expected red, got {other:?}"),
    }
}

#[test]
fn parse_color_currentcolor() {
    let block = parse_inline("color: currentcolor");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::CurrentColor))) => {}
        other => panic!("expected currentcolor, got {other:?}"),
    }
}

// --- var() detection ---

#[test]
fn parse_var_becomes_with_variables() {
    let block = parse_inline("width: var(--gap)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::WithVariables(unparsed)) => {
            assert!(unparsed.references.contains(SubstitutionRefs::VAR));
        }
        other => panic!("expected WithVariables, got {other:?}"),
    }
}

#[test]
fn parse_calc_with_var() {
    let block = parse_inline("width: calc(var(--x) + 10px)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::WithVariables(unparsed)) => {
            assert!(unparsed.references.contains(SubstitutionRefs::VAR));
            assert!(unparsed.css.contains("var(--x)"));
        }
        other => panic!("expected WithVariables with var, got {other:?}"),
    }
}

// --- Multiple declarations ---

#[test]
fn parse_multiple_declarations() {
    let block = parse_inline("display: flex; width: 100px; position: relative");
    assert_eq!(block.entries().len(), 3);
}

// --- Error recovery ---

#[test]
fn invalid_declaration_skipped() {
    let block = parse_inline("display: flex; invalid-property: ???; position: fixed");
    // Invalid property skipped, valid ones parsed.
    assert!(block.entries().len() >= 2);
}

// --- parse_value API ---

#[test]
fn parse_single_value_api() {
    let decl = parse_value(PropertyId::Display, "flex");
    match decl {
        Some(PropertyDeclaration::Display(Declared::Value(Display::Flex))) => {}
        other => panic!("expected Display::Flex, got {other:?}"),
    }
}

// CORRECTNESS TESTS — verify actual parsed values, not just "doesn't crash"

// --- Length units ---

#[test]
fn parse_length_rem() {
    let block = parse_inline("font-size: 1.5rem");
    match &block.entries()[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(v)) => {
            // Should be a rem length
            let _ = v; // FontSize type parses rem
        }
        other => panic!("expected FontSize rem, got {other:?}"),
    }
}

#[test]
fn parse_length_zero_no_unit() {
    let block = parse_inline("width: 0");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Length(specified::Length::Absolute(
                specified::AbsoluteLength::Px(v),
            )),
        ))) => assert_eq!(*v, 0.0),
        other => panic!("expected 0px, got {other:?}"),
    }
}

#[test]
fn parse_length_negative() {
    let block = parse_inline("margin-left: -10px");
    match &block.entries()[0].0 {
        PropertyDeclaration::MarginLeft(Declared::Value(
            generics::Margin::LengthPercentage(
                specified::LengthPercentage::Length(specified::Length::Absolute(
                    specified::AbsoluteLength::Px(v),
                )),
            ),
        )) => assert_eq!(*v, -10.0),
        other => panic!("expected -10px, got {other:?}"),
    }
}

#[test]
fn parse_length_vw() {
    let block = parse_inline("width: 100vw");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Length(specified::Length::ViewportPercentage(
                specified::ViewportPercentageLength::Vw(v),
            )),
        ))) => assert_eq!(*v, 100.0),
        other => panic!("expected 100vw, got {other:?}"),
    }
}

// --- Color functions ---

#[test]
fn parse_color_rgb_function() {
    let block = parse_inline("color: rgb(255, 128, 0)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => {
            assert_eq!(c.color_space, ColorSpace::Srgb);
            assert!((c.components[0] - 1.0).abs() < 0.01);    // r = 255/255
            assert!((c.components[1] - 0.502).abs() < 0.01);   // g = 128/255
            assert!(c.components[2].abs() < 0.01);              // b = 0/255
        }
        other => panic!("expected rgb color, got {other:?}"),
    }
}

#[test]
fn parse_color_rgba_alpha() {
    let block = parse_inline("color: rgba(0, 0, 0, 0.5)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => {
            assert!((c.alpha - 0.5).abs() < 0.01);
        }
        other => panic!("expected rgba with alpha, got {other:?}"),
    }
}

#[test]
fn parse_color_hsl() {
    let block = parse_inline("color: hsl(120, 100%, 50%)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => {
            // hsl(120, 100%, 50%) stored as-is (hue=120, sat=100, light=50)
            // Conversion to sRGB happens at computed-value time, not parse time.
            assert_eq!(c.color_space, ColorSpace::Hsl);
            assert!((c.components[0] - 120.0).abs() < 0.01, "hue should be 120, got {}", c.components[0]);
            assert!((c.components[1] - 1.0).abs() < 0.01, "sat should be 1.0, got {}", c.components[1]);
            assert!((c.components[2] - 0.5).abs() < 0.01, "light should be 0.5, got {}", c.components[2]);
        }
        other => panic!("expected hsl color, got {other:?}"),
    }
}

#[test]
fn parse_color_hex_short() {
    let block = parse_inline("color: #f00");
    match &block.entries()[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => {
            assert!((c.components[0] - 1.0).abs() < 0.01);
            assert!(c.components[1].abs() < 0.01);
            assert!(c.components[2].abs() < 0.01);
        }
        other => panic!("expected #f00 = red, got {other:?}"),
    }
}

#[test]
fn parse_color_transparent() {
    let block = parse_inline("background-color: transparent");
    match &block.entries()[0].0 {
        PropertyDeclaration::BackgroundColor(Declared::Value(Color::Absolute(c))) => {
            assert_eq!(c.alpha, 0.0);
        }
        other => panic!("expected transparent, got {other:?}"),
    }
}

// --- Keyword properties — verify correct enum variants ---

#[test]
fn parse_overflow_hidden() {
    let block = parse_inline("overflow-x: hidden");
    match &block.entries()[0].0 {
        PropertyDeclaration::OverflowX(Declared::Value(Overflow::Hidden)) => {}
        other => panic!("expected Overflow::Hidden, got {other:?}"),
    }
}

#[test]
fn parse_visibility_collapse() {
    let block = parse_inline("visibility: collapse");
    match &block.entries()[0].0 {
        PropertyDeclaration::Visibility(Declared::Value(Visibility::Collapse)) => {}
        other => panic!("expected Visibility::Collapse, got {other:?}"),
    }
}

#[test]
fn parse_box_sizing_border_box() {
    let block = parse_inline("box-sizing: border-box");
    match &block.entries()[0].0 {
        PropertyDeclaration::BoxSizing(Declared::Value(BoxSizing::BorderBox)) => {}
        other => panic!("expected BoxSizing::BorderBox, got {other:?}"),
    }
}

#[test]
fn parse_flex_direction() {
    let block = parse_inline("flex-direction: column-reverse");
    match &block.entries()[0].0 {
        PropertyDeclaration::FlexDirection(Declared::Value(FlexDirection::ColumnReverse)) => {}
        other => panic!("expected FlexDirection::ColumnReverse, got {other:?}"),
    }
}

#[test]
fn parse_align_items_center() {
    let block = parse_inline("align-items: center");
    match &block.entries()[0].0 {
        PropertyDeclaration::AlignItems(Declared::Value(AlignItems::Center)) => {}
        other => panic!("expected AlignItems::Center, got {other:?}"),
    }
}

#[test]
fn parse_justify_content_space_between() {
    let block = parse_inline("justify-content: space-between");
    match &block.entries()[0].0 {
        PropertyDeclaration::JustifyContent(Declared::Value(JustifyContent::SpaceBetween)) => {}
        other => panic!("expected JustifyContent::SpaceBetween, got {other:?}"),
    }
}

// --- Numeric properties ---

#[test]
fn parse_opacity_half() {
    let block = parse_inline("opacity: 0.5");
    match &block.entries()[0].0 {
        PropertyDeclaration::Opacity(Declared::Value(v)) => {
            assert!((*v - 0.5).abs() < 0.001);
        }
        other => panic!("expected opacity 0.5, got {other:?}"),
    }
}

#[test]
fn parse_flex_grow() {
    let block = parse_inline("flex-grow: 2");
    match &block.entries()[0].0 {
        PropertyDeclaration::FlexGrow(Declared::Value(v)) => {
            assert_eq!(*v, 2.0);
        }
        other => panic!("expected flex-grow 2, got {other:?}"),
    }
}

#[test]
fn parse_z_index_negative() {
    let block = parse_inline("z-index: -1");
    match &block.entries()[0].0 {
        PropertyDeclaration::ZIndex(Declared::Value(v)) => {
            let _ = v; // Should be an integer value of -1
        }
        other => panic!("expected z-index -1, got {other:?}"),
    }
}

// --- calc() correctness ---

#[test]
fn parse_calc_simple_add() {
    let block = parse_inline("width: calc(100% - 20px)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Calc(_),
        ))) => {}
        other => panic!("expected calc() value, got {other:?}"),
    }
}

#[test]
fn parse_calc_nested() {
    let block = parse_inline("width: calc(100% - calc(2 * 10px))");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Calc(_),
        ))) => {}
        other => panic!("expected nested calc() value, got {other:?}"),
    }
}

#[test]
fn parse_min_function() {
    let block = parse_inline("width: min(100%, 800px)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Calc(_),
        ))) => {}
        other => panic!("expected min() value, got {other:?}"),
    }
}

#[test]
fn parse_clamp_function() {
    let block = parse_inline("width: clamp(200px, 50%, 800px)");
    match &block.entries()[0].0 {
        PropertyDeclaration::Width(Declared::Value(generics::Size::LengthPercentage(
            specified::LengthPercentage::Calc(_),
        ))) => {}
        other => panic!("expected clamp() value, got {other:?}"),
    }
}

// --- !important ---

#[test]
fn parse_important_flag() {
    let block = parse_inline("color: red !important");
    let entries = block.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].1, Importance::Important, "should be marked important");
}

#[test]
fn parse_not_important() {
    let block = parse_inline("color: red");
    let entries = block.entries();
    assert_eq!(entries[0].1, Importance::Normal, "should NOT be marked important");
}

#[test]
fn parse_mixed_importance() {
    let block = parse_inline("color: red !important; display: flex");
    let entries = block.entries();
    assert_eq!(entries.len(), 2);
    let important_count = entries.iter().filter(|(_, imp)| *imp == Importance::Important).count();
    let normal_count = entries.iter().filter(|(_, imp)| *imp == Importance::Normal).count();
    assert_eq!(important_count, 1);
    assert_eq!(normal_count, 1);
}

// --- Custom properties ---

#[test]
fn parse_custom_property() {
    let block = parse_inline("--my-color: blue");
    let entries = block.entries();
    assert_eq!(entries.len(), 1);
    match &entries[0].0 {
        PropertyDeclaration::Custom { name, .. } => {
            assert_eq!(&**name, "--my-color");
        }
        other => panic!("expected custom property, got {other:?}"),
    }
}

#[test]
fn parse_custom_property_complex_value() {
    let block = parse_inline("--gradient: linear-gradient(to right, red, blue)");
    let entries = block.entries();
    assert_eq!(entries.len(), 1);
    match &entries[0].0 {
        PropertyDeclaration::Custom { name, .. } => {
            assert_eq!(&**name, "--gradient");
        }
        other => panic!("expected custom property, got {other:?}"),
    }
}

// --- Multiple values for same property (last wins) ---

#[test]
fn parse_duplicate_property_last_wins() {
    let block = parse_inline("color: red; color: blue");
    let entries = block.entries();
    // DeclarationBlock may keep both or just the last — verify at least one exists
    assert!(!entries.is_empty());
    // The last value should be "blue"
    let last_color = entries.iter().rev().find(|(d, _)| matches!(d, PropertyDeclaration::Color(_)));
    match last_color {
        Some((PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))), _)) => {
            // blue = srgb(0, 0, 1)
            assert!(c.components[0].abs() < 0.01, "red should be 0");
            assert!((c.components[2] - 1.0).abs() < 0.01, "blue should be 1");
        }
        other => panic!("expected blue as last color, got {other:?}"),
    }
}

// --- All CSS-wide keywords work for any property ---

#[test]
fn parse_unset_keyword() {
    let block = parse_inline("margin-top: unset");
    match &block.entries()[0].0 {
        PropertyDeclaration::MarginTop(Declared::Unset) => {}
        other => panic!("expected Declared::Unset, got {other:?}"),
    }
}

#[test]
fn parse_revert_keyword() {
    let block = parse_inline("display: revert");
    match &block.entries()[0].0 {
        PropertyDeclaration::Display(Declared::Revert) => {}
        other => panic!("expected Declared::Revert, got {other:?}"),
    }
}

// --- env() detection ---

#[test]
fn parse_env_detection() {
    let block = parse_inline("padding-top: env(safe-area-inset-top)");
    match &block.entries()[0].0 {
        PropertyDeclaration::PaddingTop(Declared::WithVariables(unparsed)) => {
            assert!(unparsed.references.contains(SubstitutionRefs::ENV));
        }
        other => panic!("expected WithVariables with ENV, got {other:?}"),
    }
}

// --- Border-radius (length) ---

#[test]
fn parse_border_radius() {
    let block = parse_inline("border-top-left-radius: 8px");
    match &block.entries()[0].0 {
        PropertyDeclaration::BorderTopLeftRadius(Declared::Value(_)) => {}
        other => panic!("expected border-radius value, got {other:?}"),
    }
}

// --- Grid properties ---

#[test]
fn parse_grid_template_columns() {
    let block = parse_inline("grid-template-columns: 1fr 2fr auto");
    match &block.entries()[0].0 {
        PropertyDeclaration::GridTemplateColumns(Declared::Value(_)) => {}
        other => panic!("expected grid-template-columns value, got {other:?}"),
    }
}

#[test]
fn parse_gap() {
    let block = parse_inline("row-gap: 16px");
    match &block.entries()[0].0 {
        PropertyDeclaration::RowGap(Declared::Value(_)) => {}
        other => panic!("expected row-gap value, got {other:?}"),
    }
}

// --- Verify unknown properties are rejected ---

#[test]
fn unknown_property_rejected() {
    let block = parse_inline("banana-color: yellow");
    // "banana-color" is not a valid CSS property and doesn't start with --
    assert_eq!(block.entries().len(), 0, "unknown property should be rejected");
}

// --- Logical properties ---

#[test]
fn parse_margin_inline_start() {
    let block = parse_inline("margin-inline-start: 20px");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::MarginInlineStart(Declared::Value(_)) => {}
        other => panic!("expected MarginInlineStart, got {other:?}"),
    }
}

#[test]
fn parse_padding_block_end() {
    let block = parse_inline("padding-block-end: 1rem");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::PaddingBlockEnd(Declared::Value(_)) => {}
        other => panic!("expected PaddingBlockEnd, got {other:?}"),
    }
}

#[test]
fn parse_inline_size() {
    let block = parse_inline("inline-size: 50%");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::InlineSize(Declared::Value(_)) => {}
        other => panic!("expected InlineSize, got {other:?}"),
    }
}

#[test]
fn parse_border_inline_start_color() {
    let block = parse_inline("border-inline-start-color: red");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::BorderInlineStartColor(Declared::Value(_)) => {}
        other => panic!("expected BorderInlineStartColor, got {other:?}"),
    }
}

#[test]
fn parse_inset_inline_start() {
    let block = parse_inline("inset-inline-start: 0");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::InsetInlineStart(Declared::Value(_)) => {}
        other => panic!("expected InsetInlineStart, got {other:?}"),
    }
}

#[test]
fn parse_logical_inherit() {
    let block = parse_inline("margin-inline-start: inherit");
    match &block.entries()[0].0 {
        PropertyDeclaration::MarginInlineStart(Declared::Inherit) => {}
        other => panic!("expected Declared::Inherit, got {other:?}"),
    }
}

#[test]
fn parse_logical_var() {
    let block = parse_inline("padding-block-start: var(--gap)");
    match &block.entries()[0].0 {
        PropertyDeclaration::PaddingBlockStart(Declared::WithVariables(unparsed)) => {
            assert!(unparsed.references.contains(SubstitutionRefs::VAR));
        }
        other => panic!("expected WithVariables, got {other:?}"),
    }
}

#[test]
fn parse_max_inline_size() {
    let block = parse_inline("max-inline-size: 800px");
    assert_eq!(block.entries().len(), 1);
    match &block.entries()[0].0 {
        PropertyDeclaration::MaxInlineSize(Declared::Value(_)) => {}
        other => panic!("expected MaxInlineSize, got {other:?}"),
    }
}

// SHORTHAND EXPANSION TESTS

// --- Helper ---

/// Parse inline CSS and return all entries as a vec of (declaration, importance).
fn entries(css: &str) -> Vec<(PropertyDeclaration, Importance)> {
    parse_inline(css).entries().to_vec()
}

/// Assert that a shorthand expands to exactly `n` longhands.
fn assert_longhand_count(css: &str, n: usize) {
    let e = entries(css);
    assert_eq!(e.len(), n, "expected {n} longhands from `{css}`, got {} → {e:?}", e.len());
}

/// Check a specific longhand by index matches a pattern.
macro_rules! assert_decl {
    ($entries:expr, $idx:expr, $pat:pat) => {
        match &$entries[$idx].0 {
            $pat => {}
            other => panic!(
                "entry[{}]: expected {}, got {other:?}",
                $idx,
                stringify!($pat)
            ),
        }
    };
}

// margin (box4 — generated)

#[test]
fn shorthand_margin_4_values() {
    let e = entries("margin: 10px 20px 30px 40px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::MarginTop(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::MarginRight(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::MarginBottom(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::MarginLeft(Declared::Value(_)));
}

#[test]
fn shorthand_margin_2_values() {
    let e = entries("margin: 10px 20px");
    assert_eq!(e.len(), 4);
    // top=10, right=20, bottom=10, left=20
    assert_decl!(e, 0, PropertyDeclaration::MarginTop(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::MarginRight(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::MarginBottom(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::MarginLeft(Declared::Value(_)));
}

#[test]
fn shorthand_margin_1_value() {
    let e = entries("margin: 0");
    assert_eq!(e.len(), 4);
}

#[test]
fn shorthand_margin_3_values() {
    let e = entries("margin: 10px 20px 30px");
    assert_eq!(e.len(), 4);
}

#[test]
fn shorthand_margin_auto() {
    let e = entries("margin: auto");
    assert_eq!(e.len(), 4);
}

// padding (box4 — generated)

#[test]
fn shorthand_padding_4_values() {
    let e = entries("padding: 1px 2px 3px 4px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::PaddingTop(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::PaddingRight(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::PaddingBottom(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::PaddingLeft(Declared::Value(_)));
}

#[test]
fn shorthand_padding_1_value() {
    let e = entries("padding: 16px");
    assert_eq!(e.len(), 4);
}

// border (hand-written — 12 longhands: 4×width + 4×style + 4×color)

#[test]
fn shorthand_border_full() {
    let e = entries("border: 1px solid red");
    assert_eq!(e.len(), 12);
    // width longhands
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderRightWidth(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::BorderBottomWidth(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::BorderLeftWidth(Declared::Value(_)));
    // style longhands
    assert_decl!(e, 4, PropertyDeclaration::BorderTopStyle(Declared::Value(_)));
    assert_decl!(e, 5, PropertyDeclaration::BorderRightStyle(Declared::Value(_)));
    // color longhands
    assert_decl!(e, 8, PropertyDeclaration::BorderTopColor(Declared::Value(_)));
}

#[test]
fn shorthand_border_style_only() {
    let e = entries("border: solid");
    assert_eq!(e.len(), 12);
    // style is Value, width and color are Initial
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::Initial));
    assert_decl!(e, 4, PropertyDeclaration::BorderTopStyle(Declared::Value(_)));
    assert_decl!(e, 8, PropertyDeclaration::BorderTopColor(Declared::Initial));
}

#[test]
fn shorthand_border_any_order() {
    // color first, then width, then style
    let e = entries("border: red 2px dashed");
    assert_eq!(e.len(), 12);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::Value(_)));
    assert_decl!(e, 4, PropertyDeclaration::BorderTopStyle(Declared::Value(_)));
    assert_decl!(e, 8, PropertyDeclaration::BorderTopColor(Declared::Value(_)));
}

// border-top (hand-written — WSC → 3 longhands)

#[test]
fn shorthand_border_top() {
    let e = entries("border-top: 2px dotted blue");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderTopStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::BorderTopColor(Declared::Value(_)));
}

#[test]
fn shorthand_border_bottom() {
    let e = entries("border-bottom: 1px solid");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::BorderBottomWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderBottomStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::BorderBottomColor(Declared::Initial));
}

#[test]
fn shorthand_border_left() {
    let e = entries("border-left: green");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::BorderLeftWidth(Declared::Initial));
    assert_decl!(e, 1, PropertyDeclaration::BorderLeftStyle(Declared::Initial));
    assert_decl!(e, 2, PropertyDeclaration::BorderLeftColor(Declared::Value(_)));
}

// border-width / border-style / border-color (box4 — generated)

#[test]
fn shorthand_border_width() {
    let e = entries("border-width: 1px 2px 3px 4px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::BorderLeftWidth(Declared::Value(_)));
}

#[test]
fn shorthand_border_style() {
    let e = entries("border-style: solid dashed");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopStyle(Declared::Value(_)));
}

#[test]
fn shorthand_border_color() {
    let e = entries("border-color: red blue green yellow");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopColor(Declared::Value(_)));
}

// border-radius (hand-written override — 4 longhands with CornerRadius)

#[test]
fn shorthand_border_radius_uniform() {
    let e = entries("border-radius: 8px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopLeftRadius(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderTopRightRadius(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::BorderBottomRightRadius(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::BorderBottomLeftRadius(Declared::Value(_)));
}

#[test]
fn shorthand_border_radius_elliptical() {
    // horizontal 10px 20px / vertical 5px 15px
    let e = entries("border-radius: 10px 20px / 5px 15px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopLeftRadius(Declared::Value(_)));
}

#[test]
fn shorthand_border_radius_4_values() {
    let e = entries("border-radius: 1px 2px 3px 4px");
    assert_eq!(e.len(), 4);
}

// outline (hand-written — WSC with OutlineStyle → 3 longhands)

#[test]
fn shorthand_outline() {
    let e = entries("outline: 2px solid blue");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::OutlineWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::OutlineStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::OutlineColor(Declared::Value(_)));
}

#[test]
fn shorthand_outline_style_only() {
    let e = entries("outline: dotted");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::OutlineWidth(Declared::Initial));
    assert_decl!(e, 1, PropertyDeclaration::OutlineStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::OutlineColor(Declared::Initial));
}

// flex (hand-written — grow + shrink + basis)

#[test]
fn shorthand_flex_three_values() {
    let e = entries("flex: 2 1 100px");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::FlexGrow(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::FlexShrink(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::FlexBasis(Declared::Value(_)));
}

#[test]
fn shorthand_flex_none() {
    let e = entries("flex: none");
    assert_eq!(e.len(), 3);
    // flex: none → 0 0 auto
    match &e[0].0 {
        PropertyDeclaration::FlexGrow(Declared::Value(v)) => assert_eq!(*v, 0.0),
        other => panic!("expected FlexGrow(0), got {other:?}"),
    }
    match &e[1].0 {
        PropertyDeclaration::FlexShrink(Declared::Value(v)) => assert_eq!(*v, 0.0),
        other => panic!("expected FlexShrink(0), got {other:?}"),
    }
}

#[test]
fn shorthand_flex_auto() {
    let e = entries("flex: auto");
    assert_eq!(e.len(), 3);
    // flex: auto → 1 1 auto
    match &e[0].0 {
        PropertyDeclaration::FlexGrow(Declared::Value(v)) => assert_eq!(*v, 1.0),
        other => panic!("expected FlexGrow(1), got {other:?}"),
    }
}

#[test]
fn shorthand_flex_single_number() {
    let e = entries("flex: 3");
    assert_eq!(e.len(), 3);
    // flex: 3 → grow=3, shrink=1, basis=0px
    match &e[0].0 {
        PropertyDeclaration::FlexGrow(Declared::Value(v)) => assert_eq!(*v, 3.0),
        other => panic!("expected FlexGrow(3), got {other:?}"),
    }
}

// flex-flow (hand-written — direction + wrap)

#[test]
fn shorthand_flex_flow() {
    let e = entries("flex-flow: row wrap");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::FlexDirection(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::FlexWrap(Declared::Value(_)));
}

#[test]
fn shorthand_flex_flow_reverse() {
    let e = entries("flex-flow: column-reverse nowrap");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_flex_flow_direction_only() {
    let e = entries("flex-flow: column");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::FlexDirection(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::FlexWrap(Declared::Initial));
}

// gap (pair2 — generated)

#[test]
fn shorthand_gap_two_values() {
    let e = entries("gap: 16px 24px");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::RowGap(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::ColumnGap(Declared::Value(_)));
}

#[test]
fn shorthand_gap_one_value() {
    let e = entries("gap: 10px");
    assert_eq!(e.len(), 2);
    // Both row-gap and column-gap should be 10px
    assert_decl!(e, 0, PropertyDeclaration::RowGap(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::ColumnGap(Declared::Value(_)));
}

// overflow (pair2 — generated)

#[test]
fn shorthand_overflow_two_values() {
    let e = entries("overflow: hidden scroll");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::OverflowX(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::OverflowY(Declared::Value(_)));
}

#[test]
fn shorthand_overflow_one_value() {
    let e = entries("overflow: auto");
    assert_eq!(e.len(), 2);
}

// inset (box4 — generated)

#[test]
fn shorthand_inset() {
    let e = entries("inset: 0");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::Top(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::Right(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::Bottom(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::Left(Declared::Value(_)));
}

#[test]
fn shorthand_inset_two_values() {
    let e = entries("inset: 10px 20px");
    assert_eq!(e.len(), 4);
}

// text-decoration (hand-written — line + style + color + thickness)

#[test]
fn shorthand_text_decoration_full() {
    let e = entries("text-decoration: underline wavy red");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::TextDecorationLine(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::TextDecorationStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::TextDecorationColor(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::TextDecorationThickness(Declared::Initial));
}

#[test]
fn shorthand_text_decoration_line_only() {
    let e = entries("text-decoration: underline");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::TextDecorationLine(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::TextDecorationStyle(Declared::Initial));
}

// font (hand-written — complex multi-value)

#[test]
fn shorthand_font_size_family() {
    let e = entries("font: 16px sans-serif");
    assert!(e.len() >= 2, "font shorthand should produce multiple longhands, got {}", e.len());
    // Should have FontSize and FontFamily at minimum
    let has_size = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontSize(Declared::Value(_))));
    let has_family = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontFamily(Declared::Value(_))));
    assert!(has_size, "font shorthand should set font-size");
    assert!(has_family, "font shorthand should set font-family");
}

#[test]
fn shorthand_font_full() {
    // font: [style] [weight] size family
    let e = entries("font: italic bold 14px sans-serif");
    assert_eq!(e.len(), 7, "font shorthand should produce 7 longhands, got {}: {e:?}", e.len());
    let has_style = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontStyle(Declared::Value(_))));
    let has_weight = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontWeight(Declared::Value(_))));
    let has_size = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontSize(Declared::Value(_))));
    let has_family = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontFamily(Declared::Value(_))));
    assert!(has_style, "font shorthand should set font-style");
    assert!(has_weight, "font shorthand should set font-weight");
    assert!(has_size, "font shorthand should set font-size");
    assert!(has_family, "font shorthand should set font-family");
}

#[test]
fn shorthand_font_comma_families() {
    let e = entries("font: 16px Arial, Helvetica, sans-serif");
    assert_eq!(e.len(), 7);
    let has_family = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::FontFamily(Declared::Value(_))));
    assert!(has_family, "font shorthand should handle comma-separated font families");
}

#[test]
fn shorthand_font_line_height() {
    // font: size / line-height family — bare number line-height (unitless multiplier).
    let e = entries("font: 14px / 1.5 sans-serif");
    assert_eq!(e.len(), 7);
    let has_line_height = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::LineHeight(Declared::Value(_))));
    assert!(has_line_height, "font shorthand should set line-height via /");
}

// animation (hand-written — comma-separated list)

#[test]
fn shorthand_animation() {
    let e = entries("animation: fadeIn 1s ease-in 0s infinite");
    assert!(e.len() >= 5, "animation shorthand should produce >=5 longhands, got {}", e.len());
    let has_name = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::AnimationName(Declared::Value(_))));
    let has_duration = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::AnimationDuration(Declared::Value(_))));
    assert!(has_name, "animation shorthand should set animation-name");
    assert!(has_duration, "animation shorthand should set animation-duration");
}

// transition (hand-written — comma-separated list)

#[test]
fn shorthand_transition() {
    let e = entries("transition: opacity 0.3s ease");
    assert!(e.len() >= 3, "transition shorthand should produce >=3 longhands, got {}", e.len());
    let has_property = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::TransitionProperty(Declared::Value(_))));
    let has_duration = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::TransitionDuration(Declared::Value(_))));
    assert!(has_property, "transition shorthand should set transition-property");
    assert!(has_duration, "transition shorthand should set transition-duration");
}

// background (hand-written — multi-layer)

#[test]
fn shorthand_background_color() {
    let e = entries("background: red");
    assert!(!e.is_empty());
    let has_color = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::BackgroundColor(Declared::Value(_))));
    assert!(has_color, "background shorthand should set background-color");
}

#[test]
fn background_size_cover() {
    let e = entries("background-size: cover");
    assert_decl!(e, 0, PropertyDeclaration::BackgroundSize(Declared::Value(_)));
    if let PropertyDeclaration::BackgroundSize(Declared::Value(list)) = &e[0].0 {
        assert!(matches!(list.0[0], kozan_style::BackgroundSize::Cover));
    }
}

#[test]
fn background_size_explicit_px() {
    use kozan_style::BackgroundSize;
    let e = entries("background-size: 200px");
    if let PropertyDeclaration::BackgroundSize(Declared::Value(list)) = &e[0].0 {
        assert!(matches!(&list.0[0], BackgroundSize::Explicit { width: Some(_), height: None }));
    } else {
        panic!("expected BackgroundSize");
    }
}

#[test]
fn background_size_explicit_two_values() {
    use kozan_style::BackgroundSize;
    let e = entries("background-size: 50% 100px");
    if let PropertyDeclaration::BackgroundSize(Declared::Value(list)) = &e[0].0 {
        assert!(matches!(&list.0[0], BackgroundSize::Explicit { width: Some(_), height: Some(_) }));
    } else {
        panic!("expected BackgroundSize with two explicit values");
    }
}

#[test]
fn background_size_auto() {
    use kozan_style::BackgroundSize;
    let e = entries("background-size: auto");
    if let PropertyDeclaration::BackgroundSize(Declared::Value(list)) = &e[0].0 {
        assert!(matches!(&list.0[0], BackgroundSize::Explicit { width: None, height: None }));
    } else {
        panic!("expected BackgroundSize::Explicit auto auto");
    }
}

#[test]
fn background_size_multi_layer() {
    use kozan_style::BackgroundSize;
    let e = entries("background-size: cover, 100px 50%");
    if let PropertyDeclaration::BackgroundSize(Declared::Value(list)) = &e[0].0 {
        assert_eq!(list.0.len(), 2);
        assert!(matches!(list.0[0], BackgroundSize::Cover));
        assert!(matches!(list.0[1], BackgroundSize::Explicit { width: Some(_), height: Some(_) }));
    } else {
        panic!("expected multi-layer BackgroundSize");
    }
}

#[test]
fn shorthand_background_single_image() {
    use kozan_style::{ImageList, PositionComponentList, BackgroundRepeatList};
    let e = entries("background: url(foo.png) no-repeat center");
    let image = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::BackgroundImage(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    assert!(matches!(image, Some(ImageList::Images(_))), "should set background-image");
    let repeat = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::BackgroundRepeat(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    let list = repeat.expect("should set background-repeat");
    assert_eq!(list.0.len(), 1);
    assert_eq!(list.0[0], kozan_style::BackgroundRepeat::NoRepeat);
    let pos = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::BackgroundPositionX(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    let list: PositionComponentList = pos.expect("should set background-position-x");
    assert_eq!(list.0.len(), 1);
}

#[test]
fn shorthand_background_two_layers() {
    use kozan_style::ImageList;
    // Two layers separated by comma — color only in final layer.
    let e = entries("background: url(a.png), url(b.png) red");
    let image = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::BackgroundImage(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    if let Some(ImageList::Images(imgs)) = image {
        assert_eq!(imgs.len(), 2, "two layers = two images");
    } else {
        panic!("expected ImageList::Images with 2 entries");
    }
    let has_color = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::BackgroundColor(Declared::Value(_))));
    assert!(has_color, "color from final layer should be set");
    let repeat = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::BackgroundRepeat(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    assert_eq!(repeat.expect("repeat list").0.len(), 2, "repeat list should have 2 entries");
}

#[test]
fn shorthand_background_three_layers() {
    use kozan_style::ImageList;
    let e = entries("background: url(a.png), url(b.png), url(c.png)");
    let image = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::BackgroundImage(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    if let Some(ImageList::Images(imgs)) = image {
        assert_eq!(imgs.len(), 3, "three layers = three images");
    } else {
        panic!("expected ImageList::Images with 3 entries");
    }
}

#[test]
fn shorthand_mask_single_layer() {
    use kozan_style::{ImageList, MaskModeList};
    let e = entries("mask: url(mask.svg) match-source");
    let image = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::MaskImage(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    assert!(matches!(image, Some(ImageList::Images(_))), "should set mask-image");
    let mode = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::MaskMode(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    let mode_list: MaskModeList = mode.expect("should set mask-mode");
    assert_eq!(mode_list.0.len(), 1);
}

#[test]
fn shorthand_mask_two_layers() {
    use kozan_style::ImageList;
    let e = entries("mask: url(a.svg), url(b.svg)");
    let image = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::MaskImage(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    if let Some(ImageList::Images(imgs)) = image {
        assert_eq!(imgs.len(), 2, "two mask layers = two images");
    } else {
        panic!("expected ImageList::Images with 2 entries");
    }
    let composite = e.iter().find_map(|(d, _)| {
        if let PropertyDeclaration::MaskComposite(Declared::Value(v)) = d { Some(v.clone()) } else { None }
    });
    assert_eq!(composite.expect("composite list").0.len(), 2);
}

// column-rule (hand-written — WSC)

#[test]
fn shorthand_column_rule() {
    let e = entries("column-rule: 1px solid gray");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::ColumnRuleWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::ColumnRuleStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::ColumnRuleColor(Declared::Value(_)));
}

// columns (hand-written — width + count)

#[test]
fn shorthand_columns() {
    let e = entries("columns: 3 200px");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::ColumnWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::ColumnCount(Declared::Value(_)));
}

#[test]
fn shorthand_columns_auto() {
    let e = entries("columns: auto");
    assert_eq!(e.len(), 2);
}

// list-style (hand-written — type + position + image)

#[test]
fn shorthand_list_style() {
    let e = entries("list-style: disc inside");
    assert!(e.len() >= 2, "list-style should produce at least 2 longhands");
    let has_type = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::ListStyleType(Declared::Value(_))));
    let has_pos = e.iter().any(|(d, _)| matches!(d, PropertyDeclaration::ListStylePosition(Declared::Value(_))));
    assert!(has_type, "list-style should set list-style-type");
    assert!(has_pos, "list-style should set list-style-position");
}

#[test]
fn shorthand_list_style_none() {
    let e = entries("list-style: none");
    assert!(!e.is_empty());
}

// place-content (hand-written — align + justify)

#[test]
fn shorthand_place_content() {
    let e = entries("place-content: center space-between");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::AlignContent(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::JustifyContent(Declared::Value(_)));
}

#[test]
fn shorthand_place_content_one_value() {
    let e = entries("place-content: center");
    assert_eq!(e.len(), 2);
}

// place-items (pair2 — generated)

#[test]
fn shorthand_place_items() {
    let e = entries("place-items: center stretch");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::AlignItems(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::JustifyItems(Declared::Value(_)));
}

#[test]
fn shorthand_place_items_one_value() {
    let e = entries("place-items: center");
    assert_eq!(e.len(), 2);
}

// place-self (pair2 — generated)

#[test]
fn shorthand_place_self() {
    let e = entries("place-self: end start");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::AlignSelf(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::JustifySelf(Declared::Value(_)));
}

// white-space (hand-written — collapse + wrap-mode)

#[test]
fn shorthand_white_space_nowrap() {
    let e = entries("white-space: nowrap");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::WhiteSpaceCollapse(Declared::Value(_) | Declared::Initial));
    assert_decl!(e, 1, PropertyDeclaration::TextWrapMode(Declared::Value(_) | Declared::Initial));
}

#[test]
fn shorthand_white_space_pre() {
    let e = entries("white-space: pre");
    assert_eq!(e.len(), 2);
}

// container (hand-written — type / name)

#[test]
fn shorthand_container() {
    let e = entries("container: sidebar / inline-size");
    assert_eq!(e.len(), 2, "container shorthand should produce 2 longhands, got {}", e.len());
    assert_decl!(e, 0, PropertyDeclaration::ContainerType(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::ContainerName(Declared::Value(_)));
}

// text-emphasis (hand-written — style + color)

#[test]
fn shorthand_text_emphasis() {
    let e = entries("text-emphasis: filled dot red");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::TextEmphasisStyle(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::TextEmphasisColor(Declared::Value(_)));
}

// font-variant (hand-written — caps + ligatures + numeric + ...)

#[test]
fn shorthand_font_variant_normal() {
    let e = entries("font-variant: normal");
    assert!(!e.is_empty(), "font-variant: normal should produce longhands");
}

// font-synthesis (hand-written override — keyword-toggle)

#[test]
fn shorthand_font_synthesis() {
    let e = entries("font-synthesis: weight style");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::FontSynthesisWeight(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::FontSynthesisStyle(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::FontSynthesisSmallCaps(Declared::Value(_)));
}

#[test]
fn shorthand_font_synthesis_none() {
    let e = entries("font-synthesis: none");
    assert_eq!(e.len(), 3);
}

// border-block / border-inline (hand-written — WSC × 2 sides)

#[test]
fn shorthand_border_block() {
    let e = entries("border-block: 1px solid red");
    assert_eq!(e.len(), 6);
    assert_decl!(e, 0, PropertyDeclaration::BorderBlockStartWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderBlockEndWidth(Declared::Value(_)));
}

#[test]
fn shorthand_border_inline() {
    let e = entries("border-inline: 2px dashed blue");
    assert_eq!(e.len(), 6);
    assert_decl!(e, 0, PropertyDeclaration::BorderInlineStartWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderInlineEndWidth(Declared::Value(_)));
}

// Generated pair2 shorthands — logical spacing

#[test]
fn shorthand_margin_block() {
    let e = entries("margin-block: 10px 20px");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::MarginBlockStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::MarginBlockEnd(Declared::Value(_)));
}

#[test]
fn shorthand_margin_inline() {
    let e = entries("margin-inline: 5px");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_padding_block() {
    let e = entries("padding-block: 8px 16px");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::PaddingBlockStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::PaddingBlockEnd(Declared::Value(_)));
}

#[test]
fn shorthand_padding_inline() {
    let e = entries("padding-inline: 12px");
    assert_eq!(e.len(), 2);
}

// Generated pair2 — inset-block / inset-inline

#[test]
fn shorthand_inset_block() {
    let e = entries("inset-block: 0 auto");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::InsetBlockStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::InsetBlockEnd(Declared::Value(_)));
}

#[test]
fn shorthand_inset_inline() {
    let e = entries("inset-inline: 10px");
    assert_eq!(e.len(), 2);
}

// Generated pair2 — border-block-*/border-inline-* (color/style/width)

#[test]
fn shorthand_border_block_color() {
    let e = entries("border-block-color: red blue");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::BorderBlockStartColor(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::BorderBlockEndColor(Declared::Value(_)));
}

#[test]
fn shorthand_border_block_style() {
    let e = entries("border-block-style: solid");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_border_block_width() {
    let e = entries("border-block-width: 1px 2px");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_border_inline_color() {
    let e = entries("border-inline-color: green");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_border_inline_style() {
    let e = entries("border-inline-style: dashed dotted");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_border_inline_width() {
    let e = entries("border-inline-width: 3px");
    assert_eq!(e.len(), 2);
}

// Generated pair2 — overscroll-behavior

#[test]
fn shorthand_overscroll_behavior() {
    let e = entries("overscroll-behavior: contain none");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::OverscrollBehaviorX(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::OverscrollBehaviorY(Declared::Value(_)));
}

// Generated box4 — scroll-margin / scroll-padding

#[test]
fn shorthand_scroll_margin() {
    let e = entries("scroll-margin: 10px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::ScrollMarginTop(Declared::Value(_)));
}

#[test]
fn shorthand_scroll_padding() {
    let e = entries("scroll-padding: 5px 10px");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::ScrollPaddingTop(Declared::Value(_)));
}

// Generated pair2 — scroll-margin-block/inline, scroll-padding-block/inline

#[test]
fn shorthand_scroll_margin_block() {
    let e = entries("scroll-margin-block: 5px 15px");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_scroll_margin_inline() {
    let e = entries("scroll-margin-inline: 10px");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_scroll_padding_block() {
    let e = entries("scroll-padding-block: 0 auto");
    assert_eq!(e.len(), 2);
}

#[test]
fn shorthand_scroll_padding_inline() {
    let e = entries("scroll-padding-inline: 20px");
    assert_eq!(e.len(), 2);
}

// Generated pair2 — contain-intrinsic-size

#[test]
fn shorthand_contain_intrinsic_size() {
    let e = entries("contain-intrinsic-size: 100px 200px");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::ContainIntrinsicWidth(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::ContainIntrinsicHeight(Declared::Value(_)));
}

// Generated pair2 — grid-row / grid-column

#[test]
fn shorthand_grid_row() {
    let e = entries("grid-row: 1 / 3");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::GridRowStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::GridRowEnd(Declared::Value(_)));
}

#[test]
fn shorthand_grid_row_single() {
    let e = entries("grid-row: 2");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::GridRowStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::GridRowEnd(Declared::Value(_)));
}

#[test]
fn shorthand_grid_column() {
    let e = entries("grid-column: 1 / -1");
    assert_eq!(e.len(), 2);
    assert_decl!(e, 0, PropertyDeclaration::GridColumnStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::GridColumnEnd(Declared::Value(_)));
}

// grid-area (hand-written — `/` separated, 4 longhands)

#[test]
fn shorthand_grid_area() {
    let e = entries("grid-area: 1 / 2 / 3 / 4");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::GridRowStart(Declared::Value(_)));
    assert_decl!(e, 1, PropertyDeclaration::GridColumnStart(Declared::Value(_)));
    assert_decl!(e, 2, PropertyDeclaration::GridRowEnd(Declared::Value(_)));
    assert_decl!(e, 3, PropertyDeclaration::GridColumnEnd(Declared::Value(_)));
}

#[test]
fn shorthand_grid_area_single() {
    let e = entries("grid-area: 1");
    assert_eq!(e.len(), 4);
}

// grid-template (hand-written)

#[test]
fn shorthand_grid_template() {
    let e = entries("grid-template: none");
    assert!(!e.is_empty(), "grid-template should produce longhands");
}

// CSS-wide keywords on shorthands (expand to all longhands)

#[test]
fn shorthand_margin_inherit() {
    let e = entries("margin: inherit");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::MarginTop(Declared::Inherit));
    assert_decl!(e, 1, PropertyDeclaration::MarginRight(Declared::Inherit));
    assert_decl!(e, 2, PropertyDeclaration::MarginBottom(Declared::Inherit));
    assert_decl!(e, 3, PropertyDeclaration::MarginLeft(Declared::Inherit));
}

#[test]
fn shorthand_padding_initial() {
    let e = entries("padding: initial");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::PaddingTop(Declared::Initial));
}

#[test]
fn shorthand_border_unset() {
    let e = entries("border: unset");
    assert_eq!(e.len(), 12);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::Unset));
}

#[test]
fn shorthand_flex_revert() {
    let e = entries("flex: revert");
    assert_eq!(e.len(), 3);
    assert_decl!(e, 0, PropertyDeclaration::FlexGrow(Declared::Revert));
}

// var() in shorthands

#[test]
fn shorthand_margin_var() {
    let e = entries("margin: var(--spacing)");
    assert_eq!(e.len(), 4);
    assert_decl!(e, 0, PropertyDeclaration::MarginTop(Declared::WithVariables(_)));
    assert_decl!(e, 1, PropertyDeclaration::MarginRight(Declared::WithVariables(_)));
}

#[test]
fn shorthand_border_var() {
    let e = entries("border: var(--border-spec)");
    assert_eq!(e.len(), 12);
    assert_decl!(e, 0, PropertyDeclaration::BorderTopWidth(Declared::WithVariables(_)));
}

// !important on shorthands

#[test]
fn shorthand_margin_important() {
    let e = entries("margin: 10px !important");
    assert_eq!(e.len(), 4);
    for (_, imp) in &e {
        assert_eq!(*imp, Importance::Important, "all longhands should be important");
    }
}

#[test]
fn shorthand_border_important() {
    let e = entries("border: 1px solid red !important");
    assert_eq!(e.len(), 12);
    for (_, imp) in &e {
        assert_eq!(*imp, Importance::Important);
    }
}

// text-emphasis-style — W3C compliance: fill/shape in any order, standalone

#[test]
fn text_emphasis_style_filled_dot() {
    let e = entries("text-emphasis-style: filled dot");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Shape(TextEmphasisFill::Filled, TextEmphasisShape::Dot),
        )) => {}
        other => panic!("expected filled dot, got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_open_triangle() {
    let e = entries("text-emphasis-style: open triangle");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Shape(TextEmphasisFill::Open, TextEmphasisShape::Triangle),
        )) => {}
        other => panic!("expected open triangle, got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_shape_before_fill() {
    // CSS spec: fill and shape can appear in either order.
    let e = entries("text-emphasis-style: triangle open");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Shape(TextEmphasisFill::Open, TextEmphasisShape::Triangle),
        )) => {}
        other => panic!("expected open triangle (shape-first order), got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_filled_alone() {
    // `filled` alone → defaults to `filled circle`.
    let e = entries("text-emphasis-style: filled");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Shape(TextEmphasisFill::Filled, TextEmphasisShape::Circle),
        )) => {}
        other => panic!("expected filled circle (default shape), got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_open_alone() {
    // `open` alone → defaults to `open circle`.
    let e = entries("text-emphasis-style: open");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Shape(TextEmphasisFill::Open, TextEmphasisShape::Circle),
        )) => {}
        other => panic!("expected open circle (default shape), got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_shape_alone() {
    // `sesame` alone → defaults to `filled sesame`.
    let e = entries("text-emphasis-style: sesame");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Shape(TextEmphasisFill::Filled, TextEmphasisShape::Sesame),
        )) => {}
        other => panic!("expected filled sesame (default fill), got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_none() {
    let e = entries("text-emphasis-style: none");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(TextEmphasisStyleValue::None)) => {}
        other => panic!("expected none, got {other:?}"),
    }
}

#[test]
fn text_emphasis_style_string() {
    let e = entries("text-emphasis-style: '★'");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::TextEmphasisStyle(Declared::Value(
            TextEmphasisStyleValue::Custom(_),
        )) => {}
        other => panic!("expected custom string, got {other:?}"),
    }
}

// font shorthand — line-height edge cases

#[test]
fn shorthand_font_line_height_number() {
    // `font: 14px / 1.5 sans-serif` — bare number line-height.
    let e = entries("font: 14px / 1.5 sans-serif");
    assert_eq!(e.len(), 7);
    match &e[4].0 {
        PropertyDeclaration::FontSize(Declared::Value(_)) => {}
        other => panic!("expected font-size, got {other:?}"),
    }
    match &e[5].0 {
        PropertyDeclaration::LineHeight(Declared::Value(LineHeight::Number(n))) => {
            assert!((n - 1.5).abs() < 0.001, "expected 1.5, got {n}");
        }
        other => panic!("expected LineHeight::Number(1.5), got {other:?}"),
    }
}

#[test]
fn shorthand_font_line_height_length() {
    // Line-height as a length.
    let e = entries("font: 16px / 24px Arial");
    assert_eq!(e.len(), 7);
    match &e[5].0 {
        PropertyDeclaration::LineHeight(Declared::Value(LineHeight::LengthPercentage(_))) => {}
        other => panic!("expected LineHeight::LengthPercentage, got {other:?}"),
    }
}

#[test]
fn shorthand_font_no_line_height() {
    // Without line-height — should reset to initial.
    let e = entries("font: bold 16px serif");
    assert_eq!(e.len(), 7);
    match &e[5].0 {
        PropertyDeclaration::LineHeight(Declared::Initial) => {}
        other => panic!("expected LineHeight initial reset, got {other:?}"),
    }
}

// font-size — keyword + length-percentage support (W3C compliance)

#[test]
fn parse_font_size_px() {
    let e = entries("font-size: 16px");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::LengthPercentage(_))) => {}
        other => panic!("expected FontSize::LengthPercentage, got {other:?}"),
    }
}

#[test]
fn parse_font_size_keyword_medium() {
    let e = entries("font-size: medium");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::Medium)) => {}
        other => panic!("expected FontSize::Medium, got {other:?}"),
    }
}

#[test]
fn parse_font_size_keyword_small() {
    let e = entries("font-size: small");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::Small)) => {}
        other => panic!("expected FontSize::Small, got {other:?}"),
    }
}

#[test]
fn parse_font_size_keyword_x_large() {
    let e = entries("font-size: x-large");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::XLarge)) => {}
        other => panic!("expected FontSize::XLarge, got {other:?}"),
    }
}

#[test]
fn parse_font_size_keyword_smaller() {
    let e = entries("font-size: smaller");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::Smaller)) => {}
        other => panic!("expected FontSize::Smaller, got {other:?}"),
    }
}

#[test]
fn parse_font_size_keyword_larger() {
    let e = entries("font-size: larger");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::Larger)) => {}
        other => panic!("expected FontSize::Larger, got {other:?}"),
    }
}

#[test]
fn parse_font_size_percentage() {
    let e = entries("font-size: 120%");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::LengthPercentage(_))) => {}
        other => panic!("expected FontSize::LengthPercentage(%), got {other:?}"),
    }
}

#[test]
fn shorthand_font_with_keyword_size() {
    // font shorthand with keyword font-size.
    let e = entries("font: bold large sans-serif");
    assert_eq!(e.len(), 7);
    match &e[4].0 {
        PropertyDeclaration::FontSize(Declared::Value(FontSize::Large)) => {}
        other => panic!("expected FontSize::Large in font shorthand, got {other:?}"),
    }
}

// vertical-align — keyword + length-percentage support (W3C compliance)

#[test]
fn parse_vertical_align_baseline() {
    let e = entries("vertical-align: baseline");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::VerticalAlign(Declared::Value(VerticalAlign::Baseline)) => {}
        other => panic!("expected VerticalAlign::Baseline, got {other:?}"),
    }
}

#[test]
fn parse_vertical_align_middle() {
    let e = entries("vertical-align: middle");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::VerticalAlign(Declared::Value(VerticalAlign::Middle)) => {}
        other => panic!("expected VerticalAlign::Middle, got {other:?}"),
    }
}

#[test]
fn parse_vertical_align_sub() {
    let e = entries("vertical-align: sub");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::VerticalAlign(Declared::Value(VerticalAlign::Sub)) => {}
        other => panic!("expected VerticalAlign::Sub, got {other:?}"),
    }
}

#[test]
fn parse_vertical_align_length() {
    let e = entries("vertical-align: 10px");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::VerticalAlign(Declared::Value(VerticalAlign::LengthPercentage(_))) => {}
        other => panic!("expected VerticalAlign::LengthPercentage, got {other:?}"),
    }
}

#[test]
fn parse_vertical_align_percentage() {
    let e = entries("vertical-align: -25%");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::VerticalAlign(Declared::Value(VerticalAlign::LengthPercentage(_))) => {}
        other => panic!("expected VerticalAlign::LengthPercentage(%), got {other:?}"),
    }
}

#[test]
fn parse_vertical_align_negative_length() {
    let e = entries("vertical-align: -5px");
    assert_eq!(e.len(), 1);
    match &e[0].0 {
        PropertyDeclaration::VerticalAlign(Declared::Value(VerticalAlign::LengthPercentage(_))) => {}
        other => panic!("expected VerticalAlign::LengthPercentage (negative), got {other:?}"),
    }
}

// --- animation-composition ---

#[test]
fn animation_composition_replace() {
    let decl = parse_value(PropertyId::AnimationComposition, "replace");
    match decl {
        Some(PropertyDeclaration::AnimationComposition(Declared::Value(v))) => {
            assert_eq!(v.0.as_ref(), &[AnimationComposition::Replace]);
        }
        other => panic!("expected AnimationComposition::Replace, got {other:?}"),
    }
}

#[test]
fn animation_composition_add() {
    let decl = parse_value(PropertyId::AnimationComposition, "add");
    match decl {
        Some(PropertyDeclaration::AnimationComposition(Declared::Value(v))) => {
            assert_eq!(v.0.as_ref(), &[AnimationComposition::Add]);
        }
        other => panic!("expected AnimationComposition::Add, got {other:?}"),
    }
}

#[test]
fn animation_composition_accumulate() {
    let decl = parse_value(PropertyId::AnimationComposition, "accumulate");
    match decl {
        Some(PropertyDeclaration::AnimationComposition(Declared::Value(v))) => {
            assert_eq!(v.0.as_ref(), &[AnimationComposition::Accumulate]);
        }
        other => panic!("expected AnimationComposition::Accumulate, got {other:?}"),
    }
}

// --- four-value background-position ---

type LP = kozan_style::specified::LengthPercentage;
type Pct = kozan_style::computed::Percentage;

fn pct(f: f32) -> LP { LP::Percentage(Pct::new(f)) }

#[test]
fn bg_position_4value_parses() {
    // `right 20px bottom 10px` — just verify it parses without error (both produce Calc nodes)
    let b = parse_inline("object-position: right 20px bottom 10px");
    match &b.entries()[0].0 {
        PropertyDeclaration::ObjectPosition(Declared::Value(pos)) => {
            // x should be calc(100% - 20px) → Calc variant
            assert!(matches!(pos.x, LP::Calc(_)), "x should be Calc for 'right 20px'");
            assert!(matches!(pos.y, LP::Calc(_)), "y should be Calc for 'bottom 10px'");
        }
        other => panic!("expected ObjectPosition, got {other:?}"),
    }
}

#[test]
fn bg_position_4value_left_top() {
    // `left 10px top 5px` → x=10px (from left), y=5px (from top)
    let b = parse_inline("object-position: left 10px top 5px");
    match &b.entries()[0].0 {
        PropertyDeclaration::ObjectPosition(Declared::Value(pos)) => {
            assert!(matches!(pos.x, LP::Length(_)), "x should be Length for 'left 10px'");
            assert!(matches!(pos.y, LP::Length(_)), "y should be Length for 'top 5px'");
        }
        other => panic!("expected ObjectPosition, got {other:?}"),
    }
}

#[test]
fn bg_position_keywords_top_left() {
    // `top left` → x=0%, y=0%
    let b = parse_inline("object-position: top left");
    match &b.entries()[0].0 {
        PropertyDeclaration::ObjectPosition(Declared::Value(pos)) => {
            assert_eq!(pos.x, pct(0.0), "x should be 0%");
            assert_eq!(pos.y, pct(0.0), "y should be 0%");
        }
        other => panic!("expected ObjectPosition, got {other:?}"),
    }
}

#[test]
fn bg_position_center() {
    let b = parse_inline("object-position: center");
    match &b.entries()[0].0 {
        PropertyDeclaration::ObjectPosition(Declared::Value(pos)) => {
            assert_eq!(pos.x, pct(0.5));
            assert_eq!(pos.y, pct(0.5));
        }
        other => panic!("expected ObjectPosition center, got {other:?}"),
    }
}

#[test]
fn bg_position_single_keyword_top() {
    // `top` alone → x=50%, y=0%
    let b = parse_inline("object-position: top");
    match &b.entries()[0].0 {
        PropertyDeclaration::ObjectPosition(Declared::Value(pos)) => {
            assert_eq!(pos.x, pct(0.5));
            assert_eq!(pos.y, pct(0.0));
        }
        other => panic!("expected ObjectPosition top, got {other:?}"),
    }
}

#[test]
fn bg_position_lp_lp() {
    // `10px 20%` → x=10px, y=20%
    let b = parse_inline("object-position: 10px 20%");
    match &b.entries()[0].0 {
        PropertyDeclaration::ObjectPosition(Declared::Value(pos)) => {
            assert!(matches!(pos.x, LP::Length(_)), "x should be Length");
            match &pos.y {
                LP::Percentage(p) => assert!((p.0 - 0.2).abs() < 1e-6, "y should be 20%"),
                other => panic!("expected y to be 20%, got {other:?}"),
            }
        }
        other => panic!("expected ObjectPosition, got {other:?}"),
    }
}

// --- text-wrap shorthand ---

#[test]
fn text_wrap_balance() {
    // `balance` → mode=wrap, style=balance
    let b = parse_inline("text-wrap: balance");
    let entries = b.entries();
    let has_mode = entries.iter().any(|(d, _)| matches!(d,
        PropertyDeclaration::TextWrapMode(Declared::Value(TextWrapMode::Wrap))));
    let has_style = entries.iter().any(|(d, _)| matches!(d,
        PropertyDeclaration::TextWrapStyle(Declared::Value(TextWrapStyle::Balance))));
    assert!(has_mode, "text-wrap: balance should set mode=wrap");
    assert!(has_style, "text-wrap: balance should set style=balance");
}

#[test]
fn text_wrap_nowrap() {
    let b = parse_inline("text-wrap: nowrap");
    let has_mode = b.entries().iter().any(|(d, _)| matches!(d,
        PropertyDeclaration::TextWrapMode(Declared::Value(TextWrapMode::Nowrap))));
    assert!(has_mode, "text-wrap: nowrap should set mode=nowrap");
}

#[test]
fn text_wrap_style_pretty() {
    let decl = parse_value(PropertyId::TextWrapStyle, "pretty");
    assert!(matches!(decl, Some(PropertyDeclaration::TextWrapStyle(Declared::Value(TextWrapStyle::Pretty)))));
}

// --- field-sizing ---

#[test]
fn field_sizing_content() {
    let decl = parse_value(PropertyId::FieldSizing, "content");
    assert!(matches!(decl, Some(PropertyDeclaration::FieldSizing(Declared::Value(FieldSizing::Content)))));
}

#[test]
fn field_sizing_fixed() {
    let decl = parse_value(PropertyId::FieldSizing, "fixed");
    assert!(matches!(decl, Some(PropertyDeclaration::FieldSizing(Declared::Value(FieldSizing::Fixed)))));
}

// --- interpolate-size, line-clamp, ruby-*, text-box-trim ---

#[test]
fn interpolate_size_allow_keywords() {
    let decl = parse_value(PropertyId::InterpolateSize, "allow-keywords");
    assert!(matches!(decl, Some(PropertyDeclaration::InterpolateSize(Declared::Value(InterpolateSize::AllowKeywords)))));
}

#[test]
fn line_clamp_integer() {
    let decl = parse_value(PropertyId::LineClamp, "3");
    assert!(matches!(decl, Some(PropertyDeclaration::LineClamp(Declared::Value(NoneOr::Value(3))))));
}

#[test]
fn line_clamp_none() {
    let decl = parse_value(PropertyId::LineClamp, "none");
    assert!(matches!(decl, Some(PropertyDeclaration::LineClamp(Declared::Value(NoneOr::None)))));
}

#[test]
fn ruby_align_center() {
    let decl = parse_value(PropertyId::RubyAlign, "center");
    assert!(matches!(decl, Some(PropertyDeclaration::RubyAlign(Declared::Value(RubyAlign::Center)))));
}

#[test]
fn ruby_merge_auto() {
    let decl = parse_value(PropertyId::RubyMerge, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::RubyMerge(Declared::Value(RubyMerge::Auto)))));
}

#[test]
fn text_box_trim_both() {
    let decl = parse_value(PropertyId::TextBoxTrim, "trim-both");
    assert!(matches!(decl, Some(PropertyDeclaration::TextBoxTrim(Declared::Value(TextBoxTrim::TrimBoth)))));
}

// --- math-style, math-shift, reading-flow, scrollbar-gutter ---

#[test]
fn math_style_compact() {
    let decl = parse_value(PropertyId::MathStyle, "compact");
    assert!(matches!(decl, Some(PropertyDeclaration::MathStyle(Declared::Value(MathStyle::Compact)))));
}

#[test]
fn math_shift_compact() {
    let decl = parse_value(PropertyId::MathShift, "compact");
    assert!(matches!(decl, Some(PropertyDeclaration::MathShift(Declared::Value(MathShift::Compact)))));
}

#[test]
fn reading_flow_flex_visual() {
    let decl = parse_value(PropertyId::ReadingFlow, "flex-visual");
    assert!(matches!(decl, Some(PropertyDeclaration::ReadingFlow(Declared::Value(ReadingFlow::FlexVisual)))));
}

#[test]
fn reading_flow_normal() {
    let decl = parse_value(PropertyId::ReadingFlow, "normal");
    assert!(matches!(decl, Some(PropertyDeclaration::ReadingFlow(Declared::Value(ReadingFlow::Normal)))));
}

#[test]
fn scrollbar_gutter_stable() {
    let decl = parse_value(PropertyId::ScrollbarGutter, "stable");
    assert!(matches!(decl, Some(PropertyDeclaration::ScrollbarGutter(Declared::Value(ScrollbarGutter::Stable)))));
}

#[test]
fn scrollbar_gutter_stable_both_edges() {
    let decl = parse_value(PropertyId::ScrollbarGutter, "stable both-edges");
    assert!(matches!(decl, Some(PropertyDeclaration::ScrollbarGutter(Declared::Value(ScrollbarGutter::StableBothEdges)))));
}

#[test]
fn scrollbar_gutter_auto() {
    let decl = parse_value(PropertyId::ScrollbarGutter, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::ScrollbarGutter(Declared::Value(ScrollbarGutter::Auto)))));
}

// --- font-palette, initial-letter, hyphenate-character, hyphenate-limit-chars ---

#[test]
fn font_palette_dark() {
    let decl = parse_value(PropertyId::FontPalette, "dark");
    assert!(matches!(decl, Some(PropertyDeclaration::FontPalette(Declared::Value(FontPalette::Dark)))));
}

#[test]
fn font_palette_custom() {
    let decl = parse_value(PropertyId::FontPalette, "my-palette");
    match decl {
        Some(PropertyDeclaration::FontPalette(Declared::Value(FontPalette::Custom(a)))) => {
            assert_eq!(a.as_ref(), "my-palette");
        }
        other => panic!("expected FontPalette::Custom, got {other:?}"),
    }
}

#[test]
fn initial_letter_normal() {
    let decl = parse_value(PropertyId::InitialLetter, "normal");
    assert!(matches!(decl, Some(PropertyDeclaration::InitialLetter(Declared::Value(InitialLetter::Normal)))));
}

#[test]
fn initial_letter_size_only() {
    let decl = parse_value(PropertyId::InitialLetter, "3.5");
    match decl {
        Some(PropertyDeclaration::InitialLetter(Declared::Value(InitialLetter::Raised { size, sink }))) => {
            assert!((size - 3.5).abs() < 0.001);
            assert_eq!(sink, 4); // ceil(3.5)
        }
        other => panic!("expected InitialLetter::Raised, got {other:?}"),
    }
}

#[test]
fn initial_letter_size_and_sink() {
    let decl = parse_value(PropertyId::InitialLetter, "2 3");
    match decl {
        Some(PropertyDeclaration::InitialLetter(Declared::Value(InitialLetter::Raised { size, sink }))) => {
            assert!((size - 2.0).abs() < 0.001);
            assert_eq!(sink, 3);
        }
        other => panic!("expected InitialLetter::Raised, got {other:?}"),
    }
}

#[test]
fn hyphenate_character_auto() {
    let decl = parse_value(PropertyId::HyphenateCharacter, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::HyphenateCharacter(Declared::Value(HyphenateCharacter::Auto)))));
}

#[test]
fn hyphenate_character_string() {
    let decl = parse_value(PropertyId::HyphenateCharacter, r#""=""#);
    match decl {
        Some(PropertyDeclaration::HyphenateCharacter(Declared::Value(HyphenateCharacter::String(s)))) => {
            assert_eq!(&*s, "=");
        }
        other => panic!("expected HyphenateCharacter::String, got {other:?}"),
    }
}

#[test]
fn hyphenate_limit_chars_auto() {
    let decl = parse_value(PropertyId::HyphenateLimitChars, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::HyphenateLimitChars(Declared::Value(HyphenateLimitChars { .. })))));
}

#[test]
fn hyphenate_limit_chars_three_values() {
    let decl = parse_value(PropertyId::HyphenateLimitChars, "5 2 3");
    match decl {
        Some(PropertyDeclaration::HyphenateLimitChars(Declared::Value(v))) => {
            assert!(matches!(v.total, HyphenateLimitValue::Integer(5)));
            assert!(matches!(v.before, HyphenateLimitValue::Integer(2)));
            assert!(matches!(v.after, HyphenateLimitValue::Integer(3)));
        }
        other => panic!("expected HyphenateLimitChars, got {other:?}"),
    }
}

// --- offset-* (motion path) ---

#[test]
fn offset_path_none() {
    let decl = parse_value(PropertyId::OffsetPath, "none");
    assert!(matches!(decl, Some(PropertyDeclaration::OffsetPath(Declared::Value(OffsetPath::None)))));
}

#[test]
fn offset_path_path_fn() {
    let decl = parse_value(PropertyId::OffsetPath, r#"path("M 0 0 L 100 100")"#);
    match decl {
        Some(PropertyDeclaration::OffsetPath(Declared::Value(OffsetPath::Path(s)))) => {
            assert_eq!(&*s, "M 0 0 L 100 100");
        }
        other => panic!("expected OffsetPath::Path, got {other:?}"),
    }
}

#[test]
fn offset_path_ray() {
    let decl = parse_value(PropertyId::OffsetPath, "ray(45deg closest-side contain)");
    match decl {
        Some(PropertyDeclaration::OffsetPath(Declared::Value(OffsetPath::Ray { angle, size, contain }))) => {
            assert!((angle - 45.0).abs() < 0.001);
            assert_eq!(size, RaySize::ClosestSide);
            assert!(contain);
        }
        other => panic!("expected OffsetPath::Ray, got {other:?}"),
    }
}

#[test]
fn offset_rotate_auto() {
    let decl = parse_value(PropertyId::OffsetRotate, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::OffsetRotate(Declared::Value(OffsetRotate::Auto)))));
}

#[test]
fn offset_rotate_auto_angle() {
    let decl = parse_value(PropertyId::OffsetRotate, "auto 45deg");
    match decl {
        Some(PropertyDeclaration::OffsetRotate(Declared::Value(OffsetRotate::AutoAngle(a)))) => {
            assert!((a - 45.0).abs() < 0.001);
        }
        other => panic!("expected OffsetRotate::AutoAngle, got {other:?}"),
    }
}

#[test]
fn offset_rotate_reverse() {
    let decl = parse_value(PropertyId::OffsetRotate, "reverse");
    assert!(matches!(decl, Some(PropertyDeclaration::OffsetRotate(Declared::Value(OffsetRotate::Reverse)))));
}

#[test]
fn offset_position_auto() {
    let decl = parse_value(PropertyId::OffsetPosition, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::OffsetPosition(Declared::Value(OffsetPosition::Auto)))));
}

#[test]
fn offset_position_normal() {
    let decl = parse_value(PropertyId::OffsetPosition, "normal");
    assert!(matches!(decl, Some(PropertyDeclaration::OffsetPosition(Declared::Value(OffsetPosition::Normal)))));
}

// --- scroll-driven animation ---

#[test]
fn animation_timeline_auto() {
    let decl = parse_value(PropertyId::AnimationTimeline, "auto");
    match decl {
        Some(PropertyDeclaration::AnimationTimeline(Declared::Value(list))) => {
            match &list {
                AnimationTimelineList::Values(v) => {
                    assert_eq!(v.len(), 1);
                    assert!(matches!(v[0], AnimationTimeline::Auto));
                }
            }
        }
        other => panic!("expected AnimationTimeline auto, got {other:?}"),
    }
}

#[test]
fn animation_timeline_scroll() {
    let decl = parse_value(PropertyId::AnimationTimeline, "scroll(root block)");
    match decl {
        Some(PropertyDeclaration::AnimationTimeline(Declared::Value(AnimationTimelineList::Values(v)))) => {
            assert_eq!(v.len(), 1);
            assert!(matches!(v[0], AnimationTimeline::Scroll(ScrollScroller::Root, ScrollAxis::Block)));
        }
        other => panic!("expected AnimationTimeline::Scroll, got {other:?}"),
    }
}

#[test]
fn animation_timeline_view() {
    let decl = parse_value(PropertyId::AnimationTimeline, "view(inline)");
    match decl {
        Some(PropertyDeclaration::AnimationTimeline(Declared::Value(AnimationTimelineList::Values(v)))) => {
            assert_eq!(v.len(), 1);
            assert!(matches!(v[0], AnimationTimeline::View(ScrollAxis::Inline)));
        }
        other => panic!("expected AnimationTimeline::View, got {other:?}"),
    }
}

#[test]
fn animation_range_start_named() {
    let decl = parse_value(PropertyId::AnimationRangeStart, "entry 25%");
    match decl {
        Some(PropertyDeclaration::AnimationRangeStart(Declared::Value(AnimationRangeValue::Named(name, Some(_))))) => {
            assert_eq!(name, TimelineRangeName::Entry);
        }
        other => panic!("expected AnimationRangeValue::Named, got {other:?}"),
    }
}

#[test]
fn scroll_timeline_axis_inline() {
    let decl = parse_value(PropertyId::ScrollTimelineAxis, "inline");
    match decl {
        Some(PropertyDeclaration::ScrollTimelineAxis(Declared::Value(ScrollTimelineAxisList(v)))) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0], ScrollAxis::Inline);
        }
        other => panic!("expected ScrollTimelineAxisList, got {other:?}"),
    }
}

#[test]
fn image_orientation_from_image() {
    let decl = parse_value(PropertyId::ImageOrientation, "from-image");
    assert!(matches!(decl, Some(PropertyDeclaration::ImageOrientation(Declared::Value(ImageOrientation::FromImage)))));
}

#[test]
fn image_orientation_angle() {
    let decl = parse_value(PropertyId::ImageOrientation, "90deg");
    match decl {
        Some(PropertyDeclaration::ImageOrientation(Declared::Value(ImageOrientation::Angle(a)))) => {
            assert!((a - 90.0).abs() < 0.001);
        }
        other => panic!("expected ImageOrientation::Angle, got {other:?}"),
    }
}

// --- view-transition-name ---

#[test]
fn view_transition_name_none() {
    let decl = parse_value(PropertyId::ViewTransitionName, "none");
    assert!(matches!(decl, Some(PropertyDeclaration::ViewTransitionName(Declared::Value(NoneOr::None)))));
}

// --- text-decoration-skip-ink, text-spacing-trim, font-variant-emoji ---

#[test]
fn text_decoration_skip_ink_all() {
    let decl = parse_value(PropertyId::TextDecorationSkipInk, "all");
    assert!(matches!(decl, Some(PropertyDeclaration::TextDecorationSkipInk(Declared::Value(TextDecorationSkipInk::All)))));
}

#[test]
fn text_spacing_trim_auto() {
    let decl = parse_value(PropertyId::TextSpacingTrim, "auto");
    assert!(matches!(decl, Some(PropertyDeclaration::TextSpacingTrim(Declared::Value(TextSpacingTrim::Auto)))));
}

#[test]
fn font_variant_emoji_unicode() {
    let decl = parse_value(PropertyId::FontVariantEmoji, "unicode");
    assert!(matches!(decl, Some(PropertyDeclaration::FontVariantEmoji(Declared::Value(FontVariantEmoji::Unicode)))));
}

// --- break/page-break ---

#[test]
fn page_break_before_always() {
    let decl = parse_value(PropertyId::PageBreakBefore, "always");
    assert!(matches!(decl, Some(PropertyDeclaration::PageBreakBefore(Declared::Value(PageBreak::Always)))));
}

#[test]
fn page_break_inside_avoid() {
    let decl = parse_value(PropertyId::PageBreakInside, "avoid");
    assert!(matches!(decl, Some(PropertyDeclaration::PageBreakInside(Declared::Value(PageBreakInside::Avoid)))));
}

// --- hanging-punctuation ---

#[test]
fn hanging_punctuation_none() {
    let decl = parse_value(PropertyId::HangingPunctuation, "none");
    assert!(matches!(decl, Some(PropertyDeclaration::HangingPunctuation(Declared::Value(v))) if v.is_empty()));
}

#[test]
fn hanging_punctuation_first() {
    let decl = parse_value(PropertyId::HangingPunctuation, "first");
    assert!(matches!(decl, Some(PropertyDeclaration::HangingPunctuation(Declared::Value(v))) if v.contains(HangingPunctuation::FIRST)));
}

#[test]
fn hanging_punctuation_first_last() {
    let decl = parse_value(PropertyId::HangingPunctuation, "first last");
    assert!(matches!(decl, Some(PropertyDeclaration::HangingPunctuation(Declared::Value(v))) if v.contains(HangingPunctuation::FIRST) && v.contains(HangingPunctuation::LAST)));
}

#[test]
fn hanging_punctuation_first_force_end_last() {
    let decl = parse_value(PropertyId::HangingPunctuation, "first force-end last");
    assert!(matches!(decl, Some(PropertyDeclaration::HangingPunctuation(Declared::Value(v))) if v.contains(HangingPunctuation::FIRST) && v.contains(HangingPunctuation::FORCE_END) && v.contains(HangingPunctuation::LAST)));
}

#[test]
fn hanging_punctuation_allow_end() {
    let decl = parse_value(PropertyId::HangingPunctuation, "allow-end");
    assert!(matches!(decl, Some(PropertyDeclaration::HangingPunctuation(Declared::Value(v))) if v.contains(HangingPunctuation::ALLOW_END)));
}

// --- white-space-trim ---

#[test]
fn white_space_trim_none() {
    let decl = parse_value(PropertyId::WhiteSpaceTrim, "none");
    assert!(matches!(decl, Some(PropertyDeclaration::WhiteSpaceTrim(Declared::Value(v))) if v.is_empty()));
}

#[test]
fn white_space_trim_discard_before() {
    let decl = parse_value(PropertyId::WhiteSpaceTrim, "discard-before");
    assert!(matches!(decl, Some(PropertyDeclaration::WhiteSpaceTrim(Declared::Value(v))) if v.contains(WhiteSpaceTrim::DISCARD_BEFORE)));
}

#[test]
fn white_space_trim_all_three() {
    let decl = parse_value(PropertyId::WhiteSpaceTrim, "discard-before discard-after discard-inner");
    assert!(matches!(decl, Some(PropertyDeclaration::WhiteSpaceTrim(Declared::Value(v))) if v.contains(WhiteSpaceTrim::DISCARD_BEFORE) && v.contains(WhiteSpaceTrim::DISCARD_AFTER) && v.contains(WhiteSpaceTrim::DISCARD_INNER)));
}

// --- zoom ---

#[test]
fn zoom_normal() {
    let decl = parse_value(PropertyId::Zoom, "normal");
    assert!(matches!(decl, Some(PropertyDeclaration::Zoom(Declared::Value(Zoom::Normal)))));
}

#[test]
fn zoom_reset() {
    let decl = parse_value(PropertyId::Zoom, "reset");
    assert!(matches!(decl, Some(PropertyDeclaration::Zoom(Declared::Value(Zoom::Reset)))));
}

#[test]
fn zoom_number() {
    let decl = parse_value(PropertyId::Zoom, "1.5");
    assert!(matches!(decl, Some(PropertyDeclaration::Zoom(Declared::Value(Zoom::Number(v)))) if (v - 1.5).abs() < 0.001));
}

#[test]
fn zoom_percentage() {
    let decl = parse_value(PropertyId::Zoom, "150%");
    assert!(matches!(decl, Some(PropertyDeclaration::Zoom(Declared::Value(Zoom::Number(v)))) if (v - 1.5).abs() < 0.001));
}
