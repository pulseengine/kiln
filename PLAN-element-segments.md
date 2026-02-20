# Implementation Plan: Fix Element Segment Initialization

**Impact**: ~20,000+ assertions (cascading effect - when 1 module fails, all subsequent tests fail)
**Goal**: Properly initialize tables from inline element segments

## Problem Summary

WAST tests fail with "Failed to initialize element segments" when modules have tables with inline element segments like `(table funcref (elem $func))`. This causes cascading failures where all subsequent assert_return tests fail with "No module loaded".

## Root Cause Analysis

The error occurs in `wrt-runtime/src/module_instance.rs` at `initialize_element_segments()` when calling `table.set_shared(table_offset, value)`. The `set_shared()` function in `table.rs` checks bounds:

```rust
if idx_usize >= elements.len() {
    return Err(Error::invalid_function_index("Table access out of bounds"));
}
```

The table has fewer elements than expected because either:
1. Table `limits.min` is being parsed/converted as 0 instead of actual element count
2. The table isn't being populated before element initialization
3. Mismatch in how wast crate encodes inline element tables vs how we decode them

## Binary Encoding

The `wast` crate encodes `(table funcref (elem $f1 $f2 ...))` as:
- **Table section**: limits.min = number_of_inline_elements
- **Element section**: active segment at offset 0

We need to verify our decoder reads `limits.min` correctly.

## Data Flow Chain

```
1. wrt-decoder/streaming_decoder.rs::process_table_section()   → reads limits.min
2. wrt-decoder/streaming_decoder.rs::process_element_section() → reads element items
3. wrt-runtime/module.rs::from_wrt_module()                    → converts to runtime
4. wrt-runtime/module_instance.rs::populate_tables_from_module() → copies tables
5. wrt-runtime/module_instance.rs::initialize_element_segments()  → initializes elements
```

---

## Implementation Steps

### Step 1: Add Diagnostic Tracing

**In `wrt-runtime/src/module_instance.rs` - `populate_tables_from_module()`:**
```rust
#[cfg(feature = "tracing")]
for (idx, table_wrapper) in self.module.tables.iter().enumerate() {
    let table = table_wrapper.inner();
    debug!("Table {} has {} elements (min limit)", idx, table.size());
}
```

**In `initialize_element_segments()`:**
```rust
#[cfg(feature = "tracing")]
debug!("Tables.len() = {}, Element segments = {}", tables.len(), self.module.elements.len());
for (idx, elem) in self.module.elements.iter().enumerate() {
    debug!("Element segment {}: mode={:?}, items={}", idx, elem.mode, elem.items.len());
}
```

**In `wrt-decoder/src/streaming_decoder.rs` - `process_table_section()`:**
```rust
#[cfg(feature = "tracing")]
trace!(table_index = i, min = min, max = ?max, "Parsed table limits");
```

**In `process_element_section()`:**
```rust
#[cfg(feature = "tracing")]
trace!(elem_idx = elem_idx, item_count = item_count, mode = ?mode, "Parsed element segment");
```

### Step 2: Verify Table Creation

**In `wrt-runtime/src/module.rs` table conversion (~line 1596-1619):**
```rust
#[cfg(feature = "tracing")]
trace!(
    table_idx = idx,
    limits_min = table_type.limits.min,
    limits_max = ?table_type.limits.max,
    "Creating runtime table"
);
```

### Step 3: Add Unit Test

**In `wrt-runtime/src/module_instance.rs`:**
```rust
#[test]
fn test_inline_element_segment_initialization() {
    // Create module with 1 table and 1 element segment
    // Verify table size matches element count
    // Verify element initialization succeeds
}
```

### Step 4: Run Diagnostics

```bash
RUST_LOG=trace cargo-wrt testsuite --run-wast --wast-filter "block" 2>&1 | grep -E "(Table|Element|limits)"
```

### Step 5: Fix Root Cause

Based on diagnostic output, fix in one of:

| Location | Issue |
|----------|-------|
| `wrt-decoder` | `limits.min` not read correctly |
| `wrt-runtime/module.rs` | Table type not copied correctly |
| `wrt-runtime/module_instance.rs` | Tables not added to instance |
| Element segment conversion | Offset calculation wrong |

---

## Likely Fix Locations

**Option A**: In `wrt-runtime/src/module.rs` around line 1603
- Check that `table_type.limits.min` matches expected value

**Option B**: In `wrt-runtime/src/module_instance.rs` around line 768
- Verify `self.module.tables` is non-empty with correct sizes

**Option C**: In element segment conversion (lines 1833-1866)
- Verify offset is evaluated correctly for inline elements (should be 0)

---

## Files to Modify

| File | Purpose |
|------|---------|
| `wrt-runtime/src/module_instance.rs` | Add tracing, potential fix location |
| `wrt-runtime/src/module.rs` | Add tracing for table/element conversion |
| `wrt-runtime/src/table.rs` | Add tracing for table creation |
| `wrt-decoder/src/streaming_decoder.rs` | Add tracing for parsing |
| `wrt-build-core/src/wast_execution.rs` | Add debug output |

---

## Testing Strategy

1. Run block.wast with `RUST_LOG=trace` to capture tracing output
2. Analyze trace to identify where table size becomes wrong
3. Apply fix to identified location
4. Verify block.wast passes
5. Run full WAST test suite for regressions

---

## Success Criteria

| Test File | Assertions | Status Goal |
|-----------|------------|-------------|
| block.wast | 223 | All pass |
| call_indirect.wast | 251 | All pass |
| i32.wast | 460 | No regression |
| br.wast, call.wast, etc. | Many | All pass |

---

## Debugging Commands

```bash
# Run single test with trace
RUST_LOG=trace cargo-wrt testsuite --run-wast --wast-filter "block" 2>&1 | head -200

# Check binary encoding
cargo test -p wrt-build-core test_simple_wast_execution -- --nocapture

# Verify table limits in decoder
RUST_LOG=wrt_decoder=trace cargo test -p wrt-decoder table -- --nocapture
```
