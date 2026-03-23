//! Paint system — converts the fragment tree into draw commands.
//!
//! Chrome equivalent: `core/paint/` + `platform/graphics/paint/`.
//!
//! # Architecture
//!
//! ```text
//! Fragment Tree (from layout)
//!       │
//!       ▼
//! ┌─────────────────────┐
//! │  Pre-Paint           │  Build paint property state
//! │  (property_state.rs) │  (transform, clip, effect)
//! └──────────┬──────────┘
//!            │
//!            ▼
//! ┌─────────────────────┐
//! │  Paint               │  Walk fragments in paint order,
//! │  (painter.rs)        │  emit display items
//! └──────────┬──────────┘
//!            │
//!            ▼
//! ┌─────────────────────┐
//! │  DisplayList         │  Ordered list of draw commands
//! │  (display_list.rs)   │  grouped by PropertyState
//! └─────────────────────┘
//! ```
//!
//! # Chrome mapping
//!
//! | Chrome | Kozan |
//! |--------|-------|
//! | `DisplayItem` | `DisplayItem` |
//! | `DisplayItemList` | `DisplayList` |
//! | `PropertyTreeState` | `PropertyState` |
//! | `PaintChunk` | `PaintChunk` |
//! | `BoxFragmentPainter` | `paint_fragment()` |
//! | `PaintController` | `PaintContext` |
//! | `PaintPhase` | `PaintPhase` |

pub mod display_item;
pub mod display_list;
pub mod painter;
pub mod property_state;

pub use display_item::{DisplayItem, DrawCommand};
pub use display_list::{DisplayList, PaintChunk};
pub(crate) use painter::Painter;
pub use property_state::PropertyState;
