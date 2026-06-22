//! 🐎 Mustang - GPU-Accelerated Effect Compositor for Exosphere
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! Mustang transforms CSS synthetic effects (backdrop-filter, transforms)
//! into GPU-native Vello operations, eliminating CPU pixel processing.
//!
//! # Architecture
//!
//! 1. **Scene-Native Effects**: Applied directly to Vello scenes (blur, transforms)
//! 2. **GPU Compute Effects**: Custom wgpu shaders for complex operations
//! 3. **Zero-Copy Composition**: Scenes composed directly into window renderer
//!
//! # Mustang - GPU-Accelerated Compositor
//!
//! Mustang provides a high-performance, GPU-accelerated effect compositor
//! built on top of Vello. It enables real-time visual effects like blur,
//! transforms, and color adjustments.

pub mod animation;
pub mod compositor;
pub mod config;
pub mod effect;
pub mod renderer;
pub mod scheduler;

// Re-export main types from effect module
pub use effect::{
    ApplyEffect, BlurParams, BlurQuality, CanonicalFilter, ColorAdjustParams, DropShadowParams,
    Effect, EffectType, TransformParams,
};
// Re-export Region from compositor
pub use animation::{
    AnimatedProperty, Animation, AnimationConfig, AnimationEngine, EasingFunction,
};
pub use compositor::region::Region;
pub use config::{MustangConfig, MustangMode};

// js_binding (boa_engine JS runtime) re-export deferred — companion
// carve-out lives at src/animation/mod.rs:15 (where `pub mod js_binding;`
// was commented out under the same s306 rationale). The boa_engine JS
// runtime is deferred because boa_engine 0.21 pins icu_normalizer ~2.0.0,
// which conflicts with parley ^0.10's requirement on ^2.1.1; the two
// cannot coexist in a single Cargo resolve graph. Re-enable when
// boa_engine ships an icu_normalizer release that resolves the ^2.1.1
// requirement. When re-enabling, uncomment BOTH this re-export AND
// the corresponding `pub mod js_binding;` at src/animation/mod.rs:15
// simultaneously.
// #[cfg(feature = "animation")]
// pub use animation::js_binding::JsAnimationRuntime;

#[cfg(feature = "gpu")]
pub use renderer::{
    EffectScene, MustangSceneBundle, VelloRendererOptions, VelloScenePainter, VelloWindowRenderer,
};

pub use compositor::*;
pub use scheduler::SceneScheduler;

use std::collections::HashMap;

/// Result of applying scene effects
#[derive(Debug, Clone)]
pub struct SceneEffectResult {
    /// Number of native effects applied
    pub native_applied: usize,
    /// Effects that need GPU compute
    pub deferred_effects: Vec<Effect>,
}

impl SceneEffectResult {
    /// Returns true if all effects were applied natively
    pub fn is_complete(&self) -> bool {
        self.deferred_effects.is_empty()
    }

    /// Returns the number of deferred effects
    pub fn deferred_count(&self) -> usize {
        self.deferred_effects.len()
    }
}

// Re-export types from compositor for metadata integration
pub use compositor::{FeatureType, SyntheticFeature};

/// Trait for extracting effects from metadata
///
/// Implement this trait to provide a bridge between your CSS/metadata system
/// and Mustang's effect system.
pub trait EffectMetadata {
    /// Extract synthetic features that can be converted to effects
    fn extract_features(&self) -> Vec<SyntheticFeature>;
}

/// The Mustang GPU Compositor
///
/// Transforms CSS synthetic effects into hardware-accelerated GPU operations.
/// Works with any PaintScene implementation.
pub struct MustangCompositor {
    config: MustangConfig,
    effect_cache: HashMap<String, Vec<Effect>>,
}

impl MustangCompositor {
    /// Create a new Mustang compositor with the given configuration
    pub fn new(config: MustangConfig) -> Self {
        Self {
            config,
            effect_cache: HashMap::new(),
        }
    }

