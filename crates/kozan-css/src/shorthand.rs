//! CSS shorthand property value parsers.
//!
//! Same-type shorthands (all longhands share one type) are auto-generated in
//! `gen_parsers.rs` → `parse_shorthand_value()`. This module provides:
//!
//! 1. Generic helpers `box4`/`pair2` called by the generated code.
//! 2. Hand-written parsers for mixed-type shorthands (border-side, outline, etc.).
//! 3. The top-level `parse_shorthand()` dispatcher.

use cssparser::Parser;
use kozan_style::{PropertyDeclaration as PD, PropertyId, Declared};
use smallvec::SmallVec;
use crate::properties::Parse;
use crate::Error;

type Decls = SmallVec<[PD; 4]>;

// Type aliases — short names to avoid collisions with PropertyId variants.
type LP = kozan_style::specified::LengthPercentage;
type BStyle = kozan_style::BorderStyle;
type OStyle = kozan_style::OutlineStyle;
type CssColor = kozan_style::Color;

// Top-level dispatcher

/// Parse a shorthand property value into expanded longhand declarations.
/// Returns `None` if `id` is not a shorthand we handle.
pub(crate) fn parse_shorthand<'i>(
    id: PropertyId,
    input: &mut Parser<'i, '_>,
) -> Option<Result<Decls, Error<'i>>> {
    // 0. Hand-written overrides for generated shorthands that need special syntax.
    //    - BorderRadius: needs `/` syntax for elliptical corners (generated box4 is wrong)
    //    - FontSynthesis: keyword-toggle syntax (`weight style`), not value list (`auto auto`)
    //    - Marker: single value only, not 1-3 values (generated triple3 is too permissive)
    match id {
        PropertyId::BorderRadius => return Some(parse_border_radius(input)),
        PropertyId::FontSynthesis => return Some(parse_font_synthesis(input)),
        PropertyId::Marker => return Some(parse_marker(input)),
        // Grid shorthands use `/` separator — generated pair2/box4 only handles space.
        PropertyId::GridRow => return Some(parse_grid_pair(input, PD::GridRowStart, PD::GridRowEnd)),
        PropertyId::GridColumn => return Some(parse_grid_pair(input, PD::GridColumnStart, PD::GridColumnEnd)),
        PropertyId::GridArea => return Some(parse_grid_area(input)),
        _ => {}
    }

    // 1. Try generated same-type shorthand parsers (box4/pair2 from TOML).
    if let result @ Some(_) = crate::properties::parse_shorthand_value(id, input) {
        return result;
    }

    // 2. Hand-written mixed-type shorthands.
    Some(match id {
        // Border-side: width + style + color, any order
        PropertyId::BorderTop => border_side(
            input, PD::BorderTopWidth, PD::BorderTopStyle, PD::BorderTopColor,
        ),
        PropertyId::BorderRight => border_side(
            input, PD::BorderRightWidth, PD::BorderRightStyle, PD::BorderRightColor,
        ),
        PropertyId::BorderBottom => border_side(
            input, PD::BorderBottomWidth, PD::BorderBottomStyle, PD::BorderBottomColor,
        ),
        PropertyId::BorderLeft => border_side(
            input, PD::BorderLeftWidth, PD::BorderLeftStyle, PD::BorderLeftColor,
        ),
        PropertyId::Border => parse_border_all(input),
        PropertyId::Outline => parse_outline(input),
        PropertyId::ColumnRule => column_rule(input),

        // Border logical (WSC × 2 sides)
        PropertyId::BorderBlock => parse_border_logical(input,
            PD::BorderBlockStartWidth, PD::BorderBlockEndWidth,
            PD::BorderBlockStartStyle, PD::BorderBlockEndStyle,
            PD::BorderBlockStartColor, PD::BorderBlockEndColor,
        ),
        PropertyId::BorderInline => parse_border_logical(input,
            PD::BorderInlineStartWidth, PD::BorderInlineEndWidth,
            PD::BorderInlineStartStyle, PD::BorderInlineEndStyle,
            PD::BorderInlineStartColor, PD::BorderInlineEndColor,
        ),

        // Flex
        PropertyId::FlexFlow => parse_flex_flow(input),
        PropertyId::Flex => parse_flex(input),

        // Text
        PropertyId::TextDecoration => parse_text_decoration(input),
        PropertyId::TextEmphasis => parse_text_emphasis(input),
        PropertyId::WhiteSpace => parse_white_space(input),

        // Multi-column
        PropertyId::Columns => parse_columns(input),

        // Container
        PropertyId::Container => parse_container(input),

        // Font
        PropertyId::Font => parse_font(input),

        // Place
        PropertyId::PlaceContent => parse_place_content(input),

        // Font sub-shorthands
        PropertyId::FontSynthesis => parse_font_synthesis(input),
        PropertyId::FontVariant => parse_font_variant(input),

        // List
        PropertyId::ListStyle => parse_list_style(input),

        // Animation / Transition
        PropertyId::Transition => parse_transition(input),
        PropertyId::Animation => parse_animation(input),

        // Background
        PropertyId::Background => parse_background(input),

        // SVG marker
        PropertyId::Marker => parse_marker(input),

        // Border image
        PropertyId::BorderImage => parse_border_image(input),

        // Mask
        PropertyId::Mask => parse_mask(input),

        // Grid
        PropertyId::GridTemplate => parse_grid_template(input),
        PropertyId::Grid => parse_grid(input),

        _ => return None,
    })
}

// Generic helpers — called by generated code

/// Parse 1–4 values for box-model shorthands.
/// CSS expansion: 1→all, 2→vert/horiz, 3→top/horiz/bottom, 4→TRBL.
pub(crate) fn box4<'i, T: Parse + Clone>(
    input: &mut Parser<'i, '_>,
    top: fn(Declared<T>) -> PD,
    right: fn(Declared<T>) -> PD,
    bottom: fn(Declared<T>) -> PD,
    left: fn(Declared<T>) -> PD,
) -> Result<Decls, Error<'i>> {
    let a = T::parse(input)?;
    let b = input.try_parse(T::parse).ok();
    let c = if b.is_some() { input.try_parse(T::parse).ok() } else { None };
    let d = if c.is_some() { input.try_parse(T::parse).ok() } else { None };

    let (t, r, bo, l) = expand_trbl(a, b, c, d);

    Ok(smallvec::smallvec![
        top(Declared::Value(t)),
        right(Declared::Value(r)),
        bottom(Declared::Value(bo)),
        left(Declared::Value(l)),
    ])
}

/// Expand 1–4 values to TRBL (top, right, bottom, left).
/// Reusable helper for `box4` and `parse_border_radius`.
fn expand_trbl<T: Clone>(a: T, b: Option<T>, c: Option<T>, d: Option<T>) -> (T, T, T, T) {
    match (b, c, d) {
        (None, _, _) => (a.clone(), a.clone(), a.clone(), a),
        (Some(b), None, _) => (a.clone(), b.clone(), a, b),
        (Some(b), Some(c), None) => (a, b.clone(), c, b),
        (Some(b), Some(c), Some(d)) => (a, b, c, d),
    }
}

