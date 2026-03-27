//! View context — the user-facing API inside a view thread.
//!
//! Like Chrome's `LocalFrame` + `LocalDOMWindow` — the entry point for
//! accessing the document, spawning async work, and communicating
//! back to the main thread.
//!
//! Zero windowing-backend knowledge. Communicates through `PlatformHost` trait.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;

use kozan_core::Document;
use kozan_core::compositor::layer_tree::LayerTree;
use kozan_core::paint::DisplayList;
use kozan_core::scroll::ScrollOffsets;
use kozan_core::scroll::ScrollTree;
use kozan_core::page::Page;
use kozan_scheduler::WakeSender;

use crate::host::PlatformHost;
use crate::id::WindowId;
use crate::pipeline::render_loop::RenderEvent;

/// Pinned boxed future that can live on the view thread.
///
/// `!Send` is fine — these are spawned into the `LocalExecutor` which
/// only runs on the view thread. `'static` so captured DOM handles
/// (which are `'static` index types) can cross `.await` points.
pub(crate) type StagedFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

/// A frame callback — like `requestAnimationFrame(callback)`.
///
/// Chrome: rAF callbacks run inside `LocalFrameView` with full access to
/// `document` and `window`. Same here — callbacks receive `&ViewContext`
/// so they can query Document, Viewport, zoom, etc.
///
/// Returns `true` to keep for next frame (loop), `false` for one-shot.
pub type FrameCallback =
    Box<dyn FnMut(kozan_scheduler::FrameInfo, &ViewContext) -> bool + 'static>;

/// Everything the view thread produces per frame — posted to the render thread.
///
/// Chrome: `LayerTreeHost::FinishCommit()` posts this to the compositor's
/// task queue. Ownership transfers — no shared state, no mutex.
pub struct FrameOutput {
    pub display_list: Arc<DisplayList>,
    pub layer_tree: LayerTree,
    pub scroll_tree: ScrollTree,
    /// Viewport size (physical pixels) this frame was laid out at.
    pub viewport_width: u32,
    pub viewport_height: u32,
    /// Page zoom factor at commit time. The render thread combines this
    /// with device_scale_factor to get the total content scale.
    pub page_zoom_factor: f64,
}

/// The user-facing API inside a view.
///
/// Passed to the view's init closure. Provides access to:
/// - The document (DOM tree, via `Page`)
/// - The cross-thread sender (for giving to background tasks)
/// - The platform host (for requesting redraws, setting title, etc.)
/// - The window identity (which window this view belongs to)
///
/// # Example
///
/// ```ignore
/// app.window(WindowConfig::default(), |ctx| {
///     let doc = ctx.document();
///     let btn = doc.create::<HtmlButtonElement>();
///     btn.set_text("Hello!");
///     doc.root().append(btn);
/// });
/// ```
pub struct ViewContext {
    /// The engine entry point — DOM, layout, paint.
    /// Chrome equivalent: `Page` + `LocalFrame` + `LocalFrameView`.
    page: Page,

    wake_sender: WakeSender,
    host: Arc<dyn PlatformHost>,
    window_id: WindowId,

    /// Channel to post frames to the render thread.
    /// Chrome: `ProxyMain` posts commits to compositor's task queue.
    render_sender: mpsc::Sender<RenderEvent>,

    /// Futures queued via `spawn()` during the init closure.
    ///
    /// After init returns, `run_view_thread` drains these into the
    /// `LocalExecutor`. Using `Rc<RefCell<...>>` (not Arc/Mutex) because
    /// `ViewContext` is `!Send` and lives entirely on the view thread.
    staged_futures: Rc<RefCell<Vec<StagedFuture>>>,

    /// Chrome: `LocalFrameView::frame_request_callbacks_`.
    /// Callbacks registered via `request_frame()`. Run each frame with
    /// `&ViewContext` so they can access Document, Viewport, etc.
    frame_callbacks: RefCell<Vec<FrameCallback>>,

    /// Callbacks registered DURING a frame (inside another callback).
    /// Moved to `frame_callbacks` after the current frame's callbacks finish.
    pending_frame_callbacks: RefCell<Vec<FrameCallback>>,

    /// Last computed FPS — updated each frame by the scheduler.
    /// Shared via `Rc<Cell>` so async tasks can read it.
    last_fps: Rc<std::cell::Cell<f64>>,
}

