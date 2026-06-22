# 🐎 Contributing to Mustang

First — **read [`README.md`](./README.md)'s `## Boundary Doctrine` section.**
The four constraints listed there are non-negotiable. PRs that violate them will
be closed before merge, and the feature re-implemented in the correct sibling
project (arniko for layout / text-shaping / reactive UI; bliss-engine for HTML,
CSS, and DOM rendering). This is the architectural settlement from s305
([`25f898c`](https://github.com/nixpt/arniko/commit/25f898c) on
[`nixpt/arniko`](https://github.com/nixpt/arniko)) and is not open for casual
redrawing.

## The four-fold test (pre-merge checklist)

For every PR, answer "no" to each of these:

1. **No layout.** Does the diff introduce `taffy`, `taffy_geom`, or any
   layout-graph dependency (transitive counts)? → If yes, the feature belongs
   in arniko, not mustang.
2. **No text shaping.** Does the diff introduce `parley`, `font-kit`, or any
   glyph-atlas / font-loading machinery? → If yes, arniko.
3. **No DOM.** Does the diff introduce `bliss-dom`, any crate under the
   `dioxus-*` umbrella, or any tree walker that operates on a node-shaped data
   structure (rather than a flat `vello::Scene`)? → If yes,
   bliss-engine (browser content), not mustang.
4. **No HTML parsing.** Does the diff introduce `html5ever`, `markup5ever`, or
   any markup-parsing crate? → If yes, bliss-engine, not mustang.

The empirical baseline as of s333 is clean — `cargo tree --features full`
returns zero hits for any of the above crate names. Keep it that way.

## Mechanical enforcement

Run the local check script before pushing:

```bash
./scripts/check-boundaries.sh
```

The script does a `cargo tree --features full --prefix none --no-dedupe`,
grep-matches each prohibited crate name (one per boundary), and exits non-zero
with a remediation hint on any violation. It runs in seconds on a warm cache
and is the machine-runnable form of the four-fold test.

> **Note on CI integration.** This is now wired — see the
> `.github/workflows/boundary-check.yml` workflow. Every push to `main` and
> every PR runs `./scripts/check-boundaries.sh` + the three Cargo-check
> feature gates (default / gpu / full) + the blur-example regression test
> (`cargo test --features gpu --examples blur`) + the `--features full`
> test suite. The script + the GHA workflow + a reviewer reading the
> four-fold test = the operational enforcement layer. The empirical
> baseline as of `2026-06-22T02:59:20Z` (the freshest cadence-row in
> `docs/architecture/audit-results.md`) is that all four gates pass on
> `main`; the workflow is the machine-runnable mirror of that baseline.

## Operating environment

> **Windows:** the boundary check requires bash 4+ (associative arrays /
> `declare -a`). Run it via **Git Bash** (ships with [git for windows](https://git-scm.com/download/win)),
> **WSL**, or any other bash ≥ 4.0 environment. Native `cmd.exe` / PowerShell
> are not supported out of the box.

## Where features belong when the four-fold test says "no"

- **Capsule / reactive UI** (component framework, layout, text-shaping,
  reactive state): → [`nixpt/arniko`](https://github.com/nixpt/arniko)
  (`nixpt/arniko`, standalone workspace crate `arniko`).
- **HTML / CSS / DOM rendering** (browser content): → bliss-engine
  (`nixpt/bliss-engine`, currently vendored inside the arniko monorepo at
  `crates/bliss`; standalone repo on GitHub).
- **Pure GPU compositing / effects** (this repo): → mustang
  (`nixpt/mustang`, published on crates.io as `arniko-mustang v0.2.99`).

If your feature seems to live at the intersection (e.g., a layout-aware
effect), the right answer is almost always: implement the effect in mustang
without any layout awareness, and have the consumer (arniko or bliss-engine)
invoke it with explicit region metadata.

## Dev workflow

1. **Branch off `main`.** Tag scope in the branch name if it's narrow
   (`fix-…`, `doc-…`, `feature-…`, `refactor-…`).
2. **Implement**, modeled on the surrounding `src/` style. Look at
   `src/effect.rs`, `src/compositor.rs`, `src/renderer.rs`, and `src/lib.rs`
   for naming + module-organization precedents.
3. **Tests green.** Default gate: `cargo test --features gpu --examples blur`
   must continue to pass (the blur example is the operational demonstration
   of the boundary doctrine — see
   [`docs/architecture/`](./docs/architecture/) and the example doc-comment).
4. **Run `./scripts/check-boundaries.sh`** locally. Zero violations expected.
5. **Push & open PR.** In the PR description, include a one-line confirmation
   for each of the four boundaries ("No layout, no text shaping, no DOM,
   no HTML parsing — verified by scripts/check-boundaries.sh").
6. **Reviewer** verifies both the four-fold test (read each boundary) and the
   script output (paste the script's exit code or stdout).

## Public-API surface

- Adding a new public type? Update `src/lib.rs`'s `pub use` exports + add a
  doc-comment example (look at `MustangCompositor`, `Effect`, `Region`,
  `VelloScenePainter` for precedents).
- Internal only? Don't re-export.
- Adding a new effect kind? File it under `src/effect.rs` (the `Effect` enum
  or its supporting enums) and the corresponding docs-`## Boundary Doctrine`
  entry if the effect introduces a NEW category.

  The `Effect` enum + `EffectType` discriminator already covers the canonical
  CSS visual surface mustang ships:

  - **Scene-native (Vello scene-side):** `BackdropBlur` (`filter: blur`),
    `Transform2D` (`transform: scale/translate/rotate`), `Clip`
    (`clip-path` security gate), `DropShadow` (`box-shadow` /
    `filter: drop-shadow`).
  - **Deferred to GPU compute (CustomPaintSource path):** `ColorAdjust`
    (multipliers + offsets), `CanonicalFilter` (`hue-rotate`, `saturate`,
    `brightness`, `contrast`, `grayscale`, `invert`).

  Categories covered as of s1+a: blur, transform, color-adjust, drop-shadow,
  and the canonical CSS filter set. Additions outside this set must not
  cross the four boundaries above.

## Architectural provenance

The doctrinal rules come from the s305 settlement and are documented at:

- [`docs/architecture/arniko-mustang-bliss-surfer-architecture.md`](./docs/architecture/arniko-mustang-bliss-surfer-architecture.md)
  *(synchronized copy; the workspace-canonical source is
  `workspace-meta/foreman-memory/arniko-mustang-bliss-surfer-architecture.md` in
  the multi-project layout — update BOTH files in the same commit if doctrine
  ever changes).*
- [`README.md`](./README.md)'s `## Boundary Doctrine` section (the in-repo
  entry-point version, cross-linked above).
- [`25f898c`](https://github.com/nixpt/arniko/commit/25f898c) — origin commit
  on `nixpt/arniko`.

## Style

- Follow the surrounding `src/` style (look at `src/effect.rs`,
  `src/compositor.rs`, `src/renderer.rs`, `src/lib.rs` for precedents).
- Rust 2024 edition; rust-version = "1.85.0" (see `Cargo.toml`).
- Doc-comments on all public items; example code in the `## Usage` example
  block must compile.
- `eprintln!`/`println!` only at the example-test boundary; the library is
  silent.
- Do NOT introduce boundary-crossing deps — see the four-fold test above.

## Releasing

The published crate name on crates.io is `arniko-mustang`. The lib name (used
in `use` statements) is `mustang`. Don't rename either without coordinating
with the standalone deploy of this repo.

## Licensing

By contributing, you agree your contributions are dual-licensed under **MIT**
and **Apache-2.0**, matching the rest of the repo.

## Next steps (not blocking this PR)

- ✅ **(done)** **CI workflow** at `.github/workflows/boundary-check.yml`
  running `./scripts/check-boundaries.sh` + the three Cargo-check feature
  gates (default / gpu / full) + the blur-example regression test + the
  `--features full` test suite on every push to `main` and every PR. The
  `.github/` infrastructure was bootstrapped in this commit; the workflow
  is the machine-runnable mirror of the cadence table in
  `docs/architecture/audit-results.md`.
- **Mirror CONTRIBUTING.md → arniko / bliss-engine** with the reciprocal
  "what THIS repo IS, not IS-NOT" framing from each sibling's perspective.
- **`cargo-deny`** integration as a heavier-weight alternative to the
  bash-script grep if/when the prohibited-crate list grows beyond ~6 names.
- **Published-crate CI**: once the repo is published to crates.io as
  `arniko-mustang`, consider a periodic CI run against the published
  tarball to detect drift between repo source and published artifact
  (the current workflow gates only the repo source).
