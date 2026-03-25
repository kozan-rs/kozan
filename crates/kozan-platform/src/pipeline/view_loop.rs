//! View thread event loop.
//!
//! Chrome: renderer main thread event loop — drain events, tick scheduler,
//! commit frame, park until next event or vsync.

use std::sync::mpsc;

use kozan_scheduler::Scheduler;

use crate::context::ViewContext;
use crate::event::{LifecycleEvent, ViewEvent};

/// The view thread's main loop. Returns when Shutdown is received.
pub fn run(scheduler: &mut Scheduler, ctx: &mut ViewContext, rx: &mpsc::Receiver<ViewEvent>) {
    loop {
        // 1. Drain pending events, coalescing consecutive resizes.
        //    Resize is handled via fast-path (immediate commit) so we only
        //    need the final size when multiple arrive in a single batch.
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
        //     The render thread is blocking until it receives this commit,
        //     so we must not defer it to the next scheduler tick.
        //     Only layout + paint run — resize doesn't dirty style.
        if let Some((w, h)) = pending_resize {
            ctx.on_resize(w, h);
            ctx.update_lifecycle_and_commit();
        }

        // 2. Drain user frame callbacks into scheduler.
        for cb in ctx.take_staged_frame_callbacks() {
            scheduler.request_frame(cb);
        }

        // 3. Tick — runs macrotasks, microtasks, frame callback (lifecycle + commit).
        let result = scheduler.tick(&mut |info| {
            ctx.set_last_fps(info.fps);
            ctx.update_lifecycle_and_commit();
        });

        // 4. Check if async tasks dirtied the DOM.
        if ctx.document_needs_frame() {
            ctx.invalidate_style();
            scheduler.set_needs_frame();
        }

        // 5. Store timing for next frame's FrameInfo.
        scheduler
            .frame_scheduler_mut()
            .set_frame_timing(ctx.last_frame_timing());

        // 6. Park until next event or timeout.
        //    Resize events are handled immediately (fast-path) so the
        //    render thread isn't left blocking on a stale commit.
        match result.park_timeout {
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
///
/// When the view thread is parked and a resize wakes it, we must commit
/// right away — the render thread is blocking in `wait_for_resize_commit`.
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
            ctx.invalidate_style();
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
        // Resize is handled via fast-path (dispatch_or_resize / drain loop).
        // Should not reach here, but if it does, handle it correctly.
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
