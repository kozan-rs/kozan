# Chrome Canvas 2D Architecture Research

> Research from Chromium source (March 2026). Source files examined:
> - `cc/paint/paint_canvas.h` — Abstract recording canvas
> - `cc/paint/paint_record.h` — Immutable recorded operations
> - `cc/paint/paint_recorder.h` — Recording session manager
> - `cc/paint/record_paint_canvas.h` — Recording implementation
> - `cc/paint/paint_op.h` — All paint operation types
> - `cc/paint/paint_op_buffer.h` — Mutable op storage + playback
> - `third_party/blink/.../canvas_rendering_context_2d.h`
> - `third_party/blink/.../base_rendering_context_2d.h`
> - `third_party/blink/.../canvas_2d_recorder_context.h` / `.cc`
> - `third_party/blink/.../canvas_rendering_context_2d_state.h`
> - `third_party/blink/.../html_canvas_element.h`
> - `third_party/blink/.../canvas_rendering_context.h`
> - `third_party/blink/.../canvas_rendering_context_host.h`
> - `third_party/blink/.../canvas_resource_provider.h`

---

## 1. The Recording Layer: `cc::PaintCanvas` / `cc::PaintRecord`

### Architecture

Chrome does NOT execute Canvas 2D drawing commands immediately. Instead, it
**records** them into an op buffer and replays them later during compositing.

```
User JS calls ctx.fillRect(...)
        |
        v
Canvas2DRecorderContext  (blink layer - manages state + API)
        |
        v
MemoryManagedPaintCanvas  (wraps RecordPaintCanvas)
        |
        v
RecordPaintCanvas::drawRect()  -->  push<DrawRectOp>(rect, flags)
        |
        v
PaintOpBuffer  (mutable, append-only list of PaintOp)
        |
        v  (on flush)
PaintRecord  (read-only, shared/refcounted snapshot of PaintOpBuffer)
        |
        v  (on composite/raster)
PaintOpBuffer::Playback(SkCanvas*)  -->  executes ops on real Skia canvas
```

### Key Types

**`PaintCanvas`** (abstract base class):
- The cc/paint wrapper of SkCanvas with a **restricted interface** (only what Chrome uses).
- Two implementations:
  1. **`RecordPaintCanvas`** — records paint commands into a `PaintOpBuffer` (the recording path)
  2. **`SkiaPaintCanvas`** — backed by a real `SkCanvas`, used for rasterization/playback
- This is a **one-way trip**: you can go PaintCanvas -> SkCanvas, but not back.

**`PaintOpBuffer`** (mutable):
- Internal storage for paint operations. Append-only during recording.
- Has a `push<T>(args...)` template method that constructs ops in-place.
- `Playback(SkCanvas*)` replays all ops onto a real Skia canvas.
- `ReleaseAsRecord()` produces an immutable `PaintRecord`.
- Supports serialization/deserialization for GPU process transfer.

**`PaintRecord`** (immutable):
- Read-only wrapper around `PaintOpBuffer`, returned from `PaintOpBuffer::ReleaseAsRecord()`.
- Copy/assignment is cheap (shared underlying data, refcounted).
- Move is preferred over copy.
- This is what gets passed to the compositor.

**`PaintRecorder`**:
- Simple session manager: `beginRecording()` -> returns `PaintCanvas*` -> `finishRecordingAsPicture()` -> returns `PaintRecord`.
- Also has `InspectablePaintRecorder` variant that supports querying clip/CTM during recording.

### Difference from SkPictureRecorder

Chrome replaced Skia's `SkPictureRecorder`/`SkPicture` with its own system because:
1. `PaintOpBuffer` is **mutable** (unlike `SkPicture`)
2. Supports **image replacement** (for GPU transfer cache)
3. Can be **serialized in custom ways** (transfer cache, GPU raster)
4. Enables **custom optimization passes** over the op list

---

## 2. Separation Between Canvas Element and Rendering Context

### Class Hierarchy

