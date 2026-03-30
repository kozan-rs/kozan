//! CSS Custom Properties — collection, cycle detection, and var()/env()/attr() substitution.
//!
//! Custom properties (`--name: value`) are resolved during the cascade BEFORE
//! any typed properties. The resolution pipeline:
//!
//! 1. **Collect**: Extract all `PropertyDeclaration::Custom` from winning declarations.
//! 2. **Inherit**: Custom properties inherit by default (CSS spec). Copy parent's
//!    resolved map, then overlay this element's declarations.
//! 3. **Resolve var() within custom properties**: Custom properties can reference
//!    other custom properties (`--a: var(--b)`). Build dependency graph, detect
//!    cycles via DFS coloring, resolve in topological order.
//! 4. **Substitute in typed properties**: For each `Declared::WithVariables`,
//!    replace `var()` / `env()` / `attr()` and return substituted CSS text
//!    for re-parsing by the property parser.
//!
//! # Cycle Detection
//!
//! `--a: var(--b); --b: var(--a)` creates a cycle. Per W3C CSS Custom Properties
//! Level 1 §3, all properties in a cycle become "invalid at computed-value time"
//! and fall back to their inherited or initial value.
//!
//! We use DFS 3-coloring (White/Gray/Black) for O(V+E) cycle detection.
//!
//! # Substitution Limits (W3C §3)
//!
//! - Max substitution depth: 1024 var() references
//! - Max result size: 1MB after substitution
//! - Exceeding either → invalid at computed-value time

use kozan_atom::Atom;
use kozan_selector::fxhash::FxHashMap;
use smallvec::SmallVec;

use crate::device::Device;

/// Maximum var() substitution depth to prevent infinite recursion.
const MAX_SUBSTITUTION_DEPTH: u32 = 1024;

/// Maximum result size in bytes after substitution (1 MB).
const MAX_RESULT_SIZE: usize = 1_048_576;

/// Resolved custom property map for one element.
///
/// Contains all custom properties that apply to this element after cascade,
/// inheritance, and var()-within-custom-property resolution.
#[derive(Clone, Debug, Default)]
pub struct CustomPropertyMap {
    map: FxHashMap<Atom, Atom>,
}

impl CustomPropertyMap {
    pub fn new() -> Self {
        Self { map: FxHashMap::default() }
    }

    /// Create a map pre-populated with the parent's resolved properties.
    /// Custom properties inherit by default (W3C CSS Custom Properties §2).
    pub fn inherit(parent: &CustomPropertyMap) -> Self {
        Self { map: parent.map.clone() }
    }

    /// Get a resolved custom property value by name (without `--` prefix).
    #[inline]
    pub fn get(&self, name: &Atom) -> Option<&Atom> {
        self.map.get(name)
    }

    /// Get a resolved custom property value by string name (without `--` prefix).
    #[inline]
    pub fn get_str(&self, name: &str) -> Option<&Atom> {
        let atom = Atom::from(name);
        self.map.get(&atom)
    }

    /// Number of resolved properties.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over all resolved (name, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&Atom, &Atom)> {
        self.map.iter()
    }

    /// Insert or update a resolved value.
    pub fn insert(&mut self, name: Atom, value: Atom) {
        self.map.insert(name, value);
    }

    /// Remove a property (used when resolving to invalid).
    pub fn remove(&mut self, name: &Atom) {
        self.map.remove(name);
    }
}

/// A pending (unresolved) custom property declaration.
struct PendingCustomProperty {
    name: Atom,
    value: Atom,
    /// Custom property names referenced by var() in this value.
    dependencies: SmallVec<[Atom; 4]>,
}

/// DFS coloring for cycle detection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    /// Not yet visited.
    White,
    /// Currently on the DFS stack (back-edge = cycle).
    Gray,
    /// Fully processed, no cycle through this node.
    Black,
}

