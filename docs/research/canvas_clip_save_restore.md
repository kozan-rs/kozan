# Canvas 2D Clip Save/Restore: Chrome, Skia, and Vello

## Research Summary

How does `save()`, `clip()`, `restore()` work end-to-end from the HTML Canvas 2D
spec through Chrome/Blink, down to Skia, and what is the correct equivalent in Vello?

---

## 1. The HTML Canvas 2D Spec Model

The spec defines a **drawing state stack**. Each state includes:
- Current transformation matrix (CTM)
- Current clipping region
- Style properties (fillStyle, strokeStyle, lineWidth, globalAlpha, etc.)

`save()` pushes a **copy** of the entire state (including the current clip) onto the stack.
`clip()` **intersects** the current clipping region with the current path (clips only shrink, never grow).
`restore()` pops the stack, restoring the **previous** clip region (effectively "undoing" any clips added since the matching `save()`).

**Key insight**: There is no `unclip()` API. The *only* way to undo a clip is `restore()`.

---

## 2. Chrome/Blink Implementation

### Two-Level Architecture

Chrome has **two layers** of save/restore state:

1. **Blink-side state stack** (`state_stack_` in `BaseRenderingContext2D`)
   - Stores style properties: fillStyle, strokeStyle, lineWidth, font, globalAlpha, etc.
   - Managed by `CanvasRenderingContext2DState` objects.
   - Pure data; no rendering backend awareness.

2. **Paint backend** (`cc::PaintCanvas` / `cc::RecordPaintCanvas`)
   - Records `SaveOp` and `RestoreOp` into a `PaintOpBuffer`.
   - Also records `ClipRectOp`, `ClipPathOp`, `TranslateOp`, `ScaleOp`, etc.
   - These ops are later replayed onto an `SkCanvas`.

### save() in Blink

```
BaseRenderingContext2D::save():
  1. GetOrCreatePaintCanvas()          // ensure backend exists
  2. state_stack_.push(current_state)   // clone blink-side state
  3. canvas->save()                     // records SaveOp into PaintOpBuffer
```

### clip() in Blink

```
BaseRenderingContext2D::clip():
  1. Build SkPath from current path
  2. canvas->clipPath(path, SkClipOp::kIntersect, antialias)
     // records ClipPathOp into PaintOpBuffer
```

### restore() in Blink

```
BaseRenderingContext2D::restore():
  1. if state_stack_.size() <= 1: return   // nothing to restore
  2. state_stack_.pop()                     // pop blink-side state
  3. canvas->restore()                      // records RestoreOp
```

### Replay

When the `PaintOpBuffer` is played back onto an actual `SkCanvas`:
- `SaveOp`    -> `SkCanvas::save()`
- `ClipPathOp` -> `SkCanvas::clipPath()`
- `RestoreOp`  -> `SkCanvas::restore()`

**Chrome does NOT track clip depth separately.** It delegates entirely to Skia's
save/restore stack for clip management. The blink-side state stack only tracks
style properties; transform and clip are fully handled by the paint op recording
and subsequent Skia replay.

---

## 3. Skia's Internal Save/Restore Model

### MCRec (Matrix-Clip Record)

Skia's `SkCanvas` maintains an internal stack of `MCRec` (Matrix-Clip Record) structures:

```cpp
struct MCRec {
    SkMatrix        fMatrix;           // current transformation matrix
    SkRasterClip    fRasterClip;       // current clip (rasterized)
    int             fDeferredSaveCount; // lazy save optimization
    SkBaseDevice*   fTopLayer;         // device stack for layers
    // ...
};
```

The stack is stored in `fMCStack` (a deque) with `fMCRec` pointing to the top.

### save()

```cpp
int SkCanvas::save() {
    fSaveCount += 1;
    fMCRec->fDeferredSaveCount += 1;  // LAZY: don't actually copy yet
    return fSaveCount - 1;
}
```

