# Kozan Roadmap

## Milestone 1: Core Engine ✅

> Foundation: DOM, Events, Style — the engine that everything builds on.

### M1.1 DOM Engine ✅
- [x] 1.1.1 Parallel arenas with generational IDs (Storage<T>, MaybeUninit + bitmap)
- [x] 1.1.2 Handle — 16 bytes, Copy, !Send, zero unsafe
- [x] 1.1.3 DocumentCell — ALL unsafe centralized in one type
- [x] 1.1.4 Trait hierarchy: HasHandle → EventTarget → Node → ContainerNode → Element → HtmlElement
- [x] 1.1.5 NodeFlags — u32 bitfield (type bits 0-3, dirty/focus/container flags)
- [x] 1.1.6 RawId — 8 bytes, Send, cross-thread safe
- [x] 1.1.7 Text node (Node only, NOT Element — compile error on text.append())
- [x] 1.1.8 Attribute system — Attribute, AttributeCollection
- [x] 1.1.9 ElementData — shared across all element types
- [x] 1.1.10 Tree operations — append, insert_before, detach, destroy, children
- [x] 1.1.11 DocumentExt trait — doc.div(), doc.span(), doc.div_in(parent)
- [x] 1.1.12 .child() chaining — GPUI-style fluent tree building
- [x] 1.1.13 HtmlBodyElement — real `<body>` tag, UA stylesheet styled
- [x] 1.1.14 ClassList API — class_add/remove/toggle/has (HashSet<Atom>, O(1))

### M1.2 Event System ✅
- [x] 1.2.1 Event trait + EventContext (phase, propagation control)
- [x] 1.2.2 EventTarget trait — on(), off(), dispatch_event()
- [x] 1.2.3 Chrome-accurate dispatch: capture → target (2 passes) → bubble
- [x] 1.2.4 Take-call-put pattern — safe tree mutation during handlers
- [x] 1.2.5 EventPath snapshot — tree changes during dispatch don't corrupt path
- [x] 1.2.6 EventStore — uses Storage<T>, lazy (None for most nodes)
- [x] 1.2.7 Once listeners, stopPropagation, stopImmediatePropagation
- [x] 1.2.8 Non-bubbling events fire both passes at target (Chrome behavior)

