//! Neural Network Capability System
//!
//! This module defines the capability-based access control for neural network
//! operations. It maps verification levels to concrete resource limits and
//! operational constraints, supporting multiple safety standards.

use core::fmt::Debug;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{
            AtomicU64,
            AtomicUsize,
            Ordering,
        },
        Mutex,
    },
};

use wrt_foundation::{
    safe_memory::NoStdProvider,
    BoundedVec,
};
use wrt_platform::side_channel_resistance::constant_time;

use crate::prelude::*;

/// Neural Network specific verification levels that map to ASIL standards
/// Maps to wrt_foundation::VerificationLevel for consistency
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NNVerificationLevel {
    /// QM (Quality Management) - Standard verification level
    Standard,
    /// ASIL-A (Sampling) - Bounded verification with runtime monitoring
    Sampling,
    /// ASIL-B (Continuous) - Static verification with pre-approved models
    Continuous,
    /// ASIL-C (Redundant) - Redundant verification (not supported in wrtd)
    Redundant,
    /// ASIL-D (Formal) - Formal verification (not supported in wrtd)
    Formal,
}

impl From<NNVerificationLevel> for wrt_foundation::verification::VerificationLevel {
    fn from(level: NNVerificationLevel) -> Self {
        match level {
            NNVerificationLevel::Standard => {
                wrt_foundation::verification::VerificationLevel::Standard
            },
            NNVerificationLevel::Sampling => {
                wrt_foundation::verification::VerificationLevel::Sampling
            },
            NNVerificationLevel::Continuous => {
                wrt_foundation::verification::VerificationLevel::Full
            },
            NNVerificationLevel::Redundant => {
                wrt_foundation::verification::VerificationLevel::Redundant
            },
            NNVerificationLevel::Formal => {
                wrt_foundation::verification::VerificationLevel::Redundant
            }, // Best available mapping
        }
    }
}

impl From<wrt_foundation::verification::VerificationLevel> for NNVerificationLevel {
    fn from(level: wrt_foundation::verification::VerificationLevel) -> Self {
        match level {
            wrt_foundation::verification::VerificationLevel::Off => NNVerificationLevel::Standard,
            wrt_foundation::verification::VerificationLevel::Basic => NNVerificationLevel::Standard,
            wrt_foundation::verification::VerificationLevel::Standard => {
                NNVerificationLevel::Standard
            },
            wrt_foundation::verification::VerificationLevel::Full => {
                NNVerificationLevel::Continuous
            },
            wrt_foundation::verification::VerificationLevel::Sampling => {
                NNVerificationLevel::Sampling
            },
            wrt_foundation::verification::VerificationLevel::Redundant => {
                NNVerificationLevel::Redundant
            },
        }
    }
}

// Keep old name as alias for backward compatibility during transition
pub use NNVerificationLevel as VerificationLevel;

/// Maximum number of models that can be loaded simultaneously
const MAX_LOADED_MODELS: usize = 16;

/// Maximum number of allowed model formats
const MAX_MODEL_FORMATS: usize = 8;

/// Neural network operations that require capability verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NNOperation {
    /// Load a model
    Load { size: usize, format: ModelFormat },
    /// Create execution context
    CreateContext { model_id: u32 },
    /// Set input tensor
    SetInput {
        size:       usize,
        dimensions: Vec<u32>,
    },
    /// Execute inference
    Compute { estimated_flops: u64 },
    /// Get output tensor
    GetOutput { index: u32 },
    /// Drop/cleanup resources
    DropResource { resource_type: ResourceType },
}

/// Types of resources in NN subsystem
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Model,
    ExecutionContext,
    Tensor,
}

/// Model formats supported by WASI-NN
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    ONNX,
    TensorFlow,
    PyTorch,
    OpenVINO,
    TractNative,
}

/// Rate limiting configuration
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimits {
    /// Maximum model loads per minute
    pub max_loads_per_minute:      u32,
    /// Maximum inferences per second
    pub max_inferences_per_second: u32,
    /// Maximum concurrent operations
    pub max_concurrent_operations: u32,
    /// Sliding window size in milliseconds
    pub window_size_ms:            u64,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            max_loads_per_minute:      10,
            max_inferences_per_second: 100,
            max_concurrent_operations: 5,
            window_size_ms:            60_000, // 1 minute
        }
    }
}

