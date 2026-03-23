// Node flags — Chrome-style packed u32 bitfield.
//
// Like Chrome's `node_flags_` (32 bits), this stores:
// - Node type (4 bits) — Element, Text, Document, etc.
// - Capability flags — is_container, is_connected
// - Dirty flags — style, layout, paint, tree
// - Focus state
//
// Type checking is a mask operation, not a virtual call or enum match.
// `is_element()` = one AND + one CMP. Same as Chrome.

use core::any::TypeId;

// ============================================================
// NodeFlags — 32-bit packed bitfield
// ============================================================

/// Packed node metadata. 4 bytes per node.
///
/// Layout (matching Chrome's design):
/// ```text
/// Bits  0-3:  NodeType (4 bits, 16 possible values)
/// Bit   4:    is_container (can have children)
/// Bit   5:    is_connected (attached to a document tree)
/// Bit   6:    needs_style_recalc
/// Bit   7:    child_needs_style_recalc
/// Bit   8:    needs_layout
/// Bit   9:    child_needs_layout
/// Bit  10:    needs_paint
/// Bit  11:    child_needs_paint
/// Bit  12:    is_focusable
/// Bit  13:    is_focused
/// Bit  14:    tree_structure_changed
/// Bits 15-31: reserved for future use
/// ```
#[derive(Copy, Clone, Debug)]
pub struct NodeFlags(u32);

// Bit masks.
const NODE_TYPE_MASK: u32 = 0b1111; // bits 0-3
const IS_CONTAINER: u32 = 1 << 4;
const IS_CONNECTED: u32 = 1 << 5;
const NEEDS_STYLE_RECALC: u32 = 1 << 6;
const CHILD_NEEDS_STYLE: u32 = 1 << 7;
const NEEDS_LAYOUT: u32 = 1 << 8;
const CHILD_NEEDS_LAYOUT: u32 = 1 << 9;
const NEEDS_PAINT: u32 = 1 << 10;
const CHILD_NEEDS_PAINT: u32 = 1 << 11;
const IS_FOCUSABLE: u32 = 1 << 12;
const IS_FOCUSED: u32 = 1 << 13;
const TREE_CHANGED: u32 = 1 << 14;

/// Node type constants (stored in bits 0-3).
/// Values match the DOM spec / Chrome's `NodeType` enum.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeType {
    Element = 1,
    Text = 3,
    Comment = 8,
    Document = 9,
    DocumentType = 10,
    DocumentFragment = 11,
}

impl NodeType {
    /// Convert from the raw 4-bit value. Returns None for unknown types.
    #[must_use] 
    pub fn from_raw(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Element),
            3 => Some(Self::Text),
            8 => Some(Self::Comment),
            9 => Some(Self::Document),
            10 => Some(Self::DocumentType),
            11 => Some(Self::DocumentFragment),
            _ => None,
        }
    }
}

impl NodeFlags {
    // ---- Constructors for each node kind ----

    /// Flags for an element node (container, not connected).
    #[must_use] 
    pub fn element(focusable: bool) -> Self {
        let mut flags = (NodeType::Element as u32) | IS_CONTAINER;
        if focusable {
            flags |= IS_FOCUSABLE;
        }
        Self(flags)
    }

    /// Flags for a text node (leaf, not container).
    #[must_use] 
    pub fn text() -> Self {
        Self(NodeType::Text as u32)
    }

    /// Flags for the document root node (container).
    #[must_use] 
    pub fn document() -> Self {
        Self((NodeType::Document as u32) | IS_CONTAINER | IS_CONNECTED)
    }

    // ---- Type queries (inline, no virtual dispatch) ----

    /// The DOM node type.
    #[inline]
    #[must_use]
    pub fn node_type(self) -> NodeType {
        NodeType::from_raw(self.0 & NODE_TYPE_MASK)
            .expect("NodeKey always stores a valid NodeType in its low bits")
    }

    /// Is this an element node? (single AND + CMP)
    #[inline]
    #[must_use] 
    pub fn is_element(self) -> bool {
        (self.0 & NODE_TYPE_MASK) == NodeType::Element as u32
    }

    /// Is this a text node?
    #[inline]
    #[must_use] 
    pub fn is_text(self) -> bool {
        (self.0 & NODE_TYPE_MASK) == NodeType::Text as u32
    }

