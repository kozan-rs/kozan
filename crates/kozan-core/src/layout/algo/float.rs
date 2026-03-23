//! Float layout — CSS float positioning and clearing.
//!
//! Chrome equivalent: `NGFloatTypes`, `NGExclusionSpace`.
//!
//! # How CSS floats work
//!
//! A floated element is removed from normal flow but still affects
//! the inline content around it. Non-floated block elements ignore
//! floats (they lay out as if floats don't exist), but their inline
//! content wraps around floats.
//!
//! # Exclusion zones
//!
//! Floats create "exclusion zones" — rectangular areas where inline
//! content cannot be placed. The exclusion space tracks all active
//! floats and computes available inline space for each line.

use kozan_primitives::geometry::Size;

/// A float exclusion — a rectangle reserved by a floated element.
#[derive(Debug, Clone, Copy)]
pub struct FloatExclusion {
    /// Float direction.
    pub side: FloatSide,
    /// Top edge of the float (block offset from container top).
    pub block_start: f32,
    /// Bottom edge of the float.
    pub block_end: f32,
    /// Width of the float.
    pub inline_size: f32,
}

/// Which side a float attaches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSide {
    Left,
    Right,
}

/// Tracks all active floats in a block formatting context.
///
/// Chrome equivalent: `NGExclusionSpace`.
#[derive(Debug, Clone, Default)]
pub struct ExclusionSpace {
    floats: Vec<FloatExclusion>,
}

impl ExclusionSpace {
    #[must_use] 
    pub fn new() -> Self {
        Self { floats: Vec::new() }
    }

    /// Add a float exclusion.
    pub fn add_float(&mut self, exclusion: FloatExclusion) {
        self.floats.push(exclusion);
    }

    /// Get the available inline range at a given block position.
    ///
    /// Returns (`left_offset`, `available_width`) after accounting for
    /// all active floats at this block position.
    #[must_use] 
    pub fn available_inline_at(&self, block_pos: f32, container_width: f32) -> (f32, f32) {
        let mut left_offset: f32 = 0.0;
        let mut right_offset: f32 = 0.0;

        for float in &self.floats {
            // Only floats that overlap this block position.
            if block_pos >= float.block_start && block_pos < float.block_end {
                match float.side {
                    FloatSide::Left => {
                        left_offset = left_offset.max(float.inline_size);
                    }
                    FloatSide::Right => {
                        right_offset = right_offset.max(float.inline_size);
                    }
                }
            }
        }

        let available = (container_width - left_offset - right_offset).max(0.0);
        (left_offset, available)
    }

    /// Find the block position where a float of given size can be placed.
    ///
    /// The float must not overlap with existing floats on the same side.
    #[must_use] 
    pub fn find_float_position(
        &self,
        side: FloatSide,
        float_size: Size,
        block_start: f32,
        container_width: f32,
    ) -> (f32, f32) {
        let mut block_pos = block_start;

        // Scan from block_start downward until there's room.
        // Guard: max iterations = number of floats + 1 (each iteration
        // advances past at least one float's block_end).
        let max_iterations = self.floats.len() + 1;
        for _ in 0..max_iterations {
            let (left_used, available) = self.available_inline_at(block_pos, container_width);

            if float_size.width <= available {
                let inline_pos = match side {
                    FloatSide::Left => left_used,
                    FloatSide::Right => {
                        let right_used = container_width - left_used - available;
                        container_width - right_used - float_size.width
                    }
                };
                return (inline_pos, block_pos);
            }

            // Move past the lowest float at this position.
            let mut next_block = f32::INFINITY;
            for float in &self.floats {
                if block_pos >= float.block_start && block_pos < float.block_end {
                    next_block = next_block.min(float.block_end);
                }
            }

            if next_block.is_infinite() {
                // No overlapping floats — place here.
                let inline_pos = match side {
                    FloatSide::Left => 0.0,
                    FloatSide::Right => container_width - float_size.width,
                };
                return (inline_pos, block_pos);
            }

            block_pos = next_block;
        }

        // Fallback: place at current block_pos if loop exhausted.
        let inline_pos = match side {
            FloatSide::Left => 0.0,
            FloatSide::Right => (container_width - float_size.width).max(0.0),
        };
        (inline_pos, block_pos)
    }

    /// Get the clear position (block offset below all floats on a side).
    ///
    /// CSS `clear: left/right/both`.
    #[must_use] 
    pub fn clear_position(&self, side: ClearSide) -> f32 {
        let mut max_end: f32 = 0.0;
        for float in &self.floats {
            let matches = match side {
                ClearSide::Left => float.side == FloatSide::Left,
                ClearSide::Right => float.side == FloatSide::Right,
                ClearSide::Both => true,
            };
            if matches {
                max_end = max_end.max(float.block_end);
            }
        }
        max_end
    }

}