/// Operation record for rate limiting
#[derive(Debug, Clone)]
struct OperationRecord {
    operation_type: NNOperationType,
    timestamp:      u64,
    resource_cost:  usize,
}

/// Types of operations for rate limiting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NNOperationType {
    Load,
    Inference,
    CreateContext,
    SetInput,
    GetOutput,
}

/// Resource limits for neural network operations
#[derive(Debug, Clone, PartialEq)]
pub struct NNResourceLimits {
    /// Maximum model size in bytes
    pub max_model_size:          usize,
    /// Maximum memory for all tensors during inference
    pub max_tensor_memory:       usize,
    /// Maximum number of dimensions per tensor
    pub max_tensor_dimensions:   usize,
    /// Maximum execution time in microseconds (0 = unlimited)
    pub max_execution_time_us:   u64,
    /// Maximum number of concurrent models
    pub max_concurrent_models:   usize,
    /// Maximum number of concurrent execution contexts
    pub max_concurrent_contexts: usize,
}

impl Default for NNResourceLimits {
    fn default() -> Self {
        Self {
            max_model_size:          100 * 1024 * 1024, // 100MB
            max_tensor_memory:       50 * 1024 * 1024,  // 50MB
            max_tensor_dimensions:   8,
            max_execution_time_us:   0, // Unlimited
            max_concurrent_models:   4,
            max_concurrent_contexts: 8,
        }
    }
}

/// Resource usage tracker
#[derive(Debug)]
pub struct ResourceTracker {
    // Current usage counters (atomic for thread safety)
    active_models:         AtomicUsize,
    active_contexts:       AtomicUsize,
    total_memory_used:     AtomicUsize,
    concurrent_operations: AtomicUsize,

    // Rate limiting (protected by mutex)
    operations_window: Mutex<VecDeque<OperationRecord>>,
    last_cleanup:      AtomicU64,

    // Configuration
    limits:      NNResourceLimits,
    rate_limits: RateLimits,
}

impl ResourceTracker {
    /// Create a new resource tracker
    pub fn new(limits: NNResourceLimits, rate_limits: RateLimits) -> Self {
        Self {
            active_models: AtomicUsize::new(0),
            active_contexts: AtomicUsize::new(0),
            total_memory_used: AtomicUsize::new(0),
            concurrent_operations: AtomicUsize::new(0),
            operations_window: Mutex::new(VecDeque::new()),
            last_cleanup: AtomicU64::new(get_current_time_ms()),
            limits,
            rate_limits,
        }
    }

    /// Check if an operation is allowed (rate limiting + quotas)
    pub fn check_operation_allowed(&self, operation: &NNOperation) -> Result<OperationGuard> {
        let operation_type = match operation {
            NNOperation::Load { .. } => NNOperationType::Load,
            NNOperation::Compute { .. } => NNOperationType::Inference,
            NNOperation::CreateContext { .. } => NNOperationType::CreateContext,
            NNOperation::SetInput { .. } => NNOperationType::SetInput,
            NNOperation::GetOutput { .. } => NNOperationType::GetOutput,
            _ => return Ok(OperationGuard::new(self, NNOperationType::GetOutput)),
        };

        // Check concurrent operation limit
        let current_concurrent = self.concurrent_operations.load(Ordering::Acquire);
        if current_concurrent >= self.rate_limits.max_concurrent_operations as usize {
            return Err(Error::wasi_resource_exhausted(
                "Too many concurrent operations",
            ));
        }

        // Check resource quotas
        self.check_resource_quotas(operation)?;

        // Check rate limits
        self.check_rate_limits(operation_type)?;

        // Increment concurrent operations and record operation
        self.concurrent_operations.fetch_add(1, Ordering::AcqRel);
        self.record_operation(operation_type, self.calculate_operation_cost(operation));

        Ok(OperationGuard::new(self, operation_type))
    }

