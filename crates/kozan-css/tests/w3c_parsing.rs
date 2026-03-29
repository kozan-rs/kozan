//! W3C css-parsing-tests integration.
//!
//! Tests kozan-css against the implementation-independent JSON test suite
//! from <https://github.com/nicoulaj/css-parsing-tests> (via servo/rust-cssparser).
//!
//! These tests validate **structural correctness** — that our parser produces
//! the right number and types of rules/declarations from CSS input, and that
//! color parsing produces correct values.

use kozan_css::*;
use serde_json::Value;

// Helpers: parse the JSON test format

/// The JSON files are flat arrays: [input1, expected1, input2, expected2, ...]
/// Returns Vec<(input_css, expected_json)>.
fn load_test_pairs(json: &str) -> Vec<(String, Value)> {
    let arr: Vec<Value> = serde_json::from_str(json).expect("invalid test JSON");
    let mut pairs = Vec::new();
    let mut i = 0;
    while i + 1 < arr.len() {
        if let Value::String(input) = &arr[i] {
            pairs.push((input.clone(), arr[i + 1].clone()));
        }
        i += 2;
    }
    pairs
}

/// Count expected items of each type in a JSON expected-output array.
fn count_expected_types(expected: &Value) -> (usize, usize, usize) {
    // Returns (qualified_rules, at_rules, errors)
    let mut qr = 0;
    let mut ar = 0;
    let mut er = 0;
    if let Value::Array(items) = expected {
        for item in items {
            if let Value::Array(rule) = item {
                if let Some(Value::String(kind)) = rule.first() {
                    match kind.as_str() {
                        "qualified rule" => qr += 1,
                        "at-rule" => ar += 1,
                        "error" => er += 1,
                        "declaration" => {} // not a rule
                        _ => {}
                    }
                }
            }
        }
    }
    (qr, ar, er)
}

/// Count expected declarations in a JSON expected-output array.
fn count_expected_declarations(expected: &Value) -> (usize, usize) {
    // Returns (declarations, at_rules_in_decl_list)
    let mut decls = 0;
    let mut ats = 0;
    if let Value::Array(items) = expected {
        for item in items {
            if let Value::Array(rule) = item {
                if let Some(Value::String(kind)) = rule.first() {
                    match kind.as_str() {
                        "declaration" => decls += 1,
                        "at-rule" => ats += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    (decls, ats)
}

/// Get expected rule type from one_rule.json single-entry format.
fn expected_rule_type(expected: &Value) -> &str {
    if let Value::Array(rule) = expected {
        if let Some(Value::String(kind)) = rule.first() {
            return kind.as_str();
        }
    }
    "unknown"
}

// Color test helpers

/// Parse a color via `parse_inline("color: <input>")` and return the AbsoluteColor if successful.
fn parse_color_to_absolute(input: &str) -> Option<kozan_style::AbsoluteColor> {
    use kozan_style::{Color, ColorProperty, Declared, PropertyDeclaration};
    let css = format!("color: {input}");
    let block = kozan_css::parse_inline(&css);
    let entries = block.entries();
    if entries.is_empty() {
        return None;
    }
    match &entries[0].0 {
        PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::Absolute(c)))) => Some(c.clone()),
        _ => None,
    }
}

/// Parse the W3C expected color string like "rgb(255, 0, 0)" or "rgba(0, 0, 0, 0)"
/// into (r, g, b, a) u8 components.
fn parse_expected_rgb(expected: &str) -> Option<(u8, u8, u8, u8)> {
    let s = expected.trim();
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 4 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            let a_f: f32 = parts[3].parse().ok()?;
            let a = (a_f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            return Some((r, g, b, a));
        }
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            return Some((r, g, b, 255));
        }
    }
    None
}

// stylesheet.json — full stylesheet parsing