    /// Apply effects to a scene
    ///
    /// This is the primary entry point for scene-native effect application.
    /// Effects are applied in-order, with proper layer management.
    #[cfg(feature = "gpu")]
    pub fn apply_scene_effects<S: anyrender::PaintScene>(
        &mut self,
        scene: &mut S,
        effects: &[Effect],
        viewport: (u32, u32),
    ) -> SceneEffectResult {
        let mut native_applied = 0;
        let mut deferred = Vec::new();

        for effect in effects {
            if effect.is_native() {
                // Apply scene-native effect immediately
                effect.apply_to_scene(scene, viewport);
                native_applied += 1;
            } else {
                // Defer non-native effects for GPU processing
                deferred.push(effect.clone());
            }
        }

        SceneEffectResult {
            native_applied,
            deferred_effects: deferred,
        }
    }

    /// Extract effects from metadata using the EffectMetadata trait
    pub fn extract_effects(
        &self,
        metadata: &dyn EffectMetadata,
        viewport: (u32, u32),
    ) -> Vec<Effect> {
        let features = metadata.extract_features();
        compositor::features_to_effects(features.as_slice(), viewport.0, viewport.1)
    }

    /// Check if an effect can be applied scene-natively
    pub fn is_effect_native(&self, effect: &Effect) -> bool {
        effect.is_native()
    }

    /// Get the current configuration
    pub fn config(&self) -> &MustangConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: MustangConfig) {
        self.config = config;
    }

    /// Cache effects for a component
    pub fn cache_effects(&mut self, component_id: &str, effects: Vec<Effect>) {
        self.effect_cache.insert(component_id.to_string(), effects);
    }

    /// Get cached effects for a component
    pub fn get_cached_effects(&self, component_id: &str) -> Option<&Vec<Effect>> {
        self.effect_cache.get(component_id)
    }

    /// Clear effect cache
    pub fn clear_cache(&mut self) {
        self.effect_cache.clear();
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> MustangStats {
        MustangStats {
            cached_components: self.effect_cache.len(),
            mode: self.config.mode,
        }
    }
}

impl Default for MustangCompositor {
    fn default() -> Self {
        Self::new(MustangConfig::default())
    }
}

/// Performance statistics for Mustang
#[derive(Debug, Clone)]
pub struct MustangStats {
    /// Number of cached components
    pub cached_components: usize,
    /// Current processing mode
    pub mode: MustangMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mustang_compositor_new() {
        let mustang = MustangCompositor::default();
        assert_eq!(mustang.config().mode, MustangMode::GpuAccelerated);
    }

    #[test]
    fn test_mustang_compositor_config() {
        let config = MustangConfig::new().mode(MustangMode::CpuOnly);
        let mustang = MustangCompositor::new(config);
        assert_eq!(mustang.config().mode, MustangMode::CpuOnly);
    }

    #[test]
    fn test_effect_caching() {
        let mut mustang = MustangCompositor::default();
        let effects = vec![Effect::blur(".test", 10.0, 800, 600)];

        mustang.cache_effects("test-component", effects.clone());
        let cached = mustang.get_cached_effects("test-component");

        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }

    #[test]
    fn test_scene_effect_result() {
        let result = SceneEffectResult {
            native_applied: 2,
            deferred_effects: vec![],
        };

        assert!(result.is_complete());
        assert_eq!(result.deferred_count(), 0);
    }

    #[test]
    fn test_mustang_stats() {
        let mustang = MustangCompositor::default();
        let stats = mustang.get_stats();

        assert_eq!(stats.cached_components, 0);
        assert_eq!(stats.mode, MustangMode::GpuAccelerated);
    }

    struct TestMetadata;
    impl EffectMetadata for TestMetadata {
        fn extract_features(&self) -> Vec<SyntheticFeature> {
            vec![SyntheticFeature {
                feature_type: FeatureType::BackdropFilter,
                selector: ".test".to_string(),
                original_value: "blur(10px)".to_string(),
            }]
        }
    }

    #[test]
    fn test_effect_metadata_trait() {
        let metadata = TestMetadata;
        let mustang = MustangCompositor::default();
        let effects = mustang.extract_effects(&metadata, (800, 600));

        assert_eq!(effects.len(), 1);
    }
}
