/// Pipeline timing breakdown for a single frame (milliseconds).
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTiming {
    pub style_ms: f64,
    pub layout_ms: f64,
    pub paint_ms: f64,
    pub total_ms: f64,
}