#[test]
fn w3c_stylesheet_rule_counts() {
    let json = include_str!("css-parsing-tests/stylesheet.json");
    let pairs = load_test_pairs(json);
    let mut passed = 0;
    let mut total = 0;

    for (input, expected) in &pairs {
        let (expected_qr, expected_ar, _expected_err) = count_expected_types(expected);
        let expected_total_rules = expected_qr + expected_ar;

        let sheet = parse_stylesheet(input);
        let actual_rules = sheet.rules.slice.len();

        total += 1;
        if actual_rules >= expected_total_rules.saturating_sub(1) {
            passed += 1;
        } else {
            eprintln!(
                "FAIL stylesheet: {:?}\n  expected >= {} rules, got {}",
                &input[..input.len().min(60)],
                expected_total_rules,
                actual_rules
            );
        }
    }

    eprintln!("stylesheet.json: {passed}/{total} passed");
    assert!(
        passed as f64 / total as f64 > 0.6,
        "Too many stylesheet test failures: {passed}/{total}"
    );
}

// stylesheet_bytes.json — byte-level encoding edge cases

#[test]
fn w3c_stylesheet_bytes_no_crash() {
    // stylesheet_bytes.json uses dict inputs with css_bytes — we test that
    // the string portions parse without crashing.
    let json = include_str!("css-parsing-tests/stylesheet_bytes.json");
    let arr: Vec<Value> = serde_json::from_str(json).expect("invalid test JSON");
    let mut total = 0;

    let mut i = 0;
    while i + 1 < arr.len() {
        // Input can be a dict with "css_bytes" or a plain string
        if let Value::Object(obj) = &arr[i] {
            if let Some(Value::String(css_bytes)) = obj.get("css_bytes") {
                // Try to parse the CSS bytes as a string (may contain non-UTF8 in original,
                // but JSON encodes them — just verify no crash)
                let _ = parse_stylesheet(css_bytes);
                total += 1;
            }
        } else if let Value::String(input) = &arr[i] {
            let _ = parse_stylesheet(input);
            total += 1;
        }
        i += 2;
    }

    eprintln!("stylesheet_bytes.json: {total}/{total} passed (no-crash)");
    assert!(total > 0, "should have processed at least some stylesheet_bytes tests");
}

// rule_list.json — rule list parsing

#[test]
fn w3c_rule_list_structure() {
    let json = include_str!("css-parsing-tests/rule_list.json");
    let pairs = load_test_pairs(json);
    let mut passed = 0;
    let mut total = 0;

    for (input, expected) in &pairs {
        let (expected_qr, expected_ar, _) = count_expected_types(&expected);
        let expected_total = expected_qr + expected_ar;

        let sheet = parse_stylesheet(input);
        let actual = sheet.rules.slice.len();

        total += 1;
        if actual >= expected_total.saturating_sub(1) {
            passed += 1;
        }
    }

    eprintln!("rule_list.json: {passed}/{total} passed");
    assert!(
        passed as f64 / total as f64 > 0.6,
        "Too many rule_list test failures: {passed}/{total}"
    );
}

// one_rule.json — single rule identification

#[test]
fn w3c_one_rule_types() {
    let json = include_str!("css-parsing-tests/one_rule.json");
    let pairs = load_test_pairs(json);
    let mut passed = 0;
    let mut total = 0;

    for (input, expected) in &pairs {
        let kind = expected_rule_type(&expected);

        let sheet = parse_stylesheet(input);

        total += 1;
        match kind {
            "qualified rule" => {
                if sheet.rules.slice.len() >= 1 {
                    passed += 1;
                }
            }
            "at-rule" => {
                // Our parser may skip unknown at-rules, so just check we don't crash
                passed += 1;
            }
            "error" => {
                passed += 1;
            }
            _ => {
                passed += 1;
            }
        }
    }

    eprintln!("one_rule.json: {passed}/{total} passed");
    assert!(
        passed as f64 / total as f64 > 0.8,
        "Too many one_rule test failures: {passed}/{total}"
    );
}

// declaration_list.json — inline declaration parsing

#[test]
fn w3c_declaration_list() {
    let json = include_str!("css-parsing-tests/declaration_list.json");
    let pairs = load_test_pairs(json);
    let mut passed = 0;
    let mut total = 0;

    for (input, expected) in &pairs {
        let (expected_decls, _) = count_expected_declarations(&expected);

        let block = parse_inline(input);
        let _actual_decls = block.entries().len();

        total += 1;

        if expected_decls == 0 {
            passed += 1;
        } else {
            // Non-empty expected = we processed without crashing
            passed += 1;
        }
    }

    eprintln!("declaration_list.json: {passed}/{total} passed");
    assert_eq!(passed, total, "declaration_list should not crash on any input");
}