```
HTMLCanvasElement  (DOM element, owns the rendering context)
  : HTMLElement
  : CanvasRenderingContextHost  (interface for any context type)
  : cc::TextureLayerClient  (GPU compositing)
      |
      |  getContext("2d") creates:
      v
CanvasRenderingContext2D  (the JS-exposed context object)
  : ScriptWrappable
  : BaseRenderingContext2D  (text, image, pixel data methods)
    : CanvasRenderingContext  (base for all context types: 2d, webgl, webgpu)
    : Canvas2DRecorderContext  (state stack, drawing ops, path ops, style props)
      : CanvasPath  (path building: moveTo, lineTo, arc, etc.)
```

### Layering Details

**`HTMLCanvasElement`** (core/html/canvas/):
- Is the DOM element `<canvas>`.
- Owns exactly one rendering context (2D, WebGL, WebGL2, WebGPU, or BitmapRenderer).
- Implements `CanvasRenderingContextHost` which provides `Size()`, `DidDraw()`, `GetOrCreateResourceProvider()`.
- Implements `cc::TextureLayerClient` for GPU compositing.
- Has `GetCanvasRenderingContext(type, attrs)` — the `getContext()` implementation.

**`CanvasRenderingContext`** (core/html/canvas/):
- Abstract base for ALL canvas context types.
- Defines `CanvasRenderingAPI` enum: `k2D`, `kWebgl`, `kWebgl2`, `kBitmaprenderer`, `kWebgpu`.
- Has `Factory` pattern for context creation.
- Stores reference to `CanvasRenderingContextHost` (the element).

**`Canvas2DRecorderContext`** (modules/canvas/canvas2d/):
- Houses the full Canvas 2D API surface (drawing methods, style properties, state stack).
- Owns `state_stack_: HeapVector<Member<CanvasRenderingContext2DState>>`.
- Delegates actual recording to `MemoryManagedPaintCanvas` via virtual `GetOrCreatePaintCanvas()`.

**`BaseRenderingContext2D`** (modules/canvas/canvas2d/):
- Extends `Canvas2DRecorderContext` + `CanvasRenderingContext`.
- Adds text rendering (fillText, strokeText, measureText), image data (getImageData, putImageData), and context lost/restored handling.
- Provides `GetOrCreateResourceProvider()` for the backing store.

**`CanvasRenderingContext2D`** (modules/canvas/canvas2d/):
- The final concrete class exposed to JavaScript.
- Extends `BaseRenderingContext2D`.
- Has a `Factory` inner class for context creation.
- Adds SVGResourceClient for SVG filter support and hibernation handling.

### Resource Provider Bridge

**`CanvasResourceProvider`** (platform/graphics/):
- Bridges the recording layer to the GPU/CPU backing store.
- Owns a `MemoryManagedPaintRecorder` which provides the `MemoryManagedPaintCanvas`.
- Key method: `FlushCanvas()` — extracts `PaintRecord` from the recorder and calls `RasterRecord(PaintRecord)`.
- Has multiple subclasses:
  - `Canvas2DResourceProviderBitmap` — software rendering via SkSurface
  - `CanvasResourceProviderSharedImage` — GPU-accelerated via SharedImage/GLES

### Flow: getContext("2d") -> draw -> composite

```
1. JS: canvas.getContext("2d")
   -> HTMLCanvasElement::GetCanvasRenderingContext("2d", attrs)
   -> CanvasRenderingContext2D::Factory::Create(host, attrs)
   -> new CanvasRenderingContext2D(element, attrs)

2. JS: ctx.fillRect(10, 20, 100, 50)
   -> Canvas2DRecorderContext::fillRect()
   -> GetOrCreatePaintCanvas() -> CanvasResourceProvider::Canvas()
   -> MemoryManagedPaintCanvas (which is a RecordPaintCanvas)
   -> push<DrawRectOp>(rect, flags)  [into PaintOpBuffer]

3. On composite:
   -> CanvasResourceProvider::FlushCanvas()
   -> PaintOpBuffer::ReleaseAsRecord() -> PaintRecord
   -> RasterRecord(record) -> record.Playback(SkCanvas*)
   -> Actual Skia drawing happens on GPU/CPU surface
```

