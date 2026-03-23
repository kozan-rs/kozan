//! Frame scheduler — Chrome's `CCScheduler` for vsync-driven rendering.
//!
//! Controls WHEN the rendering pipeline runs:
//! Style → Layout → Paint → Composite.
//!
//! # Chrome's frame pipeline
//!
//! ```text
//! Vsync signal (every ~16.6ms at 60 Hz)
//!   → BeginMainFrame
//!     → Run requestAnimationFrame callbacks
//!     → Style recalc
//!     → Layout
//!     → Paint
//!     → Commit to compositor
//! ```
//!
//! # Kozan's approach
//!
//! We don't have a separate compositor thread yet, so the frame scheduler
//! manages frame timing on the window thread directly:
//!
//! 1. **Dirty-based**: Only produce a frame if something changed.
//! 2. **Vsync-aligned**: Don't render faster than the display refresh rate.
//! 3. **Frame callbacks**: `request_frame()` registers a callback for
//!    the next frame (like `requestAnimationFrame`).
//! 4. **Budget tracking**: Measures frame time to detect jank.
//!
//! # Performance
//!
//! - Zero CPU when idle (no dirty flags = no frames).
//! - Frame callbacks stored in a `Vec` (swapped each frame for zero alloc).

use std::time::{Duration, Instant};

/// Default frame budget at 60 Hz.
const DEFAULT_FRAME_BUDGET: Duration = Duration::from_micros(16_667);

/// A frame callback — like `requestAnimationFrame(callback)`.
///
/// Returns `true` to re-register for the next frame (loop),
/// `false` to run once and stop (one-shot).
///
/// This single type handles both patterns:
/// - One-shot: `|_info| false`
/// - Loop:     `|info| { update(); true }`
type FrameCallback = Box<dyn FnMut(FrameInfo) -> bool>;

/// Information passed to frame callbacks.
///
/// Unlike a snapshot, `remaining_budget()` returns a **live** value
/// computed from the current time — callbacks later in the frame
/// see less remaining budget.
#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub timestamp: Duration,
    pub frame_budget: Duration,
    pub frame_start: Instant,
    pub frame_number: u64,
    pub fps: f64,
    /// Previous frame's pipeline timing. Zero on the first frame.
    pub prev_timing: FrameTiming,
}

impl FrameInfo {
    /// Live remaining budget — computed from current time.
    ///
    /// Callbacks later in the frame see less remaining budget.
    /// Returns `Duration::ZERO` if the budget is exceeded.
    #[inline]
    #[must_use] 
    pub fn remaining_budget(&self) -> Duration {
        self.frame_budget.saturating_sub(self.frame_start.elapsed())
    }
}

/// Controls frame timing and rendering pipeline execution.
///
/// Like Chrome's `CCScheduler` — decides when to produce frames
/// based on dirty state and vsync timing.
///
/// # Lifecycle in the event loop
///
/// ```text
/// loop {
///     // ... process tasks, microtasks ...
///
///     if frame_scheduler.should_produce_frame() {
///         let info = frame_scheduler.begin_frame();
///         frame_scheduler.run_callbacks(info);
///         // → style recalc, layout, paint happen here
///         frame_scheduler.end_frame();
///     }
///
///     // ... idle tasks ...
/// }
/// ```
/// FPS measurement window (Chrome uses 500ms).
const FPS_WINDOW: Duration = Duration::from_millis(500);

pub struct FrameScheduler {
    /// When the scheduler was created (for timestamp calculation).
    epoch: Instant,

    /// Target frame duration (e.g., 16.67ms for 60 Hz).
    frame_budget: Duration,

    /// When the last frame started.
    last_frame_start: Option<Instant>,

    /// When the last frame ended.
    last_frame_end: Option<Instant>,

    /// Frame counter.
    frame_number: u64,

    /// Whether new visual changes need rendering.
    needs_frame: bool,

    /// Callbacks registered for the next frame.
    callbacks: Vec<FrameCallback>,

    /// Double-buffer: callbacks being collected for the NEXT frame
    /// while current frame's callbacks are running.
    pending_callbacks: Vec<FrameCallback>,

