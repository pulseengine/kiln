==============
Platform Layer
==============

The Kiln Platform Abstraction Layer (PAL) provides a unified interface for platform-specific operations across diverse operating systems and embedded environments. This layer enables Kiln to run efficiently on everything from traditional POSIX systems to bare-metal embedded platforms.

Core Architecture
-----------------

The platform layer is built around two fundamental traits:

**PageAllocator**
  Manages WebAssembly memory pages (64 KiB each) with platform-specific optimizations:
  
  * Memory allocation and deallocation
  * Page growth and shrinking
  * Memory protection (guard pages, access control)
  * Hardware security features (MTE, MPK)
  * Verification levels for safety-critical systems

**FutexLike**
  Provides efficient synchronization primitives:
  
  * Atomic wait/wake operations
  * Timeout support
  * Platform-optimized implementations
  * Real-time guarantees where available

Supported Platforms
-------------------

Linux (x86_64, aarch64)
^^^^^^^^^^^^^^^^^^^^^^^^

* **Memory**: ``mmap``/``munmap`` with ``MAP_ANONYMOUS``
* **Sync**: Native ``futex`` system calls
* **ARM64 Features**: 
  
  - Memory Tagging Extension (MTE) support via ``PROT_MTE``
  - Synchronous, asynchronous, and asymmetric MTE modes
  - Hardware-accelerated memory safety

.. code-block:: rust

   // Linux allocator with MTE support
   let allocator = LinuxArm64MteAllocatorBuilder::new()
       .with_maximum_pages(100)
       .with_guard_pages(true)
       .with_mte_mode(MteMode::Synchronous)
       .build()?;

macOS (x86_64, aarch64)
^^^^^^^^^^^^^^^^^^^^^^^^

* **Memory**: ``mmap``/``munmap`` via libc or direct syscalls
* **Sync**: ``__ulock_wait``/``__ulock_wake`` private APIs
* **Features**:
  
  - Guard page support
  - Memory tagging (simulated)
  - No-libc mode for reduced dependencies

.. code-block:: rust

   // macOS allocator with guard pages
   let allocator = MacOsAllocatorBuilder::new()
       .with_maximum_pages(100)
       .with_guard_pages(true)
       .build()?;

QNX 7.1 (aarch64)
^^^^^^^^^^^^^^^^^^

* **Memory**: Advanced arena allocator with partitioning
* **Sync**: ``SyncCondvar`` APIs with priority inheritance
* **Features**:
  
  - Memory partitions for isolation
  - Arena-based allocation for determinism
  - Real-time priority support
  - See :doc:`qnx_platform` for details

.. code-block:: rust

   // QNX with memory partitioning
   let partition = QnxMemoryPartitionBuilder::new()
       .with_partition_name("wasm_runtime")
       .with_size(16 * 1024 * 1024)
       .with_flags(QnxPartitionFlags::LOCKED)
       .build()?;

VxWorks (x86_64, aarch64)
^^^^^^^^^^^^^^^^^^^^^^^^^^

* **Memory**: Memory partitions with both LKM and RTP context support
* **Sync**: VxWorks semaphores and message queues
* **Features**:
  
  - Loadable Kernel Module (LKM) execution
  - Real-Time Process (RTP) user space
  - Memory partition management
  - Real-time priority scheduling
  - Industrial-grade reliability

.. code-block:: rust

   // VxWorks with memory partitions
   let allocator = VxWorksAllocatorBuilder::new()
       .with_context(VxWorksContext::Rtp)
       .with_maximum_pages(128)
       .with_memory_partition("wasm_heap")
       .build()?;

Zephyr RTOS
^^^^^^^^^^^^

* **Memory**: ``k_mem_map``/``k_mem_unmap`` with memory domains
* **Sync**: ``k_futex_wait``/``k_futex_wake`` primitives
* **Features**:
  
  - Memory domain isolation
  - Guard regions
  - Real-time guarantees
  - Minimal overhead

.. code-block:: rust

   // Zephyr with memory domains
   let allocator = ZephyrAllocatorBuilder::new()
       .with_maximum_pages(64)
       .with_memory_domains(true)
       .with_guard_regions(true)
       .build()?;

Tock OS
^^^^^^^^

* **Memory**: Grant-based allocation with MPU enforcement
* **Sync**: IPC-based futex or semaphore fallback
* **Features**:
  
  - Hardware memory protection
  - Capability-based security
  - Static memory allocation
  - Formal verification support

.. code-block:: rust

   // Tock with full verification
   let allocator = TockAllocatorBuilder::new()
       .with_maximum_pages(32)
       .with_verification_level(VerificationLevel::Full)
       .build()?;

Fallback Implementation
^^^^^^^^^^^^^^^^^^^^^^^

* **Memory**: Static buffer allocation
* **Sync**: Spin-based futex
* **Features**:
  
  - No OS dependencies
  - Suitable for bare-metal
  - Configurable spin limits

Zero-Cost Platform Abstraction
-------------------------------

The platform layer includes a compile-time abstraction that supports different paradigms with zero runtime overhead:

**Platform Paradigms**

* **Posix**: Traditional OS with dynamic allocation (Linux, macOS, QNX)
* **SecurityFirst**: Static allocation with hardware isolation (Tock)
* **RealTime**: Deterministic behavior with priority support (Zephyr)
* **BareMetal**: Minimal abstraction for embedded systems

.. code-block:: rust

   // Compile-time platform selection
   use kiln_platform::{platform_select, PlatformConfig};
   
   // Auto-selects best paradigm based on features
   let platform = platform_select::create_auto_platform();
   
   // Or explicitly choose a paradigm
   let config = PlatformConfig::<paradigm::RealTime>::new()
       .with_max_pages(256)
       .with_rt_priority(10);
   let platform = RealtimePlatform::new(config);

