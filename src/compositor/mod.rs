//! Compositor module for Mustang
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! Provides post-processing effects for CSS features that cannot be rendered natively.
//! This includes backdrop-filter blur, transforms, color adjustments, and security clipping.

pub mod element_tracker;
pub mod region;

// Re-export main types from effect (which re-exports from mustang)
pub use crate::effect::{
    BlurParams, BlurQuality, ColorAdjustParams, Effect, EffectType, TransformParams,
};
// Re-export Region from local region module
pub use region::Region;
// Re-export element tracker types
pub use element_tracker::{SharedElementTracker, TrackedElement};

/// Convert CSS features to compositor effects
pub fn features_to_effects(
    features: &[SyntheticFeature],
    viewport_width: u32,
    viewport_height: u32,
) -> Vec<Effect> {
    let mut effects = Vec::new();

    for feature in features {
        if let Some(effect) = effect_from_feature(feature, viewport_width, viewport_height) {
            effects.push(effect);
        }
    }

    effects
}

/// Synthetic feature from CSS normalization
#[derive(Debug, Clone)]
pub struct SyntheticFeature {
    pub feature_type: FeatureType,
    pub selector: String,
    pub original_value: String,
}

/// Types of synthetic CSS features
#[derive(Debug, Clone, PartialEq)]
pub enum FeatureType {
    BackdropFilter,
    Transform,
    ColorAdjust,
    Clip,
}

fn effect_from_feature(
    feature: &SyntheticFeature,
    viewport_width: u32,
    viewport_height: u32,
) -> Option<Effect> {
    match feature.feature_type {
        FeatureType::BackdropFilter => {
            // Parse backdrop-filter: blur(10px)
            let blur_amount = parse_blur_amount(&feature.original_value);
            Some(Effect::blur(
                &feature.selector,
                blur_amount,
                viewport_width,
                viewport_height,
            ))
        }
        FeatureType::Transform => {
            // Parse transform: scale(1.1), translate(10px, 20px), etc.
            let transform = parse_transform(&feature.original_value);
            Some(Effect::transform(
                &feature.selector,
                transform,
                viewport_width,
                viewport_height,
            ))
        }
        FeatureType::ColorAdjust => {
            // Parse color-adjust: brightness(1.2) etc.
            let color_params = parse_color_adjust(&feature.original_value);
            Some(Effect::color_adjust(&feature.selector, color_params))
        }
        FeatureType::Clip => {
            // Parse clip-path or security clipping
            let region =
                parse_clip_region(&feature.original_value, viewport_width, viewport_height);
            Some(Effect::clip(region))
        }
    }
}

fn extract_first_fn_args<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{}(", name);
    let start = value.find(&prefix)?;
    let after = &value[start + prefix.len()..];
    let end = after.find(')')?;
    Some(&after[..end])
}

fn parse_blur_amount(value: &str) -> f32 {
    if let Some(args) = extract_first_fn_args(value, "blur") {
        return args
            .trim()
            .trim_end_matches("px")
            .parse::<f32>()
            .unwrap_or(10.0);
    }
    10.0
}

fn parse_transform(value: &str) -> TransformParams {
    let mut params = TransformParams::default();

    if let Some(args) = extract_first_fn_args(value, "scale") {
        let parts: Vec<&str> = args.split(',').collect();
        if let Some(x) = parts.first().and_then(|s| s.trim().parse::<f32>().ok()) {
            params.scale_x = x;
            if parts.len() == 1 {
                params.scale_y = x;
            }
        }
        if let Some(y) = parts.get(1).and_then(|s| s.trim().parse::<f32>().ok()) {
            params.scale_y = y;
        }
    }

    if let Some(args) = extract_first_fn_args(value, "translate") {
        let parts: Vec<&str> = args.split(',').collect();
        if let Some(x) = parts
            .first()
            .and_then(|s| s.trim().trim_end_matches("px").parse::<f32>().ok())
        {
            params.translate_x = x;
        }
        if let Some(y) = parts
            .get(1)
            .and_then(|s| s.trim().trim_end_matches("px").parse::<f32>().ok())
        {
            params.translate_y = y;
        }
    }

    if let Some(args) = extract_first_fn_args(value, "rotate") {
        let deg = args
            .trim()
            .trim_end_matches("deg")
            .parse::<f32>()
            .unwrap_or(0.0);
        params.rotate_degrees = deg;
    }

    params
}