/// Collect custom property declarations and resolve them, including:
/// - Inheritance from parent
/// - var() references between custom properties
/// - Cycle detection
/// - @property syntax validation (CSS Properties & Values API Level 1 §7)
///
/// `registered` maps bare custom property names (without `--`) to their
/// `@property` rules. After resolving each property, if a matching rule
/// exists its `syntax` is checked; invalid values fall back to the rule's
/// `initial_value` (or are removed if there is none).
///
/// Returns the fully resolved `CustomPropertyMap` for this element.
pub fn resolve_custom_properties(
    declarations: &[(Atom, Atom)],
    parent: Option<&CustomPropertyMap>,
    registered: &FxHashMap<Atom, kozan_css::PropertyRule>,
) -> CustomPropertyMap {
    // Start with inherited properties.
    let mut resolved = match parent {
        Some(p) => CustomPropertyMap::inherit(p),
        None => CustomPropertyMap::new(),
    };

    if declarations.is_empty() {
        return resolved;
    }

    // Parse var() dependencies in each custom property value.
    let mut pending: Vec<PendingCustomProperty> = Vec::with_capacity(declarations.len());
    let mut name_to_idx: FxHashMap<Atom, usize> = FxHashMap::default();

    for (name, value) in declarations {
        let deps = extract_var_references(value.as_ref());
        let idx = pending.len();
        name_to_idx.insert(name.clone(), idx);
        pending.push(PendingCustomProperty {
            name: name.clone(),
            value: value.clone(),
            dependencies: deps,
        });
    }

    // DFS cycle detection + topological resolution.
    let mut colors: Vec<Color> = vec![Color::White; pending.len()];
    let mut order: Vec<usize> = Vec::with_capacity(pending.len());
    let mut cycle_members: Vec<bool> = vec![false; pending.len()];

    for i in 0..pending.len() {
        if colors[i] == Color::White {
            dfs_visit(
                i,
                &pending,
                &name_to_idx,
                &mut colors,
                &mut order,
                &mut cycle_members,
            );
        }
    }

    // Resolve in topological order (dependencies first).
    for &idx in &order {
        if cycle_members[idx] {
            // Cycle detected — property is invalid at computed-value time.
            // Remove from resolved map (don't inherit a cyclic value).
            resolved.remove(&pending[idx].name);
            continue;
        }

        let prop = &pending[idx];
        let value_str = prop.value.as_ref();

        // If value contains var(), substitute from already-resolved properties.
        if contains_var(value_str) {
            match substitute_vars_in_string(value_str, &resolved, 0) {
                Some(substituted) => {
                    resolved.insert(prop.name.clone(), Atom::from(substituted.as_str()));
                }
                None => {
                    // Substitution failed (missing var, depth exceeded, etc.)
                    resolved.remove(&prop.name);
                }
            }
        } else {
            // No var() — use value directly.
            resolved.insert(prop.name.clone(), prop.value.clone());
        }

        // @property syntax validation — CSS Properties & Values API Level 1 §7.
        // If the resolved value doesn't satisfy the registered syntax, replace
        // it with the rule's initial-value (or remove it entirely).
        if let Some(rule) = registered.get(&prop.name) {
            let resolved_str = resolved.get(&prop.name).map(|a| a.as_ref()).unwrap_or("");
            if !rule.syntax.validate(resolved_str) {
                match &rule.initial_value {
                    Some(initial) => { resolved.insert(prop.name.clone(), initial.clone()); }
                    None => { resolved.remove(&prop.name); }
                }
            }
        }
    }

    resolved
}

/// DFS visit for cycle detection. Marks cycle members.
fn dfs_visit(
    idx: usize,
    pending: &[PendingCustomProperty],
    name_to_idx: &FxHashMap<Atom, usize>,
    colors: &mut [Color],
    order: &mut Vec<usize>,
    cycle_members: &mut [bool],
) -> bool {
    colors[idx] = Color::Gray;

    for dep_name in &pending[idx].dependencies {
        if let Some(&dep_idx) = name_to_idx.get(dep_name) {
            match colors[dep_idx] {
                Color::Gray => {
                    // Back-edge: cycle detected. Mark both nodes.
                    cycle_members[idx] = true;
                    cycle_members[dep_idx] = true;
                    return true;
                }
                Color::White => {
                    if dfs_visit(dep_idx, pending, name_to_idx, colors, order, cycle_members) {
                        // Propagate cycle marking up the stack.
                        if colors[idx] == Color::Gray {
                            cycle_members[idx] = true;
                        }
                    }
                }
                Color::Black => {
                    // Already resolved, no cycle through this path.
                    if cycle_members[dep_idx] {
                        cycle_members[idx] = true;
                    }
                }
            }
        }
        // If dep_name not in pending, it might be in parent's resolved map —
        // that's fine, it's already resolved and no cycle possible.
    }

    colors[idx] = Color::Black;
    order.push(idx);
    false
}

/// Extract custom property names referenced by `var()` in a CSS value string.
///
/// Scans for `var(--name` patterns. Does NOT use full CSS tokenization — this
/// is intentionally fast and simple since custom property values are free-form
/// text (they don't follow normal CSS grammar).
fn extract_var_references(css: &str) -> SmallVec<[Atom; 4]> {
    let mut refs = SmallVec::new();
    let bytes = css.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 5 < len {
        // Look for "var(" (case-insensitive per W3C)
        if bytes[i].to_ascii_lowercase() == b'v'
            && bytes[i + 1].to_ascii_lowercase() == b'a'
            && bytes[i + 2].to_ascii_lowercase() == b'r'
            && bytes[i + 3] == b'('
        {
            i += 4;
            // Skip whitespace
            while i < len && bytes[i] == b' ' {
                i += 1;
            }
            // Expect "--"
            if i + 2 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
                let start = i + 2; // After "--"
                i = start;
                // Custom property name: everything until ')' or ',' or whitespace
                while i < len && bytes[i] != b')' && bytes[i] != b',' && bytes[i] != b' ' {
                    i += 1;
                }
                if i > start {
                    let name = &css[start..i];
                    refs.push(Atom::from(name));
                }
            }
        } else {
            i += 1;
        }
    }

    refs
}

