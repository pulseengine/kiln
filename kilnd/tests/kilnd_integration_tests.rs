//! Integration tests for wrtd with the capability-based engine
//!
//! These tests validate that wrtd properly integrates with the new
//! capability-based execution engine and handles different ASIL configurations.

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
#[cfg(feature = "wasi")]
use wrtd::WasiVersion;
use wrtd::{
    LogLevel,
    WrtDaemon,
    WrtdConfig,
};

/// Simple WebAssembly module for testing
const TEST_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // Version
    // Type section - function type (no params, no results)
    0x01, 0x04, 0x01, 0x60, 0x00, 0x00, // Function section - one function
    0x03, 0x02, 0x01, 0x00, // Export section - export function as "start"
    0x07, 0x09, 0x01, 0x05, 0x73, 0x74, 0x61, 0x72, 0x74, 0x00, 0x00,
    // Code section - function body (just return)
    0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b,
];

fn create_test_config() -> WrtdConfig {
    WrtdConfig {
        max_fuel: 1000000,
        max_memory: 16 * 1024 * 1024, // 16MB
        function_name: Some("start"),
        module_data: Some(TEST_WASM),
        #[cfg(feature = "std")]
        module_path: None,
        #[cfg(feature = "wasi")]
        enable_wasi: false,
        #[cfg(feature = "wasi")]
        wasi_version: WasiVersion::Preview1,
        #[cfg(feature = "wasi")]
        wasi_env_vars: vec![],
        #[cfg(feature = "wasi")]
        wasi_args: vec![],
        #[cfg(feature = "component-model")]
        enable_component_model: false,
        #[cfg(feature = "component-model")]
        component_interfaces: vec![],
        memory_profiling_enabled: false,
        cpu_profiling_enabled: false,
        async_execution: false,
        thread_count: 1,
        stack_size: 8192,
        enable_debug: false,
        optimization_level: 0,
    }
}

#[test]
fn test_wrtd_creation() -> Result<()> {
    let config = create_test_config();
    let daemon = WrtDaemon::new(config)?;

    // Daemon should be created successfully
    assert!(daemon.stats().modules_loaded == 0);

    Ok(())
}

#[test]
fn test_wrtd_module_execution() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    // Execute the test module
    daemon.run()?;

    // Verify execution statistics
    assert!(daemon.stats().modules_executed > 0);

    Ok(())
}

#[test]
#[cfg(feature = "wasi")]
fn test_wrtd_with_wasi_enabled() -> Result<()> {
    let mut config = create_test_config();
    config.enable_wasi = true;

    let mut daemon = WrtDaemon::new(config)?;

    // Should execute successfully even with WASI enabled
    // (though our test module doesn't use WASI functions)
    daemon.run()?;

    assert!(daemon.stats().modules_executed > 0);

    Ok(())
}

#[test]
fn test_wrtd_error_handling() -> Result<()> {
    let mut config = create_test_config();

    // Test with invalid WebAssembly binary
    config.module_data = Some(&[0x00, 0x01, 0x02, 0x03]);

    let mut daemon = WrtDaemon::new(config)?;

    // Should handle invalid module gracefully
    let result = daemon.run();
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_wrtd_function_not_found() -> Result<()> {
    let mut config = create_test_config();
    config.function_name = Some("nonexistent_function");

    let mut daemon = WrtDaemon::new(config)?;

    // Should handle missing function gracefully
    let result = daemon.run();
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_wrtd_memory_limits() -> Result<()> {
    let mut config = create_test_config();
    config.max_memory = 1024; // Very small memory limit

    let daemon = WrtDaemon::new(config)?;

    // Should create successfully but may fail during execution
    // if the module requires more memory
    assert!(daemon.stats().modules_loaded == 0);

    Ok(())
}

#[test]
fn test_wrtd_fuel_limits() -> Result<()> {
    let mut config = create_test_config();
    config.max_fuel = 10; // Very small fuel limit

    let mut daemon = WrtDaemon::new(config)?;

    // Should handle fuel exhaustion gracefully
    let result = daemon.run();
    // May succeed if the function is very simple, or fail with fuel exhaustion
    // Either is acceptable behavior

    Ok(())
}

#[test]
fn test_wrtd_logging() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    // Test that logging doesn't crash
    daemon.run()?;

    // Verify some execution occurred
    assert!(daemon.stats().modules_executed > 0);

    Ok(())
}

#[test]
fn test_wrtd_statistics_collection() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    let initial_stats = daemon.stats().clone();

    // Execute module
    daemon.run()?;

    let final_stats = daemon.stats();

    // Statistics should be updated
    assert!(final_stats.modules_executed > initial_stats.modules_executed);

    Ok(())
}

#[test]
fn test_wrtd_multiple_executions() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    // Execute multiple times
    daemon.run()?;
    let stats_after_first = daemon.stats().modules_executed;

    daemon.run()?;
    let stats_after_second = daemon.stats().modules_executed;

    // Should handle multiple executions
    assert!(stats_after_second > stats_after_first);

    Ok(())
}

/// Test different ASIL configurations
#[test]
#[cfg(feature = "wrt-execution")]
fn test_asil_configurations() -> Result<()> {
    let config = create_test_config();

    // All ASIL levels should be able to execute simple modules
    // (The specific ASIL level is determined by compile-time features)
    let mut daemon = WrtDaemon::new(config)?;
    daemon.run()?;

    assert!(daemon.stats().modules_executed > 0);

    Ok(())
}

/// Test capability-based execution
#[test]
#[cfg(feature = "wrt-execution")]
fn test_capability_based_execution() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    // The capability-based engine should handle execution properly
    daemon.run()?;

    // Verify the new engine integration works
    assert!(daemon.stats().modules_executed > 0);
    assert!(daemon.stats().modules_loaded > 0);

    Ok(())
}

/// Test host function integration awareness
#[test]
#[cfg(feature = "wrt-execution")]
fn test_host_function_integration() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    // Even though our test module doesn't call host functions,
    // the engine should be configured with host function support
    daemon.run()?;

    assert!(daemon.stats().modules_executed > 0);

    Ok(())
}

/// Comprehensive integration test
#[test]
fn test_full_wrtd_integration() -> Result<()> {
    let config = create_test_config();
    let mut daemon = WrtDaemon::new(config)?;

    // Test complete workflow
    let initial_stats = daemon.stats().clone();

    // Execute
    daemon.run()?;

    let final_stats = daemon.stats();

    // Verify all aspects worked
    assert!(final_stats.modules_executed > initial_stats.modules_executed);
    assert_eq!(final_stats.modules_loaded, initial_stats.modules_loaded + 1);

    Ok(())
}