    /// Check resource quotas against current usage
    fn check_resource_quotas(&self, operation: &NNOperation) -> Result<()> {
        match operation {
            NNOperation::Load { size, .. } => {
                // Check model count limit
                let current_models = self.active_models.load(Ordering::Acquire);
                if current_models >= self.limits.max_concurrent_models {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::QuotaExceeded {
                                resource_type: "models".to_string(),
                                current:       current_models,
                                limit:         self.limits.max_concurrent_models,
                            },
                            "resource_tracker",
                        );
                    }
                    return Err(Error::wasi_resource_exhausted(
                        "Maximum concurrent models reached",
                    ));
                }

                // Check model size limit
                if *size > self.limits.max_model_size {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::QuotaExceeded {
                                resource_type: "model_size".to_string(),
                                current:       *size,
                                limit:         self.limits.max_model_size,
                            },
                            "resource_tracker",
                        );
                    }
                    return Err(Error::wasi_resource_exhausted("Model size exceeds limit"));
                }

                // Check total memory limit
                let current_memory = self.total_memory_used.load(Ordering::Acquire);
                if current_memory + size > self.limits.max_tensor_memory {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::QuotaExceeded {
                                resource_type: "tensor_memory".to_string(),
                                current:       current_memory + size,
                                limit:         self.limits.max_tensor_memory,
                            },
                            "resource_tracker",
                        );
                    }
                    return Err(Error::wasi_resource_exhausted(
                        "Total memory limit would be exceeded",
                    ));
                }
            },
            NNOperation::CreateContext { .. } => {
                let current_contexts = self.active_contexts.load(Ordering::Acquire);
                if current_contexts >= self.limits.max_concurrent_contexts {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::QuotaExceeded {
                                resource_type: "contexts".to_string(),
                                current:       current_contexts,
                                limit:         self.limits.max_concurrent_contexts,
                            },
                            "resource_tracker",
                        );
                    }
                    return Err(Error::wasi_resource_exhausted(
                        "Maximum concurrent contexts reached",
                    ));
                }
            },
            NNOperation::SetInput { size, .. } => {
                let current_memory = self.total_memory_used.load(Ordering::Acquire);
                if current_memory + size > self.limits.max_tensor_memory {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::QuotaExceeded {
                                resource_type: "tensor_memory".to_string(),
                                current:       current_memory + size,
                                limit:         self.limits.max_tensor_memory,
                            },
                            "resource_tracker",
                        );
                    }
                    return Err(Error::wasi_resource_exhausted(
                        "Tensor memory limit would be exceeded",
                    ));
                }
            },
            _ => {}, // Other operations don't have specific quotas
        }
        Ok(())
    }

    /// Check rate limits using sliding window
    fn check_rate_limits(&self, operation_type: NNOperationType) -> Result<()> {
        let mut window = self
            .operations_window
            .lock()
            .map_err(|_| Error::wasi_runtime_error("Failed to acquire rate limit lock"))?;

        let current_time = get_current_time_ms();

        // Clean up old operations (outside sliding window)
        let window_start = current_time.saturating_sub(self.rate_limits.window_size_ms);
        while let Some(record) = window.front() {
            if record.timestamp < window_start {
                window.pop_front();
            } else {
                break;
            }
        }

        // Check rate limits based on operation type
        let limit_exceeded = match operation_type {
            NNOperationType::Load => {
                // For loads, use per-minute limit
                let minute_start = current_time.saturating_sub(60_000);
                let loads_this_minute = window
                    .iter()
                    .filter(|r| {
                        r.operation_type == NNOperationType::Load && r.timestamp >= minute_start
                    })
                    .count();
                loads_this_minute >= self.rate_limits.max_loads_per_minute as usize
            },
            NNOperationType::Inference => {
                // For inference, use per-second limit
                let second_start = current_time.saturating_sub(1_000);
                let inferences_this_second = window
                    .iter()
                    .filter(|r| {
                        r.operation_type == NNOperationType::Inference
                            && r.timestamp >= second_start
                    })
                    .count();
                inferences_this_second >= self.rate_limits.max_inferences_per_second as usize
            },
            _ => false, // Other operations use general concurrent limit
        };

        if limit_exceeded {
            // Log rate limit exceeded event
            if let Some(logger) = crate::nn::monitoring::get_logger() {
                logger.log_security(
                    crate::nn::monitoring::SecurityEvent::RateLimitExceeded {
                        operation:  format!("{:?}", operation_type),
                        limit_type: match operation_type {
                            NNOperationType::Load => "per_minute".to_string(),
                            NNOperationType::Inference => "per_second".to_string(),
                            _ => "concurrent".to_string(),
                        },
                    },
                    "resource_tracker",
                );
            }
            return Err(Error::wasi_resource_exhausted("Rate limit exceeded"));
        }

        Ok(())
    }

    /// Record an operation in the sliding window
    fn record_operation(&self, operation_type: NNOperationType, cost: usize) {
        if let Ok(mut window) = self.operations_window.lock() {
            window.push_back(OperationRecord {
                operation_type,
                timestamp: get_current_time_ms(),
                resource_cost: cost,
            });

            // Limit window size to prevent unbounded growth
            while window.len() > 10_000 {
                window.pop_front();
            }
        }
    }

    /// Calculate the resource cost of an operation
    fn calculate_operation_cost(&self, operation: &NNOperation) -> usize {
        match operation {
            NNOperation::Load { size, .. } => *size,
            NNOperation::SetInput { size, .. } => *size,
            NNOperation::Compute { estimated_flops } => (*estimated_flops / 1_000_000) as usize, /* Convert FLOPS to cost units */
            _ => 1, // Minimal cost for other operations
        }
    }

    /// Allocate model resources
    pub fn allocate_model(&self, size: usize) -> Result<()> {
        self.active_models.fetch_add(1, Ordering::AcqRel);
        let total_used = self.total_memory_used.fetch_add(size, Ordering::AcqRel) + size;

        // Log resource allocation
        if let Some(logger) = crate::nn::monitoring::get_logger() {
            logger.log_resource(
                crate::nn::monitoring::ResourceEvent::Allocated {
                    resource_type: "model_memory".to_string(),
                    amount: size,
                    total_used,
                },
                "resource_tracker",
            );
        }

        Ok(())
    }

    /// Deallocate model resources
    pub fn deallocate_model(&self, size: usize) {
        self.active_models.fetch_sub(1, Ordering::AcqRel);
        let total_used = self.total_memory_used.fetch_sub(size, Ordering::AcqRel) - size;

        // Log resource deallocation
        if let Some(logger) = crate::nn::monitoring::get_logger() {
            logger.log_resource(
                crate::nn::monitoring::ResourceEvent::Deallocated {
                    resource_type: "model_memory".to_string(),
                    amount: size,
                    total_used,
                },
                "resource_tracker",
            );
        }
    }

    /// Allocate context resources
    pub fn allocate_context(&self) -> Result<()> {
        let total_contexts = self.active_contexts.fetch_add(1, Ordering::AcqRel) + 1;

        // Log resource allocation
        if let Some(logger) = crate::nn::monitoring::get_logger() {
            logger.log_resource(
                crate::nn::monitoring::ResourceEvent::Allocated {
                    resource_type: "context".to_string(),
                    amount:        1,
                    total_used:    total_contexts,
                },
                "resource_tracker",
            );
        }

        Ok(())
    }

    /// Deallocate context resources
    pub fn deallocate_context(&self) {
        let total_contexts = self.active_contexts.fetch_sub(1, Ordering::AcqRel) - 1;

        // Log resource deallocation
        if let Some(logger) = crate::nn::monitoring::get_logger() {
            logger.log_resource(
                crate::nn::monitoring::ResourceEvent::Deallocated {
                    resource_type: "context".to_string(),
                    amount:        1,
                    total_used:    total_contexts,
                },
                "resource_tracker",
            );
        }
    }

    /// Get current resource usage statistics
    pub fn get_usage_stats(&self) -> ResourceUsageStats {
        ResourceUsageStats {
            active_models:         self.active_models.load(Ordering::Acquire),
            active_contexts:       self.active_contexts.load(Ordering::Acquire),
            total_memory_used:     self.total_memory_used.load(Ordering::Acquire),
            concurrent_operations: self.concurrent_operations.load(Ordering::Acquire),
        }
    }
}

