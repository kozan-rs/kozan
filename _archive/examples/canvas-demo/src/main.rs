//! Canvas 2D Stress Test — animated, comprehensive feature coverage.
//!
//! Tests every Canvas 2D API method against a browser HTML reference.
//! Open `assets/canvas-demo.html` in Chrome for pixel-perfect comparison.
//!
//! Pipeline: recording → flush → committed → fragment → display list → vello replay.

use std::f32::consts::PI;

use kozan::prelude::*;
use kozan_canvas::style::PaintStyle;

const W: f32 = 300.0;
const H: f32 = 200.0;

fn main() -> kozan::Result<()> {
    App::new()
        .window(
            WindowConfig {
                title: "Canvas 2D Stress Test".into(),
                width: 1280,
                height: 900,
                ..Default::default()
            },
            build_ui,
        )
        .run()
}

fn build_ui(ctx: &ViewContext) {
    let doc = ctx.document();
    doc.add_stylesheet(include_str!("../assets/style.css"));

    let root = doc.div();
    root.class_add("root");

    let header = doc.div();
    header.class_add("header");
    header.append(doc.create_text("Canvas 2D Stress Test"));
    root.child(header);

    let grid = doc.div();
    grid.class_add("grid");

    // ── Static scenes ────────────────────────────────────
    scene_filled_rects(doc, &grid);
    scene_stroked_rects(doc, &grid);
    scene_paths(doc, &grid);
    scene_circles(doc, &grid);
    scene_bezier_curves(doc, &grid);
    scene_transforms(doc, &grid);
    scene_alpha(doc, &grid);
    scene_gradient(doc, &grid);
    scene_clip(doc, &grid);
    scene_line_styles(doc, &grid);
    scene_save_restore(doc, &grid);

    // ── Animated scenes ──────────────────────────────────
    let anim1 = make_canvas(doc, &grid, "12. Animated Spinner");
    let anim2 = make_canvas(doc, &grid, "13. Animated Wave");
    let anim3 = make_canvas(doc, &grid, "14. Animated Particles");

    root.child(grid);
    doc.body().child(root);

    kozan_devtools::DevTools::attach(ctx);

    // Animation loop — redraws 3 canvases every frame.
    ctx.request_frame(move |info, _ctx| {
        let t = info.frame_number as f32 * 0.02;
        draw_spinner(anim1, t);
        draw_wave(anim2, t);
        draw_particles(anim3, t, info.frame_number);
        true
    });
}

// ── Helpers ──────────────────────────────────────────────────────────

fn make_canvas(doc: &Document, grid: &HtmlDivElement, title: &str) -> Canvas2D {
    let card = doc.div();
    card.class_add("card");
    let title_el = doc.div();
    title_el.class_add("card-title");
    title_el.append(doc.create_text(title));
    card.child(title_el);

    let canvas = doc.create::<HtmlCanvasElement>();
    canvas.set_canvas_width(W);
    canvas.set_canvas_height(H);
    canvas.class_add("scene-canvas");
    card.child(canvas);
    grid.child(card);
    canvas.context_2d()
}

// ── Scene 1: Filled Rectangles ──────────────────────────────────────

fn scene_filled_rects(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "1. Filled Rectangles");
    let palette = [
        Color::from_rgb8(231, 76, 60),
        Color::from_rgb8(46, 204, 113),
        Color::from_rgb8(52, 152, 219),
        Color::from_rgb8(241, 196, 15),
        Color::from_rgb8(155, 89, 182),
        Color::from_rgb8(230, 126, 34),
        Color::from_rgb8(26, 188, 156),
        Color::from_rgb8(44, 62, 80),
    ];
    for (i, &color) in palette.iter().enumerate() {
        let col = (i % 4) as f32;
        let row = (i / 4) as f32;
        let x = 10.0 + col * 72.0;
        let y = 15.0 + row * 90.0;
        ctx.set_fill_color(color);
        ctx.fill_rect(x, y, 62.0, 80.0);
    }
}

// ── Scene 2: Stroked Rectangles ─────────────────────────────────────

