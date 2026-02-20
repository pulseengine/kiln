//! VxWorks LKM (Loadable Kernel Module) Platform Example
//!
//! This example demonstrates VxWorks LKM-specific features and usage patterns.
//! LKM components run in kernel space with direct access to VxWorks kernel APIs.

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
use wrt_platform::{
    vxworks_memory::{VxWorksAllocator, VxWorksAllocatorBuilder, VxWorksContext},
    vxworks_sync::{VxWorksFutex, VxWorksFutexBuilder},
    vxworks_threading::{VxWorksThreadBuilder, VxWorksThreadConfig},
};

use wrt_platform::{PageAllocator, FutexLike, WASM_PAGE_SIZE};
use core::sync::atomic::Ordering;
use core::time::Duration;

fn main() {
    println!("=== VxWorks LKM Platform Example ===");
    
    #[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
    {
        println!("Running VxWorks LKM examples...\n");
        run_lkm_examples);
    }
    
    #[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
    {
        println!("VxWorks platform not available - showing LKM concepts");
        show_lkm_concepts);
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn run_lkm_examples() {
    example_lkm_memory);
    example_lkm_synchronization);
    example_lkm_threading);
    example_lkm_integration);
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_lkm_memory() {
    println!("=== LKM Memory Management ===");
    
    // Binary std/no_std choice
    let mut allocator = VxWorksAllocatorBuilder::new()
        .context(VxWorksContext::Lkm)
        .max_pages(50)
        .use_dedicated_partition(true)  // LKM benefits from dedicated partitions
        .enable_guard_pages(true)       // Important for kernel space safety
        .build()
        .expect(".expect("Failed to create LKM allocator"));")
    
    println!("✓ Created LKM allocator using memory partitions");
    println!("  Features: dedicated partition, guard pages enabled");
    
    // Allocate memory using VxWorks partition APIs
    let initial_pages = 5;
    let max_pages = Some(25;
    
    match allocator.allocate(initial_pages, max_pages) {
        Ok((ptr, size)) => {
            println!("✓ Allocated {} pages ({} bytes) from partition", initial_pages, size);
            println!("  Kernel memory at: {:p}", ptr.as_ptr);
            
            // Test controlled memory growth
            if allocator.grow(initial_pages, 3).is_ok() {
                println!("✓ Grew partition memory by 3 pages");
            }
            
            // Clean up partition memory
            unsafe {
                allocator.deallocate(ptr, size).expect(".expect("Failed to deallocate partition memory"));")
            }
            println!("✓ Partition memory deallocated successfully");
        }
        Err(e) => println!("✗ Partition allocation failed: {}", e),
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_lkm_synchronization() {
    println!("\n=== LKM Synchronization ===");
    
    // Create futex using VxWorks binary semaphores
    let futex = VxWorksFutexBuilder::new(VxWorksContext::Lkm)
        .initial_value(1)
        .build()
        .expect(".expect("Failed to create LKM futex"));")
    
    println!("✓ Created LKM futex using VxWorks binary semaphores");
    println!("  Features: priority inheritance, kernel-space synchronization");
    
    // Test atomic operations in kernel space
    futex.store(42, Ordering::Release;
    let value = futex.load(Ordering::Acquire;
    println!("✓ Kernel atomic operations: stored and loaded {}", value);
    
    // Test kernel-space wait/wake
    match futex.wait(999, Some(Duration::from_millis(1))) {
        Ok(()) => println!("✓ Kernel wait operation completed (value mismatch)"),
        Err(e) => println!("  Kernel wait timed out as expected: {}", e),
    }
    
    match futex.wake(1) {
        Ok(()) => println!("✓ Kernel wake operation completed"),
        Err(e) => println!("✗ Kernel wake failed: {}", e),
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_lkm_threading() {
    println!("\n=== LKM Threading ===");
    
    // LKM uses VxWorks tasks
    let thread_config = VxWorksThreadConfig {
        context: VxWorksContext::Lkm,
        stack_size: 32768,  // Larger stack for kernel tasks
        name: Some("wrt_kernel_task".to_string()),
        floating_point: true,
        detached: true,
        priority: Some(100), // Real-time priority
        ..Default::default()
    };
    
    println!("✓ Configured LKM task with VxWorks kernel APIs");
    println!("  Stack size: {} bytes (kernel space)", thread_config.stack_size);
    println!("  Priority: {} (real-time)", thread_config.priority.unwrap());
    println!("  Task name: {}", thread_config.name.as_ref().unwrap());
    println!("  Floating point: enabled for kernel task");
    
    // In a real implementation, you would spawn the task here
    println!("  (Task spawning would use taskSpawn)");
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_lkm_integration() {
    println!("\n=== LKM Integration with WRT ===");
    
    println!("LKM integration features:");
    println!("✓ Direct kernel API access");
    println!("✓ Deterministic memory allocation");
    println!("✓ Priority inheritance synchronization");
    println!("✓ Real-time task scheduling");
    println!("✓ Hardware-level memory protection");
    println!("✓ Minimal overhead for real-time operations");
    
    println!("\nSafety considerations:");
    println!("⚠ Kernel space requires careful error handling");
    println!("⚠ Memory leaks affect entire system");
    println!("⚠ Synchronization bugs can cause system deadlock");
    println!("⚠ Stack overflow protection critical");
}

#[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
fn show_lkm_concepts() {
    println!("\n=== VxWorks LKM Concepts ===");
    
    println!("\n1. LKM Overview:");
    println!("   - Loadable Kernel Module running in kernel space");
    println!("   - Direct access to VxWorks kernel APIs");
    println!("   - Higher performance but requires more care");
    println!("   - Used for real-time critical components");
    
    println!("\n2. Memory Management:");
    println!("   - Memory partitions (memPartAlloc/memPartFree)");
    println!("   - Deterministic allocation patterns");
    println!("   - Guard pages for safety");
    println!("   - Support for 64KB WASM page alignment");
    
    println!("\n3. Synchronization:");
    println!("   - VxWorks binary semaphores (semBCreate, semTake, semGive)");
    println!("   - Counting semaphores and mutexes");
    println!("   - Priority inheritance built-in");
    println!("   - Interrupt-safe operations");
    
    println!("\n4. Threading:");
    println!("   - VxWorks tasks (taskSpawn, taskDelete)");
    println!("   - Real-time priority scheduling");
    println!("   - Preemptive multitasking");
    println!("   - Task-specific options and attributes");
    
    println!("\n5. Configuration Example:");
    println!("   ```rust");
    println!("   let allocator = VxWorksAllocatorBuilder::new()");
    println!("       .context(VxWorksContext::Lkm)");
    println!("       .max_pages(512)");
    println!("       .use_dedicated_partition(true)");
    println!("       .enable_guard_pages(true)");
    println!("       .build()?;";
    println!("   ");
    println!("   let futex = VxWorksFutexBuilder::new(VxWorksContext::Lkm)");
    println!("       .initial_value(1)");
    println!("       .build()?;";
    println!("   ```");
    
    println!("\n6. Benefits:");
    println!("   ✓ Maximum performance");
    println!("   ✓ Deterministic behavior");
    println!("   ✓ Real-time guarantees");
    println!("   ✓ Direct hardware access");
    
    println!("\n7. Considerations:");
    println!("   ⚠ Requires careful error handling");
    println!("   ⚠ Debugging is more complex");
    println!("   ⚠ System-wide impact of bugs");
    println!("   ⚠ Memory safety critical");
}