# Decisions


## 2026-10-11T12:47:18-05:00 — mustang gpu: use bliss-anyrender-vello 0.3 fork instead of upstream anyrender_vello 0.7

Reason:
Share one VelloScenePainter/VelloWindowRenderer type with bliss and cut upstream ties. MSRV stays 1.88: bliss-anyrender-vello 0.3.0 and 0.3.1 are source-identical (only rust-version metadata differs), so the MSRV-aware resolver picks 0.3.0 on 1.88. PR nixpt/mustang#3.

Author type: agent
## 2026-10-11T20:36:26-05:00 — [STRATEGIC] [VERIFIED] GPU compute path architecture (gpu_compute.rs)

Reason:
One WGSL kernel mode-dispatched over premultiplied RGBA8 (unpremultiply, sRGB math, clamp, premultiply); GpuEffectEngine behind gpu feature with graceful None on no adapter; cpu_apply_color_op as test oracle + fallback. Storage buffers not textures for exact CPU/GPU parity (first cut); hue-rotate/saturate constants verbatim from filter-effects-1 §feColorMatrix.

Artifacts: src/gpu_compute.rs


## 2026-10-11T20:36:26-05:00 — [TACTICAL] [ADOPTED] Opacity approximation + scheduler per-region dirty rects

Reason:
Opacity has no native effect kind yet: animates as ColorAdjust fade-to-black (RGB multipliers = interpolated alpha); dedicated EffectType::Opacity via Vello push_layer alpha is the follow-up. Scheduler: on_region_changed accumulates rects, on_dom_changed marks full-scene; take_dirty() returns SceneDirtyState{full,regions} consumed atomically — full supersedes rects.

Artifacts: src/animation/mod.rs,src/scheduler.rs