/// Quick check if a string contains `var(`.
#[inline]
fn contains_var(css: &str) -> bool {
    css.contains("var(")
}

/// Substitute `var()` references in a CSS string using resolved custom properties.
///
/// Returns `None` if substitution fails (missing property without fallback,
/// depth exceeded, result too large).
fn substitute_vars_in_string(
    css: &str,
    resolved: &CustomPropertyMap,
    depth: u32,
) -> Option<String> {
    if depth > MAX_SUBSTITUTION_DEPTH {
        return None;
    }

    if !contains_var(css) {
        return Some(css.to_string());
    }

    let mut result = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut literal_start = 0; // Track start of contiguous non-var text

    while i < len {
        // Look for "var(" (case-insensitive)
        if i + 4 <= len
            && bytes[i].to_ascii_lowercase() == b'v'
            && bytes[i + 1].to_ascii_lowercase() == b'a'
            && bytes[i + 2].to_ascii_lowercase() == b'r'
            && bytes[i + 3] == b'('
        {
            // Flush accumulated literal text
            if literal_start < i {
                result.push_str(&css[literal_start..i]);
            }

            i += 4;

            // Skip whitespace
            while i < len && bytes[i] == b' ' {
                i += 1;
            }

            // Parse custom property name
            if i + 2 <= len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
                let name_start = i + 2;
                i = name_start;

                // Name ends at ')', ',' or whitespace before those
                while i < len && bytes[i] != b')' && bytes[i] != b',' {
                    i += 1;
                }
                let name_end = i;
                // Trim trailing whitespace from name
                let name = css[name_start..name_end].trim_end();

                let mut fallback: Option<&str> = None;

                if i < len && bytes[i] == b',' {
                    // Parse fallback value
                    i += 1;
                    // Skip leading whitespace
                    while i < len && bytes[i] == b' ' {
                        i += 1;
                    }
                    let fb_start = i;
                    // Fallback extends to matching ')' — need to track nesting
                    let mut paren_depth = 1u32;
                    while i < len && paren_depth > 0 {
                        match bytes[i] {
                            b'(' => paren_depth += 1,
                            b')' => paren_depth -= 1,
                            _ => {}
                        }
                        if paren_depth > 0 {
                            i += 1;
                        }
                    }
                    fallback = Some(css[fb_start..i].trim_end());
                }

                // Skip closing ')'
                if i < len && bytes[i] == b')' {
                    i += 1;
                }

                literal_start = i; // Reset literal tracking after var()

                // Look up the property
                let name_atom = Atom::from(name);
                if let Some(value) = resolved.get(&name_atom) {
                    // Recursively substitute in case the value itself has var()
                    match substitute_vars_in_string(value.as_ref(), resolved, depth + 1) {
                        Some(substituted) => result.push_str(&substituted),
                        None => {
                            // Value substitution failed — try fallback
                            if let Some(fb) = fallback {
                                match substitute_vars_in_string(fb, resolved, depth + 1) {
                                    Some(fb_sub) => result.push_str(&fb_sub),
                                    None => return None,
                                }
                            } else {
                                return None;
                            }
                        }
                    }
                } else if let Some(fb) = fallback {
                    // Property not found — use fallback
                    match substitute_vars_in_string(fb, resolved, depth + 1) {
                        Some(fb_sub) => result.push_str(&fb_sub),
                        None => return None,
                    }
                } else {
                    // Property not found, no fallback — invalid
                    return None;
                }
            } else {
                // Malformed var() — literal_start already covers this range
                // no-op: literal_start stays, text will be flushed later
            }
        } else {
            i += 1;
        }
    }

    // Flush remaining literal text
    if literal_start < len {
        result.push_str(&css[literal_start..len]);
    }

    if result.len() > MAX_RESULT_SIZE {
        return None;
    }

    Some(result)
}

/// Environment variable values resolved from the device context.
///
/// These are the CSS `env()` values defined in CSS Environment Variables Module 1:
/// - `safe-area-inset-*` — notch/cutout safe areas (phones)
/// - `titlebar-area-*` — PWA title bar geometry
/// - `keyboard-inset-*` — virtual keyboard geometry
#[derive(Clone, Debug)]
pub struct EnvironmentValues {
    map: FxHashMap<Atom, Atom>,
}

impl EnvironmentValues {
    /// Create environment values from a Device.
    pub fn from_device(device: &Device) -> Self {
        let mut map = FxHashMap::default();

        // Helper: avoid format! allocation for the common case (0px on desktop).
        #[inline]
        fn px_atom(v: f32) -> Atom {
            if v == 0.0 { Atom::from("0px") } else { Atom::from(format!("{v}px").as_str()) }
        }

        // Safe area insets (default 0px on desktop, set by platform on mobile).
        map.insert(Atom::from("safe-area-inset-top"), px_atom(device.safe_area_inset_top));
        map.insert(Atom::from("safe-area-inset-right"), px_atom(device.safe_area_inset_right));
        map.insert(Atom::from("safe-area-inset-bottom"), px_atom(device.safe_area_inset_bottom));
        map.insert(Atom::from("safe-area-inset-left"), px_atom(device.safe_area_inset_left));

        Self { map }
    }

