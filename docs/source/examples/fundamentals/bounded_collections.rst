===================================
Bounded Collections: No Heap, No Problem!
===================================

.. epigraph::

   "The discipline of desire is the background of character."
   
   -- John Locke (probably talking about memory allocation)

Ever had a Vec grow until it ate all your memory? Or a HashMap that decided to reorganize itself at the worst possible moment? Say hello to bounded collections - data structures with built-in self-control!

.. admonition:: What You'll Learn
   :class: note

   - How to use ``BoundedVec``, ``BoundedStack``, and friends with MemoryProviders
   - Why compile-time capacity is your new best friend
   - How to choose between ``StdProvider`` and ``NoStdProvider``
   - Handling ``CapacityError`` like a pro
   - Real patterns for embedded and no_std environments

The Hero We Need: BoundedVec 🦸
--------------------------------

Let's start with the star of the show - ``BoundedVec``. It's like ``Vec``, but with a twist: it needs a MemoryProvider!

.. code-block:: rust
   :caption: Your first BoundedVec (std environment)
   :linenos:

   use kiln_foundation::{
       bounded::{BoundedVec, CapacityError},
       safe_memory::StdProvider,
   };
   
   fn main() -> Result<(), CapacityError> {
       // With std, we use StdProvider
       let mut my_vec: BoundedVec<u32, 10, StdProvider> = BoundedVec::new();
       
       // Push some items
       my_vec.push(42)?;
       my_vec.push(7)?;
       my_vec.push(13)?;
       
       // Check our status
       println!("Length: {}", my_vec.len());        // 3
       println!("Capacity: {}", my_vec.capacity());  // 10
       println!("Is full? {}", my_vec.is_full());   // false
       
       // Access items safely
       if let Some(first) = my_vec.get(0) {
           println!("First item: {}", first);  // 42
       }
       
       // Pop items
       while let Some(item) = my_vec.pop() {
           println!("Popped: {}", item);
       }
       
       Ok(())
   }

No-std? No Problem! 🚀
~~~~~~~~~~~~~~~~~~~~~~

Working in a no_std environment? Meet ``NoStdProvider``:

.. code-block:: rust
   :caption: BoundedVec in no_std
   :linenos:

   #![no_std]
   
   use kiln_foundation::{
       bounded::{BoundedVec, CapacityError},
       safe_memory::NoStdProvider,
   };
   
   // NoStdProvider needs a const size parameter
   type MyProvider = NoStdProvider<4096>;  // 4KB of memory
   
   fn sensor_buffer_example() -> Result<(), CapacityError> {
       // Create a bounded vector with custom provider
       let mut readings: BoundedVec<u16, 100, MyProvider> = BoundedVec::new();
       
       // Add sensor readings
       for i in 0..50 {
           readings.push(i * 10)?;
       }
       
       // Process readings
       let sum: u32 = readings.iter().map(|&x| x as u32).sum();
       let average = sum / readings.len() as u32;
       
       Ok(())
   }

The Power of MemoryProvider 💪
------------------------------

What's this ``MemoryProvider`` business? It's how Kiln stays flexible:

.. code-block:: rust
   :caption: Understanding MemoryProviders
   :linenos:

   use kiln_foundation::{
       prelude::*,
       bounded::{BoundedVec, BoundedStack, BoundedString},
       safe_memory::{NoStdProvider, StdProvider, MemoryProvider},
   };
   
   // For embedded systems with exactly 2KB available
   const EMBEDDED_MEMORY: usize = 2048;
   type EmbeddedProvider = NoStdProvider<EMBEDDED_MEMORY>;
   
   struct DataLogger {
       // String with max 255 chars (WASM name length limit)
       name: BoundedString<255, EmbeddedProvider>,
       // Stack for undo operations
       undo_stack: BoundedStack<Command, 20, EmbeddedProvider>,
       // Vector for data points
       data: BoundedVec<f32, 50, EmbeddedProvider>,
   }
   
   #[derive(Clone)]
   enum Command {
       Add(f32),
       Remove(usize),
       Clear,
   }
   
   impl DataLogger {
       fn new(name: &str) -> Result<Self, CapacityError> {
           let mut logger_name = BoundedString::new();
           logger_name.push_str(name)?;
           
           Ok(Self {
               name: logger_name,
               undo_stack: BoundedStack::new(),
               data: BoundedVec::new(),
           })
       }
       
       fn add_data(&mut self, value: f32) -> Result<(), CapacityError> {
           // Save undo command
           self.undo_stack.push(Command::Add(value))?;
           
           // Add the data
           self.data.push(value)?;
           
           Ok(())
       }
   }