### M1.3 Style System ✅ (Stylo-powered)
- [x] 1.3.1 Full Stylo CSS engine (Mozilla's production CSS engine)
- [x] 1.3.2 280+ CSS properties — full W3C coverage
- [x] 1.3.3 Inline styles via PropertyDeclaration — zero parsing for typed API
- [x] 1.3.4 Batched StyleAccess — N properties, ONE write on Drop
- [x] 1.3.5 Short aliases: w(), h(), bg(), flex(), pad(), mar(), gap(), size()
- [x] 1.3.6 CSS unit helpers: px(), pct(), em(), rem(), vw(), vh(), auto()
- [x] 1.3.7 Color helpers: rgb(), rgba(), rgb8(), hex()
- [x] 1.3.8 Alignment: align_items_center/start/end, justify_center/start/end/between
- [x] 1.3.9 Border: border_radius(), border_width(), border_style(), border_color()
- [x] 1.3.10 Flex: flex_row(), flex_col(), flex_grow(), flex_shrink(), flex_basis()
- [x] 1.3.11 Inline CSS fallback: set_attribute("style", "any: valid css")
- [x] 1.3.12 UA stylesheet with full HTML defaults (body, div, p, h1-h6, etc.)
- [x] 1.3.13 Incremental restyle with dirty ancestor propagation
- [x] 1.3.14 CSS stylesheets — add_stylesheet(&self, css) with full selector support
- [x] 1.3.15 font_family() typed builder
- [x] 1.3.16 CSS class selectors, ID selectors, tag selectors, descendant selectors

### M1.4 HTML Elements ✅
- [x] 1.4.1 HtmlElement trait — shared behavior (hidden, title, dir, tabIndex, draggable)
- [x] 1.4.2 50+ HTML element types (div, span, p, h1-h6, body, a, form, input, etc.)
- [x] 1.4.3 Category traits: FormControlElement, TextControlElement, MediaElement, ReplacedElement
- [x] 1.4.4 Runtime tag names — Element::tag_name() reads from ElementData
- [x] 1.4.5 Derive macros: #[derive(Element)], #[derive(Props)]

---

## Milestone 2: Scheduler ✅

> Task scheduling, async executor, frame timing — the engine's heartbeat.

- [x] 2.1 Task system with 6 priority levels + delayed tasks
- [x] 2.2 TaskQueue + TaskQueueManager with anti-starvation
- [x] 2.3 MicrotaskQueue — HTML spec drain semantics
- [x] 2.4 Cross-thread WakeSender/WakeReceiver
- [x] 2.5 LocalExecutor — single-threaded async (!Send futures)
- [x] 2.6 Frame scheduler — vsync-aligned, dirty-based (zero CPU when idle)
- [x] 2.7 Full HTML event loop: cross-thread → macrotask → microtask → frame

---

## Milestone 3: Layout Engine ✅

> Flexbox, grid, block, inline — turning the DOM tree into positioned boxes.

### M3.1 Layout Object Tree ✅
- [x] 3.1.1 LayoutObject — separate from DOM (display:none = no LayoutObject)
- [x] 3.1.2 LayoutTree — arena-based, parallel to DOM
- [x] 3.1.3 DOM walker — builds LayoutTree from real DOM
- [x] 3.1.4 Anonymous box fixup + display: contents

### M3.2 Layout Algorithms ✅
- [x] 3.2.1 Block layout — normal flow, margin collapsing, auto margins
- [x] 3.2.2 Flex layout (via Taffy LayoutPartialTree)
- [x] 3.2.3 Grid layout (via Taffy LayoutGridContainer)
- [x] 3.2.4 Inline layout — line breaking, baseline alignment, vertical-align
- [x] 3.2.5 Positioned layout — absolute, fixed, relative
- [x] 3.2.6 Float layout + clear
- [x] 3.2.7 Replaced element layout (intrinsic sizing, aspect ratio)
- [x] 3.2.8 RTL support — InlineDirection centralized (swap+mirror for absolute)

### M3.3 Fragment Tree ✅
- [x] 3.3.1 Immutable fragments (Arc-wrapped)
- [x] 3.3.2 BoxFragment, TextFragment, LineFragment
- [x] 3.3.3 Fragment caching with ConstraintSpace validation

### M3.4 Font System ✅
- [x] 3.4.1 Parley (FontContext + LayoutContext) — real fonts, real shaping
- [x] 3.4.2 System font discovery (Fontique), metrics (Skrifa), shaping (HarfRust)
- [x] 3.4.3 Zero hardcoding — all metrics from real fonts
- [x] 3.4.4 Custom font registration — register_font() with FontBlob (zero-copy for include_bytes!)
- [x] 3.4.5 Variable font support — normalized_coords flow through full pipeline
- [x] 3.4.6 Font hinting enabled for sharper screen text

### M3.5 Layout Fixtures
- [x] 3.5.1 4,252 Taffy XML fixtures — 78.7% pass rate (3346/4252)
- [ ] 3.5.2 Target: 90%+ pass rate
- [ ] 3.5.3 Float fixtures (0/4 — needs work)

---

## Milestone 4: Paint System ✅

> Display list generation from fragment tree — the last step before pixels.

- [x] 4.1 DrawCommand — Rect, RoundedRect, RoundedBorderRing, Border, Text, Image, Line, BoxShadow, Outline
- [x] 4.2 Clip/Opacity/Transform stacks
- [x] 4.3 CSS 2.2 Appendix E paint order (z-index stacking contexts)
- [x] 4.4 Background painting (solid + rgba transparency)
- [x] 4.5 Border painting — flat (4-edge rects) + rounded (EvenOdd compound path ring)
- [x] 4.6 Text painting — pre-shaped glyph runs from Parley
- [x] 4.7 Text decoration — underline, overline, line-through with correct color from CSS
- [x] 4.8 Multi box-shadow emission (all shadows, reverse order)
- [x] 4.9 Outline painting (outside border-box, EvenOdd ring)
- [x] 4.10 Visibility: hidden, overflow clipping
- [x] 4.11 ExternalSurface — 3D/video GPU texture compositing slot
- [x] 4.12 PaintChunk grouping by PropertyState
- [x] 4.13 Arc::ptr_eq fragment caching — skip repaint when unchanged

---

## Milestone 5: Platform ✅ (basic) / 🔲 (advanced)

> App, Window, View — the bridge to the user's screen.

### M5.1 Renderer (vello + wgpu) ✅
- [x] 5.1.1 VelloRenderer — GPU init (Instance, Adapter, Device, Queue)
- [x] 5.1.2 VelloSurface — per-window render target
- [x] 5.1.3 DisplayList → vello Scene conversion
- [x] 5.1.4 DrawCommand::Rect → vello fill
- [x] 5.1.5 DrawCommand::RoundedRect → vello fill
- [x] 5.1.6 DrawCommand::Border → 4-edge rect fills
- [x] 5.1.7 DrawCommand::Line → vello stroke
- [x] 5.1.8 Clip/Opacity/Transform stacks in scene builder
- [x] 5.1.9 DrawCommand::Text → vello draw_glyphs (hinting + normalized_coords)
- [x] 5.1.10 DrawCommand::RoundedBorderRing → EvenOdd compound path
- [x] 5.1.11 DrawCommand::Outline → EvenOdd compound path
- [ ] 5.1.12 DrawCommand::Image → wgpu texture upload + vello image
- [ ] 5.1.13 DrawCommand::BoxShadow → blur compositing
- [ ] 5.1.14 ExternalSurface compositing (3D/video)
- [ ] 5.1.15 Border dashed/dotted/double rendering

### M5.2 Window (winit) ✅ (basic) / 🔧 (refactor in progress)
- [x] 5.2.1 WinitApp — process-level, owns EventLoop
- [x] 5.2.2 Window creation + lifecycle
- [x] 5.2.3 View system — per-View thread with Doc + Scheduler + FontSystem
- [x] 5.2.4 Input event translation (winit → Kozan InputEvent)
- [x] 5.2.5 AppHandler — OS events → ViewEvent routing
- [x] 5.2.6 Cross-thread DisplayList sharing (Arc<Mutex<Option<Arc<>>>>)

### M5.6 Platform Architecture Refactor ✅ (core) / 🔧 (vello upgrade)
- [x] 5.6.1 WindowManager<R: Renderer> in kozan-platform — owns all windows + renderer, routes events
- [x] 5.6.2 WindowPipeline — spawns view + render threads, owns channels
- [x] 5.6.3 RenderLoop in kozan-platform — compositor + vsync loop (not winit)
- [x] 5.6.4 InputState per window — cursor + modifiers in kozan-platform
- [x] 5.6.5 kozan-winit as dumb OS adapter — zero state, zero logic, single run() function
- [x] 5.6.6 Message passing (mpsc) replaces Arc<Mutex> for thread communication
- [x] 5.6.7 Merge kozan-renderer into kozan-platform/renderer/
- [x] 5.6.8 Per-window render thread with own vsync loop
- [x] 5.6.9 kozan::App facade — user never sees winit/vello, just App::new().window().run()
- [x] 5.6.10 Compositor owns scroll offsets exclusively — commit never overwrites (no flicker)
- [x] 5.6.11 Compositor hit test for wheel scroll — cursor position → deepest scrollable layer
- [ ] 5.6.12 Upgrade vello 0.4 → 0.8 (render_to_texture, push_clip_layer, wgpu 28)

### M5.3 Event Bridge ✅
- [x] 5.3.1 Main → View routing (winit event → correct View thread)
- [x] 5.3.2 View → Main requests (resize, title, new window, redraw)
- [x] 5.3.3 RedrawRequested ↔ FrameScheduler integration

### M5.4 Text Rendering ✅
- [x] 5.4.1 Parley glyph shaping → ShapedTextRun → vello draw_glyphs
- [x] 5.4.2 Glyph advance accumulation (correct x positioning)
- [x] 5.4.3 Variable font axes (normalized_coords: wght, wdth, etc.)
- [x] 5.4.4 Font hinting for screen-quality text
- [x] 5.4.5 RTL text + Arabic shaping (HarfRust, automatic bidi)
- [x] 5.4.6 Text node style inheritance from parent ComputedValues
- [x] 5.4.7 CSS font properties: font-family, font-weight, font-size, font-style
- [x] 5.4.8 CSS text properties: letter-spacing, word-spacing
- [x] 5.4.9 CSS line-height resolution (Normal, Number, Length)
- [x] 5.4.10 Text measurement with correct font (FontQuery, not defaults)

### M5.5 Working Examples ✅
- [x] 5.5.1 hello-world — text + colored boxes, English + Arabic, both DOM APIs
- [x] 5.5.2 dashboard — CSS-driven, Cairo font, async animations, flex layout, bar charts
- [x] 5.5.3 dashboard.html — Chrome reference for side-by-side comparison
- [x] 5.5.4 dashboard.css — shared stylesheet (one source of truth for both)

---

## Milestone 6: Scrolling ✅ (core) / 🔧 (in progress)

> Chrome-architecture scroll system — 5 independent subsystems, compositor-ready.

### M6.1 Scroll Core ✅
- [x] 6.1.1 ScrollNode — per-element geometry (container, content, axis flags)
- [x] 6.1.2 ScrollTree — parent-child topology, `sync()` from fragment tree, `root_scroller()`
- [x] 6.1.3 ScrollOffsets — mutable offset per node (Storage-based, compositor-ready)
- [x] 6.1.4 ScrollController — chain dispatch with clamping + delta bubbling
- [x] 6.1.5 Scrollbar geometry + styling (ScrollbarStyle, ThumbPlacement, min thumb)
- [x] 6.1.6 DirtyPhases — scroll invalidates paint only (skips style+layout at vsync rate)
- [x] 6.1.7 overflow:visible/hidden/scroll/auto — all 4 values, `clips()` + `is_user_scrollable()`
- [x] 6.1.8 scrollable_overflow computed from child extents (border-box → padding-box conversion)

### M6.2 Scroll Input ✅
- [x] 6.2.1 Mouse wheel → scroll (WheelDelta::Lines with PIXELS_PER_LINE=40)
- [x] 6.2.2 Trackpad → scroll (WheelDelta::Pixels, pixel-precise, no scaling)
- [x] 6.2.3 Keyboard scroll — arrows (40px), PgUp/Dn (viewport-40px), Home/End (max), Space
- [x] 6.2.4 DefaultAction system — Chrome's `DefaultKeyboardEventHandler` pattern
- [x] 6.2.5 preventDefault() blocks scroll (dispatch_event returns bool)
- [x] 6.2.6 ScrollEvent dispatched to DOM nodes after offset changes
- [x] 6.2.7 WheelDelta sign convention — platform positive=up negated to offset positive=down (W3C)
- [x] 6.2.8 Hover suppression during scroll (Chrome behavior, prevents :hover flash)

### M6.3 Scroll Paint + Hit Test ✅
- [x] 6.3.1 Painter applies scroll offset as translate inside clip
- [x] 6.3.2 HitTester adjusts coordinates by +scroll_offset (inverse of paint)
- [x] 6.3.3 Paint scroll lookup gated on `is_user_scrollable` (overflow:hidden clips but doesn't translate)
- [x] 6.3.4 Hit test cache with invalidation on scroll offset changes

### M6.4 Scroll Advanced 🔲
- [ ] 6.4.1 Touch scroll — TouchStart/Move/End → gesture recognition → scroll
- [ ] 6.4.2 Scrollbar interaction — click track to jump, drag thumb
- [ ] 6.4.3 overscroll-behavior CSS (auto/contain/none)
- [ ] 6.4.4 scroll-snap-type + scroll-snap-align
- [ ] 6.4.5 Programmatic scroll API — element.scrollTo(), scrollBy(), scrollTop
- [ ] 6.4.6 Smooth scroll (CSS scroll-behavior) — eased animation on compositor
- [ ] 6.4.7 Scroll-linked animations
- [ ] 6.4.8 position: sticky (scroll-aware positioning)

---

## Milestone 7: Input & Interaction ✅ (basic) / 🔲 (advanced)

> Hit testing, hover, focus, cursor, selection.

### M7.1 Hit Testing ✅
- [x] 7.1.1 Point → element (fragment tree walk, deepest node)
- [x] 7.1.2 z-index aware (reverse child order = last-painted checked first)
- [x] 7.1.3 Clip-aware (overflow:hidden/scroll clips hit test)
- [x] 7.1.4 Scroll-offset-aware (HitTester takes &ScrollOffsets)
- [x] 7.1.5 HitTestCache — skip re-walk when cursor barely moved (<0.5px)
- [ ] 7.1.6 pointer-events: none support
- [ ] 7.1.7 Compositor hit testing — inverse transform matrix (see M9.5)

### M7.2 Hover ✅
- [x] 7.2.1 ElementState::HOVER set/cleared on DOM nodes
- [x] 7.2.2 Hover chain — hovering child hovers all ancestors (:hover on .card works)
- [x] 7.2.3 MouseEnter/Leave + MouseOver/Out dispatch on hover change
- [x] 7.2.4 Stylo reads ElementState → :hover CSS rules apply automatically

### M7.3 Active ✅
- [x] 7.3.1 ElementState::ACTIVE on mousedown, cleared on mouseup
- [x] 7.3.2 Active chain — :active propagates up ancestors
- [x] 7.3.3 Click detection — mousedown + mouseup on same element
- [x] 7.3.4 DblClick, ContextMenu dispatch

### M7.4 Default Actions ✅
- [x] 7.4.1 DefaultAction enum (Scroll, FocusNext, FocusPrev, Activate)
- [x] 7.4.2 Chrome pattern: dispatch DOM event first → if !prevented → default action
- [x] 7.4.3 InputContext with viewport_height + scroll_tree for keyboard scroll

### M7.5 Focus System 🔲
> W3C HTML Living Standard §6.6 + CSS Selectors Level 4 + Chrome behavior.
> Infrastructure exists: ElementState::FOCUS in Stylo, FocusEvent/BlurEvent defined,
> NodeFlags::IS_FOCUSABLE on form controls, EventHandler.focused_node tracked.
> Everything scaffolded — needs wiring.

#### M7.5.1 Core Focus State
- [ ] 7.5.1.1 Wire EventHandler.focused_node (currently dead code)
- [ ] 7.5.1.2 Document.set_focus_state() — set ElementState::FOCUS on element
- [ ] 7.5.1.3 Document.set_focus_within_chain() — set FOCUS_WITHIN on all ancestors to root
- [ ] 7.5.1.4 Focus-visible heuristic — FocusSource { Keyboard, Pointer, Programmatic }
- [ ] 7.5.1.5 ElementState::FOCUSRING set based on heuristic (always for text inputs)
- [ ] 7.5.1.6 Clear focus state on old element + all ancestors before setting new

#### M7.5.2 Click-to-Focus (mousedown default action)
- [ ] 7.5.2.1 mousedown → walk up to nearest focusable ancestor → run focus steps
- [ ] 7.5.2.2 preventDefault() on mousedown blocks focus change (Chrome behavior)
- [ ] 7.5.2.3 Non-focusable elements (plain div without tabindex) don't steal focus
- [ ] 7.5.2.4 Focus source = Pointer → no focus ring (FOCUSRING not set)
- [ ] 7.5.2.5 Clicking already-focused element is a no-op

#### M7.5.3 Focus Events (W3C UI Events)
- [ ] 7.5.3.1 Dispatch order: blur(A) → focusout(A) → focus(B) → focusin(B)
- [ ] 7.5.3.2 blur/focus do NOT bubble; focusin/focusout DO bubble
- [ ] 7.5.3.3 relatedTarget — the other element in the focus transfer (null at window boundary)
- [ ] 7.5.3.4 None are cancelable
- [ ] 7.5.3.5 FocusEvent/BlurEvent/FocusInEvent/FocusOutEvent already defined — need dispatch

#### M7.5.4 Pseudo-class Matching
- [ ] 7.5.4.1 :focus — matches focused element (Stylo already maps ElementState::FOCUS)
- [ ] 7.5.4.2 :focus-visible — matches when FOCUSRING set (keyboard nav, text inputs)
- [ ] 7.5.4.3 :focus-within — matches element + all ancestors (Stylo maps FOCUS_WITHIN)
- [ ] 7.5.4.4 UA stylesheet: `:focus-visible { outline: 2px solid rgb(59, 130, 246); outline-offset: 2px; }`
- [ ] 7.5.4.5 UA stylesheet: `:focus:not(:focus-visible) { outline: none; }` (suppress ring on click)

#### M7.5.5 Tab Navigation (Sequential Focus — W3C §6.6.3)
- [ ] 7.5.5.1 Collect all focusable elements in DOM tree order
- [ ] 7.5.5.2 tabindex > 0 first (sorted ascending by value), then tabindex=0 (DOM order)
- [ ] 7.5.5.3 tabindex < 0 = programmatically focusable but excluded from tab order
- [ ] 7.5.5.4 Tab → next in sequence, Shift+Tab → previous, wrap at boundaries
- [ ] 7.5.5.5 tabindex attribute parsing in element creation
- [ ] 7.5.5.6 Handle DefaultAction::FocusNext/FocusPrev (currently stubs in FrameWidget)
- [ ] 7.5.5.7 Focus source = Keyboard → show focus ring (FOCUSRING set)

#### M7.5.6 Focus Ring Rendering
- [ ] 7.5.6.1 :focus-visible outline via CSS (outline painting already in M4.9)
- [ ] 7.5.6.2 outline-offset support for focus ring spacing
- [ ] 7.5.6.3 Custom focus styles per element (CSS overrides UA defaults)

#### M7.5.7 Focus + Scroll Integration
- [ ] 7.5.7.1 Keyboard scroll targets focused element's nearest scrollable ancestor (not root)
- [ ] 7.5.7.2 Focus on element scrolls it into view (default behavior)
- [ ] 7.5.7.3 preventScroll option on programmatic focus()
- [ ] 7.5.7.4 scroll-padding / scroll-margin respected during focus scroll

#### M7.5.8 Programmatic Focus API
- [ ] 7.5.8.1 element.focus() — runs full focus steps
- [ ] 7.5.8.2 element.blur() — runs full blur steps, focus moves to viewport
- [ ] 7.5.8.3 document.active_element() — returns currently focused element
- [ ] 7.5.8.4 focus({ focusVisible: true/false }) — override focus-visible heuristic
- [ ] 7.5.8.5 Focusable areas: form controls + elements with tabindex + scrollable regions

### M7.6 Cursor 🔲
- [ ] 7.6.1 CSS cursor property → OS cursor
- [ ] 7.6.2 Cursor changes on hover (pointer, text, resize, etc.)

### M7.7 Text Selection 🔲
- [ ] 7.7.1 Click-to-place caret
- [ ] 7.7.2 Click-drag to select
- [ ] 7.7.3 Double-click word select, triple-click line select
- [ ] 7.7.4 Selection rendering (highlight color)
- [ ] 7.7.5 Copy to clipboard

### M7.8 Text Input 🔲
- [ ] 7.8.1 IME integration (CJK, Arabic input methods)
- [ ] 7.8.2 Editable text fields (input, textarea)
- [ ] 7.8.3 Undo/redo

---

## Milestone 8: Visual Effects 🔲

> Gradients, shadows, filters — the eye candy.

### M8.1 Backgrounds
- [ ] 8.1.1 linear-gradient()
- [ ] 8.1.2 radial-gradient()
- [ ] 8.1.3 conic-gradient()
- [ ] 8.1.4 background-image: url()
- [ ] 8.1.5 Multiple backgrounds

### M8.2 Shadows
- [ ] 8.2.1 box-shadow blur rendering (vello blur compositing)
- [ ] 8.2.2 text-shadow
- [ ] 8.2.3 Inset box-shadow

### M8.3 Filters
- [ ] 8.3.1 filter: blur()
- [ ] 8.3.2 filter: brightness/contrast/saturate/grayscale/sepia
- [ ] 8.3.3 backdrop-filter
- [ ] 8.3.4 mix-blend-mode

### M8.4 Clipping & Masking
- [ ] 8.4.1 clip-path (basic shapes + SVG path)
- [ ] 8.4.2 mask-image

---

## Milestone 9: Compositor & Animations 🔧 (in progress)

> Chrome's `cc/` layer — GPU compositing, off-main-thread scroll + animations.
>
> **Architecture**: Main thread (winit + GPU) owns the Compositor. View thread
> (DOM + style + layout + paint) sends committed frames. Compositor handles
> scroll and compositable animations at vsync rate without view thread round-trip.

### M9.1 Compositor Foundation ✅
- [x] 9.1.1 Layer type — bounds, full 4x4 transform matrix, opacity, clip, scroll_offset
- [x] 9.1.2 LayerTree — flat arena of layers with parent-child relationships
- [x] 9.1.3 Layer builder — fragment tree → LayerTree (single O(n) pass)
- [x] 9.1.4 CompositorFrame — display list + scroll offsets for GPU
- [x] 9.1.5 Compositor struct — receives committed frames, produces CompositorFrame
- [x] 9.1.6 FrameOutput — carries DisplayList + LayerTree + ScrollTree (no scroll offsets — compositor owns them)
- [x] 9.1.7 Compositor on render thread — per-window, owns scroll state exclusively

### M9.2 Layer Promotion 🔧
- [x] 9.2.1 overflow:scroll/auto → own layer (LayerTreeBuilder)
- [ ] 9.2.2 will-change:transform/opacity/filter → own layer (eager, before animation)
- [ ] 9.2.3 Active CSS animation on compositable property → auto-promote
- [ ] 9.2.4 3D transform (translate3d, perspective) → own layer
- [ ] 9.2.5 `<video>`, `<canvas>`, ExternalSurface → own layer
- [ ] 9.2.6 Layer caching — only re-raster dirty layers

### M9.3 Scroll on Compositor ✅ (core) / 🔲 (smooth/animations)
- [x] 9.3.1 Clone for Storage, ScrollTree, ScrollOffsets, ScrollNode
- [x] 9.3.2 Compositor exclusively owns scroll offsets — commit never overwrites
- [x] 9.3.3 `try_scroll()` — compositor-side ScrollController, no view thread round-trip
- [x] 9.3.4 Tag scroll transforms in display items (TransformData.scroll_node)
- [x] 9.3.5 Renderer overrides baked-in offsets with compositor's ScrollOffsets
- [x] 9.3.6 Wheel events routed to compositor via RenderEvent::Scroll { delta, point }
- [x] 9.3.7 Sync scroll offsets back to view thread via ViewEvent::ScrollSync (mpsc, not Arc<Mutex>)
- [x] 9.3.8 Compositor hit test — cursor point → deepest scrollable layer (hit_test_scroll_target)
- [ ] 9.3.9 Smooth scroll animation — ScrollOffsetAnimationCurve (ease-in-out, delta-based duration)
- [ ] 9.3.10 Fling / momentum — PhysicsBasedFlingCurve (kinematic deceleration, 2s max)
- [ ] 9.3.11 Trackpad stays instant (WheelDelta::Pixels = no animation)
- [ ] 9.3.12 Mid-scroll retargeting — new wheel tick adjusts animation target smoothly
- [ ] 9.3.13 Percent-based line→pixel conversion (Chrome's modern approach)

### M9.4 Transform System
- [ ] 9.4.1 4x4 transform matrix type (kozan-primitives) — translate, rotate, scale, skew, perspective
- [ ] 9.4.2 Matrix inverse computation (for hit testing)
- [ ] 9.4.3 Property trees — cumulative transforms from layer to root
- [ ] 9.4.4 transform-origin
- [ ] 9.4.5 Full 2D transforms (rotate, scale, skew) in paint + display items
- [ ] 9.4.6 3D transforms (perspective, rotateX/Y/Z)

### M9.5 Compositor Hit Testing (Chrome's two-path system)

> Chrome: compositor does fast-path hit testing with inverse transforms.
> Falls back to main thread for perspective, non-invertible matrices, complex clips.

- [ ] 9.5.1 HitTestRegionList — layer bounds + inverse transform per layer
- [ ] 9.5.2 Compositor hit test — screen point × inverse matrix → layer local space
- [ ] 9.5.3 During CSS transform animation: hit test uses current animated value (no main thread wait)
- [ ] 9.5.4 Slow path fallback to view thread for perspective, SVG masks, non-invertible matrices

### M9.6 Compositor-Driven Animations

> Only compositable properties: transform, opacity, filter, backdrop-filter.
> These run on compositor at vsync rate — main thread not involved during playback.

- [ ] 9.6.1 AnimationCurve trait — `value_at(t)` for float, color, transform
- [ ] 9.6.2 Easing functions — cubic-bezier, ease, ease-in/out, linear, steps
- [ ] 9.6.3 KeyframeModel — property + curve + timing + direction + iteration
- [ ] 9.6.4 AnimationHost on compositor — tick all animations per vsync
- [ ] 9.6.5 Main thread → compositor sync: push animation state during commit
- [ ] 9.6.6 Compositor applies animated values to layer properties (transform matrix, opacity float)

### M9.7 Main-Thread CSS Transitions + @keyframes

> Non-compositable properties (color, background, width, etc.) run on view thread.
> Goes through style→layout→paint pipeline every frame.

- [ ] 9.7.1 Wire Stylo `animation_rule()` + `transition_rule()` (currently return None)
- [ ] 9.7.2 Transition state machine — detect property change → create transition
- [ ] 9.7.3 @keyframes resolution from Stylo
- [ ] 9.7.4 Main-thread animation tick before style recalc
- [ ] 9.7.5 Interpolation for non-compositable properties (color, length, etc.)

### M9.8 Animation Property Classification

> Chrome's rule: if the property can change WITHOUT repainting, compositor handles it.

| Property | Thread | Reason |
|----------|--------|--------|
| transform | Compositor | Layer matrix change only |
| opacity | Compositor | Alpha blend change only |
| filter | Compositor | GPU filter pass change |
| backdrop-filter | Compositor | GPU filter pass change |
| color | Main thread | Requires repaint |
| background-color | Main thread | Requires repaint |
| width/height | Main thread | Requires relayout + repaint |
| box-shadow | Main thread | Requires repaint |
| border-radius | Main thread | Requires repaint |

---

## Milestone 10: Advanced CSS 🔲

> CSS features beyond basic layout.

### M10.1 Text
- [ ] 10.1.1 text-overflow: ellipsis
- [ ] 10.1.2 word-break, overflow-wrap, hyphens
- [ ] 10.1.3 text-transform (uppercase, lowercase, capitalize)
- [ ] 10.1.4 white-space: pre, pre-wrap, pre-line
- [ ] 10.1.5 writing-mode (vertical text)
- [ ] 10.1.6 tab-size

### M10.2 Pseudo-elements
- [ ] 10.2.1 ::before, ::after (DOM expansion during style)
- [ ] 10.2.2 ::selection (highlight rendering)
- [ ] 10.2.3 ::placeholder
- [ ] 10.2.4 list-style (bullets, numbers via ::marker)

### M10.3 Advanced Selectors
- [ ] 10.3.1 Pseudo-classes (:hover, :active, :nth-child, etc.)
- [ ] 10.3.2 CSS variables (custom properties)
- [ ] 10.3.3 calc() expressions
- [ ] 10.3.4 @media queries
- [ ] 10.3.5 @container queries

### M10.4 Table Layout
- [ ] 10.4.1 display: table / table-row / table-cell
- [ ] 10.4.2 Table layout algorithm
- [ ] 10.4.3 border-collapse, border-spacing

### M10.5 Position
- [ ] 10.5.1 position: fixed (viewport-anchored)
- [ ] 10.5.2 position: sticky (scroll-aware)

---

## Milestone 11: Image & Media 🔲

> Images, video, canvas — rich media content.

- [ ] 11.1 Image loading + decoding (PNG, JPEG, WebP, SVG)
- [ ] 11.2 wgpu texture upload
- [ ] 11.3 object-fit (fill, contain, cover, none, scale-down)
- [ ] 11.4 Image caching
- [ ] 11.5 `<canvas>` element with wgpu 3D scene (ExternalSurface)
- [ ] 11.6 `<video>` element (platform video decoder)

---

## Milestone 12: Developer Tools 🔲

> Chrome DevTools-level inspection and debugging.

### M12.1 Element Inspector
- [ ] 12.1.1 DOM tree view (expandable/collapsible)
- [ ] 12.1.2 Click-to-select element (highlight on hover)
- [ ] 12.1.3 Computed style panel
- [ ] 12.1.4 Box model visualizer (margin/border/padding/content)
- [ ] 12.1.5 Layout overlay (flex/grid lines)

### M12.2 Paint Debugger
- [ ] 12.2.1 DisplayList::dump() — human-readable text dump
- [ ] 12.2.2 Fragment::dump() — layout tree with coordinates
- [ ] 12.2.3 Paint flashing (highlight repainted regions)
- [ ] 12.2.4 Layer boundaries visualization

### M12.3 Performance Profiler
- [ ] 12.3.1 Frame time breakdown (style / layout / paint / render)
- [ ] 12.3.2 Element count + display item count per frame
- [ ] 12.3.3 Tracing integration (chrome://tracing format)
- [ ] 12.3.4 Memory usage tracking

### M12.4 Testing Infrastructure
- [ ] 12.4.1 Pipeline tests (DOM → Style → Layout → Paint → verify)
- [ ] 12.4.2 Headless render to PNG (visual regression)
- [ ] 12.4.3 Performance benchmarks (100 / 1000 / 10000 elements)
- [ ] 12.4.4 Reference test screenshots

---

## Milestone 13: Accessibility 🔲

> Screen readers, keyboard navigation, a11y tree.

- [ ] 13.1 Accessibility tree parallel to DOM
- [ ] 13.2 Role, name, description mapping (ARIA)
- [ ] 13.3 Platform a11y API (UIA on Windows, NSAccessibility on macOS)
- [ ] 13.4 Live regions (aria-live)
- [ ] 13.5 High contrast mode support

---

## Milestone 14: Framework Support 🔲

> Prove the platform works — build frameworks on top.

- [ ] 14.1 Custom element registration
- [ ] 14.2 Shadow DOM equivalent (encapsulation)
- [ ] 14.3 Reactive framework demo (signals)
- [ ] 14.4 Retained-mode framework demo (React-like)
- [ ] 14.5 Framework interop (two frameworks in same tree)

---

## Known Limitations

### Rendering
- **Subpixel AA**: vello renders grayscale only. Chrome uses ClearType (Windows). Text appears thinner.
- **text-decoration-thickness**: Stylo marks it gecko-only (`engine = "gecko"`). No technical reason — needs upstream PR or Stylo fork.

### Stylo Servo-mode Gaps
Some CSS properties are gecko-only in Stylo 0.14. Can be fixed by forking Stylo and removing `engine = "gecko"` flags.