**Deferred saves**: Skia optimizes by NOT immediately copying the MCRec.
It just increments a counter. The actual copy only happens when the clip
or matrix is about to be modified (copy-on-write).

### clipPath() / clipRect()

```cpp
void SkCanvas::onClipPath(const SkPath& path, SkClipOp op, bool doAA) {
    this->checkForDeferredSave();  // NOW actually copy MCRec if needed
    fMCRec->fRasterClip.opPath(path, fMCRec->fMatrix, op, doAA);
}
```

`checkForDeferredSave()` materializes the lazy save:
```cpp
void SkCanvas::checkForDeferredSave() {
    if (fMCRec->fDeferredSaveCount > 0) {
        fMCRec->fDeferredSaveCount -= 1;
        this->internalSave();  // actually push a new MCRec copy
    }
}
```

### restore()

```cpp
void SkCanvas::restore() {
    if (fMCRec->fDeferredSaveCount > 0) {
        // The save was never materialized; just decrement
        fMCRec->fDeferredSaveCount -= 1;
        fSaveCount -= 1;
    } else {
        this->internalRestore();  // pop MCRec, restoring matrix + clip
        fSaveCount -= 1;
    }
}
```

`internalRestore()` pops the MCRec from `fMCStack`, which naturally restores
the previous matrix and clip state (the old MCRec's `fRasterClip` and `fMatrix`
are still intact since the modification happened on the copy).

### How Clips Are "Undone"

**Clips are undone by discarding the modified copy.** When `save()` is called
(lazily materialized), Skia copies the current MCRec. When `clipPath()` modifies
the clip, it modifies the copy. When `restore()` is called, Skia pops and discards
the copy, revealing the previous MCRec with its untouched clip.

There is no "reverse clip" operation. It's pure stack-based copy-on-write.

---

## 4. Vello's Model

Vello does NOT have a save/restore API on `Scene`. Instead, it has:

- `push_clip_layer(fill_rule, transform, shape)` - push a clip layer
- `pop_layer()` - pop the most recently pushed layer

**Key differences from Skia:**
1. **No deferred saves** - each `push_clip_layer` immediately encodes into the scene.
2. **No separate transform stack** - transforms are passed per-operation (every `fill()`, `stroke()`, `push_clip_layer()` takes a transform argument).
3. **Clip = layer** - in Vello, a clip IS a layer. There is no way to clip without pushing a layer.
4. **No matrix-clip record** - transforms are NOT saved by layers.

### Implication for Canvas 2D

Since Vello ties clips to layers, the correct mapping is:

| Canvas 2D | Skia | Vello |
|-----------|------|-------|
| `save()` | `SkCanvas::save()` (deferred MCRec copy) | Track save level (no vello call needed) |
| `clip(path)` | `SkCanvas::clipPath()` (intersect into MCRec) | `scene.push_clip_layer(fill, transform, path)` + increment clip_depth on current save level |
| `restore()` | `SkCanvas::restore()` (pop MCRec) | `scene.pop_layer()` N times (once per clip in this save level) |
| transform ops | Modify MCRec.fMatrix | Track transform in player state, pass to each draw call |

---

## 5. Kozan's Current Implementation (canvas_player.rs)

**The current implementation is already correct.** Here's why:

```rust
struct SaveLevel {
    transform: Affine,
    clip_depth: usize,  // tracks how many clip layers were pushed at this level
}
```