fn scene_stroked_rects(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "2. Stroked Rectangles");
    let items: [(f32, Color); 4] = [
        (1.0, Color::from_rgb8(231, 76, 60)),
        (2.0, Color::from_rgb8(46, 204, 113)),
        (3.0, Color::from_rgb8(52, 152, 219)),
        (5.0, Color::from_rgb8(241, 196, 15)),
    ];
    for (i, &(lw, color)) in items.iter().enumerate() {
        let x = 12.0 + i as f32 * 72.0;
        ctx.set_stroke_color(color);
        ctx.set_line_width(lw);
        ctx.stroke_rect(x, 20.0, 60.0, 160.0);
    }
}

// ── Scene 3: Path Shapes ────────────────────────────────────────────

fn scene_paths(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "3. Paths (triangle, star, arrow)");

    // Filled triangle
    ctx.set_fill_color(Color::from_rgb8(46, 204, 113));
    ctx.begin_path();
    ctx.move_to(50.0, 10.0);
    ctx.line_to(95.0, 90.0);
    ctx.line_to(5.0, 90.0);
    ctx.close_path();
    ctx.fill();

    // Stroked triangle
    ctx.set_stroke_color(Color::from_rgb8(231, 76, 60));
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(50.0, 110.0);
    ctx.line_to(95.0, 190.0);
    ctx.line_to(5.0, 190.0);
    ctx.close_path();
    ctx.stroke();

    // 5-pointed star
    ctx.set_fill_color(Color::from_rgb8(241, 196, 15));
    ctx.begin_path();
    let (cx, cy, outer, inner) = (165.0, 55.0, 42.0, 18.0);
    for i in 0..10 {
        let angle = -PI / 2.0 + i as f32 * PI / 5.0;
        let r = if i % 2 == 0 { outer } else { inner };
        let x = cx + angle.cos() * r;
        let y = cy + angle.sin() * r;
        if i == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
    }
    ctx.close_path();
    ctx.fill();

    // Arrow shape
    ctx.set_fill_color(Color::from_rgb8(52, 152, 219));
    ctx.begin_path();
    ctx.move_to(210.0, 130.0);
    ctx.line_to(280.0, 160.0);
    ctx.line_to(210.0, 190.0);
    ctx.line_to(210.0, 175.0);
    ctx.line_to(130.0, 175.0);
    ctx.line_to(130.0, 145.0);
    ctx.line_to(210.0, 145.0);
    ctx.close_path();
    ctx.fill();
}

// ── Scene 4: Circles & Arcs ────────────────────────────────────────

fn scene_circles(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "4. Circles & Arcs");

    // Full circles
    let circles = [
        (50.0, 55.0, 35.0, Color::from_rgb8(231, 76, 60)),
        (130.0, 55.0, 30.0, Color::from_rgb8(52, 152, 219)),
        (210.0, 55.0, 40.0, Color::from_rgb8(46, 204, 113)),
    ];
    for &(cx, cy, r, color) in &circles {
        ctx.set_fill_color(color);
        ctx.begin_path();
        ctx.arc(cx, cy, r, 0.0, PI * 2.0, false);
        ctx.fill();
    }

    // Partial arcs (stroked)
    let arcs = [
        (60.0, 150.0, 30.0, 0.0, PI * 0.75, Color::from_rgb8(155, 89, 182)),
        (150.0, 150.0, 30.0, PI * 0.25, PI * 1.5, Color::from_rgb8(230, 126, 34)),
        (240.0, 150.0, 30.0, 0.0, PI * 1.75, Color::from_rgb8(26, 188, 156)),
    ];
    for &(cx, cy, r, start, end, color) in &arcs {
        ctx.set_stroke_color(color);
        ctx.set_line_width(3.0);
        ctx.begin_path();
        ctx.arc(cx, cy, r, start, end, false);
        ctx.stroke();
    }
}

// ── Scene 5: Bezier Curves ─────────────────────────────────────────

