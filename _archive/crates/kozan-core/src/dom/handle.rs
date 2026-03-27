//! Handle — the 16-byte universal node reference.
//!
//! # Thread safety
//!
//! `Handle` is `Send` but NOT `Sync`.
//!
//! **Send**: A `Handle` can be moved into a `WakeSender` callback that
//! runs on the view thread. This is sound because:
//!   1. `WakeSender` callbacks are guaranteed to execute on the same view
//!      thread that owns the `Document`.
//!   2. The `Document` is alive for the lifetime of the view thread.
//!   3. There is never concurrent access — the handle is moved, not shared.
//!
//! **!Sync**: Sharing `&Handle` across threads is NOT allowed. Mutations go
//! through `DocumentCell` which has no internal locking — concurrent `&mut`
//! from two threads would be UB. `!Sync` prevents this at compile time.
//!
//! This mirrors Chrome's `WeakPtr<Node>` used in cross-thread task posting:
//! the pointer is only dereferenced on the owning thread, but the closure
//! carrying it is sent through the IPC/task channel.

use core::marker::PhantomData;

use crate::dom::document_cell::DocumentCell;
use crate::dom::node::NodeType;
use crate::id::{INVALID, RawId};

/// A universal node handle. 16 bytes, `Copy`, `Send`, `!Sync`.
///
/// Internally: `RawId` (8 bytes) + `DocumentCell` (8 bytes).
/// All node types are newtypes around this.
///
/// See module-level doc for the thread-safety invariants.
#[derive(Copy, Clone)]
pub struct Handle {
    pub(crate) id: RawId,
    pub(crate) cell: DocumentCell,
    /// Suppresses `Sync` (shared-reference access from multiple threads)
    /// while allowing `Send` (moving ownership to another thread).
    _no_sync: PhantomData<*const ()>,
}

// SAFETY: See module-level doc. Callbacks carrying a Handle always run on
// the view thread that owns the Document — no concurrent access occurs.
unsafe impl Send for Handle {}

impl Handle {
    #[inline]
    pub(crate) fn new(id: RawId, cell: DocumentCell) -> Self {
        Self {
            id,
            cell,
            _no_sync: PhantomData,
        }
    }

    /// The raw node ID (index + generation). Can be sent across threads.
    #[inline]
    #[must_use]
    pub fn raw(&self) -> RawId {
        self.id
    }

