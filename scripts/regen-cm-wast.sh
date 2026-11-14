#!/usr/bin/env bash
#
# Regenerate tests/component-model/meld-<version>/ (SR-93).
#
# Clones pulseengine/meld at the given release tag, runs meld's component-model
# WAST ratchet with MELD_CM_WAST_OUT set, and replaces the vendored meld output
# with the manifest plus the fused modules (input modules are not vendored).
# Also checks that the upstream tests meld ran against are the ones vendored
# under tests/component-model/upstream.
#
# Usage: regen-cm-wast.sh [meld-version]
#   With no argument the meld version is the one the covalent layer pinned in
#   varve.toml ships (read from `varve inspect`).
#
# This is a maintainer tool. CI does not run it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CM="$ROOT/tests/component-model"

die() { echo "regen-cm-wast: $*" >&2; exit 1; }

if [[ $# -gt 1 ]]; then
  die "usage: regen-cm-wast.sh [meld-version]"
fi

if [[ $# -eq 1 ]]; then
  VERSION="$1"
else
  LAYER="$(sed -n 's/^layer *= *"\([^"]*\)".*/\1/p' "$ROOT/varve.toml")"
  [[ -n "$LAYER" ]] || die "no layer found in varve.toml"
  VARVE="${VARVE:-$(command -v varve || true)}"
  [[ -n "$VARVE" ]] || VARVE="$HOME/.varve/bin/varve"
  [[ -x "$VARVE" ]] || die "varve not found (set VARVE=/path/to/varve)"
  VERSION="$("$VARVE" inspect --layer "$LAYER" | awk '$2 == "tool" && $3 == "meld" { print $4; exit }')"
  [[ -n "$VERSION" ]] || die "could not read the meld version from 'varve inspect --layer $LAYER'"
fi
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "not a version: '$VERSION'"
TAG="v$VERSION"

git ls-remote --exit-code --tags https://github.com/pulseengine/meld.git "refs/tags/$TAG" >/dev/null \
  || die "tag $TAG not found in pulseengine/meld"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/regen-cm-wast.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

echo "cloning pulseengine/meld $TAG"
git clone --quiet --depth 1 --branch "$TAG" https://github.com/pulseengine/meld.git "$WORK/meld"

echo "running meld's component-model ratchet"
OUT="$WORK/out"
mkdir -p "$OUT"
(
  cd "$WORK/meld"
  MELD_CM_WAST_OUT="$OUT" cargo test -p meld-core --test cm_wast_ratchet --release \
    -- --exact every_reference_component_matches_the_fuse_baseline
)

[[ -f "$OUT/manifest.json" ]] || die "meld wrote no manifest.json"

# Provenance checks: the manifest must name this meld version and the upstream
# commit we vendor.
GOT_VERSION="$(sed -n 's/.*"meld_version": *"\([^"]*\)".*/\1/p' "$OUT/manifest.json" | head -n 1)"
[[ "$GOT_VERSION" == "$VERSION" ]] || die "manifest meld_version '$GOT_VERSION' != requested '$VERSION'"
GOT_COMMIT="$(sed -n 's/.*"component_model_commit": *"\([^"]*\)".*/\1/p' "$OUT/manifest.json" | head -n 1)"
WANT_COMMIT="$(tr -d '[:space:]' < "$CM/upstream/COMMIT")"
[[ "$GOT_COMMIT" == "$WANT_COMMIT" ]] \
  || die "meld ran against component-model $GOT_COMMIT but tests/component-model/upstream is $WANT_COMMIT"
# The upstream tests meld used must be byte-identical to ours.
diff -r "$WORK/meld/tests/spec/component-model/test" "$CM/upstream/test" >/dev/null \
  || die "tests/component-model/upstream/test differs from meld's vendored copy at $TAG"

# Stage the new directory, then swap it in.
STAGE="$WORK/stage"
mkdir -p "$STAGE"
cp "$OUT/manifest.json" "$STAGE/manifest.json"
(cd "$OUT" && find test -name '*.fused.wasm' | LC_ALL=C sort) | while IFS= read -r f; do
  mkdir -p "$STAGE/$(dirname "$f")"
  cp "$OUT/$f" "$STAGE/$f"
done
[[ -n "$(find "$STAGE" -name '*.fused.wasm' -print -quit)" ]] || die "no fused modules were produced"

rm -rf "$CM/meld-$VERSION"
mv "$STAGE" "$CM/meld-$VERSION"
echo "wrote $CM/meld-$VERSION ($(find "$CM/meld-$VERSION" -name '*.fused.wasm' | wc -l | tr -d ' ') fused modules)"
echo "Next: remove any older tests/component-model/meld-* directory, run the runner, and re-baseline safety/cm-wast-baseline.txt."
