//! Integration tests — uses PUBLIC API only, zero unsafe.

use kozan_core::*;

// -- Document --

#[test]
fn doc_root() {
    let doc = Document::new();
    assert_eq!(doc.node_count(), 1);
    assert!(doc.root().is_alive());
    assert!(doc.root().is_document());
    assert!(doc.root().parent().is_none());
    assert!(doc.root().children().is_empty());
}

// -- Creation --

#[test]
fn create_elements() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    let div = doc.create::<HtmlDivElement>();
    let t = doc.create_text("hello");
    assert!(btn.is_element());
    assert!(div.is_element());
    assert!(t.is_text());
    assert!(!t.is_element());
    assert_eq!(t.content(), "hello");
    assert_eq!(doc.node_count(), 4);
}

// -- Props --

#[test]
fn button_props() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    assert_eq!(btn.label(), "");
    btn.set_label("OK");
    assert_eq!(btn.label(), "OK");
    assert!(!btn.disabled());
    btn.set_disabled(true);
    assert!(btn.disabled());
}

// -- Element trait (attributes) --

#[test]
fn element_attrs() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    assert_eq!(btn.tag_name(), "button");
    btn.set_id("my-btn");
    assert_eq!(btn.id(), "my-btn");
    btn.set_class_name("foo bar");
    assert!(btn.class_contains("foo"));
    assert!(!btn.class_contains("baz"));
    btn.set_attribute("data-x", "42");
    assert_eq!(btn.attribute("data-x"), Some("42".to_string()));
    btn.remove_attribute("data-x");
    assert!(btn.attribute("data-x").is_none());
}

// -- HtmlElement trait (shared HTML behavior) --

#[test]
fn html_element_shared() {
    let doc = Document::new();
    let div = doc.create::<HtmlDivElement>();
    let btn = doc.create::<HtmlButtonElement>();

    assert!(!div.hidden());
    div.set_hidden(true);
    assert!(div.hidden());

    btn.set_title("Click me");
    assert_eq!(btn.title(), "Click me");

    div.set_dir("rtl");
    assert_eq!(div.dir(), "rtl");

    assert_eq!(div.tab_index(), -1); // not focusable
    assert_eq!(btn.tab_index(), 0); // focusable
}

// -- Tree --

#[test]
fn tree_append() {
    let doc = Document::new();
    let div = doc.create::<HtmlDivElement>();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(div);
    div.append(btn);
    assert_eq!(div.first_child().unwrap().raw(), btn.raw());
    assert_eq!(btn.parent().unwrap().raw(), div.raw());
}

#[test]
fn tree_reparent() {
    let doc = Document::new();
    let a = doc.create::<HtmlDivElement>();
    let b = doc.create::<HtmlDivElement>();
    let child = doc.create::<HtmlButtonElement>();
    doc.root().append(a);
    doc.root().append(b);
    a.append(child);
    b.append(child); // reparent
    assert_eq!(child.parent().unwrap().raw(), b.raw());
    assert!(a.children().is_empty());
}

#[test]
fn tree_siblings() {
    let doc = Document::new();
    let a = doc.create::<HtmlDivElement>();
    let b = doc.create::<HtmlDivElement>();
    let c = doc.create::<HtmlDivElement>();
    doc.root().append(a);
    doc.root().append(b);
    doc.root().append(c);
    assert_eq!(a.next_sibling().unwrap().raw(), b.raw());
    assert_eq!(c.prev_sibling().unwrap().raw(), b.raw());
}

#[test]
fn tree_text_no_children() {
    let doc = Document::new();
    let t = doc.create_text("hi");
    let btn = doc.create::<HtmlButtonElement>();
    t.handle().append(btn.handle()); // no-op via Handle
    assert!(t.handle().first_child().is_none());
}

#[test]
fn tree_deep() {
    let doc = Document::new();
    let mut p = doc.root();
    for _ in 0..100 {
        let c = doc.create::<HtmlDivElement>();
        p.append(c);
        p = c.handle();
    }
    let mut count = 0;
    let mut n = Some(p);
    while let Some(h) = n {
        count += 1;
        n = h.parent();
    }
    assert_eq!(count, 101);
}

// -- Destroy --

