//! GPU compute path for the deferred effects — [`EffectType::ColorAdjust`]
//! and [`EffectType::CanonicalFilter`].
//!
//! Copyright (c) 2026 The Exosphere Authors
//!
//! Dual-licensed under MIT or Apache-2.0.
//!
//! These two effect kinds are per-pixel color math: they cannot be expressed
//! as Vello scene primitives, so they were declared "deferred to the GPU
//! compute path" (the `CustomPaintSource` note in `src/effect.rs`) until this
//! module landed. It implements that path.
//!
//! # Layout
//!
//! - [`ColorOp`] — the op-kind + parameters extracted from an [`Effect`]
//!   (feature-agnostic, no deps).
//! - [`cpu_apply_color_op`] — CPU reference implementation of the same math
//!   on a premultiplied RGBA8 buffer. This is the test oracle for the GPU
//!   kernel AND the software fallback when no adapter is available (headless
//!   CI, llvmpipe-less environments).
//! - [`COLOR_MATH_WGSL`] — the WGSL compute kernel, single source of truth
//!   for the GPU side. Validated with naga in tests (no hardware needed).
//! - [`GpuEffectEngine`] (requires the `gpu` feature) — owns a
//!   `wgpu::Device`/`Queue`/compute pipeline and applies color ops over a
//!   region of a premultiplied RGBA8 buffer (upload → dispatch → download).
//!
//! # Color semantics (CSS filter shorthand, sRGB space)
//!
//! All ops operate on **non-premultiplied** colors, per the Filter Effects
//! spec ("Filter Functions must operate in the sRGB color space"). Since
//! Vello rasters premultiplied, both implementations
//! un-premultiply → apply the op → re-premultiply. Fully-transparent pixels
//! stay fully transparent (alpha is never touched by these ops).
//!
//! Op formulas (θ = degrees for hue-rotate, a = amount otherwise):
//!
//! - `ColorAdjust`: `rgb·multipliers + offsets` (mustang-native op)
//! - `hue-rotate(θ)`: the spec matrix `base + cos(θ)·Mcos + sin(θ)·Msin`
//!   (constants verbatim from filter-effects-1 §feColorMatrix)
//! - `saturate(a)`: the spec matrix `base + a·Mcos`
//! - `brightness(a)`: `rgb·a`
//! - `contrast(a)`: `(rgb − 0.5)·a + 0.5`
//! - `grayscale(a)`: `mix(rgb, luminance(rgb), a)` — clamped to 0..=1
//! - `invert(a)`: `mix(rgb, 1 − rgb, a)` — clamped to 0..=1
//!
//! # Renderer integration
//!
//! The compositor (`apply_scene_effects`) returns non-native effects in
//! `SceneEffectResult::deferred_effects` because pixel math cannot happen at
//! scene-encode time. The consumer flow is: rasterize the Vello scene into
//! an intermediate RGBA8 buffer (or texture), then run
//! [`GpuEffectEngine::apply_deferred_effects`] over it. Wiring the engine
//! into a concrete window render loop is the renderer-side follow-up; the
//! engine here is deliberately backend-agnostic about where the buffer came
//! from.
//!
//! # First-cut note
//!
//! The kernel currently reads/writes **flat f32 storage buffers** rather
//! than rgba8unorm storage textures: parity with the CPU oracle is exact
//! (one rounding point on the host) and support is universal. Switching to
//! rgba8unorm `read_write` storage textures is a straightforward bandwidth
//! optimization once the renderer integration is proven out.

use crate::compositor::region::Region;
use crate::effect::{CanonicalFilter, Effect, EffectType};

// ---------------------------------------------------------------------------
// ColorOp — the feature-agnostic op + params surface
// ---------------------------------------------------------------------------

/// A color-math op extracted from an [`Effect`] — the payload the CPU
/// reference and the GPU kernel both consume.
///
/// `mode()` is the WGSL kernel dispatch value (`ColorOp::to_mode` in the
/// uniform); the enum itself is the ergonomic Rust-side surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorOp {
    /// [`EffectType::ColorAdjust`] — `rgb·mults + offsets`.
    ColorAdjust {
        mults: [f32; 3],
        offsets: [f32; 3],
    },
    /// `filter: hue-rotate(<deg>deg)` — degrees, NOT normalized (spec:
    /// "Implementations must not normalize this value").
    HueRotate(f32),
    /// `filter: saturate(<amt>)`.
    Saturate(f32),
    /// `filter: brightness(<amt>)`.
    Brightness(f32),
    /// `filter: contrast(<amt>)`.
    Contrast(f32),
    /// `filter: grayscale(<amt>)` — clamped to 0..=1 (spec: UAs must clamp).
    Grayscale(f32),
    /// `filter: invert(<amt>)` — clamped to 0..=1 (spec: UAs must clamp).
    Invert(f32),
}

