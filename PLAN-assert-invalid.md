# Implementation Plan: assert_invalid Module Validation

**Impact**: ~5,660 assertions
**Goal**: Properly reject semantically invalid modules

## Summary of Findings

### Current Validation Infrastructure

The existing `WastModuleValidator` in `kiln-build-core/src/wast_validator.rs` (~3700 lines) already implements substantial validation:
- Type checking on operand stack
- Control flow tracking (block, loop, if, else, try)
- Function and memory reference validation
- Unreachable code polymorphism
- Constant expression validation for globals
- Memory alignment validation
- Element and data segment validation
- Start function signature validation

### Categories of assert_invalid Tests

1. **Type mismatches** (most common): Wrong types on operand stack, function signatures
2. **Unknown references**: unknown function, global, table, memory, label, type, tag, local
3. **Constant expression required**: Non-constant instructions in global/element initializers
4. **Immutable global**: Attempting to set immutable globals
5. **Uninitialized local**: Using non-nullable reference locals before initialization
6. **Alignment violations**: Memory operations with excessive alignment
7. **Undeclared function reference**: ref.func to function not in C.refs
8. **Start function violations**: Wrong signature for start function

### Identified Gaps

1. **Uninitialized local validation** - Not implemented (GC proposal feature)
2. **Memory64 address type validation** - Partial
3. **Table type compatibility** - Partial
4. **Duplicate export names** - May be missing
5. **Import/export compatibility** - Partial
6. **SIMD type validation** - Simplified (always assumes V128)
7. **GC type hierarchies** - Partial implementation

---

## Implementation Phases

### Phase 1: Quick Wins (High Impact, Low Effort)

#### 1.1 Duplicate Export Name Validation
- **File**: `kiln-build-core/src/wast_validator.rs`
- **Error message**: "duplicate export name"
- **Implementation**: Add HashSet tracking during export validation
- **Complexity**: Low

#### 1.2 Table Type Compatibility
- **File**: `kiln-build-core/src/wast_validator.rs`
- **Error message**: "type mismatch"
- **Implementation**: Validate table.get/table.set/table.fill match table element types
- **Complexity**: Low

#### 1.3 Element Segment Table Index Bounds
- **File**: `kiln-build-core/src/wast_validator.rs`
- **Implementation**: Check table index in element segments is valid
- **Complexity**: Low

### Phase 2: Medium Effort Improvements

#### 2.1 Uninitialized Local Tracking (GC Proposal)
- **File**: `kiln-build-core/src/wast_validator.rs`
- **Error message**: "uninitialized local"
- **Implementation**: Track which reference-typed locals have been assigned before use
- **Complexity**: Medium - requires local dataflow analysis through control flow

#### 2.2 Enhanced Constant Expression Validation
- **File**: `kiln-build-core/src/wast_validator.rs`, `validate_const_expr_typed()`
- **Missing**: Some extended constant expression operations from proposals
- **Complexity**: Low

#### 2.3 Memory64 Address Type Validation
- **File**: `kiln-build-core/src/wast_validator.rs`
- **Implementation**: Validate address types match memory index size
- **Complexity**: Medium

### Phase 3: Completeness (Lower Priority)

#### 3.1 Complete SIMD Instruction Validation
- **File**: `kiln-build-core/src/wast_validator.rs`, opcode 0xFD handling
- **Current**: Simplified - assumes V128 binary ops
- **Missing**: Individual SIMD instruction type signatures
- **Complexity**: High - many opcodes

#### 3.2 Atomic Operations Validation (Threads Proposal)
- **File**: `kiln-build-core/src/wast_validator.rs`
- **Missing**: atomic.wait/notify, atomic RMW operations
- **Complexity**: Medium

---

## Implementation Priority Order

### High Priority (Immediate Impact)
1. Duplicate export name validation
2. Table type compatibility for table.get/set/fill
3. Element segment table index bounds

### Medium Priority (Improves Coverage)
4. Uninitialized local tracking (GC feature)
5. Enhanced element type validation
6. Memory64 address type validation

### Lower Priority (Completeness)
7. Full SIMD instruction validation
8. Atomic operations validation
9. Extended constant expression features

---

## Critical Files

| File | Purpose |
|------|---------|
| `kiln-build-core/src/wast_validator.rs` | Core validator - main implementation target |
| `kiln-build-core/src/wast_execution.rs` | Integration point where validator is called |
| `kiln-decoder/src/decoder.rs` | Could add early validation during parsing |
| `kiln-format/src/module.rs` | Module structure reference |

---

## Testing Strategy

1. Run `cargo-kiln testsuite --run-wast` to establish baseline
2. For each validation added:
   - Add unit test in `wast_validator.rs`
   - Verify against official testsuite files
   - Check both positive (should fail) and negative (should pass) cases

---

## Architecture Consideration

Consider moving validation into the decoder for single-pass validation:

```
Current:  decode_module() -> WastModuleValidator::validate() -> instantiate
Proposed: decode_module_and_validate() -> instantiate
```

Benefits:
- Single pass validation possible
- Validation errors associated with exact byte positions
- More consistent with wasmparser behavior
