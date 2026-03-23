//! Scroll system — independent subsystems mirroring Chrome's `cc/` architecture.
//!
//! Chrome: `cc/input/` (scroll nodes, input handler) + `cc/trees/` (scroll tree, transform tree).
//!
//! - [`ScrollNode`] — per-element geometry (container size, content size, axis constraints).
//! - [`ScrollTree`] — parent-child topology for the scroll chain.
//! - [`ScrollOffsets`] — mutable scroll displacement per node.
//! - [`controller`] — applies deltas along the chain with clamping.
//!
//! Default actions (keyboard scroll, wheel scroll) live in `input::default_action` —
//! the scroll system only knows about geometry and offsets, not input events.

pub(crate) mod controller;
pub(crate) mod node;
pub mod offsets;
pub mod tree;

pub(crate) use controller::ScrollController;
pub use offsets::ScrollOffsets;
pub use tree::ScrollTree;
