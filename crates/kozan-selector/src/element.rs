//! The `Element` trait — the bridge between the selector engine and the DOM.
//!
//! This is the ONLY interface the selector engine uses to interact with DOM
//! elements. DOM implementations (real DOM, virtual DOM, ECS-based, etc.)
//! provide this trait; the selector engine reads through it.
//!
//! # Design Principles
//!
//! 1. **Zero coupling**: The selector engine knows nothing about the DOM
//!    representation. No tree pointers, no node types, no memory layout.
//!
//! 2. **O(1) identity checks**: String identifiers (`local_name`, `id`,
//!    classes) return `&Atom` for pointer-equality comparison. The selector
//!    engine never does string comparison for tag/class/ID matching.
//!
//! 3. **Single-instruction state checks**: `state()` returns `ElementState`
//!    bitflags. Matching `:hover` is one AND instruction, not a method call.
//!
//! 4. **Cacheable indices**: `child_index()` / `child_count()` should be
//!    cached by the DOM implementation. The selector engine may call these
//!    multiple times per element during `:nth-child()` matching.
//!
//! # Comparison with Stylo
//!
//! Stylo's `Element` trait has 30+ methods including Shadow DOM traversal,
//! namespace matching, and pseudo-element origination. Our trait has the
//! essential set for CSS Selectors Level 4 including Shadow DOM support.
//! Shadow DOM methods have default no-op implementations so non-shadow
//! DOM implementations work without any extra code.
//!
//! # Implementor's Checklist
//!
//! - `local_name()` and `id()` MUST return interned `Atom` values (same
//!   pointer for same string). Use `kozan_atom::Atom::from()` at parse time.
//! - `child_index()` / `child_count()` should be cached (avoid O(n) walks).
//! - `opaque()` must return a unique, stable identity for the element's lifetime.
//! - `direction()` should inherit from parent if not explicitly set.

use kozan_atom::Atom;
use crate::opaque::OpaqueElement;
use crate::pseudo_class::ElementState;
use crate::types::Direction;

/// A DOM element that can be matched against CSS selectors.
///
/// All string identifiers (`local_name`, `id`, classes) use `Atom` for
/// O(1) pointer-equality comparison during selector matching.
///
/// State-based pseudo-classes (`:hover`, `:focus`, etc.) are matched via
/// `ElementState` bitflags — a single AND instruction per check.
pub trait Element: Sized + Clone {
    /// Tag name (e.g. `"div"`, `"button"`). Always lowercase and interned.
    fn local_name(&self) -> &Atom;

    /// The element's ID attribute, if any.
    fn id(&self) -> Option<&Atom>;

    /// Returns `true` if the element has the given class.
    fn has_class(&self, class: &Atom) -> bool;

    /// Iterates over all classes on this element.
    fn each_class<F: FnMut(&Atom)>(&self, f: F);

    /// Returns the value of the given attribute, if present.
    fn attr(&self, name: &Atom) -> Option<&str>;

    /// Parent element, or `None` if this is the root.
    fn parent_element(&self) -> Option<Self>;

    /// Previous sibling element (skipping text nodes).
    fn prev_sibling_element(&self) -> Option<Self>;

    /// Next sibling element (skipping text nodes).
    fn next_sibling_element(&self) -> Option<Self>;

    /// First child element.
    fn first_child_element(&self) -> Option<Self>;

    /// Last child element.
    fn last_child_element(&self) -> Option<Self>;

    /// Interaction and form state flags. Matched in 1 AND instruction.
    fn state(&self) -> ElementState;

    /// Whether this is the document root element.
    fn is_root(&self) -> bool;

    /// Whether this element has no child elements and no text content.
    fn is_empty(&self) -> bool;

    /// 1-based index among siblings. Implementations should cache this.
    fn child_index(&self) -> u32;

    /// Total number of sibling elements (including self).
    fn child_count(&self) -> u32;

    /// 1-based index among siblings of the same type. Cache recommended.
    fn child_index_of_type(&self) -> u32;

    /// Total siblings of the same type (including self).
    fn child_count_of_type(&self) -> u32;

