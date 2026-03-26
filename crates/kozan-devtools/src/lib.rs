//! Chrome-style DevTools overlay for Kozan.
//!
//! Attach to any window with `DevTools::attach(ctx)` — injects a floating
//! performance overlay into the document body. No configuration required.
//!
//! # Usage
//!
//! ```ignore
//! use kozan_devtools::DevTools;
//!
//! App::new()
//!     .window(WindowConfig::default(), |ctx| {
//!         // ... build your app ...
//!         DevTools::attach(ctx);
//!     })
//!     .run()
//! ```

mod metrics;
mod overlay;
mod style;

use kozan_platform::ViewContext;

/// DevTools — attach to a window for real-time performance profiling.
///
/// Chrome: DevTools Performance panel — FPS meter, frame timing breakdown,
/// pipeline phase visualization, jank detection.
pub struct DevTools;

impl DevTools {
    /// Inject the DevTools overlay into the document body.
    ///
    /// Creates a floating, draggable panel showing real-time performance
    /// metrics. Click the panel to expand full details.
    pub fn attach(ctx: &ViewContext) {
        let doc = ctx.document();
        doc.add_stylesheet(style::STYLESHEET);
        let panel = overlay::build(doc, ctx);
        doc.body().append(panel);
    }
}
