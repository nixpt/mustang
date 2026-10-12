//! Animation system for Mustang
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! Provides JavaScript-driven animations with GPU-accelerated effects.

pub mod easing;

// JS animation runtime (boa_engine). Opt-in `js` feature: boa_engine 0.22 pins
// icu_normalizer ~2.3, which coexists with parley's ^2.1.1 (boa 0.21's ~2.0.0 did not,
// which is why this was deferred until now).
#[cfg(feature = "js")]
pub mod js_binding;

use crate::effect::{ColorAdjustParams, Effect, EffectType, TransformParams};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub use easing::EasingFunction;

/// Animation state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationState {
    /// Animation is waiting to start
    Pending,
    /// Animation is currently running
    Running,
    /// Animation has completed
    Completed,
    /// Animation was cancelled
    Cancelled,
}

/// The effect kind and interpolated scalars of an [`AnimatedProperty`] at
/// a given progress.
///
/// Produced by [`AnimatedProperty::interpolate`]. `kind` names the
/// [`EffectType`] the property animates; `value` is the primary
/// interpolated scalar — blur radius for `BackdropBlur`, uniform scale /
/// translate_x / degrees for `Transform2D`, and the multiplier for
/// `ColorAdjust` (brightness or the opacity fade-to-black approximation).
/// `value2` carries the second scalar of two-dimensional properties
/// (`translate_y`); it is `0.0` for one-dimensional properties.
///
/// Construct the full [`Effect`] with [`AnimatedProperty::to_effect`],
/// which routes through this struct so the interpolation math lives in
/// exactly one place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterpolatedValue {
    /// Which effect kind the property animates.
    pub kind: EffectType,
    /// Primary interpolated scalar at the requested progress.
    pub value: f32,
    /// Second scalar for two-dimensional properties (`translate_y`).
    /// `0.0` for one-dimensional properties.
    pub value2: f32,
}

/// A single animated property
#[derive(Debug, Clone)]
pub enum AnimatedProperty {
    /// Blur radius animation
    Blur { from: f32, to: f32 },
    /// Opacity animation (0.0 - 1.0). No native opacity effect kind yet —
    /// animates as a ColorAdjust fade-to-black approximation (see
    /// `interpolate()`'s Opacity arm for the decided semantics).
    Opacity { from: f32, to: f32 },
    /// Scale animation
    Scale { from: f32, to: f32 },
    /// Translation animation
    Translate {
        from_x: f32,
        from_y: f32,
        to_x: f32,
        to_y: f32,
    },
    /// Rotation animation (degrees)
    Rotate { from: f32, to: f32 },
    /// Color adjustment
    Brightness { from: f32, to: f32 },
}

impl AnimatedProperty {
    /// Interpolate the property at a given progress (0.0 - 1.0).
    ///
    /// Returns the effect kind the property animates **plus** the real
    /// interpolated scalars — nothing is discarded. [`AnimatedProperty::to_effect`]
    /// routes through this method so the interpolation math lives in exactly
    /// one place.
    pub fn interpolate(&self, progress: f32) -> InterpolatedValue {
        let t = progress.clamp(0.0, 1.0);
        match *self {
            AnimatedProperty::Blur { from, to } => InterpolatedValue {
                kind: EffectType::BackdropBlur,
                value: from + (to - from) * t,
                value2: 0.0,
            },
            AnimatedProperty::Opacity { from, to } => {
                // Opacity semantics (decided): mustang has no native
                // opacity/alpha-compositing effect kind yet, so opacity
                // animates toward the ColorAdjust family as a
                // fade-to-black approximation — all RGB multipliers
                // become the interpolated alpha. True alpha compositing
                // (a dedicated `EffectType::Opacity` rendered scene-natively
                // via Vello's `push_layer` alpha) is the follow-up once
                // the effect-kind roster grows.
                InterpolatedValue {
                    kind: EffectType::ColorAdjust,
                    value: from + (to - from) * t,
                    value2: 0.0,
                }
            }
            AnimatedProperty::Scale { from, to } => InterpolatedValue {
                kind: EffectType::Transform2D,
                value: from + (to - from) * t,
                value2: 0.0,
            },
            AnimatedProperty::Translate {
                from_x,
                from_y,
                to_x,
                to_y,
            } => InterpolatedValue {
                kind: EffectType::Transform2D,
                value: from_x + (to_x - from_x) * t,
                value2: from_y + (to_y - from_y) * t,
            },
            AnimatedProperty::Rotate { from, to } => InterpolatedValue {
                kind: EffectType::Transform2D,
                value: from + (to - from) * t,
                value2: 0.0,
            },
            AnimatedProperty::Brightness { from, to } => InterpolatedValue {
                kind: EffectType::ColorAdjust,
                value: from + (to - from) * t,
                value2: 0.0,
            },
        }
    }

