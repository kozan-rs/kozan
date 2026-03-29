//! Right-to-left selector matching engine.
//!
//! Three API levels with increasing capability:
//!
//! 1. `matches(selector, element)` — No context. For tests, simple queries.
//! 2. `matches_with_bloom(selector, element, bloom)` — Just bloom filter.
//! 3. `matches_in_context(selector, element, ctx)` — Full context: bloom,
//!    scope, visited handling, nth caching, quirks mode, selector flags.
//!
//! Performance design:
//! - State-based pseudo-classes: single AND instruction via `state_flag()`
//! - Atom comparison: O(1) pointer equality (`Arc::ptr_eq`)
//! - Bloom filter: pre-filters ancestor chain before walking
//! - NthIndexCache: O(1) repeated nth lookups during restyle
//! - `#[inline]` on all hot-path functions for monomorphization
//! - Zero allocation during matching (iterative :has traversal)

use kozan_atom::Atom;
use smallvec::SmallVec;

use crate::bloom::AncestorBloom;
use crate::context::{MatchingContext, QuirksMode, VisitedHandling};
use crate::element::Element;
use crate::flags::ElementSelectorFlags;
use crate::pseudo_class::{ElementState, PseudoClass};
use crate::specificity::Specificity;
use crate::types::*;

// Cached "lang" atom — hits css-atoms static lookup via Atom::new(), zero allocation.
static LANG_ATOM: std::sync::LazyLock<Atom> = std::sync::LazyLock::new(|| Atom::from("lang"));

/// Returns `true` if the selector matches the given element.
///
/// Zero-context matching. Uses pre-computed `SelectorHints` for fast rejection.
/// No bloom filter, no scope, no caching. Good for tests and simple queries.
#[inline]
pub fn matches<E: Element>(selector: &Selector, element: &E) -> bool {
    let hints = selector.hints();
    if !hints.required_state.is_empty() && !element.state().contains(hints.required_state) {
        return false;
    }
    matches_components(selector.components(), element, None, None)
}

/// Returns `true` if the selector matches, using a Bloom filter for ancestor pruning.
#[inline]
pub fn matches_with_bloom<E: Element>(
    selector: &Selector,
    element: &E,
    bloom: &AncestorBloom,
) -> bool {
    let hints = selector.hints();
    if !hints.required_state.is_empty() && !element.state().contains(hints.required_state) {
        return false;
    }
    matches_components(selector.components(), element, Some(bloom), None)
}

/// Full-featured matching with `MatchingContext`.
///
/// Supports bloom filter, `:scope`, `:visited` handling, quirks mode,
/// nth-index caching, and selector flag collection.
#[inline]
pub fn matches_in_context<E: Element>(
    selector: &Selector,
    element: &E,
    ctx: &mut MatchingContext,
) -> bool {
    let hints = selector.hints();
    if !hints.required_state.is_empty() && !element.state().contains(hints.required_state) {
        return false;
    }
    matches_components(selector.components(), element, ctx.bloom_filter, Some(ctx))
}

/// Returns the highest specificity from any matching selector in the list, or `None`.
pub fn matches_selector_list<E: Element>(
    list: &SelectorList,
    element: &E,
) -> Option<Specificity> {
    let mut best: Option<Specificity> = None;
    for selector in &list.0 {
        if matches(selector, element) {
            let spec = selector.specificity();
            best = Some(match best {
                Some(prev) => spec.max(prev),
                None => spec,
            });
        }
    }
    best
}

/// Walk components right-to-left, matching simple selectors and handling combinators.
///
/// Context is threaded through the entire matching chain via `Option<&mut MatchingContext>`.
/// Each recursive call uses `ctx.as_deref_mut()` to reborrow without consuming ownership.
#[inline]
fn matches_components<E: Element>(
    components: &[Component],
    element: &E,
    bloom: Option<&AncestorBloom>,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    let mut i = 0;
    while i < components.len() {
        match &components[i] {
            Component::Combinator(comb) => {
                return match_combinator(*comb, &components[i + 1..], element, bloom, ctx);
            }
            comp => {
                if !matches_simple(comp, element, bloom, ctx.as_deref_mut()) {
                    return false;
                }
                i += 1;
            }
        }
    }
    true
}

/// Match a combinator by navigating to the appropriate relative element(s)
/// and recursively matching the remaining components.
///
/// Context is fully threaded — ancestor/sibling matching can use nth caches,
/// quirks mode, scope element, and selector flag collection.
fn match_combinator<E: Element>(
    combinator: Combinator,
    remaining: &[Component],
    element: &E,
    bloom: Option<&AncestorBloom>,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    match combinator {
        Combinator::Child => match element.parent_element() {
            Some(parent) => matches_components(remaining, &parent, bloom, ctx),
            None => false,
        },
        Combinator::Descendant => {
            if let Some(bloom) = bloom {
                if !bloom_might_match(remaining, bloom) {
                    return false;
                }
            }
            let mut ancestor = element.parent_element();
            while let Some(anc) = ancestor {
                if matches_components(remaining, &anc, bloom, ctx.as_deref_mut()) {
                    return true;
                }
                ancestor = anc.parent_element();
            }
            false
        }
        Combinator::NextSibling => match element.prev_sibling_element() {
            Some(prev) => matches_components(remaining, &prev, bloom, ctx),
            None => false,
        },
        Combinator::LaterSibling => {
            let mut sibling = element.prev_sibling_element();
            while let Some(sib) = sibling {
                if matches_components(remaining, &sib, bloom, ctx.as_deref_mut()) {
                    return true;
                }
                sibling = sib.prev_sibling_element();
            }
            false
        }
        Combinator::Column => match element.column_element() {
            Some(col) => matches_components(remaining, &col, bloom, ctx),
            None => false,
        }
    }
}

