======================================
Runtime Platform Detection
======================================

.. epigraph::

   "The best code is the code that knows where it's running."
   
   -- Performance engineer proverb

Platform detection lets your WebAssembly runtime adapt to its environment dynamically. Whether you're running on a beefy server or a tiny microcontroller, detection helps you make intelligent decisions at runtime.

.. admonition:: What You'll Learn
   :class: note

   - Detecting platform capabilities at runtime
   - Choosing optimal implementations based on hardware
   - Feature detection for CPU extensions
   - Memory and cache topology discovery
   - Compile-time vs runtime detection strategies

The Platform Detective 🔍
------------------------

Meet ``PlatformDetector`` - your runtime environment investigator:

.. code-block:: rust
   :caption: Basic platform detection
   :linenos:

   use kiln_platform::runtime_detection::{
       PlatformDetector, 
       PlatformCapabilities
   };
   
   fn detect_environment() -> Result<PlatformCapabilities, Error> {
       let mut detector = PlatformDetector::new();
       
       // Detect everything (cached after first call)
       let caps = detector.detect()?;
       
       println!("Platform: {}", caps.platform_name);
       println!("Architecture: {}", caps.architecture);
       println!("CPU cores: {}", caps.memory.cpu_count);
       println!("Page size: {} bytes", caps.memory.page_size);
       println!("Total memory: {} MB", caps.memory.total_memory / 1024 / 1024);
       
       Ok(caps)
   }

Capability Categories 📊
-----------------------

Memory Capabilities
~~~~~~~~~~~~~~~~~~

Understand your memory subsystem:

.. code-block:: rust
   :caption: Memory subsystem detection
   :linenos:

   use kiln_platform::runtime_detection::MemoryCapabilities;
   
   fn analyze_memory(caps: &MemoryCapabilities) {
       println!("Memory Configuration:");
       println!("  Allocation granularity: {} bytes", caps.allocation_granularity);
       println!("  CPU count: {}", caps.cpu_count);
       println!("  NUMA nodes: {}", caps.numa_nodes);
       println!("  Huge page sizes: {:?}", caps.huge_page_sizes);
       
       // Cache topology
       for (level, cache) in caps.cache_topology.iter().enumerate() {
           println!("  L{} cache: {} KB (line size: {} bytes)", 
                    level + 1, 
                    cache.size / 1024,
                    cache.line_size);
       }
       
       // Make decisions based on capabilities
       if caps.numa_nodes > 1 {
           println!("⚠️  NUMA system detected - consider NUMA-aware allocation");
       }
       
       if caps.huge_page_sizes.contains(&(2 * 1024 * 1024)) {
           println!("✅ 2MB huge pages available for TLB optimization");
       }
   }

Synchronization Capabilities
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Discover available synchronization primitives:

.. code-block:: rust
   :caption: Synchronization feature detection
   :linenos:

   use kiln_platform::runtime_detection::SyncCapabilities;
   
   fn analyze_sync(caps: &SyncCapabilities) {
       println!("Synchronization Features:");
       
       if caps.has_native_futex {
           println!("✅ Native futex support (fast userspace locking)");
       } else {
           println!("❌ No native futex - using emulation");
       }
       
       if caps.has_priority_inheritance {
           println!("✅ Priority inheritance available");
       }
       
       if caps.has_robust_mutexes {
           println!("✅ Robust mutexes for crash recovery");
       }
       
       println!("Max spinlock iterations: {}", caps.max_spin_iterations);
       println!("Scheduling precision: {} ns", caps.scheduling_precision_ns);
       
       // Choose sync strategy
       let strategy = if caps.has_native_futex && caps.max_spin_iterations > 100 {
           "Hybrid spin/futex with adaptive backoff"
       } else if caps.has_native_futex {
           "Pure futex-based synchronization"
       } else {
           "Emulated futex with minimal spinning"
       };
       
       println!("Recommended strategy: {}", strategy);
   }

Security Capabilities
~~~~~~~~~~~~~~~~~~~~

Check for hardware security features:

.. code-block:: rust
   :caption: Security feature detection
   :linenos:

   use kiln_platform::runtime_detection::SecurityCapabilities;
   
   fn analyze_security(caps: &SecurityCapabilities) {
       println!("Security Features:");
       
       // Memory protection
       if caps.has_nx_bit {
           println!("✅ NX/XD bit (no-execute protection)");
       }
       
       if caps.has_memory_tagging {
           println!("✅ Hardware memory tagging");
           println!("   Tag bits: {}", caps.memory_tag_bits);
           println!("   Granularity: {} bytes", caps.memory_tag_granularity);
       }
       
       // Control flow protection
       if caps.has_cfi {
           println!("✅ Control Flow Integrity support");
       }
       
       if caps.has_shadow_stack {
           println!("✅ Shadow stack for return address protection");
       }
       
       // Isolation features
       if caps.has_virtualization {
           println!("✅ Hardware virtualization available");
       }
       
       if caps.has_secure_enclave {
           println!("✅ Secure enclave execution (SGX/TrustZone)");
       }
       
       // Crypto acceleration
       if !caps.crypto_acceleration.is_empty() {
           println!("✅ Hardware crypto: {:?}", caps.crypto_acceleration);
       }
   }