impl ColorOp {
    /// Extract the color op from an effect. Returns `None` for
    /// scene-native kinds or when the per-kind params are missing
    /// (defensive — the constructors always set them).
    pub fn from_effect(effect: &Effect) -> Option<Self> {
        match effect.effect_type {
            EffectType::ColorAdjust => {
                let p = effect.color_params.as_ref()?;
                Some(Self::ColorAdjust {
                    mults: [
                        p.red_multiplier,
                        p.green_multiplier,
                        p.blue_multiplier,
                    ],
                    offsets: [p.red_offset, p.green_offset, p.blue_offset],
                })
            }
            EffectType::CanonicalFilter => {
                let f = effect.canonical_filter_params.as_ref()?;
                Some(match *f {
                    CanonicalFilter::HueRotate(d) => Self::HueRotate(d),
                    CanonicalFilter::Saturate(a) => Self::Saturate(a),
                    CanonicalFilter::Brightness(a) => Self::Brightness(a),
                    CanonicalFilter::Contrast(a) => Self::Contrast(a),
                    CanonicalFilter::Grayscale(a) => Self::Grayscale(a.clamp(0.0, 1.0)),
                    CanonicalFilter::Invert(a) => Self::Invert(a.clamp(0.0, 1.0)),
                })
            }
            _ => None,
        }
    }

    /// WGSL kernel dispatch mode (`0u..=6u` — see `COLOR_MATH_WGSL`).
    pub fn mode(&self) -> u32 {
        match self {
            Self::ColorAdjust { .. } => 0,
            Self::HueRotate(_) => 1,
            Self::Saturate(_) => 2,
            Self::Brightness(_) => 3,
            Self::Contrast(_) => 4,
            Self::Grayscale(_) => 5,
            Self::Invert(_) => 6,
        }
    }

    /// The op amount — degrees for hue-rotate, the multiplier otherwise.
    /// Unused (0.0) for `ColorAdjust`.
    pub fn amount(&self) -> f32 {
        match self {
            Self::ColorAdjust { .. } => 0.0,
            Self::HueRotate(a)
            | Self::Saturate(a)
            | Self::Brightness(a)
            | Self::Contrast(a)
            | Self::Grayscale(a)
            | Self::Invert(a) => *a,
        }
    }
}

// ---------------------------------------------------------------------------
// Region pixel bounds — shared clamping helper (CPU + GPU must agree)
// ---------------------------------------------------------------------------

/// Clamp a region to integer pixel bounds `(x0, y0, x1_exclusive, y1_exclusive)`.
///
/// Shared by the CPU reference and the GPU engine so both agree on exactly
/// which pixels an op touches. An empty/degenerate region yields an empty
/// range (`x1 <= x0` or `y1 <= y0`).
pub fn region_pixel_bounds(region: &Region, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x0 = region.x.floor().max(0.0) as u32;
    let y0 = region.y.floor().max(0.0) as u32;
    let x1 = (region.x + region.width).ceil().min(width as f32) as u32;
    let y1 = (region.y + region.height).ceil().min(height as f32) as u32;
    (x0.min(width), y0.min(height), x1.min(width), y1.min(height))
}

// ---------------------------------------------------------------------------
// CPU reference implementation (test oracle + software fallback)
// ---------------------------------------------------------------------------

/// Apply a color op to a region of a **premultiplied** RGBA8 buffer.
///
/// Buffer layout: `width * height * 4` bytes, row-major, `[r, g, b, a]`
/// premultiplied (Vello's raster output convention). Pixels outside the
/// region are untouched. Empty regions are a no-op.
///
/// This is the same math as `COLOR_MATH_WGSL`, on the CPU — the oracle the
/// GPU path is tested against and the fallback when no adapter exists.
pub fn cpu_apply_color_op(
    buf: &mut [u8],
    width: u32,
    height: u32,
    region: &Region,
    op: &ColorOp,
) {
    let (x0, y0, x1, y1) = region_pixel_bounds(region, width, height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * width + x) * 4) as usize;

            // Un-premultiply: CSS filter math is non-premultiplied sRGB.
            let a = buf[i + 3] as f32 / 255.0;
            let (mut r, mut g, mut b) = if a > 0.0 {
                (
                    buf[i] as f32 / 255.0 / a,
                    buf[i + 1] as f32 / 255.0 / a,
                    buf[i + 2] as f32 / 255.0 / a,
                )
            } else {
                (0.0, 0.0, 0.0)
            };

            apply_op_in_place(&mut r, &mut g, &mut b, op);

            // Clamp in non-premultiplied space FIRST (the spec clamps the
            // filter result before any compositing), then re-premultiply
            // and quantize (single rounding point).
            let cl = |v: f32| v.clamp(0.0, 1.0);
            let q = |v: f32| (v * 255.0).round() as u8;
            buf[i] = q(cl(r) * a);
            buf[i + 1] = q(cl(g) * a);
            buf[i + 2] = q(cl(b) * a);
            // Alpha is never modified by these ops.
        }
    }
}