/// Bloom filter pre-check: can the next compound selector possibly match any ancestor?
#[inline]
fn bloom_might_match(components: &[Component], bloom: &AncestorBloom) -> bool {
    for comp in components {
        match comp {
            Component::Combinator(_) => break,
            Component::Type(atom) | Component::Id(atom) | Component::Class(atom) => {
                if !bloom.might_contain(AncestorBloom::hash_atom(atom)) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// Match a single simple selector component against an element.
///
/// This is the innermost hot loop of the selector engine. Every instruction counts.
/// Context is threaded for nth caching, quirks mode, scope, visited, and flag collection.
#[inline]
fn matches_simple<E: Element>(
    component: &Component,
    element: &E,
    bloom: Option<&AncestorBloom>,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    match component {
        Component::Universal => true,
        Component::Type(name) => element.local_name() == name,
        Component::Id(id) => element.id().is_some_and(|eid| eid == id),
        Component::Namespace(ns) => match &**ns {
            NamespaceConstraint::Any => true,
            NamespaceConstraint::None => element.namespace().is_none(),
            NamespaceConstraint::Specific(uri) => {
                element.namespace().is_some_and(|ns| ns == uri)
            }
        },
        Component::Class(class) => match_class(class, element, ctx),
        Component::Attribute(attr) => attr.matches(element.attr(&attr.name)),
        Component::PseudoClass(pc) => matches_pseudo_class(pc, element, ctx),
        Component::PseudoElement(_) => true,
        Component::Negation(list) => !list.0.iter().any(|sel| {
            match_sub_selector(sel, element, bloom, ctx.as_deref_mut())
        }),
        Component::Is(list) | Component::Where(list) => {
            list.0.iter().any(|sel| {
                match_sub_selector(sel, element, bloom, ctx.as_deref_mut())
            })
        }
        // Flattened fast paths — contiguous component arrays, zero pointer chasing.
        // Handles any single-component sub-selectors: class, type, id, pseudo-class, mixed.
        Component::IsSingle(comps) | Component::WhereSingle(comps) => {
            comps.slice.iter().any(|c| matches_simple(c, element, bloom, ctx.as_deref_mut()))
        }
        Component::NotSingle(comps) => {
            comps.slice.iter().all(|c| !matches_simple(c, element, bloom, ctx.as_deref_mut()))
        }
        Component::Has(rel_list) => {
            // Set flag: this element anchors a :has() relative selector.
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::ANCHORS_RELATIVE_SELECTOR);
            }
            matches_has(rel_list, element, bloom, ctx)
        }
        Component::NthChild(nth) => matches_nth(nth, element, false, ctx),
        Component::NthLastChild(nth) => matches_nth(nth, element, true, ctx),
        Component::NthOfType(a, b) => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH);
            }
            let index = match ctx {
                Some(ref mut c) => {
                    c.caches.nth.nth_of_type(element.opaque(), || element.child_index_of_type() as i32)
                }
                None => element.child_index_of_type() as i32,
            };
            NthData::formula(*a, *b, index)
        }
        Component::NthLastOfType(a, b) => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH);
            }
            let index = match ctx {
                Some(ref mut c) => {
                    c.caches.nth.nth_last_of_type(element.opaque(), || {
                        element.child_count_of_type() as i32 + 1 - element.child_index_of_type() as i32
                    })
                }
                None => {
                    element.child_count_of_type() as i32 + 1 - element.child_index_of_type() as i32
                }
            };
            NthData::formula(*a, *b, index)
        }
        Component::Lang(langs) => matches_lang(langs, element),
        Component::Dir(dir) => element.direction() == *dir,
        Component::Nesting => {
            // `&` is equivalent to `:scope` in matching.
            match ctx {
                Some(ref c) => c.scope_element
                    .map_or(element.is_root(), |scope| element.opaque() == scope),
                None => element.is_root(),
            }
        }
        Component::State(name) => element.has_custom_state(name),
        Component::Host => element.is_shadow_host(),
        Component::HostFunction(list) => {
            element.is_shadow_host() && list.0.iter().any(|sel| {
                matches_in_list(sel, element, bloom, ctx.as_deref_mut())
            })
        }
        Component::HostContext(list) => {
            if !element.is_shadow_host() {
                return false;
            }
            // :host-context(.foo) — check the host itself, then walk ancestors
            // across shadow boundaries (via containing_shadow_host).
            let mut current = Some(element.clone());
            while let Some(el) = current {
                if list.0.iter().any(|sel| {
                    matches_in_list(sel, &el, bloom, ctx.as_deref_mut())
                }) {
                    return true;
                }
                // Walk up: first try normal parent, then cross shadow boundary.
                current = el.parent_element().or_else(|| el.containing_shadow_host());
            }
            false
        }
        Component::Slotted(list) => {
            element.is_in_slot() && list.0.iter().any(|sel| {
                matches_in_list(sel, element, bloom, ctx.as_deref_mut())
            })
        }
        Component::Part(parts) => parts.iter().all(|part| element.is_part(part)),
        Component::Highlight(_) => true, // Like pseudo-elements — matched at style resolution
        Component::Combinator(_) => unreachable!(),
    }
}