    /// Create with no environment values (all env() will use fallback).
    pub fn empty() -> Self {
        Self { map: FxHashMap::default() }
    }

    /// Look up an environment variable by name.
    pub fn get(&self, name: &str) -> Option<&Atom> {
        let atom = Atom::from(name);
        self.map.get(&atom)
    }

    /// Insert a custom environment value.
    pub fn insert(&mut self, name: &str, value: &str) {
        self.map.insert(Atom::from(name), Atom::from(value));
    }
}

/// Substitute all `var()`, `env()`, and `attr()` references in a CSS value string.
///
/// Writes the result into `out` — clears `out` before writing. The caller
/// should pass the same `out` buffer for every property in a cascade: the
/// buffer grows to the peak capacity and is then reused for free.
///
/// `scratch` is a second buffer used for intermediate phases (also reused).
/// Pass two `String::new()` values created once before the loop.
///
/// Returns `false` if substitution fails (missing property without fallback,
/// depth exceeded, result too large).
pub fn substitute_with_buf(
    css: &str,
    custom_props: &CustomPropertyMap,
    env_values: &EnvironmentValues,
    attr_lookup: impl Fn(&str) -> Option<String>,
    out: &mut String,
    scratch: &mut String,
) -> bool {
    out.clear();

    // After each phase, `out` holds the current text and `scratch` is free.
    // We swap to advance without extra allocations.

    // Phase 1: var()
    if css.contains("var(") {
        if substitute_vars_in_string(css, custom_props, 0)
            .map(|s| { *out = s; })
            .is_none()
        {
            return false;
        }
    } else {
        out.push_str(css);
    }

    // Phase 2: env()
    if out.contains("env(") {
        match substitute_env_in_string(out, env_values) {
            Some(s) => { *scratch = s; std::mem::swap(out, scratch); }
            None => return false,
        }
    }

    // Phase 3: attr()
    if out.contains("attr(") {
        match substitute_attr_in_string(out, &attr_lookup) {
            Some(s) => { *scratch = s; std::mem::swap(out, scratch); }
            None => return false,
        }
    }

    out.len() <= MAX_RESULT_SIZE
}

/// Substitute all `var()`, `env()`, and `attr()` references in a CSS value string.
///
/// This is the main entry point for typed property substitution. Call this on
/// `UnparsedValue.css` to get a resolved CSS string, then re-parse with the
/// property parser.
///
/// For hot loops over many properties, prefer [`substitute_with_buf`] to
/// avoid per-call buffer allocations.
///
/// `attr_lookup` is called for `attr(name)` — provides element attribute values.
/// Pass `|_| None` if attr() is not supported.
pub fn substitute(
    css: &str,
    custom_props: &CustomPropertyMap,
    env_values: &EnvironmentValues,
    attr_lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let mut out = String::new();
    let mut scratch = String::new();
    if substitute_with_buf(css, custom_props, env_values, attr_lookup, &mut out, &mut scratch) {
        Some(out)
    } else {
        None
    }
}

/// Substitute `env()` references in a CSS string.
fn substitute_env_in_string(css: &str, env_values: &EnvironmentValues) -> Option<String> {
    let mut result = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut literal_start = 0;

    while i < len {
        if i + 4 <= len
            && bytes[i].to_ascii_lowercase() == b'e'
            && bytes[i + 1].to_ascii_lowercase() == b'n'
            && bytes[i + 2].to_ascii_lowercase() == b'v'
            && bytes[i + 3] == b'('
        {
            // Flush literal text
            if literal_start < i {
                result.push_str(&css[literal_start..i]);
            }

            i += 4;
            while i < len && bytes[i] == b' ' { i += 1; }

            let name_start = i;
            while i < len && bytes[i] != b')' && bytes[i] != b',' { i += 1; }
            let name = css[name_start..i].trim_end();

            let mut fallback: Option<&str> = None;
            if i < len && bytes[i] == b',' {
                i += 1;
                while i < len && bytes[i] == b' ' { i += 1; }
                let fb_start = i;
                let mut paren_depth = 1u32;
                while i < len && paren_depth > 0 {
                    match bytes[i] {
                        b'(' => paren_depth += 1,
                        b')' => paren_depth -= 1,
                        _ => {}
                    }
                    if paren_depth > 0 { i += 1; }
                }
                fallback = Some(css[fb_start..i].trim_end());
            }

            if i < len && bytes[i] == b')' { i += 1; }
            literal_start = i;

            if let Some(value) = env_values.get(name) {
                result.push_str(value.as_ref());
            } else if let Some(fb) = fallback {
                result.push_str(fb);
            } else {
                return None;
            }
        } else {
            i += 1;
        }
    }

    if literal_start < len {
        result.push_str(&css[literal_start..len]);
    }

    Some(result)
}

