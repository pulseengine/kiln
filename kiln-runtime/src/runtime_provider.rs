//! Runtime memory provider that intelligently chooses between stack and
//! platform allocation
//!
//! This module provides a smart runtime provider that uses stack allocation for
//! small sizes and platform allocation for large sizes to prevent stack
//! overflow.

use wrt_error::Error;
// Box is re-exported by wrt_foundation
use wrt_foundation::Box;
use wrt_foundation::{
    budget_aware_provider::CrateId,
    capabilities::{
        CapabilityAwareProvider,
        DynamicMemoryCapability,
        MemoryCapability,
    },
    heap_provider::HeapProvider,
    safe_memory::{
        NoStdProvider,
        Provider,
    },
    verification::VerificationLevel,
};

/// Stack allocation threshold - use platform allocator for sizes above this
const STACK_ALLOCATION_THRESHOLD: usize = 4096; // 4KB

/// Smart runtime provider that chooses appropriate allocation strategy
#[derive(Debug)]
pub enum SmartRuntimeProvider {
    /// Small allocation using stack
    Stack(CapabilityAwareProvider<NoStdProvider<4096>>),
    /// Large allocation using heap
    Heap(CapabilityAwareProvider<HeapProvider>),
}

impl SmartRuntimeProvider {
    /// Create a new runtime provider with appropriate allocation strategy
    pub fn new(size: usize, crate_id: CrateId) -> wrt_error::Result<Self> {
        if size <= STACK_ALLOCATION_THRESHOLD {
            // Use stack allocation for small sizes
            let base_provider = NoStdProvider::<4096>::default();
            let capability = DynamicMemoryCapability::new(
                STACK_ALLOCATION_THRESHOLD,
                crate_id,
                VerificationLevel::Standard,
            );
            let capability_provider =
                CapabilityAwareProvider::new(base_provider, Box::new(capability), crate_id);
            Ok(Self::Stack(capability_provider))
        } else {
            // Use heap allocation for large sizes
            let base_provider = HeapProvider::new(size)?;
            let capability =
                DynamicMemoryCapability::new(size, crate_id, VerificationLevel::Standard);
            let capability_provider =
                CapabilityAwareProvider::new(base_provider, Box::new(capability), crate_id);
            Ok(Self::Heap(capability_provider))
        }
    }
}

// SmartRuntimeProvider is just a wrapper enum and doesn't implement Provider
// trait directly The actual Provider implementations are in the wrapped types

/// Create a runtime provider that uses appropriate allocation strategy
pub fn create_smart_runtime_provider(size: usize) -> wrt_error::Result<SmartRuntimeProvider> {
    SmartRuntimeProvider::new(size, CrateId::Runtime)
}
