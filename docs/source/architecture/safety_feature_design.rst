# Generic Safety Feature Design for Kiln

## Philosophy: Capability-Based Safety Features

Instead of standard-specific features like `safety-asil-d`, we define features based on **what they enforce**. This makes the system:

1. **Standard-Agnostic**: Works with ISO 26262, DO-178C, IEC 61508, EN 50128, etc.
2. **Self-Documenting**: Feature names describe exactly what they do
3. **Composable**: Can mix and match capabilities as needed
4. **Future-Proof**: New standards can map to existing capabilities

## Proposed Feature Hierarchy

### Tier 1: Unrestricted (QM/Development)
```toml
# No safety features enabled - maximum flexibility
default = ["dynamic-allocation"]
```

### Tier 2: Bounded Safety 
```toml
# ASIL-A/B, DAL-D/C, SIL-1/2 equivalent
bounded-collections = [
    "compile-time-capacity-limits",
    "runtime-bounds-checking", 
    "basic-monitoring"
]
```

### Tier 3: Static Safety
```toml  
# ASIL-C, DAL-B, SIL-3 equivalent
static-allocation = [
    "bounded-collections",
    "no-runtime-allocation",
    "compile-time-memory-layout",
    "memory-budget-enforcement"
]
```

### Tier 4: Verified Static Safety
```toml
# ASIL-D, DAL-A, SIL-4 equivalent  
verified-static-allocation = [
    "static-allocation",
    "formal-verification-required",
    "redundant-safety-checks",
    "mathematical-proofs"
]
```

## Detailed Feature Definitions

### Memory Allocation Features

```toml
# Dynamic allocation (QM level)
dynamic-allocation = []

# Compile-time capacity enforcement  
compile-time-capacity-limits = ["!dynamic-allocation"]

# Runtime bounds checking
runtime-bounds-checking = []

# No runtime allocation after initialization
no-runtime-allocation = ["!dynamic-allocation", "compile-time-capacity-limits"]

# Complete static memory layout
compile-time-memory-layout = ["no-runtime-allocation"]

# Memory budget enforcement
memory-budget-enforcement = ["compile-time-capacity-limits"]
```

### Verification Features

```toml
# Basic monitoring and telemetry
basic-monitoring = []

# Formal verification integration
formal-verification-required = ["compile-time-memory-layout"]

# Redundant safety checks
redundant-safety-checks = ["runtime-bounds-checking"]

# Mathematical proof requirements
mathematical-proofs = ["formal-verification-required"]
```

### Isolation Features

```toml
# Component memory isolation
memory-isolation = ["memory-budget-enforcement"]

# Cross-component communication restrictions
component-isolation = ["memory-isolation"]

# Hardware-level isolation support
hardware-isolation = ["component-isolation"]
```

## Implementation Strategy

### 1. Feature-Driven Type Selection

```rust
// In kiln-foundation/src/safety_features.rs

#[cfg(feature = "dynamic-allocation")]
pub type DefaultMemoryProvider = DynamicProvider;

#[cfg(all(feature = "bounded-collections", not(feature = "static-allocation")))]
pub type DefaultMemoryProvider<const N: usize> = BoundedProvider<N>;

#[cfg(all(feature = "static-allocation", not(feature = "verified-static-allocation")))]
pub type DefaultMemoryProvider<const N: usize> = StaticProvider<N>;

#[cfg(feature = "verified-static-allocation")]
pub type DefaultMemoryProvider<const N: usize> = VerifiedStaticProvider<N>;
```

### 2. Compile-Time Feature Validation

```rust
// Mutually exclusive features
#[cfg(all(feature = "dynamic-allocation", feature = "no-runtime-allocation"))]
compile_error!("Cannot enable both dynamic-allocation and no-runtime-allocation");

#[cfg(all(feature = "static-allocation", feature = "dynamic-allocation"))]
compile_error!("Cannot enable both static-allocation and dynamic-allocation");

// Feature dependencies
#[cfg(all(feature = "verified-static-allocation", not(feature = "formal-verification-required")))]
compile_error!("verified-static-allocation requires formal-verification-required");

#[cfg(all(feature = "mathematical-proofs", not(feature = "compile-time-memory-layout")))]
compile_error!("mathematical-proofs requires compile-time-memory-layout");
```

### 3. Safety-Level Mapping

```rust
// In kiln-foundation/src/safety_mapping.rs

pub trait SafetyStandardMapping {
    fn required_features() -> &'static [&'static str];
}

// ISO 26262 Mapping
impl SafetyStandardMapping for AsilLevel {
    fn required_features() -> &'static [&'static str] {
        match self {
            AsilLevel::QM => &["dynamic-allocation"],
            AsilLevel::AsilA | AsilLevel::AsilB => &["bounded-collections"],
            AsilLevel::AsilC => &["static-allocation"],
            AsilLevel::AsilD => &["verified-static-allocation"],
        }
    }
}

// DO-178C Mapping  
impl SafetyStandardMapping for DalLevel {
    fn required_features() -> &'static [&'static str] {
        match self {
            DalLevel::DalE => &["dynamic-allocation"],
            DalLevel::DalD | DalLevel::DalC => &["bounded-collections"],
            DalLevel::DalB => &["static-allocation"], 
            DalLevel::DalA => &["verified-static-allocation"],
        }
    }
}
```

### 4. Automatic Allocation Strategy

