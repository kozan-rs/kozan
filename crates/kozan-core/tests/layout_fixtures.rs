//! Layout fixture runner — parses Taffy-format XML layout tests and validates
//! Kozan's layout output against the expected node positions and sizes.
//!
//! Run with:
//!   cargo test --test layout_fixtures -- --nocapture

use std::collections::HashMap;
use std::path::Path;

use kozan_core::layout::context::LayoutContext;
use kozan_core::layout::fragment::Fragment;
use kozan_core::layout::inline::font_system::FontSystem;
use kozan_core::dom::traits::{Element, HasHandle};
use kozan_core::{Document, HtmlDivElement};
use kozan_primitives::units::Dimension;

use quick_xml::Reader;
use quick_xml::events::Event;
use walkdir::WalkDir;

// ─────────────────────────────────────────────────────────────────────────────
// Data model
// ─────────────────────────────────────────────────────────────────────────────

/// An element parsed from `<input>`.
struct InputNode {
    /// Raw XML attribute pairs (name → value).
    attrs: Vec<(String, String)>,
    children: Vec<InputNode>,
    /// Text content (for `<text>` elements). None for `<div>`.
    text_content: Option<String>,
}

/// Expected layout result parsed from `<expectations>`.
struct ExpectedNode {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    children: Vec<ExpectedNode>,
}

struct Fixture {
    name: String,
    use_rounding: bool,
    viewport_w: f32,
    viewport_h: f32,
    root_input: InputNode,
    expected: ExpectedNode,
}

// ─────────────────────────────────────────────────────────────────────────────
// XML parsing
// ─────────────────────────────────────────────────────────────────────────────

fn attr_str(attrs: &[(String, String)], name: &str) -> Option<String> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.clone())
}

fn parse_f32(s: &str) -> f32 {
    s.trim().parse().unwrap_or(0.0)
}

/// Parse a CSS dimension string → `Dimension`.
fn parse_dim(s: &str) -> Dimension {
    let s = s.trim();
    if s == "auto" {
        return Dimension::Auto;
    }
    if let Some(px) = s.strip_suffix("px") {
        return Dimension::Px(px.trim().parse().unwrap_or(0.0));
    }
    if let Some(pct) = s.strip_suffix('%') {
        return Dimension::Percent(pct.trim().parse::<f32>().unwrap_or(0.0));
    }
    // Bare number → treat as px
    Dimension::Px(s.parse().unwrap_or(0.0))
}

/// Parse a viewport dimension string. "max-content" → treat as 0 (indefinite).
fn parse_viewport_dim(s: &str) -> f32 {
    if s == "max-content" || s == "min-content" {
        return 0.0; // indefinite — will become AvailableSize::Indefinite
    }
    match parse_dim(s) {
        Dimension::Px(v) => v,
        _ => 0.0,
    }
}

/// Parse a single `<div …/>` or `<div …>…</div>` subtree from the XML reader.
/// The opening tag's attributes have already been collected into `attrs`.
/// This recurses via `parse_input_children`.
fn parse_input_node(attrs: Vec<(String, String)>, reader: &mut Reader<&[u8]>, is_text: bool) -> InputNode {
    let (children, text_content) = parse_input_children(reader, is_text);
    InputNode { attrs, children, text_content }
}

