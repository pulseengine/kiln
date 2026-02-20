//! Demonstration of safety feature enforcement in wrt-wasi


fn main() {
    #[cfg(feature = "qm")]
    println!("Running with QM feature - No safety limits");

    #[cfg(feature = "asil-d")]
    println!("Running with ASIL-D feature - Maximum safety (16KB limit)");

    #[cfg(feature = "asil-c")]
    println!("Running with ASIL-C feature - Static memory safety (32KB limit)");

    #[cfg(feature = "asil-b")]
    println!("Running with ASIL-B feature - Bounded collections (64KB limit)");

    #[cfg(feature = "asil-a")]
    println!("Running with ASIL-A feature - Bounded collections (64KB limit)");

    // Show which features are active
    println!("\nActive safety features:");
    #[cfg(feature = "wrt-foundation/dynamic-allocation")]
    println!("  ✓ dynamic-allocation");
    #[cfg(feature = "wrt-foundation/bounded-collections")]
    println!("  ✓ bounded-collections");
    #[cfg(feature = "wrt-foundation/static-memory-safety")]
    println!("  ✓ static-memory-safety");
    #[cfg(feature = "wrt-foundation/maximum-safety")]
    println!("  ✓ maximum-safety");

    // Show capability features
    println!("\nActive capability features:");
    #[cfg(feature = "wrt-foundation/compile-time-capacity-limits")]
    println!("  ✓ compile-time-capacity-limits");
    #[cfg(feature = "wrt-foundation/runtime-bounds-checking")]
    println!("  ✓ runtime-bounds-checking");
    #[cfg(feature = "wrt-foundation/memory-budget-enforcement")]
    println!("  ✓ memory-budget-enforcement");
    #[cfg(feature = "wrt-foundation/verified-static-allocation")]
    println!("  ✓ verified-static-allocation");

    println!("\nSafety features are enforced through:");
    println!("1. Compile-time checks for allocation sizes");
    println!("2. Runtime budget enforcement");
    println!("3. Bounded collections with fixed capacity");
    println!("4. Safety-aware allocation macros");
}
