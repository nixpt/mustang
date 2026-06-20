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