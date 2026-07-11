#!/usr/bin/env python3
"""Turn `cargo-kiln testsuite --run-wast` stdout into a shields.io endpoint JSON.

Why this exists: the README spec-suite badge must be re-derived by CI on every
main run, never typed by hand (claim-verification skill: a number in prose
drifts from the evidence). CI pipes the suite's stdout here and publishes the
resulting JSON to the `badges` branch; the README's endpoint badge reads it.

Honesty rule baked in: the badge *message* leads with the FILE ratio, not the
assertion percentage. A file that fails to parse (e.g. the custom-descriptors
proposal) contributes zero assertions to both sides of the 99.98%, so the
assertion rate alone flatters exactly where a skeptic looks. The file ratio
cannot be spoofed that way, so it is the headline; the assertion rate rides
along as context. Colour is keyed to the file ratio for the same reason.

Usage: spec-badge.py <suite-stdout-file>   # writes JSON to stdout
Parses lines of the form the suite prints today:
    Files: 280 total, 261 passed, 19 failed
    Assertions: 65618 passed, 15 failed
Exits non-zero (leaving no JSON) if it cannot find the file totals, so CI fails
loud rather than publishing a bogus badge.
"""
import json
import re
import sys


def _find(pattern, text):
    m = re.search(pattern, text)
    return tuple(int(g) for g in m.groups()) if m else None


def color_for(ratio):
    # Keyed to the file ratio (the un-spoofable number), not the assertion %.
    if ratio >= 0.98:
        return "brightgreen"
    if ratio >= 0.90:
        return "green"
    if ratio >= 0.75:
        return "yellowgreen"
    if ratio >= 0.50:
        return "yellow"
    return "orange"


def main(argv):
    if len(argv) != 2:
        sys.stderr.write("usage: spec-badge.py <suite-stdout-file>\n")
        return 2
    with open(argv[1], encoding="utf-8", errors="replace") as fh:
        text = fh.read()

    files = _find(r"Files:\s*(\d+)\s*total,\s*(\d+)\s*passed", text)
    asserts = _find(r"Assertions:\s*(\d+)\s*passed,\s*(\d+)\s*failed", text)
    if files is None:
        sys.stderr.write("spec-badge: could not parse the 'Files:' total line\n")
        return 1

    files_total, files_passed = files
    file_ratio = files_passed / files_total if files_total else 0.0

    if asserts is not None:
        a_pass, a_fail = asserts
        a_total = a_pass + a_fail
        a_pct = (a_pass / a_total * 100.0) if a_total else 0.0
        message = f"{files_passed}/{files_total} files · {a_pct:.2f}% asserts"
    else:
        message = f"{files_passed}/{files_total} files"

    badge = {
        "schemaVersion": 1,
        "label": "wasm spec suite",
        "message": message,
        "color": color_for(file_ratio),
    }
    json.dump(badge, sys.stdout, separators=(",", ":"))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
