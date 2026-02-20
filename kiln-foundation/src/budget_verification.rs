//! Compile-time budget verification system
//! SW-REQ-ID: REQ_VERIFY_001 - Static verification
//! SW-REQ-ID: REQ_MEM_002 - Budget enforcement

use crate::budget_aware_provider::CrateId;

/// Number of crates in the system
pub const CRATE_COUNT: usize = 19;

/// Per-crate memory budgets (in bytes)
/// These are compile-time constants for verification
pub const CRATE_BUDGETS: [usize; CRATE_COUNT] = [
    256 * 1024,       // Foundation: 256 KiB
    512 * 1024,       // Decoder: 512 KiB
    1024 * 1024,      // Runtime: 1 MiB
    1024 * 1024,      // Component: 1 MiB
    256 * 1024,       // Host: 256 KiB
    128 * 1024,       // Debug: 128 KiB
    256 * 1024,       // Platform: 256 KiB
    128 * 1024,       // Instructions: 128 KiB
    256 * 1024,       // Format: 256 KiB
    128 * 1024,       // Intercept: 128 KiB
    64 * 1024,        // Sync: 64 KiB
    64 * 1024,        // Math: 64 KiB
    64 * 1024,        // Logging: 64 KiB
    32 * 1024,        // Panic: 32 KiB
    64 * 1024,        // TestRegistry: 64 KiB
    64 * 1024,        // VerificationTool: 64 KiB
    128 * 1024,       // Unknown/Reserve: 128 KiB
    4 * 1024 * 1024,  // Wasi: 4 MiB
    16 * 1024 * 1024, // WasiComponents: 16 MiB
];

/// Total system memory budget based on platform
/// SW-REQ-ID: REQ_MEM_PLATFORM_002 - Platform-specific memory configuration
#[cfg(target_os = "linux")]
pub const TOTAL_MEMORY_BUDGET: usize = 1024 * 1024 * 1024; // 1 GB for Linux

#[cfg(target_os = "macos")]
pub const TOTAL_MEMORY_BUDGET: usize = 1024 * 1024 * 1024; // 1 GB for macOS

#[cfg(target_os = "qnx")]
pub const TOTAL_MEMORY_BUDGET: usize = 512 * 1024 * 1024; // 512 MB for QNX

#[cfg(target_os = "vxworks")]
pub const TOTAL_MEMORY_BUDGET: usize = 256 * 1024 * 1024; // 256 MB for VxWorks

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub const TOTAL_MEMORY_BUDGET: usize = 64 * 1024 * 1024; // 64 MB for embedded ARM

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "qnx",
    target_os = "vxworks",
    all(target_arch = "arm", target_os = "none")
)))]
pub const TOTAL_MEMORY_BUDGET: usize = 128 * 1024 * 1024; // 128 MB default

/// Calculate total allocated budget across all crates
/// This is a const fn to enable compile-time evaluation
pub const fn calculate_total_budget() -> usize {
    let mut total = 0;
    let mut i = 0;

    // Const loop to sum all budgets
    while i < CRATE_COUNT {
        total += CRATE_BUDGETS[i];
        i += 1;
    }

    total
}

/// Verify that a specific crate has sufficient budget
/// Returns the available budget for the crate
pub const fn verify_crate_budget(crate_id: CrateId) -> usize {
    CRATE_BUDGETS[crate_id as usize]
}

/// Check if a specific allocation size fits within a crate's budget
pub const fn check_allocation_fits(crate_id: CrateId, size: usize) -> bool {
    CRATE_BUDGETS[crate_id as usize] >= size
}

/// Static assertion for total budget validation
/// This will cause a compile error if budgets exceed system memory
const fn validate_total_budget() {
    let total = calculate_total_budget();
    assert!(
        total <= TOTAL_MEMORY_BUDGET,
        "Total crate budgets exceed system memory budget!"
    );
}

// Force evaluation at compile time
const _: () = validate_total_budget();

