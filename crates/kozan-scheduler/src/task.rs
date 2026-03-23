//! Task types — Chrome's `base::OnceClosure` + `base::TaskTraits`.
//!
//! A [`Task`] is a unit of work posted to the scheduler.
//! Like Chrome's `PostTask(FROM_HERE, base::BindOnce(&DoWork))`.
//!
//! # Performance
//!
//! - `Task` is a thin wrapper around `Box<dyn FnOnce()>`.
//! - No allocations beyond the initial boxing.
//! - `TaskPriority` is a u8 — fits in a register, branchless comparison.
//!
//! # Chrome mapping
//!
//! | Chrome                     | Kozan                |
//! |----------------------------|----------------------|
//! | `base::OnceClosure`        | `Task.callback`      |
//! | `base::TaskTraits`         | `TaskPriority`       |
//! | `base::Location`           | (not needed in Rust) |
//! | `base::TimeDelta`          | `Task.delay`         |

use core::fmt;
use std::time::{Duration, Instant};

/// Priority levels for tasks — determines scheduling order.
///
/// Ordered from highest to lowest. The scheduler always picks from the
/// highest non-empty priority level (with anti-starvation for lower levels).
///
/// # Chrome mapping
///
/// | Kozan          | Chrome equivalent                    |
/// |----------------|--------------------------------------|
/// | `Input`        | Input task source (highest)          |
/// | `UserBlocking` | `base::TaskPriority::USER_BLOCKING`  |
/// | `Normal`       | `base::TaskPriority::USER_VISIBLE`   |
/// | `Timer`        | Timer task source (throttleable)     |
/// | `BestEffort`   | `base::TaskPriority::BEST_EFFORT`    |
/// | `Idle`         | `requestIdleCallback` task source    |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum TaskPriority {
    /// Input events: mouse, keyboard, touch, pointer.
    /// Always processed first — responsiveness is critical.
    Input = 0,

    /// User is actively waiting for the result.
    /// Example: loading a resource the user just clicked on.
    UserBlocking = 1,

    /// Normal DOM work, network callbacks, general application logic.
    /// The default priority for most tasks.
    Normal = 2,

    /// Timer callbacks (`setTimeout`/`setInterval` equivalent).
    /// Can be throttled for background windows.
    Timer = 3,

    /// Background work the user won't notice if delayed.
    /// Example: prefetching, metrics, analytics.
    BestEffort = 4,

    /// Idle tasks — only run when the frame budget has spare time.
    /// Example: `requestIdleCallback` equivalent, GC-like cleanup.
    Idle = 5,
}

impl TaskPriority {
    /// Total number of priority levels.
    /// Used to size the per-priority queue array.
    pub const COUNT: usize = 6;

    /// Convert to array index (0 = highest priority).
    #[inline]
    #[must_use] 
    pub const fn as_index(self) -> usize {
        self as usize
    }

    /// Convert from array index. Returns `None` if out of range.
    #[inline]
    #[must_use] 
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Input),
            1 => Some(Self::UserBlocking),
            2 => Some(Self::Normal),
            3 => Some(Self::Timer),
            4 => Some(Self::BestEffort),
            5 => Some(Self::Idle),
            _ => None,
        }
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input => write!(f, "Input"),
            Self::UserBlocking => write!(f, "UserBlocking"),
            Self::Normal => write!(f, "Normal"),
            Self::Timer => write!(f, "Timer"),
            Self::BestEffort => write!(f, "BestEffort"),
            Self::Idle => write!(f, "Idle"),
        }
    }
}

impl Default for TaskPriority {
    #[inline]
    fn default() -> Self {
        Self::Normal
    }
}

/// A unit of work to be executed by the scheduler.
///
/// Like Chrome's `base::OnceClosure` wrapped with `base::TaskTraits`.
/// Each task has a priority and an optional delay.
///
/// # Lifecycle
///
/// 1. Created via [`Task::new()`] or [`Task::delayed()`]
/// 2. Posted to [`Scheduler`](crate::Scheduler) via `post_task()`
/// 3. Scheduler puts it in the correct priority queue
/// 4. Event loop picks highest-priority ready task
/// 5. Task executes (callback consumed)
pub struct Task {
    /// The work to execute. Consumed on run.
    callback: Box<dyn FnOnce()>,

    /// Scheduling priority.
    priority: TaskPriority,