impl ViewContext {
    /// Create a new view context. Called internally by the view thread.
    pub(crate) fn new(
        page: Page,
        wake_sender: WakeSender,
        host: Arc<dyn PlatformHost>,
        window_id: WindowId,
        render_sender: mpsc::Sender<RenderEvent>,
    ) -> Self {
        Self {
            page,
            wake_sender,
            host,
            window_id,
            render_sender,
            staged_futures: Rc::new(RefCell::new(Vec::new())),
            frame_callbacks: RefCell::new(Vec::new()),
            pending_frame_callbacks: RefCell::new(Vec::new()),
            last_fps: Rc::new(std::cell::Cell::new(0.0)),
        }
    }

    /// Read-only access to the document.
    #[inline]
    pub fn document(&self) -> &Document {
        self.page.document()
    }

    /// Register custom font data (TTF/OTF/TTC bytes) into the font system.
    ///
    /// Chrome equivalent: `document.fonts.add(new FontFace(...))`.
    /// After registration, the font's family name is available for CSS
    /// `font-family` matching. The family name is auto-detected from
    /// the font file's `name` table.
    ///
    /// Accepts `&'static [u8]` (zero-copy for `include_bytes!()`),
    /// `Vec<u8>` (runtime-loaded), or `Arc<[u8]>` (pre-shared).
    ///
    /// Returns the registered family names.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Static — zero copy:
    /// ctx.register_font(include_bytes!("../assets/Cairo.ttf") as &[u8]);
    ///
    /// // Runtime — from a file:
    /// ctx.register_font(std::fs::read("font.ttf").unwrap());
    /// ```
    pub fn register_font(
        &self,
        data: impl Into<kozan_core::layout::inline::font_system::FontBlob>,
    ) -> Vec<String> {
        self.page.font_system().register_font(data)
    }

