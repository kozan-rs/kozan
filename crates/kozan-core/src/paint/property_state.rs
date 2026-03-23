//! Paint property state — the visual context for display items.
//!
//! Chrome equivalent: `PropertyTreeState` — a 4-tuple pointing to nodes
//! in the transform, clip, effect, and scroll property trees.
//!
//! For Kozan's initial implementation, we use a flat state rather than
//! full property trees. Property trees will be added when compositing
//! and incremental paint invalidation are needed.
//!
//! # Chrome mapping
//!
//! | Chrome | Kozan |
//! |--------|-------|
//! | `PropertyTreeState` | `PropertyState` |
//! | `TransformPaintPropertyNode` | `transform` field |
//! | `ClipPaintPropertyNode` | `clip` field |
//! | `EffectPaintPropertyNode` | `opacity` field |

use kozan_primitives::geometry::Rect;

/// The visual property state affecting a group of display items.
///
/// Chrome equivalent: `PropertyTreeState`.
/// Adjacent display items sharing the same `PropertyState` are grouped
/// into a `PaintChunk`.
///
/// # Future
///
/// When compositing is added, this will reference nodes in separate
/// transform/clip/effect property trees for incremental updates.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyState {
    /// Cumulative transform (translation only for now).
    /// Chrome: reference to `TransformPaintPropertyNode`.
    pub transform_x: f32,
    pub transform_y: f32,

    /// Current clip rectangle. `None` = no clip.
    /// Chrome: reference to `ClipPaintPropertyNode`.
    pub clip: Option<Rect>,

    /// Current opacity (1.0 = fully opaque).
    /// Chrome: reference to `EffectPaintPropertyNode`.
    pub opacity: f32,
}

impl Default for PropertyState {
    fn default() -> Self {
        Self {
            transform_x: 0.0,
            transform_y: 0.0,
            clip: None,
            opacity: 1.0,
        }
    }
}

impl PropertyState {
    /// Create a root property state (no transform, no clip, full opacity).
    #[must_use] 
    pub fn root() -> Self {
        Self::default()
    }

    /// Whether this state is the identity (no visual effects applied).
    #[must_use] 
    pub fn is_identity(&self) -> bool {
        self.transform_x == 0.0
            && self.transform_y == 0.0
            && self.clip.is_none()
            && self.opacity == 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_identity() {
        let state = PropertyState::root();
        assert!(state.is_identity());
    }

    #[test]
    fn non_identity_transform() {
        let state = PropertyState {
            transform_x: 10.0,
            ..PropertyState::default()
        };
        assert!(!state.is_identity());
    }

    #[test]
    fn non_identity_opacity() {
        let state = PropertyState {
            opacity: 0.5,
            ..PropertyState::default()
        };
        assert!(!state.is_identity());
    }

    #[test]
    fn non_identity_clip() {
        let state = PropertyState {
            clip: Some(Rect::new(0.0, 0.0, 100.0, 100.0)),
            ..PropertyState::default()
        };
        assert!(!state.is_identity());
    }

    #[test]
    fn equality() {
        let a = PropertyState::root();
        let b = PropertyState::root();
        assert_eq!(a, b);
    }
}
