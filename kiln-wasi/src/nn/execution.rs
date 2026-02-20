//! Neural network execution context and inference
//!
//! This module handles the execution of neural network inference with
//! capability-based resource management and safety checks.

use core::fmt;
use std::sync::{
    Mutex,
    OnceLock,
};

use wrt_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    BoundedVec,
};

use super::{
    get_graph_store,
    wit_types::ErrorCode,
    ComputeCapable,
    Graph,
    NNOperation,
    NeuralNetworkCapability,
    Tensor,
    VerificationLevel,
};
use crate::prelude::*;

/// Maximum number of execution contexts
const MAX_CONTEXTS: usize = 32;

/// Maximum number of inputs per model
const MAX_INPUTS: usize = 16;

/// Maximum number of outputs per model
const MAX_OUTPUTS: usize = 16;

/// Errors specific to compute operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComputeError {
    /// Input not set
    MissingInput(usize),
    /// Invalid input index
    InvalidInputIndex(usize),
    /// Invalid output index
    InvalidOutputIndex(usize),
    /// Execution timeout
    Timeout,
    /// Backend-specific error
    BackendError(ErrorCode),
}

impl From<ComputeError> for Error {
    fn from(err: ComputeError) -> Self {
        match err {
            ComputeError::MissingInput(_idx) => Error::wasi_invalid_argument("Input not set"),
            ComputeError::InvalidInputIndex(_idx) => {
                Error::wasi_invalid_argument("Invalid input index")
            },
            ComputeError::InvalidOutputIndex(_idx) => {
                Error::wasi_invalid_argument("Invalid output index")
            },
            ComputeError::Timeout => Error::wasi_timeout("Inference execution timeout"),
            ComputeError::BackendError(_code) => Error::wasi_runtime_error("Backend error"),
        }
    }
}

/// Execution context for neural network inference
pub struct ExecutionContext {
    /// Unique identifier
    id:               u32,
    /// Associated graph ID
    graph_id:         u32,
    /// Input tensors
    inputs:           Vec<Option<Tensor>>,
    /// Output tensors (populated after compute)
    outputs:          Vec<Option<Tensor>>,
    /// Backend-specific context
    backend_context:  Box<dyn ComputeCapable>,
    /// Capability level
    capability_level: VerificationLevel,
    /// Execution statistics
    stats:            ExecutionStats,
}

/// Statistics for execution monitoring
#[derive(Debug, Default, Clone)]
pub struct ExecutionStats {
    /// Number of inferences run
    pub inference_count:   u64,
    /// Total execution time in microseconds
    pub total_time_us:     u64,
    /// Peak memory usage
    pub peak_memory_bytes: usize,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(
        id: u32,
        graph: &Graph,
        backend_context: Box<dyn ComputeCapable>,
        capability: &dyn NeuralNetworkCapability,
    ) -> Result<Self> {
        // Validate inputs
        if id == 0 {
            return Err(Error::wasi_invalid_argument("Context ID cannot be zero"));
        }

        // Pre-allocate space for inputs/outputs based on model metadata
        let model = graph.backend_model();
        let num_inputs = model.num_inputs();
        let num_outputs = model.num_outputs();

        // Validate model has reasonable input/output counts
        if num_inputs == 0 {
            return Err(Error::wasi_invalid_argument(
                "Model must have at least one input",
            ));
        }
        if num_outputs == 0 {
            return Err(Error::wasi_invalid_argument(
                "Model must have at least one output",
            ));
        }
        if num_inputs > MAX_INPUTS {
            return Err(Error::wasi_resource_exhausted("Model has too many inputs"));
        }
        if num_outputs > MAX_OUTPUTS {
            return Err(Error::wasi_resource_exhausted("Model has too many outputs"));
        }

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        for _ in 0..num_inputs {
            inputs.push(None);
        }
        for _ in 0..num_outputs {
            outputs.push(None);
        }

        let verification_level = capability.verification_level();
        Ok(Self {
            id,
            graph_id: graph.id(),
            inputs,
            outputs,
            backend_context,
            capability_level: verification_level,
            stats: ExecutionStats::default(),
        })
    }

