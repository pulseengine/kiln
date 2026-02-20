# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Kiln (WebAssembly Runtime) Project Guidelines

## Quick Setup

```bash
# Install the unified build tool
cargo install --path cargo-kiln

# Verify installation
cargo-kiln --help
```

## Important Rules
- NEVER create hardcoded examples in the runtime code. Real implementations should parse or process actual external files.
- NEVER add dummy or simulated implementations except in dedicated test modules.
- Any example code MUST be in the `example/` directory or test files, not in the runtime implementation.
- The runtime should be able to execute real WebAssembly modules without special-casing specific files.
- Only use placeholders when absolutely necessary and clearly document them.
- **NO FALLBACK LOGIC**: NEVER add fallback code that masks bugs or incomplete implementations. Fallbacks prevent us from seeing real issues and create technical debt. If something doesn't work, fix the root cause - don't add a fallback. Examples of banned fallbacks:
  - `unwrap_or_else(|| default_value)` when the value should always exist
  - `if lookup_fails { use_hardcoded_assumption }` instead of proper lookup
  - "Fallback to instance X" when proper mapping should exist
  - Any code that says "this is a temporary workaround"
  - "Try instance 0 as fallback" or similar guessing logic
- **SPEC COMPLIANCE ONLY**: NEVER add fallback behavior that is not part of the WebAssembly or WebAssembly Component Model specification. If the spec says something must exist (like `_start` for command components), then it MUST exist - don't guess or substitute. Fallbacks create a false sense of progress and mask real implementation gaps.
- **FAIL LOUD AND EARLY**: If data is missing or incorrect, return an error immediately. Don't silently substitute defaults or guess values.
- **FIX INTERFACES, DON'T WORK AROUND THEM**: When interfaces don't match (e.g., one component expects `&mut [u8]` but another provides `read()`/`write()` methods), FIX the interface properly - don't create copy-in/copy-out workarounds or temporary buffers. Create proper abstraction traits that both sides can implement. Examples:
  - If `HostImportHandler` expects `&mut [u8]` but `Memory` uses Mutex-protected access → Create `MemoryAccessor` trait
  - If component A expects sync and B is async → Create adapter trait, don't block/spawn threads as workaround
  - Copy-in/copy-out patterns are BANNED - they waste memory and break safety guarantees
- **NO DEAD CODE**: Remove unused code rather than leaving it "for later". Dead code:
  - Increases cognitive load and maintenance burden
  - Creates false confidence when stubs silently return success
  - Accumulates as technical debt that becomes harder to remove
  - Skews test coverage metrics
  - Rules for dead code:
    - If code is unused and has no clear immediate purpose → **DELETE IT**
    - If a trait requires methods you can't implement → **Return explicit `Error::not_implemented_error()`**, never silent `Ok()`
    - If exports are commented out (e.g., `// pub use foo::...`) → **Either implement fully or delete entirely**
    - If entire modules are unused → **Delete the module and its `mod` declaration**
    - Stub methods that return `Ok(0)` or `Ok(())` without doing anything are **BANNED**
- **USE TRACING FRAMEWORK**: Always use the kiln_foundation::tracing framework for logging and debugging instead of eprintln! or println!. The tracing framework provides:
  - Structured logging with spans for context (ModuleTrace, ImportTrace, ExecutionTrace, MemoryTrace, etc.)
  - Proper log levels (trace!, debug!, info!, warn!, error!)
  - Integration with RUST_LOG environment variable for filtering
  - Import with: `#[cfg(feature = "tracing")] use kiln_foundation::tracing::{debug, error, info, trace, warn, span, Level};`
  - Use spans to provide context: `let span = span!(Level::INFO, "operation_name", field = value);`
  - Never use eprintln! or println! for debugging - always use the tracing framework

## Memory System Architecture

### Unified Capability-Based Memory System
The Kiln project uses a **single, unified memory management system** based on capabilities:

- **Primary API**: `safe_managed_alloc!(size, crate_id)` - ALL memory allocation goes through this macro
- **Factory System**: `CapabilityKilnFactory` - Capability-based factory for advanced use cases
- **Automatic Cleanup**: RAII-based automatic memory management
- **NO LEGACY PATTERNS**: All `NoStdProvider::<SIZE>::default()` patterns have been eliminated

### Memory Allocation Pattern
```rust
use kiln_foundation::{safe_managed_alloc, CrateId};

// Standard allocation pattern
let provider = safe_managed_alloc!(4096, CrateId::Component)?;
let mut vec = BoundedVec::new(provider)?;

// Memory is automatically cleaned up when provider goes out of scope
```

### Important Memory Guidelines
- **NO legacy patterns**: The codebase has been completely cleaned of ALL legacy memory patterns
- **NO NoStdProvider patterns**: NEVER use `NoStdProvider::<SIZE>::default()` - completely eliminated
- **NO fallback patterns**: No `unwrap_or_else(|_| NoStdProvider::default())` patterns allowed
- **NO unsafe extraction**: There is no `unsafe { guard.release() }` pattern anymore
- **SINGLE SYSTEM**: Only one memory system exists - capability-based allocation via `safe_managed_alloc!`
- **PROPER ERROR HANDLING**: All memory allocation failures must be handled via `Result` types and `?` operator

## Build Commands & Diagnostic System

The Kiln project uses a unified build system with `cargo-kiln` as the single entry point for all build operations. All legacy shell scripts have been migrated to this unified system.

**Usage Patterns:**
- Direct: `cargo-kiln <COMMAND>`
- Cargo subcommand: `cargo kiln <COMMAND>`

Both patterns work identically and use the same binary.

### Advanced Diagnostic System

The build system includes a comprehensive diagnostic system with LSP-compatible structured output, caching, filtering, grouping, and differential analysis. This system works across all major commands: `build`, `test`, `verify`, and `check`.

**Key Features:**
- **Structured Output**: LSP-compatible JSON for tooling/AI integration
- **Caching**: File-based caching with change detection for faster incremental builds
- **Filtering**: By severity, source tool, file patterns
- **Grouping**: Organize diagnostics by file, severity, source, or error code
- **Diff Mode**: Show only new/changed diagnostics since last cached run

**Global Diagnostic Flags (work with build, test, verify, check):**
```bash
# Output format control
--output human|json|json-lines    # Default: human

# Caching control
--cache                           # Enable diagnostic caching
--clear-cache                     # Clear cache before running
--diff-only                       # Show only new/changed diagnostics

# Filtering options
--filter-severity LIST            # error,warning,info,hint
--filter-source LIST              # rustc,clippy,miri,cargo
--filter-file PATTERNS            # *.rs,src/* (glob patterns)

# Grouping and pagination
--group-by CRITERION              # file|severity|source|code
--limit NUMBER                    # Limit diagnostic output count
```

### Core Commands
- Build all: `cargo-kiln build` or `cargo build`
- Build specific crate: `cargo-kiln build --package kiln|kilnd|example`
- Clean: `cargo-kiln clean` or `cargo clean`
- Run tests: `cargo-kiln test` or `cargo test`
- Run single test: `cargo test -p kiln -- test_name --nocapture`
- Format code: `cargo-kiln check` or `cargo fmt`
- Format check: `cargo-kiln check --strict`
- Main CI checks: `cargo-kiln ci`
- Full CI suite: `cargo-kiln verify --asil d`
- Typecheck: `cargo check`

### Diagnostic Usage Examples

**Basic Error Analysis:**
```bash
# Get all errors in JSON format
cargo-kiln build --output json --filter-severity error

# Focus on specific file patterns
cargo-kiln build --output json --filter-file "kiln-foundation/*"

# Get clippy suggestions only
cargo-kiln check --output json --filter-source clippy
```

**Incremental Development Workflow:**
```bash
# Initial baseline (clears cache and establishes new baseline)
cargo-kiln build --output json --cache --clear-cache

# Subsequent runs - see only new issues
cargo-kiln build --output json --cache --diff-only

# Focus on errors only in diff mode
cargo-kiln build --output json --cache --diff-only --filter-severity error
```

