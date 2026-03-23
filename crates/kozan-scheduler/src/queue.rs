//! Task queues — Chrome's `base::sequence_manager::TaskQueue` + `SequenceManager`.
//!
//! Two separate types, each with a single responsibility:
//!
//! - [`TaskQueue`] — A single FIFO queue. Can be enabled/disabled, throttled.
//!   Like Chrome's `TaskQueue` class. One per priority level.
//!
//! - [`TaskQueueManager`] — Picks from multiple `TaskQueue`s by priority.
//!   Like Chrome's `SequenceManager`. Applies anti-starvation.
//!
//! # Why not just `VecDeque<Task>`?
//!
//! A raw `VecDeque` has no place to add per-queue behavior. By wrapping it
//! in `TaskQueue`, we can add enable/disable, throttling, fencing, and
//! queue-level metrics without changing any caller code. This is Chrome's
//! "one class per concept" principle — the key to extensibility.
//!
//! # Delayed tasks
//!
//! Tasks with a future `run_at` are stored in a `BinaryHeap` (min-heap)
//! sorted by deadline. [`promote_delayed()`](TaskQueueManager::promote_delayed)
//! pops only the tasks that are ready — O(k log n) where k = ready count.
//! This is critical for performance with many timers (setTimeout-equivalent).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::time::Instant;

use crate::task::{Task, TaskPriority};

// ---- TaskQueue (single FIFO) ----

/// A single FIFO task queue with enable/disable support.
///
/// Like Chrome's `base::sequence_manager::TaskQueue`.
/// Each priority level in the scheduler owns one `TaskQueue`.
///
/// # Chrome features mapped
///
/// | Chrome                         | Kozan                  |
/// |--------------------------------|------------------------|
/// | `TaskQueue::SetQueueEnabled()` | `set_enabled()`        |
/// | `TaskQueue::InsertFence()`     | (future: `set_fence()` |
/// | `TaskQueue::GetNumberOfPending`| `len()`                |
/// | `PushBack` / `TakeTask`        | `push()` / `pop()`     |
pub struct TaskQueue {
    /// The underlying FIFO buffer.
    tasks: VecDeque<Task>,

    /// Whether this queue is enabled. Disabled queues are skipped by the picker.
    /// Chrome uses this for throttling background tabs — disable timer queue.
    enabled: bool,

    /// The priority this queue serves (for diagnostics and debugging).
    priority: TaskPriority,
}

impl TaskQueue {
    /// Create a new empty queue for the given priority.
    #[inline]
    #[must_use] 
    pub fn new(priority: TaskPriority) -> Self {
        Self {
            tasks: VecDeque::new(),
            enabled: true,
            priority,
        }
    }

    /// Push a task to the back of the queue.
    #[inline]
    pub fn push(&mut self, task: Task) {
        self.tasks.push_back(task);
    }

    /// Pop the front task. Returns `None` if empty or disabled.
    #[inline]
    pub fn pop(&mut self) -> Option<Task> {
        if !self.enabled {
            return None;
        }
        self.tasks.pop_front()
    }

    /// Peek at the front task without removing it.
    #[inline]
    #[must_use] 
    pub fn front(&self) -> Option<&Task> {
        if !self.enabled {
            return None;
        }
        self.tasks.front()
    }

    /// Number of tasks in this queue (regardless of enabled state).
    #[inline]
    #[must_use] 
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Whether this queue has no tasks.
    #[inline]
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Whether this queue has tasks AND is enabled (actually pickable).
    #[inline]
    #[must_use] 
    pub fn has_ready(&self) -> bool {
        self.enabled && !self.tasks.is_empty()
    }

    /// Whether this queue is enabled.
    #[inline]
    #[must_use] 
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable this queue.
    ///
    /// Disabled queues are skipped by [`TaskQueueManager::pick()`].
    /// Chrome uses this to throttle background tab timers.
    #[inline]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// The priority level this queue serves.
    #[inline]
    #[must_use] 
    pub fn priority(&self) -> TaskPriority {
        self.priority
    }
}

