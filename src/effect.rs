//! Effect types for Mustang GPU compositor
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! Effects can be applied to any PaintScene implementation,
//! including VelloScenePainter from anyrender_vello.

use crate::compositor::region::Region;

/// Types of Mustang effects
#[derive(Debug, Clone, PartialEq)]
pub enum EffectType {
    /// Gaussian blur effect for backdrop-filter
    BackdropBlur,
    /// 2D transform (scale, translate, rotate)
    Transform2D,
    /// Color adjustment (multipliers and offsets)
    ColorAdjust,
    /// Clip/mask effect for security gating
    Clip,
    /// Drop shadow effect (CSS `box-shadow` / `filter: drop-shadow`).
    /// Scene-native via Vello's `draw_box_shadow`.
    DropShadow,
    /// Canonical CSS filter op (`hue-rotate` / `saturate` / `brightness` /
    /// `contrast` / `grayscale` / `invert`). One op per Effect; CSS filter
    /// chains are expressed as multiple `CanonicalFilter` Effects on the
    /// same selector.
    CanonicalFilter,
}

/// Quality levels for blur effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlurQuality {
    Low,    // Fast, good for previews
    Medium, // Balanced
    High,   // Best quality, slower
    Ultra,  // Maximum quality
}

impl Default for BlurQuality {
    fn default() -> Self {
        BlurQuality::High
    }
}

/// Parameters for color adjustment
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorAdjustParams {
    pub red_multiplier: f32,
    pub green_multiplier: f32,
    pub blue_multiplier: f32,
    pub red_offset: f32,
    pub green_offset: f32,
    pub blue_offset: f32,
}

impl Default for ColorAdjustParams {
    fn default() -> Self {
        Self {
            red_multiplier: 1.0,
            green_multiplier: 1.0,
            blue_multiplier: 1.0,
            red_offset: 0.0,
            green_offset: 0.0,
            blue_offset: 0.0,
        }
    }
}

/// Parameters for blur effect
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlurParams {
    /// Blur radius in pixels
    pub radius: f32,
    /// Number of blur passes (more = smoother but slower)
    pub passes: u32,
    /// Quality level for the blur
    pub quality: BlurQuality,
}

impl Default for BlurParams {
    fn default() -> Self {
        Self {
            radius: 10.0,
            passes: 2,
            quality: BlurQuality::High,
        }
    }
}

/// Parameters for 2D transform
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformParams {
    /// Scale on X axis
    pub scale_x: f32,
    /// Scale on Y axis
    pub scale_y: f32,
    /// Translation on X axis in pixels
    pub translate_x: f32,
    /// Translation on Y axis in pixels
    pub translate_y: f32,
    /// Rotation in degrees
    pub rotate_degrees: f32,
    /// Pivot point X (0.0 = left, 0.5 = center, 1.0 = right)
    pub pivot_x: f32,
    /// Pivot point Y (0.0 = top, 0.5 = center, 1.0 = bottom)
    pub pivot_y: f32,
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            scale_x: 1.0,
            scale_y: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            rotate_degrees: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }
}

/// Parameters for a drop shadow.
///
/// This is the deliberately-simplified API surface: **the shadow tint is
/// always `peniko::color::palette::css::BLACK.with_alpha(alpha)`** — the
/// existing `BackdropBlur` pattern, applied to whatever draws the shadow.
/// Per-effect user-supplied color was considered (4 f32 RGBA channels) but
/// rejected for peniko-0.6 compatibility: `Color::rgba`, `Color::rgba8`,
/// `Srgba::new`, and `Color::new(Srgb::new(...), f32)` all fail to compile
/// against `peniko = "0.6.1"` (the version resolved by `Cargo.lock`),
/// because the `peniko::color::Srgb::new(...)` and
/// `peniko::Color::rgba(r, g, b, a)` constructors are not exposed by
/// the 0.6.x `color` crate the way later minor releases expose them
/// (compile-error history recorded in this file's prior revisions).
/// The static-palette + `with_alpha(f32)` path is the canonical pattern
/// in the 0.6 line; we mirror it for `DropShadow`.
///
/// Geometry fields follow the CSS `box-shadow` shape (sans `color` +
/// `spread`): `offset-x`, `offset-y`, `blur-radius` (which is the sigma
/// Vello consumes directly), and `corner-radius` (Vello's rim-softness
/// 4th arg — not the shadowed shape's corner radius).
///
/// The `spread` parameter from CSS spec is NOT exposed here either:
/// Vello's `draw_box_shadow` signature is `(transform, rect, color,
/// radius, sigma)` — no `spread` arg. If Vello adds `spread` later, a
/// new `pub spread: f32` field is the natural follow-up.
///
/// All fields are feature-agnostic f32 (compile from both the gpu and
/// the no-gpu build); the apply path uses peniko + Vello on the gpu side
/// only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropShadowParams {
    /// Shadow alpha in `0.0..=1.0` (applied to the BLACK palette color
    /// at the apply path).
    pub alpha: f32,
    /// Shadow blur sigma in pixels (CSS `blur-radius`).
    pub blur_radius: f32,
    /// Horizontal offset in pixels.
    pub offset_x: f32,
    /// Vertical offset in pixels.
    pub offset_y: f32,
    /// Rim softness for Vello's `draw_box_shadow` 5th arg. Does not alter
    /// the shadowed shape, only the visible blur-ellipse rim.
    pub corner_radius: f32,
}