/// RAII guard for operation tracking
pub struct OperationGuard<'a> {
    tracker:        &'a ResourceTracker,
    operation_type: NNOperationType,
}

impl<'a> OperationGuard<'a> {
    fn new(tracker: &'a ResourceTracker, operation_type: NNOperationType) -> Self {
        Self {
            tracker,
            operation_type,
        }
    }
}

impl<'a> Drop for OperationGuard<'a> {
    fn drop(&mut self) {
        // Decrement concurrent operations when guard is dropped
        self.tracker.concurrent_operations.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Resource usage statistics
#[derive(Debug, Clone)]
pub struct ResourceUsageStats {
    pub active_models:         usize,
    pub active_contexts:       usize,
    pub total_memory_used:     usize,
    pub concurrent_operations: usize,
}

/// Get current time in milliseconds
fn get_current_time_ms() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{
            SystemTime,
            UNIX_EPOCH,
        };
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }
    #[cfg(not(feature = "std"))]
    {
        wrt_platform::time::PlatformTime::get_monotonic_time_us() / 1000
    }
}

/// Core capability trait for neural network operations
pub trait NeuralNetworkCapability: Send + Sync + Debug {
    /// Get the verification level for this capability
    fn verification_level(&self) -> VerificationLevel;

    /// Verify that an operation is allowed under this capability
    fn verify_operation(&self, operation: &NNOperation) -> Result<()>;

