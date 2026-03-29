use kozan_style::*;
use kozan_style::specified::LengthPercentage;
use kozan_style::generics::Size;

fn accepts_size(_: impl IntoDeclared<Size<LengthPercentage>>) {}
fn accepts_auto_or_i32(_: impl IntoDeclared<AutoOr<i32>>) {}
fn accepts_color(_: impl IntoDeclared<Color>) {}
fn accepts_display(_: impl IntoDeclared<Display>) {}
fn accepts_f32(_: impl IntoDeclared<f32>) {}

#[test]
fn concrete_values() {
    accepts_size(Size::LengthPercentage(LengthPercentage::from(px(100.0))));
    accepts_size(Size::<LengthPercentage>::Auto);
    accepts_size(Size::<LengthPercentage>::MinContent);
    accepts_auto_or_i32(AutoOr::Value(5));
    accepts_color(Color::rgb(255, 0, 0));
    accepts_display(Display::Flex);
    accepts_f32(0.5_f32);
}

#[test]
fn shortcut_conversions() {
    accepts_size(px(100.0));           // Length → LengthPercentage → Size::LP
    accepts_size(percent(50.0));       // Percentage → LengthPercentage → Size::LP
    accepts_size(auto());              // Auto → Size::Auto
    accepts_auto_or_i32(5_i32);        // i32 → AutoOr::Value(5)
    accepts_auto_or_i32(auto());       // Auto → AutoOr::Auto
    accepts_color(AbsoluteColor::from_u8(255, 0, 0, 255)); // AbsoluteColor → Color
}

#[test]
fn css_wide_keywords() {
    accepts_size(inherit());
    accepts_size(initial());
    accepts_size(unset());
    accepts_size(revert());
    accepts_size(revert_layer());
    accepts_auto_or_i32(inherit());
    accepts_color(inherit());
    accepts_display(inherit());
    accepts_f32(inherit());
}

#[test]
fn var_references() {
    accepts_size(var("gap"));
    accepts_auto_or_i32(var("z-index"));
    accepts_color(var("theme-color"));
    accepts_display(var("layout"));
    accepts_f32(var("opacity"));
}

#[test]
fn specified_length_units() {
    // All unit categories work
    let _abs = px(100.0);
    let _font = em(2.0);
    let _viewport = vw(50.0);
    let _container = cqw(25.0);
    let _pct = percent(50.0);

    // They compose into LengthPercentage
    let _lp: LengthPercentage = LengthPercentage::from(em(2.0));
    let _lp: LengthPercentage = LengthPercentage::from(percent(50.0));
}

#[test]
fn length_percentage_operators() {
    // Length + Length → LengthPercentage (calc)
    let expr = px(100.0) + em(2.0);
    assert!(matches!(expr, LengthPercentage::Calc(_)));

    // Length - Length
    let expr = em(3.0) - px(20.0);
    assert!(matches!(expr, LengthPercentage::Calc(_)));

    // Length + Percentage → LengthPercentage (calc)
    let expr = px(100.0) + percent(50.0);
    assert!(matches!(expr, LengthPercentage::Calc(_)));

    // Percentage - Length → LengthPercentage (calc)
    let expr = percent(100.0) - px(20.0);
    assert!(matches!(expr, LengthPercentage::Calc(_)));
}

#[test]
fn to_computed_value_absolute() {
    let ctx = ComputeContext::default();

    // px → computed::Length
    let specified = px(100.0);
    let computed = specified.to_computed_value(&ctx);
    assert_eq!(computed.px(), 100.0);

    // cm → computed::Length (with conversion)
    let specified = cm(1.0);
    let computed = specified.to_computed_value(&ctx);
    assert!((computed.px() - 37.795_275).abs() < 0.01);
}

#[test]
fn to_computed_value_font_relative() {
    let ctx = ComputeContext {
        font_size: 20.0,
        root_font_size: 16.0,
        ..Default::default()
    };

    // em → px using font_size
    let computed = em(2.0).to_computed_value(&ctx);
    assert_eq!(computed.px(), 40.0);

    // rem → px using root_font_size
    let computed = rem(1.5).to_computed_value(&ctx);
    assert_eq!(computed.px(), 24.0);
}