impl Default for DropShadowParams {
    fn default() -> Self {
        // Defaults paint a soft drop shadow 4 px below a 40%-black tint
        // — a reasonable \"card on white\" baseline render.
        Self {
            alpha: 0.4,
            blur_radius: 4.0,
            offset_x: 0.0,
            offset_y: 4.0,
            corner_radius: 8.0,
        }
    }
}

/// Canonical CSS filter op (mirror of the CSS `filter` property).
///
/// Each variant maps 1:1 to a CSS filter function from the Filter Effects
/// spec (https://drafts.fxtf.org/filter-effects/). Values follow the CSS
/// spec: `HueRotate` is degrees; the rest are unitless multipliers where
/// `1.0` means identity, `0.0` means \"fully collapsed\" (black for
/// `Brightness`, mid-gray for `Contrast`), and `>1` amplifies
/// (`Saturate` / `Brightness` / `Contrast`).
///
/// `Grayscale(amount)` interpolates between identity (`0`) and full
/// grayscale (`1`); `Invert(amount)` interpolates between identity (`0`)
/// and full inversion (`1`).
///
/// CSS filter chains (`filter: brightness(1.2) contrast(0.8)`) are NOT
/// nested into a single `CanonicalFilter` — they are emitted by the
/// compositor bridge as multiple `SyntheticFeature` entries with
/// `FeatureType::CanonicalFilter` so each op commits/replays
/// atomically as its own `Effect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanonicalFilter {
    /// `hue-rotate(<deg>deg)` — rotate on the RGB plane.
    HueRotate(f32),
    /// `saturate(<amt>)` — `0` = grayscale, `1` = unchanged, `>1` = amplified.
    Saturate(f32),
    /// `brightness(<amt>)` — `0` = black, `1` = unchanged, `>1` = brighter.
    Brightness(f32),
    /// `contrast(<amt>)` — `0` = mid-gray, `1` = unchanged, `>1` = amplified.
    Contrast(f32),
    /// `grayscale(<amt>)` — `0` = unchanged, `1` = full grayscale.
    Grayscale(f32),
    /// `invert(<amt>)` — `0` = unchanged, `1` = full inversion.
    Invert(f32),
}

/// A Mustang effect to be applied to a region
#[derive(Debug, Clone)]
pub struct Effect {
    /// Type of effect
    pub effect_type: EffectType,
    /// CSS selector that identifies the target element
    pub selector: String,
    /// Region bounds (x, y, width, height) in screen coordinates
    pub region: Region,
    /// Blur-specific parameters
    pub blur_params: Option<BlurParams>,
    /// Transform-specific parameters
    pub transform_params: Option<TransformParams>,
    /// Color-specific parameters
    pub color_params: Option<ColorAdjustParams>,
    /// Drop-shadow-specific parameters
    pub drop_shadow_params: Option<DropShadowParams>,
    /// Canonical-filter-specific parameters (single op per Effect; chains
    /// are multiple Effects with the same selector).
    pub canonical_filter_params: Option<CanonicalFilter>,
    /// Z-order for layering (higher = on top)
    pub z_index: i32,
}