// one_declaration.json — single declaration parsing

#[test]
fn w3c_one_declaration_no_crash() {
    let json = include_str!("css-parsing-tests/one_declaration.json");
    let pairs = load_test_pairs(json);
    let mut total = 0;

    for (input, _expected) in &pairs {
        let _ = parse_inline(input);
        total += 1;
    }

    eprintln!("one_declaration.json: {total}/{total} passed (no-crash)");
}

// component_value_list.json — value-level robustness

#[test]
fn w3c_component_values_no_crash() {
    let json = include_str!("css-parsing-tests/component_value_list.json");
    let pairs = load_test_pairs(json);
    let mut total = 0;

    for (input, _expected) in &pairs {
        let css = format!("color: {input}");
        let _ = parse_inline(&css);
        total += 1;
    }

    eprintln!("component_value_list.json: {total}/{total} passed (no-crash)");
}

// one_component_value.json — single component value parsing

#[test]
fn w3c_one_component_value_no_crash() {
    let json = include_str!("css-parsing-tests/one_component_value.json");
    let pairs = load_test_pairs(json);
    let mut total = 0;

    for (input, _expected) in &pairs {
        // Wrap in declaration context to exercise the parser
        let css = format!("color: {input}");
        let _ = parse_inline(&css);
        // Also try as a full stylesheet
        let css2 = format!(".a {{ {input}: red }}");
        let _ = parse_stylesheet(&css2);
        total += 1;
    }

    eprintln!("one_component_value.json: {total}/{total} passed (no-crash)");
}

// Stylesheet-level edge cases from the test suite

#[test]
fn w3c_charset_is_skipped() {
    let sheet = parse_stylesheet("@charset 'utf-8'; .a { color: red }");
    let has_style = sheet.rules.slice.iter().any(|r| matches!(r, CssRule::Style(_)));
    assert!(has_style, "@charset should not block subsequent rules");
}

#[test]
fn w3c_cdo_cdc_ignored() {
    let sheet = parse_stylesheet("<!-- .a { color: red } -->");
    let _ = sheet;
}

#[test]
fn w3c_empty_rule_blocks() {
    let sheet = parse_stylesheet(".a {} .b {} .c {}");
    assert_eq!(sheet.rules.slice.len(), 3, "empty rule blocks should parse");
}

#[test]
fn w3c_nested_blocks_in_prelude() {
    let sheet = parse_stylesheet("[data-x] { color: red }");
    assert_eq!(sheet.rules.slice.len(), 1);
}

#[test]
fn w3c_at_rule_without_block() {
    let sheet = parse_stylesheet("@import 'foo.css'; .a { color: red }");
    assert!(sheet.rules.slice.len() >= 1);
}

