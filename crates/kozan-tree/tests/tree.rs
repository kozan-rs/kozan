use kozan_tree::{NodeEdge, NodeId, Tree};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sample_tree() -> (Tree<&'static str>, NodeId, NodeId, NodeId, NodeId, NodeId) {
    //      root
    //     / | \
    //    a  b  c
    //       |
    //       d
    let mut t = Tree::new();
    let root = t.create("root");
    let a = t.create("a");
    let b = t.create("b");
    let c = t.create("c");
    let d = t.create("d");
    t.append(root, a);
    t.append(root, b);
    t.append(root, c);
    t.append(b, d);
    (t, root, a, b, c, d)
}

// ---------------------------------------------------------------------------
// Creation & access
// ---------------------------------------------------------------------------

#[test]
fn create_and_access() {
    let mut t = Tree::new();
    let id = t.create(42);
    assert_eq!(t[id], 42);
    assert_eq!(*t.get(id).unwrap(), 42);
    assert!(t.contains(id));
    assert_eq!(t.len(), 1);
}

#[test]
fn create_multiple() {
    let mut t = Tree::new();
    let a = t.create("a");
    let b = t.create("b");
    let c = t.create("c");
    assert_eq!(t.len(), 3);
    assert_eq!(t[a], "a");
    assert_eq!(t[b], "b");
    assert_eq!(t[c], "c");
}

#[test]
fn mutate_data() {
    let mut t = Tree::new();
    let id = t.create(0);
    t[id] = 99;
    assert_eq!(t[id], 99);
}

#[test]
fn with_capacity() {
    let t = Tree::<i32>::with_capacity(100);
    assert!(t.is_empty());
    assert_eq!(t.len(), 0);
}

// ---------------------------------------------------------------------------
// Append / prepend
// ---------------------------------------------------------------------------

#[test]
fn append_basic() {
    let (t, root, a, b, c, _) = sample_tree();
    assert_eq!(t.first_child(root), Some(a));
    assert_eq!(t.last_child(root), Some(c));
    assert_eq!(t.parent(a), Some(root));
    assert_eq!(t.parent(b), Some(root));
    assert_eq!(t.parent(c), Some(root));
    assert_eq!(t.next_sibling(a), Some(b));
    assert_eq!(t.next_sibling(b), Some(c));
    assert_eq!(t.next_sibling(c), None);
    assert_eq!(t.prev_sibling(c), Some(b));
    assert_eq!(t.prev_sibling(b), Some(a));
    assert_eq!(t.prev_sibling(a), None);
}

#[test]
fn prepend_basic() {
    let mut t = Tree::new();
    let root = t.create("root");
    let a = t.create("a");
    let b = t.create("b");
    let c = t.create("c");
    t.prepend(root, c);
    t.prepend(root, b);
    t.prepend(root, a);

    // Order should be: a, b, c
    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![a, b, c]);
}

#[test]
fn append_single_child() {
    let mut t = Tree::new();
    let root = t.create("root");
    let child = t.create("child");
    t.append(root, child);

    assert_eq!(t.first_child(root), Some(child));
    assert_eq!(t.last_child(root), Some(child));
    assert_eq!(t.parent(child), Some(root));
    assert!(t.prev_sibling(child).is_none());
    assert!(t.next_sibling(child).is_none());
}

// ---------------------------------------------------------------------------
// Insert before / after
// ---------------------------------------------------------------------------

#[test]
fn insert_before_middle() {
    let (mut t, root, a, b, _, _) = sample_tree();
    let x = t.create("x");
    t.insert_before(b, x);

    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children[0], a);
    assert_eq!(children[1], x);
    assert_eq!(children[2], b);
    assert_eq!(t.prev_sibling(x), Some(a));
    assert_eq!(t.next_sibling(x), Some(b));
}

#[test]
fn insert_before_first() {
    let (mut t, root, a, _, _, _) = sample_tree();
    let x = t.create("x");
    t.insert_before(a, x);

    assert_eq!(t.first_child(root), Some(x));
    assert_eq!(t.next_sibling(x), Some(a));
    assert!(t.prev_sibling(x).is_none());
}

#[test]
fn insert_after_middle() {
    let (mut t, root, a, b, c, _) = sample_tree();
    let x = t.create("x");
    t.insert_after(b, x);

    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![a, b, x, c]);
    assert_eq!(t.prev_sibling(x), Some(b));
    assert_eq!(t.next_sibling(x), Some(c));
}

