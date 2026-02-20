//! Canonical ABI realloc function handling
//!
//! This module implements the realloc function support for the WebAssembly
//! Component Model's Canonical ABI, enabling dynamic memory allocation
//! during lifting and lowering operations.
//!
//! ## Implementation Status
//!
//! The `ReallocManager` tracks allocation metadata but requires a `ReallocCallback`
//! to actually call the WebAssembly `cabi_realloc` function. Without a registered
//! callback, allocation attempts will fail with an error.
//!
//! To properly allocate memory:
//! 1. Register a `ReallocCallback` using `set_realloc_callback()`
//! 2. The callback receives (old_ptr, old_size, align, new_size) and returns new_ptr
//! 3. The callback should invoke the component's exported `cabi_realloc` function

use wrt_error::Result;
use wrt_error::{Error, ErrorCategory, codes};
use wrt_foundation::{
    bounded::MAX_COMPONENT_TYPES, budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec, safe_managed_alloc,
};

// Type aliases for no_std compatibility
pub use crate::types::ComponentInstanceId;

/// Legacy type alias - prefer ReallocCallback
pub type ReallocFn = fn(i32, i32, i32, i32) -> i32;

/// Callback for invoking the actual wasm cabi_realloc function
///
/// The callback receives:
/// - `old_ptr`: Previous pointer (0 for new allocation)
/// - `old_size`: Previous size (0 for new allocation)
/// - `align`: Required alignment (must be power of 2)
/// - `new_size`: New size (0 for deallocation)
///
/// Returns: New pointer on success, or error
#[cfg(feature = "std")]
pub type ReallocCallback = Box<dyn FnMut(i32, i32, i32, i32) -> Result<i32> + Send>;

/// Binary std/no_std choice
#[derive(Debug, Clone)]
pub struct CanonicalOptionsWithRealloc {
    /// Memory for canonical operations
    pub memory: u32,
    /// Binary std/no_std choice
    pub realloc: Option<u32>,
    /// Post-return function index
    pub post_return: Option<u32>,
    /// String encoding
    pub string_encoding: StringEncoding,
    /// Component instance ID
    pub instance_id: ComponentInstanceId,
}

/// String encoding options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
}

/// Manages memory allocation for Component Model canonical ABI operations
///
/// Tracks allocations per component instance and delegates actual memory
/// allocation to a registered callback that invokes `cabi_realloc`.
pub struct ReallocManager {
    /// Allocations tracked per component instance
    allocations: BoundedVec<(ComponentInstanceId, InstanceAllocations), 32>,
    /// Allocation statistics
    metrics: AllocationMetrics,
    /// Maximum single allocation size (bytes)
    max_allocation_size: usize,
    /// Maximum allocations per instance
    max_instance_allocations: usize,
    /// Callback to invoke actual wasm cabi_realloc function
    #[cfg(feature = "std")]
    realloc_callback: Option<ReallocCallback>,
}

