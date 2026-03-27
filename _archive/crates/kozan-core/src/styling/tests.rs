//! Tests for the Stylo integration — verifies the cascade actually works.

#[cfg(test)]
mod tests {
    use crate::dom::document::Document;
    use crate::dom::traits::{ContainerNode, Element, HasHandle, Node};
    use crate::html::HtmlDivElement;
    use crate::html::HtmlSpanElement;

    /// Helper: run recalc and get ComputedValues for a node.
    fn get_computed(
        doc: &mut Document,
        index: u32,
    ) -> Option<servo_arc::Arc<style::properties::ComputedValues>> {
        doc.recalc_styles();
        doc.computed_style(index)
    }

    #[test]
    fn ua_stylesheet_div_is_block() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        let cv = get_computed(&mut doc, div.handle().raw().index());
        assert!(cv.is_some(), "recalc_styles should produce ComputedValues");

        let cv = cv.unwrap();
        let display = cv.get_box().clone_display();
        assert!(
            display == style::values::computed::Display::Block,
            "div should be display:block from UA stylesheet, got {:?}",
            display,
        );
    }

    #[test]
    fn ua_stylesheet_span_is_inline() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        let span = doc.create::<HtmlSpanElement>();
        doc.root().append(div);
        div.append(span);

        let cv = get_computed(&mut doc, span.handle().raw().index()).unwrap();
        let display = cv.get_box().clone_display();
        assert!(
            display.is_inline_flow(),
            "span inside div should be inline flow, got {:?}",
            display,
        );
    }

    #[test]
    fn author_stylesheet_class_selector() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);
        div.set_attribute("class", "red");

        doc.add_stylesheet(".red { color: red; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        // Red = srgb(255, 0, 0)
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            r > 0.9 && g < 0.1 && b < 0.1,
            "div.red should have color:red, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn author_stylesheet_tag_selector() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("div { color: blue; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            r < 0.1 && g < 0.1 && b > 0.9,
            "div should have color:blue, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn author_stylesheet_id_selector() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);
        div.set_attribute("id", "main");

        doc.add_stylesheet("#main { color: green; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            g > 0.4 && r < 0.1 && b < 0.1,
            "div#main should have color:green, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn inheritance_color_flows_to_child() {
        let mut doc = Document::new();
        let parent = doc.create::<HtmlDivElement>();
        let child = doc.create::<HtmlDivElement>();
        doc.root().append(parent);
        parent.append(child);

        doc.add_stylesheet("div.parent { color: red; }");
        parent.set_attribute("class", "parent");

        let cv = get_computed(&mut doc, child.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            r > 0.9 && g < 0.1 && b < 0.1,
            "child should inherit color:red from parent, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn specificity_class_beats_tag() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);
        div.set_attribute("class", "blue");

        doc.add_stylesheet("div { color: red; } .blue { color: blue; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            b > 0.9 && r < 0.1,
            ".blue class should beat div tag selector, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn multiple_stylesheets() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("div { color: red; }");
        doc.add_stylesheet("div { color: blue; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            b > 0.9 && r < 0.1,
            "later stylesheet should win, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn flex_display() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("div { display: flex; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let display = cv.get_box().clone_display();
        assert!(
            display == style::values::computed::Display::Flex,
            "div should be display:flex, got {:?}",
            display,
        );
    }

    #[test]
    fn inline_style_attribute() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);
        div.set_attribute("style", "color: red");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            r > 0.9 && g < 0.1 && b < 0.1,
            "inline style='color: red' should apply, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn attribute_selector() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);
        div.set_attribute("data-active", "true");

        doc.add_stylesheet("[data-active] { color: red; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let c = color.raw_components();
        assert!(
            c[0] > 0.9 && c[1] < 0.1,
            "[data-active] selector should match, got ({}, {}, {})",
            c[0],
            c[1],
            c[2]
        );
    }

    #[test]
    fn descendant_combinator() {
        let mut doc = Document::new();
        let outer = doc.create::<HtmlDivElement>();
        let inner = doc.create::<HtmlSpanElement>();
        doc.root().append(outer);
        outer.append(inner);
        outer.set_attribute("class", "wrapper");

        doc.add_stylesheet(".wrapper span { color: red; }");

        let cv = get_computed(&mut doc, inner.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let c = color.raw_components();
        assert!(
            c[0] > 0.9 && c[1] < 0.1,
            "descendant combinator should match, got ({}, {}, {})",
            c[0],
            c[1],
            c[2]
        );
    }

    #[test]
    fn incremental_restyle_attribute_change() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet(".red { color: red; } .blue { color: blue; }");

        // First recalc: no class
        doc.recalc_styles();

        // Set class and recalc again
        div.set_attribute("class", "red");
        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let c = cv
            .get_inherited_text()
            .clone_color()
            .raw_components()
            .clone();
        assert!(
            c[0] > 0.9,
            "after adding .red class, should be red, got ({}, {}, {})",
            c[0],
            c[1],
            c[2]
        );

        // Change class and recalc
        div.set_attribute("class", "blue");
        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let c = cv
            .get_inherited_text()
            .clone_color()
            .raw_components()
            .clone();
        assert!(
            c[2] > 0.9 && c[0] < 0.1,
            "after changing to .blue, should be blue, got ({}, {}, {})",
            c[0],
            c[1],
            c[2]
        );
    }

    #[test]
    fn remove_attribute_clears_style() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("#target { color: red; }");

        div.set_attribute("id", "target");
        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let c = cv
            .get_inherited_text()
            .clone_color()
            .raw_components()
            .clone();
        assert!(c[0] > 0.9, "with id=target, should be red");

        div.remove_attribute("id");
        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let c = cv
            .get_inherited_text()
            .clone_color()
            .raw_components()
            .clone();
        assert!(
            c[0] < 0.5,
            "after removing id, should not be red anymore, got ({}, {}, {})",
            c[0],
            c[1],
            c[2]
        );
    }

    #[test]
    fn inline_style_beats_author() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("div { color: blue; }");
        div.set_attribute("style", "color: red");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let (r, g, b, _) = {
            let c = color.raw_components();
            (c[0], c[1], c[2], c[3])
        };
        assert!(
            r > 0.9 && b < 0.1,
            "inline style should beat author stylesheet, got ({r}, {g}, {b})",
        );
    }

    #[test]
    fn stylesheet_applies_width_to_element() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("div { width: 200px; }");

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let width = cv.get_position().clone_width();
        assert!(
            !width.is_auto(),
            "div should have explicit width from stylesheet, got auto",
        );
    }

    #[test]
    fn viewport_resize_produces_different_computed_width() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        // Default viewport is 1920x1080, apply a vw-based width.
        doc.add_stylesheet(".sized { width: 50vw; }");
        div.set_attribute("class", "sized");

        let cv1 = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let w1 = cv1.get_position().clone_width();

        // Resize viewport and force restyle via attribute change.
        doc.set_viewport(800.0, 600.0);
        div.set_attribute("class", "sized");
        let cv2 = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let w2 = cv2.get_position().clone_width();

        assert_ne!(
            format!("{:?}", w1),
            format!("{:?}", w2),
            "viewport resize should change computed vw-based width",
        );
    }

    #[test]
    fn flush_inline_styles_via_style_api() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        // Set inline style via the typed API.
        div.style()
            .color(style::color::AbsoluteColor::srgb_legacy(255, 0, 0, 1.0));

        // recalc_styles calls flush_inline_styles internally.
        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let color = cv.get_inherited_text().clone_color();
        let c = color.raw_components();
        assert!(
            c[0] > 0.9 && c[1] < 0.1 && c[2] < 0.1,
            "typed style API color should apply after recalc, got ({}, {}, {})",
            c[0],
            c[1],
            c[2],
        );
    }

    #[test]
    fn inline_style_api_overrides_stylesheet() {
        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        doc.add_stylesheet("div { color: blue; }");

        // Typed inline style should override author stylesheet.
        div.style()
            .color(style::color::AbsoluteColor::srgb_legacy(255, 0, 0, 1.0));

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();
        let c = cv
            .get_inherited_text()
            .clone_color()
            .raw_components()
            .clone();
        assert!(
            c[0] > 0.9 && c[2] < 0.1,
            "typed inline style should beat author stylesheet, got ({}, {}, {})",
            c[0],
            c[1],
            c[2],
        );
    }

    #[test]
    fn multiple_inline_properties_batch_flush() {
        use style::values::specified::box_::Display;

        let mut doc = Document::new();
        let div = doc.create::<HtmlDivElement>();
        doc.root().append(div);

        // Set multiple properties in one chain — they all flush on drop.
        div.style()
            .display(Display::Flex)
            .color(style::color::AbsoluteColor::srgb_legacy(0, 0, 255, 1.0));

        let cv = get_computed(&mut doc, div.handle().raw().index()).unwrap();

        let display = cv.get_box().clone_display();
        assert_eq!(
            display,
            style::values::computed::Display::Flex,
            "display should be flex"
        );

        let c = cv
            .get_inherited_text()
            .clone_color()
            .raw_components()
            .clone();
        assert!(
            c[2] > 0.9 && c[0] < 0.1,
            "color should be blue from batched inline styles, got ({}, {}, {})",
            c[0],
            c[1],
            c[2],
        );
    }
}
