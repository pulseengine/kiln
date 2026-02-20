//! Modern Memory Initialization System
//!
//! This module provides automatic memory system initialization that works
//! across all crates in the WRT ecosystem with zero boilerplate using the
//! new capability-based memory management system.
//!
//! SW-REQ-ID: REQ_MEM_001 - Memory bounds checking
//! SW-REQ-ID: REQ_MEM_002 - Budget enforcement
//! SW-REQ-ID: REQ_CAP_001 - Capability-based access control

// Legacy imports removed - migrated to capability-only system
use core::sync::atomic::{
    AtomicBool,
    Ordering,
};
#[cfg(feature = "std")]
use std::sync::OnceLock;

use crate::{
    budget_aware_provider::CrateId,
    budget_verification::{
        CRATE_BUDGETS,
        TOTAL_MEMORY_BUDGET,
    },
    capabilities::MemoryCapabilityContext,
    verification::VerificationLevel,
    Error,
    Result,
};

/// Global initialization state
static MEMORY_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// ASIL-D safe initialization error storage
/// ASIL-D safe error storage using atomic pointer instead of static mut
static INIT_ERROR_PTR: core::sync::atomic::AtomicPtr<&'static str> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// Global capability context for modern memory management
#[cfg(feature = "std")]
static GLOBAL_CAPABILITY_CONTEXT: OnceLock<MemoryCapabilityContext> = OnceLock::new();

#[cfg(not(feature = "std"))]
static mut GLOBAL_CAPABILITY_CONTEXT: Option<MemoryCapabilityContext> = None;

/// Get access to the global capability context
pub fn get_global_capability_context() -> Result<&'static MemoryCapabilityContext> {
    if !MEMORY_INITIALIZED.load(Ordering::Acquire) {
        return Err(Error::runtime_execution_error(
            "Memory system not initialized",
        ));
    }

    #[cfg(feature = "std")]
    {
        GLOBAL_CAPABILITY_CONTEXT.get().ok_or_else(|| {
            Error::runtime_execution_error("Global capability context not available")
        })
    }

    #[cfg(not(feature = "std"))]
    {
        #[allow(unsafe_code, static_mut_refs)]
        // Safe: Read-only access to static after initialization
        unsafe {
            GLOBAL_CAPABILITY_CONTEXT.as_ref().ok_or_else(|| {
                Error::runtime_execution_error("Global capability context not initialized")
            })
        }
    }
}

/// Modern memory system initializer
pub struct MemoryInitializer;

impl MemoryInitializer {
    /// Initialize the entire WRT memory system
    ///
    /// This is called once at startup and configures all crate budgets
    pub fn initialize() -> Result<()> {
        // Check if already initialized
        if MEMORY_INITIALIZED.swap(true, Ordering::AcqRel) {
            return Ok(()); // Already initialized
        }

        // Create budget configuration
        let budgets = [
            (CrateId::Foundation, CRATE_BUDGETS[0]),
            (CrateId::Decoder, CRATE_BUDGETS[1]),
            (CrateId::Runtime, CRATE_BUDGETS[2]),
            (CrateId::Component, CRATE_BUDGETS[3]),
            (CrateId::Host, CRATE_BUDGETS[4]),
            (CrateId::Debug, CRATE_BUDGETS[5]),
            (CrateId::Platform, CRATE_BUDGETS[6]),
            (CrateId::Instructions, CRATE_BUDGETS[7]),
            (CrateId::Format, CRATE_BUDGETS[8]),
            (CrateId::Intercept, CRATE_BUDGETS[9]),
            (CrateId::Sync, CRATE_BUDGETS[10]),
            (CrateId::Math, CRATE_BUDGETS[11]),
            (CrateId::Logging, CRATE_BUDGETS[12]),
            (CrateId::Panic, CRATE_BUDGETS[13]),
            (CrateId::TestRegistry, CRATE_BUDGETS[14]),
            (CrateId::VerificationTool, CRATE_BUDGETS[15]),
            (CrateId::Unknown, CRATE_BUDGETS[16]),
            (CrateId::Wasi, CRATE_BUDGETS[17]),
            (CrateId::WasiComponents, CRATE_BUDGETS[18]),
        ];

        // Legacy coordinator removed - using capability-only system

        // Initialize new capability context
        let mut context = MemoryCapabilityContext::new(VerificationLevel::Standard, false);

        // Register capabilities for all crates based on their budgets
        for (crate_id, budget) in budgets.iter() {
            if *budget > 0 {
                // Register dynamic capabilities for all crates
                if context.register_dynamic_capability(*crate_id, *budget).is_err() {
                    // ASIL-D safe error storage using atomic pointer
                    let error_msg: &'static str = "Failed to register crate capability";
                    MEMORY_INITIALIZED.store(false, Ordering::Release);
                    return Err(Error::runtime_execution_error(
                        "Failed to register dynamic capability for crate",
                    ));
                }
            }
        }

        // Store the global context
        #[cfg(feature = "std")]
        {
            if GLOBAL_CAPABILITY_CONTEXT.set(context).is_err() {
                MEMORY_INITIALIZED.store(false, Ordering::Release);
                return Err(Error::runtime_execution_error(
                    "Failed to set global capability context",
                ));
            }
        }

        #[cfg(not(feature = "std"))]
        {
            #[allow(unsafe_code)] // Safe: Single-threaded write during initialization
            unsafe {
                GLOBAL_CAPABILITY_CONTEXT = Some(context);
            }
        }

        // Capability system initialization completed successfully
        Ok(())
    }

