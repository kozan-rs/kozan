//! Document — owns all node data through parallel arenas.
//!
//! Single owner of all DOM state. `DocumentCell` wraps a raw pointer for
//! internal subsystems that need unchecked access.

mod layout;

use core::any::TypeId;
use core::ptr::NonNull;

use crate::data_storage::DataStorage;
use crate::dom::document_cell::DocumentCell;
use crate::dom::element_data::ElementData;
use crate::dom::handle::Handle;
use crate::dom::node::{NodeFlags, NodeMeta};
use crate::dom::traits::Element;
use crate::dom::traits::HasHandle;
use crate::events::EventListenerMap;
use crate::events::listener::RegisteredListener;
use crate::events::store::EventStore;
use crate::id::{INVALID, IdAllocator, RawId};
use crate::layout::node_data::LayoutNodeData;
use crate::styling::StyleEngine;
use crate::tree;
use crate::tree::TreeData;
use crate::{Text, TextData};
use kozan_primitives::arena::Storage;

/// A document — the top-level owner of a node tree.
///
/// Owns all node data through parallel arenas. Internal subsystems
/// (layout, style, events) access data through `DocumentCell`.
///
/// The owner must keep Document at a stable address. Handles store
/// raw pointers back to this Document through `DocumentCell`.
pub struct Document {
    ids: IdAllocator,
    meta: Storage<NodeMeta>,
    tree: Storage<TreeData>,
    element_data: Storage<ElementData>,
    data: DataStorage,
    layout: Storage<LayoutNodeData>,
    style_engine: StyleEngine,
    event_store: EventStore,
    root: u32,
    body: u32,
    /// Chrome: `Document::focused_element_`. Authoritative focus state.
    focused_element: Option<RawId>,
    /// Chrome: `Document::hover_element_`. LCA diff drives state toggling.
    hover_element: Option<u32>,
    /// Chrome: `Document::active_element_`. LCA diff drives state toggling.
    active_element: Option<u32>,
    /// Chrome: `DocumentLifecycle` — enforces phase ordering.
    lifecycle: crate::lifecycle::LifecycleState,
    /// DOM structure changed (append, remove, detach) — requires full tree rebuild.
    needs_tree_rebuild: bool,
    /// DOM node indices whose content changed (text update) — incremental relayout.
    dirty_layout_nodes: Vec<u32>,
    /// Style changed — needs restyle + relayout.
    needs_style_recalc: bool,
    #[cfg(debug_assertions)]
    alive: bool,
}

impl Document {
    /// Create a new empty document with a root node.
    #[must_use]
    pub fn new() -> Self {
        let mut doc = Self {
            ids: IdAllocator::new(),
            meta: Storage::new(),
            tree: Storage::new(),
            element_data: Storage::new(),
            data: DataStorage::new(),
            layout: Storage::new(),
            style_engine: StyleEngine::new(),
            event_store: EventStore::new(),
            root: 0,
            body: 0,
            focused_element: None,
            hover_element: None,
            active_element: None,
            lifecycle: crate::lifecycle::LifecycleState::default(),
            needs_tree_rebuild: true,
            dirty_layout_nodes: Vec::new(),
            needs_style_recalc: false,
            #[cfg(debug_assertions)]
            alive: true,
        };

        // Root (document node — like <html>)
        let (root_index, _gen) = doc.alloc_node(NodeFlags::document(), TypeId::of::<()>(), None);
        doc.root = root_index;

        doc
    }

    /// Initialize the `<body>` element. Called after Document is pinned
    /// in memory (needs stable address for Handle operations).
    ///
    /// Creates a real `<body>` element — UA stylesheet provides:
    /// `body { display: block; margin: 8px; }`
    pub fn init_body(&mut self) {
        if self.body != 0 {
            return; // already initialized
        }
        let body = self.create::<crate::html::HtmlBodyElement>();
        self.root().append(body);
        self.body = body.handle().raw().index();
    }

    /// Get a `DocumentCell` for internal subsystem access.
    ///
    /// The caller must ensure `self` stays at a stable address.
    pub(crate) fn cell(&self) -> DocumentCell {
        DocumentCell::new(NonNull::from(self))
    }

    /// Allocate a new node with all parallel storages initialized.
    pub(crate) fn alloc_node(
        &mut self,
        flags: NodeFlags,
        data_type_id: TypeId,
        default_handler: Option<crate::dom::node::DefaultEventHandlerFn>,
    ) -> (u32, u32) {
        let raw = self.ids.alloc();
        let (index, generation) = (raw.index(), raw.generation());
        self.meta
            .set(index, NodeMeta::new(flags, data_type_id, default_handler));
        self.tree.set(index, TreeData::detached());
        self.layout.set(index, LayoutNodeData::new());
        self.event_store.ensure_slot(index);

        unsafe {
            self.meta.get_unchecked_mut(index).flags_mut().mark_style_dirty();
            self.meta.get_unchecked_mut(index).flags_mut().mark_tree_dirty();
        }

        (index, generation)
    }

    // ── Internal subsystem methods (used via DocumentCell::read/write) ──

