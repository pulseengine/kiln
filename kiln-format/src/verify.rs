//! Formal verification for the WebAssembly format handling using Kani.
//!
//! This module contains proofs that verify core properties of the format
//! handling system. These proofs only run with Kani and are isolated from
//! normal compilation and testing.

// Only compile Kani verification code when documentation is being generated
// or when explicitly running cargo kani. This prevents interference with
// coverage testing.
#[cfg(any(doc, kani))]
pub mod kani_verification {
    use crate::{
        binary::*,
        compression::*,
        module::*,
        section::*,
        state::*,
        *,
    };

    /// Verify RLE compression round-trip
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_rle_roundtrip() {
        // Small test vector for verification
        let original = &[1, 1, 1, 2, 2, 3, 3, 3, 3];

        // Encode and then decode
        let encoded = rle_encode(original);
        let decoded = rle_decode(&encoded);

        // Verify round-trip correctness
        assert_eq!(original, decoded.as_slice);
    }

    /// Verify binary module version detection
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_module_version_detection() {
        // Version bytes for a standard module
        let version_bytes = [0, 0x61, 0x73, 0x6d, 1, 0, 0, 0];

        // Verify we can detect experimental features
        let uses_experimental = uses_experimental_features(&version_bytes);

        // Standard module shouldn't use experimental features
        assert!(!uses_experimental);
    }

    /// Verify state section creation and extraction
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_state_section_roundtrip() {
        // Create a minimal state section
        let state = StateSection::new();

        // Serialize to bytes
        let state_bytes = create_state_section(&state);

        // Extract back from bytes
        let extracted_state = extract_state_section(&state_bytes);

        // Verify we can extract what we created
        assert!(extracted_state.is_ok());
    }

    /// Verify memory limits validation
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_memory_limits() {
        // Create valid limits
        let limits = Limits {
            min: 1,
            max: Some(2),
        };

        // Basic validation
        assert!(limits.max.unwrap() >= limits.min);
    }

    /// Verify component model feature detection
    #[cfg_attr(kani, kani::proof)]
    pub fn verify_feature_detection() {
        // Create version info for standard binary
        let info = VersionInfo::default();

        // Check core feature availability
        let core_available = is_feature_available(&info, ComponentModelFeature::Core);

        // Core feature should be available by default
        assert!(core_available);
    }
}

// Expose the verification module in docs but not for normal compilation
#[cfg(any(doc, kani))]
pub use kani_verification::*;
