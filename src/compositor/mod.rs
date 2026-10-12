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
    BlurParams, BlurQuality, CanonicalFilter, ColorAdjustParams, DropShadowParams, Effect,
    EffectType, TransformParams,
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
    /// CSS `filter: <op>(<amt>)` — canonical op (see
    /// [`CanonicalFilter`]). One op per SyntheticFeature; CSS
    /// filter chains are emitted as multiple entries with
    /// identical selectors.
    CanonicalFilter,
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
        FeatureType::CanonicalFilter => {
            // Parse `filter: <hue-rotate|saturate|brightness|contrast|grayscale|invert>(<amt>)`.
            // Chain detection (`filter: brightness(1.2) contrast(0.8)`) is a
            // single-op-per-effect model: the downstream consumer emits
            // multiple SyntheticFeature entries with FeatureType::CanonicalFilter
            // and identical selectors, so each op commits/replays atomically.
            match parse_canonical_filter(&feature.original_value) {
                Some(filter) => Some(Effect::canonical_filter(
                    &feature.selector,
                    filter,
                    viewport_width,
                    viewport_height,
                )),
                None => None,
            }
        }
    }
}

/// Extract the argument string of the FIRST occurrence of `name(` … `)` in
/// `value` — e.g. `("transform: scale(1.5, 0.8)", "scale")` → `"1.5, 0.8"`.
///
/// Shared by the per-function parsers (blur/transform/color-adjust) so
/// extraction stays uniform: find the `name(` prefix, slice to the next `)`.
/// Chained functions (`scale(1.1) translate(...)`) are handled by calling
/// this once per name — each parser looks for its own function.
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

    // scale(x) → uniform; scale(x, y) → non-uniform (CSS form).
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

    // translate(x[, y]) — `y` defaults to 0 per CSS (not to x).
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

    // rotate(<deg>deg) — unit optional (matches CSS <zero> angle form).
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

    // brightness sets the absolute multiplier; contrast and saturate
    // compose multiplicatively (CSS filter chains apply left-to-right).
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
    // Default to full viewport if parsing fails
    Region::new(0.0, 0.0, viewport_width as f32, viewport_height as f32)
}