/// `border-radius` shorthand — `<h1> <h2>? <h3>? <h4>? [ / <v1> <v2>? <v3>? <v4>? ]?`.
///
/// Overrides the generated `box4<CornerRadius>` which would greedily consume
/// two values per corner. The shorthand uses `/` to separate horizontal from
/// vertical radii — each side is expanded independently via 1–4 TRBL rules.
fn parse_border_radius<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type CR = kozan_style::CornerRadius;

    // Parse 1–4 horizontal radii.
    let h1 = LP::parse(input)?;
    let h2 = input.try_parse(LP::parse).ok();
    let h3 = if h2.is_some() { input.try_parse(LP::parse).ok() } else { None };
    let h4 = if h3.is_some() { input.try_parse(LP::parse).ok() } else { None };
    let (ht, hr, hb, hl) = expand_trbl(h1, h2, h3, h4);

    // Optional `/` + 1–4 vertical radii. If absent, vertical = horizontal.
    let (vt, vr, vb, vl) = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        let v1 = LP::parse(input)?;
        let v2 = input.try_parse(LP::parse).ok();
        let v3 = if v2.is_some() { input.try_parse(LP::parse).ok() } else { None };
        let v4 = if v3.is_some() { input.try_parse(LP::parse).ok() } else { None };
        expand_trbl(v1, v2, v3, v4)
    } else {
        (ht.clone(), hr.clone(), hb.clone(), hl.clone())
    };

    Ok(smallvec::smallvec![
        PD::BorderTopLeftRadius(Declared::Value(CR { horizontal: ht, vertical: vt })),
        PD::BorderTopRightRadius(Declared::Value(CR { horizontal: hr, vertical: vr })),
        PD::BorderBottomRightRadius(Declared::Value(CR { horizontal: hb, vertical: vb })),
        PD::BorderBottomLeftRadius(Declared::Value(CR { horizontal: hl, vertical: vl })),
    ])
}

/// Parse 1–3 values for 3-component shorthands (single value → all three).
pub(crate) fn triple3<'i, T: Parse + Clone>(
    input: &mut Parser<'i, '_>,
    first: fn(Declared<T>) -> PD,
    second: fn(Declared<T>) -> PD,
    third: fn(Declared<T>) -> PD,
) -> Result<Decls, Error<'i>> {
    let a = T::parse(input)?;
    let b = input.try_parse(T::parse).ok();
    let c = if b.is_some() { input.try_parse(T::parse).ok() } else { None };

    let (v1, v2, v3) = match (b, c) {
        (None, _) => (a.clone(), a.clone(), a),
        (Some(b), None) => (a, b.clone(), b),
        (Some(b), Some(c)) => (a, b, c),
    };

    Ok(smallvec::smallvec![
        first(Declared::Value(v1)),
        second(Declared::Value(v2)),
        third(Declared::Value(v3)),
    ])
}

/// Parse 1–2 values for 2-component shorthands.
/// If only one value, both components get the same value.
pub(crate) fn pair2<'i, T: Parse + Clone>(
    input: &mut Parser<'i, '_>,
    first: fn(Declared<T>) -> PD,
    second: fn(Declared<T>) -> PD,
) -> Result<Decls, Error<'i>> {
    let a = T::parse(input)?;
    let b = input.try_parse(T::parse).unwrap_or_else(|_| a.clone());
    Ok(smallvec::smallvec![
        first(Declared::Value(a)),
        second(Declared::Value(b)),
    ])
}

// Hand-written: border-like (width + style + color in any order)

/// Parse width/style/color in any order. Generic over the style type
/// (`BorderStyle` for border sides, `OutlineStyle` for outline).
fn parse_wsc_generic<'i, S: Parse>(
    input: &mut Parser<'i, '_>,
) -> Result<(Option<LP>, Option<S>, Option<CssColor>), Error<'i>> {
    let mut width = None;
    let mut style = None;
    let mut color = None;

    for _ in 0..3 {
        if width.is_none() {
            if let Ok(v) = input.try_parse(LP::parse) { width = Some(v); continue; }
        }
        if style.is_none() {
            if let Ok(v) = input.try_parse(S::parse) { style = Some(v); continue; }
        }
        if color.is_none() {
            if let Ok(v) = input.try_parse(CssColor::parse) { color = Some(v); continue; }
        }
        break;
    }

    if width.is_none() && style.is_none() && color.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok((width, style, color))
}

/// Border-specific WSC (width + BorderStyle + color).
fn parse_wsc<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<(Option<LP>, Option<BStyle>, Option<CssColor>), Error<'i>> {
    parse_wsc_generic::<BStyle>(input)
}

/// Single border side: `border-top: 1px solid red`.
fn border_side<'i>(
    input: &mut Parser<'i, '_>,
    mk_w: fn(Declared<LP>) -> PD,
    mk_s: fn(Declared<BStyle>) -> PD,
    mk_c: fn(Declared<CssColor>) -> PD,
) -> Result<Decls, Error<'i>> {
    let (w, s, c) = parse_wsc(input)?;
    Ok(smallvec::smallvec![
        mk_w(w.map_or(Declared::Initial, Declared::Value)),
        mk_s(s.map_or(Declared::Initial, Declared::Value)),
        mk_c(c.map_or(Declared::Initial, Declared::Value)),
    ])
}

/// `border` shorthand — same width/style/color for all 4 sides (12 longhands).
fn parse_border_all<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    let (w, s, c) = parse_wsc(input)?;
    let dw = w.map_or(Declared::Initial, Declared::Value);
    let ds = s.map_or(Declared::Initial, Declared::Value);
    let dc = c.map_or(Declared::Initial, Declared::Value);
    Ok(smallvec::smallvec![
        PD::BorderTopWidth(dw.clone()), PD::BorderRightWidth(dw.clone()),
        PD::BorderBottomWidth(dw.clone()), PD::BorderLeftWidth(dw),
        PD::BorderTopStyle(ds.clone()), PD::BorderRightStyle(ds.clone()),
        PD::BorderBottomStyle(ds.clone()), PD::BorderLeftStyle(ds),
        PD::BorderTopColor(dc.clone()), PD::BorderRightColor(dc.clone()),
        PD::BorderBottomColor(dc.clone()), PD::BorderLeftColor(dc),
    ])
}

/// `outline` shorthand — width + style + color, any order.
/// Reuses `parse_wsc_generic` with `OutlineStyle` (includes `auto`).
fn parse_outline<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    let (w, s, c) = parse_wsc_generic::<OStyle>(input)?;
    Ok(smallvec::smallvec![
        PD::OutlineWidth(w.map_or(Declared::Initial, Declared::Value)),
        PD::OutlineStyle(s.map_or(Declared::Initial, Declared::Value)),
        PD::OutlineColor(c.map_or(Declared::Initial, Declared::Value)),
    ])
}