/// Macro to verify crate budget at compile time
///
/// # Example
///
/// ```rust,ignore
/// verify_crate_budget!(CrateId::Component, 64 * 1024;
/// ```
#[macro_export]
macro_rules! verify_crate_budget {
    ($crate_id:expr, $required:expr) => {
        const _: () = {
            const CRATE_ID: usize = $crate_id as usize;
            const REQUIRED: usize = $required;
            const AVAILABLE: usize = $crate::budget_verification::CRATE_BUDGETS[CRATE_ID];
            assert!(
                AVAILABLE >= REQUIRED,
                concat!(
                    "Insufficient budget for crate! Required: ",
                    stringify!($required),
                    " bytes, but only ",
                    stringify!(AVAILABLE),
                    " bytes available"
                )
            );
        };
    };
}

/// Macro to initialize crate memory at runtime
///
/// This macro should be called once at crate initialization to register
/// the crate with the memory coordinator.
///
/// # Example
///
/// ```rust,ignore
/// init_crate_memory!(CrateId::Component;
/// ```
#[macro_export]
macro_rules! init_crate_memory {
    ($crate_id:expr) => {
        // Use ctor for automatic initialization if available
        #[cfg(all(not(test), feature = "ctor"))]
        #[ctor::ctor]
        fn __init_crate_memory() {
            use $crate::memory_budget::{
                CrateId,
                MEMORY_COORDINATOR,
            };

            // Register crate with coordinator
            if let Err(e) = MEMORY_COORDINATOR.register_crate($crate_id) {
                // Can't panic in ctor, so we need a different strategy
                #[cfg(feature = "defmt")]
                defmt::error!("Failed to register crate {:?}: {:?}", $crate_id, e);
            }
        }

        // Manual initialization function for platforms without ctor
        #[cfg(not(feature = "ctor"))]
        pub fn init_memory() {
            use $crate::memory_budget::{
                CrateId,
                MEMORY_COORDINATOR,
            };

            // Register crate with coordinator
            if let Err(e) = MEMORY_COORDINATOR.register_crate($crate_id) {
                // Log error but don't panic
                #[cfg(feature = "defmt")]
                defmt::error!("Failed to register crate {:?}: {:?}", $crate_id, e);
            }
        }
    };
}

/// Compile-time budget report
/// This can be used in build scripts to generate documentation
pub const fn generate_budget_report() -> BudgetReport {
    BudgetReport {
        total_system_memory: TOTAL_MEMORY_BUDGET,
        total_allocated:     calculate_total_budget(),
        crate_count:         CRATE_COUNT,
    }
}

/// Budget report structure
pub struct BudgetReport {
    pub total_system_memory: usize,
    pub total_allocated:     usize,
    pub crate_count:         usize,
}

impl BudgetReport {
    /// Get remaining unallocated memory
    pub const fn remaining_memory(&self) -> usize {
        self.total_system_memory - self.total_allocated
    }

    /// Get allocation percentage
    pub const fn allocation_percentage(&self) -> u8 {
        ((self.total_allocated * 100) / self.total_system_memory) as u8
    }

    /// Check if over-allocated
    pub const fn is_over_allocated(&self) -> bool {
        self.total_allocated > self.total_system_memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_calculation() {
        let total = calculate_total_budget();
        assert!(total <= TOTAL_MEMORY_BUDGET);
    }

    #[test]
    fn test_budget_report() {
        let report = generate_budget_report();
        assert!(!report.is_over_allocated());
        assert!(report.allocation_percentage() <= 100);
    }
}

/// Example compile-time checks that would fail if budgets are wrong
#[cfg(all(test, feature = "compile-time-checks"))]
mod compile_checks {
    use super::*;

    // This would fail at compile time if Component budget < 64KB
    verify_crate_budget!(CrateId::Component, 64 * 1024);

    // This would fail at compile time if Runtime budget < 128KB
    verify_crate_budget!(CrateId::Runtime, 128 * 1024);
}
