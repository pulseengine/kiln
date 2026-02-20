====================================
Sync Primitives: Coordination Made Simple
====================================

.. epigraph::

   "Concurrency is not parallelism, although it enables parallelism."
   
   -- Rob Pike

Sometimes you need more than just atomic operations. When coordination gets complex, you need the big guns: mutexes, read-write locks, and once cells. Kiln's sync primitives give you the power of coordination without the pain of platform-specific details.

.. admonition:: What You'll Learn
   :class: note

   - Using ``WrtMutex`` for mutual exclusion in std and no_std
   - ``WrtRwLock`` for reader-writer scenarios  
   - ``WrtParkingRwLock`` for advanced performance (std only)
   - ``KilnOnce`` for lazy, one-time initialization
   - Real patterns for concurrent systems

WrtMutex: Universal Mutual Exclusion 🔐
---------------------------------------

Kiln's mutex adapts to your environment - parking_lot in std, spin locks in no_std:

.. code-block:: rust
   :caption: Basic WrtMutex usage
   :linenos:

   use kiln_sync::mutex::{WrtMutex, WrtMutexGuard};
   use core::sync::atomic::{AtomicU32, Ordering};
   
   // Works in both std and no_std!
   struct SharedCounter {
       value: WrtMutex<u32>,
       operations: AtomicU32,
   }
   
   impl SharedCounter {
       fn new(initial: u32) -> Self {
           Self {
               value: WrtMutex::new(initial),
               operations: AtomicU32::new(0),
           }
       }
       
       fn increment(&self) -> u32 {
           let mut guard = self.value.lock();
           *guard += 1;
           self.operations.fetch_add(1, Ordering::Relaxed);
           *guard  // Return new value
       }
       
       fn get(&self) -> u32 {
           *self.value.lock()
       }
   }
   
   // Example with multiple threads (std feature)
   #[cfg(feature = "std")]
   fn parallel_counting() {
       use std::sync::Arc;
       use std::thread;
       
       let counter = Arc::new(SharedCounter::new(0));
       let mut handles = vec![];
       
       for _ in 0..10 {
           let counter = Arc::clone(&counter);
           handles.push(thread::spawn(move || {
               for _ in 0..1000 {
                   counter.increment();
               }
           }));
       }
       
       for handle in handles {
           handle.join().unwrap();
       }
       
       println!("Final count: {}", counter.get());
       println!("Total operations: {}", counter.operations.load(Ordering::Relaxed));
   }

No-Std Mutex: Embedded-Friendly 🎯
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In no_std environments, WrtMutex uses a spin lock implementation:

.. code-block:: rust
   :caption: WrtMutex in no_std
   :linenos:

   #![no_std]
   
   use kiln_sync::mutex::WrtMutex;
   use kiln_foundation::bounded::{BoundedVec, CapacityError};
   use kiln_foundation::safe_memory::NoStdProvider;
   
   type EventProvider = NoStdProvider<2048>;
   const MAX_EVENTS: usize = 32;
   
   #[derive(Clone, Copy)]
   struct Event {
       timestamp: u64,
       event_type: EventType,
       data: u16,
   }
   
   #[derive(Clone, Copy)]
   enum EventType {
       SensorReading,
       ButtonPress,
       TimerTick,
   }
   
   // Global event queue for embedded system
   static EVENT_QUEUE: WrtMutex<BoundedVec<Event, MAX_EVENTS, EventProvider>> = 
       WrtMutex::new(BoundedVec::new());
   
   fn log_event(event: Event) -> Result<(), CapacityError> {
       let mut queue = EVENT_QUEUE.lock();
       
       // If full, remove oldest event
       if queue.is_full() {
           queue.remove(0);
       }
       
       queue.push(event)
   }
   
   fn process_events() -> Option<Event> {
       let mut queue = EVENT_QUEUE.lock();
       if !queue.is_empty() {
           Some(queue.remove(0))
       } else {
           None
       }
   }

WrtRwLock: Many Readers, Few Writers 📚
---------------------------------------

