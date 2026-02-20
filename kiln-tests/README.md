# WRT Tests - Unified Test Workspace

This workspace provides a unified testing infrastructure for the WebAssembly Runtime (WRT) project.

## Structure

- `integration/` - Consolidated cross-crate integration tests
  - `component_model/` - Component model functionality tests
  - `runtime/` - Runtime system tests  
  - `platform/` - Platform-specific tests
  - `no_std/` - No-std compatibility tests
  - `security/` - Security and CFI tests
- `fixtures/` - Test assets (WAT, WASM files)
  - `components/` - Component model test files
  - `wasm/` - WebAssembly binary test files
  - `wat/` - WebAssembly text format test files
  - `configs/` - Test configuration files

## Running Tests

### Using cargo-wrt (Recommended)
```bash
# Run all tests via cargo-wrt
cargo-wrt test

# Verify no_std compatibility across all crates
cargo-wrt no-std

# Quick partial verification with verbose output
cargo-wrt no-std --detailed
```

### Direct Cargo Commands
```bash
# All integration tests
cargo test -p wrt-tests-integration

# Specific test categories
cargo test -p wrt-tests-integration component_model
cargo test -p wrt-tests-integration runtime
cargo test -p wrt-tests-integration platform
cargo test -p wrt-tests-integration no_std
cargo test -p wrt-tests-integration security
```

## Test Registry

The unified test registry (`wrt-test-registry`) provides:
- Consistent test runners across std/no_std environments
- Test discovery and coordination
- Standardized test reporting
- Feature-based test filtering

## Test Consolidation Status

- ✅ Unified test consolidation completed
- ✅ Migrated 55 test files from across the workspace
- ✅ Eliminated ~9,600 lines of duplicate test code
- ✅ Consolidated 7 test directories into unified structure
- ✅ Integrated all tests with xtask automation
- ✅ Created comprehensive test categorization:
  - No-std compatibility tests (15 files consolidated)
  - Parser tests (9 files consolidated)
  - Memory safety tests (18 files consolidated)
  - Component model tests
  - Runtime and execution tests
  - Platform-specific tests
  - Security and CFI tests

## Contributing

When adding new tests:
1. Place integration tests in the appropriate `integration/` subdirectory
2. Use the unified test registry for consistency
3. Follow the naming convention: `*_tests.rs`
4. Add test fixtures to the `fixtures/` directory
5. Update this README when adding new test categories