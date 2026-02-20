======================================
Synchronization Across Platforms
======================================

.. epigraph::

   "A mutex is just a fancy way of making threads wait in line. A futex is when you realize the line itself needs optimization."
   
   -- Kernel developer wisdom

Synchronization primitives are where platform differences get really interesting. From Linux futexes to QNX priority inheritance, from embedded semaphores to lock-free algorithms - let's explore how to keep your threads playing nicely together.

.. admonition:: What You'll Learn
   :class: note

   - Platform-specific synchronization primitives
   - The ``FutexLike`` trait and its implementations
   - Priority inheritance and real-time considerations
   - Lock-free programming with platform support
   - Debugging synchronization issues

The Universal Sync Interface 🔄
-------------------------------

Every platform provides the ``FutexLike`` trait - a minimal building block for higher-level synchronization:

.. code-block:: rust
   :caption: The foundation of all synchronization
   :linenos:

   pub trait FutexLike: Send + Sync {
       /// Load the current value atomically
       fn load(&self) -> u32;
       
       /// Store a value atomically
       fn store(&self, value: u32);
       
       /// Wait until the value changes from `expected`
       fn wait(&self, expected: u32) -> Result<(), Error>;
       
       /// Wait with a timeout
       fn wait_timeout(&self, expected: u32, timeout: Duration) 
           -> Result<TimeoutResult, Error>;
       
       /// Wake up to `count` waiters
       fn wake(&self, count: u32) -> Result<u32, Error>;
   }

Platform Implementations 🎯
--------------------------

Linux: The Futex Master
~~~~~~~~~~~~~~~~~~~~~~

Linux futexes are the gold standard - fast, flexible, and feature-rich:

.. code-block:: rust
   :caption: Linux futex with all features
   :linenos:

   use kiln_platform::{LinuxFutex, LinuxFutexBuilder};
   use std::time::Duration;
   
   fn create_linux_futex() -> Result<LinuxFutex, Error> {
       LinuxFutexBuilder::new()
           .with_initial_value(0)
           .with_private(true)        // Process-private for speed
           .with_realtime_clock(true) // Use CLOCK_REALTIME for waits
           .with_priority_inheritance(true) // PI-futex
           .build()
   }
   
   // Advanced: Robust futexes for crash recovery
   fn create_robust_futex() -> Result<LinuxFutex, Error> {
       LinuxFutexBuilder::new()
           .with_robust(true)
           .with_owner_died_callback(|futex| {
               println!("Owner died, recovering futex state");
               futex.store(0); // Reset to unlocked
           })
           .build()
   }

macOS: os_unfair_lock Under the Hood
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

macOS doesn't have futexes, but we can build something similar:

.. code-block:: rust
   :caption: macOS synchronization primitive
   :linenos:

   use kiln_platform::{MacOsFutex, MacOsFutexBuilder};
   
   fn create_macos_futex() -> Result<MacOsFutex, Error> {
       MacOsFutexBuilder::new()
           .with_initial_value(0)
           .with_spin_policy(SpinPolicy::Adaptive) // Spin briefly before sleeping
           .with_qos_class(QosClass::UserInteractive) // High priority
           .build()
   }
   
   // macOS-specific: unfair locks are... unfair!
   fn demonstrate_unfairness() {
       let futex = create_macos_futex().unwrap();
       
       // Unlike fair locks, os_unfair_lock doesn't guarantee FIFO
       // This is actually good for performance in many cases
       // But can lead to starvation if you're not careful
   }

QNX: Real-Time Priority Inheritance
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

QNX shines with its real-time synchronization features:

.. code-block:: rust
   :caption: QNX real-time synchronization
   :linenos:

   use kiln_platform::{QnxFutex, QnxFutexBuilder, QnxSyncPriority};
   
   fn create_qnx_realtime_futex() -> Result<QnxFutex, Error> {
       QnxFutexBuilder::new()
           .with_initial_value(0)
           .with_priority_ceiling(QnxSyncPriority::Ceiling(50))
           .with_priority_inheritance(true)
           .with_protocol(SyncProtocol::PriorityCeiling)
           .build()
   }
   
   // QNX-specific: Adaptive partitioning aware
   fn create_partition_aware_sync() -> Result<QnxFutex, Error> {
       QnxFutexBuilder::new()
           .with_partition("critical_partition")
           .with_runmask(0xFF) // Run on all CPUs
           .build()
   }