- **Save**: pushes a new `SaveLevel` with `clip_depth: 0` (analogous to Skia's MCRec push)
- **ClipPath/ClipRect**: calls `scene.push_clip_layer()` and increments `clip_depth` (analogous to modifying MCRec's rasterClip)
- **Restore**: pops the `SaveLevel` and calls `scene.pop_layer()` for each clip (analogous to discarding the MCRec)
- **Cleanup**: at the end, drains any remaining save levels and pops their clips (handles unbalanced save/restore)

This is the canonical correct approach for a Vello-based renderer. It mirrors
exactly how Chrome delegates to Skia, except that:
- Chrome/Skia uses copy-on-write MCRec stacks (one pop restores everything)
- Kozan/Vello must explicitly track and pop each clip layer individually

### Potential Improvements

1. **Nested clips (intersection)**: The current approach pushes multiple clip layers,
   which Vello will intersect (each nested clip layer further restricts drawing).
   This matches the spec's "clips only intersect" behavior correctly.

2. **Deferred save optimization**: Like Skia, you could defer the `SaveLevel` push
   until a clip or transform actually changes. This avoids allocations for
   `save(); restore();` pairs that don't modify clip/transform. However, since
   Kozan's recording model already bakes style properties into draw ops, the
   save/restore ops only carry clip/transform state, so the overhead is minimal.

3. **Transform restoration on Restore**: The current code correctly restores the
   transform by popping to the previous SaveLevel's stored transform. Since Vello
   passes transforms per-draw-call (not globally), this just works.

---

## 6. Canonical Architecture Diagram

```
                 HTML Canvas 2D API
                        |
           save() / clip() / restore()
                        |
            CanvasRenderingContext2D
            (state.rs + context.rs)
           /                        \
   Blink-side state           Op recording
   (style properties)         (CanvasOp::Save/Clip/Restore)
                                    |
                           CanvasRecording
                                    |
                            VelloCanvasPlayer
                            (canvas_player.rs)
                                    |
                    SaveLevel { transform, clip_depth }
                           /              \
                 Vello push_clip_layer    Vello pop_layer
                 (one per clip() call)   (N times on restore)
```

---

## 7. Summary: The Correct Approach

| Concern | Chrome/Skia | Kozan/Vello |
|---------|-------------|-------------|
| Save mechanism | MCRec copy-on-write stack | SaveLevel with clip_depth counter |
| Clip storage | Rasterized into MCRec.fRasterClip | Encoded as vello clip layers |
| Clip undo | Discard MCRec copy (one operation) | Pop N clip layers from scene |
| Transform save | Stored in MCRec.fMatrix | Stored in SaveLevel.transform |
| Transform restore | Pop MCRec reveals previous | Pop SaveLevel reveals previous |
| Style save | Separate state stack (Blink-side) | Separate CanvasStateStack (state.rs) |

**Your current `canvas_player.rs` implementation follows the canonical correct pattern.**
The `SaveLevel.clip_depth` counter is the standard way to bridge between a
copy-on-write clip system (Skia) and a layer-based clip system (Vello/GPU renderers).

## Sources

- [Skia SkCanvas API Reference](https://api.skia.org/classSkCanvas.html)
- [Skia SkCanvas.cpp source](https://github.com/google/skia/blob/main/src/core/SkCanvas.cpp)
- [Skia SkCanvas.h source](https://github.com/google/skia/blob/main/include/core/SkCanvas.h)
- [Skia SkClipStack.h](https://github.com/google/skia/blob/main/src/core/SkClipStack.h)
- [Chromium BaseRenderingContext2D.cc](https://github.com/chromium/chromium/blob/master/third_party/blink/renderer/modules/canvas/canvas2d/base_rendering_context_2d.cc)
- [Chromium cc/paint/paint_canvas.h](https://chromium.googlesource.com/chromium/src/+/HEAD/cc/paint/paint_canvas.h)
- [Chromium Graphics and Skia design doc](https://www.chromium.org/developers/design-documents/graphics-and-skia/)
- [Vello Scene API docs](https://docs.rs/vello/latest/vello/struct.Scene.html)
- [MDN CanvasRenderingContext2D.save()](https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/save)
- [Skia SkCanvas Reference (Chromium mirror)](https://chromium.googlesource.com/skia/+/7cfcbca7168f3e36961fe32e75a5630426097e5c/site/user/api/SkCanvas_Reference.md)
- [PoignardAzur: Patterns of use of Vello crate](https://poignardazur.github.io//2025/01/18/vello-analysis/)