/// `column-rule` shorthand — width + style + color, any order.
fn column_rule<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    let (w, s, c) = parse_wsc(input)?;
    Ok(smallvec::smallvec![
        PD::ColumnRuleWidth(w.map_or(Declared::Initial, Declared::Value)),
        PD::ColumnRuleStyle(s.map_or(Declared::Initial, Declared::Value)),
        PD::ColumnRuleColor(c.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: flex shorthands

/// `border-block` / `border-inline` — WSC applied to both sides (6 longhands).
fn parse_border_logical<'i>(
    input: &mut Parser<'i, '_>,
    mk_sw: fn(Declared<LP>) -> PD,
    mk_ew: fn(Declared<LP>) -> PD,
    mk_ss: fn(Declared<BStyle>) -> PD,
    mk_es: fn(Declared<BStyle>) -> PD,
    mk_sc: fn(Declared<CssColor>) -> PD,
    mk_ec: fn(Declared<CssColor>) -> PD,
) -> Result<Decls, Error<'i>> {
    let (w, s, c) = parse_wsc(input)?;
    let dw = w.map_or(Declared::Initial, Declared::Value);
    let ds = s.map_or(Declared::Initial, Declared::Value);
    let dc = c.map_or(Declared::Initial, Declared::Value);
    Ok(smallvec::smallvec![
        mk_sw(dw.clone()), mk_ew(dw),
        mk_ss(ds.clone()), mk_es(ds),
        mk_sc(dc.clone()), mk_ec(dc),
    ])
}

// Hand-written: flex shorthands

/// `flex-flow` shorthand — direction + wrap, any order.
fn parse_flex_flow<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type FlexDir = kozan_style::FlexDirection;
    type FlexWrp = kozan_style::FlexWrap;

    let mut direction = None;
    let mut wrap = None;

    for _ in 0..2 {
        if direction.is_none() {
            if let Ok(v) = input.try_parse(FlexDir::parse) {
                direction = Some(v);
                continue;
            }
        }
        if wrap.is_none() {
            if let Ok(v) = input.try_parse(FlexWrp::parse) {
                wrap = Some(v);
                continue;
            }
        }
        break;
    }

    if direction.is_none() && wrap.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::FlexDirection(direction.map_or(Declared::Initial, Declared::Value)),
        PD::FlexWrap(wrap.map_or(Declared::Initial, Declared::Value)),
    ])
}

/// `flex` shorthand — grow [shrink [basis]].
///
/// Special keywords: `none` → 0 0 auto, `auto` → 1 1 auto.
/// Single number: grow=N, shrink=1, basis=0px.
/// Single length: grow=1, shrink=1, basis=length.
fn parse_flex<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type FlexBasis = kozan_style::generics::Size<LP>;

    // flex: none → 0 0 auto
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::FlexGrow(Declared::Value(0.0)),
            PD::FlexShrink(Declared::Value(0.0)),
            PD::FlexBasis(Declared::Value(FlexBasis::Auto)),
        ]);
    }

    // flex: auto → 1 1 auto
    if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::FlexGrow(Declared::Value(1.0)),
            PD::FlexShrink(Declared::Value(1.0)),
            PD::FlexBasis(Declared::Value(FlexBasis::Auto)),
        ]);
    }

    let zero_basis = FlexBasis::LengthPercentage(
        kozan_style::specified::LengthPercentage::from(kozan_style::specified::length::px(0.0)),
    );

    // Try <number> first (flex-grow)
    if let Ok(grow) = input.try_parse(|i| i.expect_number()) {
        let shrink = input.try_parse(|i| i.expect_number()).unwrap_or(1.0);
        let basis = input.try_parse(FlexBasis::parse).unwrap_or(zero_basis);
        return Ok(smallvec::smallvec![
            PD::FlexGrow(Declared::Value(grow)),
            PD::FlexShrink(Declared::Value(shrink)),
            PD::FlexBasis(Declared::Value(basis)),
        ]);
    }

    // Try <length-percentage> (flex-basis)
    let basis = FlexBasis::parse(input)?;
    Ok(smallvec::smallvec![
        PD::FlexGrow(Declared::Value(1.0)),
        PD::FlexShrink(Declared::Value(1.0)),
        PD::FlexBasis(Declared::Value(basis)),
    ])
}

// Hand-written: text shorthands

/// `text-decoration` shorthand — line + style + color + thickness, any order.
fn parse_text_decoration<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type TDLine = kozan_style::TextDecorationLine;
    type TDStyle = kozan_style::TextDecorationStyle;
    type TDThickness = kozan_style::generics::LengthPercentageOrAuto<LP>;

    let mut line = None;
    let mut style = None;
    let mut color = None;
    let mut thickness = None;

    for _ in 0..4 {
        if line.is_none() {
            if let Ok(v) = input.try_parse(TDLine::parse) {
                line = Some(v);
                continue;
            }
        }
        if style.is_none() {
            if let Ok(v) = input.try_parse(TDStyle::parse) {
                style = Some(v);
                continue;
            }
        }
        // Try thickness before color — LP won't consume color keywords.
        if thickness.is_none() {
            if let Ok(v) = input.try_parse(TDThickness::parse) {
                thickness = Some(v);
                continue;
            }
        }
        if color.is_none() {
            if let Ok(v) = input.try_parse(CssColor::parse) {
                color = Some(v);
                continue;
            }
        }
        break;
    }

    if line.is_none() && style.is_none() && color.is_none() && thickness.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::TextDecorationLine(line.map_or(Declared::Initial, Declared::Value)),
        PD::TextDecorationStyle(style.map_or(Declared::Initial, Declared::Value)),
        PD::TextDecorationColor(color.map_or(Declared::Initial, Declared::Value)),
        PD::TextDecorationThickness(thickness.map_or(Declared::Initial, Declared::Value)),
    ])
}

/// `text-emphasis` shorthand — style + color, any order.
fn parse_text_emphasis<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type TEStyle = kozan_style::TextEmphasisStyleValue;

    let mut style = None;
    let mut color = None;

    for _ in 0..2 {
        if style.is_none() {
            if let Ok(v) = input.try_parse(TEStyle::parse) {
                style = Some(v);
                continue;
            }
        }
        if color.is_none() {
            if let Ok(v) = input.try_parse(CssColor::parse) {
                color = Some(v);
                continue;
            }
        }
        break;
    }

    if style.is_none() && color.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::TextEmphasisStyle(style.map_or(Declared::Initial, Declared::Value)),
        PD::TextEmphasisColor(color.map_or(Declared::Initial, Declared::Value)),
    ])
}