**Code Quality Analysis:**
```bash
# Group warnings by file for focused fixes
cargo-kiln check --output json --filter-severity warning --group-by file

# Limit output for manageable chunks
cargo-kiln check --output json --limit 10 --filter-severity warning

# Multiple tool analysis
cargo-kiln verify --output json --filter-source "rustc,clippy,miri"
```

**CI/CD Integration:**
```bash
# Fail-fast on any errors
cargo-kiln build --output json | jq '.summary.errors > 0' && exit 1

# Stream processing for large outputs
cargo-kiln build --output json-lines | while read diagnostic; do
  echo "$diagnostic" | jq '.severity'
done

# Generate reports for specific tools
cargo-kiln verify --output json --filter-source kani,miri
```

**JSON Processing with jq:**
```bash
# Extract error messages
cargo-kiln build --output json | jq '.diagnostics[] | select(.severity == "error") | .message'

# Count diagnostics by file
cargo-kiln build --output json | jq '.diagnostics | group_by(.file) | map({file: .[0].file, count: length})'

# Get files with errors
cargo-kiln build --output json | jq '.diagnostics[] | select(.severity == "error") | .file' | sort -u
```

### Advanced Commands
- Setup and tool management: `cargo-kiln setup --check` or `cargo-kiln setup --all`
- Fuzzing: `cargo-kiln fuzz --list` to see targets, `cargo-kiln fuzz` to run all
- Validation: `cargo-kiln validate` for comprehensive validation
- Platform verification: `cargo-kiln verify --asil <level>` with ASIL compliance
- Build matrix verification: `cargo-kiln verify-matrix --asil <level> --report`
- Requirements traceability: `cargo-kiln requirements verify`
- Requirements management: `cargo-kiln requirements init|score|matrix|missing|demo`
- No-std validation: `cargo-kiln no-std` 
- KANI formal verification: `cargo-kiln kani-verify --asil-profile <level>`
- WebAssembly analysis: `cargo-kiln wasm verify|imports|exports|analyze|create-test`
- Feature testing: `cargo-kiln test-features --comprehensive`
- CI simulation: `cargo-kiln simulate-ci --profile <profile>`
- WebAssembly test suite: `cargo-kiln testsuite --validate`

### Tool Management
The build system includes sophisticated tool version management with configurable requirements:

- Check tool status: `cargo-kiln setup --check`
- Install optional tools: `cargo-kiln setup --install` 
- Complete setup: `cargo-kiln setup --all`

**Tool Version Management:**
- Check all tool versions: `cargo-kiln tool-versions check --verbose`
- Generate tool configuration: `cargo-kiln tool-versions generate`
- Check specific tool: `cargo-kiln tool-versions check --tool kani`

Tool versions are managed via `tool-versions.toml` in the workspace root, specifying exact/minimum version requirements and installation commands. This ensures reproducible builds and consistent development environments.

## Build Matrix Verification
- **Comprehensive verification**: `cargo-kiln verify-matrix --report`
  - Tests all ASIL-D, ASIL-C, ASIL-B, development, and server configurations
  - Performs architectural analysis to identify root causes of failures
  - Generates detailed reports on ASIL compliance issues
  - CRITICAL: Run this before any architectural changes or feature additions
  - Required for all PRs that modify core runtime or safety-critical components

When build failures occur, the script will:
1. Analyze the root cause (not just symptoms)
2. Identify if issues are architectural problems affecting ASIL levels
3. Generate reports with specific remediation steps
4. Classify failures by their impact on safety compliance

## Debugging Rust String Literal Errors

### The "Nasty Rust Problem"
When encountering "prefix is unknown" or "unterminated double quote string" errors:

1. **DO NOT trust the line number** - Rust reports where it detected the issue, not where it started
2. **Search for the actual unterminated quotes** using:
   ```bash
   grep -n '"' filename.rs | awk '{line = $0; count = gsub(/"/, "", line); if (count % 2 == 1) print $0}'
   ```
