# Chrome Canvas 2D Context Lifecycle Research

> Research from Chromium source (March 2026). Focused on: context ownership,
> how drawing reaches the compositor, the element-context relationship,
> and frame boundaries.

---

## 1. Context Ownership: `getContext('2d')` Creates Once, Returns Forever

### Chrome Architecture

In Chrome, `HTMLCanvasElement` owns exactly one rendering context for its entire lifetime.
The relationship is established on the first `getContext()` call and never changes.

```cpp
// html_canvas_element.h
class HTMLCanvasElement : public HTMLElement,
                          public CanvasRenderingContextHost {
  Member<CanvasRenderingContext> context_;  // THE one context, created once
};

// html_canvas_element.cc
CanvasRenderingContext* HTMLCanvasElement::GetCanvasRenderingContext(
    const String& type, const CanvasContextCreationAttributesCore& attrs) {
  // If context already exists, return it (or null if type mismatch)
  if (context_) {
    if (context_->GetContextType() == context_type)
      return context_.Get();    // <-- SAME OBJECT FOREVER
    return nullptr;             // Can't switch types
  }

  // First call: create and store
  context_ = factory->Create(this, attrs);
  return context_.Get();
}
```

**Key rules:**
1. First `getContext("2d")` creates and stores `CanvasRenderingContext2D`.
2. Subsequent `getContext("2d")` returns the **same pointer**.
3. `getContext("webgl")` on a 2D canvas returns `null` — you cannot switch types.
4. The context is GC-traced from the element (prevents collection while element lives).
5. The context is **never dropped/recreated** during the element's lifetime.

### What Kozan Does Differently (Current Gap)

Kozan's `HtmlCanvasElement::context_2d()` creates a **new** `CanvasRenderingContext2D`
on every call:

```rust
// html_canvas_element.rs (CURRENT)
pub fn context_2d(&self) -> kozan_canvas::CanvasRenderingContext2D {
    kozan_canvas::CanvasRenderingContext2D::new()  // New each time!
}
```

This is a significant deviation from the spec. In Chrome:
- The element **stores** the context.
- The context is created once and reused.
- The context holds a back-reference to the element.

### Chrome's Correct Model (For Kozan to Follow)

```
HTMLCanvasElement  ──owns──>  CanvasRenderingContext2D
      ^                              |
      |                              |
      └──────── back-ref ────────────┘
           (CanvasRenderingContextHost*)
```

The element stores `context_: Option<CanvasRenderingContext2D>` in its arena data.
`getContext("2d")` returns a reference/handle to the stored context. The context
holds a handle back to the element so it can call `DidDraw()`, `GetSize()`, etc.

---

## 2. How Drawing Reaches the Compositor (No "Commit" Needed)

### Chrome's Flow: Implicit Flush During Paint

The critical insight is: **the user never calls "commit" or "flush"**. The browser's
paint system reads the recording buffer implicitly during the paint phase.

```
User calls ctx.fillRect(...)
    |
    v
Canvas2DRecorderContext::fillRect()
    |
    v
GetOrCreatePaintCanvas() -> MemoryManagedPaintCanvas
    |   (wraps RecordPaintCanvas, backed by PaintOpBuffer)
    v
PaintOpBuffer::push<DrawRectOp>(rect, flags)
    |
    v  [ops accumulate in the mutable buffer]
    |
    ═══════════════════════════════════════════
    |   FRAME BOUNDARY: browser decides to paint
    |
    v
HTMLCanvasElement participates in the paint phase
    |
    v
CanvasResourceProvider::FlushCanvas()
    |   Called by the paint system, NOT by the user
    v
PaintOpBuffer::ReleaseAsRecord() -> PaintRecord (immutable snapshot)
    |
    v
RasterRecord(record) -> record.Playback(SkCanvas*)
    |
    v
Actual Skia drawing on GPU/CPU surface
    |
    v
Compositor reads the surface as a texture layer
```

### The Three Key Transfer Points

**Transfer 1: User code -> PaintOpBuffer (immediate, during JS execution)**
- Each `ctx.fillRect()`, `ctx.arc()`, etc. immediately appends to the `PaintOpBuffer`.
- This is synchronous — the op is recorded the instant the JS method runs.
- No batching, no deferred recording. The buffer is always "up to date."

**Transfer 2: PaintOpBuffer -> PaintRecord (during paint phase, browser-initiated)**
- `CanvasResourceProvider::FlushCanvas()` is called by the browser's paint system.
- `PaintOpBuffer::ReleaseAsRecord()` produces an immutable `PaintRecord`.
- The buffer is then reset for the next frame's recording.

**Transfer 3: PaintRecord -> GPU surface (during rasterization)**
- `RasterRecord(record)` calls `record.Playback(SkCanvas*)`.
- Each `PaintOp` is replayed onto a real Skia canvas backed by a GPU texture.
- This texture becomes the canvas element's compositing layer.

### What Triggers the Flush?

The paint system triggers it, not the user. Specifically:

1. **`HTMLCanvasElement::DidDraw()`** — called after drawing ops to mark the canvas dirty.
   This schedules a compositing update via `GetDocument().GetPage()->Animator().ScheduleVisualUpdate()`.
2. The **animation frame** cycle (requestAnimationFrame or the compositor's vsync) picks up
   the dirty canvas during the paint phase.
3. During paint, `CanvasRenderingContextHost::PaintRenderingResultsToCanvas()` is called,
   which triggers `CanvasResourceProvider::FlushCanvas()`.
4. The flushed `PaintRecord` is stored as the element's paint representation.

### Chrome's DidDraw() Mechanism

```cpp
// In Canvas2DRecorderContext, after every draw operation:
void Canvas2DRecorderContext::DidDraw(const SkIRect& dirty_rect,
                                       CanvasPerformanceMonitor::DrawType draw_type) {
  // 1. Track memory pressure
  GetCanvasPerformanceMonitor().DidDraw(draw_type);

  // 2. Notify the host element that content changed
  Host()->DidDraw(dirty_rect);
}

// In HTMLCanvasElement:
void HTMLCanvasElement::DidDraw(const SkIRect& rect) {
  // Mark this element as needing repaint
  if (auto* context = GetRenderingContext())
    context->DidDraw();

  // Schedule visual update (triggers paint on next frame)
  if (GetDocument().GetPage())
    GetDocument().GetPage()->Animator().ScheduleVisualUpdate();
}
```

This means every `fillRect()` / `stroke()` / etc. implicitly schedules a repaint.
The user never has to say "I'm done drawing." The browser just picks up whatever
is in the buffer at the next paint opportunity.

---

## 3. The Context-Element Relationship: Mutual References

### Chrome's Bidirectional Pointer Design

The element and context **reference each other**:

```cpp
// CanvasRenderingContext (base class for all contexts)
class CanvasRenderingContext : public ScriptWrappable {
  Member<CanvasRenderingContextHost> host_;  // -> back to the element
};

// HTMLCanvasElement (the DOM element)
class HTMLCanvasElement : public HTMLElement,
                          public CanvasRenderingContextHost {
  Member<CanvasRenderingContext> context_;  // -> to the context
};
```

**`CanvasRenderingContextHost`** is the interface the context uses to communicate
back to the element:

```cpp
class CanvasRenderingContextHost {
  virtual gfx::Size Size() const = 0;
  virtual void DidDraw(const SkIRect&) = 0;
  virtual CanvasResourceProvider* GetOrCreateResourceProvider(RasterModeHint) = 0;
  virtual void SetNeedsCompositingUpdate() = 0;
  virtual bool IsOffscreenCanvas() const { return false; }
  // ... etc
};
```

### Why the Back-Reference Exists

The context needs to call back to the element for:

1. **`DidDraw(dirty_rect)`** — after every draw op, notify the element that
   content changed so it can schedule a repaint.
2. **`Size()`** — the context needs to know the canvas dimensions (which live
   on the element's `width`/`height` attributes).
3. **`GetOrCreateResourceProvider()`** — the context needs the backing store
   (GPU texture or software surface) which is managed by the element/host.
4. **`SetNeedsCompositingUpdate()`** — when the canvas content changes in a way
   that affects compositing (e.g., opacity changes).

### GC and Lifetime

In Chrome (Blink's Oilpan GC):
- The element traces the context (`context_` is a `Member<>` = GC strong reference).
- The context traces the host (`host_` is a `Member<>` = GC strong reference).
- This creates a cycle, but Oilpan handles cycles via mark-and-sweep.
- Neither can be collected while the other is reachable.

In Kozan (no GC), this would be modeled as:
- Element's arena data stores the context (or a handle to it).
- Context stores a `Handle` back to the element.
- Both live in the Document's arena — dropped together when the element is removed.

---

## 4. Frame Boundaries: Canvas Content Persists

### Chrome's Retained-Mode Canvas

Canvas 2D is **retained** between frames. Key behaviors:

1. **Content persists.** If you draw a red rectangle in frame 1 and do nothing in
   frame 2, the red rectangle is still visible. The canvas is NOT cleared between frames.

2. **The recording accumulates.** Between flush points, all ops accumulate in the
   `PaintOpBuffer`. If you call `fillRect()` 100 times before the next paint,
   all 100 ops are in the buffer.

3. **Flush resets the recording.** When `FlushCanvas()` extracts the `PaintRecord`,
   the `PaintOpBuffer` is reset. But the **rasterized result** (the GPU texture)
   persists. Next frame's drawing happens on top of the previous texture content.

4. **`clearRect()` is the only way to erase.** The user must explicitly clear
   (typically `ctx.clearRect(0, 0, canvas.width, canvas.height)` at the start
   of each animation frame).

### The Two Persistence Layers

```
Layer 1: PaintOpBuffer (mutable, reset on flush)
  - Accumulates ops between paint phases
  - Flushed to PaintRecord at each paint cycle
  - After flush, buffer is empty for next frame

Layer 2: GPU/CPU Surface (persistent, composited)
  - The rasterized result of playing back PaintRecords
  - Content persists across frames
  - New ops draw ON TOP of previous content
  - Only cleared by explicit clearRect() or canvas resize
```

### Frame Timeline

```
Frame 1:
  JS: ctx.fillRect(0, 0, 100, 100)  // red rect
  Paint: FlushCanvas() -> PaintRecord[FillRect(red)]
  Raster: Playback -> GPU texture has red rect
  Display: show red rect

Frame 2:
  JS: ctx.fillRect(50, 50, 100, 100)  // blue rect
  Paint: FlushCanvas() -> PaintRecord[FillRect(blue)]
  Raster: Playback onto SAME texture -> red + blue rects
  Display: show both rects (red is still there!)

Frame 3:
  JS: (nothing)
  Paint: FlushCanvas() -> empty PaintRecord
  Raster: no-op, texture unchanged
  Display: still shows both rects
```

### Canvas Resize Clears Content

Per spec, setting `canvas.width` or `canvas.height` (even to the same value)
clears the canvas:

```cpp
void HTMLCanvasElement::SetWidth(unsigned value) {
  // ... set attribute ...
  Reset();  // Clears the backing store!
}
```

---

## 5. Implications for Kozan's Architecture

### Current Model vs. Chrome Model

| Aspect | Chrome | Kozan (Current) |
|--------|--------|-----------------|
| Context storage | Element stores context in `context_` field | Context created fresh each call |
| Context lifetime | Once per element, never recreated | Ephemeral, user creates/discards |
| Back-reference | Context -> Element via `CanvasRenderingContextHost` | None |
| Flush trigger | Implicit via `DidDraw()` -> paint phase | Explicit `commit_recording()` by user |
| Content persistence | GPU texture retains content across frames | Recording is replaced wholesale |
| Recording lifecycle | Buffer accumulates, flushed at paint, buffer reset | User takes recording, replaces atomically |

### Kozan's Explicit Commit Model is Actually Fine

Chrome's implicit `DidDraw()` -> paint phase model works because:
1. JS runs on the main thread, same as paint.
2. The browser controls the frame lifecycle.
3. The paint system knows when to read the buffer.

Kozan's explicit `commit_recording()` model works because:
1. The user controls when the recording is finalized.
2. The `Arc<CanvasRecording>` is thread-safe for the render thread.
3. The layout/paint pipeline reads `CanvasData.recording` during paint.

**However**, the missing piece is **context persistence on the element**. The
element should store the context so that:
- `getContext("2d")` returns the same context.
- The context accumulates drawing across frames.
- The context can call back to the element for `DidDraw()` / size queries.

### Recommended Architecture Change

```rust
/// Element-specific data for `<canvas>`.
pub struct CanvasData {
    pub canvas_width: f32,
    pub canvas_height: f32,
    /// The committed recording for paint to consume.
    pub recording: Option<Arc<CanvasRecording>>,
    /// The rendering context, created on first getContext() call.
    /// Chrome: `Member<CanvasRenderingContext> context_`
    pub context: Option<CanvasRenderingContext2D>,
}

impl HtmlCanvasElement {
    /// Get or create the 2D rendering context.
    /// Chrome: HTMLCanvasElement::GetCanvasRenderingContext("2d")
    pub fn get_context_2d(&self) -> &mut CanvasRenderingContext2D {
        self.handle().write_data::<CanvasData, _>(|data| {
            if data.context.is_none() {
                data.context = Some(CanvasRenderingContext2D::new(/* host handle */));
            }
            // Return mutable reference to the stored context
            data.context.as_mut().unwrap()
        })
    }
}
```

This makes `getContext("2d")` idempotent (matches Chrome exactly) and allows
the context to persist state, path, and recording across calls.

### Paint Phase Flow (How Recording Reaches Display)

Current Kozan pipeline, verified from source:

```
1. User draws:
   ctx.fill_rect(...)  -> CanvasRecording::push(CanvasOp::FillRect{...})

2. User commits:
   canvas.commit_recording(ctx.take_recording())
     -> CanvasData.recording = Some(Arc::new(recording))

3. Layout phase reads it:
   DocumentLayoutView::replaced_content_for(handle)
     -> reads CanvasData.recording
     -> returns Arc<CanvasContent>

4. Fragment stores it:
   BoxFragmentData.replaced_content = Some(Arc<CanvasContent>)

5. Paint phase emits it:
   Painter::paint_replaced_content(data)
     -> content.to_draw_command() -> DrawCommand::Canvas{recording, w, h}
     -> emit(DisplayItem::Draw(cmd))

6. Vello scene builder replays it:
   match DrawCommand::Canvas{..} =>
     VelloCanvasPlayer::play(scene, recording, transform, w, h)
       -> iterates CanvasOp variants -> scene.fill() / scene.stroke() / etc.
```

This pipeline is architecturally correct and matches Chrome's separation.
The only deviation is the explicit commit vs. implicit DidDraw() — which is
a reasonable design choice for Kozan's non-browser use case.