Advanced Collections: Queue, Map, and More! 📚
----------------------------------------------

Kiln provides a full suite of bounded collections:

.. code-block:: rust
   :caption: The bounded collection family
   :linenos:

   use kiln_foundation::{
       bounded_collections::{
           BoundedQueue, BoundedDeque, BoundedMap, BoundedSet,
       },
       safe_memory::NoStdProvider,
   };
   
   // A message queue for an embedded system
   type MessageProvider = NoStdProvider<8192>;  // 8KB for messages
   
   struct Message {
       id: u32,
       payload: [u8; 64],
   }
   
   struct MessageRouter {
       // FIFO queue for incoming messages
       incoming: BoundedQueue<Message, 32, MessageProvider>,
       
       // Double-ended queue for priority handling
       priority: BoundedDeque<Message, 16, MessageProvider>,
       
       // Map for routing rules (port -> handler_id)
       routes: BoundedMap<u16, u32, 64, MessageProvider>,
       
       // Set of active connections
       active_ports: BoundedSet<u16, 128, MessageProvider>,
   }
   
   impl MessageRouter {
       fn new() -> Self {
           Self {
               incoming: BoundedQueue::new(),
               priority: BoundedDeque::new(),
               routes: BoundedMap::new(),
               active_ports: BoundedSet::new(),
           }
       }
       
       fn route_message(&mut self, port: u16, msg: Message) -> Result<(), CapacityError> {
           // Check if port is active
           if !self.active_ports.contains(&port) {
               self.active_ports.insert(port)?;
           }
           
           // Check for priority routing
           if let Some(&handler_id) = self.routes.get(&port) {
               if handler_id > 1000 {  // High priority threshold
                   self.priority.push_back(msg)?;
               } else {
                   self.incoming.enqueue(msg)?;
               }
           } else {
               // Default queue
               self.incoming.enqueue(msg)?;
           }
           
           Ok(())
       }
   }

BoundedString: When Names Matter 📝
-----------------------------------

Working with WebAssembly names? There's a type for that:

.. code-block:: rust
   :caption: BoundedString for WASM names
   :linenos:

   use kiln_foundation::{
       bounded::{BoundedString, WasmName, MAX_WASM_NAME_LENGTH},
       safe_memory::NoStdProvider,
   };
   
   // WasmName is an alias for BoundedString with WASM's name length limit
   type MyWasmName = WasmName<NoStdProvider<1024>>;
   
   struct WasmModule {
       name: MyWasmName,
       version: BoundedString<32, NoStdProvider<1024>>,
   }
   
   impl WasmModule {
       fn new(name: &str, version: &str) -> Result<Self, CapacityError> {
           let mut module_name = MyWasmName::new();
           module_name.push_str(name)?;
           
           let mut module_version = BoundedString::new();
           module_version.push_str(version)?;
           
           // MAX_WASM_NAME_LENGTH is 255
           assert!(module_name.capacity() == MAX_WASM_NAME_LENGTH);
           
           Ok(Self {
               name: module_name,
               version: module_version,
           })
       }
   }

Real-World Pattern: Ring Buffer 🔄
----------------------------------

Here's an example ring buffer implementation using bounded collections:

.. code-block:: rust
   :caption: Ring buffer for sensor data
   :linenos:

   use kiln_foundation::{
       bounded::{BoundedVec, CapacityError},
       safe_memory::NoStdProvider,
       verification::VerificationLevel,
   };
   
   const SENSOR_BUFFER_SIZE: usize = 1024;
   const SENSOR_MEMORY_SIZE: usize = 64 * 1024;  // 64KB
   
   type SensorProvider = NoStdProvider<SENSOR_MEMORY_SIZE>;
   
   struct SensorReading {
       timestamp: u64,
       temperature: f32,
       humidity: f32,
       pressure: f32,
   }
   
   struct SensorRingBuffer {
       data: BoundedVec<SensorReading, SENSOR_BUFFER_SIZE, SensorProvider>,
       total_readings: u64,
   }
   
   impl SensorRingBuffer {
       fn new() -> Self {
           Self {
               data: BoundedVec::new(),
               total_readings: 0,
           }
       }
       
       fn add_reading(&mut self, reading: SensorReading) -> Result<(), CapacityError> {
           if self.data.is_full() {
               // Remove oldest reading to make room
               self.data.remove(0);
           }
           
           self.data.push(reading)?;
           self.total_readings += 1;
           
           Ok(())
       }
       
       fn get_statistics(&self) -> SensorStats {
           if self.data.is_empty() {
               return SensorStats::default();
           }
           
           let mut temp_sum = 0.0;
           let mut humidity_sum = 0.0;
           let mut pressure_sum = 0.0;
           
           for reading in self.data.iter() {
               temp_sum += reading.temperature;
               humidity_sum += reading.humidity;
               pressure_sum += reading.pressure;
           }
           
           let count = self.data.len() as f32;
           
           SensorStats {
               avg_temperature: temp_sum / count,
               avg_humidity: humidity_sum / count,
               avg_pressure: pressure_sum / count,
               total_readings: self.total_readings,
               buffer_usage: (self.data.len() * 100) / SENSOR_BUFFER_SIZE,
           }
       }
   }
   
   #[derive(Default)]
   struct SensorStats {
       avg_temperature: f32,
       avg_humidity: f32,
       avg_pressure: f32,
       total_readings: u64,
       buffer_usage: usize,  // Percentage
   }

