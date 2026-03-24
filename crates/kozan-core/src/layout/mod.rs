//! Layout engine — computes size and position for every element.
//!
//! Chrome equivalent: `blink/renderer/core/layout/` + `ng/` (`LayoutNG`).
//!
//! # Architecture (DOM IS the layout tree)
//!
//! ```text
//! DOM Tree (Document)              Fragment Tree
//! ───────────────────              ──────────────
//! Storage<LayoutNodeData>          (final positions)
//!   .style (taffy::Style)
//!   .cache (taffy::Cache)     ──layout()──►  Fragment (Arc, immutable)
//!   .layout_children               TextFragment
//!   .unrounded_layout              Fragment
//!
//!   Taffy reads/writes LayoutNodeData
//!   directly via DocumentLayoutView
//! ```
//!
//! No separate `LayoutTree`. Taffy's trait implementations
//! (`LayoutPartialTree`, `CacheTree`, etc.) are on `DocumentLayoutView`
//! which wraps `&mut Document` + `&LayoutContext`.
//!
//! # Module structure
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`node_data`] | Per-node layout data (style, cache, layout results) |
//! | [`document_layout`] | Taffy trait impls on Document, resolve pipeline |
//! | [`fragment`] | Immutable output flowing up |
//! | [`result`] | Algorithm output wrapper |
//! | `algo::shared` | ComputedValues → taffy::Style conversion |
//! | [`inline`] | Text measurement, font system |
//! | [`hit_test`](mod@hit_test) | Fragment tree hit testing |

pub mod algo;
pub mod box_model;
pub mod context;
pub mod document_layout;
pub mod fragment;
pub mod hit_test;
pub mod inline;
pub mod node_data;
pub mod result;

// Re-exports.
pub use context::LayoutContext;
pub use fragment::{
    BoxFragmentData, ChildFragment, Fragment, FragmentKind, LineFragmentData, OverflowClip,
    PhysicalInsets, TextFragmentData,
};
pub use hit_test::HitTestResult;
pub use inline::{FontHeight, FontMetrics, TextMeasurer, TextMetrics, resolve_line_height};
pub use node_data::LayoutNodeData;
pub use result::{IntrinsicSizes, LayoutResult};