/// Lightweight sub-selector dispatch for `:is()`, `:not()`, `:where()`.
///
/// For deeply nested functionals like `:is(:not(:is(:not(.hidden))))`, each
/// level must be as cheap as possible. This function minimizes per-level cost:
/// - Single-component sub-selector with a common leaf (class/type/id/universal):
///   direct inline match, zero indirection.
/// - Single-component sub-selector that's itself functional (Is/Not/Where):
///   direct `matches_simple` call, skipping all hint/tier overhead.
/// - Everything else: falls through to `matches_in_list` for full handling.
#[inline(always)]
fn match_sub_selector<E: Element>(
    sel: &Selector,
    element: &E,
    bloom: Option<&AncestorBloom>,
    ctx: Option<&mut MatchingContext>,
) -> bool {
    let comps = sel.components();
    if comps.len() == 1 {
        // Single-component fast path — no hints check, no tier dispatch.
        // Covers: .class, type, #id, *, and nested :is/:not/:where.
        return match &comps[0] {
            Component::Class(class) => element.has_class(class),
            Component::Type(name) => element.local_name() == name,
            Component::Id(id) => element.id().is_some_and(|eid| eid == id),
            Component::Universal => true,
            other => matches_simple(other, element, bloom, ctx),
        };
    }
    matches_in_list(sel, element, bloom, ctx)
}

/// Fast-path matching for sub-selectors inside `:host()`, `::slotted()`, etc.
/// and for multi-component sub-selectors in `:is()`, `:not()`, `:where()`.
///
/// Three tiers:
/// 1. **Single common component** (class/type/id/universal): direct inline
///    dispatch — zero function call overhead.
/// 2. **Compound-only** (no combinators): match components inline, bypassing
///    the `matches_components` loop and combinator-checking overhead.
/// 3. **Has combinators**: full `matches_components` right-to-left matching.
///
/// Also pre-rejects via `required_state` hints (1 AND instruction).
#[inline(always)]
fn matches_in_list<E: Element>(
    sel: &Selector,
    element: &E,
    bloom: Option<&AncestorBloom>,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    // Pre-reject: if the sub-selector requires state bits the element lacks, skip.
    let required = sel.hints().required_state;
    if !required.is_empty() && !element.state().contains(required) {
        return false;
    }

    let comps = sel.components();

    // Tier 1: single simple selector — bypass matches_simple entirely for
    // the 4 most common cases.
    if comps.len() == 1 {
        return match &comps[0] {
            Component::Class(class) => element.has_class(class),
            Component::Type(name) => element.local_name() == name,
            Component::Id(id) => element.id().is_some_and(|eid| eid == id),
            Component::Universal => true,
            other => matches_simple(other, element, bloom, ctx),
        };
    }

    // Tier 2: compound-only (no combinators) — match inline, skip
    // matches_components loop overhead.
    if !sel.hints().deps.has_combinators() {
        return comps.iter().all(|c| match c {
            Component::Class(class) => element.has_class(class),
            Component::Type(name) => element.local_name() == name,
            Component::Id(id) => element.id().is_some_and(|eid| eid == id),
            Component::Universal => true,
            other => matches_simple(other, element, bloom, ctx.as_deref_mut()),
        });
    }

    // Tier 3: has combinators — full right-to-left matching
    matches_components(comps, element, bloom, ctx)
}

/// Class matching — respects quirks mode case-insensitivity.
#[inline]
fn match_class<E: Element>(
    class: &Atom,
    element: &E,
    ctx: Option<&mut MatchingContext>,
) -> bool {
    let quirks = ctx
        .as_ref()
        .map(|c| c.quirks_mode)
        .unwrap_or(QuirksMode::NoQuirks);

    if quirks.classes_case_insensitive() {
        // Quirks mode: case-insensitive class matching.
        let target = class.as_ref();
        let mut found = false;
        element.each_class(|c| {
            if !found && c.as_ref().eq_ignore_ascii_case(target) {
                found = true;
            }
        });
        found
    } else {
        // Standards mode: Atom pointer equality (O(1)).
        element.has_class(class)
    }
}

/// Pseudo-class matching with state_flag() fast path and context support.
///
/// State-based pseudo-classes (20+ of 30+) are matched via a single AND
/// instruction through the `state_flag()` lookup. Structural pseudo-classes
/// require tree queries and set invalidation flags. Context-dependent
/// pseudo-classes use MatchingContext.
#[inline]
fn matches_pseudo_class<E: Element>(
    pc: &PseudoClass,
    element: &E,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    if let Some(flag) = pc.state_flag() {
        // Handle :visited/:link with privacy-aware visited handling when context present.
        if matches!(pc, PseudoClass::Visited | PseudoClass::Link) && ctx.is_some() {
            return match_visited_link(pc, element, ctx);
        }
        return element.state().contains(flag);
    }

    match pc {
        PseudoClass::AnyLink => {
            let s = element.state();
            s.intersects(ElementState::LINK | ElementState::VISITED)
        }
        PseudoClass::Root => element.is_root(),
        PseudoClass::Empty => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EMPTY_SELECTOR);
            }
            element.is_empty()
        }
        PseudoClass::FirstChild => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
            }
            element.child_index() == 1
        }
        PseudoClass::LastChild => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
            }
            element.child_index() == element.child_count()
        }
        PseudoClass::OnlyChild => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
            }
            element.child_count() == 1
        }
        PseudoClass::FirstOfType => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
            }
            element.child_index_of_type() == 1
        }
        PseudoClass::LastOfType => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
            }
            element.child_index_of_type() == element.child_count_of_type()
        }
        PseudoClass::OnlyOfType => {
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR);
            }
            element.child_count_of_type() == 1
        }
        PseudoClass::Scope => match_scope(element, ctx),
        _ => unreachable!("all pseudo-classes handled by state_flag() or above"),
    }
}

