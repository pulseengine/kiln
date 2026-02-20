# wrt-build-core

Core build system library for WRT (WebAssembly Runtime) - provides the foundational building blocks for the unified cargo-wrt build tool.

## Overview

This library provides the centralized build system functionality that powers cargo-wrt. It serves as the single source of truth for all build operations, replacing the previous fragmented approach with justfile, xtask, and shell scripts.

## Architecture

The build system is organized around a central `BuildSystem` struct that manages workspace operations and coordinates various build tasks:

- **Build Operations**: Compilation of all WRT components
- **Test Execution**: Running unit, integration, and verification tests  
- **Safety Verification**: SCORE-inspired safety checks and formal verification
- **Documentation Generation**: API docs, guides, and verification reports
- **Coverage Analysis**: Code coverage metrics and reporting

## Design Principles

- **Single Source of Truth**: All build logic centralized in this library
- **AI-Friendly**: Clear, linear architecture for AI agent integration
- **Cross-Platform**: Works on all target platforms (std/no_std)
- **Functional Safety**: Supports ISO 26262, IEC 61508 compliance
- **Deterministic**: Reproducible builds with comprehensive caching

## Key Modules

- `build` - Core build system and workspace management
- `ci` - CI simulation and workflow validation
- `config` - Configuration management and ASIL levels
- `error` - Unified error handling for build operations
- `kani` - Formal verification with KANI
- `matrix` - Build matrix verification across configurations
- `test` - Test execution and result aggregation
- `verify` - Safety verification and compliance checks

## Usage

This library is typically used through the cargo-wrt CLI tool:

```bash
cargo install --path cargo-wrt
cargo-wrt build
cargo-wrt test
cargo-wrt verify --asil d
```

For programmatic usage:

```rust
use wrt_build_core::{BuildSystem, BuildConfig};

let build_system = BuildSystem::for_current_dir()?;
let results = build_system.build_all()?;
```

## Safety and Compliance

The build system supports multiple ASIL (Automotive Safety Integrity Level) configurations:

- **QM**: Quality Management (no specific safety requirements)
- **ASIL-A/B/C/D**: Progressive safety integrity levels

Safety verification includes:
- Formal verification with KANI
- Memory safety analysis
- MIRI checks
- Static analysis with Clippy
- Requirements traceability

## License

MIT - see LICENSE file for details.