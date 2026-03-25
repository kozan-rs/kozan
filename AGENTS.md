# Agent Coding Standards

Write code that would pass review for the Rust standard library. Not "it compiles" — code a senior Rust engineer would be proud of.

## Comments

Comments explain WHY, never WHAT. If the code is clear, no comment is needed.

```rust
// BAD — restates the code
// Clear the cache and return the parent
pub fn clear_cache_get_parent(&mut self) -> Option<u32> {

// BAD — explains what every Rust dev already knows
/// Named `unwrap_box` to mirror `Option::unwrap` — the name signals
/// that this panics on the wrong variant. Prefer `try_as_box()` when unsure.

// BAD — section header for obvious grouping
// ---- Playback actions ----

// BAD — ASCII separator lines
// ============================================================
// ── Clip stack ───────────────────────────────────────────────

// GOOD — explains a non-obvious constraint
// Listeners are taken before dispatch and put back after, allowing
// safe tree mutation inside handlers (take-call-put pattern).

// GOOD — explains why a seemingly wrong choice is correct
// Text nodes need relayout but NOT restyle — the parent's computed
// style is unchanged, only the inline geometry needs recomputation.

// GOOD — spec reference that isn't obvious from the code
// Chrome: CharacterData::DidModifyData() → ContainerNode::ChildrenChanged()

// BAD — AI-generated fluff that says nothing
// This should now work correctly
// Fixed the issue with the layout
// Updated to handle the edge case properly
// Now properly handles the case where...
```

## Documentation

Module docs (`//!`): one sentence — what the module IS. Add a Chrome reference if the design mirrors a Chrome subsystem.

Struct/enum docs (`///`): one sentence if the name isn't self-explanatory. Skip if the type name says it all.

Function docs: one line for simple functions. Never explain what the parameter types already say.

```rust
// BAD — 4 lines for a function whose name says everything
/// Get box fragment data, panicking if this is not a box fragment.
///
/// Named `unwrap_box` to mirror `Option::unwrap` — the name signals
/// that this panics on the wrong variant. Prefer `try_as_box()` when unsure.
pub fn unwrap_box(&self) -> &BoxFragmentData {

// GOOD
/// Panics if this is not a box fragment. Use `try_as_box()` when unsure.
pub fn unwrap_box(&self) -> &BoxFragmentData {
```

## Naming

- No `get_` prefix. `fn width()` not `fn get_width()`.
- `unwrap_foo()` for panicking accessors, `try_foo()` for `Option`-returning ones.
- Command-query separation: a function either mutates OR returns, not both. `clear_cache_get_parent()` is wrong — split it.

## Error Handling

- Invariant violations: `expect("what must be true")`, never bare `unwrap()`.
- Infallible conversions that can theoretically fail (e.g. `i32 → u16`): `unwrap_or(u16::MAX)`, not `unwrap()`.

```rust
// BAD
.get(idx).unwrap()
repeat_count.try_into().unwrap()

// GOOD
.get(idx).expect("layout node always exists during compute_child_layout")
repeat_count.try_into().unwrap_or(u16::MAX)
```

## Code Structure

No micro-wrappers — a single-call-site function that just passes a fixed argument is noise; inline it.

```rust
// BAD
fn resolve_inline_size(style: &ComputedValues, space: &ConstraintSpace, bp: f32) -> f32 {
    resolve_inline_size_with_intrinsic(style, space, bp, None)
}

// GOOD — inline at the one call site
let width = resolve_inline_size_with_intrinsic(style, space, bp, None);
```

No redundant guards:

```rust
// BAD
if is_first_child { is_first_child = false; }

// GOOD
is_first_child = false;
```

Prefer `if let` over `.is_some()` + `.unwrap()`:

```rust
// BAD
if aspect_ratio.is_some() { let ratio = aspect_ratio.unwrap_or(1.0);

// GOOD
if let Some(ratio) = aspect_ratio {
```

## Visibility

Default to the minimum that compiles. `pub(crate)` for anything not in the public API. Internal methods (hover state, viewport, computed style lookups) are never `pub`.

## Unsafe

All unsafe lives in `DocumentCell`. Every `unsafe` block has a `// SAFETY:` comment.

## Tests

Test behavior, not stubs. Test names describe the scenario (`capture_fires_before_bubble`, not `test_event`). No `#[allow(dead_code)]` in tests.

## Refactoring

Full refactors are not only allowed — they're preferred when they solve deeper problems. Never apply a quick hack when a proper restructuring is the correct fix.

- If a struct has responsibilities that belong elsewhere, move them. Don't add wrapper methods.
- If logic is in the wrong place, relocate it. Don't paper over it with delegation.
- If a pattern doesn't scale (e.g. TypeId matching instead of dispatch tables), replace the pattern entirely.
- If fixing a bug requires touching 5 files because the architecture is wrong, fix the architecture first.
- Prefer the complex-but-correct solution over the quick-but-hacky one. A 200-line refactor that eliminates a class of bugs is better than a 5-line band-aid.
- Every struct should own its behavior, not just hold data. If external code reaches into a struct's fields to do work, that work belongs on the struct.

## Hard Rules

1. No comments that restate the code.
2. No section headers for obvious groupings.
3. No multi-paragraph docs on simple functions.
4. No defensive code for impossible cases — trust the invariants.
5. No `TODO` comments — use `todo!()` so the compiler enforces them.
6. No touching files outside the task scope.
7. No adding features that weren't asked for.
8. No duplicate doc-comment drafts — one coherent block, not two merged ones.
9. No `pub` on methods that are only called internally — that is an API leak.
10. No bare `.unwrap()` anywhere in production code.
11. No AI-debt comments — "this should now work", "fixed the issue", "updated to handle X properly", "now correctly does Y". If the code works, the comment is noise. If it doesn't, the comment is a lie.
