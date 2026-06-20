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

#[cfg(feature = "gpu")]
impl TransformParams {
    /// Construct from a `kurbo::Affine`. Decomposes the Affine into
    /// scale + rotate + translate. **Skew is not supported** — Affines
    /// with non-zero shear are approximated to the closest
    /// scale+rotate+translate form (the `TransformParams` shape cannot
    /// represent shear). Pivot defaults to the center (0.5, 0.5).
    pub fn from_affine(affine: kurbo::Affine) -> Self {
        let c = affine.as_coeffs();
        let scale_x = (c[0] * c[0] + c[1] * c[1]).sqrt();
        let scale_y = (c[2] * c[2] + c[3] * c[3]).sqrt();
        let rotate_degrees = c[1].atan2(c[0]).to_degrees() as f32;
        Self {
            scale_x: scale_x as f32,
            scale_y: scale_y as f32,
            translate_x: c[4] as f32,
            translate_y: c[5] as f32,
            rotate_degrees,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }
}

/// z-index assigned to `Clip` effects so they always render on top
/// of any other effect applied to the same region.
pub const CLIP_Z_INDEX: i32 = 9999;

/// A Mustang effect to be applied to a region
#[derive(Debug, Clone)]
#[non_exhaustive]
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
            z_index: 0,
        }
    }

    /// Create a transform effect from a `kurbo::Affine`. Convenience
    /// for the common Vello use case where the caller already has an
    /// Affine and doesn't want to decompose it manually. Skew is
    /// approximated away; see [`TransformParams::from_affine`].
    #[cfg(feature = "gpu")]
    pub fn transform_with_affine(
        selector: &str,
        affine: kurbo::Affine,
        region: Region,
    ) -> Self {
        let params = TransformParams::from_affine(affine);
        Self {
            effect_type: EffectType::Transform2D,
            selector: selector.to_string(),
            region,
            blur_params: None,
            transform_params: Some(params),
            color_params: None,
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
            z_index: CLIP_Z_INDEX,
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
            EffectType::BackdropBlur | EffectType::Transform2D | EffectType::Clip
        )
    }

    /// Returns true if this effect can be applied directly via
    /// `apply_to_scene` without a layer scope. BackdropBlur and ColorAdjust
    /// qualify; Transform and Clip require scope handling.
    pub fn is_one_shot(&self) -> bool {
        matches!(
            self.effect_type,
            EffectType::BackdropBlur | EffectType::ColorAdjust
        )
    }

    /// Returns true if this effect requires a layer scope (Transform, Clip).
    /// Use `Effect::begin_layer_scope` to obtain an RAII guard that pops the
    /// layer on drop.
    pub fn is_layer_scope(&self) -> bool {
        matches!(self.effect_type, EffectType::Transform2D | EffectType::Clip)
    }

    /// Returns true if this effect requires GPU compute
    pub fn requires_gpu_compute(&self) -> bool {
        matches!(self.effect_type, EffectType::ColorAdjust)
    }

    /// Begin a layer scope for this effect. Returns `Some(guard)` for
    /// layer effects (Transform, Clip) and `None` for one-shot effects
    /// (Blur, ColorAdjust). The guard pops the layer on drop; call
    /// `LayerGuard::release` to opt out of automatic pop.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use mustang::{Effect, EffectScene};
    /// # fn render<S: EffectScene>(scene: &mut S, effect: Effect) {
    /// if let Some(_guard) = effect.begin_layer_scope(scene, (800, 600)) {
    ///     // render content into the layered scene
    /// } // guard drops, pop_layer is called
    /// # }
    /// ```
    #[cfg(feature = "gpu")]
    pub fn begin_layer_scope<'a, S: anyrender::PaintScene>(
        &self,
        scene: &'a mut S,
        _viewport: (u32, u32),
    ) -> Option<LayerGuard<'a, S>> {
        use kurbo::Rect;
        use peniko::BlendMode;

        match self.effect_type {
            EffectType::Transform2D => {
                let params = self.transform_params.as_ref()?;
                let rect = Rect::new(
                    self.region.x as f64,
                    self.region.y as f64,
                    (self.region.x + self.region.width) as f64,
                    (self.region.y + self.region.height) as f64,
                );
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
                scene.push_layer(BlendMode::default(), 1.0, transform, &rect);
                Some(LayerGuard::new(scene))
            }
            EffectType::Clip => {
                let rect = Rect::new(
                    self.region.x as f64,
                    self.region.y as f64,
                    (self.region.x + self.region.width) as f64,
                    (self.region.y + self.region.height) as f64,
                );
                scene.push_clip_layer(kurbo::Affine::IDENTITY, &rect);
                Some(LayerGuard::new(scene))
            }
            _ => None,
        }
    }
}

/// RAII guard for a Vello/anyrender layer. Pops the layer on drop.
///
/// Created by `Effect::begin_layer_scope` and `EffectScene::begin_layer_scope`.
/// Holding the guard keeps the layer active; dropping it pops the layer.
/// Call `LayerGuard::release` to disarm the guard and pop manually.
#[cfg(feature = "gpu")]
pub struct LayerGuard<'a, S: ?Sized + anyrender::PaintScene> {
    scene: &'a mut S,
    active: bool,
}