    /// Get the effect at the current progress
    ///
    /// Routes through [`AnimatedProperty::interpolate`] — the interpolated
    /// scalars come from there, this method only shapes them into the
    /// per-kind parameter structs.
    pub fn to_effect(&self, selector: &str, viewport: (u32, u32), progress: f32) -> Effect {
        let iv = self.interpolate(progress);
        match *self {
            AnimatedProperty::Blur { .. } => {
                Effect::blur(selector, iv.value, viewport.0, viewport.1)
            }
            // Fade-to-black approximation — see `interpolate()`'s
            // Opacity arm for the decided semantics.
            AnimatedProperty::Opacity { .. } => {
                let params = ColorAdjustParams {
                    red_multiplier: iv.value,
                    green_multiplier: iv.value,
                    blue_multiplier: iv.value,
                    ..Default::default()
                };
                Effect::color_adjust(selector, params)
            }
            AnimatedProperty::Scale { .. } => {
                let params = TransformParams {
                    scale_x: iv.value,
                    scale_y: iv.value,
                    ..Default::default()
                };
                Effect::transform(selector, params, viewport.0, viewport.1)
            }
            AnimatedProperty::Translate { .. } => {
                let params = TransformParams {
                    translate_x: iv.value,
                    translate_y: iv.value2,
                    ..Default::default()
                };
                Effect::transform(selector, params, viewport.0, viewport.1)
            }
            AnimatedProperty::Rotate { .. } => {
                let params = TransformParams {
                    rotate_degrees: iv.value,
                    ..Default::default()
                };
                Effect::transform(selector, params, viewport.0, viewport.1)
            }
            AnimatedProperty::Brightness { .. } => {
                let params = ColorAdjustParams {
                    red_multiplier: iv.value,
                    green_multiplier: iv.value,
                    blue_multiplier: iv.value,
                    ..Default::default()
                };
                Effect::color_adjust(selector, params)
            }
        }
    }
}

/// Animation configuration
#[derive(Debug, Clone)]
pub struct AnimationConfig {
    /// Target CSS selector
    pub selector: String,
    /// Animation duration
    pub duration: Duration,
    /// Delay before animation starts
    pub delay: Duration,
    /// Easing function
    pub easing: EasingFunction,
    /// Number of iterations (None = infinite)
    pub iterations: Option<u32>,
    /// Whether animation alternates direction
    pub alternate: bool,
    /// Whether to fill forwards after completion
    pub fill_forwards: bool,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            selector: String::new(),
            duration: Duration::from_millis(300),
            delay: Duration::ZERO,
            easing: EasingFunction::Ease,
            iterations: Some(1),
            alternate: false,
            fill_forwards: true,
        }
    }
}

/// A single animation instance
#[derive(Debug, Clone)]
pub struct Animation {
    /// Animation configuration
    pub config: AnimationConfig,
    /// Properties being animated
    pub properties: Vec<AnimatedProperty>,
    /// Animation state
    pub state: AnimationState,
    /// When the animation started
    pub started_at: Option<Instant>,
    /// Current iteration
    pub current_iteration: u32,
}

