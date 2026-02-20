# Kiln Capability-Based Safety Feature System

## Overview

Kiln now uses a **capability-based safety feature system** that defines features based on **what they enforce** rather than which safety standard they target. This makes the system:

- **Standard-Agnostic**: Works with ISO 26262, DO-178C, IEC 61508, EN 50128, etc.
- **Self-Documenting**: Feature names describe exactly what they do
- **Composable**: Can mix and match capabilities as needed
- **Future-Proof**: New standards can map to existing capabilities

## Core Capability Features

### Allocation Strategies (Mutually Exclusive)

```toml
# QM Level - Dynamic allocation allowed
dynamic-allocation = []

# ASIL-C Level - Static allocation only, no runtime allocation
static-allocation = ["no-runtime-allocation"]

# ASIL-D Level - Verified static allocation with formal proofs
verified-static-allocation = ["static-allocation", "formal-verification-required"]
```

### Composable Safety Capabilities

```toml
# Memory constraints
compile-time-capacity-limits = []     # Bounded collections with compile-time limits
runtime-bounds-checking = []          # Runtime safety validation
no-runtime-allocation = ["compile-time-capacity-limits"]
memory-budget-enforcement = ["compile-time-capacity-limits"]

# Verification capabilities
formal-verification-required = ["dep:kani-verifier"]
mathematical-proofs = ["formal-verification-required"]
redundant-safety-checks = ["runtime-bounds-checking"]

# Isolation capabilities
memory-isolation = ["memory-budget-enforcement"]
component-isolation = ["memory-isolation"] 
hardware-isolation = ["component-isolation"]
```

### Convenience Bundles

```toml
# ASIL-A/B equivalent
bounded-collections = [
    "compile-time-capacity-limits",
    "runtime-bounds-checking", 
    "basic-monitoring"
]

# ASIL-C equivalent
static-memory-safety = [
    "static-allocation",
    "memory-budget-enforcement",
    "component-isolation"
]

# ASIL-D equivalent
maximum-safety = [
    "verified-static-allocation",
    "mathematical-proofs",
    "redundant-safety-checks", 
    "hardware-isolation"
]
```

## Usage Examples

### Safety Level Selection

```bash
# QM Level (development, maximum flexibility)
cargo build --features dynamic-allocation

# ASIL-A/B Level (bounded safety)
cargo build --features bounded-collections

# ASIL-C Level (static memory safety)
cargo build --features static-memory-safety

# ASIL-D Level (maximum safety)
cargo build --features maximum-safety
```

### Cross-Standard Compatibility

```bash
# ISO 26262 ASIL-D
cargo build --features maximum-safety

# DO-178C DAL-A (same capabilities)
cargo build --features maximum-safety  

# IEC 61508 SIL-4 (same capabilities)
cargo build --features maximum-safety
```

### Custom Configurations

```bash
# Custom: Static allocation with basic monitoring (no formal verification)
cargo build --features "static-allocation,basic-monitoring"

# Custom: Bounded collections with formal verification
cargo build --features "bounded-collections,formal-verification-required"

# Custom: Maximum isolation without mathematical proofs
cargo build --features "static-allocation,hardware-isolation"
```

## Safety-Aware Code Features

### Automatic Allocation Strategy Selection

```rust
use kiln_foundation::safety_aware_alloc;

// Automatically selects allocation strategy based on enabled features:
// - QM: Dynamic allocation allowed
// - ASIL-A/B: 64KB size limit enforced
// - ASIL-C: 32KB size limit enforced  
// - ASIL-D: 16KB size limit + power-of-2 requirement
let memory_guard = safety_aware_alloc!(8192, CrateId::Component)?;
```

### Memory Strategy Auto-Selection

```rust
// ResourceTable automatically selects memory strategy:
// - QM: ZeroCopy (performance)
// - ASIL-A/B: BoundedCopy (safety)
// - ASIL-C: Isolated (high safety)
// - ASIL-D: FullIsolation (maximum safety)
let table = ResourceTable::new()?;
assert_eq!(table.default_memory_strategy(), MemoryStrategy::for_current_safety_level());
```

### Verification Level Auto-Selection

```rust
// Verification level automatically matches safety requirements:
// - QM: None (fastest performance)
// - ASIL-A/B/C: Critical (key operations verified)
// - ASIL-D: Full (all operations verified)
let level = VerificationLevel::for_current_safety_level();
```

## Compile-Time Safety Validation

The system includes `compile_error!` macros that prevent invalid feature combinations:

```rust
// These combinations will fail to compile:
cargo build --features "dynamic-allocation,static-allocation"        // ERROR: Mutually exclusive
cargo build --features "verified-static-allocation"                  // ERROR: Missing formal-verification-required
cargo build --features "mathematical-proofs"                         // ERROR: Missing compile-time-memory-layout
```

## Legacy Compatibility

Existing safety features continue to work by mapping to capability bundles:

```toml
# Legacy features automatically map to new capabilities
safety-asil-b = ["bounded-collections"]
safety-asil-c = ["static-memory-safety"] 
safety-asil-d = ["maximum-safety"]
safe-memory = ["bounded-collections"]
```

## Runtime Capability Detection

```rust
use kiln_foundation::safety_features::runtime;

// Check what capabilities are enabled
if runtime::has_capability("verified-static-allocation") {
    // ASIL-D level code
}

// Get current safety level
let level = runtime::current_safety_level(); // "maximum-safety", "static-memory-safety", etc.

// Get maximum allocation size for current level
let max_size = runtime::max_allocation_size(); // 16KB for ASIL-D, 32KB for ASIL-C, etc.
```

## Benefits

### 1. Clear Intent
```rust
// Old: What does this mean?
#[cfg(feature = "safety-asil-d")]

// New: Exactly what it enforces  
#[cfg(feature = "verified-static-allocation")]
```

### 2. Flexible Composition
```rust
// Mix capabilities as needed for specific requirements
cargo build --features "static-allocation,formal-verification-required,basic-monitoring"
```

### 3. Standard Independence
```rust
// Same code works for any safety standard
impl SafetyStandardMapping for NewStandard {
    fn required_capabilities() -> &'static [&'static str] {
        &["static-allocation", "component-isolation"] 
    }
}
```

### 4. Gradual Migration
```rust
// Incrementally add safety capabilities
// Phase 1: Add bounds checking
cargo build --features "runtime-bounds-checking"

// Phase 2: Add static allocation
cargo build --features "static-allocation"

// Phase 3: Add formal verification  
cargo build --features "verified-static-allocation"
```

## Implementation Status

✅ **Completed**:
- Core capability feature definitions in `kiln-foundation`
- Safety-aware allocation macros (`safety_aware_alloc!`)
- Memory strategy auto-selection based on safety level
- Verification level auto-selection based on safety level
- Compile-time feature validation with `compile_error!` macros
- Legacy compatibility mapping
- Runtime capability detection API
- Integration with resource management system

✅ **Tested**:
- All safety levels build successfully with `kiln-foundation`
- Feature validation prevents invalid combinations
- Memory strategies correctly selected based on features
- Custom capability combinations work as expected

This capability-based system elevates Kiln from "Safety-Aware" to "Safety-Enforced" with clear, composable, and verifiable safety guarantees.