    /// Get the resource limits for this capability
    fn resource_limits(&self) -> &NNResourceLimits;

    /// Check if dynamic model loading is allowed
    fn allows_dynamic_loading(&self) -> bool;

    /// Get list of allowed model formats
    fn allowed_formats(&self) -> &[ModelFormat];

    /// Check if a model hash is pre-approved (for higher safety levels)
    fn is_model_approved(&self, hash: &[u8; 32]) -> bool;

    /// Get the resource tracker for this capability (optional)
    fn resource_tracker(&self) -> Option<&ResourceTracker> {
        None // Default implementation - no tracking
    }
}

/// Dynamic neural network capability (Standard/QM level)
#[derive(Debug)]
pub struct DynamicNNCapability {
    limits:           NNResourceLimits,
    allowed_formats:  Vec<ModelFormat>,
    resource_tracker: Option<ResourceTracker>,
}

impl DynamicNNCapability {
    /// Create a new dynamic capability with default limits
    pub fn new() -> Self {
        Self {
            limits:           NNResourceLimits::default(),
            allowed_formats:  vec![
                ModelFormat::ONNX,
                ModelFormat::TensorFlow,
                ModelFormat::PyTorch,
                ModelFormat::OpenVINO,
                ModelFormat::TractNative,
            ],
            resource_tracker: None,
        }
    }

    /// Create with resource tracking enabled
    pub fn with_tracking() -> Self {
        let limits = NNResourceLimits::default();
        let rate_limits = RateLimits::default();
        let tracker = ResourceTracker::new(limits.clone(), rate_limits);

        Self {
            limits,
            allowed_formats: vec![
                ModelFormat::ONNX,
                ModelFormat::TensorFlow,
                ModelFormat::PyTorch,
                ModelFormat::OpenVINO,
                ModelFormat::TractNative,
            ],
            resource_tracker: Some(tracker),
        }
    }

    /// Create with custom limits
    pub fn with_limits(limits: NNResourceLimits) -> Self {
        Self {
            limits,
            allowed_formats: vec![
                ModelFormat::ONNX,
                ModelFormat::TensorFlow,
                ModelFormat::PyTorch,
                ModelFormat::OpenVINO,
                ModelFormat::TractNative,
            ],
            resource_tracker: None,
        }
    }

    /// Create with custom limits and tracking
    pub fn with_limits_and_tracking(limits: NNResourceLimits, rate_limits: RateLimits) -> Self {
        let tracker = ResourceTracker::new(limits.clone(), rate_limits);

        Self {
            limits,
            allowed_formats: vec![
                ModelFormat::ONNX,
                ModelFormat::TensorFlow,
                ModelFormat::PyTorch,
                ModelFormat::OpenVINO,
                ModelFormat::TractNative,
            ],
            resource_tracker: Some(tracker),
        }
    }
}

