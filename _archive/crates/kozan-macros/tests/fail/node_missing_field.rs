use kozan_macros::Node;

// `Node` requires a newtype tuple struct with a `Handle` as the first field.
// This struct has no fields, so `self.0` in the generated `HasHandle` impl
// will fail to compile.
#[derive(Copy, Clone, Node)]
pub struct EmptyNode;

fn main() {}