    /// Get the context ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get the associated graph ID
    pub fn graph_id(&self) -> u32 {
        self.graph_id
    }

    /// Set an input tensor
    pub fn set_input(&mut self, index: usize, tensor: Tensor) -> Result<()> {
        if index >= self.inputs.len() {
            return Err(ComputeError::InvalidInputIndex(index).into());
        }

        // Validate tensor properties
        if !tensor.dimensions().is_valid() {
            return Err(Error::wasi_invalid_argument(
                "Tensor has invalid dimensions",
            ));
        }

        if tensor.size_bytes() == 0 {
            return Err(Error::wasi_invalid_argument("Tensor cannot be empty"));
        }

        // For higher safety levels, add stricter validation
        if self.capability_level >= VerificationLevel::Sampling {
            // Validate tensor size against reasonable limits
            const MAX_TENSOR_SIZE: usize = 100 * 1024 * 1024; // 100MB
            if tensor.size_bytes() > MAX_TENSOR_SIZE {
                return Err(Error::wasi_resource_exhausted(
                    "Tensor size exceeds safety limit",
                ));
            }

            // Validate tensor capability level matches context level
            if tensor.capability_level() < self.capability_level {
                return Err(Error::wasi_verification_failed(
                    "Tensor capability level insufficient for context",
                ));
            }
        }

        self.inputs[index] = Some(tensor);
        Ok(())
    }

    /// Get an input tensor
    pub fn get_input(&self, index: usize) -> Result<&Tensor> {
        if index >= self.inputs.len() {
            return Err(ComputeError::InvalidInputIndex(index).into());
        }

        self.inputs[index]
            .as_ref()
            .ok_or_else(|| ComputeError::MissingInput(index).into())
    }

    /// Check if all inputs are set
    pub fn inputs_ready(&self) -> bool {
        self.inputs.iter().all(|input| input.is_some())
    }

    /// Get an output tensor
    pub fn get_output(&self, index: usize) -> Result<&Tensor> {
        if index >= self.outputs.len() {
            return Err(ComputeError::InvalidOutputIndex(index).into());
        }

        self.outputs[index]
            .as_ref()
            .ok_or_else(|| Error::wasi_runtime_error("Output not computed yet"))
    }

    /// Set output tensor (used by backend after compute)
    pub fn set_output(&mut self, index: usize, tensor: Tensor) -> Result<()> {
        if index >= self.outputs.len() {
            return Err(ComputeError::InvalidOutputIndex(index).into());
        }

        self.outputs[index] = Some(tensor);
        Ok(())
    }

    /// Get the backend context
    pub fn backend_context(&self) -> &dyn fmt::Debug {
        self.backend_context.as_ref()
    }

    /// Get mutable backend context
    pub fn backend_context_mut(&mut self) -> &mut dyn fmt::Debug {
        self.backend_context.as_mut()
    }

    /// Update execution statistics
    pub fn update_stats(&mut self, execution_time_us: u64, memory_used: usize) {
        self.stats.inference_count += 1;
        self.stats.total_time_us += execution_time_us;
        self.stats.peak_memory_bytes = self.stats.peak_memory_bytes.max(memory_used);
    }

    /// Get execution statistics
    pub fn stats(&self) -> &ExecutionStats {
        &self.stats
    }

    /// Clear inputs for next inference
    pub fn clear_inputs(&mut self) {
        for input in self.inputs.iter_mut() {
            *input = None;
        }
    }

    /// Clear outputs
    pub fn clear_outputs(&mut self) {
        for output in self.outputs.iter_mut() {
            *output = None;
        }
    }
}

impl fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("id", &self.id)
            .field("graph_id", &self.graph_id)
            .field(
                "inputs_set",
                &self.inputs.iter().filter(|i| i.is_some()).count(),
            )
            .field(
                "outputs_computed",
                &self.outputs.iter().filter(|o| o.is_some()).count(),
            )
            .field("capability_level", &self.capability_level)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Execution context store
