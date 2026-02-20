//! Cross-component function calls
//!
//! This module implements the mechanism for calling functions between different
//! component instances, handling type adaptation and resource management.

#[cfg(not(feature = "std"))]
use core::{fmt, mem};
#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec};
#[cfg(feature = "std")]
use std::{fmt, mem};

use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    component_value::{ComponentValue, ValType as WrtComponentType},
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

use crate::prelude::*;
use crate::{
    canonical_abi::canonical::CanonicalABI,
    execution_engine::ComponentExecutionEngine,
    resources::resource_lifecycle::ResourceLifecycleManager,
    types::{ComponentInstance, ValType, Value},
};

/// Maximum number of call targets in no_std environments
const MAX_CALL_TARGETS: usize = 256;

/// Maximum call stack depth for cross-component calls
const MAX_CROSS_CALL_DEPTH: usize = 64;

/// Call site caching key for function lookup optimization
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallSiteKey {
    /// Source component instance ID
    pub source_instance: u32,
    /// Target component instance ID  
    pub target_instance: u32,
    /// Function name or index
    pub function_name: String,
    /// Function signature hash for type checking
    pub signature_hash: u64,
}

/// Cached call target with pre-resolved information
#[derive(Debug, Clone)]
pub struct CachedCallTarget {
    /// Target function index in the component
    pub function_index: u32,
    /// Pre-validated function signature
    pub signature: FunctionSignature,
    /// ABI adaptation information
    pub abi_adapter: Option<AbiAdapter>,
    /// Resource transfer requirements
    pub resource_requirements: ResourceRequirements,
    /// Last validation timestamp
    pub last_validated: u64,
    /// Hit count for cache optimization
    pub hit_count: u32,
}

/// Call statistics for performance optimization
#[derive(Debug, Clone, Default)]
pub struct CallStats {
    /// Total number of calls to this site
    pub call_count: u32,
    /// Average call duration in nanoseconds
    pub avg_duration_ns: u64,
    /// Last call timestamp
    pub last_call_time: u64,
    /// Whether this call site is eligible for inlining
    pub inline_eligible: bool,
}

/// Pending resource transfer for batch optimization
#[derive(Debug, Clone)]
pub struct PendingTransfer {
    /// Resource handle to transfer
    pub resource_handle: u32,
    /// Source component
    pub source_component: u32,
    /// Target component
    pub target_component: u32,
    /// Transfer type (move, copy, borrow)
    pub transfer_type: ResourceTransferType,
}

/// Function signature for type checking and caching
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    /// Parameter types
    pub params: Vec<ValType>,
    /// Return types
    pub results: Vec<ValType>,
    /// Whether function is async
    pub is_async: bool,
}

/// ABI adapter for handling type conversions
#[derive(Debug, Clone)]
pub struct AbiAdapter {
    /// Input type adaptations
    pub input_adaptations: Vec<TypeAdaptation>,
    /// Output type adaptations
    pub output_adaptations: Vec<TypeAdaptation>,
}

/// Type adaptation for ABI compatibility
#[derive(Debug, Clone)]
pub struct TypeAdaptation {
    /// Source type
    pub source_type: ValType,
    /// Target type
    pub target_type: ValType,
    /// Conversion function
    pub conversion_fn: String, // Function name for conversion
}

/// Resource requirements for call optimization
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    /// Required memory in bytes
    pub memory_required: usize,
    /// Maximum number of handles
    pub max_handles: u32,
    /// Whether exclusive access is needed
    pub exclusive_access: bool,
}

/// Resource transfer type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceTransferType {
    /// Move ownership
    Move,
    /// Copy resource
    Copy,
    /// Borrow reference
    Borrow,
}

/// Cross-component call manager with performance optimizations
pub struct CrossComponentCallManager {
    /// Call targets registry
    #[cfg(feature = "std")]
    targets: Vec<CallTarget>,
    #[cfg(not(any(feature = "std",)))]
    targets: BoundedVec<CallTarget, MAX_CALL_TARGETS>,

