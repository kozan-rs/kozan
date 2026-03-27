//! Cross-thread waker — wakes the window thread from background threads.
//!
//! When a background task (fetch, file I/O, compute) completes on a
//! thread pool, it needs to wake the window thread so the result can
//! be processed. This module provides the thread-safe waking mechanism.
//!
//! # Architecture
//!
//! ```text
//! Background Thread              Window Thread
//! ┌────────────────┐            ┌──────────────────────┐
//! │ HTTP done!     │            │ Scheduler (parked)    │
//! │ sender.send()──┼───────────→│ receiver.try_recv()   │
//! │                │            │ → wakes up, runs task │
//! └────────────────┘            └──────────────────────┘
//! ```
//!
//! # Chrome mapping
//!
//! Chrome uses `base::TaskRunner::PostTask()` to cross thread boundaries.
//! The `TaskRunner` carries the target thread + priority. In Rust, we use
//! `mpsc::sync_channel` (bounded) which gives us the same semantics plus
//! backpressure when the window thread is overwhelmed.
//!
//! # Performance
//!
//! - Bounded channel prevents unbounded memory growth from runaway senders.
//! - `try_recv()` is non-blocking — fits into the event loop.
//! - No allocation per send (channel pre-allocates buffer).

use std::sync::Arc;
use std::sync::mpsc;

use crate::task::TaskPriority;

/// Maximum number of pending cross-thread tasks before senders block.
/// This prevents a runaway background thread from filling unbounded memory.
/// 1024 is generous — if the window thread can't keep up with 1024 tasks
/// per frame, something else is wrong.
const CHANNEL_CAPACITY: usize = 1024;

/// A task sent from a background thread to the window thread.
///
/// Must be `Send` because it crosses thread boundaries.
/// Carries a priority so the scheduler can route it to the correct queue.
///
/// # Example
///
/// ```ignore
/// // On background thread:
/// let data = reqwest::get(url).await?;
///
/// // Send result back to window thread at Normal priority:
/// sender.send(CrossThreadTask::new(TaskPriority::Normal, move || {
///     btn.set_text(&data.title);  // safe! runs on window thread
/// }));
/// ```
pub struct CrossThreadTask {
    callback: Box<dyn FnOnce() + Send>,
    priority: TaskPriority,
}

impl CrossThreadTask {
    /// Create a new cross-thread task with the given priority.
    #[inline]
    pub fn new(priority: TaskPriority, callback: impl FnOnce() + Send + 'static) -> Self {
        Self {
            callback: Box::new(callback),
            priority,
        }
    }

    /// The priority this task should be routed to.
    #[inline]
    #[must_use]
    pub fn priority(&self) -> TaskPriority {
        self.priority
    }

    /// Execute this task on the window thread.
    #[inline]
    pub fn run(self) {
        (self.callback)();
    }
}

/// The sending half — cloned and given to background threads.
///
/// `Send + Clone` — clone this and move the clone to each background thread.
/// Each clone can independently send tasks to the window thread.
///
/// Like Chrome's `scoped_refptr<base::TaskRunner>` which can be used
/// from any thread to post tasks to a specific sequence.
///
/// Note: `WakeSender` is `Send` but not `Sync`. You cannot share it
/// by `&` reference across threads — clone it and move the clone.
pub struct WakeSender {
    sender: mpsc::SyncSender<CrossThreadTask>,
    /// Called after a successful send to unpark the view thread.
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl WakeSender {
    /// Wire a "wake the event loop" callback into this sender.
    ///
    /// After every successful `send` / `post`, `notify` is called so the
    /// view thread stops parking and drains the cross-thread task queue.
    pub fn set_notify(&mut self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.notify = Some(notify);
    }

    /// Send a task to the window thread with the given priority.
    ///
    /// Blocks if the channel is full (backpressure). Returns `Err`
    /// if the receiver has been dropped (window closed).
    #[inline]
    pub fn send(&self, task: CrossThreadTask) -> Result<(), SendError> {
        let result = self.sender.send(task).map_err(|_| SendError::Disconnected);
        if result.is_ok() {
            if let Some(notify) = &self.notify {
                notify();
            }
        }
        result
    }

    /// Try to send without blocking. Returns `Err(Full)` if channel is full.
    #[inline]
    pub fn try_send(&self, task: CrossThreadTask) -> Result<(), SendError> {
        let result = self.sender.try_send(task).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => SendError::Full,
            mpsc::TrySendError::Disconnected(_) => SendError::Disconnected,
        });
        if result.is_ok() {
            if let Some(notify) = &self.notify {
                notify();
            }
        }
        result
    }

    /// Convenience: send a closure at Normal priority.
    #[inline]
    pub fn post(&self, callback: impl FnOnce() + Send + 'static) -> Result<(), SendError> {
        self.send(CrossThreadTask::new(TaskPriority::Normal, callback))
    }

    /// Convenience: send a closure at a specific priority.
    #[inline]
    pub fn post_with_priority(
        &self,
        priority: TaskPriority,
        callback: impl FnOnce() + Send + 'static,
    ) -> Result<(), SendError> {
        self.send(CrossThreadTask::new(priority, callback))
    }
}

