# CSS Overflow & Scroll Chaining — Exact Spec Text

Research compiled from:
- CSS Overflow Module Level 3 (Editor's Draft, 6 January 2026)
- CSS Overscroll Behavior Module Level 1 (Editor's Draft, 5 March 2026)
- CSSOM View Module Level 1 (Editor's Draft, 18 December 2025)

---

## 1. Viewport Propagation Rule (CSS Overflow 3, Section 3.3)

**Source:** https://drafts.csswg.org/css-overflow-3/#overflow-propagation

> UAs must apply the `overflow-*` values set on the root element to the viewport when the root element's `display` value is not `none`.
>
> However, when the root element is an [HTML] `html` element (including XML syntax for HTML) whose `overflow` value is `visible` (in both axes), and that element has as a child a `body` element whose `display` value is also not `none`, user agents must instead apply the `overflow-*` values of the first such child element to the viewport.
>
> The element from which the value is propagated must then have a used `overflow` value of `visible`.

**Note from spec:** Using containment on the HTML `html` or `body` elements disables this special handling of the HTML `body` element.

### Algorithm summary:
1. If root element `display != none` -> apply root's `overflow-*` to viewport
2. EXCEPT: if root is `<html>` AND root's `overflow` is `visible` in both axes AND `<body>` child exists with `display != none`
   -> apply `<body>`'s `overflow-*` to viewport instead
3. The source element (whichever was propagated from) gets used `overflow: visible`

---

## 2. `overflow` Property Values (CSS Overflow 3, Section 3.1)

**Source:** https://drafts.csswg.org/css-overflow-3/#overflow-control

### `visible`
> There is no special handling of overflow, i.e. the box is not a scroll container. Content is not clipped: it may be rendered outside the padding box.

### `hidden`
> This value indicates that the box's content is clipped to its padding box and that the UA must not provide any scrolling user interface to view the content outside the clipping region, nor allow scrolling by direct intervention of the user, such as dragging on a touch screen or using the scrolling wheel on a mouse. However, the content must still be scrollable programmatically, for example using the mechanisms defined in [CSSOM-VIEW], and the box is therefore still a scroll container.

### `clip`
> This value indicates that the box's content is clipped to its overflow clip edge and that no scrolling user interface should be provided by the user agent to view the content outside the clipping region. In addition, unlike `overflow: hidden`, content must NOT be scrollable programmatically, and therefore the box is NOT a scroll container.

### `scroll`
> This value indicates that the content is clipped to the padding box, but can be scrolled into view (and therefore the box is a scroll container). Furthermore, if the user agent uses a scrolling mechanism that is visible on the screen (such as a scroll bar or a panner), that mechanism should be displayed whether or not any of its content is clipped. This avoids any problem with scrollbars appearing and disappearing in a dynamic environment. When the target medium is print, overflowing content may be printed; it is not defined where it may be printed.

### `auto`
> Like `scroll` when the box has scrollable overflow; like `hidden` otherwise. Thus, if the user agent uses a scrolling mechanism that is visible on the screen (such as a scroll bar or a panner), that mechanism will only be displayed if there is overflow.

### Key rule — visible/clip axis interaction:
> The `visible`/`clip` values of `overflow` compute to `auto`/`hidden` (respectively) if one of `overflow-x` or `overflow-y` is neither `visible` nor `clip`.

### Scrollable vs non-scrollable classification:
- **Scrollable values:** `scroll`, `auto`, `hidden`
- **Non-scrollable values:** `visible`, `clip`

### Scroll container definition:
A box becomes a **scroll container** when overflow is `hidden`, `scroll`, or `auto`. All three make the box a scroll container. The difference is:
- `hidden` = scroll container, but no user-visible scrolling UI, only programmatic scroll
- `scroll` = scroll container, always shows scroll bars
- `auto` = scroll container with visible scrollbars only when content overflows

### "scrollable overflow" definition (Section 2.2):
> A box has **scrollable overflow** when its scrollable overflow area is larger than its scrollport (padding box) in the relevant axis.

---

## 3. Scroll Chaining (CSS Overscroll Behavior 1, Section 3)

**Source:** https://drafts.csswg.org/css-overscroll-1/#scroll-chaining-and-boundary-default-actions

> **Scroll chaining** is when scrolling is propagated from one scroll container to an ancestor scroll container following the scroll chain. Typically scroll chaining is performed starting at the event target recursing up the containing block chain. When a scroll container in this chain receives a scroll event or gesture it may act on it and/or pass it up the chain. Chaining typically occurs when the scrollport has reached its boundary.

> A **scroll chain** is the order in which scrolling is propagated from one scroll container to another. The viewport participates in scroll chaining as the document's `scrollingElement`, both regarding placement in the scroll chain as well as adhering to the chaining rules applied to it.

> **Scroll boundary** refers to when the scroll position of a scroll container reaches the maximum or minimum scroll position in a given axis. Content may or may not be present at the scroll boundary. A scroll container at the scroll boundary may accept the scroll gesture or may hand it off to the next element in the scroll chain.

> A **boundary default action** is the UA-defined action taken in response to a scroll gesture that causes scrolling at the scroll boundary. This includes actions such as scroll chaining (propagating scroll to the next scroll container in the chain) and overscroll (visual effects like rubber-banding).

> A **local boundary default action** is a boundary default action that occurs locally on a scroll container element, such as an overscroll affordance (rubber-banding visual feedback). This is in contrast to actions that propagate to other elements like scroll chaining.

---

## 4. `overscroll-behavior` Values (CSS Overscroll Behavior 1, Section 4)

**Source:** https://drafts.csswg.org/css-overscroll-1/#overscroll-behavior-properties

Property definition:
- **Name:** `overscroll-behavior`
- **Value:** `[ contain | none | auto ]{1,2}`
- **Initial:** `auto`
- **Applies to:** scroll container elements
- **Inherited:** no

### `auto`
> This value indicates that the user agent should perform the usual boundary default action with respect to scroll chaining, overscroll and navigation gestures.

### `contain`
> This value indicates that the scroll container must not perform scroll chaining to any ancestor or sibling scroll containers. The element's local boundary default actions such as showing an overscroll affordance are still performed.

### `none`
> This value implies the same behavior as `contain` and in addition this element must also not perform local boundary default actions such as showing any overscroll affordances.

**Critical note from spec:**
> Programmatic scrolling is clamped and cannot trigger any boundary default actions.

**Non-scroll-container rule:**
> An element that is not scroll container must accept but ignore the values of this property. This property must be applied to all scrolling methods supported by the user agent.

### 4.1 Overscroll and Positioned Elements

> If an element uses fixed positioning and is positioned relative to the initial containing block, or is a sticky positioned element which is currently stuck to the viewport, then when the root scroller experiences "overscroll", that element **must not** overscroll with the rest of the document's content; it must instead remain positioned as if the scroller was at its minimum/maximum scroll position, whichever it will return to when the overscroll is finished.
>
> Even though this can visually shift the fixed/sticky element relative to other elements on the page, it must be treated purely as a visual effect, and not reported as an actual layout/position change to APIs such as `getBoundingClientRect()`.

---

## 5. `position: fixed` + `overflow-y: auto` — Scroll Propagation

This question requires combining rules from CSS Overflow 3, CSSOM View, and Overscroll Behavior:

### Does scrolling inside a `position: fixed` element with `overflow-y: auto` propagate to the viewport?

**Answer: Yes, by default (overscroll-behavior: auto).**

The scroll chain follows the **containing block chain**, not the DOM tree. Per CSSOM View (Section 7, "find the nearest scrollable ancestor"):

> 1. If the element [meets certain conditions, including]:
>    - The element's computed value of the `position` property is `fixed` and no ancestor establishes a fixed position containing block.
>
> 2. Let ancestor be the containing block of the element in the flat tree and repeat these substeps:
>    1. If ancestor is the initial containing block, return the `scrollingElement` for the element's document [...]
>    2. If ancestor is not closed-shadow-hidden from the element, and is a scroll container, terminate this algorithm and return ancestor.

For a `position: fixed` element with no `transform`/`will-change`/`filter` ancestor:
- Its containing block is the **initial containing block** (the viewport).
- The viewport IS the root scroller / `scrollingElement`.
- Therefore, when the fixed element's scroll reaches its boundary, **scroll chains to the viewport** (the document scroller).

**To prevent this**, set `overscroll-behavior: contain` on the fixed element.

### Key behavioral rules:
1. A `position: fixed` element with `overflow-y: auto` IS a scroll container (Section 3.1).
2. Its scroll chain goes: fixed element -> initial containing block -> viewport/scrollingElement.
3. With default `overscroll-behavior: auto`, scroll WILL chain to the viewport when the fixed element hits its scroll boundary.
4. An ancestor with `transform`, `will-change: transform`, or `filter` changes the fixed element's containing block, potentially inserting intermediate scroll containers into the chain.

---

## Implementation Notes for Kozan

### Scroll chain construction algorithm:
```
fn build_scroll_chain(target: &Element) -> Vec<ScrollContainerId> {
    // 1. Start at event target
    // 2. Walk up containing block chain (NOT DOM tree for fixed elements)
    // 3. Each scroll container in chain gets a chance to consume scroll
    // 4. At each boundary, check overscroll-behavior:
    //    - auto: propagate to next in chain
    //    - contain: stop chaining, allow local overscroll effect
    //    - none: stop chaining, no overscroll effect
    // 5. Viewport is always last in chain (as scrollingElement)
}
```

### Viewport propagation check:
```
fn resolve_viewport_overflow(html: &Element, body: Option<&Element>) -> Overflow {
    if html.display() == Display::None {
        return Overflow::Visible; // spec says "when display is not none"
    }

    if html.overflow_x() == Visible && html.overflow_y() == Visible {
        if let Some(body) = body {
            if body.display() != Display::None {
                // Propagate body's overflow to viewport
                // Set body's used overflow to visible
                return body.overflow();
            }
        }
    }

    // Use html's overflow for viewport
    html.overflow()
}
```

### `overflow: auto` scrollbar visibility check:
```
fn should_show_scrollbar(container: &ScrollContainer, axis: Axis) -> bool {
    match container.overflow(axis) {
        Overflow::Scroll => true,  // always show
        Overflow::Auto => container.scrollable_overflow_area(axis) > container.scrollport(axis),
        Overflow::Hidden => false, // no user-visible scrollbar
        _ => false,
    }
}
```
