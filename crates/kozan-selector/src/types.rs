//! Core selector data types.
//!
//! This module defines the in-memory representation of parsed CSS selectors.
//! The type hierarchy mirrors the CSS Selectors Level 4 grammar:
//!
//! ```text
//! SelectorList  = Selector (',' Selector)*          §3.1
//! Selector      = CompoundSelector (Combinator CompoundSelector)*  §4
//! CompoundSelector = Component+                     §4
//! Component     = Type | Class | Id | Attr | Pseudo | ...
//! ```
//!
//! # Storage Layout
//!
//! Components are stored **right-to-left** for matching efficiency. The
//! matching algorithm starts from the rightmost compound selector (the "key
//! selector") and walks left through combinators to check ancestors/siblings.
//! This matches the browser's tree-walking direction (child → parent).
//!
//! Example: `div > .container .item:hover` is stored as:
//! ```text
//! [PseudoClass(Hover), Class("item"), Descendant, Class("container"), Child, Type("div")]
//!  ^^^^ key compound ^^^^              ^^^^^^^^^^^^ ancestor compound ^^^^^^^^^^^^^^^^^^
//! ```
//!
//! # Pre-computed Metadata
//!
//! Each `Selector` carries `SelectorHints` computed at parse time:
//! - `required_state`: all state flags needed by the key compound — enables
//!   rejection in 1 AND instruction before walking components.
//! - `key`: which bucket (`Id`, `Class`, `Type`, `Universal`) the selector
//!   belongs to in a `RuleMap` — enables O(1) rule lookup by element identity.
//!
//! # Spec Reference
//!
//! <https://drafts.csswg.org/selectors-4/#structure>

use triomphe::{Arc, ThinArc};

use kozan_atom::Atom;
use smallvec::SmallVec;

use crate::attr::{AttrOperation, AttrSelector, CaseSensitivity};
use crate::pseudo_class::PseudoClass;
use crate::pseudo_element::PseudoElement;
use crate::specificity::Specificity;

/// CSS text direction for the `:dir()` pseudo-class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Ltr,
    Rtl,
}

/// A comma-separated list of selectors: `div, .foo, #bar`.
///
/// Matches an element if ANY contained selector matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorList(pub SmallVec<[Selector; 4]>);

/// A single complex selector: compound selectors joined by combinators.
///
/// Components are stored **right-to-left** for matching efficiency.
/// Example: `div > .container .item:hover` is stored as:
/// `[PseudoClass(Hover), Class("item"), Descendant, Class("container"), Child, Type("div")]`
///
/// **Layout**: `ThinArc<SelectorHeader, Component>` — a single 8-byte thin
/// pointer to a contiguous heap allocation:
/// ```text
/// [refcount | length | SelectorHeader | Component₀ | Component₁ | ... ]
///  ^^^^^^^^   ^^^^^^   ^^^^^^^^^^^^^^^   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
///  8 bytes    8 bytes  ~36 bytes         N × 16 bytes (contiguous!)
/// ```
///
/// Why 8 bytes matters: SelectorList stores N Selectors for `:is()` / `:not()`.
/// At 8 bytes each, 5 sub-selectors = 40 bytes = 1 cache line.
/// Old layout (184 bytes/selector) needed 920 bytes = 15 cache lines.
///
/// All components are **contiguous in the same allocation** — perfect cache
/// locality during right-to-left matching. No pointer chasing to reach components.
///
/// Clone is a refcount bump (1 atomic instruction). Components are shared.
#[derive(Clone)]
pub struct Selector(ThinArc<SelectorHeader, Component>);

/// Header stored at the start of each Selector's ThinArc allocation.
/// Contains metadata computed at parse time — zero cost at match time.
#[derive(Debug, Clone)]
pub(crate) struct SelectorHeader {
    pub specificity: Specificity,
    pub hints: SelectorHints,
}

impl Selector {
    /// Create a selector from parse-order components (left-to-right).
    ///
    /// Reverses into match-order (right-to-left) during the ThinArc copy
    /// that already happens — zero extra cost vs an explicit `.reverse()`.
    pub(crate) fn from_parse_order(
        components: smallvec::SmallVec<[Component; 8]>,
        specificity: Specificity,
        hints: SelectorHints,
    ) -> Self {
        Selector(ThinArc::from_header_and_iter(
            SelectorHeader { specificity, hints },
            components.into_iter().rev(),
        ))
    }

