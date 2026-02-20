===============
VxWorks Support
===============

VxWorks is a real-time operating system widely used in industrial automation, aerospace, defense, and telecommunications. Kiln provides comprehensive support for VxWorks with optimizations for both kernel and user space execution.

.. contents:: On this page
   :local:
   :depth: 2

Overview
--------

VxWorks support in Kiln includes:

- **Dual Execution Contexts**: Support for both LKM (Loadable Kernel Module) and RTP (Real-Time Process) contexts
- **Memory Partitioning**: Advanced memory management with partition support
- **Real-Time Features**: Priority-based scheduling and deterministic execution
- **Industrial Reliability**: Designed for mission-critical applications

Execution Contexts
------------------

LKM (Loadable Kernel Module)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

LKM execution runs in kernel space, providing:

- Direct hardware access
- Lowest latency
- Shared memory with kernel
- Elevated privileges

.. code-block:: rust

   use kiln_platform::vxworks_memory::{VxWorksAllocator, VxWorksContext};

   let allocator = VxWorksAllocatorBuilder::new()
       .with_context(VxWorksContext::Lkm)
       .with_maximum_pages(256)
       .build()?;

RTP (Real-Time Process)
~~~~~~~~~~~~~~~~~~~~~~~

RTP execution runs in user space, providing:

- Memory protection
- Process isolation
- Standard POSIX APIs
- Fault containment

.. code-block:: rust

   use kiln_platform::vxworks_memory::{VxWorksAllocator, VxWorksContext};

   let allocator = VxWorksAllocatorBuilder::new()
       .with_context(VxWorksContext::Rtp)
       .with_maximum_pages(128)
       .with_memory_partition("wasm_heap")
       .build()?;

Memory Management
-----------------

VxWorks memory management leverages Wind River's memory partitioning:

.. code-block:: rust

   use kiln_platform::vxworks_memory::VxWorksMemoryPartition;

   // Create dedicated memory partition for WebAssembly
   let partition = VxWorksMemoryPartition::create(
       "wasm_runtime",
       16 * 1024 * 1024,  // 16 MB
       VxWorksPartitionFlags::USER_ACCESSIBLE
   )?;

   let allocator = VxWorksAllocatorBuilder::new()
       .with_memory_partition_id(partition.id())
       .with_maximum_pages(256)
       .build()?;

Synchronization
---------------

VxWorks synchronization uses native semaphores and message queues:

.. code-block:: rust

   use kiln_platform::vxworks_sync::VxWorksFutex;

   let futex = VxWorksFutexBuilder::new()
       .with_priority_inheritance(true)
       .with_timeout(Duration::from_millis(100))
       .build()?;

Threading
---------

VxWorks threading leverages Wind River's priority-based scheduler:

.. code-block:: rust

   use kiln_platform::vxworks_threading::{VxWorksThread, VxWorksThreadConfig};

   let config = VxWorksThreadConfig {
       priority: 100,
       stack_size: 64 * 1024,
       name: "wasm_worker",
       affinity_mask: 0x1,  // CPU 0
   };

   let thread = VxWorksThread::spawn(config, || {
       // WebAssembly execution
   })?;

Build Configuration
-------------------

Building for VxWorks requires the Wind River development environment:

.. code-block:: bash

   # Set VxWorks environment
   export WIND_HOME=/opt/windriver
   export WIND_BASE=$WIND_HOME/vxworks-7

   # Build for VxWorks target
   cargo build --target=aarch64-wrs-vxworks \
       --features="platform-vxworks,real-time"

Target Support
--------------

Supported VxWorks targets:

- **x86_64-wrs-vxworks** - Intel x64 architecture
- **aarch64-wrs-vxworks** - ARM 64-bit architecture
- **armv7-wrs-vxworks** - ARM 32-bit architecture

Configuration Examples
----------------------

High-Performance Configuration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

For maximum throughput:

.. code-block:: rust

   let config = VxWorksRuntimeConfig {
       context: VxWorksContext::Lkm,
       memory_pages: 512,
       thread_priority: 50,
       enable_cache_optimization: true,
       enable_dma_optimization: true,
   };

Safety-Critical Configuration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

For safety-critical applications:

.. code-block:: rust

   let config = VxWorksRuntimeConfig {
       context: VxWorksContext::Rtp,
       memory_pages: 128,
       thread_priority: 200,
       enable_memory_protection: true,
       enable_fault_isolation: true,
       verification_level: VerificationLevel::Full,
   };

Troubleshooting
---------------

Common Issues
~~~~~~~~~~~~~

1. **Memory Allocation Failures**
   - Increase system memory partition size
   - Check memory fragmentation
   - Verify partition permissions

2. **Priority Inversion**
   - Enable priority inheritance
   - Adjust thread priorities
   - Use appropriate synchronization primitives

3. **Real-Time Violations**
   - Monitor interrupt latency
   - Check system load
   - Optimize critical paths

Performance Tuning
~~~~~~~~~~~~~~~~~~

- Use LKM context for lowest latency
- Pin threads to specific CPU cores
- Enable hardware acceleration where available
- Use memory-mapped I/O for high-throughput operations

Further Reading
---------------

- `VxWorks Programmer's Guide <https://docs.windriver.com/>`_
- `Real-Time Programming Best Practices <https://www.windriver.com/resource-library>`_
- `Kiln Performance Optimization Guide <../examples/platform/performance_optimizations.html>`_