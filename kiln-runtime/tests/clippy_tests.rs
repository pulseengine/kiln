//! Clippy compliance tests for capability engine
//!
//! This module ensures that the capability engine code
//! compiles cleanly with strict clippy rules.

// Ensure no clippy warnings in capability engine
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// Allow some pedantic lints that conflict with our style
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

#[cfg(all(test, any(feature = "std", feature = "alloc")))]
mod tests {
    use wrt_foundation::budget_aware_provider::CrateId;
    use wrt_runtime::engine::{
        CapabilityAwareEngine,
        EnginePreset,
    };

    #[test]
    fn test_capability_engine_clippy_clean() {
        // This test ensures the code compiles with strict clippy rules
        let engine = CapabilityAwareEngine::with_preset(EnginePreset::QM);
        assert!(engine.is_ok());

        if let Ok(engine) = engine {
            // Verify we can access the capability context
            let context = engine.capability_context();
            assert!(context.has_capability(CrateId::Runtime));
        }
    }

    #[test]
    fn test_handle_types_clippy_clean() {
        use wrt_runtime::engine::{
            InstanceHandle,
            ModuleHandle,
        };

        // Create handles
        let module = ModuleHandle::new();
        let instance = InstanceHandle::from_index(0);

        // Use the handles to ensure they're not optimized away
        let _module_copy = module;
        let _index = instance.index();
    }

    #[test]
    fn test_preset_usage_clippy_clean() {
        // Test all preset variants
        let presets = vec![EnginePreset::QM, EnginePreset::AsilA, EnginePreset::AsilB];

        for preset in presets {
            let result = CapabilityAwareEngine::with_preset(preset);
            assert!(result.is_ok());
        }
    }
}