#[test]
fn to_computed_value_viewport() {
    let ctx = ComputeContext {
        viewport_width: 1000.0,
        viewport_height: 800.0,
        ..Default::default()
    };

    let computed = vw(50.0).to_computed_value(&ctx);
    assert_eq!(computed.px(), 500.0);

    let computed = vh(25.0).to_computed_value(&ctx);
    assert_eq!(computed.px(), 200.0);
}

#[test]
fn to_computed_length_percentage() {
    let ctx = ComputeContext {
        font_size: 16.0,
        ..Default::default()
    };

    // Pure length → computed::LengthPercentage::Length
    let lp = LengthPercentage::from(px(100.0));
    let computed = lp.to_computed_value(&ctx);
    assert!(matches!(computed, kozan_style::computed::LengthPercentage::Length(_)));
    assert_eq!(computed.resolve(kozan_style::computed::Length::new(800.0)).px(), 100.0);

    // Pure percentage → computed::LengthPercentage::Percentage
    let lp = LengthPercentage::from(percent(50.0));
    let computed = lp.to_computed_value(&ctx);
    assert!(matches!(computed, kozan_style::computed::LengthPercentage::Percentage(_)));
    // Resolve: 50% of 800px = 400px
    assert_eq!(computed.resolve(kozan_style::computed::Length::new(800.0)).px(), 400.0);

    // calc(50% - 2em) → computed calc with px + % leaves
    let lp = percent(50.0) - em(2.0);
    let computed = lp.to_computed_value(&ctx);
    // em(2.0) at font_size=16 → 32px
    // calc(50% - 32px)
    // resolve at 800px basis: 400 - 32 = 368
    assert_eq!(computed.resolve(kozan_style::computed::Length::new(800.0)).px(), 368.0);
}

#[test]
fn size_to_computed() {
    let ctx = ComputeContext::default(); // font_size=16

    let specified: Size<LengthPercentage> = Size::LengthPercentage(
        LengthPercentage::from(em(2.0))
    );
    let computed: Size<kozan_style::computed::LengthPercentage> = specified.to_computed_value(&ctx);
    match computed {
        Size::LengthPercentage(lp) => {
            let resolved = lp.resolve(kozan_style::computed::Length::new(0.0));
            assert_eq!(resolved.px(), 32.0); // 2em * 16px
        }
        _ => panic!("expected LengthPercentage"),
    }

    let auto: Size<LengthPercentage> = Size::Auto;
    let computed: Size<kozan_style::computed::LengthPercentage> = auto.to_computed_value(&ctx);
    assert!(matches!(computed, Size::Auto));
}

#[test]
fn declaration_block_builder() {
    let mut block = DeclarationBlock::new();
    block
        .display(Display::Flex)
        .width(px(100.0))                    // Length → Size::LP
        .width(percent(50.0))                // Percentage → Size::LP
        .width(auto())                       // Auto → Size::Auto
        .width(em(2.0))                      // FontRelative → Size::LP
        .height(px(200.0))
        .min_width(px(50.0))
        .max_width(css_none())               // CssNone → MaxSize::None
        .margin_top(px(10.0))
        .margin_bottom(auto())               // Auto → Margin::Auto
        .z_index(5_i32)
        .z_index(auto())
        .color(Color::rgb(255, 0, 0))
        .opacity(0.5_f32)
        .color(inherit())
        .important()
        .color(initial())
        .normal()
        .width(var("gap"));                  // VarRef

    assert!(block.len() > 0);
    assert!(block.entries().iter().any(|(d, i)| {
        d.id() == PropertyId::Color && *i == Importance::Important
    }));
}

#[test]
fn css_math_functions() {
    // css_min! — all branches
    let _min = css_min![px(100.0), percent(50.0)];
    assert!(matches!(_min, LengthPercentage::Calc(_)));

    // css_max!
    let _max = css_max![px(200.0), percent(100.0), em(3.0)];
    assert!(matches!(_max, LengthPercentage::Calc(_)));

    // css_clamp!
    let _clamp = css_clamp![px(100.0), percent(50.0), px(800.0)];
    assert!(matches!(_clamp, LengthPercentage::Calc(_)));
}

