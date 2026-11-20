//! Applying effects straight to a `vello::Scene`, for hosts (like noonoo)
//! that compose a raw Vello scene and have no anyrender painter.
#![cfg(feature = "gpu")]

use mustang::{Effect, MustangCompositor, MustangConfig, Region, VelloScenePainter};
use vello::Scene;

fn effects() -> Vec<Effect> {
    vec![Effect::blur("panel", 20.0, 800, 600).with_region(Region::new(10.0, 10.0, 200.0, 100.0))]
}

#[test]
fn effects_apply_to_a_vello_scene() {
    let mut compositor = MustangCompositor::new(MustangConfig::default());
    let mut scene = Scene::new();
    let result = compositor.apply_vello_scene_effects(&mut scene, &effects(), (800, 600));
    assert_eq!(result.native_applied, 1);
    assert!(result.is_complete());
    assert!(scene.encoding().n_paths > 0, "the blur drew into the scene");
}

#[test]
fn a_vello_scene_gets_the_same_encoding_as_the_painter_path() {
    let mut compositor = MustangCompositor::new(MustangConfig::default());
    let mut direct = Scene::new();
    compositor.apply_vello_scene_effects(&mut direct, &effects(), (800, 600));

    let mut painted = Scene::new();
    compositor.apply_scene_effects(
        &mut VelloScenePainter::new(&mut painted),
        &effects(),
        (800, 600),
    );

    assert_eq!(direct.encoding().n_paths, painted.encoding().n_paths);
    let tags = |scene: &Scene| {
        scene
            .encoding()
            .draw_tags
            .iter()
            .map(|tag| tag.0)
            .collect::<Vec<_>>()
    };
    assert_eq!(tags(&direct), tags(&painted));
}
