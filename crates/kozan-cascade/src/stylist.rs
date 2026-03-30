//! Stylesheet indexer — walks parsed CSS rule trees and builds selector-bucketed
//! rule maps for O(1) element matching.
//!
//! The `Stylist` is the bridge between parsing and cascade: it takes parsed
//! `Stylesheet`s, evaluates `@media` / `@supports` / `@layer` conditions,
//! and produces flat, pre-sorted `RuleMap`s that the cascade can query
//! per-element in microseconds.
//!
//! One `Stylist` per `View` — zero contention between independent views.

use kozan_atom::Atom;
use kozan_css::{
    CssRule, KeyframesRule, LayerRule, PropertyRule, Stylesheet,
};
use kozan_selector::fxhash::FxHashMap;
use kozan_selector::invalidation::InvalidationMap;
use kozan_selector::rule_map::{RuleMap, RuleMapBuilder};
use kozan_selector::specificity::Specificity;
use kozan_selector::types::{Component, Selector, SelectorList};
use kozan_style::{DeclarationBlock, Importance};
use smallvec::SmallVec;

use crate::device::Device;
use crate::layer::{LayerOrderMap, UNLAYERED};
use crate::media;
use crate::origin::{CascadeLevel, CascadeOrigin};

/// An indexed style rule — declarations plus cascade metadata.
///
/// `RuleEntry::data` in the `RuleMap` is a `u32` index into `Stylist::rules`.
/// During cascade, each declaration's importance combines with `origin` and
/// `layer_order` to produce a `CascadeLevel` for priority comparison.
///
/// Declarations are `Arc`-shared with the source `StyleRule` — indexing
/// is a refcount bump, not a deep clone.
pub struct IndexedRule {
    pub declarations: triomphe::Arc<DeclarationBlock>,
    pub origin: CascadeOrigin,
    pub layer_order: u16,
    /// If this rule came from a `@container` block, the condition + optional name.
    /// `None` for rules outside any container query.
    pub container: Option<ContainerInfo>,
    /// If this rule came from a `@scope` block, the scope boundaries.
    /// `None` for rules outside any scope.
    pub scope: Option<ScopeInfo>,
    /// Whether this rule came from a `@starting-style` block.
    /// Starting-style rules only apply when an element is first inserted.
    pub starting_style: bool,
}

/// Container query condition attached to an indexed rule.
#[derive(Clone)]
pub struct ContainerInfo {
    pub name: Option<Atom>,
    pub condition: triomphe::Arc<kozan_css::rules::container::ContainerCondition>,
}

/// Scope boundaries attached to an indexed rule.
#[derive(Clone)]
pub struct ScopeInfo {
    /// Scope root selector. Element must be descendant of a match.
    /// `None` = scoped to stylesheet owner (effectively root).
    pub start: Option<kozan_selector::SelectorList>,
    /// Scope limit selector. Element must NOT be at/below a match.
    /// `None` = no limit (scope extends to leaves).
    pub end: Option<kozan_selector::SelectorList>,
}

impl IndexedRule {
    /// Cascade level for a declaration with the given importance.
    #[inline]
    #[must_use] 
    pub fn level(&self, importance: Importance) -> CascadeLevel {
        CascadeLevel::new(self.origin, importance, self.layer_order)
    }
}

/// A stored stylesheet paired with its cascade origin.
struct StoredSheet {
    sheet: Stylesheet,
    origin: CascadeOrigin,
}

/// Stylesheet indexer and rule storage for a single view.
///
/// Owns all stylesheets and maintains:
/// - Per-origin `RuleMap`s for selector-bucketed O(1) rule lookup
/// - `InvalidationMap` for targeted DOM-mutation invalidation
/// - `@keyframes` and `@property` registries
/// - Cascade layer ordering
///
/// Rebuild is full (not incremental) — when stylesheets change, call
/// `rebuild()` to re-walk all rules. Sub-millisecond for UI-scale sheets.
pub struct Stylist {
    sheets: Vec<StoredSheet>,
    author_map: RuleMap,
    ua_map: RuleMap,
    user_map: RuleMap,
    rules: Vec<IndexedRule>,
    layer_order: LayerOrderMap,
    invalidation: InvalidationMap,
    keyframes: FxHashMap<Atom, KeyframesRule>,
    properties: FxHashMap<Atom, PropertyRule>,
    device: Device,
    generation: u64,
}

