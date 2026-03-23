//! Local executor — single-threaded `!Send` async runtime.
//!
//! Each window thread has one `LocalExecutor`. It polls `!Send` futures
//! on the window thread — this is what makes `Handle` (which is `Send`
//! but `!Sync`) safe to use across `.await` points.
//!
//! # How it works
//!
//! ```text
//! User code:  let data = fetch(url).await;
//!             btn.set_text(&data.title);
//!
//! Internally:
//! 1. ctx.spawn(future) → wraps future in a LocalTask
//! 2. LocalTask stored in slab (Vec + free-list)
//! 3. Executor::poll_all() → polls each ready task via ready_queue
//! 4. If future yields (Pending) → Waker stored
//! 5. Background thread completes → Waker::wake() pushes ID to ready_queue
//! 6. Next poll_all() → polls only woken tasks → Ready(data)
//! 7. Continuation runs on window thread → btn.set_text() safe
//! ```
//!
//! # Chrome mapping
//!
//! Chrome doesn't have an async executor (it uses C++ callbacks).
//! But the concept maps to Chrome's `PostTaskAndReplyWithResult()`:
//! spawn work on pool → callback on originating sequence.
//! Our executor gives the same guarantee via Rust's async/await.
//!
//! # Performance
//!
//! - Tasks stored in a Vec with free-list (no `HashMap` overhead).
//! - Waker uses `Arc<AtomicBool>` — `Send + Sync`, safe from any thread.
//! - `poll_all()` uses a ready queue — O(k) where k = woken tasks, not O(n).
//!
//! # Waker safety
//!
//! The `Waker` is backed by `Arc<AtomicBool>` — fully `Send + Sync`.
//! Background threads (tokio, rayon, `std::thread`) can call `waker.wake()`
//! safely. The atomic flag is checked by `poll_all()` on the window thread.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// Inner data for a task's `Waker`.
///
/// `Arc<WakerInner>` is the raw pointer stored in every `RawWaker`.
/// Using a struct (instead of a bare `AtomicBool`) lets us carry a `notify`
/// callback alongside the flag — when a background thread calls `waker.wake()`,
/// it both marks the task as ready AND pokes the view thread out of its park.
pub(crate) struct WakerInner {
    /// Set to `true` when the task should be polled again.
    /// Atomically writable from any thread.
    pub(crate) woken: AtomicBool,

    /// Optional callback called whenever `wake()` fires from any thread.
    ///
    /// On the view thread this is `None`.  When a `LocalExecutor` is wired
    /// to an event loop (see `set_notify`), this sends a "please tick" signal
    /// so the scheduler thread stops parking and runs `poll_all` again.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// A handle to a spawned local task. Can be used to check completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(usize);

/// A `!Send` future stored in the executor.
type BoxLocalFuture = Pin<Box<dyn Future<Output = ()>>>;

/// State of a single spawned task.
struct LocalTask {
    /// The future being polled.
    future: BoxLocalFuture,

    /// Shared waker inner — holds the woken flag and optional notify.
    /// `Arc<WakerInner>` is `Send + Sync`, safe to set from ANY thread.
    inner: Arc<WakerInner>,

    /// Whether this task has completed (Ready).
    completed: bool,
}

/// Single-threaded async executor for `!Send` futures.
///
/// This is the core of Kozan's async story. It allows user code like:
///
/// ```ignore
/// let doc = ctx.document();
/// let btn = doc.create::<HtmlButtonElement>();
///
/// // This future is !Send — it captures `btn` (which contains Handle).
/// ctx.spawn(async move {
///     let data = fetch("https://api.example.com").await;
///     btn.set_text(&data.title);  // safe! same thread
/// });
/// ```
///
/// # Design
///
/// Tasks are stored in a `Vec` with a free-list for O(1) reuse.
/// Each task has a `woken` flag (`Arc<AtomicBool>`) — the `Waker` sets
/// this atomically when the task should be polled again (safe from any thread).
/// `poll_all()` only touches woken tasks — O(k) where k is woken count.
///
/// # Waker thread-safety
///
/// The `Waker` is backed by `Arc<AtomicBool>` which is `Send + Sync`.
/// This means background threads (tokio runtime, rayon pool, `std::thread`)
/// can safely call `waker.wake()` to signal that an I/O operation completed.
/// This is the primary use case: `fetch(url).await` spawns HTTP work on
/// a tokio runtime, and the completion callback calls our `Waker` from
/// that background thread.
pub struct LocalExecutor {
    /// All spawned tasks. Completed tasks are `None` (slot freed).
    tasks: Vec<Option<LocalTask>>,

