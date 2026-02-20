======================================
Embedded Platform Support
======================================

.. epigraph::

   "640K ought to be enough for anybody... to run WebAssembly, if you're clever about it."
   
   -- Not Bill Gates, but definitely an embedded developer

Running WebAssembly on embedded systems is like fitting an elephant in a phone booth - it requires creativity, careful planning, and occasionally some magic. Kiln supports two major embedded platforms: Zephyr RTOS and Tock OS, each with unique constraints and capabilities.

.. admonition:: What You'll Learn
   :class: note

   - Zephyr RTOS integration and memory domains
   - Tock OS grants and process isolation
   - Static memory allocation strategies
   - Interrupt-safe synchronization
   - Power management considerations
   - Real-world embedded constraints

Memory: Every Byte Counts 💾
---------------------------

Static Allocation Patterns
~~~~~~~~~~~~~~~~~~~~~~~~~

On embedded systems, dynamic allocation is often forbidden:

.. code-block:: rust
   :caption: Static memory allocation
   :linenos:

   use kiln_platform::prelude::*;
   
   // Compile-time memory reservation
   #[link_section = ".wasm_memory"]
   static mut WASM_HEAP: [u8; 256 * 1024] = [0; 256 * 1024]; // 256KB
   
   #[link_section = ".wasm_stack"]  
   static mut WASM_STACK: [u8; 16 * 1024] = [0; 16 * 1024];  // 16KB
   
   fn create_static_allocator() -> Result<impl PageAllocator, Error> {
       // Use the static buffer
       let heap_ptr = unsafe { WASM_HEAP.as_mut_ptr() };
       let heap_size = unsafe { WASM_HEAP.len() };
       
       StaticAllocatorBuilder::new()
           .with_buffer(heap_ptr, heap_size)
           .with_alignment(16)  // ARM Cortex-M alignment
           .with_guard_regions(true)  // Use MPU if available
           .build()
   }
   
   // Memory pool pattern for deterministic allocation
   const POOL_SIZES: &[usize] = &[64, 128, 256, 512, 1024];
   const POOL_COUNTS: &[usize] = &[32, 16, 8, 4, 2];
   
   fn create_pool_allocator() -> Result<PoolAllocator, Error> {
       let mut pools = PoolAllocator::new();
       
       for (size, count) in POOL_SIZES.iter().zip(POOL_COUNTS.iter()) {
           pools.add_pool(*size, *count)?;
       }
       
       pools.build()
   }

Memory Protection Units
~~~~~~~~~~~~~~~~~~~~~~

Use hardware MPU for isolation:

.. code-block:: rust
   :caption: MPU configuration
   :linenos:

   use kiln_platform::embedded_mpu::{MpuRegion, AccessPermission};
   
   fn configure_mpu_for_wasm() -> Result<(), Error> {
       let mpu = MpuController::new()?;
       
       // Check capabilities
       let regions = mpu.available_regions();
       println!("MPU regions available: {}", regions);
       
       // Region 0: WASM code (read-only, executable)
       mpu.configure_region(0, MpuRegion {
           base_address: WASM_CODE_BASE,
           size: WASM_CODE_SIZE,
           permissions: AccessPermission::ReadExecute,
           attributes: MemoryAttribute::Normal,
           shareable: false,
       })?;
       
       // Region 1: WASM heap (read-write, no execute)
       mpu.configure_region(1, MpuRegion {
           base_address: WASM_HEAP_BASE,
           size: WASM_HEAP_SIZE,
           permissions: AccessPermission::ReadWrite,
           attributes: MemoryAttribute::Normal,
           shareable: false,
       })?;
       
       // Region 2: Guard page (no access)
       mpu.configure_region(2, MpuRegion {
           base_address: GUARD_PAGE_BASE,
           size: 4096,
           permissions: AccessPermission::None,
           attributes: MemoryAttribute::Device,
           shareable: false,
       })?;
       
       mpu.enable()?;
       
       Ok(())
   }

Zephyr RTOS Integration 🦓
--------------------------

Memory Domains
~~~~~~~~~~~~~

Zephyr's memory domain system for isolation:

.. code-block:: rust
   :caption: Zephyr memory domains
   :linenos:

   use kiln_platform::{
       ZephyrAllocator, 
       ZephyrAllocatorBuilder,
       ZephyrMemoryFlags
   };
   
   fn setup_zephyr_memory() -> Result<ZephyrAllocator, Error> {
       // Create allocator with Zephyr-specific features
       let allocator = ZephyrAllocatorBuilder::new()
           .with_maximum_pages(32)  // 2MB max (embedded constraint)
           .with_memory_domain("wasm_domain")
           .with_partition("wasm_code", 64 * 1024)   // 64KB code
           .with_partition("wasm_data", 128 * 1024)  // 128KB data
           .with_flags(
               ZephyrMemoryFlags::NOCACHE |  // Disable caching
               ZephyrMemoryFlags::USER       // User mode access
           )
           .with_guard_regions(true)
           .build()?;
       
       // Add current thread to domain
       allocator.add_thread_to_domain(k_current_get())?;
       
       Ok(allocator)
   }
   
   // Dynamic stack allocation for WASM
   fn allocate_wasm_stack() -> Result<*mut u8, Error> {
       use zephyr_sys::{k_thread_stack_alloc, K_THREAD_STACK_DEFINE};
       
       const WASM_STACK_SIZE: usize = 8192;
       
       // Allocate from thread stack pool
       let stack = unsafe {
           k_thread_stack_alloc(
               WASM_STACK_SIZE,
               K_USER | K_ESSENTIAL
           )
       };
       
       if stack.is_null() {
           return Err(Error::OutOfMemory);
       }
       
       Ok(stack)
   }

Zephyr Synchronization
~~~~~~~~~~~~~~~~~~~~~

Kernel primitives for thread safety:

.. code-block:: rust
   :caption: Zephyr synchronization
   :linenos:

   use kiln_platform::{ZephyrFutex, ZephyrSemaphoreFutex};
   
   fn create_zephyr_sync() -> Result<(), Error> {
       // Option 1: Semaphore-based futex (more efficient)
       let sema_futex = ZephyrSemaphoreFutex::new(1);
       
       // Option 2: Direct futex implementation
       let futex = ZephyrFutexBuilder::new()
           .with_priority_inheritance(true)
           .with_timeout_order(TimeoutOrder::Absolute)
           .build()?;
       
       // ISR-safe spinlock for interrupt contexts
       let spinlock = ZephyrSpinlock::new();
       
       // In ISR context
       let key = spinlock.lock_irqsave();
       // Critical section...
       spinlock.unlock_irqrestore(key);
       
       Ok(())
   }

Power Management
~~~~~~~~~~~~~~~

Integrate with Zephyr's power management:

.. code-block:: rust
   :caption: Power-aware WASM execution
   :linenos:

   use kiln_platform::zephyr_power::{
       PowerManager,
       PowerState,
       PowerConstraint
   };
   
   fn power_aware_execution() -> Result<(), Error> {
       let pm = PowerManager::new();
       
       // Prevent deep sleep during WASM execution
       let constraint = pm.set_constraint(PowerConstraint::MinimumState(
           PowerState::Runtime
       ))?;
       
       // Execute WASM
       execute_wasm_module()?;
       
       // Release constraint
       drop(constraint);
       
       // Register suspend/resume handlers
       pm.register_suspend_handler(|| {
           // Save WASM execution state
           save_execution_context()?;
           Ok(())
       })?;
       
       pm.register_resume_handler(|| {
           // Restore WASM execution state
           restore_execution_context()?;
           Ok(())
       })?;
       
       Ok(())
   }

Tock OS Integration 🔒
---------------------

Grant-Based Memory
~~~~~~~~~~~~~~~~~

Tock's unique grant system for process isolation:

.. code-block:: rust
   :caption: Tock grant allocation
   :linenos:

   use kiln_platform::{TockAllocator, TockAllocatorBuilder};
   use tock_registers::interfaces::Readable;
   
   fn setup_tock_grants() -> Result<TockAllocator, Error> {
       let allocator = TockAllocatorBuilder::new()
           .with_grant_count(4)  // Number of grant regions
           .with_grant_size(16 * 1024)  // 16KB per grant
           .with_allow_regions(vec![
               AllowRegion::ReadOnly(0x1000, 0x2000),
               AllowRegion::ReadWrite(0x2000, 0x3000),
           ])
           .build()?;
       
       // Allocate from grant
       let grant = allocator.allocate_grant(0)?;
       
       // Use grant memory
       grant.enter(|memory| {
           // This closure runs with access to grant memory
           let wasm_data = memory.as_mut_slice();
           process_wasm_in_grant(wasm_data)
       })?;
       
       Ok(allocator)
   }