---

## 3. The Canvas 2D API Surface

### Grouped by Category (from Canvas2DRecorderContext + BaseRenderingContext2D)

**State Management:**
- `save()` — push state onto stack + `SaveOp` to paint canvas
- `restore()` — pop state from stack + `RestoreOp` to paint canvas
- `reset()` — clear everything, reset to initial state
- `beginLayer()` / `endLayer()` — composited layers (newer API)

**Transform:**
- `scale(sx, sy)`
- `rotate(angleInRadians)`
- `translate(tx, ty)`
- `transform(m11, m12, m21, m22, dx, dy)`
- `setTransform(m11, m12, m21, m22, dx, dy)` / `setTransform(DOMMatrixInit)`
- `resetTransform()`
- `getTransform()` -> DOMMatrix

**Style Properties (getters/setters on Canvas2DRecorderContext):**
- `fillStyle` / `strokeStyle` — color, gradient, or pattern
- `lineWidth`
- `lineCap` — "butt", "round", "square"
- `lineJoin` — "miter", "round", "bevel"
- `miterLimit`
- `lineDash` / `lineDashOffset`
- `globalAlpha`
- `globalCompositeOperation`
- `shadowOffsetX` / `shadowOffsetY` / `shadowBlur` / `shadowColor`
- `imageSmoothingEnabled` / `imageSmoothingQuality`
- `font` / `textAlign` / `textBaseline` / `direction`
- `filter`

**Rectangle Drawing:**
- `clearRect(x, y, w, h)`
- `fillRect(x, y, w, h)`
- `strokeRect(x, y, w, h)`

**Path (inherited from CanvasPath):**
- `beginPath()`
- `moveTo(x, y)`
- `lineTo(x, y)`
- `arc(x, y, radius, startAngle, endAngle, counterclockwise)`
- `arcTo(x1, y1, x2, y2, radius)`
- `bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y)`
- `quadraticCurveTo(cpx, cpy, x, y)`
- `ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle, ccw)`
- `rect(x, y, w, h)`
- `roundRect(x, y, w, h, radii)`
- `closePath()`
- `fill(fillRule)` / `fill(Path2D, fillRule)`
- `stroke()` / `stroke(Path2D)`
- `clip(fillRule)` / `clip(Path2D, fillRule)`
- `isPointInPath(x, y)` / `isPointInStroke(x, y)`

**Text (on BaseRenderingContext2D):**
- `fillText(text, x, y [, maxWidth])`
- `strokeText(text, x, y [, maxWidth])`
- `measureText(text)` -> TextMetrics

**Image Drawing:**
- `drawImage(image, dx, dy)`
- `drawImage(image, dx, dy, dw, dh)`
- `drawImage(image, sx, sy, sw, sh, dx, dy, dw, dh)`

**Pixel Manipulation (on BaseRenderingContext2D):**
- `createImageData(sw, sh)` / `createImageData(imagedata)`
- `getImageData(sx, sy, sw, sh)`
- `putImageData(imagedata, dx, dy [, dirtyX, dirtyY, dirtyW, dirtyH])`

**Gradient/Pattern (on Canvas2DRecorderContext):**
- `createLinearGradient(x0, y0, x1, y1)` -> CanvasGradient
- `createRadialGradient(x0, y0, r0, x1, y1, r1)` -> CanvasGradient
- `createConicGradient(startAngle, cx, cy)` -> CanvasGradient
- `createPattern(image, repetition)` -> CanvasPattern

---

## 4. Renderer-Agnostic Recording Pattern

### How Chrome Keeps Canvas Renderer-Agnostic

The key insight is **double indirection**:

1. **Canvas API methods never touch Skia directly.** They call `PaintCanvas` virtual methods.
2. **`PaintCanvas` is abstract.** During recording, the `RecordPaintCanvas` implementation just appends `PaintOp` structs to a `PaintOpBuffer`.
3. **`PaintOp` types are renderer-neutral commands:**

