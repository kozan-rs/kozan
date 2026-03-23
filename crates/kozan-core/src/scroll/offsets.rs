//! Scroll offsets — mutable scroll position per node.
//!
//! Chrome: stored in `TransformTree` because scroll IS a translate transform.
//! Separated from [`ScrollTree`](super::ScrollTree) so offset can change
//! without touching the node graph — the only mutable state during scroll.

use kozan_primitives::arena::Storage;
use kozan_primitives::geometry::Offset;

/// Current scroll displacement for every scrollable node.
///
/// Paint subtracts these offsets to translate children.
/// Hit-test adds them back to map screen coords to content coords.
#[derive(Clone)]
pub struct ScrollOffsets {
    offsets: Storage<Offset>,
}

impl ScrollOffsets {
    pub fn new() -> Self {
        Self { offsets: Storage::new() }
    }

    /// Returns `Offset::ZERO` for nodes that have never been scrolled.
    pub fn offset(&self, dom_id: u32) -> Offset {
        self.offsets.get(dom_id).copied().unwrap_or(Offset::ZERO)
    }

    pub fn set_offset(&mut self, dom_id: u32, offset: Offset) {
        self.offsets.set(dom_id, offset);
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &Offset)> + '_ {
        self.offsets.iter()
    }

    pub fn clear(&mut self) {
        self.offsets.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_offset_is_zero() {
        let offsets = ScrollOffsets::new();
        assert_eq!(offsets.offset(42), Offset::ZERO);
    }

    #[test]
    fn set_then_read() {
        let mut offsets = ScrollOffsets::new();
        offsets.set_offset(5, Offset::new(0.0, 120.0));
        assert_eq!(offsets.offset(5), Offset::new(0.0, 120.0));
    }
}
