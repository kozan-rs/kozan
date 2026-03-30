//! Allocation-free iterators over tree structure.
//!
//! All iterators borrow the [`Tree`] immutably and yield [`NodeId`]s.
//! None of them allocate — traversal state is stored inline.

use crate::{NodeId, Tree};

impl<T> Tree<T> {
    /// Returns an iterator over the direct children of `id`, from first
    /// to last.
    ///
    /// Implements [`DoubleEndedIterator`] for reverse traversal.
    ///
    /// # Examples
    ///
    /// ```
    /// use kozan_tree::Tree;
    ///
    /// let mut tree = Tree::new();
    /// let root = tree.create("root");
    /// let a = tree.create("a");
    /// let b = tree.create("b");
    /// tree.append(root, a);
    /// tree.append(root, b);
    ///
    /// let names: Vec<_> = tree.children(root).map(|id| tree[id]).collect();
    /// assert_eq!(names, vec!["a", "b"]);
    /// ```
    #[must_use]
    pub fn children(&self, id: NodeId) -> Children<'_, T> {
        let node = self.node(id);
        Children {
            tree: self,
            front: node.and_then(|n| n.first_child),
            back: node.and_then(|n| n.last_child),
            done: false,
        }
    }

    /// Returns an iterator that yields `id`, then its parent, then its
    /// grandparent, and so on up to the root.
    ///
    /// # Examples
    ///
    /// ```
    /// use kozan_tree::Tree;
    ///
    /// let mut tree = Tree::new();
    /// let root = tree.create("root");
    /// let child = tree.create("child");
    /// tree.append(root, child);
    ///
    /// let path: Vec<_> = tree.ancestors(child).map(|id| tree[id]).collect();
    /// assert_eq!(path, vec!["child", "root"]);
    /// ```
    #[must_use]
    pub fn ancestors(&self, id: NodeId) -> Ancestors<'_, T> {
        Ancestors {
            tree: self,
            current: Some(id),
        }
    }

    /// Returns a depth-first traversal that yields [`NodeEdge::Start`]
    /// when entering a node and [`NodeEdge::End`] when leaving it.
    ///
    /// This mirrors XML open/close tag semantics and is the foundation
    /// for DOM serialization, painting, and subtree-aware algorithms.
    ///
    /// # Examples
    ///
    /// ```
    /// use kozan_tree::{Tree, NodeEdge};
    ///
    /// let mut tree = Tree::new();
    /// let root = tree.create("div");
    /// let span = tree.create("span");
    /// tree.append(root, span);
    ///
    /// let edges: Vec<_> = tree.traverse(root).collect();
    /// assert_eq!(edges, vec![
    ///     NodeEdge::Start(root),
    ///     NodeEdge::Start(span),
    ///     NodeEdge::End(span),
    ///     NodeEdge::End(root),
    /// ]);
    /// ```
    #[must_use]
    pub fn traverse(&self, root: NodeId) -> Traverse<'_, T> {
        Traverse {
            tree: self,
            root,
            current: if self.contains(root) {
                Some(NodeEdge::Start(root))
            } else {
                None
            },
        }
    }

    /// Returns a pre-order iterator over all descendants of `root`,
    /// excluding `root` itself.
    #[must_use]
    pub fn descendants(&self, root: NodeId) -> Descendants<'_, T> {
        Descendants {
            traverse: self.traverse(root),
            root,
        }
    }

    /// Returns an iterator over siblings after `id` (excludes `id`).
    #[must_use]
    pub fn following_siblings(&self, id: NodeId) -> FollowingSiblings<'_, T> {
        FollowingSiblings {
            tree: self,
            current: self.next_sibling(id),
        }
    }

    /// Returns an iterator over siblings before `id` (excludes `id`),
    /// walking backward toward the first child.
    #[must_use]
    pub fn preceding_siblings(&self, id: NodeId) -> PrecedingSiblings<'_, T> {
        PrecedingSiblings {
            tree: self,
            current: self.prev_sibling(id),
        }
    }
}