Hardware Security Features
--------------------------

The platform layer provides abstractions for modern CPU security features:

ARM Architecture
^^^^^^^^^^^^^^^^

* **Pointer Authentication (PAC)**: Hardware pointer integrity
* **Memory Tagging Extension (MTE)**: Detect use-after-free and buffer overflows
* **Branch Target Identification (BTI)**: Control flow integrity
* **TrustZone**: Secure/non-secure world separation

.. code-block:: rust

   use kiln_platform::hardware_optimizations::arm::*;
   
   // Enable BTI for control flow integrity
   let bti = BranchTargetIdentification::enable()?;
   bti.optimize_memory(code_ptr, code_size)?;

Intel x86_64
^^^^^^^^^^^^

* **Control-flow Enforcement Technology (CET)**: Shadow stack and IBT
* **Memory Protection Keys (MPK)**: Fine-grained memory protection
* **TSX**: Hardware transactional memory
* **TXT**: Trusted execution technology

.. code-block:: rust

   use kiln_platform::hardware_optimizations::intel::*;
   
   // Enable CET shadow stack
   let cet = ControlFlowEnforcement::enable()?;

RISC-V
^^^^^^

* **Physical Memory Protection (PMP)**: Hardware memory access control
* **Control Flow Integrity (CFI)**: Landing pads and shadow stack
* **Cryptographic Extensions**: Hardware acceleration

.. code-block:: rust

   use kiln_platform::hardware_optimizations::riscv::*;
   
   // Configure PMP entries
   let pmp = PhysicalMemoryProtection::enable()?;

Advanced Synchronization
------------------------

The platform layer includes advanced synchronization primitives:

* **Lock-Free Allocator**: Wait-free memory allocation
* **Priority Inheritance Mutex**: Prevents priority inversion
* **Advanced RwLock**: Reader-writer lock with upgrades
* **Lock-Free Queues**: MPSC and SPSC implementations (requires alloc)

.. code-block:: rust

   use kiln_platform::advanced_sync::*;
   
   // Priority inheritance mutex for real-time systems
   let mutex = PriorityInheritanceMutex::new(data, Priority::new(10));

Runtime Detection
-----------------

The platform layer can detect capabilities at runtime:

.. code-block:: rust

   use kiln_platform::runtime_detection::PlatformDetector;
   
   let mut detector = PlatformDetector::new();
   let caps = detector.detect()?;
   
   println!("Page size: {}", caps.memory.page_size);
   println!("Has MTE: {}", caps.security.memory_tagging);
   println!("Recommended paradigm: {}", caps.recommended_paradigm());

Formal Verification
-------------------

The platform layer includes extensive formal verification annotations:

* **Memory Safety**: Verified bounds checking and lifetime management
* **Concurrency**: Data race freedom and deadlock prevention
* **Real-Time**: WCET analysis and priority inversion prevention
* **Security**: Information flow and side-channel resistance

.. code-block:: rust

   use kiln_platform::formal_verification::annotations::*;
   
   #[verified_memory_safe]
   #[constant_time]
   fn secure_allocate(size: usize) -> Result<*mut u8, Error> {
       // Implementation with formal guarantees
   }

Side-Channel Resistance
-----------------------

The platform layer provides defenses against timing and cache attacks:

* **Constant-Time Operations**: No data-dependent branches
* **Cache-Aware Allocation**: Minimize information leakage
* **Access Pattern Obfuscation**: Hide memory access patterns

.. code-block:: rust

   use kiln_platform::side_channel_resistance::*;
   
   // Constant-time memory comparison
   let equal = constant_time::compare_memory(ptr1, ptr2, size);
   
   // Cache-line aligned allocation
   let buffer = cache_aware_allocation::allocate_aligned(size)?;

Performance Validation
----------------------

Compile-time and runtime performance validation:

.. code-block:: rust

   use kiln_platform::performance_validation::*;
   
   // Compile-time checks
   CompileTimeValidator::validate_zero_cost();
   
   // Runtime benchmarking
   let validator = PerformanceValidator::new();
   let results = validator.benchmark_all()?;

Configuration Examples
----------------------

Common platform configurations:

.. code-block:: rust

   // High-security embedded system
   let secure_config = PlatformConfig::<paradigm::SecurityFirst>::new()
       .with_max_pages(128)
       .with_static_allocation(8 * 1024 * 1024)
       .with_isolation_level(IsolationLevel::Hardware);
   
   // Real-time control system
   let rt_config = PlatformConfig::<paradigm::RealTime>::new()
       .with_max_pages(64)
       .with_rt_priority(20);
   
   // Cloud deployment
   let cloud_config = PlatformConfig::<paradigm::Posix>::new()
       .with_max_pages(4096)
       .with_guard_pages(true);

Error Handling
--------------

All platform operations use consistent error handling:

.. code-block:: rust

   match allocator.allocate(10) {
       Ok(ptr) => println!("Allocated at {:?}", ptr),
       Err(e) if e.code() == kiln_error::codes::OUT_OF_MEMORY => {
           println!("Out of memory");
       }
       Err(e) => return Err(e),
   }

References
----------

* :doc:`qnx_platform` - QNX-specific platform features
* :doc:`hardening` - Security hardening features
* ARM Architecture Reference Manual - PAC/BTI/MTE specifications
* Intel 64 and IA-32 Architectures SDM - CET/MPK documentation
* RISC-V Privileged Architecture - PMP specification 