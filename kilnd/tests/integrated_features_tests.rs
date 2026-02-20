//! Integration tests for wrtd with new WASI, component model, and platform
//! features
//!
//! These tests validate the complete integration of:
//! - WASI host function providers
//! - Component model support
//! - Memory profiling and monitoring
//! - Platform abstraction layer
//! - Host function registry
//! - Command-line argument parsing

#[cfg(all(test, feature = "std"))]
mod tests {
    use wrt_error::Result;

    use super::super::{
        MemoryProfiler,
        RuntimeStats,
        WasiVersion,
        WrtdConfig,
        WrtdEngine,
    };

    /// Test basic WASI integration with Preview 1
    #[test]
    #[cfg(feature = "wasi")]
    fn test_wasi_preview1_integration() {
        let mut config = WrtdConfig::default();
        config.enable_wasi = true;
        config.wasi_version = WasiVersion::Preview1;
        config.enable_memory_profiling = true;

        // Add some WASI capabilities
        use wrt_wasi::WasiCapabilities;
        let mut capabilities = WasiCapabilities::minimal();
        capabilities.environment.args_access = true;
        capabilities.environment.environ_access = true;
        capabilities.environment.add_allowed_var("PATH");
        capabilities.filesystem.add_allowed_path("/tmp");

        config.wasi_capabilities = Some(capabilities);
        config.wasi_args = vec!["test_program".to_string(), "--flag".to_string()];
        config.wasi_env_vars = vec!["PATH".to_string()];

        let result = WrtdEngine::new(config);
        assert!(
            result.is_ok(),
            "Failed to create WrtdEngine with WASI Preview 1"
        );

        let engine = result.unwrap();
        let stats = engine.stats();

        // Should have registered WASI host functions
        assert!(
            stats.host_functions_registered > 0,
            "No WASI host functions registered"
        );
        assert!(
            engine.memory_profiler().is_some(),
            "Memory profiler not initialized"
        );
    }

    /// Test WASI Preview 2 with component model
    #[test]
    #[cfg(all(feature = "wasi", feature = "component-model"))]
    fn test_wasi_preview2_component_model() {
        let mut config = WrtdConfig::default();
        config.enable_wasi = true;
        config.wasi_version = WasiVersion::Preview2;
        config.enable_component_model = true;
        config.component_interfaces = vec!["wasi:filesystem".to_string(), "wasi:cli".to_string()];

        let result = WrtdEngine::new(config);
        assert!(
            result.is_ok(),
            "Failed to create WrtdEngine with WASI Preview 2 and component model"
        );

        let engine = result.unwrap();
        let stats = engine.stats();

        // Should have both WASI and component model initialized
        assert!(
            stats.host_functions_registered > 0,
            "No host functions registered"
        );
    }

    /// Test memory profiling integration
    #[test]
    fn test_memory_profiling() {
        let mut profiler = MemoryProfiler::new().unwrap();

        // Test allocation recording
        profiler.record_allocation(1024);
        assert_eq!(profiler.current_usage(), 1024);
        assert_eq!(profiler.peak_usage(), 1024);

        profiler.record_allocation(512);
        assert_eq!(profiler.current_usage(), 1536);
        assert_eq!(profiler.peak_usage(), 1536);

        // Test deallocation
        profiler.record_deallocation(512);
        assert_eq!(profiler.current_usage(), 1024);
        assert_eq!(profiler.peak_usage(), 1536); // Peak should remain

        // Test with engine configuration
        let mut config = WrtdConfig::default();
        config.enable_memory_profiling = true;

        let result = WrtdEngine::new(config);
        assert!(
            result.is_ok(),
            "Failed to create WrtdEngine with memory profiling"
        );

        let engine = result.unwrap();
        assert!(
            engine.memory_profiler().is_some(),
            "Memory profiler not enabled"
        );
    }

    /// Test platform optimizations
    #[test]
    fn test_platform_optimizations() {
        // Test with optimizations enabled (default)
        let mut config = WrtdConfig::default();
        config.enable_platform_optimizations = true;

        let result = WrtdEngine::new(config);
        assert!(
            result.is_ok(),
            "Failed to create WrtdEngine with platform optimizations"
        );

        // Test with optimizations disabled
        let mut config = WrtdConfig::default();
        config.enable_platform_optimizations = false;

        let result = WrtdEngine::new(config);
        assert!(
            result.is_ok(),
            "Failed to create WrtdEngine without platform optimizations"
        );
    }