    /// Whether we're currently inside `begin_frame` / `end_frame`.
    in_frame: bool,

    // ── FPS windowed average (Chrome approach) ──
    fps_frame_count: u32,
    fps_window_start: Instant,
    fps_value: f64,

    /// Previous frame's pipeline timing (set by the platform after lifecycle).
    prev_timing: FrameTiming,
}

pub use kozan_primitives::timing::FrameTiming;

impl FrameScheduler {
    /// Create a new frame scheduler with the default 60 Hz budget.
    #[must_use] 
    pub fn new() -> Self {
        Self::with_budget(DEFAULT_FRAME_BUDGET)
    }

    /// Create a frame scheduler with a custom frame budget.
    ///
    /// ```ignore
    /// // 120 Hz display:
    /// FrameScheduler::with_budget(Duration::from_micros(8_333));
    /// ```
    #[must_use] 
    pub fn with_budget(frame_budget: Duration) -> Self {
        let now = Instant::now();
        Self {
            epoch: now,
            frame_budget,
            last_frame_start: None,
            last_frame_end: None,
            frame_number: 0,
            needs_frame: false,
            callbacks: Vec::new(),
            pending_callbacks: Vec::new(),
            in_frame: false,
            fps_frame_count: 0,
            fps_window_start: now,
            fps_value: 0.0,
            prev_timing: FrameTiming::default(),
        }
    }

    /// Mark that something visual changed — a frame is needed.
    ///
    /// Like Chrome's `SetNeedsCommit()` / `ScheduleAnimate()`.
    /// Call this after DOM mutations, style changes, etc.
    #[inline]
    pub fn set_needs_frame(&mut self) {
        self.needs_frame = true;
    }

    /// Register a callback for the next frame.
    ///
    /// Like `requestAnimationFrame(callback)`. The callback receives
    /// a [`FrameInfo`] with timing information.
    ///
    /// If called during a frame (inside a frame callback), the new
    /// callback runs in the NEXT frame (not the current one).
    pub fn request_frame(&mut self, callback: impl FnMut(FrameInfo) -> bool + 'static) {
        if self.in_frame {
            self.pending_callbacks.push(Box::new(callback));
        } else {
            self.callbacks.push(Box::new(callback));
            self.needs_frame = true;
        }
    }

    /// Whether the scheduler should produce a frame this iteration.
    ///
    /// True when:
    /// 1. Something needs rendering (dirty flag or pending callbacks), AND
    /// 2. Enough time has passed since the last frame (frame budget).
    #[must_use] 
    pub fn should_produce_frame(&self) -> bool {
        if !self.needs_frame && self.callbacks.is_empty() {
            return false;
        }
        match self.last_frame_start {
            None => true,
            Some(last) => last.elapsed() >= self.frame_budget,
        }
    }

    /// Time until the next frame should be produced.
    ///
    /// Returns `Duration::ZERO` if a frame is due now.
    /// Returns `None` if no frame is needed.
    /// Used by the scheduler to set its park timeout.
    #[must_use] 
    pub fn time_until_next_frame(&self) -> Option<Duration> {
        if !self.needs_frame && self.callbacks.is_empty() {
            return None;
        }

        match self.last_frame_start {
            None => Some(Duration::ZERO),
            Some(last) => {
                let elapsed = last.elapsed();
                if elapsed >= self.frame_budget {
                    Some(Duration::ZERO)
                } else {
                    Some(self.frame_budget - elapsed)
                }
            }
        }
    }

    /// Begin a new frame. Returns frame info for callbacks.
    ///
    /// Must be paired with [`end_frame()`](Self::end_frame).
    pub fn begin_frame(&mut self) -> FrameInfo {
        let now = Instant::now();

        // Windowed FPS: count frames over 500ms, compute average.
        // Produces stable numbers like Chrome's performance tools.
        self.fps_frame_count += 1;
        let window_elapsed = now.duration_since(self.fps_window_start);
        if window_elapsed >= FPS_WINDOW {
            self.fps_value = self.fps_frame_count as f64 / window_elapsed.as_secs_f64();
            self.fps_frame_count = 0;
            self.fps_window_start = now;
        }

        self.in_frame = true;
        self.frame_number += 1;
        self.last_frame_start = Some(now);
        self.needs_frame = false;

        FrameInfo {
            timestamp: now.duration_since(self.epoch),
            frame_budget: self.frame_budget,
            frame_start: now,
            frame_number: self.frame_number,
            fps: self.fps_value,
            prev_timing: self.prev_timing,
        }
    }