#[test]
fn insert_after_last() {
    let (mut t, root, _, _, c, _) = sample_tree();
    let x = t.create("x");
    t.insert_after(c, x);

    assert_eq!(t.last_child(root), Some(x));
    assert_eq!(t.prev_sibling(x), Some(c));
    assert!(t.next_sibling(x).is_none());
}

// ---------------------------------------------------------------------------
// Detach
// ---------------------------------------------------------------------------

#[test]
fn detach_middle() {
    let (mut t, root, a, b, c, _) = sample_tree();
    t.detach(b);

    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![a, c]);
    assert!(t.parent(b).is_none());
    assert!(t.prev_sibling(b).is_none());
    assert!(t.next_sibling(b).is_none());
    // b's children are preserved
    assert!(t.has_children(b));
}

#[test]
fn detach_first() {
    let (mut t, root, a, b, c, _) = sample_tree();
    t.detach(a);

    assert_eq!(t.first_child(root), Some(b));
    assert!(t.prev_sibling(b).is_none());
    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![b, c]);
}

#[test]
fn detach_last() {
    let (mut t, root, a, b, c, _) = sample_tree();
    t.detach(c);

    assert_eq!(t.last_child(root), Some(b));
    assert!(t.next_sibling(b).is_none());
    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![a, b]);
}

#[test]
fn detach_only_child() {
    let mut t = Tree::new();
    let root = t.create("root");
    let child = t.create("child");
    t.append(root, child);
    t.detach(child);

    assert!(!t.has_children(root));
    assert!(t.first_child(root).is_none());
    assert!(t.last_child(root).is_none());
}

#[test]
fn detach_orphan_is_noop() {
    let mut t = Tree::new();
    let id = t.create("orphan");
    t.detach(id); // should not panic
    assert!(t.parent(id).is_none());
}

#[test]
fn detach_children_basic() {
    let (mut t, root, a, b, c, _) = sample_tree();
    t.detach_children(root);

    assert!(!t.has_children(root));
    assert!(t.parent(a).is_none());
    assert!(t.parent(b).is_none());
    assert!(t.parent(c).is_none());
    // Each child is now a standalone orphan
    assert!(t.prev_sibling(b).is_none());
    assert!(t.next_sibling(b).is_none());
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

#[test]
fn remove_leaf() {
    let (mut t, _root, _a, b, _c, d) = sample_tree();
    let data = t.remove(d);
    assert_eq!(data, Some("d"));
    assert!(!t.contains(d));
    assert!(!t.has_children(b));
    assert_eq!(t.len(), 4);
}

#[test]
fn remove_subtree() {
    let (mut t, root, a, _, c, d) = sample_tree();
    // Remove b (which has child d)
    let data = t.remove(t.children(root).nth(1).unwrap());
    assert_eq!(data, Some("b"));
    assert!(!t.contains(d)); // d was freed too

    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![a, c]);
    assert_eq!(t.len(), 3);
}

#[test]
fn remove_root_subtree() {
    let (mut t, root, a, b, c, d) = sample_tree();
    let data = t.remove(root);
    assert_eq!(data, Some("root"));
    assert!(!t.contains(root));
    assert!(!t.contains(a));
    assert!(!t.contains(b));
    assert!(!t.contains(c));
    assert!(!t.contains(d));
    assert!(t.is_empty());
}

#[test]
fn remove_stale_returns_none() {
    let mut t = Tree::new();
    let id = t.create("x");
    t.remove(id);
    assert_eq!(t.remove(id), None);
}

// ---------------------------------------------------------------------------
// Free list reuse
// ---------------------------------------------------------------------------

#[test]
fn free_list_reuses_slots() {
    let mut t = Tree::new();
    let a = t.create("a");
    let b = t.create("b");
    let slot_count_before = t.slot_count();

    t.remove(a);
    t.remove(b);
    assert_eq!(t.len(), 0);
    assert_eq!(t.slot_count(), slot_count_before); // slots not deallocated

    // New nodes reuse freed slots
    let c = t.create("c");
    let d = t.create("d");
    assert_eq!(t.slot_count(), slot_count_before); // no new slots allocated
    assert_eq!(t.len(), 2);
    assert_eq!(t[c], "c");
    assert_eq!(t[d], "d");
}

// ---------------------------------------------------------------------------
// Generation safety
// ---------------------------------------------------------------------------

#[test]
fn stale_id_returns_none() {
    let mut t = Tree::new();
    let id = t.create("old");
    t.remove(id);

    assert!(!t.contains(id));
    assert!(t.get(id).is_none());
    assert!(t.parent(id).is_none());
    assert!(t.first_child(id).is_none());
}