    /// Test host function registry
    #[test]
    #[cfg(feature = "wasi")]
    fn test_host_function_registry() {
        let mut config = WrtdConfig::default();
        config.enable_wasi = true;

        // Create engine and check that host functions are registered
        let result = WrtdEngine::new(config);
        assert!(result.is_ok(), "Failed to create WrtdEngine");

        let engine = result.unwrap();
        let stats = engine.stats();

        // WASI should register multiple host functions
        assert!(
            stats.host_functions_registered >= 10,
            "Expected at least 10 WASI host functions, got {}",
            stats.host_functions_registered
        );
    }

    /// Test module execution simulation
    #[test]
    fn test_module_execution_simulation() {
        // Create a minimal valid WASM module
        let wasm_module = vec![
            0x00, 0x61, 0x73, 0x6D, // WASM magic
            0x01, 0x00, 0x00, 0x00, // Version 1
        ];

        let mut config = WrtdConfig::default();
        config.module_data = Some(&wasm_module);
        config.max_fuel = 1000;
        config.max_memory = 4096;

        let mut engine = WrtdEngine::new(config).unwrap();
        let result = engine.execute_module();

        assert!(result.is_ok(), "Module execution failed: {:?}", result);

        let stats = engine.stats();
        assert_eq!(
            stats.modules_executed, 1,
            "Module execution count incorrect"
        );
        assert!(stats.fuel_consumed > 0, "No fuel consumed");
    }

    /// Test component detection
    #[test]
    fn test_component_detection() {
        let mut config = WrtdConfig::default();

        let engine = WrtdEngine::new(config).unwrap();

        // Test traditional WASM module detection
        let wasm_module = vec![
            0x00, 0x61, 0x73, 0x6D, // WASM magic
            0x01, 0x00, 0x00, 0x00, // Version 1 (traditional module)
        ];

        let is_component = engine.detect_component_format(&wasm_module).unwrap();
        assert!(
            !is_component,
            "Traditional WASM module incorrectly detected as component"
        );

        // Test component detection (different version)
        let component_binary = vec![
            0x00, 0x61, 0x73, 0x6D, // WASM magic
            0x0A, 0x00, 0x01, 0x00, // Component version (different from 1)
        ];

        let is_component = engine.detect_component_format(&component_binary).unwrap();
        assert!(is_component, "Component binary not detected as component");

        // Test invalid binary
        let invalid_binary = vec![0x00, 0x00, 0x00, 0x00];
        let result = engine.detect_component_format(&invalid_binary);
        assert!(result.is_err(), "Invalid binary should fail detection");
    }

    /// Test comprehensive configuration
    #[test]
    #[cfg(all(feature = "wasi", feature = "component-model"))]
    fn test_comprehensive_configuration() {
        let mut config = WrtdConfig::default();

        // Enable all features
        config.enable_wasi = true;
        config.wasi_version = WasiVersion::Preview1;
        config.enable_component_model = true;
        config.enable_memory_profiling = true;
        config.enable_platform_optimizations = true;

        // Configure WASI
        config.wasi_args = vec!["test".to_string(), "arg1".to_string(), "arg2".to_string()];
        config.wasi_env_vars = vec!["HOME".to_string(), "PATH".to_string()];

        use wrt_wasi::WasiCapabilities;
        let mut capabilities = WasiCapabilities::minimal();
        capabilities.environment.args_access = true;
        capabilities.environment.environ_access = true;
        capabilities.environment.add_allowed_var("HOME");
        capabilities.environment.add_allowed_var("PATH");
        capabilities.filesystem.add_allowed_path("/tmp");
        capabilities.filesystem.add_allowed_path("/var/tmp");

        config.wasi_capabilities = Some(capabilities);

        // Configure component model
        config.component_interfaces = vec![
            "wasi:filesystem".to_string(),
            "wasi:cli".to_string(),
            "custom:interface".to_string(),
        ];

        // Configure limits
        config.max_fuel = 100_000;
        config.max_memory = 1024 * 1024; // 1MB

        let result = WrtdEngine::new(config);
        assert!(
            result.is_ok(),
            "Failed to create comprehensive WrtdEngine configuration"
        );

        let engine = result.unwrap();
        let stats = engine.stats();

        // Verify all features are active
        assert!(
            stats.host_functions_registered > 0,
            "No host functions registered"
        );
        assert!(
            engine.memory_profiler().is_some(),
            "Memory profiler not initialized"
        );
    }