```rust
// In kiln-foundation/src/auto_allocation.rs

/// Automatically select allocation strategy based on enabled features
#[macro_export]
macro_rules! auto_allocate {
    ($size:expr, $crate_id:expr) => {{
        #[cfg(feature = "dynamic-allocation")]
        {
            DynamicAllocator::allocate($size, $crate_id)
        }
        
        #[cfg(all(feature = "bounded-collections", not(feature = "static-allocation")))]
        {
            BoundedAllocator::<$size>::allocate($crate_id)
        }
        
        #[cfg(all(feature = "static-allocation", not(feature = "verified-static-allocation")))]
        {
            StaticAllocator::<$size>::allocate($crate_id)
        }
        
        #[cfg(feature = "verified-static-allocation")]
        {
            VerifiedStaticAllocator::<$size>::allocate_with_proof($crate_id)
        }
    }};
}
```

## Cargo.toml Integration

### Foundation Crate Features

```toml
# kiln-foundation/Cargo.toml
[features]
default = ["dynamic-allocation"]

# Core allocation strategies (mutually exclusive)
dynamic-allocation = []
static-allocation = ["!dynamic-allocation", "no-runtime-allocation"]
verified-static-allocation = ["static-allocation", "formal-verification-required"]

# Capability features (composable)
compile-time-capacity-limits = []
runtime-bounds-checking = []  
no-runtime-allocation = ["compile-time-capacity-limits"]
compile-time-memory-layout = ["no-runtime-allocation"]
memory-budget-enforcement = ["compile-time-capacity-limits"]

# Verification features
basic-monitoring = []
formal-verification-required = ["dep:kani-verifier"]
redundant-safety-checks = ["runtime-bounds-checking"]
mathematical-proofs = ["formal-verification-required"]

# Isolation features
memory-isolation = ["memory-budget-enforcement"]
component-isolation = ["memory-isolation"]
hardware-isolation = ["component-isolation"]

# Convenience bundles (equivalent to standard levels)
bounded-collections = [
    "compile-time-capacity-limits",
    "runtime-bounds-checking",
    "basic-monitoring"
]

static-memory-safety = [
    "static-allocation", 
    "memory-budget-enforcement",
    "component-isolation"
]

maximum-safety = [
    "verified-static-allocation",
    "mathematical-proofs", 
    "redundant-safety-checks",
    "hardware-isolation"
]
```

### Workspace-Level Configuration  

```toml
# Cargo.toml (workspace)
[features]
# Safety level presets for easy configuration
qm = ["kiln-foundation/dynamic-allocation"]

asil-a = ["kiln-foundation/bounded-collections"]
asil-b = ["kiln-foundation/bounded-collections"] 
asil-c = ["kiln-foundation/static-memory-safety"]
asil-d = ["kiln-foundation/maximum-safety"]

# DO-178C equivalents
dal-e = ["qm"]
dal-d = ["asil-a"]
dal-c = ["asil-b"] 
dal-b = ["asil-c"]
dal-a = ["asil-d"]

# IEC 61508 equivalents
sil-1 = ["asil-a"]
sil-2 = ["asil-b"]
sil-3 = ["asil-c"] 
sil-4 = ["asil-d"]
```

## Usage Examples

### Standard-Specific Configuration

```bash
# ISO 26262 ASIL-D
cargo build --features asil-d

# DO-178C DAL-A  
cargo build --features dal-a

# IEC 61508 SIL-4
cargo build --features sil-4

# All equivalent - use maximum-safety features
```

### Custom Configuration

```bash
# Custom: Static allocation with basic monitoring (no formal verification)
cargo build --features "static-allocation,basic-monitoring"

# Custom: Bounded collections with formal verification
cargo build --features "bounded-collections,formal-verification-required"

# Custom: Maximum isolation without formal proofs
cargo build --features "static-allocation,hardware-isolation"
```

### Development vs Production

```bash
# Development: Fast compilation, full debugging
cargo build --features "dynamic-allocation,debug-full"

# Production ASIL-C: Static allocation, no debug overhead
cargo build --features "asil-c" --release

# Certification: Maximum safety with all proofs
cargo build --features "asil-d,mathematical-proofs" --release
```

## Implementation Benefits

### 1. **Self-Documenting Code**
```rust
// Clear what's enabled just from feature name
#[cfg(feature = "verified-static-allocation")]
fn requires_formal_proof() { }

#[cfg(feature = "redundant-safety-checks")]  
fn double_check_operation() { }
```

### 2. **Flexible Standard Support**
```rust
// Easy to add new standards
impl SafetyStandardMapping for NewStandard {
    fn required_features() -> &'static [&'static str] {
        // Map to existing capability features
        &["static-allocation", "component-isolation"]
    }
}
```

### 3. **Gradual Migration**
```rust
// Can enable features incrementally
// Phase 1: Add bounds checking
cargo build --features "runtime-bounds-checking"

// Phase 2: Add static allocation  
cargo build --features "static-allocation"

// Phase 3: Add formal verification
cargo build --features "verified-static-allocation"
```

### 4. **Clear Feature Dependencies**
```toml
# Dependencies are explicit and logical
verified-static-allocation = [
    "static-allocation",           # Must have static allocation
    "formal-verification-required", # Must have formal verification
    "mathematical-proofs"          # Must have mathematical proofs
]
```

This approach transforms Kiln from having abstract safety levels to having concrete, enforceable capabilities that clearly communicate what safety guarantees are provided.