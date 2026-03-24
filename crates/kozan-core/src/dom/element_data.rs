// Per-element data — the single "row" for each element node.
//
// All element fields in one struct, indexed by node slot.
// Mutations update interned fields immediately.
// Inline styles stored as plain PDB (cheap mutation),
// converted to Arc<Locked<>> lazily before style traversal.

use std::cell::Cell;
use std::collections::HashSet;

use selectors::matching::ElementSelectorFlags;
use servo_arc::Arc;
use style::Atom;
use style::data::ElementDataWrapper;
use style::properties::declaration_block::PropertyDeclarationBlock;
use style::properties::{Importance, PropertyDeclaration};
use style::shared_lock::Locked;
use style_dom::ElementState;
use web_atoms::{LocalName, Namespace};

use crate::dom::attribute::AttributeCollection;

/// HTML namespace.
static HTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// All data for one element node. Stored in `Storage<ElementData>`.
pub struct ElementData {
    // ── Identity (interned at creation, updated on attribute change) ──
    /// Raw tag name (compile-time known, e.g. "div").
    pub(crate) tag_name: &'static str,

    /// Interned tag name for Stylo selector matching.
    pub(crate) local_name: LocalName,

    /// Element namespace (HTML for all Kozan elements).
    pub(crate) namespace: Namespace,

    /// Interned id attribute. Updated on `set_attribute("id", ...)`.
    pub(crate) id: Option<Atom>,

    /// Interned class names. O(1) add/remove/contains.
    /// Chrome equivalent: `DOMTokenList` backing store.
    pub(crate) classes: HashSet<Atom>,

    // ── State ──
    /// Element state flags (focus, hover, active, enabled, etc.)
    pub(crate) element_state: ElementState,

    // Focus management not yet implemented.
    #[allow(dead_code)]
    pub(crate) is_focusable: bool,
    #[allow(dead_code)]
    pub(crate) tab_index: i32,

    // ── DOM attributes ──
    /// All attributes (id, class, data-*, custom).
    pub(crate) attributes: AttributeCollection,

    // ── Inline styles ──
    //
    // Two-layer design:
    // - `inline_styles`: plain PDB, cheap to mutate (push = one Vec append)
    // - `style_attribute`: Arc<Locked<PDB>> cache for Stylo, rebuilt before traversal
    //
    // Property setters modify `inline_styles` only. No Arc/Lock touching.
    // Before traversal, `flush_inline_styles()` creates the locked cache.
    /// Plain PDB for inline styles. Mutated by property setters.
    pub(crate) inline_styles: PropertyDeclarationBlock,

    /// Locked cache for Stylo's `style_attribute()`. Rebuilt from `inline_styles`.
    pub(crate) style_attribute: Option<Arc<Locked<PropertyDeclarationBlock>>>,

    /// Whether `inline_styles` has been modified since last flush.
    pub(crate) inline_dirty: Cell<bool>,

    // ── Stylo data (style traversal reads/writes these) ──
    /// Stylo's computed styles + restyle damage + hints.
    pub(crate) stylo_data: ElementDataWrapper,

    /// Whether any descendant needs style processing.
    pub(crate) dirty_descendants: Cell<bool>,

    /// Whether this element has a state/attribute snapshot.
    pub(crate) has_snapshot: Cell<bool>,

    /// Whether the snapshot has been handled.
    pub(crate) handled_snapshot: Cell<bool>,

    /// Counter for bottom-up traversal.
    pub(crate) children_to_process: Cell<isize>,

    /// Selector flags set by Stylo during matching.
    pub(crate) selector_flags: Cell<ElementSelectorFlags>,
}

impl ElementData {
    pub(crate) fn new(tag_name: &'static str, is_focusable: bool) -> Self {
        let mut state = ElementState::DEFINED;
        if is_focusable {
            state.insert(ElementState::ENABLED);
        }

        Self {
            tag_name,
            local_name: LocalName::from(tag_name),
            namespace: Namespace::from(HTML_NS),
            id: None,
            classes: HashSet::new(),
            element_state: state,
            is_focusable,
            tab_index: if is_focusable { 0 } else { -1 },
            attributes: AttributeCollection::new(),
            inline_styles: PropertyDeclarationBlock::new(),
            style_attribute: None,
            inline_dirty: Cell::new(false),
            stylo_data: ElementDataWrapper::default(),
            dirty_descendants: Cell::new(true),
            has_snapshot: Cell::new(false),
            handled_snapshot: Cell::new(false),
            children_to_process: Cell::new(0),
            selector_flags: Cell::new(ElementSelectorFlags::empty()),
        }
    }