pub struct ContextStore {
    contexts: Vec<ExecutionContext>,
    next_id:  u32,
}

impl ContextStore {
    /// Create a new context store
    pub fn new() -> Result<Self> {
        Ok(Self {
            contexts: Vec::new(),
            next_id:  1,
        })
    }

    /// Add a context to the store
    pub fn add(&mut self, context: ExecutionContext) -> Result<u32> {
        let id = context.id();

        // Validate context ID is not already in use
        if self.contexts.iter().any(|c| c.id() == id) {
            return Err(Error::wasi_invalid_argument("Context ID already exists"));
        }

        if self.contexts.len() >= MAX_CONTEXTS {
            return Err(Error::wasi_resource_exhausted(
                "Maximum execution contexts reached",
            ));
        }

        self.contexts.push(context);
        Ok(id)
    }

    /// Get a context by ID
    pub fn get(&self, id: u32) -> Result<&ExecutionContext> {
        self.contexts
            .iter()
            .find(|c| c.id() == id)
            .ok_or_else(|| Error::wasi_invalid_argument("Execution context not found"))
    }

    /// Get a mutable context by ID
    pub fn get_mut(&mut self, id: u32) -> Result<&mut ExecutionContext> {
        self.contexts
            .iter_mut()
            .find(|c| c.id() == id)
            .ok_or_else(|| Error::wasi_invalid_argument("Execution context not found"))
    }

    /// Get the next available ID
    pub fn next_id(&mut self) -> Result<u32> {
        // Check if we're approaching wraparound danger zone
        if self.next_id > u32::MAX - 1000 {
            return Err(Error::wasi_resource_exhausted("Context ID space exhausted"));
        }

        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| Error::wasi_resource_exhausted("Context ID overflow"))?;
        Ok(id)
    }
}

/// Global context store
static CONTEXT_STORE: OnceLock<Mutex<ContextStore>> = OnceLock::new();

/// Initialize the context store
pub fn initialize_context_store() -> Result<()> {
    let store = ContextStore::new()?;
    let mutex = Mutex::new(store);

    CONTEXT_STORE
        .set(mutex)
        .map_err(|_| Error::wasi_capability_unavailable("Context store already initialized"))
}

/// Get the global context store
///
/// Returns a guard that can be used to access the store safely.
pub fn get_context_store() -> Result<std::sync::MutexGuard<'static, ContextStore>> {
    let mutex = CONTEXT_STORE
        .get()
        .ok_or_else(|| Error::wasi_capability_unavailable("Context store not initialized"))?;

    mutex
        .lock()
        .map_err(|_| Error::wasi_runtime_error("Context store mutex poisoned"))
}

/// Get the global context store for mutable operations
///
/// Returns a guard that can be used to mutate the store safely.
pub fn get_context_store_mut() -> Result<std::sync::MutexGuard<'static, ContextStore>> {
    get_context_store()
}

