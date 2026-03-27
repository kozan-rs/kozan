//! Display list — the ordered list of display items for one frame.
//!
//! Chrome equivalent: `PaintArtifact` = `DisplayItemList` + `PaintChunk`s.
//!
//! The display list is the output of the paint phase and the input
//! to the renderer backend (vello, wgpu, etc.).
//!
//! # Structure
//!
//! ```text
//! DisplayList
//!   ├── items: Vec<DisplayItem>     (all items in paint order)
//!   └── chunks: Vec<PaintChunk>     (groups of items sharing PropertyState)
//! ```
//!
//! # `PaintChunk`
//!
//! Adjacent display items sharing the same `PropertyState` (same transform,
//! clip, effect) are grouped into a `PaintChunk`. The compositor uses chunks
//! to decide GPU layer boundaries.

use super::display_item::DisplayItem;
use super::property_state::PropertyState;

/// A paint chunk — a group of display items sharing the same property state.
///
/// Chrome equivalent: `PaintChunk`.
/// The compositor uses chunks to determine GPU layer boundaries.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PaintChunk {
    /// The property state shared by all items in this chunk.
    pub state: PropertyState,
    /// Start index in the display list's items array (inclusive).
    pub start: usize,
    /// End index in the display list's items array (exclusive).
    pub end: usize,
}

impl PaintChunk {
    /// Number of display items in this chunk.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether this chunk is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// The display list — all draw commands for one paint pass.
///
/// Chrome equivalent: `PaintArtifact` (items + chunks).
///
/// Built by the painter, consumed by the renderer backend.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    /// All display items in paint order.
    items: Vec<DisplayItem>,
    /// Paint chunks — groups of items sharing the same `PropertyState`.
    chunks: Vec<PaintChunk>,
}

impl DisplayList {
    /// Create a new empty display list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start building a new display list.
    #[must_use]
    pub fn builder() -> DisplayListBuilder {
        DisplayListBuilder::new()
    }

    /// Total number of display items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the display list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get all display items in paint order.
    #[must_use]
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    /// Get all paint chunks.
    #[must_use]
    pub fn chunks(&self) -> &[PaintChunk] {
        &self.chunks
    }

    /// Iterate over display items.
    pub fn iter(&self) -> impl Iterator<Item = &DisplayItem> {
        self.items.iter()
    }

    /// Get items for a specific chunk.
    #[must_use]
    pub fn chunk_items(&self, chunk: &PaintChunk) -> &[DisplayItem] {
        &self.items[chunk.start..chunk.end]
    }
}

/// Builder for constructing a display list incrementally.
///
/// Chrome equivalent: `PaintController` (the state machine that
/// tracks current property state and builds chunks).
///
/// # Usage
///
/// ```ignore
/// let mut builder = DisplayList::builder();
/// builder.push(DisplayItem::Draw(DrawCommand::Rect { ... }));
/// builder.push_state(new_property_state);
/// builder.push(DisplayItem::Draw(DrawCommand::Text { ... }));
/// let display_list = builder.finish();
/// ```
pub struct DisplayListBuilder {
    items: Vec<DisplayItem>,
    chunks: Vec<PaintChunk>,
    current_state: PropertyState,
    current_chunk_start: usize,
}

impl DisplayListBuilder {
    /// Create a new builder with root property state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            chunks: Vec::new(),
            current_state: PropertyState::root(),
            current_chunk_start: 0,
        }
    }

    /// Push a display item into the current chunk.
    pub fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// Change the property state, starting a new chunk if different.
    ///
    /// Chrome equivalent: `PaintController::UpdateCurrentPaintChunkProperties()`.
    pub fn set_state(&mut self, state: PropertyState) {
        if state != self.current_state {
            self.finish_chunk();
            self.current_state = state;
            self.current_chunk_start = self.items.len();
        }
    }

    /// Finish building and return the display list.
    #[must_use]
    pub fn finish(mut self) -> DisplayList {
        self.finish_chunk();
        DisplayList {
            items: self.items,
            chunks: self.chunks,
        }
    }

    /// Close the current chunk (if non-empty).
    fn finish_chunk(&mut self) {
        let end = self.items.len();
        if end > self.current_chunk_start {
            self.chunks.push(PaintChunk {
                state: self.current_state.clone(),
                start: self.current_chunk_start,
                end,
            });
        }
    }
}

impl Default for DisplayListBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::display_item::{ClipData, DrawCommand};
    use kozan_primitives::color::Color;
    use kozan_primitives::geometry::Rect;

    #[test]
    fn empty_display_list() {
        let list = DisplayList::builder().finish();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.chunks().is_empty());
    }

    #[test]
    fn single_item_single_chunk() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::RED,
        }));
        let list = builder.finish();

        assert_eq!(list.len(), 1);
        assert_eq!(list.chunks().len(), 1);
        assert_eq!(list.chunks()[0].start, 0);
        assert_eq!(list.chunks()[0].end, 1);
    }

    #[test]
    fn same_state_groups_into_one_chunk() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::RED,
        }));
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 50.0, 100.0, 50.0),
            color: Color::BLUE,
        }));
        let list = builder.finish();

        // Same property state → one chunk with 2 items.
        assert_eq!(list.len(), 2);
        assert_eq!(list.chunks().len(), 1);
    }

    #[test]
    fn different_state_creates_new_chunk() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::RED,
        }));

        // Change state — new chunk starts.
        builder.set_state(PropertyState {
            opacity: 0.5,
            ..PropertyState::default()
        });

        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 50.0, 100.0, 50.0),
            color: Color::BLUE,
        }));

        let list = builder.finish();

        assert_eq!(list.len(), 2);
        assert_eq!(list.chunks().len(), 2);
        assert_eq!(list.chunks()[0].state.opacity, 1.0);
        assert_eq!(list.chunks()[1].state.opacity, 0.5);
    }

    #[test]
    fn chunk_items_accessor() {
        let mut builder = DisplayList::builder();
        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::RED,
        }));
        builder.push(DisplayItem::PushClip(ClipData {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
        }));
        let list = builder.finish();

        let chunk = &list.chunks()[0];
        let items = list.chunk_items(chunk);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn empty_state_change_no_empty_chunk() {
        let mut builder = DisplayList::builder();

        // Change state without pushing items — no empty chunk.
        builder.set_state(PropertyState {
            opacity: 0.5,
            ..PropertyState::default()
        });

        builder.push(DisplayItem::Draw(DrawCommand::Rect {
            rect: Rect::new(0.0, 0.0, 100.0, 50.0),
            color: Color::RED,
        }));

        let list = builder.finish();

        // Only one chunk (the empty first chunk was skipped).
        assert_eq!(list.chunks().len(), 1);
        assert_eq!(list.chunks()[0].state.opacity, 0.5);
    }
}
