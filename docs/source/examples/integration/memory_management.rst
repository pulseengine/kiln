======================================
Cross-Platform Memory Management
======================================

.. epigraph::

   "In theory, memory is memory. In practice, every OS has its own special way of making your life difficult."
   
   -- Anonymous systems programmer

Memory management is where platform differences really shine (or burn, depending on your perspective). This guide shows you how to wrangle memory across different operating systems while keeping your sanity intact.

.. admonition:: What You'll Learn
   :class: note

   - Platform-specific memory allocation strategies
   - Guard pages and memory protection
   - Memory tagging and hardware security features
   - Huge pages and performance optimizations
   - Static vs dynamic allocation patterns

The Universal Memory Interface 🌐
---------------------------------

Every platform in Kiln implements the same ``PageAllocator`` trait, but how they do it varies wildly:

.. code-block:: rust
   :caption: The trait every platform must implement
   :linenos:

   pub trait PageAllocator: Debug + Send + Sync {
       fn allocate(
           &mut self,
           initial_pages: u32,
           maximum_pages: Option<u32>,
       ) -> Result<(NonNull<u8>, usize)>;
       
       fn grow(
           &mut self,
           current_pages: u32,
           additional_pages: u32,
       ) -> Result<u32>;
       
       fn protect(
           &mut self,
           offset: usize,
           size: usize,
           protection: Protection,
       ) -> Result<()>;
       
       fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()>;
   }

Platform-Specific Strategies 🎯
-------------------------------

Linux: mmap and Friends
~~~~~~~~~~~~~~~~~~~~~~~

Linux gives us the most flexibility with its virtual memory system:

.. code-block:: rust
   :caption: Linux memory allocation with all the bells and whistles
   :linenos:

   use kiln_platform::{LinuxAllocator, LinuxAllocatorBuilder};
   
   fn create_linux_allocator() -> Result<LinuxAllocator, Error> {
       LinuxAllocatorBuilder::new()
           .with_maximum_pages(1024)          // 64MB max
           .with_guard_pages(true)            // Protect against overflows
           .with_huge_pages(true)             // 2MB pages for performance
           .with_numa_node(0)                 // Pin to NUMA node 0
           .with_populate(true)               // Pre-fault pages
           .build()
   }
   
   // Advanced: ARM64 Memory Tagging Extension
   #[cfg(all(target_arch = "aarch64", feature = "linux-mte"))]
   fn create_mte_allocator() -> Result<LinuxArm64MteAllocator, Error> {
       use kiln_platform::{LinuxArm64MteAllocator, LinuxArm64MteAllocatorBuilder, MteMode};
       
       LinuxArm64MteAllocatorBuilder::new()
           .with_maximum_pages(512)
           .with_mte_mode(MteMode::Synchronous) // Immediate tag checking
           .with_tag_mask(0xF0)                 // Use upper 4 bits for tags
           .build()
   }

QNX: Memory Partitions and Real-Time
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

QNX's memory partitioning provides guaranteed memory reservations:

.. code-block:: rust
   :caption: QNX memory with partition support
   :linenos:

   use kiln_platform::{QnxAllocator, QnxAllocatorBuilder, QnxPartitionFlags};
   use kiln_platform::{QnxMemoryPartition, QnxMemoryPartitionBuilder};
   
   fn create_qnx_partitioned_memory() -> Result<QnxAllocator, Error> {
       // First, create a memory partition
       let partition = QnxMemoryPartitionBuilder::new("wasm_runtime")
           .with_size(32 * 1024 * 1024)  // 32MB partition
           .with_flags(QnxPartitionFlags::LOCKED | QnxPartitionFlags::NONPAGED)
           .with_minimum_size(16 * 1024 * 1024)  // Guarantee at least 16MB
           .build()?;
       
       // Then create allocator within the partition
       QnxAllocatorBuilder::new()
           .with_partition(partition)
           .with_maximum_pages(512)
           .with_guard_pages(true)
           .with_anon_mmap(true)  // Use MAP_ANON for security
           .build()
   }

macOS: Mach VM Magic
~~~~~~~~~~~~~~~~~~~

macOS has its own special VM system with unique features:

.. code-block:: rust
   :caption: macOS memory with VM features
   :linenos:

   use kiln_platform::{MacOsAllocator, MacOsAllocatorBuilder};
   
   fn create_macos_allocator() -> Result<MacOsAllocator, Error> {
       let allocator = MacOsAllocatorBuilder::new()
           .with_maximum_pages(2048)
           .with_guard_pages(true)
           .with_memory_tagging(true)     // macOS-specific tagging
           .with_vm_copy_optimization()   // Use vm_copy for speed
           .with_purgeable_behavior(true) // Allow purging under pressure
           .build()?;
       
       // macOS-specific: mark regions as purgeable
       allocator.mark_purgeable(offset, size, Volatility::Purgeable)?;
       
       Ok(allocator)
   }

Embedded: Static Allocation
~~~~~~~~~~~~~~~~~~~~~~~~~~

For embedded systems, dynamic allocation is often a no-go:

.. code-block:: rust
   :caption: Static allocation for embedded platforms
   :linenos:

   use kiln_platform::{TockAllocator, TockAllocatorBuilder};
   
   // Static buffer allocated at compile time
   static mut WASM_MEMORY: [u8; 1024 * 1024] = [0; 1024 * 1024]; // 1MB
   
   fn create_tock_allocator() -> Result<TockAllocator, Error> {
       TockAllocatorBuilder::new()
           .with_static_buffer(unsafe { &mut WASM_MEMORY })
           .with_verification_level(VerificationLevel::Full)
           .with_maximum_pages(16)  // 1MB / 64KB = 16 pages
           .build()
   }

Memory Protection Patterns 🛡️
------------------------------

Guard Pages: Your First Line of Defense
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Guard pages catch buffer overflows before they become exploits:

.. code-block:: rust
   :caption: Guard page implementation
   :linenos:

   fn setup_guard_pages(allocator: &mut impl PageAllocator) -> Result<(), Error> {
       // Allocate with room for guards
       let (ptr, size) = allocator.allocate(10, Some(12))?;
       
       // Protect the first page (underflow guard)
       allocator.protect(0, WASM_PAGE_SIZE, Protection::None)?;
       
       // Protect the last page (overflow guard)  
       let last_page_offset = 11 * WASM_PAGE_SIZE;
       allocator.protect(last_page_offset, WASM_PAGE_SIZE, Protection::None)?;
       
       // Now pages 1-10 are usable, with guards on both sides
       Ok(())
   }

Memory Tagging: Hardware-Assisted Safety
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

On supported hardware, use memory tagging for use-after-free protection:

.. code-block:: rust
   :caption: Memory tagging example
   :linenos:

   #[cfg(feature = "memory-tagging")]
   fn tagged_allocation(allocator: &mut impl TaggedAllocator) -> Result<(), Error> {
       // Allocate with a specific tag
       let tag = 0x5;
       let (ptr, size) = allocator.allocate_tagged(10, tag)?;
       
       // Create a tagged pointer
       let tagged_ptr = ptr.with_tag(tag);
       
       // Access with wrong tag will fault
       let wrong_tag = 0x7;
       let bad_ptr = ptr.with_tag(wrong_tag);
       // unsafe { *bad_ptr } // This would trap!
       
       // Correct tag works fine
       unsafe { 
           *tagged_ptr.as_ptr() = 42;  // OK
       }
       
       Ok(())
   }

Performance Optimizations 🚀
---------------------------

Huge Pages for Big Wins
~~~~~~~~~~~~~~~~~~~~~~

Reduce TLB pressure with huge pages:

.. code-block:: rust
   :caption: Huge page optimization
   :linenos:

   use kiln_platform::memory_optimizations::{
       MemoryOptimization, 
       PlatformMemoryOptimizer
   };
   
   fn optimize_for_large_heap() -> Result<Box<dyn PageAllocator>, Error> {
       let optimizer = PlatformMemoryOptimizer::new();
       
       // Check if huge pages are available
       if optimizer.supports(MemoryOptimization::HugePages) {
           println!("Using 2MB huge pages for better TLB performance");
           
           return optimizer.create_allocator()
               .with_huge_pages(true)
               .with_minimum_huge_page_size(2 * 1024 * 1024)
               .build();
       }
       
       // Fall back to regular pages
       optimizer.create_allocator()
           .with_transparent_huge_pages(true)  // Let kernel decide
           .build()
   }

NUMA-Aware Allocation
~~~~~~~~~~~~~~~~~~~~

For multi-socket systems, NUMA awareness is crucial:

.. code-block:: rust
   :caption: NUMA-aware memory allocation
   :linenos:

   fn create_numa_aware_allocator(node_id: u32) -> Result<impl PageAllocator, Error> {
       let detector = PlatformDetector::new();
       let caps = detector.detect()?;
       
       if caps.memory.numa_nodes > 1 {
           println!("NUMA system detected with {} nodes", caps.memory.numa_nodes);
           
           // Pin allocation to specific NUMA node
           return LinuxAllocatorBuilder::new()
               .with_numa_node(node_id)
               .with_numa_binding(NumaPolicy::Bind)
               .with_prefer_local(true)
               .build();
       }
       
       // Single node system
       LinuxAllocatorBuilder::new().build()
   }

Best Practices 📚
-----------------

1. **Always Use Guard Pages** in development and test
2. **Enable Memory Tagging** on supported hardware
3. **Profile Before Optimizing** - huge pages aren't always faster
4. **Test Memory Pressure** - ensure graceful degradation
5. **Verify Static Buffers** - embedded systems need careful sizing

Platform-Specific Gotchas ⚠️
----------------------------

**Linux:**
   - Overcommit can lie about available memory
   - Transparent huge pages can cause latency spikes
   - cgroups can limit your memory unexpectedly

**macOS:**
   - VM compression can make profiling tricky
   - Hypervisor framework has its own limits
   - Code signing affects memory protection

**QNX:**
   - Partition limits are hard limits
   - Priority inheritance affects allocation
   - Adaptive partitioning changes behavior

**Embedded:**
   - No swap means no second chances
   - MPU regions are limited (8-16 typical)
   - Stack size affects available heap

.. admonition:: Remember
   :class: warning

   Platform differences in memory management aren't bugs - they're features! Each platform optimizes for its use case. Your job is to work *with* the platform, not against it.

Next Steps 🎯
-------------

- Learn about :doc:`synchronization` for multi-threaded memory access
- Explore :doc:`performance_optimizations` for memory-specific tuning
- Check out :doc:`hardware_security` for advanced protection features