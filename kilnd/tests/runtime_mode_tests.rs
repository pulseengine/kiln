//! Comprehensive tests for different runtime modes in wrtd
//!
//! This test suite validates that wrtd correctly handles different runtime
//! modes (std, alloc, no_std) and their respective capabilities and
//! limitations.

#[cfg(test)]
mod tests {
    use std::{
        env,
        path::PathBuf,
        process::Command,
    };

    /// Helper function to run wrtd with specified arguments
    fn run_wrtd_with_mode(
        wasm_file: &str,
        runtime_mode: &str,
        call: Option<&str>,
        fuel: Option<u64>,
        extra_args: &[&str],
    ) -> (bool, String, String) {
        let project_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .parent()
            .unwrap()
            .to_path_buf();

        let wrtd_path = project_root.join("target/debug/wrtd");

        let mut cmd = Command::new(wrtd_path);
        cmd.arg(wasm_file).arg("--runtime-mode").arg(runtime_mode);

        if let Some(function_name) = call {
            cmd.arg("--call").arg(function_name);
        }

        if let Some(fuel_amount) = fuel {
            cmd.arg("--fuel").arg(fuel_amount.to_string());
        }

        // Add extra arguments
        for arg in extra_args {
            cmd.arg(arg);
        }

        let output = cmd.output().expect("Failed to execute wrtd");
        let success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        (success, stdout, stderr)
    }

