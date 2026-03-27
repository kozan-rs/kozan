# Kozan

Kozan is a cross-platform UI platform — what Chrome is to the web, but for native apps. Not a framework. The platform that frameworks are built on.

## Vision

- **Platform, not framework.** Kozan provides DOM, style, layout, events, rendering. Frameworks (React-like, Flutter-like) are built on top.
- **100% native everywhere.** iOS = UIKit. Web = real DOM. Android = native Views. Desktop = GPU (Vello). Each platform renders with its own native system.
- **Framework interop.** Any framework built on Kozan works with any other. Use framework A inside framework B.
- **2D + 3D unified.** Same tree, same clipping, same events. Game viewports and UI compose together.
- **Zed-level performance.** 144fps, adaptive per-subtree rendering, zero CPU when idle.

## Architecture

### Crate Map

```
kozan-primitives     ← Types: geometry, color, units, generational arena. Zero deps.
kozan-tree           ← Generic arena-based tree. Parent/child/sibling. No DOM semantics.
kozan-dom            ← Elements, attributes, event system. DOM tree only. NO style/layout/paint.
kozan-style          ← ComputedStyle, cascade, Stylo integration. NO layout dependency.
kozan-layout         ← Layout computation, Taffy integration. NO DOM dependency.
kozan-event          ← Event dispatch, hit testing. Platform-agnostic.
kozan-pipeline       ← Phase orchestrator: DOM → Style → Layout → Render.
kozan-paint          ← Display list generation. OPTIONAL — only GPU renderers use this.
kozan-platform       ← Trait definitions: ViewRenderer, Window, InputBridge, TextMeasurer.
kozan-canvas         ← Canvas 2D recording API. Renderer-agnostic.
kozan-scheduler      ← Event loop, task queues, frame scheduling. Per-View.
kozan-macros         ← Derive macros.
kozan                ← Facade crate. Re-exports for end users.
```

Backend crates (each implements `ViewRenderer`):
```
kozan-vello          ← GPU renderer: paint → display list → Vello/wgpu.
kozan-web            ← Web renderer: tree → real HTML DOM elements.
kozan-uikit          ← iOS renderer: tree → UIView hierarchy.
kozan-android        ← Android renderer: tree → native Views.
kozan-winit          ← Desktop windowing backend.
```

### Dependency Flow (strict — no cycles)

```
primitives → tree → dom → style → layout → pipeline
                                              ↓
                              ┌────────────────┼────────────────┐
                              ↓                ↓                ↓
                           paint          ViewRenderer     ViewRenderer
                           (GPU)          (native)         (hybrid)
                              ↓
                           vello
```

### Key Types

| Type | Responsibility | Crate |
|------|---------------|-------|
| `Arena<T>` | Generational storage. All allocations go here. | primitives |
| `Handle<T>` | Copy, 16 bytes, !Send. Reference into arena. | primitives |
| `Tree` | Parent/child/sibling links. Generic over node type. | tree |
| `Document` | DOM tree ONLY. Nodes, elements, text. No style/layout/paint. | dom |
| `Element` | Tag + attributes + children. No computed style. | dom |
| `StyleEngine` | Owns Stylo data. Computes styles. Separate struct. | style |
| `ComputedStyle` | Wraps Stylo's `ComputedValues`. No bulk conversion. | style |
| `LayoutEngine` | Owns Taffy. Computes layout. Separate struct. | layout |
| `LayoutResult` | Position + size output. Owned by LayoutEngine. | layout |
| `Painter` | Walks layout results → produces `DisplayItem` list. | paint |
| `ViewPipeline` | Orchestrates phases. Calls StyleEngine, LayoutEngine, Renderer. | pipeline |
| `ViewRenderer` | Trait. The platform abstraction fork point. | platform |
| `View` | Independent rendering context. Own Document + Pipeline + thread. | platform |
| `Window` | OS window container. Holds Views. | platform |

### The ViewRenderer Fork

After style + layout, the pipeline calls the renderer. This is where native happens:

```rust
pub trait ViewRenderer {
    fn render(&mut self, tree: &StyledLayoutTree, changes: &ChangeList);
}
```

- **Vello backend:** walks tree → generates DisplayItems → submits to GPU.
- **Web backend:** walks tree → creates/updates/removes real `<div>`, `<span>`, etc.
- **UIKit backend:** walks tree → creates/updates/removes UIViews with native frames.
- **Android backend:** walks tree → creates/updates/removes native Views.

GPU renderers use `kozan-paint`. Native renderers skip it entirely.

### View Model

```
Window (OS window = container)
├── View (GPU)      ← Vello-rendered, 144fps, full pipeline
├── View (Native)   ← Real UIKit / DOM / Android Views
└── View (GPU)      ← 3D viewport, game, canvas
```

- **Window** = OS window. Container only.
- **View** = Independent rendering context. Own Document + StyleEngine + LayoutEngine + Scheduler + thread.
- One Window, many Views. Each View picks its renderer.
- Per-View threads. Main thread is event router only.

## Principles

