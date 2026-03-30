//! Style resolver — the full per-element style resolution pipeline.
//!
//! This is the central orchestrator that connects all cascade subsystems:
//! selector matching → cascade sort → custom property resolution →
//! var()/env()/attr() substitution → declaration application → ComputedStyle.
//!
//! # Pipeline (single-pass cascade)
//!
//! 1. Match selectors across all origins (UA, user, author)
//! 2. Sort by cascade priority (origin → layer → specificity → source order)
//! 3. Collect custom property declarations for var() resolution
//! 4. Resolve custom properties (cycle detection, var-within-var)
//! 5. Two-pass cascade apply (normal, then !important):
//!    - **Early properties first**: direction, writing-mode, font-size, color
//!    - **Update ComputeContext** with resolved early values
//!    - **Late properties**: everything else, using updated context
//! 6. Return ComputedStyle + resolved custom properties
//!
//! # Cache Hierarchy
//!
//! - **SharingCache** (L1): 60-80% hit rate, ~33ns per lookup
//! - **MatchedPropertiesCache** (L2): skips cascade for same rule sets
//! - **RuleMap bucketing** (L0): O(1) selector lookup

use std::sync::Arc;
use kozan_atom::Atom;
use kozan_selector::element::Element;
use kozan_style::{ComputeContext, ComputedStyle, PropertyDeclaration};
use smallvec::SmallVec;

use crate::cascade::{self, ApplicableDeclaration};
use crate::container::{self, ContainerEvalContext, ContainerLookup, NoContainers};
use crate::custom_properties::{self, CustomPropertyMap, EnvironmentValues};
use crate::sharing_cache::{hash_matched, mpc_key, MatchedPropertiesCache, SharingCache, SharingKey};
use crate::stylist::Stylist;

/// CSS initial font-size in px (W3C CSS Fonts Level 4 §3.1: `medium` = 16px).
pub(crate) const INITIAL_FONT_SIZE_PX: f32 = 16.0;

/// CSS absolute font-size keyword values in px.
/// Index 0 = xx-small, 7 = xxx-large.
/// W3C CSS Fonts Level 4 §3.1, Table 2 — matches Chrome/Firefox/Safari.
const FONT_SIZE_KEYWORDS_PX: [f32; 8] = [
    9.0,   // xx-small
    10.0,  // x-small
    13.0,  // small
    16.0,  // medium
    18.0,  // large
    24.0,  // x-large
    32.0,  // xx-large
    48.0,  // xxx-large
];

/// Relative font-size scaling factors (W3C CSS Fonts Level 4 §3.2).
/// `smaller` = previous step, `larger` = next step in the keyword table.
/// When parent is between keywords, use these ratios as approximation.
const FONT_SIZE_SMALLER_RATIO: f32 = 5.0 / 6.0;
const FONT_SIZE_LARGER_RATIO: f32 = 6.0 / 5.0;

/// Result of resolving an element's style.
#[derive(Clone)]
pub struct ResolvedStyle {
    pub style: ComputedStyle,
    pub custom_properties: CustomPropertyMap,
}

/// Per-view style resolution context. Holds caches and shared state.
///
/// Create one per view (or per restyle pass). Reuse across elements.
pub struct StyleResolver {
    sharing_cache: SharingCache,
    mpc: MatchedPropertiesCache,
    env_values: EnvironmentValues,
    containers: Box<dyn ContainerLookup>,
}

impl StyleResolver {
    pub fn new(env_values: EnvironmentValues) -> Self {
        Self {
            sharing_cache: SharingCache::new(),
            mpc: MatchedPropertiesCache::new(),
            env_values,
            containers: Box::new(NoContainers),
        }
    }

    /// Set the container lookup for `@container` query evaluation.
    /// Call before each layout pass with updated container sizes.
    pub fn set_container_lookup(&mut self, lookup: Box<dyn ContainerLookup>) {
        self.containers = lookup;
    }

    /// Invalidate caches when stylesheets change.
    pub fn on_stylesheet_change(&mut self, generation: u64) {
        self.sharing_cache.check_generation(generation);
        self.mpc.check_generation(generation);
    }

