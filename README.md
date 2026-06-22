# 🐎 Mustang

**GPU-Accelerated Effect Compositor for Exosphere**

Mustang is the high-performance rendering engine that powers advanced visual effects in the **Exosphere** ecosystem. It enables modern UI patterns like frosted glass, complex motion, and secure regional clipping by leveraging hardware acceleration.

## Overview

Mustang transforms declarative CSS-like synthetic features into native GPU operations. It is designed to be:
- **Fast**: Offloads expensive pixel math to the GPU.
- **Efficient**: Uses a zero-copy pipeline to minimize memory bandwidth.
- **Secure**: Provides hardware-level region gating for isolated components.

## Features

- `gpu` (default): Enables GPU acceleration via Vello and wgpu
- `full`: enables `gpu` + `animation` (Rust-only easing + engine; JS bridge
  is deferred because `boa_engine 0.21` pins `icu_normalizer ~2.0.0` while
  `parley ^0.10` requires `^2.1.1` — see `src/animation/mod.rs`)

## Boundary Doctrine

Mustang is intentionally a **thin GPU effect compositor and scene scheduler**. It receives a fully-built `vello::Scene`, applies blur / shadow / transform / color-adjust effects within regions-of-interest, and submits the result through WGPU. Everything else lives in a sibling project.

**What mustang IS:**

- A primitive-effect compositor (`Effect::blur`, `Effect::transform`, `Effect::color_adjust` — see `src/effect.rs`).
- A scene-graph effect layer (region targeting, per-component effect cache, native-vs-deferred scheduling — see `src/compositor.rs`).
- A WGPU submission pipeline with a `Renderer` integration point (see `src/renderer.rs` and the `gpu` feature).

**What mustang is NOT — the four boundaries:**

- **No layout.** Taffy belongs to [`nixpt/arniko`](https://github.com/nixpt/arniko) (component framework) and to **bliss-engine** (HTML/CSS DOM, currently vendored inside the arniko monorepo at `crates/bliss`). Pulling a layout dep tree into the GPU compositor is strictly out-of-scope.
- **No text shaping.** Parley belongs to arniko and bliss-engine. Glyph atlases and font loading are not mustang's domain.
- **No DOM.** There is no DOM in mustang. `bliss-dom` (bliss-engine's DOM) is a bliss-engine construct; mustang never walks a node tree, it only sees frame-region effects on a flat `Scene`.
- **No HTML parsing.** `html5ever`-class markup parsing is bliss-engine's responsibility — mustang never sees markup, only post-`vello::Scene` pixel-region effects.

**How to apply (contributor guidance):**

When adding features to mustang, do NOT cross the boundaries above — don't add layout, don't add text shaping, don't add DOM walking, don't add HTML parsing. If a feature seems to need one of those, the feature belongs in **arniko** (capsule UI / reactive component framework) or in **bliss-engine** (HTML/CSS renderer used by surfer) — not in mustang.

**See also:**

- Architecture memory: [`arniko-mustang-bliss-surfer-architecture.md`](./docs/architecture/arniko-mustang-bliss-surfer-architecture.md) — settled at s305 (2026-06-19): dep graph + boundary rationale + "how to apply" checklist. *(Synchronized copy of `workspace-meta/foreman-memory/`; resolve there is the source of truth.)*
- Origin commit: [`25f898c`](https://github.com/nixpt/arniko/commit/25f898c) — *"extract mustang → standalone `nixpt/mustang` project"* on [`nixpt/arniko`](https://github.com/nixpt/arniko). The commit that removed mustang from arniko's workspace + `[patch.crates-io]`, making it the standalone thin GPU compositor documented above.

## Key Modules

- **Compositor**: Manages the lifecycle of effects and element tracking.
- **Effect**: Defines the core visual primitives (Blur, Transform, Color).
- **Renderer**: Provides the glue for Vello and WGPU integration.

## Usage

```rust
use mustang::{MustangConfig, MustangMode, MustangCompositor};

// Configure for high-performance GPU mode
let config = MustangConfig::new()
    .mode(MustangMode::GpuAccelerated)
    .enable_caching(true);

let compositor = MustangCompositor::new(config);
```

## Integration with Arniko

Mustang was originally part of the Arniko SDK but has been moved to the platform rendering layer for better reusability across the Exosphere ecosystem.

## Licensing

Copyright (c) 2026 **The Exosphere Authors**.
Dual-licensed under **MIT** or **Apache-2.0**.