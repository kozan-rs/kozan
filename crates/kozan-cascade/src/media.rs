//! Media query evaluation against a `Device`.
//!
//! Evaluates `@media` conditions at stylesheet index time to determine
//! which rules are active for the current device. Re-evaluated when the
//! device changes (viewport resize, color scheme toggle, etc.).
//!
//! Covers Media Queries Level 4 features and Level 5 user-preference features.

use kozan_atom::Atom;
use kozan_css::{
    LengthUnit, MediaCondition, MediaFeature, MediaFeatureValue,
    MediaQuery, MediaQueryList, MediaQualifier, MediaType as CssMediaType,
};

use crate::device::{
    ColorGamut, ColorScheme, Device, DynamicRange, ForcedColors,
    HoverCapability, MediaType, Pointer, Scripting, Update,
};

/// Evaluate a media query list. Returns `true` if any query matches.
///
/// An empty query list matches everything (per CSS spec).
#[must_use]
pub fn evaluate(queries: &MediaQueryList, device: &Device) -> bool {
    if queries.0.is_empty() {
        return true;
    }
    queries.0.iter().any(|q| evaluate_query(q, device))
}

fn evaluate_query(query: &MediaQuery, device: &Device) -> bool {
    let type_matches = match &query.media_type {
        CssMediaType::All => true,
        CssMediaType::Screen => device.media_type == MediaType::Screen,
        CssMediaType::Print => device.media_type == MediaType::Print,
        CssMediaType::Custom(_) => false,
    };

    let condition_matches = match &query.condition {
        Some(cond) => evaluate_condition(cond, device),
        None => true,
    };

    let result = type_matches && condition_matches;

    match query.qualifier {
        Some(MediaQualifier::Not) => !result,
        _ => result,
    }
}

fn evaluate_condition(cond: &MediaCondition, device: &Device) -> bool {
    match cond {
        MediaCondition::Feature(feature) => evaluate_feature(feature, device),
        MediaCondition::Not(inner) => !evaluate_condition(inner, device),
        MediaCondition::And(conditions) => conditions.iter().all(|c| evaluate_condition(c, device)),
        MediaCondition::Or(conditions) => conditions.iter().any(|c| evaluate_condition(c, device)),
    }
}

fn evaluate_feature(feature: &MediaFeature, device: &Device) -> bool {
    match feature {
        MediaFeature::Boolean(name) => evaluate_boolean(name, device),
        MediaFeature::Plain { name, value } => evaluate_plain(name, value, device),
        MediaFeature::Range { name, op, value } => evaluate_range(name, *op, value, device),
    }
}

/// Boolean media features: `(color)`, `(hover)`, etc.
fn evaluate_boolean(name: &Atom, device: &Device) -> bool {
    match name.as_ref() {
        "color" => device.color_bits > 0,
        "monochrome" => device.monochrome_bits > 0,
        "hover" => device.hover == HoverCapability::Hover,
        "any-hover" => device.any_hover == HoverCapability::Hover,
        "pointer" => device.pointer != Pointer::None,
        "any-pointer" => device.any_pointer != Pointer::None,
        "grid" => device.grid,
        "prefers-reduced-motion" => device.prefers_reduced_motion,
        "prefers-reduced-transparency" => device.prefers_reduced_transparency,
        "prefers-contrast" => device.prefers_contrast,
        "inverted-colors" => device.inverted_colors,
        _ => false,
    }
}