impl Stylist {
    /// Create an empty Stylist for the given device.
    #[must_use] 
    pub fn new(device: Device) -> Self {
        Self {
            sheets: Vec::new(),
            author_map: RuleMap::builder().build(),
            ua_map: RuleMap::builder().build(),
            user_map: RuleMap::builder().build(),
            rules: Vec::new(),
            layer_order: LayerOrderMap::new(),
            invalidation: InvalidationMap::new(),
            keyframes: FxHashMap::default(),
            properties: FxHashMap::default(),
            device,
            generation: 0,
        }
    }

    /// Add a stylesheet. Call `rebuild()` after adding all sheets.
    pub fn add_stylesheet(&mut self, sheet: Stylesheet, origin: CascadeOrigin) {
        self.sheets.push(StoredSheet { sheet, origin });
    }

    /// Remove all stylesheets. Call `rebuild()` after.
    pub fn clear_sheets(&mut self) {
        self.sheets.clear();
    }

    /// Re-index all stylesheets into rule maps.
    ///
    /// Evaluates `@media` against the current device, assigns `@layer` order,
    /// skips disabled `@supports`, and flattens active `StyleRule`s into
    /// per-origin `RuleMap`s. Bumps the generation counter to invalidate caches.
    ///
    /// Pre-allocates based on previous rebuild's rule count to avoid
    /// repeated Vec reallocations on hot reload.
    pub fn rebuild(&mut self) {
        // Take sheets out to avoid borrow conflict (self.sheets is read-only
        // during indexing, but we mutate other fields).
        let sheets = std::mem::take(&mut self.sheets);

        // Remember previous capacity for pre-allocation.
        let prev_rule_count = self.rules.len();
        self.rules.clear();
        if self.rules.capacity() < prev_rule_count {
            self.rules.reserve(prev_rule_count);
        }
        self.layer_order.clear();
        self.invalidation.clear();
        self.keyframes.clear();
        self.properties.clear();

        let mut author = RuleMap::builder();
        let mut ua = RuleMap::builder();
        let mut user = RuleMap::builder();

        for stored in &sheets {
            let builder = match stored.origin {
                CascadeOrigin::Author => &mut author,
                CascadeOrigin::UserAgent => &mut ua,
                CascadeOrigin::User => &mut user,
            };
            let ctx = IndexContext::top(stored.origin);
            index_rules(
                stored.sheet.rules.slice.as_ref(),
                &ctx,
                builder,
                &mut self.rules,
                &mut self.layer_order,
                &mut self.invalidation,
                &mut self.keyframes,
                &mut self.properties,
                &self.device,
            );
        }

        self.author_map = author.build();
        self.ua_map = ua.build();
        self.user_map = user.build();
        self.generation += 1;
        self.sheets = sheets;
    }

    /// Replace a stylesheet at the given index. Call `rebuild()` after.
    ///
    /// Useful for hot reload — swap one sheet without removing all others.
    /// Panics if `index >= sheet_count()`.
    pub fn replace_stylesheet(&mut self, index: usize, sheet: Stylesheet, origin: CascadeOrigin) {
        self.sheets[index] = StoredSheet { sheet, origin };
    }

    /// Remove a stylesheet at the given index. Call `rebuild()` after.
    ///
    /// Panics if `index >= sheet_count()`.
    pub fn remove_stylesheet(&mut self, index: usize) {
        self.sheets.remove(index);
    }