    /// Check if a node is alive by `RawId`.
    pub(crate) fn is_alive_id(&self, id: RawId) -> bool {
        self.ids.is_alive(id)
    }

    /// Get the current generation for an index.
    pub(crate) fn generation(&self, index: u32) -> Option<u32> {
        if (index as usize) < self.ids.capacity() {
            Some(unsafe { self.ids.generation_unchecked(index) })
        } else {
            None
        }
    }

    /// Build a `RawId` from a raw index by looking up its current generation.
    pub(crate) fn raw_id(&self, index: u32) -> Option<RawId> {
        if (index as usize) >= self.ids.capacity() {
            return None;
        }
        let generation = unsafe { self.ids.generation_unchecked(index) };
        let id = RawId::new(index, generation);
        if self.ids.is_alive(id) {
            Some(id)
        } else {
            None
        }
    }

    /// Get node metadata by `RawId` (with liveness check).
    pub(crate) fn node_meta(&self, id: RawId) -> Option<crate::dom::node::NodeMeta> {
        if !self.ids.is_alive(id) {
            return None;
        }
        self.meta.get(id.index()).copied()
    }

    /// Get node type by `RawId`.
    pub(crate) fn node_kind(&self, id: RawId) -> Option<crate::dom::node::NodeType> {
        self.node_meta(id).map(|m| m.flags().node_type())
    }

    /// Get tree data by `RawId` (with liveness check).
    pub(crate) fn tree_data(&self, id: RawId) -> Option<crate::tree::TreeData> {
        if !self.ids.is_alive(id) {
            return None;
        }
        self.tree.get(id.index()).copied()
    }

    /// Get children as `RawIds`.
    pub(crate) fn children_ids(&self, id: RawId) -> Vec<RawId> {
        if !self.ids.is_alive(id) {
            return Vec::new();
        }
        let indices = unsafe { tree::ops::children(&self.tree, id.index()) };
        indices
            .into_iter()
            .map(|idx| {
                let child_gen = unsafe { self.ids.generation_unchecked(idx) };
                RawId::new(idx, child_gen)
            })
            .collect()
    }

    /// Read element-type-specific data (`ButtonData`, `TextInputData`, etc.).
    pub(crate) fn read_data<D: 'static, R: 'static>(
        &self,
        id: RawId,
        f: impl FnOnce(&D) -> R,
    ) -> Option<R> {
        if !self.ids.is_alive(id) {
            return None;
        }
        let meta = self.meta.get(id.index())?;
        if meta.data_type_id() != TypeId::of::<D>() {
            return None;
        }
        let data = unsafe { self.data.get::<D>(id.index()) };
        Some(f(data))
    }

    /// Write element-type-specific data. Marks the node dirty.
    pub(crate) fn write_data<D: 'static, R: 'static>(
        &mut self,
        id: RawId,
        f: impl FnOnce(&mut D) -> R,
    ) -> Option<R> {
        if !self.ids.is_alive(id) {
            return None;
        }
        let meta = self.meta.get(id.index())?;
        if meta.data_type_id() != TypeId::of::<D>() {
            return None;
        }
        let data = unsafe { self.data.get_mut::<D>(id.index()) };
        let result = f(data);
        unsafe {
            self.meta
                .get_unchecked_mut(id.index())
                .flags_mut()
                .mark_paint_dirty();
        }
        Some(result)
    }