/// Plain media features: `(name: value)`.
fn evaluate_plain(name: &Atom, value: &MediaFeatureValue, device: &Device) -> bool {
    let s = name.as_ref();
    match s {
        "prefers-color-scheme" => match_ident(value, |v| match v {
            "light" => device.prefers_color_scheme == ColorScheme::Light,
            "dark" => device.prefers_color_scheme == ColorScheme::Dark,
            _ => false,
        }),
        "pointer" => match_ident(value, |v| match v {
            "fine" => device.pointer == Pointer::Fine,
            "coarse" => device.pointer == Pointer::Coarse,
            "none" => device.pointer == Pointer::None,
            _ => false,
        }),
        "any-pointer" => match_ident(value, |v| match v {
            "fine" => device.any_pointer == Pointer::Fine,
            "coarse" => device.any_pointer == Pointer::Coarse,
            "none" => device.any_pointer == Pointer::None,
            _ => false,
        }),
        "hover" => match_ident(value, |v| match v {
            "hover" => device.hover == HoverCapability::Hover,
            "none" => device.hover == HoverCapability::None,
            _ => false,
        }),
        "any-hover" => match_ident(value, |v| match v {
            "hover" => device.any_hover == HoverCapability::Hover,
            "none" => device.any_hover == HoverCapability::None,
            _ => false,
        }),
        "orientation" => match_ident(value, |v| match v {
            "portrait" => device.viewport_height >= device.viewport_width,
            "landscape" => device.viewport_width > device.viewport_height,
            _ => false,
        }),
        "prefers-reduced-motion" => match_ident(value, |v| match v {
            "reduce" => device.prefers_reduced_motion,
            "no-preference" => !device.prefers_reduced_motion,
            _ => false,
        }),
        "forced-colors" => match_ident(value, |v| match v {
            "none" => device.forced_colors == ForcedColors::None,
            "active" => device.forced_colors == ForcedColors::Active,
            _ => false,
        }),
        "color-gamut" => match_ident(value, |v| match v {
            "srgb" => device.color_gamut >= ColorGamut::Srgb,
            "p3" => device.color_gamut >= ColorGamut::P3,
            "rec2020" => device.color_gamut >= ColorGamut::Rec2020,
            _ => false,
        }),
        "dynamic-range" => match_ident(value, |v| match v {
            "standard" => true,
            "high" => device.dynamic_range == DynamicRange::High,
            _ => false,
        }),
        "update" => match_ident(value, |v| match v {
            "none" => device.update == Update::None,
            "slow" => device.update == Update::Slow,
            "fast" => device.update == Update::Fast,
            _ => false,
        }),
        "scripting" => match_ident(value, |v| match v {
            "none" => device.scripting == Scripting::None,
            "initial-only" => device.scripting == Scripting::InitialOnly,
            "enabled" => device.scripting == Scripting::Enabled,
            _ => false,
        }),
        // Numeric plain features: treat as equality
        "width" | "height" | "device-width" | "device-height" | "aspect-ratio" => {
            let device_val = device_length_value(s, device);
            resolve_length_eq(value, device_val, device)
        }
        "color" => matches!(value, MediaFeatureValue::Integer(n) if i32::from(device.color_bits) == *n),
        "monochrome" => matches!(value, MediaFeatureValue::Integer(n) if i32::from(device.monochrome_bits) == *n),
        "resolution" => matches!(value, MediaFeatureValue::Number(n) if (device.resolution_dpi - *n).abs() < 0.5),
        // Legacy colon syntax with min-/max- prefix:
        // `(min-width: 768px)` is parsed as Plain, delegate to range evaluator.
        _ if s.starts_with("min-") || s.starts_with("max-") => {
            evaluate_range(name, kozan_css::RangeOp::Eq, value, device)
        }
        _ => false,
    }
}

/// Range media features: `min-width`, `max-width`, `width >= 768px`, etc.
fn evaluate_range(
    name: &Atom,
    op: kozan_css::RangeOp,
    value: &MediaFeatureValue,
    device: &Device,
) -> bool {
    let s = name.as_ref();

    // Handle min-/max- prefix form
    let (base_name, effective_op) = if let Some(rest) = s.strip_prefix("min-") {
        (rest, kozan_css::RangeOp::Ge)
    } else if let Some(rest) = s.strip_prefix("max-") {
        (rest, kozan_css::RangeOp::Le)
    } else {
        (s, op)
    };

    let device_val = device_length_value(base_name, device);
    let query_val = resolve_length(value, device);

    match effective_op {
        kozan_css::RangeOp::Eq => (device_val - query_val).abs() < 0.5,
        kozan_css::RangeOp::Lt => device_val < query_val,
        kozan_css::RangeOp::Le => device_val <= query_val,
        kozan_css::RangeOp::Gt => device_val > query_val,
        kozan_css::RangeOp::Ge => device_val >= query_val,
    }
}

