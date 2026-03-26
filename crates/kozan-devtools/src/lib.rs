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

mod chart;
mod metrics;
mod performance;
mod recorder;
mod shell;
mod style;

use kozan_platform::ViewContext;

/// DevTools — attach to a window for real-time performance profiling.
///
/// Chrome: DevTools Performance panel — FPS meter, frame timing breakdown,
/// pipeline phase visualization, jank detection, area charts.
pub struct DevTools;

impl DevTools {
    /// Inject the DevTools overlay into the document body.
    ///
    /// Creates a floating panel showing real-time performance metrics
    /// with smooth area charts. Click the badge to expand full details.
    pub fn attach(ctx: &ViewContext) {
        let doc = ctx.document();
        doc.add_stylesheet(style::STYLESHEET);
        let panel = shell::build(doc, ctx);
        doc.body().append(panel);
    }
}
