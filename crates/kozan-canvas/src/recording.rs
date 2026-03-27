//! Canvas recording — an immutable sequence of canvas operations.
//!
//! Chrome equivalent: `cc::PaintRecord` (finalized `PaintOpBuffer`).

use crate::op::CanvasOp;

/// A completed recording of canvas operations.
///
/// Chrome equivalent: `PaintRecord` — an immutable buffer produced by
/// `CanvasRenderingContext2D::take_recording()` and consumed by the paint
/// pipeline. Replayed by backend-specific players (e.g., `VelloCanvasPlayer`).
///
/// Wrapped in `Arc` for thread-safe sharing between the UI thread
/// (where recording happens) and the render thread (where replay happens).
#[derive(Clone, Debug, Default)]
pub struct CanvasRecording {
    ops: Vec<CanvasOp>,
}

impl CanvasRecording {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(&mut self, op: CanvasOp) {
        self.ops.push(op);
    }

    #[must_use]
    pub fn ops(&self) -> &[CanvasOp] {
        &self.ops
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Takes all ops out, leaving this recording empty.
    ///
    /// Chrome equivalent: `PaintOpBuffer::ReleaseAsRecord()`.
    pub fn take(&mut self) -> CanvasRecording {
        CanvasRecording {
            ops: std::mem::take(&mut self.ops),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_primitives::geometry::Rect;
    use crate::op::ResolvedPaint;

    #[test]
    fn empty_recording() {
        let rec = CanvasRecording::new();
        assert!(rec.is_empty());
        assert_eq!(rec.len(), 0);
    }

    #[test]
    fn push_and_take() {
        let mut rec = CanvasRecording::new();
        rec.push(CanvasOp::Save);
        rec.push(CanvasOp::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            paint: ResolvedPaint::default(),
        });
        rec.push(CanvasOp::Restore);
        assert_eq!(rec.len(), 3);

        let taken = rec.take();
        assert_eq!(taken.len(), 3);
        assert!(rec.is_empty());
    }

    #[test]
    fn take_returns_correct_ops() {
        let mut rec = CanvasRecording::new();
        rec.push(CanvasOp::Translate { tx: 10.0, ty: 20.0 });
        rec.push(CanvasOp::Rotate { angle: 1.5 });

        let taken = rec.take();
        assert_eq!(taken.len(), 2);
        assert!(matches!(taken.ops()[0], CanvasOp::Translate { tx, ty } if tx == 10.0 && ty == 20.0));
        assert!(matches!(taken.ops()[1], CanvasOp::Rotate { angle } if angle == 1.5));
    }
}