    /// Resolve the computed style for a single element.
    ///
    /// `inline_style`: optional inline `style=""` declarations. Applied with
    /// highest specificity in the author origin (CSS spec: inline > any selector).
    pub fn resolve<E: Element>(
        &mut self,
        element: &E,
        stylist: &Stylist,
        parent_style: Option<&ComputedStyle>,
        parent_custom_props: Option<&CustomPropertyMap>,
        inline_style: Option<&kozan_style::DeclarationBlock>,
        compute_ctx: &ComputeContext,
        attr_lookup: impl Fn(&str) -> Option<String>,
    ) -> Arc<ResolvedStyle> {
        // ─── Step 0: Sharing cache — skip EVERYTHING for similar elements ───
        // Elements with identical tag/id/classes/state/parent produce identical
        // styles. On typical pages, 60-80% of elements hit this cache.
        // Returns Arc clone (cheap pointer bump, no deep copy).
        // Skip if inline style present (unique per element).
        if inline_style.is_none() {
            let parent_id = parent_style.map_or(0u64, |p| p as *const ComputedStyle as u64);
            let mut classes: SmallVec<[Atom; 4]> = SmallVec::new();
            element.each_class(|c| classes.push(c.clone()));
            let sharing_key = SharingKey::new(
                element.local_name().clone(),
                element.id().cloned(),
                classes,
                element.state().bits() as u32,
                parent_id,
            );
            if let Some(cached) = self.sharing_cache.get(&sharing_key) {
                return cached;
            }
            // Store key for insert after resolution.
            return self.resolve_inner(
                element, stylist, parent_style, parent_custom_props,
                inline_style, compute_ctx, attr_lookup, Some(sharing_key),
            );
        }

        self.resolve_inner(
            element, stylist, parent_style, parent_custom_props,
            inline_style, compute_ctx, attr_lookup, None,
        )
    }