/// The op math in non-premultiplied sRGB space — in place, unclamped.
///
/// Mirrored 1:1 by `apply_color_math` in `COLOR_MATH_WGSL`; the shader's
/// spec-matrix constants must stay in sync with the ones here.
fn apply_op_in_place(r: &mut f32, g: &mut f32, b: &mut f32, op: &ColorOp) {
    match *op {
        ColorOp::ColorAdjust { mults, offsets } => {
            *r = *r * mults[0] + offsets[0];
            *g = *g * mults[1] + offsets[1];
            *b = *b * mults[2] + offsets[2];
        }
        ColorOp::HueRotate(degrees) => {
            // Filter-effects-1 §feColorMatrix, type="hueRotate":
            //   M = base + cos(θ)·Mcos + sin(θ)·Msin
            // (constants verbatim from the spec — do not round further)
            let c = degrees.to_radians().cos();
            let s = degrees.to_radians().sin();
            let col = |base: f32, cos_c: f32, sin_c: f32| {
                vec3_apply(base, cos_c, sin_c, c, s)
            };
            let m00 = col(0.213, 0.787, -0.213);
            let m01 = col(0.715, -0.715, -0.715);
            let m02 = col(0.072, -0.072, 0.928);
            let m10 = col(0.213, -0.213, 0.143);
            let m11 = col(0.715, 0.285, 0.140);
            let m12 = col(0.072, -0.072, -0.283);
            let m20 = col(0.213, -0.213, -0.787);
            let m21 = col(0.715, -0.715, 0.715);
            let m22 = col(0.072, 0.928, 0.072);
            let (ir, ig, ib) = (*r, *g, *b);
            *r = m00 * ir + m01 * ig + m02 * ib;
            *g = m10 * ir + m11 * ig + m12 * ib;
            *b = m20 * ir + m21 * ig + m22 * ib;
        }
        ColorOp::Saturate(a) => {
            // Filter-effects-1 §feColorMatrix, type="saturate":
            //   M = base + a·Mcos (same constant matrix as hue-rotate's
            //   cos component)
            let m00 = 0.213 + a * 0.787;
            let m01 = 0.715 - a * 0.715;
            let m02 = 0.072 - a * 0.072;
            let m10 = 0.213 - a * 0.213;
            let m11 = 0.715 + a * 0.285;
            let m12 = 0.072 - a * 0.072;
            let m20 = 0.213 - a * 0.213;
            let m21 = 0.715 - a * 0.715;
            let m22 = 0.072 + a * 0.928;
            let (ir, ig, ib) = (*r, *g, *b);
            *r = m00 * ir + m01 * ig + m02 * ib;
            *g = m10 * ir + m11 * ig + m12 * ib;
            *b = m20 * ir + m21 * ig + m22 * ib;
        }
        ColorOp::Brightness(a) => {
            *r *= a;
            *g *= a;
            *b *= a;
        }
        ColorOp::Contrast(a) => {
            *r = (*r - 0.5) * a + 0.5;
            *g = (*g - 0.5) * a + 0.5;
            *b = (*b - 0.5) * a + 0.5;
        }
        ColorOp::Grayscale(a) => {
            let lum = luminance(*r, *g, *b);
            *r += (lum - *r) * a;
            *g += (lum - *g) * a;
            *b += (lum - *b) * a;
        }
        ColorOp::Invert(a) => {
            *r += (1.0 - 2.0 * *r) * a;
            *g += (1.0 - 2.0 * *g) * a;
            *b += (1.0 - 2.0 * *b) * a;
        }
    }
}

/// One row-entry of the hue-rotate matrix: `base + cos(θ)·cos_c + sin(θ)·sin_c`.
fn vec3_apply(base: f32, cos_c: f32, sin_c: f32, c: f32, s: f32) -> f32 {
    base + c * cos_c + s * sin_c
}

/// sRGB luminance — the basis of CSS `grayscale()`/`sepia()` shorthand.
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

// ---------------------------------------------------------------------------
// WGSL kernel — single source of truth for the GPU side
// ---------------------------------------------------------------------------