3. **Common patterns to look for**:
   - `Error::runtime_execution_error(",` (missing closing quote and message)
   - `.ok_or_else(|| Error::runtime_error("))?;` (missing opening quote and message)
   - `#[cfg(feature = ")]` (incomplete feature flag)
4. **Fix at the source** - Add the missing error message, don't just add spaces at the reported line

## ⚠️ CRITICAL: NO MASS CHANGES WITH sed/awk/python

**NEVER use sed, awk, python scripts, or any mass text replacement tools on this codebase.**

This codebase has previously suffered from systematic corruption caused by faulty sed replacements that broke syntax across hundreds of files. The damage included:
- Missing closing parentheses (`;` instead of `);`)
- Malformed struct initializations (`;` instead of `};`) 
- Broken function calls and array syntax
- Corrupted assert macros and error constructors

**ONLY use direct Edit/MultiEdit tools for code changes.** Each change must be:
- Carefully reviewed and tested on individual files
- Applied with precise context to avoid unintended replacements
- Verified through compilation before proceeding

**If you encounter systematic issues, fix them one file at a time using Edit commands, never with mass replacement tools.**

## Code Style Guidelines

### General Formatting
- Use 4-space indentation
- Follow Rust naming conventions: snake_case for functions/variables, CamelCase for types
- Constants and statics use SCREAMING_SNAKE_CASE
- Use conventional commits: `type(scope): short summary`

### Import Organization
Organize imports in the following order:
1. Module attributes (`#![no_std]`, etc.)
2. `extern crate` declarations
3. Standard library imports (std, core, alloc) - grouped by feature flags
4. External crates/third-party dependencies
5. Internal crates (kiln_* imports)
6. Module imports (crate:: imports)
7. Each group should be separated by a blank line

Example:
```rust
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;

use thiserror::Error;

use kiln_foundation::prelude::*;

use crate::types::Value;
```

### Type Definitions
- Always derive Debug, Clone, PartialEq, Eq for data structures
- Add Hash, Ord when semantically appropriate
- Document why if any standard derives are omitted
- Use manual error implementations (not thiserror) for better no_std compatibility

### Documentation Standards
- All modules MUST have `//!` module-level documentation
- All public items MUST have `///` documentation
- Use `//` for implementation comments
- Include `# Safety` sections for unsafe code
- Examples in docs should be tested (use `no_run` if needed)

### Error Handling

#### Unified Error System
Kiln uses a unified error handling system based on `kiln_error::Error` with ASIL-compliant categorization:

**Core Principles:**
- All runtime crates must use `kiln_error::Error` as the base error type
- Use factory methods instead of direct `Error::new()` construction
- Maintain ASIL compliance through proper error categorization
- Ensure no_std compatibility across all error handling

#### Error Categories with ASIL Levels
```rust
// ASIL-D (highest safety level)
ErrorCategory::Safety           // Safety violations, integrity checks
ErrorCategory::FoundationRuntime // Bounded collection violations

// ASIL-C (high safety level)  
ErrorCategory::Memory           // Memory allocation/management errors
ErrorCategory::RuntimeTrap      // WebAssembly trap conditions
ErrorCategory::ComponentRuntime // Component threading, resources

// ASIL-B (medium safety level)
ErrorCategory::Validation       // Input validation failures
ErrorCategory::Type             // Type system violations  
ErrorCategory::PlatformRuntime  // Hardware, real-time constraints
ErrorCategory::AsyncRuntime     // Async/threading runtime errors

// QM (Quality Management - non-safety-critical)
ErrorCategory::Parse            // Parsing errors
ErrorCategory::Core             // General WebAssembly errors
```

