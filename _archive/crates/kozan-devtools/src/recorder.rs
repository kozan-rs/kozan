//! Frame recorder — captures per-frame data for timeline analysis.
//!
//! Chrome: Performance panel recording. Start/stop captures frame data
//! into a buffer for offline analysis. M2 will add a timeline browser UI.

#![allow(dead_code)]

use std::cell::{Cell, RefCell};

use kozan_primitives::timing::FrameTiming;

use crate::metrics::FrameSnapshot;

/// Captured frame data for timeline playback.
#[derive(Clone, Copy)]
pub struct FrameCapture {
    pub frame_number: u64,
    pub timestamp_ms: f64,
    pub timing: FrameTiming,
    pub fps: f64,
    pub budget_ms: f64,
    pub dom_node_count: u32,
    pub element_count: u32,
}

/// Records frame data on demand for later analysis.
pub struct FrameRecorder {
    recording: Cell<bool>,
    frames: RefCell<Vec<FrameCapture>>,
    max_frames: usize,
}

impl FrameRecorder {
    pub fn new(max_frames: usize) -> Self {
        Self {
            recording: Cell::new(false),
            frames: RefCell::new(Vec::new()),
            max_frames,
        }
    }

    pub fn start(&self) {
        self.frames.borrow_mut().clear();
        self.recording.set(true);
    }

    pub fn stop(&self) {
        self.recording.set(false);
    }

    pub fn is_recording(&self) -> bool {
        self.recording.get()
    }

    /// Capture a frame if recording. Stops automatically at max capacity.
    pub fn capture(&self, snapshot: &FrameSnapshot, timestamp_ms: f64) {
        if !self.recording.get() {
            return;
        }

        let mut frames = self.frames.borrow_mut();
        if frames.len() >= self.max_frames {
            self.recording.set(false);
            return;
        }

        frames.push(FrameCapture {
            frame_number: snapshot.frame_number,
            timestamp_ms,
            timing: snapshot.timing,
            fps: snapshot.fps,
            budget_ms: snapshot.budget_ms,
            dom_node_count: snapshot.dom_node_count,
            element_count: snapshot.element_count,
        });
    }

    pub fn frame_count(&self) -> usize {
        self.frames.borrow().len()
    }
}