Embedded: Resource-Constrained Sync
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Embedded platforms need lightweight primitives:

.. code-block:: rust
   :caption: Embedded synchronization examples
   :linenos:

   // Zephyr: Using kernel semaphores
   use kiln_platform::{ZephyrFutex, ZephyrSemaphoreFutex};
   
   fn create_zephyr_sync() -> Result<ZephyrFutex, Error> {
       // Option 1: Direct futex-like primitive
       let futex = ZephyrFutexBuilder::new()
           .with_initial_value(0)
           .with_priority_boost(true) // Boost waiting threads
           .build()?;
       
       // Option 2: Semaphore-based (more efficient on Zephyr)
       let sema = ZephyrSemaphoreFutex::new(1); // Binary semaphore
       
       Ok(futex)
   }
   
   // Tock: IPC-based synchronization
   use kiln_platform::{TockFutex, TockFutexBuilder};
   
   fn create_tock_sync() -> Result<TockFutex, Error> {
       TockFutexBuilder::new()
           .with_initial_value(0)
           .with_ipc(true) // Use Tock's IPC for cross-process sync
           .with_grant_memory(true) // Allocate from grant region
           .build()
   }

Lock-Free Magic 🎩
-----------------

When you need maximum performance, go lock-free:

.. code-block:: rust
   :caption: Platform-optimized lock-free structures
   :linenos:

   use kiln_platform::advanced_sync::{
       LockFreeAllocator, 
       LockFreeMpscQueue,
       WaitFreeSpscQueue
   };
   
   // Lock-free memory allocator
   fn create_lockfree_allocator() -> Result<LockFreeAllocator, Error> {
       LockFreeAllocator::new()
           .with_size_classes(vec![64, 128, 256, 512, 1024])
           .with_cpu_cache_optimization(true)
           .build()
   }
   
   // Multi-producer, single-consumer queue
   fn create_mpsc_queue<T: Send>() -> Result<LockFreeMpscQueue<T>, Error> {
       LockFreeMpscQueue::new()
           .with_capacity(1024)
           .with_cacheline_padding(true) // Prevent false sharing
           .build()
   }
   
   // Wait-free SPSC queue (best performance)
   fn create_spsc_queue<T: Send>() -> Result<WaitFreeSpscQueue<T>, Error> {
       WaitFreeSpscQueue::new()
           .with_capacity(4096)
           .with_cpu_affinity(0, 1) // Pin producer to CPU 0, consumer to CPU 1
           .build()
   }

Advanced Patterns 🚀
-------------------

Priority Inheritance Mutex
~~~~~~~~~~~~~~~~~~~~~~~~~

Prevent priority inversion in real-time systems:

.. code-block:: rust
   :caption: Priority inheritance implementation
   :linenos:

   use kiln_platform::advanced_sync::{PriorityInheritanceMutex, Priority};
   
   fn priority_safe_critical_section() -> Result<(), Error> {
       let mutex = PriorityInheritanceMutex::new(42);
       
       // High priority thread
       std::thread::spawn(move || {
           let guard = mutex.lock_with_priority(Priority::HIGH);
           // Critical section - priority inherited by holder
           process_critical_data(*guard);
       });
       
       // Low priority thread holding the lock
       let guard = mutex.lock_with_priority(Priority::LOW);
       // Automatically boosted to HIGH priority while holding lock!
       
       Ok(())
   }

Reader-Writer Locks with Upgrades
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Advanced RwLock with upgrade capabilities:

.. code-block:: rust
   :caption: Upgradeable read locks
   :linenos:

   use kiln_platform::advanced_sync::AdvancedRwLock;
   
   fn upgradeable_reads() -> Result<(), Error> {
       let lock = AdvancedRwLock::new(vec![1, 2, 3]);
       
       // Start with a read lock
       let read_guard = lock.read()?;
       
       if read_guard.iter().any(|&x| x > 2) {
           // Upgrade to write lock (atomic operation!)
           let mut write_guard = read_guard.upgrade()?;
           write_guard.push(4);
       }
       
       Ok(())
   }

