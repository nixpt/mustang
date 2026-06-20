# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-06-20

### Changed (breaking)
- `Effect::apply_to_scene` for `Transform2D` and `Clip` is now a no-op;
  layer-scope effects go through the new `Effect::begin_layer_scope`
  which returns an RAII `LayerGuard` that pops the layer on drop
  (`.release()` to opt out). The old "caller must pop_layer" contract
  was broken-by-design and the new API forces correctness at compile
  time.
- `EffectScene::apply_effect` and `apply_effects` got a `debug_assert!`
  for layer-scope effects; `MustangCompositor::apply_scene_effects`
  defers them to the caller.
- `Effect` is now `#[non_exhaustive]`; downstream code must use the
  `Effect::blur` / `transform` / `color_adjust` / `clip` constructors.
- `MustangCompositor::get_cached_effects` now takes `&mut self`
  (was `&self`) for the LRU access-order update.
- `MustangConfig.gpu_device` field removed (was never read); the
  `.gpu_device()` builder is gone.
- `Compositor` / `CompositeResult` / `CompositorConfig` types removed
  from the `compositor` module (the `composite()` method was an unused
  no-op stub).
- `pub use compositor::*;` wildcard in `lib.rs` replaced with an
  explicit list of the public surface.
- `parse_transform` now correctly handles non-uniform `scale(x, y)`,
  multi-function `transform: scale(...) translate(...) rotate(...)`
  chains, and uses a shared `extract_first_fn_args` helper.
- `parse_color_adjust` now handles `brightness()`, `contrast()`, and
  `saturate()` filters (only `brightness` was recognized before).
- `AnimatedProperty::interpolate` and `to_effect` are now exhaustive;
  the `Opacity` arm was a silent type error mapping to a 0-radius
  `Effect::blur` and has been removed.

### Added
- `LICENSE-MIT` and `LICENSE-APACHE` at the repo root.
- `.github/workflows/ci.yml` with `rustfmt`, `clippy -D warnings`, and
  a `test` matrix over the 4 feature combos (`default`, `gpu`,
  `animation`, `full`); plus a `cargo audit` job for the RustSec
  advisory database.
- `examples/gpu_blur.rs` — headless smoke test of the metadata→effect
  extraction pipeline.
- `Effect::transform_with_affine(selector, affine, region)` constructor
  and `TransformParams::from_affine(affine)` for the common Vello use
  case where the caller already has a `kurbo::Affine`. Skew is
  approximated away (TransformParams cannot represent shear).
- `EffectScene::begin_layer_scope` and the `LayerGuard` RAII type.
- `Effect::is_one_shot` and `Effect::is_layer_scope` predicate
  methods (in addition to the existing `is_native`).
- `CLIP_Z_INDEX` const replaces the magic number in `Effect::clip`.
- `.dejavue/` memory layer initialized; agent context (decisions,
  invariants, patterns, handoff, timeline) survives across sessions.

### Fixed
- `cubic_bezier` in `easing.rs` was a raw Bernstein polynomial; replaced
  with a proper CSS cubic-bezier Newton solve. All four CSS-named
  easings (`Ease`, `EaseIn`, `EaseOut`, `EaseInOut`) were wrong.
- `AnimationEngine::active_count` was counting `Pending` animations;
  now filters by `state == Running`.
- `cargo check --features animation` and `--features full` failed to
  compile: the `js_binding` module was commented out of
  `animation/mod.rs` but still re-exported by `lib.rs`. The dead
  module was deleted and the dead re-export removed.
- `MustangCompositor.effect_cache` was unbounded; now uses real LRU
  eviction (default `max_cache_size: 1000`, `enable_caching: true`).

## [0.2.99] - 2026-05-21

### Changed
- Extracted `arniko-mustang` as a standalone `mustang` project
  (renamed package, `[lib] name = "mustang"`).