Process Isolation
~~~~~~~~~~~~~~~~

Tock's capability-based security:

.. code-block:: rust
   :caption: Tock process isolation
   :linenos:

   use kiln_platform::tock_process::{
       ProcessId,
       Capability,
       Syscall
   };
   
   fn setup_isolated_wasm_process() -> Result<(), Error> {
       // Create new process for WASM
       let process = Process::create("wasm_runtime")?;
       
       // Grant minimal capabilities
       process.grant_capabilities(&[
           Capability::MemoryAllocate,
           Capability::Timer,
           // No GPIO, UART, etc.
       ])?;
       
       // Set up syscall filter
       process.set_syscall_filter(|syscall| {
           match syscall {
               Syscall::Yield => true,
               Syscall::Subscribe(_) => true,
               Syscall::Command(driver, cmd, _, _) => {
                   // Only allow specific drivers
                   driver == TIMER_DRIVER && cmd <= 2
               },
               _ => false,
           }
       })?;
       
       // Load WASM into process
       process.load_binary(wasm_binary)?;
       
       // Start execution
       process.start()?;
       
       Ok(())
   }

IPC Communication
~~~~~~~~~~~~~~~~

Inter-process communication in Tock:

.. code-block:: rust
   :caption: Tock IPC
   :linenos:

   use kiln_platform::{TockIpc, IpcService};
   
   fn setup_wasm_ipc_service() -> Result<(), Error> {
       // Register IPC service
       let service = IpcService::register("wasm_service")?;
       
       // Handle incoming IPC
       service.on_notify(|client_id, notify_val| {
           match notify_val {
               0 => {
                   // Load WASM module request
                   let buffer = service.get_shared_buffer(client_id)?;
                   load_module_from_buffer(buffer)?;
               },
               1 => {
                   // Execute function request  
                   let result = execute_wasm_function()?;
                   service.notify_client(client_id, result)?;
               },
               _ => {
                   service.notify_client(client_id, ERROR_INVALID)?;
               }
           }
           Ok(())
       })?;
       
       Ok(())
   }

Common Embedded Patterns 🎯
--------------------------

Interrupt-Safe Execution
~~~~~~~~~~~~~~~~~~~~~~~

Handle WASM in interrupt contexts:

.. code-block:: rust
   :caption: ISR-safe WASM execution
   :linenos:

   use kiln_platform::embedded_common::{InterruptGuard, CriticalSection};
   
   fn interrupt_safe_wasm() -> Result<(), Error> {
       // Non-blocking execution check
       static WASM_READY: AtomicBool = AtomicBool::new(false);
       
       // In interrupt handler
       #[no_mangle]
       extern "C" fn timer_isr() {
           if WASM_READY.load(Ordering::Acquire) {
               // Schedule deferred execution
               schedule_wasm_execution();
           }
       }
       
       // Deferred execution in thread context
       fn execute_wasm_deferred() -> Result<(), Error> {
           let _guard = InterruptGuard::new();
           
           // Critical section for shared resources
           critical_section::with(|_| {
               update_wasm_state()?;
               Ok(())
           })?;
           
           // Run WASM with interrupts enabled
           execute_wasm_function()?;
           
           Ok(())
       }
       
       Ok(())
   }

Watchdog Integration
~~~~~~~~~~~~~~~~~~~

Keep the watchdog happy during long WASM execution:

.. code-block:: rust
   :caption: Watchdog handling
   :linenos:

   use kiln_platform::embedded_watchdog::{Watchdog, WatchdogConfig};
   
   fn wasm_with_watchdog() -> Result<(), Error> {
       let watchdog = Watchdog::init(WatchdogConfig {
           timeout_ms: 1000,
           window_ms: Some(100),  // Windowed watchdog
       })?;
       
       // Start watchdog
       watchdog.start()?;
       
       // Execute WASM with periodic feeding
       let mut last_feed = Instant::now();
       
       loop {
           // Execute one WASM instruction
           let more = step_wasm_execution()?;
           
           // Feed watchdog periodically
           if last_feed.elapsed() > Duration::from_millis(500) {
               watchdog.feed()?;
               last_feed = Instant::now();
           }
           
           if !more {
               break;
           }
       }
       
       watchdog.stop()?;
       
       Ok(())
   }

Flash Memory Execution
~~~~~~~~~~~~~~~~~~~~~

Execute WASM directly from flash:

.. code-block:: rust
   :caption: XIP (Execute in Place)
   :linenos:

   use kiln_platform::embedded_flash::{FlashRegion, XipConfig};
   
   fn setup_xip_wasm() -> Result<(), Error> {
       // Configure flash for XIP
       let flash = FlashRegion::new(0x0800_0000, 256 * 1024)?;
       
       flash.configure_xip(XipConfig {
           cache_enable: true,
           prefetch_enable: true,
           wait_states: 2,  // Depends on CPU frequency
       })?;
       
       // Map WASM module in flash
       let module = unsafe {
           WasmModule::from_flash(
               flash.as_ptr(),
               flash.size()
           )?
       };
       
       // Validate without copying to RAM
       module.validate_in_place()?;
       
       // Execute directly from flash
       module.execute_xip()?;
       
       Ok(())
   }

Resource Constraints 📊
----------------------

Embedded Optimization Strategies:

.. code-block:: rust
   :caption: Resource optimization
   :linenos:

   use kiln_platform::embedded_optimizations::*;
   
   fn optimize_for_embedded() -> Result<(), Error> {
       // 1. Disable features to save space
       let config = WasmConfig::new()
           .disable_float_support()    // No FPU
           .disable_simd()            // No SIMD
           .disable_threads()         // Single core
           .max_memory_pages(8)       // 512KB max
           .max_table_elements(100)   // Small tables
           .max_functions(50)         // Limited functions
           .build()?;
       
       // 2. Use compact instruction encoding
       let module = compile_wasm_compact(wasm_bytes, &config)?;
       
       // 3. Share code between instances
       let shared_code = Arc::new(module.code_section());
       
       // 4. Use 16-bit addressing where possible
       if cfg!(target_pointer_width = "16") {
           use_16bit_pointers()?;
       }
       
       Ok(())
   }

Best Practices 📚
-----------------

1. **Measure Everything** - RAM, flash, and CPU cycles
2. **Static Over Dynamic** - Allocate at compile time
3. **Interrupt Awareness** - Design for ISR constraints
4. **Power Consciousness** - Every instruction costs energy
5. **Fail Gracefully** - Limited resources mean frequent failures

Platform Comparison 🔄
---------------------

.. list-table:: Embedded Platform Features
   :header-rows: 1
   :widths: 30 35 35

   * - Feature
     - Zephyr RTOS
     - Tock OS
   * - Memory Model
     - Memory domains, MPU
     - Grants, process isolation
   * - Scheduling
     - Preemptive, priority-based
     - Cooperative, time-sliced
   * - Min RAM
     - ~8KB kernel + app
     - ~64KB kernel + apps
   * - Architecture
     - Monolithic
     - Microkernel
   * - Safety Focus
     - Real-time guarantees
     - Security isolation
   * - Power Management
     - Integrated PM subsystem
     - App-driven
   * - Use Cases
     - IoT devices, sensors
     - Security-critical embedded

.. admonition:: Resource Budget Example
   :class: note

   Typical embedded WASM budget:
   
   - Flash: 256KB total (128KB WASM, 128KB runtime)
   - RAM: 64KB total (32KB heap, 16KB stack, 16KB runtime)
   - CPU: 100MHz Cortex-M4
   - Power: 10mA average current
   
   With careful optimization, this can run useful WASM modules!

Next Steps 🎯
-------------

- Learn about :doc:`memory_management` for embedded constraints
- Explore :doc:`performance_optimizations` for resource-limited systems
- Check out :doc:`hardware_security` for embedded security features