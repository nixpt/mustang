#!/usr/bin/env bash
# check-boundaries.sh — machine-runnable enforcement of mustang's four-fold
# Boundary Doctrine. See:
#   - README.md ## Boundary Doctrine (in-repo entry-point version)
#   - docs/architecture/arniko-mustang-bliss-surfer-architecture.md (mirrored from workspace-meta)
#   - CONTRIBUTING.md ## The four-fold test
#
# Requires bash 4+ (associative arrays / declare -a). Windows users:
# invoke via Git Bash or WSL — native cmd.exe / PowerShell unsupported.
# See CONTRIBUTING.md ## Operating environment for the full rationale.
#
# The four boundaries:
#   - No layout           → taffy, taffy_geom
#   - No text shaping     → parley, font-kit
#   - No DOM              → bliss-dom, dioxus (any dioxus-* crate)
#   - No HTML parsing     → html5ever, markup5ever
#
# Usage: from the project root, `./scripts/check-boundaries.sh`.
# Exit codes:
#   0 — clean (all four boundaries hold)
#   1 — violation caught (one or more prohibited crates reached the tree)
#   2 — preflight failure (cargo missing, cargo tree failed, etc.)
#
# Note on cargo tree options used:
#   --prefix none    → strips tree-drawing characters so we can grep cleanly
#                       (cargo name vVer → "crate_name vVer" line per row).
#   --no-dedupe      → we want every occurrence of a violating crate, even if
#                       it's pulled in multiple times; dedupe would hide the
#                       full blast radius.
#   --features full  → strictest feature combination (gpu + animation);
#                       catches anything slipped in under any path.

set -euo pipefail

# Resolve project root from $0 (works whether invoked from project root or from
# the scripts/ subdir itself).
PROJ_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJ_ROOT"

# Preflight: cargo available?
if ! command -v cargo >/dev/null 2>&1; then
  echo "❌ preflight: cargo not on PATH — install rustup from https://rustup.rs/ first." >&2
  exit 2
fi

# Preflight: cargo tree supports --prefix none? (cargo 1.78+)
if ! cargo tree --help 2>&1 | grep -q -- '--prefix'; then
  echo "❌ preflight: this script requires cargo 1.78+ (for --prefix none)." >&2
  exit 2
fi

# Run the dep tree under --features full (the strictest combo).
if ! TREE=$(cargo tree --features full --prefix none --no-dedupe 2>&1); then
  echo "❌ preflight: cargo tree failed. Cargo.toml / Cargo.lock may not be buildable." >&2
  echo "   Raw cargo output (truncated to last 30 lines):" >&2
  echo "$TREE" | tail -30 >&2
  exit 2
fi

# Categories are kept human-editable: each line is "<category>:<kind>:<pattern>"
# where <kind> is either "exact" or "prefix":
#   • exact   → matches `pattern <version>` only (NOT `pattern_xxx` or
#               `pattern-xxx`). Editor-safe; uses an explicit kind field
#               so semantics don't depend on trailing whitespace (which
#               auto-formatting editors would silently strip).
#   • prefix  → matches anything starting with `pattern`, including
#               `pattern`, `pattern_xxx`, `pattern-xxx`. Used for umbrella
#               catches like the full `dioxus-*` crate family.
# This way `layout:exact:taffy` does not catch `taffy_geom` (separate exact
# entry), while `dom:prefix:dioxus-` catches `dioxus-core`, `dioxus-html`,
# etc. without enumerating them. See CONTRIBUTING.md ## The four-fold test
# for the canonical reference list.
declare -a CATEGORIES=(
  "layout:exact:taffy"
  "layout:exact:taffy_geom"
  "text:exact:parley"
  "text:exact:font-kit"
  "dom:exact:bliss-dom"
  "dom:exact:dioxus"
  "dom:prefix:dioxus-"   # umbrella: dioxus-core, dioxus-html, etc.
  "dom:prefix:dioxus_"   # umbrella: dioxus_signals, dioxus_core, etc.
  "html:exact:html5ever"
  "html:exact:markup5ever"
)

VIOLATIONS=0
for entry in "${CATEGORIES[@]}"; do
  # 3-field split: category (1), kind (2), pattern (3).
  IFS=':' read -r category kind pattern <<<"${entry}"
  case "$kind" in
    exact)
      # Match `^<pattern> ` (literal name + space) — catches `pattern <version>` only.
      offending=$(echo "$TREE" | grep -E "^${pattern} " || true)
      ;;
    prefix)
      # Match `^<pattern>` — catches anything starting with pattern.
      offending=$(echo "$TREE" | grep -E "^${pattern}" || true)
      ;;
    *)
      echo "❌ internal error: unknown kind '${kind}' for entry '${entry}'" >&2
      exit 2
      ;;
  esac
  if [ -n "$offending" ]; then
    echo ""
    echo "❌ BOUNDARY VIOLATION [$category]: '${pattern}' (${kind}) reached the crate graph under --features full."
    echo "Offending entries:"
    echo "$offending" | sed 's/^/   /'
    VIOLATIONS=1
  fi
done

if [ "$VIOLATIONS" -eq 0 ]; then
  echo "✅ All four boundaries clean under --features full."
  echo "   • No layout           (no taffy)"
  echo "   • No text shaping     (no parley)"
  echo "   • No DOM              (no bliss-dom / dioxus)"
  echo "   • No HTML parsing     (no html5ever / markup5ever)"
  exit 0
fi

echo ""
echo "Remediation: pull the violating crate(s) out of mustang's dep tree."
echo "  • layout / text-shaping belong to arniko           (nixpt/arniko)"
echo "  • DOM  / HTML parsing belong to bliss-engine       (nixpt/bliss-engine)"
echo "If a feature genuinely requires one of those, route the feature to the"
echo "matching sibling project instead of crossing mustang's boundary."
echo "See CONTRIBUTING.md ## The four-fold test for the canonical ref."
exit 1
