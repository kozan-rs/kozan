use kozan_macros::Event;

#[derive(Event)]
#[event(bubbles, cancelable)]
pub struct ClickEvent {
    pub x: f32,
    pub y: f32,
}

#[derive(Event)]
pub struct ResizeEvent {
    pub width: u32,
    pub height: u32,
}

fn main() {}