/// The WGSL color-math compute kernel.
///
/// One kernel, mode-dispatched (see [`ColorOp::mode`]):
/// `0`=ColorAdjust, `1`=HueRotate, `2`=Saturate, `3`=Brightness,
/// `4`=Contrast, `5`=Grayscale, `6`=Invert.
///
/// Binding model: uniform params + two flat f32 storage buffers
/// (premultiplied RGBA in/out — one `vec4<f32>` per pixel, row-major).
/// The kernel un-premultiplies, applies the op in sRGB space, clamps, and
/// re-premultiplies — exactly what [`cpu_apply_color_op`] does.
pub const COLOR_MATH_WGSL: &str = r#"
struct Params {
    mode: u32,        // dispatch value = ColorOp::mode()
    amount: f32,      // canonical op amount (hueRotate: degrees)
    pad0: f32,
    pad1: f32,
    mults: vec4<f32>,   // xyz = RGB multipliers (ColorAdjust)
    offsets: vec4<f32>, // xyz = RGB offsets (ColorAdjust)
    region: vec4<u32>,  // x, y, w, h — pixel bounds (w/h are the dispatch extent)
    dims: vec4<u32>,    // x = buffer width, y = buffer height (pixels)
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dst: array<vec4<f32>>;

fn luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// base + cos(θ)·cosC + sin(θ)·sinC — one matrix entry of hue-rotate.
fn hue_entry(base: f32, cos_c: f32, sin_c: f32, c: f32, s: f32) -> f32 {
    return base + c * cos_c + s * sin_c;
}

fn hue_rotate(rgb: vec3<f32>, degrees: f32) -> vec3<f32> {
    let c = cos(radians(degrees));
    let s = sin(radians(degrees));
    // Filter-effects-1 §feColorMatrix, type="hueRotate":
    //   M = base + cos(θ)·Mcos + sin(θ)·Msin
    // mat3x3 columns are the spec-matrix columns (row-wise application
    // below via `m * rgb`).
    let m = mat3x3<f32>(
        vec3<f32>(
            hue_entry(0.213,  0.787, -0.213, c, s),
            hue_entry(0.213, -0.213,  0.143, c, s),
            hue_entry(0.213, -0.213, -0.787, c, s),
        ),
        vec3<f32>(
            hue_entry(0.715, -0.715, -0.715, c, s),
            hue_entry(0.715,  0.285,  0.140, c, s),
            hue_entry(0.715, -0.715,  0.715, c, s),
        ),
        vec3<f32>(
            hue_entry(0.072, -0.072,  0.928, c, s),
            hue_entry(0.072, -0.072, -0.283, c, s),
            hue_entry(0.072,  0.928,  0.072, c, s),
        ),
    );
    return m * rgb;
}

fn saturate(rgb: vec3<f32>, a: f32) -> vec3<f32> {
    // Filter-effects-1 §feColorMatrix, type="saturate": base + a·Mcos
    let m = mat3x3<f32>(
        vec3<f32>(0.213 + a * 0.787, 0.213 - a * 0.213, 0.213 - a * 0.213),
        vec3<f32>(0.715 - a * 0.715, 0.715 + a * 0.285, 0.715 - a * 0.715),
        vec3<f32>(0.072 - a * 0.072, 0.072 - a * 0.072, 0.072 + a * 0.928),
    );
    return m * rgb;
}

fn apply_color_math(rgb: vec3<f32>, p: Params) -> vec3<f32> {
    switch p.mode {
        case 0u: { // ColorAdjust
            return rgb * p.mults.xyz + p.offsets.xyz;
        }
        case 1u: { // HueRotate
            return hue_rotate(rgb, p.amount);
        }
        case 2u: { // Saturate
            return saturate(rgb, p.amount);
        }
        case 3u: { // Brightness
            return rgb * p.amount;
        }
        case 4u: { // Contrast
            return (rgb - vec3<f32>(0.5)) * p.amount + vec3<f32>(0.5);
        }
        case 5u: { // Grayscale
            let lum = vec3<f32>(luminance(rgb));
            return mix(rgb, lum, p.amount);
        }
        case 6u: { // Invert
            return mix(rgb, vec3<f32>(1.0) - rgb, p.amount);
        }
        default: { // unreachable from ColorOp::mode() — pass through
            return rgb;
        }
    }
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    // gid is region-local; reject overshoot from the ceil-div dispatch.
    if (gid.x >= params.region.z || gid.y >= params.region.w) {
        return;
    }
    let px = params.region.x + gid.x;
    let py = params.region.y + gid.y;
    if (px >= params.dims.x || py >= params.dims.y) {
        return;
    }

    let idx = py * params.dims.x + px;
    let p_in = src[idx];
    // Un-premultiply → color math (sRGB) → clamp → re-premultiply.
    // Fully-transparent pixels keep rgb = 0 (premultiplied convention).
    let unpre = select(
        vec4<f32>(p_in.rgb / p_in.a, p_in.a),
        p_in,
        p_in.a <= 0.0,
    );
    let out_rgb = clamp(
        apply_color_math(unpre.rgb, params),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    dst[idx] = vec4<f32>(out_rgb * unpre.a, unpre.a);
}
"#;

// ---------------------------------------------------------------------------
// GPU engine (requires the `gpu` feature)
// ---------------------------------------------------------------------------

/// Errors from the GPU compute engine.
#[cfg(feature = "gpu")]
#[derive(Debug, Clone, PartialEq)]
pub enum GpuComputeError {
    /// The buffer is not exactly `width * height * 4` bytes.
    BufferLength { expected: usize, got: usize },
    /// The effect kind is not a color op (native effects belong to the
    /// scene-native path, not this engine).
    NotApplicable(EffectType),
    /// A wgpu runtime failure (device lost / mapping / submit).
    Runtime(String),
}

#[cfg(feature = "gpu")]
impl std::fmt::Display for GpuComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferLength { expected, got } => {
                write!(f, "buffer length {got} does not match {expected} (width*height*4)")
            }
            Self::NotApplicable(kind) => {
                write!(f, "effect kind {kind:?} is scene-native — apply it via apply_to_scene, not the GPU compute engine")
            }
            Self::Runtime(msg) => write!(f, "wgpu runtime failure: {msg}"),
        }
    }
}