    /// Opaque identity for caching and comparison (`:scope`, nth-cache).
    ///
    /// Must be unique and stable for the element's lifetime. Implementations
    /// can use a pointer cast, arena index, ECS entity, or any u64-sized ID.
    fn opaque(&self) -> OpaqueElement;

    /// Computed text direction for `:dir(ltr)` / `:dir(rtl)` matching.
    ///
    /// Should return the element's computed direction, inheriting from
    /// the parent if not explicitly set (via `dir` attribute, etc.).
    /// Default: `Ltr` (matches CSS initial value).
    fn direction(&self) -> Direction {
        Direction::Ltr
    }

    /// The element's namespace URI, if any.
    ///
    /// Returns the namespace URI for XML/SVG elements. HTML elements in the
    /// HTML namespace may return `None` or the HTML namespace URI — both are
    /// acceptable since HTML selectors rarely use namespace prefixes.
    ///
    /// Used for `ns|element` namespace-qualified selectors.
    fn namespace(&self) -> Option<&Atom> {
        None
    }

    // -- Lifecycle --
    // These methods have default implementations suitable for most DOMs.
    // Override when needed for specific CSS features.

    /// Whether this element was just inserted into the DOM and has not yet
    /// been styled or laid out.
    ///
    /// Used by `@starting-style` (CSS Transitions Level 2): rules inside
    /// `@starting-style { }` only apply to elements on their first style
    /// resolution after insertion. After the first restyle, the element is
    /// no longer "newly inserted" and starting-style rules are skipped.
    ///
    /// The DOM implementation should:
    /// 1. Return `true` on the first `resolve()` call after `appendChild()`
    /// 2. Return `false` on all subsequent calls
    ///
    /// Default: `false` (safe — starting-style rules are skipped).
    fn is_newly_inserted(&self) -> bool {
        false
    }

    // -- Shadow DOM support --
    // These methods have default implementations that return false/empty,
    // so DOM implementations without Shadow DOM support work out of the box.
    // A DOM with full Shadow DOM support should override all of these.

    /// Whether this element is a shadow host (has an attached shadow root).
    ///
    /// Used by `:host` and `:host()`. The selector engine calls this to
    /// determine if the element can be matched from inside its shadow tree.
    fn is_shadow_host(&self) -> bool {
        false
    }

    /// The shadow host that contains this element's shadow tree, if any.
    ///
    /// If this element lives inside a shadow tree, returns the shadow host
    /// element that owns that tree. Used by `:host-context()` to correctly
    /// walk ancestors across shadow boundaries.
    ///
    /// Returns `None` if the element is in the light DOM (no shadow ancestor).
    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    /// Whether this element has been distributed into a `<slot>`.
    ///
    /// Used by `::slotted()`. Returns `true` if this element was assigned
    /// to a slot in a shadow tree via the slotting algorithm.
    fn is_in_slot(&self) -> bool {
        false
    }

    /// The slot element this element is assigned to, if any.
    ///
    /// If this element is slotted (distributed into a `<slot>`), returns
    /// the `<slot>` element. Used to verify `::slotted()` context.
    fn assigned_slot(&self) -> Option<Self> {
        None
    }

    /// Whether this element exposes the given `part` name via the `part` attribute.
    ///
    /// Used by `::part()`. Elements expose parts via the `part` HTML attribute:
    /// `<div part="header footer">` exposes both "header" and "footer".
    fn is_part(&self, _name: &Atom) -> bool {
        false
    }

    /// Whether this element has the given custom state (`:state()` pseudo-class).
    ///
    /// Custom elements can expose internal state via the `CustomStateSet` API.
    /// Returns `true` if the element's custom state set contains `name`.
    /// <https://html.spec.whatwg.org/multipage/custom-elements.html#custom-state-pseudo-class>
    fn has_custom_state(&self, _name: &Atom) -> bool {
        false
    }

    /// Returns the `<col>` or `<colgroup>` element that this table cell belongs to.
    ///
    /// Used by the column combinator (`||`). Only meaningful for `<td>` and `<th>`
    /// elements inside a table. Returns `None` if the element is not a table cell
    /// or has no associated column.
    fn column_element(&self) -> Option<Self> {
        None
    }
}