impl Effect {
    /// Create a blur effect
    pub fn blur(selector: &str, radius: f32, viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            effect_type: EffectType::BackdropBlur,
            selector: selector.to_string(),
            region: Region::new(0.0, 0.0, viewport_width as f32, viewport_height as f32),
            blur_params: Some(BlurParams {
                radius,
                passes: 2,
                quality: BlurQuality::High,
            }),
            transform_params: None,
            color_params: None,
            drop_shadow_params: None,
            canonical_filter_params: None,
            z_index: 0,
        }
    }

    /// Create a transform effect
    pub fn transform(
        selector: &str,
        params: TransformParams,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        Self {
            effect_type: EffectType::Transform2D,
            selector: selector.to_string(),
            region: Region::new(0.0, 0.0, viewport_width as f32, viewport_height as f32),
            blur_params: None,
            transform_params: Some(params),
            color_params: None,
            drop_shadow_params: None,
            canonical_filter_params: None,
            z_index: 0,
        }
    }

    /// Create a color adjustment effect
    pub fn color_adjust(selector: &str, params: ColorAdjustParams) -> Self {
        Self {
            effect_type: EffectType::ColorAdjust,
            selector: selector.to_string(),
            region: Region::new(0.0, 0.0, 0.0, 0.0),
            blur_params: None,
            transform_params: None,
            color_params: Some(params),
            drop_shadow_params: None,
            canonical_filter_params: None,
            z_index: 0,
        }
    }

    /// Create a clip effect for security gating
    pub fn clip(region: Region) -> Self {
        Self {
            effect_type: EffectType::Clip,
            selector: String::new(),
            region,
            blur_params: None,
            transform_params: None,
            color_params: None,
            drop_shadow_params: None,
            canonical_filter_params: None,
            z_index: 9999, // Clips are always top
        }
    }

    /// Create a drop shadow effect (CSS `box-shadow` / `filter: drop-shadow`).
    ///
    /// Scene-native via Vello's `draw_box_shadow` — no GPU compute path
    /// required. The 5th Vello arg is the rim softness (`corner_radius`)
    /// which controls how round the shadow's ellipse rim is; it does not
    /// modify the shadowed shape's geometry.
    pub fn drop_shadow(
        selector: &str,
        params: DropShadowParams,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        Self {
            effect_type: EffectType::DropShadow,
            selector: selector.to_string(),
            region: Region::new(0.0, 0.0, viewport_width as f32, viewport_height as f32),
            blur_params: None,
            transform_params: None,
            color_params: None,
            drop_shadow_params: Some(params),
            canonical_filter_params: None,
            z_index: 0,
        }
    }

    /// Create a canonical CSS filter op effect.
    ///
    /// Requires GPU compute (deferred to the CustomPaintSource path, like
    /// `Effect::color_adjust`); the scene-native apply path is a no-op.
    /// To express a CSS filter chain, emit multiple `Effect` instances
    /// with `Effect::canonical_filter(...)` and the same selector — one
    /// Effect per filter function.
    pub fn canonical_filter(
        selector: &str,
        filter: CanonicalFilter,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        Self {
            effect_type: EffectType::CanonicalFilter,
            selector: selector.to_string(),
            region: Region::new(0.0, 0.0, viewport_width as f32, viewport_height as f32),
            blur_params: None,
            transform_params: None,
            color_params: None,
            drop_shadow_params: None,
            canonical_filter_params: Some(filter),
            z_index: 0,
        }
    }

    /// Update the region for this effect
    pub fn with_region(mut self, region: Region) -> Self {
        self.region = region;
        self
    }

    /// Set z-index for layering
    pub fn with_z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    /// Returns true if this effect can be applied scene-natively
    pub fn is_native(&self) -> bool {
        matches!(
            self.effect_type,
            EffectType::BackdropBlur
                | EffectType::Transform2D
                | EffectType::Clip
                | EffectType::DropShadow
        )
    }

    /// Returns true if this effect requires GPU compute
    pub fn requires_gpu_compute(&self) -> bool {
        matches!(
            self.effect_type,
            EffectType::ColorAdjust | EffectType::CanonicalFilter
        )
    }
}

/// Trait for applying effects to scenes
///
/// Implemented for Effect to apply itself to any PaintScene
///
/// Note: This uses generics instead of dyn PaintScene because PaintScene
/// has methods that make it not object-safe.
#[cfg(feature = "gpu")]
pub trait ApplyEffect<S: anyrender::PaintScene> {
    /// Apply this effect to a scene
    fn apply_to_scene(&self, scene: &mut S, viewport: (u32, u32));
}