    /// Run all frame callbacks for the current frame.
    ///
    /// Callbacks that return `true` are kept for the next frame (loop).
    /// Callbacks that return `false` are removed (one-shot).
    /// Returns the number of callbacks executed.
    pub fn run_callbacks(&mut self, info: FrameInfo) -> usize {
        debug_assert!(
            self.in_frame,
            "run_callbacks called outside of begin_frame/end_frame"
        );

        let mut callbacks = std::mem::take(&mut self.callbacks);
        let count = callbacks.len();

        // Call each, keep the ones that return true.
        callbacks.retain_mut(|cb| cb(info));

        // Put survivors back — they run again next frame.
        // Any new callbacks registered during this frame are in pending_callbacks.
        self.callbacks = callbacks;

        count
    }

    /// End the current frame. Swaps callback buffers for next frame.
    pub fn end_frame(&mut self) {
        debug_assert!(self.in_frame, "end_frame called without begin_frame");

        self.last_frame_end = Some(Instant::now());
        self.in_frame = false;

        // Move pending callbacks (registered during this frame) to main buffer.
        self.callbacks.append(&mut self.pending_callbacks);
    }

    /// Duration of the last completed frame.
    ///
    /// Returns `None` if no frame has completed yet.
    /// Use this for jank detection (`frame_time` > budget = dropped frame).
    #[must_use] 
    pub fn last_frame_time(&self) -> Option<Duration> {
        match (self.last_frame_start, self.last_frame_end) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }

    /// Whether the last frame exceeded its budget (jank).
    #[must_use] 
    pub fn last_frame_janked(&self) -> bool {
        self.last_frame_time()
            .is_some_and(|t| t > self.frame_budget)
    }

    /// The current frame number.
    #[inline]
    #[must_use] 
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    /// The target frame budget.
    #[inline]
    #[must_use] 
    pub fn frame_budget(&self) -> Duration {
        self.frame_budget
    }

    #[inline]
    pub fn set_frame_budget(&mut self, budget: Duration) {
        self.frame_budget = budget;
    }

    /// Store the previous frame's pipeline timing.
    /// Called by the platform after `update_lifecycle`. Exposed via `FrameInfo`.
    #[inline]
    pub fn set_frame_timing(&mut self, timing: FrameTiming) {
        self.prev_timing = timing;
    }

    /// Time remaining in the current frame's budget.
    ///
    /// Returns `Duration::ZERO` if not in a frame or budget is exceeded.
    /// Useful for idle tasks: "how much time do I have left?"
    #[must_use] 
    pub fn remaining_budget(&self) -> Duration {
        match self.last_frame_start {
            Some(start) if self.in_frame => {
                let elapsed = start.elapsed();
                self.frame_budget.saturating_sub(elapsed)
            }
            _ => Duration::ZERO,
        }
    }

    /// Number of pending frame callbacks.
    #[inline]
    #[must_use] 
    pub fn pending_callback_count(&self) -> usize {
        self.callbacks.len() + self.pending_callbacks.len()
    }
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn no_frame_when_clean() {
        let scheduler = FrameScheduler::new();
        assert!(!scheduler.should_produce_frame());
        assert!(scheduler.time_until_next_frame().is_none());
    }

    #[test]
    fn frame_needed_after_set_needs_frame() {
        let mut scheduler = FrameScheduler::new();
        scheduler.set_needs_frame();
        assert!(scheduler.should_produce_frame());
    }

    #[test]
    fn frame_needed_after_request_frame() {
        let mut scheduler = FrameScheduler::new();
        scheduler.request_frame(|_| false);
        assert!(scheduler.should_produce_frame());
    }

