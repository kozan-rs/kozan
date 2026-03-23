//! Dirty phase tracking — which pipeline phases need re-running.
//!
//! Chrome: `DocumentLifecycle` dirty bits separate from the phase enum.
//!
//! Scroll invalidates paint only. Hover invalidates style→layout→paint.
//! Resize invalidates all three. Each phase clears its own flag on completion.

/// Tracks which rendering phases need re-running.
///
/// Packed into a `u8` — no allocation, no external dep.
/// The ordering is strict: style before layout, layout before paint.
#[derive(Debug, Clone, Copy, Default)]
pub struct DirtyPhases(u8);

impl DirtyPhases {
    const STYLE: u8  = 0b001;
    const LAYOUT: u8 = 0b010;
    const PAINT: u8  = 0b100;
    const ALL: u8    = Self::STYLE | Self::LAYOUT | Self::PAINT;

    /// DOM mutation, resize — everything needs re-running.
    pub fn invalidate_all(&mut self) { self.0 = Self::ALL; }

    /// Hover, class change — style onward.
    pub fn invalidate_style(&mut self) { self.0 |= Self::ALL; }

    /// Viewport resize — layout + paint, skip style (styles didn't change).
    pub fn invalidate_layout(&mut self) { self.0 |= Self::LAYOUT | Self::PAINT; }

    /// Scroll offset changed — repaint only, skip style+layout.
    pub fn invalidate_paint(&mut self) { self.0 |= Self::PAINT; }

    pub fn needs_style(self) -> bool { self.0 & Self::STYLE != 0 }
    pub fn needs_layout(self) -> bool { self.0 & Self::LAYOUT != 0 }
    pub fn needs_paint(self) -> bool { self.0 & Self::PAINT != 0 }
    pub fn needs_update(self) -> bool { self.0 != 0 }

    pub fn clear_style(&mut self) { self.0 &= !Self::STYLE; }
    pub fn clear_layout(&mut self) { self.0 &= !Self::LAYOUT; }
    pub fn clear_paint(&mut self) { self.0 &= !Self::PAINT; }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_clean() {
        let d = DirtyPhases::default();
        assert!(!d.needs_update());
    }

    #[test]
    fn invalidate_all_sets_every_flag() {
        let mut d = DirtyPhases::default();
        d.invalidate_all();
        assert!(d.needs_style());
        assert!(d.needs_layout());
        assert!(d.needs_paint());
    }

    #[test]
    fn invalidate_paint_only() {
        let mut d = DirtyPhases::default();
        d.invalidate_paint();
        assert!(!d.needs_style());
        assert!(!d.needs_layout());
        assert!(d.needs_paint());
    }

    #[test]
    fn clear_one_phase() {
        let mut d = DirtyPhases::default();
        d.invalidate_all();
        d.clear_style();
        assert!(!d.needs_style());
        assert!(d.needs_layout());
        assert!(d.needs_paint());
    }

    #[test]
    fn clear_all_leaves_clean() {
        let mut d = DirtyPhases::default();
        d.invalidate_all();
        d.clear_style();
        d.clear_layout();
        d.clear_paint();
        assert!(!d.needs_update());
    }
}