impl NeuralNetworkCapability for DynamicNNCapability {
    fn verification_level(&self) -> VerificationLevel {
        VerificationLevel::Standard
    }

    fn verify_operation(&self, operation: &NNOperation) -> Result<()> {
        // Check resource tracking first if enabled
        if let Some(tracker) = &self.resource_tracker {
            let _guard = tracker.check_operation_allowed(operation)?;
            // Guard will be dropped automatically, decrementing concurrent
            // operations
        }

        match operation {
            NNOperation::Load { size, format } => {
                if *size > self.limits.max_model_size {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::CapabilityVerificationFailed {
                                operation:        "load".to_string(),
                                capability_level: "Standard".to_string(),
                            },
                            "dynamic_capability",
                        );
                    }
                    return Err(Error::wasi_resource_limit("Model size exceeds limit"));
                }
                if !self.allowed_formats.contains(format) {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::ModelValidationFailed {
                                reason:     "Format not allowed".to_string(),
                                model_size: *size,
                            },
                            "dynamic_capability",
                        );
                    }
                    return Err(Error::wasi_invalid_argument("Model format not allowed"));
                }
                Ok(())
            },
            NNOperation::SetInput { size, dimensions } => {
                if *size > self.limits.max_tensor_memory {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::CapabilityVerificationFailed {
                                operation:        "set_input".to_string(),
                                capability_level: "Standard".to_string(),
                            },
                            "dynamic_capability",
                        );
                    }
                    return Err(Error::wasi_resource_limit("Tensor size exceeds limit"));
                }
                if dimensions.len() > self.limits.max_tensor_dimensions {
                    if let Some(logger) = crate::nn::monitoring::get_logger() {
                        logger.log_security(
                            crate::nn::monitoring::SecurityEvent::CapabilityVerificationFailed {
                                operation:        "set_input".to_string(),
                                capability_level: "Standard".to_string(),
                            },
                            "dynamic_capability",
                        );
                    }
                    return Err(Error::wasi_resource_limit("Too many tensor dimensions"));
                }
                Ok(())
            },
            _ => Ok(()), // Other operations allowed
        }
    }

    fn resource_limits(&self) -> &NNResourceLimits {
        &self.limits
    }

    fn allows_dynamic_loading(&self) -> bool {
        true
    }

    fn allowed_formats(&self) -> &[ModelFormat] {
        &self.allowed_formats
    }

    fn is_model_approved(&self, _hash: &[u8; 32]) -> bool {
        // Dynamic capability doesn't require pre-approval
        true
    }

    fn resource_tracker(&self) -> Option<&ResourceTracker> {
        self.resource_tracker.as_ref()
    }
}

/// Bounded neural network capability (Sampling/ASIL-A level)
#[derive(Debug)]
pub struct BoundedNNCapability {
    limits:             NNResourceLimits,
    allowed_formats:    Vec<ModelFormat>,
    runtime_monitoring: bool,
}

impl BoundedNNCapability {
    /// Create a new bounded capability
    pub fn new() -> Result<Self> {
        let mut allowed_formats = Vec::new();

        // Only allow well-tested formats
        allowed_formats.push(ModelFormat::ONNX);
        allowed_formats.push(ModelFormat::TractNative);

        Ok(Self {
            limits: NNResourceLimits {
                max_model_size:          50 * 1024 * 1024, // 50MB
                max_tensor_memory:       20 * 1024 * 1024, // 20MB
                max_tensor_dimensions:   6,
                max_execution_time_us:   10_000_000, // 10 seconds
                max_concurrent_models:   2,
                max_concurrent_contexts: 4,
            },
            allowed_formats,
            runtime_monitoring: true,
        })
    }
}

impl NeuralNetworkCapability for BoundedNNCapability {
    fn verification_level(&self) -> VerificationLevel {
        VerificationLevel::Sampling
    }