    #[test]
    fn begin_end_frame_lifecycle() {
        let mut scheduler = FrameScheduler::new();
        scheduler.set_needs_frame();

        let info = scheduler.begin_frame();
        assert_eq!(info.frame_number, 1);
        assert!(info.frame_budget > Duration::ZERO);
        assert!(info.remaining_budget() > Duration::ZERO);

        scheduler.end_frame();
        assert_eq!(scheduler.frame_number(), 1);
        assert!(!scheduler.should_produce_frame()); // needs_frame was cleared
    }

    #[test]
    fn callbacks_executed_during_frame() {
        let mut scheduler = FrameScheduler::new();
        let called = Rc::new(Cell::new(false));

        let c = called.clone();
        scheduler.request_frame(move |_| { c.set(true); false });

        let info = scheduler.begin_frame();
        let count = scheduler.run_callbacks(info);
        scheduler.end_frame();

        assert!(called.get());
        assert_eq!(count, 1);
    }

    #[test]
    fn callback_during_frame_goes_to_next() {
        let mut scheduler = FrameScheduler::new();
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));

        let l = log.clone();
        scheduler.request_frame(move |_| { l.borrow_mut().push("frame1"); false });

        // Frame 1.
        let info = scheduler.begin_frame();

        // Register during frame → goes to next frame.
        let l = log.clone();
        scheduler.request_frame(move |_| { l.borrow_mut().push("frame2"); false });

        scheduler.run_callbacks(info);
        scheduler.end_frame();

        assert_eq!(*log.borrow(), vec!["frame1"]);
        assert_eq!(scheduler.pending_callback_count(), 1);

        // Frame 2 — need to bypass vsync check for testing.
        scheduler.set_needs_frame();
        scheduler.last_frame_start = Some(Instant::now() - Duration::from_millis(20));
        let info = scheduler.begin_frame();
        scheduler.run_callbacks(info);
        scheduler.end_frame();

        assert_eq!(*log.borrow(), vec!["frame1", "frame2"]);
    }

    #[test]
    fn frame_info_timestamp() {
        let mut scheduler = FrameScheduler::new();
        scheduler.set_needs_frame();

        let info = scheduler.begin_frame();
        // Timestamp should be positive (time since epoch).
        assert!(info.timestamp >= Duration::ZERO);
        scheduler.end_frame();
    }

    #[test]
    fn frame_budget_default_60hz() {
        let scheduler = FrameScheduler::new();
        assert_eq!(scheduler.frame_budget(), Duration::from_micros(16_667));
    }

    #[test]
    fn custom_frame_budget() {
        let scheduler = FrameScheduler::with_budget(Duration::from_micros(8_333));
        assert_eq!(scheduler.frame_budget(), Duration::from_micros(8_333));
    }

    #[test]
    fn set_frame_budget() {
        let mut scheduler = FrameScheduler::new();
        scheduler.set_frame_budget(Duration::from_micros(8_333));
        assert_eq!(scheduler.frame_budget(), Duration::from_micros(8_333));
    }

    #[test]
    fn remaining_budget_outside_frame() {
        let scheduler = FrameScheduler::new();
        assert_eq!(scheduler.remaining_budget(), Duration::ZERO);
    }

    #[test]
    fn last_frame_time_none_initially() {
        let scheduler = FrameScheduler::new();
        assert!(scheduler.last_frame_time().is_none());
        assert!(!scheduler.last_frame_janked());
    }

    #[test]
    fn vsync_throttle() {
        let mut scheduler = FrameScheduler::new();
        scheduler.set_needs_frame();

        // First frame — always allowed.
        assert!(scheduler.should_produce_frame());
        scheduler.begin_frame();
        scheduler.end_frame();

        // Immediately after — should be throttled.
        scheduler.set_needs_frame();
        assert!(!scheduler.should_produce_frame());

        // After budget elapsed — allowed again.
        scheduler.last_frame_start = Some(Instant::now() - Duration::from_millis(20));
        assert!(scheduler.should_produce_frame());
    }
}