impl Animation {
    /// Create a new animation
    pub fn new(config: AnimationConfig, properties: Vec<AnimatedProperty>) -> Self {
        Self {
            config,
            properties,
            state: AnimationState::Pending,
            started_at: None,
            current_iteration: 0,
        }
    }

    /// Start the animation
    pub fn start(&mut self) {
        self.state = AnimationState::Running;
        self.started_at = Some(Instant::now());
    }

    /// Update animation state and return current effects
    pub fn tick(&mut self, viewport: (u32, u32)) -> Vec<Effect> {
        if self.state != AnimationState::Running {
            return Vec::new();
        }

        let now = Instant::now();
        let started_at = match self.started_at {
            Some(t) => t,
            None => return Vec::new(),
        };

        let elapsed = now.duration_since(started_at);

        // Handle delay
        if elapsed < self.config.delay {
            return Vec::new();
        }

        let anim_elapsed = elapsed - self.config.delay;
        let duration_ms = self.config.duration.as_millis() as f64;
        let elapsed_ms = anim_elapsed.as_millis() as f64;

        // Calculate progress
        let raw_progress = (elapsed_ms / duration_ms).min(1.0) as f32;

        // Apply easing
        let eased_progress = self.config.easing.apply(raw_progress);

        // Handle iteration
        if raw_progress >= 1.0 {
            self.current_iteration += 1;

            // Check if animation is complete
            if let Some(max_iterations) = self.config.iterations {
                if self.current_iteration >= max_iterations {
                    self.state = AnimationState::Completed;
                    if self.config.fill_forwards {
                        // Return final state
                        return self
                            .properties
                            .iter()
                            .map(|p| p.to_effect(&self.config.selector, viewport, 1.0))
                            .collect();
                    }
                    return Vec::new();
                }
            }

            // Reset for next iteration
            self.started_at = Some(now);
        }

        // Calculate actual progress based on direction
        let progress = if self.config.alternate && self.current_iteration % 2 == 1 {
            1.0 - eased_progress
        } else {
            eased_progress
        };

        // Generate effects
        self.properties
            .iter()
            .map(|p| p.to_effect(&self.config.selector, viewport, progress))
            .collect()
    }

    /// Cancel the animation
    pub fn cancel(&mut self) {
        self.state = AnimationState::Cancelled;
    }

    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        self.state == AnimationState::Completed || self.state == AnimationState::Cancelled
    }
}

/// Animation engine that manages all active animations
#[derive(Debug, Default)]
pub struct AnimationEngine {
    animations: Vec<Animation>,
    named_animations: HashMap<String, usize>,
}