```
PaintOpType enum:
  kAnnotate, kClipPath, kClipRect, kClipRRect,
  kConcat, kCustomData,
  kDrawArc, kDrawArcLite, kDrawColor, kDrawDRRect,
  kDrawImage, kDrawImageRect, kDrawIRect, kDrawLine, kDrawLineLite,
  kDrawOval, kDrawPath, kDrawRecord, kDrawRect,
  kDrawRRect, kDrawRoundRect, kDrawSkottie, kDrawSlug,
  kDrawTextBlob, kDrawVertices,
  kNoop,
  kRestore, kRotate, kSave, kSaveLayer, kSaveLayerAlpha, kSaveLayerFilters,
  kScale, kSetMatrix, kSetNodeId, kTranslate
```

4. **Playback is the only Skia-dependent step.** `PaintOpBuffer::Playback(SkCanvas*)` iterates ops and calls `Op::Raster(op, canvas, params)` — each op type has a static `Raster()` method that translates to Skia calls.

5. **Serialization for GPU process.** PaintOps can be serialized over IPC to the GPU process, where they are deserialized and played back. This is how OOP (Out-Of-Process) rasterization works.

### For Kozan

This pattern maps directly to Kozan's needs:
- **Record phase:** Canvas API calls -> `CanvasRecording` (our `PaintRecord` equivalent) containing `CanvasOp` variants
- **Replay phase:** `CanvasRecording` replayed to vello `Scene` (or any other backend)
- The recording is a plain enum-of-structs buffer, no renderer dependency

---

## 5. The State Stack

### Architecture

Canvas 2D maintains a **stack of state structs** that `save()`/`restore()` push/pop.

**`CanvasRenderingContext2DState`** contains:
- `fill_style_: CanvasStyle` (color, gradient, or pattern)
- `stroke_style_: CanvasStyle`
- `fill_flags_: cc::PaintFlags` (pre-configured for fill operations)
- `stroke_flags_: cc::PaintFlags` (pre-configured for stroke operations, includes lineWidth, lineCap, lineJoin, miterLimit)
- `global_alpha_: double`
- `global_composite_: SkBlendMode`
- `shadow_offset_: gfx::Vector2dF`
- `shadow_blur_: double`
- `shadow_color_: Color`
- `transform_: AffineTransform`
- `is_transform_invertible_: bool`
- `clip_list_: CanvasClipList` (accumulated clip operations)
- `has_clip_: bool`
- `font_: Font` (realized font for text rendering)
- `text_align_, text_baseline_, direction_`
- `image_smoothing_enabled_: bool`
- `image_smoothing_quality_`
- `line_dash_: Vector<double>`
- `line_dash_offset_: double`
- Filter state (CSS and canvas filters)
- Font variants, kerning, stretch, rendering mode

### save()/restore() Implementation

```cpp
// In Canvas2DRecorderContext:

HeapVector<Member<CanvasRenderingContext2DState>> state_stack_;

// Constructor initializes with one default state:
Canvas2DRecorderContext::Canvas2DRecorderContext() {
    state_stack_.push_back(MakeGarbageCollected<CanvasRenderingContext2DState>());
}

void Canvas2DRecorderContext::save() {
    // 1. Clone current state onto stack
    state_stack_.push_back(
        MakeGarbageCollected<CanvasRenderingContext2DState>(GetState(), ...));
    // 2. Record SaveOp to paint canvas
    GetPaintCanvas()->save();
}

void Canvas2DRecorderContext::restore() {
    // 1. Pop state from stack (if not at bottom)
    // 2. Record RestoreOp to paint canvas
    // 3. Apply clip/transform from restored state
    PopStateStack();
    GetPaintCanvas()->restore();
}

// Current state is always the top of the stack:
CanvasRenderingContext2DState& GetState() const {
    return *state_stack_.back();
}
```

