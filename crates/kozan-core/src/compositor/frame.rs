//! Compositor frame — what the GPU receives each vsync.
//!
//! Chrome: `viz::CompositorFrame`.

use std::sync::Arc;

use crate::paint::DisplayList;
use crate::scroll::ScrollOffsets;

/// The output the compositor produces each vsync for the GPU.
///
/// Contains the display list (from the view thread's last paint) and
/// the compositor's current scroll offsets. The renderer uses these
/// offsets to override tagged scroll transforms — no repaint needed.
pub struct CompositorFrame {
    pub display_list: Arc<DisplayList>,
    /// Compositor's authoritative scroll offsets.
    /// The renderer looks up offsets by DOM node ID (O(1) via Storage)
    /// when it encounters a `PushTransform` tagged with `scroll_node`.
    pub scroll_offsets: ScrollOffsets,
}
