//! Document lifecycle — state machine enforcing phase ordering.
//!
//! Chrome equivalent: `DocumentLifecycle` in `core/dom/document_lifecycle.h`.
//!
//! Phases progress in strict order:
//! ```text
//! VisualUpdatePending → InStyleRecalc → StyleClean
//!                     → InLayout → LayoutClean
//!                     → InPrePaint → PrePaintClean
//!                     → InPaint → PaintClean
//! ```
//!
//! Each phase gates the next. You cannot run layout before style is clean,
//! and you cannot paint before layout is clean.

/// The current lifecycle state of a document.
///
/// Chrome: `DocumentLifecycle::LifecycleState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Default)]
pub enum LifecycleState {
    /// Something visual changed — needs work.
    VisualUpdatePending,
    /// Style recalc is running.
    InStyleRecalc,
    /// Styles computed — ready for layout.
    StyleClean,
    /// Layout is running.
    InLayout,
    /// Layout complete — ready for paint.
    LayoutClean,
    /// Pre-paint (paint property trees) is running.
    InPrePaint,
    /// Pre-paint complete.
    PrePaintClean,
    /// Paint (display list generation) is running.
    InPaint,
    /// All phases complete — everything clean.
    #[default]
    PaintClean,
}


impl LifecycleState {
    /// Whether this state is "clean" (not in the middle of a phase).
    #[inline]
    #[must_use] 
    pub fn is_clean(self) -> bool {
        matches!(
            self,
            Self::StyleClean | Self::LayoutClean | Self::PrePaintClean | Self::PaintClean
        )
    }

    /// Whether layout results are up-to-date.
    #[inline]
    #[must_use] 
    pub fn is_layout_clean(self) -> bool {
        self >= Self::LayoutClean
    }

    /// Whether paint results are up-to-date.
    #[inline]
    #[must_use] 
    pub fn is_paint_clean(self) -> bool {
        self >= Self::PaintClean
    }

    /// Mark that a visual update is needed (invalidate all phases).
    ///
    /// Chrome: `DocumentLifecycle::SetVisualUpdatePending()`.
    /// Called when DOM changes, style changes, or viewport resizes.
    #[inline]
    pub fn invalidate(&mut self) {
        *self = Self::VisualUpdatePending;
    }

    /// Whether any work needs to be done.
    #[inline]
    #[must_use] 
    pub fn needs_update(self) -> bool {
        self != Self::PaintClean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_clean() {
        assert_eq!(LifecycleState::default(), LifecycleState::PaintClean);
    }

    #[test]
    fn ordering() {
        assert!(LifecycleState::VisualUpdatePending < LifecycleState::StyleClean);
        assert!(LifecycleState::StyleClean < LifecycleState::LayoutClean);
        assert!(LifecycleState::LayoutClean < LifecycleState::PaintClean);
    }

    #[test]
    fn invalidate_resets() {
        let mut state = LifecycleState::PaintClean;
        assert!(!state.needs_update());
        state.invalidate();
        assert!(state.needs_update());
        assert_eq!(state, LifecycleState::VisualUpdatePending);
    }

    #[test]
    fn is_layout_clean_checks() {
        assert!(!LifecycleState::StyleClean.is_layout_clean());
        assert!(LifecycleState::LayoutClean.is_layout_clean());
        assert!(LifecycleState::PaintClean.is_layout_clean());
    }
}
