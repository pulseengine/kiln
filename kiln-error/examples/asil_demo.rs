//! Example demonstrating ASIL-aware error handling
//!
//! This example shows how to use wrt-error with different ASIL safety levels.
//!
//! Run with different features to see different behaviors:
//! - `cargo run --example asil_demo --features asil-b`
//! - `cargo run --example asil_demo --features asil-c`
//! - `cargo run --example asil_demo --features asil-d`

use wrt_error::Error;
#[cfg(any(feature = "asil-c", feature = "asil-d"))]
use wrt_error::SafetyMonitor;
#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
use wrt_error::{AsilErrorContext, AsilLevel};

fn main() {
    println!("WRT Error ASIL Demo");
    println!("===================\n");

    // Show current ASIL level
    #[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
    {
        let current_level = AsilLevel::current();
        println!("Current ASIL Level: {}", current_level.name());
        println!();
    }

    #[cfg(not(any(feature = "asil-b", feature = "asil-c", feature = "asil-d")))]
    {
        println!("Current ASIL Level: QM (Quality Management)");
        println!();
    }

    // Demonstrate basic error creation
    demonstrate_basic_errors();

    // Demonstrate ASIL-specific features
    #[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
    demonstrate_asil_features();

    // Demonstrate safety monitoring (ASIL-C and above)
    #[cfg(any(feature = "asil-c", feature = "asil-d"))]
    demonstrate_safety_monitoring();

    // Demonstrate ASIL-D specific features
    #[cfg(feature = "asil-d")]
    demonstrate_asil_d_features();
}

fn demonstrate_basic_errors() {
    println!("Basic Error Creation");
    println!("-------------------");

    // Create various error types
    let memory_error = Error::memory_error("Memory allocation failed");
    let validation_error = Error::validation_error("Invalid input parameter");
    let runtime_error = Error::runtime_error("Execution failed");

    println!("Memory Error: {memory_error}");
    println!("Validation Error: {validation_error}");
    println!("Runtime Error: {runtime_error}");
    println!();
}

#[cfg(any(feature = "asil-b", feature = "asil-c", feature = "asil-d"))]
fn demonstrate_asil_features() {
    println!("ASIL-Aware Features");
    println!("------------------");

    // Create errors and check their ASIL levels
    let safety_error = Error::safety_violation("Critical safety violation detected");
    let memory_error = Error::runtime_execution_error("Memory allocation failed");
    let type_error = Error::type_mismatch_error("Type validation failed");

    println!(
        "Safety Error: {} [Level: {}]",
        safety_error,
        safety_error.asil_level()
    );
    println!(
        "Memory Error: {} [Level: {}]",
        memory_error,
        memory_error.asil_level()
    );
    println!(
        "Type Error: {} [Level: {}]",
        type_error,
        type_error.asil_level()
    );

    // Check safe state requirements
    #[cfg(any(feature = "asil-c", feature = "asil-d"))]
    {
        println!("\nSafe State Requirements:");
        println!(
            "Safety Error requires safe state: {}",
            safety_error.requires_safe_state()
        );
        println!(
            "Memory Error requires safe state: {}",
            memory_error.requires_safe_state()
        );
        println!(
            "Type Error requires safe state: {}",
            type_error.requires_safe_state()
        );
    }

    // Create error context
    let context = AsilErrorContext::new(safety_error)
        .with_timestamp(1_234_567_890)
        .with_module_id(42);

    println!("\nError Context:");
    println!("- ASIL Level: {}", context.asil_level.name());
    println!("- Timestamp: {:?}", context.timestamp);
    println!("- Module ID: {:?}", context.module_id);
    println!(
        "- Requires Immediate Action: {}",
        context.requires_immediate_action()
    );
    println!();
}

#[cfg(any(feature = "asil-c", feature = "asil-d"))]
fn demonstrate_safety_monitoring() {
    println!("Safety Monitoring (ASIL-C+)");
    println!("--------------------------");

    let monitor = SafetyMonitor::new();

    // Simulate some errors
    let errors = [
        Error::memory_error("Out of bounds access"),
        Error::validation_error("Invalid parameter"),
        Error::safety_violation("Safety check failed"),
        Error::runtime_error("Execution timeout"),
    ];

    // Record errors
    for (i, error) in errors.iter().enumerate() {
        monitor.record_error(error);
        println!("Recorded error {}: {}", i + 1, error);
    }

    println!("\nTotal errors recorded: {}", monitor.error_count());

    // Reset monitor
    monitor.reset();
    println!("Monitor reset. Error count: {}", monitor.error_count());
    println!();
}

#[cfg(feature = "asil-d")]
fn demonstrate_asil_d_features() {
    use wrt_error::validate_error_consistency;

    println!("ASIL-D Specific Features");
    println!("-----------------------");

    // Demonstrate error integrity validation
    let valid_error =
        Error::runtime_execution_error("Memory allocation failed during runtime execution");
    let valid_safety = Error::safety_violation("Safety constraint violated in critical path");

    println!("Error Integrity Validation:");
    println!(
        "Memory Error: {} - Valid: {}",
        valid_error,
        valid_error.validate_integrity()
    );
    println!(
        "Safety Error: {} - Valid: {}",
        valid_safety,
        valid_safety.validate_integrity()
    );

    // Demonstrate determinism and redundancy errors
    let det_error = Error::safety_violation("Non-deterministic behavior detected");
    let red_error = Error::safety_violation("Redundancy check failed");

    println!("\nASIL-D Specific Errors:");
    println!("Determinism Error: {det_error}");
    println!("Redundancy Error: {red_error}");

    // Validate consistency using standalone function
    println!("\nStandalone Consistency Validation:");
    println!(
        "Memory Error consistent: {}",
        validate_error_consistency(&valid_error)
    );
    println!(
        "Safety Error consistent: {}",
        validate_error_consistency(&valid_safety)
    );

    println!();
}

// Helper function to demonstrate error propagation
#[allow(dead_code)]
#[allow(clippy::unnecessary_wraps)]
fn process_with_asil_checks() -> Result<(), Error> {
    // This would use the asil_assert! macro in real code
    #[cfg(feature = "asil-b")]
    {
        // ASIL-B: Return error on failure
        let condition = false;
        if !condition {
            return Err(Error::validation_error("ASIL-B validation failed"));
        }
    }

    #[cfg(any(feature = "asil-c", feature = "asil-d"))]
    {
        // ASIL-C/D: Would panic on failure in real implementation
        let condition = true;
        if !condition {
            return Err(Error::safety_violation("ASIL-C/D safety check failed"));
        }
    }

    Ok(())
}
