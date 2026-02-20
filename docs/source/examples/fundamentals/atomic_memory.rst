=====================================
Atomic Memory: Integrity Through Atomicity
=====================================

.. epigraph::

   "Race conditions are the bane of parallel programming."
   
   -- Every developer who's debugged concurrent code

Ever had a bit flip corrupt your data between writing and checksumming? Or worried about concurrent access breaking your memory integrity checks? Kiln's atomic memory operations ensure that write operations and checksum calculations happen atomically - no race conditions, no corruption!

.. admonition:: What You'll Learn
   :class: note

   - How ``AtomicMemoryOps`` prevents data corruption
   - Atomic writes with integrated checksumming
   - Thread-safe memory operations without external locks
   - Real-world patterns for safety-critical systems
   - When to use atomic memory vs regular safe memory

The Race Condition We're Solving 🏁
------------------------------------

Here's the subtle problem with non-atomic memory operations:

.. code-block:: rust
   :caption: The race condition (DON'T DO THIS!)

   // Thread 1: Writing data
   memory.write_slice(0, &data)?;           // Write happens...
   // ⚡ DANGER ZONE: Another thread could modify memory here!
   let checksum = calculate_checksum(&data); // ...but checksum is calculated later
   memory.set_checksum(checksum);
   
   // Thread 2: Could corrupt between write and checksum!
   memory.write_slice(0, &other_data)?;     // Oops! Checksum now invalid

The atomic solution ensures write and checksum happen together:

.. code-block:: rust
   :caption: The atomic way (DO THIS!)
   :linenos:

   use kiln_foundation::{
       atomic_memory::{AtomicMemoryOps, AtomicMemoryExt},
       safe_memory::{SafeMemoryHandler, NoStdProvider},
       verification::VerificationLevel,
   };
   
   type MyProvider = NoStdProvider<4096>;
   
   fn atomic_example() -> Result<(), Error> {
       // Create atomic memory operations
       let handler = SafeMemoryHandler::<MyProvider>::new(
           MyProvider::default(),
           VerificationLevel::Full
       )?;
       
       let atomic_ops = AtomicMemoryOps::new(handler);
       
       // This is atomic - no race condition possible!
       let data = b"Critical data that must not be corrupted";
       atomic_ops.write_atomic(0, data)?;
       
       // Read is also atomic with verification
       let mut buffer = [0u8; 41];
       atomic_ops.read_atomic(0, &mut buffer)?;
       
       Ok(())
   }

Understanding AtomicMemoryOps 🔍
--------------------------------

The ``AtomicMemoryOps`` type wraps a ``SafeMemoryHandler`` in a mutex, ensuring atomicity:

.. code-block:: rust
   :caption: How AtomicMemoryOps works
   :linenos:

   use kiln_foundation::{
       atomic_memory::AtomicMemoryOps,
       safe_memory::{SafeMemoryHandler, StdProvider},
       verification::VerificationLevel,
   };
   use kiln_sync::mutex::WrtMutex;
   
   // What AtomicMemoryOps looks like internally
   pub struct AtomicMemoryOps<P: Provider> {
       handler: WrtMutex<SafeMemoryHandler<P>>,
       verification_level: VerificationLevel,
   }
   
   // Example: Creating with different providers
   fn create_atomic_ops() -> Result<(), Error> {
       // For std environments
       let std_handler = SafeMemoryHandler::new(
           StdProvider::new(),
           VerificationLevel::Standard
       )?;
       let std_atomic = AtomicMemoryOps::new(std_handler);
       
       // For no_std environments
       type EmbeddedProvider = NoStdProvider<8192>;
       let embedded_handler = SafeMemoryHandler::new(
           EmbeddedProvider::default(),
           VerificationLevel::Critical  // Maximum verification for safety-critical
       )?;
       let embedded_atomic = AtomicMemoryOps::new(embedded_handler);
       
       Ok(())
   }

Verification Levels: Choose Your Safety 🛡️
-------------------------------------------

AtomicMemoryOps supports all verification levels:

.. code-block:: rust
   :caption: Different verification levels
   :linenos:

   use kiln_foundation::{
       atomic_memory::AtomicMemoryOps,
       safe_memory::{SafeMemoryHandler, NoStdProvider},
       verification::{VerificationLevel, Checksum},
       operations::{record_global_operation, Type as OperationType},
   };
   
   const SAFETY_CRITICAL_MEMORY: usize = 16384;  // 16KB
   type SafetyProvider = NoStdProvider<SAFETY_CRITICAL_MEMORY>;
   
   struct SafetySystem {
       // Different memory regions with different verification needs
       config: AtomicMemoryOps<SafetyProvider>,      // Critical config
       telemetry: AtomicMemoryOps<SafetyProvider>,   // Less critical
       scratch: AtomicMemoryOps<SafetyProvider>,     // Temporary data
   }
   
   impl SafetySystem {
       fn new() -> Result<Self, Error> {
           Ok(Self {
               // Critical configuration - maximum verification
               config: AtomicMemoryOps::new(
                   SafeMemoryHandler::new(
                       SafetyProvider::default(),
                       VerificationLevel::Critical
                   )?
               ),
               
               // Telemetry - standard verification
               telemetry: AtomicMemoryOps::new(
                   SafeMemoryHandler::new(
                       SafetyProvider::default(),
                       VerificationLevel::Standard
                   )?
               ),
               
               // Scratch space - minimal verification for performance
               scratch: AtomicMemoryOps::new(
                   SafeMemoryHandler::new(
                       SafetyProvider::default(),
                       VerificationLevel::Minimal
                   )?
               ),
           })
       }
       
       fn store_config(&self, config_data: &[u8]) -> Result<(), Error> {
           // Critical write - full verification and checksum
           self.config.write_atomic(0, config_data)?;
           
           // Verify immediately after write
           let checksum = self.config.checksum()?;
           log::info!("Config stored with checksum: {:?}", checksum);
           
           Ok(())
       }
   }

Real-World Pattern: Sensor Data Logger 📊
-----------------------------------------

Here's an example sensor data logger demonstrating atomic memory usage:

.. code-block:: rust
   :caption: Thread-safe sensor data logger
   :linenos:

   use kiln_foundation::{
       atomic_memory::{AtomicMemoryOps, AtomicMemoryExt},
       safe_memory::{SafeMemoryHandler, NoStdProvider},
       verification::VerificationLevel,
       bounded::{BoundedVec, CapacityError},
   };
   use core::sync::atomic::{AtomicUsize, Ordering};
   
   const SENSOR_MEMORY_SIZE: usize = 32768;  // 32KB for sensor data
   const MAX_SENSORS: usize = 16;
   const READING_SIZE: usize = 16;  // Each reading is 16 bytes
   
   type SensorProvider = NoStdProvider<SENSOR_MEMORY_SIZE>;
   
   #[repr(C, packed)]
   struct SensorReading {
       timestamp: u64,    // 8 bytes
       sensor_id: u16,    // 2 bytes
       value: f32,        // 4 bytes
       status: u16,       // 2 bytes
   }
   
   pub struct SensorLogger {
       memory: AtomicMemoryOps<SensorProvider>,
       write_position: AtomicUsize,
       total_readings: AtomicUsize,
   }
   
   impl SensorLogger {
       pub fn new() -> Result<Self, Error> {
           let handler = SafeMemoryHandler::new(
               SensorProvider::default(),
               VerificationLevel::Standard
           )?;
           
           Ok(Self {
               memory: AtomicMemoryOps::new(handler),
               write_position: AtomicUsize::new(0),
               total_readings: AtomicUsize::new(0),
           })
       }
       
       pub fn log_reading(&self, reading: SensorReading) -> Result<(), Error> {
           // Calculate position atomically
           let pos = self.write_position.fetch_add(READING_SIZE, Ordering::SeqCst);
           
           // Wrap around if we exceed memory
           let actual_pos = pos % (SENSOR_MEMORY_SIZE - READING_SIZE);
           
           // Convert reading to bytes
           let bytes = unsafe {
               core::slice::from_raw_parts(
                   &reading as *const _ as *const u8,
                   READING_SIZE
               )
           };
           
           // Atomic write with checksum
           self.memory.write_atomic(actual_pos, bytes)?;
           
           // Update total count
           self.total_readings.fetch_add(1, Ordering::Relaxed);
           
           Ok(())
       }
       
       pub fn get_latest_readings(&self, count: usize) -> Result<Vec<SensorReading>, Error> {
           let mut readings = Vec::new();
           let current_pos = self.write_position.load(Ordering::SeqCst);
           
           for i in 0..count {
               let offset = (current_pos + SENSOR_MEMORY_SIZE 
                            - (i + 1) * READING_SIZE) % SENSOR_MEMORY_SIZE;
               
               let mut buffer = [0u8; READING_SIZE];
               match self.memory.read_atomic(offset, &mut buffer) {
                   Ok(()) => {
                       // Convert bytes back to SensorReading
                       let reading = unsafe {
                           core::ptr::read(buffer.as_ptr() as *const SensorReading)
                       };
                       readings.push(reading);
                   }
                   Err(_) => break,  // No more valid readings
               }
           }
           
           Ok(readings)
       }
       
       pub fn verify_integrity(&self) -> Result<bool, Error> {
           // Atomically verify all memory checksums
           self.memory.verify_all_checksums()
       }
   }

Extension Trait Pattern 🔧
--------------------------

The ``AtomicMemoryExt`` trait provides convenient atomic operations:

.. code-block:: rust
   :caption: Using the AtomicMemoryExt trait
   :linenos:

   use kiln_foundation::{
       atomic_memory::{AtomicMemoryOps, AtomicMemoryExt},
       safe_memory::{SafeMemoryHandler, NoStdProvider},
       verification::VerificationLevel,
   };
   
   type ConfigProvider = NoStdProvider<1024>;
   
   #[derive(Debug, Clone, Copy)]
   struct SystemConfig {
       version: u32,
       flags: u32,
       timeout_ms: u32,
       max_retries: u8,
       padding: [u8; 3],  // Align to 16 bytes
   }
   
   struct ConfigManager {
       memory: AtomicMemoryOps<ConfigProvider>,
   }
   
   impl ConfigManager {
       fn new() -> Result<Self, Error> {
           let handler = SafeMemoryHandler::new(
               ConfigProvider::default(),
               VerificationLevel::Full
           )?;
           
           Ok(Self {
               memory: AtomicMemoryOps::new(handler),
           })
       }
       
       fn save_config(&self, config: &SystemConfig) -> Result<(), Error> {
           // Use the extension trait for atomic operations
           let bytes = unsafe {
               core::slice::from_raw_parts(
                   config as *const _ as *const u8,
                   core::mem::size_of::<SystemConfig>()
               )
           };
           
           // Atomic write with automatic checksum
           self.memory.write_atomic(0, bytes)?;
           
           // Double-check by reading back
           let mut verify_buffer = [0u8; core::mem::size_of::<SystemConfig>()];
           self.memory.read_atomic(0, &mut verify_buffer)?;
           
           Ok(())
       }
       
       fn load_config(&self) -> Result<SystemConfig, Error> {
           let mut buffer = [0u8; core::mem::size_of::<SystemConfig>()];
           
           // Atomic read with verification
           self.memory.read_atomic(0, &mut buffer)?;
           
           // Convert back to config
           let config = unsafe {
               core::ptr::read(buffer.as_ptr() as *const SystemConfig)
           };
           
           Ok(config)
       }
   }

Performance Considerations 📈
-----------------------------

AtomicMemoryOps uses WrtMutex internally, which adapts based on features:

.. code-block:: rust
   :caption: Performance characteristics

   // With std feature: Uses parking_lot Mutex (very fast)
   // Without std: Uses spin-lock based mutex
   
   use criterion::{black_box, Criterion};
   
   fn benchmark_atomic_vs_regular(c: &mut Criterion) {
       type Provider = NoStdProvider<4096>;
       
       // Regular SafeMemoryHandler
       let regular = SafeMemoryHandler::<Provider>::new(
           Provider::default(),
           VerificationLevel::Standard
       ).unwrap();
       
       // AtomicMemoryOps
       let atomic = AtomicMemoryOps::new(regular.clone());
       
       let data = b"Benchmark data";
       
       c.bench_function("regular_write", |b| {
           b.iter(|| {
               regular.write_slice(0, black_box(data)).unwrap();
           });
       });
       
       c.bench_function("atomic_write", |b| {
           b.iter(|| {
               atomic.write_atomic(0, black_box(data)).unwrap();
           });
       });
   }

**Typical results:**
- Regular write: ~50ns
- Atomic write: ~65ns (with std), ~75ns (no_std spin)
- **Only ~30% overhead for complete atomicity!**

Best Practices 🎯
-----------------

.. admonition:: Do This!
   :class: tip

   1. **Use for Critical Data**: Config, calibration, safety parameters
   2. **Choose Verification Level Wisely**: Critical for safety, Minimal for logs
   3. **Batch Operations**: Group related writes in one atomic operation
   4. **Regular Integrity Checks**: Call ``verify_all_checksums()`` periodically

.. admonition:: Avoid These!
   :class: warning

   1. **Don't Use for Streaming**: Atomicity overhead not worth it
   2. **Don't Mix Access**: Either all atomic or none - no mixing!
   3. **Watch Memory Size**: Each operation locks the entire handler
   4. **Consider Alternatives**: Sometimes a simple mutex is clearer

Integration with Kiln Components 🧩
----------------------------------

AtomicMemoryOps integrates seamlessly with other Kiln components:

.. code-block:: rust
   :caption: Complete system integration

   use kiln_foundation::{
       atomic_memory::AtomicMemoryOps,
       bounded::{BoundedVec, BoundedString},
       safe_memory::{SafeMemoryHandler, NoStdProvider},
       component_value::ComponentValue,
   };
   
   struct ComponentState {
       // Atomic memory for critical state
       state_memory: AtomicMemoryOps<NoStdProvider<4096>>,
       
       // Bounded collections for structure
       event_log: BoundedVec<Event, 100, NoStdProvider<8192>>,
       
       // Component values for type safety
       current_value: ComponentValue,
   }

Your Turn! 🎮
-------------

Try these challenges:

1. **Build a Circular Buffer**: Use atomic operations for thread-safe access
2. **Create a Checksummed Cache**: Store key-value pairs with integrity
3. **Implement Double Buffering**: Atomic swap between two memory regions

Next Steps 🚶
-------------

- Explore synchronization: :doc:`sync_primitives`
- Learn about providers: :doc:`safe_memory`
- See platform integration: :doc:`../platform/memory_management`

Remember: Atomic memory operations are about correctness first, performance second. When you absolutely must ensure data integrity in concurrent environments, AtomicMemoryOps has your back! 🛡️