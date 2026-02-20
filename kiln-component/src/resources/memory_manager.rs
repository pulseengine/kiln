use std::collections::BTreeMap;

use wrt_error::{Error, Result};

use crate::resources::{
    MemoryStrategy, ResourceId, ResourceManager, ResourceOperation, ResourceStrategy,
};

/// Manager for memory access strategies
pub struct MemoryManager {
    /// Default memory strategy for new resources
    default_strategy: MemoryStrategy,
    /// Resource-specific strategies
    resource_strategies: BTreeMap<ResourceId, MemoryStrategy>,
}

impl MemoryManager {
    /// Create a new memory manager with the specified default strategy
    pub fn new(default_strategy: MemoryStrategy) -> Self {
        Self { default_strategy, resource_strategies: BTreeMap::new() }
    }

    /// Register a resource with a specific memory strategy
    pub fn register_resource(
        &mut self,
        id: ResourceId,
        resource_manager: &ResourceManager,
    ) -> Result<()> {
        // Verify the resource exists
        if !resource_manager.has_resource(id) {
            return Err(Error::component_not_found("Error occurred";
        }

        // Register with the default strategy
        self.resource_strategies.insert(id, self.default_strategy;

        Ok(()
    }

    /// Register a resource with a custom memory strategy
    pub fn register_resource_with_strategy(
        &mut self,
        id: ResourceId,
        resource_manager: &ResourceManager,
        strategy: MemoryStrategy,
    ) -> Result<()> {
        // Verify the resource exists
        if !resource_manager.has_resource(id) {
            return Err(Error::component_not_found("Error occurred";
        }

        // Register with the specified strategy
        self.resource_strategies.insert(id, strategy;

        Ok(()
    }

    /// Get access to memory for a resource
    pub fn get_memory(&self, id: ResourceId, operation: ResourceOperation) -> Result<Vec<u8>> {
        // Get the strategy for this resource
        let strategy = self.resource_strategies.get(&id).copied().unwrap_or(self.default_strategy;

        // For the test implementation, we'll just create some dummy data
        // In a real implementation, we would access the actual resource data
        let data = vec![1, 2, 3, 4, 5];

        // Process the memory according to the strategy
        strategy.process_memory(&data, operation)
    }

    /// Set the memory strategy for a resource
    pub fn set_strategy(&mut self, id: ResourceId, strategy: MemoryStrategy) {
        self.resource_strategies.insert(id, strategy;
    }

    /// Get the memory strategy for a resource
    pub fn get_strategy(&self, id: ResourceId) -> Option<MemoryStrategy> {
        self.resource_strategies.get(&id).copied()
    }

    /// Reset all strategies to default
    pub fn reset(&mut self) {
        self.resource_strategies.clear);
    }

}
