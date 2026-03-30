//! Arena-backed tree with O(1) mutations and generational safety.
//!
//! Nodes live in a flat [`Vec`] indexed by [`NodeId`]. Each node carries
//! user data plus parent/child/sibling links. Freed slots are recycled
//! via a free list; generational IDs detect use-after-free in all builds.
//!
//! # Memory layout
//!
//! | Component | Size |
//! |-----------|------|
//! | [`NodeId`] | 8 bytes (`u32` index + [`NonZeroU32`] generation) |
//! | `Option<NodeId>` | 8 bytes (niche-optimized) |
//! | Tree links per node | 40 bytes (5 × `Option<NodeId>`) |
//!
//! # Free list
//!
//! Freed slots are pushed onto a LIFO free list with a bumped generation.
//! [`create`](Tree::create) pops from the head, reusing recently-freed
//! slots for cache locality. Generation wraps from `u32::MAX` to 1,
//! providing ~4 billion reuses per slot before a theoretical collision.
//!
//! # Roots
//!
//! The tree has no implicit root. Any node without a parent is a root.
//! The consumer (e.g. `Document`) tracks which [`NodeId`] is the
//! document root.

use core::num::NonZeroU32;

mod iter;
pub use iter::{
    Ancestors, Children, Descendants, FollowingSiblings, NodeEdge, PrecedingSiblings, Traverse,
};

/// Generational handle into a [`Tree`].
///
/// Stores a slot index and a generation counter. Access through a stale
/// `NodeId` (one whose slot has been freed and reused) returns `None`
/// instead of silently reading the wrong node.
///
/// # Size
///
/// `NodeId` is 8 bytes. `Option<NodeId>` is also 8 bytes thanks to the
/// [`NonZeroU32`] niche in the generation field.
///
/// # Examples
///
/// ```
/// use kozan_tree::Tree;
///
/// let mut tree = Tree::new();
/// let root = tree.create("root");
/// assert_eq!(tree[root], "root");
/// assert_eq!(root.index(), 0);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    index: u32,
    generation: NonZeroU32,
}

impl NodeId {
    /// Returns the raw slot index.
    ///
    /// Useful for indexing parallel data arrays that store per-node
    /// information outside the tree (e.g. computed styles, layout results).
    #[inline]
    #[must_use]
    pub fn index(self) -> u32 {
        self.index
    }
}

impl core::fmt::Debug for NodeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "NodeId({}v{})", self.index, self.generation)
    }
}

const GEN_ONE: NonZeroU32 = match NonZeroU32::new(1) {
    Some(v) => v,
    None => unreachable!(),
};

enum Entry<T> {
    Occupied(Node<T>),
    Vacant(VacantEntry),
}

struct Node<T> {
    generation: NonZeroU32,
    data: T,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
    prev_sibling: Option<NodeId>,
    next_sibling: Option<NodeId>,
}

struct VacantEntry {
    generation: NonZeroU32,
    next_free: Option<u32>,
}

/// Wraps from `u32::MAX` to 1, preserving the [`NonZeroU32`] invariant.
#[inline]
fn next_generation(current: NonZeroU32) -> NonZeroU32 {
    current.checked_add(1).unwrap_or(GEN_ONE)
}

/// An arena-backed tree where all structural mutations are O(1).
///
/// Children are stored as a doubly-linked sibling list, giving O(1)
/// append, prepend, insert-before, insert-after, and detach. Nodes
/// are allocated from a flat `Vec` with free-list recycling.
///
/// # Examples
///
/// ```
/// use kozan_tree::Tree;
///
/// let mut tree = Tree::new();
/// let root = tree.create("html");
/// let head = tree.create("head");
/// let body = tree.create("body");
///
/// tree.append(root, head);
/// tree.append(root, body);
///
/// let children: Vec<_> = tree.children(root).collect();
/// assert_eq!(tree[children[0]], "head");
/// assert_eq!(tree[children[1]], "body");
/// ```
pub struct Tree<T> {
    entries: Vec<Entry<T>>,
    free_head: Option<u32>,
    len: u32,
}