Real-Time Capabilities
~~~~~~~~~~~~~~~~~~~~~

For real-time systems:

.. code-block:: rust
   :caption: Real-time feature detection
   :linenos:

   use kiln_platform::runtime_detection::RealtimeCapabilities;
   
   fn analyze_realtime(caps: &RealtimeCapabilities) {
       println!("Real-Time Features:");
       
       if caps.has_realtime_scheduler {
           println!("✅ Real-time scheduler available");
           println!("   Priority levels: {}", caps.priority_levels);
           println!("   Timer precision: {} ns", caps.timer_precision_ns);
       }
       
       if caps.has_cpu_isolation {
           println!("✅ CPU isolation support");
       }
       
       if caps.has_preemption_control {
           println!("✅ Preemption control available");
       }
       
       // Determine RT suitability
       let rt_score = calculate_rt_score(caps);
       match rt_score {
           90..=100 => println!("⭐ Excellent real-time platform"),
           70..=89 => println!("✅ Good real-time capabilities"),
           50..=69 => println!("⚠️  Limited real-time support"),
           _ => println!("❌ Not suitable for hard real-time"),
       }
   }

Platform-Specific Detection 🎯
-----------------------------

Linux Deep Dive
~~~~~~~~~~~~~~

Linux-specific capability detection:

.. code-block:: rust
   :caption: Linux-specific detection
   :linenos:

   #[cfg(target_os = "linux")]
   fn detect_linux_features() -> Result<LinuxFeatures, Error> {
       use kiln_platform::linux_detection;
       
       let features = linux_detection::detect_all()?;
       
       // Kernel version affects available features
       if features.kernel_version >= (5, 10, 0) {
           println!("✅ Modern kernel with latest features");
       }
       
       // cgroup detection
       match features.cgroup_version {
           CgroupVersion::V2 => println!("✅ cgroups v2 (unified hierarchy)"),
           CgroupVersion::V1 => println!("⚠️  cgroups v1 (legacy)"),
           CgroupVersion::None => println!("❌ No cgroup support"),
       }
       
       // Security modules
       if features.has_selinux {
           println!("✅ SELinux active");
       }
       if features.has_apparmor {
           println!("✅ AppArmor active");
       }
       
       // Special features
       if features.has_io_uring {
           println!("✅ io_uring available for async I/O");
       }
       
       Ok(features)
   }

Architecture-Specific Detection
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

CPU feature detection:

.. code-block:: rust
   :caption: CPU architecture detection
   :linenos:

   use kiln_platform::hardware_optimizations::compile_time;
   
   fn detect_cpu_features() {
       // ARM features
       #[cfg(target_arch = "aarch64")]
       {
           if compile_time::has_arm_feature("neon") {
               println!("✅ ARM NEON SIMD");
           }
           if compile_time::has_arm_feature("sve") {
               println!("✅ ARM SVE (Scalable Vector Extension)");
           }
           if compile_time::has_arm_feature("mte") {
               println!("✅ ARM MTE (Memory Tagging Extension)");
           }
           if compile_time::has_arm_feature("bti") {
               println!("✅ ARM BTI (Branch Target Identification)");
           }
       }
       
       // x86 features
       #[cfg(target_arch = "x86_64")]
       {
           if compile_time::has_x86_feature("avx2") {
               println!("✅ Intel AVX2 SIMD");
           }
           if compile_time::has_x86_feature("avx512f") {
               println!("✅ Intel AVX-512");
           }
           if compile_time::has_x86_feature("cet") {
               println!("✅ Intel CET (Control-flow Enforcement)");
           }
       }
   }

Adaptive Implementation Selection 🎨
-----------------------------------

Choose Optimal Allocator
~~~~~~~~~~~~~~~~~~~~~~~

Select the best allocator based on detected features:

.. code-block:: rust
   :caption: Adaptive allocator selection
   :linenos:

   use kiln_platform::prelude::*;
   
   fn create_optimal_allocator() -> Result<Box<dyn PageAllocator>, Error> {
       let detector = PlatformDetector::new();
       let caps = detector.detect()?;
       
       // Decision tree based on capabilities
       if caps.security.has_memory_tagging && caps.platform_name.contains("Linux") {
           println!("Using MTE-enabled allocator for hardware memory safety");
           return Ok(Box::new(
               LinuxArm64MteAllocatorBuilder::new()
                   .with_mte_mode(MteMode::Synchronous)
                   .build()?
           ));
       }
       
       if caps.memory.numa_nodes > 1 {
           println!("Using NUMA-aware allocator");
           return Ok(Box::new(
               LinuxAllocatorBuilder::new()
                   .with_numa_awareness(true)
                   .build()?
           ));
       }
       
       if caps.realtime.has_realtime_scheduler {
           println!("Using real-time optimized allocator");
           return Ok(Box::new(
               QnxAllocatorBuilder::new()
                   .with_deterministic_allocation(true)
                   .build()?
           ));
       }
       
       // Default fallback
       println!("Using standard platform allocator");
       Ok(Box::new(NoStdProviderBuilder::new().build()))
   }

Choose Optimal Synchronization
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Pick the best sync primitive:

.. code-block:: rust
   :caption: Adaptive synchronization selection
   :linenos:

   fn create_optimal_mutex() -> Result<Box<dyn MutexLike>, Error> {
       let detector = PlatformDetector::new();
       let caps = detector.detect()?;
       
       // High contention scenario
       if caps.memory.cpu_count > 32 {
           if caps.sync.has_hardware_transactional_memory {
               println!("Using HTM-based mutex for high contention");
               return Ok(Box::new(HtmMutex::new()));
           } else {
               println!("Using MCS lock for NUMA scalability");
               return Ok(Box::new(McsLock::new()));
           }
       }
       
       // Real-time scenario
       if caps.realtime.has_priority_inheritance {
           println!("Using priority-inheriting mutex");
           return Ok(Box::new(PriorityInheritanceMutex::new()));
       }
       
       // Low-latency scenario
       if caps.sync.scheduling_precision_ns < 1000 {
           println!("Using hybrid spin/block mutex");
           return Ok(Box::new(HybridMutex::with_spin_count(1000)));
       }
       
       // Default
       println!("Using standard platform mutex");
       Ok(Box::new(DefaultMutex::new()))
   }

Capability Scoring 📈
--------------------

Rate platform suitability:

.. code-block:: rust
   :caption: Platform scoring system
   :linenos:

   fn score_platform_for_wasm(caps: &PlatformCapabilities) -> PlatformScore {
       let mut score = PlatformScore::default();
       
       // Memory score (0-100)
       score.memory = calculate_memory_score(caps);
       
       // Performance score (0-100)
       score.performance = calculate_performance_score(caps);
       
       // Security score (0-100)
       score.security = calculate_security_score(caps);
       
       // Real-time score (0-100)
       score.realtime = calculate_realtime_score(caps);
       
       // Overall suitability
       score.overall = (score.memory + score.performance + 
                       score.security + score.realtime) / 4;
       
       // Recommendations
       if score.overall > 80 {
           score.recommendation = "Excellent platform for Kiln";
       } else if score.overall > 60 {
           score.recommendation = "Good platform with some limitations";
       } else if score.overall > 40 {
           score.recommendation = "Usable but consider optimizations";
       } else {
           score.recommendation = "Limited platform - use with caution";
       }
       
       score
   }

Best Practices 📚
-----------------

1. **Cache Detection Results** - Don't re-detect on every call
2. **Fail Gracefully** - Always have fallbacks
3. **Trust but Verify** - OS can lie about capabilities
4. **Profile on Target** - Detection isn't everything
5. **Document Assumptions** - Make requirements explicit

Common Patterns 🎯
-----------------

**Feature Flags Pattern:**

.. code-block:: rust

   struct RuntimeFeatures {
       use_huge_pages: bool,
       use_numa_aware: bool,
       use_hardware_crypto: bool,
       use_lock_free: bool,
   }
   
   impl RuntimeFeatures {
       fn detect() -> Self {
           let caps = PlatformDetector::new().detect().unwrap();
           Self {
               use_huge_pages: !caps.memory.huge_page_sizes.is_empty(),
               use_numa_aware: caps.memory.numa_nodes > 1,
               use_hardware_crypto: !caps.security.crypto_acceleration.is_empty(),
               use_lock_free: caps.memory.cpu_count > 4,
           }
       }
   }

.. admonition:: Remember
   :class: tip

   Platform detection is about making intelligent defaults, not absolute decisions. Always provide manual overrides for when auto-detection gets it wrong!

Next Steps 🎯
-------------

- Platform-specific guides: :doc:`linux_features`, :doc:`qnx_features`, :doc:`macos_features`
- Optimization strategies: :doc:`performance_optimizations`
- Security hardening: :doc:`hardware_security`