/// `white-space` shorthand — collapse + wrap-mode, any order.
///
/// Also handles legacy keywords per CSS Text 4:
/// `normal` → collapse + wrap, `nowrap` → collapse + nowrap,
/// `pre` → preserve + nowrap, `pre-wrap` → preserve + wrap,
/// `pre-line` → preserve-breaks + wrap, `break-spaces` → break-spaces + wrap.
fn parse_white_space<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type WSCollapse = kozan_style::WhiteSpaceCollapse;
    type TWMode = kozan_style::TextWrapMode;

    // Try legacy keywords first (uses try_parse so input resets on failure).
    if let Ok((c, w)) = input.try_parse(|i| {
        let ident = i.expect_ident()?;
        match &**ident {
            s if s.eq_ignore_ascii_case("normal") => Ok((WSCollapse::Collapse, TWMode::Wrap)),
            s if s.eq_ignore_ascii_case("nowrap") => Ok((WSCollapse::Collapse, TWMode::Nowrap)),
            s if s.eq_ignore_ascii_case("pre") => Ok((WSCollapse::Preserve, TWMode::Nowrap)),
            s if s.eq_ignore_ascii_case("pre-wrap") => Ok((WSCollapse::Preserve, TWMode::Wrap)),
            s if s.eq_ignore_ascii_case("pre-line") => Ok((WSCollapse::PreserveBreaks, TWMode::Wrap)),
            s if s.eq_ignore_ascii_case("break-spaces") => Ok((WSCollapse::BreakSpaces, TWMode::Wrap)),
            _ => Err(i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue)),
        }
    }) {
        return Ok(smallvec::smallvec![
            PD::WhiteSpaceCollapse(Declared::Value(c)),
            PD::TextWrapMode(Declared::Value(w)),
        ]);
    }

    // Component value parsing: <white-space-collapse> || <text-wrap-mode>.
    let mut collapse = None;
    let mut wrap = None;

    for _ in 0..2 {
        if collapse.is_none() {
            if let Ok(v) = input.try_parse(WSCollapse::parse) {
                collapse = Some(v);
                continue;
            }
        }
        if wrap.is_none() {
            if let Ok(v) = input.try_parse(TWMode::parse) {
                wrap = Some(v);
                continue;
            }
        }
        break;
    }

    if collapse.is_none() && wrap.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::WhiteSpaceCollapse(collapse.map_or(Declared::Initial, Declared::Value)),
        PD::TextWrapMode(wrap.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: multi-column

/// `columns` shorthand — width + count, any order.
/// Width tried first to avoid `auto` ambiguity (both accept it).
fn parse_columns<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type ColWidth = kozan_style::generics::LengthPercentageOrAuto<LP>;
    let mut width = None;
    let mut count = None;

    for _ in 0..2 {
        // Try count first — integers are unambiguous (no units).
        if count.is_none() {
            if let Ok(v) = input.try_parse(|i| {
                let n = i.expect_integer()?;
                if n < 1 { return Err(i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue)); }
                Ok(kozan_style::AutoOr::Value(n as u32))
            }) {
                count = Some(v);
                continue;
            }
        }
        // Then width (lengths need units, won't match bare integers).
        if width.is_none() {
            if let Ok(v) = input.try_parse(ColWidth::parse) {
                width = Some(v);
                continue;
            }
        }
        break;
    }

    if width.is_none() && count.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::ColumnWidth(width.map_or(Declared::Initial, Declared::Value)),
        PD::ColumnCount(count.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: container

/// `container` shorthand — `<name> [ / <type> ]`.
///
/// CSS spec: name comes first, type after the optional `/`.
/// `container: sidebar / inline-size` → name=sidebar, type=inline-size.
fn parse_container<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type CType = kozan_style::ContainerType;
    type CName = kozan_style::Ident;

    let name = CName::parse(input)?;

    let ctype = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        Declared::Value(CType::parse(input)?)
    } else {
        Declared::Initial
    };

    Ok(smallvec::smallvec![
        PD::ContainerType(ctype),
        PD::ContainerName(Declared::Value(name)),
    ])
}

// Hand-written: font

/// `font` shorthand — `[style||variant||weight||stretch]? size [/line-height]? family`.
///
/// The optional prefix (style, variant-caps, weight, stretch) can appear in any
/// order. Then font-size is REQUIRED, optionally followed by `/line-height`.
/// Finally, font-family is REQUIRED and consumes the rest.
fn parse_font<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type FStyle = kozan_style::FontStyle;
    type FVCaps = kozan_style::FontVariantCaps;
    type FWeight = kozan_style::FontWeight;
    type FStretch = kozan_style::FontStretch;
    type LHeight = kozan_style::LineHeight;
    type FFamily = kozan_style::FontFamily;

    let mut style = None;
    let mut variant = None;
    let mut weight = None;
    let mut stretch = None;

    // Optional prefix: style, variant-caps, weight, stretch in any order.
    for _ in 0..4 {
        if style.is_none() {
            if let Ok(v) = input.try_parse(FStyle::parse) {
                style = Some(v);
                continue;
            }
        }
        if weight.is_none() {
            if let Ok(v) = input.try_parse(FWeight::parse) {
                weight = Some(v);
                continue;
            }
        }
        if variant.is_none() {
            if let Ok(v) = input.try_parse(FVCaps::parse) {
                variant = Some(v);
                continue;
            }
        }
        if stretch.is_none() {
            if let Ok(v) = input.try_parse(FStretch::parse) {
                stretch = Some(v);
                continue;
            }
        }
        break;
    }

    // Required: font-size.
    let size = kozan_style::FontSize::parse(input)?;

    // Optional: / line-height.
    let line_height = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        Declared::Value(LHeight::parse(input)?)
    } else {
        Declared::Initial
    };

    // Required: font-family (consumes rest).
    let family = FFamily::parse(input)?;

    Ok(smallvec::smallvec![
        PD::FontStyle(style.map_or(Declared::Initial, Declared::Value)),
        PD::FontVariantCaps(variant.map_or(Declared::Initial, Declared::Value)),
        PD::FontWeight(weight.map_or(Declared::Initial, Declared::Value)),
        PD::FontStretch(stretch.map_or(Declared::Initial, Declared::Value)),
        PD::FontSize(Declared::Value(size)),
        PD::LineHeight(line_height),
        PD::FontFamily(Declared::Value(family)),
    ])
}

// Hand-written: place-content

/// `place-content` shorthand — `<align-content> [<justify-content>]`.
/// Single value sets both; different types so we re-parse from saved state.
fn parse_place_content<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type AC = kozan_style::AlignContent;
    type JC = kozan_style::JustifyContent;

    // Save state BEFORE parsing align, so we can re-parse for justify if single-value.
    let state = input.state();
    let align = AC::parse(input)?;
    let justify = if let Ok(j) = input.try_parse(JC::parse) {
        // Two-value form: place-content: center space-between
        j
    } else {
        // Single value — re-parse same keyword as JustifyContent.
        let saved = input.state();
        input.reset(&state);
        let j = JC::parse(input)?;
        // Skip back to where we were (after the single token).
        input.reset(&saved);
        j
    };
    Ok(smallvec::smallvec![
        PD::AlignContent(Declared::Value(align)),
        PD::JustifyContent(Declared::Value(justify)),
    ])
}

// Hand-written: font-synthesis

/// `font-synthesis` shorthand — `none | [weight || style || small-caps]`.
fn parse_font_synthesis<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type AON = kozan_style::AutoOrNone;

    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::FontSynthesisWeight(Declared::Value(AON::None)),
            PD::FontSynthesisStyle(Declared::Value(AON::None)),
            PD::FontSynthesisSmallCaps(Declared::Value(AON::None)),
        ]);
    }

    let mut weight = false;
    let mut style = false;
    let mut small_caps = false;

    for _ in 0..3 {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            match &*ident {
                s if s.eq_ignore_ascii_case("weight") => { weight = true; continue; }
                s if s.eq_ignore_ascii_case("style") => { style = true; continue; }
                s if s.eq_ignore_ascii_case("small-caps") => { small_caps = true; continue; }
                _ => return Err(input.new_custom_error(crate::CustomError::InvalidValue)),
            }
        }
        break;
    }

    if !weight && !style && !small_caps {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::FontSynthesisWeight(Declared::Value(if weight { AON::Auto } else { AON::None })),
        PD::FontSynthesisStyle(Declared::Value(if style { AON::Auto } else { AON::None })),
        PD::FontSynthesisSmallCaps(Declared::Value(if small_caps { AON::Auto } else { AON::None })),
    ])
}

// Hand-written: font-variant

