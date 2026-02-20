# Test Fixtures

This directory contains test assets used across the WRT test suite.

## Structure

- `wasm/` - WebAssembly binary files (.wasm)
- `wat/` - WebAssembly text files (.wat)
- `components/` - Component model test files
- `configs/` - Test configuration files

## Usage

Test fixtures are organized by category and can be referenced from any test in the suite using relative paths from the test file location.

Example:
```rust
let wasm_bytes = include_bytes!("../../fixtures/wasm/simple_module.wasm");
```

## Adding New Fixtures

When adding new test fixtures:
1. Place them in the appropriate subdirectory
2. Use descriptive names that indicate the test scenario
3. Include both WAT source and compiled WASM when applicable
4. Document any special requirements or expected behavior