    /// Lightweight constructor for sub-selectors inside :is()/:not()/:where().
    ///
    /// Sub-selectors don't need key extraction (no rule-map bucketing) or
    /// required_state (match_sub_selector bypasses hint checks for single-
    /// component selectors). Only `has_combinators` is needed for matching
    /// tier dispatch in `matches_in_list`.
    pub(crate) fn from_parse_order_sub(
        components: smallvec::SmallVec<[Component; 8]>,
        specificity: Specificity,
    ) -> Self {
        let mut deps = SelectorDeps::default();
        let len = components.len();
        deps.component_count = len as u16;
        // Single-component selectors can never contain a Combinator — skip the scan.
        if len > 1 {
            for c in &components {
                if matches!(c, Component::Combinator(_)) {
                    deps.set_combinators();
                    break;
                }
            }
        }
        let hints = SelectorHints {
            required_state: crate::pseudo_class::ElementState::empty(),
            key: KeySelector::Universal,
            deps,
        };
        Selector(ThinArc::from_header_and_iter(
            SelectorHeader { specificity, hints },
            components.into_iter().rev(),
        ))
    }

    /// Returns the specificity of this selector, computed at parse time.
    #[inline]
    pub fn specificity(&self) -> Specificity {
        self.0.header.header.specificity
    }

    /// Returns the components (right-to-left order). Contiguous in memory.
    #[inline]
    pub fn components(&self) -> &[Component] {
        &self.0.slice
    }

    /// Returns pre-computed hints for fast rejection and rule bucketing.
    #[inline]
    pub fn hints(&self) -> &SelectorHints {
        &self.0.header.header.hints
    }
}

impl fmt::Debug for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Selector")
            .field("components", &self.components())
            .field("specificity", &self.specificity())
            .field("hints", &self.hints())
            .finish()
    }
}

impl PartialEq for Selector {
    fn eq(&self, other: &Self) -> bool {
        self.specificity() == other.specificity() && self.components() == other.components()
    }
}

impl Eq for Selector {}

/// Pre-computed selector metadata for fast rejection and rule bucketing.
///
/// Computed once at parse time. Zero cost at match time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorHints {
    /// All `ElementState` flags required by state-based pseudo-classes in
    /// the key (rightmost) compound selector. If the element doesn't have
    /// ALL these bits set, the selector cannot match — reject in 1 AND.
    pub required_state: crate::pseudo_class::ElementState,

    /// The key selector — identifies what to bucket this rule by.
    /// Extracted from the rightmost compound selector (before any combinator).
    pub key: KeySelector,

    /// Pre-computed dependency analysis for optimization and invalidation.
    pub deps: SelectorDeps,
}

/// Parse-time dependency analysis — what a selector requires from the DOM.
///
/// Packed into 4 bytes (u16 flags + u16 count) instead of 10 bytes of bools.
/// This matters: every `Selector` carries one, and large stylesheets have
/// thousands of selectors — saving 6 bytes/selector adds up.
///
/// Instead of discovering dependencies at match time (requiring runtime branches),
/// we compute them once at parse time. The restyle system uses these to:
/// - Skip matching entirely when irrelevant mutations occur
/// - Choose optimal matching strategies per selector
/// - Pre-allocate caches for expensive pseudo-classes
///
/// # Comparison with Stylo
///
/// Stylo discovers dependencies at match time through flag collection in
/// `MatchingContext`. Our approach pre-computes them, trading a small amount
/// of parse-time work for zero-cost dependency queries during matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorDeps {
    /// Packed boolean flags — see `DepFlags`.
    flags: u16,
    /// Total number of components (for cost estimation).
    pub component_count: u16,
}

bitflags::bitflags! {
    /// Packed dependency flags for `SelectorDeps`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct DepFlags: u16 {
        /// Uses `:nth-child`, `:nth-of-type`, or related.
        const NTH        = 1 << 0;
        /// Uses `:empty`.
        const EMPTY      = 1 << 1;
        /// Uses `:first-child`, `:last-child`, `:only-child`, etc.
        const EDGE_CHILD = 1 << 2;
        /// Uses `:visited` or `:link`.
        const VISITED    = 1 << 3;
        /// Uses `:has()`.
        const HAS        = 1 << 4;
        /// Uses `:scope`.
        const SCOPE      = 1 << 5;
        /// Uses combinators (descendant, child, sibling).
        const COMBINATORS = 1 << 6;
    }
}

impl Default for SelectorDeps {
    fn default() -> Self {
        Self { flags: 0, component_count: 0 }
    }
}

