# Kozan — The Rust UI Platform

## What Is Kozan?

Kozan is not a UI framework. It is the **platform** — the equivalent of what Chrome/the browser engine is to the web, but for Rust.

Every UI library in Rust today (iced, druid, egui, slint, dioxus, floem) is monolithic. Each one builds its own rendering, layout, events, and state management from scratch. They are incompatible with each other. A widget from iced cannot live inside an egui app. A Bevy game cannot embed a dioxus panel with correct clipping, events, and layout.

Kozan solves this permanently.

## The Problem

1. **No shared foundation.** Every Rust UI library reinvents the wheel. There is no "DOM" equivalent that frameworks can build on.
2. **Ownership hell.** UI trees are graph-like. Rust's ownership model is tree-like. Every library fights this battle differently (Rc/RefCell, Arc/Mutex, ECS, signals). None are satisfying.
3. **No interop.** Framework A and Framework B cannot coexist. You cannot render a React-like component inside a Flutter-like layout. On the web, this is trivial because everything speaks DOM.
4. **No 2D/3D unification.** Rendering a game viewport inside a UI panel with correct clipping, layering, and event handling is unsolved in Rust.

## The Solution

Kozan provides the shared foundation:

- **A node tree** with parallel arena storage. No lifetimes, no Arc/Mutex, no locks. 16-byte copyable Handles. Single-threaded, compiler-enforced.
- **A style system** with Chrome-level architecture: property groups behind Arc (copy-on-write), cascade, inheritance, incremental invalidation.
- **A layout engine** (flexbox, grid, block) that is trait-abstracted and incremental.
- **An event system** with capture/bubble/target phases, type-safe events, take-call-put pattern for safe re-entrant mutation, Chrome-accurate dispatch.
- **A render pipeline** that supports 2D and 3D content in the same tree, with correct clipping, layering, and compositing.
- **A platform abstraction** for windows, input, clipboard, and timers.

Frameworks (Flutter-like, React-like, ELM-like, signals-based, immediate-mode) are built ON TOP of Kozan. Because they all share the same tree, they automatically interoperate.

## Who Uses Kozan?

### Normal User
Uses the Document API directly. Creates elements, sets styles, handles events. Like a web developer writing vanilla JavaScript. They never think about ownership or lifetimes.

### Framework Author
Builds a higher-level framework (reactive, component-based, whatever) on top of Kozan's Document API. They get rendering, layout, events, and tree management for free. They focus on their programming model.

### Framework Mixer
Uses Framework A and Framework B in the same application. A data table from Framework B renders inside Framework A's layout. Events bubble correctly across boundaries. Layout composes. Rendering layers correctly. Like Astro on the web.

### Component Library Author
Creates reusable elements (data grid, chart, video player, code editor) that work with ANY framework built on Kozan. Like web components — they're just nodes in the tree.

### Game Engine Integration
A game engine (Bevy, etc.) can render into a Kozan element. The game viewport is a node in the tree. UI elements render on top with correct clipping. Events flow through both game and UI content. No hacks.

## Architecture Principles

1. **Single-threaded UI.** Like the browser. Zero locks. Async work returns results via channels.
2. **Parallel arenas + generational IDs.** No references, no lifetimes for users. Handle is Copy, 16 bytes, !Send.
3. **Take-call-put for events.** Handlers taken from storage during dispatch. No RefCell, no re-entrancy issues.
4. **Phase-based pipeline.** Mutate → Style → Layout → Paint → Composite. Strict ordering.
5. **Every layer trait-abstracted.** Renderer, layout engine, platform — all swappable.
6. **2D + 3D unified.** A 3D viewport is just an element that paints using a 3D renderer.
7. **Framework interop by design.** Two frameworks in the same tree. Events, layout, rendering all compose.
8. **No coupling.** Adding features never requires refactoring existing architecture.
9. **Type-safe where it matters.** Typed elements, typed events, typed styles. No string-based APIs.
10. **Incremental everything.** Dirty flags. Only recompute what changed.
11. **Chrome is the benchmark.** Every subsystem mirrors Chrome's proven architecture.

## Quality Standards

- Chrome/Google level. Every component must be production-grade.
- Unit tests for every module. No code without tests.
- Zero warnings. Zero hacks.
- If the architecture is wrong, redesign from scratch. No band-aids.

## The Ambition

Kozan is the future of UI for Rust. The standard foundation that every framework, every game engine, every application builds on. The shared platform that makes an ecosystem possible.