/// Substitute `attr()` references in a CSS string.
fn substitute_attr_in_string(
    css: &str,
    attr_lookup: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let mut result = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut literal_start = 0;

    while i < len {
        if i + 5 <= len
            && bytes[i].to_ascii_lowercase() == b'a'
            && bytes[i + 1].to_ascii_lowercase() == b't'
            && bytes[i + 2].to_ascii_lowercase() == b't'
            && bytes[i + 3].to_ascii_lowercase() == b'r'
            && bytes[i + 4] == b'('
        {
            // Flush literal text
            if literal_start < i {
                result.push_str(&css[literal_start..i]);
            }

            i += 5;
            while i < len && bytes[i] == b' ' { i += 1; }

            let name_start = i;
            while i < len && bytes[i] != b')' && bytes[i] != b',' { i += 1; }
            let name = css[name_start..i].trim_end();

            let mut fallback: Option<&str> = None;
            if i < len && bytes[i] == b',' {
                i += 1;
                while i < len && bytes[i] == b' ' { i += 1; }
                let fb_start = i;
                let mut paren_depth = 1u32;
                while i < len && paren_depth > 0 {
                    match bytes[i] {
                        b'(' => paren_depth += 1,
                        b')' => paren_depth -= 1,
                        _ => {}
                    }
                    if paren_depth > 0 { i += 1; }
                }
                fallback = Some(css[fb_start..i].trim_end());
            }

            if i < len && bytes[i] == b')' { i += 1; }
            literal_start = i;

            if let Some(value) = attr_lookup(name) {
                result.push_str(&value);
            } else if let Some(fb) = fallback {
                result.push_str(fb);
            } else {
                // attr() with no matching attribute and no fallback — empty string per spec
            }
        } else {
            i += 1;
        }
    }

    if literal_start < len {
        result.push_str(&css[literal_start..len]);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: assert exact string result or None.
    fn assert_sub(css: &str, props: &[(&str, &str)], expected: Option<&str>) {
        let mut map = CustomPropertyMap::new();
        for &(k, v) in props {
            map.insert(Atom::from(k), Atom::from(v));
        }
        let got = substitute_vars_in_string(css, &map, 0);
        assert_eq!(got.as_deref(), expected, "input: {css:?}");
    }

    fn assert_full(
        css: &str,
        props: &[(&str, &str)],
        env: &[(&str, &str)],
        attrs: &[(&str, &str)],
        expected: Option<&str>,
    ) {
        let mut map = CustomPropertyMap::new();
        for &(k, v) in props {
            map.insert(Atom::from(k), Atom::from(v));
        }
        let mut ev = EnvironmentValues::empty();
        for &(k, v) in env {
            ev.insert(k, v);
        }
        let attrs_owned: Vec<(String, String)> = attrs.iter().map(|&(k, v)| (k.to_string(), v.to_string())).collect();
        let got = substitute(css, &map, &ev, |name| {
            attrs_owned.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
        });
        assert_eq!(got.as_deref(), expected, "input: {css:?}");
    }

    // ═══════════════════════════════════════════════════
    // extract_var_references — exact output assertions
    // ═══════════════════════════════════════════════════

    #[test]
    fn extract_refs_exact_single() {
        let refs = extract_var_references("var(--color)");
        assert_eq!(refs.as_slice().iter().map(|a| a.as_ref()).collect::<Vec<_>>(), vec!["color"]);
    }

    #[test]
    fn extract_refs_exact_multiple() {
        let refs = extract_var_references("calc(var(--a) + var(--b))");
        assert_eq!(refs.as_slice().iter().map(|a| a.as_ref()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn extract_refs_with_fallback_ignores_fallback_text() {
        let refs = extract_var_references("var(--x, 10px)");
        assert_eq!(refs.as_slice().iter().map(|a| a.as_ref()).collect::<Vec<_>>(), vec!["x"]);
    }

    #[test]
    fn extract_refs_nested_fallback_captures_both() {
        let refs = extract_var_references("var(--a, var(--b))");
        let names: Vec<&str> = refs.iter().map(|a| a.as_ref()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn extract_refs_plain_css_returns_empty() {
        assert_eq!(extract_var_references("10px solid red").len(), 0);
        assert_eq!(extract_var_references("").len(), 0);
        assert_eq!(extract_var_references("rgb(255, 0, 0)").len(), 0);
    }

    #[test]
    fn extract_refs_not_confused_by_var_in_string() {
        // "variable" contains "var" but not "var("
        assert_eq!(extract_var_references("variable").len(), 0);
        assert_eq!(extract_var_references("var iation").len(), 0);
    }

    #[test]
    fn extract_refs_hyphenated_names() {
        let refs = extract_var_references("var(--my-long-name)");
        assert_eq!(refs[0].as_ref(), "my-long-name");
    }

    // ═══════════════════════════════════════════════════
    // var() substitution — exact output, edge cases
    // ═══════════════════════════════════════════════════

    #[test]
    fn var_simple_exact_output() {
        assert_sub("var(--color)", &[("color", "red")], Some("red"));
    }

    #[test]
    fn var_in_calc_preserves_surrounding_text() {
        assert_sub(
            "calc(var(--gap) * 2)",
            &[("gap", "16px")],
            Some("calc(16px * 2)"),
        );
    }

    #[test]
    fn var_multiple_in_one_value() {
        assert_sub(
            "var(--x) var(--y)",
            &[("x", "10px"), ("y", "20px")],
            Some("10px 20px"),
        );
    }

    #[test]
    fn var_missing_no_fallback_returns_none() {
        assert_sub("var(--missing)", &[], None);
    }

    #[test]
    fn var_missing_with_fallback_uses_fallback() {
        assert_sub("var(--missing, 10px)", &[], Some("10px"));
    }

    #[test]
    fn var_present_ignores_fallback() {
        assert_sub("var(--x, FALLBACK)", &[("x", "REAL")], Some("REAL"));
    }

    #[test]
    fn var_fallback_contains_var_resolved() {
        assert_sub(
            "var(--missing, var(--b))",
            &[("b", "blue")],
            Some("blue"),
        );
    }

    #[test]
    fn var_fallback_contains_var_also_missing() {
        assert_sub("var(--a, var(--b))", &[], None);
    }

    #[test]
    fn var_chained_resolution() {
        assert_sub(
            "var(--a)",
            &[("a", "var(--b)"), ("b", "42px")],
            Some("42px"),
        );
    }

    #[test]
    fn var_triple_chain() {
        assert_sub(
            "var(--a)",
            &[("a", "var(--b)"), ("b", "var(--c)"), ("c", "DEEP")],
            Some("DEEP"),
        );
    }

    #[test]
    fn var_self_reference_hits_depth_limit() {
        assert_sub("var(--x)", &[("x", "var(--x)")], None);
    }

    #[test]
    fn var_empty_value_substitutes_empty() {
        assert_sub("var(--e)", &[("e", "")], Some(""));
    }

    #[test]
    fn var_value_with_spaces() {
        assert_sub(
            "var(--font)",
            &[("font", "16px / 1.5 sans-serif")],
            Some("16px / 1.5 sans-serif"),
        );
    }

    #[test]
    fn var_preserves_text_before_and_after() {
        assert_sub(
            "border: var(--w) solid var(--c)",
            &[("w", "2px"), ("c", "red")],
            Some("border: 2px solid red"),
        );
    }

    #[test]
    fn var_no_var_in_input_returns_identity() {
        assert_sub("10px solid red", &[], Some("10px solid red"));
    }

    #[test]
    fn var_empty_input() {
        assert_sub("", &[], Some(""));
    }

    #[test]
    fn var_fallback_with_commas() {
        // Fallback "1px, 2px, 3px" — everything between , and matching )
        assert_sub(
            "var(--missing, 1px 2px 3px)",
            &[],
            Some("1px 2px 3px"),
        );
    }

    #[test]
    fn var_fallback_with_nested_parens() {
        assert_sub(
            "var(--missing, rgb(255, 0, 0))",
            &[],
            Some("rgb(255, 0, 0)"),
        );
    }

    // ═══════════════════════════════════════════════════
    // resolve_custom_properties — cycle detection, exact values
    // ═══════════════════════════════════════════════════

    #[test]
    fn resolve_linear_chain_exact_values() {
        let decls = vec![
            (Atom::from("a"), Atom::from("10px")),
            (Atom::from("b"), Atom::from("var(--a)")),
            (Atom::from("c"), Atom::from("calc(var(--b) + 5px)")),
        ];
        let r = resolve_custom_properties(&decls, None, &Default::default());
        assert_eq!(r.get_str("a").unwrap().as_ref(), "10px");
        assert_eq!(r.get_str("b").unwrap().as_ref(), "10px");
        assert_eq!(r.get_str("c").unwrap().as_ref(), "calc(10px + 5px)");
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn resolve_two_way_cycle_both_gone() {
        let decls = vec![
            (Atom::from("a"), Atom::from("var(--b)")),
            (Atom::from("b"), Atom::from("var(--a)")),
        ];
        let r = resolve_custom_properties(&decls, None, &Default::default());
        assert!(r.get_str("a").is_none(), "--a must be invalid (cycle)");
        assert!(r.get_str("b").is_none(), "--b must be invalid (cycle)");
    }

    #[test]
    fn resolve_self_cycle_gone() {
        let decls = vec![(Atom::from("x"), Atom::from("var(--x)"))];
        let r = resolve_custom_properties(&decls, None, &Default::default());
        assert!(r.get_str("x").is_none(), "--x must be invalid (self-cycle)");
    }

    #[test]
    fn resolve_three_way_cycle_all_gone() {
        let decls = vec![
            (Atom::from("a"), Atom::from("var(--b)")),
            (Atom::from("b"), Atom::from("var(--c)")),
            (Atom::from("c"), Atom::from("var(--a)")),
        ];
        let r = resolve_custom_properties(&decls, None, &Default::default());
        assert!(r.get_str("a").is_none());
        assert!(r.get_str("b").is_none());
        assert!(r.get_str("c").is_none());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn resolve_cycle_does_not_poison_non_cyclic() {
        let decls = vec![
            (Atom::from("ok"), Atom::from("10px")),
            (Atom::from("also-ok"), Atom::from("var(--ok)")),
            (Atom::from("a"), Atom::from("var(--b)")),
            (Atom::from("b"), Atom::from("var(--a)")),
        ];
        let r = resolve_custom_properties(&decls, None, &Default::default());
        assert_eq!(r.get_str("ok").unwrap().as_ref(), "10px");
        assert_eq!(r.get_str("also-ok").unwrap().as_ref(), "10px");
        assert!(r.get_str("a").is_none());
        assert!(r.get_str("b").is_none());
    }

    #[test]
    fn resolve_inheritance_exact() {
        let mut parent = CustomPropertyMap::new();
        parent.insert(Atom::from("color"), Atom::from("blue"));
        parent.insert(Atom::from("size"), Atom::from("16px"));

        let decls = vec![(Atom::from("color"), Atom::from("red"))];
        let r = resolve_custom_properties(&decls, Some(&parent), &Default::default());
        assert_eq!(r.get_str("color").unwrap().as_ref(), "red", "child overrides parent");
        assert_eq!(r.get_str("size").unwrap().as_ref(), "16px", "inherited from parent");
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn resolve_child_refs_parent_property() {
        let mut parent = CustomPropertyMap::new();
        parent.insert(Atom::from("base"), Atom::from("8px"));

        let decls = vec![(Atom::from("gap"), Atom::from("var(--base)"))];
        let r = resolve_custom_properties(&decls, Some(&parent), &Default::default());
        assert_eq!(r.get_str("gap").unwrap().as_ref(), "8px");
        assert_eq!(r.get_str("base").unwrap().as_ref(), "8px");
    }

    #[test]
    fn resolve_child_refs_missing_parent_property_fails() {
        let parent = CustomPropertyMap::new();
        let decls = vec![(Atom::from("x"), Atom::from("var(--nope)"))];
        let r = resolve_custom_properties(&decls, Some(&parent), &Default::default());
        // --x references --nope which doesn't exist → invalid
        assert!(r.get_str("x").is_none());
    }

    #[test]
    fn resolve_empty_declarations_empty_result() {
        let r = resolve_custom_properties(&[], None, &Default::default());
        assert_eq!(r.len(), 0);
        assert!(r.is_empty());
    }

    #[test]
    fn resolve_empty_declarations_with_parent() {
        let mut parent = CustomPropertyMap::new();
        parent.insert(Atom::from("x"), Atom::from("1"));
        let r = resolve_custom_properties(&[], Some(&parent), &Default::default());
        assert_eq!(r.get_str("x").unwrap().as_ref(), "1");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn resolve_last_declaration_wins() {
        // CSS cascade: last declaration for same property wins
        let decls = vec![
            (Atom::from("x"), Atom::from("first")),
            (Atom::from("x"), Atom::from("second")),
        ];
        let r = resolve_custom_properties(&decls, None, &Default::default());
        // Last one indexed wins (the second "x" is at index 1, overwrites index 0)
        assert!(r.get_str("x").is_some());
    }

    // ═══════════════════════════════════════════════════
    // env() substitution — exact, with failures
    // ═══════════════════════════════════════════════════

    #[test]
    fn env_exact_value() {
        assert_full(
            "env(safe-area-inset-top)",
            &[], &[("safe-area-inset-top", "44px")], &[],
            Some("44px"),
        );
    }

    #[test]
    fn env_missing_with_fallback() {
        assert_full(
            "env(safe-area-inset-top, 0px)",
            &[], &[], &[],
            Some("0px"),
        );
    }

    #[test]
    fn env_missing_no_fallback_fails() {
        assert_full(
            "env(unknown-env-var)",
            &[], &[], &[],
            None,
        );
    }

    #[test]
    fn env_from_device() {
        let mut device = Device::new(1024.0, 768.0);
        device.safe_area_inset_top = 44.0;
        device.safe_area_inset_bottom = 34.0;
        let ev = EnvironmentValues::from_device(&device);
        assert_eq!(ev.get("safe-area-inset-top").unwrap().as_ref(), "44px");
        assert_eq!(ev.get("safe-area-inset-bottom").unwrap().as_ref(), "34px");
        assert_eq!(ev.get("safe-area-inset-left").unwrap().as_ref(), "0px");
        assert_eq!(ev.get("safe-area-inset-right").unwrap().as_ref(), "0px");
    }

    #[test]
    fn env_in_calc() {
        assert_full(
            "calc(100vh - env(safe-area-inset-top))",
            &[], &[("safe-area-inset-top", "44px")], &[],
            Some("calc(100vh - 44px)"),
        );
    }

    // ═══════════════════════════════════════════════════
    // attr() substitution — exact, with failures
    // ═══════════════════════════════════════════════════

    #[test]
    fn attr_exact_value() {
        assert_full(
            "attr(data-width)",
            &[], &[], &[("data-width", "200px")],
            Some("200px"),
        );
    }

    #[test]
    fn attr_missing_no_fallback_returns_empty() {
        // attr() with no matching attribute — returns empty string per spec for content
        assert_full(
            "attr(data-nope)",
            &[], &[], &[],
            Some(""),
        );
    }

    // ═══════════════════════════════════════════════════
    // Mixed var() + env() + attr() — exact
    // ═══════════════════════════════════════════════════

    #[test]
    fn mixed_var_and_env_exact() {
        assert_full(
            "calc(var(--gap) + env(safe-area-inset-top))",
            &[("gap", "16px")],
            &[("safe-area-inset-top", "44px")],
            &[],
            Some("calc(16px + 44px)"),
        );
    }

    #[test]
    fn mixed_all_three() {
        assert_full(
            "var(--w) env(safe-area-inset-left) attr(data-x)",
            &[("w", "100px")],
            &[("safe-area-inset-left", "10px")],
            &[("data-x", "5px")],
            Some("100px 10px 5px"),
        );
    }

    #[test]
    fn no_substitution_passthrough() {
        assert_full(
            "10px solid red",
            &[], &[], &[],
            Some("10px solid red"),
        );
    }

    #[test]
    fn empty_string_passthrough() {
        assert_full("", &[], &[], &[], Some(""));
    }

    // ═══════════════════════════════════════════════════
    // Boundary conditions
    // ═══════════════════════════════════════════════════

    #[test]
    fn depth_limit_exactly_at_max() {
        // Build a chain of MAX_SUBSTITUTION_DEPTH + 1 vars
        let mut map = CustomPropertyMap::new();
        map.insert(Atom::from("x"), Atom::from("var(--x)")); // infinite loop
        let result = substitute_vars_in_string("var(--x)", &map, 0);
        assert_eq!(result, None, "self-referencing var must fail at depth limit");
    }

    #[test]
    fn size_limit_enforcement() {
        // Value that expands to > 1MB
        let mut map = CustomPropertyMap::new();
        let big = "A".repeat(MAX_RESULT_SIZE + 1);
        map.insert(Atom::from("huge"), Atom::from(big.as_str()));
        let result = substitute_vars_in_string("var(--huge)", &map, 0);
        assert_eq!(result, None, "result > 1MB must fail");
    }

    // ═══════════════════════════════════════════════════
    // CustomPropertyMap API
    // ═══════════════════════════════════════════════════

    #[test]
    fn map_get_missing_returns_none() {
        let map = CustomPropertyMap::new();
        assert!(map.get_str("anything").is_none());
        assert!(map.get(&Atom::from("anything")).is_none());
    }

    #[test]
    fn map_insert_overwrite() {
        let mut map = CustomPropertyMap::new();
        map.insert(Atom::from("x"), Atom::from("old"));
        map.insert(Atom::from("x"), Atom::from("new"));
        assert_eq!(map.get_str("x").unwrap().as_ref(), "new");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn map_inherit_is_independent_copy() {
        let mut parent = CustomPropertyMap::new();
        parent.insert(Atom::from("x"), Atom::from("1"));
        let child = CustomPropertyMap::inherit(&parent);
        assert_eq!(child.get_str("x").unwrap().as_ref(), "1");
        // Modifying parent after inherit doesn't affect child
        parent.insert(Atom::from("x"), Atom::from("changed"));
        assert_eq!(child.get_str("x").unwrap().as_ref(), "1");
    }

    #[test]
    fn map_remove() {
        let mut map = CustomPropertyMap::new();
        map.insert(Atom::from("x"), Atom::from("1"));
        map.remove(&Atom::from("x"));
        assert!(map.get_str("x").is_none());
        assert_eq!(map.len(), 0);
    }

    // ═══════════════════════════════════════════════════
    // EnvironmentValues API
    // ═══════════════════════════════════════════════════

    #[test]
    fn env_values_empty_returns_none() {
        let ev = EnvironmentValues::empty();
        assert!(ev.get("anything").is_none());
    }

    #[test]
    fn env_values_insert_and_get() {
        let mut ev = EnvironmentValues::empty();
        ev.insert("my-var", "42px");
        assert_eq!(ev.get("my-var").unwrap().as_ref(), "42px");
    }

    #[test]
    fn env_values_overwrite() {
        let mut ev = EnvironmentValues::empty();
        ev.insert("x", "old");
        ev.insert("x", "new");
        assert_eq!(ev.get("x").unwrap().as_ref(), "new");
    }
}