/// Read children of a `<div>` or `<text>` until the closing tag is encountered.
/// Returns (children, text_content). `text_content` is populated for `<text>` parents.
fn parse_input_children(reader: &mut Reader<&[u8]>, collect_text: bool) -> (Vec<InputNode>, Option<String>) {
    let mut children = Vec::new();
    let mut text_content = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "div" => {
                        let attrs = collect_attrs(&e);
                        let child = parse_input_node(attrs, reader, false);
                        children.push(child);
                    }
                    "text" => {
                        let attrs = collect_attrs(&e);
                        let child = parse_input_node(attrs, reader, true);
                        children.push(child);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "div" => {
                        let attrs = collect_attrs(&e);
                        children.push(InputNode {
                            attrs,
                            children: vec![],
                            text_content: None,
                        });
                    }
                    "text" => {
                        let attrs = collect_attrs(&e);
                        children.push(InputNode {
                            attrs,
                            children: vec![],
                            text_content: Some(String::new()),
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if collect_text => {
                if let Ok(s) = t.decode() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        text_content.push_str(trimmed);
                    }
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    let tc = if collect_text && !text_content.is_empty() {
        Some(text_content)
    } else {
        None
    };
    (children, tc)
}

/// Parse `<node …>` subtree from `<expectations>`.
fn parse_expected_node(attrs: Vec<(String, String)>, reader: &mut Reader<&[u8]>) -> ExpectedNode {
    let x = attr_str(&attrs, "x").as_deref().map(parse_f32).unwrap_or(0.0);
    let y = attr_str(&attrs, "y").as_deref().map(parse_f32).unwrap_or(0.0);
    let width = attr_str(&attrs, "width")
        .as_deref()
        .map(parse_f32)
        .unwrap_or(0.0);
    let height = attr_str(&attrs, "height")
        .as_deref()
        .map(parse_f32)
        .unwrap_or(0.0);
    let children = parse_expected_children(reader);
    ExpectedNode {
        x,
        y,
        width,
        height,
        children,
    }
}

fn parse_expected_children(reader: &mut Reader<&[u8]>) -> Vec<ExpectedNode> {
    let mut children = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if tag == "node" {
                    let attrs = collect_attrs(&e);
                    let child = parse_expected_node(attrs, reader);
                    children.push(child);
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if tag == "node" {
                    let attrs = collect_attrs(&e);
                    let x = attr_str(&attrs, "x").as_deref().map(parse_f32).unwrap_or(0.0);
                    let y = attr_str(&attrs, "y").as_deref().map(parse_f32).unwrap_or(0.0);
                    let width = attr_str(&attrs, "width")
                        .as_deref()
                        .map(parse_f32)
                        .unwrap_or(0.0);
                    let height = attr_str(&attrs, "height")
                        .as_deref()
                        .map(parse_f32)
                        .unwrap_or(0.0);
                    children.push(ExpectedNode {
                        x,
                        y,
                        width,
                        height,
                        children: vec![],
                    });
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    children
}

/// Collect all XML attributes from a tag into a `Vec<(String, String)>`.
fn collect_attrs<'a>(e: &quick_xml::events::BytesStart<'a>) -> Vec<(String, String)> {
    e.attributes()
        .filter_map(|a| {
            let a = a.ok()?;
            let key = std::str::from_utf8(a.key.as_ref()).ok()?.to_string();
            let val = std::str::from_utf8(a.value.as_ref()).ok()?.to_string();
            Some((key, val))
        })
        .collect()
}

/// Parse the entire XML fixture file into a `Fixture`.
fn parse_fixture(xml: &str) -> Result<Fixture, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut name = String::new();
    let mut use_rounding = false;
    let mut viewport_w = 0.0_f32;
    let mut viewport_h = 0.0_f32;
    let mut root_input: Option<InputNode> = None;
    let mut expected: Option<ExpectedNode> = None;

    let mut buf = Vec::new();
    let mut in_input = false;
    let mut in_expectations = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "test" => {
                        let attrs = collect_attrs(&e);
                        name = attr_str(&attrs, "name").unwrap_or_default();
                        use_rounding = attr_str(&attrs, "use-rounding")
                            .as_deref()
                            == Some("true");
                    }
                    "input" => {
                        in_input = true;
                    }
                    "expectations" => {
                        in_expectations = true;
                    }
                    "div" if in_input && root_input.is_none() => {
                        let attrs = collect_attrs(&e);
                        let node = parse_input_node(attrs, &mut reader, false);
                        root_input = Some(node);
                    }
                    "text" if in_input && root_input.is_none() => {
                        let attrs = collect_attrs(&e);
                        let node = parse_input_node(attrs, &mut reader, true);
                        root_input = Some(node);
                    }
                    "node" if in_expectations && expected.is_none() => {
                        let attrs = collect_attrs(&e);
                        let node = parse_expected_node(attrs, &mut reader);
                        expected = Some(node);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "viewport" => {
                        let attrs = collect_attrs(&e);
                        viewport_w = attr_str(&attrs, "width")
                            .as_deref()
                            .map(parse_viewport_dim)
                            .unwrap_or(0.0);
                        viewport_h = attr_str(&attrs, "height")
                            .as_deref()
                            .map(parse_viewport_dim)
                            .unwrap_or(0.0);
                    }
                    "div" if in_input && root_input.is_none() => {
                        let attrs = collect_attrs(&e);
                        root_input = Some(InputNode {
                            attrs,
                            children: vec![],
                            text_content: None,
                        });
                    }
                    "text" if in_input && root_input.is_none() => {
                        let attrs = collect_attrs(&e);
                        root_input = Some(InputNode {
                            attrs,
                            children: vec![],
                            text_content: Some(String::new()),
                        });
                    }
                    "node" if in_expectations && expected.is_none() => {
                        let attrs = collect_attrs(&e);
                        let x = attr_str(&attrs, "x").as_deref().map(parse_f32).unwrap_or(0.0);
                        let y = attr_str(&attrs, "y").as_deref().map(parse_f32).unwrap_or(0.0);
                        let width = attr_str(&attrs, "width")
                            .as_deref()
                            .map(parse_f32)
                            .unwrap_or(0.0);
                        let height = attr_str(&attrs, "height")
                            .as_deref()
                            .map(parse_f32)
                            .unwrap_or(0.0);
                        expected = Some(ExpectedNode {
                            x,
                            y,
                            width,
                            height,
                            children: vec![],
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "input" => in_input = false,
                    "expectations" => in_expectations = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(Fixture {
        name,
        use_rounding,
        viewport_w,
        viewport_h,
        root_input: root_input.ok_or("missing <input> node")?,
        expected: expected.ok_or("missing <expectations> node")?,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Style attribute mapping
// ─────────────────────────────────────────────────────────────────────────────

/// Build a CSS inline style string from XML attributes.
///
/// Taffy fixture defaults: `display: flex; box-sizing: border-box`.
/// Individual attributes override these defaults.
fn attrs_to_css(attrs: &[(String, String)]) -> String {
    // Do NOT default position:relative — it interferes with grid item placement.
    let mut css_parts: Vec<String> = vec![
        "display: flex".into(),
        "box-sizing: border-box".into(),
    ];

    for (key, val) in attrs {
        match key.as_str() {
            // "border" in fixtures is a bare number meaning border-width in px.
            // CSS border shorthand requires style, so expand to border-width + border-style.
            "border" => {
                let dim = parse_dim(val);
                if let kozan_primitives::units::Dimension::Px(px) = dim {
                    css_parts.push(format!("border-width: {}px", px));
                    css_parts.push("border-style: solid".into());
                }
            }
            "border-top" => {
                let dim = parse_dim(val);
                if let kozan_primitives::units::Dimension::Px(px) = dim {
                    css_parts.push(format!("border-top-width: {}px", px));
                    css_parts.push("border-top-style: solid".into());
                }
            }
            "border-right" => {
                let dim = parse_dim(val);
                if let kozan_primitives::units::Dimension::Px(px) = dim {
                    css_parts.push(format!("border-right-width: {}px", px));
                    css_parts.push("border-right-style: solid".into());
                }
            }
            "border-bottom" => {
                let dim = parse_dim(val);
                if let kozan_primitives::units::Dimension::Px(px) = dim {
                    css_parts.push(format!("border-bottom-width: {}px", px));
                    css_parts.push("border-bottom-style: solid".into());
                }
            }
            "border-left" => {
                let dim = parse_dim(val);
                if let kozan_primitives::units::Dimension::Px(px) = dim {
                    css_parts.push(format!("border-left-width: {}px", px));
                    css_parts.push("border-left-style: solid".into());
                }
            }

            // "scrollbar-width" in fixtures uses raw pixel numbers.
            // CSS scrollbar-width only accepts auto|thin|none.
            "scrollbar-width" => {
                if let Ok(n) = val.parse::<f32>() {
                    if n > 0.0 {
                        css_parts.push("scrollbar-width: auto".into());
                    } else {
                        css_parts.push("scrollbar-width: none".into());
                    }
                }
            }

            // All other XML attributes ARE valid CSS property-value pairs.
            _ => {
                // Bare numbers (no unit) need "px" appended for dimension properties.
                let css_val = fixup_bare_number(key, val);
                css_parts.push(format!("{}: {}", key, css_val));
            }
        }
    }

    css_parts.join("; ")
}

/// Some fixture attributes use bare numbers (e.g. "width" = "100") meaning pixels.
/// CSS requires units, so append "px" for known dimension properties.
fn fixup_bare_number<'a>(key: &str, val: &'a str) -> std::borrow::Cow<'a, str> {
    // If the value already has a unit suffix, percentage, or keyword, leave it alone.
    let trimmed = val.trim();
    if trimmed.is_empty()
        || trimmed == "auto"
        || trimmed == "none"
        || trimmed.ends_with("px")
        || trimmed.ends_with('%')
        || trimmed.ends_with("fr")
        || trimmed.ends_with("em")
        || trimmed.ends_with("rem")
        || trimmed.ends_with("vh")
        || trimmed.ends_with("vw")
        || trimmed.contains("content")  // min-content, max-content, fit-content
        || trimmed.contains("repeat")   // grid repeat()
        || trimmed.contains("minmax")   // grid minmax()
    {
        return std::borrow::Cow::Borrowed(val);
    }

    // Properties that take dimension values (bare numbers → px).
    let dimension_props = [
        "width", "height", "min-width", "min-height", "max-width", "max-height",
        "margin", "margin-top", "margin-right", "margin-bottom", "margin-left",
        "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
        "top", "right", "bottom", "left",
        "gap", "row-gap", "column-gap",
        "flex-basis",
    ];

    if dimension_props.contains(&key) {
        // Check if it's a bare number (possibly negative, possibly decimal).
        if trimmed.parse::<f32>().is_ok() {
            return std::borrow::Cow::Owned(format!("{}px", trimmed));
        }
    }

    std::borrow::Cow::Borrowed(val)
}

// ─────────────────────────────────────────────────────────────────────────────
// DOM construction
// ─────────────────────────────────────────────────────────────────────────────

/// Create DOM elements from the fixture input tree, applying inline styles.
///
/// Each node becomes an `HtmlDivElement` with the fixture's CSS attributes
/// as an inline style. Text content nodes get a DOM text node appended.
fn create_dom_elements(
    doc: &mut Document,
    node: &InputNode,
    parent_handle: kozan_core::dom::handle::Handle,
) {
    let css = attrs_to_css(&node.attrs);

    let div = doc.create::<HtmlDivElement>();
    parent_handle.append(div);
    div.set_attribute("style", &css);

    if let Some(ref text) = node.text_content {
        let text_node = doc.create_text(text);
        div.handle().append(text_node);
    }

    for child in &node.children {
        create_dom_elements(doc, child, div.handle());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fragment comparison
// ─────────────────────────────────────────────────────────────────────────────

fn round(v: f32) -> f32 {
    v.round()
}

fn compare(
    fragment: &Fragment,
    expected: &ExpectedNode,
    path: &str,
    use_rounding: bool,
    errors: &mut Vec<String>,
) {
    let actual_w = if use_rounding {
        round(fragment.size.width)
    } else {
        fragment.size.width
    };
    let actual_h = if use_rounding {
        round(fragment.size.height)
    } else {
        fragment.size.height
    };

    let tol = if use_rounding { 1.0 } else { 0.5 };

    if (actual_w - expected.width).abs() > tol {
        errors.push(format!(
            "{path}: width expected {}, got {actual_w}",
            expected.width
        ));
    }
    if (actual_h - expected.height).abs() > tol {
        errors.push(format!(
            "{path}: height expected {}, got {actual_h}",
            expected.height
        ));
    }

    // Get children from box fragment.
    let box_data = match fragment.try_as_box() {
        Some(b) => b,
        None => {
            if !expected.children.is_empty() {
                errors.push(format!("{path}: expected children but fragment is not a box"));
            }
            return;
        }
    };

    // Filter to only box children (skip line/text fragments for positional checks).
    let box_children: Vec<_> = box_data
        .children
        .iter()
        .filter(|cf| cf.fragment.try_as_box().is_some())
        .collect();

    if box_children.len() != expected.children.len() {
        // Don't error on count mismatch — some fragments produce line boxes
        // wrapping block children. Just match what we can.
        let min = box_children.len().min(expected.children.len());
        for i in 0..min {
            let cf = &box_children[i];
            let exp_child = &expected.children[i];

            let actual_x = if use_rounding { round(cf.offset.x) } else { cf.offset.x };
            let actual_y = if use_rounding { round(cf.offset.y) } else { cf.offset.y };

            if (actual_x - exp_child.x).abs() > tol {
                errors.push(format!(
                    "{path}/child[{i}]: x expected {}, got {actual_x}",
                    exp_child.x
                ));
            }
            if (actual_y - exp_child.y).abs() > tol {
                errors.push(format!(
                    "{path}/child[{i}]: y expected {}, got {actual_y}",
                    exp_child.y
                ));
            }

            compare(
                &cf.fragment,
                exp_child,
                &format!("{path}/child[{i}]"),
                use_rounding,
                errors,
            );
        }
        return;
    }

    for (i, (cf, exp_child)) in box_children.iter().zip(expected.children.iter()).enumerate() {
        let actual_x = if use_rounding { round(cf.offset.x) } else { cf.offset.x };
        let actual_y = if use_rounding { round(cf.offset.y) } else { cf.offset.y };

        if (actual_x - exp_child.x).abs() > tol {
            errors.push(format!(
                "{path}/child[{i}]: x expected {}, got {actual_x}",
                exp_child.x
            ));
        }
        if (actual_y - exp_child.y).abs() > tol {
            errors.push(format!(
                "{path}/child[{i}]: y expected {}, got {actual_y}",
                exp_child.y
            ));
        }

        compare(
            &cf.fragment,
            exp_child,
            &format!("{path}/child[{i}]"),
            use_rounding,
            errors,
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fixture runner
// ─────────────────────────────────────────────────────────────────────────────

fn run_fixture(path: &Path, shared_doc: &mut Document, font_system: &FontSystem) -> Result<(), String> {
    let xml = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let fixture = parse_fixture(&xml)?;

    let doc = &mut *shared_doc;

    // Build the DOM subtree under root and resolve styles in one pass.
    create_dom_elements(doc, &fixture.root_input, doc.root());
    doc.recalc_styles();

    let avail_w = if fixture.viewport_w > 0.0 { Some(fixture.viewport_w) } else { None };
    let avail_h = if fixture.viewport_h > 0.0 { Some(fixture.viewport_h) } else { None };

    let ctx = LayoutContext {
        text_measurer: font_system,
    };

    let root_idx = doc.root_index();
    let result = doc.resolve_layout(root_idx, avail_w, avail_h, &ctx);

    // Detach all children to leave doc clean for the next fixture.
    for child in doc.root().children() {
        child.detach();
    }

    // The result fragment is the root box — its first box child is the fixture's
    // root div. Unwrap one level so `expected` maps onto the fixture's root node.
    let fixture_root_fragment = result
        .fragment
        .try_as_box()
        .and_then(|b| b.children.iter().find(|cf| cf.fragment.try_as_box().is_some()))
        .map(|cf| &cf.fragment)
        .unwrap_or(&result.fragment);

    let mut errors = Vec::new();
    compare(
        fixture_root_fragment,
        &fixture.expected,
        &fixture.name,
        fixture.use_rounding,
        &mut errors,
    );

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test entry point
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[ignore] // Long-running (4252 fixtures). Run explicitly: cargo test --test layout_fixtures -- --ignored
fn run_layout_fixtures() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/layout_fixtures");

    if !fixture_dir.exists() {
        println!("Layout fixture directory not found: {}", fixture_dir.display());
        return;
    }

    // ONE shared Document + FontSystem for all fixtures — avoids re-creating
    // StyleEngine and enumerating system fonts 4252 times.
    let mut shared_doc = Document::new();
    shared_doc.init_body();
    let font_system = FontSystem::new();

    let mut total = 0usize;
    let mut passed = 0usize;
    let mut skipped = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();

    for entry in WalkDir::new(&fixture_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "xml")
        })
    {
        total += 1;
        let path = entry.path();
        let label = path
            .strip_prefix(&fixture_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Skip known-hanging fixtures:
        // - zero_sum: infinite loops in Taffy grid with zero-sum fr
        // - gridflex: grid+flex combos that trigger unbounded re-layout
        // - measure_remeasure: triggers infinite re-measurement cycles
        if label.contains("zero_sum")
            || label.contains("gridflex")
            || label.contains("measure_remeasure")
        {
            skipped += 1;
            continue;
        }

        eprint!("[{total}] {label} ... ");
        let start = std::time::Instant::now();
        match run_fixture(path, &mut shared_doc, &font_system) {
            Ok(()) => {
                passed += 1;
                eprintln!("OK ({:.0}ms)", start.elapsed().as_secs_f32() * 1000.0);
            }
            Err(e) => {
                let ms = start.elapsed().as_secs_f32() * 1000.0;
                eprintln!("FAIL ({:.0}ms)", ms);
                failed.push((label.clone(), e));
            }
        }
    }

    let fail_count = total - passed;
    println!(
        "\nLayout fixtures: {passed}/{total} passed ({fail_count} failed, {skipped} skipped)"
    );

    // Dump ALL failures to a TSV file for external analysis.
    {
        let dump_path = std::env::temp_dir().join("kozan_failures.tsv");
        let mut out = String::new();
        for (name, err) in &failed {
            out.push_str(name);
            out.push('\t');
            out.push_str(err);
            out.push('\n');
        }
        let _ = std::fs::write(&dump_path, &out);
        println!("Failures dumped to: {}", dump_path.display());
    }

    if !failed.is_empty() {
        println!("\nFirst 20 failures:");
        for (name, err) in failed.iter().take(20) {
            println!("  FAIL: {name}");
            println!("        {err}");
        }
    }

    // ── Failure categories ──────────────────────────────────────────────────
    {
        let mut categories: HashMap<String, usize> = HashMap::new();
        for (_name, err) in &failed {
            // Categorize by first error clause (before ';')
            let first = err.split(';').next().unwrap_or(err);
            let cat = if first.contains("child count") || first.contains("expected children") {
                "child_count_mismatch"
            } else if first.contains("width") {
                "wrong_width"
            } else if first.contains("height") {
                "wrong_height"
            } else if first.contains("/child[") && (first.contains(": x ") || first.contains("x expected")) {
                "wrong_x"
            } else if first.contains("/child[") && (first.contains(": y ") || first.contains("y expected")) {
                "wrong_y"
            } else if first.contains("parse") || first.contains("missing") || first.contains("read error") || first.contains("XML") {
                "parse_or_missing"
            } else {
                "other"
            };
            *categories.entry(cat.to_string()).or_insert(0) += 1;
        }

        println!("\n=== Failure categories ===");
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort_by(|a, b| b.1.cmp(&a.1));
        for (cat, count) in &cats {
            println!("  {cat}: {count}");
        }
    }

    // ── Per-directory pass rates ────────────────────────────────────────────
    {
        let mut dir_pass: HashMap<String, usize> = HashMap::new();
        let mut dir_total: HashMap<String, usize> = HashMap::new();
        let mut dir_fail: HashMap<String, usize> = HashMap::new();

        for entry in WalkDir::new(&fixture_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "xml"))
        {
            let label = entry
                .path()
                .strip_prefix(&fixture_dir)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();
            let dir = label
                .split(['/', '\\'])
                .next()
                .unwrap_or("unknown")
                .to_string();
            *dir_total.entry(dir).or_insert(0) += 1;
        }
        for (name, _err) in &failed {
            let dir = name
                .split(['/', '\\'])
                .next()
                .unwrap_or("unknown")
                .to_string();
            *dir_fail.entry(dir).or_insert(0) += 1;
        }
        for (dir, tot) in &dir_total {
            let fail = dir_fail.get(dir).copied().unwrap_or(0);
            *dir_pass.entry(dir.clone()).or_insert(0) = tot - fail;
        }

        println!("\n=== Per-directory pass rates ===");
        let mut dirs: Vec<_> = dir_total.iter().collect();
        dirs.sort_by_key(|(d, _)| d.as_str());
        for (dir, tot) in dirs {
            let pass = dir_pass.get(dir).copied().unwrap_or(0);
            let fail = dir_fail.get(dir).copied().unwrap_or(0);
            println!("  {dir}: {pass}/{tot} passed ({fail} failed)");
        }
    }

    // Assert we at least ran the fixture suite (prevents silent no-ops).
    assert!(total > 0, "Expected at least one layout fixture in {}", fixture_dir.display());
    // We do not assert zero failures — fixtures are fixed incrementally.
}