    /// Check if this node is still alive.
    #[inline]
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.cell.read(|doc| doc.is_alive_id(self.id))
    }

    // ---- Data access ----

    /// Read element-specific data through a scoped closure.
    #[inline]
    pub fn read_data<D: 'static, R: 'static>(&self, f: impl FnOnce(&D) -> R) -> Option<R> {
        self.cell.check_alive();
        self.cell.read(|doc| doc.read_data(self.id, f))
    }

    /// Write element-specific data. Marks dirty.
    #[inline]
    pub fn write_data<D: 'static, R: 'static>(&self, f: impl FnOnce(&mut D) -> R) -> Option<R> {
        self.cell.check_alive();
        self.cell.write(|doc| doc.write_data(self.id, f))
    }

    /// Mark this node's parent element for layout only (not restyle).
    ///
    /// Used by text nodes: text content changes affect the parent's layout
    /// (text sizing). Chrome: `CharacterData::DidModifyData()` →
    /// `ContainerNode::ChildrenChanged()` → `SetNeedsStyleRecalc()`.
    pub(crate) fn mark_parent_needs_layout(&self) {
        self.cell.write(|doc| {
            doc.mark_child_content_changed(self.id.index());
        });
    }

    // ---- Element data (attributes) ----

    /// Tag name (e.g., "div", "button"). None for non-element nodes.
    #[must_use]
    pub fn tag_name(&self) -> Option<&'static str> {
        self.cell
            .read(|doc| doc.read_element_data(self.id, |ed| ed.tag_name))
    }

    /// Get the `id` attribute.
    #[must_use]
    pub fn id(&self) -> String {
        self.attribute("id").unwrap_or_default()
    }

    /// Set the `id` attribute.
    pub fn set_id(&self, id: impl Into<String>) {
        self.set_attribute("id", id);
    }

    /// Get the `class` attribute.
    #[must_use]
    pub fn class_name(&self) -> String {
        self.attribute("class").unwrap_or_default()
    }

    /// Set the `class` attribute.
    pub fn set_class_name(&self, class: impl Into<String>) {
        self.set_attribute("class", class);
    }

    /// Returns the attribute value for `name`, or `None` if absent.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<String> {
        self.cell
            .read(|doc| {
                doc.read_element_data(self.id, |ed| ed.attributes.get(name).map(|v| v.to_string()))
            })
            .flatten()
    }

    /// Set an attribute.
    pub fn set_attribute(&self, name: &str, value: impl Into<String>) {
        let value = value.into();
        let index = self.id.index();
        self.cell.write(|doc| {
            let guard = doc.shared_lock().clone();
            doc.write_element_data(self.id, |ed| {
                ed.on_attribute_set(name, &value, &guard);
                ed.attributes.set(name, value);
            });
            doc.mark_for_restyle(index);
        });
    }

    /// Remove an attribute.
    #[must_use]
    pub fn remove_attribute(&self, name: &str) -> Option<String> {
        let index = self.id.index();
        self.cell.write(|doc| {
            let removed = doc
                .write_element_data(self.id, |ed| {
                    ed.on_attribute_removed(name);
                    ed.attributes.remove(name)
                })
                .flatten();
            doc.mark_for_restyle(index);
            removed
        })
    }

    // ---- ClassList — Chrome: element.classList (DOMTokenList) ----

    /// Add a CSS class. No-op if already present.
    /// Chrome equivalent: `element.classList.add("name")`.
    pub fn class_add(&self, name: &str) {
        let index = self.id.index();
        self.cell.write(|doc| {
            let changed = doc
                .write_element_data(self.id, |ed| ed.class_add(name))
                .unwrap_or(false);
            if changed {
                doc.mark_for_restyle(index);
            }
        });
    }

    /// Remove a CSS class. No-op if not present.
    /// Chrome equivalent: `element.classList.remove("name")`.
    pub fn class_remove(&self, name: &str) {
        let index = self.id.index();
        self.cell.write(|doc| {
            let changed = doc
                .write_element_data(self.id, |ed| ed.class_remove(name))
                .unwrap_or(false);
            if changed {
                doc.mark_for_restyle(index);
            }
        });
    }

    /// Toggle a CSS class. Returns true if now present.
    /// Chrome equivalent: `element.classList.toggle("name")`.
    #[must_use]
    pub fn class_toggle(&self, name: &str) -> bool {
        let index = self.id.index();
        self.cell.write(|doc| {
            let present = doc
                .write_element_data(self.id, |ed| ed.class_toggle(name))
                .unwrap_or(false);
            doc.mark_for_restyle(index);
            present
        })
    }

    /// Check if an element has a CSS class.
    /// Chrome equivalent: `element.classList.contains("name")`.
    #[must_use]
    pub fn class_contains(&self, name: &str) -> bool {
        self.cell
            .read(|doc| doc.read_element_data(self.id, |ed| ed.class_contains(name)))
            .unwrap_or(false)
    }

    // ---- Tree operations ----

    /// Append `child` as the last child.
    ///
    /// Accepts any node type (`HtmlDivElement`, `Text`, `Handle`, etc.).
    pub fn append(&self, child: impl Into<Handle>) -> &Self {
        let child = child.into();
        self.cell.check_alive();
        self.cell.write(|doc| doc.append_child(self.id, child.id));
        self
    }

    /// Append a child and return self (Copy) for chaining.
    ///
    /// ```ignore
    /// doc.root()
    ///     .child(doc.div())
    ///     .child(doc.div());
    /// ```
    pub fn child(self, child: impl Into<Handle>) -> Self {
        self.append(child);
        self
    }

    /// Append multiple children at once.
    pub fn add_children<I, C>(self, items: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Handle>,
    {
        self.cell.check_alive();
        self.cell.write(|doc| {
            for child in items {
                doc.append_child(self.id, child.into().id);
            }
        });
        self
    }

    /// Insert `child` before this node.
    pub fn insert_before(&self, child: impl Into<Handle>) {
        let child = child.into();
        self.cell.check_alive();
        self.cell.write(|doc| doc.insert_before(self.id, child.id));
    }

    /// Remove from parent.
    pub fn detach(&self) {
        self.cell.check_alive();
        self.cell.write(|doc| doc.detach_node(self.id));
    }

    /// Destroy this node. All handles become stale.
    pub fn destroy(&self) {
        self.cell.check_alive();
        self.cell.write(|doc| doc.destroy_node(self.id));
    }

    // ---- Tree queries ----

    /// Parent node.
    #[must_use]
    pub fn parent(&self) -> Option<Handle> {
        self.cell
            .read(|doc| {
                let tree = doc.tree_data(self.id)?;
                if tree.parent == INVALID {
                    return None;
                }
                let id = doc.raw_id(tree.parent)?;
                Some(id)
            })
            .map(|id| Handle::new(id, self.cell))
    }

    /// First child.
    #[must_use]
    pub fn first_child(&self) -> Option<Handle> {
        self.cell
            .read(|doc| {
                let tree = doc.tree_data(self.id)?;
                if tree.first_child == INVALID {
                    return None;
                }
                doc.raw_id(tree.first_child)
            })
            .map(|id| Handle::new(id, self.cell))
    }

    /// Last child.
    #[must_use]
    pub fn last_child(&self) -> Option<Handle> {
        self.cell
            .read(|doc| {
                let tree = doc.tree_data(self.id)?;
                if tree.last_child == INVALID {
                    return None;
                }
                doc.raw_id(tree.last_child)
            })
            .map(|id| Handle::new(id, self.cell))
    }

    /// Next sibling.
    #[must_use]
    pub fn next_sibling(&self) -> Option<Handle> {
        self.cell
            .read(|doc| {
                let tree = doc.tree_data(self.id)?;
                if tree.next_sibling == INVALID {
                    return None;
                }
                doc.raw_id(tree.next_sibling)
            })
            .map(|id| Handle::new(id, self.cell))
    }

    /// Previous sibling.
    #[must_use]
    pub fn prev_sibling(&self) -> Option<Handle> {
        self.cell
            .read(|doc| {
                let tree = doc.tree_data(self.id)?;
                if tree.prev_sibling == INVALID {
                    return None;
                }
                doc.raw_id(tree.prev_sibling)
            })
            .map(|id| Handle::new(id, self.cell))
    }

    /// All children.
    #[must_use]
    pub fn children(&self) -> Vec<Handle> {
        let ids = self.cell.read(|doc| doc.children_ids(self.id));
        ids.into_iter()
            .map(|id| Handle::new(id, self.cell))
            .collect()
    }

    /// All children that are Elements, collected in a single cell read.
    ///
    /// Prefer this over iterating with `first_element_child()`/`next_element_sibling()`
    /// which each do a separate cell read per sibling.
    #[must_use]
    pub fn element_children(&self) -> Vec<Handle> {
        self.cell.read(|doc| {
            let Some(td) = doc.tree_data(self.id) else {
                return Vec::new();
            };
            let mut result = Vec::new();
            let mut idx = td.first_child;
            while idx != INVALID {
                let Some(child_td) = doc.tree_data_by_index(idx) else {
                    break;
                };
                if let Some(meta) = doc.node_meta_by_index(idx) {
                    if meta.flags().is_element() {
                        if let Some(raw) = doc.raw_id(idx) {
                            result.push(Handle::new(raw, self.cell));
                        }
                    }
                }
                idx = child_td.next_sibling;
            }
            result
        })
    }

    /// First child that is an Element (skip Text, Comment, etc.).
    #[must_use]
    pub fn first_element_child(&self) -> Option<Handle> {
        let mut cursor = self.first_child();
        while let Some(node) = cursor {
            if node.is_element() {
                return Some(node);
            }
            cursor = node.next_sibling();
        }
        None
    }

    /// Last child that is an Element.
    #[must_use]
    pub fn last_element_child(&self) -> Option<Handle> {
        let mut cursor = self.last_child();
        while let Some(node) = cursor {
            if node.is_element() {
                return Some(node);
            }
            cursor = node.prev_sibling();
        }
        None
    }

    /// Next sibling that is an Element.
    #[must_use]
    pub fn next_element_sibling(&self) -> Option<Handle> {
        let mut cursor = self.next_sibling();
        while let Some(node) = cursor {
            if node.is_element() {
                return Some(node);
            }
            cursor = node.next_sibling();
        }
        None
    }

    /// Previous sibling that is an Element.
    #[must_use]
    pub fn prev_element_sibling(&self) -> Option<Handle> {
        let mut cursor = self.prev_sibling();
        while let Some(node) = cursor {
            if node.is_element() {
                return Some(node);
            }
            cursor = node.prev_sibling();
        }
        None
    }

    // ---- Node kind ----

    #[must_use]
    pub fn node_kind(&self) -> Option<NodeType> {
        self.cell.read(|doc| doc.node_kind(self.id))
    }
    #[must_use]
    pub fn is_element(&self) -> bool {
        self.node_kind() == Some(NodeType::Element)
    }
    #[must_use]
    pub fn is_text(&self) -> bool {
        self.node_kind() == Some(NodeType::Text)
    }
    #[must_use]
    pub fn is_document(&self) -> bool {
        self.node_kind() == Some(NodeType::Document)
    }

    // ---- CSSOM View — layout geometry ----

    /// Border-box width after layout.
    ///
    /// CSSOM View: `HTMLElement.offsetWidth`. Returns the element's laid-out
    /// border-box width in CSS pixels. Returns `0.0` before first layout.
    #[must_use]
    pub fn offset_width(&self) -> f32 {
        self.cell.read(|doc| doc.layout_border_box(self.id).0)
    }

    /// Border-box height after layout.
    ///
    /// CSSOM View: `HTMLElement.offsetHeight`. Returns the element's laid-out
    /// border-box height in CSS pixels. Returns `0.0` before first layout.
    #[must_use]
    pub fn offset_height(&self) -> f32 {
        self.cell.read(|doc| doc.layout_border_box(self.id).1)
    }

    // ---- Style ----

    /// Per-property style access — like JavaScript's `element.style`.
    ///
    /// ```ignore
    /// div.style().color(AbsoluteColor::RED);
    /// div.style().display(Display::Flex);
    /// ```
    #[must_use]
    pub fn style(&self) -> crate::styling::builder::StyleAccess {
        crate::styling::builder::StyleAccess::new(self.cell, self.id)
    }

    // ---- Overscroll behavior ----

    /// Set CSS `overscroll-behavior` for both axes.
    // ---- Event dispatch (for untyped handles from hit testing) ----

    /// Dispatch an event to this node.
    ///
    /// Runs the full capture -> target -> bubble pipeline.
    /// Returns `true` if the default action was NOT prevented.
    ///
    /// This mirrors `EventTarget::dispatch_event()` but works on raw `Handle`
    /// without requiring the `HasHandle` trait (which would conflict with the
    /// blanket `From<T: HasHandle>` impl). Used by `EventHandler` after hit
    /// testing returns a node index resolved to a Handle.
    pub fn dispatch_event(&self, event: &dyn crate::events::Event) -> bool {
        if !self.is_alive() {
            return false;
        }
        let mut store = crate::events::dispatcher::EventStoreAccess::new(self.cell);
        crate::events::dispatch(self.cell, self.id, event, &mut store)
    }

    /// Chrome: `Node::DefaultEventHandler()` — per-element default behavior.
    ///
    /// Called after the bubble phase when `!preventDefault`. Each element type
    /// can handle events before they fall through to browser-level defaults.
    pub(crate) fn default_event_handler(&self, event: &dyn std::any::Any) -> bool {
        let handler = self.cell.read(|doc| {
            doc.node_meta(self.id)
                .and_then(|meta| meta.default_event_handler())
        });
        match handler {
            Some(f) => f(self, event),
            None => false,
        }
    }

    /// Programmatic focus (W3C `HTMLElement.focus()`).
    pub fn focus(&self) {
        if !self.is_alive() {
            return;
        }
        // Mutate state under cell.write(), then resolve handles and dispatch
        // events with no cell borrow held — event handlers do their own
        // cell access so nesting would alias &Document and &mut Document.
        let (old, new_id) = self.cell.write(|doc| {
            let old = doc.focused_element();
            let new_id = doc.raw_id(self.id.index());
            if old == new_id {
                return (None, None);
            }
            doc.apply_focus_state(old, new_id, false);
            (old, new_id)
        });
        if old.is_some() || new_id.is_some() {
            Self::fire_focus_events(self.cell, old, new_id);
        }
    }

    /// Programmatic blur (W3C `HTMLElement.blur()`).
    pub fn blur(&self) {
        let old = self.cell.write(|doc| {
            if doc.focused_element() != Some(self.id) {
                return None;
            }
            let old = doc.focused_element();
            doc.apply_focus_state(old, None, false);
            old
        });
        if let Some(old) = old {
            Self::fire_focus_events(self.cell, Some(old), None);
        }
    }

    /// UI Events §5.2.2: focusout(A) → focusin(B) → blur(A) → focus(B).
    ///
    /// Resolves handles under a cell read, then dispatches events with no
    /// cell borrow held so handlers can safely access the document.
    fn fire_focus_events(cell: DocumentCell, old: Option<RawId>, new: Option<RawId>) {
        use crate::events::focus_event::*;

        let (old_handle, new_handle) = cell.read(|doc| {
            (
                old.and_then(|id| doc.resolve(id)),
                new.and_then(|id| doc.resolve(id)),
            )
        });

        if let Some(h) = &old_handle {
            h.dispatch_event(&FocusOutEvent { related_target: new });
        }
        if let Some(h) = &new_handle {
            h.dispatch_event(&FocusInEvent { related_target: old });
        }
        if let Some(h) = &old_handle {
            h.dispatch_event(&BlurEvent { related_target: new });
        }
        if let Some(h) = &new_handle {
            h.dispatch_event(&FocusEvent { related_target: old });
        }
    }
}

/// Any type with `HasHandle` can convert to `Handle`.
/// Enables `doc.root().append(btn)` instead of `doc.root().append(btn.handle())`.
impl<T: crate::dom::traits::HasHandle> From<T> for Handle {
    #[inline]
    fn from(value: T) -> Self {
        value.handle()
    }
}

impl core::fmt::Debug for Handle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Handle({:?})", self.id)
    }
}