    /// Earliest time this task can run.
    /// `None` = ready immediately.
    /// Used for `setTimeout`/`setInterval` equivalent.
    run_at: Option<Instant>,
}

impl Task {
    /// Create a task with the given priority.
    ///
    /// ```ignore
    /// Task::new(TaskPriority::Normal, || {
    ///     println!("hello from task");
    /// });
    /// ```
    #[inline]
    pub fn new(priority: TaskPriority, callback: impl FnOnce() + 'static) -> Self {
        Self {
            callback: Box::new(callback),
            priority,
            run_at: None,
        }
    }

    /// Create a delayed task.
    ///
    /// Like Chrome's `PostDelayedTask()`. The task won't execute until
    /// `delay` has elapsed. Equivalent to `setTimeout(callback, delay)`.
    ///
    /// ```ignore
    /// Task::delayed(TaskPriority::Timer, Duration::from_millis(100), || {
    ///     println!("fires after 100ms");
    /// });
    /// ```
    #[inline]
    pub fn delayed(
        priority: TaskPriority,
        delay: Duration,
        callback: impl FnOnce() + 'static,
    ) -> Self {
        Self {
            callback: Box::new(callback),
            priority,
            run_at: Some(Instant::now() + delay),
        }
    }

    /// The task's priority level.
    #[inline]
    #[must_use] 
    pub fn priority(&self) -> TaskPriority {
        self.priority
    }

    /// Whether this task is ready to execute (delay has elapsed).
    #[inline]
    #[must_use] 
    pub fn is_ready(&self) -> bool {
        match self.run_at {
            None => true,
            Some(at) => Instant::now() >= at,
        }
    }

    /// Time remaining until this task is ready.
    /// Returns `Duration::ZERO` if already ready.
    #[inline]
    #[must_use] 
    pub fn time_until_ready(&self) -> Duration {
        match self.run_at {
            None => Duration::ZERO,
            Some(at) => at.saturating_duration_since(Instant::now()),
        }
    }

    /// The scheduled run time, if delayed.
    #[inline]
    #[must_use] 
    pub fn run_at(&self) -> Option<Instant> {
        self.run_at
    }

    /// Execute this task, consuming the callback.
    #[inline]
    pub fn run(self) {
        (self.callback)();
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("priority", &self.priority)
            .field("run_at", &self.run_at)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn task_executes_callback() {
        let called = Rc::new(Cell::new(false));
        let called2 = called.clone();
        let task = Task::new(TaskPriority::Normal, move || called2.set(true));
        assert!(!called.get());
        task.run();
        assert!(called.get());
    }

    #[test]
    fn task_ready_when_no_delay() {
        let task = Task::new(TaskPriority::Input, || {});
        assert!(task.is_ready());
        assert_eq!(task.time_until_ready(), Duration::ZERO);
        assert!(task.run_at().is_none());
    }

    #[test]
    fn task_not_ready_with_future_delay() {
        let task = Task::delayed(TaskPriority::Timer, Duration::from_secs(60), || {});
        assert!(!task.is_ready());
        assert!(task.time_until_ready() > Duration::ZERO);
        assert!(task.run_at().is_some());
    }

    #[test]
    fn task_ready_with_zero_delay() {
        let task = Task::delayed(TaskPriority::Timer, Duration::ZERO, || {});
        assert!(task.is_ready());
    }

    #[test]
    fn priority_ordering() {
        assert!(TaskPriority::Input < TaskPriority::UserBlocking);
        assert!(TaskPriority::UserBlocking < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::Timer);
        assert!(TaskPriority::Timer < TaskPriority::BestEffort);
        assert!(TaskPriority::BestEffort < TaskPriority::Idle);
    }

    #[test]
    fn priority_index_roundtrip() {
        for i in 0..TaskPriority::COUNT {
            let p = TaskPriority::from_index(i).unwrap();
            assert_eq!(p.as_index(), i);
        }
        assert!(TaskPriority::from_index(TaskPriority::COUNT).is_none());
    }

    #[test]
    fn default_priority_is_normal() {
        assert_eq!(TaskPriority::default(), TaskPriority::Normal);
    }

    #[test]
    fn task_debug_format() {
        let task = Task::new(TaskPriority::Input, || {});
        let debug = format!("{:?}", task);
        assert!(debug.contains("Input"));
    }
}