/// `:visited` / `:link` matching with privacy-aware visited handling.
///
/// Without context: standard state flag matching.
/// With context: respects `VisitedHandling` policy to prevent timing attacks.
#[inline]
fn match_visited_link<E: Element>(
    pc: &PseudoClass,
    element: &E,
    ctx: Option<&mut MatchingContext>,
) -> bool {
    let handling = ctx
        .as_ref()
        .map(|c| c.visited_handling)
        .unwrap_or(VisitedHandling::AllLinksUnvisited);

    let state = element.state();
    match handling {
        VisitedHandling::AllLinksUnvisited => {
            // Privacy mode: treat all links as unvisited.
            // :link matches everything with LINK, :visited never matches.
            match pc {
                PseudoClass::Link => state.contains(ElementState::LINK),
                PseudoClass::Visited => false,
                _ => unreachable!(),
            }
        }
        VisitedHandling::AllLinksVisitedAndUnvisited => {
            // Both match — used during invalidation to be conservative.
            match pc {
                PseudoClass::Link | PseudoClass::Visited => {
                    state.intersects(ElementState::LINK | ElementState::VISITED)
                }
                _ => unreachable!(),
            }
        }
        VisitedHandling::RelevantLinkVisited => {
            // The "relevant link" is visited; others are unvisited.
            let is_relevant = ctx
                .and_then(|c| c.relevant_link)
                .is_some_and(|rl| rl == element.opaque());

            if is_relevant {
                match pc {
                    PseudoClass::Visited => state.contains(ElementState::VISITED),
                    PseudoClass::Link => false,
                    _ => unreachable!(),
                }
            } else {
                match pc {
                    PseudoClass::Link => state.contains(ElementState::LINK),
                    PseudoClass::Visited => false,
                    _ => unreachable!(),
                }
            }
        }
    }
}

/// `:scope` matching.
///
/// With context: matches the scope element from `MatchingContext`.
/// Without context: matches `:root` (spec default fallback).
#[inline]
fn match_scope<E: Element>(element: &E, ctx: Option<&mut MatchingContext>) -> bool {
    match ctx.and_then(|c| c.scope_element) {
        Some(scope) => element.opaque() == scope,
        None => element.is_root(),
    }
}

/// `:nth-child()` / `:nth-last-child()` matching with caching and flag collection.
///
/// When a `MatchingContext` is present:
/// - Sets `HAS_SLOW_SELECTOR_NTH` (or `_NTH_OF` for filtered variants)
/// - Uses `NthIndexCache` for O(1) repeated lookups
#[inline]
fn matches_nth<E: Element>(
    nth: &NthData,
    element: &E,
    from_end: bool,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    match nth.of_selector {
        None => {
            // Set invalidation flag.
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH);
            }

            let index = match (&mut ctx, from_end) {
                (Some(c), false) => {
                    c.caches.nth.nth_child(element.opaque(), || element.child_index() as i32)
                }
                (Some(c), true) => {
                    c.caches.nth.nth_last_child(element.opaque(), || {
                        element.child_count() as i32 + 1 - element.child_index() as i32
                    })
                }
                (None, false) => element.child_index() as i32,
                (None, true) => element.child_count() as i32 + 1 - element.child_index() as i32,
            };
            nth.matches_index(index)
        }
        Some(ref of_sel) => {
            // :nth-child(... of <selector>) — more expensive invalidation.
            if let Some(ref mut c) = ctx {
                c.add_element_flag(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH_OF);
            }

            // Element must itself match the filter selector.
            if !of_sel.0.iter().any(|sel| matches(sel, element)) {
                return false;
            }

            // Use cache with selector pointer as key discriminator.
            let selector_key = &**of_sel as *const SelectorList as usize;
            let index = match (&mut ctx, from_end) {
                (Some(c), false) => {
                    let opaque = element.opaque();
                    let of = of_sel;
                    c.caches.nth.nth_child_of(opaque, selector_key, || {
                        nth_filtered_index_compute(element, of, false)
                    })
                }
                (Some(c), true) => {
                    let opaque = element.opaque();
                    let of = of_sel;
                    c.caches.nth.nth_last_child_of(opaque, selector_key, || {
                        nth_filtered_index_compute(element, of, true)
                    })
                }
                (None, _) => nth_filtered_index_compute(element, of_sel, from_end),
            };
            nth.matches_index(index)
        }
    }
}

/// Compute the 1-based index of `element` among siblings matching `selector`.
fn nth_filtered_index_compute<E: Element>(
    element: &E,
    of_sel: &SelectorList,
    from_end: bool,
) -> i32 {
    let mut count = 1i32;
    let mut sibling = if from_end {
        element.next_sibling_element()
    } else {
        element.prev_sibling_element()
    };
    while let Some(sib) = sibling {
        if of_sel.0.iter().any(|sel| matches(sel, &sib)) {
            count += 1;
        }
        sibling = if from_end {
            sib.next_sibling_element()
        } else {
            sib.prev_sibling_element()
        };
    }
    count
}

