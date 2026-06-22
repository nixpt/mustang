---
name: arniko-mustang-bliss-surfer-architecture
description: "Confirmed architecture: arniko (component framework) + mustang (thin GPU compositor) + bliss-engine (HTML/CSS for surfer) + surfer (browser). Decided s305."
metadata:
  node_type: memory
  type: project
  originSessionId: 9fc47fd6-273f-4943-8535-7a10f1c4f0d2
---

> **Note (mustang-repo copy):** This file is a synchronized copy of the
> workspace-canonical source at
> `workspace-meta/foreman-memory/arniko-mustang-bliss-surfer-architecture.md`.
> The workspace-meta path is the source of truth; this copy lives in the
> standalone `projects/mustang/` repo so the `[Boundary Doctrine]` section
> in this README's cross-link resolves on GitHub
> (`https://github.com/nixpt/mustang`) — `workspace-meta/` is not part of
> this repo's tree. (Reachable in-workspace via
> `../../../../workspace-meta/foreman-memory/arniko-mustang-bliss-surfer-architecture.md`
> from this file; **not** a clickable link here because GitHub's markdown
> renderer refuses to let relative paths escape the repo root, so any
> `[…](…)` wrapper would 404 on `github.com/nixpt/mustang`.)
> **When updating the doctrine, edit BOTH files in the same commit** —
> drift between `workspace-meta/` and this repo copy is a known risk;
> never let them diverge silently.

---

Settled architecture (s305, 2026-06-19) for the four related projects:

**mustang** (`nixpt/mustang`, crate `arniko-mustang`): thin GPU effect compositor and scene scheduler. Receives a Vello `Scene`, applies blur/shadow/color effects, schedules and submits to WGPU. Does NOT do layout or text. Deps: Vello, WGPU, Peniko, Kurbo, anyrender.

**arniko** (`nixpt/arniko`): reactive component framework (Signal, Computed, View, For, Switch). Does its own layout (Taffy direct), text (Parley direct), painting (Vello direct), then hands the final `Scene` to mustang for effects/compositing. No Stylo. No bliss-dom. arniko = "what to draw"; mustang = "how to composite and submit".

**bliss-engine** (`nixpt/bliss-engine`, fork of dioxus/blitz): HTML/CSS DOM rendering engine. Stylo (CSS) → Taffy → Parley → Vello. Used by surfer to render web page content. Also routes its final Scene through mustang for GPU effects (convergence point).

**surfer**: the browser. arniko = browser chrome (tabs, omnibar, UI components). bliss-engine = web content rendering. mustang = final GPU composite layer for both.

**Why:** arniko is for capsule UI / agent-native OS UI, NOT for rendering arbitrary web HTML — that's surfer's job (via bliss-engine). Arniko dropped the Stylo/bliss-dom stack entirely (session s305).

**Key dep graph:**
- `mustang` ← Vello, WGPU (no bliss, no Stylo)
- `arniko` ← mustang, Taffy, Parley, Vello (no bliss, no Stylo)
- `bliss-engine` ← Stylo, Taffy, Parley, Vello, mustang
- `surfer` ← arniko + bliss-engine

**How to apply:** When adding features to any of these, don't cross the boundaries — don't add HTML/CSS parsing to arniko, don't add reactive signals to bliss-engine, don't add layout to mustang.
