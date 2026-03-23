//! Wheel (scroll) event types.
//!
//! Like Chrome's `WebMouseWheelEvent` — a dedicated struct for scroll input.
//! Distinguishes between pixel-precise scrolling (trackpad) and line-based
//! scrolling (mouse wheel) via [`WheelDelta`].
//!
//! Chrome equivalent: `third_party/blink/public/common/input/web_mouse_wheel_event.h`

use std::time::Instant;

use super::modifiers::Modifiers;

/// Mouse wheel or trackpad scroll event.
///
/// Chrome equivalent: `WebMouseWheelEvent`.
///
/// The cursor position is included because scroll events should be
/// dispatched to the element under the cursor (like Chrome's scroll targeting).
#[derive(Debug, Clone, Copy)]
pub struct WheelEvent {
    /// Cursor X position in physical pixels, relative to view origin.
    pub x: f64,
    /// Cursor Y position in physical pixels, relative to view origin.
    pub y: f64,
    /// Scroll amount and type.
    pub delta: WheelDelta,
    /// Modifier keys and mouse button state at the time of this event.
    pub modifiers: Modifiers,
    /// When this event was received from the OS.
    pub timestamp: Instant,
}

/// Scroll delta — distinguishes scroll source.
///
/// Chrome equivalent: `WebMouseWheelEvent::delta_x/y` +
/// `has_precise_scrolling_deltas` flag.
///
/// Mouse wheels produce line deltas (discrete ticks).
/// Trackpads produce pixel deltas (smooth, precise).
/// The distinction matters for scroll behavior — line deltas get multiplied
/// by a line height, pixel deltas are used directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelDelta {
    /// Mouse wheel: discrete line ticks. Positive = scroll up/left.
    /// Chrome: `delta_x/y` when `has_precise_scrolling_deltas` is false.
    Lines(f32, f32),

    /// Trackpad: precise pixel offset. Positive = scroll up/left.
    /// Chrome: `delta_x/y` when `has_precise_scrolling_deltas` is true.
    Pixels(f64, f64),
}

/// Chrome: `kPixelsPerLineStep = 40`. Blink default for mouse wheel.
const PIXELS_PER_LINE: f64 = 40.0;

impl WheelDelta {
    /// Raw horizontal component (lines or pixels, unscaled).
    #[must_use]
    pub fn dx(&self) -> f64 {
        match self {
            WheelDelta::Lines(x, _) => *x as f64,
            WheelDelta::Pixels(x, _) => *x,
        }
    }

    /// Raw vertical component (lines or pixels, unscaled).
    #[must_use]
    pub fn dy(&self) -> f64 {
        match self {
            WheelDelta::Lines(_, y) => *y as f64,
            WheelDelta::Pixels(_, y) => *y,
        }
    }

    /// Horizontal delta in CSS pixels. Lines are scaled by `PIXELS_PER_LINE`.
    #[must_use]
    pub fn px_dx(&self) -> f32 {
        match self {
            WheelDelta::Lines(x, _) => (*x as f64 * PIXELS_PER_LINE) as f32,
            WheelDelta::Pixels(x, _) => *x as f32,
        }
    }

    /// Vertical delta in CSS pixels. Lines are scaled by `PIXELS_PER_LINE`.
    #[must_use]
    pub fn px_dy(&self) -> f32 {
        match self {
            WheelDelta::Lines(_, y) => (*y as f64 * PIXELS_PER_LINE) as f32,
            WheelDelta::Pixels(_, y) => *y as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_delta_lines() {
        let delta = WheelDelta::Lines(0.0, -3.0);
        assert_eq!(delta.dx(), 0.0);
        assert_eq!(delta.dy(), -3.0);
    }

    #[test]
    fn wheel_delta_pixels() {
        let delta = WheelDelta::Pixels(10.5, -20.0);
        assert_eq!(delta.dx(), 10.5);
        assert_eq!(delta.dy(), -20.0);
    }

    #[test]
    fn wheel_event_carries_position_and_modifiers() {
        let evt = WheelEvent {
            x: 300.0,
            y: 400.0,
            delta: WheelDelta::Lines(0.0, -1.0),
            modifiers: Modifiers::EMPTY.with_shift(),
            timestamp: Instant::now(),
        };
        assert!(evt.modifiers.shift());
        assert_eq!(evt.x, 300.0);
    }
}
