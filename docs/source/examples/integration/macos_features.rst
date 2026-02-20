======================================
macOS Platform Features
======================================

.. epigraph::

   "macOS: It's UNIX! (Sort of. When it feels like it. Terms and conditions apply.)"
   
   -- Every developer trying to port Linux code

macOS brings its own unique blend of BSD heritage, Mach microkernel, and Apple's special sauce. While it may frustrate systems programmers used to Linux, it also offers some unique capabilities. Let's explore how Kiln makes the most of Apple's platform.

.. admonition:: What You'll Learn
   :class: note

   - Mach VM system and memory management
   - Grand Central Dispatch integration
   - Hypervisor.framework for isolation
   - Metal compute shaders for WASM
   - macOS security features (sandboxing, entitlements)
   - Development and debugging tools

The Mach VM System 🍎
--------------------

Understanding macOS Memory
~~~~~~~~~~~~~~~~~~~~~~~~~

macOS uses the Mach microkernel for virtual memory:

.. code-block:: rust
   :caption: Mach VM operations
   :linenos:

   use kiln_platform::{MacOsAllocator, MacOsAllocatorBuilder};
   use kiln_platform::macos_memory::{VmFlags, VmProt, VmInherit};
   
   fn create_mach_vm_allocator() -> Result<MacOsAllocator, Error> {
       MacOsAllocatorBuilder::new()
           .with_maximum_pages(2048)
           .with_guard_pages(true)
           .with_vm_flags(
               VmFlags::PURGABLE |      // Can be purged under pressure
               VmFlags::RANDOM_ADDR |   // ASLR
               VmFlags::NO_CACHE       // Bypass buffer cache
           )
           .with_vm_protection(VmProt::READ | VmProt::WRITE)
           .with_vm_inherit(VmInherit::NONE)  // Don't inherit on fork
           .build()
   }
   
   // macOS-specific: using vm_copy for fast cloning
   fn fast_memory_copy(allocator: &MacOsAllocator) -> Result<(), Error> {
       let (src_ptr, size) = allocator.allocate(100, None)?;
       let (dst_ptr, _) = allocator.allocate(100, None)?;
       
       // vm_copy is optimized for large copies
       allocator.vm_copy(src_ptr, dst_ptr, size)?;
       
       // For really large regions, vm_remap with copy-on-write
       let cow_ptr = allocator.vm_remap(
           src_ptr, 
           size, 
           VmFlags::COPY_ON_WRITE
       )?;
       
       Ok(())
   }

Purgeable Memory
~~~~~~~~~~~~~~~

macOS's unique memory pressure handling:

.. code-block:: rust
   :caption: Purgeable memory for caches
   :linenos:

   use kiln_platform::macos_memory::{PurgeableState, Volatility};
   
   fn create_purgeable_cache() -> Result<(), Error> {
       let allocator = MacOsAllocatorBuilder::new()
           .with_purgeable_behavior(true)
           .build()?;
       
       // Allocate purgeable memory
       let (ptr, size) = allocator.allocate_purgeable(1024)?;
       
       // Mark as purgeable when not in use
       allocator.set_purgeable_state(ptr, PurgeableState::Volatile)?;
       
       // Before use, make non-volatile
       let was_purged = allocator.set_purgeable_state(
           ptr, 
           PurgeableState::NonVolatile
       )?;
       
       if was_purged {
           println!("Memory was purged - need to regenerate cache");
           regenerate_cache(ptr, size)?;
       }
       
       // Use the memory...
       
       // Mark as purgeable again
       allocator.set_purgeable_state(ptr, PurgeableState::Volatile)?;
       
       Ok(())
   }

Memory Pressure Notifications
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Respond to system memory pressure:

.. code-block:: rust
   :caption: Memory pressure handling
   :linenos:

   use kiln_platform::macos_memory::{MemoryPressureHandler, PressureLevel};
   
   fn setup_memory_pressure_handling() -> Result<(), Error> {
       let handler = MemoryPressureHandler::new();
       
       handler.on_pressure(|level| {
           match level {
               PressureLevel::Normal => {
                   // System memory is fine
               },
               PressureLevel::Warning => {
                   println!("Memory pressure warning");
                   // Reduce cache sizes
                   shrink_caches(50); // Reduce by 50%
               },
               PressureLevel::Urgent => {
                   println!("Memory pressure urgent!");
                   // Drop all non-essential memory
                   drop_all_caches();
                   // Mark memory as purgeable
                   mark_all_purgeable();
               },
               PressureLevel::Critical => {
                   println!("Memory pressure CRITICAL!");
                   // Emergency measures
                   emergency_memory_release();
               }
           }
       })?;
       
       handler.start()?;
       
       Ok(())
   }

Grand Central Dispatch 🚦
------------------------

Integrate with macOS's concurrency system:

.. code-block:: rust
   :caption: GCD integration
   :linenos:

   use kiln_platform::macos_sync::{
       DispatchQueue, 
       DispatchGroup,
       DispatchSemaphore,
       QosClass
   };
   
   fn setup_gcd_execution() -> Result<(), Error> {
       // Create queue for WASM execution
       let queue = DispatchQueue::create("com.kiln.wasm.execution")
           .with_qos(QosClass::UserInitiated)
           .with_concurrent(true)
           .build()?;
       
       // Group for coordinating multiple WASM modules
       let group = DispatchGroup::new();
       
       // Execute multiple WASM modules concurrently
       for module in modules {
           group.enter();
           queue.async(move || {
               execute_wasm_module(module);
               group.leave();
           })?;
       }
       
       // Wait for all to complete
       group.wait()?;
       
       // Or use notify for async completion
       group.notify(queue, || {
           println!("All WASM modules completed");
       })?;
       
       Ok(())
   }
   
   // Using dispatch semaphores for rate limiting
   fn rate_limited_execution() -> Result<(), Error> {
       let semaphore = DispatchSemaphore::new(3); // Max 3 concurrent
       
       for task in tasks {
           semaphore.wait();
           
           DispatchQueue::global(QosClass::Default).async(move || {
               process_task(task);
               semaphore.signal();
           })?;
       }
       
       Ok(())
   }

os_unfair_lock 🔓
----------------

macOS's fastest synchronization primitive:

.. code-block:: rust
   :caption: Unfair lock usage
   :linenos:

   use kiln_platform::macos_sync::{MacOsFutex, SpinPolicy};
   
   fn create_unfair_lock() -> Result<MacOsFutex, Error> {
       // Note: "unfair" means no FIFO guarantee - can be faster!
       MacOsFutexBuilder::new()
           .with_spin_policy(SpinPolicy::None)  // Don't spin
           .with_priority_inheritance(false)     // Not supported
           .build()
   }
   
   // Benchmark unfair vs fair locks
   fn benchmark_lock_fairness() -> Result<(), Error> {
       let unfair = create_unfair_lock()?;
       let fair = create_fair_lock()?;  // Emulated with queue
       
       // Unfair lock: May starve some threads but faster overall
       // Fair lock: FIFO ordering but higher overhead
       
       println!("Use unfair locks unless fairness is required!");
       
       Ok(())
   }

Hypervisor.framework 🛡️
-----------------------

Hardware-accelerated isolation:

.. code-block:: rust
   :caption: Hypervisor framework for isolation
   :linenos:

   use kiln_platform::macos_hypervisor::{
       Hypervisor,
       VirtualMachine,
       VmExitReason
   };
   
   fn create_isolated_wasm_vm() -> Result<(), Error> {
       // Check if Hypervisor.framework is available
       if !Hypervisor::is_available()? {
           return Err(Error::FeatureNotAvailable("Hypervisor.framework"));
       }
       
       // Create VM for WASM isolation
       let mut vm = VirtualMachine::new()?;
       
       // Allocate guest memory
       vm.map_memory(0x0, 16 * 1024 * 1024)?; // 16MB at address 0
       
       // Create virtual CPU
       let vcpu = vm.create_vcpu()?;
       
       // Configure WASM execution environment
       vcpu.set_registers(InitialRegisters {
           // Set up for WASM execution
           ..Default::default()
       })?;
       
       // Run until exit
       loop {
           let exit = vcpu.run()?;
           
           match exit.reason {
               VmExitReason::Halt => break,
               VmExitReason::MemoryFault(addr) => {
                   println!("Memory fault at {:#x}", addr);
                   handle_memory_fault(&mut vm, addr)?;
               },
               VmExitReason::Hypercall(call) => {
                   handle_wasm_hypercall(&mut vm, call)?;
               },
               _ => {
                   return Err(Error::UnexpectedVmExit(exit.reason));
               }
           }
       }
       
       Ok(())
   }

Metal Compute Shaders 🎮
-----------------------

GPU acceleration for WebAssembly:

.. code-block:: rust
   :caption: Metal compute integration
   :linenos:

   use kiln_platform::macos_metal::{
       MetalDevice,
       ComputePipeline,
       Buffer
   };
   
   fn accelerate_wasm_with_metal() -> Result<(), Error> {
       // Get default Metal device
       let device = MetalDevice::default()?;
       
       // Compile WASM to Metal shader
       let shader_source = compile_wasm_to_metal(wasm_module)?;
       let pipeline = device.create_compute_pipeline(shader_source)?;
       
       // Create buffers
       let input_buffer = device.create_buffer(input_data)?;
       let output_buffer = device.create_buffer_uninitialized(output_size)?;
       
       // Create command queue and encoder
       let queue = device.create_command_queue();
       let command_buffer = queue.create_command_buffer();
       let encoder = command_buffer.create_compute_encoder();
       
       // Set up execution
       encoder.set_compute_pipeline(&pipeline);
       encoder.set_buffer(0, &input_buffer);
       encoder.set_buffer(1, &output_buffer);
       
       // Dispatch threads
       let thread_groups = MTLSize { 
           width: 32, 
           height: 1, 
           depth: 1 
       };
       encoder.dispatch_thread_groups(thread_groups);
       
       // Execute
       encoder.end_encoding();
       command_buffer.commit();
       command_buffer.wait_until_completed();
       
       // Read results
       let results = output_buffer.contents();
       
       Ok(())
   }

macOS Security 🔐
----------------

App Sandbox and Entitlements
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Configure sandboxing for WASM execution:

.. code-block:: rust
   :caption: macOS sandboxing
   :linenos:

   use kiln_platform::macos_security::{
       Sandbox,
       SandboxProfile,
       Entitlements
   };
   
   fn setup_wasm_sandbox() -> Result<(), Error> {
       // Define sandbox profile
       let profile = SandboxProfile::new()
           .deny_network()              // No network access
           .deny_file_write_all()       // Read-only filesystem
           .allow_file_read(&["/usr/lib", "/System"])
           .deny_process_fork()         // No subprocesses
           .deny_mach_lookup()          // No IPC
           .allow_signal(Signal::SIGTERM)
           .build()?;
       
       // Apply sandbox
       Sandbox::enter(profile)?;
       
       // Check entitlements
       let entitlements = Entitlements::current()?;
       if !entitlements.has("com.apple.security.cs.allow-jit") {
           println!("Warning: JIT not allowed by entitlements");
       }
       
       Ok(())
   }

Code Signing
~~~~~~~~~~~

Handle code signing for JIT:

.. code-block:: rust
   :caption: Code signing for JIT
   :linenos:

   use kiln_platform::macos_security::{CodeSigning, SecCodeRef};
   
   fn setup_jit_code_signing() -> Result<(), Error> {
       // Check if we have JIT entitlement
       let code = SecCodeRef::for_self()?;
       let entitlements = code.entitlements()?;
       
       if !entitlements.contains_key("com.apple.security.cs.allow-jit") {
           return Err(Error::MissingEntitlement("allow-jit"));
       }
       
       // Enable JIT
       CodeSigning::enable_jit()?;
       
       // For each JIT page
       let page = allocate_executable_memory()?;
       
       // Must toggle W^X (write XOR execute)
       page.make_writable()?;
       write_jit_code(&mut page)?;
       
       page.make_executable()?;  // Can't be writable anymore
       
       Ok(())
   }

