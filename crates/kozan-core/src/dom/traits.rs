//! DOM trait hierarchy — Chrome's class hierarchy as Rust traits.
//!
//! ```text
//! HasHandle          ← fn handle() -> Handle   (base, derives generate this)
//!   EventTarget      ← on(), off(), dispatch()  [in events/ module]
//!     Node           ← parent, siblings, raw()  [Chrome: core/dom/node.h]
//!       ContainerNode ← append, children         [Chrome: core/dom/container_node.h]
//!         Element     ← attributes, tag name     [Chrome: core/dom/element.h]
//! ```
//!
//! `HasHandle` lives here. `EventTarget` lives in `events/`.
//! Both `Node` and `Window` extend `EventTarget` (same as Chrome).

use crate::dom::handle::Handle;
use crate::dom::node::NodeType;
use crate::events::EventTarget;
use crate::id::RawId;

/// The single base trait: provides access to the inner [`Handle`].
///
/// This is the ONE method that derive macros generate.
/// Every other trait in the hierarchy builds on top with default
/// implementations — zero boilerplate for implementors.
///
/// # Implementors
///
/// - All DOM node types (`Text`, `HtmlDivElement`, `HtmlButtonElement`, ...)
/// - `Window` (future)
/// - Custom elements defined by users
pub trait HasHandle: Copy + 'static {
    /// Get the inner [`Handle`] for this node/target.
    ///
    /// The Handle provides low-level access to the document's parallel arenas.
    /// Most users should call trait methods instead of using the Handle directly.
    fn handle(&self) -> Handle;
}

/// All DOM node types: Text, Element, Document.
///
/// Like Chrome's `Node` class (`core/dom/node.h`).
/// Extends [`EventTarget`] — every node can receive events.
/// Provides tree position (parent, siblings) and identity (raw ID, node kind).
///
/// # What implements `Node`
///
/// - [`Text`](crate::Text) — text content, no children
/// - All elements via [`Element`] → [`ContainerNode`] → `Node`
/// - Document root (accessed via [`Document::root()`](crate::Document::root))
pub trait Node: EventTarget {
    /// Check if this node is still alive in the document.
    ///
    /// Returns `false` after [`destroy()`](Self::destroy) is called.
    /// All other methods return defaults (None, empty, false) on dead nodes.
    fn is_alive(&self) -> bool {
        self.handle().is_alive()
    }

    /// Get the parent node, or `None` if this is the root or detached.
    fn parent(&self) -> Option<Handle> {
        self.handle().parent()
    }

    /// Get the next sibling in the parent's child list.
    fn next_sibling(&self) -> Option<Handle> {
        self.handle().next_sibling()
    }

    /// Get the previous sibling in the parent's child list.
    fn prev_sibling(&self) -> Option<Handle> {
        self.handle().prev_sibling()
    }

    /// Detach this node from its parent. The node stays alive but is no longer in the tree.
    fn detach(&self) {
        self.handle().detach();
    }

    /// Destroy this node. Frees the slot — all handles to it become stale.
    fn destroy(&self) {
        self.handle().destroy();
    }

    /// Get the raw ID for cross-thread communication.
    ///
    /// [`RawId`] is `Send` — use it to reference nodes from background tasks.
    /// Resolve back to a Handle via [`Document::resolve()`](crate::Document::resolve).
    fn raw(&self) -> RawId {
        self.handle().raw()
    }

    /// Get the DOM node type (Element, Text, Document, etc.).
    fn node_kind(&self) -> Option<NodeType> {
        self.handle().node_kind()
    }

    /// Is this an element node?
    fn is_element(&self) -> bool {
        self.handle().is_element()
    }

    /// Is this a text node?
    fn is_text(&self) -> bool {
        self.handle().is_text()
    }

    /// Is this the document root?
    fn is_document(&self) -> bool {
        self.handle().is_document()
    }

    // ---- Style ----

    /// Per-property style access — like JavaScript's `element.style`.
    fn style(&self) -> crate::styling::builder::StyleAccess {
        self.handle().style()
    }

    /// Apply styles via closure and return self for chaining.
    ///
    /// ```ignore
    /// doc.div()
    ///     .styled(|s| s.width(px(200.0)).height(px(100.0)).background_color(rgb8(255, 0, 0)))
    ///     .child(doc.div().styled(|s| s.width(px(50.0)).height(px(50.0))))
    /// ```
    fn styled(self, f: impl FnOnce(&mut crate::styling::builder::StyleAccess)) -> Self {
        f(&mut self.handle().style());
        self
    }
}

/// Nodes that can have children: Element and Document.
///
/// Like Chrome's `ContainerNode` (`core/dom/container_node.h`).
/// Text nodes do NOT implement this — `text.append(child)` is a **compile error**.
pub trait ContainerNode: Node {
    /// Append `child` as the last child of this node.
    ///
    /// If `child` is already attached elsewhere, it is detached first (reparenting).
    fn append(&self, child: impl HasHandle) {
        self.handle().append(child.handle());
    }

