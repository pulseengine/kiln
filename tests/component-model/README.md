# Component Model WAST on meld-fused output (SR-93)

`cargo-kiln testsuite --run-cm-wast` runs the upstream Component Model
reference tests through meld and the kiln interpreter.

- `upstream/` is `WebAssembly/component-model`'s `test/` directory at the
  commit in `upstream/COMMIT`, copied byte for byte from meld's vendored copy
  (with upstream's licences and `SHA256SUMS`; check with
  `cd upstream && shasum -a 256 -c SHA256SUMS`).
- `meld-<version>/` is what meld's `cm_wast_ratchet` test produced for those
  files: `manifest.json` (every component's meld outcome plus the WAST commands
  that target it) and the fused core module for each component meld fused
  (`test/**/*.fused.wasm`). The `input_path` entries in the manifest point at
  input modules that are not vendored, and the runner never opens them.

The runner parses each WAST itself, matches its directives to the manifest by
directive index (checking line and kind agree), and executes the commands on
the fused module in kiln. Each file gets one verdict:

- `pass`: every check passed.
- `refused-at-fuse`: meld refused at least one component (a feature meld does
  not support yet) and kiln failed nothing. This is not a kiln failure.
- `fail`: kiln failed at least one check, including values it cannot yet lower
  ("unsupported: component value lowering").

`safety/cm-wast-baseline.txt` lists every file that is not `pass`;
`scripts/cm-wast-gate.sh` fails CI on any difference from it.

## Regenerating

`scripts/regen-cm-wast.sh [meld-version]` clones meld at that release tag, runs
its ratchet, and replaces `meld-<version>/`. The default version is the one the
covalent layer pinned in `varve.toml` ships. After a regeneration, remove the
old `meld-*` directory, re-run the runner, and re-baseline.
