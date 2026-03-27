//! Timer registry — drives `async` sleep / deadline futures on the view thread.
//!
//! # Design
//!
//! ```text
//! Sleep::poll()                 Scheduler::tick()
//! ──────────────────────────    ──────────────────────────────────
//! register(deadline, waker) ──► fire_expired()
//!                                  deadline ≤ now?  →  waker.wake()
//!                                  executor marks task woken
//!                                  next tick polls Sleep → Ready
//! ```
//!
//! The registry is a **thread-local** — no locking needed because both
//! the scheduler and the futures run on the same view thread.
//!
//! Timers are kept in a `Vec` sorted ascending by deadline (earliest first),
//! giving O(log n) insert and O(k) fire where k = expired count.
//! For UI apps O(10) concurrent timers this is perfect.
//!
//! # Why not `post_delayed_task`?
//!
//! Delayed tasks are macrotasks — they run one-at-a-time inside the HTML
//! event loop and count against the frame budget.  Timer wakeups should be
//! lightweight signals that tell the executor to re-poll a future, not full
//! macrotasks.  Using `waker.wake()` directly keeps the mechanism minimal.

use std::cell::RefCell;
use std::task::Waker;
use std::time::{Duration, Instant};

// ── Registry ──────────────────────────────────────────────────────────────────

struct TimerEntry {
    deadline: Instant,
    waker: Waker,
}

struct TimerRegistry {
    /// Sorted ascending by deadline (earliest first).
    timers: Vec<TimerEntry>,
}

impl TimerRegistry {
    const fn new() -> Self {
        Self { timers: Vec::new() }
    }

    /// Register a waker to be called when `deadline` is reached.
    ///
    /// Keeps the list sorted so `fire_expired` and `next_deadline` are fast.
    fn register(&mut self, deadline: Instant, waker: Waker) {
        let pos = self.timers.partition_point(|e| e.deadline <= deadline);
        self.timers.insert(pos, TimerEntry { deadline, waker });
    }

    /// Wake all entries whose deadline has passed. Returns count fired.
    fn fire_expired(&mut self) -> usize {
        let now = Instant::now();
        // All entries with deadline ≤ now are at the front (sorted ascending).
        let split = self.timers.partition_point(|e| e.deadline <= now);
        if split == 0 {
            return 0;
        }
        let fired: Vec<_> = self.timers.drain(..split).collect();
        let count = fired.len();
        for entry in fired {
            entry.waker.wake();
        }
        count
    }

    /// Time until the nearest deadline, for `calculate_park_timeout`.
    fn next_deadline(&self) -> Option<Duration> {
        self.timers.first().map(|e| {
            let now = Instant::now();
            if e.deadline <= now {
                Duration::ZERO
            } else {
                e.deadline - now
            }
        })
    }
}

thread_local! {
    static REGISTRY: RefCell<TimerRegistry> = const {
        RefCell::new(TimerRegistry::new())
    };
}

// ── Public surface (called by Sleep::poll and Scheduler) ──────────────────────

/// Register a timer.
///
/// Called by `Sleep::poll` (from `kozan_platform::time`) on the view thread.
/// The scheduler fires the waker when `deadline` is reached.
///
/// Duplicate registrations (from repeated polls before the deadline) are
/// harmless — each produces one extra `waker.wake()` call which the
/// executor handles gracefully.
pub fn register(deadline: Instant, waker: Waker) {
    REGISTRY.with(|r| r.borrow_mut().register(deadline, waker));
}

/// Fire all expired timers. Called by `Scheduler::tick()`.
pub(crate) fn fire_expired() -> usize {
    REGISTRY.with(|r| r.borrow_mut().fire_expired())
}

/// Time until the next timer fires. Used by `calculate_park_timeout()`.
pub(crate) fn next_deadline() -> Option<Duration> {
    REGISTRY.with(|r| r.borrow().next_deadline())
}