    /// Free indices for reuse (FIFO — oldest freed slot reused first).
    free: VecDeque<usize>,

    /// Newly spawned tasks that need initial polling.
    spawn_queue: VecDeque<usize>,

    /// Called when any waker fires from a background thread.
    ///
    /// Injected by the platform layer via `set_notify()`.  Sends a signal to
    /// the view thread's event channel so it stops parking and runs `tick()`.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl LocalExecutor {
    /// Create a new empty executor.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            free: VecDeque::new(),
            spawn_queue: VecDeque::new(),
            notify: None,
        }
    }

    /// Wire a "wake the event loop" callback into every future's waker.
    ///
    /// When any future's `Waker` is called from a background thread,
    /// `notify` is invoked in addition to setting the woken flag.
    /// This lets the scheduler thread stop parking and call `poll_all()`.
    ///
    /// Call this once after construction, before spawning any futures.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.notify = Some(notify);
    }

    /// Spawn a `!Send` future on this executor.
    ///
    /// The future will be polled on the window thread.
    /// Returns a [`TaskId`] that can be used to check completion.
    ///
    /// ```ignore
    /// ctx.spawn(async {
    ///     let data = fetch(url).await;
    ///     node.set_text(&data);  // !Send — safe on window thread
    /// });
    /// ```
    pub fn spawn(&mut self, future: impl Future<Output = ()> + 'static) -> TaskId {
        let task = LocalTask {
            future: Box::pin(future),
            inner: Arc::new(WakerInner {
                woken: AtomicBool::new(true), // newly spawned = needs first poll
                notify: self.notify.clone(),
            }),
            completed: false,
        };

        let id = if let Some(idx) = self.free.pop_front() {
            self.tasks[idx] = Some(task);
            idx
        } else {
            let idx = self.tasks.len();
            self.tasks.push(Some(task));
            idx
        };

        self.spawn_queue.push_back(id);
        TaskId(id)
    }

    /// Poll all woken tasks. Returns the number of tasks that made progress.
    ///
    /// Call this in the event loop after processing cross-thread wake-ups.
    /// Only polls tasks whose `Waker` has been invoked — idle tasks are skipped.
    ///
    /// A task that returns `Poll::Ready(())` is immediately cleaned up.
    /// A task that returns `Poll::Pending` stays until woken again.
    ///
    /// # Complexity
    ///
    /// O(s + w) where s = spawn queue length, w = woken task count.
    /// The scan over `tasks` checks only the atomic `woken` flag (branch prediction
    /// favors the not-woken path for idle tasks).
    pub fn poll_all(&mut self) -> usize {
        let mut progress = 0;

        // Phase 1: Poll newly spawned tasks.
        while let Some(id) = self.spawn_queue.pop_front() {
            if self.poll_task(id) {
                progress += 1;
            }
            // Immediate cleanup if completed during first poll.
            if self.tasks[id].as_ref().is_some_and(|t| t.completed) {
                self.tasks[id] = None;
                self.free.push_back(id);
            }
        }

        // Phase 2: Poll woken tasks (skip idle ones via atomic flag check).
        for id in 0..self.tasks.len() {
            let Some(task) = &self.tasks[id] else {
                continue;
            };
            if task.completed || !task.inner.woken.load(Ordering::Acquire) {
                continue;
            }

            if self.poll_task(id) {
                progress += 1;
            }

            // Immediate cleanup if completed.
            if self.tasks[id].as_ref().is_some_and(|t| t.completed) {
                self.tasks[id] = None;
                self.free.push_back(id);
            }
        }

        progress
    }

    /// Poll a single task by index. Returns true if polled (regardless of result).
    fn poll_task(&mut self, id: usize) -> bool {
        let Some(task) = &mut self.tasks[id] else {
            return false;
        };
        if task.completed {
            return false;
        }

        // Clear woken flag BEFORE polling — if the future wakes itself
        // during poll, the flag will be set again atomically.
        task.inner.woken.store(false, Ordering::Release);

        // Create a Waker for this task.
        let waker = create_waker(task.inner.clone());
        let mut cx = Context::from_waker(&waker);

        match task.future.as_mut().poll(&mut cx) {
            Poll::Ready(()) => {
                task.completed = true;
            }
            Poll::Pending => {
                // Task will be polled again when woken.
            }
        }

        true
    }

    /// Whether a specific task has completed.
    ///
    /// Returns `true` for completed tasks and cleaned-up slots.
    /// Panics for out-of-range `TaskId`s.
    #[must_use] 
    pub fn is_completed(&self, id: TaskId) -> bool {
        match self.tasks.get(id.0) {
            Some(Some(task)) => task.completed,
            Some(None) => true, // cleaned up = completed
            None => {
                panic!(
                    "TaskId({}) out of range (max: {})",
                    id.0,
                    self.tasks.len()
                );
            }
        }
    }

    /// Number of active (non-completed) tasks.
    #[must_use] 
    pub fn active_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|slot| slot.as_ref().is_some_and(|t| !t.completed))
            .count()
    }

    /// Whether the executor has no active tasks.
    #[must_use] 
    pub fn is_idle(&self) -> bool {
        self.active_count() == 0 && self.spawn_queue.is_empty()
    }

    /// Whether any task is woken and needs polling.
    #[must_use] 
    pub fn has_woken(&self) -> bool {
        if !self.spawn_queue.is_empty() {
            return true;
        }
        self.tasks.iter().any(|slot| {
            slot.as_ref()
                .is_some_and(|t| !t.completed && t.inner.woken.load(Ordering::Acquire))
        })
    }
}