impl SelectorDeps {
    pub fn depends_on_nth(&self) -> bool { self.flags & DepFlags::NTH.bits() != 0 }
    pub fn depends_on_empty(&self) -> bool { self.flags & DepFlags::EMPTY.bits() != 0 }
    pub fn depends_on_edge_child(&self) -> bool { self.flags & DepFlags::EDGE_CHILD.bits() != 0 }
    pub fn depends_on_visited(&self) -> bool { self.flags & DepFlags::VISITED.bits() != 0 }
    pub fn depends_on_has(&self) -> bool { self.flags & DepFlags::HAS.bits() != 0 }
    pub fn depends_on_scope(&self) -> bool { self.flags & DepFlags::SCOPE.bits() != 0 }
    pub fn has_combinators(&self) -> bool { self.flags & DepFlags::COMBINATORS.bits() != 0 }

    pub(crate) fn set_nth(&mut self) { self.flags |= DepFlags::NTH.bits(); }
    pub(crate) fn set_empty(&mut self) { self.flags |= DepFlags::EMPTY.bits(); }
    pub(crate) fn set_edge_child(&mut self) { self.flags |= DepFlags::EDGE_CHILD.bits(); }
    pub(crate) fn set_visited(&mut self) { self.flags |= DepFlags::VISITED.bits(); }
    pub(crate) fn set_has(&mut self) { self.flags |= DepFlags::HAS.bits(); }
    pub(crate) fn set_scope(&mut self) { self.flags |= DepFlags::SCOPE.bits(); }
    pub(crate) fn set_combinators(&mut self) { self.flags |= DepFlags::COMBINATORS.bits(); }
}

/// The "key selector" — the rightmost simple selector used for rule bucketing.
///
/// Priority: ID > Class > Type > Universal.
/// A selector like `div.foo#bar:hover` has key = `Id("bar")`.
/// A selector like `.item:first-child` has key = `Class("item")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySelector {
    /// Bucket by element ID — highest priority, most selective.
    Id(Atom),
    /// Bucket by class name.
    Class(Atom),
    /// Bucket by tag name.
    Type(Atom),
    /// No specific key — goes into the universal bucket (tested against all).
    Universal,
}

impl SelectorHints {
    /// Compute hints from a component list in **parse order** (left-to-right).
    ///
    /// The rightmost compound (match target) is at the END of parse-order
    /// components. We find it via the last combinator position, then extract
    /// key + required_state from that compound only.
    pub(crate) fn compute_parse_order(components: &[Component]) -> Self {
        let mut required_state = crate::pseudo_class::ElementState::empty();
        let mut key = KeySelector::Universal;
        let mut deps = SelectorDeps::default();

        deps.component_count = components.len() as u16;

        // Find rightmost compound start (after last combinator).
        let rightmost_start = components.iter()
            .rposition(|c| matches!(c, Component::Combinator(_)))
            .map(|i| i + 1)
            .unwrap_or(0);

        if rightmost_start > 0 {
            deps.set_combinators();
        }

        // Extract key + required_state from rightmost compound only.
        for comp in &components[rightmost_start..] {
            match comp {
                Component::Id(atom) => {
                    key = KeySelector::Id(atom.clone());
                }
                Component::Class(atom) => {
                    if !matches!(key, KeySelector::Id(_)) {
                        key = KeySelector::Class(atom.clone());
                    }
                }
                Component::Type(atom) => {
                    if matches!(key, KeySelector::Universal) {
                        key = KeySelector::Type(atom.clone());
                    }
                }
                Component::PseudoClass(pc) => {
                    if let Some(flag) = pc.state_flag() {
                        required_state |= flag;
                    }
                }
                _ => {}
            }
        }

        // Scan ALL components for dependency flags.
        for comp in components {
            match comp {
                Component::PseudoClass(pc) => {
                    if matches!(pc, PseudoClass::Visited | PseudoClass::Link) {
                        deps.set_visited();
                    }
                    if pc.is_structural() {
                        if matches!(pc, PseudoClass::Empty) {
                            deps.set_empty();
                        }
                        if matches!(pc,
                            PseudoClass::FirstChild
                            | PseudoClass::LastChild
                            | PseudoClass::OnlyChild
                            | PseudoClass::FirstOfType
                            | PseudoClass::LastOfType
                            | PseudoClass::OnlyOfType
                        ) {
                            deps.set_edge_child();
                        }
                    }
                    if matches!(pc, PseudoClass::Scope) {
                        deps.set_scope();
                    }
                }
                Component::NthChild(_)
                | Component::NthLastChild(_)
                | Component::NthOfType(_, _)
                | Component::NthLastOfType(_, _) => {
                    deps.set_nth();
                }
                Component::Has(_) => {
                    deps.set_has();
                }
                _ => {}
            }
        }

        Self { required_state, key, deps }
    }
}