    /// Test std runtime mode capabilities
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_std_runtime_mode() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/std-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Test basic std functionality
        let (success, stdout, stderr) = run_wrtd_with_mode(
            &test_wasm,
            "std",
            Some("hello"),
            Some(1000000),
            &["--stats"],
        );

        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);

        assert!(success, "std mode execution should succeed");
        assert!(stdout.contains("Runtime mode: Std"));
    }

    /// Binary std/no_std choice
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_alloc_runtime_mode() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/alloc-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Binary std/no_std choice
        let (success, stdout, stderr) = run_wrtd_with_mode(
            &test_wasm,
            "alloc",
            Some("dynamic_array"),
            Some(100000),
            &["--stats", "--validate-mode"],
        );

        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);

        assert!(success, "alloc mode execution should succeed");
        assert!(stdout.contains("Runtime mode: Alloc"));
        assert!(stdout.contains("Configuration validated"));
    }

    /// Test no_std runtime mode with minimal functionality
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_nostd_runtime_mode() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/nostd-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Test basic arithmetic
        let (success, stdout, stderr) = run_wrtd_with_mode(
            &test_wasm,
            "no-std",
            Some("add"),
            Some(10000),
            &["--stats", "--validate-mode"],
        );

        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);

        assert!(success, "no_std mode execution should succeed");
        assert!(stdout.contains("Runtime mode: NoStd"));
        assert!(stdout.contains("Configuration validated"));
    }

    /// Test fibonacci calculation in no_std mode
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_nostd_fibonacci() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/nostd-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        let (success, stdout, stderr) = run_wrtd_with_mode(
            &test_wasm,
            "no-std",
            Some("fibonacci"),
            Some(50000),
            &["--stats"],
        );

        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);

        assert!(
            success,
            "fibonacci calculation should succeed in no_std mode"
        );
    }

    /// Test runtime mode validation catches incompatible configurations
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_mode_validation_limits() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/nostd-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Try to use excessive fuel with no_std mode (should fail validation)
        let (success, stdout, stderr) = run_wrtd_with_mode(
            &test_wasm,
            "no-std",
            Some("add"),
            Some(1000000), // Exceeds no_std limit of 100,000
            &["--validate-mode"],
        );

        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);

        // Should fail due to fuel limit validation
        assert!(
            !success,
            "Should fail validation with excessive fuel for no_std mode"
        );
        assert!(stderr.contains("exceeds maximum") || stderr.contains("Fuel limit"));
    }

    /// Test buffer size validation for different modes
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_buffer_size_validation() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/nostd-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Try to use large buffer with no_std mode (should fail validation)
        let (success, stdout, stderr) = run_wrtd_with_mode(
            &test_wasm,
            "no-std",
            Some("add"),
            Some(10000),
            &["--validate-mode", "--buffer-size", "2000000"], // 2MB > 1MB limit
        );

        println!("STDOUT: {}", stdout);
        println!("STDERR: {}", stderr);

        // Should fail due to buffer size validation
        assert!(
            !success,
            "Should fail validation with excessive buffer size for no_std mode"
        );
        assert!(stderr.contains("exceeds maximum") || stderr.contains("Buffer size"));
    }

    /// Test capability display for different modes
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_show_capabilities() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/std-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Test std mode capabilities
        let (success, stdout, stderr) =
            run_wrtd_with_mode(&test_wasm, "std", None, None, &["--show-capabilities"]);

        println!("STD Capabilities STDOUT: {}", stdout);
        println!("STD Capabilities STDERR: {}", stderr);

        assert!(success, "Showing std capabilities should succeed");
        assert!(stdout.contains("Runtime Capabilities for Std Mode"));
        assert!(stdout.contains("Standard library:     ✅ Yes"));
        assert!(stdout.contains("Heap allocation:      ✅ Yes"));
        assert!(stdout.contains("WASI support:         ✅ Yes"));

        // Test no_std mode capabilities
        let (success, stdout, stderr) =
            run_wrtd_with_mode(&test_wasm, "no-std", None, None, &["--show-capabilities"]);

        println!("NoStd Capabilities STDOUT: {}", stdout);
        println!("NoStd Capabilities STDERR: {}", stderr);

        assert!(success, "Showing no_std capabilities should succeed");
        assert!(stdout.contains("Runtime Capabilities for NoStd Mode"));
        assert!(stdout.contains("Standard library:     ❌ No"));
        assert!(stdout.contains("Heap allocation:      ❌ No"));
        assert!(stdout.contains("WASI support:         ❌ No"));
        assert!(stdout.contains("Maximum memory:       1048576 bytes"));
    }

    /// Test performance comparison between modes
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_performance_comparison() {
        // Test the same computation in different modes to compare performance
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/nostd-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Test std mode
        let (success_std, stdout_std, _) = run_wrtd_with_mode(
            &test_wasm,
            "std",
            Some("fibonacci"),
            Some(1000000),
            &["--stats"],
        );

        // Test no_std mode
        let (success_nostd, stdout_nostd, _) = run_wrtd_with_mode(
            &test_wasm,
            "no-std",
            Some("fibonacci"),
            Some(50000), // Lower fuel for no_std
            &["--stats"],
        );

        assert!(success_std, "std mode fibonacci should succeed");
        assert!(success_nostd, "no_std mode fibonacci should succeed");

        // Both should produce statistics
        assert!(stdout_std.contains("Execution Statistics") || stdout_std.contains("executed"));
        assert!(stdout_nostd.contains("Execution Statistics") || stdout_nostd.contains("executed"));
    }

    /// Test memory strategy compatibility with different runtime modes
    #[test]
    #[ignore = "Requires compilation fixes in core WRT crates"]
    fn test_memory_strategy_compatibility() {
        let test_wasm = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("tests/fixtures/alloc-mode-example.wasm")
            .to_str()
            .unwrap()
            .to_string();

        // Binary std/no_std choice
        let strategies = ["zero-copy", "bounded-copy", "full-isolation"];

        for strategy in &strategies {
            let (success, stdout, stderr) = run_wrtd_with_mode(
                &test_wasm,
                "alloc",
                Some("memory_test"),
                Some(100000),
                &["--memory-strategy", strategy, "--validate-mode"],
            );

            println!("Strategy {} STDOUT: {}", strategy, stdout);
            println!("Strategy {} STDERR: {}", strategy, stderr);

            assert!(
                success,
                "Memory strategy {} should work with alloc mode",
                strategy
            );
            assert!(stdout.contains("Runtime mode: Alloc"));
        }
    }
}