#[test]
fn animate_lengths() {
    use kozan_style::computed::Length;

    let a = Length::new(100.0);
    let b = Length::new(200.0);

    // 50% interpolation
    let mid = a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap();
    assert_eq!(mid.px(), 150.0);

    // 0% = start
    let start = a.animate(&b, Procedure::Interpolate { progress: 0.0 }).unwrap();
    assert_eq!(start.px(), 100.0);

    // 100% = end
    let end = a.animate(&b, Procedure::Interpolate { progress: 1.0 }).unwrap();
    assert_eq!(end.px(), 200.0);

    // Addition
    let sum = a.animate(&b, Procedure::Add).unwrap();
    assert_eq!(sum.px(), 300.0);
}

#[test]
fn animate_length_percentage() {
    use kozan_style::computed::{Length, LengthPercentage as LP, Percentage};

    // Pure length animation
    let a = LP::Length(Length::new(0.0));
    let b = LP::Length(Length::new(100.0));
    let mid = a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap();
    assert_eq!(mid.resolve(Length::new(0.0)).px(), 50.0);

    // Pure percentage animation
    let a = LP::Percentage(Percentage::new(0.0));
    let b = LP::Percentage(Percentage::new(1.0));
    let mid = a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap();
    // 50% of 800px = 400px
    assert_eq!(mid.resolve(Length::new(800.0)).px(), 400.0);

    // Mixed: length + percentage → calc
    let a = LP::Length(Length::new(0.0));
    let b = LP::Percentage(Percentage::new(1.0));
    let mid = a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap();
    // At 50%: 0px + 50% → resolve at 800px basis = 400px
    assert_eq!(mid.resolve(Length::new(800.0)).px(), 400.0);
}

#[test]
fn zero_trait() {
    use kozan_style::computed::{Length, Percentage, LengthPercentage};

    assert!(Length::zero().is_zero());
    assert!(Percentage::zero().is_zero());
    assert!(LengthPercentage::zero().is_zero());
    assert!(!Length::new(1.0).is_zero());
}

#[test]
fn to_animated_zero() {
    use kozan_style::computed::{Length, LengthPercentage};

    let l = Length::new(42.0);
    let z = l.to_animated_zero().unwrap();
    assert!(z.is_zero());

    let lp = LengthPercentage::px(100.0);
    let z = lp.to_animated_zero().unwrap();
    assert!(z.is_zero());
}

#[test]
fn builder_with_expressions() {

    let mut block = DeclarationBlock::new();
    block
        // Size properties accept: Length, Percentage, LengthPercentage, auto(), css_none(), var(), inherit()
        .width(px(200.0))                         // Length → Size::LP
        .width(em(2.0))                           // FontRelative → Size::LP
        .width(percent(50.0))                     // Percentage → Size::LP
        .width(auto())                            // → Size::Auto
        .width(percent(100.0) - px(20.0))         // calc expression → Size::LP(Calc)
        .width(css_min![px(300.0), percent(50.0)]) // css_min → Size::LP(Calc)
        .width(var("gap"))                        // → Declared::Var

        // MaxSize: same but with none()
        .max_width(px(800.0))
        .max_width(css_none())                    // → MaxSize::None

        // Margin: accepts auto()
        .margin_top(px(10.0))
        .margin_top(percent(5.0))
        .margin_top(auto())                       // → Margin::Auto
        .margin_top(em(1.0) + percent(2.0))       // calc → Margin::LP(Calc)

        // Insets: LPOrAuto
        .top(px(0.0))
        .top(auto())
        .top(percent(10.0))

        // Gap: LPOrNormal
        .row_gap(px(10.0))
        .row_gap(normal())

        // Non-length properties still work
        .display(Display::Grid)
        .z_index(10_i32)
        .opacity(0.8_f32)
        .color(inherit());

    assert!(block.len() > 0);
}

