//! Portable VxWorks Platform Usage Example
//!
//! This example demonstrates VxWorks platform usage and compiles on all platforms,
//! showing conditional compilation patterns for platform-specific code.
//! 
//! This is part of the platform-specific examples that show how external developers
//! can implement and use platform extensions with WRT.

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
use wrt_platform::{
    vxworks_memory::{VxWorksAllocator, VxWorksAllocatorBuilder, VxWorksContext},
    vxworks_sync::{VxWorksFutex, VxWorksFutexBuilder},
    vxworks_threading::{VxWorksThread, VxWorksThreadBuilder, VxWorksThreadConfig},
};

use wrt_platform::{PageAllocator, FutexLike, WASM_PAGE_SIZE};
use core::sync::atomic::Ordering;
use core::time::Duration;

fn main() {
    println!("=== VxWorks Platform Usage Example ===");
    
    #[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
    {
        println!("Running on VxWorks platform!");
        run_vxworks_examples();
    }
    
    #[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
    {
        println!("VxWorks platform not available on this system.");
        println!("This example demonstrates how VxWorks support would work:");
        show_vxworks_concepts();
    }
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn run_vxworks_examples() {
    // Real VxWorks implementation examples
    example_rtp_memory_allocation();
    example_lkm_memory_allocation();
    example_synchronization();
    example_threading();
    example_complete_integration();
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_rtp_memory_allocation() {
    println!("\n=== Example 1: RTP Memory Allocation ===");
    
    let mut allocator = VxWorksAllocatorBuilder::new()
        .context(VxWorksContext::Rtp)
        .max_pages(100)
        .enable_guard_pages(false)
        .build()
        .expect("Failed to create RTP allocator");
    
    println!("Created RTP allocator with max {} pages", allocator.max_pages);
    
    let pages_to_allocate = 10;
    let ptr = allocator.allocate_pages(pages_to_allocate)
        .expect("Failed to allocate pages");
    
    println!("Allocated {} pages", pages_to_allocate);
    
    allocator.deallocate_pages(ptr, pages_to_allocate)
        .expect("Failed to deallocate");
    
    println!("Deallocated all pages");
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_lkm_memory_allocation() {
    println!("\n=== Example 2: LKM Memory Allocation ===");
    
    let mut allocator = VxWorksAllocatorBuilder::new()
        .context(VxWorksContext::Lkm)
        .max_pages(50)
        .use_dedicated_partition(true)
        .enable_guard_pages(true)
        .build()
        .expect("Failed to create LKM allocator");
    
    println!("Created LKM allocator with dedicated partition");
    
    let pages = 5;
    let ptr = allocator.allocate_pages(pages)
        .expect("Failed to allocate from partition");
    
    println!("Allocated {} pages from partition", pages);
    
    allocator.deallocate_pages(ptr, pages)
        .expect("Failed to deallocate");
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_synchronization() {
    println!("\n=== Example 3: Synchronization ===");
    
    let futex = VxWorksFutexBuilder::new(VxWorksContext::Rtp)
        .initial_value(0)
        .build()
        .expect("Failed to create futex");
    
    println!("Created VxWorks futex");
    
    futex.store(42, Ordering::Release);
    let value = futex.load(Ordering::Acquire);
    println!("Stored and loaded value: {}", value);
    
    let woken = futex.wake_one().expect("Failed to wake");
    println!("Woke {} waiters", woken);
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_threading() {
    println!("\n=== Example 4: Threading ===");
    
    // Binary std/no_std choice
    println!("Threading examples would work with VxWorks tasks and pthreads");
}

#[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
fn example_complete_integration() {
    println!("\n=== Example 5: Complete Integration ===");
    println!("VxWorks platform integrated with WRT runtime");
}

// This function demonstrates the concepts even on non-VxWorks platforms
#[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
fn show_vxworks_concepts() {
    println!("\n=== VxWorks Platform Concepts ===");
    
    println!("\n1. Execution Contexts:");
    println!("   - RTP (Real-Time Process): User-space applications");
    println!("   - LKM (Loadable Kernel Module): Kernel-space modules");
    
    println!("\n2. Memory Management:");
    println!("   - RTP: Uses standard C malloc/free or POSIX APIs");
    println!("   - LKM: Uses VxWorks memory partitions (memPartAlloc/memPartFree)");
    println!("   - Both support 64KB aligned WASM pages");
    
    println!("\n3. Synchronization:");
    println!("   - RTP: POSIX semaphores (sem_init, sem_wait, sem_post)");
    println!("   - LKM: VxWorks semaphores (semBCreate, semTake, semGive)");
    println!("   - Both provide futex-like semantics for WRT");
    
    println!("\n4. Threading:");
    println!("   - RTP: POSIX threads (pthread_create, pthread_join)");
    println!("   - LKM: VxWorks tasks (taskSpawn, taskDelete)");
    
    println!("\n5. Configuration Example:");
    println!("   ```rust");
    println!("   let allocator = VxWorksAllocatorBuilder::new()");
    println!("       .context(VxWorksContext::Rtp)");
    println!("       .max_pages(1024)");
    println!("       .enable_guard_pages(true)");
    println!("       .build()?;";
    println!("   ```");
    
    println!("\n6. Platform Integration:");
    println!("   - Implements wrt_platform::PageAllocator trait");
    println!("   - Implements wrt_platform::FutexLike trait");
    println!("   - Works seamlessly with WRT runtime components");
    
    println!("\n7. Real-time Features:");
    println!("   - Priority inheritance for semaphores");
    println!("   - Deterministic memory allocation with partitions");
    println!("   - Support for real-time task priorities");
    
    println!("\nTo use VxWorks platform support:");
    println!("1. Enable the 'platform-vxworks' feature");
    println!("2. Compile for target_os = \"vxworks\"");
    println!("3. Link with VxWorks runtime libraries");
    
    println!("\nExample usage in application:");
    println!("```rust");
    println!("use wrt_platform::vxworks_memory::*;");
    println!("use wrt_platform::vxworks_sync::*;");
    println!("");
    println!("// Detect execution context");
    println!("let context = detect_vxworks_context();";
    println!("");
    println!("// Create platform components");
    println!("let allocator = VxWorksAllocatorBuilder::new()");
    println!("    .context(context)");
    println!("    .build()?;";
    println!("");
    println!("let futex = VxWorksFutexBuilder::new(context)");
    println!("    .build()?;";
    println!("");
    println!("// Use with WRT runtime");
    println!("let runtime = wrt::Runtime::builder()");
    println!("    .with_allocator(Box::new(allocator))");
    println!("    .with_futex(Box::new(futex))");
    println!("    .build()?;";
    println!("```");
    
    // Demonstrate trait usage with mock implementations
    demonstrate_trait_usage();
}

#[cfg(not(all(feature = "platform-vxworks", target_os = "vxworks")))]
fn demonstrate_trait_usage() {
    println!("\n=== Trait Usage Demonstration ===");
    
    // Binary std/no_std choice
    struct MockVxWorksAllocator {
        allocated_pages: usize,
        max_pages: usize,
    }
    
    impl MockVxWorksAllocator {
        fn new(max_pages: usize) -> Self {
            Self { allocated_pages: 0, max_pages }
        }
    }
    
    impl PageAllocator for MockVxWorksAllocator {
        fn allocate_pages(&mut self, pages: usize) -> Result<core::ptr::NonNull<u8>, wrt_error::Error> {
            if self.allocated_pages + pages > self.max_pages {
                return Err(wrt_error::Error::runtime_execution_error("
                ;
            }
            
            // Binary std/no_std choice
            // In real implementation, this would call VxWorks APIs
            let ptr = Box::into_raw(vec![0u8; pages * WASM_PAGE_SIZE].into_boxed_slice()) as *mut u8;
            self.allocated_pages += pages;
            
            core::ptr::NonNull::new(ptr).ok_or_else(|| 
                wrt_error::Error::new(ErrorKind::Memory, "Allocation failed"))
        }
        
        fn deallocate_pages(&mut self, ptr: core::ptr::NonNull<u8>, pages: usize) -> Result<(), wrt_error::Error> {
            // Binary std/no_std choice
            let slice = unsafe {
                Box::from_raw(core::slice::from_raw_parts_mut(ptr.as_ptr(), pages * WASM_PAGE_SIZE))
            };
            drop(slice);
            
            self.allocated_pages = self.allocated_pages.saturating_sub(pages);
            Ok(())
        }
        
        fn grow_pages(&mut self, old_ptr: core::ptr::NonNull<u8>, old_pages: usize, new_pages: usize) 
            -> Result<core::ptr::NonNull<u8>, wrt_error::Error> {
            let new_ptr = self.allocate_pages(new_pages)?;
            
            unsafe {
                core::ptr::copy_nonoverlapping(
                    old_ptr.as_ptr(),
                    new_ptr.as_ptr(),
                    old_pages * WASM_PAGE_SIZE
                );
            }
            
            self.deallocate_pages(old_ptr, old_pages)?;
            Ok(new_ptr)
        }
        
        fn allocated_pages(&self) -> usize {
            self.allocated_pages
        }
        
        fn max_pages(&self) -> usize {
            self.max_pages
        }
    }
    
    // Mock futex for demonstration
    struct MockVxWorksFutex {
        value: core::sync::atomic::AtomicU32,
    }
    
    impl MockVxWorksFutex {
        fn new(initial: u32) -> Self {
            Self {
                value: core::sync::atomic::AtomicU32::new(initial),
            }
        }
    }
    
    impl FutexLike for MockVxWorksFutex {
        fn wait(&self, expected: u32, _timeout: Option<Duration>) -> Result<(), wrt_error::Error> {
            if self.value.load(Ordering::Acquire) != expected {
                return Ok();
            }
            // Mock wait - in real implementation would call VxWorks wait APIs
            Ok(())
        }
        
        fn wake_one(&self) -> Result<u32, wrt_error::Error> {
            // Mock wake - in real implementation would call VxWorks signal APIs
            Ok(1)
        }
        
        fn wake_all(&self) -> Result<u32, wrt_error::Error> {
            // Mock wake all - in real implementation would call VxWorks broadcast APIs
            Ok(u32::MAX)
        }
        
        fn load(&self, ordering: Ordering) -> u32 {
            self.value.load(ordering)
        }
        
        fn store(&self, value: u32, ordering: Ordering) {
            self.value.store(value, ordering);
        }
        
        fn compare_exchange_weak(
            &self,
            current: u32,
            new: u32,
            success: Ordering,
            failure: Ordering,
        ) -> Result<u32, u32> {
            self.value.compare_exchange_weak(current, new, success, failure)
        }
    }
    
    // Demonstrate usage
    let mut allocator = MockVxWorksAllocator::new(100);
    let futex = MockVxWorksFutex::new(0);
    
    println!("Created mock VxWorks allocator and futex");
    
    // Binary std/no_std choice
    match allocator.allocate_pages(10) {
        Ok(ptr) => {
            println!("✓ Allocated 10 pages successfully");
            println!("  Current allocated: {} pages", allocator.allocated_pages);
            
            if let Err(e) = allocator.deallocate_pages(ptr, 10) {
                println!("✗ Deallocation failed: {}", e);
            } else {
                println!("✓ Deallocated successfully");
                println!("  Final allocated: {} pages", allocator.allocated_pages);
            }
        }
        Err(e) => println!("✗ Allocation failed: {}", e),
    }
    
    // Test futex
    futex.store(42, Ordering::SeqCst);
    let value = futex.load(Ordering::SeqCst);
    println!("✓ Futex operations: stored and loaded {}", value);
    
    match futex.wake_one() {
        Ok(woken) => println!("✓ Wake operation: woke {} waiters", woken),
        Err(e) => println!("✗ Wake failed: {}", e),
    }
    
    println!("\nThis demonstrates how VxWorks types implement WRT traits!");
}