Error Handling Best Practices 🎯
---------------------------------

Let's talk about handling ``CapacityError`` gracefully:

.. code-block:: rust
   :caption: Error handling patterns
   :linenos:

   use kiln_foundation::{
       bounded::{BoundedVec, CapacityError},
       safe_memory::NoStdProvider,
       Error, ErrorCategory,
   };
   
   type MyProvider = NoStdProvider<2048>;
   
   enum DataError {
       Capacity(CapacityError),
       Invalid(&'static str),
   }
   
   impl From<CapacityError> for DataError {
       fn from(err: CapacityError) -> Self {
           DataError::Capacity(err)
       }
   }
   
   fn process_data(
       buffer: &mut BoundedVec<u32, 100, MyProvider>,
       value: u32
   ) -> Result<(), DataError> {
       // Validate input
       if value > 1000 {
           return Err(DataError::Invalid("Value too large"));
       }
       
       // Try to add to buffer
       match buffer.push(value) {
           Ok(()) => Ok(()),
           Err(CapacityError) => {
               // Buffer full - implement your strategy
               if value > buffer[0] {  // Replace if larger than first
                   buffer[0] = value;
                   Ok(())
               } else {
                   Err(DataError::Capacity(CapacityError))
               }
           }
       }
   }

Performance Characteristics 📊
------------------------------

Understanding the performance of bounded collections:

.. list-table:: Operation Complexity
   :header-rows: 1
   :widths: 30 20 20 30

   * - Operation
     - BoundedVec
     - BoundedQueue
     - Notes
   * - Push/Enqueue
     - O(1)
     - O(1)
     - Fails if full
   * - Pop/Dequeue
     - O(1)
     - O(1)
     - Returns None if empty
   * - Insert at index
     - O(n)
     - N/A
     - Shifts elements
   * - Remove at index
     - O(n)
     - N/A
     - Shifts elements
   * - Access by index
     - O(1)
     - N/A
     - Direct memory access
   * - Memory overhead
     - Minimal
     - Minimal
     - Fixed at compile time

Tips and Tricks 🎩
------------------

.. admonition:: Pro Tips
   :class: tip

   1. **Choose Your Provider Wisely**: 
      - ``StdProvider`` for normal Rust programs
      - ``NoStdProvider<N>`` for embedded/no_std
      - Custom providers for special memory regions

   2. **Size Your Memory**: 
      - Calculate: ``size_of::<T>() * N + overhead``
      - NoStdProvider needs enough for metadata too

   3. **Const Generic Patterns**:
      .. code-block:: rust

         fn process<const N: usize, P: MemoryProvider>(
             vec: &mut BoundedVec<u32, N, P>
         ) -> Result<(), CapacityError> {
             // Works with any capacity!
             vec.push(42)
         }

   4. **Default Providers**: Many examples use this pattern:
      .. code-block:: rust

         type Provider = NoStdProvider<4096>;
         type MyVec<T, const N: usize> = BoundedVec<T, N, Provider>;

Your Turn! 🎮
-------------

Try these challenges:

1. **Build a Command Buffer**: Fixed-size buffer for a CLI with history
2. **Create a Priority Queue**: Using BoundedVec with manual sorting
3. **Implement an LRU Cache**: Using BoundedMap and access tracking

Next Steps 🚶
-------------

- Dive into memory safety: :doc:`safe_memory`
- Explore thread-safe collections: :doc:`atomic_memory`
- Learn about sync primitives: :doc:`sync_primitives`

Remember: In embedded systems, knowing your limits isn't a weakness—it's a superpower! 🦸‍♀️