fn parse_color_adjust(value: &str) -> ColorAdjustParams {
    let mut params = ColorAdjustParams::default();

    if let Some(args) = extract_first_fn_args(value, "brightness") {
        if let Ok(b) = args.trim().parse::<f32>() {
            params.red_multiplier = b;
            params.green_multiplier = b;
            params.blue_multiplier = b;
        }
    }

    if let Some(args) = extract_first_fn_args(value, "contrast") {
        if let Ok(c) = args.trim().parse::<f32>() {
            params.red_multiplier *= c;
            params.green_multiplier *= c;
            params.blue_multiplier *= c;
        }
    }

    if let Some(args) = extract_first_fn_args(value, "saturate") {
        if let Ok(s) = args.trim().parse::<f32>() {
            params.red_multiplier *= s;
            params.green_multiplier *= s;
            params.blue_multiplier *= s;
        }
    }

    params
}

fn parse_clip_region(_value: &str, viewport_width: u32, viewport_height: u32) -> Region {
    Region::new(0.0, 0.0, viewport_width as f32, viewport_height as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transform_uniform_scale() {
        let p = parse_transform("transform: scale(1.5)");
        assert_eq!(p.scale_x, 1.5);
        assert_eq!(p.scale_y, 1.5);
    }

    #[test]
    fn parse_transform_non_uniform_scale() {
        let p = parse_transform("transform: scale(1.5, 0.8)");
        assert_eq!(p.scale_x, 1.5);
        assert_eq!(p.scale_y, 0.8);
    }

    #[test]
    fn parse_transform_multi_function() {
        let p = parse_transform("transform: scale(1.1) translate(10px, 20px) rotate(45deg)");
        assert_eq!(p.scale_x, 1.1);
        assert_eq!(p.scale_y, 1.1);
        assert_eq!(p.translate_x, 10.0);
        assert_eq!(p.translate_y, 20.0);
        assert_eq!(p.rotate_degrees, 45.0);
    }

    #[test]
    fn parse_transform_translate_x_only() {
        let p = parse_transform("transform: translate(5px)");
        assert_eq!(p.translate_x, 5.0);
        assert_eq!(p.translate_y, 0.0);
    }

    #[test]
    fn parse_transform_rotate_no_unit() {
        let p = parse_transform("transform: rotate(30)");
        assert_eq!(p.rotate_degrees, 30.0);
    }

    #[test]
    fn parse_blur_with_px() {
        assert_eq!(parse_blur_amount("backdrop-filter: blur(15px)"), 15.0);
    }

    #[test]
    fn parse_blur_without_px() {
        assert_eq!(parse_blur_amount("backdrop-filter: blur(15)"), 15.0);
    }

    #[test]
    fn parse_blur_invalid_defaults_to_10() {
        assert_eq!(parse_blur_amount("backdrop-filter: blur(abc)"), 10.0);
    }

    #[test]
    fn parse_blur_missing_defaults_to_10() {
        assert_eq!(parse_blur_amount("filter: brightness(1.2)"), 10.0);
    }

    #[test]
    fn parse_color_brightness_only() {
        let p = parse_color_adjust("filter: brightness(1.2)");
        assert_eq!(p.red_multiplier, 1.2);
        assert_eq!(p.green_multiplier, 1.2);
        assert_eq!(p.blue_multiplier, 1.2);
    }

    #[test]
    fn parse_color_brightness_and_contrast() {
        let p = parse_color_adjust("filter: brightness(1.2) contrast(0.8)");
        let expected = 1.2 * 0.8;
        assert!((p.red_multiplier - expected).abs() < 1e-5);
        assert!((p.green_multiplier - expected).abs() < 1e-5);
        assert!((p.blue_multiplier - expected).abs() < 1e-5);
    }

    #[test]
    fn parse_color_with_saturate() {
        let p = parse_color_adjust("filter: brightness(1.0) saturate(1.5)");
        assert_eq!(p.red_multiplier, 1.5);
        assert_eq!(p.green_multiplier, 1.5);
        assert_eq!(p.blue_multiplier, 1.5);
    }
}