#[test]
fn destroy_stale() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    btn.set_label("alive");
    btn.destroy();
    assert!(!btn.is_alive());
    assert_eq!(btn.label(), "");
    // All ops safe on stale handle:
    btn.detach();
    btn.destroy();
}

#[test]
fn slot_reuse() {
    let doc = Document::new();
    let b1 = doc.create::<HtmlButtonElement>();
    let r1 = b1.raw();
    b1.destroy();
    let b2 = doc.create::<HtmlButtonElement>();
    let r2 = b2.raw();
    assert_eq!(r1.index(), r2.index());
    assert_ne!(r1.generation(), r2.generation());
}

// -- RawId --

#[test]
fn raw_id() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    let raw = btn.raw();
    assert!(doc.resolve(raw).is_some());
    btn.destroy();
    assert!(doc.resolve(raw).is_none());

    fn assert_send<T: Send>() {}
    assert_send::<RawId>();
}

// -- Sizes --

#[test]
fn type_sizes() {
    assert_eq!(core::mem::size_of::<Handle>(), 16);
    assert_eq!(core::mem::size_of::<HtmlButtonElement>(), 16);
    assert_eq!(core::mem::size_of::<HtmlDivElement>(), 16);
    assert_eq!(core::mem::size_of::<Text>(), 16);
    assert_eq!(core::mem::size_of::<RawId>(), 8);
    assert_eq!(core::mem::size_of::<NodeFlags>(), 4);
}

// -- Copy --

#[test]
fn copy_semantics() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    let b2 = btn;
    btn.set_label("shared");
    assert_eq!(b2.label(), "shared");
}

// -- Stress --

#[test]
fn stress_create_destroy() {
    let doc = Document::new();
    for _ in 0..1000 {
        let btn = doc.create::<HtmlButtonElement>();
        doc.root().append(btn);
        btn.destroy();
    }
    assert_eq!(doc.node_count(), 1);
}

// -- Events (EventTarget trait) --

#[test]
fn event_dispatch_and_listen() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::Yes
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(btn);

    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    btn.on::<Click>(move |_, _| c.set(true));

    btn.dispatch_event(&Click);
    assert!(called.get());
}

#[test]
fn event_bubbles_to_parent() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let div = doc.create::<HtmlDivElement>();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(div);
    div.append(btn);

    let parent_called = Rc::new(Cell::new(false));
    let pc = parent_called.clone();
    div.on::<Click>(move |_, _| pc.set(true));

    btn.dispatch_event(&Click);
    assert!(parent_called.get());
}

// layout pipeline tests live in layout_fixtures.rs

#[test]
fn event_stop_propagation() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let div = doc.create::<HtmlDivElement>();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(div);
    div.append(btn);

    let parent_called = Rc::new(Cell::new(false));
    let pc = parent_called.clone();
    btn.on::<Click>(move |_, ctx| ctx.stop_propagation());
    div.on::<Click>(move |_, _| pc.set(true));

    btn.dispatch_event(&Click);
    assert!(!parent_called.get()); // stopped!
}

#[test]
fn event_prevent_default() {
    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::Yes
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    btn.on::<Click>(move |_, ctx| ctx.prevent_default());

    let result = btn.dispatch_event(&Click);
    assert!(!result); // default prevented
}

#[test]
fn event_once() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::No
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    let count = Rc::new(Cell::new(0u32));
    let c = count.clone();
    btn.on_once::<Click>(move |_, _| c.set(c.get() + 1));

    btn.dispatch_event(&Click);
    btn.dispatch_event(&Click);
    btn.dispatch_event(&Click);
    assert_eq!(count.get(), 1); // only once
}

// layout pipeline tests live in layout_fixtures.rs

// ============================================================
// New HTML Elements — category traits, replaced, headings
// ============================================================

#[test]
fn heading_element_runtime_tag() {
    let doc = Document::new();
    let h1 = doc.create_heading(1);
    let h3 = doc.create_heading(3);
    let h6 = doc.create_heading(6);

    assert_eq!(h1.tag_name(), "h1");
    assert_eq!(h3.tag_name(), "h3");
    assert_eq!(h6.tag_name(), "h6");

    assert_eq!(h1.heading_level(), 1);
    assert_eq!(h3.heading_level(), 3);
    assert_eq!(h6.heading_level(), 6);
}

