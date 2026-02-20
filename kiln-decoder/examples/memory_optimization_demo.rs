//! Demonstration of memory optimization features in wrt-decoder
//!
//! This example shows how the memory optimizations reduce allocation overhead
//! and provide bounds checking to prevent malicious over-allocation.

#[cfg(feature = "std")]
fn main() {
    use wrt_decoder::memory_optimized::{MemoryPool, check_bounds_u32, safe_usize_conversion};
    use wrt_foundation::NoStdProvider;

    println!("=== WRT-Decoder Memory Optimization Demo ===\n");

    // 1. Demonstrate bounds checking protection
    println!("1. Bounds Checking Protection:");

    // Simulate parsing a section header with a reasonable count
    let reasonable_count = 1000u32;
    let max_allowed = 10000u32;

    match check_bounds_u32(reasonable_count, max_allowed, "function count") {
        Ok(()) => println!("✓ Reasonable count {} accepted", reasonable_count),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Simulate a malicious WebAssembly file with excessive count
    let malicious_count = u32::MAX;
    match check_bounds_u32(malicious_count, max_allowed, "function count") {
        Ok(()) => println!(
            "✗ Malicious count {} incorrectly accepted!",
            malicious_count
        ),
        Err(e) => println!(
            "✓ Malicious count {} properly rejected: {}",
            malicious_count, e
        ),
    }

    // 2. Demonstrate safe usize conversion
    println!("\n2. Safe usize Conversion:");

    match safe_usize_conversion(reasonable_count, "allocation size") {
        Ok(size) => println!(
            "✓ Successfully converted {} to usize: {}",
            reasonable_count, size
        ),
        Err(e) => println!("✗ Conversion failed: {}", e),
    }

    // 3. Demonstrate memory pool efficiency
    println!("\n3. Memory Pool Efficiency:");

    let provider = NoStdProvider::<4096>::default();
    let mut pool = MemoryPool::new(provider);

    // Simulate parsing multiple functions - reusing vectors
    println!("Parsing 5 functions with vector reuse:");
    for i in 1..=5 {
        let mut instruction_vec = pool.get_instruction_vector();

        // Simulate adding some instructions
        for j in 0..10 {
            instruction_vec.push((i * 10 + j) as u8);
        }

        println!(
            "  Function {}: processed {} instructions",
            i,
            instruction_vec.len()
        );

        // Return vector to pool for reuse
        pool.return_instruction_vector(instruction_vec);
    }

    println!("✓ All vectors returned to pool for reuse");

    // Binary std/no_std choice
    println!("\n4. Conservative Allocation Strategy:");

    let declared_count = 1000000u32; // 1M items claimed
    let max_conservative = 1024usize; // Conservative limit

    if let Ok(()) = check_bounds_u32(declared_count, 2000000, "items") {
        let safe_count = safe_usize_conversion(declared_count, "items").unwrap();
        let allocated_count = safe_count.min(max_conservative);

        println!("  Declared count: {}", declared_count);
        println!("  Conservative allocation: {}", allocated_count);
        println!(
            "  Memory saved: {}x reduction",
            declared_count / allocated_count as u32
        );
    }

    println!("\n=== Memory Optimization Benefits ===");
    println!("✓ Prevents allocation attacks through bounds checking");
    println!("✓ Reduces memory fragmentation through vector pooling");
    println!("✓ Uses conservative allocation strategies");
    println!("✓ Provides safe integer conversions");
    println!("✓ Works across std, no_std+alloc, and pure no_std environments");

    println!("\nDemo completed successfully! 🎉");
}

#[cfg(not(feature = "std"))]
fn main() {
    println!("This demo requires the 'alloc' feature to be enabled.");
    println!("Run with: cargo run --example memory_optimization_demo --features alloc");
}
