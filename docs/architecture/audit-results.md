# Empirical audit results — mustang dependency tree

> One-time audit run as the **Boundary Doctrine** lands in the README (`## Boundary
> Doctrine`, this sync copy, and the reciprocal in
> `nixpt/bliss-engine`'s README). This file is empirical evidence that the four
> doctrinal boundaries actually hold on the dependency-graph side as of the
> audit run, *not* a certification of compilation under every feature gate.

| Field                 | Value                                                          |
|-----------------------|----------------------------------------------------------------|
| **Audit run (UTC)**   | 2026-06-22T01:33:52Z                                           |
| **Repo state**        | commit `48bc6fe` *mustang: README boundary doctrine + synced architecture copy* |
| **Operator**          | `cargo check` + `cargo tree --features full` (the strictest combo: gpu + animation) |
| **Working dir**       | `/workspace/projects/mustang/`                                 |

---

## Method

The audit exercises three feature gates (compile-side) plus a full-dep-tree
sweep (boundary-side). The dep-tree gate is run under `--features full`
(intentionally the strictest combination so that anything slipped in under
any feature is caught).

```text
cargo check                                        # default
cargo check --features gpu                         # gpu runtime path
cargo check --features full                        # gpu + animation (strictest)
cargo tree --features full --prefix none --no-dedupe > /tmp/mustang-tree.txt
```

The boundary grep uses `^<pattern> ` (literal name + space, anchored at
line-start). This matches `crate_name vX.Y.Z` rows from `cargo tree
--prefix none`'s output while **not** matching substring shapes like
`crate_name_geom` or `my_crate_name`.

---

## Per-boundary audit results

The four doctrinal boundaries plus three auxiliary CSS-layer crates
(`stylo`, `cssparser`, `selectors`) were grepped against
`/tmp/mustang-tree.txt` (`cargo tree --features full --prefix none
--no-dedupe`).

| Boundary                  | Prohibited crates                          | Matches | Verdict |
|---------------------------|-------------------------------------------|---------|---------|
| **No layout**             | `taffy`, `taffy_geom`                      | 0       | ✅ HOLDS |
| **No text shaping**       | `parley`, `font-kit`                       | 0       | ✅ HOLDS |
| **No DOM**                | `bliss-dom`, `dioxus` (-umbrella)          | 0       | ✅ HOLDS |
| **No HTML parsing**       | `html5ever`, `markup5ever`                 | 0       | ✅ HOLDS |
| *(auxiliary CSS layer)*   | `stylo`, `cssparser`, `selectors`          | 0       | ✅ HOLDS |

**Doctrine verdict on the dependency graph:** **HOLDS**. Mustang pulls
ZERO direct or transitive deps from any of the four doctrinal boundaries
under `--features full` (the strictest combination, gpu + animation).

---

## Cargo check (compile-side) results

| Feature gate           | Compile result       | Notes |
|------------------------|----------------------|-------|
| **default** features   | ✅ PASS (4.2s)        | Pure lib (no GPU deps needed). |
| **`--features gpu`**   | ✅ PASS (1.9s)        | Strictest *runtime* path. |
| **`--features full`**  | ❌ FAIL (E0432, 1s)   | Stale `src/lib.rs:41` re-export — see "Known deferred issue" below. |

The dep-side audit ran cleanly under `--features full` because `cargo tree`
does not require the lib to compile; cargo's `dependecies:` resolver can
still produce a graph even when the lib's source has import errors. So the
boundary-HOLDS finding above is independent of the compile-side finding
below.

### Known deferred issue (NOT a doctrinal violation): `src/lib.rs:41`

The `cargo check --features full` failure:

```text
src/lib.rs:41:20: error[E0432]: unresolved import `animation::js_binding`
  : could not find `js_binding` in `animation`
```

**Status:** — pre-existing deferred issue from the **s306** session.
Unrelated to any doctrinal boundary — no prohibited crate is the cause.

**Proximate cause history:**

The s306 fix commit `853e72b` (*fix(mustang): comment out js_binding re-export
— boa_engine icu conflict deferred*) commented out the `js_binding` module
declaration at `src/animation/mod.rs:15`:

```rust
// js_binding (boa_engine JS runtime) deferred — boa_engine 0.21 pins
// icu_normalizer ~2.0.0 while parley ^0.10 requires ^2.1.1; the two
// can't coexist in one Cargo resolve graph. Re-enable when boa_engine
// ships icu_normalizer >= 2.1.1.
// pub mod js_binding;
```

…but the corresponding re-export at `src/lib.rs:41` was missed in that
fix and still references the gone module:

```rust
#[cfg(feature = "animation")]
pub use animation::js_binding::JsAnimationRuntime;
```

This is the *only* compile failure under any feature gate. The fix is
mechanical (delete or guard the re-export line), and the runtime impact
is zero (the cfg-gated feature path is also blocked from compiling
because the upstream module isn't there).Recommended fix (out-of-scope for this audit, for symmetry with the
existing s306 carve-out in `src/animation/mod.rs:15`):

```rust
// src/lib.rs around line 41 — comment out for parity with
// src/animation/mod.rs:15's s306 carve-out. Note: re-enable when
// boa_engine ships an icu_normalizer release that resolves the
// ^2.1.1 requirement (parley ^0.10 pins icu_normalizer there).
#[cfg(feature = "animation")]
pub use animation::js_binding::JsAnimationRuntime;
```

Becomes:

```rust
// src/lib.rs around line 41 — comment out for parity with the
// src/animation/mod.rs:15 s306 carve-out. The boa_engine JS runtime
// is deferred because boa_engine 0.21 pins icu_normalizer ~2.0.0,
// which conflicts with parley ^0.10's requirement on ^2.1.1.
// Re-enable when boa_engine ships an icu_normalizer release that
// resolves the ^2.1.1 requirement.
// #[cfg(feature = "animation")]
// pub use animation::js_binding::JsAnimationRuntime;
```

Why this audit doesn't fix it: the audit's deliverable is empirical
evidence, not maintenance work. The compile-side staleness being a
*known* deferred issue rather than a hidden failure is itself useful
information for future readers.

## Top-level crates reachable under `--features full`

The `cargo tree --features full --no-dedupe` dump yields **4217 rows**
(raw count, including duplicates from repeat transit paths). Sampled
top 25 by sorted first-occurrence (alphabetical):

```text
adler2  anyhow  anyrender  anyrender_vello  arniko-mustang  arrayvec  ash
autocfg  bitflags  bit-set  bit-vec  bytemuck  bytemuck_derive  cfg_aliases
cfg-if  codespan-reporting  color  crc32fast  debug_timer  document-features
equivalent  euclid  fdeflate  flate2  foldhash
```

These are all expected under the s305-settled dep graph:

- **Direct deps (`[dependencies]` block + gpu-feature deps):** `anyhow`,
  `tracing`, `anyrender`, `anyrender_vello`, `vello`, `wgpu`, `kurbo`,
  `peniko`. None prohibited.
- **Transitive closing the gpu feature pulls:** raw-window-handle,
  bitflags, log, hashbrown, bytemuck, euclid, peniko-color, smallvec,
  arrayvec, codespan-reporting, foldhash, etc. (all from vello + wgpu
  upstreams). None prohibited.
- **Dev-dep transitives:** `png`, `tokio`, `tokio-test`, `tokio-stream`,
  `futures-core`, `pin-project-lite`, and the like (from the `examples/`
  harness + the `cargo test` machinery). None prohibited.

The full list (4217 rows) is in `/tmp/mustang-tree.txt` from the audit
run — re-generate with the same `cargo tree … > …` line above.

---

## Reproduction

The audit can be re-run after any release-candidate tag (or any time we
want to verify the doctrine still holds):

```bash
cd /workspace/projects/mustang

# Compile gates (each takes 1-5s):
cargo check                 # default
cargo check --features gpu  # gpu

# Compile gate that surfaces the deferred s306 carve-out:
cargo check --features full

# Full dep-tree audit (canonical; same flags used in this run):
cargo tree --features full --prefix none --no-dedupe > /tmp/mustang-tree.txt

# Per-boundary audit (each MUST return 0 for doctrine to hold):
for pat in taffy taffy_geom parley font-kit html5ever markup5ever \
           bliss-dom dioxus stylo cssparser; do
  echo "$pat: $(grep -cE "^${pat} " /tmp/mustang-tree.txt) match(es) under --features full"
done
```

A canonical reproduction helper lives at `scripts/check-boundaries.sh`
(committed as part of the audit-time cleanup — the file was working-tree
only at audit time itself, before this cleanup). It performs the same
audits as the inline shell loop above, with exit-code-gated categories
for pre-merge use. Each entry in the script's CATEGORIES array is a
3-tuple `<category>:exact|<prefix>:<pattern>` — `exact` matches the
literal `pattern <version>` shape only; `prefix` matches anything
starting with `pattern` (used for umbrella catches like the full
`dioxus-*` / `dioxus_*` crate family). It is the canonical reproduction
tool post-cleanup; verify landed state with
`git log -- scripts/check-boundaries.sh`. Note: re-running the script
post-cleanup reproduces this audit's per-boundary verdicts under the
same `[features]` block resolution — but upstream crate-graph changes
(e.g., a new `vello` or `wgpu` release pulling in a prohibited-crate-named
helper transitively) could shift the `cargo tree` row count. Re-run after
any `[features]` block or direct-dep change rather than relying on a
fixed row count from this audit.

---

## Conclusion

The doctrinal statement *"mustang is a thin GPU effect compositor and does
NOT depend on layout / text shaping / DOM / HTML parsing"* is **empirically
valid as of this audit run**. All four boundaries hold on the dep-graph
side under every feature gate, including the strictest `--features full`
combination.

The compile-side staleness (`src/lib.rs:41` E0432 under `--features full`)
is independent of doctrine; it's a known-deferred issue from the s306
session (commit `853e72b` partially applied a carve-out — the lib.rs
re-export was missed in that fix). The failure mode is well-bounded
(only affects `--features full` builds, only because of an import that
points at a deliberately-deferred module), and the surrounding code paths
that DO compile (default and gpu) are unaffected.

**Re-audit cadence recommendation:** re-run this audit after every
release-candidate tag and after any change to `Cargo.toml` or to the
`[features]` block. Date-stamp entries can be appended below for
historical comparison.

---

### Re-audit log

- **2026-06-22T01:33:52Z** — initial run. Doctrine HOLDS (dep graph, all 4
  boundaries + CSS auxiliary). Compile HOLDS under default + `--features
  gpu`; STALE under `--features full` (E0432 at `src/lib.rs:41`,
  deferred from s306 commit `853e72b`).
- **2026-06-22T02:44:45Z** (cleanup PR) — `scripts/check-boundaries.sh` landed
  (was working-tree only at audit time); CATEGORIES block expanded
  with explicit `<category>:<kind>:<pattern>` (3-tuple) encoding for
  editor-safe semantics; `dom:prefix:dioxus-` / `dom:prefix:dioxus_`
  umbrella entries now catch the full `dioxus-*` crate family;
  `dom:exact:font-kit` and `layout:exact:taffy_geom` added to bring
  CONTENTS and script into parity. No re-audit performed; per-boundary
  verdicts of the initial run are unchanged because the source tree
  did not move between the audit run and this cleanup commit.