    /// Is this the document node?
    #[inline]
    #[must_use] 
    pub fn is_document(self) -> bool {
        (self.0 & NODE_TYPE_MASK) == NodeType::Document as u32
    }

    /// Can this node have children? (Element, Document, `DocumentFragment`)
    #[inline]
    #[must_use] 
    pub fn is_container(self) -> bool {
        (self.0 & IS_CONTAINER) != 0
    }

    /// Is this node connected to a document tree?
    #[inline]
    #[must_use] 
    pub fn is_connected(self) -> bool {
        (self.0 & IS_CONNECTED) != 0
    }

    // ---- Focus ----

    #[inline]
    #[must_use] 
    pub fn is_focusable(self) -> bool {
        (self.0 & IS_FOCUSABLE) != 0
    }

    #[inline]
    #[must_use] 
    pub fn is_focused(self) -> bool {
        (self.0 & IS_FOCUSED) != 0
    }

    pub fn set_focused(&mut self, focused: bool) {
        if focused {
            self.0 |= IS_FOCUSED;
        } else {
            self.0 &= !IS_FOCUSED;
        }
    }

    // ---- Connection ----

    pub fn set_connected(&mut self, connected: bool) {
        if connected {
            self.0 |= IS_CONNECTED;
        } else {
            self.0 &= !IS_CONNECTED;
        }
    }

    // ---- Dirty flags ----

    #[inline]
    #[must_use] 
    pub fn needs_style_recalc(self) -> bool {
        (self.0 & NEEDS_STYLE_RECALC) != 0
    }

    #[inline]
    #[must_use] 
    pub fn child_needs_style_recalc(self) -> bool {
        (self.0 & CHILD_NEEDS_STYLE) != 0
    }

    #[inline]
    #[must_use] 
    pub fn needs_layout(self) -> bool {
        (self.0 & NEEDS_LAYOUT) != 0
    }

    #[inline]
    #[must_use] 
    pub fn child_needs_layout(self) -> bool {
        (self.0 & CHILD_NEEDS_LAYOUT) != 0
    }

    #[inline]
    #[must_use] 
    pub fn needs_paint(self) -> bool {
        (self.0 & NEEDS_PAINT) != 0
    }

    #[inline]
    #[must_use] 
    pub fn child_needs_paint(self) -> bool {
        (self.0 & CHILD_NEEDS_PAINT) != 0
    }

    /// Mark this node as needing style recalculation.
    /// Cascades to layout + paint.
    pub fn mark_style_dirty(&mut self) {
        self.0 |= NEEDS_STYLE_RECALC | NEEDS_LAYOUT | NEEDS_PAINT;
    }

    /// Mark this node as needing layout. Cascades to paint.
    pub fn mark_layout_dirty(&mut self) {
        self.0 |= NEEDS_LAYOUT | NEEDS_PAINT;
    }

    /// Mark this node as needing repaint only.
    pub fn mark_paint_dirty(&mut self) {
        self.0 |= NEEDS_PAINT;
    }

    /// Mark tree structure changed.
    pub fn mark_tree_dirty(&mut self) {
        self.0 |= TREE_CHANGED | NEEDS_LAYOUT | NEEDS_PAINT;
    }

    /// Mark that a child needs style recalculation.
    pub fn mark_child_style_dirty(&mut self) {
        self.0 |= CHILD_NEEDS_STYLE;
    }

    /// Mark that a child needs layout.
    pub fn mark_child_layout_dirty(&mut self) {
        self.0 |= CHILD_NEEDS_LAYOUT;
    }

    /// Mark that a child needs paint.
    pub fn mark_child_paint_dirty(&mut self) {
        self.0 |= CHILD_NEEDS_PAINT;
    }

    /// Is any dirty flag set on this node or its children?
    #[inline]
    #[must_use] 
    pub fn is_dirty(self) -> bool {
        (self.0
            & (NEEDS_STYLE_RECALC
                | NEEDS_LAYOUT
                | NEEDS_PAINT
                | CHILD_NEEDS_STYLE
                | CHILD_NEEDS_LAYOUT
                | CHILD_NEEDS_PAINT
                | TREE_CHANGED))
            != 0
    }

