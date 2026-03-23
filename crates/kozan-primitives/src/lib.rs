//! Foundational value types for Kozan.
//!
//! This crate provides the core geometric and visual primitives used throughout the
//! Kozan UI platform: 2D/3D geometry (`Point`, `Size`, `Rect`), colors (`Color`),
//! CSS-compatible length units, affine and 3D transforms, and a typed arena allocator.
//! These types are intentionally dependency-free and `Copy`/`Clone` where possible,
//! serving as the shared vocabulary between layout, paint, and rendering.

pub mod arena;
pub mod color;
pub mod geometry;
pub mod rounded_rect;
pub mod timing;
pub mod transform;
pub mod units;