    /// Number of stylesheets currently added.
    #[inline]
    #[must_use]
    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    /// Update the device (viewport resize, color scheme change, etc.).
    ///
    /// Does NOT automatically rebuild — call `rebuild()` if media queries
    /// may have changed. Use `update_device()` for automatic rebuild.
    pub fn set_device(&mut self, device: Device) {
        self.device = device;
    }

    /// Update the device and rebuild if media-affecting properties changed.
    ///
    /// Compares the new device's viewport, color scheme, and preferences
    /// against the current device. If any media-affecting property changed,
    /// triggers a full `rebuild()` to re-evaluate `@media` conditions.
    ///
    /// Returns `true` if a rebuild was triggered (callers should restyle).
    pub fn update_device(&mut self, new_device: Device) -> bool {
        let needs_rebuild = self.device.viewport_width != new_device.viewport_width
            || self.device.viewport_height != new_device.viewport_height
            || self.device.device_pixel_ratio != new_device.device_pixel_ratio
            || self.device.prefers_color_scheme != new_device.prefers_color_scheme
            || self.device.prefers_reduced_motion != new_device.prefers_reduced_motion
            || self.device.prefers_contrast != new_device.prefers_contrast
            || self.device.forced_colors != new_device.forced_colors
            || self.device.pointer != new_device.pointer
            || self.device.hover != new_device.hover;

        self.device = new_device;

        if needs_rebuild {
            self.rebuild();
            true
        } else {
            false
        }
    }

    #[inline]
    #[must_use] 
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Generation counter — incremented on each `rebuild()`.
    /// Used by caches to detect staleness.
    #[inline]
    #[must_use] 
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[inline]
    #[must_use] 
    pub fn author_map(&self) -> &RuleMap {
        &self.author_map
    }

    #[inline]
    #[must_use] 
    pub fn ua_map(&self) -> &RuleMap {
        &self.ua_map
    }

    #[inline]
    #[must_use] 
    pub fn user_map(&self) -> &RuleMap {
        &self.user_map
    }

    /// Access indexed rules. `RuleEntry::data` indexes into this slice.
    #[inline]
    #[must_use] 
    pub fn rules(&self) -> &[IndexedRule] {
        &self.rules
    }

    #[inline]
    #[must_use] 
    pub fn invalidation_map(&self) -> &InvalidationMap {
        &self.invalidation
    }

    #[inline]
    #[must_use] 
    pub fn layer_order(&self) -> &LayerOrderMap {
        &self.layer_order
    }

    /// Look up `@keyframes` by animation name.
    #[inline]
    #[must_use] 
    pub fn keyframes(&self, name: &Atom) -> Option<&KeyframesRule> {
        self.keyframes.get(name)
    }

    /// Look up a registered `@property` by name.
    #[inline]
    #[must_use] 
    pub fn registered_property(&self, name: &Atom) -> Option<&PropertyRule> {
        self.properties.get(name)
    }

    /// All registered `@property` rules as a map (bare name → rule).
    #[inline]
    #[must_use]
    pub fn registered_properties(&self) -> &FxHashMap<Atom, PropertyRule> {
        &self.properties
    }

    /// Iterate over all registered `@property` rules.
    pub fn registered_properties_iter(&self) -> impl Iterator<Item = (&Atom, &PropertyRule)> {
        self.properties.iter()
    }

    /// Number of indexed style rules across all origins.
    #[inline]
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// Cascade context accumulated while walking the rule tree.
/// Replaces the 13-parameter `index_rules` with a clean struct.
#[derive(Clone)]
struct IndexContext<'a> {
    origin: CascadeOrigin,
    layer: u16,
    container: Option<ContainerInfo>,
    scope: Option<ScopeInfo>,
    starting_style: bool,
    /// Parent rule's composed selectors for CSS Nesting.
    /// When indexing nested rules, `&` is replaced with these.
    parent_selectors: Option<&'a SelectorList>,
}

impl<'a> IndexContext<'a> {
    fn top(origin: CascadeOrigin) -> Self {
        Self {
            origin,
            layer: UNLAYERED,
            container: None,
            scope: None,
            starting_style: false,
            parent_selectors: None,
        }
    }