    /// Call stack for tracking cross-component calls
    #[cfg(feature = "std")]
    call_stack: Vec<CrossCallFrame>,
    #[cfg(not(any(feature = "std",)))]
    call_stack: BoundedVec<CrossCallFrame, MAX_CROSS_CALL_DEPTH>,

    /// Call site cache for frequently called functions
    #[cfg(feature = "std")]
    call_cache: std::collections::HashMap<CallSiteKey, CachedCallTarget>,
    #[cfg(not(any(feature = "std",)))]
    call_cache: BoundedVec<(CallSiteKey, CachedCallTarget), 128>,

    /// Call frequency tracking for optimization
    #[cfg(feature = "std")]
    call_frequency: std::collections::HashMap<CallSiteKey, CallStats>,
    #[cfg(not(any(feature = "std",)))]
    call_frequency: BoundedVec<(CallSiteKey, CallStats), 128>,

    /// Batch resource transfer buffer for optimization
    #[cfg(feature = "std")]
    pending_transfers: Vec<PendingTransfer>,
    #[cfg(not(any(feature = "std",)))]
    pending_transfers: BoundedVec<PendingTransfer, 64>,

    /// Canonical ABI processor
    canonical_abi: CanonicalABI,

    /// Resource manager for cross-component resource transfer
    resource_manager: ResourceLifecycleManager,

    /// Maximum call depth
    max_call_depth: usize,
}

/// Call target for cross-component calls
#[derive(Debug, Clone)]
pub struct CallTarget {
    /// Target component instance ID
    pub target_instance: u32,
    /// Target function index within the component
    pub function_index: u32,
    /// Function signature
    pub signature: WrtComponentType<NoStdProvider<4096>>,
    /// Call permissions
    pub permissions: CallPermissions,
    /// Resource transfer policy
    pub resource_policy: ResourceTransferPolicy,
}

/// Call permissions for cross-component calls
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallPermissions {
    /// Whether the call is allowed
    pub allowed: bool,
    /// Whether resources can be transferred
    pub allow_resource_transfer: bool,
    /// Whether memory access is allowed
    pub allow_memory_access: bool,
    /// Maximum call frequency (calls per second, 0 for unlimited)
    pub max_frequency: u32,
}

/// Resource transfer policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceTransferPolicy {
    /// No resource transfer allowed
    None,
    /// Transfer ownership
    Transfer,
    /// Borrow resources (caller retains ownership)
    Borrow,
    /// Copy resources (if possible)
    Copy,
}

/// Cross-component call frame
#[derive(Debug, Clone)]
pub struct CrossCallFrame {
    /// Caller instance ID
    pub caller_instance: u32,
    /// Target instance ID
    pub target_instance: u32,
    /// Function being called
    pub function_index: u32,
    /// Call start time (simplified - would use proper time type)
    pub start_time: u64,
    /// Resources transferred in this call
    #[cfg(feature = "std")]
    pub transferred_resources: Vec<TransferredResource>,
    #[cfg(not(any(feature = "std",)))]
    pub transferred_resources: BoundedVec<TransferredResource, 32>,
}

/// Record of a transferred resource
#[derive(Debug, Clone)]
pub struct TransferredResource {
    /// Resource handle
    pub handle: u32,
    /// Transfer type
    pub transfer_type: ResourceTransferPolicy,
    /// Original owner (for restoration on error)
    pub original_owner: u32,
}

/// Call result with resource tracking
#[derive(Debug, Clone)]
pub struct CrossCallResult {
    /// Function call result
    pub result: wrt_error::Result<Value>,
    /// Resources that were transferred
    #[cfg(feature = "std")]
    pub transferred_resources: Vec<TransferredResource>,
    #[cfg(not(any(feature = "std",)))]
    pub transferred_resources: BoundedVec<TransferredResource, 32>,
    /// Call statistics
    pub stats: CallStatistics,
}

