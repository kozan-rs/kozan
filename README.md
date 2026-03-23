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