Development Tools 🛠️
--------------------

Instruments Integration
~~~~~~~~~~~~~~~~~~~~~~

Profile with Instruments:

.. code-block:: rust
   :caption: Instruments profiling
   :linenos:

   use kiln_platform::macos_instruments::{
       InstrumentsRecorder,
       SignpostID
   };
   
   fn profile_with_instruments() -> Result<(), Error> {
       let recorder = InstrumentsRecorder::new("com.kiln.profiling")?;
       
       // Define signpost intervals
       let load_id = SignpostID::new();
       recorder.begin("Module Load", load_id);
       let module = load_wasm_module()?;
       recorder.end("Module Load", load_id);
       
       // Point events
       recorder.event("Compilation Start");
       
       // Numeric data
       recorder.log_value("Memory Usage", get_memory_usage());
       
       // Custom instruments
       recorder.custom_interval("WASM Execution") {
           execute_wasm()
       }?;
       
       Ok(())
   }

Console.app Logging
~~~~~~~~~~~~~~~~~~

Structured logging for Console.app:

.. code-block:: rust
   :caption: os_log integration
   :linenos:

   use kiln_platform::macos_logging::{OSLog, LogType};
   
   fn setup_system_logging() -> Result<(), Error> {
       let log = OSLog::new("com.kiln.runtime", "wasm")?;
       
       // Different log types
       log.default("WASM module loaded: {}", module_name);
       log.info("Execution started");
       log.debug("Stack pointer: {:#x}", sp);
       log.error("Execution failed: {}", error);
       log.fault("Critical failure - stopping runtime");
       
       // Activity tracing
       let activity = log.create_activity("WASM Execution");
       activity.enter();
       
       execute_wasm()?;
       
       activity.leave();
       
       Ok(())
   }

Performance Tips 🚀
------------------

macOS-Specific Optimizations:

.. code-block:: rust
   :caption: Platform optimizations
   :linenos:

   fn optimize_for_macos() -> Result<(), Error> {
       // 1. Use Accelerate.framework for SIMD
       use_accelerate_for_vector_ops()?;
       
       // 2. Prefer dispatch queues over threads
       use_gcd_not_threads()?;
       
       // 3. Respect QoS classes
       set_appropriate_qos()?;
       
       // 4. Use mach_absolute_time for timing
       let start = mach_absolute_time();
       
       // 5. Avoid vm_copy for small copies (overhead)
       use_memcpy_for_small_regions()?;
       
       Ok(())
   }

Best Practices 📚
-----------------

1. **Respect App Nap** - Don't fight power management
2. **Use QoS Classes** - Let the system prioritize
3. **Handle Memory Pressure** - Be a good citizen
4. **Sign for JIT** - Required for executable memory
5. **Profile with Instruments** - Great tools, use them!

macOS Gotchas ⚠️
----------------

**Memory:**
   - No overcommit - allocation can fail
   - Compressed memory can hide true usage
   - Wired memory limit is real

**Threading:**
   - pthread priorities need special entitlements
   - GCD is preferred over raw threads
   - QoS inversions are logged

**Security:**
   - Hardened runtime is default
   - Library validation may block plugins
   - Notarization affects distribution

.. admonition:: Apple Silicon Notes
   :class: note

   On M1/M2 Macs:
   
   - Use AMX for matrix operations
   - Efficiency cores affect scheduling
   - Unified memory changes assumptions
   - TSO mode available for x86 compatibility

Next Steps 🎯
-------------

- Explore :doc:`embedded_platforms` for smaller systems
- Learn about :doc:`hardware_security` for Apple Silicon features
- Check out :doc:`performance_optimizations` for macOS