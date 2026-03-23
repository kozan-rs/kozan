<div align="center">

# Kozan

A native UI engine for Rust built on browser architecture.

[![Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

[Roadmap](ROADMAP.md) | [Vision](VISION.md)

</div>

---

Kozan is a platform layer — DOM, events, style, layout, paint, scroll, compositing — that UI frameworks build on top of. Two frameworks on Kozan share the same tree, the same event system, and the same rendering pipeline.

> This is experimental software. APIs change without notice.

```rust
use kozan::prelude::*;

fn main() -> kozan::Result<()> {
    App::new().window(WindowConfig::default(), build_ui).run()
}

fn build_ui(ctx: &ViewContext) {
    let doc = ctx.document();

    let row = doc.div();
    row.style().flex().gap(px(16.0)).pad(px(20.0)).bg(rgb8(44, 62, 80));
    row.append(doc.create_text("Hello, Kozan!"));

    doc.body().child(row);
}
```

## Pipeline

```
DOM → Style → Layout → Paint → Composite → GPU
```

Three threads per window. Main thread routes OS events. View thread runs the DOM, style, layout, and paint. Render thread runs the compositor and GPU — scroll happens here at vsync rate, independent of layout.

## Crates

| Crate | |
|-------|---|
| `kozan` | Facade — re-exports everything |
| `kozan-core` | DOM, events, style, layout, paint, scroll, compositor |
| `kozan-primitives` | Geometry, color, arena allocator |
| `kozan-scheduler` | Event loop, task queues, async executor |
| `kozan-macros` | Derive macros for Element, Node, Props |
| `kozan-platform` | Window management, threading, renderer traits |
| `kozan-winit` | winit adapter |
| `kozan-vello` | Vello + wgpu backend |

## Usage

### Styling

Two ways — inline CSS strings or the type-safe builder:

```rust
// CSS string (parsed by Stylo)
div.set_attribute("style", "display: flex; gap: 16px; padding: 20px");

// Builder API
div.style().flex().gap(px(16.0)).pad(px(20.0)).bg(rgb8(44, 62, 80));
```

### CSS classes

Load a stylesheet and toggle classes:

```rust
doc.load_css_string(include_str!("../assets/dashboard.css"));

card.class_add("card");
card.class_add("card-blue");
card.class_remove("card-blue");
```

### Events

```rust
btn.on::<ClickEvent>(|event, ctx| {
    println!("clicked at ({}, {})", event.x, event.y);
});

// Capture phase
container.on_capture::<ClickEvent>(|event, ctx| {
    ctx.stop_propagation();
});

// One-shot listener (auto-removed after first call)
btn.on_once::<ClickEvent>(|_, _| {
    println!("only fires once");
});
```

### Async tasks

```rust
ctx.spawn(async move {
    sleep(Duration::from_millis(500)).await;
    card.class_add("visible");
    progress_bar.style().w(pct(75.0));
});
```

### HTML from strings

```rust
doc.load_html_string(include_str!("../assets/dashboard.html"));
doc.load_css_string(include_str!("../assets/dashboard.css"));
```

### DOM manipulation

```rust
let doc = ctx.document();

let container = doc.div();
let child = doc.div();
let text = doc.create_text("Hello");

container.append(child);
child.append(text);
doc.body().child(container);

// Query
let first = container.first_child();
let kids = container.children();

// Remove
child.remove();
```

## What works today

**Layout:** block, flexbox, grid (tracks, repeat, minmax, named areas, auto-placement), inline text with shaping (HarfBuzz), RTL/bidi, float (partial).

**CSS:** width/height/margin/padding (px, %, auto), border (width, color, radius), background-color, color, opacity, font-size/weight/family/style, text-align, text-decoration, visibility, overflow (visible, hidden, scroll), gap, aspect-ratio, box-shadow, outline.

**Events:** click, dblclick, mousedown/up/move/enter/leave/over/out, contextmenu, keydown/keyup, wheel, focus/blur/focusin/focusout, scroll, resize. Full W3C capture/target/bubble dispatch.

**Rendering:** rectangles, rounded rectangles, borders (solid), text (pre-shaped glyphs), lines, box shadows, outlines, opacity layers, clip regions, scroll transforms.

**Elements:** div, span, p, h1-h6, a, button, input (18 types), textarea, select, img, canvas, video, audio, table, ul/ol/li, form, section/article/nav/aside, and more.

## Not yet supported

- Animations and transitions
- Gradients (linear, radial, conic)
- Images (element exists, rendering stubbed)
- Filters (blur, brightness, etc.)
- `position: fixed` / `position: sticky`
- Text selection, clipboard
- Media playback (video/audio stubs only)
- Custom properties (CSS variables)

## Build

```bash
cargo run --example hello-world
cargo run --example dashboard
cargo test --workspace
```

Requires Rust 1.85+.

## License

[Apache-2.0](LICENSE).

The name "Kozan" is a trademark of Youssef Khalil. The code is yours to use under Apache-2.0. The name and logo are not — forks can't use them to imply official status. Same policy as [Rust](https://foundation.rust-lang.org/policies/logo-policy-and-media-guide/) and [Firefox](https://www.mozilla.org/en-US/foundation/trademarks/policy/).