// ---- DelayedEntry (for BinaryHeap min-heap) ----

/// Wrapper for delayed tasks in the min-heap.
/// Ordered by `run_at` (earliest deadline first).
struct DelayedEntry {
    task: Task,
    /// Cached `run_at` for heap ordering. Avoids calling `task.run_at()`
    /// repeatedly during heap operations.
    deadline: Instant,
}

impl PartialEq for DelayedEntry {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for DelayedEntry {}

impl PartialOrd for DelayedEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DelayedEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse: BinaryHeap is max-heap, we want min-heap (earliest first).
        other.deadline.cmp(&self.deadline)
    }
}

// ---- TaskQueueManager (priority picker) ----

/// Number of consecutive high-priority picks before forcing a lower-priority pick.
///
/// Prevents starvation of low-priority tasks when high-priority tasks
/// are continuously posted. After this many consecutive picks from
/// Input/UserBlocking queues, one task from the lowest non-empty queue
/// is guaranteed to run.
const STARVATION_THRESHOLD: u32 = 64;

/// Manages multiple [`TaskQueue`]s and picks by priority.
///
/// Like Chrome's `base::sequence_manager::SequenceManager` — maintains one
/// `TaskQueue` per priority level and picks from the highest non-empty
/// enabled queue each iteration.
///
/// # Delayed tasks
///
/// Tasks with a future `run_at` go into a `BinaryHeap` (min-heap sorted
/// by deadline). [`promote_delayed()`](Self::promote_delayed) pops only
/// ready tasks — O(k log n) where k = newly ready tasks. With 1000 timers
/// and 0 ready, promotion is O(1) (peek at heap top).
///
/// # Anti-starvation
///
/// After `STARVATION_THRESHOLD` consecutive picks from high-priority
/// queues (Input, `UserBlocking`), forces one pick from the lowest non-empty
/// queue. Counter resets whether the forced pick succeeds or not.
pub struct TaskQueueManager {
    /// One queue per priority level.
    /// Index 0 = Input (highest), Index 5 = Idle (lowest).
    queues: [TaskQueue; TaskPriority::COUNT],

    /// Delayed tasks in a min-heap sorted by deadline (earliest first).
    /// O(log n) insert, O(1) peek, O(k log n) promote k ready tasks.
    delayed: BinaryHeap<DelayedEntry>,

    /// Counter for anti-starvation.
    consecutive_high: u32,
}