fn scene_bezier_curves(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "5. Bezier Curves");

    // Quadratic bezier
    ctx.set_stroke_color(Color::from_rgb8(231, 76, 60));
    ctx.set_line_width(3.0);
    ctx.begin_path();
    ctx.move_to(20.0, 80.0);
    ctx.quadratic_curve_to(75.0, 10.0, 130.0, 80.0);
    ctx.stroke();

    // Another quadratic
    ctx.set_stroke_color(Color::from_rgb8(46, 204, 113));
    ctx.begin_path();
    ctx.move_to(20.0, 120.0);
    ctx.quadratic_curve_to(75.0, 190.0, 130.0, 120.0);
    ctx.stroke();

    // Cubic bezier
    ctx.set_stroke_color(Color::from_rgb8(52, 152, 219));
    ctx.set_line_width(3.0);
    ctx.begin_path();
    ctx.move_to(160.0, 30.0);
    ctx.bezier_curve_to(180.0, 180.0, 260.0, 20.0, 280.0, 170.0);
    ctx.stroke();

    // S-curve
    ctx.set_stroke_color(Color::from_rgb8(241, 196, 15));
    ctx.begin_path();
    ctx.move_to(160.0, 100.0);
    ctx.bezier_curve_to(200.0, 40.0, 240.0, 160.0, 280.0, 100.0);
    ctx.stroke();
}

// ── Scene 6: Transforms ────────────────────────────────────────────

fn scene_transforms(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "6. Transforms");

    // Rotated fan of rectangles
    let colors = [
        Color::from_rgb8(231, 76, 60),
        Color::from_rgb8(46, 204, 113),
        Color::from_rgb8(52, 152, 219),
        Color::from_rgb8(241, 196, 15),
        Color::from_rgb8(155, 89, 182),
        Color::from_rgb8(230, 126, 34),
        Color::from_rgb8(26, 188, 156),
    ];
    for (i, &color) in colors.iter().enumerate() {
        ctx.save();
        ctx.translate(90.0, 100.0);
        ctx.rotate(i as f32 * PI / 14.0);
        ctx.set_global_alpha(0.6);
        ctx.set_fill_color(color);
        ctx.fill_rect(-50.0, -10.0, 100.0, 20.0);
        ctx.restore();
    }

    // Scaled nested squares
    ctx.save();
    ctx.translate(220.0, 100.0);
    for i in 0..5 {
        let s = 1.0 - i as f32 * 0.15;
        ctx.save();
        ctx.scale(s, s);
        let a = (i as f32 + 1.0) / 6.0;
        ctx.set_fill_color(Color::rgba(0.341, 0.706, 0.973, a));
        ctx.fill_rect(-40.0, -40.0, 80.0, 80.0);
        ctx.restore();
    }
    ctx.restore();
}

// ── Scene 7: Alpha Blending ────────────────────────────────────────

fn scene_alpha(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "7. Alpha Blending");

    // Gradient of alpha values
    for i in 0..10 {
        let alpha = (i as f32 + 1.0) / 10.0;
        ctx.set_global_alpha(alpha);
        ctx.set_fill_color(Color::from_rgb8(52, 152, 219));
        ctx.fill_rect(10.0 + i as f32 * 28.0, 15.0, 24.0, 60.0);
    }
    ctx.set_global_alpha(1.0);

    // Overlapping transparent circles
    ctx.set_global_alpha(0.5);
    let overlap = [
        (80.0, 145.0, 40.0, Color::from_rgb8(231, 76, 60)),
        (120.0, 145.0, 40.0, Color::from_rgb8(46, 204, 113)),
        (100.0, 115.0, 40.0, Color::from_rgb8(52, 152, 219)),
    ];
    for &(cx, cy, r, color) in &overlap {
        ctx.set_fill_color(color);
        ctx.begin_path();
        ctx.arc(cx, cy, r, 0.0, PI * 2.0, false);
        ctx.fill();
    }
    ctx.set_global_alpha(1.0);

    // Alpha rectangles overlapping
    let rects = [
        (180.0, 100.0, 80.0, 70.0, Color::from_rgb8(241, 196, 15)),
        (210.0, 115.0, 80.0, 70.0, Color::from_rgb8(155, 89, 182)),
    ];
    ctx.set_global_alpha(0.4);
    for &(x, y, w, h, color) in &rects {
        ctx.set_fill_color(color);
        ctx.fill_rect(x, y, w, h);
    }
    ctx.set_global_alpha(1.0);
}

// ── Scene 8: Gradients ─────────────────────────────────────────────