/// Namespace constraint for type and universal selectors.
///
/// CSS Namespaces (CSS Namespaces Module Level 3) allow selectors to match
/// elements in specific XML/HTML namespaces via the `@namespace` at-rule:
///
/// ```css
/// @namespace svg "http://www.w3.org/2000/svg";
/// svg|rect { fill: blue; }     /* Only <rect> in SVG namespace */
/// |div { color: red; }         /* Only <div> with NO namespace */
/// *|div { color: green; }      /* <div> in ANY namespace */
/// ```
///
/// In HTML-only contexts, namespace selectors are rarely used. The default
/// (no prefix) matches elements in any namespace, which is correct for HTML.
///
/// # Spec Reference
///
/// <https://drafts.csswg.org/css-namespaces-3/>
/// <https://drafts.csswg.org/selectors-4/#type-nmsp>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NamespaceConstraint {
    /// No namespace prefix — matches elements in any namespace (default for HTML).
    Any,
    /// `|element` — matches only elements with NO namespace (null namespace).
    None,
    /// `ns|element` — matches elements in the specific namespace URI.
    Specific(Atom),
}

/// A single component in a selector.
///
/// Components fall into four categories:
/// 1. **Simple selectors** — match a single aspect of an element (type, class, ID, attr)
/// 2. **Pseudo-classes** — match element state or tree position
/// 3. **Pseudo-elements** — target virtual sub-elements (::before, ::after)
/// 4. **Combinators** — describe relationship between compound selectors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Component {
    /// `*` — matches any element.
    Universal,
    /// `div` — matches elements with the given tag name.
    Type(Atom),
    /// `#foo` — matches elements with the given ID.
    Id(Atom),
    /// `.bar` — matches elements with the given class.
    Class(Atom),
    /// `ns|*` or `ns|div` — namespace constraint on type/universal selectors.
    /// Only emitted when an explicit namespace prefix is used.
    /// Boxed because namespaces are rare and keeping it inline bloats Component.
    Namespace(Box<NamespaceConstraint>),

    /// `[attr]`, `[attr=val]`, etc.
    Attribute(Box<AttrSelector>),

    /// `:hover`, `:first-child`, etc.
    PseudoClass(PseudoClass),

    /// `:not(.foo, #bar)` — matches if none of the arguments match.
    /// Arc-shared: cheap clone when selectors are duplicated across rule maps.
    Negation(Arc<SelectorList>),
    /// `:is(.foo, #bar)` — matches if any argument matches. Specificity = max.
    Is(Arc<SelectorList>),
    /// `:where(.foo, #bar)` — like :is() but zero specificity.
    Where(Arc<SelectorList>),

    // -- Flattened fast-path variants for functional pseudo-classes --
    // Created at parse time when ALL sub-selectors are single-component
    // (class, type, id, pseudo-class, universal — any mix). Stores components
    // in one contiguous ThinArc — eliminates per-sub-selector ThinArc + Arc
    // allocations. One heap allocation instead of N+1.
    //
    // Handles cases no browser optimizes: `:is(.btn, div, #main)` (mixed),
    // `:not(.hidden, :disabled)` (class + pseudo-class), etc.

    /// Flattened `:is(...)` — all sub-selectors are single-component.
    /// Matching: any component matches element → true.
    IsSingle(ThinArc<(), Component>),
    /// Flattened `:not(...)` — all sub-selectors are single-component.
    /// Matching: no component matches element → true.
    NotSingle(ThinArc<(), Component>),
    /// Flattened `:where(...)` — like IsSingle but zero specificity.
    WhereSingle(ThinArc<(), Component>),
    /// `:has(> .bar)` — matches if the element has a descendant/sibling matching the argument.
    Has(Arc<RelativeSelectorList>),
    /// `:nth-child(An+B)` or `:nth-child(An+B of S)`.
    /// Boxed because NthData (16B) would bloat Component for all other variants.
    NthChild(Box<NthData>),
    /// `:nth-last-child(An+B)` or `:nth-last-child(An+B of S)`.
    NthLastChild(Box<NthData>),
    /// `:nth-of-type(An+B)`.
    NthOfType(i32, i32),
    /// `:nth-last-of-type(An+B)`.
    NthLastOfType(i32, i32),
    /// `:lang(en)` or `:lang(en, fr, zh)` — comma-separated language list (CSS Selectors Level 4).
    /// Boxed to keep Component at 16 bytes (SmallVec is 24B inline).
    Lang(Box<SmallVec<[Atom; 1]>>),
    /// `:dir(ltr)` / `:dir(rtl)` — matches the computed text direction.
    Dir(Direction),

    /// `::before`, `::after`, etc.
    PseudoElement(PseudoElement),

    /// `:state(name)` — CSS Custom State pseudo-class for custom elements.
    /// Matches when the custom element exposes `name` as a custom state.
    /// <https://html.spec.whatwg.org/multipage/custom-elements.html#custom-state-pseudo-class>
    State(Atom),

    // -- Shadow DOM selectors --

    /// `:host` — matches the shadow host from inside a shadow tree.
    Host,
    /// `:host(.foo)` — matches the shadow host if it also matches the argument.
    HostFunction(Arc<SelectorList>),
    /// `:host-context(.foo)` — matches if the shadow host or any ancestor matches.
    HostContext(Arc<SelectorList>),
    /// `::slotted(.foo)` — matches elements distributed into a `<slot>`.
    Slotted(Arc<SelectorList>),
    /// `::part(name)` — matches shadow parts exposed via the `part` attribute.
    /// Boxed to keep Component at 16 bytes.
    Part(Box<SmallVec<[Atom; 1]>>),

    /// `::highlight(name)` — CSS Custom Highlight API pseudo-element.
    /// <https://drafts.csswg.org/css-highlight-api-1/#custom-highlight-pseudo>
    Highlight(Atom),

    /// `&` — CSS Nesting selector (equivalent to `:scope` in selector matching).
    ///
    /// In nested CSS rules, `&` refers to the parent rule's selector. At the
    /// selector engine level, this is matched exactly like `:scope` — against
    /// the scoping element in `MatchingContext`. The stylesheet layer resolves
    /// `&` to the actual parent selector during rule insertion.
    ///
    /// <https://drafts.csswg.org/css-nesting-1/#nest-selector>
    Nesting,

    /// Relationship between compound selectors.
    Combinator(Combinator),
}

