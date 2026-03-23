//! Hello World — the first Kozan application.
//!
//! Demonstrates:
//! 1. Two API styles (standard DOM + shortcuts)
//! 2. Text rendering (English + Arabic)
//! 3. Full pipeline: DOM → Stylo → Taffy → Paint → Vello → pixels

use kozan::prelude::*;

/// Row 1 — Standard DOM API with text.
fn build_standard_dom(doc: &Document) {
    let row = doc.create::<HtmlDivElement>();
    row.set_attribute("style",
        "display: flex; gap: 16px; padding: 20px; align-items: center; background-color: rgb(44, 62, 80); color: white"
    );

    // Text node (English)
    let label = doc.create_text("Hello, Kozan!");
    row.append(label);

    // Colored boxes
    for color in ["rgb(231,76,60)", "rgb(46,204,113)", "rgb(52,152,219)"] {
        let box_ = doc.create::<HtmlDivElement>();
        box_.set_attribute("style", format!("width: 60px; height: 60px; border-radius: 8px; background-color: {color}"));
        row.append(box_);
    }

    doc.body().append(row);
}

/// Row 2 — Shortcuts API with Arabic text (RTL).
fn build_with_shortcuts(doc: &Document) {
    let row = doc.div();
    row.style().flex().gap(px(16.0)).pad(px(20.0)).align_items_center().bg(rgb8(39, 174, 96)).color(rgb8(255, 255, 255));

    // Arabic text (RTL — HarfRust handles joining + bidi automatically)
    let label = doc.create_text("مرحبا بالعالم!");
    row.append(label);

    let red = doc.div();
    red.style().size(px(60.0)).border_radius(px(8.0)).bg(rgb8(231, 76, 60));

    let green = doc.div();
    green.style().size(px(60.0)).border_radius(px(8.0)).bg(rgb8(46, 204, 113));

    let blue = doc.div();
    blue.style().size(px(60.0)).border_radius(px(8.0)).bg(rgb8(52, 152, 219));

    doc.body().child(row.child(red).child(green).child(blue));
}

/// Row 3 — Mixed LTR + RTL text.
fn build_mixed_text(doc: &Document) {
    let row = doc.div();
    row.style().flex_col().gap(px(8.0)).pad(px(20.0)).bg(rgb8(52, 73, 94)).color(rgb8(255, 255, 255));

    let en = doc.create_text("Kozan renders text beautifully.");
    row.append(en);

    let ar = doc.create_text("كوزان يرسم النصوص بجودة عالية.");
    row.append(ar);

    let mixed = doc.create_text("Welcome مرحبا to Kozan كوزان!");
    row.append(mixed);

    doc.body().child(row);
}

fn main() -> kozan::Result<()> {
    let config = WindowConfig {
        title: "Hello, Kozan!".into(),
        ..Default::default()
    };

    App::new().window(config, build_ui).run()
}

fn build_ui(ctx: &ViewContext) {
    let doc = ctx.document();
    doc.body().style().w(pct(100.0)).h(pct(100.0)).mar(px(0.0));

    build_standard_dom(doc);
    build_with_shortcuts(doc);
    build_mixed_text(doc);
}
