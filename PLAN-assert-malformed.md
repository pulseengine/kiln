# Implementation Plan: Binary Validation for assert_malformed Tests

**Impact**: ~2,895 assertions (302 distinct failure patterns identified)
**Goal**: Properly reject malformed WebAssembly binaries

## Summary

The Kiln decoder currently accepts malformed WebAssembly binaries that should be rejected. This plan addresses 8 categories of validation failures.

---

## Category 1: UTF-8 Validation (176 failures)

**Problem**: Custom section names, import/export names not validated for UTF-8 conformance.

**Root Cause**: `process_custom_section()` in `streaming_decoder.rs` (lines 2350-2354) does nothing - returns `Ok(())` without parsing the section name.

**Implementation**:
1. Modify `process_custom_section()` to parse the section name using `validate_utf8_name()`
2. Custom section format: name_len (LEB128), name (bytes), data (remaining)
3. Validate the name is valid UTF-8 before proceeding

**File**: `/Users/r/git/wrt2/kiln-decoder/src/streaming_decoder.rs` lines 2350-2354

---

## Category 2: Section Ordering and Duplicates (46 failures)

**Problem**: Decoder accepts duplicate non-custom sections (two start sections, two function sections).

**Root Cause**: `process_next_section()` does not track which sections have been seen.

**Implementation**:
1. Add field to `StreamingDecoder`: `seen_sections: u16` (bitmask for sections 1-13)
2. In `process_next_section()`:
   - For section_id 1-13: Check if bit already set
   - If duplicate, return error "unexpected content after last section"
   - Set bit after processing
3. Allow multiple custom sections (ID 0), reject duplicates of IDs 1-13

**File**: `/Users/r/git/wrt2/kiln-decoder/src/streaming_decoder.rs` lines 220-244

---

## Category 3: Section Size Mismatch (14 failures)

**Problem**: Decoder does not verify section content exactly fills declared section size.

**Implementation**:
1. Modify each `process_*_section()` method to track bytes consumed
2. In `process_next_section()` after calling `process_section()`:
   - Verify all bytes consumed within section
   - If content remains, return error "section size mismatch"

**File**: `/Users/r/git/wrt2/kiln-decoder/src/streaming_decoder.rs` - all `process_*_section()` methods

---

## Category 4: LEB128 Overflow (13 failures)

**Problem**: Some LEB128 integers that overflow u32 are accepted.

**Implementation**:
1. In `/Users/r/git/wrt2/kiln-format/src/binary.rs` `read_leb128_u32()`:
   - On 5th byte: check `(byte & 0xF0) == 0` (only low 4 bits valid)
   - Check before `result |= ...` to prevent overflow
   - Reject 5+ byte encodings

**File**: `/Users/r/git/wrt2/kiln-format/src/binary.rs` lines 625-669

---

## Category 5: Malformed Section ID (10 failures)

**Problem**: Section IDs >= 14 or LEB128-encoded section IDs accepted as custom sections.

**Implementation**:
1. In `process_next_section()`:
   - Validate section_id <= 13
   - Section ID should be single byte, not LEB128
   - If section_id > 13 OR byte has continuation bit (0x80), return "malformed section id"
2. Change `_ =>` arm to explicitly reject unknown IDs:

```rust
match section_id {
    0 => self.process_custom_section(data),
    1 => self.process_type_section(data),
    // ... other valid sections
    _ => Err(Error::parse_error("malformed section id")),
}
```

**File**: `/Users/r/git/wrt2/kiln-decoder/src/streaming_decoder.rs` lines 226-264

---

## Category 6: Cross-Section Validation (20 failures)

**Problem**: Function count != code count, data count mismatches, missing data count section.

**Implementation**:
1. Add tracking fields to `StreamingDecoder`:
   - `function_count: Option<u32>`
   - `code_count: Option<u32>`
   - `data_count_value: Option<u32>`
   - `data_section_count: Option<u32>`
   - `uses_memory_init_or_data_drop: bool`

2. In `finish()` method:
   - If function_count and code_count both exist, must match
   - If data_count_value exists, must equal data_section_count
   - If uses_memory_init_or_data_drop, data_count_value must exist

**File**: `/Users/r/git/wrt2/kiln-decoder/src/streaming_decoder.rs` struct definition and `finish()`

---

## Category 7: Malformed Limits Flags (8 failures)

**Problem**: Invalid limits flags (0x08, 0x10, LEB128-encoded) accepted.

**Implementation**:
1. In `process_table_section()`:
   - Read flags as single byte
   - Basic tables: only 0x00 and 0x01 valid
   - Typed tables (GC): 0x40 and 0x41 valid
   - Reject others with "malformed limits flags"

2. In `process_memory_section()`:
   - MVP: only 0x00 and 0x01 valid
   - Threads: 0x00-0x03 valid
   - Memory64: 0x00-0x07 valid

**File**: `/Users/r/git/wrt2/kiln-decoder/src/streaming_decoder.rs` lines 1172-1188

---

## Category 8: Remaining Validations (15 failures)

| Issue | Count | Fix |
|-------|-------|-----|
| END opcode expected | 2 | Verify function bodies end with 0x0B |
| Unexpected end | 6 | Better bounds checking when reading |
| Malformed reference type | 2 | Validate reftype is 0x70/0x6F in element section |
| Illegal opcode | 2 | Validate opcodes in element init expressions |
| Malformed mutability | 1 | Validate mutability is 0x00 or 0x01 |
| Integer representation too long | 4 | Stricter LEB128 validation |

---

## Implementation Priority Order

| Priority | Category | Failures Fixed | Complexity |
|----------|----------|----------------|------------|
| 1 | UTF-8 validation | 176 | Low |
| 2 | Section ordering | 46 | Medium |
| 3 | Cross-section validation | 20 | Medium |
| 4 | Section size mismatch | 14 | Medium |
| 5 | LEB128 overflow | 13 | Careful testing |
| 6 | Section ID validation | 10 | Low |
| 7 | Limits flags | 8 | Low |
| 8 | Remaining | 15 | Various |

---

## Critical Files

| File | Changes Needed |
|------|----------------|
| `kiln-decoder/src/streaming_decoder.rs` | UTF-8, section ordering, section ID, limits flags, cross-section |
| `kiln-format/src/binary.rs` | LEB128 overflow checks |
| `kiln-decoder/src/optimized_string.rs` | UTF-8 validation utilities (reuse) |
| `kiln-decoder/src/decoder_no_alloc.rs` | Existing section order validation (reference) |
| `kiln-decoder/src/sections.rs` | Section parsing size validation |
