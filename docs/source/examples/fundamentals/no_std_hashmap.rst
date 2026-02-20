===============================
No-Std HashMap: Hash Without the Heap
===============================

.. epigraph::

   "When you don't have much, you make the most of what you have."
   
   -- Every embedded developer ever

Who says you need a heap to hash? Kiln's no-std HashMap gives you all the power of hash-based lookups in environments where every byte counts. Perfect for embedded systems, WebAssembly modules, and anywhere the standard library fears to tread.

.. admonition:: What You'll Learn
   :class: note

   - Using HashMap without std or alloc
   - Custom hashers for embedded systems
   - Memory-bounded hash maps
   - Performance characteristics in constrained environments
   - Real-world embedded lookup patterns

The Challenge: Hashing Without Allocating 🧩
--------------------------------------------

Standard library HashMap is great, but it assumes you have:
- A heap allocator
- Unbounded memory growth
- Dynamic resizing capability

In embedded systems, these assumptions don't hold:

.. code-block:: rust
   :caption: What we can't do in no-std

   // This won't compile in no_std!
   use std::collections::HashMap;
   
   fn cant_do_this() {
       let mut map = HashMap::new(); // ❌ Needs allocator
       map.insert("key", "value");   // ❌ Might trigger allocation
   }

Enter Kiln's no-std solution:

.. code-block:: rust
   :caption: The Kiln way
   :linenos:

   #![no_std]
   use kiln_foundation::no_std_hashmap::NoStdHashMap;
   use kiln_foundation::bounded::BoundedVec;
   
   fn embed_friendly_lookup() {
       // Fixed capacity, no allocations!
       let mut map: NoStdHashMap<&str, u32, 32> = NoStdHashMap::new();
       
       // Insert configuration values
       map.insert("max_temp", 85);
       map.insert("min_temp", -40);
       map.insert("sample_rate", 1000);
       
       // Look up values
       if let Some(&max_temp) = map.get("max_temp") {
           // Use max_temp for sensor validation
           println!("Maximum temperature: {}°C", max_temp);
       }
   }

Practical Example: Configuration Manager 📋
-------------------------------------------

Let's build a configuration system for an embedded device:

.. code-block:: rust
   :caption: Embedded configuration with no-std HashMap
   :linenos:

   #![no_std]
   use kiln_foundation::no_std_hashmap::NoStdHashMap;
   use core::fmt::Write;
   use heapless::String; // For no-std strings
   
   const MAX_CONFIG_ENTRIES: usize = 64;
   const MAX_STRING_LEN: usize = 32;
   
   #[derive(Clone, Debug)]
   enum ConfigValue {
       Integer(i32),
       Boolean(bool),
       Text(String<MAX_STRING_LEN>),
   }
   
   impl ConfigValue {
       fn as_int(&self) -> Option<i32> {
           match self {
               ConfigValue::Integer(i) => Some(*i),
               _ => None,
           }
       }
       
       fn as_bool(&self) -> Option<bool> {
           match self {
               ConfigValue::Boolean(b) => Some(*b),
               _ => None,
           }
       }
       
       fn as_str(&self) -> Option<&str> {
           match self {
               ConfigValue::Text(s) => Some(s.as_str()),
               _ => None,
           }
       }
   }
   
   struct DeviceConfig {
       settings: NoStdHashMap<String<MAX_STRING_LEN>, ConfigValue, MAX_CONFIG_ENTRIES>,
   }
   
   impl DeviceConfig {
       fn new() -> Self {
           let mut config = Self {
               settings: NoStdHashMap::new(),
           };
           
           // Set defaults
           config.set_default_values();
           config
       }
       
       fn set_default_values(&mut self) {
           let _ = self.set_int("sensor_interval_ms", 1000);
           let _ = self.set_bool("debug_enabled", false);
           let _ = self.set_text("device_name", "Kiln-Device-001");
           let _ = self.set_int("max_connections", 4);
           let _ = self.set_bool("auto_calibrate", true);
       }
       
       fn set_int(&mut self, key: &str, value: i32) -> Result<(), &'static str> {
           let key_string = String::from(key);
           self.settings.insert(key_string, ConfigValue::Integer(value))
               .map_err(|_| "Config full")?;
           Ok(())
       }
       
       fn set_bool(&mut self, key: &str, value: bool) -> Result<(), &'static str> {
           let key_string = String::from(key);
           self.settings.insert(key_string, ConfigValue::Boolean(value))
               .map_err(|_| "Config full")?;
           Ok(())
       }
       
       fn set_text(&mut self, key: &str, value: &str) -> Result<(), &'static str> {
           let key_string = String::from(key);
           let value_string = String::from(value);
           self.settings.insert(key_string, ConfigValue::Text(value_string))
               .map_err(|_| "Config full")?;
           Ok(())
       }
       
       fn get_int(&self, key: &str) -> Option<i32> {
           let key_string = String::from(key);
           self.settings.get(&key_string)?.as_int()
       }
       
       fn get_bool(&self, key: &str) -> Option<bool> {
           let key_string = String::from(key);
           self.settings.get(&key_string)?.as_bool()
       }
       
       fn get_text(&self, key: &str) -> Option<&str> {
           let key_string = String::from(key);
           self.settings.get(&key_string)?.as_str()
       }
       
       fn serialize_to_buffer(&self, buffer: &mut [u8]) -> Result<usize, &'static str> {
           let mut written = 0;
           
           for (key, value) in self.settings.iter() {
               let line = match value {
                   ConfigValue::Integer(i) => format!("{}={}\n", key, i),
                   ConfigValue::Boolean(b) => format!("{}={}\n", key, b),
                   ConfigValue::Text(s) => format!("{}={}\n", key, s),
               };
               
               if written + line.len() > buffer.len() {
                   return Err("Buffer too small");
               }
               
               buffer[written..written + line.len()].copy_from_slice(line.as_bytes());
               written += line.len();
           }
           
           Ok(written)
       }
   }

Custom Hasher for Embedded Systems 🔧
-------------------------------------

For deterministic behavior, you might want a custom hasher:

.. code-block:: rust
   :caption: Simple, deterministic hasher
   :linenos:

   use core::hash::{Hasher, BuildHasher};
   
   /// Simple FNV-1a hasher - fast and good distribution
   struct FnvHasher {
       state: u64,
   }
   
   impl Default for FnvHasher {
       fn default() -> Self {
           Self {
               state: 0xcbf29ce484222325, // FNV offset basis
           }
       }
   }
   
   impl Hasher for FnvHasher {
       fn finish(&self) -> u64 {
           self.state
       }
       
       fn write(&mut self, bytes: &[u8]) {
           const FNV_PRIME: u64 = 0x100000001b3;
           
           for byte in bytes {
               self.state ^= *byte as u64;
               self.state = self.state.wrapping_mul(FNV_PRIME);
           }
       }
   }
   
   struct FnvBuildHasher;
   
   impl BuildHasher for FnvBuildHasher {
       type Hasher = FnvHasher;
       
       fn build_hasher(&self) -> Self::Hasher {
           FnvHasher::default()
       }
   }
   
   // Use it with your HashMap
   type CustomHashMap<K, V, const N: usize> = 
       NoStdHashMap<K, V, N, FnvBuildHasher>;

Real-World Example: Packet Classifier 📦
-----------------------------------------

Here's how you might use this for network packet classification:

.. code-block:: rust
   :caption: High-speed packet classifier
   :linenos:

   #![no_std]
   use kiln_foundation::no_std_hashmap::NoStdHashMap;
   use kiln_foundation::bounded::BoundedVec;
   
   const MAX_FLOWS: usize = 1024;
   const MAX_PACKET_BUFFER: usize = 64;
   
   #[derive(Hash, PartialEq, Eq, Clone, Copy)]
   struct FlowKey {
       src_ip: u32,
       dst_ip: u32,
       src_port: u16,
       dst_port: u16,
       protocol: u8,
   }
   
   #[derive(Clone)]
   struct FlowStats {
       packet_count: u32,
       byte_count: u64,
       last_seen: u32, // Timestamp
       priority: u8,
   }
   
   impl FlowStats {
       fn new(timestamp: u32) -> Self {
           Self {
               packet_count: 0,
               byte_count: 0,
               last_seen: timestamp,
               priority: 0,
           }
       }
       
       fn update(&mut self, packet_size: u16, timestamp: u32) {
           self.packet_count += 1;
           self.byte_count += packet_size as u64;
           self.last_seen = timestamp;
       }
   }
   
   struct PacketClassifier {
       flows: NoStdHashMap<FlowKey, FlowStats, MAX_FLOWS>,
       high_priority_flows: BoundedVec<FlowKey, 64>,
   }
   
   impl PacketClassifier {
       fn new() -> Self {
           Self {
               flows: NoStdHashMap::new(),
               high_priority_flows: BoundedVec::new(),
           }
       }
       
       fn classify_packet(
           &mut self,
           flow_key: FlowKey,
           packet_size: u16,
           timestamp: u32
       ) -> PacketAction {
           // Update or create flow stats
           match self.flows.get_mut(&flow_key) {
               Some(stats) => {
                   stats.update(packet_size, timestamp);
                   
                   // Promote to high priority if lots of traffic
                   if stats.packet_count > 100 && stats.priority == 0 {
                       stats.priority = 1;
                       let _ = self.high_priority_flows.push(flow_key);
                   }
               }
               None => {
                   // New flow
                   let stats = FlowStats::new(timestamp);
                   if self.flows.insert(flow_key, stats).is_err() {
                       // Flow table full - evict oldest
                       self.evict_oldest_flow();
                       let _ = self.flows.insert(flow_key, FlowStats::new(timestamp));
                   }
               }
           }
           
           // Determine action based on flow characteristics
           if self.is_high_priority_flow(&flow_key) {
               PacketAction::FastPath
           } else if self.is_suspicious_flow(&flow_key) {
               PacketAction::Drop
           } else {
               PacketAction::NormalPath
           }
       }
       
       fn is_high_priority_flow(&self, flow_key: &FlowKey) -> bool {
           self.high_priority_flows.iter().any(|&key| key == *flow_key)
       }
       
       fn is_suspicious_flow(&self, flow_key: &FlowKey) -> bool {
           if let Some(stats) = self.flows.get(flow_key) {
               // Simple heuristic: too many small packets
               stats.packet_count > 1000 && stats.byte_count / stats.packet_count as u64 < 64
           } else {
               false
           }
       }
       
       fn evict_oldest_flow(&mut self) {
           let mut oldest_key = None;
           let mut oldest_time = u32::MAX;
           
           for (key, stats) in self.flows.iter() {
               if stats.last_seen < oldest_time {
                   oldest_time = stats.last_seen;
                   oldest_key = Some(*key);
               }
           }
           
           if let Some(key) = oldest_key {
               self.flows.remove(&key);
               // Also remove from high priority list
               if let Some(pos) = self.high_priority_flows.iter().position(|&k| k == key) {
                   self.high_priority_flows.remove(pos);
               }
           }
       }
       
       fn get_flow_count(&self) -> usize {
           self.flows.len()
       }
       
       fn cleanup_expired_flows(&mut self, current_time: u32, timeout: u32) {
           let expired_keys: BoundedVec<FlowKey, MAX_FLOWS> = self.flows
               .iter()
               .filter(|(_, stats)| current_time - stats.last_seen > timeout)
               .map(|(key, _)| *key)
               .collect();
           
           for key in expired_keys.iter() {
               self.flows.remove(key);
           }
       }
   }
   
   #[derive(Debug, PartialEq)]
   enum PacketAction {
       FastPath,
       NormalPath,
       Drop,
   }