/// Which sides to clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearSide {
    Left,
    Right,
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_exclusion_space() {
        let space = ExclusionSpace::new();
        let (left, available) = space.available_inline_at(0.0, 800.0);
        assert_eq!(left, 0.0);
        assert_eq!(available, 800.0);
    }

    #[test]
    fn left_float_reduces_available() {
        let mut space = ExclusionSpace::new();
        space.add_float(FloatExclusion {
            side: FloatSide::Left,
            block_start: 0.0,
            block_end: 100.0,
            inline_size: 200.0,
        });

        let (left, available) = space.available_inline_at(50.0, 800.0);
        assert_eq!(left, 200.0);
        assert_eq!(available, 600.0);

        // Below the float — full width available.
        let (left, available) = space.available_inline_at(150.0, 800.0);
        assert_eq!(left, 0.0);
        assert_eq!(available, 800.0);
    }

    #[test]
    fn both_sides_float() {
        let mut space = ExclusionSpace::new();
        space.add_float(FloatExclusion {
            side: FloatSide::Left,
            block_start: 0.0,
            block_end: 100.0,
            inline_size: 200.0,
        });
        space.add_float(FloatExclusion {
            side: FloatSide::Right,
            block_start: 0.0,
            block_end: 80.0,
            inline_size: 150.0,
        });

        let (left, available) = space.available_inline_at(50.0, 800.0);
        assert_eq!(left, 200.0);
        assert_eq!(available, 450.0); // 800 - 200 - 150
    }

    #[test]
    fn clear_position() {
        let mut space = ExclusionSpace::new();
        space.add_float(FloatExclusion {
            side: FloatSide::Left,
            block_start: 0.0,
            block_end: 100.0,
            inline_size: 200.0,
        });
        space.add_float(FloatExclusion {
            side: FloatSide::Right,
            block_start: 20.0,
            block_end: 150.0,
            inline_size: 100.0,
        });

        assert_eq!(space.clear_position(ClearSide::Left), 100.0);
        assert_eq!(space.clear_position(ClearSide::Right), 150.0);
        assert_eq!(space.clear_position(ClearSide::Both), 150.0);
    }

    #[test]
    fn find_float_position_no_overlap() {
        let space = ExclusionSpace::new();
        let (x, y) =
            space.find_float_position(FloatSide::Left, Size::new(200.0, 100.0), 0.0, 800.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn find_float_position_with_existing() {
        let mut space = ExclusionSpace::new();
        space.add_float(FloatExclusion {
            side: FloatSide::Left,
            block_start: 0.0,
            block_end: 100.0,
            inline_size: 300.0,
        });

        // Second left float — should go to the right of the first.
        let (x, y) = space.find_float_position(FloatSide::Left, Size::new(200.0, 50.0), 0.0, 800.0);
        assert_eq!(x, 300.0); // After the first float.
        assert_eq!(y, 0.0);
    }

    #[test]
    fn two_right_floats() {
        // Two right floats at the same block position.
        // Second should be pushed below the first (not overlap).
        let mut space = ExclusionSpace::new();
        space.add_float(FloatExclusion {
            side: FloatSide::Right,
            block_start: 0.0,
            block_end: 100.0,
            inline_size: 200.0,
        });

        // Second right float: 200px wide in an 800px container.
        // First right float already takes 200px, leaving 600px.
        // 200px fits in 600px, so it should be placed on the same row
        // but the exclusion space tracks them. Let's place one that
        // does NOT fit: 700px wide, so it must go below.
        let (x, y) =
            space.find_float_position(FloatSide::Right, Size::new(700.0, 50.0), 0.0, 800.0);
        // Not enough room at y=0 (available = 800 - 200 = 600 < 700).
        // Should be placed at y=100 (after the first float ends).
        assert_eq!(y, 100.0, "second float should stack below first");
        assert_eq!(x, 100.0, "right float at x = 800 - 700 = 100");
    }

    #[test]
    fn right_float_accounts_for_existing_right_float() {
        let mut space = ExclusionSpace::new();
        space.add_float(FloatExclusion {
            side: FloatSide::Right,
            block_start: 0.0,
            block_end: 100.0,
            inline_size: 200.0,
        });

        // 150px float fits alongside the existing 200px right float.
        // Available = 800 - 200 = 600, so 150 fits.
        // x = 800 - 200 (existing) - 150 (new) = 450.
        let (x, y) =
            space.find_float_position(FloatSide::Right, Size::new(150.0, 50.0), 0.0, 800.0);
        assert_eq!(y, 0.0, "fits on the same row");
        assert_eq!(x, 450.0, "placed to the left of the existing right float");
    }

    #[test]
    fn float_with_no_room() {
        // Float wider than container. Should still be placed (overflow).
        let space = ExclusionSpace::new();
        let (x, y) = space.find_float_position(
            FloatSide::Left,
            Size::new(1000.0, 50.0), // 1000px in an 800px container
            0.0,
            800.0,
        );
        // No existing floats, so available = 800. Float is 1000 > 800,
        // but with no floats to dodge, the fallback places it at (0, 0).
        // The float overflows the container — that's fine per CSS spec.
        assert_eq!(x, 0.0, "oversized left float starts at x=0");
        assert_eq!(y, 0.0, "placed at the requested block start");
    }
}