    /// Spawn a `!Send` async task on the view thread's executor.
    ///
    /// The future runs on the view thread — it can safely capture and
    /// mutate DOM handles across `.await` points. No `Arc`, no `Mutex`,
    /// no `WakeSender` needed.
    ///
    /// ```ignore
    /// ctx.spawn(async move {
    ///     kozan_platform::time::sleep(Duration::from_millis(500)).await;
    ///     card.set_style(activated_style());
    /// });
    /// ```
    ///
    /// If called during the init closure the future is queued and started
    /// on the first scheduler tick. If called later (e.g. from an event
    /// handler posted via `WakeSender`) it is spawned into the executor
    /// immediately — use `WakeSender::post` for that case.
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        self.staged_futures.borrow_mut().push(Box::pin(future));
    }

    /// Drain futures queued by `spawn()` during init.
    ///
    /// Called by `run_view_thread` after the init closure returns.
    pub(crate) fn take_staged_futures(&self) -> Vec<StagedFuture> {
        self.staged_futures.borrow_mut().drain(..).collect()
    }

    /// Whether there are frame callbacks waiting to run.
    ///
    /// Checks BOTH the main buffer and pending buffer — callbacks
    /// registered during init go to pending and must still trigger
    /// frame production.
    pub(crate) fn has_frame_callbacks(&self) -> bool {
        !self.frame_callbacks.borrow().is_empty()
            || !self.pending_frame_callbacks.borrow().is_empty()
    }

    /// Chrome: `LocalFrameView::RunAnimationFrameCallbacks()`.
    ///
    /// Runs all registered frame callbacks with `&self` so they can access
    /// Document, Viewport, zoom, etc. Callbacks that return `true` are kept
    /// for the next frame. Uses take-call-put to allow callbacks to call
    /// `request_frame` (new registrations go to pending, merged after).
    pub(crate) fn run_frame_callbacks(&self, info: kozan_scheduler::FrameInfo) {
        // 1. Merge pending (from init or previous frame's request_frame calls).
        self.frame_callbacks
            .borrow_mut()
            .append(&mut *self.pending_frame_callbacks.borrow_mut());

        // 2. Take all callbacks — leaves frame_callbacks empty so
        //    request_frame() during execution goes to pending.
        let mut cbs = std::mem::take(&mut *self.frame_callbacks.borrow_mut());

        // 3. Run. Callbacks that return true are kept for next frame.
        cbs.retain_mut(|cb| cb(info, self));

        // 4. Put survivors back.
        *self.frame_callbacks.borrow_mut() = cbs;
    }

    /// Get a clone of the cross-thread sender.
    ///
    /// Give this to background threads so they can send results
    /// back to this view's scheduler.
    #[inline]
    pub fn wake_sender(&self) -> WakeSender {
        self.wake_sender.clone()
    }

    /// Request a redraw for this view's window.
    pub fn request_redraw(&self) {
        self.host.request_redraw(self.window_id);
    }

    /// Set the window title.
    pub fn set_title(&self, title: &str) {
        self.host.set_title(self.window_id, title);
    }

    /// Close this view's window.
    pub fn close_window(&self) {
        self.host.close_window(self.window_id);
    }

    /// The `WindowId` this view belongs to.
    #[inline]
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Access to window and app-level operations.
    ///
    /// Chrome: `chrome.devtools.inspectedWindow` — elevated access
    /// beyond the page's own context. Commands (create/close/resize window)
    /// and queries (window count, renderer name).
    #[inline]
    pub fn platform(&self) -> &dyn PlatformHost {
        &*self.host
    }

    /// Current FPS — updated each frame by the scheduler.
    ///
    /// Returns 0.0 on the first frame. Use this to build FPS overlays.
    ///
    /// ```ignore
    /// let fps_rc = ctx.fps_cell();
    /// ctx.spawn(async move {
    ///     loop {
    ///         sleep(Duration::from_millis(200)).await;
    ///         label.set_text(&format!("{:.0} FPS", fps_rc.get()));
    ///     }
    /// });
    /// ```
    #[inline]
    pub fn fps(&self) -> f64 {
        self.last_fps.get()
    }

    /// Returns the shared FPS counter — callers may read or write.
    #[inline]
    pub fn fps_cell(&self) -> Rc<std::cell::Cell<f64>> {
        Rc::clone(&self.last_fps)
    }

    /// Register a frame callback — like `requestAnimationFrame`.
    ///
    /// Chrome: rAF callbacks run inside `LocalFrameView` with full access
    /// to `document` and `window`. Same here — callbacks receive
    /// `&ViewContext` so they can read Document, Viewport, zoom, etc.
    ///
    /// Returns `bool`: `true` = keep for next frame, `false` = one-shot.
    ///
    /// ```ignore
    /// // One-shot:
    /// ctx.request_frame(|_info, _ctx| { do_something(); false });
    ///
    /// // Render loop with live viewport access:
    /// ctx.request_frame(move |info, ctx| {
    ///     let zoom = ctx.page_zoom();
    ///     let nodes = ctx.document().node_count();
    ///     label.set_content(format!("{:.0} FPS | zoom {:.0}%", info.fps, zoom * 100.0));
    ///     true
    /// });
    /// ```
    pub fn request_frame(
        &self,
        callback: impl FnMut(kozan_scheduler::FrameInfo, &ViewContext) -> bool + 'static,
    ) {
        // During `run_frame_callbacks`, `frame_callbacks` is taken then
        // restored. New registrations during execution go to pending
        // and are merged after the current batch finishes.
        self.pending_frame_callbacks
            .borrow_mut()
            .push(Box::new(callback));
    }

    // ── Viewport & display ────────────────────────────────────────────────

    /// Chrome: `window.devicePixelRatio`.
    #[inline]
    pub fn device_pixel_ratio(&self) -> f64 {
        self.page.viewport().scale_factor()
    }

    /// Chrome: Ctrl+/- page zoom level (1.0 = 100%).
    #[inline]
    pub fn page_zoom(&self) -> f64 {
        self.page.page_zoom()
    }

    /// Read-only snapshot of the viewport.
    ///
    /// Chrome: `VisualProperties` — physical size, DPI, zoom, logical size.
    #[inline]
    pub fn viewport(&self) -> &kozan_core::page::Viewport {
        self.page.viewport()
    }

    // ── Internal (view thread only) ───────────────────────────────────────

    /// Update FPS from the scheduler's frame info.
    pub(crate) fn set_last_fps(&self, fps: f64) {
        self.last_fps.set(fps);
    }

    /// Previous frame's pipeline timing.
    pub(crate) fn last_frame_timing(&self) -> kozan_primitives::timing::FrameTiming {
        self.page.last_timing()
    }

    /// Check if the document has pending changes that need a frame.
    ///
    /// Called after `scheduler.tick()` — spawned tasks may have mutated the DOM
    /// (e.g. `style().w(pct(...))`) without requesting a frame.
    /// Chrome equivalent: checking `Document::NeedsStyleRecalc()` after microtask checkpoint.
    pub(crate) fn document_needs_frame(&self) -> bool {
        self.page.document().needs_visual_update()
    }

    /// Apply scroll offsets received from the compositor.
    /// Chrome: main thread applies scroll deltas posted from compositor thread.
    pub(crate) fn apply_scroll_sync(&mut self, offsets: ScrollOffsets) {
        self.page.apply_compositor_scroll(&offsets);
    }

    /// Run style → layout → paint, then post the result to the render thread.
    /// Chrome: `LocalFrameView::UpdateLifecyclePhases()` then `FinishCommit()`.
    pub(crate) fn update_lifecycle_and_commit(&mut self) {
        self.page.update_lifecycle();

        let dl = self.page.last_display_list();
        let layer_tree = self.page.take_layer_tree();

        if let (Some(dl), Some(tree)) = (dl, layer_tree) {
            let (scroll_tree, _scroll_offsets) = self.page.scroll_state_snapshot();
            let _ = self.render_sender.send(RenderEvent::Commit(FrameOutput {
                display_list: dl,
                layer_tree: tree,
                scroll_tree,
                viewport_width: self.page.viewport().width(),
                viewport_height: self.page.viewport().height(),
                page_zoom_factor: self.page.page_zoom(),
            }));
        }
    }

    /// Process an input event — hit test + DOM dispatch.
    ///
    /// Returns `true` if DOM state changed (hover/focus/active changed,
    /// or event listeners were dispatched that may have mutated the DOM).
    ///
    /// Chrome: browser-level shortcuts (Ctrl+/-, Ctrl+0) are intercepted
    /// before reaching Blink. Same here — zoom never hits DOM dispatch.
    pub(crate) fn on_input(&mut self, mut input: kozan_core::InputEvent) -> bool {
        if let Some(handled) = self.try_browser_shortcut(&input) {
            return handled;
        }
        input.apply_page_zoom(self.page.page_zoom());
        let (state_changed, scroll_action) = self.page.handle_input(input);

        if let Some((target, delta)) = scroll_action {
            let _ = self.render_sender.send(RenderEvent::ScrollTo { target, delta });
        }

        state_changed
    }

    /// Browser-level keyboard shortcuts intercepted before DOM dispatch.
    fn try_browser_shortcut(&mut self, input: &kozan_core::InputEvent) -> Option<bool> {
        use kozan_core::input::keyboard::KeyboardEvent;
        use kozan_core::{ButtonState, KeyCode};

        let kozan_core::InputEvent::Keyboard(KeyboardEvent {
            physical_key,
            state: ButtonState::Pressed,
            modifiers,
            ..
        }) = input
        else {
            return None;
        };

        if !modifiers.ctrl() {
            return None;
        }

        const ZOOM_STEP: f64 = 0.1;
        const MIN_ZOOM: f64 = 0.25;
        const MAX_ZOOM: f64 = 5.0;

        match physical_key {
            KeyCode::Equal => {
                let new = (self.page.page_zoom() + ZOOM_STEP).min(MAX_ZOOM);
                self.page.set_page_zoom(new);
                Some(true)
            }
            KeyCode::Minus => {
                let new = (self.page.page_zoom() - ZOOM_STEP).max(MIN_ZOOM);
                self.page.set_page_zoom(new);
                Some(true)
            }
            KeyCode::Digit0 => {
                self.page.set_page_zoom(1.0);
                Some(true)
            }
            _ => None,
        }
    }

    /// Mark that styles need recalculation.
    ///
    /// Called when `ElementState` changes (hover, focus, active) or when
    /// event listeners may have mutated the DOM. Chrome equivalent:
    /// `Document::SetNeedsStyleRecalc()`.
    pub(crate) fn invalidate_style(&mut self) {
        self.page.mark_needs_update();
    }

    /// Notify the frame of a resize event.
    pub(crate) fn on_resize(&mut self, width: u32, height: u32) {
        self.page.resize(width, height);
    }

    /// Notify the frame of a scale factor change.
    pub(crate) fn on_scale_factor_changed(&mut self, factor: f64) {
        self.page.set_scale_factor(factor);
    }

    /// Window gained or lost focus.
    /// On blur: clears element focus (dispatch blur/focusout events).
    /// On focus: no-op (previously focused element stays focused — Chrome behavior).
    pub(crate) fn on_focus_changed(&mut self, focused: bool) {
        self.page.set_window_focused(focused);
    }
}