    fn with_parent_selectors<'b>(&self, selectors: &'b SelectorList) -> IndexContext<'b> {
        IndexContext {
            origin: self.origin,
            layer: self.layer,
            container: self.container.clone(),
            scope: self.scope.clone(),
            starting_style: self.starting_style,
            parent_selectors: Some(selectors),
        }
    }
}

/// CSS Nesting: compose a child selector with the parent selector.
///
/// Replaces every `Component::Nesting` (`&`) in the child with the parent's
/// components. If no `&` exists (shouldn't happen after `ensure_nesting`),
/// prepends `parent descendant child`.
///
/// Specificity: each `&` replacement removes one class-level (from the `&`
/// itself) and adds the parent's specificity.
///
/// Components are stored right-to-left (match-order). We reverse to
/// parse-order, do the replacement, then rebuild via `from_parse_order`
/// (which reverses back).
fn compose_selector(parent: &Selector, child: &Selector) -> Selector {
    let child_comps = child.components(); // match-order (right-to-left)
    let parent_comps = parent.components(); // match-order (right-to-left)

    // Build in parse-order (left-to-right) for from_parse_order.
    let mut composed: SmallVec<[Component; 8]> = SmallVec::new();
    let mut nesting_count = 0u32;

    // Walk child in parse-order (reverse of stored match-order).
    for c in child_comps.iter().rev() {
        if matches!(c, Component::Nesting) {
            // Replace & with parent's components (in parse-order).
            for pc in parent_comps.iter().rev() {
                composed.push(pc.clone());
            }
            nesting_count += 1;
        } else {
            composed.push(c.clone());
        }
    }

    // Compute specificity: child - (nesting_count * class) + (nesting_count * parent)
    let class_value = Specificity::new(0, 1, 0).value();
    let child_spec = child.specificity().value();
    let parent_spec = parent.specificity().value();
    let composed_spec = child_spec
        .wrapping_sub(nesting_count * class_value)
        .wrapping_add(nesting_count * parent_spec);

    // Rebuild hints from the child (key selector is the rightmost = child's subject).
    let mut hints = child.hints().clone();
    if parent_comps.iter().any(|c| matches!(c, Component::Combinator(_))) {
        hints.deps.set_combinators();
    }

    Selector::from_parse_order(composed, Specificity::from_raw(composed_spec), hints)
}

/// Compose all combinations of parent × child selectors.
/// `.a, .b { .child {} }` → `.a .child, .b .child`
fn compose_selector_list(parents: &SelectorList, children: &SelectorList) -> SelectorList {
    let mut out = SmallVec::new();
    for child in &children.0 {
        for parent in &parents.0 {
            out.push(compose_selector(parent, child));
        }
    }
    SelectorList(out)
}

