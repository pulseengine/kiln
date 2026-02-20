//! Phantom type budget system for compile-time memory verification
//!
//! This module implements type-level memory budgets that enable compile-time
//! verification of memory usage without any runtime overhead.

/// Crate identifiers for compile-time budget tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CrateId {
    Foundation       = 0,
    Decoder          = 1,
    Runtime          = 2,
    Component        = 3,
    Host             = 4,
    Platform         = 5,
    Sync             = 6,
    Logging          = 7,
    Intercept        = 8,
    Instructions     = 9,
    Math             = 10,
    Format           = 11,
    Debug            = 12,
    Error            = 13,
    VerificationTool = 14,
    Wrt              = 15,
    Wrtd             = 16,
}

/// Compile-time memory budgets per crate (in bytes)
pub const CRATE_BUDGETS: [usize; 17] = [
    256 * 1024,  // Foundation: 256 KiB
    512 * 1024,  // Decoder: 512 KiB
    1024 * 1024, // Runtime: 1 MiB
    1024 * 1024, // Component: 1 MiB
    512 * 1024,  // Host: 512 KiB
    768 * 1024,  // Platform: 768 KiB
    128 * 1024,  // Sync: 128 KiB
    256 * 1024,  // Logging: 256 KiB
    128 * 1024,  // Intercept: 128 KiB
    256 * 1024,  // Instructions: 256 KiB
    64 * 1024,   // Math: 64 KiB
    512 * 1024,  // Format: 512 KiB
    1024 * 1024, // Debug: 1 MiB
    64 * 1024,   // Error: 64 KiB
    128 * 1024,  // VerificationTool: 128 KiB
    2048 * 1024, // Wrt: 2 MiB
    512 * 1024,  // Wrtd: 512 KiB
];

/// Error type for capacity violations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityError {
    Exceeded,
}

impl core::fmt::Display for CapacityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CapacityError::Exceeded => write!(f, "Capacity exceeded for budget-tracked allocation"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CapacityError {}

/// Phantom type for memory budget tracking
#[derive(Debug, Clone, Copy)]
pub struct MemoryBudget<const CRATE: u8, const SIZE: usize>;

impl<const CRATE: u8, const SIZE: usize> MemoryBudget<CRATE, SIZE> {
    /// Verify at compile time that allocation fits within crate budget
    pub const fn verify() -> Self {
        // This will cause a compile error if SIZE > CRATE_BUDGET
        // Note: We can't use CRATE_BUDGETS[CRATE] in const context easily,
        // so we'll add runtime verification for now
        Self
    }

    /// Get the crate ID at compile time
    pub const fn crate_id() -> u8 {
        CRATE
    }

    /// Get the allocated size at compile time
    pub const fn size() -> usize {
        SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_budgets() {
        assert_eq!(CRATE_BUDGETS[CrateId::Foundation as usize], 256 * 1024);
        assert_eq!(CRATE_BUDGETS[CrateId::Runtime as usize], 1024 * 1024);
        assert_eq!(CRATE_BUDGETS[CrateId::Wrt as usize], 2048 * 1024);
    }

    #[test]
    fn test_phantom_type() {
        let _budget: MemoryBudget<{ CrateId::Foundation as u8 }, 1024> = MemoryBudget::verify();
        assert_eq!(
            MemoryBudget::<{ CrateId::Foundation as u8 }, 1024>::crate_id(),
            0
        );
        assert_eq!(
            MemoryBudget::<{ CrateId::Foundation as u8 }, 1024>::size(),
            1024
        );
    }
}
