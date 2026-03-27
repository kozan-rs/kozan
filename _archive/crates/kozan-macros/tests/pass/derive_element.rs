use kozan_core::Handle;
use kozan_macros::Element;

#[derive(Copy, Clone, Element)]
#[element(tag = "x-custom")]
pub struct CustomElement(Handle);

fn main() {}
