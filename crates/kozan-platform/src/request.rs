//! Window configuration — settings for creating new windows.
//!
//! Pure data types. No windowing backend dependency.

/// Configuration for creating a new window.
///
/// Passed to `App::window()` or sent via `PlatformHost::create_window()`.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    /// Window title.
    pub title: String,
    /// Initial width in logical pixels.
    pub width: u32,
    /// Initial height in logical pixels.
    pub height: u32,
    /// Whether the window can be resized by the user.
    pub resizable: bool,
    /// Whether the window has OS decorations (title bar, borders).
    pub decorations: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: String::from("Kozan"),
            width: 800,
            height: 600,
            resizable: true,
            decorations: true,
        }
    }
}
