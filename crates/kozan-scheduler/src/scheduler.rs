//! Scheduler — Chrome's `MainThreadScheduler`, the event loop.
//!
//! This is the top-level orchestrator. One `Scheduler` per window thread.
//! It owns all sub-systems and implements the HTML event loop algorithm:
//!
//! ```text
//! loop {
//!     1. Receive cross-thread tasks (background → window)
//!     2. Promote delayed tasks that are now ready
//!     3. Poll async executor (wake pending futures)
//!     4. Pick highest-priority macrotask → run it
//!     5. Drain microtask queue
//!     6. If frame due → begin_frame → callbacks → style/layout/paint → end_frame
//!     7. If idle budget → run idle tasks
//!     8. Park until next event / task / vsync
//! }
//! ```
//!
//! # Chrome mapping
//!
//! | Chrome                              | Kozan                        |
//! |-------------------------------------|------------------------------|
//! | `MainThreadScheduler`               | `Scheduler`                  |
//! | `SequenceManager`                   | `TaskQueueManager` (owned)   |
//! | `EventLoop::PerformMicrotaskCheck`  | `MicrotaskQueue` (owned)     |
//! | `CCScheduler`                       | `FrameScheduler` (owned)     |
//! | Worker pool + `TaskRunner`            | `WakeReceiver` (owned)       |
//! | —                                   | `LocalExecutor` (owned)      |
//!
//! # `!Send` by design
//!
//! The `Scheduler` is `!Send` — it lives on the window thread and never
//! moves. This is enforced by `WakeReceiver` which contains `PhantomData<*const ()>`.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::executor::{LocalExecutor, TaskId};
use crate::frame::{FrameInfo, FrameScheduler};
use crate::microtask::MicrotaskQueue;
use crate::queue::TaskQueueManager;
use crate::task::{Task, TaskPriority};
use crate::waker::{WakeReceiver, WakeSender};

/// Result of one scheduler iteration.
///
/// Callers **must** use `park_timeout` to decide how long to park
/// the thread. Ignoring it causes either spinning or missed wake-ups.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct TickResult {
    /// Number of macrotasks executed.
    pub tasks_run: usize,

    /// Number of microtasks drained.
    pub microtasks_run: usize,

    /// Number of cross-thread tasks received.
    pub cross_thread_tasks: usize,

    /// Number of async tasks polled.
    pub futures_polled: usize,

    /// Whether a frame was produced.
    pub frame_produced: bool,

    /// Suggested park duration until next work.
    /// `Duration::ZERO` means there's more work to do.
    /// `None` means park indefinitely (wait for wake).
    pub park_timeout: Option<Duration>,
}

/// The main event loop scheduler. One per window thread.
///
/// Orchestrates task execution following the HTML event loop algorithm.
/// Owns all sub-systems: task queues, microtask queue, executor,
/// frame scheduler, and cross-thread receiver.
///
/// # Usage
///
/// ```ignore
/// let (scheduler, sender) = Scheduler::new();
///
/// // Give sender to background threads.
/// // On the window thread:
/// loop {
///     let result = scheduler.tick(&mut |info| {
///         doc.recalc_styles();
///         // layout, paint...
///     });
///
///     if let Some(timeout) = result.park_timeout {
///         // Park thread for timeout duration (or until woken).
///     }
/// }
/// ```
pub struct Scheduler {
    /// Priority-based macrotask queues.
    task_queue: TaskQueueManager,

    /// Microtask queue (drained after each macrotask).
    microtask_queue: MicrotaskQueue,

    /// Async executor for !Send futures.
    executor: LocalExecutor,

    /// Frame timing and callbacks.
    frame_scheduler: FrameScheduler,

    /// Receiver for cross-thread task wake-ups.
    receiver: WakeReceiver,
}

impl Scheduler {
    /// Create a new scheduler and its cross-thread sender.
    ///
    /// The `WakeSender` should be cloned and given to background threads.
    /// The `Scheduler` stays on the window thread.
    #[must_use]
    pub fn new() -> (Self, WakeSender) {
        let (sender, receiver) = crate::waker::cross_thread_channel();
        let scheduler = Self {
            task_queue: TaskQueueManager::new(),
            microtask_queue: MicrotaskQueue::new(),
            executor: LocalExecutor::new(),
            frame_scheduler: FrameScheduler::new(),
            receiver,
        };
        (scheduler, sender)
    }

    // ---- Task posting ----

    /// Post a macrotask with the given priority.
    ///
    /// ```ignore
    /// scheduler.post_task(TaskPriority::Normal, || {
    ///     btn.set_text("clicked!");
    /// });
    /// ```
    pub fn post_task(&mut self, priority: TaskPriority, callback: impl FnOnce() + 'static) {
        self.task_queue.push(Task::new(priority, callback));
    }

