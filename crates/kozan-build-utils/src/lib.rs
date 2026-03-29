//! Shared build-script utilities for Kozan codegen.
//!
//! Contains the TOML schema parser and code writer used by
//! both `kozan-style` and `kozan-css` build scripts.

mod schema;
mod writer;
pub mod match_algo;

pub use schema::*;
pub use writer::CodeWriter;
