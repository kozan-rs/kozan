//! View thread event loop.
//!
//! Chrome: renderer main thread event loop — drain events, tick scheduler,
//! commit frame, park until next event or vsync.
//!
//! # Frame scheduling
//!
//! When the view thread needs a frame (dirty DOM, active frame callbacks),
//! it calls `request_redraw()` which asks the OS for a vsync-aligned
//! `RedrawRequested` event. This replaces the old `recv_timeout(budget)`
//! approach with real OS vsync signaling.
//!
//! ```text
//! [needs frame] → request_redraw() → OS schedules vsync
//!     → RedrawRequested arrives as ViewEvent::Paint
//!     → tick() produces frame
//!     → if still dirty → request_redraw() again
//! ```

use std::sync::mpsc;

use kozan_scheduler::Scheduler;

use crate::context::ViewContext;
use crate::event::{LifecycleEvent, ViewEvent};

/// The view thread's main loop. Returns when Shutdown is received.
pub fn run(scheduler: &mut Scheduler, ctx: &mut ViewContext, rx: &mpsc::Receiver<ViewEvent>) {
    // Kick off the first frame if there are callbacks registered during init.
    if ctx.has_frame_callbacks() || ctx.document_needs_frame() {
        ctx.request_redraw();
    }

    loop {
        // 1. Drain pending events, coalescing consecutive resizes.
        let mut pending_resize: Option<(u32, u32)> = None;
        while let Ok(event) = rx.try_recv() {
            match event {
                ViewEvent::Shutdown => return,
                ViewEvent::Lifecycle(LifecycleEvent::Resized { width, height }) => {
                    pending_resize = Some((width, height));
                }
                other => dispatch(scheduler, ctx, other),
            }
        }

        // 1b. Fast-path resize: layout + paint + commit immediately.
        //     The render thread is blocking until it receives this commit.
        if let Some((w, h)) = pending_resize {
            ctx.on_resize(w, h);
            ctx.update_lifecycle_and_commit();
        }

        // 2. Signal the scheduler that a frame is needed.
        if ctx.has_frame_callbacks() {
            scheduler.set_needs_frame();
        }

        // 3. Tick — macrotasks, microtasks, frame production.
        //    Chrome: rAF callbacks run in LocalFrameView with full context.
        let _ = scheduler.tick(&mut |info| {
            ctx.set_last_fps(info.fps);
            ctx.run_frame_callbacks(info);
            ctx.update_lifecycle_and_commit();
        });

        // 4. Post-tick: check if more frames are needed.
        if ctx.document_needs_frame() {
            ctx.invalidate_style();
            scheduler.set_needs_frame();
        }
        if ctx.has_frame_callbacks() {
            scheduler.set_needs_frame();
        }

        // 5. Store timing for next frame's FrameInfo.
        scheduler
            .frame_scheduler_mut()
            .set_frame_timing(ctx.last_frame_timing());

        // 6. If more frames are needed, request vsync-aligned wake.
        //    Chrome: CCScheduler calls SetNeedsBeginFrame(true) which
        //    subscribes to the display's vsync signal. Here, request_redraw()
        //    asks the OS for a RedrawRequested at the next vsync.
        let needs_next_frame = scheduler.frame_scheduler().should_produce_frame()
            || ctx.has_frame_callbacks()
            || ctx.document_needs_frame();

        if needs_next_frame {
            ctx.request_redraw();
        }

        // 7. Park until next event.
        //    With vsync-driven scheduling, we don't need recv_timeout.
        //    If a frame is needed, request_redraw() will wake us via
        //    ViewEvent::Paint. Otherwise, park until input/lifecycle event.
        let park_timeout = scheduler.park_timeout();
        match park_timeout {
            Some(t) if t.is_zero() => continue,
            Some(t) => match rx.recv_timeout(t) {
                Ok(ViewEvent::Shutdown) => return,
                Ok(event) => dispatch_or_resize(scheduler, ctx, event),
                Err(_) => {}
            },
            None => match rx.recv() {
                Ok(ViewEvent::Shutdown) => return,
                Ok(event) => dispatch_or_resize(scheduler, ctx, event),
                Err(_) => return,
            },
        }
    }
}

/// Dispatch an event, but handle resize via the immediate fast-path.
fn dispatch_or_resize(scheduler: &mut Scheduler, ctx: &mut ViewContext, event: ViewEvent) {
    if let ViewEvent::Lifecycle(LifecycleEvent::Resized { width, height }) = event {
        ctx.on_resize(width, height);
        ctx.update_lifecycle_and_commit();
    } else {
        dispatch(scheduler, ctx, event);
    }
}

fn dispatch(scheduler: &mut Scheduler, ctx: &mut ViewContext, event: ViewEvent) {
    match event {
        ViewEvent::Input(input) => {
            if ctx.on_input(input) {
                ctx.invalidate_style();
                scheduler.set_needs_frame();
            }
        }
        ViewEvent::Lifecycle(lc) => on_lifecycle(scheduler, ctx, lc),
        ViewEvent::Paint => {
            scheduler.set_needs_frame();
        }
        ViewEvent::ScrollSync(offsets) => {
            ctx.apply_scroll_sync(offsets);
            scheduler.set_needs_frame();
        }
        ViewEvent::Shutdown => unreachable!(),
    }
}

fn on_lifecycle(scheduler: &mut Scheduler, ctx: &mut ViewContext, lc: LifecycleEvent) {
    match lc {
        LifecycleEvent::Resized { width, height } => {
            ctx.on_resize(width, height);
            ctx.update_lifecycle_and_commit();
            return;
        }
        LifecycleEvent::ScaleFactorChanged {
            scale_factor,
            refresh_rate_millihertz,
        } => {
            ctx.on_scale_factor_changed(scale_factor);
            if let Some(mhz) = refresh_rate_millihertz {
                let budget = std::time::Duration::from_micros(1_000_000_000 / mhz as u64);
                scheduler.frame_scheduler_mut().set_frame_budget(budget);
            }
            ctx.invalidate_style();
        }
        LifecycleEvent::Focused(focused) => {
            ctx.on_focus_changed(focused);
        }
    }
    scheduler.set_needs_frame();
}