/// Relationship between two compound selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Combinator {
    /// ` ` (space) — descendant: `div .foo` matches `.foo` inside `div` at any depth.
    Descendant,
    /// `>` — child: `div > .foo` matches `.foo` that is a direct child of `div`.
    Child,
    /// `+` — adjacent sibling: `div + .foo` matches `.foo` immediately after `div`.
    NextSibling,
    /// `~` — general sibling: `div ~ .foo` matches `.foo` after `div` (any distance).
    LaterSibling,
    /// `||` — column: `col || td` matches `td` cells belonging to a `col`.
    /// CSS Selectors Level 4 § 13.1. Experimental — no browser ships stable.
    /// <https://drafts.csswg.org/selectors-4/#the-column-combinator>
    Column,
}

/// A list of relative selectors used inside `:has()`.
///
/// Relative selectors may start with a combinator:
/// `:has(> .bar)` means "has a child matching .bar".
/// `:has(.bar)` means "has a descendant matching .bar" (implicit descendant combinator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeSelectorList(pub SmallVec<[RelativeSelector; 2]>);

/// A single relative selector — an optional leading combinator + a regular selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeSelector {
    /// The leading combinator. Defaults to `Descendant` if omitted.
    pub combinator: Combinator,
    /// The selector to match against the related element.
    pub selector: Selector,
    /// Pre-computed traversal direction — avoids runtime branching in matches_has().
    pub traversal: HasTraversal,
}

/// Pre-computed `:has()` traversal direction.
///
/// Computed at parse time from the leading combinator. The matching engine
/// uses this to skip irrelevant tree walks (e.g., don't walk siblings when
/// the selector only needs descendants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HasTraversal {
    /// Walk all descendants (`:has(.foo)`, `:has(> .foo .bar)`).
    Subtree,
    /// Check direct children only (`:has(> .foo)` with no further descendant combinators).
    Children,
    /// Check next sibling only (`:has(+ .foo)`).
    NextSibling,
    /// Walk all subsequent siblings (`:has(~ .foo)`).
    Siblings,
}

impl HasTraversal {
    /// Compute from a leading combinator and the inner selector's dependency flags.
    pub fn from_combinator(comb: Combinator, inner_has_combinators: bool) -> Self {
        match comb {
            Combinator::Descendant => Self::Subtree,
            Combinator::Child => {
                if inner_has_combinators {
                    // `:has(> .foo .bar)` — child match, then descendant walk from child.
                    Self::Subtree
                } else {
                    Self::Children
                }
            }
            Combinator::NextSibling => {
                if inner_has_combinators {
                    Self::Subtree
                } else {
                    Self::NextSibling
                }
            }
            Combinator::LaterSibling => {
                if inner_has_combinators {
                    Self::Subtree
                } else {
                    Self::Siblings
                }
            }
            Combinator::Column => Self::Subtree, // Column inside :has() — walk subtree
        }
    }
}