When reads dominate, use ``WrtRwLock`` for better concurrency:

.. code-block:: rust
   :caption: Read-write lock for configuration
   :linenos:

   use kiln_sync::rwlock::{WrtRwLock, WrtRwLockReadGuard, WrtRwLockWriteGuard};
   use kiln_foundation::bounded::{BoundedString, CapacityError};
   use kiln_foundation::safe_memory::NoStdProvider;
   
   type ConfigProvider = NoStdProvider<4096>;
   
   struct Config {
       server_url: BoundedString<256, ConfigProvider>,
       timeout_ms: u32,
       max_retries: u8,
       debug_enabled: bool,
   }
   
   impl Config {
       fn new() -> Result<Self, CapacityError> {
           let mut server_url = BoundedString::new();
           server_url.push_str("https://api.example.com")?;
           
           Ok(Config {
               server_url,
               timeout_ms: 5000,
               max_retries: 3,
               debug_enabled: false,
           })
       }
   }
   
   struct ConfigManager {
       config: WrtRwLock<Config>,
       update_count: core::sync::atomic::AtomicU32,
   }
   
   impl ConfigManager {
       fn new() -> Result<Self, CapacityError> {
           Ok(Self {
               config: WrtRwLock::new(Config::new()?),
               update_count: core::sync::atomic::AtomicU32::new(0),
           })
       }
       
       // Multiple threads can read simultaneously
       fn get_timeout(&self) -> u32 {
           let guard: WrtRwLockReadGuard<'_, Config> = self.config.read();
           guard.timeout_ms
       }
       
       fn is_debug_enabled(&self) -> bool {
           self.config.read().debug_enabled
       }
       
       // Only one thread can write
       fn update_timeout(&self, new_timeout: u32) {
           let mut guard: WrtRwLockWriteGuard<'_, Config> = self.config.write();
           guard.timeout_ms = new_timeout;
           self.update_count.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
       }
       
       // Try to acquire locks without blocking
       fn try_update_debug(&self, enabled: bool) -> bool {
           if let Some(mut guard) = self.config.try_write() {
               guard.debug_enabled = enabled;
               true
           } else {
               false  // Someone else is writing
           }
       }
   }

WrtParkingRwLock: Advanced Performance (std only) 🚀
----------------------------------------------------

When you have std and need maximum performance:

.. code-block:: rust
   :caption: Parking lot based RwLock
   :linenos:

   #[cfg(feature = "std")]
   use kiln_sync::rwlock::parking_impl::{
       WrtParkingRwLock, 
       WrtParkingRwLockReadGuard,
       WrtParkingRwLockWriteGuard
   };
   use std::collections::HashMap;
   use std::sync::Arc;
   
   #[cfg(feature = "std")]
   struct CacheEntry {
       value: String,
       access_count: u64,
       last_access: std::time::Instant,
   }
   
   #[cfg(feature = "std")]
   struct HighPerformanceCache {
       entries: Arc<WrtParkingRwLock<HashMap<String, CacheEntry>>>,
       max_size: usize,
   }
   
   #[cfg(feature = "std")]
   impl HighPerformanceCache {
       fn new(max_size: usize) -> Self {
           Self {
               entries: Arc::new(WrtParkingRwLock::new(HashMap::new())),
               max_size,
           }
       }
       
       fn get(&self, key: &str) -> Option<String> {
           // Fast read path - multiple threads can read
           let mut entries = self.entries.read();
           
           if let Some(entry) = entries.get_mut(key) {
               entry.access_count += 1;
               entry.last_access = std::time::Instant::now();
               Some(entry.value.clone())
           } else {
               None
           }
       }
       
       fn insert(&self, key: String, value: String) {
           let mut entries = self.entries.write();
           
           // Evict LRU if at capacity
           if entries.len() >= self.max_size {
               if let Some(lru_key) = entries.iter()
                   .min_by_key(|(_, e)| e.last_access)
                   .map(|(k, _)| k.clone())
               {
                   entries.remove(&lru_key);
               }
           }
           
           entries.insert(key, CacheEntry {
               value,
               access_count: 0,
               last_access: std::time::Instant::now(),
           });
       }
   }