/// Get the device's value for a dimension feature.
fn device_length_value(name: &str, device: &Device) -> f32 {
    match name {
        "width" | "device-width" => device.viewport_width,
        "height" | "device-height" => device.viewport_height,
        "aspect-ratio" => device.aspect_ratio(),
        "resolution" => device.resolution_dpi,
        "color" => f32::from(device.color_bits),
        "monochrome" => f32::from(device.monochrome_bits),
        _ => 0.0,
    }
}

/// Resolve a media feature length value to px.
fn resolve_length(value: &MediaFeatureValue, device: &Device) -> f32 {
    match value {
        MediaFeatureValue::Length(n, unit) => resolve_unit(*n, *unit, device),
        MediaFeatureValue::Number(n) => *n,
        MediaFeatureValue::Integer(n) => *n as f32,
        _ => 0.0,
    }
}

/// Check equality for a plain length feature.
fn resolve_length_eq(value: &MediaFeatureValue, device_val: f32, device: &Device) -> bool {
    let query_val = resolve_length(value, device);
    (device_val - query_val).abs() < 0.5
}

/// Convert a length with unit to px.
///
/// W3C Media Queries Level 4 §4.6: relative units in media queries use
/// the initial value of the corresponding property (font-size = 16px).
fn resolve_unit(value: f32, unit: LengthUnit, device: &Device) -> f32 {
    let fs = device.default_font_size();
    let vw = device.viewport_width;
    let vh = device.viewport_height;
    match unit {
        // Absolute
        LengthUnit::Px => value,
        LengthUnit::Cm => value * 96.0 / 2.54,
        LengthUnit::Mm => value * 96.0 / 25.4,
        LengthUnit::In => value * 96.0,
        LengthUnit::Pt => value * 96.0 / 72.0,
        LengthUnit::Pc => value * 96.0 / 6.0,
        // Font-relative (use initial font-size per MQ spec §4.6)
        LengthUnit::Em | LengthUnit::Rem | LengthUnit::Ch | LengthUnit::Ex => value * fs,
        // Viewport (default = large for media queries)
        LengthUnit::Vw | LengthUnit::Svw | LengthUnit::Lvw | LengthUnit::Dvw => value * vw / 100.0,
        LengthUnit::Vh | LengthUnit::Svh | LengthUnit::Lvh | LengthUnit::Dvh => value * vh / 100.0,
        LengthUnit::Vmin => value * vw.min(vh) / 100.0,
        LengthUnit::Vmax => value * vw.max(vh) / 100.0,
        LengthUnit::Vi => value * vw / 100.0, // horizontal-tb default
        LengthUnit::Vb => value * vh / 100.0,
        // Container query units in @media context — use viewport as fallback
        LengthUnit::Cqw | LengthUnit::Cqi => value * vw / 100.0,
        LengthUnit::Cqh | LengthUnit::Cqb => value * vh / 100.0,
    }
}

