# WRT Allocator Performance Benchmarks

This directory contains performance benchmarks comparing the WRT safety-critical allocator with standard library collections.

## Benchmarks

### 1. `wrt_allocator_benchmarks.rs`
Comprehensive performance comparison including:
- Vector push operations (small/medium/large sizes)
- Vector iteration performance
- HashMap insertion and lookup
- Mixed component workload simulation
- Capacity error handling overhead

### 2. `zero_cost_validation.rs`
Validates the zero-cost abstraction claim:
- Single operation comparison
- Direct memory access patterns
- Iterator performance parity
- Capacity check overhead in happy path
- Memory layout validation

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p wrt-component --features "std,safety-critical"

# Run specific benchmark
cargo bench -p wrt-component --features "std,safety-critical" -- wrt_allocator

# Generate detailed HTML reports
cargo bench -p wrt-component --features "std,safety-critical" -- --verbose

# Run with baseline comparison
cargo bench -p wrt-component --features "std,safety-critical" -- --save-baseline wrt-allocator
```

## Expected Results

### Performance Parity
The WRT allocator should show near-identical performance to std collections for:
- Push/insert operations: ±5% variance
- Iteration: ±2% variance (should compile to identical code)
- Direct indexing: 0% overhead (identical assembly)
- Lookup operations: ±5% variance

### Zero-Cost Validation
- Memory layout: Identical size and alignment
- Happy path: No measurable overhead for capacity checks
- Iterator chains: Identical assembly generation
- Inline operations: Full optimization by compiler

### Overhead Sources
Small overhead (≤5%) is expected only from:
- Capacity checks on push/insert operations
- Error result wrapping (Result<T, E> vs direct return)
- PhantomData field (optimized out in most cases)

## Interpreting Results

### Good Performance
```
std_vec_push/1000       time:   [20.5 µs 20.7 µs 20.9 µs]
wrt_vec_push/1000       time:   [20.8 µs 21.0 µs 21.2 µs]
                        ^^^^ Within 2% - excellent
```

### Concerning Performance
```
std_vec_push/1000       time:   [20.5 µs 20.7 µs 20.9 µs]
wrt_vec_push/1000       time:   [25.1 µs 25.4 µs 25.7 µs]
                        ^^^^ >20% overhead - investigate
```

## Performance Tips

1. **Use with_capacity()** when size is known
2. **Check capacity before loops** to avoid per-iteration checks
3. **Use try_extend()** for bulk operations
4. **Profile with target architecture** (results vary by CPU)

## Safety vs Performance Trade-offs

The WRT allocator prioritizes safety with:
- Compile-time capacity limits (no runtime allocation)
- Explicit error handling (no panics)
- Deterministic behavior (predictable performance)

These safety features have minimal performance impact (<5%) while providing:
- Memory safety guarantees
- ASIL-C compliance
- Real-time predictability
- Resource isolation