KilnOnce: Initialize Once, Use Forever 🎯
----------------------------------------

Perfect for expensive one-time initialization:

.. code-block:: rust
   :caption: Lazy initialization with KilnOnce
   :linenos:

   use kiln_sync::once::KilnOnce;
   use kiln_foundation::{
       bounded::{BoundedVec, CapacityError},
       safe_memory::NoStdProvider,
   };
   
   type LookupProvider = NoStdProvider<8192>;
   
   // Global lookup table initialized on first use
   static LOOKUP_TABLE: KilnOnce<BoundedVec<u16, 256, LookupProvider>> = KilnOnce::new();
   
   fn get_lookup_table() -> &'static BoundedVec<u16, 256, LookupProvider> {
       LOOKUP_TABLE.get_or_init(|| {
           // This runs exactly once, even with concurrent access
           let mut table = BoundedVec::new();
           
           // Generate lookup values (expensive computation)
           for i in 0..256u16 {
               // Some complex calculation
               let value = (i * i + 42) % 1024;
               table.push(value).expect("Table size mismatch");
           }
           
           table
       })
   }
   
   // Configuration loaded once at startup
   struct SystemConfig {
       device_id: u32,
       calibration_offset: i16,
       features_enabled: u32,
   }
   
   static SYSTEM_CONFIG: KilnOnce<SystemConfig> = KilnOnce::new();
   
   fn initialize_system(device_id: u32) {
       SYSTEM_CONFIG.get_or_init(|| {
           // Load from flash/EEPROM/etc
           SystemConfig {
               device_id,
               calibration_offset: read_calibration_from_flash(),
               features_enabled: 0xFF00FF00,
           }
       });
   }
   
   fn get_device_id() -> u32 {
       SYSTEM_CONFIG.get()
           .expect("System not initialized")
           .device_id
   }
   
   fn read_calibration_from_flash() -> i16 {
       // Simulate reading from non-volatile memory
       42
   }

Real-World Pattern: Producer-Consumer Queue 🏭
----------------------------------------------

Combining sync primitives for a robust queue:

.. code-block:: rust
   :caption: Thread-safe producer-consumer queue
   :linenos:

   use kiln_sync::{mutex::WrtMutex, once::KilnOnce};
   use kiln_foundation::{
       bounded_collections::{BoundedQueue, CapacityError},
       safe_memory::NoStdProvider,
   };
   use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
   
   type QueueProvider = NoStdProvider<16384>;  // 16KB for queue
   const MAX_MESSAGES: usize = 128;
   
   #[derive(Clone)]
   struct Message {
       id: u64,
       payload: [u8; 64],
       priority: u8,
   }
   
   pub struct MessageQueue {
       queue: WrtMutex<BoundedQueue<Message, MAX_MESSAGES, QueueProvider>>,
       shutdown: AtomicBool,
       pending_count: AtomicUsize,
   }
   
   impl MessageQueue {
       fn new() -> Self {
           Self {
               queue: WrtMutex::new(BoundedQueue::new()),
               shutdown: AtomicBool::new(false),
               pending_count: AtomicUsize::new(0),
           }
       }
       
       fn send(&self, msg: Message) -> Result<(), CapacityError> {
           if self.shutdown.load(Ordering::Acquire) {
               return Err(CapacityError);
           }
           
           let mut queue = self.queue.lock();
           queue.enqueue(msg)?;
           self.pending_count.fetch_add(1, Ordering::Release);
           
           // In std, could notify waiting consumers here
           #[cfg(feature = "std")]
           {
               // self.condvar.notify_one();
           }
           
           Ok(())
       }
       
       fn receive(&self) -> Option<Message> {
           let mut queue = self.queue.lock();
           
           if let Some(msg) = queue.dequeue() {
               self.pending_count.fetch_sub(1, Ordering::Release);
               Some(msg)
           } else {
               None
           }
       }
       
       fn pending(&self) -> usize {
           self.pending_count.load(Ordering::Acquire)
       }
       
       fn shutdown(&self) {
           self.shutdown.store(true, Ordering::Release);
       }
   }
   
   // Global message queue instance
   static MESSAGE_QUEUE: KilnOnce<MessageQueue> = KilnOnce::new();
   
   fn get_message_queue() -> &'static MessageQueue {
       MESSAGE_QUEUE.get_or_init(MessageQueue::new)
   }

