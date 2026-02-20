# wrt-math

> Mathematical operations and numeric types for WebAssembly Runtime

## Overview

Provides WebAssembly-compliant mathematical operations and numeric type handling. Implements the complete set of WebAssembly numeric instructions with bit-precise semantics, supporting std, no_std+alloc, and pure no_std environments.

## Features

- **🔢 Complete Numeric Ops**: All WebAssembly numeric instructions (i32, i64, f32, f64)
- **🎯 Spec Compliance**: Bit-precise WebAssembly semantics
- **🔄 Cross-Platform**: Consistent behavior across all targets
- **⚡ Optimized**: LLVM-friendly implementations with optional intrinsics
- **🛡️ Safe**: Zero unsafe code, comprehensive error handling

## Quick Start

```toml
[dependencies]
wrt-math = "0.1"
```

### Basic Usage

```rust
use wrt_math::prelude::*;

// Integer operations
let result = i32_add(10, 32)?;          // 42
let wrapped = i32_add(i32::MAX, 1)?;    // Wrapping arithmetic
let clamped = i32_add_sat_s(100, 50)?;  // Saturating arithmetic

// Floating-point operations  
let sum = f32_add(3.14, 2.86)?;         // 6.0
let precise = f64_mul(0.1, 0.2)?;       // WebAssembly-precise result

// Bit manipulation
let leading = i32_clz(0x0000_FF00)?;    // Count leading zeros
let trailing = i32_ctz(0x0000_FF00)?;   // Count trailing zeros
let popcount = i32_popcnt(0xFF)?;       // Population count

// Conversions
let truncated = i32_trunc_f32_s(42.7)?; // 42
let converted = f64_convert_i32_u(100)?; // 100.0
```

### Type Utilities

```rust
use wrt_math::float_bits::*;

// Float bit manipulation
let bits = f32_to_bits(3.14);
let float = f32_from_bits(bits);

// NaN and infinity handling
let is_nan = f32_is_nan(f32::NAN);
let is_inf = f64_is_infinite(f64::INFINITY);
```

## WebAssembly Instruction Mapping

| WebAssembly | wrt-math Function | Description |
|-------------|-------------------|-------------|
| `i32.add` | `i32_add()` | 32-bit integer addition |
| `i64.mul` | `i64_mul()` | 64-bit integer multiplication |
| `f32.div` | `f32_div()` | 32-bit float division |
| `f64.sqrt` | `f64_sqrt()` | 64-bit float square root |
| `i32.clz` | `i32_clz()` | Count leading zeros |
| `f32.abs` | `f32_abs()` | Absolute value |
| `i64.extend_i32_s` | `i64_extend_i32_s()` | Sign extension |

## Environment Support

### Standard Library
```toml
wrt-math = { version = "0.1", features = ["std"] }
```
Full functionality with std math functions.

### no_std + alloc  
```toml
wrt-math = { version = "0.1", features = ["alloc"] }
```
Core operations with heap allocation support.

### Pure no_std
```toml
wrt-math = { version = "0.1", default-features = false }
```
Essential operations only, no heap allocation.

## Advanced Features

### Saturating Arithmetic
```rust
use wrt_math::ops::*;

// Saturating operations (clamp to min/max instead of wrapping)
let sat_add = i32_add_sat_s(i32::MAX, 100)?; // i32::MAX
let sat_sub = i32_sub_sat_u(10, 20)?;         // 0
```

### Bit-Precise Float Operations
```rust
use wrt_math::float_bits::*;

// WebAssembly-compliant float operations
let canonical_nan = f32_canonical_nan();
let quiet_nan = f64_arithmetic_nan();

// Bit pattern analysis
let (sign, exp, mantissa) = f64_decompose(3.14159);
let recomposed = f64_compose(sign, exp, mantissa);
```

### Platform Optimizations
```rust
// CPU-specific optimizations (when available)
#[cfg(target_feature = "popcnt")]
let fast_popcount = i32_popcnt_native(value);

#[cfg(target_feature = "lzcnt")]
let fast_clz = i32_clz_native(value);
```

## Performance

The implementation prioritizes correctness while allowing LLVM to optimize:

- **Basic arithmetic**: Compiles to single CPU instructions
- **Bit operations**: Maps to hardware instructions when available
- **Float operations**: Uses hardware FPU with WebAssembly semantics
- **Type conversions**: Optimized conversion paths

### Benchmarks
```bash
cargo bench --features=std
```

## WebAssembly Compliance

All operations follow WebAssembly specification semantics:

- **Deterministic**: Same results across all platforms
- **Wrapping arithmetic**: Integer overflow wraps consistently  
- **IEEE 754**: Precise floating-point behavior
- **NaN propagation**: Correct NaN handling in all operations
- **Trap conditions**: Proper error handling for division by zero, etc.

## Integration Example

```rust
use wrt_math::prelude::*;

// WebAssembly runtime integration
fn execute_numeric_instruction(opcode: u8, lhs: Value, rhs: Value) -> Result<Value> {
    match opcode {
        0x6A => Ok(Value::I32(i32_add(lhs.as_i32()?, rhs.as_i32())?)),
        0x6B => Ok(Value::I32(i32_sub(lhs.as_i32()?, rhs.as_i32())?)),
        0x6C => Ok(Value::I32(i32_mul(lhs.as_i32()?, rhs.as_i32())?)),
        0x92 => Ok(Value::F32(f32_add(lhs.as_f32()?, rhs.as_f32())?)),
        // ... other operations
        _ => Err(Error::UnsupportedInstruction(opcode)),
    }
}
```

## See Also

- [API Documentation](https://docs.rs/wrt-math)
- [WebAssembly Numeric Instructions](https://webassembly.github.io/spec/core/syntax/instructions.html#numeric-instructions)
- [CPU Acceleration Guide](../docs/source/architecture/cpu_acceleration.rst)