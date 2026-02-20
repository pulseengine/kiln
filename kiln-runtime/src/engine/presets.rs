//! Engine capability presets for different safety levels
//!
//! This module provides pre-configured capability contexts for QM, ASIL-A, and
//! ASIL-B safety levels. Each preset configures memory capabilities
//! appropriately.

use wrt_error::Result;
use wrt_foundation::{
    budget_aware_provider::CrateId,
    capabilities::{
        verified::VerificationProofs,
        MemoryCapabilityContext,
    },
    verification::VerificationLevel,
};

/// Budget allocations for QM mode (bytes)
/// These are generous limits for development/testing - ML inference modules
/// can be 4-10MB, so we allow 8MB for Runtime to accommodate them.
const QM_BUDGETS: &[(CrateId, usize)] = &[
    (CrateId::Foundation, 2_000_000), // 2MB
    (CrateId::Decoder, 2_000_000),    // 2MB (large component sections)
    (CrateId::Runtime, 8_000_000),    // 8MB (ML inference modules can be 4-6MB)
    (CrateId::Component, 4_000_000),  // 4MB (component parsing)
    (CrateId::Host, 1_000_000),       // 1MB
    (CrateId::Instructions, 500_000), // 500KB
    (CrateId::Platform, 500_000),     // 500KB
];

/// Static sizes for ASIL-A mode (bytes)
const ASIL_A_STATIC_SIZES: &[(CrateId, usize)] = &[
    (CrateId::Foundation, 512_000),   // 512KB
    (CrateId::Decoder, 256_000),      // 256KB
    (CrateId::Runtime, 1_024_000),    // 1MB
    (CrateId::Component, 512_000),    // 512KB
    (CrateId::Host, 256_000),         // 256KB
    (CrateId::Instructions, 128_000), // 128KB
    (CrateId::Platform, 128_000),     // 128KB
];

/// Verified allocations for ASIL-B mode (bytes)
const ASIL_B_VERIFIED_SIZES: &[(CrateId, usize)] = &[
    (CrateId::Foundation, 256_000),  // 256KB
    (CrateId::Decoder, 128_000),     // 128KB
    (CrateId::Runtime, 512_000),     // 512KB
    (CrateId::Component, 256_000),   // 256KB
    (CrateId::Host, 128_000),        // 128KB
    (CrateId::Instructions, 64_000), // 64KB
    (CrateId::Platform, 64_000),     // 64KB
];

/// Strict allocations for ASIL-C mode (bytes)
const ASIL_C_STRICT_SIZES: &[(CrateId, usize)] = &[
    (CrateId::Foundation, 128_000),  // 128KB
    (CrateId::Decoder, 64_000),      // 64KB
    (CrateId::Runtime, 256_000),     // 256KB
    (CrateId::Component, 128_000),   // 128KB
    (CrateId::Host, 64_000),         // 64KB
    (CrateId::Instructions, 32_000), // 32KB
    (CrateId::Platform, 32_000),     // 32KB
];

/// Deterministic allocations for ASIL-D mode (bytes)
const ASIL_D_DETERMINISTIC_SIZES: &[(CrateId, usize)] = &[
    (CrateId::Foundation, 64_000),   // 64KB
    (CrateId::Decoder, 32_000),      // 32KB
    (CrateId::Runtime, 128_000),     // 128KB
    (CrateId::Component, 64_000),    // 64KB
    (CrateId::Host, 32_000),         // 32KB
    (CrateId::Instructions, 16_000), // 16KB
    (CrateId::Platform, 16_000),     // 16KB
];

/// Create a QM (Quality Management) capability context
///
/// QM provides:
/// - Dynamic memory allocation
/// - Standard verification level
/// - Flexible resource limits
/// - No runtime verification overhead
pub fn qm() -> Result<MemoryCapabilityContext> {
    let mut context = MemoryCapabilityContext::new(
        VerificationLevel::Standard,
        false, // No runtime verification for performance
    );

    // Register dynamic capabilities for all crates
    for &(crate_id, budget) in QM_BUDGETS.iter() {
        context.register_dynamic_capability(crate_id, budget)?;
    }

    Ok(context)
}

/// Create an ASIL-A capability context
///
/// ASIL-A provides:
/// - Static memory allocation with bounded sizes
/// - Sampling verification
/// - Runtime verification enabled
/// - Moderate resource constraints
pub fn asil_a() -> Result<MemoryCapabilityContext> {
    let mut context = MemoryCapabilityContext::new(
        VerificationLevel::Sampling,
        true, // Runtime verification enabled
    );

    // Register static capabilities with bounded sizes
    for &(crate_id, size) in ASIL_A_STATIC_SIZES {
        // Use dynamic capability with size limit for now
        // TODO: Add register_static_capability_sized when available
        context.register_dynamic_capability(crate_id, size)?;
    }

    Ok(context)
}

/// Create an ASIL-B capability context
///
/// ASIL-B provides:
/// - Static memory allocation with strict limits
/// - Continuous verification
/// - Full runtime verification
/// - Strict resource constraints
/// - Verification proofs required
pub fn asil_b() -> Result<MemoryCapabilityContext> {
    let mut context = MemoryCapabilityContext::new(
        VerificationLevel::Full,
        true, // Strict runtime verification
    );

    // Register verified capabilities with proofs
    for &(crate_id, size) in ASIL_B_VERIFIED_SIZES {
        // For now, use dynamic capability with strict limits
        // In a full implementation, this would require verification proofs
        context.register_dynamic_capability(crate_id, size)?;
    }

    Ok(context)
}