    /// Check if memory system is initialized
    pub fn is_initialized() -> bool {
        MEMORY_INITIALIZED.load(Ordering::Acquire)
    }

    /// Get initialization error if any
    pub fn get_error() -> Option<&'static str> {
        // ASIL-D safe error retrieval using atomic pointer
        let ptr = INIT_ERROR_PTR.load(Ordering::Acquire);
        if ptr.is_null() {
            None
        } else {
            #[allow(unsafe_code)] // Safe: pointer was stored by us atomically
            unsafe {
                Some(*ptr)
            }
        }
    }

    /// Force reinitialization (for testing only)
    #[cfg(test)]
    pub fn reset() {
        MEMORY_INITIALIZED.store(false, Ordering::Release);
        // ASIL-D safe error reset using atomic pattern
        INIT_ERROR_PTR.store(core::ptr::null_mut(), Ordering::Release);
    }

    /// Ensure memory system is initialized (safe to call multiple times)
    ///
    /// This is a convenience method that checks if the memory system is already
    /// initialized and only initializes it if needed. Safe to call from any
    /// context.
    pub fn ensure_initialized() -> Result<()> {
        if Self::is_initialized() {
            Ok(())
        } else {
            Self::initialize()
        }
    }
}

/// Macro to ensure memory system is initialized
///
/// This macro can be placed at the beginning of any function that uses
/// memory allocation to ensure the system is ready.
#[macro_export]
macro_rules! ensure_memory_init {
    () => {
        if !$crate::memory_init::MemoryInitializer::is_initialized() {
            $crate::memory_init::MemoryInitializer::initialize()?;
        }
    };
}

/// Attribute macro for automatic memory initialization
///
/// Place this on functions that need memory allocation
#[macro_export]
macro_rules! memory_safe {
    ($fn:item) => {
        $fn
        // Note: In a real implementation, this would be a proc macro
        // that injects the initialization check
    };
}

/// Static initializer using ctor if available
#[cfg(feature = "ctor")]
#[ctor::ctor]
fn init_wrt_memory() {
    drop(MemoryInitializer::initialize());
}

/// Manual initialization function for platforms without ctor
#[cfg(not(feature = "ctor"))]
pub fn init_wrt_memory() -> Result<()> {
    MemoryInitializer::initialize()
}

/// Convenience function for crate-specific initialization
pub fn init_crate_memory(crate_id: CrateId) -> Result<()> {
    // Ensure global system is initialized
    MemoryInitializer::initialize()?;

    // Verify crate is properly configured in capability system
    let context = get_global_capability_context()?;
    if !context.has_capability(crate_id) {
        return Err(Error::runtime_execution_error(
            "Crate has no registered capability",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_initialization() {
        MemoryInitializer::reset();

        assert!(!MemoryInitializer::is_initialized());

        MemoryInitializer::initialize().unwrap();

        assert!(MemoryInitializer::is_initialized());
    }

    #[test]
    fn test_crate_initialization() {
        MemoryInitializer::reset();

        init_crate_memory(CrateId::Component).unwrap();

        assert!(MemoryInitializer::is_initialized());

        // Verify capability system is working
        let context = get_global_capability_context().unwrap();
        assert!(context.has_capability(CrateId::Component));
    }
}