impl TaskQueueManager {
    /// Create a new set with one empty queue per priority level.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            queues: [
                TaskQueue::new(TaskPriority::Input),
                TaskQueue::new(TaskPriority::UserBlocking),
                TaskQueue::new(TaskPriority::Normal),
                TaskQueue::new(TaskPriority::Timer),
                TaskQueue::new(TaskPriority::BestEffort),
                TaskQueue::new(TaskPriority::Idle),
            ],
            delayed: BinaryHeap::new(),
            consecutive_high: 0,
        }
    }

    /// Post a task to the appropriate priority queue.
    ///
    /// If the task has a future `run_at`, it goes to the delayed min-heap.
    /// Otherwise it goes directly into the priority queue.
    pub fn push(&mut self, task: Task) {
        if let Some(deadline) = task.run_at() {
            if task.is_ready() {
                // Deadline already passed — go straight to queue.
                self.queues[task.priority().as_index()].push(task);
            } else {
                self.delayed.push(DelayedEntry { task, deadline });
            }
        } else {
            self.queues[task.priority().as_index()].push(task);
        }
    }

    /// Pick the highest-priority ready task.
    ///
    /// Returns `None` if all queues are empty or disabled.
    /// Applies anti-starvation after consecutive high-priority picks.
    pub fn pick(&mut self) -> Option<Task> {
        // Anti-starvation: force a lower-priority pick if threshold exceeded.
        if self.consecutive_high >= STARVATION_THRESHOLD {
            // Reset counter regardless of whether forced pick succeeds.
            // This prevents infinite re-checking when no low-priority tasks exist.
            self.consecutive_high = 0;

            if let Some(task) = self.pick_lowest_nonempty() {
                return Some(task);
            }
        }

        // Normal path: highest-priority non-empty enabled queue.
        for idx in 0..TaskPriority::COUNT {
            if let Some(task) = self.queues[idx].pop() {
                if idx <= TaskPriority::UserBlocking.as_index() {
                    self.consecutive_high += 1;
                } else {
                    self.consecutive_high = 0;
                }
                return Some(task);
            }
        }

        None
    }

    /// Move delayed tasks that are now ready into their priority queues.
    ///
    /// Uses a min-heap: peeks at the earliest deadline, pops if ready.
    /// O(k log n) where k = tasks becoming ready. When no tasks are ready
    /// (the common case), this is O(1) — just one peek.
    pub fn promote_delayed(&mut self) {
        let now = Instant::now();
        while let Some(entry) = self.delayed.peek() {
            if entry.deadline > now {
                break; // Earliest deadline is in the future — nothing more to promote.
            }
            let entry = self.delayed.pop().unwrap();
            self.queues[entry.task.priority().as_index()].push(entry.task);
        }
    }

    /// Time until the next delayed task is ready.
    ///
    /// Returns `None` if there are no delayed tasks.
    /// O(1) — just peeks at the heap top.
    #[must_use] 
    pub fn next_delayed_ready_in(&self) -> Option<std::time::Duration> {
        self.delayed
            .peek()
            .map(|entry| entry.deadline.saturating_duration_since(Instant::now()))
    }

    /// Get a reference to a specific priority queue.
    #[inline]
    #[must_use] 
    pub fn queue(&self, priority: TaskPriority) -> &TaskQueue {
        &self.queues[priority.as_index()]
    }

    /// Get a mutable reference to a specific priority queue.
    ///
    /// Use for per-queue operations like enable/disable:
    /// ```ignore
    /// manager.queue_mut(TaskPriority::Timer).set_enabled(false);
    /// ```
    #[inline]
    pub fn queue_mut(&mut self, priority: TaskPriority) -> &mut TaskQueue {
        &mut self.queues[priority.as_index()]
    }

    /// Total tasks across all queues that are actually pickable
    /// (in enabled queues).
    #[must_use] 
    pub fn ready_count(&self) -> usize {
        self.queues
            .iter()
            .filter(|q| q.is_enabled())
            .map(|q| q.len())
            .sum()
    }

    /// Number of delayed tasks waiting for their deadline.
    #[inline]
    #[must_use] 
    pub fn delayed_count(&self) -> usize {
        self.delayed.len()
    }

    /// Whether there are any pickable tasks in enabled queues.
    #[must_use] 
    pub fn has_ready(&self) -> bool {
        self.queues.iter().any(|q| q.has_ready())
    }

    /// Whether there are no tasks at all (ready or delayed).
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        !self.has_ready() && self.delayed.is_empty()
    }

    /// Whether there are delayed tasks pending (for park timeout calculation).
    #[inline]
    #[must_use] 
    pub fn has_delayed(&self) -> bool {
        !self.delayed.is_empty()
    }

    /// Pick from the lowest non-empty enabled queue (for anti-starvation).
    fn pick_lowest_nonempty(&mut self) -> Option<Task> {
        for idx in (0..TaskPriority::COUNT).rev() {
            if let Some(task) = self.queues[idx].pop() {
                return Some(task);
            }
        }
        None
    }
}

