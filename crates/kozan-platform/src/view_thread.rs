//! View thread types — handle and errors.
//!
//! The event loop and spawn logic live in `pipeline/`.

use std::sync::mpsc;
use std::thread;

use kozan_scheduler::WakeSender;

use crate::event::ViewEvent;

/// Error returned when a thread fails to start.
#[derive(Debug)]
pub enum SpawnError {
    ThreadSpawn(std::io::Error),
    SetupFailed,
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpawnError::ThreadSpawn(e) => write!(f, "failed to spawn thread: {e}"),
            SpawnError::SetupFailed => write!(f, "thread setup failed"),
        }
    }
}

impl std::error::Error for SpawnError {}

/// Handle to a running view thread.
pub struct ViewThreadHandle {
    sender: mpsc::Sender<ViewEvent>,
    wake_sender: WakeSender,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl ViewThreadHandle {
    pub fn from_parts(
        sender: mpsc::Sender<ViewEvent>,
        wake_sender: WakeSender,
        join_handle: thread::JoinHandle<()>,
    ) -> Self {
        Self { sender, wake_sender, join_handle: Some(join_handle) }
    }

    pub fn send(&self, event: ViewEvent) -> bool {
        self.sender.send(event).is_ok()
    }

    pub fn wake_sender(&self) -> WakeSender {
        self.wake_sender.clone()
    }

    pub fn shutdown(&mut self) {
        let _ = self.sender.send(ViewEvent::Shutdown);
        if let Some(h) = self.join_handle.take() { let _ = h.join(); }
    }
}

impl Drop for ViewThreadHandle {
    fn drop(&mut self) { self.shutdown(); }
}
