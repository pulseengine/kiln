#!/usr/bin/env bash
#
# Self-test for cm-wast-gate.sh: one fixture per failure mode plus the clean
# pass. Run it directly; it exits non-zero on the first wrong gate verdict.

set -euo pipefail

GATE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/cm-wast-gate.sh"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/cm-wast-gate-test.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

report() { # report <path:verdict>...
  {
    echo "| File | Verdict | Pass | Refused | Fail | First failure or refusal |"
    echo "|---|---|---|---|---|---|"
    for pv in "$@"; do
      echo "| ${pv%%:*} | ${pv##*:} | 1 | 0 | 0 | a reason that says fail and pass |"
    done
  } > "$TMP/report.md"
}

baseline() { # baseline <verdict:path>...
  : > "$TMP/baseline.txt"
  for vp in "$@"; do
    printf '%s\t%s\n' "${vp%%:*}" "${vp#*:}" >> "$TMP/baseline.txt"
  done
}

expect() { # expect <name> <want exit code>
  local rc=0
  "$GATE" "$TMP/report.md" "$TMP/baseline.txt" > "$TMP/out.txt" 2>&1 || rc=$?
  if [[ "$rc" != "$2" ]]; then
    echo "FAIL $1: exit $rc, wanted $2" >&2
    cat "$TMP/out.txt" >&2
    exit 1
  fi
  echo "ok   $1 (exit $rc)"
}

report test/a.wast:pass test/b.wast:fail test/c.wast:refused-at-fuse
baseline fail:test/b.wast refused-at-fuse:test/c.wast
expect "clean pass" 0

report test/a.wast:pass test/b.wast:fail test/c.wast:fail
expect "refused-at-fuse got worse" 1

report test/a.wast:fail test/b.wast:fail test/c.wast:refused-at-fuse
expect "unbaselined failure" 1

report test/a.wast:refused-at-fuse test/b.wast:fail test/c.wast:refused-at-fuse
expect "unbaselined refusal" 1

report test/a.wast:pass test/b.wast:pass test/c.wast:refused-at-fuse
expect "baselined file now passes (stale)" 1

report test/a.wast:pass test/b.wast:refused-at-fuse test/c.wast:refused-at-fuse
expect "fail improved to refused-at-fuse (notice only)" 0
grep -q '^NOTICE improved: test/b.wast' "$TMP/out.txt"

report test/a.wast:pass test/c.wast:refused-at-fuse
expect "baselined file missing from the report" 1

report
expect "empty report" 2

report test/a.wast:pass test/b.wast:fail test/c.wast:refused-at-fuse
baseline pass:test/b.wast
expect "bad baseline verdict" 2

echo "all gate fixtures behaved"