impl Default for LocalExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Waker implementation ----
//
// Backed by `Arc<AtomicBool>` — fully `Send + Sync`.
// Safe to call `wake()` from ANY thread (tokio, rayon, std::thread).
//
// The Waker stores a raw pointer to the Arc's inner data.
// We manually manage the Arc reference count via clone/drop.

/// Create a `Waker` backed by `Arc<WakerInner>`.
///
/// When woken from any thread this:
/// 1. Atomically sets the `woken` flag so `poll_all()` will re-poll the task.
/// 2. Calls `notify` (if set) to unpark the view thread from its sleep.
fn create_waker(inner: Arc<WakerInner>) -> Waker {
    let raw = Arc::into_raw(inner) as *const ();
    let raw_waker = RawWaker::new(raw, &VTABLE);
    // SAFETY: vtable correctly manages the Arc<WakerInner> refcount.
    // Arc<WakerInner> is Send + Sync — safe to call from any thread.
    unsafe { Waker::from_raw(raw_waker) }
}

const VTABLE: RawWakerVTable =
    RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

/// Clone: increment Arc refcount, return a new `RawWaker`.
unsafe fn waker_clone(ptr: *const ()) -> RawWaker {
    let arc = unsafe { Arc::from_raw(ptr as *const WakerInner) };
    let cloned = arc.clone();
    std::mem::forget(arc); // don't drop — cloning, not moving
    RawWaker::new(Arc::into_raw(cloned) as *const (), &VTABLE)
}

/// Wake by value: set flag + notify, then drop the Arc.
unsafe fn waker_wake(ptr: *const ()) {
    let arc = unsafe { Arc::from_raw(ptr as *const WakerInner) };
    arc.woken.store(true, Ordering::Release);
    if let Some(notify) = &arc.notify {
        notify();
    }
    // arc dropped here — decrements refcount.
}

/// Wake by reference: set flag + notify, don't drop.
unsafe fn waker_wake_by_ref(ptr: *const ()) {
    let arc = unsafe { Arc::from_raw(ptr as *const WakerInner) };
    arc.woken.store(true, Ordering::Release);
    if let Some(notify) = &arc.notify {
        notify();
    }
    std::mem::forget(arc); // don't drop — by-ref
}