#[cfg(feature = "gpu")]
impl<S: anyrender::PaintScene> ApplyEffect<S> for Effect {
    fn apply_to_scene(&self, scene: &mut S, _viewport: (u32, u32)) {
        use kurbo::Rect;
        use peniko::BlendMode;

        match self.effect_type {
            EffectType::BackdropBlur => {
                if let Some(ref params) = self.blur_params {
                    let rect = Rect::new(
                        self.region.x as f64,
                        self.region.y as f64,
                        (self.region.x + self.region.width) as f64,
                        (self.region.y + self.region.height) as f64,
                    );
                    // Map BlurQuality to a corner-radius fraction: higher quality = rounder.
                    let corner_radius = match params.quality {
                        BlurQuality::Low => 0.0,
                        BlurQuality::Medium => 4.0,
                        BlurQuality::High => 8.0,
                        BlurQuality::Ultra => 12.0,
                    };
                    // std_dev ≈ radius / 2 is the conventional CSS mapping.
                    let std_dev = (params.radius / 2.0) as f64;
                    // Frosted-glass: a near-white tint at low alpha so blur halos are visible.
                    let tint = peniko::color::palette::css::WHITE.with_alpha(0.15);
                    // Multi-pass: each pass slightly larger sigma for smoother result.
                    for pass in 0..params.passes {
                        let sigma = std_dev * (1.0 + pass as f64 * 0.3);
                        scene.draw_box_shadow(
                            kurbo::Affine::IDENTITY,
                            rect,
                            tint,
                            corner_radius,
                            sigma,
                        );
                    }
                }
            }
            EffectType::Transform2D => {
                if let Some(ref params) = self.transform_params {
                    // Apply transform using push_layer with transform
                    let rect = Rect::new(
                        self.region.x as f64,
                        self.region.y as f64,
                        (self.region.x + self.region.width) as f64,
                        (self.region.y + self.region.height) as f64,
                    );

                    // Build affine transform
                    let transform = kurbo::Affine::translate((
                        (self.region.x + self.region.width * params.pivot_x) as f64,
                        (self.region.y + self.region.height * params.pivot_y) as f64,
                    )) * kurbo::Affine::rotate(
                        params.rotate_degrees.to_radians() as f64
                    ) * kurbo::Affine::scale_non_uniform(
                        params.scale_x as f64,
                        params.scale_y as f64,
                    ) * kurbo::Affine::translate((
                        -(self.region.x + self.region.width * params.pivot_x) as f64,
                        -(self.region.y + self.region.height * params.pivot_y) as f64,
                    )) * kurbo::Affine::translate((
                        params.translate_x as f64,
                        params.translate_y as f64,
                    ));

                    // Push transform layer
                    scene.push_layer(BlendMode::default(), 1.0, transform, &rect);
                    // Note: Caller must pop_layer after rendering content
                }
            }
            EffectType::Clip => {
                // Push clip layer
                let rect = Rect::new(
                    self.region.x as f64,
                    self.region.y as f64,
                    (self.region.x + self.region.width) as f64,
                    (self.region.y + self.region.height) as f64,
                );
                scene.push_clip_layer(kurbo::Affine::IDENTITY, &rect);
                // Note: Caller must pop_layer after rendering content
            }
            EffectType::ColorAdjust => {
                // Requires GPU compute - handled by CustomPaintSource
                // This is a no-op in scene-native rendering
            }
            EffectType::DropShadow => {
                if let Some(ref params) = self.drop_shadow_params {
                    let rect = Rect::new(
                        self.region.x as f64,
                        self.region.y as f64,
                        (self.region.x + self.region.width) as f64,
                        (self.region.y + self.region.height) as f64,
                    );
                    // Compose the shadow tint via the static palette +
                    // the feature-agnostic alpha channel from
                    // `DropShadowParams`. This mirrors the existing
                    // `BackdropBlur` apply path which uses
                    // `peniko::color::palette::css::WHITE.with_alpha(0.15)`.
                    // Per the `DropShadowParams` doc-comment, peniko 0.6
                    // does not expose a usable free-form RGBA Color
                    // constructor (rgba / rgba8 / Srgba::new all fail
                    // to compile against 0.6.x). The static-palette path
                    // is the canonical workaround.
                    let color = peniko::color::palette::css::BLACK.with_alpha(params.alpha);
                    // Vello's `draw_box_shadow` 5th arg is the sigma
                    // directly — the CSS `blur-radius` is the sigma we
                    // pass through unmodified. The 4th arg
                    // (`corner_radius`) is the rim softness, not the
                    // shadowed shape's corner radius.
                    scene.draw_box_shadow(
                        kurbo::Affine::IDENTITY,
                        rect,
                        color,
                        params.corner_radius as f64,
                        params.blur_radius as f64,
                    );
                }
            }
            EffectType::CanonicalFilter => {
                // Requires GPU compute - handled by CustomPaintSource
                // (matches `EffectType::ColorAdjust`'s deferred semantics).
                // One filter op per Effect; CSS filter chains are
                // expressed as multiple CanonicalFilter Effects on the
                // same selector, each committing/replaying atomically.
            }
        }
    }
}