#### Factory Method Usage (Preferred)
```rust
// Use factory methods for common error patterns
let error = kiln_error::Error::platform_memory_allocation_failed("Buffer allocation failed");
let error = kiln_error::Error::component_thread_spawn_failed("Thread spawn resource limit exceeded");
let error = kiln_error::Error::foundation_bounded_capacity_exceeded("BoundedVec capacity exceeded");

// Available factory methods by category:

// Memory errors (Memory category)
Error::memory_error(message)
Error::memory_out_of_bounds(message)

// Component Runtime errors (ComponentRuntime category)
Error::component_thread_spawn_failed(message)
Error::component_handle_representation_error(message)
Error::component_resource_lifecycle_error(message)
Error::component_capability_denied(message)

// Platform Runtime errors (PlatformRuntime category)  
Error::platform_memory_allocation_failed(message)
Error::platform_thread_creation_failed(message)
Error::platform_realtime_constraint_violated(message)

// Foundation Runtime errors (FoundationRuntime category)
Error::foundation_bounded_capacity_exceeded(message)
Error::foundation_memory_provider_failed(message)
Error::foundation_verification_failed(message)

// Async Runtime errors (AsyncRuntime category)
Error::async_task_spawn_failed(message)
Error::async_fuel_exhausted(message)
Error::async_channel_closed(message)
```

#### Error Construction Guidelines
1. **Factory Methods First**: Always check if a factory method exists for your error pattern
2. **From Trait Implementation**: Implement `From<YourError> for kiln_error::Error` for crate-specific errors
3. **Result Type Aliases**: Use `kiln_error::Result<T>` consistently across crates
4. **Direct Construction**: Only use `Error::new()` when no factory method exists

#### From Trait Implementation Pattern
```rust
impl From<YourCrateError> for kiln_error::Error {
    fn from(err: YourCrateError) -> Self {
        use kiln_error::{ErrorCategory, codes};
        match err.kind {
            YourErrorKind::ResourceLimit => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_RESOURCE_LIFECYCLE_ERROR,
                "Resource limit exceeded",
            ),
            // ... other variants
        }
    }
}
```

#### Crate-Specific Guidelines
- **kiln-component**: Use ComponentRuntime category for threading, virtualization, resource errors
- **kiln-foundation**: Use FoundationRuntime category for bounded collection violations
- **kiln-platform**: Use PlatformRuntime category for hardware, real-time constraint errors
- **kiln-runtime**: Use appropriate category based on error context (Memory, RuntimeTrap, etc.)

#### Code Standards
- NO `.unwrap()` in production code except:
  - Constants/static initialization
  - Documented infallible operations (with safety comment)
- Use `kiln_error::Result<T>` consistently instead of `Result<T, CrateError>`
- Implement proper error context through factory method selection

### Testing Standards
- Unit tests: Use `#[cfg(test)] mod tests {}` in source files
- Integration tests: Place in `tests/` directory only
- No test files in `src/` directory (except `#[cfg(test)]` modules)
- Test naming: `test_<functionality>` or `<functionality>_<condition>`

### Feature Flags
- Use simple, clear feature flag patterns
- Group feature-gated imports together
- Avoid redundant feature checks

### Type Consistency
- WebAssembly spec uses u32 for sizes
- Convert between u32 and usize explicitly when working with Rust memory
- Break cyclic references with Box<T> for recursive types
- Handle no_std environments with appropriate macro imports

## ASIL Compliance Requirements
When working on safety-critical components (ASIL-D, ASIL-C, ASIL-B):
1. **Always run `cargo-kiln verify-matrix --report` before committing**
2. **No unsafe code** in safety-critical configurations
3. **Deterministic compilation** - all feature combinations must build consistently
4. **Memory budget compliance** - no dynamic allocation after initialization for ASIL-D
5. **Module boundaries** - clear separation between safety-critical and non-critical code
6. **Formal verification** - Kani proofs must pass for safety-critical paths

## Architectural Analysis
The build matrix verification performs deep architectural analysis:
- **Dependency cycles** that violate ASIL modular design
- **Feature flag interactions** that create compilation conflicts
- **Memory allocation patterns** incompatible with no_std requirements
- **Trait coherence issues** indicating poor abstraction boundaries
- **Import/visibility problems** breaking deterministic builds

If architectural issues are detected, they must be resolved before merging, as they directly impact ASIL compliance and safety certification.

## Architecture Notes

### Memory System Migration (Completed)
The Kiln project has successfully migrated to a unified capability-based memory system:

