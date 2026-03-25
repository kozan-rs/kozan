//! `KozanNode` — pointer-sized DOM wrapper for Stylo traversal.
//!
//! Stylo's sharing cache requires the element type to be exactly `usize`.
//! We use a usize index + thread-local `DocumentCell` (set once per traversal)
//! to satisfy this constraint without raw references into the arena.
//!
//! This is safe because:
//! - `KozanNode` only exists during `recalc_styles()` (short-lived)
//! - One document per thread (single-threaded)
//! - Thread-local is set before traversal, cleared after

use std::cell::Cell;

use crate::dom::document_cell::DocumentCell;
use crate::dom::node::NodeType;
use crate::id::INVALID;

// ── Traversal-scoped DocumentCell ──

thread_local! {
    static DOC: Cell<Option<DocumentCell>> = const { Cell::new(None) };
}

/// Call before traversal to make `DocumentCell` available to all `KozanNodes`.
pub(crate) fn enter(cell: DocumentCell) {
    DOC.with(|c| c.set(Some(cell)));
}

/// Call after traversal to clean up.
pub(crate) fn exit() {
    DOC.with(|c| c.set(None));
}

/// Get the current `DocumentCell`.
#[inline]
pub(super) fn doc() -> DocumentCell {
    DOC.with(|c| c.get().expect("KozanNode used outside style traversal"))
}

// ── KozanNode ──

/// Pointer-sized DOM reference for Stylo traversal.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct KozanNode {
    index: usize,
}

const _: () = assert!(std::mem::size_of::<KozanNode>() == std::mem::size_of::<usize>());

impl KozanNode {
    #[inline]
    pub(crate) fn new(index: u32) -> Self {
        Self {
            index: index as usize,
        }
    }

    #[inline]
    pub(super) fn idx(&self) -> u32 {
        self.index as u32
    }

    #[inline]
    fn at(&self, index: u32) -> Option<Self> {
        if index == INVALID {
            None
        } else {
            Some(Self {
                index: index as usize,
            })
        }
    }

    // ── Tree navigation ──

    pub fn parent_node(&self) -> Option<Self> {
        let t = doc().read(|d| d.tree_data_by_index(self.idx()))?;
        self.at(t.parent)
    }

    pub fn first_child(&self) -> Option<Self> {
        let t = doc().read(|d| d.tree_data_by_index(self.idx()))?;
        self.at(t.first_child)
    }

    pub fn last_child(&self) -> Option<Self> {
        let t = doc().read(|d| d.tree_data_by_index(self.idx()))?;
        self.at(t.last_child)
    }

    pub fn next_sibling(&self) -> Option<Self> {
        let t = doc().read(|d| d.tree_data_by_index(self.idx()))?;
        self.at(t.next_sibling)
    }

    pub fn prev_sibling(&self) -> Option<Self> {
        let t = doc().read(|d| d.tree_data_by_index(self.idx()))?;
        self.at(t.prev_sibling)
    }

    // ── Node type ──

    pub fn node_type(&self) -> Option<NodeType> {
        Some(
            doc()
                .read(|d| d.node_meta_by_index(self.idx()))?
                .flags()
                .node_type(),
        )
    }

    pub fn is_element(&self) -> bool {
        self.node_type() == Some(NodeType::Element)
    }
    pub fn is_text(&self) -> bool {
        self.node_type() == Some(NodeType::Text)
    }
    pub fn is_document(&self) -> bool {
        self.node_type() == Some(NodeType::Document)
    }

    // ── Element data ──

    pub fn tag_name(&self) -> Option<&'static str> {
        doc().read(|d| d.tag_name(self.idx()))
    }

    #[allow(dead_code)]
    pub fn attr(&self, name: &str) -> Option<String> {
        doc().read(|d| d.attribute(self.idx(), name))
    }
}

// ── Stylo-required trait impls ──

impl std::fmt::Debug for KozanNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.tag_name() {
            Some(tag) => write!(f, "<{tag}>#{}", self.index),
            None if self.is_text() => write!(f, "#text#{}", self.index),
            _ => write!(f, "#node#{}", self.index),
        }
    }
}

impl PartialEq for KozanNode {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl Eq for KozanNode {}

impl std::hash::Hash for KozanNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}