#[test]
fn stale_id_does_not_see_reused_slot() {
    let mut t = Tree::new();
    let old = t.create("old");
    t.remove(old);
    let new = t.create("new");

    // Old and new have the same slot index but different generations
    assert_eq!(old.index(), new.index());
    assert!(!t.contains(old));
    assert!(t.contains(new));
    assert!(t.get(old).is_none());
    assert_eq!(t[new], "new");
}

#[test]
#[should_panic(expected = "stale NodeId")]
fn index_stale_panics() {
    let mut t = Tree::new();
    let id = t.create("x");
    t.remove(id);
    let _ = t[id]; // should panic
}

// ---------------------------------------------------------------------------
// Structure queries
// ---------------------------------------------------------------------------

#[test]
fn is_ancestor_of() {
    let (t, root, a, b, _, d) = sample_tree();
    assert!(t.is_ancestor_of(root, d)); // root → b → d
    assert!(t.is_ancestor_of(b, d));    // b → d
    assert!(t.is_ancestor_of(d, d));    // self
    assert!(!t.is_ancestor_of(a, d));   // a is sibling of b, not ancestor of d
    assert!(!t.is_ancestor_of(d, root));
}

#[test]
fn child_count() {
    let (t, root, _, b, _, _) = sample_tree();
    assert_eq!(t.child_count(root), 3);
    assert_eq!(t.child_count(b), 1);
}

#[test]
fn depth() {
    let (t, root, a, _, _, d) = sample_tree();
    assert_eq!(t.depth(root), 0);
    assert_eq!(t.depth(a), 1);
    assert_eq!(t.depth(d), 2);
}

// ---------------------------------------------------------------------------
// Children iterator
// ---------------------------------------------------------------------------

#[test]
fn children_forward() {
    let (t, root, a, b, c, _) = sample_tree();
    let children: Vec<_> = t.children(root).collect();
    assert_eq!(children, vec![a, b, c]);
}

#[test]
fn children_reverse() {
    let (t, root, a, b, c, _) = sample_tree();
    let children: Vec<_> = t.children(root).rev().collect();
    assert_eq!(children, vec![c, b, a]);
}

#[test]
fn children_double_ended() {
    let (t, root, a, _, c, _) = sample_tree();
    let mut iter = t.children(root);
    assert_eq!(iter.next(), Some(a));
    assert_eq!(iter.next_back(), Some(c));
    // Only b remains, from either direction
    let remaining: Vec<_> = iter.collect();
    assert_eq!(remaining.len(), 1);
}

#[test]
fn children_empty() {
    let (t, _, a, _, _, _) = sample_tree();
    let children: Vec<_> = t.children(a).collect();
    assert!(children.is_empty());
}

#[test]
fn children_single() {
    let (t, _, _, b, _, d) = sample_tree();
    let children: Vec<_> = t.children(b).collect();
    assert_eq!(children, vec![d]);

    // Double-ended on single element
    let mut iter = t.children(b);
    assert_eq!(iter.next(), Some(d));
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next_back(), None);
}

// ---------------------------------------------------------------------------
// Ancestors iterator
// ---------------------------------------------------------------------------

#[test]
fn ancestors_deep() {
    let (t, root, _, b, _, d) = sample_tree();
    let path: Vec<_> = t.ancestors(d).collect();
    assert_eq!(path, vec![d, b, root]);
}

#[test]
fn ancestors_root() {
    let (t, root, _, _, _, _) = sample_tree();
    let path: Vec<_> = t.ancestors(root).collect();
    assert_eq!(path, vec![root]);
}

// ---------------------------------------------------------------------------
// Traverse iterator
// ---------------------------------------------------------------------------

#[test]
fn traverse_full() {
    let (t, root, a, b, c, d) = sample_tree();
    let edges: Vec<_> = t.traverse(root).collect();
    assert_eq!(
        edges,
        vec![
            NodeEdge::Start(root),
            NodeEdge::Start(a),
            NodeEdge::End(a),
            NodeEdge::Start(b),
            NodeEdge::Start(d),
            NodeEdge::End(d),
            NodeEdge::End(b),
            NodeEdge::Start(c),
            NodeEdge::End(c),
            NodeEdge::End(root),
        ]
    );
}

#[test]
fn traverse_leaf() {
    let (t, _, _, _, _, d) = sample_tree();
    let edges: Vec<_> = t.traverse(d).collect();
    assert_eq!(edges, vec![NodeEdge::Start(d), NodeEdge::End(d)]);
}