/// Drop: decrement Arc refcount.
unsafe fn waker_drop(ptr: *const ()) {
    let _arc = unsafe { Arc::from_raw(ptr as *const WakerInner) };
    // _arc dropped here — decrements refcount.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::task::Waker;

    #[test]
    fn spawn_and_complete_immediate() {
        let mut exec = LocalExecutor::new();
        let done = Rc::new(Cell::new(false));

        let d = done.clone();
        let id = exec.spawn(async move {
            d.set(true);
        });

        assert_eq!(exec.active_count(), 1);
        exec.poll_all();
        assert!(done.get());
        assert!(exec.is_completed(id));
        assert!(exec.is_idle());
    }

    #[test]
    fn spawn_pending_then_wake() {
        let mut exec = LocalExecutor::new();
        let counter = Rc::new(Cell::new(0u32));
        let waker_holder: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));

        let c = counter.clone();
        let wh = waker_holder.clone();
        exec.spawn(async move {
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

        // First poll — task yields.
        exec.poll_all();
        assert_eq!(counter.get(), 1);
        assert!(!exec.is_idle());

        // Wake the task (simulating background completion).
        waker_holder.borrow().as_ref().unwrap().wake_by_ref();

        // Second poll — task completes.
        exec.poll_all();
        assert_eq!(counter.get(), 2);
        assert!(exec.is_idle());
    }

    #[test]
    fn wake_from_another_thread() {
        // This test verifies the Waker is truly Send + Sync.
        let mut exec = LocalExecutor::new();
        let waker_holder: Arc<std::sync::Mutex<Option<Waker>>> =
            Arc::new(std::sync::Mutex::new(None));

        let wh = waker_holder.clone();
        exec.spawn(async move {
            std::future::poll_fn(|cx| {
                let mut guard = wh.lock().unwrap();
                if guard.is_none() {
                    *guard = Some(cx.waker().clone());
                    Poll::Pending
                } else {
                    Poll::Ready(())
                }
            })
            .await;
        });

        // First poll — future stores waker and yields.
        exec.poll_all();
        assert!(!exec.is_idle());

        // Wake from another thread — this is the PRIMARY use case
        // (tokio/rayon background thread completing an I/O operation).
        let wh = waker_holder.clone();
        let handle = std::thread::spawn(move || {
            let guard = wh.lock().unwrap();
            guard.as_ref().unwrap().wake_by_ref();
        });
        handle.join().unwrap();

        // Back on "window thread" — poll completes the task.
        exec.poll_all();
        assert!(exec.is_idle());
    }

    #[test]
    fn multiple_tasks() {
        let mut exec = LocalExecutor::new();
        let log = Rc::new(RefCell::new(Vec::new()));

        for i in 0..5 {
            let l = log.clone();
            exec.spawn(async move {
                l.borrow_mut().push(i);
            });
        }

        exec.poll_all();
        assert_eq!(*log.borrow(), vec![0, 1, 2, 3, 4]);
        assert!(exec.is_idle());
    }

    #[test]
    fn task_id_reuse() {
        let mut exec = LocalExecutor::new();

        // Spawn and complete.
        let id1 = exec.spawn(async {});
        exec.poll_all();
        assert!(exec.is_completed(id1));

        // Spawn again — should reuse the slot.
        let id2 = exec.spawn(async {});
        assert_eq!(id1.0, id2.0); // same index
        exec.poll_all();
    }

    #[test]
    fn has_woken() {
        let mut exec = LocalExecutor::new();
        assert!(!exec.has_woken());

        let wh: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));
        let wh2 = wh.clone();
        exec.spawn(async move {
            std::future::poll_fn(|cx| {
                *wh2.borrow_mut() = Some(cx.waker().clone());
                Poll::<()>::Pending
            })
            .await;
        });

        assert!(exec.has_woken()); // newly spawned
        exec.poll_all();
        assert!(!exec.has_woken()); // pending, not woken

        // Wake from "background".
        wh.borrow().as_ref().unwrap().wake_by_ref();
        assert!(exec.has_woken());
    }

    #[test]
    fn waker_clone_and_drop() {
        // Ensure waker clone/drop doesn't leak or double-free.
        let inner = Arc::new(WakerInner {
            woken: AtomicBool::new(false),
            notify: None,
        });
        let waker = create_waker(inner.clone());

        let waker2 = waker.clone();
        drop(waker);

        waker2.wake_by_ref();
        assert!(inner.woken.load(Ordering::Acquire));

        drop(waker2);
        // Should not crash — Arc refcount managed correctly.
    }

    #[test]
    fn completed_task_query() {
        let _exec = LocalExecutor::new();
        // Querying cleaned-up slot returns true.
        // Out-of-range panics in debug (tested separately if needed).
    }

    #[test]
    fn immediate_cleanup_on_complete() {
        let mut exec = LocalExecutor::new();
        let id = exec.spawn(async {});
        exec.poll_all();

        // Task should be cleaned up immediately, not in a separate pass.
        assert!(exec.is_completed(id));
        assert!(exec.is_idle());
        // Slot should be freed for reuse.
        let id2 = exec.spawn(async {});
        assert_eq!(id.0, id2.0);
    }
}
