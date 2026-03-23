//! Modifier key state — tracks which modifier keys are held.
//!
//! Like Chrome's `WebInputEvent::modifiers_` — a u16 bitfield carried
//! on every input event. Includes keyboard modifiers (Shift, Ctrl, Alt, Meta)
//! and mouse button state (which buttons are currently pressed).
//!
//! # Why a bitfield?
//!
//! - 2 bytes vs 8+ bytes for a struct of bools
//! - Matches Chrome's approach (`kShiftKey | kControlKey | ...`)
//! - Cheap to copy, compare, combine with bitwise OR

/// Modifier key and button state, carried on every input event.
///
/// Chrome equivalent: `WebInputEvent::modifiers_` bitfield.
///
/// ```ignore
/// if event.modifiers.ctrl() && event.modifiers.shift() {
///     // Ctrl+Shift is held
/// }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u16);

// Bit positions — keyboard modifiers.
const SHIFT: u16 = 1 << 0;
const CTRL: u16 = 1 << 1;
const ALT: u16 = 1 << 2;
const META: u16 = 1 << 3; // Win key / Cmd key
const CAPS_LOCK: u16 = 1 << 4;
const NUM_LOCK: u16 = 1 << 5;

// Bit positions — mouse button state (which buttons are currently held).
// Chrome tracks these in the same bitfield as keyboard modifiers.
const LEFT_BUTTON: u16 = 1 << 6;
const RIGHT_BUTTON: u16 = 1 << 7;
const MIDDLE_BUTTON: u16 = 1 << 8;

// Bit positions — keyboard event flags.
const IS_AUTO_REPEAT: u16 = 1 << 9;

impl Modifiers {
    /// Empty — no modifiers held.
    pub const EMPTY: Self = Self(0);

    /// Create from raw bits.
    #[inline]
    #[must_use] 
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Get the raw bits.
    #[inline]
    #[must_use] 
    pub const fn bits(self) -> u16 {
        self.0
    }

    // ---- Keyboard modifier queries ----

    /// Shift key is held.
    #[inline]
    #[must_use] 
    pub const fn shift(self) -> bool {
        self.0 & SHIFT != 0
    }

    /// Ctrl key is held (Control on Mac).
    #[inline]
    #[must_use] 
    pub const fn ctrl(self) -> bool {
        self.0 & CTRL != 0
    }

    /// Alt key is held (Option on Mac).
    #[inline]
    #[must_use] 
    pub const fn alt(self) -> bool {
        self.0 & ALT != 0
    }

    /// Meta key is held (Win on Windows, Cmd on Mac).
    #[inline]
    #[must_use] 
    pub const fn meta(self) -> bool {
        self.0 & META != 0
    }

    /// Caps Lock is active.
    #[inline]
    #[must_use] 
    pub const fn caps_lock(self) -> bool {
        self.0 & CAPS_LOCK != 0
    }

    /// Num Lock is active.
    #[inline]
    #[must_use] 
    pub const fn num_lock(self) -> bool {
        self.0 & NUM_LOCK != 0
    }

    // ---- Mouse button state queries ----

    /// Left mouse button is currently held.
    #[inline]
    #[must_use] 
    pub const fn left_button(self) -> bool {
        self.0 & LEFT_BUTTON != 0
    }

    /// Right mouse button is currently held.
    #[inline]
    #[must_use] 
    pub const fn right_button(self) -> bool {
        self.0 & RIGHT_BUTTON != 0
    }

    /// Middle mouse button is currently held.
    #[inline]
    #[must_use] 
    pub const fn middle_button(self) -> bool {
        self.0 & MIDDLE_BUTTON != 0
    }

    // ---- Keyboard event flags ----

    /// This is an auto-repeat key event (key held down).
    #[inline]
    #[must_use] 
    pub const fn is_auto_repeat(self) -> bool {
        self.0 & IS_AUTO_REPEAT != 0
    }

