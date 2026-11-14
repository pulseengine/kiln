#!/usr/bin/env bash
#
# Component Model WAST ratchet gate (SR-93).
#
# Compares the per-file verdicts in the runner's markdown report
# (`cargo-kiln testsuite --run-cm-wast --cm-wast-report <md>`) with the
# checked-in baseline (`safety/cm-wast-baseline.txt`, lines of
# `<verdict><TAB><wast path>` for every file that does not pass).
#
# FAILS when
#   1. a file's verdict is worse than baselined (pass < refused-at-fuse < fail),
#   2. a file is not baselined and does not pass,
#   3. a baselined file now passes (the baseline must be tightened in the same
#      change; this is stricter than the core WAST gate), or
#   4. a baselined file is absent from the report.
# A file that improved from fail to refused-at-fuse is only a notice.
#
# Usage: cm-wast-gate.sh <report.md> <baseline.txt>
# Exit: 0 clean, 1 gate failure, 2 usage or unreadable input.

set -euo pipefail
export LC_ALL=C

REPORT="${1:?usage: cm-wast-gate.sh <report.md> <baseline.txt>}"
BASELINE="${2:?usage: cm-wast-gate.sh <report.md> <baseline.txt>}"

[[ -f "$REPORT" ]] || { echo "cm-wast-gate: report not found: $REPORT" >&2; exit 2; }
[[ -f "$BASELINE" ]] || { echo "cm-wast-gate: baseline not found: $BASELINE" >&2; exit 2; }

TMP="$(mktemp -d "${TMPDIR:-/tmp}/cm-wast-gate.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

# Report rows: `| <path> | <verdict> | ...`, anchored on both columns so a
# reason text that mentions "fail" cannot match.
awk -F'|' '
  /^\| test\/[^ |]+\.wast \| (pass|refused-at-fuse|fail) \|/ {
    p = $2; v = $3
    gsub(/^ +| +$/, "", p); gsub(/^ +| +$/, "", v)
    print p "\t" v
  }' "$REPORT" | sort > "$TMP/current"

if [[ ! -s "$TMP/current" ]]; then
  echo "cm-wast-gate: no file rows in $REPORT (did the runner produce a report?)" >&2
  exit 2
fi

# Baseline rows: `<verdict>\t<path>`, rewritten as `<path>\t<verdict>`.
awk -F'\t' '
  NF == 0 || $0 ~ /^[[:space:]]*$/ { next }
  NF != 2 || ($1 != "refused-at-fuse" && $1 != "fail") {
    printf "cm-wast-gate: bad baseline line %d: %s\n", NR, $0 > "/dev/stderr"; bad = 1; next
  }
  { print $2 "\t" $1 }
  END { exit bad }' "$BASELINE" | sort > "$TMP/baseline" || { echo "cm-wast-gate: unreadable baseline" >&2; exit 2; }

awk -F'\t' -v total="$(wc -l < "$TMP/current" | tr -d ' ')" '
  function rank(v) { return v == "pass" ? 0 : (v == "refused-at-fuse" ? 1 : 2) }
  FNR == NR { base[$1] = $2; next }
  { cur[$1] = $2 }
  END {
    nb = 0; for (p in base) nb++
    printf "Baseline: %d non-passing files | report: %d files\n", nb, total
    errors = 0
    for (p in cur) {
      v = cur[p]
      if (p in base) {
        if (rank(v) > rank(base[p])) {
          printf "ERROR worse: %s baselined %s, now %s\n", p, base[p], v; errors++
        } else if (v == "pass") {
          printf "ERROR stale: %s baselined %s, now passes (remove it from the baseline)\n", p, base[p]; errors++
        } else if (rank(v) < rank(base[p])) {
          printf "NOTICE improved: %s baselined %s, now %s (tighten the baseline)\n", p, base[p], v
        }
      } else if (v != "pass") {
        printf "ERROR unbaselined: %s is %s and not in the baseline\n", p, v; errors++
      }
    }
    for (p in base) {
      if (!(p in cur)) { printf "ERROR missing: baselined %s is not in the report\n", p; errors++ }
    }
    if (errors > 0) { printf "FAILED: %d problem(s)\n", errors; exit 1 }
    print "OK: the report matches the baseline"
  }' "$TMP/baseline" "$TMP/current" > "$TMP/out" || STATUS=$?

# awk visits files in arbitrary order: print the sorted findings, then the
# summary lines, and keep awk's exit status.
STATUS="${STATUS:-0}"
{ grep -E '^(ERROR|NOTICE)' "$TMP/out" || true; } | sort
grep -vE '^(ERROR|NOTICE)' "$TMP/out" || true
exit "$STATUS"
