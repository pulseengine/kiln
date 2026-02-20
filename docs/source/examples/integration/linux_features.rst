======================================
Linux Platform Features
======================================

.. epigraph::

   "Linux is only free if your time has no value."
   
   -- Jamie Zawinski (but we're here to save your time!)

Linux is the Swiss Army knife of operating systems - it runs on everything from tiny Raspberry Pis to massive supercomputers. Kiln leverages Linux's rich feature set while abstracting away the complexity. Let's dive into the penguin-powered goodness!

.. admonition:: What You'll Learn
   :class: note

   - Advanced memory management with mmap and huge pages
   - Futex-based synchronization primitives
   - cgroups for resource isolation
   - io_uring for high-performance I/O
   - Security features (SELinux, seccomp, capabilities)
   - ARM Memory Tagging Extension (MTE) on Linux

Linux Memory Magic 🐧
--------------------

The Power of mmap
~~~~~~~~~~~~~~~~

Linux's virtual memory system is incredibly flexible:

.. code-block:: rust
   :caption: Advanced mmap usage
   :linenos:

   use kiln_platform::{LinuxAllocator, LinuxAllocatorBuilder};
   
   fn create_advanced_allocator() -> Result<LinuxAllocator, Error> {
       LinuxAllocatorBuilder::new()
           .with_maximum_pages(4096)     // 256MB max
           .with_guard_pages(true)       // PROT_NONE guards
           .with_populate(true)          // MAP_POPULATE for pre-fault
           .with_huge_pages(true)        // MAP_HUGETLB if available
           .with_numa_node(0)            // Bind to NUMA node 0
           .with_mlock(true)             // Lock in RAM (no swap)
           .build()
   }
   
   // Custom mmap flags for special cases
   fn create_shared_memory_allocator() -> Result<LinuxAllocator, Error> {
       use kiln_platform::linux_memory::MmapFlags;
       
       LinuxAllocatorBuilder::new()
           .with_custom_flags(
               MmapFlags::SHARED |       // MAP_SHARED for IPC
               MmapFlags::ANONYMOUS |    // No file backing
               MmapFlags::NORESERVE |    // Overcommit allowed
               MmapFlags::STACK         // Optimize for stack-like access
           )
           .with_name("wasm_shared_heap")  // Shows in /proc/pid/maps
           .build()
   }

Transparent Huge Pages
~~~~~~~~~~~~~~~~~~~~~

Optimize TLB usage without explicit huge page allocation:

.. code-block:: rust
   :caption: THP configuration
   :linenos:

   use kiln_platform::linux_memory::{ThpMode, MadviseFlags};
   
   fn optimize_with_thp(allocator: &mut LinuxAllocator) -> Result<(), Error> {
       // Enable THP for the allocation
       allocator.set_thp_mode(ThpMode::Always)?;
       
       // Allocate memory
       let (ptr, size) = allocator.allocate(1024, None)?;
       
       // Advise kernel about usage patterns
       allocator.madvise(ptr, size, MadviseFlags::HUGEPAGE)?;
       
       // For sequential access patterns
       allocator.madvise(ptr, size, MadviseFlags::SEQUENTIAL)?;
       
       // Check if we got huge pages
       let stats = allocator.get_memory_stats()?;
       println!("Huge pages: {} / {} pages", 
                stats.huge_pages, 
                stats.total_pages);
       
       Ok(())
   }

NUMA Awareness
~~~~~~~~~~~~~

Optimize for multi-socket systems:

.. code-block:: rust
   :caption: NUMA-aware allocation
   :linenos:

   use kiln_platform::linux_memory::{NumaPolicy, NumaNode};
   
   fn setup_numa_memory() -> Result<(), Error> {
       // Detect NUMA topology
       let topology = LinuxAllocator::detect_numa_topology()?;
       
       println!("NUMA nodes: {}", topology.num_nodes);
       for node in &topology.nodes {
           println!("  Node {}: {} MB free", node.id, node.free_mb);
       }
       
       // Create per-node allocators
       let mut allocators = Vec::new();
       for node in &topology.nodes {
           let allocator = LinuxAllocatorBuilder::new()
               .with_numa_node(node.id)
               .with_numa_policy(NumaPolicy::Bind)  // Strict binding
               .with_preferred_node(node.id)       // Fallback preference
               .build()?;
           allocators.push(allocator);
       }
       
       // Interleave allocation across nodes
       let interleaved = LinuxAllocatorBuilder::new()
           .with_numa_policy(NumaPolicy::Interleave)
           .with_numa_mask(&topology.all_nodes_mask())
           .build()?;
       
       Ok(())
   }

ARM Memory Tagging Extension 🏷️
-------------------------------

Hardware memory safety on ARM64:

.. code-block:: rust
   :caption: MTE for memory safety
   :linenos:

   #[cfg(all(target_arch = "aarch64", feature = "linux-mte"))]
   use kiln_platform::{
       LinuxArm64MteAllocator, 
       LinuxArm64MteAllocatorBuilder,
       MteMode
   };
   
   fn create_mte_protected_memory() -> Result<LinuxArm64MteAllocator, Error> {
       // Check if MTE is available
       if !LinuxArm64MteAllocator::is_available()? {
           return Err(Error::FeatureNotAvailable("MTE"));
       }
       
       let allocator = LinuxArm64MteAllocatorBuilder::new()
           .with_maximum_pages(1024)
           .with_mte_mode(MteMode::Synchronous)  // Immediate tag checks
           .with_tag_mask(0xFF)                  // Use all tag bits
           .with_tag_check_on_free(true)         // Detect use-after-free
           .build()?;
       
       // Allocate tagged memory
       let (tagged_ptr, size) = allocator.allocate_tagged(16, 0xA)?;
       
       // Access with correct tag works
       unsafe {
           let ptr = tagged_ptr.as_ptr();
           *ptr = 42;  // OK
       }
       
       // Access with wrong tag would fault
       // let wrong_ptr = tagged_ptr.with_tag(0xB);
       // unsafe { *wrong_ptr.as_ptr() = 42; }  // SIGSEGV!
       
       Ok(allocator)
   }

Futex: Fast Userspace Mutex 🔒
------------------------------

Linux's futex is the foundation of all synchronization:

.. code-block:: rust
   :caption: Advanced futex usage
   :linenos:

   use kiln_platform::{LinuxFutex, LinuxFutexBuilder};
   use std::time::Duration;
   
   fn advanced_futex_patterns() -> Result<(), Error> {
       // Priority inheritance futex
       let pi_futex = LinuxFutexBuilder::new()
           .with_priority_inheritance(true)
           .with_robust(true)  // Survive owner death
           .build()?;
       
       // Condition variable pattern
       let cond_futex = LinuxFutexBuilder::new()
           .with_shared(true)  // Cross-process
           .with_realtime_clock(true)
           .build()?;
       
       // Adaptive mutex (spin then sleep)
       let adaptive = LinuxFutexBuilder::new()
           .with_spin_count(1000)
           .with_backoff_ns(100)
           .build()?;
       
       // Benchmark different wait strategies
       benchmark_futex_strategies()?;
       
       Ok(())
   }
   
   // Futex-based semaphore
   fn create_counting_semaphore(initial: u32) -> LinuxFutex {
       let futex = LinuxFutexBuilder::new()
           .with_initial_value(initial)
           .build()
           .unwrap();
       
       // Acquire (P operation)
       fn acquire(futex: &LinuxFutex) {
           loop {
               let current = futex.load();
               if current > 0 {
                   if futex.compare_exchange(current, current - 1).is_ok() {
                       return;
                   }
               } else {
                   futex.wait(0).ok();
               }
           }
       }
       
       // Release (V operation)  
       fn release(futex: &LinuxFutex) {
           futex.fetch_add(1);
           futex.wake(1);
       }
       
       futex
   }

cgroups Resource Control 📊
--------------------------

Isolate and limit WebAssembly execution:

.. code-block:: rust
   :caption: cgroups v2 integration
   :linenos:

   use kiln_platform::linux_cgroups::{
       Cgroup,
       CgroupBuilder,
       ResourceLimits
   };
   
   fn isolate_wasm_execution() -> Result<(), Error> {
       // Create cgroup for WASM runtime
       let cgroup = CgroupBuilder::new("wasm_runtime")
           .with_parent("/sys/fs/cgroup/user.slice")
           .build()?;
       
       // Set resource limits
       cgroup.set_limits(ResourceLimits {
           memory_max: Some(1 << 30),        // 1GB max memory
           memory_high: Some(768 << 20),     // 768MB soft limit
           cpu_max: Some((50000, 100000)),   // 50% CPU (50ms/100ms)
           cpu_weight: Some(100),            // Default priority
           io_max: Some(vec![
               ("8:0", "rbps=10485760 wbps=10485760"), // 10MB/s
           ]),
           pids_max: Some(100),              // Max 100 processes
       })?;
       
       // Add current process to cgroup
       cgroup.add_process(std::process::id())?;
       
       // Monitor resource usage
       let stats = cgroup.get_stats()?;
       println!("Memory usage: {} MB", stats.memory_current / 1024 / 1024);
       println!("CPU usage: {} ms", stats.cpu_usage_usec / 1000);
       
       // Set up memory pressure notification
       cgroup.on_memory_pressure(|level| {
           match level {
               PressureLevel::Low => println!("Memory pressure: low"),
               PressureLevel::Medium => reduce_cache_size(),
               PressureLevel::Critical => emergency_gc(),
           }
       })?;
       
       Ok(())
   }

io_uring: Modern Async I/O 🚀
-----------------------------

High-performance I/O for WebAssembly:

.. code-block:: rust
   :caption: io_uring integration
   :linenos:

   use kiln_platform::linux_io_uring::{IoUring, IoUringBuilder};
   
   fn setup_async_io() -> Result<IoUring, Error> {
       // Check if io_uring is available
       if !IoUring::is_available()? {
           return Err(Error::FeatureNotAvailable("io_uring"));
       }
       
       let ring = IoUringBuilder::new()
           .with_entries(256)           // Queue depth
           .with_sq_thread(true)        // Kernel SQ polling thread
           .with_sq_thread_cpu(0)       // Pin to CPU 0
           .with_cq_size(512)          // Completion queue 2x submit
           .build()?;
       
       // Example: Async file read
       let fd = open_file("data.wasm")?;
       let buffer = vec![0u8; 4096];
       
       // Submit read operation
       let user_data = 42;
       ring.submit_read(fd, buffer, 0, user_data)?;
       
       // Do other work...
       
       // Wait for completion
       let completion = ring.wait_completion()?;
       assert_eq!(completion.user_data, user_data);
       
       if completion.result >= 0 {
           println!("Read {} bytes", completion.result);
       }
       
       Ok(ring)
   }

Security Hardening 🔐
--------------------

Linux security features for WebAssembly isolation:

.. code-block:: rust
   :caption: Security sandboxing
   :linenos:

   use kiln_platform::linux_security::{
       Seccomp,
       SeccompBuilder,
       Capabilities,
       Namespace
   };
   
   fn create_secure_sandbox() -> Result<(), Error> {
       // 1. Drop capabilities
       Capabilities::current()?
           .drop_all_except(&[
               Capability::SYS_RESOURCE,  // For resource limits
           ])?;
       
       // 2. Set up namespaces
       Namespace::unshare(&[
           NamespaceType::Mount,    // Isolated filesystem
           NamespaceType::PID,      // Isolated process tree
           NamespaceType::Network,  // No network access
           NamespaceType::IPC,      // Isolated IPC
       ])?;
       
       // 3. Configure seccomp filter
       let filter = SeccompBuilder::new()
           .allow(&[
               // Minimal syscalls for WASM execution
               "read", "write", "mmap", "munmap", "mprotect",
               "futex", "clock_gettime", "exit_group",
           ])
           .with_default_action(SeccompAction::Kill)
           .with_log_failures(true)
           .build()?;
       
       filter.load()?;
       
       // 4. Set up SELinux context if available
       if SeLinux::is_enabled()? {
           SeLinux::set_context("system_u:system_r:wasm_runtime_t:s0")?;
       }
       
       println!("Sandbox configured successfully");
       Ok(())
   }

Performance Monitoring 📈
------------------------

Linux perf integration:

.. code-block:: rust
   :caption: Performance monitoring
   :linenos:

   use kiln_platform::linux_perf::{
       PerfEvent,
       PerfEventBuilder,
       PerfCounter
   };
   
   fn profile_wasm_execution() -> Result<(), Error> {
       // Create performance counters
       let mut counters = vec![
           PerfEventBuilder::new()
               .with_type(PerfCounter::CpuCycles)
               .with_config(0)
               .build()?,
           PerfEventBuilder::new()
               .with_type(PerfCounter::Instructions)
               .with_config(0)
               .build()?,
           PerfEventBuilder::new()
               .with_type(PerfCounter::CacheMisses)
               .with_config(0)
               .build()?,
           PerfEventBuilder::new()
               .with_type(PerfCounter::BranchMisses)
               .with_config(0)
               .build()?,
       ];
       
       // Start counting
       for counter in &mut counters {
           counter.enable()?;
       }
       
       // Execute WASM code
       execute_wasm_benchmark()?;
       
       // Read results
       for counter in &counters {
           let value = counter.read()?;
           println!("{}: {}", counter.name(), value);
       }
       
       // Calculate IPC (Instructions Per Cycle)
       let cycles = counters[0].read()?;
       let instructions = counters[1].read()?;
       let ipc = instructions as f64 / cycles as f64;
       println!("IPC: {:.2}", ipc);
       
       Ok(())
   }

Container Integration 🐳
-----------------------

Running Kiln in containers:

.. code-block:: rust
   :caption: Container-aware configuration
   :linenos:

   use kiln_platform::linux_container;
   
   fn detect_container_environment() -> Result<(), Error> {
       let env = linux_container::detect_environment()?;
       
       match env {
           ContainerEnvironment::Docker => {
               println!("Running in Docker");
               // Adjust memory limits based on container limits
               adjust_for_docker()?;
           },
           ContainerEnvironment::Kubernetes => {
               println!("Running in Kubernetes");
               // Read resource limits from downward API
               read_k8s_limits()?;
           },
           ContainerEnvironment::Systemd => {
               println!("Running under systemd");
               // Use systemd resource controls
               configure_systemd_limits()?;
           },
           ContainerEnvironment::None => {
               println!("Running on bare metal");
           },
       }
       
       Ok(())
   }

Best Practices 📚
-----------------

1. **Use cgroups v2** for better resource control
2. **Enable huge pages** for large heaps
3. **Profile with perf** before optimizing
4. **Sandbox with seccomp** for security
5. **Monitor with eBPF** for deep insights

Linux-Specific Tips 💡
---------------------

**Memory:**
   - Check `/proc/sys/vm/overcommit_memory`
   - Use `madvise()` for access patterns
   - Consider `userfaultfd` for lazy loading

**Performance:**
   - Pin threads to CPUs with `sched_setaffinity`
   - Use `SCHED_FIFO` for real-time threads
   - Enable `CONFIG_NO_HZ_FULL` for dedicated cores

**Security:**
   - Combine multiple layers (capabilities + seccomp + SELinux)
   - Use `landlock` LSM on newer kernels
   - Consider `grsecurity` patches for high security

.. admonition:: Kernel Tuning
   :class: tip

   For optimal WebAssembly performance on Linux:
   
   .. code-block:: bash
   
      # Huge pages
      echo 1024 > /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
      
      # Disable THP defrag
      echo never > /sys/kernel/mm/transparent_hugepage/defrag
      
      # Increase futex hash table
      echo 65536 > /proc/sys/kernel/futex_hashsize

Next Steps 🎯
-------------

- Compare with :doc:`qnx_features` for real-time systems
- Explore :doc:`macos_features` for development
- Learn about :doc:`hardware_security` for advanced protection