======================================
QNX Platform Features
======================================

.. epigraph::

   "In QNX, everything is a message. Even this quote is probably being passed through a channel somewhere."
   
   -- QNX systems programmer

QNX Neutrino RTOS is the platform of choice for safety-critical systems. From cars to medical devices, from trains to nuclear reactors, QNX provides the determinism and reliability that keeps the world running. Let's explore how Kiln leverages QNX's unique features.

.. admonition:: What You'll Learn
   :class: note

   - QNX memory partitioning for guaranteed resources
   - Adaptive partitioning for dynamic resource allocation
   - Priority inheritance and real-time scheduling
   - Message passing and resource managers
   - POSIX real-time extensions
   - Safety certification considerations

QNX Architecture Overview 🏗️
----------------------------

QNX is a microkernel RTOS where everything runs in user space:

.. code-block:: rust
   :caption: QNX system architecture awareness
   :linenos:

   use kiln_platform::qnx_detection;
   
   fn understand_qnx_system() -> Result<QnxSystemInfo, Error> {
       let info = qnx_detection::get_system_info()?;
       
       println!("QNX Version: {}", info.version);
       println!("Microkernel: {}", info.kernel_version);
       println!("CPU cores: {}", info.num_cpus);
       println!("Clock resolution: {} ns", info.clock_resolution_ns);
       
       // Check for safety-critical features
       if info.has_watchdog {
           println!("✅ Hardware watchdog available");
       }
       
       if info.has_high_availability_manager {
           println!("✅ HAM (High Availability Manager) present");
       }
       
       Ok(info)
   }

Memory Partitioning 🎯
---------------------

QNX's killer feature for safety-critical systems:

.. code-block:: rust
   :caption: Memory partition management
   :linenos:

   use kiln_platform::{
       QnxMemoryPartition, 
       QnxMemoryPartitionBuilder,
       QnxPartitionFlags
   };
   
   fn create_guaranteed_memory() -> Result<QnxMemoryPartition, Error> {
       // Create a memory partition with guaranteed minimums
       let partition = QnxMemoryPartitionBuilder::new("wasm_runtime")
           .with_size(64 * 1024 * 1024)        // Request 64MB
           .with_minimum_size(32 * 1024 * 1024) // Guarantee at least 32MB
           .with_flags(
               QnxPartitionFlags::LOCKED |      // Cannot be paged out
               QnxPartitionFlags::NONPAGED |     // Never swapped
               QnxPartitionFlags::CRITICAL       // System critical
           )
           .with_inheritance(true)              // Child processes inherit
           .build()?;
       
       println!("Partition created: {}", partition.name());
       println!("Guaranteed size: {} MB", partition.guaranteed_size() / 1024 / 1024);
       println!("Current usage: {} MB", partition.current_usage() / 1024 / 1024);
       
       Ok(partition)
   }
   
   // Using partitions with allocators
   fn partition_aware_allocation() -> Result<(), Error> {
       let partition = create_guaranteed_memory()?;
       
       // Create allocator within the partition
       let allocator = QnxAllocatorBuilder::new()
           .with_partition(partition.clone())
           .with_maximum_pages(512)
           .with_guard_pages(true)
           .with_arena_allocation(true)  // Use arena within partition
           .build()?;
       
       // Monitor partition health
       if partition.usage_percentage() > 80.0 {
           println!("⚠️  Partition usage high: {}%", partition.usage_percentage());
       }
       
       Ok(())
   }

Adaptive Partitioning 🔄
------------------------

Dynamic resource allocation with guarantees:

.. code-block:: rust
   :caption: Adaptive partition scheduling
   :linenos:

   use kiln_platform::qnx_adaptive::{
       AdaptivePartition,
       AdaptivePartitionBuilder,
       PartitionBudget
   };
   
   fn setup_adaptive_partitioning() -> Result<AdaptivePartition, Error> {
       // Create adaptive partition for WebAssembly execution
       let partition = AdaptivePartitionBuilder::new("wasm_exec")
           .with_budget(PartitionBudget {
               guaranteed: 30,    // 30% CPU guaranteed
               maximum: 70,       // Can use up to 70% if available
               critical: 10,      // Additional 10% in critical time
           })
           .with_window_size(100) // 100ms averaging window
           .with_critical_time_threshold(Duration::from_millis(10))
           .build()?;
       
       // Join current thread to partition
       partition.join_current_thread()?;
       
       // Execute with partition budget
       partition.run_with_budget(|| {
           // This code runs within the adaptive partition
           execute_wasm_module()
       })?;
       
       // Monitor budget usage
       let stats = partition.get_stats()?;
       println!("CPU usage: {}% (guaranteed: {}%)", 
                stats.usage_percentage, 
                stats.guaranteed_percentage);
       
       Ok(partition)
   }

Real-Time Scheduling 🚀
----------------------

