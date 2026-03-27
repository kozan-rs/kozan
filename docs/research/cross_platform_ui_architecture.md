# Cross-Platform UI Framework Architecture Research

Research date: 2026-03-27

How major frameworks handle the native vs web target problem.
Focused on: abstraction layers, rendering strategies, code sharing, and crate/module structure.

---

## 1. Dioxus (Rust)

### Architecture Pattern: VirtualDom + WriteMutations Trait

**User code changes between targets?** NO. Same RSX code runs everywhere.

**Abstraction layer:** The `VirtualDom` (in `packages/core`) is the central abstraction. It manages
the component tree, schedules re-renders, and generates platform-agnostic `Mutations`. Each renderer
implements the `WriteMutations` trait to apply these mutations to its target platform.

```
User RSX code
    |
    v
VirtualDom (packages/core)
    |
    v  generates Mutations
WriteMutations trait
    |
    +---> dioxus-web        (compiles to WASM, manipulates real browser DOM via web-sys)
    +---> dioxus-desktop    (system WebView via wry — renders HTML/CSS in native window)
    +---> dioxus-native     (Blitz — custom WGPU HTML/CSS renderer, no webview)
    +---> dioxus-liveview   (server-side VirtualDom, sends diffs over WebSocket)
    +---> dioxus-ssr        (renders to HTML string on server)
```

**Web target uses real DOM?** YES (dioxus-web). The web renderer compiles to WASM and manipulates
real browser DOM nodes. The interpreter implements WriteMutations from dioxus-core to modify the DOM
with the diffs the VirtualDom generates.

**Desktop:** Uses system WebView (WKWebView on macOS, WebView2 on Windows, webkit2gtk on Linux)
via the `wry` crate. Your Rust code runs natively; UI renders in the embedded webview.

**Native (experimental):** Uses Blitz, a custom HTML/CSS renderer built on Servo ecosystem
components (stylo, html5ever, taffy, parley, vello, wgpu). No webview, no browser — just direct
GPU rendering of the DOM tree.

**Key crates:**
| Crate | Role |
|---|---|
| `packages/core` | VirtualDom, WriteMutations trait, Mutations, diffing |
| `packages/core-types` | Shared types across crates |
| `packages/html` | HTML element definitions (RSX elements) |
| `packages/web` | WASM/browser renderer (real DOM) |
| `packages/desktop` | System webview renderer via wry |
| `packages/native` | Blitz-based native renderer (WGPU) |
| `packages/native-dom` | DOM implementation for native renderer |
| `packages/interpreter` | Shared DOM mutation interpreter (used by web, desktop, liveview) |
| `packages/liveview` | Server-side rendering with WebSocket diffs |
| `packages/ssr` | Static HTML string rendering |
| `packages/rsx` | RSX macro parsing |
| `packages/signals` | Reactive state management |
| `packages/hooks` | React-style hooks |
| `packages/router` | Client-side routing |
| `packages/fullstack` | SSR + hydration + server functions |

**Critical insight:** The RSX representation is generic — you can swap out the element definitions.
The `packages/interpreter` crate is shared by web, desktop, and liveview renderers, meaning
all three use the same DOM mutation logic (just targeting different DOM implementations).

---

## 2. Flutter

### Architecture Pattern: Own Rendering Engine (Skia/Impeller) on Every Platform

**User code changes between targets?** NO. Same Dart code everywhere.

**Abstraction layer:** Flutter does NOT use platform UI widgets at all. It has its own complete
widget set, layout engine, and rendering pipeline. The rendering engine (Skia on most platforms,
Impeller on iOS) draws directly to a canvas/surface.

```
Dart Widget Code
    |
    v
Widget Tree -> Element Tree -> RenderObject Tree
    |
    v
Skia / Impeller (rendering engine)
    |
    +---> iOS/Android: native Skia/Impeller draws to GPU surface
    +---> Desktop: same Skia draws to native window surface
    +---> Web: Skia compiled to WASM (CanvasKit/SkWasm) draws to <canvas>
```

**Web target uses real DOM?** NO. Flutter web renders everything to a single `<canvas>` element
using Skia compiled to WebAssembly (CanvasKit or SkWasm). The old HTML renderer (which used real
DOM elements like `<div>`, `<p>`, CSS) was deprecated in 2024 and removed in early 2025.

