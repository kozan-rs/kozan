//! Frame metrics ring buffer — stores last N frames for graphing.
//!
//! Chrome: `FrameTimingTracker` in `cc/metrics/`.

#![allow(dead_code)]

use std::cell::Cell;

use kozan_primitives::timing::FrameTiming;

/// How many frames to keep for the graph.
const HISTORY_SIZE: usize = 120;

/// Per-frame snapshot stored in the ring buffer.
#[derive(Clone, Copy, Default)]
pub struct FrameSnapshot {
    pub timing: FrameTiming,
    pub fps: f64,
    pub frame_number: u64,
    pub budget_ms: f64,
}

impl FrameSnapshot {
    /// Fraction of budget consumed (1.0 = exactly on budget).
    pub fn budget_usage(&self) -> f64 {
        if self.budget_ms <= 0.0 {
            return 0.0;
        }
        self.timing.total_ms / self.budget_ms
    }

    pub fn is_jank(&self) -> bool {
        self.timing.total_ms > self.budget_ms
    }
}

/// Ring buffer of frame snapshots — lock-free, single-threaded.
pub struct FrameHistory {
    frames: [Cell<FrameSnapshot>; HISTORY_SIZE],
    write_idx: Cell<usize>,
    total_frames: Cell<u64>,
    jank_count: Cell<u64>,
    peak_ms: Cell<f64>,
    avg_accum: Cell<f64>,
}

impl FrameHistory {
    pub fn new() -> Self {
        Self {
            frames: std::array::from_fn(|_| Cell::new(FrameSnapshot::default())),
            write_idx: Cell::new(0),
            total_frames: Cell::new(0),
            jank_count: Cell::new(0),
            peak_ms: Cell::new(0.0),
            avg_accum: Cell::new(0.0),
        }
    }

    /// Record a new frame snapshot.
    pub fn push(&self, snapshot: FrameSnapshot) {
        let is_jank = snapshot.is_jank();
        let total_ms = snapshot.timing.total_ms;

        let idx = self.write_idx.get();
        self.frames[idx].set(snapshot);
        self.write_idx.set((idx + 1) % HISTORY_SIZE);

        let total = self.total_frames.get() + 1;
        self.total_frames.set(total);

        if is_jank {
            self.jank_count.set(self.jank_count.get() + 1);
        }

        if total_ms > self.peak_ms.get() {
            self.peak_ms.set(total_ms);
        }

        let prev_avg = self.avg_accum.get();
        self.avg_accum.set(prev_avg + (total_ms - prev_avg) / total as f64);
    }

    /// Reset accumulated stats (avg/peak/jank) and clear the graph.
    pub fn reset(&self) {
        for cell in &self.frames {
            cell.set(FrameSnapshot::default());
        }
        self.write_idx.set(0);
        self.total_frames.set(0);
        self.jank_count.set(0);
        self.peak_ms.set(0.0);
        self.avg_accum.set(0.0);
    }

    /// Get frame at position (0 = oldest visible, HISTORY_SIZE-1 = newest).
    pub fn frame_at(&self, pos: usize) -> FrameSnapshot {
        let idx = (self.write_idx.get() + pos) % HISTORY_SIZE;
        self.frames[idx].get()
    }

    /// Iterate frames oldest → newest.
    pub fn iter(&self) -> impl Iterator<Item = FrameSnapshot> + '_ {
        (0..HISTORY_SIZE).map(move |i| self.frame_at(i))
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames.get()
    }

    pub fn jank_count(&self) -> u64 {
        self.jank_count.get()
    }

    pub fn peak_ms(&self) -> f64 {
        self.peak_ms.get()
    }

    pub fn avg_ms(&self) -> f64 {
        self.avg_accum.get()
    }

    /// Number of filled slots (may be < HISTORY_SIZE at startup).
    pub fn len(&self) -> usize {
        (self.total_frames.get() as usize).min(HISTORY_SIZE)
    }

    pub fn capacity(&self) -> usize {
        HISTORY_SIZE
    }
}