    /// Inner resolution — separated so sharing cache check can early-return.
    fn resolve_inner<E: Element>(
        &mut self,
        element: &E,
        stylist: &Stylist,
        parent_style: Option<&ComputedStyle>,
        parent_custom_props: Option<&CustomPropertyMap>,
        inline_style: Option<&kozan_style::DeclarationBlock>,
        compute_ctx: &ComputeContext,
        attr_lookup: impl Fn(&str) -> Option<String>,
        sharing_key: Option<SharingKey>,
    ) -> Arc<ResolvedStyle> {
        // ─── Step 1: Match selectors across all origins ───
        let author_matches = stylist.author_map().find_matching(element);
        let ua_matches = stylist.ua_map().find_matching(element);
        let user_matches = stylist.user_map().find_matching(element);
        let rules = stylist.rules();

        let total = author_matches.len() + ua_matches.len() + user_matches.len();
        let mut decls: SmallVec<[ApplicableDeclaration; 32]> = SmallVec::with_capacity(total);
        for entry in ua_matches.iter().chain(user_matches.iter()).chain(author_matches.iter()) {
            let rule = &rules[entry.data as usize];
            decls.push(ApplicableDeclaration {
                rule_index: entry.data,
                specificity: entry.specificity.value(),
                source_order: entry.source_order,
                origin: rule.origin,
                layer_order: rule.layer_order,
                // scope_depth is set to 0 here; the caller may override it
                // after matching when the rule comes from an @scope block.
                scope_depth: 0,
            });
        }

        // ─── Step 2: MPC check — skip cascade if identical rules were resolved before ───
        let matched_hash = hash_matched(&decls);
        let parent_ptr = parent_style.map_or(0u64, |p| p as *const ComputedStyle as u64);
        let cache_key = mpc_key(matched_hash, parent_ptr, inline_style.is_some());

        if inline_style.is_none() {
            if let Some(cached) = self.mpc.get(cache_key) {
                return cached;
            }
        }

        // ─── Step 2b: Filter out rules whose @container condition doesn't match ───
        let element_id = element.opaque().raw();
        let base_container_ctx = ContainerEvalContext {
            font_size: compute_ctx.font_size,
            root_font_size: compute_ctx.root_font_size,
            viewport_width: compute_ctx.viewport_width,
            viewport_height: compute_ctx.viewport_height,
            container_width: 0.0,
            container_height: 0.0,
        };
        decls.retain(|d| {
            let rule = &rules[d.rule_index as usize];
            match &rule.container {
                None => true,
                Some(info) => {
                    match self.containers.find_container(element_id, info.name.as_ref()) {
                        None => false, // no container found → condition doesn't match
                        Some(size) => {
                            // Set container dimensions so cqw/cqh units resolve
                            // against the actual container, not the viewport.
                            let ctx = ContainerEvalContext {
                                container_width: size.width,
                                container_height: size.height,
                                ..base_container_ctx
                            };
                            container::evaluate_container_condition(
                                &info.condition, &size, &ctx,
                            )
                        }
                    }
                }
            }
        });

        // ─── Step 2c: Filter @scope rules — element must be in scope ───
        decls.retain(|d| {
            let rule = &rules[d.rule_index as usize];
            match &rule.scope {
                None => true,
                Some(scope_info) => {
                    // Check scope root: element must be descendant of a match.
                    if let Some(start) = &scope_info.start {
                        let in_scope = Self::ancestor_matches(element, start);
                        if !in_scope { return false; }
                    }
                    // Check scope limit: element must NOT be at/below a match.
                    if let Some(end) = &scope_info.end {
                        if Self::self_or_ancestor_matches(element, end) {
                            return false;
                        }
                    }
                    true
                }
            }
        });

        // ─── Step 2d: Filter @starting-style ───
        // CSS Transitions Level 2: @starting-style rules apply ONLY to elements
        // on their first style resolution after DOM insertion. After the first
        // restyle, the element is no longer "newly inserted" and these rules
        // are skipped. This provides the "from" state for entry animations.
        if !element.is_newly_inserted() {
            decls.retain(|d| !rules[d.rule_index as usize].starting_style);
        }

        // ─── Step 3: Cascade sort ───
        cascade::sort(&mut decls, rules);

        // ─── Step 4: Collect and resolve custom properties ───
        let mut custom_decls: SmallVec<[(Atom, Atom); 8]> = SmallVec::new();
        for ad in &decls {
            let rule = &rules[ad.rule_index as usize];
            for (prop_decl, _imp) in rule.declarations.entries() {
                if let PropertyDeclaration::Custom { name, value } = prop_decl {
                    let bare = name.as_ref().strip_prefix("--").unwrap_or(name.as_ref());
                    custom_decls.push((Atom::from(bare), value.clone()));
                }
            }
        }
        // Inline style custom properties override author rules.
        if let Some(inline) = inline_style {
            for (prop_decl, _imp) in inline.entries() {
                if let PropertyDeclaration::Custom { name, value } = prop_decl {
                    let bare = name.as_ref().strip_prefix("--").unwrap_or(name.as_ref());
                    custom_decls.push((Atom::from(bare), value.clone()));
                }
            }
        }
        let mut custom_props = custom_properties::resolve_custom_properties(
            &custom_decls, parent_custom_props, stylist.registered_properties(),
        );

        // ─── Step 4b: Apply @property initial values ───
        // Registered custom properties that weren't set get their initial-value.
        // Properties with `inherits: false` that weren't explicitly set do NOT
        // inherit from parent — they reset to initial-value.
        for (name, prop_rule) in stylist.registered_properties_iter() {
            let bare = name.as_ref().strip_prefix("--").unwrap_or(name.as_ref());
            let bare_atom = Atom::from(bare);
            let was_set = custom_decls.iter().any(|(n, _)| *n == bare_atom);

            if !was_set {
                if !prop_rule.inherits {
                    // Non-inheriting: remove any inherited value, apply initial.
                    if let Some(initial) = &prop_rule.initial_value {
                        custom_props.insert(bare_atom, initial.clone());
                    } else {
                        custom_props.remove(&bare_atom);
                    }
                } else if custom_props.get(&bare_atom).is_none() {
                    // Inheriting but not in parent: apply initial-value.
                    if let Some(initial) = &prop_rule.initial_value {
                        custom_props.insert(bare_atom, initial.clone());
                    }
                }
            }
        }

        // ─── Step 5: Build ComputedStyle with single-pass cascade ───
        let mut style = match parent_style {
            Some(parent) => ComputedStyle::inherit(parent),
            None => ComputedStyle::default(),
        };

        // Phase 1: Apply early properties (two-pass: normal + !important).
        // These MUST resolve before late properties because:
        // - direction + writing-mode → logical property resolution
        // - font-size → em/ex/ch unit resolution
        // - color → currentColor resolution
        self.apply_early_properties(&decls, rules, &mut style, parent_style, compute_ctx);

        // Apply inline early properties (highest specificity, overrides selectors).
        if let Some(inline) = inline_style {
            for (prop_decl, _imp) in inline.entries() {
                if is_early_property(prop_decl) {
                    style.apply_declaration(prop_decl, parent_style, compute_ctx);
                }
            }
        }

        // Phase 2: Build updated ComputeContext with resolved early values.
        let parent_font_px = parent_style
            .map(|p| resolve_font_size_px(&p.text.font_size, INITIAL_FONT_SIZE_PX, compute_ctx))
            .unwrap_or(INITIAL_FONT_SIZE_PX);
        let resolved_font_px = resolve_font_size_px(&style.text.font_size, parent_font_px, compute_ctx);
        let is_root = parent_style.is_none();

        let late_ctx = ComputeContext {
            horizontal_writing_mode: matches!(
                style.text.writing_mode,
                kozan_style::WritingMode::HorizontalTb
            ),
            font_size: resolved_font_px,
            root_font_size: if is_root { resolved_font_px } else { compute_ctx.root_font_size },
            current_color: style.text.color,
            inherited_color: parent_style
                .map(|p| p.text.color)
                .unwrap_or(compute_ctx.inherited_color),
            ..*compute_ctx
        };

        // Phase 3: Apply late properties (two-pass: normal + !important).
        self.apply_late_properties(
            &decls, rules, &mut style, parent_style, &late_ctx,
            &custom_props, &attr_lookup,
        );

        // Apply inline late properties (highest specificity, after cascade).
        if let Some(inline) = inline_style {
            for (prop_decl, _imp) in inline.entries() {
                if !is_early_property(prop_decl) && !matches!(prop_decl, PropertyDeclaration::Custom { .. }) {
                    style.apply_declaration(prop_decl, parent_style, &late_ctx);
                }
            }
        }

        let result = Arc::new(ResolvedStyle { style, custom_properties: custom_props });

        // Cache the result for future elements matching the same rules + parent.
        // Arc clone is a cheap pointer bump — no deep copy of ComputedStyle.
        if inline_style.is_none() {
            self.mpc.insert(cache_key, Arc::clone(&result));
        }

        // Insert into sharing cache (L1) so similar elements skip everything.
        if let Some(sk) = sharing_key {
            self.sharing_cache.insert(sk, Arc::clone(&result));
        }

        result
    }