/// Parsed `An+B` data for `:nth-child()` and related pseudo-classes.
///
/// Matches the `An+B`th element, optionally filtered by a selector list.
/// Examples: `2n+1` (odd), `3n`, `-n+3` (first 3), `even`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NthData {
    pub a: i32,
    pub b: i32,
    /// Optional selector filter: `:nth-child(2n of .active)`.
    pub of_selector: Option<Box<SelectorList>>,
}

impl NthData {
    /// Returns `true` if the given 1-based index matches this An+B formula.
    #[inline]
    pub fn matches_index(&self, index: i32) -> bool {
        Self::formula(self.a, self.b, index)
    }

    /// Static An+B formula check. Used by both NthData and standalone (a, b) pairs.
    ///
    /// An element at 1-based `index` matches `An+B` when:
    /// - `A == 0`: index must equal B exactly
    /// - `A != 0`: (index - B) must be divisible by A with non-negative quotient
    #[inline]
    pub fn formula(a: i32, b: i32, index: i32) -> bool {
        if a == 0 {
            return index == b;
        }
        let diff = index - b;
        diff % a == 0 && diff / a >= 0
    }
}

use std::fmt;

impl fmt::Display for SelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, sel) in self.0.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{sel}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Components are stored right-to-left; display left-to-right.
        for comp in self.components().iter().rev() {
            write!(f, "{comp}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Universal => f.write_str("*"),
            Self::Type(name) => f.write_str(name.as_ref()),
            Self::Id(id) => write!(f, "#{}", id.as_ref()),
            Self::Class(class) => write!(f, ".{}", class.as_ref()),
            Self::Namespace(ns) => match &**ns {
                NamespaceConstraint::Any => f.write_str("*|"),
                NamespaceConstraint::None => f.write_str("|"),
                NamespaceConstraint::Specific(uri) => write!(f, "{}|", uri.as_ref()),
            },
            Self::Attribute(attr) => write!(f, "{attr}"),
            Self::PseudoClass(pc) => write!(f, ":{pc}"),
            Self::PseudoElement(pe) => write!(f, "::{pe}"),
            Self::Negation(list) => write!(f, ":not({list})"),
            Self::Is(list) => write!(f, ":is({list})"),
            Self::Where(list) => write!(f, ":where({list})"),
            Self::Has(rel) => write!(f, ":has({rel})"),
            Self::NthChild(nth) => write!(f, ":nth-child({nth})"),
            Self::NthLastChild(nth) => write!(f, ":nth-last-child({nth})"),
            Self::NthOfType(a, b) => {
                write!(f, ":nth-of-type(")?;
                fmt_an_plus_b(f, *a, *b)?;
                f.write_str(")")
            }
            Self::NthLastOfType(a, b) => {
                write!(f, ":nth-last-of-type(")?;
                fmt_an_plus_b(f, *a, *b)?;
                f.write_str(")")
            }
            Self::Lang(langs) => {
                f.write_str(":lang(")?;
                for (i, lang) in langs.iter().enumerate() {
                    if i > 0 { f.write_str(", ")?; }
                    f.write_str(lang.as_ref())?;
                }
                f.write_str(")")
            }
            Self::Dir(dir) => write!(f, ":dir({dir})"),
            Self::State(name) => write!(f, ":state({})", name.as_ref()),
            Self::Host => f.write_str(":host"),
            Self::HostFunction(list) => write!(f, ":host({list})"),
            Self::HostContext(list) => write!(f, ":host-context({list})"),
            Self::Slotted(list) => write!(f, "::slotted({list})"),
            Self::Part(parts) => {
                f.write_str("::part(")?;
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 { f.write_str(" ")?; }
                    f.write_str(part.as_ref())?;
                }
                f.write_str(")")
            }
            Self::Highlight(name) => write!(f, "::highlight({})", name.as_ref()),
            Self::Nesting => f.write_str("&"),
            Self::Combinator(c) => write!(f, "{c}"),
            // Flattened variants display as their original form
            Self::IsSingle(comps) => {
                f.write_str(":is(")?;
                for (i, c) in comps.slice.iter().enumerate() {
                    if i > 0 { f.write_str(", ")?; }
                    write!(f, "{c}")?;
                }
                f.write_str(")")
            }
            Self::WhereSingle(comps) => {
                f.write_str(":where(")?;
                for (i, c) in comps.slice.iter().enumerate() {
                    if i > 0 { f.write_str(", ")?; }
                    write!(f, "{c}")?;
                }
                f.write_str(")")
            }
            Self::NotSingle(comps) => {
                f.write_str(":not(")?;
                for (i, c) in comps.slice.iter().enumerate() {
                    if i > 0 { f.write_str(", ")?; }
                    write!(f, "{c}")?;
                }
                f.write_str(")")
            }
        }
    }
}

