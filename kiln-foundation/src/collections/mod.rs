// WRT - wrt-foundation
// Module: Static Collections (heapless-inspired)
// SW-REQ-ID: REQ_RESOURCE_001, REQ_MEM_SAFETY_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Static, inline-storage collections for safety-critical systems.
//!
//! This module provides heapless-inspired data structures with:
//! - **Inline storage**: Data stored directly in the struct (no Provider abstraction)
//! - **Compile-time capacity**: Bounds enforced via const generics
//! - **Zero allocation**: All memory layout determined at compile time
//! - **Deterministic WCET**: Constant-time operations for ASIL-D compliance
//! - **RAII cleanup**: Automatic resource management via Drop
//!
//! # Design Principles
//!
//! 1. **Simplicity**: Certifiable code minimizes abstractions
//! 2. **Static by Default**: ASIL-D mode = zero runtime allocation
//! 3. **Explicit Capacity**: Compile-time bounds, runtime `Result`
//! 4. **Const-Time Ops**: Deterministic WCET for safety-critical
//!
//! # Usage
//!
//! ```rust
//! use wrt_foundation::collections::StaticVec;
//!
//! // Create a vector with capacity 10
//! let mut vec = StaticVec::<u32, 10>::new();
//!
//! // Push elements (returns Result)
//! vec.push(42)?;
//! vec.push(100)?;
//!
//! // Access elements
//! assert_eq!(vec.get(0), Some(&42));
//! assert_eq!(vec.len(), 2);
//!
//! // Iterate
//! for value in vec.iter() {
//!     println!("{}", value);
//! }
//! # Ok::<(), wrt_error::Error>(())
//! ```
//!
//! # ASIL Compliance
//!
//! - **ASIL-D**: Use default feature (core-only, no allocation)
//! - **ASIL-C**: Use with `alloc` feature (static pools)
//! - **QM**: Use with `std` feature (full standard library)
//!
//! # Requirements Traceability
//!
//! - REQ_RESOURCE_001: Static resource allocation
//! - REQ_MEM_SAFETY_001: Memory bounds validation
//! - REQ_TEMPORAL_001: Bounded execution time
//! - REQ_LFUNC_026: Minimize complexity for certification

mod static_vec;
mod static_queue;
mod static_map;

pub use static_vec::StaticVec;
pub use static_queue::StaticQueue;
pub use static_map::StaticMap;