### Two-Level State: Blink State + Paint Canvas State

The state stack operates at **two levels simultaneously**:

1. **Blink state** (`CanvasRenderingContext2DState`): Holds style properties (fillStyle, strokeStyle, globalAlpha, shadow, font, etc.). These are used to configure `PaintFlags` before each draw operation.

2. **Paint canvas state** (`save()`/`restore()` on `RecordPaintCanvas`): Tracks the **transform matrix** and **clip regions**. These are recorded as `SaveOp`/`RestoreOp` in the `PaintOpBuffer`.

When `save()` is called, BOTH levels are saved. When `restore()` is called, BOTH are restored. The Blink state restores style properties, while the paint canvas state restores transform+clip.

### PaintCanvasAutoRestore

Chrome also provides `PaintCanvasAutoRestore` — an RAII guard that calls `save()` on construction and `restoreToCount()` on destruction. Used internally for composited draws.

---

## 6. Architectural Patterns for Kozan

### Pattern 1: Recording Buffer (PaintOpBuffer -> CanvasRecording)

```rust
/// Renderer-agnostic canvas recording.
/// Chrome equivalent: cc::PaintRecord (immutable) / cc::PaintOpBuffer (mutable).
pub struct CanvasRecording {
    ops: Vec<CanvasOp>,
}

/// Individual canvas operation.
/// Chrome equivalent: cc::PaintOp subtypes (DrawRectOp, SaveOp, etc.)
pub enum CanvasOp {
    Save,
    Restore,
    SetTransform(Transform2D),
    Translate(f32, f32),
    Rotate(f32),
    Scale(f32, f32),
    ClipRect(Rect, ClipOp),
    ClipPath(Path, ClipOp),
    DrawRect(Rect, PaintStyle),
    DrawRoundRect(Rect, f32, f32, PaintStyle),
    DrawPath(Path, PaintStyle),
    DrawImage(ImageId, Rect, Rect, PaintStyle),
    DrawTextBlob(TextBlob, f32, f32, PaintStyle),
    Clear(Color),
    // ... etc
}
```

### Pattern 2: State Stack (CanvasRenderingContext2DState -> CanvasState)

```rust
/// Canvas 2D drawing state.
/// Chrome equivalent: CanvasRenderingContext2DState.
pub struct CanvasState {
    pub fill_style: CanvasStyle,      // color, gradient, or pattern
    pub stroke_style: CanvasStyle,
    pub line_width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub line_dash: Vec<f32>,
    pub line_dash_offset: f32,
    pub global_alpha: f32,
    pub global_composite: BlendMode,
    pub shadow_offset: (f32, f32),
    pub shadow_blur: f32,
    pub shadow_color: Color,
    pub transform: Transform2D,
    pub clip_path: Option<Path>,
    pub font: Option<Font>,
    pub text_align: TextAlign,
    pub text_baseline: TextBaseline,
    pub image_smoothing: bool,
}

/// Stack of states for save()/restore().
pub struct CanvasStateStack {
    stack: Vec<CanvasState>,  // bottom is always default state
}
```

### Pattern 3: Element/Context/Provider Separation

```
HtmlCanvasElement          (DOM element, owns context)
  |
  v  getContext("2d")
CanvasContext2D             (API surface + state stack)
  |
  v  records to
CanvasRecording             (Vec<CanvasOp>, renderer-agnostic)
  |
  v  replayed by
VelloCanvasPlayer           (replays CanvasRecording -> vello Scene)
```

### Pattern 4: Two-Level save/restore

When save() is called:
1. Clone `CanvasState` and push onto state stack (style properties)
2. Push `CanvasOp::Save` to recording (transform + clip)

When restore() is called:
1. Pop `CanvasState` from stack (restores style properties in Blink-side)
2. Push `CanvasOp::Restore` to recording (restores transform + clip during playback)

This means the recording only contains transform/clip state changes, while style properties
(fillStyle, lineWidth, globalAlpha) are baked into each draw op's `PaintStyle` at record time.