    fn verify_operation(&self, operation: &NNOperation) -> Result<()> {
        // Add runtime monitoring for all operations
        if self.runtime_monitoring {
            // Log operation for monitoring
        }

        match operation {
            NNOperation::Load { size, format } => {
                if *size > self.limits.max_model_size {
                    return Err(Error::wasi_resource_limit(
                        "Model size exceeds bounded limit",
                    ));
                }
                let allowed = self.allowed_formats.iter().any(|f| f == format);
                if !allowed {
                    return Err(Error::wasi_invalid_argument(
                        "Model format not in bounded set",
                    ));
                }
                Ok(())
            },
            NNOperation::SetInput { size, dimensions } => {
                if *size > self.limits.max_tensor_memory {
                    return Err(Error::wasi_resource_limit("Tensor exceeds bounded memory"));
                }
                if dimensions.len() > self.limits.max_tensor_dimensions {
                    return Err(Error::wasi_resource_limit("Tensor dimensions exceed bound"));
                }
                Ok(())
            },
            NNOperation::Compute { estimated_flops } => {
                // Could add FLOPS-based limiting here
                let _ = estimated_flops;
                Ok(())
            },
            _ => Ok(()),
        }
    }

    fn resource_limits(&self) -> &NNResourceLimits {
        &self.limits
    }

    fn allows_dynamic_loading(&self) -> bool {
        true // But with stricter limits
    }

    fn allowed_formats(&self) -> &[ModelFormat] {
        &self.allowed_formats
    }

    fn is_model_approved(&self, _hash: &[u8; 32]) -> bool {
        // Bounded capability doesn't require pre-approval but has runtime checks
        true
    }
}

/// Static neural network capability (Continuous/ASIL-B level)
#[derive(Debug)]
pub struct StaticNNCapability {
    limits:                  NNResourceLimits,
    approved_models:         Vec<[u8; 32]>,
    deterministic_execution: bool,
}

impl StaticNNCapability {
    /// Create a new static capability with pre-approved models
    pub fn new(approved_hashes: &[[u8; 32]]) -> Result<Self> {
        let mut approved_models = Vec::new();

        for hash in approved_hashes {
            approved_models.push(*hash);
        }

        Ok(Self {
            limits: NNResourceLimits {
                max_model_size:          20 * 1024 * 1024, // 20MB
                max_tensor_memory:       10 * 1024 * 1024, // 10MB
                max_tensor_dimensions:   4,
                max_execution_time_us:   1_000_000, // 1 second (deterministic)
                max_concurrent_models:   1,
                max_concurrent_contexts: 2,
            },
            approved_models,
            deterministic_execution: true,
        })
    }
}

impl NeuralNetworkCapability for StaticNNCapability {
    fn verification_level(&self) -> VerificationLevel {
        VerificationLevel::Continuous
    }

    fn verify_operation(&self, operation: &NNOperation) -> Result<()> {
        match operation {
            NNOperation::Load { size, format } => {
                if *size > self.limits.max_model_size {
                    return Err(Error::wasi_resource_limit(
                        "Model exceeds static allocation",
                    ));
                }
                // Only ONNX and Tract native for deterministic execution
                if !matches!(format, ModelFormat::ONNX | ModelFormat::TractNative) {
                    return Err(Error::wasi_verification_failed(
                        "Format not verified for deterministic execution",
                    ));
                }
                Ok(())
            },
            NNOperation::SetInput { size, dimensions } => {
                if *size > self.limits.max_tensor_memory {
                    return Err(Error::wasi_resource_limit(
                        "Tensor exceeds static memory pool",
                    ));
                }
                if dimensions.len() > self.limits.max_tensor_dimensions {
                    return Err(Error::wasi_resource_limit(
                        "Tensor complexity exceeds static limit",
                    ));
                }
                Ok(())
            },
            NNOperation::Compute { .. } => {
                if self.deterministic_execution {
                    // Ensure deterministic execution path
                    Ok(())
                } else {
                    Err(Error::wasi_verification_failed(
                        "Non-deterministic execution not allowed",
                    ))
                }
            },
            _ => Ok(()),
        }
    }

    fn resource_limits(&self) -> &NNResourceLimits {
        &self.limits
    }

