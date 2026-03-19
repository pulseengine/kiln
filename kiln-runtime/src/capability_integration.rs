//! Capability Integration for Platform Runtime
//!
//! This module provides simple integration examples for the capability system.

use kiln_foundation::capabilities::{
    PlatformAllocator,
    PlatformCapabilityBuilder,
    PlatformCapabilityProvider,
};

use crate::prelude::*;

/// Simple demonstration of capability integration
pub fn create_simple_capability_provider(
    memory_limit: usize,
) -> Result<PlatformCapabilityProvider> {
    let allocator = Arc::new(SimpleAllocator);

    PlatformCapabilityBuilder::new()
        .with_memory_limit(memory_limit)
        .build(allocator)
}

/// Simple allocator for demonstration
#[derive(Debug)]
struct SimpleAllocator;

impl PlatformAllocator for SimpleAllocator {
    fn available_memory(&self) -> usize {
        1024 * 1024 * 1024 // 1GB
    }

    fn platform_id(&self) -> &str {
        "simple"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_capability_provider() {
        let provider = create_simple_capability_provider(1024 * 1024);
        assert!(provider.is_ok());
    }
}
