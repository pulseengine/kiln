# wrt-sync

> Synchronization primitives for WebAssembly runtime

## Overview

Provides cross-platform synchronization primitives optimized for WebAssembly runtimes. Supports std, no_std+alloc, and pure no_std environments with consistent APIs across all configurations.

## Features

- **🔒 WrtMutex**: Mutual exclusion primitive with transparent std/no_std adaptation
- **📖 WrtRwLock**: Reader-writer lock for concurrent read access
- **🔄 WrtOnce**: One-time initialization primitive
- **🏷️ Cross-platform**: Consistent API across std and no_std environments
- **⚡ Performance**: Optimized implementations for each environment
- **🛡️ Safety**: Formal verification support via Kani

## Quick Start

```toml
[dependencies]
wrt-sync = "0.1"
```

### Basic Usage

```rust
use wrt_sync::{Mutex, RwLock, Once};

// Mutex for exclusive access
let mutex = Mutex::new(42);
{
    let mut guard = mutex.lock().unwrap();
    *guard = 100;
}

// RwLock for concurrent reads
let rwlock = RwLock::new(vec![1, 2, 3]);
let read_guard = rwlock.read().unwrap();
println!("Data: {:?}", *read_guard);

// Once for one-time initialization
static INIT: Once = Once::new();
INIT.call_once(|| {
    println!("Initialized!");
});
```

## Environment Support

### Standard Library
```toml
wrt-sync = { version = "0.1", features = ["std"] }
```
Uses `parking_lot` for high-performance synchronization.

### no_std + alloc
```toml
wrt-sync = { version = "0.1", features = ["alloc"] }
```
Custom implementations with heap allocation.

### Pure no_std
```toml
wrt-sync = { version = "0.1", default-features = false }
```
Spin-lock based implementations, no heap allocation.

## API Reference

### WrtMutex
```rust
use wrt_sync::Mutex;

let mutex = Mutex::new(String::from("hello"));
let guard = mutex.lock().unwrap();
println!("{}", *guard);
```

### WrtRwLock
```rust
use wrt_sync::RwLock;

let lock = RwLock::new(5);

// Multiple readers
let r1 = lock.read().unwrap();
let r2 = lock.read().unwrap();
assert_eq!(*r1, 5);
assert_eq!(*r2, 5);
drop(r1);
drop(r2);

// Exclusive writer
let mut w = lock.write().unwrap();
*w = 10;
```

### WrtOnce
```rust
use wrt_sync::Once;

static START: Once = Once::new();

START.call_once(|| {
    // Expensive initialization here
    setup_runtime();
});
```

## Performance Characteristics

| Environment | Mutex | RwLock | Once | Memory |
|-------------|-------|--------|------|--------|
| **std** | parking_lot | parking_lot | std::sync | Dynamic |
| **no_std+alloc** | Custom | Custom | Custom | Heap |
| **pure no_std** | Spinlock | Spinlock | Atomic | Static |

## See Also

- [API Documentation](https://docs.rs/wrt-sync)
- [Synchronization Guide](../docs/source/architecture/sync.rst)
- [WRT Architecture](../docs/source/architecture/)