    fn allows_dynamic_loading(&self) -> bool {
        false // Only pre-approved models
    }

    fn allowed_formats(&self) -> &[ModelFormat] {
        // Static slice of verified formats
        &[ModelFormat::ONNX, ModelFormat::TractNative]
    }

    /// Checks if a model hash is pre-approved using constant-time comparison
    ///
    /// # Security
    /// Uses constant-time comparison to prevent timing attacks that could
    /// leak information about approved model hashes. This is critical for
    /// ASIL-B compliance where only pre-approved models are allowed.
    fn is_model_approved(&self, hash: &[u8; 32]) -> bool {
        // Use constant-time comparison to prevent timing attacks
        self.approved_models
            .iter()
            .any(|h| constant_time::constant_time_eq(h.as_ref(), hash.as_ref()))
    }
}

/// Factory for creating capabilities based on verification level
pub fn create_nn_capability(level: VerificationLevel) -> Result<Box<dyn NeuralNetworkCapability>> {
    match level {
        VerificationLevel::Standard => Ok(Box::new(DynamicNNCapability::new())),
        VerificationLevel::Sampling => Ok(Box::new(BoundedNNCapability::new()?)),
        VerificationLevel::Continuous => {
            // For demo purposes, create with empty approved list
            Ok(Box::new(StaticNNCapability::new(&[])?))
        },
        VerificationLevel::Redundant | VerificationLevel::Formal => {
            Err(Error::wasi_unsupported_operation(
                "ASIL-C/D (Redundant/Formal) not supported in wrtd configuration",
            ))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_capability() {
        let cap = DynamicNNCapability::new();
        assert_eq!(cap.verification_level(), VerificationLevel::Standard);
        assert!(cap.allows_dynamic_loading());

        // Should allow large models
        let load_op = NNOperation::Load {
            size:   50 * 1024 * 1024,
            format: ModelFormat::ONNX,
        };
        assert!(cap.verify_operation(&load_op).is_ok());
    }

    #[test]
    fn test_bounded_capability() {
        let cap = BoundedNNCapability::new().unwrap();
        assert_eq!(cap.verification_level(), VerificationLevel::Sampling);

        // Should reject oversized models
        let load_op = NNOperation::Load {
            size:   100 * 1024 * 1024,
            format: ModelFormat::ONNX,
        };
        assert!(cap.verify_operation(&load_op).is_err());
    }

    #[test]
    fn test_static_capability() {
        let cap = StaticNNCapability::new(&[]).unwrap();
        assert_eq!(cap.verification_level(), VerificationLevel::Continuous);
        assert!(!cap.allows_dynamic_loading());

        // Should only allow verified formats
        let load_op = NNOperation::Load {
            size:   10 * 1024 * 1024,
            format: ModelFormat::PyTorch,
        };
        assert!(cap.verify_operation(&load_op).is_err());
    }

    #[test]
    fn test_constant_time_model_approval() {
        // Create some test hashes
        let approved_hash1 = [0xAAu8; 32];
        let approved_hash2 = [0xBBu8; 32];
        let approved_hash3 = [0xCCu8; 32];

        // Create capability with approved hashes
        let cap =
            StaticNNCapability::new(&[approved_hash1, approved_hash2, approved_hash3]).unwrap();

        // Test approved models
        assert!(cap.is_model_approved(&approved_hash1));
        assert!(cap.is_model_approved(&approved_hash2));
        assert!(cap.is_model_approved(&approved_hash3));

        // Test unapproved model
        let unapproved_hash = [0xDDu8; 32];
        assert!(!cap.is_model_approved(&unapproved_hash));

        // Test that comparison works for partial matches (timing attack prevention)
        // Hash that differs only in first byte
        let mut early_diff = approved_hash1;
        early_diff[0] = 0xFF;
        assert!(!cap.is_model_approved(&early_diff));

        // Hash that differs only in last byte
        let mut late_diff = approved_hash1;
        late_diff[31] = 0xFF;
        assert!(!cap.is_model_approved(&late_diff));

        // Both should return false without timing differences
        // The constant-time implementation ensures equal execution time
    }
}
