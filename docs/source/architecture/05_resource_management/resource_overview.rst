.. _resource_overview:

Resource Management Overview
============================

This section provides a comprehensive overview of resource management in Pulseengine (Kiln Edition),
focusing on how resources are allocated, tracked, and managed across different runtime environments.

.. arch_component:: ARCH_COMP_RES_001
   :title: Unified Resource Management System
   :status: implemented
   :version: 1.0
   :rationale: Provide consistent resource management across std, no_std+alloc, and no_std+no_alloc environments

   Resource management system that adapts allocation strategies and tracking mechanisms
   based on the target environment while maintaining consistent resource semantics.

Resource Management Architecture
--------------------------------

Core Resource Types
~~~~~~~~~~~~~~~~~~~

Pulseengine manages several categories of resources (``kiln-component/src/resources/mod.rs:45-89``):

.. code-block:: rust

   /// Core resource categories in the system
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub enum ResourceCategory {
       /// Linear memory regions
       Memory,
       /// WebAssembly tables
       Tables,
       /// Component instances
       Components,
       /// Host function handles
       HostFunctions,
       /// I/O handles (files, sockets, etc.)
       IoHandles,
       /// Synchronization primitives
       Synchronization,
       /// Platform-specific resources
       Platform,
   }

   /// Resource descriptor with environment-adaptive storage
   #[derive(Debug, Clone)]
   pub struct ResourceDescriptor {
       pub id: ResourceId,
       pub category: ResourceCategory,
       pub size_hint: Option<usize>,
       pub alignment: usize,
       pub metadata: BoundedMap<BoundedString, BoundedString>,
   }

   /// Environment-specific resource storage
   pub enum ResourceStorage {
       #[cfg(feature = "std")]
       Dynamic {
           heap_allocator: Box<dyn Allocator>,
           resource_map: HashMap<ResourceId, Box<dyn Any>>,
       },
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       SemiDynamic {
           allocator: alloc::alloc::Global,
           resource_map: BTreeMap<ResourceId, Box<dyn Any>>,
       },
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       Static {
           memory_pools: [MemoryPool; 8],
           resource_slots: heapless::Pool<ResourceSlot, 1024>,
           resource_map: heapless::FnvIndexMap<ResourceId, usize, 1024>,
       },
   }

Multi-Environment Resource Strategies
-------------------------------------

Resource Allocation Strategies
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Different environments use different allocation strategies based on available capabilities:

.. list-table:: Resource Allocation by Environment
   :header-rows: 1
   :widths: 20 25 25 30

   * - Resource Type
     - std Environment
     - no_std+alloc Environment
     - no_std+no_alloc Environment
   * - Memory Regions
     - Dynamic Vec allocation
     - Dynamic Vec allocation
     - Fixed-size arrays [u8; N]
   * - Component Storage
     - HashMap<Id, Component>
     - BTreeMap<Id, Component>
     - Fixed array + bitmap
   * - Resource Tables
     - HashMap with growth
     - BTreeMap with growth
     - Fixed pool with slots
   * - String Storage
     - String (heap)
     - String (heap)
     - heapless::String<N>
   * - Collection Storage
     - Vec<T> (dynamic)
     - Vec<T> (dynamic)
     - heapless::Vec<T, N>

**Implementation Example** (``kiln-component/src/resources/resource_manager.rs:67-134``):

.. code-block:: rust

   /// Resource manager with environment-adaptive allocation
   pub struct ResourceManager {
       storage: ResourceStorage,
       next_id: ResourceId,
       allocation_stats: AllocationStats,
   }

   impl ResourceManager {
       /// Allocate a resource with environment-appropriate strategy
       pub fn allocate<T: Any + Send + Sync>(
           &mut self,
           resource: T,
       ) -> Result<ResourceId, ResourceError> {
           let id = self.next_id;
           self.next_id = ResourceId(self.next_id.0 + 1);

           match &mut self.storage {
               #[cfg(feature = "std")]
               ResourceStorage::Dynamic { resource_map, .. } => {
                   resource_map.insert(id, Box::new(resource));
               }
               
               #[cfg(all(not(feature = "std"), feature = "alloc"))]
               ResourceStorage::SemiDynamic { resource_map, .. } => {
                   resource_map.insert(id, Box::new(resource));
               }
               
               #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
               ResourceStorage::Static { resource_slots, resource_map, .. } => {
                   // Check if we have available slots
                   let slot_index = resource_slots.alloc()
                       .map_err(|_| ResourceError::PoolExhausted)?;
                   
                   // Store resource in fixed slot
                   resource_slots[slot_index] = ResourceSlot::new(resource)?;
                   resource_map.insert(id, slot_index)
                       .map_err(|_| ResourceError::MapFull)?;
               }
           }

           self.update_allocation_stats(core::mem::size_of::<T>());
           Ok(id)
       }
   }