#[test]
fn create_with_tag_overrides_default() {
    let doc = Document::new();
    let h4 = doc.create_with_tag::<HtmlHeadingElement>("h4");
    assert_eq!(h4.tag_name(), "h4");
}

#[test]
#[should_panic(expected = "no fixed tag")]
fn create_heading_without_tag_panics() {
    let doc = Document::new();
    // HtmlHeadingElement has no TAG_NAME — must use create_heading() or create_with_tag().
    let _ = doc.create::<HtmlHeadingElement>();
}

#[test]
fn form_control_disabled() {
    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    let input = doc.create::<HtmlInputElement>();

    // Both implement FormControlElement.
    assert!(!btn.disabled());
    assert!(!input.disabled());

    btn.set_disabled(true);
    input.set_disabled(true);
    assert!(btn.disabled());
    assert!(input.disabled());
}

#[test]
fn text_control_value() {
    let doc = Document::new();
    let input = doc.create::<HtmlInputElement>();
    let textarea = doc.create::<HtmlTextAreaElement>();

    // Both implement TextControlElement.
    input.set_value("hello");
    textarea.set_value("world");

    assert_eq!(input.value(), "hello");
    assert_eq!(textarea.value(), "world");
}

#[test]
fn input_type_classification() {
    assert!(InputType::Text.is_text_type());
    assert!(InputType::Password.is_text_type());
    assert!(InputType::Checkbox.is_checkable());
    assert!(InputType::Radio.is_checkable());
    assert!(InputType::Submit.is_button_type());
    assert!(!InputType::Hidden.is_focusable());
}

#[test]
fn media_element_attributes() {
    let doc = Document::new();
    let audio = doc.create::<HtmlAudioElement>();
    let video = doc.create::<HtmlVideoElement>();

    // Both implement MediaElement.
    audio.set_src("music.mp3");
    video.set_src("movie.mp4");
    assert_eq!(audio.src(), "music.mp3");
    assert_eq!(video.src(), "movie.mp4");

    audio.set_controls(true);
    assert!(audio.controls());
    assert!(!audio.autoplay());
}

#[test]
fn image_intrinsic_sizing() {
    let doc = Document::new();
    let img = doc.create::<HtmlImageElement>();

    img.set_natural_width(800.0);
    img.set_natural_height(600.0);

    let sizing = img.intrinsic_sizing();
    assert_eq!(sizing.width, Some(800.0));
    assert_eq!(sizing.height, Some(600.0));
    assert!((sizing.aspect_ratio.unwrap() - 1.333).abs() < 0.01);
}

#[test]
fn canvas_default_intrinsic_size() {
    let doc = Document::new();
    let canvas = doc.create::<HtmlCanvasElement>();

    // HTML spec default: 300x150.
    let sizing = canvas.intrinsic_sizing();
    assert_eq!(sizing.width, Some(300.0));
    assert_eq!(sizing.height, Some(150.0));
}

#[test]
fn video_element_default_intrinsic_size() {
    let doc = Document::new();
    let video = doc.create::<HtmlVideoElement>();

    // Before metadata: 300x150 per spec.
    let sizing = video.intrinsic_sizing();
    assert_eq!(sizing.width, Some(300.0));
    assert_eq!(sizing.height, Some(150.0));

    // After metadata load.
    video.set_video_width(1920.0);
    video.set_video_height(1080.0);
    let sizing = video.intrinsic_sizing();
    assert_eq!(sizing.width, Some(1920.0));
    assert_eq!(sizing.height, Some(1080.0));
}

#[test]
fn section_elements_have_correct_tags() {
    let doc = Document::new();

    let section = doc.create::<html::HtmlSectionElement>();
    let article = doc.create::<html::HtmlArticleElement>();
    let nav = doc.create::<html::HtmlNavElement>();
    let header = doc.create::<html::HtmlHeaderElement>();
    let footer = doc.create::<html::HtmlFooterElement>();

    assert_eq!(section.tag_name(), "section");
    assert_eq!(article.tag_name(), "article");
    assert_eq!(nav.tag_name(), "nav");
    assert_eq!(header.tag_name(), "header");
    assert_eq!(footer.tag_name(), "footer");
}

#[test]
fn anchor_element_props() {
    let doc = Document::new();
    let a = doc.create::<HtmlAnchorElement>();

    a.set_href("https://example.com");
    a.set_target("_blank");
    assert_eq!(a.href(), "https://example.com");
    assert_eq!(a.target(), "_blank");
}