/// `:lang()` matching — walk ancestors looking for a `lang` attribute.
/// Supports comma-separated language list: `:lang(en, fr, zh)`.
fn matches_lang<E: Element>(langs: &[Atom], element: &E) -> bool {
    let lang_attr = &*LANG_ATOM;
    let mut current = Some(element.clone());
    while let Some(el) = current {
        if let Some(value) = el.attr(lang_attr) {
            return langs.iter().any(|lang| {
                let target = lang.as_ref();
                value.eq_ignore_ascii_case(target)
                    || (value.len() > target.len()
                        && value.as_bytes()[target.len()] == b'-'
                        && value[..target.len()].eq_ignore_ascii_case(target))
            });
        }
        current = el.parent_element();
    }
    false
}

/// `:has()` matching — check descendants/siblings against relative selectors.
///
/// When a `MatchingContext` is present, results are cached in `HasCache` to
/// avoid redundant subtree walks during restyle.
fn matches_has<E: Element>(
    rel_list: &RelativeSelectorList,
    element: &E,
    _bloom: Option<&AncestorBloom>,
    mut ctx: Option<&mut MatchingContext>,
) -> bool {
    // Try the cache first.
    let selector_key = rel_list as *const RelativeSelectorList as usize;
    if let Some(ref c) = ctx {
        if let Some(result) = c.caches.has.get(element.opaque(), selector_key) {
            return result.matched();
        }
    }

    let result = matches_has_uncached(rel_list, element);

    // Store in cache.
    if let Some(ref mut c) = ctx {
        c.caches.has.insert(element.opaque(), selector_key, result.into());
    }

    result
}

/// Uncached `:has()` matching — the actual subtree/sibling walk.
fn matches_has_uncached<E: Element>(
    rel_list: &RelativeSelectorList,
    element: &E,
) -> bool {
    rel_list.0.iter().any(|rel| match rel.traversal {
        HasTraversal::Children => {
            // Only direct children — no subtree walk needed.
            let mut child = element.first_child_element();
            while let Some(ch) = child {
                if matches(&rel.selector, &ch) {
                    return true;
                }
                child = ch.next_sibling_element();
            }
            false
        }
        HasTraversal::Subtree => {
            // Full subtree walk — descendant or child-then-descendant.
            has_matching_descendant(element, &rel.selector)
        }
        HasTraversal::NextSibling => {
            // Only immediate next sibling — zero iteration.
            element
                .next_sibling_element()
                .is_some_and(|next| matches(&rel.selector, &next))
        }
        HasTraversal::Siblings => {
            // Walk all subsequent siblings.
            let mut sib = element.next_sibling_element();
            while let Some(s) = sib {
                if matches(&rel.selector, &s) {
                    return true;
                }
                sib = s.next_sibling_element();
            }
            false
        }
    })
}

