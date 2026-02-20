# Fuzzing for wrt-component

This directory contains fuzzing targets for the wrt-component crate to ensure robustness against malformed input.

## Prerequisites

Install cargo-fuzz:
```bash
cargo install cargo-fuzz
```

Note: Fuzzing requires a nightly Rust toolchain and is only supported on Unix-like systems (Linux, macOS).

## Running Fuzzing Targets

To run a specific fuzzing target:

```bash
# From the wrt-component directory
cd fuzz

# Run the WIT parser fuzzer
cargo +nightly fuzz run fuzz_wit_parser

# Run the component parser fuzzer
cargo +nightly fuzz run fuzz_component_parser

# Run the canonical options fuzzer
cargo +nightly fuzz run fuzz_canonical_options

# Run the type bounds fuzzer
cargo +nightly fuzz run fuzz_type_bounds
```

## Fuzzing Targets

### fuzz_wit_parser
Tests the WIT (WebAssembly Interface Types) parser with random input to ensure it handles malformed WIT syntax gracefully.

### fuzz_component_parser
Tests the component parser with random binary data to ensure it properly validates and rejects invalid component formats.

### fuzz_canonical_options
Tests the canonical options and realloc manager with random configurations and operations.

### fuzz_type_bounds
Tests the type bounds checker with random type hierarchies and relationships.

## Running with Options

```bash
# Run for a specific number of iterations
cargo +nightly fuzz run fuzz_wit_parser -- -runs=10000

# Run with a timeout (in seconds)
cargo +nightly fuzz run fuzz_component_parser -- -max_total_time=300

# Run with more workers (parallel fuzzing)
cargo +nightly fuzz run fuzz_type_bounds -- -workers=4

# Run with a specific seed
cargo +nightly fuzz run fuzz_canonical_options -- -seed=12345
```

## Analyzing Crashes

If fuzzing finds a crash, it will be saved in `fuzz/artifacts/<target_name>`. To reproduce:

```bash
# Reproduce a crash
cargo +nightly fuzz run fuzz_wit_parser fuzz/artifacts/fuzz_wit_parser/crash-<hash>

# Get more details with address sanitizer
RUSTFLAGS="-Zsanitizer=address" cargo +nightly fuzz run fuzz_wit_parser fuzz/artifacts/fuzz_wit_parser/crash-<hash>
```

## Coverage Analysis

To analyze code coverage from fuzzing:

```bash
# Build with coverage instrumentation
cargo +nightly fuzz coverage fuzz_wit_parser

# Generate coverage report
cargo +nightly fuzz coverage fuzz_wit_parser -- -report=html
```

## Adding New Fuzzing Targets

1. Add a new binary entry to `fuzz/Cargo.toml`
2. Create the fuzzing target in `fuzz/fuzz_targets/`
3. Use `libfuzzer_sys::fuzz_target!` macro
4. Ensure all operations handle arbitrary input gracefully

## Best Practices

1. **Avoid assertions in fuzzed code** - Use error handling instead
2. **Handle all edge cases** - Empty input, maximum sizes, invalid UTF-8
3. **Test error paths** - Ensure error handling doesn't panic
4. **Keep targets focused** - Each target should test a specific component
5. **Use fuzzer input creatively** - Use bytes to control test flow

## Continuous Fuzzing

For production use, consider setting up continuous fuzzing with:
- OSS-Fuzz integration
- Regular fuzzing runs in CI
- Corpus minimization
- Coverage tracking over time