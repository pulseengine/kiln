//! Build tests for capability-based engine
//!
//! These tests ensure that the capability engine builds correctly
//! with all feature combinations.

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "std", feature = "alloc"))]
    use wrt_runtime::engine::{
        CapabilityAwareEngine,
        EnginePreset,
    };

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_capability_engine_builds_with_all_features() {
        // Test that each feature combination builds
        #[cfg(feature = "qm")]
        {
            let engine = CapabilityAwareEngine::with_preset(EnginePreset::QM);
            assert!(engine.is_ok(), "QM engine should build successfully");
        }

        #[cfg(feature = "asil-a")]
        {
            let engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilA);
            assert!(engine.is_ok(), "ASIL-A engine should build successfully");
        }

        #[cfg(feature = "asil-b")]
        {
            let engine = CapabilityAwareEngine::with_preset(EnginePreset::AsilB);
            assert!(engine.is_ok(), "ASIL-B engine should build successfully");
        }

        // Test default QM build
        #[cfg(not(any(feature = "qm", feature = "asil-a", feature = "asil-b")))]
        {
            let engine = CapabilityAwareEngine::with_preset(EnginePreset::QM);
            assert!(
                engine.is_ok(),
                "Default QM engine should build successfully"
            );
        }
    }

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_engine_handle_types() {
        use wrt_runtime::engine::{
            InstanceHandle,
            ModuleHandle,
        };

        // Test handle creation
        let module_handle = ModuleHandle::new();
        let instance_handle = InstanceHandle::from_index(0);

        // Test handle comparison
        let another_module = ModuleHandle::new();
        assert_ne!(
            module_handle, another_module,
            "Module handles should be unique"
        );

        // Test instance handle indexing
        assert_eq!(instance_handle.index(), 0);
        let instance_handle2 = InstanceHandle::from_index(42);
        assert_eq!(instance_handle2.index(), 42);
    }

    #[test]
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn test_preset_enum() {
        use wrt_runtime::engine::EnginePreset;

        // Test that all variants can be created
        let _qm = EnginePreset::QM;
        let _asil_a = EnginePreset::AsilA;
        let _asil_b = EnginePreset::AsilB;

        // Test equality
        assert_eq!(EnginePreset::QM, EnginePreset::QM);
        assert_ne!(EnginePreset::QM, EnginePreset::AsilA);
    }
}