#[test]
fn full_pipeline_calc_expression() {
    // User writes: width: calc(50% - 2em)
    let specified_lp = percent(50.0) - em(2.0);
    assert!(matches!(specified_lp, LengthPercentage::Calc(_)));

    // Cascade resolves with context (font-size: 20px)
    let ctx = ComputeContext {
        font_size: 20.0,
        ..Default::default()
    };
    let computed = specified_lp.to_computed_value(&ctx);

    // At computed level: calc(50% - 40px) — em resolved to px, % survives
    // Resolve at layout time with 1000px containing block
    let used = computed.resolve(kozan_style::computed::Length::new(1000.0));
    // 50% of 1000 = 500, minus 40px = 460
    assert_eq!(used.px(), 460.0);

    // Animate between two LengthPercentage values
    let a = kozan_style::computed::LengthPercentage::px(100.0);
    let b = computed; // calc(50% - 40px)
    let mid = a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap();
    // At 50%: halfway between 100px and calc(50% - 40px)
    // Resolve at 1000px: halfway between 100 and 460 = 280
    assert_eq!(mid.resolve(kozan_style::computed::Length::new(1000.0)).px(), 280.0);
}

#[test]
fn transform_helpers_ergonomic() {
    // Transform helpers accept specified::Length directly — px/em/vw all work
    let _t1 = translate(px(10.0), px(20.0));
    let _t2 = translate_x(em(2.0));
    let _t3 = translate_y(vw(50.0));
    let _t4 = perspective(px(500.0));
    let _t5 = rotate(45.0);
    let _t6 = scale(1.5);
    let _t7 = skew_x(10.0);
    let _t8 = translate_3d(px(1.0), px(2.0), px(3.0));

    // Build a transform list with the macro
    let _list = transforms![
        translate(px(10.0), px(20.0)),
        rotate(45.0),
        scale(1.5),
    ];
}

#[test]
fn atom_and_boxed_slice_ergonomic() {
    use kozan_style::Atom;

    // Interned string
    let a = Atom::new("hello");
    assert_eq!(&*a, "hello");

    // Same string shares the Arc
    let b = Atom::new("hello");
    assert_eq!(a, b);

    // Boxed slice from vec
    let items: Box<[f32]> = vec![1.0, 2.0, 3.0].into_boxed_slice();
    assert_eq!(items.len(), 3);

    // Boxed slice from array
    let items: Box<[f32]> = Box::from([1.0, 2.0]);
    assert_eq!(items.len(), 2);
}

#[test]
fn from_impls_complete() {
    // Length → LengthPercentage (From)
    let lp: LengthPercentage = px(100.0).into();
    assert!(matches!(lp, LengthPercentage::Length(_)));

    // Percentage → LengthPercentage (From)
    let lp: LengthPercentage = percent(50.0).into();
    assert!(matches!(lp, LengthPercentage::Percentage(_)));

    // Operator result is directly LengthPercentage
    let calc = px(100.0) + percent(50.0);
    assert!(matches!(calc, LengthPercentage::Calc(_)));

    // All these work with the builder via IntoDeclared bridges:
    // px(v)       → Length → Size::LP, Margin::LP, etc.
    // percent(v)  → Percentage → Size::LP, Margin::LP, etc.
    // expr        → LengthPercentage → Size::LP, Margin::LP, etc.
    // auto()      → Size::Auto, Margin::Auto, etc.
    // var("x")    → Declared::Var for any type
    // inherit()   → Declared::Inherit for any type
}

#[test]
fn color_specified_to_computed() {
    // Absolute color passes through
    let specified = Color::rgb(255, 0, 0);
    let ctx = ComputeContext::default();
    let computed = specified.to_computed_value(&ctx);
    match computed {
        ComputedColor::Absolute(c) => {
            let [r, g, b, a] = c.to_u8();
            assert_eq!((r, g, b, a), (255, 0, 0, 255));
        }
        _ => panic!("expected absolute"),
    }

    // CurrentColor stays as CurrentColor at computed level
    let specified = Color::CurrentColor;
    let computed = specified.to_computed_value(&ctx);
    assert_eq!(computed, ComputedColor::CurrentColor);

    // System color resolves to absolute based on scheme
    let light_ctx = ComputeContext { color_scheme: ColorScheme::Light, ..Default::default() };
    let dark_ctx = ComputeContext { color_scheme: ColorScheme::Dark, ..Default::default() };
    let specified = Color::System(SystemColor::Canvas);
    let light = specified.to_computed_value(&light_ctx);
    let dark = specified.to_computed_value(&dark_ctx);
    // Light canvas = white, dark canvas = dark
    assert_ne!(light, dark);
    match light {
        ComputedColor::Absolute(c) => assert_eq!(c, AbsoluteColor::WHITE),
        _ => panic!("expected absolute"),
    }
}