    // ── Inline style mutation (cheap — no Arc/Lock) ──

    /// Push a property declaration into inline styles.
    /// Just a Vec push — O(1), no allocation beyond PDB growth.
    pub(crate) fn set_inline_property(&mut self, decl: PropertyDeclaration) {
        self.inline_styles.push(decl, Importance::Normal);
        self.inline_dirty.set(true);
    }

    /// Overwrite all inline styles from a CSS string.
    pub(crate) fn set_inline_from_css(
        &mut self,
        value: &str,
        guard: &style::shared_lock::SharedRwLock,
    ) {
        let url = url::Url::parse("kozan://inline").expect("hardcoded URL is always valid");
        let url_data = style::stylesheets::UrlExtraData(Arc::new(url));
        self.inline_styles = style::properties::parse_style_attribute(
            value,
            &url_data,
            None,
            selectors::matching::QuirksMode::NoQuirks,
            style::stylesheets::CssRuleType::Style,
        );
        self.inline_dirty.set(true);
        // Also rebuild the cache immediately for `style="..."` attribute.
        self.style_attribute = Some(Arc::new(guard.wrap(self.inline_styles.clone())));
        self.inline_dirty.set(false);
    }

    /// Clear all inline styles.
    pub(crate) fn clear_inline_styles(&mut self) {
        self.inline_styles = PropertyDeclarationBlock::new();
        self.style_attribute = None;
        self.inline_dirty.set(false);
    }

    /// Flush `inline_styles` → `style_attribute` cache (called before traversal).
    pub(crate) fn flush_inline_styles(&mut self, guard: &style::shared_lock::SharedRwLock) {
        if !self.inline_dirty.get() {
            return;
        }
        if self.inline_styles.is_empty() {
            self.style_attribute = None;
        } else {
            self.style_attribute = Some(Arc::new(guard.wrap(self.inline_styles.clone())));
        }
        self.inline_dirty.set(false);
    }

    // ── Restyle marking ──

    pub(crate) fn mark_for_restyle(&self) {
        use style::invalidation::element::restyle_hints::RestyleHint;
        let mut data = self.stylo_data.borrow_mut();
        data.hint.insert(RestyleHint::RESTYLE_SELF);
    }

    // ── Attribute change hooks ──

    pub(crate) fn on_attribute_set(
        &mut self,
        name: &str,
        value: &str,
        guard: &style::shared_lock::SharedRwLock,
    ) -> bool {
        match name {
            "id" => {
                self.id = Some(Atom::from(value));
                true
            }
            "class" => {
                self.classes = value.split_ascii_whitespace().map(Atom::from).collect();
                true
            }
            "style" => {
                self.set_inline_from_css(value, guard);
                true
            }
            _ => false,
        }
    }

    // ── ClassList — Chrome: DOMTokenList (element.classList) ──

    /// Add a class. Returns true if it was newly inserted.
    pub(crate) fn class_add(&mut self, name: &str) -> bool {
        self.classes.insert(Atom::from(name))
    }

    /// Remove a class. Returns true if it was present.
    pub(crate) fn class_remove(&mut self, name: &str) -> bool {
        self.classes.remove(&Atom::from(name))
    }

    /// Toggle a class. Returns true if now present, false if removed.
    pub(crate) fn class_toggle(&mut self, name: &str) -> bool {
        let atom = Atom::from(name);
        if !self.classes.remove(&atom) {
            self.classes.insert(atom);
            true
        } else {
            false
        }
    }

    /// Check if a class is present.
    pub(crate) fn class_contains(&self, name: &str) -> bool {
        self.classes.contains(&Atom::from(name))
    }

    pub(crate) fn on_attribute_removed(&mut self, name: &str) -> bool {
        match name {
            "id" => {
                self.id = None;
                true
            }
            "class" => {
                self.classes.clear();
                true
            }
            "style" => {
                self.clear_inline_styles();
                true
            }
            _ => false,
        }
    }
}