    /// Test runtime statistics tracking
    #[test]
    fn test_runtime_statistics() {
        let stats = RuntimeStats::default();

        // Verify default values
        assert_eq!(stats.modules_executed, 0);
        assert_eq!(stats.components_executed, 0);
        assert_eq!(stats.fuel_consumed, 0);
        assert_eq!(stats.peak_memory, 0);
        assert_eq!(stats.wasi_functions_called, 0);
        assert_eq!(stats.host_functions_registered, 0);
        assert_eq!(stats.cross_component_calls, 0);
    }

    /// Test error handling for invalid configurations
    #[test]
    fn test_error_handling() {
        // Test invalid WASI configuration
        #[cfg(feature = "wasi")]
        {
            let mut config = WrtdConfig::default();
            config.enable_wasi = true;
            // Leave wasi_capabilities as None - should use minimal

            let result = WrtdEngine::new(config);
            assert!(
                result.is_ok(),
                "Engine creation should succeed with minimal WASI capabilities"
            );
        }

        // Test invalid module data
        let mut config = WrtdConfig::default();
        config.module_data = Some(&[0x00, 0x00]); // Too small

        let mut engine = WrtdEngine::new(config).unwrap();
        let result = engine.execute_module();
        assert!(result.is_err(), "Should fail with too-small module data");
    }

    /// Test no-std compatibility (structure only, no allocation)
    #[test]
    fn test_no_std_compatibility() {
        // Test that our types can be used in no_std context
        let config = WrtdConfig {
            max_fuel: 1000,
            max_memory: 4096,
            function_name: Some("_start"),
            module_data: None,
            #[cfg(feature = "std")]
            module_path: None,
            #[cfg(feature = "wasi")]
            enable_wasi: false,
            #[cfg(feature = "wasi")]
            wasi_version: WasiVersion::Preview1,
            #[cfg(feature = "wasi")]
            wasi_env_vars: Vec::new(),
            #[cfg(feature = "wasi")]
            wasi_args: Vec::new(),
            #[cfg(feature = "wasi")]
            wasi_capabilities: None,
            #[cfg(feature = "component-model")]
            enable_component_model: false,
            #[cfg(feature = "component-model")]
            component_interfaces: Vec::new(),
            enable_memory_profiling: false,
            enable_platform_optimizations: true,
        };

        // Should be able to create config without std allocations
        assert_eq!(config.max_fuel, 1000);
        assert_eq!(config.max_memory, 4096);
        assert!(config.function_name.is_some());
    }
}

/// Benchmark tests for performance validation
#[cfg(all(test, feature = "std"))]
mod benchmarks {
    use std::time::Instant;

    use super::super::{
        WrtdConfig,
        WrtdEngine,
    };

    /// Benchmark engine creation time
    #[test]
    fn benchmark_engine_creation() {
        let config = WrtdConfig::default();

        let start = Instant::now();
        let _engine = WrtdEngine::new(config).unwrap();
        let duration = start.elapsed();

        // Engine creation should be fast (< 100ms)
        assert!(
            duration.as_millis() < 100,
            "Engine creation took too long: {:?}",
            duration
        );
    }

    /// Benchmark WASI initialization time
    #[test]
    #[cfg(feature = "wasi")]
    fn benchmark_wasi_initialization() {
        let mut config = WrtdConfig::default();
        config.enable_wasi = true;

        let start = Instant::now();
        let _engine = WrtdEngine::new(config).unwrap();
        let duration = start.elapsed();

        // WASI initialization should be reasonable (< 500ms)
        assert!(
            duration.as_millis() < 500,
            "WASI initialization took too long: {:?}",
            duration
        );
    }
}
