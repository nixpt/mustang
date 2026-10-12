//! `blur_gpu` example — the FULL pipeline demonstration, end to end:
//!
//! 1. Construct a `MustangCompositor` and declare both effect categories:
//!    a scene-native `BackdropBlur` (top half) AND a deferred `ColorAdjust`
//!    (bottom half) — the latter is what `apply_scene_effects` returns in
//!    `deferred_effects`.
//! 2. Rasterize through the REAL GPU pipeline: `VelloImageRenderer` renders
//!    the scene with vello-on-wgpu into a texture and downloads the
//!    premultiplied RGBA8 buffer — no procedural pixel math, actual GPU
//!    rasterization (this is the follow-on that `examples/blur.rs` deferred).
//! 3. Post-process the deferred effects over that buffer through the GPU
//!    compute engine (`GpuEffectEngine`) — the WGSL color-math kernel runs
//!    on the rasterized output, exactly the flow a window renderer would use.
//! 4. Assert pixel ground truth (interior pixels, away from AA edges) and
//!    emit `target/blur-gpu-demo.png`.
//!
//! Run: `cargo run -p arniko-mustang --example blur_gpu --features gpu`
//! Test: `cargo test -p arniko-mustang --features gpu --examples blur_gpu`
//!
//! Skips gracefully (exit 0 + notice) when no wgpu adapter exists — headless
//! CI runners without Vulkan/GL keep every other gate intact.

use std::io::BufWriter;

#[cfg(feature = "gpu")]
fn adapter_available() -> bool {
    use wgpu::InstanceDescriptor;
    let instance = wgpu::Instance::new(&InstanceDescriptor::default());
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .is_ok()
}