/// Execute inference with capability checks and resource monitoring
pub fn execute_inference(
    context: &mut ExecutionContext,
    capability: &dyn NeuralNetworkCapability,
) -> Result<()> {
    // Comprehensive input validation
    if !context.inputs_ready() {
        let missing_inputs: Vec<usize> = context
            .inputs
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| if opt.is_none() { Some(i) } else { None })
            .collect();
        return Err(Error::wasi_invalid_argument("Not all inputs are set"));
    }

    // Validate all inputs have consistent capability levels
    let context_level = context.capability_level;
    for (idx, input_opt) in context.inputs.iter().enumerate() {
        if let Some(input) = input_opt {
            if input.capability_level() < context_level {
                return Err(Error::wasi_verification_failed(
                    "Input capability level insufficient",
                ));
            }
        }
    }

    // Calculate estimated FLOPS for resource checking
    let estimated_flops = estimate_inference_flops(context)?;

    // Verify operation is allowed
    capability.verify_operation(&NNOperation::Compute { estimated_flops })?;

    // Validate total input memory against limits
    let total_input_memory = calculate_input_memory_usage(context);
    let limits = capability.resource_limits();
    if total_input_memory > limits.max_tensor_memory {
        return Err(Error::wasi_resource_exhausted("Input memory exceeds limit"));
    }

    // Record start time for timeout checking
    let start_time = get_time_us();

    // Execute inference through backend
    let graph_store = get_graph_store()?;
    let graph = graph_store.get(context.graph_id())?;

    // Prepare input tensors with validation
    let inputs: Vec<Tensor> =
        context.inputs.iter().filter_map(|opt| opt.as_ref().cloned()).collect();

    // Validate input count matches expectation
    let expected_inputs = graph.backend_model().num_inputs();
    if inputs.len() != expected_inputs {
        return Err(Error::wasi_invalid_argument("Input count mismatch"));
    }

    // Execute compute through the backend context
    let outputs = context.backend_context.compute(&inputs, graph.backend_model())?;

    // Validate output count
    let expected_outputs = graph.backend_model().num_outputs();
    if outputs.len() != expected_outputs {
        return Err(Error::wasi_runtime_error("Output count mismatch"));
    }

    // Validate output tensor properties
    for (idx, output) in outputs.iter().enumerate() {
        if !output.dimensions().is_valid() {
            return Err(Error::wasi_runtime_error("Output has invalid dimensions"));
        }
        if output.size_bytes() == 0 {
            return Err(Error::wasi_runtime_error("Output is empty"));
        }
    }

    // Store outputs in context
    context.outputs.clear();
    for output in outputs {
        context.outputs.push(Some(output));
    }

    // Check execution time against limits
    let execution_time = get_time_us() - start_time;
    if limits.max_execution_time_us > 0 && execution_time > limits.max_execution_time_us {
        return Err(ComputeError::Timeout.into());
    }

    // Update statistics
    let memory_used = calculate_memory_usage(context);
    context.update_stats(execution_time, memory_used);

    Ok(())
}

/// Estimate FLOPS for an inference
fn estimate_inference_flops(_context: &ExecutionContext) -> Result<u64> {
    // Simplified estimation
    // Real implementation would analyze the model structure
    Ok(1_000_000) // 1 MFLOP placeholder
}

/// Calculate memory usage for monitoring
fn calculate_memory_usage(context: &ExecutionContext) -> usize {
    let input_memory = calculate_input_memory_usage(context);
    let output_memory = calculate_output_memory_usage(context);
    input_memory + output_memory
}

/// Calculate input memory usage
fn calculate_input_memory_usage(context: &ExecutionContext) -> usize {
    context.inputs.iter().filter_map(|t| t.as_ref()).map(|t| t.size_bytes()).sum()
}

/// Calculate output memory usage
fn calculate_output_memory_usage(context: &ExecutionContext) -> usize {
    context.outputs.iter().filter_map(|t| t.as_ref()).map(|t| t.size_bytes()).sum()
}

/// Get current time in microseconds (platform-specific)
fn get_time_us() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{
            SystemTime,
            UNIX_EPOCH,
        };
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_micros() as u64
    }
    #[cfg(not(feature = "std"))]
    {
        // Use platform-specific monotonic time for no_std environments
        // This ensures consistent timing even if system clock changes
        wrt_platform::time::PlatformTime::get_monotonic_time_us()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_error_conversion() {
        let err: Error = ComputeError::MissingInput(0).into();
        assert!(matches!(err.category, ErrorCategory::Validation));

        let err: Error = ComputeError::Timeout.into();
        assert!(matches!(err.category, ErrorCategory::Core));
    }

    #[test]
    fn test_execution_stats() {
        let mut stats = ExecutionStats::default();
        assert_eq!(stats.inference_count, 0);

        stats.inference_count += 1;
        stats.total_time_us += 1000;
        stats.peak_memory_bytes = 1024;

        assert_eq!(stats.inference_count, 1);
        assert_eq!(stats.total_time_us, 1000);
        assert_eq!(stats.peak_memory_bytes, 1024);
    }
}