impl AnimationEngine {
    /// Create a new animation engine
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
            named_animations: HashMap::new(),
        }
    }

    /// Add an animation
    pub fn add_animation(&mut self, animation: Animation) -> usize {
        let id = self.animations.len();
        self.animations.push(animation);
        id
    }

    /// Add a named animation
    pub fn add_named_animation(&mut self, name: &str, animation: Animation) -> usize {
        let id = self.add_animation(animation);
        self.named_animations.insert(name.to_string(), id);
        id
    }

    /// Get animation by ID
    pub fn get_animation(&mut self, id: usize) -> Option<&mut Animation> {
        self.animations.get_mut(id)
    }

    /// Get animation by name
    pub fn get_named_animation(&mut self, name: &str) -> Option<&mut Animation> {
        let id = self.named_animations.get(name)?;
        self.animations.get_mut(*id)
    }

    /// Start an animation by ID
    pub fn start(&mut self, id: usize) -> bool {
        if let Some(anim) = self.animations.get_mut(id) {
            anim.start();
            true
        } else {
            false
        }
    }

    /// Start an animation by name
    pub fn start_named(&mut self, name: &str) -> bool {
        if let Some(id) = self.named_animations.get(name).copied() {
            self.start(id)
        } else {
            false
        }
    }

    /// Tick all animations and return active effects
    pub fn tick(&mut self, viewport: (u32, u32)) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut completed = Vec::new();

        for (i, anim) in self.animations.iter_mut().enumerate() {
            if anim.state == AnimationState::Running {
                let mut anim_effects = anim.tick(viewport);
                effects.append(&mut anim_effects);

                if anim.is_complete() {
                    completed.push(i);
                }
            }
        }

        // Clean up completed animations (optional - could be deferred)
        // self.animations.retain(|a| !a.is_complete());

        effects
    }

    /// Cancel an animation by ID
    pub fn cancel(&mut self, id: usize) -> bool {
        if let Some(anim) = self.animations.get_mut(id) {
            anim.cancel();
            true
        } else {
            false
        }
    }

    /// Cancel all animations
    pub fn cancel_all(&mut self) {
        for anim in &mut self.animations {
            anim.cancel();
        }
    }

    /// Count of RUNNING animations. Pending/queued and Completed/Cancelled
    /// animations are NOT counted. Note: `cleanup()` retains non-complete
    /// animations (Pending + Running), so the engine may hold pending
    /// animations that aren't reflected in `active_count`.
    /// Matches `AnimationEngine::test_animation_engine`'s "Not started yet" assertion.
    pub fn active_count(&self) -> usize {
        self.animations
            .iter()
            .filter(|a| matches!(a.state, AnimationState::Running))
            .count()
    }

    /// Clear all completed animations
    pub fn cleanup(&mut self) {
        self.animations.retain(|a| !a.is_complete());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_config_default() {
        let config = AnimationConfig::default();
        assert_eq!(config.duration, Duration::from_millis(300));
        assert_eq!(config.delay, Duration::ZERO);
        assert_eq!(config.iterations, Some(1));
    }

    #[test]
    fn test_interpolate_carries_real_values() {
        // Blur: 0..10 at mid-progress → radius 5.0, kind BackdropBlur.
        let blur = AnimatedProperty::Blur { from: 0.0, to: 10.0 };
        let iv = blur.interpolate(0.5);
        assert_eq!(iv.kind, EffectType::BackdropBlur);
        assert!((iv.value - 5.0).abs() < f32::EPSILON, "radius must be interpolated, got {}", iv.value);

        // Clamp: progress > 1.0 clamps to 1.0 → radius 10.0.
        assert_eq!(blur.interpolate(1.5).value, 10.0);

        // Translate: x and y both interpolated (value / value2).
        let translate = AnimatedProperty::Translate {
            from_x: 0.0,
            from_y: 10.0,
            to_x: 100.0,
            to_y: 20.0,
        };
        let iv = translate.interpolate(0.25);
        assert_eq!(iv.kind, EffectType::Transform2D);
        assert!((iv.value - 25.0).abs() < f32::EPSILON);
        assert!((iv.value2 - 12.5).abs() < f32::EPSILON);

        // Rotate / Scale / Brightness carry their single scalar.
        assert_eq!(AnimatedProperty::Scale { from: 1.0, to: 2.0 }.interpolate(0.5).value, 1.5);
        assert_eq!(
            AnimatedProperty::Rotate { from: 0.0, to: 90.0 }.interpolate(1.0).value,
            90.0
        );
        assert_eq!(
            AnimatedProperty::Brightness { from: 1.0, to: 2.0 }.interpolate(0.5).value,
            1.5
        );
    }

    #[test]
    fn test_opacity_semantics_fade_to_black() {
        // Decided semantics: opacity approximates as a ColorAdjust
        // fade-to-black (RGB multipliers = interpolated alpha).
        let opacity = AnimatedProperty::Opacity { from: 1.0, to: 0.0 };
        let iv = opacity.interpolate(0.25);
        assert_eq!(iv.kind, EffectType::ColorAdjust, "opacity animates toward ColorAdjust (fade-to-black approximation)");
        assert!((iv.value - 0.75).abs() < f32::EPSILON);

        let effect = opacity.to_effect(".target", (100, 100), 0.5);
        assert_eq!(effect.effect_type, EffectType::ColorAdjust);
        let params = effect.color_params.expect("opacity → ColorAdjust params");
        assert!((params.red_multiplier - 0.5).abs() < f32::EPSILON);
        assert_eq!(params.red_multiplier, params.green_multiplier);
        assert_eq!(params.green_multiplier, params.blue_multiplier);
    }

    #[test]
    fn test_to_effect_matches_interpolate() {
        // to_effect() routes through interpolate(): every property's
        // effect params must carry the same scalars interpolate() returns.
        let props = [
            AnimatedProperty::Blur { from: 2.0, to: 12.0 },
            AnimatedProperty::Scale { from: 1.0, to: 3.0 },
            AnimatedProperty::Translate {
                from_x: 0.0,
                from_y: 5.0,
                to_x: 50.0,
                to_y: 25.0,
            },
            AnimatedProperty::Rotate { from: 0.0, to: 180.0 },
            AnimatedProperty::Brightness { from: 1.0, to: 2.0 },
        ];
        for p in props {
            let iv = p.interpolate(0.5);
            let effect = p.to_effect(".x", (80, 60), 0.5);
            match iv.kind {
                EffectType::BackdropBlur => {
                    let bp = effect.blur_params.expect("blur params");
                    assert!((bp.radius - iv.value).abs() < f32::EPSILON);
                }
                EffectType::Transform2D => {
                    let tp = effect.transform_params.expect("transform params");
                    let carried = if matches!(p, AnimatedProperty::Translate { .. }) {
                        (tp.translate_x, tp.translate_y)
                    } else if matches!(p, AnimatedProperty::Rotate { .. }) {
                        (tp.rotate_degrees, 0.0)
                    } else {
                        // Scale is uniform: both axes ride `value`.
                        (tp.scale_x, tp.scale_y)
                    };
                    assert!((carried.0 - iv.value).abs() < f32::EPSILON);
                    let expected = if matches!(p, AnimatedProperty::Scale { .. }) {
                        iv.value
                    } else {
                        iv.value2
                    };
                    assert!((carried.1 - expected).abs() < f32::EPSILON);
                }
                EffectType::ColorAdjust => {
                    let cp = effect.color_params.expect("color params");
                    assert!((cp.red_multiplier - iv.value).abs() < f32::EPSILON);
                }
                other => panic!("unexpected kind from interpolation: {other:?}"),
            }
        }
    }

    #[test]
    fn test_animation_state_transitions() {
        let config = AnimationConfig::default();
        let properties = vec![AnimatedProperty::Blur {
            from: 0.0,
            to: 10.0,
        }];
        let mut anim = Animation::new(config, properties);

        assert_eq!(anim.state, AnimationState::Pending);
        anim.start();
        assert_eq!(anim.state, AnimationState::Running);
        anim.cancel();
        assert_eq!(anim.state, AnimationState::Cancelled);
    }

    #[test]
    fn test_animation_engine() {
        let mut engine = AnimationEngine::new();
        let config = AnimationConfig::default();
        let properties = vec![AnimatedProperty::Blur {
            from: 0.0,
            to: 10.0,
        }];
        let anim = Animation::new(config, properties);

        let id = engine.add_animation(anim);
        assert_eq!(engine.active_count(), 0); // Not started yet

        engine.start(id);
        assert_eq!(engine.active_count(), 1);
    }

    #[test]
    fn test_named_animation() {
        let mut engine = AnimationEngine::new();
        let config = AnimationConfig {
            selector: ".test".to_string(),
            ..Default::default()
        };
        let properties = vec![AnimatedProperty::Scale { from: 1.0, to: 1.5 }];
        let anim = Animation::new(config, properties);

        engine.add_named_animation("test-anim", anim);
        assert!(engine.start_named("test-anim"));
        assert!(!engine.start_named("nonexistent"));
    }
}