    /// Read shared element data (attributes, id, class, focus state).
    pub(crate) fn read_element_data<R: 'static>(
        &self,
        id: RawId,
        f: impl FnOnce(&ElementData) -> R,
    ) -> Option<R> {
        if !self.ids.is_alive(id) {
            return None;
        }
        let meta = self.meta.get(id.index())?;
        if !meta.flags().is_element() {
            return None;
        }
        let ed = self.element_data.get(id.index())?;
        Some(f(ed))
    }

    /// Write shared element data. Marks the node dirty.
    pub(crate) fn write_element_data<R: 'static>(
        &mut self,
        id: RawId,
        f: impl FnOnce(&mut ElementData) -> R,
    ) -> Option<R> {
        if !self.ids.is_alive(id) {
            return None;
        }
        let meta = self.meta.get(id.index())?;
        if !meta.flags().is_element() {
            return None;
        }
        let ed = self.element_data.get_mut(id.index())?;
        let result = f(ed);
        unsafe {
            self.meta
                .get_unchecked_mut(id.index())
                .flags_mut()
                .mark_style_dirty();
        }
        // Propagate dirty_descendants up to root for Stylo's pre_traverse.
        self.propagate_dirty_ancestors(id.index());
        Some(result)
    }

    /// Walk up the tree setting `dirty_descendants` on each ancestor's `ElementData`.
    /// Stops when an ancestor already has the flag set (already propagated).
    pub(crate) fn propagate_dirty_ancestors(&mut self, index: u32) {
        let mut current = index;
        loop {
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => {
                    let parent = td.parent;
                    if let Some(ed) = self.element_data.get(parent) {
                        if ed.dirty_descendants.get() {
                            break;
                        }
                        ed.dirty_descendants.set(true);
                    }
                    current = parent;
                }
                _ => break,
            }
        }
    }

    // ── Tree mutations ──

    /// Append `child` as the last child of `parent`.
    pub(crate) fn append_child(&mut self, parent: RawId, child: RawId) {
        debug_assert!(
            !matches!(self.lifecycle, crate::lifecycle::LifecycleState::InLayout | crate::lifecycle::LifecycleState::InPaint),
            "DOM mutation during {:?} — Chrome: tree mutations are forbidden during layout/paint",
            self.lifecycle
        );
        if !self.ids.is_alive(parent) {
            return;
        }
        if !self.ids.is_alive(child) {
            return;
        }
        if let Some(meta) = self.meta.get(parent.index()) {
            if !meta.flags().is_container() {
                return;
            }
        } else {
            return;
        }
        if unsafe { tree::ops::is_ancestor(&self.tree, child.index(), parent.index()) } {
            return;
        }
        unsafe {
            tree::ops::append(&mut self.tree, parent.index(), child.index());
            self.meta
                .get_unchecked_mut(parent.index())
                .flags_mut()
                .mark_tree_dirty();
            self.meta
                .get_unchecked_mut(child.index())
                .flags_mut()
                .mark_tree_dirty();
        }
        self.needs_tree_rebuild = true;
    }

    /// Insert `child` before `ref_id` in the child list.
    pub(crate) fn insert_before(&mut self, ref_id: RawId, child: RawId) {
        debug_assert!(
            !matches!(self.lifecycle, crate::lifecycle::LifecycleState::InLayout | crate::lifecycle::LifecycleState::InPaint),
            "DOM mutation during {:?}",
            self.lifecycle
        );
        if !self.ids.is_alive(ref_id) {
            return;
        }
        if !self.ids.is_alive(child) {
            return;
        }
        if let Some(tree) = self.tree.get(ref_id.index()) {
            if !tree.has_parent() {
                return;
            }
        } else {
            return;
        }
        if unsafe { tree::ops::is_ancestor(&self.tree, child.index(), ref_id.index()) } {
            return;
        }
        unsafe {
            tree::ops::insert_before(&mut self.tree, ref_id.index(), child.index());
            self.meta
                .get_unchecked_mut(child.index())
                .flags_mut()
                .mark_tree_dirty();
        }
        self.needs_tree_rebuild = true;
    }

    /// Detach a node from the tree.
    pub(crate) fn detach_node(&mut self, id: RawId) {
        debug_assert!(
            !matches!(self.lifecycle, crate::lifecycle::LifecycleState::InLayout | crate::lifecycle::LifecycleState::InPaint),
            "DOM mutation during {:?}",
            self.lifecycle
        );
        if !self.ids.is_alive(id) {
            return;
        }
        let parent = self
            .tree
            .get(id.index())
            .map_or(crate::id::INVALID, |t| t.parent);
        unsafe {
            tree::ops::detach(&mut self.tree, id.index());
        }
        if parent != crate::id::INVALID {
            unsafe {
                self.meta.get_unchecked_mut(parent).flags_mut().mark_tree_dirty();
            }
            self.needs_tree_rebuild = true;
        }
    }

    /// Destroy a node: detach, drop data, free slot.
    pub(crate) fn destroy_node(&mut self, id: RawId) {
        if !self.ids.is_alive(id) {
            return;
        }

        let index = id.index();

        // Chrome: Element::RemovedFrom — clear tracked state pointing to this
        // node so no stale references survive past slot reuse.
        if self.focused_element == Some(id) {
            // Direct field clear, not set_focused_element — the node is being
            // destroyed so firing blur/focusout events would hit a dead target.
            self.focused_element = None;
        }
        if self.hover_element == Some(index) {
            self.hover_element = None;
        }
        if self.active_element == Some(index) {
            self.active_element = None;
        }

        // Scrub interactive ElementState flags so a future occupant of this
        // slot doesn't inherit stale :hover / :active / :focus styling.
        if let Some(ed) = self.element_data.get_mut(index) {
            ed.element_state.remove(
                style_dom::ElementState::HOVER
                    | style_dom::ElementState::ACTIVE
                    | style_dom::ElementState::FOCUS
                    | style_dom::ElementState::FOCUS_WITHIN
                    | style_dom::ElementState::FOCUSRING,
            );
        }

        self.detach_node(id);
        // Drop element-specific data.
        if let Some(meta) = self.meta.get(id.index()).copied() {
            if meta.data_type_id() != TypeId::of::<()>() {
                self.data.remove(meta.data_type_id(), id.index());
            }
        }
        // Reset Stylo data on ElementData (prevents stale computed styles).
        if let Some(ed) = self.element_data.get_mut(id.index()) {
            ed.stylo_data = style::data::ElementDataWrapper::default();
        }
        // Clear layout data.
        self.layout.clear_slot(id.index());
        // Remove event listeners.
        self.event_store.remove_node(id.index());
        // Free slot (bumps generation, invalidates all handles).
        self.ids.free(id);
    }

    // ── Internal helpers (by raw index, for styling/layout tree walking) ──

    /// Get node metadata by raw index (no generation check).
    pub(crate) fn node_meta_by_index(&self, index: u32) -> Option<crate::dom::node::NodeMeta> {
        self.meta.get(index).copied()
    }

    /// Tag name by raw index.
    pub(crate) fn tag_name(&self, index: u32) -> Option<&'static str> {
        Some(self.element_data.get(index)?.tag_name)
    }

    /// Get tree data by raw index.
    pub(crate) fn tree_data_by_index(&self, index: u32) -> Option<TreeData> {
        self.tree.get(index).copied()
    }

    /// Get the computed style for a node (from Stylo's `ElementData`).
    ///
    /// Returns `None` if styles haven't been computed yet or for non-element nodes.
    pub fn computed_style(
        &self,
        index: u32,
    ) -> Option<servo_arc::Arc<style::properties::ComputedValues>> {
        let ed = self.element_data.get(index)?;
        let data = ed.stylo_data.borrow();
        if data.has_styles() {
            Some(data.styles.primary().clone())
        } else {
            None
        }
    }

    // ── Event listener access ──

    /// Get or create the per-node `EventListenerMap` (lazy allocation).
    pub(crate) fn ensure_event_listeners(&mut self, index: u32) -> &mut EventListenerMap {
        self.event_store.ensure_listeners(index)
    }

    /// Get the per-node `EventListenerMap` mutably, if it has been allocated.
    pub(crate) fn event_listeners_mut(&mut self, index: u32) -> Option<&mut EventListenerMap> {
        self.event_store.get_mut(index)
    }

    /// Take listeners for dispatch (take-call-put pattern).
    pub(crate) fn take_event_listeners(
        &mut self,
        index: u32,
        type_id: TypeId,
    ) -> Option<Vec<RegisteredListener>> {
        self.event_store.take(index, type_id)
    }

    /// Put listeners back after dispatch.
    pub(crate) fn put_event_listeners(
        &mut self,
        index: u32,
        type_id: TypeId,
        listeners: Vec<RegisteredListener>,
    ) {
        self.event_store.put(index, type_id, listeners);
    }

    /// Initialize typed data for a slot.
    pub(crate) fn init_typed_data<T: Default + 'static>(&mut self, index: u32) {
        self.data.init::<T>(index);
    }

    /// Set element data for a freshly allocated element.
    pub(crate) fn set_element_data_new(&mut self, index: u32, data: ElementData) {
        self.element_data.set(index, data);
    }

    /// Set text content for a freshly allocated text node.
    pub(crate) fn set_text_content_new(&mut self, index: u32, content: &str) {
        unsafe {
            self.data.get_mut::<TextData>(index).content = content.to_string();
        }
    }

    // ── Handle creation ──

    fn make_handle(&self, index: u32, generation: u32) -> Handle {
        Handle::new(RawId::new(index, generation), self.cell())
    }

    // ── Shorthand element creation ──

    pub fn div(&self) -> crate::html::HtmlDivElement {
        self.create::<crate::html::HtmlDivElement>()
    }
    pub fn span(&self) -> crate::html::HtmlSpanElement {
        self.create::<crate::html::HtmlSpanElement>()
    }
    pub fn p(&self) -> crate::html::HtmlParagraphElement {
        self.create::<crate::html::HtmlParagraphElement>()
    }
    pub fn button(&self) -> crate::html::HtmlButtonElement {
        self.create::<crate::html::HtmlButtonElement>()
    }
    pub fn img(&self) -> crate::html::HtmlImageElement {
        self.create::<crate::html::HtmlImageElement>()
    }
    pub fn input(&self) -> crate::html::HtmlInputElement {
        self.create::<crate::html::HtmlInputElement>()
    }
    pub fn label(&self) -> crate::html::HtmlLabelElement {
        self.create::<crate::html::HtmlLabelElement>()
    }
    pub fn h1(&self) -> crate::html::HtmlHeadingElement {
        self.create_heading(1)
    }
    pub fn h2(&self) -> crate::html::HtmlHeadingElement {
        self.create_heading(2)
    }
    pub fn h3(&self) -> crate::html::HtmlHeadingElement {
        self.create_heading(3)
    }
    pub fn text_node(&self, content: &str) -> Text {
        self.create_text(content)
    }

    pub fn div_in(&self, parent: impl Into<Handle>) -> crate::html::HtmlDivElement {
        let el = self.div();
        parent.into().append(el);
        el
    }

    pub fn span_in(&self, parent: impl Into<Handle>) -> crate::html::HtmlSpanElement {
        let el = self.span();
        parent.into().append(el);
        el
    }

    pub fn text_in(&self, parent: impl Into<Handle>, content: &str) -> Text {
        let t = self.text_node(content);
        parent.into().append(t);
        t
    }

    // ── Node creation (public API) ──

    pub fn create<T: Element>(&self) -> T {
        assert!(
            !T::TAG_NAME.is_empty(),
            "Element type has no fixed tag — use create_with_tag() instead"
        );
        self.create_with_tag::<T>(T::TAG_NAME)
    }

    pub fn create_with_tag<T: Element>(&self, tag: &'static str) -> T {
        let cell = self.cell();
        let (index, generation) = cell.write(|doc| {
            let (index, generation) =
                doc.alloc_node(NodeFlags::element(T::IS_FOCUSABLE), TypeId::of::<T::Data>(), T::DEFAULT_EVENT_HANDLER);
            doc.set_element_data_new(index, ElementData::new(tag, T::IS_FOCUSABLE));
            if TypeId::of::<T::Data>() != TypeId::of::<()>() {
                doc.init_typed_data::<T::Data>(index);
            }
            (index, generation)
        });

        T::from_handle(self.make_handle(index, generation))
    }

    pub fn create_heading(&self, level: u8) -> crate::html::HtmlHeadingElement {
        assert!(
            (1..=6).contains(&level),
            "heading level must be 1-6, got {level}"
        );
        let tag: &'static str = match level {
            1 => "h1",
            2 => "h2",
            3 => "h3",
            4 => "h4",
            5 => "h5",
            6 => "h6",
            _ => unreachable!(),
        };
        let elem = self.create_with_tag::<crate::html::HtmlHeadingElement>(tag);
        elem.set_level(level);
        elem
    }

    pub fn create_text(&self, content: &str) -> Text {
        let cell = self.cell();
        let content_owned = content.to_string();
        let (index, generation) = cell.write(|doc| {
            let (index, generation) = doc.alloc_node(NodeFlags::text(), TypeId::of::<TextData>(), None);
            doc.init_typed_data::<TextData>(index);
            doc.set_text_content_new(index, &content_owned);
            (index, generation)
        });

        Text::from_raw(self.make_handle(index, generation))
    }

    // ── Queries ──

    pub fn root(&self) -> Handle {
        Handle::new(RawId::new(self.root, 0), self.cell())
    }

    /// The `<body>` element — main content container.
    ///
    /// Like Chrome's `document.body`. Created by `init_body()` after the
    /// Document is pinned in memory. Styles come from the UA stylesheet:
    /// `body { display: block; margin: 8px; }`.
    ///
    /// All user content goes in `body()`, not `root()`.
    pub fn body(&self) -> Handle {
        debug_assert!(self.body != 0, "body() called before init_body()");
        Handle::new(RawId::new(self.body, 0), self.cell())
    }

    pub fn resolve(&self, raw: RawId) -> Option<Handle> {
        if self.ids.is_alive(raw) {
            Some(Handle::new(raw, self.cell()))
        } else {
            None
        }
    }

    pub fn node_count(&self) -> u32 {
        self.ids.count()
    }

    pub fn root_index(&self) -> u32 {
        self.root
    }

    pub(crate) fn root_handle(&self) -> Option<Handle> {
        self.handle_for_index(self.root)
    }

    // ── Lifecycle ──

    /// Advance the lifecycle to `target`. Debug-asserts forward progress.
    ///
    /// Chrome: `DocumentLifecycle::AdvanceTo()` — enforces that lifecycle
    /// only moves forward (style → layout → paint), never backwards.
    /// Chrome: `DocumentLifecycle::AdvanceTo()`.
    ///
    /// Allows forward progress within a frame AND restart from PaintClean
    /// (entering a new frame). PaintClean can transition to any `In*` state
    /// because DirtyPhases may skip earlier phases (e.g., scroll dirties
    /// only paint, so style+layout are skipped).
    pub(crate) fn advance_lifecycle(&mut self, target: crate::lifecycle::LifecycleState) {
        use crate::lifecycle::LifecycleState;
        let restarting_frame = self.lifecycle == LifecycleState::PaintClean
            && matches!(
                target,
                LifecycleState::InStyleRecalc
                    | LifecycleState::InLayout
                    | LifecycleState::InPaint
            );
        debug_assert!(
            target > self.lifecycle
                || target == LifecycleState::VisualUpdatePending
                || restarting_frame,
            "invalid lifecycle transition: {:?} → {:?}",
            self.lifecycle, target
        );
        self.lifecycle = target;
    }

    /// Whether any pending changes need a visual update.
    ///
    /// Chrome equivalent: `Document::NeedsStyleRecalc()` + layout dirty checks.
    /// Used by the scheduler to request a frame after spawned tasks mutate the DOM.
    pub fn needs_visual_update(&self) -> bool {
        self.needs_style_recalc || self.needs_tree_rebuild || !self.dirty_layout_nodes.is_empty()
    }

    /// Returns `true` if the DOM tree structure changed (append / insert /
    /// detach) since the last layout pass.
    ///
    /// Chrome: `LayoutTreeRebuildRoot` — structural changes require a full
    /// Taffy cache clear because `layout_children` lists are stale.
    ///
    /// Style recalc (`needs_style_recalc`) intentionally does NOT trigger a
    /// full cache clear. Instead, `flush_styles_to_layout` compares old vs
    /// new Taffy styles per node and only clears nodes that actually changed.
    /// This is the Kozan equivalent of Chrome's `StyleDifference` — a hover
    /// that changes only `background-color` will match the old Taffy style
    /// exactly, clearing zero caches and making layout essentially free.
    pub(crate) fn take_needs_full_layout_clear(&mut self) -> bool {
        let tree_changed = self.needs_tree_rebuild;
        self.needs_tree_rebuild = false;
        self.needs_style_recalc = false;
        self.dirty_layout_nodes.clear();
        tree_changed
    }

    /// Get a Handle for a node by its arena index.
    ///
    /// Used by hit testing and event dispatch: the fragment tree stores only
    /// the node index (`u32`), this recovers the full Handle needed for
    /// event dispatch. Returns `None` if the index is dead or out of bounds.
    ///
    /// Chrome equivalent: resolving a `LayoutObject`'s `GetNode()` pointer.
    pub fn handle_for_index(&self, index: u32) -> Option<Handle> {
        let generation = self.ids.live_generation(index)?;
        Some(Handle::new(RawId::new(index, generation), self.cell()))
    }

    /// Chrome: `Document::SetHoverElement()` — computes LCA between old and
    /// new hover targets, only toggling elements in the diff. Shared ancestors
    /// keep their state (no double restyle flash).
    pub(crate) fn set_hover_element(&self, new: Option<u32>) {
        let old = self.hover_element;
        if old == new {
            return;
        }
        self.cell().write(|doc| {
            doc.apply_state_lca(old, new, style_dom::ElementState::HOVER);
            doc.hover_element = new;
        });
    }

    /// Chrome: `Document::SetActiveElement()` — same LCA pattern as hover.
    pub(crate) fn set_active_element(&self, new: Option<u32>) {
        let old = self.active_element;
        if old == new {
            return;
        }
        self.cell().write(|doc| {
            doc.apply_state_lca(old, new, style_dom::ElementState::ACTIVE);
            doc.active_element = new;
        });
    }

    /// Toggle `flag` on the symmetric difference between old and new ancestor
    /// chains, using LCA to avoid touching shared ancestors.
    fn apply_state_lca(
        &mut self,
        old: Option<u32>,
        new: Option<u32>,
        flag: style_dom::ElementState,
    ) {
        let lca = match (old, new) {
            (Some(a), Some(b)) => self.lca(a, b),
            _ => None,
        };

        if let Some(old_idx) = old {
            self.set_state_up_to(old_idx, lca, flag, false);
        }
        if let Some(new_idx) = new {
            self.set_state_up_to(new_idx, lca, flag, true);
        }
    }

    /// Set or clear `flag` from `index` up to (not including) `stop`.
    fn set_state_up_to(
        &mut self,
        index: u32,
        stop: Option<u32>,
        flag: style_dom::ElementState,
        insert: bool,
    ) {
        let mut current = index;
        loop {
            if Some(current) == stop {
                break;
            }
            if let Some(ed) = self.element_data.get_mut(current) {
                if insert {
                    ed.element_state.insert(flag);
                } else {
                    ed.element_state.remove(flag);
                }
            }
            self.mark_for_restyle(current);
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => break,
            }
        }
    }

    /// Find the Least Common Ancestor of two nodes.
    ///
    /// Collects ancestors of `a` into a set, then walks `b` upward until a
    /// hit — O(depth) instead of O(depth^2) from zipping two full chains.
    /// Called on every mouse move (hover LCA), so this matters.
    fn lca(&self, a: u32, b: u32) -> Option<u32> {
        use std::collections::HashSet;

        let mut ancestors_a = HashSet::new();
        let mut current = a;
        loop {
            ancestors_a.insert(current);
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => break,
            }
        }

        let mut current = b;
        loop {
            if ancestors_a.contains(&current) {
                return Some(current);
            }
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => break,
            }
        }
        None
    }

    pub(crate) fn focused_element(&self) -> Option<RawId> {
        self.focused_element
    }

    /// HTML §6.6.3 — focusable when natively interactive, has tabindex,
    /// or is a scrollable overflow region.
    pub(crate) fn is_focusable(&self, index: u32) -> bool {
        if let Some(style) = self.computed_style(index) {
            if style.get_box().display.is_none() {
                return false;
            }
        }

        let native = self
            .meta
            .get(index)
            .is_some_and(|m| m.flags().is_focusable());
        if native {
            return self
                .element_data
                .get(index)
                .is_some_and(|ed| {
                    ed.element_state.contains(style_dom::ElementState::ENABLED)
                });
        }

        if self
            .element_data
            .get(index)
            .is_some_and(|ed| ed.attributes.get("tabindex").is_some())
        {
            return true;
        }

        self.has_scrollable_overflow(index)
    }

    /// Explicit tabindex wins; natively focusable and scrollable default to 0.
    pub(crate) fn effective_tab_index(&self, index: u32) -> i32 {
        if let Some(ed) = self.element_data.get(index) {
            if let Some(val) = ed.attributes.get("tabindex") {
                return val.parse().unwrap_or(0);
            }
        }
        let native = self
            .meta
            .get(index)
            .is_some_and(|m| m.flags().is_focusable());
        if native || self.has_scrollable_overflow(index) {
            0
        } else {
            -1
        }
    }

    /// Walk from `index` up ancestors, return first focusable node.
    pub(crate) fn find_focusable_ancestor(&self, index: u32) -> Option<u32> {
        let mut current = index;
        loop {
            if self.is_focusable(current) {
                return Some(current);
            }
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => return None,
            }
        }
    }

    /// W3C sequential focus navigation order.
    pub(crate) fn tab_order(&self) -> Vec<u32> {
        let mut positive: Vec<(i32, usize, u32)> = Vec::new();
        let mut zero: Vec<u32> = Vec::new();
        let mut order = 0usize;

        let mut stack = vec![self.root];
        while let Some(index) = stack.pop() {
            if self.is_focusable(index) {
                let ti = self.effective_tab_index(index);
                if ti > 0 {
                    positive.push((ti, order, index));
                } else if ti == 0 {
                    zero.push(index);
                }
                order += 1;
            }

            if let Some(td) = self.tree.get(index) {
                let mut child = td.last_child;
                while child != INVALID {
                    stack.push(child);
                    child = self.tree.get(child).map_or(INVALID, |t| t.prev_sibling);
                }
            }
        }

        positive.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut result: Vec<u32> = positive.into_iter().map(|(_, _, idx)| idx).collect();
        result.extend(zero);
        result
    }

    /// Keyboard scroll target: walk from focused element up to find scrollable ancestor.
    pub(crate) fn scroll_target(&self, scroll_tree: &crate::scroll::ScrollTree) -> Option<u32> {
        let focused_idx = self.focused_element()?.index();
        let mut current = focused_idx;
        loop {
            if scroll_tree.contains(current) {
                return Some(current);
            }
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => return None,
            }
        }
    }

    fn has_scrollable_overflow(&self, index: u32) -> bool {
        use style::computed_values::overflow_x::T;
        let Some(style) = self.computed_style(index) else {
            return false;
        };
        matches!(style.clone_overflow_x(), T::Scroll | T::Auto)
            || matches!(style.clone_overflow_y(), T::Scroll | T::Auto)
    }

    /// W3C: `document.activeElement` — returns focused element, or body if none.
    pub fn active_element_api(&self) -> Handle {
        self.focused_element
            .and_then(|id| self.resolve(id))
            .unwrap_or_else(|| self.body())
    }

    pub fn has_focus(&self) -> bool {
        self.focused_element.is_some()
    }

    /// UI Events §5.2.2 — focus transition: fires events, updates state flags,
    /// sets `self.focused_element`.
    pub(crate) fn set_focused_element(&self, new: Option<u32>, focus_visible: bool) {
        let old = self.focused_element;
        let new_id = new.and_then(|idx| self.raw_id(idx));
        if old == new_id {
            return;
        }

        // State mutation before events so handlers see consistent focus.
        self.cell().write(|doc| {
            doc.apply_focus_state(old, new_id, focus_visible);
        });

        self.dispatch_focus_events(old, new_id);
    }

    /// UI Events §5.2.2: focusout(A) → focusin(B) → blur(A) → focus(B).
    pub(crate) fn dispatch_focus_events(&self, old: Option<RawId>, new: Option<RawId>) {
        use crate::events::focus_event::*;

        let old_handle = old.and_then(|id| self.resolve(id));
        let new_handle = new.and_then(|id| self.resolve(id));

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

    /// All focus state mutations in one pass — called inside `cell().write()`.
    pub(crate) fn apply_focus_state(
        &mut self,
        old: Option<RawId>,
        new: Option<RawId>,
        focus_visible: bool,
    ) {
        if let Some(oid) = old {
            self.set_focus_chain(oid.index(), false, false);
        }
        if let Some(nid) = new {
            self.set_focus_chain(nid.index(), true, focus_visible);
        }
        self.focused_element = new;
    }

    fn set_focus_chain(&mut self, index: u32, focused: bool, focus_visible: bool) {
        if let Some(ed) = self.element_data.get_mut(index) {
            if focused {
                ed.element_state.insert(style_dom::ElementState::FOCUS);
                if focus_visible {
                    ed.element_state
                        .insert(style_dom::ElementState::FOCUSRING);
                }
            } else {
                ed.element_state.remove(
                    style_dom::ElementState::FOCUS | style_dom::ElementState::FOCUSRING,
                );
            }
        }

        let mut current = index;
        loop {
            if let Some(ed) = self.element_data.get_mut(current) {
                if focused {
                    ed.element_state
                        .insert(style_dom::ElementState::FOCUS_WITHIN);
                } else {
                    ed.element_state
                        .remove(style_dom::ElementState::FOCUS_WITHIN);
                }
            }
            self.mark_for_restyle(current);
            match self.tree.get(current) {
                Some(td) if td.parent != INVALID => current = td.parent,
                _ => break,
            }
        }
    }

    /// Mark an element as needing restyle and propagate dirty flags up.
    ///
    /// Chrome equivalent: `Element::SetNeedsStyleRecalc()` +
    /// `Element::MarkAncestorsWithChildNeedsStyleRecalc()`.
    ///
    /// Sets `RestyleHint::RESTYLE_SELF` on the element's Stylo data, then
    /// walks up the parent chain setting `dirty_descendants = true` on each
    /// ancestor. This ensures Stylo's `pre_traverse` sees dirty flags and
    /// decides to traverse the tree.
    pub(crate) fn mark_for_restyle(&mut self, index: u32) {
        self.needs_style_recalc = true;

        if let Some(ed) = self.element_data.get(index) {
            ed.mark_for_restyle();
        }

        self.propagate_dirty_ancestors(index);
    }

    // ── Layout data access ──

    /// Text content (or similar child-local data) changed — mark for relayout
    /// without triggering restyle.
    ///
    /// Chrome: `CharacterData::DidModifyData()` → `ContainerNode::ChildrenChanged()`.
    pub(crate) fn mark_child_content_changed(&mut self, index: u32) {
        let has_parent = self
            .tree
            .get(index)
            .is_some_and(|td| td.parent != crate::id::INVALID);
        if has_parent {
            self.dirty_layout_nodes.push(index);
            self.mark_layout_dirty(index);
        }
    }

    /// Clear layout cache on a node and propagate up the layout ancestor chain.
    ///
    /// When a child's content or style changes, parent caches are stale
    /// (parent size depends on child size). Walk up clearing each ancestor's
    /// cache until we reach the root or an already-cleared node.
    ///
    /// Chrome equivalent: `LayoutObject::SetNeedsLayout()` propagation.
    pub(crate) fn mark_layout_dirty(&mut self, index: u32) {
        let mut current = index;
        while let Some(data) = self.layout.get_mut(current) {
            data.clear_cache();
            let Some(parent) = data.layout_parent() else {
                break;
            };
            current = parent;
        }
    }

    // ── Style engine (delegates to concrete StyleEngine) ──

    pub fn recalc_styles(&mut self) {
        // Style engine needs DocumentCell for tree walking.
        // This is safe: the engine only accesses through read/write.
        let cell = self.cell();
        let root = self.root;
        self.style_engine.recalc_styles(cell, root);
    }

    /// Add a CSS stylesheet with full selector support.
    ///
    /// Chrome equivalent: `<style>` element or `document.adoptedStyleSheets`.
    /// Supports all CSS selectors: `.class`, `#id`, `tag`, `[attr]`,
    /// descendant, child, pseudo-classes — everything Stylo/Chrome supports.
    ///
    /// ```ignore
    /// doc.add_stylesheet(".card { background: red; border-radius: 8px; }");
    /// doc.add_stylesheet(include_str!("../assets/dashboard.css"));
    /// ```
    pub fn add_stylesheet(&self, css: &str) {
        let cell = self.cell();
        let css_owned = css.to_string();
        cell.write(|doc| {
            doc.style_engine.add_stylesheet(&css_owned);
        });
    }

    pub(crate) fn set_viewport(&mut self, width: f32, height: f32) {
        self.style_engine.set_viewport(width, height);
    }

    pub(crate) fn flush_all_inline_styles(&mut self, guard: &style::shared_lock::SharedRwLock) {
        let capacity = self.ids.capacity() as u32;
        for i in 0..capacity {
            if let Some(ed) = self.element_data.get_mut(i) {
                ed.flush_inline_styles(guard);
            }
        }
    }

    pub(crate) fn shared_lock(&self) -> &style::shared_lock::SharedRwLock {
        self.style_engine.shared_lock()
    }

    pub(crate) fn lifecycle(&self) -> crate::lifecycle::LifecycleState {
        self.lifecycle
    }

    #[cfg(debug_assertions)]
    pub(crate) fn is_alive_debug(&self) -> bool {
        self.alive
    }

    pub(crate) fn element_data_by_index(&self, index: u32) -> Option<&ElementData> {
        self.element_data.get(index)
    }

    /// Raw mutable element data access for the Stylo integration boundary.
    pub(crate) fn element_data_by_index_mut(&mut self, index: u32) -> Option<&mut ElementData> {
        self.element_data.get_mut(index)
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.alive = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::traits::HasHandle;

    #[test]
    fn new_creates_exactly_one_root_node() {
        let doc = Document::new();
        assert_eq!(doc.node_count(), 1);
    }

    #[test]
    fn created_element_handle_is_alive() {
        let doc = Document::new();
        let div = doc.div();
        assert!(div.handle().is_alive());
    }

    #[test]
    fn destroy_makes_handle_dead() {
        let doc = Document::new();
        let div = doc.div();
        let handle = div.handle();
        handle.destroy();
        assert!(!handle.is_alive());
    }

    #[test]
    fn handle_raw_index_matches_arena_slot() {
        let doc = Document::new();
        // Root occupies slot 0; first allocated element goes to slot 1.
        let div = doc.div();
        let raw = div.handle().raw();
        assert!(doc.ids.is_alive(raw));
    }
}
