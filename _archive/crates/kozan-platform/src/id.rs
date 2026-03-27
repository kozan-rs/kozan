//! Platform-level identifiers.
//!
//! All IDs are Kozan's own types — no windowing backend types leak.
//! The backend (kozan-winit) maps between its IDs and ours internally.

use kozan_primitives::arena::RawId;

/// Unique identifier for a View.
///
/// Type-safe wrapper around [`RawId`]. `Copy + Send + Sync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewId(RawId);

impl ViewId {
    /// Create from a raw arena ID. Used by windowing backends.
    #[inline]
    #[must_use]
    pub fn from_raw(raw: RawId) -> Self {
        Self(raw)
    }

    #[inline]
    #[must_use]
    pub fn raw(self) -> RawId {
        self.0
    }
}

impl std::fmt::Display for ViewId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "View({})", self.0)
    }
}

/// Unique identifier for a Window.
///
/// Kozan's own type — the windowing backend maps its native ID to this.
/// `Copy + Send + Sync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

static NEXT_WINDOW_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl WindowId {
    /// Allocate a new unique `WindowId`.
    pub fn next() -> Self {
        Self(NEXT_WINDOW_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    /// Get the raw u64 value.
    #[inline]
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Window({})", self.0)
    }
}