#[cfg(feature = "gpu")]
impl std::error::Error for GpuComputeError {}

#[cfg(feature = "gpu")]
use wgpu::util::DeviceExt;

#[cfg(feature = "gpu")]
/// The GPU compute engine — applies [`ColorOp`]s over a region of a
/// premultiplied RGBA8 buffer through the `COLOR_MATH_WGSL` kernel.
///
/// Owns its `wgpu::Device` + `Queue` + compute pipeline. Construct with
/// [`GpuEffectEngine::try_new`] — returns `None` gracefully when no
/// suitable adapter exists (headless CI without Vulkan/GL), so callers can
/// fall back to [`cpu_apply_color_op`].
pub struct GpuEffectEngine {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

#[cfg(feature = "gpu")]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct ColorMathUniform {
    mode: u32,
    amount: f32,
    pad0: f32,
    pad1: f32,
    mults: [f32; 4],
    offsets: [f32; 4],
    region: [u32; 4],
    dims: [u32; 4],
}

#[cfg(feature = "gpu")]
impl GpuEffectEngine {
    /// Try to create the engine. `None` when no adapter is available —
    /// callers should fall back to [`cpu_apply_color_op`].
    pub fn try_new() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok()?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("mustang.gpu_effect_engine"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            },
        ))
        .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mustang.color_math.wgsl"),
            source: wgpu::ShaderSource::Wgsl(COLOR_MATH_WGSL.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("mustang.color_math.pipeline"),
            layout: None, // inferred from the shader's bindings
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Some(Self {
            device,
            queue,
            pipeline,
        })
    }

    /// The uniform payload for an op over a pixel-clamped region.
    fn uniform_for(op: &ColorOp, bounds: (u32, u32, u32, u32), width: u32, height: u32) -> ColorMathUniform {
        let (x0, y0, x1, y1) = bounds;
        let mults_offsets = match op {
            ColorOp::ColorAdjust { mults, offsets } => (*mults, *offsets),
            _ => ([1.0, 1.0, 1.0], [0.0, 0.0, 0.0]),
        };
        ColorMathUniform {
            mode: op.mode(),
            amount: op.amount(),
            pad0: 0.0,
            pad1: 0.0,
            mults: [
                mults_offsets.0[0],
                mults_offsets.0[1],
                mults_offsets.0[2],
                0.0,
            ],
            offsets: [
                mults_offsets.1[0],
                mults_offsets.1[1],
                mults_offsets.1[2],
                0.0,
            ],
            region: [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)],
            dims: [width, height, 0, 0],
        }
    }

    /// Apply a color op over `region` of a premultiplied RGBA8 buffer.
    ///
    /// Upload → compute dispatch → download, all synchronous (block_on).
    /// Buffer must be exactly `width * height * 4` bytes.
    pub fn apply_to_buffer(
        &self,
        width: u32,
        height: u32,
        buffer: &mut [u8],
        op: &ColorOp,
        region: &Region,
    ) -> Result<(), GpuComputeError> {
        let expected = (width as usize) * (height as usize) * 4;
        if buffer.len() != expected {
            return Err(GpuComputeError::BufferLength {
                expected,
                got: buffer.len(),
            });
        }

        let bounds = region_pixel_bounds(region, width, height);
        let (x0, y0, x1, y1) = bounds;
        if x1 <= x0 || y1 <= y0 {
            return Ok(()); // empty region — nothing to do
        }

        // Expand u8 RGBA → flat f32 vec4s (one upload, one download).
        let src: Vec<f32> = buffer
            .chunks_exact(4)
            .map(|px| [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0, px[3] as f32 / 255.0])
            .flatten()
            .collect();

        let uniform = Self::uniform_for(op, bounds, width, height);
        let uniform_bytes = bytemuck::bytes_of(&uniform);

        let device = &self.device;
        let src_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mustang.color_math.src"),
            contents: bytemuck::cast_slice(&src),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let dst_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mustang.color_math.dst"),
            size: (src.len() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            // Seeded with the source so pixels OUTSIDE the region survive
            // the round-trip untouched (the kernel only writes the region).
            mapped_at_creation: true,
        });
        dst_buf
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytemuck::cast_slice(&src));
        dst_buf.unmap();
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mustang.color_math.uniform"),
            contents: uniform_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mustang.color_math.bind_group"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: src_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: dst_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mustang.color_math.encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mustang.color_math.pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let gw = (x1 - x0).div_ceil(8);
            let gh = (y1 - y0).div_ceil(8);
            pass.dispatch_workgroups(gw, gh, 1);
        }

        // Read back dst_buf via a staging buffer (STORAGE buffers are not
        // directly mappable).
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mustang.color_math.staging"),
            size: dst_buf.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(&dst_buf, 0, &staging, 0, dst_buf.size());
        self.queue.submit(Some(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        staging.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuComputeError::Runtime(format!("device.poll: {e}")))?;
        rx.recv()
            .map_err(|e| GpuComputeError::Runtime(format!("map_async channel: {e}")))?
            .map_err(|e| GpuComputeError::Runtime(format!("map_async: {e}")))?;

        let mapped = staging.slice(..).get_mapped_range();
        let out: &[f32] = bytemuck::cast_slice(&mapped);
        for (px, quad) in buffer.chunks_exact_mut(4).zip(out.chunks_exact(4)) {
            let q = |v: f32| ((v.clamp(0.0, 1.0)) * 255.0).round() as u8;
            px[0] = q(quad[0]);
            px[1] = q(quad[1]);
            px[2] = q(quad[2]);
            px[3] = q(quad[3]);
        }
        drop(mapped);
        staging.unmap();

        Ok(())
    }

    /// Apply one effect's color op over its region. Errors with
    /// [`GpuComputeError::NotApplicable`] for scene-native kinds — those
    /// belong to `apply_to_scene`, not this engine.
    pub fn apply_effect(
        &self,
        width: u32,
        height: u32,
        buffer: &mut [u8],
        effect: &Effect,
    ) -> Result<(), GpuComputeError> {
        let op = ColorOp::from_effect(effect).ok_or(GpuComputeError::NotApplicable(effect.effect_type))?;
        self.apply_to_buffer(width, height, buffer, &op, &effect.region)
    }

    /// Apply every color op in a deferred-effects list (the
    /// `SceneEffectResult::deferred_effects` payload from the compositor),
    /// in order, over each effect's own region. Returns the number of
    /// effects applied; non-color entries are skipped.
    pub fn apply_deferred_effects(
        &self,
        width: u32,
        height: u32,
        buffer: &mut [u8],
        effects: &[Effect],
    ) -> Result<usize, GpuComputeError> {
        let mut applied = 0;
        for effect in effects {
            if let Some(op) = ColorOp::from_effect(effect) {
                self.apply_to_buffer(width, height, buffer, &op, &effect.region)?;
                applied += 1;
            }
        }
        Ok(applied)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::ColorAdjustParams;

    /// Opaque red pixel, premultiplied RGBA8.
    const RED: [u8; 4] = [255, 0, 0, 255];
    /// 50%-alpha blue, premultiplied: rgb = (0, 0, 255) * 0.5.
    const HALF_BLUE: [u8; 4] = [0, 0, 128, 128];
    /// Fully transparent black (premultiplied convention).
    const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

    fn fill_buf(pixels: &[[u8; 4]]) -> Vec<u8> {
        pixels.iter().flatten().copied().collect()
    }

    fn to_pixels(buf: &[u8]) -> Vec<[u8; 4]> {
        buf.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
    }

    #[test]
    fn test_color_op_from_effect() {
        let effect = Effect::color_adjust(
            ".x",
            ColorAdjustParams {
                red_multiplier: 2.0,
                ..Default::default()
            },
        );
        let op = ColorOp::from_effect(&effect).expect("ColorAdjust is a color op");
        assert_eq!(
            op,
            ColorOp::ColorAdjust {
                mults: [2.0, 1.0, 1.0],
                offsets: [0.0, 0.0, 0.0],
            }
        );
        assert_eq!(op.mode(), 0);

        let effect = Effect::canonical_filter(".x", CanonicalFilter::HueRotate(90.0), 10, 10);
        assert_eq!(ColorOp::from_effect(&effect), Some(ColorOp::HueRotate(90.0)));
        assert_eq!(ColorOp::from_effect(&effect).unwrap().mode(), 1);

        // Grayscale/Invert clamp to 0..=1 per the spec's "UAs must clamp".
        assert_eq!(
            ColorOp::from_effect(&Effect::canonical_filter(
                ".x",
                CanonicalFilter::Grayscale(2.5),
                10,
                10
            )),
            Some(ColorOp::Grayscale(1.0))
        );

        // Native kinds are not color ops.
        assert_eq!(
            ColorOp::from_effect(&Effect::blur(".x", 1.0, 10, 10)),
            None
        );
    }

    #[test]
    fn test_cpu_identity_ops_are_no_ops() {
        // Each op at its identity amount must leave every pixel unchanged.
        let buf_in = fill_buf(&[RED, HALF_BLUE, TRANSPARENT]);
        let identity = [
            ColorOp::HueRotate(0.0),
            ColorOp::Saturate(1.0),
            ColorOp::Brightness(1.0),
            ColorOp::Contrast(1.0),
            ColorOp::Grayscale(0.0),
            ColorOp::Invert(0.0),
        ];
        for op in &identity {
            let mut buf = buf_in.clone();
            cpu_apply_color_op(&mut buf, 3, 1, &Region::new(0.0, 0.0, 3.0, 1.0), op);
            assert_eq!(to_pixels(&buf), vec![RED, HALF_BLUE, TRANSPARENT], "{op:?} identity must be a no-op");
        }
    }

    #[test]
    fn test_cpu_brightness_and_color_adjust() {
        // brightness(0) → black (premultiplied keeps alpha).
        let mut buf = fill_buf(&[RED, HALF_BLUE]);
        cpu_apply_color_op(
            &mut buf,
            2,
            1,
            &Region::new(0.0, 0.0, 2.0, 1.0),
            &ColorOp::Brightness(0.0),
        );
        assert_eq!(to_pixels(&buf), vec![[0, 0, 0, 255], [0, 0, 0, 128]]);

        // brightness(2) doubles rgb (premultiplied, clamped at 255).
        let mut buf = fill_buf(&[[100, 0, 0, 255]]);
        cpu_apply_color_op(
            &mut buf,
            1,
            1,
            &Region::new(0.0, 0.0, 1.0, 1.0),
            &ColorOp::Brightness(2.0),
        );
        assert_eq!(to_pixels(&buf), vec![[200, 0, 0, 255]]);

        // ColorAdjust: rgb·mults + offsets, premultiplied path.
        let mut buf = fill_buf(&[HALF_BLUE]);
        cpu_apply_color_op(
            &mut buf,
            1,
            1,
            &Region::new(0.0, 0.0, 1.0, 1.0),
            &ColorOp::ColorAdjust {
                mults: [1.0, 1.0, 2.0],
                offsets: [0.5, 0.0, 0.0],
            },
        );
        // premultiplied in: (0, 0, 128, 128) → unpre (0, 0, 2.0) → math
        // (0.5, 0, 4.0) → clamp (0.5, 0, 1) → premult (0.5·0.502, 0, 1·0.502)
        // → (64, 0, 128, 128).
        assert_eq!(to_pixels(&buf), vec![[64, 0, 128, 128]]);
    }

    #[test]
    fn test_cpu_full_invert_and_grayscale() {
        // invert(1) → 255 − rgb.
        let mut buf = fill_buf(&[RED]);
        cpu_apply_color_op(
            &mut buf,
            1,
            1,
            &Region::new(0.0, 0.0, 1.0, 1.0),
            &ColorOp::Invert(1.0),
        );
        assert_eq!(to_pixels(&buf), vec![[0, 255, 255, 255]]);

        // grayscale(1) → luminance (0.2126·1 = 54).
        let mut buf = fill_buf(&[RED]);
        cpu_apply_color_op(
            &mut buf,
            1,
            1,
            &Region::new(0.0, 0.0, 1.0, 1.0),
            &ColorOp::Grayscale(1.0),
        );
        let px = to_pixels(&buf)[0];
        assert_eq!(px[0], px[1]);
        assert_eq!(px[1], px[2]);
        assert!((px[0] as f32 - 0.2126 * 255.0).abs() <= 1.0, "luminance of pure red: got {}", px[0]);
    }

    #[test]
    fn test_cpu_contrast_zero_is_mid_gray() {
        let mut buf = fill_buf(&[RED]);
        cpu_apply_color_op(
            &mut buf,
            1,
            1,
            &Region::new(0.0, 0.0, 1.0, 1.0),
            &ColorOp::Contrast(0.0),
        );
        let px = to_pixels(&buf)[0];
        // (r−0.5)·0+0.5 = 0.5 → 127.5 → rounds to 128.
        assert_eq!(px[0], 128);
        assert_eq!(px[1], 128);
        assert_eq!(px[2], 128);
    }

    #[test]
    fn test_cpu_hue_rotate_90() {
        // hue-rotate(90°) moves red toward green (spec matrix).
        let mut buf = fill_buf(&[RED]);
        cpu_apply_color_op(
            &mut buf,
            1,
            1,
            &Region::new(0.0, 0.0, 1.0, 1.0),
            &ColorOp::HueRotate(90.0),
        );
        let px = to_pixels(&buf)[0];
        // Spec matrix at 90°: r' = 0·r + (0.715−0.715·0−0.715)·g + (0.072−0.072·0+0.928)·b
        //                     g' = (0.213−0+0.143)·r + (0.715+0+0.140)·g + (0.072−0−0.283)·b
        //                     b' = (0.213−0−0.787)·r + (0.715−0+0.715)·g + (0.072+0+0.072)·b
        // For (r,g,b)=(1,0,0): (0, 0.356, −0.574) → clamped (0, 0.356, 0).
        assert_eq!(px[0], 0);
        assert_eq!(px[1], 91); // 0.356·255 = 90.78 → 91
        assert_eq!(px[2], 0);
        assert_eq!(px[3], 255);
    }

    #[test]
    fn test_cpu_region_isolation() {
        // Only the region's pixels change; outside pixels stay untouched.
        let mut buf = fill_buf(&[RED, RED, RED, RED]);
        cpu_apply_color_op(
            &mut buf,
            2,
            2,
            &Region::new(1.0, 1.0, 1.0, 1.0), // bottom-right pixel only
            &ColorOp::Brightness(0.0),
        );
        let px = to_pixels(&buf);
        assert_eq!(px[0], RED);
        assert_eq!(px[1], RED);
        assert_eq!(px[2], RED);
        assert_eq!(px[3], [0, 0, 0, 255]);
    }

    #[test]
    fn test_cpu_transparent_pixels_stay_transparent() {
        // Color math must not paint fully-transparent pixels.
        let buf_in = fill_buf(&[TRANSPARENT]);
        for op in [
            ColorOp::ColorAdjust {
                mults: [1.0; 3],
                offsets: [1.0, 1.0, 1.0],
            },
            ColorOp::Brightness(3.0),
            ColorOp::Contrast(5.0),
            ColorOp::Invert(1.0),
        ] {
            let mut buf = buf_in.clone();
            cpu_apply_color_op(&mut buf, 1, 1, &Region::new(0.0, 0.0, 1.0, 1.0), &op);
            assert_eq!(to_pixels(&buf), vec![TRANSPARENT], "{op:?} must not paint transparent pixels");
        }
    }

    #[test]
    fn test_region_pixel_bounds_clamps() {
        // Degenerate/empty regions → empty ranges.
        assert_eq!(region_pixel_bounds(&Region::new(0.0, 0.0, 0.0, 0.0), 10, 10).2, 0);
        // Fully offscreen (left of the texture) → empty range.
        assert_eq!(region_pixel_bounds(&Region::new(-5.0, -5.0, 3.0, 3.0), 10, 10), (0, 0, 0, 0));
        // Straddles the top-left corner → clipped to the in-bounds part.
        assert_eq!(region_pixel_bounds(&Region::new(-5.0, -5.0, 8.0, 8.0), 10, 10), (0, 0, 3, 3));
        // Runs off the bottom-right → clipped to the texture.
        assert_eq!(region_pixel_bounds(&Region::new(8.0, 8.0, 100.0, 100.0), 10, 10), (8, 8, 10, 10));
    }

    #[test]
    fn test_wgsl_kernel_validates_with_naga() {
        // No hardware needed: the kernel must parse + validate under naga
        // (the same compiler wgpu itself uses). Runs under every feature
        // gate — this is the shader-side CI gate.
        let module =
            naga::front::wgsl::parse_str(COLOR_MATH_WGSL).expect("COLOR_MATH_WGSL must parse");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("COLOR_MATH_WGSL must validate");
    }

    #[test]
    #[cfg(feature = "gpu")] // engine types only exist under the gpu feature
    fn test_gpu_parity_with_cpu_reference_when_adapter_available() {
        let Some(engine) = GpuEffectEngine::try_new() else {
            // No adapter (headless CI without Vulkan/GL) — the CPU oracle
            // still holds; GPU parity is exercised on adapter-equipped
            // machines only.
            eprintln!("[mustang] no wgpu adapter available — GPU parity test skipped");
            return;
        };

        // Deterministic 8x6 premultiplied buffer.
        let mut original = Vec::with_capacity(8 * 6 * 4);
        for y in 0..6u32 {
            for x in 0..8u32 {
                let r = ((x * 30 + 10) % 256) as u8;
                let g = ((y * 40 + 5) % 256) as u8;
                let b = (((x + y) * 20) % 256) as u8;
                let a = 255u8;
                original.extend_from_slice(&[r, g, b, a]);
            }
        }

        let region = Region::new(1.0, 2.0, 5.0, 3.0);
        for op in [
            ColorOp::Brightness(0.6),
            ColorOp::Contrast(1.7),
            ColorOp::Saturate(0.3),
            ColorOp::HueRotate(137.0),
            ColorOp::Grayscale(0.8),
            ColorOp::Invert(0.9),
            ColorOp::ColorAdjust {
                mults: [1.1, 0.9, 1.3],
                offsets: [0.05, 0.0, -0.05],
            },
        ] {
            let mut cpu_buf = original.clone();
            cpu_apply_color_op(&mut cpu_buf, 8, 6, &region, &op);
            let mut gpu_buf = original.clone();
            engine
                .apply_to_buffer(8, 6, &mut gpu_buf, &op, &region)
                .expect("GPU apply_to_buffer");
            // Same math on both sides; allow ≤1 LSB quantization drift.
            for (i, (c, g)) in cpu_buf.iter().zip(gpu_buf.iter()).enumerate() {
                assert!(
                    i32::from(*c) - i32::from(*g) <= 1
                        && i32::from(*g) - i32::from(*c) <= 1,
                    "{op:?} parity drift at byte {i}: cpu={c} gpu={g}"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "gpu")] // engine types only exist under the gpu feature
    fn test_gpu_buffer_length_validation() {
        let Some(engine) = GpuEffectEngine::try_new() else {
            eprintln!("[mustang] no wgpu adapter available — length-validation test skipped");
            return;
        };
        let mut too_small = vec![0u8; 4 * 4];
        let err = engine.apply_to_buffer(
            4,
            4,
            &mut too_small,
            &ColorOp::Brightness(1.0),
            &Region::new(0.0, 0.0, 4.0, 4.0),
        );
        assert!(matches!(err, Err(GpuComputeError::BufferLength { expected: 64, got: 16 })));
    }
}