- **Unified allocation**: All memory goes through `safe_managed_alloc!` macro
- **Capability verification**: Every allocation is capability-checked
- **Budget enforcement**: Automatic per-crate budget tracking
- **RAII cleanup**: Automatic memory management via Drop
- **Zero legacy patterns**: All old patterns have been eliminated

### Build System Migration (Completed)
The Kiln project has completed its migration to a unified build system:

- **cargo-kiln**: Single CLI entry point for all build operations
- **kiln-build-core**: Core library containing all build logic and functionality
- **Legacy cleanup**: All shell scripts and fragmented build tools have been removed
- **Integration**: Former kiln-verification-tool functionality integrated into kiln-build-core
- **API consistency**: All commands follow consistent patterns and error handling

### Removed Legacy Components
- Shell scripts: `verify_build.sh`, `fuzz_all.sh`, `verify_no_std.sh`, `test_features.sh`, `documentation_audit.sh`
- Kani verification scripts: `test_kani_phase4.sh`, `validate_kani_phase4.sh`
- justfile and xtask references (functionality ported to kiln-build-core)
- Legacy memory patterns: `KILN_MEMORY_COORDINATOR`, `KilnProviderFactory`, `unsafe { guard.release() }`

## Current System Status

### Memory Architecture
- **Single System**: 100% capability-based memory management
- **Consistent API**: `safe_managed_alloc!()` throughout - NO EXCEPTIONS
- **NO Legacy Patterns**: All `NoStdProvider::<SIZE>::default()` patterns eliminated
- **Modern Factory**: `CapabilityKilnFactory` for advanced use cases
- **Automatic Cleanup**: RAII-based memory management
- **Proper Error Handling**: All allocation failures handled via `Result` types

### Build System
- **Unified Tool**: `cargo-kiln` for all operations
- **Consistent Commands**: Same patterns across all functionality
- **Tool Management**: Automated version checking and installation
- **ASIL Verification**: Built-in safety compliance checking

### JSON Diagnostic Format Specification

The JSON output follows LSP (Language Server Protocol) specification for maximum compatibility:

```json
{
  "version": "1.0",
  "timestamp": "2025-06-21T11:39:57.067142+00:00",
  "workspace_root": "/Users/r/git/kiln",
  "command": "build",
  "diagnostics": [
    {
      "file": "kiln-foundation/src/capabilities/factory.rs",
      "range": {
        "start": {"line": 17, "character": 29},
        "end": {"line": 17, "character": 53}
      },
      "severity": "warning",
      "code": "deprecated",
      "message": "use of deprecated struct `CapabilityFactoryBuilder`",
      "source": "rustc",
      "related_info": [
        {
          "file": "kiln-foundation/src/lib.rs",
          "range": {
            "start": {"line": 29, "character": 4},
            "end": {"line": 29, "character": 16}
          },
          "message": "the lint level is defined here"
        }
      ]
    }
  ],
  "summary": {
    "total": 221,
    "errors": 7,
    "warnings": 214,
    "infos": 0,
    "hints": 0,
    "files_with_diagnostics": 29,
    "duration_ms": 1486
  }
}
```

**Key Field Explanations:**
- `file`: Relative path from workspace root
- `range`: LSP-compatible position (0-indexed line/character)
- `severity`: "error"|"warning"|"info"|"hint"
- `code`: Tool-specific error/warning code (optional)
- `source`: Tool that generated diagnostic ("rustc", "clippy", "miri", etc.)
- `related_info`: Array of related diagnostic locations

**Performance Benefits:**
- Initial run: Full analysis (3-4 seconds)
- Cached run: Incremental analysis (~0.7 seconds)
- Diff-only: Shows only changed diagnostics (filtering ~5-10% of output)

## Memories
- Build and test with `cargo-kiln` commands
- Memory allocation uses `safe_managed_alloc!` macro EXCLUSIVELY - NO EXCEPTIONS
- ALL NoStdProvider::<SIZE>::default() patterns ELIMINATED - no fallbacks allowed
- All legacy patterns removed from code, comments, and documentation
- Memory allocation failures handled via Result types and ? operator
- All builds must pass ASIL verification before merging
- Legacy memory system migration IN PROGRESS - ~344 instances remaining to eliminate
- **Test execution**: Tests that conflict on global resources use `#[serial_test::serial]` attribute
  - CFI engine tests share global capability-based memory budget and run serially
  - Most tests run in parallel for speed
  - Use `serial_test` crate to mark new tests that need serial execution
