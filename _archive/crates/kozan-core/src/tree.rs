// Tree structure — parent/child/sibling links stored as parallel data.
//
// Each node's tree links are stored in a `Storage<TreeData>`.
// All operations are O(1) except child iteration (O(children)).
// Uses sentinel value INVALID (u32::MAX) for "no link."

use crate::id::INVALID;

/// Tree links for a single node.
///
/// Stored in `Storage<TreeData>`, one per node.
/// Uses u32 indices (not generational IDs) for internal links because
/// tree mutations always go through `Document` which validates liveness.
#[derive(Copy, Clone, Debug)]
pub(crate) struct TreeData {
    pub parent: u32,
    pub first_child: u32,
    pub last_child: u32,
    pub next_sibling: u32,
    pub prev_sibling: u32,
}

impl TreeData {
    /// A disconnected node with no links.
    pub fn detached() -> Self {
        Self {
            parent: INVALID,
            first_child: INVALID,
            last_child: INVALID,
            next_sibling: INVALID,
            prev_sibling: INVALID,
        }
    }

    #[inline]
    pub fn has_parent(&self) -> bool {
        self.parent != INVALID
    }
}

/// Tree mutation operations.
///
/// All functions take raw `&mut [TreeData]`-equivalent access through the
/// storage to enable split borrows with other storages.
pub(crate) mod ops {
    use super::*;
    use kozan_primitives::arena::Storage;

    /// Detach a node from its parent and siblings.
    /// Does NOT free the node — just unlinks it from the tree.
    ///
    /// # Safety
    /// All indices must refer to initialized slots in the storage.
    pub unsafe fn detach(tree: &mut Storage<TreeData>, index: u32) {
        let node = unsafe { *tree.get_unchecked(index) };

        if node.parent == INVALID {
            return; // Already detached.
        }

        // Update previous sibling or parent's first_child.
        if node.prev_sibling != INVALID {
            unsafe { tree.get_unchecked_mut(node.prev_sibling) }.next_sibling = node.next_sibling;
        } else {
            // This was the first child.
            unsafe { tree.get_unchecked_mut(node.parent) }.first_child = node.next_sibling;
        }

        // Update next sibling or parent's last_child.
        if node.next_sibling != INVALID {
            unsafe { tree.get_unchecked_mut(node.next_sibling) }.prev_sibling = node.prev_sibling;
        } else {
            // This was the last child.
            unsafe { tree.get_unchecked_mut(node.parent) }.last_child = node.prev_sibling;
        }

        // Clear the node's links.
        let node = unsafe { tree.get_unchecked_mut(index) };
        node.parent = INVALID;
        node.prev_sibling = INVALID;
        node.next_sibling = INVALID;
    }

    /// Append `child` as the last child of `parent`.
    /// If `child` is already attached somewhere, it is detached first.
    /// No-op if `child == parent` or `child` is an ancestor of `parent`.
    ///
    /// # Safety
    /// All indices must refer to initialized slots in the storage.
    pub unsafe fn append(tree: &mut Storage<TreeData>, parent: u32, child: u32) {
        if parent == child || unsafe { is_ancestor(tree, child, parent) } {
            return;
        }

        // Detach child from its current position.
        unsafe { detach(tree, child) };

        let old_last = unsafe { tree.get_unchecked(parent) }.last_child;

        // Set child's links.
        let child_data = unsafe { tree.get_unchecked_mut(child) };
        child_data.parent = parent;
        child_data.prev_sibling = old_last;
        child_data.next_sibling = INVALID;

        // Update old last child's next_sibling.
        if old_last != INVALID {
            unsafe { tree.get_unchecked_mut(old_last) }.next_sibling = child;
        } else {
            // Parent had no children — child becomes first.
            unsafe { tree.get_unchecked_mut(parent) }.first_child = child;
        }

        // Child is now last.
        unsafe { tree.get_unchecked_mut(parent) }.last_child = child;
    }

