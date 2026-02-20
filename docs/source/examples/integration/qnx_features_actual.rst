======================================
QNX Platform Features (Implemented)
======================================

.. note::

   This document describes the QNX features that are **actually implemented** 
   in Kiln. For a vision of potential future features, see the original 
   qnx_features.rst document.

QNX Memory Management 🎯
-----------------------

Kiln leverages QNX's memory management for WebAssembly isolation:

.. code-block:: rust
   :caption: QNX memory allocation with mmap
   :linenos:

   use kiln_platform::{QnxAllocator, QnxAllocatorBuilder};
   
   // Create allocator with guard pages
   let allocator = QnxAllocatorBuilder::new()
       .with_guard_pages(true)
       .with_data_protection(QnxProtFlags::ReadWrite)
       .build();
   
   // Allocate WASM memory with protection
   let (ptr, size) = allocator.allocate(64, Some(256))?;

Memory Partitioning 🔒
---------------------

Implemented for resource isolation:

.. code-block:: rust
   :caption: QNX memory partitions
   :linenos:

   use kiln_platform::{QnxMemoryPartition, QnxMemoryPartitionBuilder};
   
   // Create isolated partition for WASM execution
   let partition = QnxMemoryPartitionBuilder::new()
       .with_name("wasm_sandbox")
       .with_flags(QnxPartitionFlags::MemoryIsolation)
       .with_memory_size(32 * 1024 * 1024, 64 * 1024 * 1024, 8 * 1024 * 1024)
       .build()?;
   
   // Execute within partition
   partition.with_partition(|| {
       // WASM execution happens here with memory limits enforced
       execute_wasm_module()
   })?;

Real-Time Threading 🚀
---------------------

Platform-aware thread pool with QNX scheduling:

.. code-block:: rust
   :caption: QNX real-time thread execution
   :linenos:

   use kiln_platform::threading::{ThreadPoolConfig, ThreadPriority};
   use kiln_platform::qnx_threading::QnxThreadPool;
   
   // Configure thread pool with RT priorities
   let config = ThreadPoolConfig {
       max_threads: 16,
       priority_range: (ThreadPriority::Low, ThreadPriority::Realtime),
       cpu_affinity: Some(CpuSet::from_mask(0xF)), // CPUs 0-3
       stack_size: 2 * 1024 * 1024,
       ..Default::default()
   };
   
   let pool = QnxThreadPool::new(config)?;
   
   // Spawn WASM thread with real-time priority
   let task = WasmTask {
       id: 1,
       function_id: 100,
       args: vec![],
       priority: ThreadPriority::High,
       cpu_affinity: Some(CpuSet::from_mask(0x1)), // Pin to CPU 0
       ..Default::default()
   };
   
   let handle = pool.spawn_wasm_thread(task)?;

Synchronization Primitives 🔄
----------------------------

QNX pulse-based synchronization:

.. code-block:: rust
   :caption: QNX futex implementation
   :linenos:

   use kiln_platform::{QnxFutex, QnxFutexBuilder};
   
   // Create QNX futex with priority inheritance
   let futex = QnxFutexBuilder::new()
       .with_priority(QnxSyncPriority::High)
       .build()?;
   
   // Use for thread synchronization
   futex.wait(expected_value, Some(timeout_ms))?;
   futex.wake_one()?;

Software Watchdog 🐕
-------------------

Lightweight monitoring for WASM execution:

.. code-block:: rust
   :caption: Software watchdog for hang detection
   :linenos:

   use kiln_platform::watchdog::{SoftwareWatchdog, WatchdogConfig};
   
   // Configure watchdog
   let watchdog = SoftwareWatchdog::new(WatchdogConfig {
       default_timeout: Duration::from_secs(60),
       check_interval: Duration::from_millis(100),
       auto_kill: false,
       ..Default::default()
   });
   
   watchdog.start()?;
   
   // Watch WASM execution
   let handle = watchdog.watch_wasm_execution(
       "critical_module",
       Duration::from_secs(30)
   )?;
   
   // Send periodic heartbeats during execution
   loop {
       handle.heartbeat()?;
       // Do work...
   }

Thread Health Monitoring 🏥
--------------------------

Built into the safe threading system:

.. code-block:: rust
   :caption: Thread health monitoring
   :linenos:

   use kiln_component::builtins::safe_threading;
   
   // Health check all threads
   let health_results = thread_manager.health_check()?;
   
   for (thread_id, health) in health_results {
       match health {
           ThreadHealth::Healthy => continue,
           ThreadHealth::CpuQuotaExceeded => {
               eprintln!("Thread {} using too much CPU", thread_id);
           }
           ThreadHealth::LifetimeExceeded => {
               eprintln!("Thread {} exceeded lifetime", thread_id);
           }
           ThreadHealth::Unresponsive => {
               eprintln!("Thread {} not responding", thread_id);
           }
           _ => {}
       }
   }
   
   // Automatically kill unhealthy threads
   let killed_count = thread_manager.kill_unhealthy_threads()?;

Arena-Based Allocation 📦
------------------------

QNX arena allocator for efficient memory use:

.. code-block:: rust
   :caption: QNX arena allocation
   :linenos:

   use kiln_platform::{QnxArenaAllocator, QnxArenaAllocatorBuilder};
   
   // Configure arena allocator
   let allocator = QnxArenaAllocatorBuilder::new()
       .with_arena_size(64 * 1024)     // 64KB arenas
       .with_arena_cache_max_blocks(8)
       .with_memory_hold(true)         // Keep memory for reuse
       .build()?;
   
   // Efficient allocation within arenas
   let (ptr, size) = allocator.allocate(pages, max_pages)?;

Integration Example 🔧
---------------------

Complete example using implemented features:

.. code-block:: rust
   :caption: Production QNX configuration
   :linenos:

   use kiln_platform::*;
   use kiln_component::builtins::safe_threading::*;
   
   fn setup_qnx_wasm_runtime() -> Result<WasmRuntime> {
       // 1. Create memory partition
       let partition = QnxMemoryPartitionBuilder::new()
           .with_name("wasm_runtime")
           .with_flags(QnxPartitionFlags::MemoryIsolation)
           .with_memory_size(128 * MB, 256 * MB, 32 * MB)
           .build()?;
       
       // 2. Configure thread pool
       let thread_config = ThreadPoolConfig {
           max_threads: 32,
           priority_range: (ThreadPriority::Low, ThreadPriority::High),
           memory_limit_per_thread: Some(8 * MB),
           stack_size: 2 * MB,
           name_prefix: "wasm-worker",
           ..Default::default()
       };
       
       // 3. Set up watchdog
       let watchdog = SoftwareWatchdog::new(WatchdogConfig {
           default_timeout: Duration::from_secs(120),
           check_interval: Duration::from_millis(100),
           ..Default::default()
       });
       watchdog.start()?;
       
       // 4. Create thread manager with limits
       let limits = ThreadingLimits {
           max_threads_per_module: 16,
           max_total_threads: 64,
           max_thread_lifetime: Duration::from_secs(300),
           cpu_quota_per_thread: Duration::from_secs(60),
           memory_limit_per_module: 64 * MB,
       };
       
       let thread_manager = WasmThreadManager::new(
           thread_config,
           limits,
           executor
       )?;
       
       // 5. Register module with safety constraints
       thread_manager.register_module(WasmModuleInfo {
           id: 1,
           name: "critical_module".to_string(),
           max_threads: 8,
           memory_limit: 32 * MB,
           cpu_quota: Duration::from_secs(30),
           default_priority: ThreadPriority::Normal,
       })?;
       
       Ok(WasmRuntime {
           partition,
           thread_manager,
           watchdog,
       })
   }

What's NOT Implemented ❌
------------------------

The following features from the original documentation are **not** implemented
as they don't align with Kiln's architecture:

1. **QNX Message Passing IPC** - All components run in-process
2. **QNX Resource Manager (/dev/wasm)** - Direct API access instead
3. **Hardware Watchdog** - Software watchdog is sufficient
4. **High Availability Manager (HAM)** - WASM modules are short-lived
5. **System Profiler Integration** - Use standard profiling tools

Future Considerations 🔮
-----------------------

If multi-process WASM execution becomes necessary:

- IPC could be added using the generic `ipc.rs` traits
- HAM integration could provide process-level recovery
- Resource Manager could expose WASM as a system service

But currently, these would add complexity without clear benefits.