Performance Characteristics 📊
------------------------------

Understanding the cost of each primitive:

.. list-table:: Sync Primitive Performance
   :header-rows: 1
   :widths: 30 25 25 20

   * - Primitive
     - Uncontended (std)
     - Uncontended (no_std)
     - Best Use Case
   * - WrtMutex
     - ~5ns (parking_lot)
     - ~10ns (spin)
     - Exclusive access
   * - WrtRwLock (read)
     - ~7ns
     - ~12ns
     - Read-heavy workloads
   * - WrtRwLock (write)
     - ~15ns
     - ~20ns
     - Infrequent updates
   * - WrtParkingRwLock
     - ~3ns (read)
     - N/A
     - Maximum performance
   * - KilnOnce (after init)
     - ~1ns
     - ~1ns
     - One-time setup

Best Practices 💡
-----------------

.. admonition:: Sync Wisdom
   :class: tip

   1. **Choose the Right Tool**: 
      - Mutex for exclusive access
      - RwLock when reads >> writes
      - Once for initialization
      
   2. **Lock Scope**: Keep critical sections minimal:
      .. code-block:: rust
      
         // Good - short lock scope
         let value = {
             let guard = mutex.lock();
             guard.clone()
         };  // Lock released here
         
         process_value(value);  // Heavy work outside lock
   
   3. **Avoid Nested Locks**: Prevent deadlocks by design
   
   4. **Use Try Methods**: For responsive systems:
      .. code-block:: rust
      
         if let Some(guard) = rwlock.try_write() {
             // Got the lock
         } else {
             // Do something else
         }

Common Pitfalls 🕳️
------------------

.. admonition:: Avoid These!
   :class: warning

   1. **Poisoned Locks**: In std, panicking while holding a lock poisons it
   2. **Priority Inversion**: High-priority tasks waiting on low-priority locks
   3. **Reader Starvation**: Continuous writes preventing reads in RwLock
   4. **Spin Lock Power**: In no_std, spinning wastes CPU cycles

Integration Patterns 🧩
-----------------------

Kiln sync primitives work seamlessly with other components:

.. code-block:: rust
   :caption: Complete system example

   use kiln_sync::{mutex::WrtMutex, rwlock::WrtRwLock, once::KilnOnce};
   use kiln_foundation::{
       atomic_memory::AtomicMemoryOps,
       bounded::{BoundedVec, BoundedString},
       safe_memory::{SafeMemoryHandler, NoStdProvider},
   };
   
   struct EmbeddedSystem {
       // One-time config
       config: &'static SystemConfig,
       
       // Shared state with mutex
       sensor_data: WrtMutex<SensorData>,
       
       // Read-heavy status
       system_status: WrtRwLock<SystemStatus>,
       
       // Atomic memory for critical data
       critical_memory: AtomicMemoryOps<NoStdProvider<4096>>,
   }

Your Turn! 🎮
-------------

Try these challenges:

1. **Build a Thread Pool**: Use WrtMutex for work queue, KilnOnce for config
2. **Create a Stats Collector**: RwLock for metrics, atomic counters
3. **Implement Async Executor**: Combine all primitives for task scheduling

Next Steps 🚶
-------------

- Compare with atomics: :doc:`atomic_memory`
- See real usage: :doc:`../core/bounded_engine`
- Platform-specific sync: :doc:`../platform/synchronization`

Remember: The best synchronization is invisible - your code just works! Choose the right primitive for the job, and your concurrent code will be both safe and fast. 🎯