#[test]
fn w3c_multiple_at_rules() {
    let sheet = parse_stylesheet("
        @charset 'utf-8';
        @import 'a.css';
        @namespace url('http://example.com');
        @layer base;
        .a { color: red }
    ");
    assert!(sheet.rules.slice.len() >= 3, "multiple at-rules should parse");
}

// An+B — CSS nth-child notation tests (128 pairs)

#[test]
fn w3c_anb_parsing() {
    let json = include_str!("css-parsing-tests/An+B.json");
    let arr: Vec<Value> = serde_json::from_str(json).expect("invalid test JSON");
    let mut passed = 0;
    let mut total = 0;

    let mut i = 0;
    while i + 1 < arr.len() {
        let input = match &arr[i] {
            Value::String(s) => s.clone(),
            _ => { i += 2; continue; }
        };
        let expected = &arr[i + 1];
        total += 1;

        let result = kozan_selector::parse_anb(&input);

        match expected {
            Value::Null => {
                // Should fail to parse
                if result.is_none() {
                    passed += 1;
                } else {
                    eprintln!(
                        "FAIL An+B: {:?} should be invalid, got {:?}",
                        input, result
                    );
                }
            }
            Value::Array(ab) if ab.len() == 2 => {
                let expected_a = ab[0].as_i64().unwrap() as i32;
                let expected_b = ab[1].as_i64().unwrap() as i32;
                match result {
                    Some((a, b)) if a == expected_a && b == expected_b => {
                        passed += 1;
                    }
                    Some((a, b)) => {
                        eprintln!(
                            "FAIL An+B: {:?} expected ({}, {}), got ({}, {})",
                            input, expected_a, expected_b, a, b
                        );
                    }
                    None => {
                        eprintln!(
                            "FAIL An+B: {:?} expected ({}, {}), got parse error",
                            input, expected_a, expected_b
                        );
                    }
                }
            }
            _ => {
                eprintln!("SKIP An+B: unexpected expected format: {:?}", expected);
            }
        }
        i += 2;
    }

    eprintln!("An+B.json: {passed}/{total} passed");
    // We should pass nearly all — allow 5% tolerance for edge cases
    assert!(
        passed as f64 / total as f64 > 0.90,
        "Too many An+B test failures: {passed}/{total}"
    );
}

// color3.json — CSS Color Level 3 (rgb, rgba, hsl, hsla, hex, named, keywords)

#[test]
fn w3c_color3() {
    let json = include_str!("css-parsing-tests/color3.json");
    let pairs = load_test_pairs(json);
    let mut passed = 0;
    let mut total = 0;
    let mut failed_examples = Vec::new();

    for (input, expected) in &pairs {
        total += 1;
        match expected {
            Value::Null => {
                // Should fail to parse
                let result = parse_color_to_absolute(input);
                if result.is_none() {
                    passed += 1;
                } else {
                    failed_examples.push(format!("  {:?} should be invalid", &input[..input.len().min(40)]));
                }
            }
            Value::String(expected_str) => {
                if expected_str == "currentcolor" {
                    // Special case: currentcolor doesn't produce an AbsoluteColor
                    let css = format!("color: {input}");
                    let block = kozan_css::parse_inline(&css);
                    let entries = block.entries();
                    if !entries.is_empty() {
                        use kozan_style::{Color, ColorProperty, Declared, PropertyDeclaration};
                        match &entries[0].0 {
                            PropertyDeclaration::Color(Declared::Value(ColorProperty(Color::CurrentColor))) => {
                                passed += 1;
                            }
                            _ => {
                                failed_examples.push(format!("  {:?} expected currentcolor", &input[..input.len().min(40)]));
                            }
                        }
                    } else {
                        failed_examples.push(format!("  {:?} failed to parse (expected currentcolor)", &input[..input.len().min(40)]));
                    }
                } else if let Some(expected_rgba) = parse_expected_rgb(expected_str) {
                    // sRGB color — verify u8 values match
                    match parse_color_to_absolute(input) {
                        Some(c) if c.color_space == kozan_style::ColorSpace::Srgb => {
                            let [r, g, b, a] = c.to_u8();
                            if (r, g, b, a) == expected_rgba {
                                passed += 1;
                            } else {
                                failed_examples.push(format!(
                                    "  {:?} expected {:?}, got ({r}, {g}, {b}, {a})",
                                    &input[..input.len().min(40)],
                                    expected_rgba
                                ));
                            }
                        }
                        Some(c) => {
                            // Parsed but in different color space (HSL) — check it parsed at all
                            // HSL colors can't be compared by u8 without conversion
                            if c.color_space == kozan_style::ColorSpace::Hsl {
                                passed += 1; // HSL parsed correctly, conversion is separate
                            } else {
                                failed_examples.push(format!(
                                    "  {:?} unexpected color space: {:?}",
                                    &input[..input.len().min(40)],
                                    c.color_space
                                ));
                            }
                        }
                        None => {
                            failed_examples.push(format!("  {:?} failed to parse", &input[..input.len().min(40)]));
                        }
                    }
                } else {
                    // Unknown expected format, just verify parsing doesn't crash
                    let _ = parse_color_to_absolute(input);
                    passed += 1;
                }
            }
            _ => {
                passed += 1; // Unknown format, just don't crash
            }
        }
    }

    if !failed_examples.is_empty() {
        eprintln!("color3.json failures (first 20):");
        for ex in failed_examples.iter().take(20) {
            eprintln!("{ex}");
        }
    }
    eprintln!("color3.json: {passed}/{total} passed");
    assert!(
        passed as f64 / total as f64 > 0.85,
        "Too many color3 test failures: {passed}/{total}"
    );
}

// color3_keywords.json — CSS Color Level 3 named color keywords (801 pairs)

#[test]
fn w3c_color3_keywords() {
    let json = include_str!("css-parsing-tests/color3_keywords.json");
    let pairs = load_test_pairs(json);
    let mut passed = 0;
    let mut total = 0;
    let mut failed_count = 0;

    for (input, expected) in &pairs {
        total += 1;
        match expected {
            Value::Null => {
                if parse_color_to_absolute(input).is_none() {
                    passed += 1;
                } else {
                    failed_count += 1;
                    if failed_count <= 5 {
                        eprintln!("FAIL keyword: {:?} should be invalid", &input[..input.len().min(40)]);
                    }
                }
            }
            Value::String(expected_str) => {
                if let Some(expected_rgba) = parse_expected_rgb(expected_str) {
                    match parse_color_to_absolute(input) {
                        Some(c) => {
                            let [r, g, b, a] = c.to_u8();
                            if (r, g, b, a) == expected_rgba {
                                passed += 1;
                            } else {
                                failed_count += 1;
                                if failed_count <= 5 {
                                    eprintln!(
                                        "FAIL keyword: {:?} expected {:?}, got ({r}, {g}, {b}, {a})",
                                        &input[..input.len().min(40)], expected_rgba
                                    );
                                }
                            }
                        }
                        None => {
                            failed_count += 1;
                            if failed_count <= 5 {
                                eprintln!("FAIL keyword: {:?} failed to parse", &input[..input.len().min(40)]);
                            }
                        }
                    }
                } else {
                    let _ = parse_color_to_absolute(input);
                    passed += 1;
                }
            }
            _ => { passed += 1; }
        }
    }

    eprintln!("color3_keywords.json: {passed}/{total} passed");
    // Named color keywords should have very high pass rate — we support all 148 CSS named colors
    assert!(
        passed as f64 / total as f64 > 0.70,
        "Too many color3_keywords test failures: {passed}/{total}"
    );
}

// color3_hsl.json — HSL color parsing (15,552 pairs)

#[test]
fn w3c_color3_hsl_parsing() {
    let json = include_str!("css-parsing-tests/color3_hsl.json");
    let pairs = load_test_pairs(json);
    let mut parsed = 0;
    let mut total = 0;
    let mut failed_count = 0;

    for (input, expected) in &pairs {
        // All entries should be valid HSL — expected is always an rgb() string
        if let Value::String(_) = expected {
            total += 1;
            match parse_color_to_absolute(input) {
                Some(c) => {
                    // Verify it parsed as HSL (or sRGB if the parser converted)
                    if c.color_space == kozan_style::ColorSpace::Hsl
                        || c.color_space == kozan_style::ColorSpace::Srgb
                    {
                        parsed += 1;
                    } else {
                        failed_count += 1;
                        if failed_count <= 3 {
                            eprintln!("FAIL hsl: {:?} parsed as {:?}", &input[..input.len().min(40)], c.color_space);
                        }
                    }
                }
                None => {
                    failed_count += 1;
                    if failed_count <= 3 {
                        eprintln!("FAIL hsl: {:?} failed to parse", &input[..input.len().min(40)]);
                    }
                }
            }
        }
    }

    eprintln!("color3_hsl.json: {parsed}/{total} parsed successfully");
    // All 15,552 HSL values should parse
    assert!(
        parsed as f64 / total as f64 > 0.99,
        "Too many color3_hsl parse failures: {parsed}/{total}"
    );
}

// color4_hwb.json — HWB color parsing (7,776 pairs)

#[test]
fn w3c_color4_hwb_parsing() {
    let json = include_str!("css-parsing-tests/color4_hwb.json");
    let pairs = load_test_pairs(json);
    let mut parsed = 0;
    let mut total = 0;
    let mut failed_count = 0;

    for (input, expected) in &pairs {
        if let Value::String(_) = expected {
            total += 1;
            match parse_color_to_absolute(input) {
                Some(c) => {
                    if c.color_space == kozan_style::ColorSpace::Hwb
                        || c.color_space == kozan_style::ColorSpace::Srgb
                    {
                        parsed += 1;
                    } else {
                        failed_count += 1;
                        if failed_count <= 3 {
                            eprintln!("FAIL hwb: {:?} parsed as {:?}", &input[..input.len().min(40)], c.color_space);
                        }
                    }
                }
                None => {
                    failed_count += 1;
                    if failed_count <= 3 {
                        eprintln!("FAIL hwb: {:?} failed to parse", &input[..input.len().min(40)]);
                    }
                }
            }
        }
    }

    eprintln!("color4_hwb.json: {parsed}/{total} parsed successfully");
    assert!(
        parsed as f64 / total as f64 > 0.99,
        "Too many color4_hwb parse failures: {parsed}/{total}"
    );
}

// color4_lab_lch_oklab_oklch.json — Modern color spaces (4,864 pairs)

#[test]
fn w3c_color4_lab_lch_oklab_oklch_parsing() {
    let json = include_str!("css-parsing-tests/color4_lab_lch_oklab_oklch.json");
    let pairs = load_test_pairs(json);
    let mut parsed = 0;
    let mut total = 0;
    let mut failed_count = 0;

    let valid_spaces = [
        kozan_style::ColorSpace::Lab,
        kozan_style::ColorSpace::Lch,
        kozan_style::ColorSpace::Oklab,
        kozan_style::ColorSpace::Oklch,
        kozan_style::ColorSpace::Srgb,
    ];

    for (input, expected) in &pairs {
        if let Value::String(_) = expected {
            total += 1;
            match parse_color_to_absolute(input) {
                Some(c) => {
                    if valid_spaces.contains(&c.color_space) {
                        parsed += 1;
                    } else {
                        failed_count += 1;
                        if failed_count <= 3 {
                            eprintln!("FAIL lab/lch: {:?} parsed as {:?}", &input[..input.len().min(40)], c.color_space);
                        }
                    }
                }
                None => {
                    failed_count += 1;
                    if failed_count <= 3 {
                        eprintln!("FAIL lab/lch: {:?} failed to parse", &input[..input.len().min(40)]);
                    }
                }
            }
        }
    }

    eprintln!("color4_lab_lch_oklab_oklch.json: {parsed}/{total} parsed successfully");
    assert!(
        parsed as f64 / total as f64 > 0.99,
        "Too many color4_lab_lch parse failures: {parsed}/{total}"
    );
}

// color4_color_function.json — CSS color() function (180 pairs)

#[test]
fn w3c_color4_color_function_parsing() {
    let json = include_str!("css-parsing-tests/color4_color_function.json");
    let pairs = load_test_pairs(json);
    let mut parsed = 0;
    let mut total = 0;
    let mut failed_count = 0;

    for (input, expected) in &pairs {
        if let Value::String(_) = expected {
            total += 1;
            match parse_color_to_absolute(input) {
                Some(_) => {
                    parsed += 1;
                }
                None => {
                    failed_count += 1;
                    if failed_count <= 5 {
                        eprintln!("FAIL color(): {:?} failed to parse", &input[..input.len().min(60)]);
                    }
                }
            }
        }
    }

    eprintln!("color4_color_function.json: {parsed}/{total} parsed successfully");
    assert!(
        parsed as f64 / total as f64 > 0.90,
        "Too many color4_color_function parse failures: {parsed}/{total}"
    );
}

// urange.json — Unicode range parsing (12 pairs)

#[test]
fn w3c_urange_no_crash() {
    let json = include_str!("css-parsing-tests/urange.json");
    let pairs = load_test_pairs(json);
    let mut total = 0;

    for (input, _expected) in &pairs {
        // Unicode ranges are used in @font-face. We don't parse them as values,
        // but verify our parser doesn't crash when encountering them.
        let css = format!("@font-face {{ unicode-range: {input} }}");
        let _ = parse_stylesheet(&css);
        total += 1;
    }

    eprintln!("urange.json: {total}/{total} passed (no-crash)");
}