**Current web renderers (2025):**
- **CanvasKit:** Skia compiled to WASM (~1.5MB), renders via WebGL on main thread
- **SkWasm:** Compact Skia-to-WASM, renders on a separate Web Worker thread (2-3x faster than
  CanvasKit). This is the future direction.

**Consequences of canvas rendering on web:**
- No real DOM elements = no browser accessibility by default (Flutter adds its own a11y tree)
- No CSS styling, no browser text selection, no native scrollbars
- SEO is difficult (no real HTML content)
- Pixel-perfect consistency across all platforms

**Key architectural insight:** Flutter treats the browser as just another GPU surface. The web
is not special — it gets the same rendering pipeline as mobile/desktop, just with Skia compiled
to WASM instead of running natively.

---

## 3. React Native / React Native Web

### Architecture Pattern: Component Abstraction + Platform-Specific Renderers

**User code changes between targets?** MOSTLY NO. Same React components, but platform-specific
code sometimes needed for features that don't map cleanly.

**Abstraction layer:** React's reconciler (the diffing algorithm) is separated from the renderer.
React Native provides platform-agnostic components (`View`, `Text`, `Image`, `ScrollView`) that
map to different things on each platform.

```
React Component Code (View, Text, Image, etc.)
    |
    v
React Reconciler (shared)
    |
    +---> React Native Renderer (iOS) -> UIView, UILabel, UIImageView
    +---> React Native Renderer (Android) -> android.view.View, TextView, ImageView
    +---> React Native Web (react-native-web) -> <div>, <span>, <img>
```

**Web target uses real DOM?** YES. React Native Web (`react-native-web` by Nicolas Gallagher,
maintained by Meta) maps React Native components to real HTML elements:
- `View` -> `<div>`
- `Text` -> `<span>` (with accessibility attributes)
- `Image` -> `<img>`
- `TextInput` -> `<input>` / `<textarea>`
- StyleSheet -> CSS (converted from JS style objects to native CSS)

**How it works concretely:**
- `react-native-web` is a drop-in replacement for `react-native` when bundling for web
- Webpack/Metro aliases `react-native` imports to `react-native-web`
- Each component has a web implementation that renders to semantic HTML + CSS
- Styles written in React Native's StyleSheet API are converted to atomic CSS classes

**Key architectural insight:** This is a **component mapping** approach, not a rendering engine
approach. Each platform gets truly native elements (real UIViews on iOS, real `<div>`s on web).
The abstraction is at the component/API level, not the rendering level.

**Companies using this:** Twitter/X runs the same React Native codebase on iOS, Android, and web.

---

## 4. Tauri

### Architecture Pattern: System WebView + Rust Backend (NOT a cross-platform UI framework)

**User code changes between targets?** The frontend is always web code (HTML/CSS/JS). The Rust
backend provides native capabilities via IPC.

**Abstraction layer:** Tauri is NOT trying to solve "same code, different renderers." It is
a desktop/mobile app shell that embeds the system's native WebView and provides a Rust backend.

```
Frontend (HTML/CSS/JS — any web framework)
    |
    v  IPC (message passing)
Rust Backend (native system access)
    |
    v
System WebView
    +---> macOS/iOS: WKWebView (WebKit)
    +---> Windows: WebView2 (Chromium/Edge)
    +---> Linux: webkit2gtk (WebKit)
```

**Web target uses real DOM?** YES — it IS a webview. The entire frontend is standard web content.

**Key crate: `wry`** — Tauri's cross-platform WebView library that abstracts over platform-specific
webview implementations (WKWebView, WebView2, webkit2gtk).

**IPC mechanism:** JavaScript calls `window.__TAURI__.invoke('command_name', { args })` which
sends a message to the Rust backend. The Rust side handles it and returns a result.

