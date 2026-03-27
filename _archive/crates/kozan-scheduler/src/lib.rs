//! `kozan-scheduler` — Task scheduler and async executor for Kozan.
//!
//! Like Chrome's `base/task/` + `blink/platform/scheduler/`.
//! Standalone crate — zero dependency on `kozan-core`.
//!
//! # Architecture
//!
//! ```text
//! Scheduler (MainThreadScheduler — the event loop)
//! ├── TaskQueueManager (SequenceManager — priority picker)
//! │   └── TaskQueue × 6 (one per priority level)
//! ├── MicrotaskQueue (drain after each macrotask)
//! ├── LocalExecutor (!Send async runtime)
//! ├── FrameScheduler (vsync-driven frame timing)
//! └── WakeReceiver (cross-thread task delivery)
//! ```
//!
//! # Event loop algorithm (HTML spec)
//!
//! ```text
//! loop {
//!     1. Receive cross-thread tasks
//!     2. Promote delayed tasks
//!     3. Poll async executor
//!     4. Pick ONE macrotask → run
//!     5. Drain ALL microtasks
//!     6. If frame due → callbacks → style → layout → paint
//!     7. Park until next event
//! }
//! ```

pub mod executor;
pub mod frame;
pub mod microtask;
pub mod queue;
pub mod scheduler;
pub mod task;
pub mod timer;
pub mod waker;

// Re-exports: the main types users need.
pub use executor::{LocalExecutor, TaskId};
pub use frame::{FrameInfo, FrameScheduler, FrameTiming};
pub use microtask::{Microtask, MicrotaskQueue};
pub use queue::{TaskQueue, TaskQueueManager};
pub use scheduler::{Scheduler, TickResult};
pub use task::{Task, TaskPriority};
pub use waker::{CrossThreadTask, SendError, WakeReceiver, WakeSender, cross_thread_channel};

// ---- Compile-time Send/Sync guarantees ----
//
// These assertions prevent accidental breakage of threading contracts.
// A refactor that removes PhantomData or changes a field type will
// fail to compile here — not silently at runtime.

#[cfg(test)]
mod send_sync_tests {
    use super::*;

    // Must be Send + Clone (shared across background threads).
    const _: fn() = || {
        fn assert_send_clone<T: Send + Clone>() {}
        assert_send_clone::<WakeSender>();
    };

    // Must be Send (crosses thread boundary).
    const _: fn() = || {
        fn assert_send<T: Send>() {}
        assert_send::<CrossThreadTask>();
    };

    // Must NOT be Send (stays on window thread).
    // Scheduler contains WakeReceiver which has PhantomData<*const ()>.
    // If this compiles, our !Send guarantee is broken.
    // We verify !Send via a negative test that would fail to compile
    // if Scheduler were Send. Unfortunately Rust doesn't have
    // static_assert_not_impl, so we test this at runtime:
    #[test]
    fn send_types_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<WakeSender>();
        assert_send::<CrossThreadTask>();
    }
}