/// Iterator over the direct children of a node.
///
/// Created by [`Tree::children`]. Implements [`DoubleEndedIterator`].
pub struct Children<'a, T> {
    tree: &'a Tree<T>,
    front: Option<NodeId>,
    back: Option<NodeId>,
    done: bool,
}

impl<T> Iterator for Children<'_, T> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        if self.done {
            return None;
        }
        let id = self.front?;
        if self.front == self.back {
            self.done = true;
        } else {
            self.front = self.tree.next_sibling(id);
        }
        Some(id)
    }
}

impl<T> DoubleEndedIterator for Children<'_, T> {
    fn next_back(&mut self) -> Option<NodeId> {
        if self.done {
            return None;
        }
        let id = self.back?;
        if self.front == self.back {
            self.done = true;
        } else {
            self.back = self.tree.prev_sibling(id);
        }
        Some(id)
    }
}

/// Iterator from a node up to the root.
///
/// Yields the starting node first, then its parent, grandparent, etc.
/// Created by [`Tree::ancestors`].
pub struct Ancestors<'a, T> {
    tree: &'a Tree<T>,
    current: Option<NodeId>,
}

impl<T> Iterator for Ancestors<'_, T> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = self.tree.parent(id);
        Some(id)
    }
}

/// Edge type for depth-first traversal.
///
/// [`NodeEdge::Start`] is emitted when entering a node (pre-order),
/// [`NodeEdge::End`] when leaving it (post-order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeEdge {
    /// Entering a node (pre-order visit).
    Start(NodeId),
    /// Leaving a node (post-order visit).
    End(NodeId),
}

/// Depth-first traversal yielding [`NodeEdge::Start`] and
/// [`NodeEdge::End`] for every node in a subtree.
///
/// Created by [`Tree::traverse`].
pub struct Traverse<'a, T> {
    tree: &'a Tree<T>,
    root: NodeId,
    current: Option<NodeEdge>,
}

impl<T> Iterator for Traverse<'_, T> {
    type Item = NodeEdge;

    fn next(&mut self) -> Option<NodeEdge> {
        let edge = self.current.take()?;
        self.current = match edge {
            NodeEdge::Start(id) => {
                if let Some(child) = self.tree.first_child(id) {
                    Some(NodeEdge::Start(child))
                } else {
                    Some(NodeEdge::End(id))
                }
            }
            NodeEdge::End(id) => {
                if id == self.root {
                    None
                } else if let Some(sibling) = self.tree.next_sibling(id) {
                    Some(NodeEdge::Start(sibling))
                } else {
                    self.tree.parent(id).map(NodeEdge::End)
                }
            }
        };
        Some(edge)
    }
}

/// Pre-order iterator over all descendants, excluding the root.
///
/// Created by [`Tree::descendants`].
pub struct Descendants<'a, T> {
    traverse: Traverse<'a, T>,
    root: NodeId,
}

impl<T> Iterator for Descendants<'_, T> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        loop {
            match self.traverse.next()? {
                NodeEdge::Start(id) if id != self.root => return Some(id),
                _ => {}
            }
        }
    }
}

/// Iterator over siblings after a given node.
///
/// Created by [`Tree::following_siblings`].
pub struct FollowingSiblings<'a, T> {
    tree: &'a Tree<T>,
    current: Option<NodeId>,
}

impl<T> Iterator for FollowingSiblings<'_, T> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = self.tree.next_sibling(id);
        Some(id)
    }
}

/// Iterator over siblings before a given node, walking backward.
///
/// Created by [`Tree::preceding_siblings`].
pub struct PrecedingSiblings<'a, T> {
    tree: &'a Tree<T>,
    current: Option<NodeId>,
}

impl<T> Iterator for PrecedingSiblings<'_, T> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        let id = self.current?;
        self.current = self.tree.prev_sibling(id);
        Some(id)
    }
}