fn scene_gradient(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "8. Gradients");

    // Horizontal linear gradient
    let mut h_grad = Canvas2D::create_linear_gradient(10.0, 0.0, 140.0, 0.0);
    h_grad.add_color_stop(0.0, Color::from_rgb8(231, 76, 60));
    h_grad.add_color_stop(0.5, Color::from_rgb8(241, 196, 15));
    h_grad.add_color_stop(1.0, Color::from_rgb8(46, 204, 113));
    ctx.set_fill_style(PaintStyle::LinearGradient(h_grad));
    ctx.fill_rect(10.0, 10.0, 130.0, 80.0);

    // Vertical linear gradient
    let mut v_grad = Canvas2D::create_linear_gradient(0.0, 10.0, 0.0, 90.0);
    v_grad.add_color_stop(0.0, Color::from_rgb8(52, 152, 219));
    v_grad.add_color_stop(1.0, Color::from_rgb8(155, 89, 182));
    ctx.set_fill_style(PaintStyle::LinearGradient(v_grad));
    ctx.fill_rect(155.0, 10.0, 130.0, 80.0);

    // Diagonal gradient
    let mut d_grad = Canvas2D::create_linear_gradient(10.0, 105.0, 285.0, 190.0);
    d_grad.add_color_stop(0.0, Color::from_rgb8(26, 188, 156));
    d_grad.add_color_stop(0.33, Color::from_rgb8(52, 152, 219));
    d_grad.add_color_stop(0.66, Color::from_rgb8(155, 89, 182));
    d_grad.add_color_stop(1.0, Color::from_rgb8(231, 76, 60));
    ctx.set_fill_style(PaintStyle::LinearGradient(d_grad));
    ctx.fill_rect(10.0, 105.0, 275.0, 85.0);
}

// ── Scene 9: Clipping ──────────────────────────────────────────────

fn scene_clip(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "9. Clipping");

    // Clip to circle, then draw stripes
    ctx.save();
    ctx.begin_path();
    ctx.arc(80.0, 100.0, 60.0, 0.0, PI * 2.0, false);
    ctx.clip();

    let colors = [
        Color::from_rgb8(231, 76, 60),
        Color::from_rgb8(241, 196, 15),
        Color::from_rgb8(46, 204, 113),
        Color::from_rgb8(52, 152, 219),
        Color::from_rgb8(155, 89, 182),
    ];
    for (i, &color) in colors.iter().enumerate() {
        ctx.set_fill_color(color);
        ctx.fill_rect(20.0 + i as f32 * 24.0, 40.0, 24.0, 120.0);
    }
    ctx.restore();

    // Clip to rectangle, then draw circles
    ctx.save();
    ctx.begin_path();
    ctx.rect(175.0, 50.0, 110.0, 100.0);
    ctx.clip();

    ctx.set_fill_color(Color::from_rgb8(231, 76, 60));
    ctx.begin_path();
    ctx.arc(200.0, 80.0, 50.0, 0.0, PI * 2.0, false);
    ctx.fill();

    ctx.set_fill_color(Color::from_rgb8(52, 152, 219));
    ctx.begin_path();
    ctx.arc(260.0, 120.0, 50.0, 0.0, PI * 2.0, false);
    ctx.fill();
    ctx.restore();
}

// ── Scene 10: Line Styles ──────────────────────────────────────────

fn scene_line_styles(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "10. Line Styles & Dashes");

    // Different line widths
    let widths = [1.0, 2.0, 4.0, 6.0, 8.0];
    ctx.set_stroke_color(Color::from_rgb8(52, 152, 219));
    for (i, &w) in widths.iter().enumerate() {
        ctx.set_line_width(w);
        ctx.begin_path();
        ctx.move_to(20.0, 20.0 + i as f32 * 18.0);
        ctx.line_to(130.0, 20.0 + i as f32 * 18.0);
        ctx.stroke();
    }

    // Dashed lines
    ctx.set_stroke_color(Color::from_rgb8(231, 76, 60));
    ctx.set_line_width(2.0);
    ctx.set_line_dash(vec![10.0, 5.0]);
    ctx.begin_path();
    ctx.move_to(20.0, 120.0);
    ctx.line_to(130.0, 120.0);
    ctx.stroke();

    ctx.set_line_dash(vec![15.0, 5.0, 5.0, 5.0]);
    ctx.begin_path();
    ctx.move_to(20.0, 140.0);
    ctx.line_to(130.0, 140.0);
    ctx.stroke();

    ctx.set_line_dash(vec![2.0, 4.0]);
    ctx.begin_path();
    ctx.move_to(20.0, 160.0);
    ctx.line_to(130.0, 160.0);
    ctx.stroke();

    ctx.set_line_dash(vec![]);

    // Zigzag path with thick stroke
    ctx.set_stroke_color(Color::from_rgb8(46, 204, 113));
    ctx.set_line_width(3.0);
    ctx.begin_path();
    ctx.move_to(160.0, 30.0);
    for i in 0..8 {
        let x = 160.0 + (i + 1) as f32 * 16.0;
        let y = if i % 2 == 0 { 70.0 } else { 30.0 };
        ctx.line_to(x, y);
    }
    ctx.stroke();

    // Spiral-ish path
    ctx.set_stroke_color(Color::from_rgb8(155, 89, 182));
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.move_to(220.0, 140.0);
    for i in 0..30 {
        let t = i as f32 * 0.3;
        let r = 5.0 + t * 5.0;
        let x = 220.0 + t.cos() * r;
        let y = 140.0 + t.sin() * r;
        ctx.line_to(x, y);
    }
    ctx.stroke();
}

