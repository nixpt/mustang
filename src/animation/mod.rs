//! Animation system for Mustang
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! Provides JavaScript-driven animations with GPU-accelerated effects.

pub mod easing;

// js_binding (boa_engine JS runtime) deferred — boa_engine 0.21 pins icu_normalizer ~2.0.0
// which conflicts with parley ^0.10 -> icu_normalizer ^2.1.1. Re-enable when boa_engine
// upgrades its icu dep.
// #[cfg(feature = "animation")]
// pub mod js_binding;

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

/// A single animated property
#[derive(Debug, Clone)]
pub enum AnimatedProperty {
    /// Blur radius animation
    Blur { from: f32, to: f32 },
    /// Opacity animation (0.0 - 1.0)
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
    /// Interpolate the property at a given progress (0.0 - 1.0)
    pub fn interpolate(&self, progress: f32) -> EffectType {
        let t = progress.clamp(0.0, 1.0);
        match *self {
            AnimatedProperty::Blur { from, to } => {
                let _radius = from + (to - from) * t;
                EffectType::BackdropBlur
            }
            AnimatedProperty::Opacity { from, to } => {
                let _opacity = from + (to - from) * t;
                // Opacity would be handled via blend mode
                EffectType::BackdropBlur // Placeholder
            }
            AnimatedProperty::Scale { from, to } => {
                let _scale = from + (to - from) * t;
                EffectType::Transform2D
            }
            AnimatedProperty::Translate {
                from_x,
                from_y,
                to_x,
                to_y,
            } => {
                let _x = from_x + (to_x - from_x) * t;
                let _y = from_y + (to_y - from_y) * t;
                EffectType::Transform2D
            }
            AnimatedProperty::Rotate { from, to } => {
                let _degrees = from + (to - from) * t;
                EffectType::Transform2D
            }
            AnimatedProperty::Brightness { from, to } => {
                let _brightness = from + (to - from) * t;
                EffectType::ColorAdjust
            }
        }
    }

    /// Get the effect at the current progress
    pub fn to_effect(&self, selector: &str, viewport: (u32, u32), progress: f32) -> Effect {
        let t = progress.clamp(0.0, 1.0);
        match *self {
            AnimatedProperty::Blur { from, to } => {
                let radius = from + (to - from) * t;
                Effect::blur(selector, radius, viewport.0, viewport.1)
            }
            AnimatedProperty::Scale { from, to } => {
                let scale = from + (to - from) * t;
                let params = TransformParams {
                    scale_x: scale,
                    scale_y: scale,
                    ..Default::default()
                };
                Effect::transform(selector, params, viewport.0, viewport.1)
            }
            AnimatedProperty::Translate {
                from_x,
                from_y,
                to_x,
                to_y,
            } => {
                let x = from_x + (to_x - from_x) * t;
                let y = from_y + (to_y - from_y) * t;
                let params = TransformParams {
                    translate_x: x,
                    translate_y: y,
                    ..Default::default()
                };
                Effect::transform(selector, params, viewport.0, viewport.1)
            }
            AnimatedProperty::Rotate { from, to } => {
                let degrees = from + (to - from) * t;
                let params = TransformParams {
                    rotate_degrees: degrees,
                    ..Default::default()
                };
                Effect::transform(selector, params, viewport.0, viewport.1)
            }
            AnimatedProperty::Brightness { from, to } => {
                let brightness = from + (to - from) * t;
                let params = ColorAdjustParams {
                    red_multiplier: brightness,
                    green_multiplier: brightness,
                    blue_multiplier: brightness,
                    ..Default::default()
                };
                Effect::color_adjust(selector, params)
            }
            _ => Effect::blur(selector, 0.0, viewport.0, viewport.1),
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