#[test]
fn label_for_attribute() {
    let doc = Document::new();
    let label = doc.create::<HtmlLabelElement>();
    let input = doc.create::<HtmlInputElement>();

    input.set_id("email");
    label.set_html_for("email");

    assert_eq!(label.html_for(), "email");
}

#[test]
fn form_and_controls() {
    let doc = Document::new();
    let form = doc.create::<HtmlFormElement>();
    let input = doc.create::<HtmlInputElement>();
    let btn = doc.create::<HtmlButtonElement>();

    doc.root().append(form);
    form.append(input);
    form.append(btn);

    form.set_action("/submit");
    form.set_method("post");
    input.set_name("username");
    input.set_value("alice");
    btn.set_label("Submit");

    assert_eq!(form.action(), "/submit");
    assert_eq!(input.name(), "username");
    assert_eq!(input.value(), "alice");
    assert_eq!(btn.label(), "Submit");
}

// layout pipeline tests live in layout_fixtures.rs

// ============================================================
// Event dispatch correctness — phase ordering + propagation
// ============================================================

#[test]
fn capture_fires_before_bubble() {
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let div = doc.create::<HtmlDivElement>();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(div);
    div.append(btn);

    let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let o1 = order.clone();
    div.on_capture::<Click>(move |_, _| o1.borrow_mut().push("capture"));

    let o2 = order.clone();
    div.on::<Click>(move |_, _| o2.borrow_mut().push("bubble"));

    btn.dispatch_event(&Click);

    let fired = order.borrow();
    assert_eq!(*fired, vec!["capture", "bubble"]);
}

#[test]
fn at_target_fires_both_capture_and_bubble() {
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::No
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(btn);

    let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    // Register bubble listener first, then capture — both must fire at-target.
    let o1 = order.clone();
    btn.on::<Click>(move |_, _| o1.borrow_mut().push("bubble"));

    let o2 = order.clone();
    btn.on_capture::<Click>(move |_, _| o2.borrow_mut().push("capture"));

    btn.dispatch_event(&Click);

    let fired = order.borrow();
    // Both listeners fire; event does not bubble.
    assert_eq!(fired.len(), 2);
    assert!(fired.contains(&"bubble"));
    assert!(fired.contains(&"capture"));
}

#[test]
fn stop_propagation_prevents_ancestor() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let outer = doc.create::<HtmlDivElement>();
    let inner = doc.create::<HtmlDivElement>();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(outer);
    outer.append(inner);
    inner.append(btn);

    // Stop at `inner` — `outer` bubble listener must not fire.
    inner.on::<Click>(move |_, ctx| ctx.stop_propagation());

    let outer_called = Rc::new(Cell::new(false));
    let oc = outer_called.clone();
    outer.on::<Click>(move |_, _| oc.set(true));

    btn.dispatch_event(&Click);
    assert!(!outer_called.get());
}

#[test]
fn stop_immediate_propagation_stops_same_node() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::Yes
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();
    let btn = doc.create::<HtmlButtonElement>();
    doc.root().append(btn);

    // First listener stops immediate propagation.
    btn.on::<Click>(move |_, ctx| ctx.stop_immediate_propagation());

    // Second listener on the same node must NOT fire.
    let second_called = Rc::new(Cell::new(false));
    let sc = second_called.clone();
    btn.on::<Click>(move |_, _| sc.set(true));

    btn.dispatch_event(&Click);
    assert!(!second_called.get());
}

#[test]
fn off_on_node_with_no_listeners_is_noop() {
    // Calling off() on a node that has never had a listener added must not
    // allocate an EventListenerMap or panic.
    struct Click;
    impl Event for Click {
        fn bubbles(&self) -> events::Bubbles {
            events::Bubbles::No
        }
        fn cancelable(&self) -> events::Cancelable {
            events::Cancelable::No
        }
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    }

    let doc = Document::new();

    // Get a real ListenerId by registering on one node.
    let donor = doc.create::<HtmlDivElement>();
    let id = donor.on::<Click>(|_, _| {});

    // A fresh node has no listener map. off() must be a no-op, not a panic.
    let btn = doc.create::<HtmlButtonElement>();
    btn.off(id);
}