impl core::fmt::Debug for ReallocManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut s = f.debug_struct("ReallocManager");
        s.field("allocations", &self.allocations)
            .field("metrics", &self.metrics)
            .field("max_allocation_size", &self.max_allocation_size)
            .field("max_instance_allocations", &self.max_instance_allocations);
        #[cfg(feature = "std")]
        s.field(
            "realloc_callback",
            &self.realloc_callback.as_ref().map(|_| "<callback>"),
        );
        s.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstanceAllocations {
    /// Binary std/no_std choice
    allocations: BoundedVec<Allocation, MAX_COMPONENT_TYPES>,
    /// Binary std/no_std choice
    total_bytes: usize,
    /// Binary std/no_std choice
    realloc_fn: Option<ReallocFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Allocation {
    /// Binary std/no_std choice
    ptr: i32,
    /// Binary std/no_std choice
    size: i32,
    /// Alignment requirement
    align: i32,
    /// Binary std/no_std choice
    active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ReallocFunction {
    /// Function index in the component
    func_index: u32,
    /// Cached function reference for performance (simplified for no_std)
    func_available: bool,
}

#[derive(Debug, Default, Clone)]
pub struct AllocationMetrics {
    /// Binary std/no_std choice
    total_allocations: u64,
    /// Binary std/no_std choice
    total_deallocations: u64,
    /// Binary std/no_std choice
    total_bytes_allocated: u64,
    /// Binary std/no_std choice
    total_bytes_deallocated: u64,
    /// Peak memory usage
    peak_memory_usage: u64,
    /// Binary std/no_std choice
    failed_allocations: u64,
}

impl ReallocManager {
    /// Create a new ReallocManager with specified limits
    ///
    /// Note: You must call `set_realloc_callback()` before allocating memory,
    /// otherwise allocations will fail with an error.
    pub fn new(max_allocation_size: usize, max_instance_allocations: usize) -> Result<Self> {
        Ok(Self {
            allocations: BoundedVec::new(),
            metrics: AllocationMetrics::default(),
            max_allocation_size,
            max_instance_allocations,
            #[cfg(feature = "std")]
            realloc_callback: None,
        })
    }

    /// Set the callback for invoking the actual wasm cabi_realloc function
    ///
    /// The callback should:
    /// 1. Find the `cabi_realloc` export in the component instance
    /// 2. Call it with (old_ptr, old_size, align, new_size)
    /// 3. Return the resulting pointer
    ///
    /// # Example
    /// ```ignore
    /// manager.set_realloc_callback(Box::new(move |old_ptr, old_size, align, new_size| {
    ///     engine.call_function(instance_id, realloc_idx, vec![
    ///         Value::I32(old_ptr),
    ///         Value::I32(old_size),
    ///         Value::I32(align),
    ///         Value::I32(new_size),
    ///     ])
    /// }));
    /// ```
    #[cfg(feature = "std")]
    pub fn set_realloc_callback(&mut self, callback: ReallocCallback) {
        self.realloc_callback = Some(callback);
    }

    /// Check if a realloc callback is registered
    #[cfg(feature = "std")]
    pub fn has_realloc_callback(&self) -> bool {
        self.realloc_callback.is_some()
    }

    /// Binary std/no_std choice
    pub fn register_realloc(
        &mut self,
        instance_id: ComponentInstanceId,
        func_index: u32,
    ) -> Result<()> {
        // Find existing instance or create new one
        let mut found = false;
        for (id, instance_allocs) in &mut self.allocations {
            if *id == instance_id {
                instance_allocs.realloc_fn = Some(ReallocFunction {
                    func_index,
                    func_available: true,
                });
                found = true;
                break;
            }
        }

        if !found {
            let instance_allocs = InstanceAllocations {
                allocations: BoundedVec::new(),
                total_bytes: 0,
                realloc_fn: Some(ReallocFunction {
                    func_index,
                    func_available: true,
                }),
            };
            self.allocations
                .push((instance_id, instance_allocs))
                .map_err(|_| Error::capacity_exceeded("too_many_allocations"))?;
        }

        Ok(())
    }

    /// Allocate memory in the component instance
    ///
    /// This calls the registered `realloc_callback` to invoke the actual
    /// `cabi_realloc(0, 0, align, size)` WebAssembly function.
    ///
    /// # Errors
    /// - Returns error if no realloc callback is registered
    /// - Returns error if allocation limits are exceeded
    /// - Returns error if the wasm cabi_realloc call fails
    pub fn allocate(
        &mut self,
        instance_id: ComponentInstanceId,
        size: i32,
        align: i32,
    ) -> Result<i32> {
        self.validate_allocation(size, align)?;

        // Check limits first before getting mutable borrow
        {
            let instance_allocs_check = self
                .allocations
                .iter()
                .find(|(id, _)| *id == instance_id)
                .map(|(_, allocs)| allocs)
                .ok_or(Error::resource_not_found(
                    "Instance not registered for allocation",
                ))?;

            if instance_allocs_check.allocations.len() >= self.max_instance_allocations {
                self.metrics.failed_allocations += 1;
                return Err(Error::capacity_exceeded(
                    "Maximum allocations per instance exceeded",
                ));
            }
        }

        // Call the actual wasm cabi_realloc function via callback
        // cabi_realloc(0, 0, align, size) for new allocation
        #[cfg(feature = "std")]
        let ptr = {
            let callback = self.realloc_callback.as_mut().ok_or_else(|| {
                self.metrics.failed_allocations += 1;
                Error::new(
                    ErrorCategory::Core,
                    codes::CANONICAL_ABI_ERROR,
                    "No realloc callback registered - cannot allocate memory. \
                     Call set_realloc_callback() with a callback that invokes cabi_realloc.",
                )
            })?;

            if size == 0 {
                0 // Zero-size allocation returns null pointer
            } else {
                // Call cabi_realloc(old_ptr=0, old_size=0, align, new_size)
                callback(0, 0, align, size)?
            }
        };

        #[cfg(not(feature = "std"))]
        let ptr = {
            // In no_std mode, we need a different approach
            // For now, fail loud - no fake pointers
            self.metrics.failed_allocations += 1;
            return Err(Error::new(
                ErrorCategory::Core,
                codes::CANONICAL_ABI_ERROR,
                "Memory allocation not supported in no_std mode without callback",
            ));
        };

        // Record the allocation
        let instance_allocs = self
            .allocations
            .iter_mut()
            .find(|(id, _)| *id == instance_id)
            .map(|(_, allocs)| allocs)
            .ok_or(Error::resource_not_found("Instance not found"))?;

        let allocation = Allocation {
            ptr,
            size,
            align,
            active: true,
        };

        instance_allocs
            .allocations
            .push(allocation)
            .map_err(|_| Error::capacity_exceeded("Too many allocations"))?;

        instance_allocs.total_bytes += size as usize;

        // Update metrics
        self.metrics.total_allocations += 1;
        self.metrics.total_bytes_allocated += size as u64;
        self.update_peak_memory();

        Ok(ptr)
    }

    /// Binary std/no_std choice
    pub fn reallocate(
        &mut self,
        instance_id: ComponentInstanceId,
        old_ptr: i32,
        old_size: i32,
        align: i32,
        new_size: i32,
    ) -> Result<i32> {
        // Binary std/no_std choice
        self.validate_allocation(new_size, align)?;

        // Find allocation index first
        let alloc_index = {
            let instance_allocs_check = self
                .allocations
                .iter()
                .find(|(id, _)| *id == instance_id)
                .map(|(_, allocs)| allocs)
                .ok_or(Error::resource_not_found("resource_not_found"))?;

            instance_allocs_check
                .allocations
                .iter()
                .position(|a| a.ptr == old_ptr && a.size == old_size && a.active)
                .ok_or(Error::resource_not_found("resource_not_found"))?
        };

        // Validate that a realloc function is registered (using a scoped borrow)
        {
            let instance_allocs = self
                .allocations
                .iter()
                .find(|(id, _)| *id == instance_id)
                .map(|(_, allocs)| allocs)
                .ok_or(Error::resource_not_found("Instance not registered"))?;

            let _realloc_fn = instance_allocs.realloc_fn.as_ref().ok_or(
                Error::resource_not_found("No realloc function registered for instance"),
            )?;
        }
        // Borrow is released here

        // Call the actual wasm cabi_realloc function via callback
        #[cfg(feature = "std")]
        let new_ptr = {
            let callback = self.realloc_callback.as_mut().ok_or_else(|| {
                self.metrics.failed_allocations += 1;
                Error::new(
                    ErrorCategory::Core,
                    codes::CANONICAL_ABI_ERROR,
                    "No realloc callback registered - cannot reallocate memory. \
                     Call set_realloc_callback() with a callback that invokes cabi_realloc.",
                )
            })?;

            // Call cabi_realloc(old_ptr, old_size, align, new_size)
            callback(old_ptr, old_size, align, new_size)?
        };

        #[cfg(not(feature = "std"))]
        let new_ptr = {
            // In no_std mode, fail loud - no fake pointers
            self.metrics.failed_allocations += 1;
            return Err(Error::new(
                ErrorCategory::Core,
                codes::CANONICAL_ABI_ERROR,
                "Memory reallocation not supported in no_std mode without callback",
            ));
        };

        // Re-borrow for updating allocation tracking
        let instance_allocs = self
            .allocations
            .iter_mut()
            .find(|(id, _)| *id == instance_id)
            .map(|(_, allocs)| allocs)
            .ok_or(Error::resource_not_found(
                "Instance not found after realloc",
            ))?;

        // Binary std/no_std choice
        if new_size == 0 {
            // Binary std/no_std choice
            instance_allocs.allocations[alloc_index].active = false;
            instance_allocs.total_bytes -= old_size as usize;
            self.metrics.total_deallocations += 1;
            self.metrics.total_bytes_deallocated += old_size as u64;
        } else {
            // Binary std/no_std choice
            instance_allocs.allocations[alloc_index].ptr = new_ptr;
            instance_allocs.allocations[alloc_index].size = new_size;
            instance_allocs.total_bytes =
                instance_allocs.total_bytes - (old_size as usize) + (new_size as usize);
            self.metrics.total_bytes_allocated += (new_size - old_size).max(0) as u64;
        }

        self.update_peak_memory();
        Ok(new_ptr)
    }

    /// Binary std/no_std choice
    pub fn deallocate(
        &mut self,
        instance_id: ComponentInstanceId,
        ptr: i32,
        size: i32,
        align: i32,
    ) -> Result<()> {
        self.reallocate(instance_id, ptr, size, align, 0)?;
        Ok(())
    }

    /// Legacy helper - deprecated in favor of callback-based allocation
    ///
    /// This method is no longer used. Use `set_realloc_callback()` and then
    /// call `allocate()` or `reallocate()` which will invoke the callback.
    #[allow(dead_code)]
    fn call_realloc(
        &self,
        instance_allocs: &InstanceAllocations,
        _old_ptr: i32,
        _old_size: i32,
        _align: i32,
        _new_size: i32,
    ) -> Result<i32> {
        // Validate that realloc function is registered
        let _realloc_fn = instance_allocs
            .realloc_fn
            .as_ref()
            .ok_or(Error::resource_not_found("No realloc function registered"))?;

        // FAIL LOUD: This method cannot invoke wasm functions.
        // Use the callback-based approach via allocate()/reallocate() instead.
        Err(Error::new(
            ErrorCategory::Core,
            codes::CANONICAL_ABI_ERROR,
            "call_realloc is deprecated - use callback-based allocation via \
             set_realloc_callback() and allocate()/reallocate()",
        ))
    }

    /// Binary std/no_std choice
    fn validate_allocation(&self, size: i32, align: i32) -> Result<()> {
        if size < 0 {
            return Err(Error::validation_error("type_mismatch"));
        }

        if size as usize > self.max_allocation_size {
            return Err(Error::resource_not_found("resource_not_found"));
        }

        // Check alignment is power of 2
        if align <= 0 || (align & (align - 1)) != 0 {
            return Err(Error::validation_error("type_mismatch"));
        }

        Ok(())
    }

    /// Update peak memory usage
    fn update_peak_memory(&mut self) {
        let current_usage: u64 = self.allocations.iter().map(|(_, a)| a.total_bytes as u64).sum();

        if current_usage > self.metrics.peak_memory_usage {
            self.metrics.peak_memory_usage = current_usage;
        }
    }

    /// Binary std/no_std choice
    pub fn cleanup_instance(&mut self, instance_id: ComponentInstanceId) -> Result<()> {
        // Find and remove the instance
        if let Some(pos) = self.allocations.iter().position(|(id, _)| *id == instance_id) {
            let (_, instance_allocs) = self.allocations.remove(pos);
            // Update metrics for cleanup
            for alloc in instance_allocs.allocations.iter() {
                if alloc.active {
                    self.metrics.total_deallocations += 1;
                    self.metrics.total_bytes_deallocated += alloc.size as u64;
                }
            }
        }
        Ok(())
    }

    /// Binary std/no_std choice
    pub fn metrics(&self) -> &AllocationMetrics {
        &self.metrics
    }

    /// Reset metrics
    pub fn reset_metrics(&mut self) {
        self.metrics = AllocationMetrics::default();
    }
}

/// Memory layout for allocation calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLayout {
    pub size: usize,
    pub align: usize,
}

/// Binary std/no_std choice
pub mod helpers {
    use super::*;

    /// Binary std/no_std choice
    pub fn align_size(size: usize, align: usize) -> usize {
        (size + align - 1) & !(align - 1)
    }

    /// Validate pointer alignment
    pub fn is_aligned(ptr: i32, align: i32) -> bool {
        (ptr & (align - 1)) == 0
    }

    /// Binary std/no_std choice
    pub fn calculate_allocation_size(layout: &MemoryLayout, count: usize) -> Result<usize> {
        let item_size = layout.size;
        let align = layout.align;

        // Check for overflow
        let total_size =
            item_size.checked_mul(count).ok_or(Error::validation_error("type_mismatch"))?;

        // Add alignment padding
        let aligned_size = align_size(total_size, align);

        Ok(aligned_size)
    }
}

impl ReallocManager {
    fn default() -> Result<Self> {
        Self::new(
            10 * 1024 * 1024, // Binary std/no_std choice
            1024,             // Binary std/no_std choice
        )
    }
}

/// Helper functions for connecting ReallocManager to runtime engines
#[cfg(feature = "std")]
pub mod engine_integration {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Trait for engines that can provide cabi_realloc functionality
    ///
    /// This trait allows the ReallocManager to call back into the engine
    /// to invoke the component's cabi_realloc export.
    pub trait ReallocProvider {
        /// Call the cabi_realloc function in a component instance
        ///
        /// # Arguments
        /// * `instance_id` - The component instance ID
        /// * `old_ptr` - Previous pointer (0 for new allocation)
        /// * `old_size` - Previous size (0 for new allocation)
        /// * `align` - Required alignment (must be power of 2)
        /// * `new_size` - New size (0 for deallocation)
        ///
        /// # Returns
        /// The new pointer on success, or error
        fn call_cabi_realloc(
            &mut self,
            instance_id: usize,
            old_ptr: i32,
            old_size: i32,
            align: i32,
            new_size: i32,
        ) -> Result<i32>;

        /// Find the cabi_realloc export function index for an instance
        ///
        /// Returns Ok(func_idx) if cabi_realloc is exported, Err otherwise
        fn find_cabi_realloc(&self, instance_id: usize) -> Result<usize>;
    }

    /// Create a ReallocCallback that delegates to a ReallocProvider engine
    ///
    /// This creates a callback that can be registered with ReallocManager
    /// to invoke cabi_realloc through the provided engine.
    ///
    /// # Arguments
    /// * `engine` - Arc-wrapped Mutex-protected engine implementing ReallocProvider
    /// * `instance_id` - The component instance ID for realloc calls
    /// * `realloc_func_idx` - The function index of cabi_realloc in the instance
    ///
    /// # Example
    /// ```ignore
    /// use wrt_component::canonical_abi::canonical_realloc::engine_integration::*;
    ///
    /// // Find cabi_realloc export
    /// let realloc_idx = engine.find_cabi_realloc(instance_id)?;
    ///
    /// // Create and register callback
    /// let callback = create_realloc_callback(engine.clone(), instance_id, realloc_idx);
    /// realloc_manager.set_realloc_callback(callback);
    /// ```
    pub fn create_realloc_callback<E: ReallocProvider + Send + 'static>(
        engine: Arc<Mutex<E>>,
        instance_id: usize,
        _realloc_func_idx: usize,
    ) -> ReallocCallback {
        Box::new(
            move |old_ptr: i32, old_size: i32, align: i32, new_size: i32| -> Result<i32> {
                let mut engine_guard = engine
                    .lock()
                    .map_err(|_| Error::runtime_error("Failed to lock engine for cabi_realloc"))?;

                engine_guard.call_cabi_realloc(instance_id, old_ptr, old_size, align, new_size)
            },
        )
    }

    /// Set up a ReallocManager with proper callback for a component instance
    ///
    /// This is a convenience function that:
    /// 1. Finds the cabi_realloc export in the instance
    /// 2. Creates a callback that invokes it
    /// 3. Sets up the ReallocManager with proper configuration
    ///
    /// # Arguments
    /// * `engine` - Arc-wrapped Mutex-protected engine
    /// * `instance_id` - The component instance ID
    ///
    /// # Returns
    /// A configured ReallocManager ready for use with CanonicalABI
    ///
    /// # Errors
    /// Returns error if cabi_realloc is not exported or cannot be found
    pub fn setup_realloc_manager<E: ReallocProvider + Send + 'static>(
        engine: Arc<Mutex<E>>,
        instance_id: usize,
    ) -> Result<ReallocManager> {
        // Find cabi_realloc in the instance
        let realloc_idx = {
            let engine_guard = engine
                .lock()
                .map_err(|_| Error::runtime_error("Failed to lock engine to find cabi_realloc"))?;
            engine_guard.find_cabi_realloc(instance_id)?
        };

        // Create the manager
        let mut manager = ReallocManager::new(
            10 * 1024 * 1024, // 10MB max allocation
            1024,             // 1024 max allocations per instance
        )?;

        // Register the instance
        manager.register_realloc(
            ComponentInstanceId::new(instance_id as u32),
            realloc_idx as u32,
        )?;

        // Create and set the callback
        let callback = create_realloc_callback(engine, instance_id, realloc_idx);
        manager.set_realloc_callback(callback);

        Ok(manager)
    }
}