    /// Insert `child` before `reference` in the child list.
    /// If `child` is already attached somewhere, it is detached first.
    /// No-op if `child == reference`, reference has no parent, or
    /// `child` is an ancestor of the reference's parent.
    ///
    /// # Safety
    /// All indices must refer to initialized slots in the storage.
    pub unsafe fn insert_before(tree: &mut Storage<TreeData>, reference: u32, child: u32) {
        if reference == child {
            return;
        }

        let parent = unsafe { tree.get_unchecked(reference) }.parent;
        if parent == INVALID {
            return;
        }

        if unsafe { is_ancestor(tree, child, parent) } {
            return;
        }

        // Detach child from its current position.
        unsafe { detach(tree, child) };

        let prev = unsafe { tree.get_unchecked(reference) }.prev_sibling;

        // Set child's links.
        let child_data = unsafe { tree.get_unchecked_mut(child) };
        child_data.parent = parent;
        child_data.prev_sibling = prev;
        child_data.next_sibling = reference;

        // Update reference's prev_sibling.
        unsafe { tree.get_unchecked_mut(reference) }.prev_sibling = child;

        // Update previous sibling or parent's first_child.
        if prev != INVALID {
            unsafe { tree.get_unchecked_mut(prev) }.next_sibling = child;
        } else {
            unsafe { tree.get_unchecked_mut(parent) }.first_child = child;
        }
    }

    /// Iterate children of `parent`. Returns indices in order.
    ///
    /// # Safety
    /// All indices must refer to initialized slots in the storage.
    pub unsafe fn children(tree: &Storage<TreeData>, parent: u32) -> Vec<u32> {
        let mut result = Vec::new();
        let mut cursor = unsafe { tree.get_unchecked(parent) }.first_child;
        while cursor != INVALID {
            result.push(cursor);
            cursor = unsafe { tree.get_unchecked(cursor) }.next_sibling;
        }
        result
    }

