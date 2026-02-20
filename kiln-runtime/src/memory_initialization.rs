//! WRT Memory Initialization Pattern
//!
//! This module demonstrates the recommended pattern for initializing 
//! the WRT runtime with strict memory budget enforcement.

use wrt_foundation::{
    memory_budget::{initialize_global_budget, global_budget}, 
    memory_enforcement::{pre_allocate_component_memory, lock_memory_system},
    safety_system::SafetyLevel,
    global_memory_config::{initialize_global_memory_system, global_memory_config},
    Result, Error, ErrorCategory, codes,
};

use wrt_platform::comprehensive_limits::discover_platform_limits;

/// Runtime initialization phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationPhase {
    /// System startup - configuring limits
    Startup,
    /// Pre-allocating component memory
    PreAllocation,
    /// All memory allocated, system locked
    Locked,
    /// Runtime execution (no new allocations)
    Runtime,
}

/// WRT Runtime Memory Manager
pub struct RuntimeMemoryManager {
    phase: InitializationPhase,
    safety_level: SafetyLevel,
    total_budget: usize,
    allocated_components: Vec<(u32, usize)>, // (component_id, allocated_bytes)
}

impl RuntimeMemoryManager {
    /// Initialize the runtime memory manager
    /// 
    /// This is the main entry point for setting up memory management.
    /// Call this once at application startup.
    pub fn initialize(safety_level: SafetyLevel) -> Result<Self> {
        // Step 1: Discover platform limits
        let platform_limits = discover_platform_limits()
            .map_err(|_| Error::platform_error("Failed to discover platform memory limits"))?;

        // Step 2: Initialize global memory system with platform limits
        #[cfg(feature = "platform-memory")]
        initialize_global_memory_system()?;

        // Step 3: Initialize budget system with discovered limits
        let total_budget = platform_limits.max_total_memory;
        initialize_global_budget(total_budget, safety_level)?;

        println!("WRT Memory Manager initialized:");
        println!("  Platform: {:?}", platform_limits.platform_id);
        println!("  Total Budget: {} bytes ({} MB)", total_budget, total_budget / (1024 * 1024);
        println!("  Safety Level: {:?}", safety_level);

        Ok(Self {
            phase: InitializationPhase::Startup,
            safety_level,
            total_budget,
            allocated_components: Vec::new(),
        })
    }

    /// Pre-allocate memory for a component
    /// 
    /// This must be called during initialization phase before lock_system().
    pub fn pre_allocate_component(
        &mut self,
        component_id: u32,
        memory_requirements: &[(usize, &str)],
        component_safety_level: SafetyLevel,
    ) -> Result<Vec<Box<[u8]>>> {
        if self.phase != InitializationPhase::Startup && self.phase != InitializationPhase::PreAllocation {
            return Err(Error::memory_error("Cannot pre-allocate: initialization phase complete";
        }

        self.phase = InitializationPhase::PreAllocation;

        // Register component with budget system
        let total_component_memory: usize = memory_requirements.iter().map(|(size, _)| *size).sum);
        global_budget()?.register_component(component_id, total_component_memory, component_safety_level)?;

        // Pre-allocate all memory for this component
        let allocated_blocks = pre_allocate_component_memory(
            component_id,
            memory_requirements,
            component_safety_level,
        )?;

        self.allocated_components.push((component_id, total_component_memory);

        println!("Pre-allocated {} bytes for component {} (Safety: {:?})", 
                 total_component_memory, component_id, component_safety_level;

        Ok(allocated_blocks)
    }

    /// Lock the memory system after all components are initialized
    /// 
    /// After this call, no new memory allocations are allowed.
    /// The system enters runtime mode.
    pub fn lock_system(mut self) -> Result<LockedRuntimeMemoryManager> {
        if self.phase == InitializationPhase::Locked || self.phase == InitializationPhase::Runtime {
            return Err(Error::memory_error("System already locked";
        }

        // Lock the memory system
        lock_memory_system()?;
        self.phase = InitializationPhase::Locked;

        let total_allocated: usize = self.allocated_components.iter().map(|(_, size)| *size).sum);
        let utilization = (total_allocated * 100) / self.total_budget.max(1;

        println!("Memory system locked:");
        println!("  Total allocated: {} bytes ({} MB)", total_allocated, total_allocated / (1024 * 1024);
        println!("  Budget utilization: {}%", utilization);
        println!("  Components: {}", self.allocated_components.len();

        Ok(LockedRuntimeMemoryManager {
            phase: InitializationPhase::Locked,
            safety_level: self.safety_level,
            total_budget: self.total_budget,
            allocated_components: self.allocated_components,
        })
    }

    /// Get current memory statistics
    pub fn memory_stats(&self) -> RuntimeMemoryStats {
        let total_allocated: usize = self.allocated_components.iter().map(|(_, size)| *size).sum);
        RuntimeMemoryStats {
            phase: self.phase,
            total_budget: self.total_budget,
            total_allocated,
            component_count: self.allocated_components.len(),
            utilization_percent: (total_allocated * 100) / self.total_budget.max(1),
        }
    }
}

/// Locked runtime memory manager - no further allocations allowed
pub struct LockedRuntimeMemoryManager {
    phase: InitializationPhase,
    safety_level: SafetyLevel,
    total_budget: usize,
    allocated_components: Vec<(u32, usize)>,
}

impl LockedRuntimeMemoryManager {
    /// Enter runtime execution mode
    pub fn enter_runtime(mut self) -> RuntimeExecutionManager {
        self.phase = InitializationPhase::Runtime;
        println!("Entering runtime execution mode - memory allocation locked");
        
        RuntimeExecutionManager {
            safety_level: self.safety_level,
            total_budget: self.total_budget,
            allocated_components: self.allocated_components,
        }
    }

    /// Get memory statistics for the locked system
    pub fn memory_stats(&self) -> RuntimeMemoryStats {
        let total_allocated: usize = self.allocated_components.iter().map(|(_, size)| *size).sum);
        RuntimeMemoryStats {
            phase: self.phase,
            total_budget: self.total_budget,
            total_allocated,
            component_count: self.allocated_components.len(),
            utilization_percent: (total_allocated * 100) / self.total_budget.max(1),
        }
    }
}

/// Runtime execution manager - all memory pre-allocated, system locked
pub struct RuntimeExecutionManager {
    safety_level: SafetyLevel,
    total_budget: usize,
    allocated_components: Vec<(u32, usize)>,
}

impl RuntimeExecutionManager {
    /// Verify system integrity (can be called periodically)
    pub fn verify_system_integrity(&self) -> Result<()> {
        // Verify memory system is still locked
        if !wrt_foundation::memory_enforcement::is_memory_system_locked() {
            return Err(Error::safety_violation("Memory system lock compromised";
        }

        // Additional integrity checks based on safety level
        match self.safety_level.asil_level() {
            wrt_foundation::safety_system::AsildLevel::C | 
            wrt_foundation::safety_system::AsildLevel::D => {
                // Enhanced verification for highest safety levels
                self.verify_component_integrity()?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Verify integrity of component allocations
    fn verify_component_integrity(&self) -> Result<()> {
        // Check that all component allocations are still within budget
        let stats = global_memory_config().memory_stats);
        let expected_total: usize = self.allocated_components.iter().map(|(_, size)| *size).sum);
        
        if stats.allocated > expected_total {
            return Err(Error::safety_violation("Unexpected memory allocation detected";
        }

        Ok(())
    }

    /// Get current runtime statistics
    pub fn runtime_stats(&self) -> RuntimeStats {
        let total_allocated: usize = self.allocated_components.iter().map(|(_, size)| *size).sum);
        RuntimeStats {
            safety_level: self.safety_level,
            total_budget: self.total_budget,
            total_allocated,
            component_count: self.allocated_components.len(),
            utilization_percent: (total_allocated * 100) / self.total_budget.max(1),
            is_locked: true,
        }
    }
}

/// Runtime memory statistics
#[derive(Debug, Clone)]
pub struct RuntimeMemoryStats {
    pub phase: InitializationPhase,
    pub total_budget: usize,
    pub total_allocated: usize,
    pub component_count: usize,
    pub utilization_percent: usize,
}

/// Runtime execution statistics
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    pub safety_level: SafetyLevel,
    pub total_budget: usize,
    pub total_allocated: usize,
    pub component_count: usize,
    pub utilization_percent: usize,
    pub is_locked: bool,
}

/// Example usage pattern
#[cfg(test)]
mod example_usage {
    use super::*;

    #[test]
    fn demonstrate_initialization_pattern() -> Result<()> {
        // Step 1: Initialize memory manager
        let mut memory_manager = RuntimeMemoryManager::initialize(SafetyLevel::AsilC)?;

        // Step 2: Pre-allocate memory for each component
        let component1_memory = memory_manager.pre_allocate_component(
            1, // component_id
            &[
                (1024 * 1024, "Main buffer"),      // 1MB main buffer
                (256 * 1024, "Work buffer"),       // 256KB work buffer
                (64 * 1024, "Debug buffer"),       // 64KB debug buffer
            ],
            SafetyLevel::AsilC,
        )?;

        let component2_memory = memory_manager.pre_allocate_component(
            2, // component_id
            &[
                (512 * 1024, "Processing buffer"), // 512KB processing buffer
                (128 * 1024, "Output buffer"),     // 128KB output buffer
            ],
            SafetyLevel::AsilB,
        )?;

        // Step 3: Lock the system after all components are initialized
        let locked_manager = memory_manager.lock_system()?;

        // Step 4: Enter runtime execution
        let runtime_manager = locked_manager.enter_runtime);

        // Step 5: During runtime, verify system integrity periodically
        runtime_manager.verify_system_integrity()?;

        // Step 6: Get runtime statistics
        let stats = runtime_manager.runtime_stats);
        println!("Runtime stats: {:?}", stats);

        Ok(())
    }
}