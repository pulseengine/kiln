#!/usr/bin/env bash
#
# WAST conformance regression gate.
#
# Conformance is reproducible because external/testsuite is pinned (gitlink ==
# checkout); see kiln#360. This gate runs the suite, extracts the per-file
# failing set from the runner's own markdown report (--wast-report), and FAILS
# if any file fails that is NOT in the checked-in baseline — i.e. a regression.
# A baseline file that now PASSES is reported as progress (tighten the baseline)
# but does not fail the gate. We gate on the failing-FILE set (stable), not the
# assertion count.
#
# Usage: wast-conformance-gate.sh <report.md> <baseline.txt>

set -euo pipefail

REPORT="${1:?usage: wast-conformance-gate.sh <report.md> <baseline.txt>}"
BASELINE="${2:?usage: wast-conformance-gate.sh <report.md> <baseline.txt>}"

if [[ ! -f "$REPORT" ]]; then
  echo "::error::WAST report not found: $REPORT (did the suite run?)" >&2
  exit 2
fi

current="$(mktemp)"
# The report's "## File Results" table rows look like: `| foo.wast | ❌ Failed | ...`
grep "❌ Failed" "$REPORT" | sed -E 's/^\| *//; s/ *\|.*//' | sort -u > "$current"

regressions="$(comm -13 "$BASELINE" "$current" || true)"
fixed="$(comm -23 "$BASELINE" "$current" || true)"

echo "Known-failing baseline: $(wc -l < "$BASELINE" | tr -d ' ') files | failing now: $(wc -l < "$current" | tr -d ' ') files"

if [[ -n "$fixed" ]]; then
  echo "::notice::WAST files newly passing (tighten the baseline):"
  echo "$fixed"
fi

if [[ -n "$regressions" ]]; then
  echo "::error::WAST conformance REGRESSION — files now failing that were NOT in the baseline:" >&2
  echo "$regressions" >&2
  rm -f "$current"
  exit 1
fi

echo "✅ No WAST conformance regressions."
rm -f "$current"