impl<T> Tree<T> {
    /// Creates an empty tree with no pre-allocated capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            free_head: None,
            len: 0,
        }
    }

    /// Creates an empty tree with space for at least `capacity` nodes.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            free_head: None,
            len: 0,
        }
    }

    /// Returns the number of live nodes.
    #[inline]
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Returns `true` if the tree contains no live nodes.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the total number of allocated slots (live + freed).
    ///
    /// This is **not** the number of live nodes — use [`len`](Self::len)
    /// for that. Useful for sizing parallel data arrays.
    #[inline]
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if `id` refers to a live node.
    #[inline]
    #[must_use]
    pub fn contains(&self, id: NodeId) -> bool {
        self.node(id).is_some()
    }

    /// Creates a new orphan node with the given data.
    ///
    /// The node has no parent and no children. Reuses a freed slot if
    /// one is available, otherwise appends to the backing `Vec`.
    pub fn create(&mut self, data: T) -> NodeId {
        self.len = self.len.checked_add(1).expect("tree node count overflow");

        if let Some(idx) = self.free_head {
            let slot = &mut self.entries[idx as usize];
            let Entry::Vacant(vacant) = slot else {
                unreachable!("free list pointed to an occupied slot");
            };
            let generation = vacant.generation;
            self.free_head = vacant.next_free;
            *slot = Entry::Occupied(Node {
                generation,
                data,
                parent: None,
                first_child: None,
                last_child: None,
                prev_sibling: None,
                next_sibling: None,
            });
            NodeId { index: idx, generation }
        } else {
            let index = u32::try_from(self.entries.len()).expect("tree slot overflow");
            self.entries.push(Entry::Occupied(Node {
                generation: GEN_ONE,
                data,
                parent: None,
                first_child: None,
                last_child: None,
                prev_sibling: None,
                next_sibling: None,
            }));
            NodeId { index, generation: GEN_ONE }
        }
    }

    #[inline]
    fn node(&self, id: NodeId) -> Option<&Node<T>> {
        match self.entries.get(id.index as usize)? {
            Entry::Occupied(n) if n.generation == id.generation => Some(n),
            _ => None,
        }
    }

    #[inline]
    fn node_mut(&mut self, id: NodeId) -> Option<&mut Node<T>> {
        match self.entries.get_mut(id.index as usize)? {
            Entry::Occupied(n) if n.generation == id.generation => Some(n),
            _ => None,
        }
    }

    /// Returns a reference to the node's data, or `None` if the ID is
    /// stale (the slot was freed and possibly reused).
    #[inline]
    #[must_use]
    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.node(id).map(|n| &n.data)
    }

    /// Returns a mutable reference to the node's data, or `None` if
    /// the ID is stale.
    #[inline]
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut T> {
        self.node_mut(id).map(|n| &mut n.data)
    }

    /// Returns the parent of `id`, or `None` if `id` is a root or stale.
    #[inline]
    #[must_use]
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.parent
    }

    /// Returns the first child of `id`, or `None` if the node has no
    /// children or the ID is stale.
    #[inline]
    #[must_use]
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.first_child
    }

    /// Returns the last child of `id`.
    #[inline]
    #[must_use]
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.last_child
    }

    /// Returns the next sibling of `id`.
    #[inline]
    #[must_use]
    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.next_sibling
    }

    /// Returns the previous sibling of `id`.
    #[inline]
    #[must_use]
    pub fn prev_sibling(&self, id: NodeId) -> Option<NodeId> {
        self.node(id)?.prev_sibling
    }

    /// Returns `true` if the node has at least one child.
    #[inline]
    #[must_use]
    pub fn has_children(&self, id: NodeId) -> bool {
        self.first_child(id).is_some()
    }

    /// Returns `true` if `ancestor` is an ancestor of `descendant`,
    /// or if they are the same node. Walks up from `descendant`.
    #[must_use]
    pub fn is_ancestor_of(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        let mut current = Some(descendant);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.parent(id);
        }
        false
    }

    /// Returns the number of direct children. O(children).
    #[must_use]
    pub fn child_count(&self, id: NodeId) -> usize {
        self.children(id).count()
    }

    /// Returns the depth of a node (distance to the nearest root).
    /// A root node has depth 0. O(depth).
    #[must_use]
    pub fn depth(&self, id: NodeId) -> usize {
        self.ancestors(id).count().saturating_sub(1)
    }

    /// Appends `child` as the last child of `parent`.
    ///
    /// # Panics
    ///
    /// Panics if either ID is stale, if `child` already has a parent
    /// (call [`detach`](Self::detach) first), or if `child == parent`.
    /// In debug builds, also panics if attaching would create a cycle.
    pub fn append(&mut self, parent: NodeId, child: NodeId) {
        self.validate_attach(parent, child);
        let last = self.node(parent).expect("stale parent id").last_child;

        self.node_mut(child).expect("stale child id").parent = Some(parent);
        self.node_mut(child).unwrap().prev_sibling = last;

        if let Some(last_id) = last {
            self.node_mut(last_id).expect("corrupt last_child link").next_sibling = Some(child);
        } else {
            self.node_mut(parent).unwrap().first_child = Some(child);
        }
        self.node_mut(parent).unwrap().last_child = Some(child);
    }

    /// Prepends `child` as the first child of `parent`.
    ///
    /// # Panics
    ///
    /// Same as [`append`](Self::append).
    pub fn prepend(&mut self, parent: NodeId, child: NodeId) {
        self.validate_attach(parent, child);
        let first = self.node(parent).expect("stale parent id").first_child;

        self.node_mut(child).expect("stale child id").parent = Some(parent);
        self.node_mut(child).unwrap().next_sibling = first;

        if let Some(first_id) = first {
            self.node_mut(first_id).expect("corrupt first_child link").prev_sibling = Some(child);
        } else {
            self.node_mut(parent).unwrap().last_child = Some(child);
        }
        self.node_mut(parent).unwrap().first_child = Some(child);
    }

    /// Inserts `new_node` immediately before `sibling` in the child list.
    ///
    /// # Panics
    ///
    /// Panics if `sibling` has no parent, if `new_node` already has a
    /// parent, or if either ID is stale.
    pub fn insert_before(&mut self, sibling: NodeId, new_node: NodeId) {
        let sib = self.node(sibling).expect("stale sibling id");
        let parent = sib.parent.expect("sibling has no parent");
        let prev = sib.prev_sibling;
        debug_assert_ne!(sibling, new_node);
        debug_assert!(
            self.node(new_node).expect("stale new_node id").parent.is_none(),
            "new_node already has a parent — detach first"
        );

        let n = self.node_mut(new_node).unwrap();
        n.parent = Some(parent);
        n.prev_sibling = prev;
        n.next_sibling = Some(sibling);

        self.node_mut(sibling).unwrap().prev_sibling = Some(new_node);

        if let Some(prev_id) = prev {
            self.node_mut(prev_id).unwrap().next_sibling = Some(new_node);
        } else {
            self.node_mut(parent).unwrap().first_child = Some(new_node);
        }
    }

    /// Inserts `new_node` immediately after `sibling` in the child list.
    ///
    /// # Panics
    ///
    /// Same as [`insert_before`](Self::insert_before).
    pub fn insert_after(&mut self, sibling: NodeId, new_node: NodeId) {
        let sib = self.node(sibling).expect("stale sibling id");
        let parent = sib.parent.expect("sibling has no parent");
        let next = sib.next_sibling;
        debug_assert_ne!(sibling, new_node);
        debug_assert!(
            self.node(new_node).expect("stale new_node id").parent.is_none(),
            "new_node already has a parent — detach first"
        );

        let n = self.node_mut(new_node).unwrap();
        n.parent = Some(parent);
        n.prev_sibling = Some(sibling);
        n.next_sibling = next;

        self.node_mut(sibling).unwrap().next_sibling = Some(new_node);

        if let Some(next_id) = next {
            self.node_mut(next_id).unwrap().prev_sibling = Some(new_node);
        } else {
            self.node_mut(parent).unwrap().last_child = Some(new_node);
        }
    }

    /// Detaches a node from its parent, making it an orphan.
    ///
    /// The node and all its descendants remain alive. No-op if the node
    /// is already a root (has no parent).
    pub fn detach(&mut self, id: NodeId) {
        let n = self.node(id).expect("stale id");
        let parent = match n.parent {
            Some(p) => p,
            None => return,
        };
        let prev = n.prev_sibling;
        let next = n.next_sibling;

        if let Some(prev_id) = prev {
            self.node_mut(prev_id).unwrap().next_sibling = next;
        } else {
            self.node_mut(parent).unwrap().first_child = next;
        }

        if let Some(next_id) = next {
            self.node_mut(next_id).unwrap().prev_sibling = prev;
        } else {
            self.node_mut(parent).unwrap().last_child = prev;
        }

        let n = self.node_mut(id).unwrap();
        n.parent = None;
        n.prev_sibling = None;
        n.next_sibling = None;
    }

    /// Detaches all children of `id`, making each one an orphan.
    pub fn detach_children(&mut self, id: NodeId) {
        let mut child = self.first_child(id);
        while let Some(c) = child {
            let next = self.next_sibling(c);
            let n = self.node_mut(c).unwrap();
            n.parent = None;
            n.prev_sibling = None;
            n.next_sibling = None;
            child = next;
        }
        let n = self.node_mut(id).unwrap();
        n.first_child = None;
        n.last_child = None;
    }

    /// Removes a node and its entire subtree, returning the root's data.
    ///
    /// The node is detached from its parent, then all descendants are
    /// freed recursively. Every [`NodeId`] pointing into the removed
    /// subtree becomes stale. Returns `None` if the ID is already stale.
    pub fn remove(&mut self, id: NodeId) -> Option<T> {
        self.node(id)?;
        self.detach(id);
        self.free_subtree(id)
    }

    fn free_subtree(&mut self, id: NodeId) -> Option<T> {
        let mut child = self.first_child(id);
        while let Some(c) = child {
            let next = self.next_sibling(c);
            self.free_subtree(c);
            child = next;
        }
        self.free_slot(id)
    }

    fn free_slot(&mut self, id: NodeId) -> Option<T> {
        let slot = self.entries.get_mut(id.index as usize)?;
        let Entry::Occupied(node) = slot else { return None };
        if node.generation != id.generation {
            return None;
        }
        let bumped = next_generation(node.generation);
        let old = core::mem::replace(
            slot,
            Entry::Vacant(VacantEntry {
                generation: bumped,
                next_free: self.free_head,
            }),
        );
        self.free_head = Some(id.index);
        self.len -= 1;
        match old {
            Entry::Occupied(n) => Some(n.data),
            Entry::Vacant(_) => unreachable!(),
        }
    }

    fn validate_attach(&self, parent: NodeId, child: NodeId) {
        assert_ne!(parent, child, "cannot attach a node to itself");
        assert!(self.node(parent).is_some(), "stale parent id");
        assert!(
            self.node(child).expect("stale child id").parent.is_none(),
            "child already has a parent — detach first"
        );
        debug_assert!(
            !self.is_ancestor_of(child, parent),
            "attaching would create a cycle"
        );
    }
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for Tree<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Tree")
            .field("len", &self.len)
            .field("slots", &self.entries.len())
            .finish_non_exhaustive()
    }
}

impl<T> core::ops::Index<NodeId> for Tree<T> {
    type Output = T;

    /// Returns a reference to the node's data.
    ///
    /// # Panics
    ///
    /// Panics if `id` is stale.
    #[inline]
    fn index(&self, id: NodeId) -> &T {
        self.get(id).expect("stale NodeId")
    }
}

impl<T> core::ops::IndexMut<NodeId> for Tree<T> {
    /// Returns a mutable reference to the node's data.
    ///
    /// # Panics
    ///
    /// Panics if `id` is stale.
    #[inline]
    fn index_mut(&mut self, id: NodeId) -> &mut T {
        self.get_mut(id).expect("stale NodeId")
    }
}