/// Call statistics
#[derive(Debug, Clone)]
pub struct CallStatistics {
    /// Call duration in nanoseconds
    pub duration_ns: u64,
    /// Number of arguments
    pub arg_count: u32,
    /// Number of resources transferred
    pub resources_transferred: u32,
    /// Memory bytes accessed
    pub memory_accessed: u64,
}

impl CrossComponentCallManager {
    /// Create a new cross-component call manager
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            targets: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            targets: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            call_stack: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            call_stack: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            call_cache: std::collections::HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            call_cache: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            call_frequency: std::collections::HashMap::new(),
            #[cfg(not(any(feature = "std",)))]
            call_frequency: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            #[cfg(feature = "std")]
            pending_transfers: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            pending_transfers: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            canonical_abi: CanonicalABI::new(64),
            resource_manager: ResourceLifecycleManager::new(),
            max_call_depth: MAX_CROSS_CALL_DEPTH,
        })
    }

    /// Set maximum call depth
    pub fn set_max_call_depth(&mut self, depth: usize) {
        self.max_call_depth = depth;
    }

    /// Register a call target
    pub fn register_target(&mut self, target: CallTarget) -> wrt_error::Result<u32> {
        let target_id = self.targets.len() as u32;

        #[cfg(feature = "std")]
        {
            self.targets.push(target);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.targets
                .push(target)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many call targets"))?;
        }

        Ok(target_id)
    }

    /// Make a cross-component call
    pub fn call(
        &mut self,
        caller_instance: u32,
        target_id: u32,
        args: &[Value],
        engine: &mut ComponentExecutionEngine,
    ) -> wrt_error::Result<CrossCallResult> {
        // Check call depth
        if self.call_stack.len() >= self.max_call_depth {
            return Err(wrt_error::Error::resource_exhausted(
                "Maximum call depth exceeded",
            ));
        }

        // Get target
        let target = self
            .targets
            .get(target_id as usize)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))?
            .clone();

        // Check permissions
        if !target.permissions.allowed {
            return Err(wrt_error::Error::runtime_error(
                "Cross-component call not allowed",
            ));
        }

        // Create call frame
        let start_time = self.get_current_time();
        let call_frame = CrossCallFrame {
            caller_instance,
            target_instance: target.target_instance,
            function_index: target.function_index,
            start_time,
            #[cfg(feature = "std")]
            transferred_resources: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            transferred_resources: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
        };

        // Push call frame
        #[cfg(feature = "std")]
        {
            self.call_stack.push(call_frame);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.call_stack
                .push(call_frame)
                .map_err(|_| wrt_error::Error::resource_exhausted("Call stack overflow"))?;
        }

        // Prepare arguments with resource transfer
        let (prepared_args, transferred_resources) =
            self.prepare_arguments(args, &target, caller_instance)?;

        // Update call frame with transferred resources
        if let Some(frame) = self.call_stack.last_mut() {
            frame.transferred_resources = transferred_resources.clone();
        }

        // Make the actual call
        let call_result = engine.call_function(
            target.target_instance,
            target.function_index,
            prepared_args.as_slice(),
        );

        // Calculate statistics
        let end_time = self.get_current_time();
        let duration_ns = end_time - start_time;
        let stats = CallStatistics {
            duration_ns,
            arg_count: args.len() as u32,
            resources_transferred: transferred_resources.len() as u32,
            memory_accessed: 0, // Would be tracked by memory manager
        };

        // Update call statistics for optimization
        let signature_hash = self.calculate_signature_hash(&target.signature);
        self.update_call_stats(
            caller_instance,
            target.target_instance,
            &format!("func_{}", target.function_index), // Function name approximation
            signature_hash,
            duration_ns,
        );

        // Handle call result
        let result = match call_result {
            Ok(value) => {
                // Call succeeded - finalize resource transfers
                self.finalize_resource_transfers(transferred_resources.as_slice())?;
                CrossCallResult {
                    result: Ok(value),
                    transferred_resources,
                    stats,
                }
            },
            Err(error) => {
                // Call failed - restore resources
                self.restore_resources(transferred_resources.as_slice())?;
                CrossCallResult {
                    result: Err(error),
                    #[cfg(feature = "std")]
                    transferred_resources: Vec::new(),
                    #[cfg(not(any(feature = "std",)))]
                    transferred_resources: {
                        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                        BoundedVec::new().unwrap()
                    },
                    stats,
                }
            },
        };

        // Pop call frame
        #[cfg(feature = "std")]
        {
            self.call_stack.pop();
        }
        #[cfg(not(any(feature = "std",)))]
        {
            let _ = self.call_stack.pop();
        }

        Ok(result)
    }

    /// Prepare arguments for cross-component call
    #[cfg(feature = "std")]
    fn prepare_arguments(
        &mut self,
        args: &[Value],
        target: &CallTarget,
        caller_instance: u32,
    ) -> wrt_error::Result<(Vec<Value>, Vec<TransferredResource>)> {
        let mut prepared_args = Vec::new();
        let mut transferred_resources = Vec::new();

        for arg in args {
            match arg {
                Value::Own(handle) | Value::Borrow(handle) => {
                    // Handle resource arguments
                    if target.permissions.allow_resource_transfer {
                        let transfer_type = target.resource_policy;
                        let transferred = self.transfer_resource(
                            *handle,
                            caller_instance,
                            target.target_instance,
                            transfer_type,
                        )?;
                        transferred_resources.push(transferred);
                        prepared_args.push(arg.clone());
                    } else {
                        return Err(wrt_error::Error::runtime_error(
                            "Resource transfer not allowed",
                        ));
                    }
                },
                _ => {
                    // Regular value arguments
                    prepared_args.push(arg.clone());
                },
            }
        }

        Ok((prepared_args, transferred_resources))
    }

    #[cfg(not(any(feature = "std",)))]
    fn prepare_arguments(
        &mut self,
        args: &[Value],
        target: &CallTarget,
        caller_instance: u32,
    ) -> wrt_error::Result<(BoundedVec<Value, 32>, BoundedVec<TransferredResource, 32>)> {
        let mut prepared_args = BoundedVec::new();
        let mut transferred_resources = BoundedVec::new();

        for arg in args {
            match arg {
                Value::Own(handle) | Value::Borrow(handle) => {
                    // Handle resource arguments
                    if target.permissions.allow_resource_transfer {
                        let transfer_type = target.resource_policy;
                        let transferred = self.transfer_resource(
                            *handle,
                            caller_instance,
                            target.target_instance,
                            transfer_type,
                        )?;
                        transferred_resources.push(transferred).map_err(|_| {
                            wrt_error::Error::runtime_error("Too many transferred resources")
                        })?;
                        prepared_args
                            .push(arg.clone())
                            .map_err(|_| wrt_error::Error::runtime_error("Too many arguments"))?;
                    } else {
                        return Err(wrt_error::Error::runtime_error(
                            "Resource transfer not permitted",
                        ));
                    }
                },
                _ => {
                    prepared_args
                        .push(arg.clone())
                        .map_err(|_| wrt_error::Error::runtime_error("Too many arguments"))?;
                },
            }
        }

        Ok((prepared_args, transferred_resources))
    }

    /// Transfer a resource between components
    fn transfer_resource(
        &mut self,
        handle: u32,
        from_instance: u32,
        to_instance: u32,
        transfer_type: ResourceTransferPolicy,
    ) -> wrt_error::Result<TransferredResource> {
        match transfer_type {
            ResourceTransferPolicy::None => Err(wrt_error::Error::runtime_error(
                "Resource transfer not allowed",
            )),
            ResourceTransferPolicy::Transfer => {
                // Transfer ownership
                self.resource_manager.transfer_ownership(handle, from_instance, to_instance)?;
                Ok(TransferredResource {
                    handle,
                    transfer_type,
                    original_owner: from_instance,
                })
            },
            ResourceTransferPolicy::Borrow => {
                // Borrow resource (no ownership change)
                Ok(TransferredResource {
                    handle,
                    transfer_type,
                    original_owner: from_instance,
                })
            },
            ResourceTransferPolicy::Copy => {
                // Copy resource (if possible)
                // This would create a new resource with copied data
                // For now, treat as borrow
                Ok(TransferredResource {
                    handle,
                    transfer_type: ResourceTransferPolicy::Borrow,
                    original_owner: from_instance,
                })
            },
        }
    }

    /// Finalize resource transfers after successful call
    fn finalize_resource_transfers(
        &mut self,
        transfers: &[TransferredResource],
    ) -> wrt_error::Result<()> {
        for transfer in transfers {
            match transfer.transfer_type {
                ResourceTransferPolicy::Transfer => {
                    // Transfer is already finalized
                },
                ResourceTransferPolicy::Borrow => {
                    // Nothing to finalize for borrow
                },
                ResourceTransferPolicy::Copy => {
                    // Finalize copy (would commit the copied resource)
                },
                ResourceTransferPolicy::None => {
                    // Should not happen
                },
            }
        }
        Ok(())
    }

    /// Restore resources after failed call
    fn restore_resources(&mut self, transfers: &[TransferredResource]) -> wrt_error::Result<()> {
        for transfer in transfers {
            match transfer.transfer_type {
                ResourceTransferPolicy::Transfer => {
                    // Restore ownership to original owner (need to know current owner - use placeholder 0)
                    // Note: This may need refinement to track current owner
                    self.resource_manager.transfer_ownership(
                        transfer.handle,
                        0, // Current owner - would need to be tracked properly
                        transfer.original_owner,
                    )?;
                },
                ResourceTransferPolicy::Borrow => {
                    // Nothing to restore for borrow
                },
                ResourceTransferPolicy::Copy => {
                    // Remove copied resource (would clean up the copy)
                },
                ResourceTransferPolicy::None => {
                    // Should not happen
                },
            }
        }
        Ok(())
    }

    /// Get current time (simplified)
    fn get_current_time(&self) -> u64 {
        // In a real implementation, would use proper time measurement
        0
    }

    /// Get or create a cached call target for optimized execution
    fn get_cached_call_target(
        &mut self,
        caller_instance: u32,
        target_instance: u32,
        function_name: &str,
        signature_hash: u64,
    ) -> Option<&CachedCallTarget> {
        let key = CallSiteKey {
            source_instance: caller_instance,
            target_instance,
            function_name: function_name.to_string(),
            signature_hash,
        };

        #[cfg(feature = "std")]
        {
            // Update hit count first if entry exists
            if self.call_cache.contains_key(&key) {
                if let Some(mut_cached) = self.call_cache.get_mut(&key) {
                    mut_cached.hit_count += 1;
                }
            }
            self.call_cache.get(&key)
        }
        #[cfg(not(any(feature = "std",)))]
        {
            for (cached_key, cached_target) in &mut self.call_cache {
                if cached_key == &key {
                    cached_target.hit_count += 1;
                    return Some(cached_target);
                }
            }
            None
        }
    }

    /// Update call statistics for performance optimization
    fn update_call_stats(
        &mut self,
        caller_instance: u32,
        target_instance: u32,
        function_name: &str,
        signature_hash: u64,
        duration_ns: u64,
    ) {
        let key = CallSiteKey {
            source_instance: caller_instance,
            target_instance,
            function_name: function_name.to_string(),
            signature_hash,
        };

        #[cfg(feature = "std")]
        {
            // Extract current time before mutable borrow
            let current_time = self.get_current_time();

            let stats = self.call_frequency.entry(key).or_insert_with(CallStats::default);
            stats.call_count += 1;

            // Update running average
            let total_time = stats.avg_duration_ns * (stats.call_count - 1) as u64 + duration_ns;
            stats.avg_duration_ns = total_time / stats.call_count as u64;
            stats.last_call_time = current_time;

            // Determine if eligible for inlining (fast, frequent calls)
            stats.inline_eligible = stats.call_count > 10 && stats.avg_duration_ns < 1000;
            // < 1μs
        }
        #[cfg(not(any(feature = "std",)))]
        {
            // Find existing stats or add new ones (extract time first to avoid borrow conflict)
            let current_time = self.get_current_time();
            let mut found = false;
            for (stats_key, stats) in &mut self.call_frequency {
                if stats_key == &key {
                    stats.call_count += 1;
                    let total_time =
                        stats.avg_duration_ns * (stats.call_count - 1) as u64 + duration_ns;
                    stats.avg_duration_ns = total_time / stats.call_count as u64;
                    stats.last_call_time = current_time;
                    stats.inline_eligible = stats.call_count > 10 && stats.avg_duration_ns < 1000;
                    found = true;
                    break;
                }
            }

            if !found {
                let mut new_stats = CallStats::default();
                new_stats.call_count = 1;
                new_stats.avg_duration_ns = duration_ns;
                new_stats.last_call_time = self.get_current_time();
                let _ = self.call_frequency.push((key, new_stats));
            }
        }
    }

    /// Add pending resource transfer for batch optimization
    fn add_pending_transfer(
        &mut self,
        resource_handle: u32,
        source_component: u32,
        target_component: u32,
        transfer_type: ResourceTransferType,
    ) -> wrt_error::Result<()> {
        let transfer = PendingTransfer {
            resource_handle,
            source_component,
            target_component,
            transfer_type,
        };

        #[cfg(feature = "std")]
        {
            self.pending_transfers.push(transfer);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.pending_transfers
                .push(transfer)
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many pending transfers"))?;
        }

        Ok(())
    }

    /// Process batch resource transfers for optimization
    fn flush_pending_transfers(&mut self) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            if self.pending_transfers.is_empty() {
                return Ok(());
            }

            // Group transfers by target component for batch processing
            let mut transfers_by_target: std::collections::HashMap<u32, Vec<PendingTransfer>> =
                std::collections::HashMap::new();

            for transfer in self.pending_transfers.drain(..) {
                transfers_by_target
                    .entry(transfer.target_component)
                    .or_insert_with(Vec::new)
                    .push(transfer);
            }

            // Process each batch
            for (target_component, transfers) in transfers_by_target {
                // Batch resource transfers to the same target component
                for transfer in transfers {
                    match transfer.transfer_type {
                        ResourceTransferType::Move => {
                            self.resource_manager.transfer_ownership(
                                transfer.resource_handle,
                                transfer.source_component,
                                target_component,
                            )?;
                        },
                        ResourceTransferType::Copy => {
                            // Would implement resource copying here
                        },
                        ResourceTransferType::Borrow => {
                            // Borrowing doesn't require immediate action
                        },
                    }
                }
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            // Process transfers sequentially in no_std
            while let Some(transfer) = self.pending_transfers.pop() {
                match transfer.transfer_type {
                    ResourceTransferType::Move => {
                        self.resource_manager.transfer_ownership(
                            transfer.resource_handle,
                            transfer.source_component,
                            transfer.target_component,
                        )?;
                    },
                    ResourceTransferType::Copy => {
                        // Would implement resource copying here
                    },
                    ResourceTransferType::Borrow => {
                        // Borrowing doesn't require immediate action
                    },
                }
            }
        }

        Ok(())
    }

    /// Cache a call target for future optimized access
    fn cache_call_target(
        &mut self,
        key: CallSiteKey,
        function_index: u32,
        signature: FunctionSignature,
        abi_adapter: Option<AbiAdapter>,
        resource_requirements: ResourceRequirements,
    ) -> wrt_error::Result<()> {
        let cached_target = CachedCallTarget {
            function_index,
            signature,
            abi_adapter,
            resource_requirements,
            last_validated: self.get_current_time(),
            hit_count: 0,
        };

        #[cfg(feature = "std")]
        {
            self.call_cache.insert(key, cached_target);
        }
        #[cfg(not(any(feature = "std",)))]
        {
            self.call_cache
                .push((key, cached_target))
                .map_err(|_| wrt_error::Error::resource_exhausted("Call cache full"))?;
        }

        Ok(())
    }

    /// Calculate a hash of the function signature for caching
    fn calculate_signature_hash(&self, signature: &WrtComponentType<NoStdProvider<4096>>) -> u64 {
        // Simple hash implementation - in real implementation would use a proper hasher
        use core::hash::{Hash, Hasher};

        struct SimpleHasher(u64);

        impl Hasher for SimpleHasher {
            fn finish(&self) -> u64 {
                self.0
            }

            fn write(&mut self, bytes: &[u8]) {
                for &byte in bytes {
                    self.0 = self.0.wrapping_mul(31).wrapping_add(byte as u64);
                }
            }
        }

        let mut hasher = SimpleHasher(0);
        // For now, just hash a simple representation of the type
        match signature {
            WrtComponentType::Bool => 1u8.hash(&mut hasher),
            WrtComponentType::S8 => 2u8.hash(&mut hasher),
            WrtComponentType::U8 => 3u8.hash(&mut hasher),
            WrtComponentType::S16 => 4u8.hash(&mut hasher),
            WrtComponentType::U16 => 5u8.hash(&mut hasher),
            WrtComponentType::S32 => 6u8.hash(&mut hasher),
            WrtComponentType::U32 => 7u8.hash(&mut hasher),
            WrtComponentType::S64 => 8u8.hash(&mut hasher),
            WrtComponentType::U64 => 9u8.hash(&mut hasher),
            WrtComponentType::F32 => 10u8.hash(&mut hasher),
            WrtComponentType::F64 => 11u8.hash(&mut hasher),
            WrtComponentType::Char => 12u8.hash(&mut hasher),
            WrtComponentType::String => 13u8.hash(&mut hasher),
            WrtComponentType::Void => 0u8.hash(&mut hasher),
            // For complex types, would need more sophisticated hashing
            _ => 255u8.hash(&mut hasher),
        }
        hasher.finish()
    }

    /// Get call target by ID
    pub fn get_target(&self, target_id: u32) -> Option<&CallTarget> {
        self.targets.get(target_id as usize)
    }

    /// Get current call depth
    pub fn call_depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Check if a call from one instance to another is allowed
    pub fn is_call_allowed(&self, from_instance: u32, to_instance: u32) -> bool {
        // Check if there's a registered target for this call
        for target in &self.targets {
            if target.target_instance == to_instance {
                return target.permissions.allowed;
            }
        }
        false
    }

    /// Get call statistics for the current call
    pub fn current_call_stats(&self) -> Option<&CrossCallFrame> {
        self.call_stack.last()
    }
}

impl CallTarget {
    /// Create a new call target
    pub fn new(
        target_instance: u32,
        function_index: u32,
        signature: WrtComponentType<NoStdProvider<4096>>,
        permissions: CallPermissions,
        resource_policy: ResourceTransferPolicy,
    ) -> Self {
        Self {
            target_instance,
            function_index,
            signature,
            permissions,
            resource_policy,
        }
    }
}

impl Default for CallPermissions {
    fn default() -> Self {
        Self {
            allowed: true,
            allow_resource_transfer: false,
            allow_memory_access: false,
            max_frequency: 0, // Unlimited
        }
    }
}

impl Default for CrossComponentCallManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // In case of allocation failure, panic as this is a critical error
            panic!("Failed to create CrossComponentCallManager: memory allocation failed")
        })
    }
}

impl fmt::Display for ResourceTransferPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceTransferPolicy::None => write!(f, "none"),
            ResourceTransferPolicy::Transfer => write!(f, "transfer"),
            ResourceTransferPolicy::Borrow => write!(f, "borrow"),
            ResourceTransferPolicy::Copy => write!(f, "copy"),
        }
    }
}