/// `font-variant` shorthand — `normal | none | [caps || ligatures || numeric || east-asian || alternates || position]`.
fn parse_font_variant<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type FVCaps = kozan_style::FontVariantCaps;
    type FVLig = kozan_style::FontVariantLigatures;
    type FVNum = kozan_style::FontVariantNumeric;
    type FVEA = kozan_style::FontVariantEastAsian;
    type FVAlt = kozan_style::FontVariantAlternates;
    type FVPos = kozan_style::FontVariantPosition;

    // font-variant: normal → all initial
    if input.try_parse(|i| i.expect_ident_matching("normal")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::FontVariantCaps(Declared::Initial),
            PD::FontVariantLigatures(Declared::Initial),
            PD::FontVariantNumeric(Declared::Initial),
            PD::FontVariantEastAsian(Declared::Initial),
            PD::FontVariantAlternates(Declared::Initial),
            PD::FontVariantPosition(Declared::Initial),
        ]);
    }

    // font-variant: none → ligatures=none (0 = no ligatures), rest initial
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::FontVariantCaps(Declared::Initial),
            PD::FontVariantLigatures(Declared::Value(kozan_style::FontVariantLigatures(0))),
            PD::FontVariantNumeric(Declared::Initial),
            PD::FontVariantEastAsian(Declared::Initial),
            PD::FontVariantAlternates(Declared::Initial),
            PD::FontVariantPosition(Declared::Initial),
        ]);
    }

    let mut caps = None;
    let mut ligatures = None;
    let mut numeric = None;
    let mut east_asian = None;
    let mut alternates = None;
    let mut position = None;

    for _ in 0..6 {
        if caps.is_none() {
            if let Ok(v) = input.try_parse(FVCaps::parse) { caps = Some(v); continue; }
        }
        if ligatures.is_none() {
            if let Ok(v) = input.try_parse(FVLig::parse) { ligatures = Some(v); continue; }
        }
        if numeric.is_none() {
            if let Ok(v) = input.try_parse(FVNum::parse) { numeric = Some(v); continue; }
        }
        if east_asian.is_none() {
            if let Ok(v) = input.try_parse(FVEA::parse) { east_asian = Some(v); continue; }
        }
        if alternates.is_none() {
            if let Ok(v) = input.try_parse(FVAlt::parse) { alternates = Some(v); continue; }
        }
        if position.is_none() {
            if let Ok(v) = input.try_parse(FVPos::parse) { position = Some(v); continue; }
        }
        break;
    }

    if caps.is_none() && ligatures.is_none() && numeric.is_none()
       && east_asian.is_none() && alternates.is_none() && position.is_none()
    {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::FontVariantCaps(caps.map_or(Declared::Initial, Declared::Value)),
        PD::FontVariantLigatures(ligatures.map_or(Declared::Initial, Declared::Value)),
        PD::FontVariantNumeric(numeric.map_or(Declared::Initial, Declared::Value)),
        PD::FontVariantEastAsian(east_asian.map_or(Declared::Initial, Declared::Value)),
        PD::FontVariantAlternates(alternates.map_or(Declared::Initial, Declared::Value)),
        PD::FontVariantPosition(position.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: list-style

/// `list-style` shorthand — `<type> || <position> || <image>`, any order.
fn parse_list_style<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type LSType = kozan_style::ListStyleType;
    type LSPos = kozan_style::ListStylePosition;
    type Img = kozan_style::Image;

    // list-style: none → type=none, image=none
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::ListStyleType(Declared::Value(LSType::None)),
            PD::ListStylePosition(Declared::Initial),
            PD::ListStyleImage(Declared::Value(Img::None)),
        ]);
    }

    let mut ls_type = None;
    let mut position = None;
    let mut image = None;

    for _ in 0..3 {
        if position.is_none() {
            if let Ok(v) = input.try_parse(LSPos::parse) { position = Some(v); continue; }
        }
        if image.is_none() {
            if let Ok(v) = input.try_parse(Img::parse) { image = Some(v); continue; }
        }
        if ls_type.is_none() {
            if let Ok(v) = input.try_parse(LSType::parse) { ls_type = Some(v); continue; }
        }
        break;
    }

    if ls_type.is_none() && position.is_none() && image.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::ListStyleType(ls_type.map_or(Declared::Initial, Declared::Value)),
        PD::ListStylePosition(position.map_or(Declared::Initial, Declared::Value)),
        PD::ListStyleImage(image.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: transition

/// `transition` shorthand — comma-separated `<single-transition>` groups.
///
/// Each group: `[property] || <time> || <easing> || <time> || [behavior]`
/// First `<time>` = duration, second = delay.
fn parse_transition<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    use kozan_style::{
        TransitionPropertyList, TransitionBehaviorList, TransitionBehavior,
        DurationList, TimingFunctionList, TimingFunction, Atom,
    };
    use std::time::Duration;

    // transition: none
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::TransitionProperty(Declared::Value(TransitionPropertyList::None)),
            PD::TransitionDuration(Declared::Initial),
            PD::TransitionTimingFunction(Declared::Initial),
            PD::TransitionDelay(Declared::Initial),
            PD::TransitionBehavior(Declared::Initial),
        ]);
    }

    let mut properties = Vec::new();
    let mut durations = Vec::new();
    let mut timings = Vec::new();
    let mut delays = Vec::new();
    let mut behaviors = Vec::new();

    loop {
        let mut prop: Option<Option<Atom>> = None; // None=not set, Some(None)=all, Some(Some(x))=named
        let mut dur = None;
        let mut timing = None;
        let mut delay = None;
        let mut behavior = None;

        for _ in 0..5 {
            if timing.is_none() {
                if let Ok(v) = input.try_parse(<kozan_style::TimingFunction as crate::Parse>::parse) {
                    timing = Some(v); continue;
                }
            }
            if dur.is_none() || delay.is_none() {
                if let Ok(t) = input.try_parse(<std::time::Duration as crate::Parse>::parse) {
                    if dur.is_none() { dur = Some(t); } else { delay = Some(t); }
                    continue;
                }
            }
            if behavior.is_none() {
                if let Ok(v) = input.try_parse(|i| {
                    let ident = i.expect_ident()?;
                    if ident.eq_ignore_ascii_case("allow-discrete") {
                        Ok(TransitionBehavior::AllowDiscrete)
                    } else if ident.eq_ignore_ascii_case("normal") {
                        // "normal" as behavior conflicts with potential property name;
                        // only match if nothing else consumed yet or explicitly positioned.
                        Ok(TransitionBehavior::Normal)
                    } else {
                        Err(i.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue))
                    }
                }) {
                    behavior = Some(v); continue;
                }
            }
            if prop.is_none() {
                if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
                    if ident.eq_ignore_ascii_case("all") {
                        prop = Some(None);
                    } else {
                        prop = Some(Some(Atom::new(&*ident)));
                    }
                    continue;
                }
            }
            break;
        }

        properties.push(prop);
        durations.push(dur.unwrap_or(Duration::ZERO));
        timings.push(timing.unwrap_or(TimingFunction::Ease));
        delays.push(delay.unwrap_or(Duration::ZERO));
        behaviors.push(behavior.unwrap_or(TransitionBehavior::Normal));

        if input.try_parse(|i| i.expect_comma()).is_err() { break; }
    }

    let prop_list = if properties.len() == 1 && properties[0].is_none() {
        TransitionPropertyList::All
    } else {
        let atoms: Vec<Atom> = properties.into_iter().map(|p| match p {
            Some(Some(atom)) => atom,
            _ => Atom::new("all"),
        }).collect();
        TransitionPropertyList::Properties(atoms.into())
    };

    Ok(smallvec::smallvec![
        PD::TransitionProperty(Declared::Value(prop_list)),
        PD::TransitionDuration(Declared::Value(DurationList(durations.into()))),
        PD::TransitionTimingFunction(Declared::Value(TimingFunctionList(timings.into()))),
        PD::TransitionDelay(Declared::Value(DurationList(delays.into()))),
        PD::TransitionBehavior(Declared::Value(TransitionBehaviorList(behaviors.into()))),
    ])
}