#[test]
fn traverse_subtree() {
    let (t, _, _, b, _, d) = sample_tree();
    let edges: Vec<_> = t.traverse(b).collect();
    assert_eq!(
        edges,
        vec![
            NodeEdge::Start(b),
            NodeEdge::Start(d),
            NodeEdge::End(d),
            NodeEdge::End(b),
        ]
    );
}

// ---------------------------------------------------------------------------
// Descendants iterator
// ---------------------------------------------------------------------------

#[test]
fn descendants_full() {
    let (t, root, a, b, c, d) = sample_tree();
    let desc: Vec<_> = t.descendants(root).collect();
    assert_eq!(desc, vec![a, b, d, c]);
}

#[test]
fn descendants_leaf() {
    let (t, _, a, _, _, _) = sample_tree();
    let desc: Vec<_> = t.descendants(a).collect();
    assert!(desc.is_empty());
}

// ---------------------------------------------------------------------------
// Sibling iterators
// ---------------------------------------------------------------------------

#[test]
fn following_siblings() {
    let (t, _, a, b, c, _) = sample_tree();
    let sibs: Vec<_> = t.following_siblings(a).collect();
    assert_eq!(sibs, vec![b, c]);
}

#[test]
fn preceding_siblings() {
    let (t, _, a, b, c, _) = sample_tree();
    let sibs: Vec<_> = t.preceding_siblings(c).collect();
    assert_eq!(sibs, vec![b, a]);
}

#[test]
fn following_siblings_last() {
    let (t, _, _, _, c, _) = sample_tree();
    let sibs: Vec<_> = t.following_siblings(c).collect();
    assert!(sibs.is_empty());
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn move_node_detach_then_append() {
    let (mut t, root, a, b, c, _) = sample_tree();
    // Move 'a' to be a child of 'c'
    t.detach(a);
    t.append(c, a);

    assert_eq!(t.parent(a), Some(c));
    let root_children: Vec<_> = t.children(root).collect();
    assert_eq!(root_children, vec![b, c]);
    let c_children: Vec<_> = t.children(c).collect();
    assert_eq!(c_children, vec![a]);
}

#[test]
fn deep_tree() {
    let mut t = Tree::new();
    let mut parent = t.create(0);
    for i in 1..100 {
        let child = t.create(i);
        t.append(parent, child);
        parent = child;
    }
    assert_eq!(t.len(), 100);
    assert_eq!(t.depth(parent), 99);
}

#[test]
fn wide_tree() {
    let mut t = Tree::new();
    let root = t.create(0u32);
    let mut ids = Vec::new();
    for i in 1..=1000 {
        let child = t.create(i);
        t.append(root, child);
        ids.push(child);
    }
    assert_eq!(t.child_count(root), 1000);
    assert_eq!(t.first_child(root), Some(ids[0]));
    assert_eq!(t.last_child(root), Some(ids[999]));
}

#[test]
fn remove_then_create_stress() {
    let mut t = Tree::new();
    let mut ids: Vec<NodeId> = (0u32..50).map(|i| t.create(i)).collect();

    // Remove every other node
    for i in (0..50).step_by(2) {
        t.remove(ids[i]);
    }
    assert_eq!(t.len(), 25);

    // Create 25 new nodes — should reuse freed slots
    let slot_count = t.slot_count();
    for i in 50u32..75 {
        ids.push(t.create(i));
    }
    assert_eq!(t.slot_count(), slot_count); // no new slots
    assert_eq!(t.len(), 50);
}

#[test]
#[should_panic(expected = "cannot attach a node to itself")]
fn append_self_panics() {
    let mut t = Tree::new();
    let id = t.create("x");
    t.append(id, id);
}

#[test]
#[should_panic(expected = "child already has a parent")]
fn append_already_attached_panics() {
    let (mut t, _root, a, _, _, _) = sample_tree();
    let new_parent = t.create("new");
    t.append(new_parent, a); // a already has root as parent
}

#[test]
fn default_tree() {
    let t = Tree::<i32>::default();
    assert!(t.is_empty());
}

#[test]
fn debug_format() {
    let (t, _, _, _, _, _) = sample_tree();
    let s = format!("{t:?}");
    assert!(s.contains("Tree"));
    assert!(s.contains("len: 5"));
}

#[test]
fn node_id_debug() {
    let mut t = Tree::new();
    let id = t.create(0);
    let s = format!("{id:?}");
    assert!(s.contains("NodeId(0v1)"));
}