Memory Resource Management
--------------------------

Linear Memory Management
~~~~~~~~~~~~~~~~~~~~~~~~

WebAssembly linear memory is managed differently across environments:

.. code-block:: rust

   /// Linear memory management (kiln-foundation/src/safe_memory.rs:178-245)
   pub struct LinearMemoryManager {
       #[cfg(feature = "std")]
       regions: Vec<MemoryRegion>,
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       regions: alloc::vec::Vec<MemoryRegion>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       regions: heapless::Vec<MemoryRegion, 16>,
       
       total_allocated: usize,
       max_memory: Option<usize>,
   }

   impl LinearMemoryManager {
       /// Allocate linear memory with bounds checking
       pub fn allocate_memory(
           &mut self,
           size: usize,
           protection: MemoryProtection,
       ) -> Result<LinearMemory, MemoryError> {
           // Check memory limits
           if let Some(max) = self.max_memory {
               if self.total_allocated + size > max {
                   return Err(MemoryError::AllocationLimitExceeded {
                       requested: size,
                       available: max - self.total_allocated,
                   });
               }
           }

           let memory = match self.create_memory_region(size, protection) {
               #[cfg(any(feature = "std", feature = "alloc"))]
               Ok(region) => {
                   let mut data = vec![0u8; size];
                   LinearMemory::new_dynamic(data, protection)
               }
               
               #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
               Ok(_) => {
                   if size > 65536 {
                       return Err(MemoryError::SizeTooLarge { 
                           requested: size, 
                           max_size: 65536 
                       });
                   }
                   LinearMemory::new_bounded(size, protection)
               }
               
               Err(e) => return Err(e),
           }?;

           self.total_allocated += size;
           self.regions.push(MemoryRegion {
               base: memory.base_address(),
               size,
               protection,
           })?;

           Ok(memory)
       }
   }

Memory Pool Management (no_alloc)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

In no_alloc environments, memory is managed through pre-allocated pools:

.. code-block:: rust

   /// Memory pool for no_alloc environments
   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct MemoryPool {
       /// Pool of memory blocks
       blocks: heapless::pool::Pool<MemoryBlock>,
       /// Block size for this pool
       block_size: usize,
       /// Total blocks in pool
       total_blocks: usize,
       /// Currently allocated blocks
       allocated_blocks: usize,
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct MemoryBlock {
       data: [u8; 4096], // 4KB blocks
       in_use: bool,
       protection: MemoryProtection,
   }

   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   impl MemoryPool {
       /// Create memory pool with fixed capacity
       pub const fn new(block_size: usize, block_count: usize) -> Self {
           Self {
               blocks: heapless::pool::Pool::new(),
               block_size,
               total_blocks: block_count,
               allocated_blocks: 0,
           }
       }
       
       /// Allocate memory block from pool
       pub fn allocate_block(&mut self) -> Result<&mut MemoryBlock, MemoryError> {
           if self.allocated_blocks >= self.total_blocks {
               return Err(MemoryError::PoolExhausted);
           }
           
           let block = self.blocks.alloc()
               .map_err(|_| MemoryError::PoolExhausted)?;
           
           block.in_use = true;
           self.allocated_blocks += 1;
           Ok(block)
       }
   }

Component Resource Management
-----------------------------

Component Instance Resources
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Each component instance manages its own set of resources:

.. code-block:: rust

   /// Component-specific resource management
   pub struct ComponentResourceManager {
       component_id: ComponentId,
       
       /// Memory regions owned by this component
       memory_regions: BoundedVec<MemoryRegion>,
       
       /// Resources created by this component
       owned_resources: BoundedVec<ResourceId>,
       
       /// Resources imported by this component
       imported_resources: BoundedVec<ResourceId>,
       
       /// Resource usage limits
       limits: ResourceLimits,
       
       /// Current resource usage statistics
       usage: ResourceUsage,
   }

   /// Resource limits for component
   #[derive(Debug, Clone, Copy)]
   pub struct ResourceLimits {
       pub max_memory: usize,
       pub max_table_size: usize,
       pub max_function_calls: usize,
       pub max_host_calls: usize,
       pub max_execution_time: Option<Duration>,
   }

   /// Current resource usage tracking
   #[derive(Debug, Clone, Copy, Default)]
   pub struct ResourceUsage {
       pub memory_used: usize,
       pub table_entries_used: usize,
       pub function_calls_made: usize,
       pub host_calls_made: usize,
       pub execution_time: Duration,
   }

   impl ComponentResourceManager {
       /// Check if resource allocation is within limits
       pub fn check_allocation_allowed(
           &self,
           resource_type: ResourceCategory,
           size_hint: Option<usize>,
       ) -> Result<(), ResourceError> {
           match resource_type {
               ResourceCategory::Memory => {
                   let additional_memory = size_hint.unwrap_or(0);
                   if self.usage.memory_used + additional_memory > self.limits.max_memory {
                       return Err(ResourceError::MemoryLimitExceeded {
                           requested: additional_memory,
                           available: self.limits.max_memory - self.usage.memory_used,
                       });
                   }
               }
               ResourceCategory::Tables => {
                   if self.usage.table_entries_used >= self.limits.max_table_size {
                       return Err(ResourceError::TableLimitExceeded);
                   }
               }
               _ => {} // Other resource types
           }
           Ok(())
       }
   }

Resource Lifecycle Management
-----------------------------

Resource State Tracking
~~~~~~~~~~~~~~~~~~~~~~~

Resources go through a defined lifecycle with state tracking:

.. code-block:: rust

   /// Resource lifecycle states
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum ResourceState {
       /// Resource allocated but not yet initialized
       Allocated,
       /// Resource initialized and ready for use
       Ready,
       /// Resource currently being used
       Active { owner: ComponentId },
       /// Resource locked by a component
       Locked { lock_holder: ComponentId },
       /// Resource marked for cleanup
       PendingCleanup,
       /// Resource deallocated
       Deallocated,
   }

   /// Resource with lifecycle tracking
   pub struct ManagedResource {
       id: ResourceId,
       state: ResourceState,
       category: ResourceCategory,
       created_at: Timestamp,
       last_accessed: Timestamp,
       access_count: usize,
       
       #[cfg(any(feature = "std", feature = "alloc"))]
       data: Box<dyn Any>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       data_slot: usize, // Index into resource pool
   }

Resource Cleanup and Garbage Collection
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Resource cleanup is handled differently across environments:

.. code-block:: rust

   /// Resource cleanup manager
   pub struct ResourceCleanupManager {
       #[cfg(feature = "std")]
       cleanup_queue: std::collections::VecDeque<ResourceId>,
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       cleanup_queue: alloc::collections::VecDeque<ResourceId>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       cleanup_queue: heapless::Deque<ResourceId, 256>,
       
       cleanup_interval: Duration,
       last_cleanup: Timestamp,
   }

   impl ResourceCleanupManager {
       /// Perform resource cleanup cycle
       pub fn perform_cleanup(&mut self, resource_manager: &mut ResourceManager) -> Result<CleanupStats, ResourceError> {
           let mut stats = CleanupStats::default();
           let now = self.get_current_timestamp();
           
           // Process cleanup queue
           while let Some(resource_id) = self.cleanup_queue.pop_front() {
               match resource_manager.deallocate(resource_id) {
                   Ok(_) => {
                       stats.resources_cleaned += 1;
                       stats.memory_freed += resource_manager.get_resource_size(resource_id);
                   }
                   Err(e) => {
                       stats.cleanup_errors += 1;
                       // Re-queue for later cleanup
                       self.cleanup_queue.push_back(resource_id)?;
                   }
               }
           }
           
           // Update cleanup timestamp
           self.last_cleanup = now;
           Ok(stats)
       }
   }

Platform-Specific Resource Management
------------------------------------

Platform Resource Adapters
~~~~~~~~~~~~~~~~~~~~~~~~~~

Different platforms provide different resource management capabilities:

.. code-block:: rust

   /// Platform-specific resource management
   pub trait PlatformResourceProvider {
       type Handle;
       type Error;
       
       /// Allocate platform-specific resource
       fn allocate_platform_resource(
           &self,
           resource_type: PlatformResourceType,
           config: &PlatformResourceConfig,
       ) -> Result<Self::Handle, Self::Error>;
       
       /// Get platform resource capabilities
       fn get_capabilities(&self) -> PlatformResourceCapabilities;
   }

   /// Linux-specific resource management
   #[cfg(target_os = "linux")]
   pub struct LinuxResourceProvider {
       use_hugetlb: bool,
       use_numa: bool,
       memory_cgroups: Option<CgroupManager>,
   }

   /// QNX-specific resource management  
   #[cfg(target_os = "qnx")]
   pub struct QnxResourceProvider {
       memory_partitions: QnxPartitionManager,
       priority_inheritance: bool,
   }

   /// Embedded platform resource management
   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   pub struct EmbeddedResourceProvider {
       static_memory_regions: [StaticMemoryRegion; 8],
       interrupt_handlers: [Option<InterruptHandler>; 16],
   }

Resource Monitoring and Metrics
-------------------------------

Resource Usage Tracking
~~~~~~~~~~~~~~~~~~~~~~~

Resource usage is tracked for monitoring and optimization:

.. code-block:: rust

   /// Resource metrics collection
   #[derive(Debug, Clone, Default)]
   pub struct ResourceMetrics {
       /// Total allocations by category
       pub allocations_by_category: BoundedMap<ResourceCategory, usize>,
       
       /// Current memory usage
       pub current_memory_usage: usize,
       
       /// Peak memory usage
       pub peak_memory_usage: usize,
       
       /// Allocation success rate
       pub allocation_success_rate: f32,
       
       /// Average allocation time
       pub avg_allocation_time: Duration,
       
       /// Resource contention events
       pub contention_events: usize,
       
       /// Environment-specific metrics
       #[cfg(feature = "std")]
       pub heap_fragmentation: f32,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       pub pool_utilization: [f32; 8], // Per-pool utilization
   }

   /// Resource metrics collector
   pub struct ResourceMetricsCollector {
       metrics: ResourceMetrics,
       collection_interval: Duration,
       last_collection: Timestamp,
   }

   impl ResourceMetricsCollector {
       /// Collect current resource metrics
       pub fn collect_metrics(&mut self, resource_manager: &ResourceManager) -> &ResourceMetrics {
           let now = self.get_current_timestamp();
           
           // Update allocation counts
           for category in ResourceCategory::iter() {
               let count = resource_manager.get_allocation_count(category);
               self.metrics.allocations_by_category.insert(category, count);
           }
           
           // Update memory metrics
           self.metrics.current_memory_usage = resource_manager.get_total_memory_usage();
           if self.metrics.current_memory_usage > self.metrics.peak_memory_usage {
               self.metrics.peak_memory_usage = self.metrics.current_memory_usage;
           }
           
           // Environment-specific metrics collection
           #[cfg(feature = "std")]
           {
               self.metrics.heap_fragmentation = resource_manager.calculate_heap_fragmentation();
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               for (i, pool) in resource_manager.get_memory_pools().iter().enumerate() {
                   self.metrics.pool_utilization[i] = pool.get_utilization();
               }
           }
           
           self.last_collection = now;
           &self.metrics
       }
   }

Resource Optimization Strategies
--------------------------------

Environment-Specific Optimizations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Different environments enable different optimization strategies:

.. list-table:: Resource Optimization by Environment
   :header-rows: 1
   :widths: 25 25 25 25

   * - Optimization
     - std Environment
     - no_std+alloc Environment
     - no_std+no_alloc Environment
   * - Memory pooling
     - Custom allocators
     - Custom allocators
     - Pre-allocated pools
   * - Resource reuse
     - Reference counting
     - Reference counting
     - Manual lifecycle
   * - Lazy allocation
     - On-demand allocation
     - On-demand allocation
     - Pre-allocation only
   * - Memory compaction
     - Runtime compaction
     - Runtime compaction
     - Static layout
   * - Cache optimization
     - Dynamic sizing
     - Dynamic sizing
     - Fixed cache sizes

**Optimization Implementation Example**:

.. code-block:: rust

   /// Resource optimization manager
   pub struct ResourceOptimizer {
       #[cfg(feature = "std")]
       allocation_tracker: AllocationTracker,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       pool_optimizer: PoolOptimizer,
   }

   impl ResourceOptimizer {
       /// Optimize resource allocation patterns
       pub fn optimize_allocations(&mut self, metrics: &ResourceMetrics) -> OptimizationResult {
           #[cfg(feature = "std")]
           {
               // Dynamic environment optimizations
               self.optimize_heap_layout(metrics);
               self.adjust_allocation_strategies(metrics);
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               // Static environment optimizations
               self.optimize_pool_sizes(metrics);
               self.rebalance_pool_allocations(metrics);
           }
           
           OptimizationResult {
               memory_saved: self.calculate_memory_savings(),
               allocation_time_improved: self.calculate_time_savings(),
               fragmentation_reduced: self.calculate_fragmentation_reduction(),
           }
       }
   }

Cross-References
-----------------

.. seealso::

   * :doc:`memory_budgets` for detailed memory allocation strategies
   * :doc:`cpu_budgets` for CPU resource management
   * :doc:`io_constraints` for I/O resource constraints
   * :doc:`../01_architectural_design/patterns` for resource management patterns
   * :doc:`../03_interfaces/internal` for resource management interfaces