// Hand-written: animation

/// `animation` shorthand — comma-separated `<single-animation>` groups.
///
/// Each group: `<time> || <easing> || <time> || <iteration-count> || <direction> ||
///              <fill-mode> || <play-state> || [none | <name>]`
/// First `<time>` = duration, second = delay.
fn parse_animation<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    use kozan_style::{
        AnimationNameList, AnimationDirectionList, AnimationFillModeList,
        AnimationIterationCountList, AnimationPlayStateList,
        DurationList, TimingFunctionList, TimingFunction,
        AnimationDirection, AnimationFillMode, AnimationPlayState,
        IterationCount, Atom,
    };
    use std::time::Duration;

    // animation: none
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::AnimationName(Declared::Value(AnimationNameList::None)),
            PD::AnimationDuration(Declared::Initial),
            PD::AnimationTimingFunction(Declared::Initial),
            PD::AnimationDelay(Declared::Initial),
            PD::AnimationIterationCount(Declared::Initial),
            PD::AnimationDirection(Declared::Initial),
            PD::AnimationFillMode(Declared::Initial),
            PD::AnimationPlayState(Declared::Initial),
        ]);
    }

    let mut names = Vec::new();
    let mut durations = Vec::new();
    let mut timings = Vec::new();
    let mut delays = Vec::new();
    let mut iterations = Vec::new();
    let mut directions = Vec::new();
    let mut fill_modes = Vec::new();
    let mut play_states = Vec::new();

    loop {
        let mut name: Option<Atom> = None;
        let mut dur = None;
        let mut timing = None;
        let mut delay = None;
        let mut iteration = None;
        let mut direction = None;
        let mut fill_mode = None;
        let mut play_state = None;
        let mut found_any = false;

        for _ in 0..8 {
            // Timing function (keyword + function forms)
            if timing.is_none() {
                if let Ok(v) = input.try_parse(<kozan_style::TimingFunction as crate::Parse>::parse) {
                    timing = Some(v); found_any = true; continue;
                }
            }
            // Time (first = duration, second = delay)
            if dur.is_none() || delay.is_none() {
                if let Ok(t) = input.try_parse(<std::time::Duration as crate::Parse>::parse) {
                    if dur.is_none() { dur = Some(t); } else { delay = Some(t); }
                    found_any = true; continue;
                }
            }
            // Iteration count
            if iteration.is_none() {
                if let Ok(v) = input.try_parse(<kozan_style::IterationCount as crate::Parse>::parse) {
                    iteration = Some(v); found_any = true; continue;
                }
            }
            // Direction
            if direction.is_none() {
                if let Ok(v) = input.try_parse(AnimationDirection::parse) {
                    direction = Some(v); found_any = true; continue;
                }
            }
            // Play state
            if play_state.is_none() {
                if let Ok(v) = input.try_parse(AnimationPlayState::parse) {
                    play_state = Some(v); found_any = true; continue;
                }
            }
            // Fill mode — skip "none" (reserved for animation-name in shorthand context)
            if fill_mode.is_none() {
                if let Ok(v) = input.try_parse(|i| {
                    let loc = i.current_source_location();
                    let ident = i.expect_ident()?;
                    use kozan_style_macros::css_match;
                    css_match! { &ident,
                        "forwards" => Ok(AnimationFillMode::Forwards),
                        "backwards" => Ok(AnimationFillMode::Backwards),
                        "both" => Ok(AnimationFillMode::Both),
                        _ => Err(loc.new_custom_error::<_, crate::CustomError>(crate::CustomError::InvalidValue))
                    }
                }) {
                    fill_mode = Some(v); found_any = true; continue;
                }
            }
            // Animation name — ident (not a keyword above) or quoted string
            if name.is_none() {
                if let Ok(n) = input.try_parse(|i| -> Result<Atom, crate::Error> {
                    if let Ok(s) = i.try_parse(|i| Ok::<_, crate::Error>(i.expect_string()?.as_ref().to_string())) {
                        return Ok(Atom::new(&s));
                    }
                    let ident = i.expect_ident()?;
                    Ok(Atom::new(&*ident))
                }) {
                    name = Some(n); found_any = true; continue;
                }
            }
            break;
        }

        if !found_any {
            return Err(input.new_custom_error(crate::CustomError::InvalidValue));
        }

        names.push(name);
        durations.push(dur.unwrap_or(Duration::ZERO));
        timings.push(timing.unwrap_or(TimingFunction::Ease));
        delays.push(delay.unwrap_or(Duration::ZERO));
        iterations.push(iteration.unwrap_or(IterationCount::Finite(1.0)));
        directions.push(direction.unwrap_or(AnimationDirection::Normal));
        fill_modes.push(fill_mode.unwrap_or(AnimationFillMode::None));
        play_states.push(play_state.unwrap_or(AnimationPlayState::Running));

        if input.try_parse(|i| i.expect_comma()).is_err() { break; }
    }

    // Build AnimationNameList
    let name_list = if names.iter().all(|n| n.is_none()) {
        AnimationNameList::None
    } else {
        let atoms: Vec<Atom> = names.into_iter().map(|n| {
            n.unwrap_or_else(|| Atom::new("none"))
        }).collect();
        AnimationNameList::Names(atoms.into())
    };

    Ok(smallvec::smallvec![
        PD::AnimationName(Declared::Value(name_list)),
        PD::AnimationDuration(Declared::Value(DurationList(durations.into()))),
        PD::AnimationTimingFunction(Declared::Value(TimingFunctionList(timings.into()))),
        PD::AnimationDelay(Declared::Value(DurationList(delays.into()))),
        PD::AnimationIterationCount(Declared::Value(AnimationIterationCountList(iterations.into()))),
        PD::AnimationDirection(Declared::Value(AnimationDirectionList(directions.into()))),
        PD::AnimationFillMode(Declared::Value(AnimationFillModeList(fill_modes.into()))),
        PD::AnimationPlayState(Declared::Value(AnimationPlayStateList(play_states.into()))),
    ])
}

// Hand-written: background

