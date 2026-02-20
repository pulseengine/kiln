# cargo-wrt

Unified build tool for WRT (WebAssembly Runtime) - the next-generation safety-critical WebAssembly runtime.

## Installation

### From crates.io (once published)
```bash
cargo install cargo-wrt
```

### From source
```bash
git clone https://github.com/pulseengine/wrt
cd wrt
cargo install --path cargo-wrt
```

### For development
```bash
cargo build -p cargo-wrt
# Binary will be available at ./target/debug/cargo-wrt
```

## Usage

cargo-wrt supports two usage patterns:

### 1. Direct Usage
```bash
cargo-wrt --help
cargo-wrt build
cargo-wrt test
```

### 2. Cargo Subcommand
```bash
cargo wrt --help
cargo wrt build  
cargo wrt test
```

Both patterns work identically - the same binary automatically detects how it's being called and adjusts accordingly.

### Available Commands

#### Core Build & Test Commands
- `build` - Build all WRT components
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
- `wrtd` - Build WRTD (WebAssembly Runtime Daemon) binaries

#### Advanced Features
- `help-diagnostics` - Comprehensive diagnostic system guide
- Diagnostic system with JSON output, caching, and filtering
- LSP-compatible structured output for IDE integration

### Examples

#### Basic Usage
```bash
# Setup development environment
cargo-wrt setup --check
cargo-wrt setup --all

# Build and test
cargo-wrt build
cargo-wrt test
cargo-wrt check

# Safety verification
cargo-wrt verify --asil d
cargo-wrt verify-matrix --report
```

#### Advanced Diagnostic System
```bash
# JSON output for tooling/AI agents
cargo-wrt build --output json

# Filter errors only with caching
cargo-wrt build --output json --filter-severity error --cache

# Show only new diagnostics since last run
cargo-wrt build --cache --diff-only

# Group diagnostics by file
cargo-wrt build --output json --group-by file

# Filter by specific tool (clippy, rustc, etc.)
cargo-wrt check --output json --filter-source clippy
```

#### WebAssembly Analysis
```bash
# Analyze WebAssembly modules
cargo-wrt wasm verify module.wasm
cargo-wrt wasm imports module.wasm
cargo-wrt wasm exports module.wasm
cargo-wrt wasm analyze *.wasm

# Create test modules
cargo-wrt wasm create-test test_module.wasm
```

#### Requirements Management (SCORE)
```bash
# Initialize and manage requirements
cargo-wrt requirements init
cargo-wrt requirements verify
cargo-wrt requirements score
cargo-wrt requirements matrix

# Check missing requirements
cargo-wrt requirements missing
cargo-wrt requirements demo
```

#### Comprehensive Verification
```bash
# Full ASIL compliance verification
cargo-wrt verify-matrix --asil d --report

# Formal verification with KANI
cargo-wrt kani-verify --asil-profile d

# Feature combination testing
cargo-wrt test-features --comprehensive

# WebAssembly test suite
cargo-wrt testsuite --validate
```

#### CI/CD Integration
```bash
# Simulate CI locally
cargo-wrt simulate-ci --profile asil-d

# Generate structured reports
cargo-wrt ci --output json
cargo-wrt verify --output json --filter-severity error
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

cargo-wrt automatically detects and manages external tool dependencies:

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
cargo-wrt setup --check

# Install missing optional tools
cargo-wrt setup --install

# Complete development setup
cargo-wrt setup --all
```

### Tool Version Management

cargo-wrt now includes sophisticated tool version management with configurable requirements:

```bash
# Check all tool versions against requirements
cargo-wrt tool-versions check

# Show detailed version information
cargo-wrt tool-versions check --verbose

# Check specific tool only
cargo-wrt tool-versions check --tool kani

# Generate tool-versions.toml configuration file
cargo-wrt tool-versions generate

# Generate comprehensive configuration (all tools)
cargo-wrt tool-versions generate --all

# Update existing configuration (future feature)
cargo-wrt tool-versions update --all
```

The tool version system uses a `tool-versions.toml` file in the workspace root to specify:
- Exact version requirements (e.g., kani must be exactly 0.63.0)
- Minimum version requirements (e.g., git must be at least 2.30.0)
- Installation commands for each tool
- Which cargo-wrt commands require each tool

This ensures reproducible builds and consistent development environments across all contributors.

When you run a command that needs a missing tool, cargo-wrt will:
1. Detect the missing tool
2. Show a helpful error message
3. Provide exact installation commands
4. Suggest running `cargo-wrt setup --install`

## License

MIT - see LICENSE file for details.