1. **Single-threaded UI.** Zero locks. Async via channels. Like Chrome.
2. **Arena + Handle.** Handle is Copy, 16 bytes, !Send. No lifetimes for users.
3. **Take-call-put.** Event handlers taken during dispatch, put back after. No RefCell.
4. **Phase pipeline.** Mutate → Style → Layout → Render. Strict order.
5. **Trait-abstracted.** Renderer, layout engine, style engine — all swappable via traits.
6. **2D + 3D unified.** Same tree, same clipping, same events.
7. **Framework interop.** Two frameworks in one tree must work.
8. **Zero coupling.** Adding features never requires refactoring existing code.
9. **Type-safe.** Typed elements, typed events. No string-based APIs.
10. **Incremental.** Dirty flags (u32 bitfield). Only recompute what changed.
11. **Chrome is the benchmark.** Every subsystem mirrors Chrome's proven design.
12. **Unit test everything.** No module without tests. Each subsystem testable in isolation.
13. **DocumentCell centralizes ALL unsafe.** Handle has zero unsafe blocks.

## Anti-Patterns

**Never do these. If you catch yourself doing any of these, stop and redesign.**

- **God objects.** No struct owns more than one subsystem. Document does NOT own style/layout/paint.
- **Coupling.** Style must not depend on layout. Layout must not depend on DOM. Paint must not depend on Document.
- **Hardcoded values.** Never hardcode sizes, colors, spacing. Everything flows from style/CSS/font systems.
- **Paint on Element.** Paint and measure logic belongs on LayoutResult/Fragment, NOT on Element/Node.
- **Bulk conversion.** ComputedStyle wraps Stylo's ComputedValues directly. No copying into Kozan types.
- **TaffyTree.** Use Taffy's `LayoutPartialTree` low-level API, not the high-level `TaffyTree`.
- **Shortcuts.** No "make it work now, fix later." Bad architecture compounds. A codebase with compiler errors but perfect architecture > working code with bad architecture.
- **Large files.** No file over 400 lines. If it's bigger, it has mixed concerns — split it.
- **pub leaks.** Default to `pub(crate)`. Only `pub` what's in the public API.

## Chrome Mappings

| Kozan | Chrome | Note |
|-------|--------|------|
| `Document` | `Document` | DOM tree only — no style/layout |
| `Element` | `Element` → `HTMLElement` | Typed: `DivElement`, `InputElement`, etc. |
| `StyleEngine` | `StyleEngine` | Separate struct, not inside Document |
| `ComputedStyle` | `ComputedStyle` | Per-element, cached, wraps engine output |
| `LayoutEngine` | `LayoutTreeAsText` / `LayoutView` | Separate from DOM tree |
| `LayoutResult` | `LayoutResult` / `PhysicalFragment` | Position + size, owned by engine |
| `Painter` | `PaintLayerPainter` | Walks layout → display list |
| `ViewPipeline` | `DocumentLifecycle` | Guards phase transitions |
| `View` | `RenderFrame` / `Page` | Independent rendering context |
| `Window` | `Browser Window` | OS container for Views |
| `ViewRenderer` | `RenderWidget` | Platform rendering abstraction |

## Phase Pipeline

```
1. Mutate    ← DOM changes, event handlers fire, JS/framework code runs
2. Style     ← StyleEngine resolves dirty nodes → ComputedStyle per element
3. Layout    ← LayoutEngine computes positions/sizes from styles
4. Render    ← ViewRenderer.render() — GPU paints OR native widgets update
```

Each phase reads only the output of the previous phase. No reaching back.
ViewPipeline enforces ordering — you cannot layout before style, cannot render before layout.

## Coding Standards

Write code that would pass review for the Rust standard library.

### Comments

Comments explain WHY, never WHAT. If the code is clear, no comment is needed.

```rust
// BAD — restates the code
// Clear the cache and return the parent

// BAD — AI-generated fluff
// This should now work correctly
// Fixed the issue with the layout

// GOOD — explains a non-obvious constraint
// Listeners are taken before dispatch and put back after, allowing
// safe tree mutation inside handlers (take-call-put pattern).

// GOOD — spec reference
// Chrome: CharacterData::DidModifyData() → ContainerNode::ChildrenChanged()
```

### Documentation

- Module docs (`//!`): one sentence — what the module IS.
- Struct/enum docs (`///`): one sentence if name isn't self-explanatory. Skip if obvious.
- Function docs: one line. Never explain what parameter types already say.

### Naming

- No `get_` prefix. `fn width()` not `fn get_width()`.
- `unwrap_foo()` panics, `try_foo()` returns Option.
- Command-query separation: mutate OR return, never both.

### Error Handling

- `expect("what must be true")`, never bare `unwrap()`.
- Infallible conversions: `unwrap_or(fallback)`, not `unwrap()`.

### Code Structure

- No micro-wrappers — inline single-call-site functions.
- `if let Some(x)` over `.is_some()` + `.unwrap()`.
- No redundant guards.

### Visibility

`pub(crate)` by default. `pub` only for the public API.

### Unsafe

All unsafe lives in `DocumentCell`. Every `unsafe` block has a `// SAFETY:` comment.

### Tests

Test behavior, not stubs. Names describe scenarios: `capture_fires_before_bubble`.

### Hard Rules

1. No comments that restate the code.
2. No section headers for obvious groupings.
3. No multi-paragraph docs on simple functions.
4. No defensive code for impossible cases.
5. `todo!()` not `// TODO`.
6. No touching files outside the task scope.
7. No adding features that weren't asked for.
8. No `pub` on internal methods.
9. No bare `.unwrap()` in production code.
10. No AI-debt comments.
11. No file over 400 lines.
12. No struct that owns more than one subsystem.
