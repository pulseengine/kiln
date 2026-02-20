//! Component Model resource type implementation
//!
//! This module provides resource type handling for the WebAssembly Component
//! Model, including resource lifetime management, memory optimization, and
//! interception support.

#[cfg(not(feature = "std"))]
use core::time::Duration;
#[cfg(feature = "std")]
use std::{sync::Weak, time::Duration};

// Submodules
pub mod bounded_buffer_pool;
#[cfg(feature = "std")]
pub mod buffer_pool;
pub mod dynamic_quota_manager;
#[cfg(feature = "std")]
pub mod memory_access;
pub mod memory_strategy;
pub mod resource_arena; // Consolidated: contains both std and no_std implementations
pub mod resource_builder;
pub mod resource_interceptor;
pub mod resource_lifecycle;
pub mod resource_manager; // Consolidated: contains both std and no_std implementations
#[cfg(feature = "std")]
pub mod resource_operation;
// resource_operation_no_std.rs removed - was an empty stub
pub mod resource_strategy; // Consolidated: contains both trait and implementation
#[cfg(feature = "std")]
pub mod resource_table;
#[cfg(feature = "std")]
pub mod resource_table_budget_integration;
pub mod resource_table_no_std;
#[cfg(feature = "std")]
pub mod size_class_buffer_pool;

// Re-export for no_std feature
#[cfg(not(feature = "std"))]
pub use bounded_buffer_pool::{BoundedBufferPool, BoundedBufferStats as BufferPoolStats};
// Re-export for std feature
#[cfg(feature = "std")]
pub use buffer_pool::BufferPool;
// Export dynamic quota management
pub use dynamic_quota_manager::{
    DynamicQuotaManager, QuotaNode, QuotaNodeType, QuotaPolicy, QuotaRequest, QuotaResponse,
    QuotaStatus, QuotaStrategy, QuotaWatcher, ResourceType as QuotaResourceType,
};
#[cfg(feature = "std")]
pub use memory_access::MemoryAccessMode;
// Common re-exports for both std and no_std
// pub use memory_strategy::MemoryStrategy as MemoryStrategyTrait;
// ResourceArena handles std/no_std internally
pub use resource_arena::ResourceArena;
// Export Builder types
pub use resource_builder::{ResourceBuilder, ResourceManagerBuilder, ResourceTableBuilder};
// Export ResourceInterceptor
pub use resource_interceptor::ResourceInterceptor;
// Export ResourceId and ResourceManager (consolidated implementation)
pub use resource_manager::{HostResource, ResourceId, ResourceManager};
// Export resource_operation based on feature flags
#[cfg(feature = "std")]
pub use resource_operation::{from_format_resource_operation, to_format_resource_operation};

// Export ResourceStrategy trait and implementation (works for both std and no_std)
pub use resource_strategy::{
    GenericResourceStrategy, MAX_RESOURCE_BUFFER_SIZE, ResourceStrategy, ResourceStrategyNoStd,
};
// Re-export MAX_BUFFER_SIZE directly from wrt_foundation for public access
pub use wrt_foundation::bounded::MAX_BUFFER_SIZE;
// Export ResourceTable components based on feature flags
#[cfg(feature = "std")]
pub use resource_table::{
    BufferPoolTrait, MemoryStrategy, Resource, ResourceTable, VerificationLevel,
};
#[cfg(feature = "std")]
pub use resource_table_budget_integration::{
    BudgetAwareResourceTablePool, ResourceTableUsageStats, create_budget_aware_resource_table,
    verify_budget_integration,
};
#[cfg(not(feature = "std"))]
pub use resource_table_no_std::{
    BufferPoolTrait, MemoryStrategy, Resource, ResourceTable, VerificationLevel,
};
// Export size class buffer pool for std environment
#[cfg(feature = "std")]
pub use size_class_buffer_pool::{BufferPoolStats, SizeClassBufferPool};

/// Timestamp implementation for no_std
#[derive(Debug, Clone, Copy)]
pub struct Instant {
    // Store a monotonic counter for elapsed time simulation
    dummy: u64,
}

impl Instant {
    // Create a new instant at the current monotonic time
    pub fn now() -> Self {
        // In a real implementation, we might read from a hardware timer
        // Here we just use a placeholder value for no_std compatibility
        Self { dummy: 0 }
    }

    // Get the elapsed time since this instant was created
    pub fn elapsed(&self) -> Duration {
        // In a real implementation, we'd compare with the current monotonic time
        // Here we just return zero for no_std compatibility
        Duration::from_secs(0)
    }

    // Calculate the duration between two instants
    pub fn duration_since(&self, earlier: &Self) -> Duration {
        // Just a placeholder implementation for no_std
        Duration::from_secs(0)
    }
}
