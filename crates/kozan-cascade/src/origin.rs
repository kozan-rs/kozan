//! Cascade origin, importance, and level — packed into a single sortable `u32`.
//!
//! The CSS cascade resolves property conflicts in this order:
//! 1. Origin + importance (UA < User < Author for normal; reversed for !important)
//! 2. Cascade layer (higher layer order wins for normal; reversed for !important)
//! 3. Specificity (handled externally, not encoded here)
//! 4. Source order (handled externally, not encoded here)
//!
//! `CascadeLevel` encodes steps 1-2 into a single `u32` so that cascade
//! priority comparison is a single integer comparison.

use core::cmp::Ordering;

pub use kozan_style::Importance;

/// CSS cascade origin — where a stylesheet comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CascadeOrigin {
    /// Browser default styles (lowest priority for normal declarations).
    UserAgent = 0,
    /// User preferences / accessibility overrides.
    User = 1,
    /// Page author stylesheets (highest priority for normal declarations).
    Author = 2,
}

/// Cascade priority level — origin + importance + layer order packed into `u32`.
///
/// Bit layout (MSB first):
/// ```text
/// [31]     importance        (1 bit)
/// [30..29] effective_origin  (2 bits)
/// [28..16] effective_layer   (13 bits — up to 8191 layers)
/// [15..0]  reserved          (16 bits — available for future use)
/// ```
///
/// For **normal** declarations:
///   effective_origin = origin (Author=2 > User=1 > UA=0)
///   effective_layer  = layer_order (higher = higher priority)
///
/// For **!important** declarations (CSS spec reverses priority):
///   effective_origin = 2 - origin (UA=2 > User=1 > Author=0)
///   effective_layer  = MAX_LAYER - layer_order (lower layer = higher priority)
///
/// This inversion at construction time means natural `u32` ordering gives
/// the correct cascade priority without any branch at comparison time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CascadeLevel(u32);

const IMPORTANCE_SHIFT: u32 = 31;
const ORIGIN_SHIFT: u32 = 29;
const LAYER_SHIFT: u32 = 16;
const MAX_LAYER: u16 = 0x1FFF; // 13 bits

impl CascadeLevel {
    /// Create a cascade level from origin, importance, and layer order.
    ///
    /// Layer order is assigned by `LayerOrderMap` — higher values mean
    /// later-declared layers. Unlayered rules use `LayerOrderMap::UNLAYERED`.
    #[must_use] 
    pub fn new(origin: CascadeOrigin, importance: Importance, layer_order: u16) -> Self {
        let is_important = importance == Importance::Important;

        let effective_origin = if is_important {
            2 - origin as u32
        } else {
            origin as u32
        };

        let clamped_layer = layer_order.min(MAX_LAYER);
        let effective_layer = if is_important {
            MAX_LAYER - clamped_layer
        } else {
            clamped_layer
        };

        let bits = (effective_origin << ORIGIN_SHIFT)
            | ((is_important as u32) << IMPORTANCE_SHIFT)
            | ((effective_layer as u32) << LAYER_SHIFT);

        Self(bits)
    }

    /// The raw packed value, usable as a sort key.
    #[inline]
    #[must_use] 
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Whether this level represents an `!important` declaration.
    #[inline]
    #[must_use] 
    pub fn is_important(self) -> bool {
        (self.0 >> IMPORTANCE_SHIFT) & 1 != 0
    }
}

impl Ord for CascadeLevel {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for CascadeLevel {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNLAYERED: u16 = super::MAX_LAYER;

    fn level(origin: CascadeOrigin, important: bool, layer: u16) -> CascadeLevel {
        let imp = if important { Importance::Important } else { Importance::Normal };
        CascadeLevel::new(origin, imp, layer)
    }

    #[test]
    fn normal_origin_order() {
        let ua = level(CascadeOrigin::UserAgent, false, UNLAYERED);
        let user = level(CascadeOrigin::User, false, UNLAYERED);
        let author = level(CascadeOrigin::Author, false, UNLAYERED);
        assert!(ua < user);
        assert!(user < author);
    }

    #[test]
    fn important_reverses_origin() {
        let ua_imp = level(CascadeOrigin::UserAgent, true, UNLAYERED);
        let user_imp = level(CascadeOrigin::User, true, UNLAYERED);
        let author_imp = level(CascadeOrigin::Author, true, UNLAYERED);
        assert!(author_imp < user_imp);
        assert!(user_imp < ua_imp);
    }

    #[test]
    fn important_beats_normal() {
        let author_normal = level(CascadeOrigin::Author, false, UNLAYERED);
        let author_imp = level(CascadeOrigin::Author, true, UNLAYERED);
        assert!(author_normal < author_imp);
    }

    #[test]
    fn normal_layer_order() {
        let layer0 = level(CascadeOrigin::Author, false, 0);
        let layer1 = level(CascadeOrigin::Author, false, 1);
        let unlayered = level(CascadeOrigin::Author, false, UNLAYERED);
        assert!(layer0 < layer1);
        assert!(layer1 < unlayered);
    }

    #[test]
    fn important_reverses_layer_order() {
        let layer0_imp = level(CascadeOrigin::Author, true, 0);
        let layer1_imp = level(CascadeOrigin::Author, true, 1);
        let unlayered_imp = level(CascadeOrigin::Author, true, UNLAYERED);
        assert!(unlayered_imp < layer1_imp);
        assert!(layer1_imp < layer0_imp);
    }

    #[test]
    fn full_cascade_order() {
        // CSS Cascading Level 5 full order (ascending priority):
        // 1. UA normal
        // 2. User normal
        // 3. Author normal (layer 0 < layer 1 < unlayered)
        // 4. Author !important (unlayered < layer 1 < layer 0)
        // 5. User !important
        // 6. UA !important
        let levels = [
            level(CascadeOrigin::UserAgent, false, UNLAYERED),
            level(CascadeOrigin::User, false, UNLAYERED),
            level(CascadeOrigin::Author, false, 0),
            level(CascadeOrigin::Author, false, 1),
            level(CascadeOrigin::Author, false, UNLAYERED),
            level(CascadeOrigin::Author, true, UNLAYERED),
            level(CascadeOrigin::Author, true, 1),
            level(CascadeOrigin::Author, true, 0),
            level(CascadeOrigin::User, true, UNLAYERED),
            level(CascadeOrigin::UserAgent, true, UNLAYERED),
        ];
        for i in 0..levels.len() - 1 {
            assert!(
                levels[i] < levels[i + 1],
                "expected levels[{}] < levels[{}]: {:?} vs {:?}",
                i, i + 1, levels[i], levels[i + 1],
            );
        }
    }

    #[test]
    fn is_important_flag() {
        assert!(!level(CascadeOrigin::Author, false, 0).is_important());
        assert!(level(CascadeOrigin::Author, true, 0).is_important());
    }
}