/// `background` shorthand — single-layer: `[<image> || <position> [/ <size>]? ||
/// <repeat> || <attachment> || <origin> [<clip>]? || <color>]`.
///
/// Multi-layer backgrounds use comma-separated layers; only the final layer
/// may include a `<color>`. Our longhand types are single-value (not per-layer
/// lists), so this handles the common single-layer case.
fn parse_background<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type Img = kozan_style::Image;
    type ImgList = kozan_style::ImageList;
    type PosComp = kozan_style::PositionComponent;
    type BgSize = kozan_style::BackgroundSize;
    type BgRepeat = kozan_style::BackgroundRepeat;
    type BgAttach = kozan_style::BackgroundAttachment;
    type BgClip = kozan_style::BackgroundClip;
    type BgOrigin = kozan_style::BackgroundOrigin;

    let mut color = None;
    let mut image = None;
    let mut pos_x = None;
    let mut pos_y = None;
    let mut size = None;
    let mut repeat = None;
    let mut attachment = None;
    let mut clip = None;
    let mut origin = None;

    for _ in 0..9 {
        // Position — two-component, must come before color (keyword overlap)
        if pos_x.is_none() {
            if let Ok((x, y)) = input.try_parse(|i| {
                let x = PosComp::parse(i)?;
                let y = i.try_parse(PosComp::parse).unwrap_or(PosComp::Center);
                Ok::<_, crate::Error>((x, y))
            }) {
                pos_x = Some(x);
                pos_y = Some(y);
                if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                    size = Some(BgSize::parse(input)?);
                }
                continue;
            }
        }
        // Image (url(...) or gradient)
        if image.is_none() {
            if let Ok(v) = input.try_parse(Img::parse) { image = Some(v); continue; }
        }
        // Repeat
        if repeat.is_none() {
            if let Ok(v) = input.try_parse(BgRepeat::parse) { repeat = Some(v); continue; }
        }
        // Attachment
        if attachment.is_none() {
            if let Ok(v) = input.try_parse(BgAttach::parse) { attachment = Some(v); continue; }
        }
        // Origin (+ optional clip)
        if origin.is_none() {
            if let Ok(v) = input.try_parse(BgOrigin::parse) {
                origin = Some(v);
                clip = input.try_parse(BgClip::parse).ok();
                continue;
            }
        }
        // Color (only in final/single layer)
        if color.is_none() {
            if let Ok(v) = input.try_parse(CssColor::parse) { color = Some(v); continue; }
        }
        break;
    }

    if color.is_none() && image.is_none() && pos_x.is_none()
       && repeat.is_none() && attachment.is_none() && origin.is_none()
    {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::BackgroundColor(color.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundImage(image.map_or(Declared::Initial, |img| {
            Declared::Value(ImgList::Images(Box::from([img])))
        })),
        PD::BackgroundPositionX(pos_x.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundPositionY(pos_y.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundSize(size.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundRepeat(repeat.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundAttachment(attachment.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundClip(clip.map_or(Declared::Initial, Declared::Value)),
        PD::BackgroundOrigin(origin.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: marker (SVG)

/// `marker` shorthand — single value applied to start, mid, and end.
fn parse_marker<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type MarkerVal = kozan_style::NoneOr<kozan_style::Url>;

    let v = MarkerVal::parse(input)?;
    Ok(smallvec::smallvec![
        PD::MarkerStart(Declared::Value(v.clone())),
        PD::MarkerMid(Declared::Value(v.clone())),
        PD::MarkerEnd(Declared::Value(v)),
    ])
}

// Hand-written: border-image

/// `border-image` shorthand — `<source> || <slice> [/ <width> [/ <outset>]]? || <repeat>`.
fn parse_border_image<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type Img = kozan_style::Image;
    type Slice = kozan_style::BorderImageSlice;
    type EdgesLP = kozan_style::Edges<LP>;
    type BIRepeat = kozan_style::BorderImageRepeat;

    let mut source = None;
    let mut slice = None;
    let mut width = None;
    let mut outset = None;
    let mut repeat = None;

    for _ in 0..4 {
        // Try slice (numbers/percentages + optional fill)
        if slice.is_none() {
            if let Ok(v) = input.try_parse(Slice::parse) {
                slice = Some(v);
                // Optional / width [/ outset]
                if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                    width = input.try_parse(EdgesLP::parse).ok();
                    if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                        outset = input.try_parse(EdgesLP::parse).ok();
                    }
                }
                continue;
            }
        }
        // Try repeat
        if repeat.is_none() {
            if let Ok(v) = input.try_parse(BIRepeat::parse) { repeat = Some(v); continue; }
        }
        // Try source (url/gradient/none)
        if source.is_none() {
            if let Ok(v) = input.try_parse(Img::parse) { source = Some(v); continue; }
        }
        break;
    }

    if source.is_none() && slice.is_none() && repeat.is_none() {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::BorderImageSource(source.map_or(Declared::Initial, Declared::Value)),
        PD::BorderImageSlice(slice.map_or(Declared::Initial, Declared::Value)),
        PD::BorderImageWidth(width.map_or(Declared::Initial, Declared::Value)),
        PD::BorderImageOutset(outset.map_or(Declared::Initial, Declared::Value)),
        PD::BorderImageRepeat(repeat.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: mask

/// `mask` shorthand — `<image> || <mode> || <repeat> || <position> [/ <size>]? ||
/// <clip> || <origin> || <composite>`.
///
/// Single-layer only (our longhand types are single-value, not per-layer lists).
fn parse_mask<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type Img = kozan_style::Image;
    type ImgList = kozan_style::ImageList;
    type MMode = kozan_style::MaskMode;
    type MRepeat = kozan_style::BackgroundRepeat;
    type Pos = kozan_style::Position2D;
    type MClip = kozan_style::MaskClip;
    type MOrigin = kozan_style::BackgroundOrigin;
    type MSize = kozan_style::BackgroundSize;
    type MComp = kozan_style::MaskComposite;

    let mut image = None;
    let mut mode = None;
    let mut repeat = None;
    let mut position = None;
    let mut size = None;
    let mut clip = None;
    let mut origin = None;
    let mut composite = None;

    for _ in 0..8 {
        // Position (must come early — keyword overlap)
        if position.is_none() {
            if let Ok(v) = input.try_parse(Pos::parse) {
                position = Some(v);
                if input.try_parse(|i| i.expect_delim('/')).is_ok() {
                    size = input.try_parse(MSize::parse).ok();
                }
                continue;
            }
        }
        if image.is_none() {
            if let Ok(v) = input.try_parse(Img::parse) { image = Some(v); continue; }
        }
        if mode.is_none() {
            if let Ok(v) = input.try_parse(MMode::parse) { mode = Some(v); continue; }
        }
        if repeat.is_none() {
            if let Ok(v) = input.try_parse(MRepeat::parse) { repeat = Some(v); continue; }
        }
        if composite.is_none() {
            if let Ok(v) = input.try_parse(MComp::parse) { composite = Some(v); continue; }
        }
        if origin.is_none() {
            if let Ok(v) = input.try_parse(MOrigin::parse) {
                origin = Some(v);
                clip = input.try_parse(MClip::parse).ok();
                continue;
            }
        }
        break;
    }

    if image.is_none() && mode.is_none() && repeat.is_none()
       && position.is_none() && clip.is_none() && origin.is_none() && composite.is_none()
    {
        return Err(input.new_custom_error(crate::CustomError::InvalidValue));
    }

    Ok(smallvec::smallvec![
        PD::MaskImage(image.map_or(Declared::Initial, |img| {
            Declared::Value(ImgList::Images(Box::from([img])))
        })),
        PD::MaskMode(mode.map_or(Declared::Initial, Declared::Value)),
        PD::MaskRepeat(repeat.map_or(Declared::Initial, Declared::Value)),
        PD::MaskPosition(position.map_or(Declared::Initial, Declared::Value)),
        PD::MaskClip(clip.map_or(Declared::Initial, Declared::Value)),
        PD::MaskOrigin(origin.map_or(Declared::Initial, Declared::Value)),
        PD::MaskSize(size.map_or(Declared::Initial, Declared::Value)),
        PD::MaskComposite(composite.map_or(Declared::Initial, Declared::Value)),
    ])
}

// Hand-written: grid-row / grid-column / grid-area (need `/` separator)

/// `grid-row` / `grid-column` shorthand — `<line> [ / <line> ]`.
/// If only one value, both start and end get the same value.
fn parse_grid_pair<'i>(
    input: &mut Parser<'i, '_>,
    mk_start: fn(Declared<kozan_style::GridLine>) -> PD,
    mk_end: fn(Declared<kozan_style::GridLine>) -> PD,
) -> Result<Decls, Error<'i>> {
    type GL = kozan_style::GridLine;
    let start = GL::parse(input)?;
    let end = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        GL::parse(input)?
    } else {
        start.clone()
    };
    Ok(smallvec::smallvec![
        mk_start(Declared::Value(start)),
        mk_end(Declared::Value(end)),
    ])
}

/// `grid-area` shorthand — `<row-start> [ / <column-start> [ / <row-end> [ / <column-end> ]]]`.
/// Missing values default to the corresponding start value (or auto).
fn parse_grid_area<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type GL = kozan_style::GridLine;
    let row_start = GL::parse(input)?;
    let col_start = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        GL::parse(input)?
    } else {
        row_start.clone()
    };
    let row_end = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        GL::parse(input)?
    } else {
        row_start.clone()
    };
    let col_end = if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        GL::parse(input)?
    } else {
        col_start.clone()
    };
    Ok(smallvec::smallvec![
        PD::GridRowStart(Declared::Value(row_start)),
        PD::GridColumnStart(Declared::Value(col_start)),
        PD::GridRowEnd(Declared::Value(row_end)),
        PD::GridColumnEnd(Declared::Value(col_end)),
    ])
}