/// Parse one canonical CSS filter op into a `CanonicalFilter`.
///
/// Supports the six canonical CSS filter functions from the Filter Effects
/// spec (https://drafts.fxtf.org/filter-effects/): `hue-rotate`, `saturate`,
/// `brightness`, `contrast`, `grayscale`, `invert`. Returns `None` if the
/// input string does not match any of those function names OR if the
/// numeric argument fails to parse.
///
/// The result is a single op; CSS filter chains
/// (`filter: brightness(1.2) contrast(0.8)`) are decomposed by the caller
/// into multiple SyntheticFeature entries so each op becomes its own Effect.
fn parse_canonical_filter(value: &str) -> Option<CanonicalFilter> {
    // (function-name, ctor) — hue-rotate is listed first decoratively,
    // matching the CSS Filter Effects spec column ordering. No `rotate(`
    // function exists in this array (and no other function follows a
    // substring of hue-rotate), so the priority does not protect against
    // substring matches; it is purely cosmetic.
    const FILTERS: &[(&str, fn(f32) -> CanonicalFilter)] = &[
        ("hue-rotate(", |d| CanonicalFilter::HueRotate(d)),
        ("saturate(", |a| CanonicalFilter::Saturate(a)),
        ("brightness(", |a| CanonicalFilter::Brightness(a)),
        ("contrast(", |a| CanonicalFilter::Contrast(a)),
        ("grayscale(", |a| CanonicalFilter::Grayscale(a)),
        ("invert(", |a| CanonicalFilter::Invert(a)),
    ];
    for (name, ctor) in FILTERS {
        if let Some(start) = value.find(name) {
            let after = &value[start + name.len()..];
            if let Some(end) = after.find(')') {
                // Strip the trailing `deg` suffix (only meaningful for
                // hue-rotate; ignored for the others, which are unitless).
                let amt_str = after[..end].trim().trim_end_matches("deg").trim();
                if let Ok(amt) = amt_str.parse::<f32>() {
                    return Some(ctor(amt));
                }
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_transform (harvested from PR#1's hardened test set) -------

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

    // --- parse_blur_amount -------------------------------------------------

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

    // --- parse_color_adjust --------------------------------------------------

    #[test]
    fn parse_color_brightness_and_contrast_compose() {
        // CSS filter chains compose left-to-right: brightness(1.2) then
        // contrast(0.8) → multiplier 1.2 × 0.8.
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

    #[test]
    fn test_parse_canonical_filter_hue_rotate() {
        assert_eq!(
            parse_canonical_filter("hue-rotate(45deg)"),
            Some(CanonicalFilter::HueRotate(45.0))
        );
    }

    #[test]
    fn test_parse_canonical_filter_unit_ops() {
        assert_eq!(
            parse_canonical_filter("brightness(1.2)"),
            Some(CanonicalFilter::Brightness(1.2))
        );
        assert_eq!(
            parse_canonical_filter("saturate(0.5)"),
            Some(CanonicalFilter::Saturate(0.5))
        );
        assert_eq!(
            parse_canonical_filter("contrast(0.8)"),
            Some(CanonicalFilter::Contrast(0.8))
        );
        assert_eq!(
            parse_canonical_filter("grayscale(1.0)"),
            Some(CanonicalFilter::Grayscale(1.0))
        );
        assert_eq!(
            parse_canonical_filter("invert(0.5)"),
            Some(CanonicalFilter::Invert(0.5))
        );
    }

    #[test]
    fn test_parse_canonical_filter_negative_amounts() {
        // CSS spec allows negative hue-rotate degrees (rotate the other way)
        // and the parser does not bound brightness/contrast (the GPU compute
        // path clamps via the Filter Effects spec). The parser forwards the
        // parsed value as-is.
        assert_eq!(
            parse_canonical_filter("hue-rotate(-30deg)"),
            Some(CanonicalFilter::HueRotate(-30.0))
        );
        assert_eq!(
            parse_canonical_filter("brightness(-0.2)"),
            Some(CanonicalFilter::Brightness(-0.2))
        );
    }

    #[test]
    fn test_parse_canonical_filter_unsupported_returns_none() {
        // CSS filter functions not in the canonical set (sepia, opacity,
        // drop-shadow, blur) are not routed through CanonicalFilter.
        assert_eq!(parse_canonical_filter("sepia(1.0)"), None);
        assert_eq!(parse_canonical_filter("opacity(0.5)"), None);
        assert_eq!(parse_canonical_filter("blur(10px)"), None);
        assert_eq!(parse_canonical_filter("drop-shadow(0 4px red)"), None);
    }

    #[test]
    fn test_parse_canonical_filter_chain_returns_first_only() {
        // A chained `filter` string matches the FIRST known function
        // found by `value.find(...)`. The compositor bridge is expected
        // to decompose chains into multiple SyntheticFeature entries;
        // single-op-per-feature is the documented contract.
        let parsed = parse_canonical_filter("brightness(1.2) contrast(0.8)");
        assert_eq!(parsed, Some(CanonicalFilter::Brightness(1.2)));
    }

    #[test]
    fn test_features_to_effects_canonical_filter_route() {
        // End-to-end through the bridge: SyntheticFeature::CanonicalFilter
        // resolves to `Effect::canonical_filter(...)` with the parsed op.
        let feature = SyntheticFeature {
            feature_type: FeatureType::CanonicalFilter,
            selector: ".hero".to_string(),
            original_value: "saturate(0.7)".to_string(),
        };
        let effects = features_to_effects(&[feature], 1280, 800);
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0].effect_type, EffectType::CanonicalFilter));
        assert_eq!(
            effects[0].canonical_filter_params,
            Some(CanonicalFilter::Saturate(0.7))
        );
    }
}

/// Configuration for the compositor
#[derive(Debug, Clone)]
pub struct CompositorConfig {
    /// Maximum number of effects to apply per frame
    pub max_effects: usize,
    /// Enable GPU acceleration
    pub gpu_enabled: bool,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            max_effects: 64,
            gpu_enabled: true,
        }
    }
}

/// Result of a compositing operation
pub struct CompositeResult {
    /// The composited buffer
    pub buffer: Vec<u8>,
}

/// Buffer-based compositor for applying effects to raw pixel data
pub struct Compositor {
    #[allow(dead_code)]
    config: CompositorConfig,
}

impl Compositor {
    /// Create a new compositor with default configuration
    pub fn new() -> Self {
        Self {
            config: CompositorConfig::default(),
        }
    }

    /// Create a new compositor with custom configuration
    pub fn with_config(config: CompositorConfig) -> Self {
        Self { config }
    }

    /// Apply effects to a raw pixel buffer
    pub fn composite(
        &mut self,
        buffer: &[u8],
        _width: u32,
        _height: u32,
        _effects: &[Effect],
    ) -> anyhow::Result<CompositeResult> {
        // For now, return the buffer unchanged
        // Full implementation would apply effects to the pixel data
        Ok(CompositeResult {
            buffer: buffer.to_vec(),
        })
    }
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}
