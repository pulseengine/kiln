# Safety-Critical Test Suite for WRT Component

This directory contains comprehensive test files for validating the safety-critical features of the WRT allocator integration in wrt-component.

## Test Files

### 1. `safety_critical_capacity_tests.rs`
Tests capacity limits for all migrated collections (Vec/HashMap):
- Vector capacity enforcement (component, export, resource, etc.)
- Map capacity limits
- String length limits
- Stack overflow protection
- Memory invariants

### 2. `safety_critical_concurrency_tests.rs`
Tests concurrent access patterns for thread safety:
- Resource table concurrent access
- Bounded vector concurrent push/pop
- Map concurrent operations
- Resource lifecycle under concurrent access
- Deadlock prevention
- Memory pressure under concurrency

### 3. `safety_critical_memory_budget_tests.rs`
Tests memory budget enforcement:
- Component-level memory budgets
- Cross-collection memory sharing
- Memory reclamation
- Mixed allocation sizes
- Budget enforcement consistency

### 4. `safety_critical_error_handling_tests.rs`
Ensures no panics occur (only Result<T,E> errors):
- Capacity exceeded error handling
- String operation errors
- Map error handling
- Resource table errors
- Empty collection operations
- Boundary value errors
- Error propagation

### 5. `safety_critical_integration_tests.rs`
Comprehensive integration tests:
- Component lifecycle with bounded resources
- Canonical ABI with resource limits
- Cross-component communication
- Resource sharing between components
- Memory allocation across components
- Component linking
- Error propagation through layers

### 6. `safety_critical_feature_flag_tests.rs`
Tests feature flag combinations:
- std vs no_std compatibility
- safety-critical feature flag behavior
- Generic type support
- API consistency
- Compile-time limits

## Safety Requirements Validated

- **SW-REQ-ID: REQ_MEM_001** - Memory bounds checking
- **SW-REQ-ID: REQ_MEM_002** - Memory budget enforcement
- **SW-REQ-ID: REQ_MEM_003** - Static memory allocation
- **SW-REQ-ID: REQ_COMP_001** - Component capacity limits
- **SW-REQ-ID: REQ_COMP_002** - Component isolation
- **SW-REQ-ID: REQ_COMP_003** - Component integration
- **SW-REQ-ID: REQ_THREAD_001** - Thread-safe resource access
- **SW-REQ-ID: REQ_SYNC_001** - Synchronization primitives
- **SW-REQ-ID: REQ_ERR_001** - No panic paths allowed
- **SW-REQ-ID: REQ_ERR_002** - Explicit error propagation
- **SW-REQ-ID: REQ_INT_001** - System integration validation
- **SW-REQ-ID: REQ_FEAT_001** - Feature flag validation
- **SW-REQ-ID: REQ_BUILD_001** - Build configuration testing

## ASIL Level

All tests target **ASIL-C** compliance with:
- No dynamic memory allocation
- Bounded execution time
- No panic paths
- Deterministic behavior
- Complete error handling

## Running the Tests

```bash
# Run all safety-critical tests
cargo test -p wrt-component safety_critical

# Run with safety-critical feature flag
cargo test -p wrt-component --features safety-critical

# Run specific test suite
cargo test -p wrt-component safety_critical_capacity_tests

# Run in no_std environment
cargo test -p wrt-component --no-default-features
```

## Key Testing Patterns

1. **Capacity Testing**: Fill collections to capacity and verify error handling
2. **Concurrent Testing**: Use barriers and multiple threads to test race conditions
3. **Budget Testing**: Allocate until memory exhausted, verify proper errors
4. **Error Testing**: Trigger all error paths, verify no panics
5. **Integration Testing**: Combine multiple components and verify interactions

## Notes

- Some tests use simplified mock structures to avoid complex trait requirements
- Map tests use `BoundedTypeMap<V>` with u32 keys instead of `BoundedExportMap<V>` due to trait constraints
- All collections use the same `ComponentProvider` with 128KB budget
- Tests verify both functional correctness and safety properties