    /// Check if `ancestor` is an ancestor of `node` (to prevent cycles).
    ///
    /// # Safety
    /// All indices must refer to initialized slots in the storage.
    pub unsafe fn is_ancestor(tree: &Storage<TreeData>, ancestor: u32, mut node: u32) -> bool {
        while node != INVALID {
            if node == ancestor {
                return true;
            }
            node = unsafe { tree.get_unchecked(node) }.parent;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_primitives::arena::Storage;

    fn make_tree(count: u32) -> Storage<TreeData> {
        let mut s = Storage::new();
        for i in 0..count {
            s.set(i, TreeData::detached());
        }
        s
    }

    #[test]
    fn append_single_child() {
        let mut tree = make_tree(2);
        unsafe {
            ops::append(&mut tree, 0, 1);
            assert_eq!(tree.get_unchecked(0).first_child, 1);
            assert_eq!(tree.get_unchecked(0).last_child, 1);
            assert_eq!(tree.get_unchecked(1).parent, 0);
        }
    }

    #[test]
    fn append_multiple_children() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 0, 2);
            ops::append(&mut tree, 0, 3);

            assert_eq!(tree.get_unchecked(0).first_child, 1);
            assert_eq!(tree.get_unchecked(0).last_child, 3);

            assert_eq!(tree.get_unchecked(1).next_sibling, 2);
            assert_eq!(tree.get_unchecked(2).next_sibling, 3);
            assert_eq!(tree.get_unchecked(3).next_sibling, INVALID);

            assert_eq!(tree.get_unchecked(3).prev_sibling, 2);
            assert_eq!(tree.get_unchecked(2).prev_sibling, 1);
            assert_eq!(tree.get_unchecked(1).prev_sibling, INVALID);

            let kids = ops::children(&tree, 0);
            assert_eq!(kids, vec![1, 2, 3]);
        }
    }

    #[test]
    fn detach_middle_child() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 0, 2);
            ops::append(&mut tree, 0, 3);

            ops::detach(&mut tree, 2);

            assert_eq!(tree.get_unchecked(1).next_sibling, 3);
            assert_eq!(tree.get_unchecked(3).prev_sibling, 1);
            assert!(!tree.get_unchecked(2).has_parent());
        }
    }

    #[test]
    fn insert_before_first() {
        let mut tree = make_tree(3);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::insert_before(&mut tree, 1, 2);

            assert_eq!(tree.get_unchecked(0).first_child, 2);
            assert_eq!(tree.get_unchecked(2).next_sibling, 1);
            assert_eq!(tree.get_unchecked(1).prev_sibling, 2);
        }
    }

    #[test]
    fn reparent_detaches_first() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 2);
            ops::append(&mut tree, 1, 2); // moves 2 from parent 0 to parent 1

            assert_eq!(tree.get_unchecked(0).first_child, INVALID);
            assert_eq!(tree.get_unchecked(1).first_child, 2);
            assert_eq!(tree.get_unchecked(2).parent, 1);
        }
    }

    #[test]
    fn is_ancestor_check() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 1, 2);
            ops::append(&mut tree, 2, 3);

            assert!(ops::is_ancestor(&tree, 0, 3));
            assert!(ops::is_ancestor(&tree, 1, 3));
            assert!(!ops::is_ancestor(&tree, 3, 0));
            assert!(!ops::is_ancestor(&tree, 3, 1));
        }
    }

    #[test]
    fn detach_orphan_is_noop() {
        let mut tree = make_tree(1);
        unsafe {
            ops::detach(&mut tree, 0);
            assert_eq!(tree.get_unchecked(0).parent, INVALID);
            assert_eq!(tree.get_unchecked(0).first_child, INVALID);
        }
    }

    #[test]
    fn detach_first_child_updates_parent_first() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 0, 2);
            ops::append(&mut tree, 0, 3);

            ops::detach(&mut tree, 1);

            assert_eq!(tree.get_unchecked(0).first_child, 2);
            assert_eq!(tree.get_unchecked(2).prev_sibling, INVALID);
            assert!(!tree.get_unchecked(1).has_parent());
            assert_eq!(ops::children(&tree, 0), vec![2, 3]);
        }
    }

    #[test]
    fn detach_last_child_updates_parent_last() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 0, 2);
            ops::append(&mut tree, 0, 3);

            ops::detach(&mut tree, 3);

            assert_eq!(tree.get_unchecked(0).last_child, 2);
            assert_eq!(tree.get_unchecked(2).next_sibling, INVALID);
            assert!(!tree.get_unchecked(3).has_parent());
            assert_eq!(ops::children(&tree, 0), vec![1, 2]);
        }
    }

    #[test]
    fn detach_only_child_leaves_parent_empty() {
        let mut tree = make_tree(2);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::detach(&mut tree, 1);

            assert_eq!(tree.get_unchecked(0).first_child, INVALID);
            assert_eq!(tree.get_unchecked(0).last_child, INVALID);
            assert!(!tree.get_unchecked(1).has_parent());
        }
    }

    #[test]
    fn insert_before_middle_sibling() {
        let mut tree = make_tree(4);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 0, 2);
            // Insert 3 before 2 (between 1 and 2).
            ops::insert_before(&mut tree, 2, 3);

            assert_eq!(ops::children(&tree, 0), vec![1, 3, 2]);
            assert_eq!(tree.get_unchecked(1).next_sibling, 3);
            assert_eq!(tree.get_unchecked(3).prev_sibling, 1);
            assert_eq!(tree.get_unchecked(3).next_sibling, 2);
            assert_eq!(tree.get_unchecked(2).prev_sibling, 3);
        }
    }

    #[test]
    fn children_of_empty_parent_returns_empty() {
        let tree = make_tree(1);
        unsafe {
            assert_eq!(ops::children(&tree, 0), Vec::<u32>::new());
        }
    }

    #[test]
    fn append_preserves_subtree() {
        let mut tree = make_tree(4);
        unsafe {
            // Build: 0 -> 1 -> 3, then append 2 under 0.
            // Node 1 has child 3.
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 1, 3);
            ops::append(&mut tree, 0, 2);

            // 1's subtree (child 3) should be intact.
            assert_eq!(ops::children(&tree, 0), vec![1, 2]);
            assert_eq!(ops::children(&tree, 1), vec![3]);
            assert_eq!(tree.get_unchecked(3).parent, 1);
        }
    }

    #[test]
    fn append_self_is_noop() {
        let mut tree = make_tree(2);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 1, 1);
            assert_eq!(tree.get_unchecked(1).parent, 0);
            assert_eq!(ops::children(&tree, 0), vec![1]);
        }
    }

    #[test]
    fn append_ancestor_cycle_is_noop() {
        let mut tree = make_tree(3);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 1, 2);

            // Appending 0 (ancestor) under 2 (descendant) would create a cycle.
            ops::append(&mut tree, 2, 0);

            assert_eq!(tree.get_unchecked(0).parent, INVALID);
            assert_eq!(ops::children(&tree, 2), Vec::<u32>::new());
            assert_eq!(ops::children(&tree, 0), vec![1]);
        }
    }

    #[test]
    fn insert_before_self_is_noop() {
        let mut tree = make_tree(3);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 0, 2);
            ops::insert_before(&mut tree, 1, 1);
            assert_eq!(ops::children(&tree, 0), vec![1, 2]);
        }
    }

    #[test]
    fn insert_before_ancestor_cycle_is_noop() {
        let mut tree = make_tree(3);
        unsafe {
            ops::append(&mut tree, 0, 1);
            ops::append(&mut tree, 1, 2);

            // Inserting 0 (ancestor) before 2 (descendant) would create a cycle.
            ops::insert_before(&mut tree, 2, 0);

            assert_eq!(tree.get_unchecked(0).parent, INVALID);
            assert_eq!(ops::children(&tree, 1), vec![2]);
        }
    }
}