    // ---- Builder methods (set individual bits) ----

    #[inline]
    #[must_use] 
    pub const fn with_shift(self) -> Self {
        Self(self.0 | SHIFT)
    }
    #[inline]
    #[must_use] 
    pub const fn with_ctrl(self) -> Self {
        Self(self.0 | CTRL)
    }
    #[inline]
    #[must_use] 
    pub const fn with_alt(self) -> Self {
        Self(self.0 | ALT)
    }
    #[inline]
    #[must_use] 
    pub const fn with_meta(self) -> Self {
        Self(self.0 | META)
    }
    #[inline]
    #[must_use] 
    pub const fn with_caps_lock(self) -> Self {
        Self(self.0 | CAPS_LOCK)
    }
    #[inline]
    #[must_use] 
    pub const fn with_num_lock(self) -> Self {
        Self(self.0 | NUM_LOCK)
    }
    #[inline]
    #[must_use] 
    pub const fn with_left_button(self) -> Self {
        Self(self.0 | LEFT_BUTTON)
    }
    #[inline]
    #[must_use] 
    pub const fn with_right_button(self) -> Self {
        Self(self.0 | RIGHT_BUTTON)
    }
    #[inline]
    #[must_use] 
    pub const fn with_middle_button(self) -> Self {
        Self(self.0 | MIDDLE_BUTTON)
    }
    #[inline]
    #[must_use] 
    pub const fn with_auto_repeat(self) -> Self {
        Self(self.0 | IS_AUTO_REPEAT)
    }

    /// Combine two modifier sets.
    #[inline]
    #[must_use] 
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if all bits in `other` are set in `self`.
    #[inline]
    #[must_use] 
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::fmt::Display for Modifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl() {
            parts.push("Ctrl");
        }
        if self.shift() {
            parts.push("Shift");
        }
        if self.alt() {
            parts.push("Alt");
        }
        if self.meta() {
            parts.push("Meta");
        }
        if parts.is_empty() {
            write!(f, "None")
        } else {
            write!(f, "{}", parts.join("+"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_modifiers() {
        let m = Modifiers::EMPTY;
        assert!(!m.shift());
        assert!(!m.ctrl());
        assert!(!m.alt());
        assert!(!m.meta());
        assert!(!m.left_button());
        assert_eq!(m.bits(), 0);
    }

    #[test]
    fn builder_methods() {
        let m = Modifiers::EMPTY.with_ctrl().with_shift();
        assert!(m.ctrl());
        assert!(m.shift());
        assert!(!m.alt());
        assert!(!m.meta());
    }

    #[test]
    fn bitor_combines() {
        let a = Modifiers::EMPTY.with_ctrl();
        let b = Modifiers::EMPTY.with_shift();
        let c = a | b;
        assert!(c.ctrl());
        assert!(c.shift());
    }

    #[test]
    fn contains_checks_subset() {
        let m = Modifiers::EMPTY.with_ctrl().with_shift().with_alt();
        let subset = Modifiers::EMPTY.with_ctrl().with_shift();
        assert!(m.contains(subset));
        assert!(!subset.contains(m));
    }

    #[test]
    fn display_format() {
        let m = Modifiers::EMPTY.with_ctrl().with_shift();
        assert_eq!(format!("{}", m), "Ctrl+Shift");
        assert_eq!(format!("{}", Modifiers::EMPTY), "None");
    }

    #[test]
    fn mouse_button_state() {
        let m = Modifiers::EMPTY.with_left_button().with_ctrl();
        assert!(m.left_button());
        assert!(m.ctrl());
        assert!(!m.right_button());
    }

    #[test]
    fn auto_repeat_flag() {
        let m = Modifiers::EMPTY.with_auto_repeat();
        assert!(m.is_auto_repeat());
    }

    #[test]
    fn size_is_two_bytes() {
        assert_eq!(std::mem::size_of::<Modifiers>(), 2);
    }
}
