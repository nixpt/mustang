//! gpu_blur — minimal example wiring up a Mustang compositor.
//!
//! Demonstrates the metadata → effect extraction pipeline. Runs headless
//! (no GPU device required); for a full windowed render with Vello/wgpu
//! integration, add `--features gpu` and use `EffectScene` on a
//! `VelloScenePainter`.
//!
//! Run with:  cargo run --example gpu_blur

use mustang::{
    EffectMetadata, FeatureType, MustangCompositor, MustangConfig, MustangMode, SyntheticFeature,
};

struct StaticMetadata;

impl EffectMetadata for StaticMetadata {
    fn extract_features(&self) -> Vec<SyntheticFeature> {
        vec![
            SyntheticFeature {
                feature_type: FeatureType::BackdropFilter,
                selector: ".glass".to_string(),
                original_value: "backdrop-filter: blur(10px) brightness(1.2)".to_string(),
            },
            SyntheticFeature {
                feature_type: FeatureType::Transform,
                selector: ".card".to_string(),
                original_value: "transform: scale(1.1) translate(10px, 20px) rotate(45deg)"
                    .to_string(),
            },
        ]
    }
}

fn main() {
    let config = MustangConfig::new()
        .mode(MustangMode::GpuAccelerated)
        .enable_caching(true);

    let compositor = MustangCompositor::new(config);

    let effects = compositor.extract_effects(&StaticMetadata, (800, 600));

    println!("Mustang extracted {} effect(s):", effects.len());
    for e in &effects {
        println!(
            "  {:>13}  selector={:<10}  z={:>4}  region=({},{},{},{})",
            format!("{:?}", e.effect_type),
            e.selector,
            e.z_index,
            e.region.x,
            e.region.y,
            e.region.width,
            e.region.height,
        );
    }
}