**Key architectural insight:** Tauri explicitly chose NOT to build its own rendering engine.
It leverages the OS-provided webview, keeping binary sizes tiny (~5MB vs Electron's ~150MB).
The tradeoff: rendering behavior varies between platforms because different webview engines
(WebKit vs Chromium) have different CSS/JS behaviors.

---

## 5. Kotlin Compose Multiplatform

### Architecture Pattern: Skia Canvas Rendering via Skiko (Similar to Flutter)

**User code changes between targets?** NO. Same @Composable functions everywhere.

**Abstraction layer:** Compose Multiplatform uses Skiko (Skia bindings for Kotlin) as a
cross-platform graphics backend. The Compose runtime builds a UI tree, measures/lays it out,
generates draw commands, and Skia executes them.

```
@Composable Functions (shared Kotlin code)
    |
    v
Compose Runtime (tree diffing, recomposition)
    |
    v
Compose UI (measurement, layout, drawing)
    |
    v
Skiko (Kotlin Skia bindings)
    |
    +---> Desktop (JVM): Skia renders to native window via OpenGL/Metal/DirectX
    +---> iOS: Skia renders to native UIView canvas
    +---> Android: Jetpack Compose (native Android, Skia via HWUI)
    +---> Web: Kotlin/Wasm + Skia renders to <canvas> via CanvasBasedWindow
```

**Web target uses real DOM?** NO. Like Flutter, Compose Multiplatform renders everything to a
single `<canvas>` element. The `CanvasBasedWindow` API draws all composables to a canvas element.

**There is also Compose HTML** (`org.jetbrains.compose.web`), a SEPARATE library that renders to
real DOM elements — but it is NOT the same code as Compose Multiplatform. Compose HTML is
Kotlin/JS-only and uses a different API. You cannot share UI code between Compose Multiplatform
and Compose HTML.

**Web performance (2025):** Kotlin/Wasm is ~3x faster than Kotlin/JS in UI scenarios. Compose for
Web reached Beta in September 2025.

**Key architectural insight:** Nearly identical strategy to Flutter. Same rendering engine on every
platform. Canvas-based web rendering. Same tradeoffs (no real DOM, accessibility challenges, SEO
issues, but pixel-perfect cross-platform consistency).

---

## 6. Servo / Stylo

### Architecture Pattern: Pipeline Stages with Emerging Renderer Trait Abstraction

**User code changes between targets?** N/A — Servo is a browser engine, not a UI framework.

**Current architecture:** Tightly coupled to WebRender (Mozilla's GPU-based 2D renderer) and OpenGL.

```
HTML/CSS (parsed by html5ever + Stylo)
    |
    v
DOM Tree -> Style Resolution (Stylo)
    |
    v
Layout (box tree -> fragment tree -> display list)
    |
    v
Display List sent to Compositor
    |
    v
WebRender (GPU-based rasterization via OpenGL)
    |
    v
Surfman (OpenGL context management)
```

**Rendering backend abstraction (in progress, issue #37149):**
Servo is actively working on abstracting its renderer into traits. The motivation:
- WebRender is tightly coupled to OpenGL, which is being replaced by Vulkan/Metal/DirectX
- The team wants to support alternative renderers like **Vello** (GPU compute-based 2D renderer)
- Vello has already been integrated as an alternative 2D canvas backend (PR #36821)
- The `RenderingContext` trait has been simplified to reduce coupling to surfman-specific types

**Key components:**
| Component | Role |
|---|---|
| `stylo` | CSS style resolution (shared with Firefox) |
| `layout` | Box tree, fragment tree, display list construction |
| `webrender` | GPU-based 2D rendering (display list -> pixels) |
| `surfman` | Cross-platform OpenGL context management |
| `compositing` | IOCompositor manages WebRender instances and RenderingContexts |
| `canvas` | 2D canvas with pluggable backends (raqote, vello, vello_cpu) |

**Key architectural insight:** Servo's renderer abstraction is NOT about "native vs web" — it's
about making the rendering backend pluggable (WebRender vs Vello vs future engines). The canvas
2D subsystem already has a `Backend` trait with multiple implementations. The main rendering
pipeline is being refactored toward similar trait-based abstraction.

---

## Summary Comparison

| Framework | Same User Code? | Web Rendering | Abstraction Type | Real DOM on Web? |
|---|---|---|---|---|
| **Dioxus** | Yes (RSX) | WASM + real DOM manipulation | VirtualDom + WriteMutations trait | YES |
| **Flutter** | Yes (Dart) | Skia-to-WASM on `<canvas>` | Own render engine everywhere | NO |
| **React Native Web** | Yes (JSX) | Real HTML elements | Component mapping (View->div) | YES |
| **Tauri** | Web code only | System WebView | Not a UI framework — app shell | YES (IS webview) |
| **Compose MP** | Yes (@Composable) | Skia-to-WASM on `<canvas>` | Own render engine everywhere | NO |
| **Servo** | N/A (browser) | N/A | Pipeline with emerging renderer traits | N/A |

## Key Patterns Identified

### Pattern A: "Own Rendering Engine" (Flutter, Compose Multiplatform)
- Same rendering engine (Skia) compiled to every platform
- Web = Skia compiled to WASM, drawing to `<canvas>`
- Pros: pixel-perfect consistency, no platform quirks
- Cons: no real DOM (bad SEO, accessibility requires extra work, large WASM bundle)

### Pattern B: "VirtualDom + Platform Renderers" (Dioxus, React Native Web)
- Shared tree/diffing abstraction, platform-specific renderers
- Web = real DOM elements
- Pros: native web semantics (a11y, SEO, CSS), smaller bundles
- Cons: platform differences can leak through, renderer implementations diverge

### Pattern C: "Embedded WebView" (Tauri, Dioxus Desktop)
- Use the OS-provided web engine as the rendering surface
- Frontend is standard web code
- Pros: tiny binary, full web compatibility
- Cons: cross-platform rendering differences (WebKit vs Chromium), limited to what webview supports

### Pattern D: "Renderer Trait Abstraction" (Servo, Dioxus)
- Define a trait interface between the engine and the rendering backend
- Servo: `Backend` trait for canvas, `RenderingContext` trait for compositor
- Dioxus: `WriteMutations` trait for DOM operations
- Allows swapping rendering backends without changing the engine

---

## Relevance to Kozan

Kozan is a browser engine / UI platform, not a framework. The most relevant patterns are:

1. **Servo's approach (Pattern D):** Abstract the renderer behind traits. Kozan already uses Vello
   via `kozan-vello`, which is the right direction. The key is ensuring the paint/compositor layer
   talks through a trait, not directly to Vello.

2. **Dioxus's Blitz is the closest analog:** Blitz IS a browser engine (like Kozan) that uses
   stylo + taffy + vello + wgpu. It renders HTML/CSS without a webview. The difference is Blitz
   serves as Dioxus's native renderer, while Kozan is the platform itself.

3. **For a future web target (kozan compiled to WASM):** The choice would be between:
   - **Canvas rendering** (like Flutter/Compose): compile Vello to WASM, render to `<canvas>`.
     This preserves pixel-perfect rendering but loses DOM semantics.
   - **DOM output** (like Dioxus-web/React Native Web): emit real DOM elements. This would
     require a completely different paint backend that generates HTML/CSS instead of Vello scenes.
   - **Hybrid**: use `<canvas>` for custom rendering but emit real DOM for text/a11y overlays
     (this is what Flutter tried with its deprecated HTML renderer).

Sources:
- https://dioxuslabs.com/
- https://github.com/DioxusLabs/dioxus
- https://deepwiki.com/DioxusLabs/dioxus
- https://docs.rs/dioxus-core/latest/dioxus_core/trait.WriteMutations.html
- https://github.com/DioxusLabs/blitz
- https://docs.flutter.dev/platform-integration/web/renderers
- https://docs.flutter.dev/resources/architectural-overview
- https://github.com/flutter/flutter/issues/145954
- https://necolas.github.io/react-native-web/docs/
- https://github.com/necolas/react-native-web
- https://v2.tauri.app/concept/architecture/
- https://deepwiki.com/tauri-apps/tauri/1.1-what-is-tauri
- https://github.com/JetBrains/skiko
- https://deepwiki.com/JetBrains/compose-multiplatform
- https://blog.jetbrains.com/kotlin/2025/09/compose-multiplatform-1-9-0-compose-for-web-beta/
- https://book.servo.org/architecture/overview.html
- https://github.com/servo/servo/issues/37149
- https://github.com/servo/servo/wiki/Webrender-Overview
- https://github.com/servo/servo/pull/36821