Performance in Constrained Environments 🏁
------------------------------------------

No-std HashMap is optimized for embedded use:

.. list-table:: Performance Characteristics
   :header-rows: 1
   :widths: 30 25 25 20

   * - Operation
     - Time Complexity
     - Memory Usage
     - Notes
   * - Insert
     - O(1) average
     - Fixed at compile time
     - No allocations
   * - Lookup
     - O(1) average
     - Zero additional memory
     - Cache-friendly
   * - Remove
     - O(1) average
     - Frees slot immediately
     - No defragmentation
   * - Iteration
     - O(capacity)
     - Stack-based iterator
     - Predictable timing

Memory Layout and Cache Efficiency 💾
-------------------------------------

The no-std HashMap is designed for cache efficiency:

.. code-block:: rust
   :caption: Understanding memory layout

   use kiln_foundation::no_std_hashmap::NoStdHashMap;
   
   // All data is stored in a fixed array
   let map: NoStdHashMap<u32, u32, 64> = NoStdHashMap::new();
   
   // Memory layout (simplified):
   // [Entry][Entry][Entry]...[Entry] <- 64 entries, contiguous
   // Each Entry contains:
   // - hash: u64
   // - key: u32  
   // - value: u32
   // - state: EntryState (Empty/Occupied/Deleted)
   
   // This layout provides:
   // ✅ Predictable memory usage
   // ✅ Good cache locality
   // ✅ No heap fragmentation
   // ✅ Deterministic performance

Debugging and Diagnostics 🔍
----------------------------

Built-in diagnostics for embedded debugging:

.. code-block:: rust
   :caption: HashMap diagnostics

   fn analyze_hash_distribution<K, V, const N: usize>(
       map: &NoStdHashMap<K, V, N>
   ) -> HashMapStats {
       let mut stats = HashMapStats::default();
       
       stats.capacity = N;
       stats.occupied = map.len();
       stats.load_factor = (map.len() as f32) / (N as f32);
       
       // Analyze collision chains
       let mut chain_lengths = [0u32; N];
       for i in 0..N {
           if map.is_slot_occupied(i) {
               let chain_len = map.probe_distance(i);
               chain_lengths[chain_len as usize] += 1;
           }
       }
       
       stats.max_probe_distance = chain_lengths.iter()
           .enumerate()
           .rfind(|(_, &count)| count > 0)
           .map(|(len, _)| len as u32)
           .unwrap_or(0);
       
       stats
   }
   
   #[derive(Default, Debug)]
   struct HashMapStats {
       capacity: usize,
       occupied: usize,
       load_factor: f32,
       max_probe_distance: u32,
   }

Best Practices for Embedded Hash Maps 💡
----------------------------------------

.. admonition:: Embedded HashMap Wisdom
   :class: tip

   1. **Size Appropriately**: Choose capacity based on expected load
   2. **Monitor Load Factor**: Keep below 75% for best performance
   3. **Handle Full Gracefully**: Always check insert() return values
   4. **Use Good Hash Functions**: FNV or SipHash for small keys
   5. **Profile Probe Distances**: Long chains indicate poor hashing

Common Pitfalls 🕳️
------------------

.. admonition:: Watch Out!
   :class: warning

   1. **Capacity Planning**: HashMap capacity is fixed at compile time
   2. **Hash Quality**: Poor hash functions cause clustering
   3. **Key Equality**: Make sure Eq implementation is correct
   4. **Clone Costs**: Large keys/values make operations expensive

Your Turn! 🎮
-------------

Try these challenges:

1. **Build a Symbol Table**: For a simple programming language
2. **Create a Resource Registry**: Map handles to resources
3. **Implement a Cache**: With LRU eviction policy

Next Steps 🚶
-------------

- Learn about resource management: :doc:`resources`
- Explore component values: :doc:`component_values`
- See real applications: :doc:`../core/index`

Remember: Constraints breed creativity. When you can't allocate, you innovate! 🚀