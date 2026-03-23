//! `Text` — a leaf node containing text content.
//!
//! Like Chrome's `Text` class: inherits from `CharacterData → Node`.
//! Text nodes are NOT elements — they have no tag name, no attributes,
//! and cannot have children.
//!
//! `text.append(child)` is a **compile error** because `Text`
//! only implements `Node`, not `ContainerNode`.

use kozan_macros::Node;

use crate::Handle;

/// A text node. Leaf only — cannot have children.
///
/// Implements `Node` but NOT `ContainerNode` or `Element`.
/// Access text content via `content()` / `set_content()`.
#[derive(Copy, Clone, Node)]
pub struct Text(Handle);

/// Text content storage in `DataStorage`.
#[derive(Default, Clone)]
pub struct TextData {
    pub content: String,
}

impl Text {
    /// Wrap a Handle into a Text node. Called by `Document::create_text`.
    pub(crate) fn from_raw(handle: Handle) -> Self {
        Self(handle)
    }

    /// Get the text content.
    #[inline]
    #[must_use] 
    pub fn content(&self) -> String {
        self.0
            .read_data::<TextData, _>(|d| d.content.clone())
            .unwrap_or_default()
    }

    /// Set the text content. No-op if value is unchanged.
    ///
    /// Chrome: `CharacterData::setData()` — compares old vs new, skips if same.
    /// When text changes, marks parent for restyle + sets `needs_layout`.
    pub fn set_content(&self, value: impl Into<String>) {
        let value = value.into();
        let changed = self.0.write_data::<TextData, _>(|d| {
            if d.content == value {
                return false; // Same value — no-op.
            }
            d.content = value;
            true
        }).unwrap_or(false);

        if changed {
            // Text content changed → parent needs relayout (text affects sizing).
            // Text nodes have no ElementData, so mark the parent element.
            self.0.mark_parent_needs_layout();
        }
    }
}