    /// Clear all dirty flags after a full pipeline pass.
    pub fn clear_all_dirty(&mut self) {
        self.0 &= !(NEEDS_STYLE_RECALC
            | CHILD_NEEDS_STYLE
            | NEEDS_LAYOUT
            | CHILD_NEEDS_LAYOUT
            | NEEDS_PAINT
            | CHILD_NEEDS_PAINT
            | TREE_CHANGED);
    }

    /// Get the raw u32 value.
    #[inline]
    #[must_use] 
    pub fn raw(self) -> u32 {
        self.0
    }
}

// ============================================================
// NodeMeta — per-node metadata in parallel storage
// ============================================================

/// Per-node metadata stored in `Storage<NodeMeta>`.
///
/// Combines `NodeFlags` (4 bytes) + data `TypeId` (16 bytes on 64-bit).
/// The `TypeId` identifies which column in `DataStorage` holds this node's
/// element-specific data (e.g., `TypeId::of::<ButtonData>()`).
#[derive(Copy, Clone)]
pub struct NodeMeta {
    /// Packed flags: node type, container, dirty, focus, connection.
    pub flags: NodeFlags,
    /// `TypeId` of the data in `DataStorage`. `TypeId::of::<()>()` for no data.
    pub data_type_id: TypeId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_flags() {
        let flags = NodeFlags::element(false);
        assert!(flags.is_element());
        assert!(!flags.is_text());
        assert!(!flags.is_document());
        assert!(flags.is_container());
        assert!(!flags.is_focusable());
    }

    #[test]
    fn focusable_element() {
        let flags = NodeFlags::element(true);
        assert!(flags.is_element());
        assert!(flags.is_focusable());
        assert!(flags.is_container());
    }

    #[test]
    fn text_flags() {
        let flags = NodeFlags::text();
        assert!(flags.is_text());
        assert!(!flags.is_element());
        assert!(!flags.is_container()); // text nodes can't have children
    }

    #[test]
    fn document_flags() {
        let flags = NodeFlags::document();
        assert!(flags.is_document());
        assert!(flags.is_container());
        assert!(flags.is_connected());
    }

    #[test]
    fn dirty_flags_cascade() {
        let mut flags = NodeFlags::element(false);
        assert!(!flags.is_dirty());

        flags.mark_style_dirty();
        assert!(flags.needs_style_recalc());
        assert!(flags.needs_layout());
        assert!(flags.needs_paint());
        assert!(flags.is_dirty());

        flags.clear_all_dirty();
        assert!(!flags.is_dirty());
        assert!(!flags.needs_style_recalc());
        assert!(!flags.needs_layout());
        assert!(!flags.needs_paint());
    }

    #[test]
    fn child_dirty_propagation() {
        let mut flags = NodeFlags::element(false);
        flags.mark_child_style_dirty();
        assert!(flags.child_needs_style_recalc());
        assert!(flags.is_dirty());
    }

    #[test]
    fn focus_state() {
        let mut flags = NodeFlags::element(true);
        assert!(!flags.is_focused());
        flags.set_focused(true);
        assert!(flags.is_focused());
        flags.set_focused(false);
        assert!(!flags.is_focused());
    }

    #[test]
    fn node_type_values_match_dom_spec() {
        assert_eq!(NodeType::Element as u8, 1);
        assert_eq!(NodeType::Text as u8, 3);
        assert_eq!(NodeType::Comment as u8, 8);
        assert_eq!(NodeType::Document as u8, 9);
        assert_eq!(NodeType::DocumentType as u8, 10);
        assert_eq!(NodeType::DocumentFragment as u8, 11);
    }

    #[test]
    fn node_type_from_raw() {
        assert_eq!(NodeType::from_raw(1), Some(NodeType::Element));
        assert_eq!(NodeType::from_raw(3), Some(NodeType::Text));
        assert_eq!(NodeType::from_raw(9), Some(NodeType::Document));
        assert_eq!(NodeType::from_raw(0), None);
        assert_eq!(NodeType::from_raw(99), None);
    }

    #[test]
    fn size_of_node_flags() {
        assert_eq!(core::mem::size_of::<NodeFlags>(), 4); // must be u32
    }
}
