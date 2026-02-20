# WRT Foundation Fuzz Testing

This directory contains both traditional fuzz testing and property-based tests for WRT Foundation components.

## Structure

```
fuzz/
├── Cargo.toml              # Standalone fuzz package configuration
├── README.md               # This file
├── mod.rs                  # Library module with test utilities
├── fuzz_targets/           # Actual fuzz targets for cargo-fuzz
│   ├── fuzz_bounded_vec.rs    # Fuzz BoundedVec operations
│   ├── fuzz_bounded_stack.rs  # Fuzz BoundedStack operations
│   ├── fuzz_memory_adapter.rs # Fuzz memory adapter operations
│   └── fuzz_safe_slice.rs     # Fuzz SafeSlice operations
├── bounded_collections_fuzz.rs # Property-based tests for collections
├── memory_adapter_fuzz.rs      # Property-based tests for memory adapters
└── safe_memory_fuzz.rs         # Property-based tests for safe memory
```

## Types of Testing

### Fuzz Testing (cargo-fuzz)

The `fuzz_targets/` directory contains traditional fuzz targets that can be run with cargo-fuzz:

```bash
# Run a specific fuzz target
cargo fuzz run fuzz_bounded_vec

# Run with specific options
cargo fuzz run fuzz_bounded_vec -- -max_total_time=300
```

These tests use arbitrary input generation to exercise edge cases and find potential bugs.

### Property-Based Testing (CI-friendly)

The `*_fuzz.rs` modules contain deterministic property-based tests that:
- Run in CI without special setup
- Test the same operation patterns as fuzz tests
- Use fixed test cases to verify invariants
- Can be run with standard `cargo test`

```bash
# Run property-based tests
cargo test --package wrt-foundation-fuzz
```

## Migration from Root `/fuzz` Directory

This fuzz testing setup replaces the previous `/fuzz` directory. Key changes:

1. **Location**: Moved from `/fuzz/` to `/wrt-tests/fuzz/`
2. **Structure**: Added property-based tests alongside fuzz targets
3. **Dependencies**: Updated import paths for new location
4. **CI Integration**: Property tests can run in CI without cargo-fuzz

## Testing Strategy

### Bounded Collections
- Tests push/pop operations, capacity management, validation
- Simulates memory corruption to test verification levels
- Covers BoundedVec and BoundedStack implementations

### Memory Adapters  
- Tests store/load operations, memory growth, integrity checks
- Simulates corruption scenarios with full verification
- Covers SafeMemoryAdapter with different verification levels

### Safe Memory
- Tests slice operations, copy operations, integrity validation
- Covers SafeSlice with various memory providers
- Tests boundary conditions and large data operations

## Verification Levels

All tests exercise four verification levels:
- `None`: No verification overhead
- `Sampling`: Periodic verification checks
- `Standard`: Regular verification with moderate overhead
- `Full`: Comprehensive verification with maximum safety

## Running Tests

### Quick Test (Property-based only)
```bash
cargo test --package wrt-foundation-fuzz
```

### Full Fuzz Testing
```bash
# Install cargo-fuzz first
cargo install cargo-fuzz

# Run individual targets
cargo fuzz run fuzz_bounded_vec
cargo fuzz run fuzz_bounded_stack
cargo fuzz run fuzz_memory_adapter  
cargo fuzz run fuzz_safe_slice

# Run all targets for 5 minutes each
for target in fuzz_bounded_vec fuzz_bounded_stack fuzz_memory_adapter fuzz_safe_slice; do
    cargo fuzz run $target -- -max_total_time=300
done
```

### Debugging Failed Inputs
```bash
# If a fuzz target finds an issue, debug with:
cargo fuzz fmt fuzz_bounded_vec <crash_file>
```

## Adding New Fuzz Targets

1. Create the fuzz target in `fuzz_targets/fuzz_new_component.rs`
2. Add corresponding property tests in `new_component_fuzz.rs`
3. Update `Cargo.toml` to include the new binary target
4. Update this README.md

## Dependencies

The fuzz package has its own Cargo.toml with:
- `libfuzzer-sys`: For traditional fuzzing
- `arbitrary`: For generating test inputs
- Local WRT crates with `std` features enabled

This ensures fuzz tests can use standard library features while the main crates remain `no_std` compatible.