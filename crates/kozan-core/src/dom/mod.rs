//! DOM types — like Chrome's `core/dom/`.
//!
//! Contains the core DOM abstractions: Node, `EventTarget`, Element,
//! Handle, Document, Text, and attribute management.

pub(crate) mod document_cell;

pub mod attribute;
pub mod document;
pub mod element_data;
pub mod handle;
pub mod node;
pub mod text;
pub mod traits;
