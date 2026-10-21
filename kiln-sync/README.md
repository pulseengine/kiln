# kiln-sync

> Synchronization primitives for WebAssembly runtime

## Overview

Provides cross-platform synchronization primitives optimized for WebAssembly runtimes. Supports std, no_std+alloc, and pure no_std environments with consistent APIs across all configurations.

## Features

- **🔒 KilnMutex**: Mutual exclusion primitive with transparent std/no_std adaptation
- **📖 KilnRwLock**: Reader-writer lock for concurrent read access
- **🔄 KilnOnce**: One-time initialization primitive
- **🏷️ Cross-platform**: Consistent API across std and no_std environments
- **⚡ Performance**: Optimized implementations for each environment
- **🛡️ Safety**: Formal verification support via Kani

## Quick Start

```toml
[dependencies]
kiln-sync = "0.1"
```

### Basic Usage

```rust
use kiln_sync::{Mutex, RwLock, KilnOnce};

// Mutex for exclusive access
let mutex = Mutex::new(42);
{
    let mut guard = mutex.lock();
    *guard = 100;
}

// RwLock for concurrent reads
let rwlock = RwLock::new(vec![1, 2, 3]);
let read_guard = rwlock.read();
println!("Data: {:?}", *read_guard);

// KilnOnce for one-time initialization
static INIT: KilnOnce<u32> = KilnOnce::new();
let value = INIT.get_or_init(|| {
    println!("Initialized!");
    7
});
assert_eq!(*value, 7);
```

## Environment Support

### Standard Library
```toml
kiln-sync = { version = "0.1", features = ["std"] }
```
Uses `parking_lot` for high-performance synchronization.

### no_std + alloc
```toml
kiln-sync = { version = "0.1", features = ["alloc"] }
```
Custom implementations with heap allocation.

### Pure no_std
```toml
kiln-sync = { version = "0.1", default-features = false }
```
Spin-lock based implementations, no heap allocation.

## API Reference

### KilnMutex
```rust
use kiln_sync::Mutex;

let mutex = Mutex::new(String::from("hello"));
let guard = mutex.lock();
println!("{}", *guard);
```

### KilnRwLock
```rust
use kiln_sync::RwLock;

let lock = RwLock::new(5);

// Multiple readers
let r1 = lock.read();
let r2 = lock.read();
assert_eq!(*r1, 5);
assert_eq!(*r2, 5);
drop(r1);
drop(r2);

// Exclusive writer
let mut w = lock.write();
*w = 10;
```

### KilnOnce
```rust
use kiln_sync::KilnOnce;

fn setup_runtime() -> u32 {
    // Expensive initialization here
    42
}

static START: KilnOnce<u32> = KilnOnce::new();

let runtime = START.get_or_init(setup_runtime);
assert_eq!(*runtime, 42);
```

## Performance Characteristics

| Environment | Mutex | RwLock | Once | Memory |
|-------------|-------|--------|------|--------|
| **std** | parking_lot | parking_lot | std::sync | Dynamic |
| **no_std+alloc** | Custom | Custom | Custom | Heap |
| **pure no_std** | Spinlock | Spinlock | Atomic | Static |

## See Also

- [API Documentation](https://docs.rs/kiln-sync)
- [Kiln Architecture](../docs/architecture/rfc46-toolchain-architecture.md)