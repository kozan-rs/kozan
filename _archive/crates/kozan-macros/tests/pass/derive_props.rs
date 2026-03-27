use kozan_core::Handle;
use kozan_macros::{Element, Props};

#[derive(Copy, Clone, Element)]
#[element(tag = "x-button", data = ButtonData)]
pub struct Button(Handle);

#[derive(Default, Props)]
#[props(element = Button)]
pub struct ButtonData {
    #[prop]
    pub label: String,
    #[prop]
    pub disabled: bool,
}

fn main() {}