/// Helper: extract an ident value and apply a matcher function.
fn match_ident(value: &MediaFeatureValue, f: impl FnOnce(&str) -> bool) -> bool {
    match value {
        MediaFeatureValue::Ident(ident) => f(ident.as_ref()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kozan_css::{MediaQuery, MediaQueryList, MediaCondition, MediaFeature, MediaFeatureValue, LengthUnit, RangeOp};

    fn screen_device(width: f32, height: f32) -> Device {
        Device::new(width, height)
    }

    fn make_query_list(queries: Vec<MediaQuery>) -> MediaQueryList {
        MediaQueryList(queries.into())
    }

    fn range_query(name: &str, op: RangeOp, px: f32) -> MediaQuery {
        MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from(name),
                op,
                value: MediaFeatureValue::Length(px, LengthUnit::Px),
            })),
        }
    }

    fn plain_ident_query(name: &str, value: &str) -> MediaQuery {
        MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(MediaCondition::Feature(MediaFeature::Plain {
                name: Atom::from(name),
                value: MediaFeatureValue::Ident(Atom::from(value)),
            })),
        }
    }

    fn boolean_query(name: &str) -> MediaQuery {
        MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(MediaCondition::Feature(MediaFeature::Boolean(
                Atom::from(name),
            ))),
        }
    }

    #[test]
    fn empty_query_matches_all() {
        let device = screen_device(1024.0, 768.0);
        assert!(evaluate(&MediaQueryList::empty(), &device));
    }

    #[test]
    fn min_width_match() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![range_query("min-width", RangeOp::Ge, 768.0)]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn min_width_no_match() {
        let device = screen_device(600.0, 400.0);
        let queries = make_query_list(vec![range_query("min-width", RangeOp::Ge, 768.0)]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn max_width() {
        let device = screen_device(600.0, 400.0);
        let queries = make_query_list(vec![range_query("max-width", RangeOp::Le, 768.0)]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn width_range_syntax() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![range_query("width", RangeOp::Ge, 768.0)]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn width_range_lt() {
        let device = screen_device(600.0, 400.0);
        let queries = make_query_list(vec![range_query("width", RangeOp::Lt, 768.0)]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn prefers_color_scheme() {
        let mut device = screen_device(1024.0, 768.0);
        device.prefers_color_scheme = ColorScheme::Dark;
        let queries = make_query_list(vec![plain_ident_query("prefers-color-scheme", "dark")]);
        assert!(evaluate(&queries, &device));

        let queries = make_query_list(vec![plain_ident_query("prefers-color-scheme", "light")]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn not_qualifier() {
        let device = screen_device(1024.0, 768.0);
        let query = MediaQuery {
            qualifier: Some(MediaQualifier::Not),
            media_type: CssMediaType::Print,
            condition: None,
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn screen_type_match() {
        let device = screen_device(1024.0, 768.0);
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::Screen,
            condition: None,
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn print_type_no_match() {
        let device = screen_device(1024.0, 768.0);
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::Print,
            condition: None,
        };
        assert!(!evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn and_condition() {
        let device = screen_device(1024.0, 768.0);
        let cond = MediaCondition::And(smallvec::smallvec![
            Box::new(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from("min-width"),
                op: RangeOp::Ge,
                value: MediaFeatureValue::Length(768.0, LengthUnit::Px),
            })),
            Box::new(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from("max-width"),
                op: RangeOp::Le,
                value: MediaFeatureValue::Length(1200.0, LengthUnit::Px),
            })),
        ]);
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(cond),
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn or_condition() {
        let device = screen_device(600.0, 400.0);
        let cond = MediaCondition::Or(smallvec::smallvec![
            Box::new(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from("min-width"),
                op: RangeOp::Ge,
                value: MediaFeatureValue::Length(1200.0, LengthUnit::Px),
            })),
            Box::new(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from("max-width"),
                op: RangeOp::Le,
                value: MediaFeatureValue::Length(768.0, LengthUnit::Px),
            })),
        ]);
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(cond),
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn not_condition() {
        let device = screen_device(1024.0, 768.0);
        let cond = MediaCondition::Not(Box::new(MediaCondition::Feature(
            MediaFeature::Range {
                name: Atom::from("max-width"),
                op: RangeOp::Le,
                value: MediaFeatureValue::Length(500.0, LengthUnit::Px),
            },
        )));
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(cond),
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn boolean_color_feature() {
        let device = screen_device(1024.0, 768.0);
        assert!(evaluate(&make_query_list(vec![boolean_query("color")]), &device));
    }

    #[test]
    fn boolean_monochrome_false() {
        let device = screen_device(1024.0, 768.0);
        assert!(!evaluate(&make_query_list(vec![boolean_query("monochrome")]), &device));
    }

    #[test]
    fn boolean_grid_false() {
        let device = screen_device(1024.0, 768.0);
        assert!(!evaluate(&make_query_list(vec![boolean_query("grid")]), &device));
    }

    #[test]
    fn hover_feature() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("hover", "hover")]);
        assert!(evaluate(&queries, &device));

        let queries = make_query_list(vec![plain_ident_query("hover", "none")]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn pointer_feature() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("pointer", "fine")]);
        assert!(evaluate(&queries, &device));

        let queries = make_query_list(vec![plain_ident_query("pointer", "coarse")]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn any_pointer_feature() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("any-pointer", "fine")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn any_hover_feature() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("any-hover", "hover")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn orientation_portrait() {
        let device = screen_device(768.0, 1024.0);
        let queries = make_query_list(vec![plain_ident_query("orientation", "portrait")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn orientation_landscape() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("orientation", "landscape")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn forced_colors() {
        let mut device = screen_device(1024.0, 768.0);
        device.forced_colors = ForcedColors::Active;
        let queries = make_query_list(vec![plain_ident_query("forced-colors", "active")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn color_gamut() {
        let mut device = screen_device(1024.0, 768.0);
        device.color_gamut = ColorGamut::P3;
        // P3 includes srgb
        let queries = make_query_list(vec![plain_ident_query("color-gamut", "srgb")]);
        assert!(evaluate(&queries, &device));
        let queries = make_query_list(vec![plain_ident_query("color-gamut", "p3")]);
        assert!(evaluate(&queries, &device));
        // But not rec2020
        let queries = make_query_list(vec![plain_ident_query("color-gamut", "rec2020")]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn prefers_reduced_motion() {
        let mut device = screen_device(1024.0, 768.0);
        device.prefers_reduced_motion = true;
        let queries = make_query_list(vec![plain_ident_query("prefers-reduced-motion", "reduce")]);
        assert!(evaluate(&queries, &device));
        let queries = make_query_list(vec![plain_ident_query("prefers-reduced-motion", "no-preference")]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn scripting_feature() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("scripting", "enabled")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn update_feature() {
        let device = screen_device(1024.0, 768.0);
        let queries = make_query_list(vec![plain_ident_query("update", "fast")]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn em_unit_resolution() {
        let device = screen_device(1024.0, 768.0);
        // 48em = 48 * 16px = 768px, min-width: 768px on 1024px device
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from("min-width"),
                op: RangeOp::Ge,
                value: MediaFeatureValue::Length(48.0, LengthUnit::Em),
            })),
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn vw_unit_resolution() {
        let device = screen_device(1000.0, 800.0);
        // 50vw = 500px, min-width: 500px on 1000px device
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::All,
            condition: Some(MediaCondition::Feature(MediaFeature::Range {
                name: Atom::from("width"),
                op: RangeOp::Ge,
                value: MediaFeatureValue::Length(50.0, LengthUnit::Vw),
            })),
        };
        assert!(evaluate(&make_query_list(vec![query]), &device));
    }

    #[test]
    fn zero_height_aspect_ratio() {
        let device = screen_device(1024.0, 0.0);
        // Should not panic with zero height
        let queries = make_query_list(vec![range_query("aspect-ratio", RangeOp::Gt, 0.5)]);
        assert!(!evaluate(&queries, &device));
    }

    #[test]
    fn multiple_queries_or_semantics() {
        let device = screen_device(600.0, 400.0);
        // First query doesn't match (min-width: 1200px), second does (max-width: 768px)
        let queries = make_query_list(vec![
            range_query("min-width", RangeOp::Ge, 1200.0),
            range_query("max-width", RangeOp::Le, 768.0),
        ]);
        assert!(evaluate(&queries, &device));
    }

    #[test]
    fn custom_media_type_no_match() {
        let device = screen_device(1024.0, 768.0);
        let query = MediaQuery {
            qualifier: None,
            media_type: CssMediaType::Custom(Atom::from("tv")),
            condition: None,
        };
        assert!(!evaluate(&make_query_list(vec![query]), &device));
    }
}