impl fmt::Display for Combinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Descendant => f.write_str(" "),
            Self::Child => f.write_str(" > "),
            Self::NextSibling => f.write_str(" + "),
            Self::LaterSibling => f.write_str(" ~ "),
            Self::Column => f.write_str(" || "),
        }
    }
}

impl fmt::Display for RelativeSelectorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, rel) in self.0.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            match rel.combinator {
                Combinator::Descendant => {}
                Combinator::Child => f.write_str("> ")?,
                Combinator::NextSibling => f.write_str("+ ")?,
                Combinator::LaterSibling => f.write_str("~ ")?,
                Combinator::Column => f.write_str("|| ")?,
            }
            write!(f, "{}", rel.selector)?;
        }
        Ok(())
    }
}

impl fmt::Display for NthData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_an_plus_b(f, self.a, self.b)?;
        if let Some(ref sel) = self.of_selector {
            write!(f, " of {sel}")?;
        }
        Ok(())
    }
}

fn fmt_an_plus_b(f: &mut fmt::Formatter<'_>, a: i32, b: i32) -> fmt::Result {
    if a == 0 {
        return write!(f, "{b}");
    }
    if a == 2 && b == 1 {
        return f.write_str("odd");
    }
    if a == 2 && b == 0 {
        return f.write_str("even");
    }
    match a {
        1 => f.write_str("n")?,
        -1 => f.write_str("-n")?,
        _ => write!(f, "{a}n")?,
    }
    match b.cmp(&0) {
        std::cmp::Ordering::Greater => write!(f, "+{b}"),
        std::cmp::Ordering::Less => write!(f, "{b}"),
        std::cmp::Ordering::Equal => Ok(()),
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        })
    }
}

impl fmt::Display for PseudoClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Hover => "hover",
            Self::Active => "active",
            Self::Focus => "focus",
            Self::FocusWithin => "focus-within",
            Self::FocusVisible => "focus-visible",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Checked => "checked",
            Self::Indeterminate => "indeterminate",
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::ReadOnly => "read-only",
            Self::ReadWrite => "read-write",
            Self::PlaceholderShown => "placeholder-shown",
            Self::Default => "default",
            Self::Target => "target",
            Self::Visited => "visited",
            Self::Link => "link",
            Self::AnyLink => "any-link",
            Self::Fullscreen => "fullscreen",
            Self::Modal => "modal",
            Self::PopoverOpen => "popover-open",
            Self::Defined => "defined",
            Self::Autofill => "autofill",
            Self::UserValid => "user-valid",
            Self::UserInvalid => "user-invalid",
            Self::Root => "root",
            Self::Empty => "empty",
            Self::FirstChild => "first-child",
            Self::LastChild => "last-child",
            Self::OnlyChild => "only-child",
            Self::FirstOfType => "first-of-type",
            Self::LastOfType => "last-of-type",
            Self::OnlyOfType => "only-of-type",
            Self::Scope => "scope",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Seeking => "seeking",
            Self::Buffering => "buffering",
            Self::Stalled => "stalled",
            Self::Muted => "muted",
            Self::VolumeLocked => "volume-locked",
            Self::Blank => "blank",
            Self::InRange => "in-range",
            Self::OutOfRange => "out-of-range",
            Self::Open => "open",
            Self::Closed => "closed",
            Self::PictureInPicture => "picture-in-picture",
            Self::TargetWithin => "target-within",
            Self::LocalLink => "local-link",
            Self::Current => "current",
            Self::Past => "past",
            Self::Future => "future",
        })
    }
}

impl fmt::Display for PseudoElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Before => "before",
            Self::After => "after",
            Self::FirstLine => "first-line",
            Self::FirstLetter => "first-letter",
            Self::Placeholder => "placeholder",
            Self::Selection => "selection",
            Self::Marker => "marker",
            Self::Backdrop => "backdrop",
            Self::FileSelectorButton => "file-selector-button",
            Self::GrammarError => "grammar-error",
            Self::SpellingError => "spelling-error",
        })
    }
}