#[cfg(feature = "gpu")]
impl<'a, S: ?Sized + anyrender::PaintScene> LayerGuard<'a, S> {
    /// Create a new active guard wrapping the given scene.
    pub fn new(scene: &'a mut S) -> Self {
        Self {
            scene,
            active: true,
        }
    }

    /// Disarm the guard so its `Drop` will not pop the layer. After
    /// calling this, the caller is responsible for popping the layer
    /// (e.g. when transferring ownership to an async pipeline).
    pub fn release(mut self) {
        self.active = false;
    }
}

#[cfg(feature = "gpu")]
impl<'a, S: ?Sized + anyrender::PaintScene> Drop for LayerGuard<'a, S> {
    fn drop(&mut self) {
        if self.active {
            self.scene.pop_layer();
        }
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
    /// Apply this effect to a scene. Only valid for one-shot effects
    /// (BackdropBlur, ColorAdjust). For layer effects (Transform, Clip),
    /// use `Effect::begin_layer_scope` / `EffectScene::begin_layer_scope`
    /// to obtain an RAII guard that pops the layer on drop.
    fn apply_to_scene(&self, scene: &mut S, viewport: (u32, u32));
}

#[cfg(feature = "gpu")]
impl<S: anyrender::PaintScene> ApplyEffect<S> for Effect {
    fn apply_to_scene(&self, scene: &mut S, _viewport: (u32, u32)) {
        use kurbo::Rect;

        match self.effect_type {
            EffectType::BackdropBlur => {
                if let Some(ref params) = self.blur_params {
                    let rect = Rect::new(
                        self.region.x as f64,
                        self.region.y as f64,
                        (self.region.x + self.region.width) as f64,
                        (self.region.y + self.region.height) as f64,
                    );
                    let corner_radius = match params.quality {
                        BlurQuality::Low => 0.0,
                        BlurQuality::Medium => 4.0,
                        BlurQuality::High => 8.0,
                        BlurQuality::Ultra => 12.0,
                    };
                    let std_dev = (params.radius / 2.0) as f64;
                    let tint = peniko::color::palette::css::WHITE.with_alpha(0.15);
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
            EffectType::ColorAdjust => {
                // Requires GPU compute - handled by CustomPaintSource
            }
            EffectType::Transform2D | EffectType::Clip => {
                // Layer effects require scope handling via
                // `Effect::begin_layer_scope` / `EffectScene::begin_layer_scope`.
                // `apply_to_scene` is a no-op for these so the RAII guard
                // is the only path that can push a layer — preventing the
                // "caller must pop_layer" footgun the old API had.
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

    #[test]
    fn test_effect_one_shot_vs_layer_scope() {
        let blur = Effect::blur(".blur", 10.0, 100, 100);
        assert!(blur.is_one_shot());
        assert!(!blur.is_layer_scope());

        let transform = Effect::transform(".x", TransformParams::default(), 100, 100);
        assert!(!transform.is_one_shot());
        assert!(transform.is_layer_scope());

        let clip = Effect::clip(Region::new(0.0, 0.0, 10.0, 10.0));
        assert!(!clip.is_one_shot());
        assert!(clip.is_layer_scope());

        let color = Effect::color_adjust(".c", ColorAdjustParams::default());
        assert!(color.is_one_shot());
        assert!(!color.is_layer_scope());
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_transform_params_from_identity_affine() {
        let p = TransformParams::from_affine(kurbo::Affine::IDENTITY);
        assert_eq!(p.scale_x, 1.0);
        assert_eq!(p.scale_y, 1.0);
        assert_eq!(p.translate_x, 0.0);
        assert_eq!(p.translate_y, 0.0);
        assert_eq!(p.rotate_degrees, 0.0);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_transform_params_from_translation() {
        let p = TransformParams::from_affine(kurbo::Affine::translate((10.0, 20.0)));
        assert_eq!(p.scale_x, 1.0);
        assert_eq!(p.scale_y, 1.0);
        assert_eq!(p.translate_x, 10.0);
        assert_eq!(p.translate_y, 20.0);
        assert_eq!(p.rotate_degrees, 0.0);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_transform_params_from_scale() {
        let p = TransformParams::from_affine(kurbo::Affine::scale(2.0));
        assert_eq!(p.scale_x, 2.0);
        assert_eq!(p.scale_y, 2.0);
        assert_eq!(p.translate_x, 0.0);
        assert_eq!(p.translate_y, 0.0);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_transform_params_from_rotation() {
        let p = TransformParams::from_affine(kurbo::Affine::rotate(std::f64::consts::FRAC_PI_4));
        assert!((p.rotate_degrees - 45.0).abs() < 1e-3, "got {}", p.rotate_degrees);
        assert!((p.scale_x - 1.0).abs() < 1e-6);
        assert!((p.scale_y - 1.0).abs() < 1e-6);
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn test_transform_with_affine_constructor() {
        let affine = kurbo::Affine::translate((5.0, 10.0));
        let effect = Effect::transform_with_affine(
            ".x",
            affine,
            Region::new(0.0, 0.0, 100.0, 100.0),
        );
        assert!(matches!(effect.effect_type, EffectType::Transform2D));
        let params = effect.transform_params.expect("transform_params set");
        assert_eq!(params.translate_x, 5.0);
        assert_eq!(params.translate_y, 10.0);
        assert_eq!(effect.region.x, 0.0);
        assert_eq!(effect.region.width, 100.0);
    }
}