#[cfg(feature = "gpu")]
fn run_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    use anyrender::ImageRenderer;
    use anyrender::Paint;
    use anyrender::PaintScene;
    use kurbo::{Affine, Circle, Rect};
    use mustang::{
        Effect, GpuEffectEngine, MustangCompositor, MustangConfig, Region, VelloImageRenderer,
    };
    use peniko::color::palette::css;
    use peniko::Fill;

    let width = 400u32;
    let height = 400u32;

    // 0. Adapter probe — the renderer and the compute engine both need one.
    //    The `VelloImageRenderer` constructor `.expect()`s on a missing
    //    device, so skip BEFORE touching it (headless CI keeps exit 0).
    if !adapter_available() {
        eprintln!(
            "[blur-gpu] no wgpu adapter available — skipping (headless CI mode, exit 0)"
        );
        return Ok(());
    }

    // 1. Compositor + the two effect categories.
    let mut mustang = MustangCompositor::new(MustangConfig::default());
    let blur = Effect::blur("gpu-demo", 20.0_f32, width, height)
        .with_region(Region::new(0.0, 0.0, width as f32, (height / 2) as f32));
    let brighten = Effect::color_adjust(
        "gpu-demo",
        crate_color_adjust(0.5_f32),
    )
    .with_region(Region::new(
        0.0,
        (height / 2) as f32,
        width as f32,
        (height / 2) as f32,
    ));
    let effects = vec![blur, brighten];

    // 2. Real GPU rasterization. The draw closure paints the scene through
    //    the renderer's own painter (red bg + blue rect + green disc), then
    //    lets the compositor apply the scene-native effects to it; the
    //    deferred (GPU-compute) effects are captured for step 3.
    let mut deferred = Vec::new();
    let native_applied;
    let mut rgba = Vec::new();
    {
        let mut renderer = VelloImageRenderer::new(width, height);
        let mut deferred_out = Vec::new();
        let mut native_out = 0usize;
        renderer.render_to_vec(
            |painter| {
                // Full-canvas red background (makes every pixel opaque —
                // premultiply is a no-op, keeping the assertions exact).
                painter.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    Paint::Solid(css::RED),
                    None,
                    &Rect::new(0.0, 0.0, width as f64, height as f64),
                );
                // Blue centered rect.
                painter.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    Paint::Solid(css::BLUE),
                    None,
                    &Rect::new(80.0, 80.0, 320.0, 320.0),
                );
                // Green disc on lower-left.
                painter.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    Paint::Solid(css::GREEN),
                    None,
                    &Circle::new((100.0, 320.0), 40.0),
                );

                let result = mustang.apply_scene_effects(painter, &effects, (width, height));
                native_out = result.native_applied;
                deferred_out = result.deferred_effects;
            },
            &mut rgba,
        );
        native_applied = native_out;
        deferred = deferred_out;
    }
    eprintln!(
        "[blur-gpu] rasterized: native_applied={} deferred={}",
        native_applied,
        deferred.len()
    );
    assert_eq!(native_applied, 1, "BackdropBlur is scene-native");
    assert_eq!(deferred.len(), 1, "ColorAdjust defers to GPU compute");

    // 3. GPU-compute post-processing over the rasterized buffer — the same
    //    call a window renderer's present loop would make.
    let engine = GpuEffectEngine::try_new()
        .ok_or("adapter vanished between probe and compute engine — skipping")?;
    let applied = engine.apply_deferred_effects(width, height, &mut rgba, &deferred)?;
    eprintln!("[blur-gpu] gpu-compute applied {} deferred effect(s)", applied);
    assert_eq!(applied, 1, "the deferred ColorAdjust must be applied");

    // 4. Pixel ground truth. Interior pixels only (AA edges excluded); the
    //    GPU wrote these — the assertions pin the pipeline end to end.
    let px = |x: u32, y: u32| {
        let off = ((y as usize) * (width as usize) + (x as usize)) * 4;
        (rgba[off], rgba[off + 1], rgba[off + 2], rgba[off + 3])
    };
    let near = |a: u8, b: u8, tol: i32| (a as i32 - b as i32).abs() <= tol;

    // Bottom half — brightness(0.5) applied by the WGSL kernel:
    //   red bg (255,0,0) → (128,0,0); blue rect (0,0,255) → (0,0,128);
    //   green disc (0,128,0) → (0,64,0). (0.5·255 = 127.5 → rounds to 128.)
    let (r, g, b, a) = px(20, height - 20);
    assert!(near(r, 128, 3) && g <= 3 && b <= 3 && a == 255,
        "bottom red bg should be brightness-halved, got ({r},{g},{b},{a})");
    let (r, g, b, a) = px(200, 300);
    assert!(r <= 3 && g <= 3 && near(b, 128, 3) && a == 255,
        "bottom blue rect should be brightness-halved, got ({r},{g},{b},{a})");
    let (r, g, b, a) = px(100, 320);
    assert!(r <= 3 && near(g, 64, 4) && b <= 3 && a == 255,
        "green disc (bottom half) should be brightness-halved, got ({r},{g},{b},{a})");

    // Top half — scene-native blur path (white-tinted multi-pass box shadow):
    // red bg stays red-dominant but is lifted off pure (255,0,0).
    let (r, g, b, a) = px(20, 20);
    assert!(r >= 240 && (20..=110).contains(&g) && (20..=110).contains(&b) && a == 255,
        "top red bg should carry the scene-native blur tint, got ({r},{g},{b},{a})");
    // Sharpness boundary: blue rect in the TOP half (row 150, inside the
    // rect and clear of the blur-region shadow edge) is tinted but NOT
    // brightness-halved — blue stays dominant.
    let (r, g, b, a) = px(200, 150);
    assert!(b >= 200 && r <= 110 && g <= 110 && a == 255,
        "top blue rect must be untouched by the bottom-half color adjust, got ({r},{g},{b},{a})");
    // And the row just BELOW the boundary (y=201) IS color-adjusted: blue
    // halves to 128. (r/g carry the scene-native blur shadow's rim spill
    // past the region edge, composited before the halving — real
    // cross-effect overlap, not a bug; the clean-halved ground truth is
    // pinned at (200,300) above.)
    let (r, g, b, a) = px(200, 201);
    assert!(near(b, 128, 3) && r <= 60 && g <= 60 && a == 255,
        "row 201 is inside the color-adjust region, got ({r},{g},{b},{a})");

    // 5. PNG emit.
    let path = std::path::Path::new("target/blur-gpu-demo.png");
    std::fs::create_dir_all("target")?;
    let file = std::fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(&mut writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(&rgba)?;
    eprintln!(
        "[blur-gpu] wrote {} ({} bytes RGBA)",
        path.display(),
        rgba.len()
    );

    Ok(())
}

#[cfg(feature = "gpu")]
/// brightness(<amt>) params without importing ColorAdjustParams at the top
/// (keeps the `use mustang::` list focused on the pipeline surface).
fn crate_color_adjust(brightness: f32) -> mustang::ColorAdjustParams {
    mustang::ColorAdjustParams {
        red_multiplier: brightness,
        green_multiplier: brightness,
        blue_multiplier: brightness,
        ..Default::default()
    }
}

#[cfg(feature = "gpu")]
fn main() {
    match run_pipeline() {
        Ok(()) => eprintln!("[blur-gpu] success"),
        Err(e) => {
            eprintln!("[blur-gpu] ERROR: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "gpu")]
#[test]
fn test_blur_gpu_example_end_to_end() {
    run_pipeline().expect("blur_gpu pipeline should succeed (or skip on headless CI)");
}