    /// Append a child and return self for chaining (GPUI-style).
    ///
    /// Elements are `Copy` — zero-cost value return.
    ///
    /// ```ignore
    /// doc.root().child(
    ///     doc.div()
    ///         .child(doc.div().bg(rgb8(255, 0, 0)))
    ///         .child(doc.div().bg(rgb8(0, 255, 0)))
    /// );
    /// ```
    fn child(self, child: impl HasHandle) -> Self {
        self.handle().append(child.handle());
        self
    }

    /// Append multiple children at once.
    fn add_children<I, C>(self, items: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: HasHandle,
    {
        for c in items {
            self.handle().append(c.handle());
        }
        self
    }

    /// Insert `child` before this node in the parent's child list.
    fn insert_before(&self, child: impl HasHandle) {
        self.handle().insert_before(child.handle());
    }

    /// Get the first child, or `None` if empty.
    fn first_child(&self) -> Option<Handle> {
        self.handle().first_child()
    }

    /// Get the last child, or `None` if empty.
    fn last_child(&self) -> Option<Handle> {
        self.handle().last_child()
    }

    /// Collect all children as Handles.
    fn children(&self) -> Vec<Handle> {
        self.handle().children()
    }
}

/// Element nodes — attributes, tag name, class, id.
///
/// Like Chrome's `Element` (`core/dom/element.h`).
/// Text and Document do NOT implement this.
pub trait Element: ContainerNode {
    /// Element-specific data type (e.g., `ButtonData`). Use `()` if none.
    type Data: Default + 'static;

    /// The default tag name (e.g., `"div"`, `"button"`, `"input"`).
    ///
    /// Empty string means this element type has no fixed tag and MUST be
    /// created via `Document::create_with_tag()` (e.g., `HtmlHeadingElement`
    /// represents h1-h6 — the tag varies at runtime).
    ///
    /// Chrome: tag name is always runtime data on `Element::tagName()`.
    /// This const is a Kozan convenience for single-tag elements.
    const TAG_NAME: &'static str = "";

    /// Whether this element is focusable by default.
    const IS_FOCUSABLE: bool = false;

    /// Per-element default event handler (Chrome: `HTMLElement::DefaultEventHandler`).
    ///
    /// Override to add activation behavior (e.g., Enter/Space → click on buttons).
    const DEFAULT_EVENT_HANDLER: Option<crate::dom::node::DefaultEventHandlerFn> = None;

    /// Wrap a raw Handle into this element's typed handle.
    ///
    /// Called by [`Document::create()`](crate::Document::create).
    fn from_handle(handle: Handle) -> Self;

    /// Get the tag name (runtime — reads from `ElementData`).
    ///
    /// Usually matches `TAG_NAME`, but can differ for elements created
    /// via `Document::create_with_tag()` (e.g., `HtmlHeadingElement` represents h1-h6).
    /// Chrome: `Element::tagName()` always returns the actual tag.
    fn tag_name(&self) -> &'static str {
        self.handle().tag_name().unwrap_or(Self::TAG_NAME)
    }

    /// Get the `id` attribute.
    fn id(&self) -> String {
        self.handle().id()
    }

    /// Set the `id` attribute.
    fn set_id(&self, id: impl Into<String>) {
        self.handle().set_id(id);
    }

    /// Get the `class` attribute.
    fn class_name(&self) -> String {
        self.handle().class_name()
    }

    /// Set the `class` attribute.
    fn set_class_name(&self, class: impl Into<String>) {
        self.handle().set_class_name(class);
    }

    // ── ClassList — Chrome: element.classList (DOMTokenList) ──

    /// Add a CSS class. No-op if already present.
    /// Chrome: `element.classList.add("name")`.
    fn class_add(&self, name: &str) {
        self.handle().class_add(name);
    }

    /// Remove a CSS class. No-op if absent.
    /// Chrome: `element.classList.remove("name")`.
    fn class_remove(&self, name: &str) {
        self.handle().class_remove(name);
    }

    /// Toggle a CSS class. Returns true if now present.
    /// Chrome: `element.classList.toggle("name")`.
    fn class_toggle(&self, name: &str) -> bool {
        self.handle().class_toggle(name)
    }

    /// Check if an element has a CSS class.
    /// Chrome: `element.classList.contains("name")`.
    fn class_contains(&self, name: &str) -> bool {
        self.handle().class_contains(name)
    }

    // ── Attributes ──

    /// Returns the attribute value for `name`, or `None` if absent.
    fn attribute(&self, name: &str) -> Option<String> {
        self.handle().attribute(name)
    }

    /// Set an attribute.
    fn set_attribute(&self, name: &str, value: impl Into<String>) {
        self.handle().set_attribute(name, value);
    }

    /// Remove an attribute. Returns the old value.
    fn remove_attribute(&self, name: &str) -> Option<String> {
        self.handle().remove_attribute(name)
    }

    /// Check if an attribute is present.
    fn has_attribute(&self, name: &str) -> bool {
        self.handle().attribute(name).is_some()
    }

    // ── CSSOM View — layout geometry ──

    /// Border-box width after layout (CSSOM `offsetWidth`).
    fn offset_width(&self) -> f32 {
        self.handle().offset_width()
    }

    /// Border-box height after layout (CSSOM `offsetHeight`).
    fn offset_height(&self) -> f32 {
        self.handle().offset_height()
    }
}