impl fmt::Display for AttrSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.operation {
            AttrOperation::Exists => write!(f, "[{}]", self.name.as_ref()),
            op => {
                write!(f, "[{}", self.name.as_ref())?;
                let (sym, val, cs) = match op {
                    AttrOperation::Exists => unreachable!(),
                    AttrOperation::Equals(v, c) => ("=", v, c),
                    AttrOperation::Includes(v, c) => ("~=", v, c),
                    AttrOperation::DashMatch(v, c) => ("|=", v, c),
                    AttrOperation::Prefix(v, c) => ("^=", v, c),
                    AttrOperation::Suffix(v, c) => ("$=", v, c),
                    AttrOperation::Substring(v, c) => ("*=", v, c),
                };
                // Per CSSOM §6.4.2, attribute values always serialize as quoted strings.
                let v = val.as_ref();
                write!(f, "{sym}\"")?;
                for ch in v.chars() {
                    if ch == '"' {
                        f.write_str("\\\"")?;
                    } else {
                        write!(f, "{ch}")?;
                    }
                }
                f.write_str("\"")?;
                if *cs == CaseSensitivity::AsciiCaseInsensitive {
                    f.write_str(" i")?;
                }
                f.write_str("]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_sizes_optimized() {
        assert_eq!(std::mem::size_of::<Atom>(), 8, "Atom = ThinArc thin pointer");
        assert_eq!(std::mem::size_of::<Component>(), 16, "Component fits in 1 cache line pair");
    }


    #[test]
    fn nth_odd() {
        // 2n+1 matches 1, 3, 5, 7, ...
        let nth = NthData {
            a: 2,
            b: 1,
            of_selector: None,
        };
        assert!(nth.matches_index(1));
        assert!(!nth.matches_index(2));
        assert!(nth.matches_index(3));
        assert!(nth.matches_index(5));
    }

    #[test]
    fn nth_even() {
        // 2n matches 2, 4, 6, ...
        let nth = NthData {
            a: 2,
            b: 0,
            of_selector: None,
        };
        assert!(!nth.matches_index(1));
        assert!(nth.matches_index(2));
        assert!(nth.matches_index(4));
    }

    #[test]
    fn nth_specific() {
        // 0n+3 = just the 3rd element.
        let nth = NthData {
            a: 0,
            b: 3,
            of_selector: None,
        };
        assert!(!nth.matches_index(1));
        assert!(!nth.matches_index(2));
        assert!(nth.matches_index(3));
        assert!(!nth.matches_index(4));
    }

    #[test]
    fn nth_first_three() {
        // -n+3 matches 1, 2, 3.
        let nth = NthData {
            a: -1,
            b: 3,
            of_selector: None,
        };
        assert!(nth.matches_index(1));
        assert!(nth.matches_index(2));
        assert!(nth.matches_index(3));
        assert!(!nth.matches_index(4));
    }

    // ---------------------------------------------------------------
    // SelectorDeps tests
    // ---------------------------------------------------------------

    fn parse_hints(css: &str) -> SelectorHints {
        let list = crate::parser::parse(css).unwrap();
        list.0[0].hints().clone()
    }

    #[test]
    fn deps_simple_selector() {
        let h = parse_hints("div.foo");
        assert!(!h.deps.has_combinators());
        assert!(!h.deps.depends_on_nth());
        assert!(!h.deps.depends_on_has());
        assert_eq!(h.deps.component_count, 2);
    }

    #[test]
    fn deps_descendant_combinator() {
        let h = parse_hints("div .foo");
        assert!(h.deps.has_combinators());
    }

    #[test]
    fn deps_nth_child() {
        let h = parse_hints(":nth-child(2n)");
        assert!(h.deps.depends_on_nth());
    }

    #[test]
    fn deps_first_child() {
        let h = parse_hints(":first-child");
        assert!(h.deps.depends_on_edge_child());
    }

    #[test]
    fn deps_empty() {
        let h = parse_hints(":empty");
        assert!(h.deps.depends_on_empty());
    }

    #[test]
    fn deps_visited() {
        let h = parse_hints(":visited");
        assert!(h.deps.depends_on_visited());
    }

    #[test]
    fn deps_has() {
        let h = parse_hints(":has(.child)");
        assert!(h.deps.depends_on_has());
    }

    #[test]
    fn deps_scope() {
        let h = parse_hints(":scope");
        assert!(h.deps.depends_on_scope());
    }

    #[test]
    fn deps_complex() {
        let h = parse_hints("div > .container:first-child .item:nth-child(2n)");
        assert!(h.deps.has_combinators());
        assert!(h.deps.depends_on_nth());
        assert!(h.deps.depends_on_edge_child());
        assert!(!h.deps.depends_on_has());
        assert!(!h.deps.depends_on_visited());
    }
}
