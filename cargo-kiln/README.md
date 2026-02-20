# cargo-kiln

Unified build tool for Kiln (WebAssembly Runtime) - the next-generation safety-critical WebAssembly runtime.

## Installation

### From crates.io (once published)
```bash
cargo install cargo-kiln
```

### From source
```bash
git clone https://github.com/pulseengine/kiln
cd kiln
cargo install --path cargo-kiln
```

### For development
```bash
cargo build -p cargo-kiln
# Binary will be available at ./target/debug/cargo-kiln
```

## Usage

cargo-kiln supports two usage patterns:

### 1. Direct Usage
```bash
cargo-kiln --help
cargo-kiln build
cargo-kiln test
```

### 2. Cargo Subcommand
```bash
cargo kiln --help
cargo kiln build  
cargo kiln test
```

Both patterns work identically - the same binary automatically detects how it's being called and adjusts accordingly.

### Available Commands

#### Core Build & Test Commands
- `build` - Build all Kiln components
- `test` - Run tests across the workspace  
- `check` - Run static analysis (clippy + formatting)
- `clean` - Clean build artifacts
- `coverage` - Run coverage analysis
- `docs` - Generate documentation

#### Safety & Verification Commands
- `verify` - Run safety verification and compliance checks
- `verify-matrix` - Comprehensive build matrix verification for ASIL compliance
- `kani-verify` - Run KANI formal verification
- `validate` - Run code validation checks
- `no-std` - Verify no_std compatibility

#### WebAssembly Tools
- `wasm verify <file>` - Verify WebAssembly module
- `wasm imports <file>` - List imports from module
- `wasm exports <file>` - List exports from module  
- `wasm analyze <files...>` - Analyze multiple modules
- `wasm create-test <output>` - Create minimal test module

#### Requirements Management (SCORE Methodology)
- `requirements init` - Initialize requirements.toml
- `requirements verify` - Verify requirements against implementation  
- `requirements score` - Show compliance scores
- `requirements matrix` - Generate traceability matrix
- `requirements missing` - List requirements needing attention
- `requirements demo` - Demonstrate SCORE methodology

#### Development & CI Tools
- `ci` - Run comprehensive CI checks
- `simulate-ci` - Simulate CI workflow for local testing
- `setup` - Setup development environment and tools
- `tool-versions` - Manage tool version configuration
- `fuzz` - Run fuzzing tests
- `test-features` - Test feature combinations
- `testsuite` - WebAssembly test suite management

#### Specialized Commands
- `kilnd` - Build kilnd (WebAssembly Runtime Daemon) binaries

#### Advanced Features
- `help-diagnostics` - Comprehensive diagnostic system guide
- Diagnostic system with JSON output, caching, and filtering
- LSP-compatible structured output for IDE integration

### Examples

#### Basic Usage
```bash
# Setup development environment
cargo-kiln setup --check
cargo-kiln setup --all

# Build and test
cargo-kiln build
cargo-kiln test
cargo-kiln check

# Safety verification
cargo-kiln verify --asil d
cargo-kiln verify-matrix --report
```

#### Advanced Diagnostic System
```bash
# JSON output for tooling/AI agents
cargo-kiln build --output json

# Filter errors only with caching
cargo-kiln build --output json --filter-severity error --cache

# Show only new diagnostics since last run
cargo-kiln build --cache --diff-only

# Group diagnostics by file
cargo-kiln build --output json --group-by file

# Filter by specific tool (clippy, rustc, etc.)
cargo-kiln check --output json --filter-source clippy
```

#### WebAssembly Analysis
```bash
# Analyze WebAssembly modules
cargo-kiln wasm verify module.wasm
cargo-kiln wasm imports module.wasm
cargo-kiln wasm exports module.wasm
cargo-kiln wasm analyze *.wasm

# Create test modules
cargo-kiln wasm create-test test_module.wasm
```

