# Mustang — Improvement Plan (2026-06-20)

Captured by: opencode (agent lifecycle)
Source: end-to-end review of `/workspace/projects/mustang/` on `main` @ 6eeac27
Scope: full crate, ~1900 LOC, 38 tests

## Build matrix (baseline)

| features      | status                |
| ------------- | --------------------- |
| default       | clean                 |
| `gpu`         | clean                 |
| `animation`   | **fails**             |
| `full`        | **fails**             |

## Test baseline (default features)

38 tests · 36 pass · 2 fail:

- `animation::easing::tests::test_ease_bounds` — `Ease(0.0)` returns 0.25, not ~0.
  `cubic_bezier()` in `easing.rs:199` is the raw Bernstein polynomial evaluated at
  `t`, not a proper CSS cubic-bezier (which requires Newton solve on x-axis, then
  evaluate y at the found t). All 4 CSS-named easings (Ease, EaseIn, EaseOut,
  EaseInOut) are wrong.
- `animation::tests::test_animation_engine` — `engine.active_count()` returns 1
  for a `Pending` animation; the test expects 0. `active_count()` at
  `animation/mod.rs:396` only filters out `Completed`/`Cancelled`; it should also
  filter `Pending`.

## Critical (C) — correctness / build

### C1 — Fix `cubic_bezier`
**Where:** `src/animation/easing.rs:199-207`
**Action:** Replace the direct Bernstein evaluation with proper CSS cubic-bezier
parameterization (Newton's-method solve on x-axis, then evaluate y at the found t).
**Affects:** `Ease`, `EaseIn`, `EaseOut`, `EaseInOut` (and the test
`test_ease_bounds`). Downstream `Animation::tick` final-state correctness also
depends on easing(1.0) = 1.0, which is only true for a well-behaved easing.

### C2 — Resolve `animation` / `full` build break
**Where:** `src/lib.rs:41` ↔ `src/animation/mod.rs:14-15`
**Action:** The `js_binding` module exists on disk but is commented out of
`animation/mod.rs`. The boa_engine ↔ parley `icu_normalizer` conflict is the
cited reason. Pick one:
  - **a)** Swap `boa_engine` for `rquickjs` or `mquickjs` (no icu dep) and
    re-enable the module. Re-enables JS-driven animations.
  - **b)** Delete `src/animation/js_binding.rs` and remove the
    `pub use animation::js_binding::JsAnimationRuntime;` from `lib.rs`.
    The Rust `AnimationEngine` API already covers programmatic use; the JS
    binding is currently 100% dead code.
**Recommendation:** (b) for the C1-C3 PR. The Rust API is solid; reviving JS
is a separate arc that should pick a different engine and ship a
`js_runtime` feature alongside it.

### C3 — Fix `active_count` semantics
**Where:** `src/animation/mod.rs:396-398`
**Action:** Change `active_count` to count only `Running` animations (filter
out `Pending` along with `Completed`/`Cancelled`). "Active" means "currently
running on the timeline".

## High (H) — design / API quality

### H1 — Enforce `max_cache_size`
**Where:** `src/lib.rs:161-168` (cache_effects / get_cached_effects),
`src/config.rs:32` (config field)
**Action:** `cache_effects()` currently inserts unbounded. Either evict when
size > `max_cache_size` (LRU) or remove the fence-post `enable_caching` /
`max_cache_size` fields.

### H2 — Drop dead `gpu_device` config field
**Where:** `src/config.rs:36`
**Action:** Field is never read. Remove or wire it into the Vello renderer
init path.

### H3 — RAII guard for `push_layer` / `pop_layer`
**Where:** `src/effect.rs:303-317` (Transform2D + Clip apply paths)
**Action:** `Effect::apply_to_scene` for `Transform2D` and `Clip` pushes a layer
and comments "caller must pop_layer after rendering content". One missed
`pop_layer` corrupts every subsequent draw. Introduce `EffectScope<'a, S>`
that pushes on `new()` and pops on `Drop`. Change `EffectScene::apply_effect`
to take the guard and remove the comment footgun.

### H4 — Implement or delete `Compositor::composite` stub
**Where:** `src/compositor/mod.rs:232-244`
**Action:** Currently `buffer.to_vec()` with `#[allow(dead_code)]`. Either
implement a CPU pixel-path version of blur/transform, or delete the stub
and the `Compositor` type.

### H5 — Drop wildcard re-export
**Where:** `src/lib.rs:48` `pub use compositor::*;`
**Action:** Public surface is implicit and overlaps the explicit re-exports
above. Replace with the explicit item set.

### H6 — Real CSS parser
**Where:** `src/compositor/mod.rs:96-186` (`parse_blur_amount`,
`parse_transform`, `parse_color_adjust`, `parse_clip_region`)
**Action:** Hand-rolled `find("blur(")` etc. can't handle
`backdrop-filter: blur(10px) brightness(1.2)` (first function wins, not last),
`scale(1.1, 0.5)` (non-uniform), or `clip-path: inset(...)`. Either add
`cssparser` dep or at minimum split on whitespace and parse the last
function with `,` for the arg list.

## Medium (M) — completeness

| #   | Item                                                                              | Where                                  |
| --- | --------------------------------------------------------------------------------- | -------------------------------------- |
| M1  | Add `LICENSE-MIT` + `LICENSE-APACHE` files                                         | repo root                              |
| M2  | Add `.github/workflows/ci.yml` (`fmt`, `clippy -D warnings`, `test` x 4 features) | repo root                              |
| M3  | Add `examples/gpu_blur.rs` for the README snippet                                  | `examples/`                            |
| M4  | `#[non_exhaustive]` on `Effect`                                                    | `src/effect.rs:120`                    |
| M5  | Name the `clip` z-index                                                            | `src/effect.rs:195` → `CLIP_Z_INDEX`   |
| M6  | `Opacity` arm in `AnimatedProperty::to_effect` catch-all                          | `src/animation/mod.rs:147`             |

## Low (L) — polish

- L1 `CHANGELOG.md` documenting the arniko → standalone extraction.
- L2 `transform` constructor accepting `kurbo::Affine` directly.
- L3 `Region` ↔ `kurbo::Rect` — Region is a 1:1 f32 wrapper; consider aliasing
  under `gpu` feature or moving to a shared types crate.
- L4 `apply_scene_effects` clones deferred effects — flag in profile.
- L5 `cargo-deny` / `rustsec` check in CI.

## Recommendation (land order)

1. **C1–C3 as one PR** — three small correctness fixes, all touch the animation
   module, unblock a clean feature-matrix build.
2. **H1–H3 as a follow-up** — API quality; the pop_layer guard in H3 is a real
   footgun for downstream users.
3. Batch the rest (H4–H6, M1–M6, L1–L5) into a polish PR.

## Status

- [x] C1 — cubic_bezier → proper CSS Newton solve
- [x] C2 — js_binding build break (deleted dead module)
- [x] C3 — active_count filters Pending
