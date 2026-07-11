#!/usr/bin/env python3
"""claim-check — gate a repo's documentation claims against live evidence.

Reference implementation for the `claim-verification` skill. A repo drops a
`claims.yaml` next to its docs; CI runs this; drift between a claim, its recorded
ledger, and the *actual source* fails the build. Truth-over-time becomes a property
of the gate, not the author.

Usage:  claim-check.py [claims.yaml]     (default: ./claims.yaml)
Exit:   0 = all claims hold · 1 = one or more drifted

A repo may reimplement this in Rust (cf. tools/fetch-reports); the shape is what
matters. Evidence predicates re-derive from source — never a number typed in prose.

claims.yaml shape:
  claims:
    - id: LOOM-BADGE
      doc: README.md
      text: "Translation-Validated"          # must appear VERBATIM in `doc`
      evidence:
        - kind: file-exists
          path: proofs/Correctness.v
        - kind: count-max                      # re-count from source; fail if over
          pattern: '#\\[verifier::external_body\\]'
          glob: ['src/**/*.rs']
          max: 133
        - kind: no-new                         # no new sorry/admit since recorded
          pattern: '\\bAdmitted\\b'
          glob: ['proofs/**/*.v']
          recorded: 74
"""
import sys
import re
import glob
import pathlib

try:
    import yaml
except ImportError:
    sys.exit("claim-check: needs PyYAML  (pip install pyyaml)")


def _count(pattern, globs, root):
    rx = re.compile(pattern)
    globs = [globs] if isinstance(globs, str) else globs
    total = 0
    matched_any = False
    for g in globs:
        # Resolve globs relative to the claims file's directory, NOT the CWD —
        # otherwise the predicate silently matches nothing and greens a claim it
        # never checked (the "oracle that measures nothing" failure).
        for f in glob.glob(str(root / g), recursive=True):
            p = pathlib.Path(f)
            if p.is_file():
                matched_any = True
                total += len(rx.findall(p.read_text(errors="ignore")))
    return total, matched_any


def check_claim(c, root):
    fails = []
    doc_path = root / c["doc"]
    doc = doc_path.read_text(errors="ignore") if doc_path.exists() else ""
    if not doc_path.exists():
        return [f'doc not found: {c["doc"]}']

    text = c.get("text")
    if text and text not in doc:
        fails.append(f'claim text not found verbatim in {c["doc"]}: "{text}"')

    for ev in c.get("evidence", []):
        kind = ev.get("kind")
        if kind == "verbatim":
            s = ev.get("text", text)
            if s and s not in doc:
                fails.append(f'verbatim string absent from {c["doc"]}: "{s}"')
        elif kind == "file-exists":
            if not (root / ev["path"]).exists():
                fails.append(f'evidence file missing: {ev["path"]}')
        elif kind == "count-max":
            n, matched = _count(ev["pattern"], ev["glob"], root)
            if not matched:
                fails.append(f'predicate matched NO files (measures nothing): glob {ev["glob"]}')
            elif n > ev["max"]:
                fails.append(
                    f'trusted base grew: {n} > recorded max {ev["max"]}  '
                    f'[/{ev["pattern"]}/]  — update the claim, not the number'
                )
        elif kind == "no-new":
            n, matched = _count(ev["pattern"], ev["glob"], root)
            if not matched:
                fails.append(f'predicate matched NO files (measures nothing): glob {ev["glob"]}')
            elif n > ev.get("recorded", 0):
                fails.append(
                    f'new unproven obligations: {n} > recorded {ev.get("recorded", 0)}  '
                    f'[/{ev["pattern"]}/]'
                )
        else:
            fails.append(f'unknown evidence kind: {kind!r}')
    return fails


def main():
    path = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "claims.yaml")
    if not path.exists():
        sys.exit(f"claim-check: {path} not found")
    root = path.parent
    data = yaml.safe_load(path.read_text()) or {}
    claims = data.get("claims", [])
    if not claims:
        print("claim-check: no claims declared — nothing to gate.")
        return

    bad = 0
    for c in claims:
        fails = check_claim(c, root)
        if fails:
            bad += 1
            print(f"✗ {c['id']}")
            for f in fails:
                print(f"    {f}")
        else:
            print(f"✓ {c['id']}")

    print(f"\n{len(claims) - bad}/{len(claims)} claims hold.")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