#### Requirements Management (SCORE)
```bash
# Initialize and manage requirements
cargo-kiln requirements init
cargo-kiln requirements verify
cargo-kiln requirements score
cargo-kiln requirements matrix

# Check missing requirements
cargo-kiln requirements missing
cargo-kiln requirements demo
```

#### Comprehensive Verification
```bash
# Full ASIL compliance verification
cargo-kiln verify-matrix --asil d --report

# Formal verification with KANI
cargo-kiln kani-verify --asil-profile d

# Feature combination testing
cargo-kiln test-features --comprehensive

# WebAssembly test suite
cargo-kiln testsuite --validate
```

#### CI/CD Integration
```bash
# Simulate CI locally
cargo-kiln simulate-ci --profile asil-d

# Generate structured reports
cargo-kiln ci --output json
cargo-kiln verify --output json --filter-severity error
```

## Features

### Safety & Compliance
- **ASIL-D Compliance**: Full automotive safety integrity verification
- **Requirements Traceability**: SCORE methodology implementation
- **Formal Verification**: KANI integration for mathematical proofs
- **Build Matrix Verification**: Comprehensive configuration testing
- **No-Std Compatibility**: Safety-critical embedded environment support

### WebAssembly Tooling
- **Module Analysis**: Import/export inspection and validation
- **Resource Limits**: Embedding safety constraints into binaries
- **Test Suite Management**: Comprehensive WebAssembly testing
- **Binary Verification**: Structural and semantic validation

### Advanced Diagnostics
- **LSP-Compatible Output**: JSON format for IDE integration
- **Intelligent Caching**: Incremental analysis with change detection
- **Multi-Tool Integration**: Unified output from rustc, clippy, miri, kani
- **Filtering & Grouping**: Precise diagnostic control and organization
- **Performance Monitoring**: Build time analysis and optimization

### Development Experience
- **Smart Tool Management**: Automatic dependency detection and installation
- **Version Consistency**: Reproducible builds across environments
- **CI/CD Integration**: Local simulation and structured reporting
- **Feature Testing**: Comprehensive feature combination validation
- **Modern Architecture**: AI-friendly, linear build processes

## Tool Management

cargo-kiln automatically detects and manages external tool dependencies:

### Required Tools (Usually Available)
- `cargo` - Rust package manager
- `rustc` - Rust compiler

### Optional Tools (Installed as Needed)  
- `kani` - Formal verification (for `verify` and `kani-verify` commands)
- `cargo-fuzz` - Fuzzing support (for `fuzz` command)
- `git` - Version control (for `setup` command)

### Setup Commands
```bash
# Check what tools are available
cargo-kiln setup --check

# Install missing optional tools
cargo-kiln setup --install

# Complete development setup
cargo-kiln setup --all
```

### Tool Version Management

cargo-kiln now includes sophisticated tool version management with configurable requirements:

```bash
# Check all tool versions against requirements
cargo-kiln tool-versions check

# Show detailed version information
cargo-kiln tool-versions check --verbose

# Check specific tool only
cargo-kiln tool-versions check --tool kani

# Generate tool-versions.toml configuration file
cargo-kiln tool-versions generate

# Generate comprehensive configuration (all tools)
cargo-kiln tool-versions generate --all

# Update existing configuration (future feature)
cargo-kiln tool-versions update --all
```

The tool version system uses a `tool-versions.toml` file in the workspace root to specify:
- Exact version requirements (e.g., kani must be exactly 0.63.0)
- Minimum version requirements (e.g., git must be at least 2.30.0)
- Installation commands for each tool
- Which cargo-kiln commands require each tool

This ensures reproducible builds and consistent development environments across all contributors.

When you run a command that needs a missing tool, cargo-kiln will:
1. Detect the missing tool
2. Show a helpful error message
3. Provide exact installation commands
4. Suggest running `cargo-kiln setup --install`

## License

MIT - see LICENSE file for details.