// ── Scene 11: Save / Restore State Stack ───────────────────────────

fn scene_save_restore(doc: &Document, grid: &HtmlDivElement) {
    let ctx = make_canvas(doc, grid, "11. Save / Restore State");

    // Nested save/restore with different styles
    ctx.set_fill_color(Color::from_rgb8(44, 62, 80));
    ctx.fill_rect(10.0, 10.0, 280.0, 180.0);

    ctx.save();
    ctx.set_fill_color(Color::from_rgb8(231, 76, 60));
    ctx.set_global_alpha(0.8);
    ctx.fill_rect(20.0, 20.0, 120.0, 70.0);

    ctx.save();
    ctx.set_fill_color(Color::from_rgb8(46, 204, 113));
    ctx.set_global_alpha(0.6);
    ctx.fill_rect(40.0, 40.0, 120.0, 70.0);

    ctx.save();
    ctx.set_fill_color(Color::from_rgb8(52, 152, 219));
    ctx.set_global_alpha(0.4);
    ctx.fill_rect(60.0, 60.0, 120.0, 70.0);

    // Restore back through the stack
    ctx.restore(); // back to green state
    ctx.fill_rect(160.0, 20.0, 60.0, 40.0);

    ctx.restore(); // back to red state
    ctx.fill_rect(160.0, 70.0, 60.0, 40.0);

    ctx.restore(); // back to dark background state
    ctx.fill_rect(160.0, 120.0, 60.0, 40.0);

    // After all restores, default style
    ctx.set_fill_color(Color::from_rgb8(241, 196, 15));
    ctx.fill_rect(230.0, 20.0, 50.0, 140.0);
}

// ── Animated Scene 12: Spinner ──────────────────────────────────────

fn draw_spinner(ctx: Canvas2D, t: f32) {
    ctx.clear_rect(0.0, 0.0, W, H);

    let cx = W / 2.0;
    let cy = H / 2.0;

    // Spinning arcs
    for i in 0..8 {
        let angle = t + i as f32 * PI / 4.0;
        let alpha = (i as f32 + 1.0) / 8.0;
        ctx.save();
        ctx.translate(cx, cy);
        ctx.rotate(angle);
        ctx.set_global_alpha(alpha);
        ctx.set_fill_color(Color::from_rgb8(52, 152, 219));
        ctx.fill_rect(30.0, -5.0, 30.0, 10.0);
        ctx.restore();
    }

    // Center circle
    ctx.set_global_alpha(1.0);
    ctx.set_fill_color(Color::from_rgb8(231, 76, 60));
    ctx.begin_path();
    ctx.arc(cx, cy, 15.0, 0.0, PI * 2.0, false);
    ctx.fill();

    // Orbiting dot
    let ox = cx + (t * 1.5).cos() * 70.0;
    let oy = cy + (t * 1.5).sin() * 70.0;
    ctx.set_fill_color(Color::from_rgb8(46, 204, 113));
    ctx.begin_path();
    ctx.arc(ox, oy, 8.0, 0.0, PI * 2.0, false);
    ctx.fill();
}

// ── Animated Scene 13: Wave ─────────────────────────────────────────