// Hand-written: grid-template / grid

/// `grid-template` shorthand — `none | [<rows> / <columns>]`.
///
/// Full syntax includes `<grid-template-areas>` with row track sizing inline,
/// but we handle the common `rows / columns` form and `none`.
fn parse_grid_template<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type TList = kozan_style::TrackList;
    type Areas = kozan_style::GridTemplateAreas;

    // grid-template: none → all initial
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::GridTemplateColumns(Declared::Value(TList::None)),
            PD::GridTemplateRows(Declared::Value(TList::None)),
            PD::GridTemplateAreas(Declared::Value(Areas::None)),
        ]);
    }

    // Try: <rows> / <columns>
    let rows = TList::parse(input)?;
    if input.try_parse(|i| i.expect_delim('/')).is_ok() {
        let cols = TList::parse(input)?;
        return Ok(smallvec::smallvec![
            PD::GridTemplateColumns(Declared::Value(cols)),
            PD::GridTemplateRows(Declared::Value(rows)),
            PD::GridTemplateAreas(Declared::Initial),
        ]);
    }

    // Single value — rows only (columns initial)
    Ok(smallvec::smallvec![
        PD::GridTemplateColumns(Declared::Initial),
        PD::GridTemplateRows(Declared::Value(rows)),
        PD::GridTemplateAreas(Declared::Initial),
    ])
}

/// `grid` shorthand — `none | <grid-template> | <rows> / auto-flow [dense]? <columns>
///                       | auto-flow [dense]? <rows> / <columns>`.
///
/// Handles `none` and the `<grid-template>` form (rows / columns).
/// The `auto-flow` syntax sets `grid-auto-flow` + auto-columns/rows.
fn parse_grid<'i>(input: &mut Parser<'i, '_>) -> Result<Decls, Error<'i>> {
    type TList = kozan_style::TrackList;
    type TSize = kozan_style::TrackSize;
    type Areas = kozan_style::GridTemplateAreas;
    type GAFlow = kozan_style::GridAutoFlow;

    // grid: none
    if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
        return Ok(smallvec::smallvec![
            PD::GridTemplateColumns(Declared::Value(TList::None)),
            PD::GridTemplateRows(Declared::Value(TList::None)),
            PD::GridTemplateAreas(Declared::Value(Areas::None)),
            PD::GridAutoColumns(Declared::Initial),
            PD::GridAutoRows(Declared::Initial),
            PD::GridAutoFlow(Declared::Initial),
        ]);
    }

    // Try auto-flow [dense]? <auto-size> / <columns>
    // → sets grid-auto-flow + grid-auto-rows, grid-template-columns
    if let Ok(()) = input.try_parse(|i| {
        i.expect_ident_matching("auto-flow").map(|_| ())
    }) {
        let dense = input.try_parse(|i| i.expect_ident_matching("dense")).is_ok();
        let auto_rows = input.try_parse(TSize::parse).ok();
        input.expect_delim('/')?;
        let cols = TList::parse(input)?;
        let flow = if dense { GAFlow::RowDense } else { GAFlow::Row };
        return Ok(smallvec::smallvec![
            PD::GridTemplateColumns(Declared::Value(cols)),
            PD::GridTemplateRows(Declared::Initial),
            PD::GridTemplateAreas(Declared::Initial),
            PD::GridAutoColumns(Declared::Initial),
            PD::GridAutoRows(auto_rows.map_or(Declared::Initial, Declared::Value)),
            PD::GridAutoFlow(Declared::Value(flow)),
        ]);
    }

    // Try <rows> / auto-flow [dense]? <auto-size>
    // → sets grid-template-rows, grid-auto-flow=column, grid-auto-columns
    let state = input.state();
    if let Ok(rows) = input.try_parse(TList::parse) {
        if input.try_parse(|i| i.expect_delim('/')).is_ok() {
            if input.try_parse(|i| i.expect_ident_matching("auto-flow")).is_ok() {
                let dense = input.try_parse(|i| i.expect_ident_matching("dense")).is_ok();
                let auto_cols = input.try_parse(TSize::parse).ok();
                let flow = if dense { GAFlow::ColumnDense } else { GAFlow::Column };
                return Ok(smallvec::smallvec![
                    PD::GridTemplateColumns(Declared::Initial),
                    PD::GridTemplateRows(Declared::Value(rows)),
                    PD::GridTemplateAreas(Declared::Initial),
                    PD::GridAutoColumns(auto_cols.map_or(Declared::Initial, Declared::Value)),
                    PD::GridAutoRows(Declared::Initial),
                    PD::GridAutoFlow(Declared::Value(flow)),
                ]);
            }
            // Not auto-flow — this is grid-template form: rows / columns
            let cols = TList::parse(input)?;
            return Ok(smallvec::smallvec![
                PD::GridTemplateColumns(Declared::Value(cols)),
                PD::GridTemplateRows(Declared::Value(rows)),
                PD::GridTemplateAreas(Declared::Initial),
                PD::GridAutoColumns(Declared::Initial),
                PD::GridAutoRows(Declared::Initial),
                PD::GridAutoFlow(Declared::Initial),
            ]);
        }
    }

    // If nothing matched, reset and fail
    input.reset(&state);
    Err(input.new_custom_error(crate::CustomError::InvalidValue))
}
