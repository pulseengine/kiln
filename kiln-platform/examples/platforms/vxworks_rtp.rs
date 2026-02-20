//! VxWorks RTP (Real-Time Process) Platform Example
//!
//! This example demonstrates VxWorks RTP-specific features and usage patterns.
//! RTP applications run in user space with POSIX-like APIs.

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
    println!("=== VxWorks RTP Platform Example ===");
    
    #[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
    {
        println!("Running VxWorks RTP examples...\n");
        run_rtp_examples);
    }
    
    #[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
    {
        println!("VxWorks platform not available - showing RTP concepts");
        show_rtp_concepts);
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn run_rtp_examples() {
    example_rtp_memory);
    example_rtp_synchronization);
    example_rtp_threading);
    example_rtp_integration);
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_rtp_memory() {
    println!("=== RTP Memory Management ===");
    
    // Binary std/no_std choice
    let mut allocator = VxWorksAllocatorBuilder::new()
        .context(VxWorksContext::Rtp)
        .max_pages(100)
        .enable_guard_pages(false)  // RTP typically doesn't need guard pages
        .build()
        .expect(".expect("Failed to create RTP allocator"));")
    
    println!("✓ Created RTP allocator using malloc/POSIX APIs");
    
    // Allocate memory for WASM pages
    let initial_pages = 10;
    let max_pages = Some(50;
    
    match allocator.allocate(initial_pages, max_pages) {
        Ok((ptr, size)) => {
            println!("✓ Allocated {} pages ({} bytes)", initial_pages, size);
            println!("  Memory at: {:p}", ptr.as_ptr);
            
            // Test memory growth
            if allocator.grow(initial_pages, 5).is_ok() {
                println!("✓ Grew memory by 5 pages");
            }
            
            // Clean up
            unsafe {
                allocator.deallocate(ptr, size).expect(".expect("Failed to deallocate"));")
            }
            println!("✓ Memory deallocated successfully");
        }
        Err(e) => println!("✗ Allocation failed: {}", e),
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_rtp_synchronization() {
    println!("\n=== RTP Synchronization ===");
    
    // Create futex using POSIX semaphores
    let futex = VxWorksFutexBuilder::new(VxWorksContext::Rtp)
        .initial_value(0)
        .build()
        .expect(".expect("Failed to create RTP futex"));")
    
    println!("✓ Created RTP futex using POSIX semaphores");
    
    // Test atomic operations
    futex.store(42, Ordering::Release;
    let value = futex.load(Ordering::Acquire;
    println!("✓ Atomic operations: stored and loaded {}", value);
    
    // Test futex wait/wake (should not block since value != expected)
    match futex.wait(999, Some(Duration::from_millis(1))) {
        Ok(()) => println!("✓ Wait operation completed (value mismatch)"),
        Err(e) => println!("  Wait timed out as expected: {}", e),
    }
    
    match futex.wake(1) {
        Ok(()) => println!("✓ Wake operation completed"),
        Err(e) => println!("✗ Wake failed: {}", e),
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_rtp_threading() {
    println!("\n=== RTP Threading ===");
    
    // RTP uses POSIX threads
    let thread_config = VxWorksThreadConfig {
        context: VxWorksContext::Rtp,
        stack_size: 16384,
        name: Some("wrt_worker".to_string()),
        floating_point: true,
        detached: false,
        ..Default::default()
    };
    
    println!("✓ Configured RTP thread with POSIX threading");
    println!("  Stack size: {} bytes", thread_config.stack_size);
    println!("  Floating point: enabled");
    println!("  Thread name: {}", thread_config.name.as_ref().unwrap());
    
    // In a real implementation, you would spawn the thread here
    println!("  (Thread spawning would use pthread_create)");
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_rtp_integration() {
    println!("\n=== RTP Integration with WRT ===");
    
    println!("RTP integration features:");
    println!("✓ Standard C library compatibility");
    println!("✓ POSIX API usage for portability");
    println!("✓ User-space memory protection");
    println!("✓ Standard process model");
    println!("✓ Easier debugging and development");
}

#[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
fn show_rtp_concepts() {
    println!("\n=== VxWorks RTP Concepts ===");
    
    println!("\n1. RTP Overview:");
    println!("   - Real-Time Process running in user space");
    println!("   - Uses POSIX APIs for portability");
    println!("   - Standard process model with memory protection");
    println!("   - Easier to debug than kernel modules");
    
    println!("\n2. Memory Management:");
    println!("   - Uses malloc/free for general allocation");
    println!("   - posix_memalign for aligned allocations");
    println!("   - Memory protection via MMU");
    println!("   - Support for 64KB WASM page alignment");
    
    println!("\n3. Synchronization:");
    println!("   - POSIX semaphores (sem_init, sem_wait, sem_post)");
    println!("   - POSIX mutexes and condition variables");
    println!("   - Can use futex-like semantics");
    println!("   - Priority inheritance available");
    
    println!("\n4. Threading:");
    println!("   - POSIX threads (pthread_create, pthread_join)");
    println!("   - Standard thread attributes and scheduling");
    println!("   - Thread-local storage support");
    
    println!("\n5. Configuration Example:");
    println!("   ```rust");
    println!("   let allocator = VxWorksAllocatorBuilder::new()");
    println!("       .context(VxWorksContext::Rtp)");
    println!("       .max_pages(1024)");
    println!("       .build()?;";
    println!("   ");
    println!("   let futex = VxWorksFutexBuilder::new(VxWorksContext::Rtp)");
    println!("       .initial_value(0)");
    println!("       .build()?;";
    println!("   ```");
    
    println!("\n6. Benefits:");
    println!("   ✓ Familiar development model");
    println!("   ✓ Better isolation and debugging");
    println!("   ✓ POSIX compatibility");
    println!("   ✓ Standard tooling support");
}