QNX's real-time scheduler in action:

.. code-block:: rust
   :caption: Real-time thread configuration
   :linenos:

   use kiln_platform::qnx_realtime::{
       SchedPolicy,
       ThreadAttributes,
       inherit_scheduling
   };
   
   fn configure_realtime_execution() -> Result<(), Error> {
       // Configure thread for real-time execution
       let attr = ThreadAttributes::new()
           .with_policy(SchedPolicy::FIFO)      // First-in-first-out
           .with_priority(50)                   // Priority 1-255
           .with_runmask(0b1111)               // Run on CPUs 0-3
           .with_inherit_sched(false)          // Don't inherit from parent
           .build()?;
       
       // Apply to current thread
       attr.apply_to_current()?;
       
       // Create high-priority interrupt handler thread
       let handler = std::thread::Builder::new()
           .name("wasm_interrupt_handler".to_string())
           .spawn_with_attributes(attr.clone(), || {
               // Set up interrupt handling
               handle_wasm_interrupts()
           })?;
       
       // Use priority inheritance for synchronization
       let mutex = QnxFutexBuilder::new()
           .with_priority_ceiling(60)  // Higher than any accessor
           .with_priority_inheritance(true)
           .build()?;
       
       Ok(())
   }

Message Passing IPC 📬
---------------------

QNX's native IPC for zero-copy communication:

.. code-block:: rust
   :caption: QNX message passing
   :linenos:

   use kiln_platform::qnx_ipc::{
       Channel,
       Message,
       MessageType
   };
   
   // Server side - WebAssembly runtime
   fn create_wasm_server() -> Result<(), Error> {
       let channel = Channel::create()?;
       
       println!("WASM server listening on channel: {}", channel.id());
       
       loop {
           // Receive message (blocks until message arrives)
           let (msg, client_id) = channel.receive()?;
           
           match msg.msg_type() {
               MessageType::LoadModule => {
                   let module_data = msg.data();
                   let result = load_wasm_module(module_data)?;
                   
                   // Reply with zero-copy
                   channel.reply(client_id, &result)?;
               },
               MessageType::Execute => {
                   let params = msg.data();
                   let result = execute_wasm_function(params)?;
                   
                   // Pulse for async notification
                   channel.pulse(client_id, result.completion_code)?;
               },
               _ => {
                   channel.error_reply(client_id, ErrorCode::NotSupported)?;
               }
           }
       }
   }
   
   // Client side
   fn connect_to_wasm_server() -> Result<(), Error> {
       let channel = Channel::connect("/dev/wasm_runtime")?;
       
       // Send module for loading
       let module = std::fs::read("app.wasm")?;
       let reply = channel.send_receive(
           MessageType::LoadModule,
           &module,
           Duration::from_secs(5)
       )?;
       
       println!("Module loaded: {:?}", reply);
       
       Ok(())
   }

Resource Managers 🗄️
--------------------

Create a POSIX-compliant WebAssembly device:

.. code-block:: rust
   :caption: QNX resource manager
   :linenos:

   use kiln_platform::qnx_resource_manager::{
       ResourceManager,
       ResourceManagerBuilder,
       IoHandlers
   };
   
   fn create_wasm_device() -> Result<(), Error> {
       // Create resource manager for /dev/wasm
       let mut mgr = ResourceManagerBuilder::new("/dev/wasm")
           .with_permissions(0o666)
           .with_single_threaded(false)
           .build()?;
       
       // Register I/O handlers
       mgr.register_handlers(IoHandlers {
           open: |path, flags| {
               println!("Opening WASM device: {}", path);
               Ok(DeviceHandle::new())
           },
           read: |handle, buffer| {
               // Read WASM execution results
               let results = handle.get_results()?;
               buffer.copy_from_slice(&results);
               Ok(results.len())
           },
           write: |handle, data| {
               // Write WASM module or commands
               handle.process_command(data)?;
               Ok(data.len())
           },
           devctl: |handle, cmd, data| {
               // Device control for special operations
               match cmd {
                   DCMD_WASM_RESET => handle.reset()?,
                   DCMD_WASM_GET_STATS => {
                       let stats = handle.get_stats()?;
                       data.copy_from_slice(&stats.to_bytes());
                   },
                   _ => return Err(Error::InvalidCommand),
               }
               Ok(())
           },
       });
       
       // Start resource manager
       mgr.start()?;
       
       Ok(())
   }

High Availability 🛡️
--------------------

Using QNX HAM for fault tolerance:

.. code-block:: rust
   :caption: High Availability Manager integration
   :linenos:

   use kiln_platform::qnx_ham::{
       Ham,
       Entity,
       Condition,
       Action
   };
   
   fn setup_high_availability() -> Result<(), Error> {
       let mut ham = Ham::attach()?;
       
       // Create entity for WASM runtime
       let entity = ham.create_entity("wasm_runtime")?;
       
       // Add heartbeat condition
       entity.add_condition(
           Condition::Heartbeat {
               interval: Duration::from_secs(1),
               tolerance: 3,  // Miss 3 heartbeats = dead
           },
           vec![
               Action::Restart,
               Action::Notify("wasm_runtime_died"),
               Action::Execute("/scripts/wasm_recovery.sh"),
           ]
       )?;
       
       // Add death condition
       entity.add_condition(
           Condition::Death,
           vec![
               Action::RestartWithEscalation {
                   max_restarts: 3,
                   window: Duration::from_mins(5),
                   escalation: Box::new(Action::Reboot),
               }
           ]
       )?;
       
       // Start heartbeat
       entity.start_heartbeat()?;
       
       // In main loop
       loop {
           // Do work...
           entity.heartbeat()?;  // Signal we're alive
           
           std::thread::sleep(Duration::from_millis(500));
       }
   }

Safety Certification 📋
----------------------

QNX features for safety-critical systems:

.. code-block:: rust
   :caption: Safety-critical configuration
   :linenos:

   use kiln_platform::qnx_safety::{
       SafetyLevel,
       WatchdogConfig,
       ErrorHandler
   };
   
   fn configure_for_asil_d() -> Result<(), Error> {
       // Configure for ASIL-D (highest automotive safety level)
       
       // 1. Set up hardware watchdog
       let watchdog = WatchdogConfig::new()
           .with_timeout(Duration::from_millis(100))
           .with_pretimeout(Duration::from_millis(80))
           .with_action(WatchdogAction::Reset)
           .enable()?;
       
       // 2. Configure memory protection
       let partition = QnxMemoryPartitionBuilder::new("safety_critical")
           .with_flags(QnxPartitionFlags::all_safety())
           .with_error_detection(true)
           .with_ecc_memory(true)
           .build()?;
       
       // 3. Set up error handling
       ErrorHandler::install(|error| {
           match error.severity() {
               Severity::Critical => {
                   // Log to persistent storage
                   log_to_nvram(&error);
                   // Trigger safe state
                   enter_safe_state();
                   // Never return
                   qnx_abort();
               },
               Severity::Major => {
                   // Attempt recovery
                   attempt_recovery(&error);
               },
               _ => {
                   // Log and continue
                   log_error(&error);
               }
           }
       })?;
       
       // 4. Enable execution time monitoring
       enable_execution_monitoring()?;
       
       Ok(())
   }

Performance Profiling 📊
-----------------------

QNX's system profiler integration:

.. code-block:: rust
   :caption: Performance monitoring
   :linenos:

   use kiln_platform::qnx_profiling::{
       SystemProfiler,
       TraceEvent
   };
   
   fn profile_wasm_execution() -> Result<(), Error> {
       let profiler = SystemProfiler::new()?;
       
       // Start profiling
       profiler.start()?;
       
       // Trace custom events
       profiler.trace_event(TraceEvent::Custom {
           class: "WASM",
           event: "module_load_start",
           data: module_name.as_bytes(),
       })?;
       
       // Execute WASM
       let result = execute_wasm();
       
       profiler.trace_event(TraceEvent::Custom {
           class: "WASM",  
           event: "module_load_end",
           data: &result.stats.to_bytes(),
       })?;
       
       // Stop and analyze
       let report = profiler.stop()?;
       
       println!("Execution time: {} µs", report.total_time_us);
       println!("Context switches: {}", report.context_switches);
       println!("CPU usage: {}%", report.cpu_percentage);
       
       Ok(())
   }

Best Practices 📚
-----------------

1. **Always Use Partitions** for memory isolation
2. **Set Real-Time Priorities** appropriately
3. **Monitor Resource Usage** via partition stats
4. **Use HAM** for production deployments
5. **Test with System Profiler** for bottlenecks

QNX-Specific Tips 💡
-------------------

**Memory:**
   - Pre-allocate in partitions to avoid runtime failures
   - Use typed memory for special hardware regions
   - Lock critical pages to prevent paging

**Scheduling:**
   - FIFO for deterministic behavior
   - Round-robin for fairness
   - Sporadic for aperiodic tasks

**IPC:**
   - Prefer pulses for notifications
   - Use shared memory for large data
   - Message passing for synchronization

.. admonition:: Safety First
   :class: warning

   When building safety-critical systems on QNX:
   
   - Follow IEC 61508 / ISO 26262 guidelines
   - Use QNX Safety kernel if required
   - Implement redundancy and voting
   - Test failure modes exhaustively
   - Document safety arguments

Next Steps 🎯
-------------

- Compare with :doc:`linux_features` for desktop/server
- Learn about :doc:`embedded_platforms` for smaller systems
- Explore :doc:`performance_optimizations` for QNX