    /// Apply early properties: direction, writing-mode, font-size, color.
    ///
    /// Handles `revert`/`revert-layer` with the same fallback map pattern as
    /// `apply_late_properties()` so that `direction: revert` works correctly.
    fn apply_early_properties(
        &self,
        decls: &[ApplicableDeclaration],
        rules: &[crate::stylist::IndexedRule],
        style: &mut ComputedStyle,
        parent: Option<&ComputedStyle>,
        ctx: &ComputeContext,
    ) {
        use kozan_selector::fxhash::FxHashMap;

        let has_any_revert = decls.iter().any(|d| {
            let rule = &rules[d.rule_index as usize];
            rule.declarations.entries().iter().any(|(pd, _)|
                is_early_property(pd) && (pd.is_revert() || pd.is_revert_layer())
            )
        });

        let mut origin_fallbacks: Option<FxHashMap<(u16, u8), PropertyDeclaration>> =
            if has_any_revert { Some(FxHashMap::default()) } else { None };
        let mut layer_fallbacks: Option<FxHashMap<(u16, u8), std::collections::BTreeMap<u16, PropertyDeclaration>>> =
            if has_any_revert { Some(FxHashMap::default()) } else { None };

        cascade::cascade_apply(decls, rules, |rule, _level, importance| {
            for (prop_decl, decl_imp) in rule.declarations.entries() {
                if *decl_imp != importance { continue; }
                if !is_early_property(prop_decl) { continue; }

                let is_revert = prop_decl.is_revert();
                let is_revert_layer = prop_decl.is_revert_layer();

                if is_revert || is_revert_layer {
                    let prop_id = prop_decl.id() as u16;
                    if is_revert {
                        let origin = rule.origin as u8;
                        let map = origin_fallbacks.get_or_insert_with(FxHashMap::default);
                        let mut found = false;
                        for o in (0..origin).rev() {
                            if let Some(fallback) = map.get(&(prop_id, o)) {
                                style.apply_declaration(fallback, parent, ctx);
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            style.apply_declaration(prop_decl, parent, ctx);
                        }
                    } else {
                        let origin = rule.origin as u8;
                        let layer = rule.layer_order;
                        let map = layer_fallbacks.get_or_insert_with(FxHashMap::default);
                        let found = map.get(&(prop_id, origin))
                            .and_then(|tree| tree.range(..layer).next_back())
                            .map(|(_, decl)| decl);
                        if let Some(fallback) = found {
                            style.apply_declaration(fallback, parent, ctx);
                        } else {
                            style.apply_declaration(prop_decl, parent, ctx);
                        }
                    }
                    continue;
                }

                style.apply_declaration(prop_decl, parent, ctx);

                let prop_id = prop_decl.id() as u16;
                let origin = rule.origin as u8;
                let layer = rule.layer_order;
                if let Some(map) = origin_fallbacks.as_mut() {
                    map.insert((prop_id, origin), prop_decl.clone());
                }
                if let Some(map) = layer_fallbacks.as_mut() {
                    map.entry((prop_id, origin))
                        .or_insert_with(std::collections::BTreeMap::new)
                        .insert(layer, prop_decl.clone());
                }
            }
        });
    }

    /// Apply late properties: everything except early + custom.
    ///
    /// Handles `revert` and `revert-layer` via a pre-built per-property history
    /// that tracks the winning declaration at each (origin, layer) level. When a
    /// `revert` or `revert-layer` is encountered, the fallback is an O(1) lookup
    /// instead of an O(n) backward scan.
    fn apply_late_properties(
        &self,
        decls: &[ApplicableDeclaration],
        rules: &[crate::stylist::IndexedRule],
        style: &mut ComputedStyle,
        parent: Option<&ComputedStyle>,
        ctx: &ComputeContext,
        custom_props: &CustomPropertyMap,
        attr_lookup: &impl Fn(&str) -> Option<String>,
    ) {
        // Pre-build revert fallback index: for each property, track the last
        // non-revert declaration at each origin (for `revert`) and at each
        // (origin, layer) pair (for `revert-layer`).
        //
        // Key insight: declarations are sorted by cascade priority (ascending).
        // As we walk them, each new non-revert value for a property at origin O
        // becomes the fallback for any future `revert` from origin O+1.
        //
        // We store: per_origin[property_id][origin] = last non-revert decl
        //           per_layer[property_id][(origin, layer)] = last non-revert decl
        //
        // PropertyId is repr(u16), so we use a flat Vec indexed by property ID.
        // Origin has 3 values, layers are sparse → use SmallVec/HashMap.

        // Track per-property, per-origin winning declarations for revert fallback.
        // origin_winners[prop_id as usize] = [Option<&PD>; 3] (UA=0, User=1, Author=2)
        use kozan_selector::fxhash::FxHashMap;

        // Fast path: check if any matched rule contains revert/revert-layer.
        // If not (typical case), skip all fallback tracking — zero overhead.
        let has_any_revert = decls.iter().any(|d| {
            let rule = &rules[d.rule_index as usize];
            rule.declarations.entries().iter().any(|(pd, _)| pd.is_revert() || pd.is_revert_layer())
        });

        // Per-property fallback maps, only allocated when revert is present.
        let mut origin_fallbacks: Option<FxHashMap<(u16, u8), PropertyDeclaration>> =
            if has_any_revert { Some(FxHashMap::default()) } else { None };
        // Layer fallbacks: (prop_id, origin) → BTreeMap<layer_order, decl>.
        // BTreeMap gives O(log n) range query for "highest layer below current".
        let mut layer_fallbacks: Option<FxHashMap<(u16, u8), std::collections::BTreeMap<u16, PropertyDeclaration>>> =
            if has_any_revert { Some(FxHashMap::default()) } else { None };

        // Reusable buffers for var()/env()/attr() substitution — allocated once,
        // grow to peak capacity and are reused for every property in this cascade.
        let mut sub_out = String::new();
        let mut sub_scratch = String::new();

        cascade::cascade_apply(decls, rules, |rule, _level, importance| {
            for (prop_decl, decl_imp) in rule.declarations.entries() {
                if *decl_imp != importance { continue; }
                if is_early_property(prop_decl) { continue; }
                if matches!(prop_decl, PropertyDeclaration::Custom { .. }) { continue; }

                // WithVariables: substitute + re-parse + apply.
                if prop_decl.has_variables() {
                    if let Some(unparsed) = prop_decl.unparsed_css() {
                        let prop_id = prop_decl.id();
                        if custom_properties::substitute_with_buf(
                            unparsed.css.as_ref(), custom_props, &self.env_values, attr_lookup,
                            &mut sub_out, &mut sub_scratch,
                        ) {
                            if let Some(resolved) = kozan_css::parse_value(prop_id, &sub_out) {
                                style.apply_declaration(&resolved, parent, ctx);
                            }
                        }
                    }
                    continue;
                }

                let is_revert = prop_decl.is_revert();
                let is_revert_layer = prop_decl.is_revert_layer();

                if is_revert || is_revert_layer {
                    let prop_id = prop_decl.id() as u16;

                    if is_revert {
                        // Find fallback from a strictly lower origin.
                        let origin = rule.origin as u8;
                        let map = origin_fallbacks.get_or_insert_with(FxHashMap::default);
                        // Search origins below current (e.g. Author=2 → try User=1, then UA=0)
                        let mut found = false;
                        for o in (0..origin).rev() {
                            if let Some(fallback) = map.get(&(prop_id, o)) {
                                style.apply_declaration(fallback, parent, ctx);
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            // No lower origin has this property → treat as unset
                            style.apply_declaration(prop_decl, parent, ctx);
                        }
                    } else {
                        // revert-layer: find fallback from same origin, strictly lower layer.
                        // O(log n) via BTreeMap range query.
                        let origin = rule.origin as u8;
                        let layer = rule.layer_order;
                        let map = layer_fallbacks.get_or_insert_with(FxHashMap::default);
                        let found = map.get(&(prop_id, origin))
                            .and_then(|tree| tree.range(..layer).next_back())
                            .map(|(_, decl)| decl);
                        if let Some(fallback) = found {
                            style.apply_declaration(fallback, parent, ctx);
                        } else {
                            // No lower layer → treat as unset
                            style.apply_declaration(prop_decl, parent, ctx);
                        }
                    }
                    continue;
                }

                // Non-revert declaration: apply it and record in fallback maps.
                style.apply_declaration(prop_decl, parent, ctx);

                // Record this value as potential revert fallback (lazy — only if
                // maps were initialized, meaning we've seen at least one revert).
                let prop_id = prop_decl.id() as u16;
                let origin = rule.origin as u8;
                let layer = rule.layer_order;
                if let Some(map) = origin_fallbacks.as_mut() {
                    map.insert((prop_id, origin), prop_decl.clone());
                }
                if let Some(map) = layer_fallbacks.as_mut() {
                    map.entry((prop_id, origin))
                        .or_insert_with(std::collections::BTreeMap::new)
                        .insert(layer, prop_decl.clone());
                }
            }
        });
    }

    /// Check if any ANCESTOR of the element matches any selector in the list.
    /// Used for @scope root: element must be inside (descendant of) a scope root.
    fn ancestor_matches<E: Element>(element: &E, list: &kozan_selector::SelectorList) -> bool {
        let mut current = element.parent_element();
        while let Some(ancestor) = current {
            for sel in &list.0 {
                if kozan_selector::matching::matches(sel, &ancestor) {
                    return true;
                }
            }
            current = ancestor.parent_element();
        }
        false
    }

    /// Check if the element itself OR any ancestor matches any selector in the list.
    /// Used for @scope limit: element must not be at or below a limit boundary.
    fn self_or_ancestor_matches<E: Element>(element: &E, list: &kozan_selector::SelectorList) -> bool {
        // Check self first.
        for sel in &list.0 {
            if kozan_selector::matching::matches(sel, element) {
                return true;
            }
        }
        Self::ancestor_matches(element, list)
    }
}

/// Check if a property is "early" — must resolve before late properties.
#[inline]
fn is_early_property(decl: &PropertyDeclaration) -> bool {
    matches!(
        decl,
        PropertyDeclaration::Direction(_)
        | PropertyDeclaration::WritingMode(_)
        | PropertyDeclaration::TextOrientation(_)
        | PropertyDeclaration::FontSize(_)
        | PropertyDeclaration::Color(_)
    )
}

/// Resolve a FontSize enum to px for the ComputeContext.
///
/// Uses the REAL `ComputeContext` from the resolver (correct viewport, root
/// font-size, font metrics) — only overrides `font_size` with parent's value
/// so `em` units resolve against the parent, not the element itself.
fn resolve_font_size_px(
    fs: &kozan_style::FontSize,
    parent_px: f32,
    base_ctx: &ComputeContext,
) -> f32 {
    use kozan_style::{FontSize, ToComputedValue};
    use kozan_style::specified::LengthPercentage;

    match fs {
        FontSize::XxSmall  => FONT_SIZE_KEYWORDS_PX[0],
        FontSize::XSmall   => FONT_SIZE_KEYWORDS_PX[1],
        FontSize::Small    => FONT_SIZE_KEYWORDS_PX[2],
        FontSize::Medium   => FONT_SIZE_KEYWORDS_PX[3],
        FontSize::Large    => FONT_SIZE_KEYWORDS_PX[4],
        FontSize::XLarge   => FONT_SIZE_KEYWORDS_PX[5],
        FontSize::XxLarge  => FONT_SIZE_KEYWORDS_PX[6],
        FontSize::XxxLarge => FONT_SIZE_KEYWORDS_PX[7],
        FontSize::Smaller  => (parent_px * FONT_SIZE_SMALLER_RATIO).round(),
        FontSize::Larger   => (parent_px * FONT_SIZE_LARGER_RATIO).round(),
        FontSize::Math     => parent_px,
        FontSize::LengthPercentage(lp) => {
            // Real context with parent's font-size for em, but correct viewport,
            // root_font_size, and font metrics from the actual device.
            let font_ctx = ComputeContext {
                font_size: parent_px,
                ..*base_ctx
            };
            match lp {
                LengthPercentage::Length(l) => (*l).to_computed_value(&font_ctx).px(),
                LengthPercentage::Percentage(p) => parent_px * p.value(),
                LengthPercentage::Calc(calc) => {
                    // Resolve calc with full context. Percentages in font-size
                    // are relative to parent's font-size (W3C CSS Fonts §3.5).
                    let specified = LengthPercentage::Calc(calc.clone());
                    let computed = specified.to_computed_value(&font_ctx);
                    computed.resolve(kozan_style::computed::Length::new(parent_px)).px()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_css::parse_stylesheet;
    use crate::device::Device;
    use crate::origin::CascadeOrigin;

    fn stylist_with_css(css: &str) -> Stylist {
        let sheet = parse_stylesheet(css);
        let mut stylist = Stylist::new(Device::new(1024.0, 768.0));
        stylist.add_stylesheet(sheet, CascadeOrigin::Author);
        stylist.rebuild();
        stylist
    }

    #[derive(Clone)]
    struct TestEl {
        tag: Atom,
        id: Option<Atom>,
        classes: Vec<Atom>,
    }

    impl TestEl {
        fn new(tag: &str) -> Self {
            Self { tag: Atom::from(tag), id: None, classes: Vec::new() }
        }
        fn with_class(mut self, class: &str) -> Self {
            self.classes.push(Atom::from(class));
            self
        }
    }

    impl kozan_selector::element::Element for TestEl {
        fn local_name(&self) -> &Atom { &self.tag }
        fn id(&self) -> Option<&Atom> { self.id.as_ref() }
        fn has_class(&self, class: &Atom) -> bool { self.classes.iter().any(|c| c == class) }
        fn each_class<F: FnMut(&Atom)>(&self, mut f: F) {
            for c in &self.classes { f(c); }
        }
        fn attr(&self, _: &Atom) -> Option<&str> { None }
        fn parent_element(&self) -> Option<Self> { None }
        fn prev_sibling_element(&self) -> Option<Self> { None }
        fn next_sibling_element(&self) -> Option<Self> { None }
        fn first_child_element(&self) -> Option<Self> { None }
        fn last_child_element(&self) -> Option<Self> { None }
        fn state(&self) -> kozan_selector::pseudo_class::ElementState {
            kozan_selector::pseudo_class::ElementState::empty()
        }
        fn is_root(&self) -> bool { false }
        fn is_empty(&self) -> bool { true }
        fn child_index(&self) -> u32 { 1 }
        fn child_count(&self) -> u32 { 1 }
        fn child_index_of_type(&self) -> u32 { 1 }
        fn child_count_of_type(&self) -> u32 { 1 }
        fn opaque(&self) -> kozan_selector::opaque::OpaqueElement {
            kozan_selector::opaque::OpaqueElement::new(0)
        }
    }

    #[test]
    fn resolve_basic_style() {
        let stylist = stylist_with_css(".btn { display: flex }");
        let el = TestEl::new("div").with_class("btn");
        let ctx = ComputeContext::default();
        let env = EnvironmentValues::empty();
        let mut resolver = StyleResolver::new(env);

        let result = resolver.resolve(&el, &stylist, None, None, None, &ctx, |_| None);
        assert_eq!(result.style.layout.display, kozan_style::Display::Flex);
    }

    #[test]
    fn resolve_inherits_color() {
        let stylist = stylist_with_css("div { color: rgb(255, 0, 0) }");
        let parent_el = TestEl::new("div");
        let ctx = ComputeContext::default();
        let env = EnvironmentValues::empty();
        let mut resolver = StyleResolver::new(env);

        let parent = resolver.resolve(&parent_el, &stylist, None, None, None, &ctx, |_| None);
        let child_stylist = stylist_with_css("");
        let child = resolver.resolve(
            &TestEl::new("span"), &child_stylist,
            Some(&parent.style), Some(&parent.custom_properties), None, &ctx, |_| None,
        );
        assert_eq!(child.style.text.color, parent.style.text.color);
    }

    #[test]
    fn resolve_does_not_inherit_display() {
        let stylist = stylist_with_css("div { display: flex }");
        let ctx = ComputeContext::default();
        let env = EnvironmentValues::empty();
        let mut resolver = StyleResolver::new(env);

        let parent = resolver.resolve(&TestEl::new("div"), &stylist, None, None, None, &ctx, |_| None);
        let empty = stylist_with_css("");
        let child = resolver.resolve(
            &TestEl::new("span"), &empty,
            Some(&parent.style), None, None, &ctx, |_| None,
        );
        assert_eq!(child.style.layout.display, kozan_style::Display::Inline);
    }

    #[test]
    fn resolve_specificity_wins() {
        let stylist = stylist_with_css(".a { display: block } .a.b { display: flex }");
        let el = TestEl::new("div").with_class("a").with_class("b");
        let ctx = ComputeContext::default();
        let mut resolver = StyleResolver::new(EnvironmentValues::empty());
        let r = resolver.resolve(&el, &stylist, None, None, None, &ctx, |_| None);
        assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
    }

    #[test]
    fn resolve_source_order_wins() {
        let stylist = stylist_with_css(".x { display: block } .x { display: flex }");
        let el = TestEl::new("div").with_class("x");
        let ctx = ComputeContext::default();
        let mut resolver = StyleResolver::new(EnvironmentValues::empty());
        let r = resolver.resolve(&el, &stylist, None, None, None, &ctx, |_| None);
        assert_eq!(r.style.layout.display, kozan_style::Display::Flex);
    }

    #[test]
    fn resolve_no_rules_initial() {
        let stylist = stylist_with_css(".nomatch { display: flex }");
        let el = TestEl::new("div");
        let ctx = ComputeContext::default();
        let mut resolver = StyleResolver::new(EnvironmentValues::empty());
        let r = resolver.resolve(&el, &stylist, None, None, None, &ctx, |_| None);
        assert_eq!(r.style.layout.display, kozan_style::Display::Inline);
    }

    #[test]
    fn font_size_keyword_table_values() {
        let ctx = ComputeContext::default();
        assert_eq!(resolve_font_size_px(&kozan_style::FontSize::Medium, 16.0, &ctx), INITIAL_FONT_SIZE_PX);
        assert_eq!(resolve_font_size_px(&kozan_style::FontSize::Small, 16.0, &ctx), 13.0);
        assert_eq!(resolve_font_size_px(&kozan_style::FontSize::XxLarge, 16.0, &ctx), 32.0);
    }

    #[test]
    fn font_size_smaller_larger_relative() {
        let ctx = ComputeContext::default();
        let parent = 16.0;
        let smaller = resolve_font_size_px(&kozan_style::FontSize::Smaller, parent, &ctx);
        let larger = resolve_font_size_px(&kozan_style::FontSize::Larger, parent, &ctx);
        assert!(smaller < parent, "smaller must be less than parent");
        assert!(larger > parent, "larger must be greater than parent");
    }
}