Platform-Specific Optimizations 🎯
----------------------------------

Spinning vs Sleeping
~~~~~~~~~~~~~~~~~~~

Different platforms have different optimal spin strategies:

.. code-block:: rust
   :caption: Adaptive spinning based on platform
   :linenos:

   use kiln_platform::sync::{SpinFutex, SpinFutexBuilder};
   
   fn create_adaptive_spinlock() -> Result<SpinFutex, Error> {
       let detector = PlatformDetector::new();
       let caps = detector.detect()?;
       
       let builder = SpinFutexBuilder::new();
       
       let spinlock = if caps.realtime.scheduling_precision_ns < 1000 {
           // High precision scheduling - aggressive spinning
           builder
               .with_spin_iterations(1000)
               .with_backoff_strategy(BackoffStrategy::Exponential)
               .build()
       } else if caps.memory.cpu_count > 8 {
           // Many cores - moderate spinning  
           builder
               .with_spin_iterations(100)
               .with_yield_after(50)
               .build()
       } else {
           // Few cores or low precision - minimal spinning
           builder
               .with_spin_iterations(10)
               .with_immediate_sleep(true)
               .build()
       };
       
       Ok(spinlock)
   }

NUMA-Aware Synchronization
~~~~~~~~~~~~~~~~~~~~~~~~~

Keep synchronization local to NUMA nodes:

.. code-block:: rust
   :caption: NUMA-aware locking
   :linenos:

   use kiln_platform::advanced_sync::NumaAwareMutex;
   
   fn numa_optimized_sync() -> Result<(), Error> {
       // Create per-NUMA-node mutexes
       let mutex = NumaAwareMutex::new()
           .with_numa_distribution(true)
           .with_local_spinning(true) // Spin on local node only
           .build()?;
       
       // Access prefers local NUMA node
       let guard = mutex.lock_on_node(numa_node_id())?;
       
       Ok(())
   }

Debugging Synchronization 🐛
---------------------------

Debug Futex with Tracking
~~~~~~~~~~~~~~~~~~~~~~~~

Track futex operations for debugging:

.. code-block:: rust
   :caption: Instrumented futex for debugging
   :linenos:

   #[cfg(debug_assertions)]
   fn create_debug_futex() -> Result<impl FutexLike, Error> {
       use kiln_platform::sync::DebugFutex;
       
       DebugFutex::new()
           .with_operation_tracking(true)
           .with_deadlock_detection(Duration::from_secs(5))
           .with_contention_reporting(true)
           .on_deadlock(|info| {
               eprintln!("Potential deadlock detected!");
               eprintln!("Waiters: {:?}", info.waiters);
               eprintln!("Hold time: {:?}", info.hold_duration);
           })
           .build()
   }

Best Practices 📚
-----------------

1. **Start Simple** - Use basic futexes before advanced features
2. **Measure Contention** - Profile before optimizing
3. **Avoid False Sharing** - Align to cache lines
4. **Test Priority Inversion** - Especially on real-time systems
5. **Use Lock-Free Carefully** - It's not always faster

Platform Pitfalls ⚠️
--------------------

**Linux:**
   - PI-futexes have overhead - use only when needed
   - Robust futexes require careful cleanup
   - NUMA effects can be dramatic

**macOS:**
   - No true futexes - higher overhead than Linux
   - Unfair locks can starve threads
   - QoS classes affect scheduling

**QNX:**
   - Priority ceiling must be chosen carefully
   - Partition budget affects lock hold time
   - Runmask limits can cause issues

**Embedded:**
   - Limited priority levels (typically 32)
   - No memory for wait queues
   - ISR constraints on primitives

.. admonition:: Golden Rule
   :class: tip

   The best synchronization is no synchronization. Design lock-free where possible, use coarse-grained locking where practical, and only reach for advanced primitives when profiling proves it necessary.

Next Steps 🎯
-------------

- Explore :doc:`platform_detection` to choose optimal primitives
- Learn about :doc:`performance_optimizations` for sync tuning
- Check out :doc:`hardware_security` for secure synchronization