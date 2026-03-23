//! View-thread async timers — sleep, interval, timeout.
//!
//! All three integrate with the [`Scheduler`](kozan_scheduler::Scheduler)'s
//! timer registry.  Zero threads spawned, zero extra allocations beyond the
//! sorted registry entry.
//!
//! # How the registry drives everything
//!
//! ```text
//! Sleep/Interval/Timeout::poll()     Scheduler::tick()
//! ──────────────────────────────     ──────────────────────────────────────
//! register(deadline, waker) ───────► fire_expired()
//!                                        deadline ≤ now?
//!                                          yes → waker.wake()
//!                                               → executor marks task woken
//!                                               → next poll → Poll::Ready
//!
//!                                    calculate_park_timeout()
//!                                      includes next timer deadline
//!                                      → thread parks exactly until expiry
//! ```

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

// ── sleep ─────────────────────────────────────────────────────────────────────

/// A future that completes after a given duration.
///
/// Created by [`sleep()`].
pub struct Sleep {
    deadline: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if Instant::now() >= self.deadline {
            return Poll::Ready(());
        }
        kozan_scheduler::timer::register(self.deadline, cx.waker().clone());
        Poll::Pending
    }
}

/// Suspend the current task for `duration` without blocking the view thread.
///
/// ```ignore
/// ctx.spawn(async move {
///     sleep(Duration::from_millis(500)).await;
///     card.set_style(activated_style());
/// });
/// ```
#[must_use] 
pub fn sleep(duration: Duration) -> Sleep {
    Sleep { deadline: Instant::now() + duration }
}

// ── interval ──────────────────────────────────────────────────────────────────

/// A periodic timer. Each call to [`tick()`](Interval::tick) returns a future
/// that resolves at the next scheduled instant.
///
/// Deadlines are **fixed-period** (next = prev + period), not floating
/// (next = now + period), so accumulated drift is O(1) not O(n).
///
/// Created by [`interval()`].
///
/// ```ignore
/// ctx.spawn(async move {
///     let mut frame_timer = interval(Duration::from_millis(16));
///     loop {
///         frame_timer.tick().await;
///         update_animation();
///     }
/// });
/// ```
pub struct Interval {
    period: Duration,
    next_deadline: Instant,
}

impl Interval {
    /// Wait until the next tick.
    ///
    /// Returns a [`Sleep`] future targeting the pre-computed deadline,
    /// then advances the deadline by one period.
    pub fn tick(&mut self) -> Sleep {
        let deadline = self.next_deadline;
        self.next_deadline += self.period;
        Sleep { deadline }
    }
}

/// Create a periodic timer that fires every `period`.
///
/// The first tick fires after one full `period` (not immediately).
#[must_use] 
pub fn interval(period: Duration) -> Interval {
    Interval {
        period,
        next_deadline: Instant::now() + period,
    }
}

// ── timeout ───────────────────────────────────────────────────────────────────

/// Error returned when a [`timeout`] expires before the wrapped future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed;

impl std::fmt::Display for Elapsed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for Elapsed {}

/// Wraps a future and cancels it if it does not complete within `duration`.
///
/// Created by [`timeout()`]. Returns `Ok(value)` if the future wins the race,
/// or `Err(Elapsed)` if the timer fires first.
///
/// # Implementation
///
/// Both the inner future and the deadline share the same `Waker`.  Whichever
/// completes first causes `poll` to be called again, where the winner is
/// detected and returned.
///
/// ```ignore
/// ctx.spawn(async move {
///     match timeout(Duration::from_secs(5), fetch_data()).await {
///         Ok(data) => display(data),
///         Err(Elapsed) => show_error("Request timed out"),
///     }
/// });
/// ```
pub struct Timeout<F: Future> {
    /// The inner future (structurally pinned).
    future: F,
    deadline: Instant,
}

impl<F: Future> Future for Timeout<F> {
    type Output = Result<F::Output, Elapsed>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Poll the inner future first (priority: completion over timeout).
        //
        // SAFETY: `Timeout` is only used through `Pin` once placed in the
        // executor's task.  We project the pin to the `future` field, which
        // is structurally pinned — we never move it out of `Timeout`.
        let future = unsafe { self.as_mut().map_unchecked_mut(|t| &mut t.future) };
        if let Poll::Ready(val) = future.poll(cx) {
            return Poll::Ready(Ok(val));
        }

        let deadline = self.deadline;
        if Instant::now() >= deadline {
            return Poll::Ready(Err(Elapsed));
        }

        // Register the deadline.  When it fires, the shared waker wakes this
        // task, we re-poll, and the `Instant::now() >= deadline` check wins.
        kozan_scheduler::timer::register(deadline, cx.waker().clone());
        Poll::Pending
    }
}

/// Run `future`, but give up and return [`Elapsed`] after `duration`.
///
/// ```ignore
/// let result = timeout(Duration::from_secs(3), expensive_future()).await;
/// ```
pub fn timeout<F: Future>(duration: Duration, future: F) -> Timeout<F> {
    Timeout {
        future,
        deadline: Instant::now() + duration,
    }
}