impl Clone for WakeSender {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            notify: self.notify.clone(),
        }
    }
}

/// The receiving half — stays on the window thread.
///
/// `!Send` by design (via `PhantomData`). Only the window thread should
/// drain incoming cross-thread tasks.
pub struct WakeReceiver {
    receiver: mpsc::Receiver<CrossThreadTask>,
    _not_send: std::marker::PhantomData<*const ()>,
}

impl WakeReceiver {
    /// Try to receive a cross-thread task without blocking.
    ///
    /// Returns `None` if no tasks are available.
    #[inline]
    #[must_use]
    pub fn try_recv(&self) -> Option<CrossThreadTask> {
        self.receiver.try_recv().ok()
    }

    /// Process all pending cross-thread tasks through a callback.
    ///
    /// Calls `f` for each task. No allocation — iterates inline.
    /// Returns the number of tasks processed.
    ///
    /// ```ignore
    /// receiver.drain_into(|task| {
    ///     task_queue.push(Task::new(task.priority(), move || task.run()));
    /// });
    /// ```
    pub fn drain_into(&self, mut f: impl FnMut(CrossThreadTask)) -> usize {
        let mut count = 0;
        while let Some(task) = self.try_recv() {
            f(task);
            count += 1;
        }
        count
    }
}

/// Error returned when sending to a closed or full channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    /// The window thread's receiver was dropped (window closed).
    Disconnected,
    /// The channel is full (backpressure — window thread is overwhelmed).
    Full,
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Disconnected => write!(f, "window thread disconnected"),
            SendError::Full => write!(f, "cross-thread channel full"),
        }
    }
}

impl std::error::Error for SendError {}

/// Create a linked sender/receiver pair.
///
/// The `WakeSender` can be cloned and sent to background threads.
/// The `WakeReceiver` stays on the window thread.
/// The channel is bounded to `CHANNEL_CAPACITY` to prevent
/// unbounded memory growth from runaway background senders.
///
/// ```ignore
/// let (sender, receiver) = cross_thread_channel();
///
/// // Give sender to background threads
/// tokio::spawn(async move {
///     let data = fetch(url).await;
///     sender.post(move || btn.set_text(&data)).unwrap();
/// });
///
/// // On window thread event loop
/// receiver.drain_into(|task| {
///     queue.push(Task::new(task.priority(), move || task.run()));
/// });
/// ```
#[must_use]
pub fn cross_thread_channel() -> (WakeSender, WakeReceiver) {
    let (sender, receiver) = mpsc::sync_channel(CHANNEL_CAPACITY);
    (
        WakeSender {
            sender,
            notify: None,
        },
        WakeReceiver {
            receiver,
            _not_send: std::marker::PhantomData,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    #[test]
    fn send_and_receive_task() {
        let (sender, receiver) = cross_thread_channel();

        let called = Arc::new(AtomicBool::new(false));
        let c = called.clone();

        sender
            .post(move || c.store(true, Ordering::SeqCst))
            .unwrap();

        let task = receiver.try_recv().unwrap();
        task.run();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn try_recv_empty_returns_none() {
        let (_sender, receiver) = cross_thread_channel();
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn drain_into_runs_all() {
        let (sender, receiver) = cross_thread_channel();
        let counter = Arc::new(AtomicU32::new(0));

        for _ in 0..5 {
            let c = counter.clone();
            sender
                .post(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
        }

        let executed = receiver.drain_into(|task| task.run());
        assert_eq!(executed, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn sender_clone_works() {
        let (sender, receiver) = cross_thread_channel();
        let sender2 = sender.clone();

        sender.post(|| {}).unwrap();
        sender2.post(|| {}).unwrap();

        assert!(receiver.try_recv().is_some());
        assert!(receiver.try_recv().is_some());
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn send_from_another_thread() {
        let (sender, receiver) = cross_thread_channel();

        let handle = std::thread::spawn(move || {
            sender.post(|| {}).unwrap();
        });

        handle.join().unwrap();
        assert!(receiver.try_recv().is_some());
    }

    #[test]
    fn send_to_dropped_receiver_errors() {
        let (sender, receiver) = cross_thread_channel();
        drop(receiver);

        let result = sender.post(|| {});
        assert_eq!(result, Err(SendError::Disconnected));
    }

    #[test]
    fn send_error_display() {
        assert_eq!(
            SendError::Disconnected.to_string(),
            "window thread disconnected"
        );
        assert_eq!(SendError::Full.to_string(), "cross-thread channel full");
    }

    #[test]
    fn cross_thread_task_carries_priority() {
        let task = CrossThreadTask::new(TaskPriority::Input, || {});
        assert_eq!(task.priority(), TaskPriority::Input);
    }

    #[test]
    fn post_with_priority() {
        let (sender, receiver) = cross_thread_channel();
        sender
            .post_with_priority(TaskPriority::Input, || {})
            .unwrap();

        let task = receiver.try_recv().unwrap();
        assert_eq!(task.priority(), TaskPriority::Input);
    }
}