fn draw_wave(ctx: Canvas2D, t: f32) {
    ctx.clear_rect(0.0, 0.0, W, H);
    let mid_y = H / 2.0;

    // Draw 3 overlapping waves
    let waves: [(Color, f32, f32, f32); 3] = [
        (Color::from_rgb8(231, 76, 60), 40.0, 3.0, 0.0),
        (Color::from_rgb8(46, 204, 113), 30.0, 4.0, 1.0),
        (Color::from_rgb8(52, 152, 219), 20.0, 5.0, 2.0),
    ];

    for &(color, amplitude, freq, phase) in &waves {
        ctx.set_global_alpha(0.4);
        ctx.set_fill_color(color);

        // Filled area wave
        ctx.begin_path();
        ctx.move_to(0.0, H);
        for i in 0..=60 {
            let x = i as f32 * W / 60.0;
            let y = mid_y + (x * freq / W * PI * 2.0 + t + phase).sin() * amplitude;
            ctx.line_to(x, y);
        }
        ctx.line_to(W, H);
        ctx.close_path();
        ctx.fill();

        // Stroke on top
        ctx.set_global_alpha(0.8);
        ctx.set_stroke_color(color);
        ctx.set_line_width(2.0);
        ctx.begin_path();
        for i in 0..=60 {
            let x = i as f32 * W / 60.0;
            let y = mid_y + (x * freq / W * PI * 2.0 + t + phase).sin() * amplitude;
            if i == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
        }
        ctx.stroke();
    }
    ctx.set_global_alpha(1.0);
}

// ── Animated Scene 14: Particles ────────────────────────────────────

fn draw_particles(ctx: Canvas2D, t: f32, _frame: u64) {
    ctx.clear_rect(0.0, 0.0, W, H);

    let colors = [
        Color::from_rgb8(231, 76, 60),
        Color::from_rgb8(46, 204, 113),
        Color::from_rgb8(52, 152, 219),
        Color::from_rgb8(241, 196, 15),
        Color::from_rgb8(155, 89, 182),
        Color::from_rgb8(230, 126, 34),
    ];

    // 30 particles with pseudo-random motion
    for i in 0..30 {
        let seed = i as f32 * 73.37;
        let speed = 0.5 + (seed * 0.13).sin().abs() * 1.5;
        let phase = seed * 0.7;
        let radius = 4.0 + (seed * 0.3).sin().abs() * 8.0;

        let x = ((seed + t * speed).sin() * 0.5 + 0.5) * (W - 20.0) + 10.0;
        let y = ((phase + t * speed * 0.7).cos() * 0.5 + 0.5) * (H - 20.0) + 10.0;

        let alpha = 0.3 + (t + seed).sin().abs() * 0.7;
        ctx.set_global_alpha(alpha);
        ctx.set_fill_color(colors[i % colors.len()]);
        ctx.begin_path();
        ctx.arc(x, y, radius, 0.0, PI * 2.0, false);
        ctx.fill();
    }

    // Connecting lines between close particles (first 10)
    ctx.set_global_alpha(0.15);
    ctx.set_stroke_color(Color::WHITE);
    ctx.set_line_width(1.0);
    for i in 0..10 {
        let seed_a = i as f32 * 73.37;
        let speed_a = 0.5 + (seed_a * 0.13).sin().abs() * 1.5;
        let phase_a = seed_a * 0.7;
        let xa = ((seed_a + t * speed_a).sin() * 0.5 + 0.5) * (W - 20.0) + 10.0;
        let ya = ((phase_a + t * speed_a * 0.7).cos() * 0.5 + 0.5) * (H - 20.0) + 10.0;

        for j in (i + 1)..10 {
            let seed_b = j as f32 * 73.37;
            let speed_b = 0.5 + (seed_b * 0.13).sin().abs() * 1.5;
            let phase_b = seed_b * 0.7;
            let xb = ((seed_b + t * speed_b).sin() * 0.5 + 0.5) * (W - 20.0) + 10.0;
            let yb = ((phase_b + t * speed_b * 0.7).cos() * 0.5 + 0.5) * (H - 20.0) + 10.0;

            let dx = xa - xb;
            let dy = ya - yb;
            if dx * dx + dy * dy < 10000.0 {
                ctx.begin_path();
                ctx.move_to(xa, ya);
                ctx.line_to(xb, yb);
                ctx.stroke();
            }
        }
    }
    ctx.set_global_alpha(1.0);
}
