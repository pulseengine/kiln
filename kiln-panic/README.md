# kiln-panic

ASIL-B/D compliant panic handler for the Kiln WebAssembly Runtime.

Provides configurable panic handling for `no_std` and `std` environments with safety-level-aware behavior for automotive and safety-critical applications.

## Features

- `std` — Standard library panic handling with enhanced diagnostics
- `no_std` — Standalone panic handler for embedded targets
- `asil-b` — ASIL-B compliant panic handling (≥90% SPFM)
- `asil-d` — ASIL-D compliant panic handling (≥99% SPFM)
- `dev` — Development profile with enhanced debugging info
- `release` — Minimal, optimized production panic handler

## Usage

```toml
[dependencies]
kiln-panic = { workspace = true, features = ["std"] }
```

## License

See the workspace root for license information.