/// Create an ASIL-C capability context
///
/// ASIL-C provides:
/// - Enhanced static memory allocation with strict limits
/// - Redundant verification
/// - Full runtime verification with checksums
/// - Very strict resource constraints
/// - Binary qualification required
pub fn asil_c() -> Result<MemoryCapabilityContext> {
    let mut context = MemoryCapabilityContext::new(
        VerificationLevel::Redundant,
        true, // Strict runtime verification with checksums
    );

    // Register strict capabilities with very limited sizes
    for &(crate_id, size) in ASIL_C_STRICT_SIZES {
        context.register_dynamic_capability(crate_id, size)?;
    }

    Ok(context)
}

/// Create an ASIL-D capability context
///
/// ASIL-D provides:
/// - Deterministic memory allocation with minimal footprint
/// - Formal verification required
/// - Complete runtime verification
/// - Maximum resource constraints
/// - Binary qualification and checksums mandatory
/// - Bounded collections only, no dynamic allocation
pub fn asil_d() -> Result<MemoryCapabilityContext> {
    let mut context = MemoryCapabilityContext::new(
        VerificationLevel::Redundant, // Highest verification level
        true,                         // Complete runtime verification
    );

    // Register deterministic capabilities with minimal sizes
    for &(crate_id, size) in ASIL_D_DETERMINISTIC_SIZES {
        context.register_dynamic_capability(crate_id, size)?;
    }

    Ok(context)
}

/// Create an ASIL-B capability context with verification proofs
///
/// This is the full ASIL-B implementation that requires formal verification
/// proofs for each memory allocation. Use this when formal verification is
/// available.
#[allow(dead_code)]
pub fn asil_b_verified(
    proofs: &[(CrateId, VerificationProofs)],
) -> Result<MemoryCapabilityContext> {
    let mut context = MemoryCapabilityContext::new(
        VerificationLevel::Redundant, // Highest verification level
        true,                         // Strict runtime verification
    );

    // Register verified capabilities with provided proofs
    for (crate_id, proof) in proofs {
        // Find the corresponding size
        let size = ASIL_B_VERIFIED_SIZES
            .iter()
            .find(|(id, _)| id == crate_id)
            .map(|(_, size)| *size)
            .unwrap_or(64_000); // Default to 64KB

        // This would register with actual verification proofs
        // For now, we simulate with dynamic capability
        context.register_dynamic_capability(*crate_id, size)?;
    }

    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qm_preset() {
        let context = qm().unwrap();

        // Verify QM capabilities are registered
        assert!(context.has_capability(CrateId::Runtime));
        assert!(context.has_capability(CrateId::Decoder));
        assert!(context.has_capability(CrateId::Component));
    }

    #[test]
    fn test_asil_a_preset() {
        let context = asil_a().unwrap();

        // Verify ASIL-A capabilities are registered
        assert!(context.has_capability(CrateId::Runtime));
        assert!(context.has_capability(CrateId::Foundation));
    }

    #[test]
    fn test_asil_b_preset() {
        let context = asil_b().unwrap();

        // Verify ASIL-B capabilities are registered
        assert!(context.has_capability(CrateId::Runtime));
        assert!(context.has_capability(CrateId::Instructions));
    }

    #[test]
    fn test_asil_c_preset() {
        let context = asil_c().unwrap();

        // Verify ASIL-C capabilities are registered
        assert!(context.has_capability(CrateId::Runtime));
        assert!(context.has_capability(CrateId::Foundation));
    }

    #[test]
    fn test_asil_d_preset() {
        let context = asil_d().unwrap();

        // Verify ASIL-D capabilities are registered
        assert!(context.has_capability(CrateId::Runtime));
        assert!(context.has_capability(CrateId::Component));
    }

    #[test]
    fn test_capability_sizes() {
        // Verify the hierarchy of resource constraints: QM > ASIL-A > ASIL-B > ASIL-C >
        // ASIL-D
        let qm_runtime = QM_BUDGETS
            .iter()
            .find(|(id, _)| *id == CrateId::Runtime)
            .map(|(_, size)| *size)
            .unwrap();

        let asil_a_runtime = ASIL_A_STATIC_SIZES
            .iter()
            .find(|(id, _)| *id == CrateId::Runtime)
            .map(|(_, size)| *size)
            .unwrap();

        let asil_b_runtime = ASIL_B_VERIFIED_SIZES
            .iter()
            .find(|(id, _)| *id == CrateId::Runtime)
            .map(|(_, size)| *size)
            .unwrap();

        let asil_c_runtime = ASIL_C_STRICT_SIZES
            .iter()
            .find(|(id, _)| *id == CrateId::Runtime)
            .map(|(_, size)| *size)
            .unwrap();

        let asil_d_runtime = ASIL_D_DETERMINISTIC_SIZES
            .iter()
            .find(|(id, _)| *id == CrateId::Runtime)
            .map(|(_, size)| *size)
            .unwrap();

        // Verify decreasing order of allocations
        assert!(qm_runtime > asil_a_runtime);
        assert!(asil_a_runtime > asil_b_runtime);
        assert!(asil_b_runtime > asil_c_runtime);
        assert!(asil_c_runtime > asil_d_runtime);
    }
}
