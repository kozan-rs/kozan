//! Event propagation path.
//!
//! Chrome's `EventPath`: a snapshot of the ancestor chain from target to root,
//! built once at dispatch start. Uses `NodeEventContext` entries.
//!
//! Kozan: uses raw `u32` indices (node slot indices) since all tree data
//! is accessed through `DocumentCell`.

use crate::dom::document_cell::DocumentCell;
use crate::id::{INVALID, RawId};

/// The propagation path for an event.
///
/// Built from target node up to the document root.
/// Stored as `(index, generation)` pairs for liveness validation.
///
/// Index 0 = target, Index N = root (same order as Chrome).
/// Capture iterates in reverse. Bubble iterates forward.
pub struct EventPath {
    /// (`node_index`, `node_generation`) pairs. Target-first order.
    entries: Vec<(u32, u32)>,
}

impl EventPath {
    /// Build the propagation path from target up to root.
    ///
    /// Takes a snapshot of the ancestor chain at dispatch time.
    /// Changes to the tree during dispatch do not affect the path.
    pub(crate) fn build(cell: DocumentCell, target: RawId) -> Self {
        let mut entries = Vec::new();

        if !cell.read(|doc| doc.is_alive_id(target)) {
            return Self { entries };
        }

        entries.push((target.index(), target.generation()));

        // Walk up the tree via parent pointers.
        let mut current = target.index();
        loop {
            let Some(cur_gen) = cell.read(|doc| doc.generation(current)) else {
                break;
            };
            let Some(tree) = cell.read(|doc| doc.tree_data(RawId::new(current, cur_gen))) else {
                break;
            };

            if tree.parent == INVALID {
                break;
            }

            let Some(parent_gen) = cell.read(|doc| doc.generation(tree.parent)) else {
                break;
            };

            entries.push((tree.parent, parent_gen));
            current = tree.parent;
        }

        Self { entries }
    }

    /// Number of nodes in the path.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is the path empty? (target was dead)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The target node (first entry).
    #[must_use]
    pub fn target(&self) -> Option<(u32, u32)> {
        self.entries.first().copied()
    }

    /// Index of the target in the path (always 0 if non-empty).
    #[must_use]
    pub fn target_index(&self) -> usize {
        0
    }

    /// Iterate in capture order (root -> target).
    pub fn capture_order(&self) -> impl Iterator<Item = (usize, u32, u32)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .rev()
            .map(|(i, &(idx, generation))| (i, idx, generation))
    }

    /// Iterate in bubble order (target -> root).
    pub fn bubble_order(&self) -> impl Iterator<Item = (usize, u32, u32)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, &(idx, generation))| (i, idx, generation))
    }

    /// Get entry at position.
    #[must_use]
    pub fn get(&self, pos: usize) -> Option<(u32, u32)> {
        self.entries.get(pos).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::document::Document;
    use crate::dom::traits::{ContainerNode, HasHandle};
    use crate::html::{HtmlButtonElement, HtmlDivElement};

    #[test]
    fn path_target_to_root_ordering() {
        let doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(div);
        div.append(btn);

        let cell = doc.cell();
        let path = EventPath::build(cell, btn.handle().raw());

        // Path: target (btn) → div → root. Length = 3.
        assert_eq!(path.len(), 3);
        assert_eq!(
            path.target().expect("non-empty path").0,
            btn.handle().raw().index()
        );
        // Last entry is the root.
        let (root_idx, _) = path.get(2).expect("root entry");
        assert_eq!(root_idx, doc.root().raw().index());
    }

    #[test]
    fn path_single_node_at_root() {
        let doc = Document::new();
        let cell = doc.cell();
        let root_id = doc.root().raw();
        let path = EventPath::build(cell, root_id);

        // Root has no parent, so path is just the root itself.
        assert_eq!(path.len(), 1);
        assert_eq!(path.target().expect("non-empty").0, root_id.index());
    }

    #[test]
    fn path_dead_target_produces_empty_path() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        let raw = btn.handle().raw();
        btn.handle().destroy();

        let cell = doc.cell();
        let path = EventPath::build(cell, raw);
        assert!(path.is_empty());
    }

    #[test]
    fn capture_order_is_root_to_target() {
        let doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(div);
        div.append(btn);

        let cell = doc.cell();
        let path = EventPath::build(cell, btn.handle().raw());

        let capture: Vec<u32> = path.capture_order().map(|(_, idx, _)| idx).collect();
        // Capture goes root → div → btn.
        assert_eq!(capture[0], doc.root().raw().index());
        assert_eq!(capture[1], div.handle().raw().index());
        assert_eq!(capture[2], btn.handle().raw().index());
    }

    #[test]
    fn bubble_order_is_target_to_root() {
        let doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(div);
        div.append(btn);

        let cell = doc.cell();
        let path = EventPath::build(cell, btn.handle().raw());

        let bubble: Vec<u32> = path.bubble_order().map(|(_, idx, _)| idx).collect();
        // Bubble goes btn → div → root.
        assert_eq!(bubble[0], btn.handle().raw().index());
        assert_eq!(bubble[1], div.handle().raw().index());
        assert_eq!(bubble[2], doc.root().raw().index());
    }

    #[test]
    fn detached_node_path_contains_only_target() {
        let doc = Document::new();
        let btn = doc.create::<HtmlButtonElement>();
        // Not attached to tree — no parent.
        let cell = doc.cell();
        let path = EventPath::build(cell, btn.handle().raw());

        assert_eq!(path.len(), 1);
        assert_eq!(
            path.target().expect("has target").0,
            btn.handle().raw().index()
        );
    }
}
