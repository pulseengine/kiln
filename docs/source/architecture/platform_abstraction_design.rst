# Platform Abstraction Architecture for ASIL Compliance

## Overview

This document describes the Platform Abstraction Interface (PAI) architecture implemented in kiln-runtime to achieve ASIL compliance and proper separation of concerns between platform-dependent and platform-independent code.

## Problem Statement

The original architecture had several issues:

1. **Direct Platform Dependencies**: Runtime modules directly imported platform-specific types from `kiln_platform::sync`
2. **Missing Feature Gates**: Platform-specific code wasn't properly gated behind feature flags
3. **Circular Dependencies**: Platform abstractions were scattered across multiple crates
4. **ASIL Compliance Violations**: No clear boundary between safety-critical and platform-specific code

## Solution: Platform Abstraction Interface (PAI)

### Core Design Principles

1. **Zero-Cost Abstractions**: All platform abstractions compile down to direct calls through inlining
2. **Feature-Gated Imports**: Platform dependencies are only imported when appropriate features are enabled
3. **Fallback Implementations**: Pure no_std environments have safe fallbacks for all platform features
4. **Clear Module Boundaries**: Platform-specific code is isolated in designated modules

### Architecture Components

#### 1. Platform Stubs Module (`platform_stubs.rs`)

This module serves as the central abstraction layer:

```rust
// Platform-agnostic atomic types with fallbacks
#[cfg(feature = "platform-sync")]
pub use kiln_platform::sync::{AtomicU32, AtomicU64, AtomicUsize};

#[cfg(not(feature = "platform-sync"))]
pub use self::atomic_fallback::{AtomicU32, AtomicU64, AtomicUsize};
```

Features:
- Provides platform-agnostic atomic types with spinlock-based fallbacks
- Exports Duration abstraction that works in all environments
- Defines PlatformInterface trait for runtime configuration

#### 2. Feature Structure

```toml
# Platform features in Cargo.toml
platform-sync = ["dep:kiln-platform", "kiln-platform?/std"]
platform-macos = ["platform-sync", "kiln-platform?/platform-macos"]
platform-linux = ["platform-sync", "kiln-platform?/platform-linux"]
platform-qnx = ["platform-sync", "kiln-platform?/platform-qnx"]
platform-embedded = ["dep:kiln-platform"] # Minimal support
```

Benefits:
- `platform-sync` is the master feature for platform synchronization
- Platform-specific features require `platform-sync`
- Embedded platforms can use minimal features without full sync

#### 3. Import Pattern

All runtime modules now import platform abstractions through the PAI:

```rust
// Instead of:
use kiln_platform::sync::{AtomicU32, AtomicU64};

// Use:
use crate::platform_stubs::{AtomicU32, AtomicU64, PlatformOrdering};
```

#### 4. ASIL Level Mapping

Each platform has a default ASIL level:

```rust
impl PlatformId {
    pub const fn default_asil_level(&self) -> AsilLevel {
        match self {
            PlatformId::Linux => AsilLevel::QM,
            PlatformId::QNX => AsilLevel::AsilB,
            PlatformId::Embedded => AsilLevel::AsilD,
            // ...
        }
    }
}
```

### Compilation Modes

#### 1. Pure no_std (ASIL-D)
- No platform dependencies
- Spinlock-based atomic fallbacks
- Static memory allocation only
- Deterministic compilation

#### 2. no_std + platform-sync (ASIL-B/C)
- Platform atomics available
- Limited dynamic features
- Safety-critical platform support

#### 3. std (QM/Development)
- Full platform features
- Dynamic allocation
- Development tools

### Migration Guide

To adapt existing code to use PAI:

1. Replace direct `kiln_platform` imports with `platform_stubs` imports
2. Add appropriate feature gates for platform-specific code
3. Provide fallbacks for pure no_std environments
4. Test compilation in all ASIL modes

### Benefits

1. **ASIL Compliance**: Clear separation between safety levels
2. **Deterministic Builds**: Feature combinations are well-defined
3. **Portability**: Code works on all platforms with appropriate fallbacks
4. **Zero Overhead**: Abstractions compile away completely
5. **Testability**: Each platform configuration can be tested independently

## Implementation Status

The following modules have been migrated to use PAI:
- `atomic_execution.rs`
- `atomic_memory_model.rs`
- `wait_queue.rs`
- `module_instance.rs`

Remaining work:
- Complete migration of remaining platform-dependent modules
- Add platform-specific tests
- Implement full kiln-platform integration
- Verify ASIL compliance with build matrix