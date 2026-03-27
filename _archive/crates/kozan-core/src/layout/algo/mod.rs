//! Layout algorithms — shared utilities and compliance tests.
//!
//! The actual layout dispatch lives in [`super::document_layout`] via
//! `DocumentLayoutView`, which implements Taffy's traits against `Document`.
//!
//! This module provides:
//! - [`shared`]: `ComputedValues` → `taffy::Style` conversion, used by
//!   `DocumentLayoutView` and tests.
//! - [`css_tests`]: CSS compliance tests exercising the full pipeline.

#[cfg(test)]
mod css_tests;
#[cfg(test)]
mod float;
pub(crate) mod shared;