#[test]
fn color_resolve_current_color() {
    let current = AbsoluteColor::from_hex(0xFF0000); // red
    // CurrentColor resolves to element's color at paint time
    assert_eq!(ComputedColor::CurrentColor.resolve(current), current);
    // Absolute ignores current_color
    let blue = AbsoluteColor::from_hex(0x0000FF);
    assert_eq!(ComputedColor::Absolute(blue).resolve(current), blue);
}

#[test]
fn color_animate() {
    let red = AbsoluteColor::srgb(1.0, 0.0, 0.0, 1.0);
    let blue = AbsoluteColor::srgb(0.0, 0.0, 1.0, 1.0);
    let mid = red.animate(&blue, Procedure::Interpolate { progress: 0.5 }).unwrap();
    assert!((mid.c0() - 0.5).abs() < 0.01);
    assert!((mid.c2() - 0.5).abs() < 0.01);
    assert!((mid.alpha - 1.0).abs() < 0.01);

    // ComputedColor: absolute + absolute interpolates
    let a = ComputedColor::Absolute(red);
    let b = ComputedColor::Absolute(blue);
    let mid = a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap();
    match mid {
        ComputedColor::Absolute(c) => assert!((c.c0() - 0.5).abs() < 0.01),
        _ => panic!("expected absolute"),
    }

    // ComputedColor: currentColor + currentColor stays currentColor
    let a = ComputedColor::CurrentColor;
    let b = ComputedColor::CurrentColor;
    assert_eq!(a.animate(&b, Procedure::Interpolate { progress: 0.5 }).unwrap(), ComputedColor::CurrentColor);
}

#[test]
fn color_builder() {
    let mut block = DeclarationBlock::new();
    block
        .color(Color::rgb(255, 0, 0))
        .color(Color::CurrentColor)
        .color(Color::from_hex(0x336699))
        .color(AbsoluteColor::WHITE)
        .color(SystemColor::Canvas)
        .color(inherit());
    assert!(block.len() == 6);
}

#[test]
fn generated_enum_traits() {
    // ToCss (via Display blanket)
    assert_eq!(Display::Flex.to_css_string(), "flex");
    assert_eq!(Display::Grid.to_css_string(), "grid");
    assert_eq!(Display::InlineBlock.to_css_string(), "inline-block");

    // ToComputedValue — identity for enums
    let ctx = ComputeContext::default();
    let specified = Display::Flex;
    let computed = specified.to_computed_value(&ctx);
    assert_eq!(computed, Display::Flex);
    let back = Display::from_computed_value(&computed);
    assert_eq!(back, Display::Flex);

    // Animate — discrete: swap at 50%
    let a = Display::Block;
    let b = Display::Flex;
    let at_25 = a.animate(&b, Procedure::Interpolate { progress: 0.25 }).unwrap();
    assert_eq!(at_25, Display::Block); // still a (< 50%)
    let at_75 = a.animate(&b, Procedure::Interpolate { progress: 0.75 }).unwrap();
    assert_eq!(at_75, Display::Flex);  // swapped to b (> 50%)

    // ToAnimatedZero — Err for enums
    assert!(Display::Flex.to_animated_zero().is_err());

    // ComputeSquaredDistance
    assert_eq!(Display::Flex.compute_squared_distance(&Display::Flex).unwrap(), 0.0);
    assert_eq!(Display::Flex.compute_squared_distance(&Display::Block).unwrap(), 1.0);

    // FromStr
    assert_eq!("flex".parse::<Display>(), Ok(Display::Flex));
    assert_eq!("inline-block".parse::<Display>(), Ok(Display::InlineBlock));
}