- **Diagnostic system**: Use `--output json` for structured output, `--cache --diff-only` for incremental analysis
- **AI Integration**: JSON output is LSP-compatible for IDE and tooling integration
- **Performance**: Caching reduces analysis time from 3-4s to ~0.7s on subsequent runs
## WAST (WebAssembly Script) Test Format

### Understanding WAST
WAST is the official WebAssembly test format - a superset of WAT (WebAssembly Text format) used for conformance testing. Key characteristics:

- **Not part of the official spec**: WAST is a testing tool only, defined in the interpreter specification
- **Quoted modules**: Modules can be defined as `(module quote <string>*)` where the strings are concatenated and parsed as WAT AT EXECUTION TIME
- **Binary modules**: Also support `(module binary <string>*)` for WASM binaries
- **Intentional parsing during execution**: This design allows testing for malformed modules with `assert_malformed`

### QuoteModule Handling
The `wast` crate represents quoted modules as:
- `QuoteWat::QuoteModule(Span, Vec<(Span, Vec<u8>)>)` - contains the quoted byte strings
- These must be:
  1. Concatenated together
  2. Parsed as WAT text (using `wast::parse()` from the ParseBuffer)
  3. Encoded to WASM binary
  4. Loaded and instantiated

This is fundamentally different from inline WAT modules which are already parsed.

### References
- [WebAssembly Specification 3.0 (2025-12-03)](https://webassembly.github.io/spec/core/)
- [WABT Test Documentation](https://github.com/WebAssembly/wabt/blob/main/test/README.md)
- [Official WebAssembly Interpreter](https://github.com/WebAssembly/spec/tree/main/interpreter)

### Excluded Tests
The following test directories are **permanently excluded** from Kiln testing:

#### `external/testsuite/legacy/` - Legacy Exception Handling
**Reason**: Kiln implements only the modern exception handling syntax (`try_table` + `exnref`) from the October 2023 WebAssembly CG decision. The legacy `try/catch/delegate/rethrow` syntax is deprecated and being phased out.

- **Legacy syntax** (NOT implemented): `try`, `catch`, `catch_all`, `delegate`, `rethrow`
- **Modern syntax** (implemented): `try_table`, `throw`, `throw_ref`, `exnref`

The legacy tests use opcodes 0x06-0x09 and 0x18-0x19 which we do not decode or execute.

Reference: [Exception Handling Proposal - Legacy](https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/legacy/Exceptions.md)

When running WAST tests, always exclude the legacy directory:
```bash
# Correct - runs modern exception tests
cargo-kiln testsuite --run-wast --wast-dir external/testsuite --wast-filter throw.wast

# WRONG - legacy tests will fail
cargo-kiln testsuite --run-wast --wast-dir external/testsuite/legacy
```

### Test Components
For testing WASI component execution, use the pre-built components from:
- [pulseengine/wasm-component-examples](https://github.com/pulseengine/wasm-component-examples/releases)

Download and extract with:
```bash
mkdir -p /tmp/wasm-components && cd /tmp/wasm-components
gh release download v0.2.0 --repo pulseengine/wasm-component-examples --pattern "wasm-components-0.2.0.tar.gz"
tar -xzf wasm-components-0.2.0.tar.gz
# Components are in release-0.2.0/{rust,c,cpp}/
```

Available test components:
- `rust/hello_rust.wasm` - Basic Rust CLI component
- `rust/calculator.wasm` - Calculator using get-arguments
- `rust/datetime.wasm` - Uses wasi:clocks
- `c/hello_c_cli.wasm` - C CLI component (uses terminal interfaces)
- `cpp/hello_cpp_cli.wasm` - C++ CLI component (uses terminal interfaces)

# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.