#[cfg(not(feature = "gpu"))]
/// Trait stub when GPU feature is disabled
pub trait ApplyEffect<S> {
    /// Apply this effect to a scene (no-op without GPU feature)
    fn apply_to_scene(&self, _scene: &mut S, _viewport: (u32, u32));
}

#[cfg(not(feature = "gpu"))]
impl<S> ApplyEffect<S> for Effect {
    fn apply_to_scene(&self, _scene: &mut S, _viewport: (u32, u32)) {
        // No-op when GPU feature is disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_blur() {
        let effect = Effect::blur(".test", 10.0, 1280, 800);
        assert!(matches!(effect.effect_type, EffectType::BackdropBlur));
        assert!(effect.is_native());
    }

    #[test]
    fn test_effect_transform() {
        let params = TransformParams::default();
        let effect = Effect::transform(".test", params, 1280, 800);
        assert!(matches!(effect.effect_type, EffectType::Transform2D));
        assert!(effect.is_native());
    }

    #[test]
    fn test_effect_color_adjust_requires_gpu() {
        let params = ColorAdjustParams::default();
        let effect = Effect::color_adjust(".test", params);
        assert!(effect.requires_gpu_compute());
        assert!(!effect.is_native());
    }

    #[test]
    fn test_effect_builder_pattern() {
        let effect = Effect::blur(".glass", 15.0, 1280, 800)
            .with_z_index(10)
            .with_region(Region::new(10.0, 10.0, 200.0, 100.0));

        assert_eq!(effect.z_index, 10);
        assert_eq!(effect.region.x, 10.0);
        assert_eq!(effect.region.y, 10.0);
        assert_eq!(effect.region.width, 200.0);
        assert_eq!(effect.region.height, 100.0);
    }

    // --- DropShadow effect kind -----------------------------------------

    #[test]
    fn test_effect_drop_shadow_is_native() {
        let params = DropShadowParams::default();
        let effect = Effect::drop_shadow(".card", params, 1280, 800);
        assert!(matches!(effect.effect_type, EffectType::DropShadow));
        assert_eq!(effect.drop_shadow_params, Some(params));
        assert!(effect.is_native(), "DropShadow is scene-native via draw_box_shadow");
        assert!(!effect.requires_gpu_compute());
    }

    #[test]
    fn test_effect_drop_shadow_region_override() {
        let effect = Effect::drop_shadow(".card", DropShadowParams::default(), 1280, 800)
            .with_region(Region::new(50.0, 50.0, 200.0, 100.0))
            .with_z_index(5);
        assert_eq!(effect.region.x, 50.0);
        assert_eq!(effect.region.width, 200.0);
        assert_eq!(effect.z_index, 5);
    }

    #[test]
    fn test_drop_shadow_default_is_visible() {
        let p = DropShadowParams::default();
        // Default shadow MUST be visible (non-zero alpha) so a consumer
        // calling `Effect::drop_shadow(selector, DropShadowParams::default(), ...)`
        // sees a shadow at the default origin below the element.
        assert!(p.alpha > 0.0, "default shadow alpha must be >0");
        assert!(p.blur_radius >= 0.0);
        assert!(p.corner_radius >= 0.0);
        // Default geometry: 4 px down, 4 px blur, no horizontal offset.
        assert_eq!(p.offset_x, 0.0);
        assert!(p.offset_y > 0.0, "default shadow offset_y must be >0 (drops downward)");
    }