#[test]
fn custom_properties() {
    let mut block = DeclarationBlock::new();
    block
        // Set custom properties on an element
        .property("--gap", "10px")
        .property("--theme-color", "oklch(0.7 0.15 180)")
        .property("--card-x", "20px")
        // Use them via var()
        .width(var("gap"))
        .color(var("theme-color"))
        // Complex: calc with var
        .width(var("gap") + px(100.0))
        .margin_top(percent(50.0) - var("offset"));

    assert!(block.len() == 7);

    // Verify custom property declaration
    let custom_entries: Vec<_> = block.entries().iter()
        .filter(|(d, _)| d.id() == PropertyId::Custom)
        .collect();
    assert_eq!(custom_entries.len(), 3);
}

#[test]
fn unparsed_value_operators() {
    // var + var
    let v = var("a") + var("b");
    assert_eq!(&*v.css, "calc(var(--a) + var(--b))");
    assert!(v.references.contains(SubstitutionRefs::VAR));

    // var + length
    let v = var("gap") + px(100.0);
    assert!(v.css.contains("var(--gap)"));
    assert!(v.css.contains("100px"));

    // length - var
    let v = px(50.0) - var("offset");
    assert!(v.css.contains("50px"));
    assert!(v.css.contains("var(--offset)"));

    // env
    let v = env("safe-area-inset-top");
    assert_eq!(&*v.css, "env(safe-area-inset-top)");
    assert!(v.references.contains(SubstitutionRefs::ENV));

    // env + length
    let v = env("safe-area-inset-top") + px(10.0);
    assert!(v.references.contains(SubstitutionRefs::ENV));

    // unparsed raw CSS
    let v = unparsed("calc(var(--x) + env(safe-area-inset-left))");
    assert!(v.references.contains(SubstitutionRefs::VAR));
    assert!(v.references.contains(SubstitutionRefs::ENV));

    // attr
    let v = attr("data-width");
    assert_eq!(&*v.css, "attr(data-width)");
    assert!(v.references.contains(SubstitutionRefs::ATTR));
}

#[test]
fn color_spaces() {
    // sRGB
    let c = AbsoluteColor::from_hex(0xFF0000);
    assert_eq!(c.color_space, ColorSpace::Srgb);
    assert!((c.c0() - 1.0).abs() < 0.01);

    // OKLCh
    let c = AbsoluteColor::oklch(0.7, 0.15, 180.0, 1.0);
    assert_eq!(c.color_space, ColorSpace::Oklch);
    assert_eq!(c.c0(), 0.7);

    // Display P3
    let c = AbsoluteColor::display_p3(1.0, 0.0, 0.0, 1.0);
    assert_eq!(c.color_space, ColorSpace::DisplayP3);

    // Mix
    let red = AbsoluteColor::srgb(1.0, 0.0, 0.0, 1.0);
    let blue = AbsoluteColor::srgb(0.0, 0.0, 1.0, 1.0);
    let mid = red.mix(blue, 0.5);
    assert!((mid.c0() - 0.5).abs() < 0.01);
    assert!((mid.c2() - 0.5).abs() < 0.01);

    // with_alpha
    let c = AbsoluteColor::WHITE.with_alpha(0.5);
    assert!((c.alpha - 0.5).abs() < 0.01);
}

#[test]
fn color_mix_and_light_dark() {
    let ctx = ComputeContext::default(); // Light scheme

    // color-mix in sRGB
    let mixed = Color::color_mix(
        ColorSpace::Srgb,
        Color::rgb(255, 0, 0),
        0.5,
        Color::rgb(0, 0, 255),
        0.5,
    );
    let computed = mixed.to_computed_value(&ctx);
    match computed {
        ComputedColor::Absolute(c) => assert!((c.c0() - 0.5).abs() < 0.01),
        _ => panic!("expected absolute"),
    }

    // light-dark picks light in default context
    let ld = Color::light_dark(Color::WHITE, Color::BLACK);
    let computed = ld.to_computed_value(&ctx);
    match computed {
        ComputedColor::Absolute(c) => assert_eq!(c, AbsoluteColor::WHITE),
        _ => panic!("expected absolute"),
    }

    // light-dark picks dark in dark scheme
    let dark_ctx = ComputeContext {
        color_scheme: ColorScheme::Dark,
        ..Default::default()
    };
    let computed = ld.to_computed_value(&dark_ctx);
    match computed {
        ComputedColor::Absolute(c) => assert_eq!(c, AbsoluteColor::BLACK),
        _ => panic!("expected absolute"),
    }
}