/// Iterative depth-first descendant search — no recursion, no stack overflow.
fn has_matching_descendant<E: Element>(element: &E, selector: &Selector) -> bool {
    let mut worklist: SmallVec<[E; 8]> = SmallVec::new();

    let mut child = element.first_child_element();
    while let Some(ch) = child {
        worklist.push(ch.clone());
        child = ch.next_sibling_element();
    }

    while let Some(el) = worklist.pop() {
        if matches(selector, &el) {
            return true;
        }
        let mut child = el.first_child_element();
        while let Some(ch) = child {
            worklist.push(ch.clone());
            child = ch.next_sibling_element();
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opaque::OpaqueElement;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[derive(Clone)]
    struct MockElement {
        tag: Atom,
        id: Option<Atom>,
        classes: Vec<Atom>,
        attrs: Vec<(Atom, String)>,
        state: ElementState,
        parent: Option<Box<MockElement>>,
        children: Vec<MockElement>,
        prev_sibling: Option<Box<MockElement>>,
        next_sibling: Option<Box<MockElement>>,
        index: u32,
        sibling_count: u32,
        is_root: bool,
        opaque_id: u64,
        dir: Direction,
    }

    impl MockElement {
        fn new(tag: &str) -> Self {
            Self {
                tag: Atom::from(tag),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
                state: ElementState::empty(),
                parent: None,
                children: Vec::new(),
                prev_sibling: None,
                next_sibling: None,
                index: 1,
                sibling_count: 1,
                is_root: false,
                opaque_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                dir: Direction::Ltr,
            }
        }

        fn with_id(mut self, id: &str) -> Self {
            self.id = Some(Atom::from(id));
            self
        }

        fn with_class(mut self, class: &str) -> Self {
            self.classes.push(Atom::from(class));
            self
        }

        fn with_attr(mut self, name: &str, value: &str) -> Self {
            self.attrs.push((Atom::from(name), value.to_string()));
            self
        }

        fn with_state(mut self, state: ElementState) -> Self {
            self.state = state;
            self
        }

        fn with_parent(mut self, parent: MockElement) -> Self {
            self.parent = Some(Box::new(parent));
            self
        }

        fn at_index(mut self, index: u32, count: u32) -> Self {
            self.index = index;
            self.sibling_count = count;
            self
        }

        fn with_direction(mut self, dir: Direction) -> Self {
            self.dir = dir;
            self
        }
    }

    impl Element for MockElement {
        fn local_name(&self) -> &Atom { &self.tag }
        fn id(&self) -> Option<&Atom> { self.id.as_ref() }
        fn has_class(&self, class: &Atom) -> bool { self.classes.iter().any(|c| c == class) }
        fn each_class<F: FnMut(&Atom)>(&self, mut f: F) {
            for c in &self.classes { f(c); }
        }
        fn attr(&self, name: &Atom) -> Option<&str> {
            self.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
        }
        fn parent_element(&self) -> Option<Self> {
            self.parent.as_ref().map(|p| *p.clone())
        }
        fn prev_sibling_element(&self) -> Option<Self> {
            self.prev_sibling.as_ref().map(|s| *s.clone())
        }
        fn next_sibling_element(&self) -> Option<Self> {
            self.next_sibling.as_ref().map(|s| *s.clone())
        }
        fn first_child_element(&self) -> Option<Self> {
            self.children.first().cloned()
        }
        fn last_child_element(&self) -> Option<Self> {
            self.children.last().cloned()
        }
        fn state(&self) -> ElementState { self.state }
        fn is_root(&self) -> bool { self.is_root }
        fn is_empty(&self) -> bool { self.children.is_empty() }
        fn child_index(&self) -> u32 { self.index }
        fn child_count(&self) -> u32 { self.sibling_count }
        fn child_index_of_type(&self) -> u32 { self.index }
        fn child_count_of_type(&self) -> u32 { self.sibling_count }
        fn opaque(&self) -> OpaqueElement { OpaqueElement::new(self.opaque_id) }
        fn direction(&self) -> Direction { self.dir }
    }

    fn parse_and_match(css: &str, el: &MockElement) -> bool {
        let list = crate::parser::parse(css).unwrap();
        list.0.iter().any(|sel| matches(sel, el))
    }

    #[test]
    fn match_type() {
        let el = MockElement::new("div");
        assert!(parse_and_match("div", &el));
        assert!(!parse_and_match("span", &el));
    }

    #[test]
    fn match_class() {
        let el = MockElement::new("div").with_class("foo");
        assert!(parse_and_match(".foo", &el));
        assert!(!parse_and_match(".bar", &el));
    }

    #[test]
    fn match_id() {
        let el = MockElement::new("div").with_id("main");
        assert!(parse_and_match("#main", &el));
        assert!(!parse_and_match("#other", &el));
    }

    #[test]
    fn match_compound() {
        let el = MockElement::new("div").with_class("foo").with_id("bar");
        assert!(parse_and_match("div.foo#bar", &el));
        assert!(!parse_and_match("span.foo#bar", &el));
    }

    #[test]
    fn match_universal() {
        assert!(parse_and_match("*", &MockElement::new("anything")));
    }

    #[test]
    fn match_descendant() {
        let parent = MockElement::new("div").with_class("container");
        let el = MockElement::new("span").with_class("item").with_parent(parent);
        assert!(parse_and_match("div .item", &el));
        assert!(parse_and_match(".container span", &el));
        assert!(!parse_and_match("p .item", &el));
    }

    #[test]
    fn match_child() {
        let parent = MockElement::new("div");
        let el = MockElement::new("span").with_parent(parent);
        assert!(parse_and_match("div > span", &el));
    }

    #[test]
    fn match_hover() {
        let el = MockElement::new("button").with_state(ElementState::HOVER);
        assert!(parse_and_match(":hover", &el));
        assert!(parse_and_match("button:hover", &el));
        assert!(!parse_and_match(":hover", &MockElement::new("button")));
    }

    #[test]
    fn match_not() {
        let el = MockElement::new("div").with_class("foo");
        assert!(parse_and_match(":not(.bar)", &el));
        assert!(!parse_and_match(":not(.foo)", &el));
    }

    #[test]
    fn match_first_child() {
        assert!(parse_and_match(":first-child", &MockElement::new("li").at_index(1, 5)));
        assert!(!parse_and_match(":first-child", &MockElement::new("li").at_index(3, 5)));
    }

    #[test]
    fn match_last_child() {
        assert!(parse_and_match(":last-child", &MockElement::new("li").at_index(5, 5)));
        assert!(!parse_and_match(":last-child", &MockElement::new("li").at_index(3, 5)));
    }

    #[test]
    fn match_only_child() {
        assert!(parse_and_match(":only-child", &MockElement::new("li").at_index(1, 1)));
        assert!(!parse_and_match(":only-child", &MockElement::new("li").at_index(1, 3)));
    }

    #[test]
    fn match_nth_child() {
        let el = MockElement::new("li").at_index(3, 5);
        assert!(parse_and_match(":nth-child(odd)", &el));
        assert!(!parse_and_match(":nth-child(even)", &el));
    }

    #[test]
    fn match_attribute() {
        let el = MockElement::new("input").with_attr("type", "text");
        assert!(parse_and_match("[type]", &el));
        assert!(parse_and_match("[type=text]", &el));
        assert!(!parse_and_match("[type=password]", &el));
    }

    #[test]
    fn match_selector_list() {
        let el = MockElement::new("div");
        assert!(parse_and_match("div, span", &el));
        assert!(parse_and_match("span, div", &el));
        assert!(!parse_and_match("span, p", &el));
    }

    #[test]
    fn match_root() {
        let mut el = MockElement::new("html");
        el.is_root = true;
        assert!(parse_and_match(":root", &el));
        assert!(!parse_and_match(":root", &MockElement::new("div")));
    }

    #[test]
    fn match_empty() {
        assert!(parse_and_match(":empty", &MockElement::new("div")));
    }

    #[test]
    fn match_any_link() {
        let link = MockElement::new("a").with_state(ElementState::LINK);
        let visited = MockElement::new("a").with_state(ElementState::VISITED);
        let plain = MockElement::new("a");
        assert!(parse_and_match(":any-link", &link));
        assert!(parse_and_match(":any-link", &visited));
        assert!(!parse_and_match(":any-link", &plain));
    }

    #[test]
    fn match_deep_descendant() {
        let root = MockElement::new("html");
        let body = MockElement::new("body").with_parent(root);
        let div = MockElement::new("div").with_parent(body);
        let span = MockElement::new("span").with_class("target").with_parent(div);
        assert!(parse_and_match("html .target", &span));
        assert!(parse_and_match("body span", &span));
    }

    // -- New feature tests --

    #[test]
    fn match_scope_without_context() {
        // Without context, :scope matches :root.
        let mut el = MockElement::new("html");
        el.is_root = true;
        assert!(parse_and_match(":scope", &el));
        assert!(!parse_and_match(":scope", &MockElement::new("div")));
    }

    #[test]
    fn match_scope_with_context() {
        use crate::context::{MatchingContext, SelectorCaches};

        let el = MockElement::new("div").with_id("target");
        let scope_id = el.opaque();

        let list = crate::parser::parse(":scope").unwrap();
        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        ctx.scope_element = Some(scope_id);

        assert!(matches_in_context(&list.0[0], &el, &mut ctx));

        // Different element shouldn't match :scope.
        let other = MockElement::new("span");
        let mut ctx2 = MatchingContext::new(&mut caches);
        ctx2.scope_element = Some(scope_id);
        assert!(!matches_in_context(&list.0[0], &other, &mut ctx2));
    }

    #[test]
    fn match_dir_ltr() {
        let el = MockElement::new("p").with_direction(Direction::Ltr);
        assert!(parse_and_match(":dir(ltr)", &el));
        assert!(!parse_and_match(":dir(rtl)", &el));
    }

    #[test]
    fn match_dir_rtl() {
        let el = MockElement::new("p").with_direction(Direction::Rtl);
        assert!(parse_and_match(":dir(rtl)", &el));
        assert!(!parse_and_match(":dir(ltr)", &el));
    }

    #[test]
    fn visited_privacy_default() {
        // Default (AllLinksUnvisited): :visited never matches.
        let visited = MockElement::new("a").with_state(ElementState::VISITED);
        let list = crate::parser::parse(":visited").unwrap();

        // Without context: standard flag check — :visited matches VISITED state.
        assert!(matches(&list.0[0], &visited));
    }

    #[test]
    fn visited_privacy_unvisited_mode() {
        use crate::context::{MatchingContext, SelectorCaches, VisitedHandling};

        let visited = MockElement::new("a").with_state(ElementState::VISITED);
        let list = crate::parser::parse(":visited").unwrap();

        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        ctx.visited_handling = VisitedHandling::AllLinksUnvisited;

        // With privacy mode: :visited must NOT match.
        assert!(!matches_in_context(&list.0[0], &visited, &mut ctx));
    }

    // ---------------------------------------------------------------
    // ElementSelectorFlags collection tests
    // ---------------------------------------------------------------

    #[test]
    fn flags_first_child() {
        use crate::context::{MatchingContext, SelectorCaches};
        use crate::flags::{ElementSelectorFlags, MatchingFlags};

        let el = MockElement::new("li").at_index(1, 5);
        let list = crate::parser::parse(":first-child").unwrap();

        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        ctx.flags |= MatchingFlags::COLLECT_SELECTOR_FLAGS;

        matches_in_context(&list.0[0], &el, &mut ctx);
        let flags = ctx.take_element_flags();
        assert!(flags.contains(ElementSelectorFlags::HAS_EDGE_CHILD_SELECTOR));
    }

    #[test]
    fn flags_nth_child() {
        use crate::context::{MatchingContext, SelectorCaches};
        use crate::flags::{ElementSelectorFlags, MatchingFlags};

        let el = MockElement::new("li").at_index(3, 5);
        let list = crate::parser::parse(":nth-child(odd)").unwrap();

        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        ctx.flags |= MatchingFlags::COLLECT_SELECTOR_FLAGS;

        matches_in_context(&list.0[0], &el, &mut ctx);
        let flags = ctx.take_element_flags();
        assert!(flags.contains(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH));
    }

    #[test]
    fn flags_empty() {
        use crate::context::{MatchingContext, SelectorCaches};
        use crate::flags::{ElementSelectorFlags, MatchingFlags};

        let el = MockElement::new("div"); // empty by default
        let list = crate::parser::parse(":empty").unwrap();

        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        ctx.flags |= MatchingFlags::COLLECT_SELECTOR_FLAGS;

        matches_in_context(&list.0[0], &el, &mut ctx);
        let flags = ctx.take_element_flags();
        assert!(flags.contains(ElementSelectorFlags::HAS_EMPTY_SELECTOR));
    }

    #[test]
    fn flags_not_collected_without_flag() {
        use crate::context::{MatchingContext, SelectorCaches};

        let el = MockElement::new("li").at_index(1, 5);
        let list = crate::parser::parse(":first-child").unwrap();

        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        // No COLLECT_SELECTOR_FLAGS set.

        matches_in_context(&list.0[0], &el, &mut ctx);
        let flags = ctx.take_element_flags();
        assert!(flags.is_empty());
    }

    // ---------------------------------------------------------------
    // NthIndexCache integration tests
    // ---------------------------------------------------------------

    #[test]
    fn nth_cache_used_in_context() {
        use crate::context::{MatchingContext, SelectorCaches};

        let el = MockElement::new("li").at_index(3, 5);
        let list = crate::parser::parse(":nth-child(odd)").unwrap();

        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);

        // First match computes and caches.
        assert!(matches_in_context(&list.0[0], &el, &mut ctx));

        // Cache should have an entry now.
        let cached = ctx.caches.nth.nth_child(el.opaque(), || panic!("should be cached"));
        assert_eq!(cached, 3);
    }

    // ---------------------------------------------------------------
    // Quirks mode class matching
    // ---------------------------------------------------------------

    #[test]
    fn quirks_mode_class_case_insensitive() {
        use crate::context::{MatchingContext, QuirksMode, SelectorCaches};

        let el = MockElement::new("div").with_class("FOO");
        let list = crate::parser::parse(".foo").unwrap();

        // Standards mode: case-sensitive — should NOT match.
        let mut caches = SelectorCaches::new();
        let mut ctx = MatchingContext::new(&mut caches);
        assert!(!matches_in_context(&list.0[0], &el, &mut ctx));

        // Quirks mode: case-insensitive — SHOULD match.
        let mut caches2 = SelectorCaches::new();
        let mut ctx2 = MatchingContext::new(&mut caches2);
        ctx2.quirks_mode = QuirksMode::Quirks;
        assert!(matches_in_context(&list.0[0], &el, &mut ctx2));
    }

    // ---------------------------------------------------------------
    // Namespace matching
    // ---------------------------------------------------------------

    #[test]
    fn namespace_matching_default() {
        // Without namespace component, any element matches regardless of namespace.
        let el = MockElement::new("rect");
        assert!(parse_and_match("rect", &el));
    }

    // ---------------------------------------------------------------
    // Complex matching scenarios
    // ---------------------------------------------------------------

    #[test]
    fn match_is_selector() {
        let el = MockElement::new("div").with_class("foo");
        assert!(parse_and_match(":is(div, span)", &el));
        assert!(parse_and_match(":is(.foo, .bar)", &el));
        assert!(!parse_and_match(":is(.bar, .baz)", &el));
    }

    #[test]
    fn match_where_selector() {
        let el = MockElement::new("div").with_class("foo");
        assert!(parse_and_match(":where(div, span)", &el));
        assert!(!parse_and_match(":where(.bar)", &el));
    }

    #[test]
    fn match_not_with_list() {
        let el = MockElement::new("div").with_class("active");
        assert!(parse_and_match(":not(.hidden, .disabled)", &el));
        assert!(!parse_and_match(":not(.active, .disabled)", &el));
    }

    #[test]
    fn match_nth_of_type() {
        let el = MockElement::new("li").at_index(2, 4);
        assert!(parse_and_match(":nth-of-type(even)", &el));
        assert!(!parse_and_match(":nth-of-type(odd)", &el));
    }

    #[test]
    fn match_nth_last_child() {
        // Index 3, count 5 → from-end index = 5+1-3 = 3 (odd)
        let el = MockElement::new("li").at_index(3, 5);
        assert!(parse_and_match(":nth-last-child(odd)", &el));
    }

    #[test]
    fn match_lang() {
        let el = MockElement::new("p").with_attr("lang", "en-US");
        assert!(parse_and_match(":lang(en)", &el));
        assert!(!parse_and_match(":lang(fr)", &el));
    }

    #[test]
    fn match_lang_inherited() {
        let parent = MockElement::new("html").with_attr("lang", "en");
        let el = MockElement::new("p").with_parent(parent);
        assert!(parse_and_match(":lang(en)", &el));
    }

    #[test]
    fn match_multiple_state_flags() {
        let el = MockElement::new("button")
            .with_state(ElementState::HOVER | ElementState::FOCUS);
        assert!(parse_and_match(":hover:focus", &el));
        assert!(!parse_and_match(":hover:active", &el));
    }

    #[test]
    fn match_complex_real_world() {
        // Simulate: <nav> > <ul class="menu"> > <li class="item active">
        let nav = MockElement::new("nav");
        let ul = MockElement::new("ul").with_class("menu").with_parent(nav);
        let li = MockElement::new("li")
            .with_class("item")
            .with_class("active")
            .at_index(1, 5)
            .with_parent(ul);

        assert!(parse_and_match("nav .item", &li));
        assert!(parse_and_match("nav > .menu > .active", &li));
        assert!(parse_and_match(".menu > li.item.active:first-child", &li));
        assert!(!parse_and_match(".menu > li.item.active:last-child", &li));
    }

    #[test]
    fn match_attribute_operations() {
        let el = MockElement::new("a")
            .with_attr("href", "https://example.com/path")
            .with_attr("class", "btn btn-primary")
            .with_attr("data-lang", "en-US");

        assert!(parse_and_match("[href^=https]", &el));
        assert!(parse_and_match("[href$=path]", &el));
        assert!(parse_and_match("[href*=example]", &el));
        assert!(parse_and_match("[class~=btn]", &el));
        assert!(parse_and_match("[data-lang|=en]", &el));
    }
}