impl Default for TaskQueueManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    // ---- TaskQueue tests ----

    #[test]
    fn queue_fifo_order() {
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut q = TaskQueue::new(TaskPriority::Normal);

        for i in 0..3 {
            let l = log.clone();
            q.push(Task::new(TaskPriority::Normal, move || {
                l.borrow_mut().push(i)
            }));
        }

        assert_eq!(q.len(), 3);
        while let Some(task) = q.pop() {
            task.run();
        }
        assert_eq!(*log.borrow(), vec![0, 1, 2]);
    }

    #[test]
    fn queue_disabled_returns_none() {
        let mut q = TaskQueue::new(TaskPriority::Normal);
        q.push(Task::new(TaskPriority::Normal, || {}));

        q.set_enabled(false);
        assert!(!q.is_enabled());
        assert!(q.pop().is_none());
        assert!(!q.has_ready());
        assert_eq!(q.len(), 1); // still has the task

        q.set_enabled(true);
        assert!(q.pop().is_some());
    }

    #[test]
    fn queue_front_peek() {
        let mut q = TaskQueue::new(TaskPriority::Input);
        assert!(q.front().is_none());

        q.push(Task::new(TaskPriority::Input, || {}));
        assert!(q.front().is_some());
        assert_eq!(q.len(), 1); // peek didn't consume
    }

    #[test]
    fn queue_front_none_when_disabled() {
        let mut q = TaskQueue::new(TaskPriority::Normal);
        q.push(Task::new(TaskPriority::Normal, || {}));
        q.set_enabled(false);
        assert!(q.front().is_none());
    }

    // ---- TaskQueueManager tests ----

    #[test]
    fn set_empty_returns_none() {
        let mut mgr = TaskQueueManager::new();
        assert!(mgr.pick().is_none());
        assert!(mgr.is_empty());
    }

    #[test]
    fn set_higher_priority_first() {
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut mgr = TaskQueueManager::new();

        let l = log.clone();
        mgr.push(Task::new(TaskPriority::Idle, move || {
            l.borrow_mut().push("idle")
        }));
        let l = log.clone();
        mgr.push(Task::new(TaskPriority::Input, move || {
            l.borrow_mut().push("input")
        }));
        let l = log.clone();
        mgr.push(Task::new(TaskPriority::Normal, move || {
            l.borrow_mut().push("normal")
        }));

        while let Some(task) = mgr.pick() {
            task.run();
        }
        assert_eq!(*log.borrow(), vec!["input", "normal", "idle"]);
    }

    #[test]
    fn set_fifo_within_priority() {
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut mgr = TaskQueueManager::new();

        for i in 0..3 {
            let l = log.clone();
            mgr.push(Task::new(TaskPriority::Normal, move || {
                l.borrow_mut().push(i)
            }));
        }

        while let Some(task) = mgr.pick() {
            task.run();
        }
        assert_eq!(*log.borrow(), vec![0, 1, 2]);
    }

    #[test]
    fn set_disabled_queue_skipped() {
        let mut mgr = TaskQueueManager::new();
        mgr.push(Task::new(TaskPriority::Normal, || {}));

        mgr.queue_mut(TaskPriority::Normal).set_enabled(false);
        assert!(mgr.pick().is_none());

        mgr.queue_mut(TaskPriority::Normal).set_enabled(true);
        assert!(mgr.pick().is_some());
    }

    #[test]
    fn set_anti_starvation() {
        let log = Rc::new(std::cell::RefCell::new(Vec::<&str>::new()));
        let mut mgr = TaskQueueManager::new();

        for _ in 0..STARVATION_THRESHOLD + 1 {
            let l = log.clone();
            mgr.push(Task::new(TaskPriority::Input, move || {
                l.borrow_mut().push("input")
            }));
        }

        let l = log.clone();
        mgr.push(Task::new(TaskPriority::Idle, move || {
            l.borrow_mut().push("idle")
        }));

        for _ in 0..STARVATION_THRESHOLD + 1 {
            mgr.pick().unwrap().run();
        }

        let entries = log.borrow();
        assert_eq!(entries[STARVATION_THRESHOLD as usize], "idle");
    }

    #[test]
    fn anti_starvation_resets_when_no_low_priority() {
        // When threshold is reached but no low-priority tasks exist,
        // the counter must reset to avoid re-checking every pick.
        let mut mgr = TaskQueueManager::new();

        // Only high-priority tasks — no low-priority to force-pick.
        for _ in 0..STARVATION_THRESHOLD * 3 {
            mgr.push(Task::new(TaskPriority::Input, || {}));
        }

        // Should not hang or degrade — counter resets on failed forced pick.
        for _ in 0..STARVATION_THRESHOLD * 3 {
            assert!(mgr.pick().is_some());
        }
    }

    #[test]
    fn delayed_task_not_immediately_ready() {
        let mut mgr = TaskQueueManager::new();
        mgr.push(Task::delayed(
            TaskPriority::Timer,
            Duration::from_secs(60),
            || {},
        ));

        assert_eq!(mgr.ready_count(), 0);
        assert_eq!(mgr.delayed_count(), 1);
        assert!(mgr.pick().is_none());
        assert!(mgr.has_delayed());
    }

    #[test]
    fn delayed_task_with_zero_delay_goes_to_queue() {
        let counter = Rc::new(Cell::new(0u32));
        let mut mgr = TaskQueueManager::new();

        let c = counter.clone();
        mgr.push(Task::delayed(
            TaskPriority::Timer,
            Duration::ZERO,
            move || c.set(1),
        ));

        // Zero delay → should go directly to queue (is_ready() = true).
        assert_eq!(mgr.ready_count(), 1);
        mgr.pick().unwrap().run();
        assert_eq!(counter.get(), 1);
    }

    #[test]
    fn promote_delayed_uses_heap() {
        let mut mgr = TaskQueueManager::new();

        // Add tasks with different delays.
        mgr.push(Task::delayed(
            TaskPriority::Timer,
            Duration::from_secs(60),
            || {},
        ));
        mgr.push(Task::delayed(
            TaskPriority::Timer,
            Duration::from_secs(30),
            || {},
        ));
        mgr.push(Task::delayed(
            TaskPriority::Timer,
            Duration::from_secs(90),
            || {},
        ));

        assert_eq!(mgr.delayed_count(), 3);

        // Nothing ready yet.
        mgr.promote_delayed();
        assert_eq!(mgr.ready_count(), 0);

        // next_delayed_ready_in should be ~30s (the earliest).
        let next = mgr.next_delayed_ready_in().unwrap();
        assert!(next <= Duration::from_secs(31));
        assert!(next >= Duration::from_secs(28));
    }

    #[test]
    fn next_delayed_ready_in_none_when_empty() {
        let mgr = TaskQueueManager::new();
        assert!(mgr.next_delayed_ready_in().is_none());
    }

    #[test]
    fn set_queue_access() {
        let mgr = TaskQueueManager::new();
        assert_eq!(
            mgr.queue(TaskPriority::Input).priority(),
            TaskPriority::Input
        );
        assert_eq!(mgr.queue(TaskPriority::Idle).priority(), TaskPriority::Idle);
    }

    #[test]
    fn ready_count_excludes_disabled() {
        let mut mgr = TaskQueueManager::new();
        mgr.push(Task::new(TaskPriority::Input, || {}));
        mgr.push(Task::new(TaskPriority::Normal, || {}));

        assert_eq!(mgr.ready_count(), 2);

        mgr.queue_mut(TaskPriority::Input).set_enabled(false);
        assert_eq!(mgr.ready_count(), 1); // Input disabled, only Normal counts
    }

    #[test]
    fn ready_count_excludes_delayed() {
        let mut mgr = TaskQueueManager::new();
        mgr.push(Task::new(TaskPriority::Input, || {}));
        mgr.push(Task::delayed(
            TaskPriority::Timer,
            Duration::from_secs(60),
            || {},
        ));

        assert_eq!(mgr.ready_count(), 1);
        assert!(mgr.has_ready());
    }
}
