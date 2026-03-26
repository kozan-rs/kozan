//! Scroll system — Chrome's `cc/` architecture.
//!
//! Chrome: `cc/input/` + `cc/trees/`.

pub(crate) mod controller;
pub(crate) mod node;
pub mod offsets;
pub mod scrollbar;
pub mod tree;

pub(crate) use controller::ScrollController;
pub use offsets::ScrollOffsets;
pub use scrollbar::Orientation;
pub use tree::ScrollTree;
