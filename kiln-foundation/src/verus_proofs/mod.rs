// Kiln - kiln-foundation
// Module: Verus Formal Verification Proofs
// SW-REQ-ID: REQ_MEM_SAFETY_001, REQ_RESOURCE_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Verus formal verification proofs for Kiln's safety-critical data structures.
//!
//! This module contains SMT-backed deductive proofs that verify unbounded
//! correctness properties of Kiln's core collections. Unlike Kani (bounded
//! model checking), Verus proves properties for ALL possible inputs and
//! ALL possible sequences of operations.
//!
//! # Architecture
//!
//! Each proof module contains a *verified model* of the corresponding
//! production type. The model mirrors the production type's fields and
//! logic, but adds Verus specifications (`requires`, `ensures`, `invariant`)
//! that the SMT solver checks.
//!
//! # Build Integration
//!
//! This module is gated behind `#[cfg(verus)]` and is invisible to normal
//! `cargo build`. To run verification:
//!
//! ```bash
//! verus --crate-type lib kiln-foundation/src/verus_proofs/static_vec_proofs.rs
//! ```
//!
//! # Verification Tiers
//!
//! | Tier | Tool | Scope | Annotation Effort |
//! |------|------|-------|-------------------|
//! | 1 | Kani | Bounded model checking (up to unwind depth N) | Low |
//! | 2 | Verus | SMT-backed unbounded proofs | Medium |
//! | 3 | Rocq/Coq | Full theorem proving | High |
//!
//! This module implements Tier 2 (Verus) verification.

pub mod static_vec_proofs;
pub mod static_queue_proofs;
pub mod static_string_proofs;
pub mod safe_memory_proofs;