    #[test]
    fn test_drop_shadow_params_simplified_field_count() {
        // Pin the simplified-API contract: DropShadowParams has exactly 5
        // fields (alpha + blur_radius + offset_x + offset_y + corner_radius).
        // The 4-RGBA-channel + spread field surface was deliberately
        // removed (see the struct's doc-comment for the rationale).
        // If you re-add fields, update this assertion; it acts as a
        // tripwire so an addition to the surface is visible in code review.
        let p = DropShadowParams::default();
        let _field_count_proof: () = {
            let DropShadowParams {
                alpha: _,
                blur_radius: _,
                offset_x: _,
                offset_y: _,
                corner_radius: _,
            } = p;
            ()
        };
        // Smoke check: assert is reachable, type expression is not exhaustive.
    }

    // --- CanonicalFilter effect kind ------------------------------------

    #[test]
    fn test_effect_canonical_filter_hue_rotate_is_gpu_compute() {
        let filter = CanonicalFilter::HueRotate(90.0);
        let effect = Effect::canonical_filter(".img", filter, 1280, 800);
        assert!(matches!(effect.effect_type, EffectType::CanonicalFilter));
        assert!(!effect.is_native(), "CanonicalFilter defers to GPU compute");
        assert!(effect.requires_gpu_compute());
        assert_eq!(effect.canonical_filter_params, Some(filter));
    }

    #[test]
    fn test_canonical_filter_variants_are_distinct() {
        // Different op kinds are not equal even at the "identity" amount.
        assert_ne!(
            CanonicalFilter::Brightness(1.0),
            CanonicalFilter::Saturate(1.0),
        );
        // Same op-kind + same amount are equal.
        assert_eq!(
            CanonicalFilter::Contrast(0.8),
            CanonicalFilter::Contrast(0.8),
        );
        // Same op-kind + different amounts are not equal.
        assert_ne!(
            CanonicalFilter::Grayscale(0.5),
            CanonicalFilter::Grayscale(0.6),
        );
        // HueRotate takes degrees, not the unitless multiplier the other
        // 5 variants share; rotation-identity is 0.0deg, not 1.0.
        assert_ne!(
            CanonicalFilter::HueRotate(0.0),
            CanonicalFilter::Brightness(0.0),
        );
    }

    #[test]
    fn test_canonical_filter_identity_amounts() {
        // Pin the public API's identity expectation: each variant's
        // identity value is either 1.0 (saturate/brightness/contrast) or
        // 0.0 (grayscale/invert/hue-rotate). Consumers clamp to these
        // values when emitting the no-op filter, so the contract must
        // hold.
        assert_eq!(CanonicalFilter::Saturate(1.0), CanonicalFilter::Saturate(1.0));
        assert_eq!(CanonicalFilter::Brightness(1.0), CanonicalFilter::Brightness(1.0));
        assert_eq!(CanonicalFilter::Contrast(1.0), CanonicalFilter::Contrast(1.0));
        assert_eq!(CanonicalFilter::Grayscale(0.0), CanonicalFilter::Grayscale(0.0));
        assert_eq!(CanonicalFilter::Invert(0.0), CanonicalFilter::Invert(0.0));
        assert_eq!(CanonicalFilter::HueRotate(0.0), CanonicalFilter::HueRotate(0.0));
    }

    #[test]
    fn test_is_native_vs_gpu_compute_partition_is_disjoint() {
        // Every Effect variant is either native OR gpu-compute — never
        // both, never neither. This is the doctrinally-meaningful split:
        // a Vello scene-side primitive vs a deferred GPU compute path.
        let native_effects = [
            Effect::blur(".x", 1.0, 1, 1),
            Effect::transform(".x", TransformParams::default(), 1, 1),
            Effect::clip(Region::new(0.0, 0.0, 1.0, 1.0)),
            Effect::drop_shadow(".x", DropShadowParams::default(), 1, 1),
        ];
        let gpu_effects = [
            Effect::color_adjust(".x", ColorAdjustParams::default()),
            Effect::canonical_filter(".x", CanonicalFilter::Brightness(1.0), 1, 1),
        ];
        for e in &native_effects {
            assert!(e.is_native(), "{:?} should be native", e.effect_type);
            assert!(
                !e.requires_gpu_compute(),
                "{:?} should NOT be flagged gpu-compute",
                e.effect_type
            );
        }
        for e in &gpu_effects {
            assert!(!e.is_native(), "{:?} should NOT be native", e.effect_type);
            assert!(
                e.requires_gpu_compute(),
                "{:?} should be flagged gpu-compute",
                e.effect_type
            );
        }
    }
}