/// Recursively walk a rule tree and index style rules.
///
/// Free function to avoid borrow conflicts — the `Stylist` fields are passed
/// individually so the caller can borrow sheets immutably while mutating
/// the index state.
fn index_rules(
    rules: &[CssRule],
    ctx: &IndexContext<'_>,
    builder: &mut RuleMapBuilder,
    indexed: &mut Vec<IndexedRule>,
    layers: &mut LayerOrderMap,
    invalidation: &mut InvalidationMap,
    keyframes: &mut FxHashMap<Atom, KeyframesRule>,
    properties: &mut FxHashMap<Atom, PropertyRule>,
    device: &Device,
) {
    for rule in rules {
        match rule {
            CssRule::Style(style) => {
                if style.declarations.is_empty() && style.rules.slice.is_empty() {
                    continue;
                }

                // CSS Nesting: compose selectors with parent if nested.
                // ensure_nesting (called at parse time) guarantees `&` is present
                // in nested selectors. compose_selector replaces `&` with the
                // parent's actual selector components — fully resolved, no `&`
                // at match time.
                let composed: SelectorList;
                let selectors = if let Some(parents) = ctx.parent_selectors {
                    composed = compose_selector_list(parents, &style.selectors);
                    &composed
                } else {
                    &style.selectors
                };

                if !style.declarations.is_empty() {
                    let rule_index = indexed.len() as u32;

                    indexed.push(IndexedRule {
                        declarations: style.declarations.clone(),
                        origin: ctx.origin,
                        layer_order: ctx.layer,
                        container: ctx.container.clone(),
                        scope: ctx.scope.clone(),
                        starting_style: ctx.starting_style,
                    });

                    for selector in &selectors.0 {
                        builder.insert(selector.clone(), rule_index);
                    }

                    invalidation.add_selector_list(selectors, rule_index);
                }

                // Recurse into nested rules with this rule's composed selectors as parent.
                let nested: &[CssRule] = style.rules.slice.as_ref();
                if !nested.is_empty() {
                    let nested_ctx = ctx.with_parent_selectors(selectors);
                    index_rules(
                        nested, &nested_ctx, builder, indexed, layers,
                        invalidation, keyframes, properties, device,
                    );
                }
            }

            CssRule::Media(media_rule) => {
                if media::evaluate(&media_rule.queries, device) {
                    index_rules(
                        media_rule.rules.slice.as_ref(), ctx, builder, indexed,
                        layers, invalidation, keyframes, properties, device,
                    );
                }
            }

            CssRule::Layer(layer_rule) => match layer_rule.as_ref() {
                LayerRule::Block { name, rules } => {
                    let order = match name {
                        Some(n) => layers.get_or_insert(n),
                        None => layers.next_anonymous(),
                    };
                    let layer_ctx = IndexContext { layer: order, ..ctx.clone() };
                    index_rules(
                        rules.slice.as_ref(), &layer_ctx, builder, indexed,
                        layers, invalidation, keyframes, properties, device,
                    );
                }
                LayerRule::Statement { names } => {
                    for name in names {
                        layers.get_or_insert(name);
                    }
                }
            },

            CssRule::Supports(supports) => {
                if supports.enabled {
                    index_rules(
                        supports.rules.slice.as_ref(), ctx, builder, indexed,
                        layers, invalidation, keyframes, properties, device,
                    );
                }
            }

            CssRule::Container(cq) => {
                let cq_info = ContainerInfo {
                    name: cq.name.clone(),
                    condition: triomphe::Arc::new(cq.condition.clone()),
                };
                let cq_ctx = IndexContext { container: Some(cq_info), ..ctx.clone() };
                index_rules(
                    cq.rules.slice.as_ref(), &cq_ctx, builder, indexed,
                    layers, invalidation, keyframes, properties, device,
                );
            }

            CssRule::Scope(sc) => {
                let scope_info = ScopeInfo {
                    start: sc.start.clone(),
                    end: sc.end.clone(),
                };
                let sc_ctx = IndexContext { scope: Some(scope_info), ..ctx.clone() };
                index_rules(
                    sc.rules.slice.as_ref(), &sc_ctx, builder, indexed,
                    layers, invalidation, keyframes, properties, device,
                );
            }

            CssRule::StartingStyle(ss) => {
                let ss_ctx = IndexContext { starting_style: true, ..ctx.clone() };
                index_rules(
                    ss.rules.slice.as_ref(), &ss_ctx, builder, indexed,
                    layers, invalidation, keyframes, properties, device,
                );
            }

            CssRule::Keyframes(kf) => {
                // Last @keyframes with same name wins (per CSS spec).
                keyframes.insert(kf.name.clone(), kf.as_ref().clone());
            }

            CssRule::Property(prop) => {
                // Last @property with same name wins.
                properties.insert(prop.name.clone(), prop.as_ref().clone());
            }

            // @import: resolved before indexing (loader fetches and inlines).
            // @namespace, @page, @font-face, @counter-style: don't affect
            // selector matching — handled by other subsystems.
            CssRule::Import(_)
            | CssRule::Namespace(_)
            | CssRule::Page(_)
            | CssRule::FontFace(_)
            | CssRule::CounterStyle(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_css::parse_stylesheet;

    fn stylist_with_css(css: &str) -> Stylist {
        let sheet = parse_stylesheet(css);
        let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
        stylist.add_stylesheet(sheet, CascadeOrigin::Author);
        stylist.rebuild();
        stylist
    }

    #[test]
    fn indexes_style_rules() {
        let stylist = stylist_with_css("h1 { color: red } .foo { display: block }");
        assert_eq!(stylist.rule_count(), 2);
        assert_eq!(stylist.generation(), 1);
    }

    #[test]
    fn skips_inactive_media() {
        // Device is 1024x768 — min-width: 2000px should not match.
        let stylist = stylist_with_css(
            "@media (min-width: 2000px) { .wide { display: block } }
             .always { color: red }",
        );
        assert_eq!(stylist.rule_count(), 1);
    }

    #[test]
    fn includes_active_media() {
        // Device is 1024x768 — min-width: 768px should match.
        let stylist = stylist_with_css(
            "@media (min-width: 768px) { .tablet { display: flex } }",
        );
        assert_eq!(stylist.rule_count(), 1);
    }

    #[test]
    fn layer_ordering() {
        let stylist = stylist_with_css(
            "@layer base, utils;
             @layer base { .a { color: red } }
             @layer utils { .b { color: blue } }",
        );
        assert_eq!(stylist.rule_count(), 2);
        assert_eq!(stylist.layer_order().len(), 2);

        // base = layer 0, utils = layer 1
        let rule_a = &stylist.rules()[0];
        let rule_b = &stylist.rules()[1];
        assert!(rule_a.layer_order < rule_b.layer_order);
    }

    #[test]
    fn unlayered_has_max_layer() {
        let stylist = stylist_with_css(".plain { color: red }");
        assert_eq!(stylist.rules()[0].layer_order, UNLAYERED);
    }

    #[test]
    fn supports_disabled_skipped() {
        // @supports with enabled=false should be skipped.
        // Since we can't easily craft a disabled @supports through parsing
        // (the parser evaluates at parse time), test that a valid @supports
        // does get included.
        let stylist = stylist_with_css(
            "@supports (display: flex) { .flex { display: flex } }",
        );
        assert_eq!(stylist.rule_count(), 1);
    }

    #[test]
    fn keyframes_registered() {
        let stylist = stylist_with_css(
            "@keyframes fadeIn { from { opacity: 0 } to { opacity: 1 } }
             .animate { animation-name: fadeIn }",
        );
        assert!(stylist.keyframes(&Atom::from("fadeIn")).is_some());
        assert!(stylist.keyframes(&Atom::from("missing")).is_none());
    }

    #[test]
    fn property_registered() {
        let stylist = stylist_with_css(
            "@property --gap { syntax: '<length>'; inherits: false; initial-value: 0px }",
        );
        assert!(stylist.registered_property(&Atom::from("--gap")).is_some());
    }

    #[test]
    fn rebuild_bumps_generation() {
        let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
        assert_eq!(stylist.generation(), 0);
        stylist.rebuild();
        assert_eq!(stylist.generation(), 1);
        stylist.rebuild();
        assert_eq!(stylist.generation(), 2);
    }

    #[test]
    fn multiple_origins() {
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
    fn empty_rule_skipped() {
        // A rule with no declarations and no nested rules should not be indexed.
        let stylist = stylist_with_css(".empty {} .notempty { color: red }");
        assert_eq!(stylist.rule_count(), 1);
    }

    #[test]
    fn cascade_level_ordering() {
        let stylist = stylist_with_css(
            "@layer base { .a { color: red } }
             .b { color: blue }",
        );
        let rule_a = &stylist.rules()[0]; // layer 0
        let rule_b = &stylist.rules()[1]; // unlayered

        let level_a = rule_a.level(Importance::Normal);
        let level_b = rule_b.level(Importance::Normal);

        // Unlayered (b) beats layered (a) for normal declarations.
        assert!(level_a < level_b);
    }
}