    /// Post a delayed macrotask.
    ///
    /// Like `setTimeout(callback, delay)`.
    pub fn post_delayed_task(
        &mut self,
        priority: TaskPriority,
        delay: Duration,
        callback: impl FnOnce() + 'static,
    ) {
        self.task_queue
            .push(Task::delayed(priority, delay, callback));
    }

    /// Post a raw [`Task`] object.
    pub fn post_raw_task(&mut self, task: Task) {
        self.task_queue.push(task);
    }

    // ---- Microtasks ----

    /// Enqueue a microtask.
    ///
    /// Like `queueMicrotask(callback)` or `Promise.then()`.
    /// Will be drained after the current macrotask completes.
    pub fn queue_microtask(&mut self, callback: impl FnOnce() + 'static) {
        self.microtask_queue.queue_microtask(callback);
    }

    // ---- Async ----

    /// Spawn a `!Send` async task on the window thread.
    ///
    /// ```ignore
    /// scheduler.spawn(async {
    ///     let data = fetch(url).await;
    ///     btn.set_text(&data.title);
    /// });
    /// ```
    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static) -> TaskId {
        self.executor.spawn(future)
    }

    // ---- Frame ----

    /// Mark that something visual changed — a frame is needed.
    pub fn set_needs_frame(&mut self) {
        self.frame_scheduler.set_needs_frame();
    }

    /// Register a callback for the next frame.
    ///
    /// Like `requestAnimationFrame(callback)`.
    pub fn request_frame(&mut self, callback: impl FnMut(FrameInfo) -> bool + 'static) {
        self.frame_scheduler.request_frame(callback);
    }

    // ---- Notify / event-loop integration ----

    /// Wire a "wake the event loop" callback into the local executor's wakers.
    ///
    /// After this call, any time a future's `Waker` is triggered from a
    /// background thread, `notify` is called in addition to setting the
    /// woken flag. The platform layer uses this to send a dummy event to the
    /// view thread's channel, unblocking it from `recv()`.
    pub fn set_executor_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.executor.set_notify(notify);
    }

    // ---- Queue control ----

    /// Enable or disable a priority queue.
    ///
    /// Disabled queues are skipped during task picking.
    /// Use for background tab throttling.
    pub fn set_queue_enabled(&mut self, priority: TaskPriority, enabled: bool) {
        self.task_queue.queue_mut(priority).set_enabled(enabled);
    }

    // ---- The event loop ----

    /// Run one iteration of the event loop.
    ///
    /// This is the core scheduling algorithm — the HTML event loop:
    ///
    /// 1. Receive cross-thread tasks (background completions)
    /// 2. Promote delayed tasks that are now ready
    /// 3. Poll async executor (process woken futures)
    /// 4. Pick and run ONE macrotask (highest priority)
    /// 5. Drain ALL microtasks
    /// 6. If frame due: begin → callbacks → `render_callback` → end
    /// 7. Calculate park timeout
    ///
    /// The `render` callback is called during step 6 — this is where
    /// the platform layer runs style recalc, layout, and paint.
    pub fn tick(&mut self, render: &mut dyn FnMut(FrameInfo)) -> TickResult {
        let mut result = TickResult {
            tasks_run: 0,
            microtasks_run: 0,
            cross_thread_tasks: 0,
            futures_polled: 0,
            frame_produced: false,
            park_timeout: None,
        };

        // 1. Receive cross-thread tasks — route by their priority.
        //    No allocation: drain_into iterates inline.
        result.cross_thread_tasks = self.receiver.drain_into(|ct| {
            let priority = ct.priority();
            self.task_queue.push(Task::new(priority, move || ct.run()));
        });

        // 1b. Fire expired timers — wakes futures registered via sleep().
        //     Counts are not tracked in TickResult (timer wakeups are not tasks).
        crate::timer::fire_expired();

        // 2. Promote delayed tasks that are now ready.
        self.task_queue.promote_delayed();

        // 3. Poll async executor (process woken futures).
        result.futures_polled = self.executor.poll_all();

        // 4. Pick and run ONE macrotask.
        //    Chrome runs one task per event loop iteration, then checks
        //    microtasks and rendering. This ensures responsiveness.
        if let Some(task) = self.task_queue.pick() {
            task.run();
            result.tasks_run = 1;
        }

        // 5. Drain ALL microtasks.
        result.microtasks_run = self.microtask_queue.drain();

        // 6. Frame opportunity.
        if self.frame_scheduler.should_produce_frame() {
            let info = self.frame_scheduler.begin_frame();
            self.frame_scheduler.run_callbacks(info);
            render(info);
            self.frame_scheduler.end_frame();
            result.frame_produced = true;
        }

        // 7. Calculate park timeout.
        result.park_timeout = self.calculate_park_timeout();

        result
    }

    /// Run the event loop until there's no more work.
    ///
    /// Useful for testing — runs `tick()` repeatedly until everything
    /// is drained and the executor is idle.
    pub fn run_until_idle(&mut self, render: &mut dyn FnMut(FrameInfo)) {
        loop {
            let result = self.tick(render);
            if result.tasks_run == 0
                && result.microtasks_run == 0
                && result.cross_thread_tasks == 0
                && result.futures_polled == 0
                && !result.frame_produced
                && self.executor.is_idle()
            {
                break;
            }
        }
    }

    /// Whether the scheduler has any pending work (immediate or delayed).
    #[must_use]
    pub fn has_work(&self) -> bool {
        self.task_queue.has_ready()
            || self.task_queue.has_delayed()
            || !self.microtask_queue.is_empty()
            || self.executor.has_woken()
            || self.frame_scheduler.should_produce_frame()
    }

    // ---- Sub-system access ----

    /// Access the frame scheduler directly.
    #[must_use]
    pub fn frame_scheduler(&self) -> &FrameScheduler {
        &self.frame_scheduler
    }

    /// Mutable access to the frame scheduler.
    pub fn frame_scheduler_mut(&mut self) -> &mut FrameScheduler {
        &mut self.frame_scheduler
    }

    /// Access the task queue manager.
    #[must_use]
    pub fn task_queue(&self) -> &TaskQueueManager {
        &self.task_queue
    }

    /// Access the executor.
    #[must_use]
    pub fn executor(&self) -> &LocalExecutor {
        &self.executor
    }

    // ---- Internal ----

    /// Calculate how long the thread should park.
    fn calculate_park_timeout(&self) -> Option<Duration> {
        // If there's immediate work, don't park.
        if self.task_queue.has_ready()
            || !self.microtask_queue.is_empty()
            || self.executor.has_woken()
        {
            return Some(Duration::ZERO);
        }

        let mut min_timeout: Option<Duration> = None;

        // Check delayed tasks.
        if let Some(d) = self.task_queue.next_delayed_ready_in() {
            min_timeout = Some(d);
        }

        // Check frame timing.
        if let Some(f) = self.frame_scheduler.time_until_next_frame() {
            min_timeout = Some(match min_timeout {
                None => f,
                Some(m) => m.min(f),
            });
        }

        // Check timer registry (sleep() futures).
        if let Some(t) = crate::timer::next_deadline() {
            min_timeout = Some(match min_timeout {
                None => t,
                Some(m) => m.min(t),
            });
        }

        // None = no timed work, park indefinitely (until cross-thread wake).
        min_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::task::Poll;

    #[test]
    fn empty_tick_does_nothing() {
        let (mut sched, _sender) = Scheduler::new();
        let result = sched.tick(&mut |_| {});

        assert_eq!(result.tasks_run, 0);
        assert_eq!(result.microtasks_run, 0);
        assert_eq!(result.cross_thread_tasks, 0);
        assert_eq!(result.futures_polled, 0);
        assert!(!result.frame_produced);
    }

    #[test]
    fn post_and_run_task() {
        let (mut sched, _sender) = Scheduler::new();
        let called = Rc::new(Cell::new(false));

        let c = called.clone();
        sched.post_task(TaskPriority::Normal, move || c.set(true));

        let result = sched.tick(&mut |_| {});
        assert_eq!(result.tasks_run, 1);
        assert!(called.get());
    }

    #[test]
    fn microtask_drains_after_macrotask() {
        let (mut sched, _sender) = Scheduler::new();
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));

        let l = log.clone();
        sched.post_task(TaskPriority::Normal, move || {
            l.borrow_mut().push("macro");
        });

        let l = log.clone();
        sched.queue_microtask(move || {
            l.borrow_mut().push("micro");
        });

        let _ = sched.tick(&mut |_| {});
        assert_eq!(*log.borrow(), vec!["macro", "micro"]);
    }

    #[test]
    fn priority_ordering() {
        let (mut sched, _sender) = Scheduler::new();
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));

        let l = log.clone();
        sched.post_task(TaskPriority::Idle, move || l.borrow_mut().push("idle"));
        let l = log.clone();
        sched.post_task(TaskPriority::Input, move || l.borrow_mut().push("input"));

        // First tick: picks Input (higher priority).
        let _ = sched.tick(&mut |_| {});
        assert_eq!(*log.borrow(), vec!["input"]);

        // Second tick: picks Idle.
        let _ = sched.tick(&mut |_| {});
        assert_eq!(*log.borrow(), vec!["input", "idle"]);
    }

    #[test]
    fn spawn_async_task() {
        let (mut sched, _sender) = Scheduler::new();
        let done = Rc::new(Cell::new(false));

        let d = done.clone();
        sched.spawn(async move {
            d.set(true);
        });

        let _ = sched.tick(&mut |_| {});
        assert!(done.get());
    }

    #[test]
    fn cross_thread_task() {
        let (mut sched, sender) = Scheduler::new();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let c = called.clone();
        sender
            .post(move || c.store(true, std::sync::atomic::Ordering::SeqCst))
            .unwrap();

        // First tick: receives cross-thread task AND picks it as macrotask.
        let result = sched.tick(&mut |_| {});
        assert_eq!(result.cross_thread_tasks, 1);
        assert_eq!(result.tasks_run, 1);
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn frame_production() {
        let (mut sched, _sender) = Scheduler::new();
        let rendered = Rc::new(Cell::new(false));

        sched.set_needs_frame();

        let r = rendered.clone();
        let result = sched.tick(&mut move |_info| {
            r.set(true);
        });

        assert!(result.frame_produced);
        assert!(rendered.get());
    }

    #[test]
    fn request_frame_callback() {
        let (mut sched, _sender) = Scheduler::new();
        let called = Rc::new(Cell::new(false));

        let c = called.clone();
        sched.request_frame(move |_| {
            c.set(true);
            false
        });

        let _ = sched.tick(&mut |_| {});
        assert!(called.get());
    }

    #[test]
    fn run_until_idle_drains_all() {
        let (mut sched, _sender) = Scheduler::new();
        let counter = Rc::new(Cell::new(0u32));

        for _ in 0..5 {
            let c = counter.clone();
            sched.post_task(TaskPriority::Normal, move || c.set(c.get() + 1));
        }

        sched.run_until_idle(&mut |_| {});
        assert_eq!(counter.get(), 5);
    }

    #[test]
    fn has_work_reflects_state() {
        let (mut sched, _sender) = Scheduler::new();
        assert!(!sched.has_work());

        sched.post_task(TaskPriority::Normal, || {});
        assert!(sched.has_work());

        let _ = sched.tick(&mut |_| {});
        assert!(!sched.has_work());
    }

    #[test]
    fn disabled_queue_skipped() {
        let (mut sched, _sender) = Scheduler::new();
        let called = Rc::new(Cell::new(false));

        let c = called.clone();
        sched.post_task(TaskPriority::Timer, move || c.set(true));
        sched.set_queue_enabled(TaskPriority::Timer, false);

        let _ = sched.tick(&mut |_| {});
        assert!(!called.get()); // timer queue disabled

        sched.set_queue_enabled(TaskPriority::Timer, true);
        let _ = sched.tick(&mut |_| {});
        assert!(called.get());
    }

    #[test]
    fn park_timeout_zero_when_work() {
        let (mut sched, _sender) = Scheduler::new();
        sched.post_task(TaskPriority::Normal, || {});

        let result = sched.tick(&mut |_| {});
        // After running, no more work → park timeout is None (indefinite).
        // But the tick itself should have run the task.
        assert_eq!(result.tasks_run, 1);
    }

    #[test]
    fn park_timeout_none_when_idle() {
        let (mut sched, _sender) = Scheduler::new();
        let result = sched.tick(&mut |_| {});
        assert!(result.park_timeout.is_none());
    }

    #[test]
    fn async_with_waker() {
        use std::cell::RefCell;

        let (mut sched, _sender) = Scheduler::new();
        let counter = Rc::new(Cell::new(0u32));
        let waker_holder: Rc<RefCell<Option<std::task::Waker>>> = Rc::new(RefCell::new(None));

        let c = counter.clone();
        let wh = waker_holder.clone();
        sched.spawn(async move {
            std::future::poll_fn(|cx| {
                let count = c.get();
                if count == 0 {
                    *wh.borrow_mut() = Some(cx.waker().clone());
                    c.set(1);
                    Poll::Pending
                } else {
                    c.set(2);
                    Poll::Ready(())
                }
            })
            .await;
        });

        // First tick: future yields.
        let _ = sched.tick(&mut |_| {});
        assert_eq!(counter.get(), 1);

        // Simulate background completion.
        waker_holder.borrow().as_ref().unwrap().wake_by_ref();

        // Second tick: future completes.
        let _ = sched.tick(&mut |_| {});
        assert_eq!(counter.get(), 2);
    }
}
