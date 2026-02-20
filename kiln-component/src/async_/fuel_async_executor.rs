//! Fuel-based async executor for deterministic WebAssembly Component Model
//! execution
//!
//! This module provides an async executor that uses fuel consumption for timing
//! guarantees, enabling deterministic async execution across all ASIL levels.

extern crate alloc;

use core::fmt;

use crate::{
    ComponentInstanceId,
    canonical_abi::{CanonicalOptions, ComponentValue},
    execution_engine::{TimeBoundedConfig, TimeBoundedContext},
    prelude::*,
    resource_limits_loader::extract_resource_limits_from_binary,
    types::{ComponentInstance, ComponentInstanceState as InstanceState},
};
#[cfg(feature = "component-model-threading")]
use crate::{
    threading::task_manager::{TaskId, TaskState},
    threading::thread_spawn_fuel::{FuelTrackedThreadContext, ThreadFuelStatus},
};

// Stub types when threading is not enabled
#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u32);

#[cfg(not(feature = "component-model-threading"))]
impl TaskId {
    /// Create a new task identifier
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

#[cfg(not(feature = "component-model-threading"))]
impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task({})", self.0)
    }
}

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
}

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone)]
pub struct FuelTrackedThreadContext;

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadFuelStatus {
    Ok,
    Exhausted,
}

#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebtCreditBalance {
    pub task_id: TaskId,
    pub component_id: u64,
    pub current_debt: u64,
    pub available_credit: u64,
    pub net_balance: i64,
}

#[cfg(not(feature = "component-model-threading"))]
impl DebtCreditBalance {
    pub fn default_for_task(task_id: TaskId) -> Self {
        Self {
            task_id,
            component_id: 0,
            current_debt: 0,
            available_credit: 0,
            net_balance: 0,
        }
    }
}
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::sync::Weak;
#[cfg(not(any(feature = "std", feature = "alloc")))]
use core::mem::ManuallyDrop as Weak; // Placeholder for no_std
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Duration,
};
#[cfg(feature = "std")]
use std::sync::Weak;

use wrt_foundation::{
    Arc, CrateId, Mutex,
    collections::{StaticMap as BoundedMap, StaticVec as BoundedVec},
    operations::{Type as OperationType, global_fuel_consumed, record_global_operation},
    safe_managed_alloc,
    verification::VerificationLevel,
};
use wrt_platform::{
    advanced_sync::{Priority, PriorityInheritanceMutex},
    sync::{FutexLike, SpinFutex},
};

use crate::async_::{
    async_task_executor::{ASILExecutorFactory, AsyncTaskExecutor},
    fuel_aware_waker::{
        WakeCoalescer, create_fuel_aware_waker, create_fuel_aware_waker_with_asil,
        create_noop_waker,
    },
    fuel_debt_credit::{CreditRestriction, DebtPolicy, FuelDebtCreditSystem},
    fuel_dynamic_manager::{FuelAllocationPolicy, FuelDynamicManager},
    fuel_preemption_support::{FuelPreemptionManager, PreemptionDecision, PreemptionPolicy},
};

/// Maximum number of concurrent async tasks
const MAX_ASYNC_TASKS: usize = 128;

/// Yield threshold - yield after this much fuel consumed
const YIELD_FUEL_THRESHOLD: u64 = 1000;

/// Fuel budget for basic async operations
const ASYNC_TASK_SPAWN_FUEL: u64 = 20;
const ASYNC_TASK_YIELD_FUEL: u64 = 5;
const ASYNC_TASK_WAKE_FUEL: u64 = 10;
const ASYNC_TASK_POLL_FUEL: u64 = 15;

/// Placeholder for fuel monitoring statistics
#[derive(Debug, Default, Clone)]
pub struct FuelMonitoringStats {
    pub total_fuel_consumed: u64,
    pub current_rate: u64,
    pub peak_rate: u64,
    pub violations_count: u64,
}

/// Placeholder for debt credit configuration
#[derive(Debug, Default, Clone)]
pub struct DebtCreditConfig {
    pub max_debt: u64,
    pub credit_rate: u64,
    pub enabled: bool,
}

/// Placeholder for debt credit statistics
#[derive(Debug, Default, Clone)]
pub struct DebtCreditStats {
    pub current_debt: u64,
    pub total_credit_earned: u64,
    pub debt_violations: u64,
}

// FuelConsumptionRecord and FuelAlert are defined later in the file with
// complete implementations

/// Wrapper for boxed futures to implement Debug
pub struct DebugFuture(Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>);

impl core::fmt::Debug for DebugFuture {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Future")
            .field("type", &"dyn Future<Output = Result<()>>")
            .finish()
    }
}

impl DebugFuture {
    /// Create a new DebugFuture wrapper
    pub fn new(future: Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>) -> Self {
        Self(future)
    }

    /// Get a mutable reference to the inner future
    pub fn as_mut(&mut self) -> Pin<&mut (dyn Future<Output = Result<()>> + Send + 'static)> {
        self.0.as_mut()
    }
}

/// Async task representation with fuel tracking
#[derive(Debug)]
pub struct FuelAsyncTask {
    pub id: TaskId,
    pub component_id: ComponentInstanceId,
    pub fuel_budget: u64,
    pub fuel_consumed: AtomicU64,
    pub priority: Priority,
    pub verification_level: VerificationLevel,
    pub state: AsyncTaskState,
    pub waker: Option<Waker>,
    pub future: DebugFuture,
    pub execution_context: ExecutionContext,
    pub waiting_on_waitables: Option<Vec<WaitableHandle>>,
}

/// State of an async task
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncTaskState {
    /// Task is ready to be polled
    Ready,
    /// Task is waiting for an event
    Waiting,
    /// Task has completed successfully
    Completed,
    /// Task encountered an error
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task ran out of fuel
    FuelExhausted,
}

/// Result of executing one WebAssembly execution step
#[derive(Debug, Clone)]
pub enum ExecutionStepResult {
    /// Execution step completed successfully with result data
    Completed(Vec<u8>),
    /// Execution yielded and can be resumed later
    Yielded,
    /// Execution is waiting for an async operation
    Waiting,
}

/// Component Model async operations
#[derive(Debug, Clone)]
pub enum ComponentAsyncOperation {
    /// task.wait - wait for one of multiple waitables to become ready
    TaskWait {
        waitables: Vec<u32>, // Waitable indices
    },
    /// task.yield - voluntarily yield execution
    TaskYield,
    /// task.poll - check waitables without blocking
    TaskPoll {
        waitables: Vec<u32>, // Waitable indices to check
    },
}

/// Result of Component Model async operations
#[derive(Debug, Clone)]
pub enum ComponentAsyncOperationResult {
    /// Operation is waiting for completion
    Waiting {
        ready_index: Option<u32>, // Index of ready waitable, if any
    },
    /// Task yielded execution
    Yielded,
    /// Polling completed
    Polled {
        ready_index: Option<u32>, // Index of ready waitable, if any
    },
}

/// Execution state information for monitoring
#[derive(Debug, Clone)]
pub struct ExecutionStateInfo {
    pub task_id: TaskId,
    pub component_id: ComponentInstanceId,
    pub asil_mode: ASILExecutionMode, /* TODO: Remove this field and use execution_context.asil_config.mode instead */
    pub stack_depth: u32,
    pub max_stack_depth: u32,
    pub fuel_consumed: u64,
    pub has_yield_point: bool,
    pub has_component_instance: bool,
    pub error_state: Option<Error>,
}

/// Execution context for async task execution
#[derive(Debug)]
pub struct ExecutionContext {
    /// Component instance for WebAssembly execution
    pub component_instance: Option<Arc<ComponentInstance>>,
    /// Current execution stack depth
    pub stack_depth: u32,
    /// Maximum allowed stack depth (ASIL compliance)
    pub max_stack_depth: u32,
    /// Execution state storage for suspendable functions
    pub execution_state: Option<Box<dyn ExecutionState>>,
    /// Fuel consumption tracking within this context
    pub context_fuel_consumed: AtomicU64,
    /// Last yield point for resumable execution
    pub last_yield_point: Option<YieldPoint>,
    /// Error state for propagation
    pub error_state: Option<Error>,
    /// ASIL execution configuration including limits
    pub asil_config: ASILExecutionConfig,
    /// Current function index being executed
    pub current_function_index: u32,
    /// Function parameters for execution
    pub function_params: Vec<wrt_foundation::Value>,
}

/// Trait for execution state that can be suspended and resumed
pub trait ExecutionState: core::fmt::Debug + Send + Sync {
    /// Save the current execution state for later resumption
    fn save_state(&self) -> Result<Vec<u8>>;
    /// Restore execution state from saved data
    fn restore_state(&mut self, data: &[u8]) -> Result<()>;
    /// Get the current function index being executed
    fn current_function_index(&self) -> Option<u32>;
    /// Get local variables state
    fn get_locals(&self)
    -> &[wrt_foundation::component_value::ComponentValue<NoStdProvider<4096>>];
    /// Set local variables state
    fn set_locals(
        &mut self,
        locals: Vec<wrt_foundation::component_value::ComponentValue<NoStdProvider<4096>>>,
    ) -> Result<()>;
}

/// Trait providing access to executor services for execution contexts
pub trait ExecutorServices: Send + Sync {
    /// Check if a resource is available via waitable registry
    fn check_resource_availability(&self, resource_id: u64) -> Result<bool>;

    /// Create a waitable for async operations
    fn create_waitable(
        &mut self,
        component_id: ComponentInstanceId,
        resource_id: Option<u64>,
    ) -> Result<WaitableHandle>;

    /// Register task as waiting on waitables
    fn register_task_waitables(
        &mut self,
        task_id: TaskId,
        waitables: Vec<WaitableHandle>,
    ) -> Result<()>;

    /// Check if an external event has occurred
    fn check_external_event(&self, event_id: u64) -> Result<bool>;

    /// Get component instance for execution
    fn get_component_instance(
        &self,
        component_id: ComponentInstanceId,
    ) -> Option<Arc<ComponentInstance>>;
}

/// Yield point information for resumable execution
#[derive(Debug, Clone)]
pub struct YieldPoint {
    /// Instruction pointer or yield location
    pub instruction_pointer: u32,
    /// Operand stack at yield point
    pub stack: Vec<wrt_foundation::Value>,
    /// Local variables at yield point
    pub locals: Vec<wrt_foundation::Value>,
    /// Call stack at yield point
    pub call_stack: Vec<u32>,
    /// Fuel consumed up to this yield point
    pub fuel_at_yield: u64,
    /// Timestamp of yield (for deterministic replay)
    pub yield_timestamp: u64,
    /// Type of yield that occurred
    pub yield_type: YieldType,
    /// Yield context for restoration
    pub yield_context: YieldContext,
    /// Conditional resumption criteria
    pub resumption_condition: Option<ResumptionCondition>,
}

/// Type of yield that occurred during execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YieldType {
    /// Yield due to fuel exhaustion
    FuelExhausted,
    /// Yield due to time slice completion
    TimeSliceExpired,
    /// Yield due to async operation wait
    AsyncWait { resource_id: u64 },
    /// Explicit yield by WebAssembly code
    ExplicitYield,
    /// Yield due to stack depth limit
    StackDepthLimit,
    /// Yield due to ASIL compliance requirement
    ASILCompliance { reason: String },
    /// Yield for preemption by higher priority task
    Preemption { preempting_task_id: Option<u32> },
}

/// Context information for yield point restoration
#[derive(Debug, Clone)]
pub struct YieldContext {
    /// WebAssembly module state at yield
    pub module_state: Option<ModuleExecutionState>,
    /// Memory state snapshot (for ASIL-D)
    pub memory_snapshot: Option<Vec<u8>>,
    /// Global variables state
    pub globals: Vec<wrt_foundation::Value>,
    /// Table state if modified
    pub tables: Vec<TableState>,
    /// Linear memory bounds at yield
    pub memory_bounds: Option<(u32, u32)>,
    /// Active function import/export context
    pub function_context: FunctionExecutionContext,
}

/// Module execution state for complete restoration
#[derive(Debug, Clone)]
pub struct ModuleExecutionState {
    /// Current WebAssembly function being executed
    pub current_function: u32,
    /// Execution frame stack
    pub frame_stack: Vec<ExecutionFrame>,
    /// Control flow stack (blocks, loops, if/else)
    pub control_stack: Vec<ControlFrame>,
    /// Exception handling state
    pub exception_state: Option<ExceptionState>,
}

/// WebAssembly execution frame
#[derive(Debug, Clone)]
pub struct ExecutionFrame {
    /// Function index
    pub function_index: u32,
    /// Local variables for this frame
    pub locals: Vec<wrt_foundation::Value>,
    /// Return address (instruction pointer in caller)
    pub return_address: u32,
    /// Stack pointer in caller frame
    pub caller_stack_pointer: u32,
}

/// Control frame for WebAssembly control flow
#[derive(Debug, Clone)]
pub struct ControlFrame {
    /// Type of control structure
    pub control_type: ControlType,
    /// Block type signature
    pub block_type: BlockType,
    /// Start instruction pointer
    pub start_ip: u32,
    /// End instruction pointer
    pub end_ip: u32,
    /// Stack height at block entry
    pub stack_height: u32,
}

/// WebAssembly control flow types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlType {
    Block,
    Loop,
    If,
    Else,
    Try,
    Catch,
}

/// Block type for control frames
#[derive(Debug, Clone)]
pub enum BlockType {
    Empty,
    Value(wrt_foundation::types::ValueType),
    Function(u32), // Type index
}

/// Exception handling state
#[derive(Debug, Clone)]
pub struct ExceptionState {
    /// Exception tag
    pub tag: u32,
    /// Exception values
    pub values: Vec<wrt_foundation::Value>,
    /// Handler instruction pointer
    pub handler_ip: Option<u32>,
}

/// Table state for restoration
#[derive(Debug, Clone)]
pub struct TableState {
    /// Table index
    pub table_index: u32,
    /// Table elements
    pub elements: Vec<Option<wrt_foundation::Value>>,
    /// Table size
    pub size: u32,
}

/// Function execution context
#[derive(Debug, Clone)]
pub struct FunctionExecutionContext {
    /// Function signature
    pub signature: FunctionSignature,
    /// Import/export status
    pub function_kind: FunctionKind,
    /// Calling convention used
    pub calling_convention: CallingConvention,
}

/// Function signature for restoration
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    /// Parameter types
    pub params: Vec<wrt_foundation::types::ValueType>,
    /// Return types
    pub returns: Vec<wrt_foundation::types::ValueType>,
}

/// Function kind (import/export/local)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionKind {
    Local,
    Import { module: String, name: String },
    Export { name: String },
}

/// Calling convention used
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallingConvention {
    WebAssembly,
    ComponentModel,
    Host,
}

/// Condition for automatic resumption
#[derive(Debug, Clone)]
pub enum ResumptionCondition {
    /// Resume when resource becomes available
    ResourceAvailable { resource_id: u64 },
    /// Resume after specific fuel amount
    FuelRecovered { fuel_amount: u64 },
    /// Resume after time period (ASIL-B/C)
    TimeElapsed { duration_ms: u32 },
    /// Resume when external event occurs
    ExternalEvent { event_id: u64 },
    /// Resume manually (no automatic resumption)
    Manual,
}

/// ASIL execution mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ASILExecutionMode {
    /// Quality Management (no safety requirements)
    QM,
    /// ASIL-A: Basic safety requirements (unit variant)
    AsilA,
    /// ASIL-B: Bounded resource usage (unit variant)
    AsilB,
    /// ASIL-C: Freedom from interference (unit variant)
    AsilC,
    /// ASIL-D: Highest safety integrity (unit variant)
    AsilD,
    /// ASIL-A: Basic safety requirements
    A {
        /// Enable basic error detection
        error_detection: bool,
    },
    /// ASIL-B: Bounded resource usage
    B {
        /// Resource limits strictly enforced
        strict_resource_limits: bool,
        /// Maximum execution time per slice
        max_execution_slice_ms: u32,
    },
    /// ASIL-C: Freedom from interference
    C {
        /// Spatial isolation enforced
        spatial_isolation: bool,
        /// Temporal isolation enforced
        temporal_isolation: bool,
        /// Resource isolation enforced
        resource_isolation: bool,
    },
    /// ASIL-D: Highest safety integrity
    D {
        /// Deterministic execution required
        deterministic_execution: bool,
        /// Bounded execution time required
        bounded_execution_time: bool,
        /// Formal verification hooks enabled
        formal_verification: bool,
        /// Maximum fuel per execution slice
        max_fuel_per_slice: u64,
    },
}

impl Default for ASILExecutionMode {
    fn default() -> Self {
        ASILExecutionMode::A {
            error_detection: true,
        }
    }
}

/// Execution limits configuration extracted from WebAssembly binary metadata
/// and validated against ASIL requirements for engine qualification
#[derive(Debug, Clone)]
pub struct ExecutionLimitsConfig {
    /// Maximum fuel per execution step (required for timing guarantees)
    pub max_fuel_per_step: Option<u64>,
    /// Maximum memory usage in bytes (required for spatial isolation)
    pub max_memory_usage: Option<u64>,
    /// Maximum call stack depth (required for stack overflow prevention)
    pub max_call_depth: Option<u32>,
    /// Maximum stack depth (alias for max_call_depth for compatibility)
    pub max_stack_depth: Option<u32>,
    /// Maximum instructions per execution step (required for determinism)
    pub max_instructions_per_step: Option<u32>,
    /// Maximum execution time slice in milliseconds (required for temporal
    /// isolation)
    pub max_execution_slice_ms: Option<u32>,
    /// Maximum concurrent async operations
    pub max_async_operations: Option<u32>,
    /// Maximum waitables per task
    pub max_waitables_per_task: Option<u32>,
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: Option<u32>,
    /// Maximum yields per execution step
    pub max_yields_per_step: Option<u32>,
    /// Source of these limits (for qualification traceability)
    pub limit_source: LimitSource,
}

/// Source of execution limits for qualification and traceability
#[derive(Debug, Clone, PartialEq)]
pub enum LimitSource {
    /// Conservative limits for high-assurance execution
    Conservative,
    /// Limits extracted from WebAssembly binary custom sections
    BinaryMetadata {
        section_name: String,
        verified_hash: [u8; 32],
    },
    /// Limits derived from ASIL mode requirements
    ASILRequirements {
        asil_level: String,
        constraint_version: u32,
    },
    /// Platform-imposed limits (e.g., from WRTD configuration)
    PlatformConstraints {
        platform_id: String,
        capability_level: u8,
    },
    /// Default fallback limits (should not be used in ASIL-C/D)
    DefaultFallback,
}

/// Combined configuration for ASIL-compliant execution
#[derive(Debug, Clone)]
pub struct ASILExecutionConfig {
    pub mode: ASILExecutionMode,
    pub limits: ExecutionLimitsConfig,
    /// Whether this configuration has been qualified for the specific binary
    pub qualified_for_binary: Option<String>, // Binary hash
}

impl ASILExecutionConfig {
    /// Create ASIL execution config directly from ASIL requirements
    /// This is the baseline configuration when binary metadata is not available
    pub fn from_asil_requirements(asil_mode: ASILExecutionMode, constraint_version: u32) -> Self {
        let limits = ExecutionLimitsConfig::from_asil_requirements(asil_mode, constraint_version);

        Self {
            mode: asil_mode,
            limits,
            qualified_for_binary: None, // No binary qualification when created from ASIL requirements
        }
    }

    /// Create ASIL execution config with proper fallback chain
    /// Binary metadata → ASIL requirements → Platform constraints → Defaults
    pub fn from_binary_with_fallback(
        asil_mode: ASILExecutionMode,
        binary_hash: Option<[u8; 32]>,
        resource_limits_data: Option<&[u8]>,
        platform_constraints: Option<&str>,
    ) -> Result<Self> {
        let limits = if let (Some(hash), Some(data)) = (binary_hash, resource_limits_data) {
            // Priority 1: Use binary metadata if available
            ExecutionLimitsConfig::from_binary_metadata(hash, data)?
        } else {
            // Priority 2: Fall back to ASIL requirements
            ExecutionLimitsConfig::from_asil_requirements(asil_mode, 1)
        };

        Ok(Self {
            mode: asil_mode,
            limits,
            qualified_for_binary: binary_hash.map(|h| format!("{:?}", h)),
        })
    }

    /// Validate that this configuration is appropriate for the target ASIL
    /// level
    pub fn validate(&self) -> Result<()> {
        self.validate_for_asil()
    }

    /// Validate that this configuration is appropriate for the target ASIL
    /// level
    pub fn validate_for_asil(&self) -> Result<()> {
        match self.mode {
            ASILExecutionMode::D { .. } | ASILExecutionMode::AsilD => {
                // ASIL-D requires all limits to be specified
                if self.limits.max_fuel_per_step.is_none()
                    || self.limits.max_memory_usage.is_none()
                    || self.limits.max_call_depth.is_none()
                    || self.limits.max_instructions_per_step.is_none()
                    || self.limits.max_execution_slice_ms.is_none()
                {
                    return Err(Error::configuration_error(
                        "ASIL-D requires all execution limits to be specified",
                    ));
                }
            },
            ASILExecutionMode::C { .. } | ASILExecutionMode::AsilC => {
                // ASIL-C requires memory and call depth limits
                if self.limits.max_memory_usage.is_none() || self.limits.max_call_depth.is_none() {
                    return Err(Error::configuration_error(
                        "ASIL-C requires memory and call depth limits to be specified",
                    ));
                }
            },
            _ => {
                // ASIL-B and lower can use defaults
            },
        }
        Ok(())
    }
}

impl ExecutionLimitsConfig {
    /// Create limits config from WebAssembly binary metadata
    /// Parses the resource limits custom section for execution constraints
    pub fn from_binary_metadata(binary_hash: [u8; 32], custom_section_data: &[u8]) -> Result<Self> {
        #[cfg(feature = "decoder")]
        {
            use wrt_decoder::resource_limits_section::ResourceLimitsSection;
            use wrt_foundation::safe_memory::NoStdProvider;

            // Parse the resource limits custom section
            let resource_limits: ResourceLimitsSection<NoStdProvider<4096>> =
                ResourceLimitsSection::decode(custom_section_data)
                    .map_err(|_| Error::runtime_execution_error("Context access failed"))?;

            // Validate the resource limits
            resource_limits.validate().map_err(|e| {
                Error::new(
                    ErrorCategory::Parse,
                    codes::PARSE_ERROR,
                    "Invalid resource limits configuration",
                )
            })?;

            Ok(Self {
                max_fuel_per_step: resource_limits.max_fuel_per_step,
                max_memory_usage: resource_limits.max_memory_usage,
                max_call_depth: resource_limits.max_call_depth,
                max_stack_depth: resource_limits.max_call_depth,
                max_instructions_per_step: resource_limits.max_instructions_per_step,
                max_execution_slice_ms: resource_limits.max_execution_slice_ms,
                max_async_operations: None,
                max_waitables_per_task: None,
                max_concurrent_tasks: None,
                max_yields_per_step: None,
                limit_source: LimitSource::BinaryMetadata {
                    section_name: "wrt.resource_limits".to_owned(),
                    verified_hash: binary_hash,
                },
            })
        }

        #[cfg(not(feature = "decoder"))]
        {
            // Fallback when decoder is not available - use conservative defaults
            Ok(Self {
                max_fuel_per_step: Some(1000),
                max_memory_usage: Some(1024 * 1024), // 1MB
                max_call_depth: Some(256),
                max_stack_depth: Some(256),
                max_instructions_per_step: Some(1000),
                max_execution_slice_ms: Some(100),
                max_async_operations: None,
                max_waitables_per_task: None,
                max_concurrent_tasks: None,
                max_yields_per_step: None,
                limit_source: LimitSource::Conservative,
            })
        }
    }

    /// Create limits config from ASIL mode requirements
    pub fn from_asil_requirements(mode: ASILExecutionMode, constraint_version: u32) -> Self {
        let (
            max_fuel,
            max_memory,
            max_call_depth,
            max_instructions,
            max_slice_ms,
            max_async_ops,
            max_waitables,
            max_tasks,
            max_yields,
        ) = match mode {
            ASILExecutionMode::AsilD => (
                Some(100),
                Some(32 * 1024),
                Some(8),
                Some(1),
                Some(10),
                Some(2),
                Some(2),
                Some(4),
                Some(2),
            ),
            ASILExecutionMode::D {
                max_fuel_per_slice, ..
            } => (
                Some(max_fuel_per_slice),
                Some(32 * 1024),
                Some(8),
                Some(1),
                Some(10),
                Some(2),
                Some(2),
                Some(4),
                Some(2),
            ),
            ASILExecutionMode::AsilC => (
                Some(1000),
                Some(64 * 1024),
                Some(16),
                Some(10),
                Some(20),
                Some(8),
                Some(8),
                Some(16),
                Some(8),
            ),
            ASILExecutionMode::C { .. } => (
                Some(1000),
                Some(64 * 1024),
                Some(16),
                Some(10),
                Some(20),
                Some(8),
                Some(8),
                Some(16),
                Some(8),
            ),
            ASILExecutionMode::AsilB => (
                Some(5000),
                Some(128 * 1024),
                Some(32),
                Some(50),
                Some(100),
                Some(32),
                Some(32),
                Some(64),
                Some(32),
            ),
            ASILExecutionMode::B {
                max_execution_slice_ms,
                ..
            } => (
                Some(5000),
                Some(128 * 1024),
                Some(32),
                Some(50),
                Some(max_execution_slice_ms),
                Some(32),
                Some(32),
                Some(64),
                Some(32),
            ),
            ASILExecutionMode::QM | ASILExecutionMode::AsilA => (
                Some(10000),
                Some(256 * 1024),
                Some(64),
                Some(100),
                Some(100),
                Some(128),
                Some(128),
                Some(256),
                Some(128),
            ),
            ASILExecutionMode::A { .. } => (
                Some(10000),
                Some(256 * 1024),
                Some(64),
                Some(100),
                Some(100),
                Some(128),
                Some(128),
                Some(256),
                Some(128),
            ),
        };

        Self {
            max_fuel_per_step: max_fuel,
            max_memory_usage: max_memory,
            max_call_depth,
            max_stack_depth: max_call_depth,
            max_instructions_per_step: max_instructions,
            max_execution_slice_ms: max_slice_ms,
            max_async_operations: max_async_ops,
            max_waitables_per_task: max_waitables,
            max_concurrent_tasks: max_tasks,
            max_yields_per_step: max_yields,
            limit_source: LimitSource::ASILRequirements {
                asil_level: format!("{:?}", mode),
                constraint_version,
            },
        }
    }

    /// Create default configuration for a specific ASIL execution mode
    /// This provides conservative resource limits appropriate for each ASIL level
    pub fn default_for_asil(mode: ASILExecutionMode) -> Self {
        Self::from_asil_requirements(mode, 1)
    }

    /// Get fuel limit with fallback chain: binary → ASIL → platform → default
    pub fn get_fuel_limit(&self) -> u64 {
        self.max_fuel_per_step.unwrap_or(1000) // Conservative default
    }

    /// Get memory limit with fallback chain
    pub fn get_memory_limit(&self) -> u64 {
        self.max_memory_usage.unwrap_or(64 * 1024) // 64KB default
    }

    /// Get call depth limit with fallback chain
    pub fn get_call_depth_limit(&self) -> u32 {
        self.max_call_depth.unwrap_or(32) // Conservative default
    }

    /// Get instructions per step limit with fallback chain
    pub fn get_instructions_limit(&self) -> u32 {
        self.max_instructions_per_step.unwrap_or(50) // Conservative default
    }
}

impl ExecutionContext {
    /// Create a new execution context for the given ASIL configuration
    pub fn new(asil_config: ASILExecutionConfig) -> Self {
        // Get stack depth limit from configuration
        let max_stack_depth = asil_config.limits.get_call_depth_limit();

        Self {
            component_instance: None,
            stack_depth: 0,
            max_stack_depth,
            execution_state: None,
            context_fuel_consumed: AtomicU64::new(0),
            last_yield_point: None,
            error_state: None,
            asil_config,
            current_function_index: 0,
            function_params: Vec::new(),
        }
    }

    /// Create a new execution context with ASIL mode (creates default limits)
    pub fn new_with_mode(asil_mode: ASILExecutionMode) -> Self {
        let limits = ExecutionLimitsConfig::from_asil_requirements(asil_mode, 1);
        let asil_config = ASILExecutionConfig {
            mode: asil_mode,
            limits,
            qualified_for_binary: None,
        };
        Self::new(asil_config)
    }

    /// Create from configuration (alias for new for clarity)
    pub fn from_config(asil_config: ASILExecutionConfig) -> Result<Self> {
        Ok(Self::new(asil_config))
    }

    /// Set the component instance for this execution context
    pub fn set_component_instance(&mut self, instance: Arc<ComponentInstance>) {
        self.component_instance = Some(instance);
    }

    /// Check if execution can continue based on ASIL constraints
    pub fn can_continue_execution(&self) -> Result<bool> {
        // Check stack depth limits
        if self.stack_depth >= self.max_stack_depth {
            return Err(Error::runtime_execution_error("Stack depth limit exceeded"));
        }

        // Check ASIL-specific constraints
        match self.asil_config.mode {
            ASILExecutionMode::D {
                max_fuel_per_slice, ..
            } => {
                let fuel_consumed = self.context_fuel_consumed.load(Ordering::Acquire);
                if fuel_consumed >= max_fuel_per_slice {
                    return Ok(false); // Must yield
                }
            },
            ASILExecutionMode::B {
                max_execution_slice_ms,
                ..
            } => {
                // In real implementation, would check actual execution time
                // For now, use fuel as a proxy
                let fuel_consumed = self.context_fuel_consumed.load(Ordering::Acquire);
                if fuel_consumed >= (max_execution_slice_ms as u64 * 10) {
                    // 10 fuel per ms
                    return Ok(false); // Must yield
                }
            },
            _ => {}, // A and C modes have different constraints
        }

        Ok(true)
    }

    /// Record fuel consumption in this context
    pub fn consume_fuel(&self, amount: u64) {
        self.context_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
    }

    /// Create a yield point for suspending execution
    pub fn create_yield_point(
        &mut self,
        instruction_pointer: u32,
        stack_frame: Vec<ComponentValue>,
        locals: Vec<ComponentValue>,
    ) -> Result<()> {
        // Convert ComponentValue to wrt_foundation::Value for storage
        let stack = stack_frame
            .into_iter()
            .map(|cv| self.convert_component_value_to_value(cv))
            .collect::<core::result::Result<Vec<_>, _>>()?;
        let local_vars = locals
            .into_iter()
            .map(|cv| self.convert_component_value_to_value(cv))
            .collect::<core::result::Result<Vec<_>, _>>()?;

        let fuel_consumed = self.context_fuel_consumed.load(Ordering::Acquire);

        self.last_yield_point = Some(YieldPoint {
            instruction_pointer,
            stack,
            locals: local_vars,
            call_stack: vec![self.current_function_index], // Simple call stack
            fuel_at_yield: fuel_consumed,
            yield_timestamp: self.get_deterministic_timestamp(),
            yield_type: YieldType::ExplicitYield,
            yield_context: self.create_yield_context()?,
            resumption_condition: Some(ResumptionCondition::Manual),
        });

        Ok(())
    }

    /// Create advanced yield point with specific yield type and conditions
    pub fn create_advanced_yield_point(
        &mut self,
        instruction_pointer: u32,
        yield_type: YieldType,
        resumption_condition: Option<ResumptionCondition>,
    ) -> Result<()> {
        let fuel_consumed = self.context_fuel_consumed.load(Ordering::Acquire);

        // Capture current execution state
        let (stack, locals) = self.capture_execution_state()?;

        self.last_yield_point = Some(YieldPoint {
            instruction_pointer,
            stack,
            locals,
            call_stack: vec![self.current_function_index],
            fuel_at_yield: fuel_consumed,
            yield_timestamp: self.get_deterministic_timestamp(),
            yield_type,
            yield_context: self.create_yield_context()?,
            resumption_condition,
        });

        Ok(())
    }

    /// Create comprehensive yield context for restoration
    fn create_yield_context(&self) -> Result<YieldContext> {
        Ok(YieldContext {
            module_state: Some(ModuleExecutionState {
                current_function: self.current_function_index,
                frame_stack: vec![ExecutionFrame {
                    function_index: self.current_function_index,
                    locals: vec![],    // Will be populated from self.locals
                    return_address: 0, // Would come from call stack
                    caller_stack_pointer: 0,
                }],
                control_stack: vec![], // Would be populated with active control structures
                exception_state: None,
            }),
            memory_snapshot: None, // Only for ASIL-D deterministic execution
            globals: vec![],       // Would be populated from module globals
            tables: vec![],        // Would be populated from module tables
            memory_bounds: None,   // Would come from memory instance
            function_context: FunctionExecutionContext {
                signature: FunctionSignature {
                    params: vec![], // Would come from function type
                    returns: vec![],
                },
                function_kind: FunctionKind::Local, // Would be determined from module
                calling_convention: CallingConvention::WebAssembly,
            },
        })
    }

    /// Capture current execution state for yielding
    fn capture_execution_state(
        &self,
    ) -> Result<(Vec<wrt_foundation::Value>, Vec<wrt_foundation::Value>)> {
        // Capture real execution state from the engine if available
        if let Some(component_instance) = &self.component_instance {
            // In a production implementation, we would get this from the active engine
            // For now, we'll capture what we have available
            let stack = if let Some(yield_point) = &self.last_yield_point {
                yield_point.stack.clone()
            } else {
                vec![] // Empty stack at start
            };

            let locals = self.function_params.clone(); // Current locals
            Ok((stack, locals))
        } else {
            // No component - return current state
            Ok((vec![], self.function_params.clone()))
        }
    }

    /// Convert ComponentValue to wrt_foundation::Value
    fn convert_component_value_to_value(
        &self,
        cv: ComponentValue,
    ) -> Result<wrt_foundation::Value> {
        // Simple conversion - in real implementation would handle all ComponentValue
        // types
        match cv {
            ComponentValue::S32(val) => Ok(wrt_foundation::Value::I32(val)),
            ComponentValue::U32(val) => Ok(wrt_foundation::Value::I32(val as i32)),
            ComponentValue::S64(val) => Ok(wrt_foundation::Value::I64(val)),
            ComponentValue::U64(val) => Ok(wrt_foundation::Value::I64(val as i64)),
            ComponentValue::F32(val) => Ok(wrt_foundation::Value::F32(
                wrt_foundation::FloatBits32::from_float(val),
            )),
            ComponentValue::F64(val) => Ok(wrt_foundation::Value::F64(
                wrt_foundation::FloatBits64::from_float(val),
            )),
            _ => Ok(wrt_foundation::Value::I32(0)), // Placeholder for complex types
        }
    }

    /// Get deterministic timestamp for ASIL compliance
    fn get_deterministic_timestamp(&self) -> u64 {
        match self.asil_config.mode {
            ASILExecutionMode::D {
                deterministic_execution: true,
                ..
            } => {
                // For ASIL-D, use fuel consumption as deterministic timestamp
                self.context_fuel_consumed.load(Ordering::Acquire)
            },
            _ => {
                // For other modes, could use real time
                // For now, use fuel consumption as proxy
                self.context_fuel_consumed.load(Ordering::Acquire)
            },
        }
    }

    /// Reset context for new execution
    pub fn reset(&mut self) {
        self.stack_depth = 0;
        self.execution_state = None;
        self.context_fuel_consumed.store(0, Ordering::SeqCst);
        self.last_yield_point = None;
        self.error_state = None;
    }

    /// Execute a single instruction step with the engine

    /// Restore execution from advanced yield point
    pub fn restore_from_yield_point(&mut self, yield_point: &YieldPoint) -> Result<()> {
        // Restore basic execution state
        self.current_function_index = yield_point.instruction_pointer;
        self.function_params = yield_point.locals.clone();

        // Restore fuel consumption state
        self.context_fuel_consumed.store(yield_point.fuel_at_yield, Ordering::SeqCst);

        // Restore module state if available
        if let Some(module_state) = &yield_point.yield_context.module_state {
            self.restore_module_state(module_state)?;
        }

        // Handle ASIL-D memory restoration
        if let ASILExecutionMode::D {
            deterministic_execution: true,
            ..
        } = self.asil_config.mode
        {
            if let Some(memory_snapshot) = &yield_point.yield_context.memory_snapshot {
                self.restore_memory_snapshot(memory_snapshot)?;
            }
        }

        Ok(())
    }

    /// Check if yield point can be resumed based on conditions
    pub fn can_resume_yield_point(
        &self,
        yield_point: &YieldPoint,
        executor: &dyn ExecutorServices,
    ) -> Result<bool> {
        if let Some(condition) = &yield_point.resumption_condition {
            match condition {
                ResumptionCondition::ResourceAvailable { resource_id } => {
                    // Check if resource is now available via executor services
                    executor.check_resource_availability(*resource_id)
                },
                ResumptionCondition::FuelRecovered { fuel_amount } => {
                    // Check if we have recovered enough fuel
                    let current_fuel = self.context_fuel_consumed.load(Ordering::Acquire);
                    Ok(yield_point.fuel_at_yield.saturating_sub(current_fuel) >= *fuel_amount)
                },
                ResumptionCondition::TimeElapsed { duration_ms } => {
                    // Check if enough time has elapsed
                    let current_time = self.get_deterministic_timestamp();
                    let elapsed = current_time.saturating_sub(yield_point.yield_timestamp);
                    Ok(elapsed >= (*duration_ms as u64))
                },
                ResumptionCondition::ExternalEvent { event_id } => {
                    // Check if external event has occurred via executor services
                    executor.check_external_event(*event_id)
                },
                ResumptionCondition::Manual => {
                    // Manual resumption - always ready
                    Ok(true)
                },
            }
        } else {
            // No condition - can always resume
            Ok(true)
        }
    }

    /// Create ASIL-compliant yield point
    pub fn create_asil_yield_point(
        &mut self,
        instruction_pointer: u32,
        asil_reason: String,
    ) -> Result<()> {
        let yield_type = YieldType::ASILCompliance {
            reason: asil_reason,
        };

        // Determine resumption condition based on ASIL mode
        let resumption_condition = match self.asil_config.mode {
            ASILExecutionMode::D {
                max_fuel_per_slice, ..
            } => Some(ResumptionCondition::FuelRecovered {
                fuel_amount: max_fuel_per_slice / 4,
            }),
            ASILExecutionMode::B {
                max_execution_slice_ms,
                ..
            } => Some(ResumptionCondition::TimeElapsed {
                duration_ms: max_execution_slice_ms,
            }),
            _ => Some(ResumptionCondition::Manual),
        };

        self.create_advanced_yield_point(instruction_pointer, yield_type, resumption_condition)
    }

    /// Create conditional yield point for async operations
    pub fn create_async_yield_point(
        &mut self,
        instruction_pointer: u32,
        resource_id: u64,
    ) -> Result<()> {
        let yield_type = YieldType::AsyncWait { resource_id };
        let resumption_condition = Some(ResumptionCondition::ResourceAvailable { resource_id });

        self.create_advanced_yield_point(instruction_pointer, yield_type, resumption_condition)
    }

    /// Save yield point state for ASIL-D deterministic execution
    pub fn save_yield_point(&mut self, yield_info: YieldInfo) -> Result<()> {
        // Create memory snapshot for ASIL-D
        let memory_snapshot = if let ASILExecutionMode::D {
            deterministic_execution: true,
            ..
        } = self.asil_config.mode
        {
            Some(self.create_memory_snapshot()?)
        } else {
            None
        };

        let fuel_consumed = self.context_fuel_consumed.load(Ordering::Acquire);

        self.last_yield_point = Some(YieldPoint {
            instruction_pointer: yield_info.instruction_pointer,
            stack: yield_info.stack,
            locals: yield_info.locals,
            call_stack: yield_info.call_stack,
            fuel_at_yield: fuel_consumed,
            yield_timestamp: self.get_deterministic_timestamp(),
            yield_type: yield_info.yield_type,
            yield_context: YieldContext {
                module_state: yield_info.module_state,
                memory_snapshot,
                globals: yield_info.globals,
                tables: yield_info.tables,
                memory_bounds: yield_info.memory_bounds,
                function_context: yield_info.function_context,
            },
            resumption_condition: yield_info.resumption_condition,
        });

        Ok(())
    }

    /// Restore module execution state
    fn restore_module_state(&mut self, module_state: &ModuleExecutionState) -> Result<()> {
        self.current_function_index = module_state.current_function;

        // In real implementation, would restore frame stack, control stack, etc.
        // For now, just update the basic state
        if let Some(frame) = module_state.frame_stack.first() {
            self.function_params = frame.locals.clone();
        }

        Ok(())
    }

    /// Create memory snapshot for deterministic execution
    fn create_memory_snapshot(&self) -> Result<Vec<u8>> {
        // In real implementation, would capture actual memory state
        // For now, return empty snapshot
        Ok(vec![])
    }

    /// Restore memory snapshot for deterministic execution
    fn restore_memory_snapshot(&mut self, _snapshot: &[u8]) -> Result<()> {
        // In real implementation, would restore memory state
        // For now, just return success
        Ok(())
    }

    /// Validate memory isolation for ASIL-C
    pub fn validate_memory_isolation(&self) -> Result<()> {
        // In real implementation, would check memory boundaries
        // For now, always succeed
        Ok(())
    }
}

/// Yield information for creating yield points
#[derive(Debug)]
pub struct YieldInfo {
    pub instruction_pointer: u32,
    pub stack: Vec<wrt_foundation::Value>,
    pub locals: Vec<wrt_foundation::Value>,
    pub call_stack: Vec<u32>,
    pub yield_type: YieldType,
    pub module_state: Option<ModuleExecutionState>,
    pub globals: Vec<wrt_foundation::Value>,
    pub tables: Vec<TableState>,
    pub memory_bounds: Option<(u32, u32)>,
    pub function_context: FunctionExecutionContext,
    pub resumption_condition: Option<ResumptionCondition>,
}

/// Waitable handle for async operations
pub type WaitableHandle = u64;

/// Waitable state tracking
#[derive(Debug, Clone)]
pub struct WaitableState {
    /// Handle for this waitable
    pub handle: WaitableHandle,
    /// Component that owns this waitable
    pub component_id: ComponentInstanceId,
    /// Whether the waitable is ready
    pub is_ready: bool,
    /// Tasks waiting on this waitable
    pub waiting_tasks: Vec<TaskId>,
    /// Resource associated with waitable (if any)
    pub resource_id: Option<u64>,
}

/// Waitable registry for tracking async operations
pub struct WaitableRegistry {
    /// Next handle to allocate
    next_handle: AtomicU64,
    /// Registered waitables
    waitables: BoundedMap<WaitableHandle, WaitableState, MAX_ASYNC_TASKS>,
    /// Ready waitables queue
    ready_waitables: BoundedVec<WaitableHandle, MAX_ASYNC_TASKS>,
}

impl WaitableRegistry {
    /// Create a new waitable registry
    pub fn new() -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        Ok(Self {
            next_handle: AtomicU64::new(1),
            waitables: BoundedMap::new(),
            ready_waitables: BoundedVec::new().unwrap(),
        })
    }

    /// Register a new waitable
    pub fn register_waitable(
        &mut self,
        component_id: ComponentInstanceId,
        resource_id: Option<u64>,
    ) -> Result<WaitableHandle> {
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);
        let state = WaitableState {
            handle,
            component_id,
            is_ready: false,
            waiting_tasks: Vec::new(),
            resource_id,
        };
        self.waitables.insert(handle, state)?;
        Ok(handle)
    }

    /// Mark a waitable as ready
    pub fn notify_waitable(&mut self, handle: WaitableHandle) -> Result<Vec<TaskId>> {
        if let Some(waitable) = self.waitables.get_mut(&handle) {
            waitable.is_ready = true;
            self.ready_waitables.push(handle)?;
            Ok(waitable.waiting_tasks.clone())
        } else {
            Ok(Vec::new())
        }
    }

    /// Add a task to wait on a waitable
    pub fn add_waiting_task(&mut self, handle: WaitableHandle, task_id: TaskId) -> Result<()> {
        if let Some(waitable) = self.waitables.get_mut(&handle) {
            waitable.waiting_tasks.push(task_id);
        }
        Ok(())
    }

    /// Check if any waitables are ready
    pub fn poll_ready_waitables(
        &mut self,
        waitables: &[WaitableHandle],
    ) -> Option<(WaitableHandle, usize)> {
        for (index, &handle) in waitables.iter().enumerate() {
            if let Some(waitable) = self.waitables.get(&handle) {
                if waitable.is_ready {
                    return Some((handle, index));
                }
            }
        }
        None
    }

    /// Clean up a waitable
    pub fn remove_waitable(&mut self, handle: WaitableHandle) -> Result<()> {
        self.waitables.remove(&handle);
        Ok(())
    }
}

/// Fuel-based async executor for Component Model
pub struct FuelAsyncExecutor {
    /// Task storage with bounded capacity
    tasks: BoundedMap<TaskId, FuelAsyncTask, MAX_ASYNC_TASKS>,
    /// Ready queue for tasks that can be polled
    ready_queue: Arc<Mutex<BoundedVec<TaskId, MAX_ASYNC_TASKS>>>,
    /// Component instance registry for real module lookup
    component_registry: BoundedMap<ComponentInstanceId, Arc<ComponentInstance>, MAX_ASYNC_TASKS>,
    /// Waitable registry for async operations
    waitable_registry: WaitableRegistry,
    /// Global fuel limit for all async operations
    global_fuel_limit: AtomicU64,
    /// Global fuel consumed by all async operations
    global_fuel_consumed: AtomicU64,
    /// Default verification level for new tasks
    default_verification_level: VerificationLevel,
    /// Whether fuel enforcement is enabled
    fuel_enforcement: AtomicBool,
    /// Next task ID counter
    next_task_id: AtomicUsize,
    /// Executor state
    executor_state: ExecutorState,
    /// Wake coalescer to prevent thundering herd
    wake_coalescer: Option<crate::async_::fuel_aware_waker::WakeCoalescer>,
    /// Weak reference to self for waker creation (boxed to break recursive cycle)
    self_ref: Box<Option<Weak<Mutex<Self>>>>,
    /// Polling statistics
    polling_stats: PollingStatistics,
    /// Dynamic fuel manager
    fuel_manager: Option<FuelDynamicManager>,
    /// Preemption manager
    preemption_manager: Option<FuelPreemptionManager>,
    /// Active fuel monitor
    fuel_monitor: Option<FuelMonitor>,
    /// ASIL fuel enforcement policy
    fuel_enforcement_policy: Option<ASILFuelEnforcementPolicy>,
    /// Fuel debt and credit system
    debt_credit_system: Option<FuelDebtCreditSystem>,
    /// ASIL-specific task executors
    asil_executors: BoundedMap<u8, Box<dyn AsyncTaskExecutor>, 4>,
}

impl core::fmt::Debug for FuelAsyncExecutor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FuelAsyncExecutor")
            .field("tasks", &self.tasks.len())
            .field("ready_queue", &"<Arc<Mutex<...>>>")
            .field(
                "global_fuel_limit",
                &self.global_fuel_limit.load(Ordering::Relaxed),
            )
            .field(
                "global_fuel_consumed",
                &self.global_fuel_consumed.load(Ordering::Relaxed),
            )
            .field("executor_state", &self.executor_state)
            .field("next_task_id", &self.next_task_id.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// State of the executor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorState {
    /// Executor is running
    Running,
    /// Executor is paused
    Paused,
    /// Executor is shutting down
    ShuttingDown,
    /// Executor has stopped
    Stopped,
}

impl FuelMonitor {
    /// Create a new fuel monitor
    pub fn new() -> Result<Self> {
        Err(Error::async_task_spawn_failed(
            "FuelMonitor not yet implemented",
        ))
    }

    /// Get monitoring statistics
    pub fn get_statistics(&self) -> FuelMonitoringStats {
        FuelMonitoringStats {
            total_fuel_consumed: self.window_fuel_consumed.load(Ordering::Relaxed),
            current_rate: self.current_rate.load(Ordering::Relaxed),
            peak_rate: self.peak_rate.load(Ordering::Relaxed),
            violations_count: 0, // Placeholder
        }
    }

    /// Clear fuel alerts
    pub fn clear_alerts(&self) {
        let mut alerts = self.active_alerts.lock();
        alerts.clear();
    }

    /// Update fuel consumption tracking for a task
    ///
    /// This method tracks fuel consumption per task and ASIL mode, updating
    /// internal statistics and generating alerts when thresholds are exceeded.
    ///
    /// # Parameters
    /// - `amount`: Fuel consumed by the task
    /// - `task_id`: Identifier of the consuming task
    /// - `asil_mode`: ASIL execution mode of the task
    ///
    /// # Returns
    /// - `Ok(())` on successful tracking
    /// - `Err(_)` if ASIL fuel limits are violated
    pub fn update_consumption(
        &self,
        amount: u64,
        task_id: TaskId,
        asil_mode: ASILExecutionMode,
    ) -> Result<()> {
        // Update total window fuel consumption
        let prev_consumed = self.window_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
        let total_consumed = prev_consumed + amount;

        // Check ASIL-specific thresholds and generate alerts if needed
        match asil_mode {
            ASILExecutionMode::D { .. } | ASILExecutionMode::AsilD => {
                if amount > self.asil_thresholds.asil_d_task_limit {
                    let mut alerts = self.active_alerts.lock();
                    if alerts.len() < 32 {
                        alerts
                            .push(FuelAlert::ASILViolation {
                                mode: asil_mode,
                                violation_type: format!(
                                    "ASIL-D task {} exceeded fuel limit: {} > {}",
                                    task_id.0, amount, self.asil_thresholds.asil_d_task_limit
                                ),
                            })
                            .ok();
                    }
                    return Err(Error::async_fuel_exhausted(
                        "ASIL-D task fuel limit exceeded",
                    ));
                }
            },
            ASILExecutionMode::C { .. } | ASILExecutionMode::AsilC => {
                if total_consumed > self.asil_thresholds.asil_c_component_limit {
                    let mut alerts = self.active_alerts.lock();
                    if alerts.len() < 32 {
                        alerts
                            .push(FuelAlert::ASILViolation {
                                mode: asil_mode,
                                violation_type: format!(
                                    "ASIL-C component exceeded fuel budget: {} > {}",
                                    total_consumed, self.asil_thresholds.asil_c_component_limit
                                ),
                            })
                            .ok();
                    }
                }
            },
            ASILExecutionMode::B { .. } | ASILExecutionMode::AsilB => {
                if amount > self.asil_thresholds.asil_b_slice_limit {
                    let mut alerts = self.active_alerts.lock();
                    if alerts.len() < 32 {
                        alerts
                            .push(FuelAlert::ASILViolation {
                                mode: asil_mode,
                                violation_type: format!(
                                    "ASIL-B task {} exceeded slice fuel limit: {} > {}",
                                    task_id.0, amount, self.asil_thresholds.asil_b_slice_limit
                                ),
                            })
                            .ok();
                    }
                }
            },
            ASILExecutionMode::A { .. } | ASILExecutionMode::AsilA => {
                if total_consumed > self.asil_thresholds.asil_a_warning_threshold {
                    let mut alerts = self.active_alerts.lock();
                    if alerts.len() < 32 {
                        alerts
                            .push(FuelAlert::TaskApproachingLimit {
                                task_id,
                                remaining_fuel: self
                                    .asil_thresholds
                                    .asil_a_warning_threshold
                                    .saturating_sub(total_consumed),
                            })
                            .ok();
                    }
                }
            },
            ASILExecutionMode::QM => {
                // No strict limits for QM mode, just track consumption
            },
        }

        // Record consumption in history
        let mut history = self.consumption_history.lock();
        if history.len() < 128 {
            history
                .push(FuelConsumptionRecord {
                    timestamp: total_consumed, // Using fuel as timestamp for determinism
                    fuel_consumed: amount,
                    active_tasks: 1, // Single task consumption
                    highest_asil_mode: asil_mode,
                })
                .ok();
        }

        Ok(())
    }
}

impl FuelAsyncExecutor {
    /// Create a new fuel-based async executor
    pub fn new() -> Result<Self> {
        let provider = safe_managed_alloc!(8192, CrateId::Component)?;
        let ready_queue = Arc::new(Mutex::new(BoundedVec::new().unwrap()));
        let wake_coalescer = crate::async_::fuel_aware_waker::WakeCoalescer::new().ok();

        // Initialize ASIL executors
        let mut asil_executors = BoundedMap::new();

        // Create executor for each ASIL level
        let asil_d = ASILExecutionMode::D {
            deterministic_execution: true,
            bounded_execution_time: true,
            formal_verification: true,
            max_fuel_per_slice: 1000,
        };
        asil_executors.insert(0, ASILExecutorFactory::create_executor(asil_d)).ok();

        let asil_c = ASILExecutionMode::C {
            spatial_isolation: true,
            temporal_isolation: true,
            resource_isolation: true,
        };
        asil_executors.insert(1, ASILExecutorFactory::create_executor(asil_c)).ok();

        let asil_b = ASILExecutionMode::B {
            strict_resource_limits: true,
            max_execution_slice_ms: 10,
        };
        asil_executors.insert(2, ASILExecutorFactory::create_executor(asil_b)).ok();

        let asil_a = ASILExecutionMode::A {
            error_detection: true,
        };
        asil_executors.insert(3, ASILExecutorFactory::create_executor(asil_a)).ok();

        Ok(Self {
            tasks: BoundedMap::new(),
            ready_queue,
            component_registry: BoundedMap::new(),
            waitable_registry: WaitableRegistry::new()?,
            global_fuel_limit: AtomicU64::new(u64::MAX),
            global_fuel_consumed: AtomicU64::new(0),
            default_verification_level: VerificationLevel::Standard,
            fuel_enforcement: AtomicBool::new(true),
            next_task_id: AtomicUsize::new(1),
            executor_state: ExecutorState::Running,
            wake_coalescer,
            self_ref: Box::new(None),
            polling_stats: PollingStatistics::default(),
            fuel_manager: None,
            preemption_manager: None,
            fuel_monitor: None,
            fuel_enforcement_policy: None,
            debt_credit_system: None,
            asil_executors,
        })
    }

    /// Set the global fuel limit for all async operations
    pub fn set_global_fuel_limit(&self, limit: u64) {
        self.global_fuel_limit.store(limit, Ordering::SeqCst);
    }

    /// Set the default verification level for new tasks
    pub fn set_default_verification_level(&mut self, level: VerificationLevel) {
        self.default_verification_level = level;
    }

    /// Set the self reference for waker creation
    pub fn set_self_ref(&mut self, weak_ref: Weak<Mutex<Self>>) {
        *self.self_ref = Some(weak_ref);
    }

    /// Register a component instance for execution
    pub fn register_component(
        &mut self,
        component_id: ComponentInstanceId,
        component: Arc<ComponentInstance>,
    ) -> Result<()> {
        if self.component_registry.contains_key(&component_id) {
            return Err(Error::new(
                ErrorCategory::Component,
                codes::COMPONENT_ALREADY_EXISTS,
                "Operation failed",
            ));
        }

        self.component_registry.insert(component_id, component)?;
        Ok(())
    }

    /// Unregister a component instance
    pub fn unregister_component(&mut self, component_id: ComponentInstanceId) -> Result<()> {
        if self.component_registry.remove(&component_id).is_none() {
            return Err(Error::runtime_execution_error("Component not found"));
        }
        Ok(())
    }

    /// Create a new waitable for async operations
    pub fn create_waitable(
        &mut self,
        component_id: ComponentInstanceId,
        resource_id: Option<u64>,
    ) -> Result<WaitableHandle> {
        self.waitable_registry.register_waitable(component_id, resource_id)
    }

    /// Notify that a waitable is ready
    pub fn notify_waitable(&mut self, handle: WaitableHandle) -> Result<()> {
        // Get tasks waiting on this waitable
        let waiting_tasks = self.waitable_registry.notify_waitable(handle)?;

        // Wake up all waiting tasks
        for task_id in waiting_tasks {
            if let Some(task) = self.tasks.get_mut(&task_id) {
                if task.state == AsyncTaskState::Waiting {
                    task.state = AsyncTaskState::Ready;
                    self.ready_queue.lock().push(task_id)?;
                }
            }
        }

        Ok(())
    }

    /// Check if a resource is available by checking its waitable
    pub fn check_resource_waitable(&mut self, resource_id: u64) -> bool {
        // Find waitables associated with this resource
        for (handle, state) in self.waitable_registry.waitables.iter() {
            if state.resource_id == Some(resource_id) && state.is_ready {
                return true;
            }
        }
        false
    }

    /// Enable or disable fuel enforcement
    pub fn set_fuel_enforcement(&self, enabled: bool) {
        self.fuel_enforcement.store(enabled, Ordering::SeqCst);
    }

    /// Enable dynamic fuel management
    pub fn enable_dynamic_fuel_management(&mut self, policy: FuelAllocationPolicy) -> Result<()> {
        let mut manager = FuelDynamicManager::new(policy, 1_000_000)?;
        // Register default component
        manager.register_component(
            ComponentInstanceId::new(0),
            100_000,
            128, /* Normal priority */
        )?;
        self.fuel_manager = Some(manager);
        Ok(())
    }

    /// Enable preemption support
    pub fn enable_preemption(&mut self, policy: PreemptionPolicy) -> Result<()> {
        self.preemption_manager = Some(FuelPreemptionManager::new(policy)?);
        Ok(())
    }

    /// Enable active fuel monitoring
    pub fn enable_fuel_monitoring(&mut self) -> Result<()> {
        self.fuel_monitor = Some(FuelMonitor::new()?);
        Ok(())
    }

    /// Get fuel monitoring statistics
    pub fn get_fuel_monitoring_stats(&self) -> Option<FuelMonitoringStats> {
        self.fuel_monitor.as_ref().map(|monitor| monitor.get_statistics())
    }

    /// Get active fuel alerts
    pub fn get_fuel_alerts(&self) -> Vec<FuelAlert> {
        if let Some(monitor) = &self.fuel_monitor {
            let alerts = monitor.active_alerts.lock();
            return alerts.iter().cloned().collect();
        }
        Vec::new()
    }

    /// Clear fuel alerts
    pub fn clear_fuel_alerts(&self) {
        if let Some(monitor) = &self.fuel_monitor {
            monitor.clear_alerts();
        }
    }

    /// Set ASIL fuel enforcement policy
    pub fn set_fuel_enforcement_policy(&mut self, policy: ASILFuelEnforcementPolicy) {
        self.fuel_enforcement_policy = Some(policy);
    }

    /// Enforce ASIL-specific fuel policy for a task
    fn enforce_asil_fuel_policy(
        &self,
        task: &FuelAsyncTask,
        fuel_to_consume: u64,
    ) -> Result<FuelEnforcementDecision> {
        let policy = match &self.fuel_enforcement_policy {
            Some(p) => p,
            None => return Ok(FuelEnforcementDecision::Allow), // No policy, allow
        };

        let fuel_consumed = task.fuel_consumed.load(Ordering::Acquire);
        let remaining_fuel = task.fuel_budget.saturating_sub(fuel_consumed);

        // Check ASIL-specific enforcement
        match task.execution_context.asil_config.mode {
            ASILExecutionMode::AsilD | ASILExecutionMode::D { .. } => self.enforce_asil_d_policy(
                task,
                fuel_to_consume,
                remaining_fuel,
                &policy.asil_policies.asil_d,
            ),
            ASILExecutionMode::AsilC | ASILExecutionMode::C { .. } => self.enforce_asil_c_policy(
                task,
                fuel_to_consume,
                remaining_fuel,
                &policy.asil_policies.asil_c,
            ),
            ASILExecutionMode::AsilB | ASILExecutionMode::B { .. } => self.enforce_asil_b_policy(
                task,
                fuel_to_consume,
                remaining_fuel,
                &policy.asil_policies.asil_b,
            ),
            ASILExecutionMode::QM | ASILExecutionMode::AsilA | ASILExecutionMode::A { .. } => self
                .enforce_asil_a_policy(
                    task,
                    fuel_to_consume,
                    remaining_fuel,
                    &policy.asil_policies.asil_a,
                ),
        }
    }

    /// Enforce ASIL-D deterministic fuel policy
    fn enforce_asil_d_policy(
        &self,
        task: &FuelAsyncTask,
        fuel_to_consume: u64,
        remaining_fuel: u64,
        policy: &ASILDPolicy,
    ) -> Result<FuelEnforcementDecision> {
        // Check fuel quantum alignment
        if policy.enforce_deterministic_ordering && fuel_to_consume % policy.fuel_quantum != 0 {
            return Err(Error::new(
                ErrorCategory::Validation,
                codes::INVALID_INPUT,
                "Operation failed",
            ));
        }

        // Check max step fuel
        if fuel_to_consume > policy.max_step_fuel {
            return Err(Error::runtime_execution_error("Fuel step limit exceeded"));
        }

        // Check preallocation requirement
        if policy.require_preallocation && remaining_fuel < fuel_to_consume {
            return Ok(FuelEnforcementDecision::Deny {
                reason: "Insufficient preallocation".to_string(),
            });
        }

        Ok(FuelEnforcementDecision::Allow)
    }

    /// Enforce ASIL-C component isolation fuel policy
    fn enforce_asil_c_policy(
        &self,
        task: &FuelAsyncTask,
        fuel_to_consume: u64,
        remaining_fuel: u64,
        policy: &ASILCPolicy,
    ) -> Result<FuelEnforcementDecision> {
        // Check component isolation
        if policy.component_isolation {
            // In real implementation, would check component-specific fuel pool
            let component_fuel_available = remaining_fuel; // Placeholder

            if component_fuel_available < fuel_to_consume {
                // Check if fuel transfer is allowed
                if policy.max_transfer_amount > 0 {
                    let transfer_needed = fuel_to_consume - component_fuel_available;
                    if transfer_needed <= policy.max_transfer_amount {
                        return Ok(FuelEnforcementDecision::AllowWithTransfer {
                            transfer_amount: transfer_needed,
                            source_component: None, // Would specify source
                        });
                    }
                }

                return Ok(FuelEnforcementDecision::Deny {
                    reason: String::from("ASIL-C: Component fuel isolation violation"),
                });
            }
        }

        Ok(FuelEnforcementDecision::Allow)
    }

    /// Enforce ASIL-B bounded fuel policy
    fn enforce_asil_b_policy(
        &self,
        task: &FuelAsyncTask,
        fuel_to_consume: u64,
        remaining_fuel: u64,
        policy: &ASILBPolicy,
    ) -> Result<FuelEnforcementDecision> {
        // Check slice budget
        let current_slice_consumed =
            task.execution_context.context_fuel_consumed.load(Ordering::Acquire);

        if current_slice_consumed + fuel_to_consume > policy.slice_fuel_budget {
            // Check if rollover is allowed
            if policy.allow_rollover {
                let rollover_allowed =
                    policy.slice_fuel_budget * policy.max_rollover_percent as u64 / 100;
                if current_slice_consumed + fuel_to_consume
                    <= policy.slice_fuel_budget + rollover_allowed
                {
                    return Ok(FuelEnforcementDecision::AllowWithRollover {
                        rollover_amount: (current_slice_consumed + fuel_to_consume)
                            - policy.slice_fuel_budget,
                    });
                }
            }

            return Ok(FuelEnforcementDecision::RequireYield {
                reason: String::from("ASIL-B: Time slice fuel budget exceeded"),
            });
        }

        Ok(FuelEnforcementDecision::Allow)
    }

    /// Enforce ASIL-A flexible fuel policy
    fn enforce_asil_a_policy(
        &self,
        task: &FuelAsyncTask,
        fuel_to_consume: u64,
        _remaining_fuel: u64,
        policy: &ASILAPolicy,
    ) -> Result<FuelEnforcementDecision> {
        let total_consumed = task.fuel_consumed.load(Ordering::Acquire) + fuel_to_consume;

        // Check hard limit
        if total_consumed > policy.hard_limit {
            return Ok(FuelEnforcementDecision::Deny {
                reason: String::from("ASIL-A: Hard fuel limit exceeded"),
            });
        }

        // Check soft limit
        if total_consumed > policy.soft_limit {
            // In real implementation, would check grace period timing
            return Ok(FuelEnforcementDecision::AllowWithWarning {
                warning: format!(
                    "ASIL-A: Soft limit exceeded: {} > {}",
                    total_consumed, policy.soft_limit
                ),
            });
        }

        Ok(FuelEnforcementDecision::Allow)
    }

    /// Spawn a new async task with fuel budget and optional binary data
    pub fn spawn_task_with_binary<F>(
        &mut self,
        component_id: ComponentInstanceId,
        fuel_budget: u64,
        priority: Priority,
        future: F,
        binary_data: Option<&[u8]>,
    ) -> Result<TaskId>
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        // Extract resource limits from binary if available
        let asil_config = if let Some(wasm_bytes) = binary_data {
            extract_resource_limits_from_binary(wasm_bytes, self.asil_mode_for_priority(priority))
                .unwrap_or_else(|_| {
                    Some(ASILExecutionConfig::from_asil_requirements(
                        self.asil_mode_for_priority(priority),
                        1,
                    ))
                })
                .unwrap_or_else(|| {
                    ASILExecutionConfig::from_asil_requirements(
                        self.asil_mode_for_priority(priority),
                        1,
                    )
                })
        } else {
            ASILExecutionConfig::from_asil_requirements(self.asil_mode_for_priority(priority), 1)
        };

        self.spawn_task_with_config(component_id, fuel_budget, priority, future, asil_config)
    }

    /// Spawn a new async task with fuel budget
    pub fn spawn_task<F>(
        &mut self,
        component_id: ComponentInstanceId,
        fuel_budget: u64,
        priority: Priority,
        future: F,
    ) -> Result<TaskId>
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.spawn_task_with_binary(component_id, fuel_budget, priority, future, None)
    }

    /// Spawn a new async task with explicit configuration
    fn spawn_task_with_config<F>(
        &mut self,
        component_id: ComponentInstanceId,
        fuel_budget: u64,
        priority: Priority,
        future: F,
        asil_config: ASILExecutionConfig,
    ) -> Result<TaskId>
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        // Calculate dynamic fuel allocation if enabled
        let allocated_fuel = if let Some(ref mut fuel_mgr) = self.fuel_manager {
            fuel_mgr.calculate_fuel_allocation(
                self.next_task_id.load(Ordering::Acquire) as u32,
                component_id,
                fuel_budget,
                priority,
            )?
        } else {
            fuel_budget
        };

        // Check global fuel availability
        if self.fuel_enforcement.load(Ordering::Acquire) {
            let global_consumed = self.global_fuel_consumed.load(Ordering::Acquire);
            let global_limit = self.global_fuel_limit.load(Ordering::Acquire);

            if global_consumed + allocated_fuel > global_limit {
                return Err(Error::resource_limit_exceeded(
                    "Global fuel limit would be exceeded",
                ));
            }
        }

        // Generate new task ID
        let task_id = TaskId::new(self.next_task_id.fetch_add(1, Ordering::AcqRel) as u32);

        // Record task spawn operation
        record_global_operation(OperationType::FunctionCall, self.default_verification_level);

        // Consume fuel for task spawn
        self.consume_global_fuel(ASYNC_TASK_SPAWN_FUEL)?;

        // Register with preemption manager if enabled
        if let Some(ref mut preempt_mgr) = self.preemption_manager {
            preempt_mgr.register_task(task_id.into_inner(), priority, true, allocated_fuel)?;
        }

        // Create the task with ExecutionContext integration using the provided config
        let mut execution_context = ExecutionContext::from_config(asil_config)?;

        // Try to set component instance for real WebAssembly execution
        if let Some(component_instance) = self.get_component_instance(component_id) {
            execution_context.set_component_instance(component_instance);
        }

        let task = FuelAsyncTask {
            id: task_id,
            component_id,
            fuel_budget: allocated_fuel,
            fuel_consumed: AtomicU64::new(0),
            priority,
            verification_level: self.default_verification_level,
            state: AsyncTaskState::Ready,
            waker: None,
            future: DebugFuture::new(Box::pin(future)),
            execution_context,
            waiting_on_waitables: None,
        };

        // Store the task
        self.tasks
            .insert(task_id, task)
            .map_err(|_| Error::resource_limit_exceeded("Too many concurrent async tasks"))?;

        // Add to ready queue
        self.ready_queue
            .lock()
            .push(task_id)
            .map_err(|_| Error::resource_limit_exceeded("Ready queue is full"))?;

        Ok(task_id)
    }

    /// Poll ready tasks and advance execution
    pub fn poll_tasks(&mut self) -> Result<usize> {
        if self.executor_state != ExecutorState::Running {
            return Ok(0);
        }

        let mut tasks_polled = 0;
        let mut fuel_consumed_this_batch = 0u64;

        // Process wake coalescing if available
        if let Some(ref mut coalescer) = self.wake_coalescer {
            // Convert StaticVec<TaskId, 128> to StaticVec<u32, 128> for process_wakes
            // Note: wake_coalescer expects u32 task IDs, not TaskId struct
            // This is a temporary solution until the type system is unified
            let wakes_processed = 0; // TODO: Implement conversion or update wake_coalescer signature
            self.polling_stats.wakes_coalesced += wakes_processed;
        }

        // Process ready tasks with fuel-aware scheduling
        while let Some(task_id) = self.get_next_ready_task() {
            // Extract task data before borrowing self
            let (should_check, fuel_exhausted, needs_preemption_check) = {
                if let Some(task) = self.tasks.get(&task_id) {
                    let should_check_fuel = self.should_check_fuel(task);
                    let fuel_exhausted = should_check_fuel
                        && (task.fuel_consumed.load(Ordering::Acquire) >= task.fuel_budget);
                    (
                        should_check_fuel,
                        fuel_exhausted,
                        self.preemption_manager.is_some(),
                    )
                } else {
                    continue;
                }
            };

            // Handle fuel exhaustion
            if fuel_exhausted {
                if let Some(ref mut fuel_mgr) = self.fuel_manager {
                    if let Ok(emergency_fuel) =
                        fuel_mgr.handle_fuel_exhaustion(task_id.into_inner())
                    {
                        if let Some(task) = self.tasks.get_mut(&task_id) {
                            task.fuel_budget += emergency_fuel;
                        }
                    } else {
                        if let Some(task) = self.tasks.get_mut(&task_id) {
                            task.state = AsyncTaskState::FuelExhausted;
                        }
                        continue;
                    }
                } else {
                    if let Some(task) = self.tasks.get_mut(&task_id) {
                        task.state = AsyncTaskState::FuelExhausted;
                    }
                    continue;
                }
            }

            // Consume fuel - need to do this without holding task borrow
            {
                let task = match self.tasks.get(&task_id) {
                    Some(t) => t,
                    None => continue,
                };
                self.consume_task_fuel(task, ASYNC_TASK_POLL_FUEL)?
                // task borrow is dropped here
            }
            fuel_consumed_this_batch += ASYNC_TASK_POLL_FUEL;

            // Check preemption if enabled - avoid borrowing self.preemption_manager and self at same time
            if needs_preemption_check {
                let preemption_decision = {
                    // Temporarily take ownership to avoid borrow conflicts
                    let mut temp_mgr = self.preemption_manager.take();
                    let decision = if let Some(ref mut mgr) = temp_mgr {
                        mgr.check_preemption(task_id.into_inner(), ASYNC_TASK_POLL_FUEL, self)?
                    } else {
                        PreemptionDecision::Continue
                    };
                    // Put it back
                    self.preemption_manager = temp_mgr;
                    decision
                };

                match preemption_decision {
                    PreemptionDecision::Continue => {},
                    PreemptionDecision::YieldPoint => {
                        // Re-add to ready queue and yield
                        self.ready_queue.lock().push(task_id).ok();
                        continue;
                    },
                    PreemptionDecision::Preempt(_reason) => {
                        // Save state and preempt
                        if let Some(task) = self.tasks.get_mut(&task_id) {
                            task.state = AsyncTaskState::Waiting;
                        }
                        self.ready_queue.lock().push(task_id).ok();
                        self.polling_stats.tasks_yielded += 1;
                        continue;
                    },
                }
            }

            // Continue with task execution
            // Step 1: Update task waker (needs mutable borrow)
            {
                if let Some(task) = self.tasks.get_mut(&task_id) {
                    let waker = create_noop_waker();
                    task.waker = Some(waker);
                }
                // task borrow is dropped here
            }

            // Step 2: Execute task (needs &mut self, so no task borrow can be held)
            let verification_level = {
                self.tasks
                    .get(&task_id)
                    .map(|t| t.verification_level)
                    .unwrap_or(wrt_foundation::verification::VerificationLevel::Standard)
            };
            record_global_operation(OperationType::FunctionCall, verification_level);

            let waker = create_noop_waker();
            let mut context = Context::from_waker(&waker);
            let execution_result = self.execute_task_with_context(task_id, &mut context);

            // Step 3: Handle execution result (get fresh mutable borrow for each operation)
            match execution_result {
                Ok(Some(_result)) => {
                    // Task completed successfully
                    if let Some(task) = self.tasks.get_mut(&task_id) {
                        task.state = AsyncTaskState::Completed;
                    }

                    // Grant credit for unused fuel when task completes
                    if let Some(system) = &mut self.debt_credit_system {
                        if let Some(task) = self.tasks.get(&task_id) {
                            let consumed = task.fuel_consumed.load(Ordering::Acquire);
                            let unused_fuel = task.fuel_budget.saturating_sub(consumed);

                            if unused_fuel > 0 {
                                // Grant credit to the component for efficient fuel usage
                                let _ = system.grant_credit(
                                    task.component_id,
                                    unused_fuel,
                                    CreditRestriction::ForComponent {
                                        component_id: task.component_id,
                                    },
                                );
                            }
                        }
                    }

                    self.remove_task_fuel_tracking(task_id);
                    self.polling_stats.tasks_completed += 1;

                    // Update fuel manager history
                    if let Some(ref mut fuel_mgr) = self.fuel_manager {
                        if let Some(task) = self.tasks.get(&task_id) {
                            let fuel_consumed = task.fuel_consumed.load(Ordering::Acquire);
                            fuel_mgr
                                .update_task_history(task_id.into_inner(), fuel_consumed, 1, true)
                                .ok();
                        }
                    }
                },
                Ok(None) => {
                    // Task is waiting for async operation or yielded
                    if let Some(task) = self.tasks.get_mut(&task_id) {
                        task.state = AsyncTaskState::Waiting;
                    }
                    self.polling_stats.tasks_yielded += 1;
                    // Task will be re-added to ready queue when woken
                },
                Err(_) => {
                    // Task failed
                    if let Some(task) = self.tasks.get_mut(&task_id) {
                        task.state = AsyncTaskState::Failed;
                    }
                    self.remove_task_fuel_tracking(task_id);
                    self.polling_stats.tasks_failed += 1;
                },
            }

            tasks_polled += 1;
            self.polling_stats.total_polls += 1;

            // Check if we should yield based on fuel consumption
            if fuel_consumed_this_batch >= YIELD_FUEL_THRESHOLD {
                break;
            }
        }

        Ok(tasks_polled)
    }

    /// Get next ready task with priority consideration
    fn get_next_ready_task(&mut self) -> Option<TaskId> {
        self.ready_queue.lock().pop()
    }

    /// Wake a task and add it to the ready queue
    pub fn wake_task(&mut self, task_id: TaskId) -> Result<()> {
        // Check state and extract verification level before mutable borrow
        let (needs_wake, verification_level) = if let Some(task) = self.tasks.get(&task_id) {
            (
                task.state == AsyncTaskState::Waiting,
                task.verification_level,
            )
        } else {
            return Ok(());
        };

        if needs_wake {
            // Consume fuel before mutable borrow
            self.consume_fuel(ASYNC_TASK_WAKE_FUEL, verification_level)?;

            // Now mutate the task
            if let Some(task) = self.tasks.get_mut(&task_id) {
                task.state = AsyncTaskState::Ready;
                // Use wake coalescer if available
                if let Some(ref mut coalescer) = self.wake_coalescer {
                    coalescer.add_wake(task_id)?;
                } else {
                    self.ready_queue
                        .lock()
                        .push(task_id)
                        .map_err(|_| Error::resource_limit_exceeded("Ready queue is full"))?;
                }

                record_global_operation(OperationType::ControlFlow, verification_level);
            }
        }
        Ok(())
    }

    /// Get task status including fuel information
    pub fn get_task_status(&self, task_id: TaskId) -> Option<AsyncTaskStatus> {
        self.tasks.get(&task_id).map(|task| AsyncTaskStatus {
            id: task.id,
            component_id: task.component_id,
            state: task.state,
            fuel_budget: task.fuel_budget,
            fuel_consumed: task.fuel_consumed.load(Ordering::Acquire),
            priority: task.priority,
            verification_level: task.verification_level,
        })
    }

    /// Get global fuel status
    pub fn get_global_fuel_status(&self) -> GlobalAsyncFuelStatus {
        GlobalAsyncFuelStatus {
            limit: self.global_fuel_limit.load(Ordering::Acquire),
            consumed: self.global_fuel_consumed.load(Ordering::Acquire),
            enforcement_enabled: self.fuel_enforcement.load(Ordering::Acquire),
            active_tasks: self.tasks.len(),
            ready_tasks: self.ready_queue.lock().len(),
        }
    }

    /// Get polling statistics
    pub fn get_polling_stats(&self) -> PollingStatistics {
        self.polling_stats.clone()
    }

    /// Reset polling statistics
    pub fn reset_polling_stats(&mut self) {
        self.polling_stats = PollingStatistics::default();
    }

    /// Consume fuel from global budget with verification level tracking
    pub fn consume_fuel(
        &mut self,
        amount: u64,
        verification_level: VerificationLevel,
    ) -> Result<()> {
        // Check global fuel limit
        let current_consumed = self.global_fuel_consumed.load(Ordering::Acquire);
        let limit = self.global_fuel_limit.load(Ordering::Acquire);

        if self.fuel_enforcement.load(Ordering::Acquire) && current_consumed + amount > limit {
            return Err(Error::async_fuel_exhausted("Global fuel limit exceeded"));
        }

        // Update global consumption
        self.global_fuel_consumed.fetch_add(amount, Ordering::AcqRel);

        // Record operation for verification tracking
        record_global_operation(OperationType::Computation, verification_level);

        Ok(())
    }

    /// Shutdown the executor gracefully
    pub fn shutdown(&mut self) -> Result<()> {
        self.executor_state = ExecutorState::ShuttingDown;

        // Cancel all remaining tasks
        for (_, task) in self.tasks.iter_mut() {
            if matches!(task.state, AsyncTaskState::Ready | AsyncTaskState::Waiting) {
                task.state = AsyncTaskState::Cancelled;
            }
        }

        // Clear ready queue
        {
            let mut queue = self.ready_queue.lock();
            queue.clear();
        }

        self.executor_state = ExecutorState::Stopped;
        Ok(())
    }

    /// Consume fuel for a specific task by ID
    pub fn consume_fuel_for_task(&self, task_id: TaskId, amount: u64) -> Result<()> {
        if let Some(task) = self.tasks.get(&task_id) {
            self.consume_task_fuel(task, amount)?;
        }
        Ok(())
    }

    /// Update task state
    pub fn set_task_state(&mut self, task_id: TaskId, state: AsyncTaskState) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.state = state;
            Ok(())
        } else {
            Err(Error::validation_invalid_input("Task not found"))
        }
    }

    /// Get mutable reference to a task's execution context for yielding
    pub fn get_task_execution_context_mut(
        &mut self,
        task_id: TaskId,
    ) -> Option<&mut ExecutionContext> {
        self.tasks.get_mut(&task_id).map(|task| &mut task.execution_context)
    }

    /// Get task state
    pub fn get_task_state(&self, task_id: TaskId) -> Option<AsyncTaskState> {
        self.tasks.get(&task_id).map(|task| task.state)
    }

    // Private helper methods

    fn asil_mode_for_priority(&self, priority: Priority) -> ASILExecutionMode {
        match priority {
            225..=255 => ASILExecutionMode::D {
                // Critical priority
                deterministic_execution: true,
                bounded_execution_time: true,
                formal_verification: true,
                max_fuel_per_slice: 1000,
            },
            161..=224 => ASILExecutionMode::C {
                // High priority
                spatial_isolation: true,
                temporal_isolation: true,
                resource_isolation: true,
            },
            97..=160 => ASILExecutionMode::B {
                // Normal priority
                strict_resource_limits: true,
                max_execution_slice_ms: 10,
            },
            0..=96 => ASILExecutionMode::A {
                // Low priority
                error_detection: true,
            },
        }
    }

    fn should_check_fuel(&self, task: &FuelAsyncTask) -> bool {
        self.fuel_enforcement.load(Ordering::Acquire) && task.fuel_budget > 0
    }

    fn consume_task_fuel(&self, task: &FuelAsyncTask, amount: u64) -> Result<()> {
        if !self.should_check_fuel(task) {
            return Ok(());
        }

        // Check if task has sufficient fuel budget
        let consumed = task.fuel_consumed.load(Ordering::Acquire);
        let remaining = task.fuel_budget.saturating_sub(consumed);

        if amount > remaining {
            // Not enough fuel - check if debt is allowed
            let deficit = amount - remaining;

            if let Some(system) = &self.debt_credit_system {
                // First try to use component credit
                if let Ok(credit_used) = system.use_credit(task.component_id, deficit, task.id) {
                    if credit_used >= deficit {
                        // Credit covered the deficit
                        task.fuel_consumed.fetch_add(amount, Ordering::AcqRel);
                        self.consume_global_fuel(amount)?;
                        return Ok(());
                    }
                    // Partial credit - reduce deficit
                    let remaining_deficit = deficit - credit_used;

                    // Check if we can incur debt for the rest
                    if self.can_incur_debt(task.id, remaining_deficit) {
                        // We'll handle the actual debt incurral after
                        // consumption
                        // to ensure atomicity
                    } else {
                        return Err(Error::runtime_execution_error(
                            "Task credit exhausted: deficit exceeds credit limit",
                        ));
                    }
                } else if !self.can_incur_debt(task.id, deficit) {
                    return Err(Error::new(
                        ErrorCategory::Resource,
                        codes::RESOURCE_LIMIT_EXCEEDED,
                        "Task cannot incur additional fuel debt: budget exceeded",
                    ));
                }
            } else {
                // No debt/credit system - strict enforcement
                return Err(Error::runtime_execution_error(
                    "Insufficient fuel available for operation",
                ));
            }
        }

        // Enforce ASIL-specific fuel policy before consumption
        if let Some(_policy) = &self.fuel_enforcement_policy {
            match self.enforce_asil_fuel_policy(task, amount)? {
                FuelEnforcementDecision::Allow => {
                    // Continue with normal consumption
                },
                FuelEnforcementDecision::Deny { reason: _ } => {
                    return Err(Error::new(
                        ErrorCategory::Resource,
                        codes::RESOURCE_LIMIT_EXCEEDED,
                        "ASIL fuel policy denied operation",
                    ));
                },
                FuelEnforcementDecision::AllowWithWarning { warning } => {
                    // Log warning but continue
                },
                FuelEnforcementDecision::AllowWithDebt { debt_amount } => {
                    // Allow consumption with debt tracking
                    // In real implementation, would log: warning
                },
                FuelEnforcementDecision::AllowWithTransfer {
                    transfer_amount,
                    source_component,
                } => {
                    // In real implementation, would transfer fuel from source
                    // For now, just continue
                },
                FuelEnforcementDecision::AllowWithRollover { rollover_amount } => {
                    // In real implementation, would track rollover
                    // For now, just continue
                },
                FuelEnforcementDecision::RequireYield { .. } => {
                    // Task must yield before consuming more fuel
                    // Use a static string to avoid lifetime issues
                    return Err(Error::new(
                        ErrorCategory::Resource,
                        codes::EXECUTION_LIMIT_EXCEEDED,
                        "Task must yield before consuming more fuel",
                    ));
                },
            }
        }

        task.fuel_consumed.fetch_add(amount, Ordering::AcqRel);

        // Update fuel monitor if enabled
        // Note: FuelMonitor uses interior mutability (Atomics and Mutex)
        // so it can be called from &self context
        if let Some(monitor) = &self.fuel_monitor {
            monitor.update_consumption(amount, task.id, task.execution_context.asil_config.mode)?;
        }

        self.consume_global_fuel(amount)
    }

    pub fn consume_global_fuel(&self, amount: u64) -> Result<()> {
        if self.fuel_enforcement.load(Ordering::Acquire) {
            let consumed = self.global_fuel_consumed.fetch_add(amount, Ordering::AcqRel);
            let limit = self.global_fuel_limit.load(Ordering::Acquire);

            if consumed + amount > limit {
                return Err(Error::runtime_execution_error("Global fuel limit exceeded"));
            }
        }
        Ok(())
    }

    fn remove_task_fuel_tracking(&self, task_id: TaskId) {
        if let Some(task) = self.tasks.get(&task_id) {
            let consumed = task.fuel_consumed.load(Ordering::Acquire);
            let remaining = task.fuel_budget.saturating_sub(consumed);

            // Return unused fuel to global pool
            if remaining > 0 && self.fuel_enforcement.load(Ordering::Acquire) {
                self.global_fuel_consumed.fetch_sub(remaining, Ordering::AcqRel);
            }
        }
    }

    /// Execute task using ExecutionContext for real WebAssembly execution
    fn execute_task_with_context(
        &mut self,
        task_id: TaskId,
        waker_context: &mut Context<'_>,
    ) -> Result<Option<Vec<u8>>> {
        // Extract needed data before taking mutable borrows
        let (can_continue, asil_key, step_fuel, has_component_instance) = {
            let task = self
                .tasks
                .get_mut(&task_id)
                .ok_or_else(|| Error::validation_invalid_input("Task not found for execution"))?;

            let can_continue = task.execution_context.can_continue_execution()?;
            let asil_key = match task.execution_context.asil_config.mode {
                ASILExecutionMode::D { .. } | ASILExecutionMode::AsilD => 0,
                ASILExecutionMode::C { .. } | ASILExecutionMode::AsilC => 1,
                ASILExecutionMode::B { .. } | ASILExecutionMode::AsilB => 2,
                ASILExecutionMode::A { .. } | ASILExecutionMode::AsilA => 3,
                ASILExecutionMode::QM => 4,
            };
            let step_fuel = task.execution_context.asil_config.limits.get_fuel_limit();
            let has_component = task.execution_context.component_instance.is_some();

            (can_continue, asil_key, step_fuel, has_component)
        };

        // Check if execution can continue based on ASIL constraints
        if !can_continue {
            // Must yield due to ASIL constraints
            return Ok(None);
        }

        // Use ASIL-specific executor if available
        if let Some(asil_executor) = self.asil_executors.get_mut(&asil_key) {
            let task = self.tasks.get_mut(&task_id).unwrap();
            // Execute using the ASIL-specific executor
            let waker = waker_context.waker().clone();
            // Convert from fuel_async_executor::TaskId to types::TaskId
            let types_task_id = crate::types::TaskId::new(task_id.into_inner());
            match asil_executor.execute_step(types_task_id, &mut task.execution_context, &waker)? {
                ExecutionStepResult::Completed(result) => {
                    return Ok(Some(result));
                },
                ExecutionStepResult::Yielded => {
                    return Ok(None);
                },
                ExecutionStepResult::Waiting => {
                    task.state = AsyncTaskState::Waiting;
                    return Ok(None);
                },
            }
        }

        // Fallback to original execution if no ASIL executor
        // Consume fuel for execution step based on configuration
        {
            let task = self.tasks.get_mut(&task_id).unwrap();
            task.execution_context.consume_fuel(step_fuel);
        }
        // Consume task fuel in separate scope to avoid borrow conflict
        {
            let task = self.tasks.get(&task_id).unwrap();
            self.consume_task_fuel(task, step_fuel)?;
        }

        // Execute WebAssembly function if component instance is available
        if has_component_instance {
            // For now, return a placeholder result
            // Real WebAssembly execution would require refactoring to avoid double borrow
            // The issue is that execute_wasm_function_with_fuel needs &mut self AND task
            // TODO: Refactor execution to work with task_id instead of task reference
            Ok(None)
        } else {
            // Poll the future as fallback when no component instance
            let task = self.tasks.get_mut(&task_id).unwrap();
            match task.future.as_mut().poll(waker_context) {
                Poll::Ready(Ok(())) => Ok(Some(vec![])),
                Poll::Ready(Err(e)) => Err(e),
                Poll::Pending => Ok(None),
            }
        }
    }

    /// Execute WebAssembly function using component instance
    fn execute_wasm_function(
        &mut self,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
        _waker_context: &mut Context<'_>,
    ) -> Result<ExecutionStepResult> {
        // Increment stack depth for this execution step
        task.execution_context.stack_depth += 1;

        // Check stack depth limits
        if task.execution_context.stack_depth >= task.execution_context.max_stack_depth {
            task.execution_context.stack_depth -= 1;
            return Err(Error::resource_limit_exceeded("Stack depth limit exceeded"));
        }

        // Execute based on ASIL mode constraints
        let execution_result = match task.execution_context.asil_config.mode {
            ASILExecutionMode::AsilD => {
                // ASIL-D requires deterministic execution (unit variant defaults)
                self.execute_deterministic_step(task, component_instance)
            },
            ASILExecutionMode::D {
                deterministic_execution: true,
                ..
            } => {
                // ASIL-D requires deterministic execution
                self.execute_deterministic_step(task, component_instance)
            },
            ASILExecutionMode::AsilC => {
                // ASIL-C requires isolation enforcement (unit variant defaults)
                self.execute_isolated_step(task, component_instance)
            },
            ASILExecutionMode::C {
                spatial_isolation: true,
                ..
            } => {
                // ASIL-C requires isolation enforcement
                self.execute_isolated_step(task, component_instance)
            },
            ASILExecutionMode::AsilB => {
                // ASIL-B requires resource limit enforcement (unit variant defaults)
                self.execute_resource_limited_step(task, component_instance)
            },
            ASILExecutionMode::B {
                strict_resource_limits: true,
                ..
            } => {
                // ASIL-B requires resource limit enforcement
                self.execute_resource_limited_step(task, component_instance)
            },
            ASILExecutionMode::QM | ASILExecutionMode::AsilA => {
                // QM and ASIL-A have basic execution requirements
                self.execute_basic_step(task, component_instance)
            },
            ASILExecutionMode::A { .. } => {
                // ASIL-A has basic execution requirements
                self.execute_basic_step(task, component_instance)
            },
            _ => {
                // Catch-all for any other modes
                self.execute_basic_step(task, component_instance)
            },
        };

        // Decrement stack depth after execution
        task.execution_context.stack_depth -= 1;

        execution_result
    }

    /// Execute deterministic step for ASIL-D
    fn execute_deterministic_step(
        &mut self,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
    ) -> Result<ExecutionStepResult> {
        // For ASIL-D, execution must be deterministic and bounded
        let fuel_consumed = task.execution_context.context_fuel_consumed.load(Ordering::Acquire);

        if let ASILExecutionMode::D {
            max_fuel_per_slice, ..
        } = task.execution_context.asil_config.mode
        {
            if fuel_consumed >= max_fuel_per_slice {
                // Must yield to maintain deterministic timing
                return Ok(ExecutionStepResult::Yielded);
            }
        }

        // Execute real WebAssembly with ASIL-D constraints from configuration
        // Note: wrt-runtime defaults to std feature, so StacklessEngine::new() returns Self
        let mut engine = wrt_runtime::stackless::engine::StacklessEngine::new();

        // Note: StacklessEngine is a simple engine without fuel/constraint management
        // Fuel tracking is done at the task level via task.fuel_consumed
        // Set fuel limit from configuration
        let _fuel_limit = task.execution_context.asil_config.limits.get_fuel_limit();
        // engine.set_fuel(Some(fuel_limit)); // Not supported by StacklessEngine

        // Enable deterministic mode
        // engine.set_deterministic_mode(true); // Not supported by StacklessEngine

        // Set instructions per step from configuration
        let _max_instructions = task.execution_context.asil_config.limits.get_instructions_limit();
        // engine.set_max_instructions_per_step(max_instructions); // Not supported by StacklessEngine

        // Execute single instruction step
        self.execute_single_instruction_step(&mut engine, task, component_instance, _fuel_limit)
    }

    /// Execute isolated step for ASIL-C
    fn execute_isolated_step(
        &mut self,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
    ) -> Result<ExecutionStepResult> {
        // For ASIL-C, ensure spatial, temporal, and resource isolation

        // Check temporal isolation - no interference from other tasks
        let current_time = task.execution_context.get_deterministic_timestamp();

        // Check if we need to yield for temporal isolation
        if current_time % 1000 == 0 {
            // Yield every 1000 fuel units
            return Ok(ExecutionStepResult::Yielded);
        }

        // Execute real WebAssembly with isolation constraints from configuration
        // Note: wrt-runtime defaults to std feature, so StacklessEngine::new() returns Self
        let mut engine = wrt_runtime::stackless::engine::StacklessEngine::new();

        // Note: StacklessEngine is a simple engine without fuel/constraint management
        // Set fuel limit from configuration
        let _fuel_limit = task.execution_context.asil_config.limits.get_fuel_limit();
        // engine.set_fuel(Some(fuel_limit)); // Not supported by StacklessEngine

        // Set up isolation constraints from configuration
        // engine.set_memory_isolation(true); // Not supported by StacklessEngine
        let _max_stack_depth = task.execution_context.asil_config.limits.get_call_depth_limit();
        // engine.set_max_stack_depth(max_stack_depth); // Not supported by StacklessEngine

        // Set memory limit from configuration
        let _memory_limit = task.execution_context.asil_config.limits.get_memory_limit();
        // engine.set_max_memory_usage(memory_limit); // Not supported by StacklessEngine

        let max_instructions = task.execution_context.asil_config.limits.get_instructions_limit();
        self.execute_single_instruction_step(
            &mut engine,
            task,
            component_instance,
            max_instructions as u64,
        )
    }

    /// Execute resource-limited step for ASIL-B
    fn execute_resource_limited_step(
        &mut self,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
    ) -> Result<ExecutionStepResult> {
        // For ASIL-B, enforce strict resource limits

        if let ASILExecutionMode::B {
            max_execution_slice_ms,
            ..
        } = task.execution_context.asil_config.mode
        {
            let fuel_consumed =
                task.execution_context.context_fuel_consumed.load(Ordering::Acquire);
            let max_fuel = max_execution_slice_ms as u64 * 10; // 10 fuel per ms

            if fuel_consumed >= max_fuel {
                return Ok(ExecutionStepResult::Yielded);
            }
        }

        // Execute real WebAssembly with resource limits
        // Note: wrt-runtime defaults to std feature, so StacklessEngine::new() returns Self
        let mut engine = wrt_runtime::stackless::engine::StacklessEngine::new();
        // Note: StacklessEngine is a simple engine without fuel/constraint management
        // engine.set_fuel(Some(400)); // Bounded fuel for ASIL-B - Not supported

        // Set resource limits
        // engine.set_max_memory_usage(64 * 1024); // 64KB memory limit - Not supported
        // engine.set_max_call_depth(10); // Not supported

        self.execute_single_instruction_step(&mut engine, task, component_instance, 50)
    }

    /// Execute basic step for ASIL-A
    fn execute_basic_step(
        &mut self,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
    ) -> Result<ExecutionStepResult> {
        // For ASIL-A, basic execution with error detection

        // Basic error detection
        if task.execution_context.stack_depth > 100 {
            return Err(Error::runtime_execution_error("Stack depth warning"));
        }

        // Execute real WebAssembly with relaxed constraints
        // Note: wrt-runtime defaults to std feature, so StacklessEngine::new() returns Self
        let mut engine = wrt_runtime::stackless::engine::StacklessEngine::new();
        // Note: StacklessEngine is a simple engine without fuel/constraint management
        // engine.set_fuel(Some(1000)); // More fuel for ASIL-A - Not supported

        // Relaxed limits for ASIL-A
        // engine.set_max_call_depth(50); // Not supported

        self.execute_single_instruction_step(&mut engine, task, component_instance, 100)
    }

    /// Execute a single instruction step with the given engine
    /// This is a stub implementation - real implementation would execute WebAssembly
    fn execute_single_instruction_step(
        &mut self,
        _engine: &mut wrt_runtime::stackless::engine::StacklessEngine,
        task: &mut FuelAsyncTask,
        _component_instance: &Arc<ComponentInstance>,
        fuel_limit: u64,
    ) -> Result<ExecutionStepResult> {
        // Update fuel consumption
        let consumed = task.fuel_consumed.load(Ordering::Acquire);
        let new_consumed = consumed.saturating_add(1); // Consume 1 unit of fuel per step
        task.fuel_consumed.store(new_consumed, Ordering::Release);

        // Check if we've exceeded the fuel limit
        if new_consumed >= fuel_limit {
            return Ok(ExecutionStepResult::Yielded);
        }

        // For now, just yield - real implementation would execute actual WebAssembly
        Ok(ExecutionStepResult::Yielded)
    }

    /// Get component instance for the given component ID
    fn get_component_instance(
        &self,
        component_id: ComponentInstanceId,
    ) -> Option<Arc<ComponentInstance>> {
        self.component_registry.get(&component_id).cloned()
    }

    /// Handle Component Model async operations (task.wait, task.yield,
    /// task.poll)
    pub fn handle_component_async_operation(
        &mut self,
        task_id: TaskId,
        operation: ComponentAsyncOperation,
    ) -> Result<ComponentAsyncOperationResult> {
        match operation {
            ComponentAsyncOperation::TaskWait { waitables } => {
                let task = self
                    .tasks
                    .get_mut(&task_id)
                    .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

                // Handle task.wait - suspend until one of the waitables is ready

                // Register all waitables for this task
                for &handle in &waitables {
                    self.waitable_registry.add_waiting_task(handle as u64, task_id)?;
                }

                // Check if any waitables are already ready
                let waitables_u64: Vec<u64> = waitables.iter().map(|&h| h as u64).collect();
                if let Some((ready_handle, index)) =
                    self.waitable_registry.poll_ready_waitables(&waitables_u64)
                {
                    // A waitable is ready - return immediately
                    return Ok(ComponentAsyncOperationResult::Waiting {
                        ready_index: Some(index as u32),
                    });
                }

                // No waitables ready - create yield point and suspend
                task.execution_context.create_async_yield_point(
                    task.execution_context.current_function_index,
                    waitables[0] as u64, // Use first waitable as resource ID
                )?;

                task.state = AsyncTaskState::Waiting;

                // Store waitables in task for later polling
                task.waiting_on_waitables = Some(waitables.iter().map(|&h| h as u64).collect());

                Ok(ComponentAsyncOperationResult::Waiting { ready_index: None })
            },
            ComponentAsyncOperation::TaskYield => {
                // Extract task data before consuming fuel
                let asil_mode = {
                    let task = self
                        .tasks
                        .get_mut(&task_id)
                        .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

                    // Handle task.yield - voluntarily yield execution
                    task.execution_context.create_yield_point(
                        0,      // Current instruction pointer
                        vec![], // Current stack frame
                        vec![], // Current local variables
                    )?;

                    task.execution_context.consume_fuel(ASYNC_TASK_YIELD_FUEL);
                    task.state = AsyncTaskState::Waiting;
                    task.execution_context.asil_config.mode
                };

                // Now consume fuel with separate borrow
                let task = self.tasks.get(&task_id).unwrap();
                self.consume_task_fuel(task, ASYNC_TASK_YIELD_FUEL)?;

                // Monitor yield operation
                if let Some(monitor) = &self.fuel_monitor {
                    // Yielding is important for ASIL-D determinism
                    if let ASILExecutionMode::D { .. } = asil_mode {
                        monitor.update_consumption(ASYNC_TASK_YIELD_FUEL, task_id, asil_mode)?;
                    }
                }

                Ok(ComponentAsyncOperationResult::Yielded)
            },
            ComponentAsyncOperation::TaskPoll { waitables } => {
                // Extract task data before consuming fuel
                {
                    let task = self
                        .tasks
                        .get_mut(&task_id)
                        .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;
                    task.execution_context.consume_fuel(ASYNC_TASK_POLL_FUEL);
                }

                // Now consume fuel with separate borrow
                let task = self.tasks.get(&task_id).unwrap();
                self.consume_task_fuel(task, ASYNC_TASK_POLL_FUEL)?;

                // Check if any waitables are ready
                let waitables_u64: Vec<u64> = waitables.iter().map(|&h| h as u64).collect();
                if let Some((ready_handle, index)) =
                    self.waitable_registry.poll_ready_waitables(&waitables_u64)
                {
                    // Found a ready waitable
                    Ok(ComponentAsyncOperationResult::Polled {
                        ready_index: Some(index as u32),
                    })
                } else {
                    // No waitables ready
                    Ok(ComponentAsyncOperationResult::Polled { ready_index: None })
                }
            },
        }
    }

    /// Resume a task from a yield point
    pub fn resume_task_from_yield_point(&mut self, task_id: TaskId) -> Result<()> {
        // Clone yield point data before mutable borrow
        let yield_point_data = self
            .tasks
            .get(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?
            .execution_context
            .last_yield_point
            .clone();

        // Check if task has a yield point to resume from
        if let Some(yield_point) = yield_point_data {
            // Get mutable reference to task and restore state
            let task = self.tasks.get_mut(&task_id).unwrap();
            Self::restore_execution_state_from_yield_point(task, &yield_point)?;

            // Mark as ready for execution
            task.state = AsyncTaskState::Ready;

            // Add back to ready queue
            self.ready_queue
                .lock()
                .push(task_id)
                .map_err(|_| Error::resource_limit_exceeded("Ready queue is full"))?;
        }

        Ok(())
    }

    /// Restore execution state from a yield point
    fn restore_execution_state_from_yield_point(
        task: &mut FuelAsyncTask,
        yield_point: &YieldPoint,
    ) -> Result<()> {
        // 1. Restore instruction pointer
        // In real WebAssembly execution, would set the program counter
        // For now, we store it in the execution context for tracking
        if let Some(execution_state) = &mut task.execution_context.execution_state {
            // Would restore actual execution state here
            execution_state.restore_state(&[])?; // Would use real saved state
        }

        // 2. Restore stack frame
        // Restore the stack frame values at the time of yield
        for (index, value) in yield_point.stack.iter().enumerate() {
            if index < task.execution_context.stack_depth as usize {
                // In real implementation, would restore actual stack values
                // For now, just track that we're restoring
            }
        }

        // 3. Restore local variables
        // Restore local variable state from yield point
        if let Some(execution_state) = &mut task.execution_context.execution_state {
            // Convert from yield point locals (wrt_foundation::Value) to ComponentValue
            let mut locals_vec = wrt_foundation::Vec::new();
            for local in yield_point.locals.iter() {
                // Convert Value to ComponentValue
                let component_val = match local {
                    wrt_foundation::Value::I32(v) => WrtComponentValue::U32(*v as u32),
                    wrt_foundation::Value::I64(v) => WrtComponentValue::U64(*v as u64),
                    wrt_foundation::Value::F32(v) => {
                        WrtComponentValue::F32(wrt_foundation::FloatBits32::from_bits(v.to_bits()))
                    },
                    wrt_foundation::Value::F64(v) => {
                        WrtComponentValue::F64(wrt_foundation::FloatBits64::from_bits(v.to_bits()))
                    },
                    _ => WrtComponentValue::Unit,
                };
                locals_vec.push(component_val);
            }
            execution_state.set_locals(locals_vec)?;
        }

        // 4. Restore fuel consumption state
        // Don't double-count fuel that was already consumed before yield
        let fuel_at_yield = yield_point.fuel_at_yield;
        let current_fuel = task.execution_context.context_fuel_consumed.load(Ordering::Acquire);

        // Ensure we don't go backwards in fuel consumption
        if current_fuel < fuel_at_yield {
            task.execution_context
                .context_fuel_consumed
                .store(fuel_at_yield, Ordering::Release);
        }

        // 5. Validate deterministic execution for ASIL-D
        if let ASILExecutionMode::D {
            deterministic_execution: true,
            ..
        } = task.execution_context.asil_config.mode
        {
            // Verify deterministic timestamp consistency
            let current_timestamp = task.execution_context.get_deterministic_timestamp();
            if current_timestamp < yield_point.yield_timestamp {
                return Err(Error::runtime_execution_error(
                    "Deterministic execution violation during resume",
                ));
            }
        }

        // 6. Clear the yield point since we've resumed
        task.execution_context.last_yield_point = None;

        Ok(())
    }

    /// Set component instance for real WebAssembly execution
    pub fn set_component_instance_for_task(
        &mut self,
        task_id: TaskId,
        component_instance: Arc<ComponentInstance>,
    ) -> Result<()> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

        task.execution_context.set_component_instance(component_instance);
        Ok(())
    }

    /// Execute WebAssembly function with fuel integration using StacklessEngine
    fn execute_wasm_function_with_fuel(
        &mut self,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
        _waker_context: &mut Context<'_>,
    ) -> Result<ExecutionStepResult> {
        // Create a StacklessEngine for WebAssembly execution
        // Note: wrt-runtime defaults to std feature, so StacklessEngine::new() returns Self
        let mut engine = wrt_runtime::stackless::engine::StacklessEngine::new();

        // Set fuel limit based on task's remaining fuel budget
        let consumed = task.fuel_consumed.load(Ordering::Acquire);
        let remaining_fuel = task.fuel_budget.saturating_sub(consumed);

        if remaining_fuel == 0 {
            return Ok(ExecutionStepResult::Yielded);
        }

        // Set fuel for the engine - this integrates with instruction-level fuel
        // consumption
        // Note: StacklessEngine doesn't support fuel management
        // Fuel tracking is done at the task level via task.fuel_consumed
        // engine.set_fuel(Some(remaining_fuel)); // Not supported by StacklessEngine
        let _ = remaining_fuel; // Silence unused variable warning

        // Set verification level to match task's ASIL mode
        let verification_level = match task.execution_context.asil_config.mode {
            ASILExecutionMode::D { .. } | ASILExecutionMode::AsilD => {
                wrt_foundation::verification::VerificationLevel::Full
            },
            ASILExecutionMode::C { .. } | ASILExecutionMode::AsilC => {
                wrt_foundation::verification::VerificationLevel::Standard
            },
            ASILExecutionMode::B { .. } | ASILExecutionMode::AsilB => {
                wrt_foundation::verification::VerificationLevel::Basic
            },
            ASILExecutionMode::A { .. } | ASILExecutionMode::AsilA => {
                wrt_foundation::verification::VerificationLevel::Off
            },
            ASILExecutionMode::QM => wrt_foundation::verification::VerificationLevel::Off,
        };

        // Execute a limited number of WebAssembly instructions based on configuration
        let max_instructions_per_step =
            task.execution_context.asil_config.limits.get_instructions_limit();

        // Real WebAssembly execution step
        let initial_fuel = engine.remaining_fuel().unwrap_or(0);

        // Get function to execute from execution context
        // Clone yield point to avoid borrow conflict
        let yield_point_clone = task.execution_context.last_yield_point.clone();
        let execution_result = if let Some(ref yield_point) = yield_point_clone {
            // Resume from yield point
            self.resume_from_yield_point(&mut engine, task, yield_point, max_instructions_per_step)
        } else {
            // Start fresh execution
            self.execute_fresh_function(
                &mut engine,
                task,
                component_instance,
                max_instructions_per_step,
            )
        };

        // Update task fuel consumption based on what the engine consumed
        let final_fuel = engine.remaining_fuel().unwrap_or(0);
        let fuel_consumed_this_step = initial_fuel.saturating_sub(final_fuel);

        // Update task fuel tracking
        task.fuel_consumed.fetch_add(fuel_consumed_this_step, Ordering::AcqRel);
        task.execution_context
            .context_fuel_consumed
            .fetch_add(fuel_consumed_this_step, Ordering::AcqRel);

        execution_result
    }

    /// Execute fresh function from the beginning
    fn execute_fresh_function(
        &mut self,
        engine: &mut wrt_runtime::stackless::engine::StacklessEngine,
        task: &mut FuelAsyncTask,
        component_instance: &Arc<ComponentInstance>,
        max_instructions: u32,
    ) -> Result<ExecutionStepResult> {
        // Get the function to execute from the task's execution context
        let function_index = task.execution_context.current_function_index;

        // TODO: Fix module instance type mismatch
        // The component layer stores simplified ModuleInstance (with just module_index)
        // but the runtime engine needs wrt_runtime::ModuleInstance (full runtime instance).
        // This requires architectural changes to properly expose runtime instances through
        // the component layer.

        // For now, return an error indicating this path is not yet implemented
        Err(Error::runtime_execution_error(
            "Direct component instance execution not yet fully integrated - module instance type mismatch",
        ))

        // This is the intended code path once the type issue is resolved:
        // let _module_instance = match component_instance.get_core_module_instance(0) {
        //     Some(instance) => instance,
        //     None => {
        //         return Err(Error::runtime_execution_error(
        //             "Component instance not found",
        //         ));
        //     },
        // };
        //
        // let params = &task.execution_context.function_params;
        // let mut engine_params = wrt_foundation::Vec::new();
        // for param in params.iter() {
        //     let val = match param {
        //         &wrt_foundation::Value::I32(v) => wrt_foundation::Value::I32(v),
        //         &wrt_foundation::Value::I64(v) => wrt_foundation::Value::I64(v),
        //         &wrt_foundation::Value::F32(v) => wrt_foundation::Value::F32(v),
        //         &wrt_foundation::Value::F64(v) => wrt_foundation::Value::F64(v),
        //         _ => wrt_foundation::Value::I32(0),
        //     };
        //     engine_params.push(val);
        // }
        //
        // match engine.execute_function_step(
        //     runtime_module_instance,  // Need proper runtime instance here
        //     function_index as usize,
        //     &engine_params,
        //     max_instructions,
        // ) {
        //     Ok(wrt_runtime::stackless::ExecutionResult::Completed(values)) => {
        //         let result_bytes = self.serialize_values(values.as_slice()?)?;
        //         Ok(ExecutionStepResult::Completed(result_bytes))
        //     },
        //     Ok(wrt_runtime::stackless::ExecutionResult::Yielded(yield_info)) => {
        //         #[cfg(feature = "std")]
        //         let stack_vec: Vec<wrt_foundation::Value> = yield_info.operand_stack.iter().cloned().collect();
        //         #[cfg(not(feature = "std"))]
        //         let stack_vec: Vec<wrt_foundation::Value> = {
        //             let mut v = Vec::new();
        //             for item in yield_info.operand_stack.iter() {
        //                 v.push(item.clone());
        //             }
        //             v
        //         };
        //
        //         #[cfg(feature = "std")]
        //         let locals_vec: Vec<wrt_foundation::Value> = yield_info.locals.iter().cloned().collect();
        //         #[cfg(not(feature = "std"))]
        //         let locals_vec: Vec<wrt_foundation::Value> = {
        //             let mut v = Vec::new();
        //             for item in yield_info.locals.iter() {
        //                 v.push(item.clone());
        //             }
        //             v
        //         };
        //
        //         #[cfg(feature = "std")]
        //         let call_stack_vec: Vec<u32> = yield_info.call_stack.iter().cloned().collect();
        //         #[cfg(not(feature = "std"))]
        //         let call_stack_vec: Vec<u32> = {
        //             let mut v = Vec::new();
        //             for item in yield_info.call_stack.iter() {
        //                 v.push(item);
        //             }
        //             v
        //         };
        //
        //         task.execution_context.save_yield_point(YieldInfo {
        //             instruction_pointer: yield_info.instruction_pointer,
        //             stack:               stack_vec,
        //             locals:              locals_vec,
        //             call_stack:          call_stack_vec,
        //             yield_type:          YieldType::ExplicitYield,
        //             module_state:        None,
        //             globals:             Vec::new(),
        //             tables:              Vec::new(),
        //             memory_bounds:       None,
        //             function_context:    FunctionExecutionContext {
        //                 signature:          FunctionSignature {
        //                     params:  Vec::new(),
        //                     returns: Vec::new(),
        //                 },
        //                 function_kind:      FunctionKind::Local,
        //                 calling_convention: CallingConvention::WebAssembly,
        //             },
        //             resumption_condition: None,
        //         })?;
        //         Ok(ExecutionStepResult::Yielded)
        //     },
        //     Ok(wrt_runtime::stackless::ExecutionResult::Waiting(resource_id)) => {
        //         let waitable_handle = self
        //             .waitable_registry
        //             .register_waitable(task.component_id, Some(resource_id as u64))?;
        //         self.waitable_registry.add_waiting_task(waitable_handle, task.id)?;
        //         task.waiting_on_waitables = Some(vec![waitable_handle]);
        //         task.execution_context
        //             .create_async_yield_point(engine.get_instruction_pointer()?, resource_id as u64)?;
        //         Ok(ExecutionStepResult::Waiting)
        //     },
        //     Ok(wrt_runtime::stackless::ExecutionResult::FuelExhausted) => {
        //         Ok(ExecutionStepResult::Yielded)
        //     },
        //     Err(e) => {
        //         Err(e)
        //     },
        // }
    }

    /// Resume execution from a yield point
    fn resume_from_yield_point(
        &mut self,
        engine: &mut wrt_runtime::stackless::engine::StacklessEngine,
        task: &mut FuelAsyncTask,
        yield_point: &YieldPoint,
        max_instructions: u32,
    ) -> Result<ExecutionStepResult> {
        // Restore engine state from yield point
        use wrt_foundation::safe_memory::NoStdProvider;

        // Create BoundedVecs with the exact provider types expected by EngineState
        let mut operand_stack = wrt_foundation::bounded::BoundedVec::<
            wrt_foundation::Value,
            256,
            NoStdProvider<4096>,
        >::new(NoStdProvider::default())?;
        for val in &yield_point.stack {
            operand_stack.push(val.clone())?;
        }

        let mut locals_vec = wrt_foundation::bounded::BoundedVec::<
            wrt_foundation::Value,
            128,
            NoStdProvider<2048>,
        >::new(NoStdProvider::default())?;
        for val in &yield_point.locals {
            locals_vec.push(val.clone())?;
        }

        let mut call_stack_vec =
            wrt_foundation::bounded::BoundedVec::<u32, 64, NoStdProvider<512>>::new(
                NoStdProvider::default(),
            )?;
        for val in &yield_point.call_stack {
            call_stack_vec.push(*val)?;
        }

        engine.restore_state(wrt_runtime::stackless::EngineState {
            instruction_pointer: yield_point.instruction_pointer,
            operand_stack,
            locals: locals_vec,
            call_stack: call_stack_vec,
        })?;

        // Continue execution from where we left off
        match engine.continue_execution(max_instructions) {
            Ok(wrt_runtime::stackless::ExecutionResult::Completed(values)) => {
                // Function completed - clear yield point
                task.execution_context.last_yield_point = None;
                let result_bytes = self.serialize_values(values.as_slice()?)?;
                Ok(ExecutionStepResult::Completed(result_bytes))
            },
            Ok(wrt_runtime::stackless::ExecutionResult::Yielded(yield_info)) => {
                // Yielded again - update yield point
                // Convert wrt_runtime::stackless::YieldInfo to fuel_async_executor::YieldInfo
                #[cfg(feature = "std")]
                let stack_vec2: Vec<wrt_foundation::Value> =
                    yield_info.operand_stack.iter().collect();
                #[cfg(not(feature = "std"))]
                let stack_vec2: Vec<wrt_foundation::Value> = {
                    let mut v = Vec::new();
                    for item in yield_info.operand_stack.iter() {
                        v.push(item.clone());
                    }
                    v
                };

                #[cfg(feature = "std")]
                let locals_vec2: Vec<wrt_foundation::Value> = yield_info.locals.iter().collect();
                #[cfg(not(feature = "std"))]
                let locals_vec2: Vec<wrt_foundation::Value> = {
                    let mut v = Vec::new();
                    for item in yield_info.locals.iter() {
                        v.push(item.clone());
                    }
                    v
                };

                #[cfg(feature = "std")]
                let call_stack_vec2: Vec<u32> = yield_info.call_stack.iter().collect();
                #[cfg(not(feature = "std"))]
                let call_stack_vec2: Vec<u32> = {
                    let mut v = Vec::new();
                    for item in yield_info.call_stack.iter() {
                        v.push(item);
                    }
                    v
                };

                task.execution_context.save_yield_point(YieldInfo {
                    instruction_pointer: yield_info.instruction_pointer,
                    stack: stack_vec2,
                    locals: locals_vec2,
                    call_stack: call_stack_vec2,
                    yield_type: YieldType::ExplicitYield,
                    module_state: None,
                    globals: Vec::new(),
                    tables: Vec::new(),
                    memory_bounds: None,
                    function_context: FunctionExecutionContext {
                        signature: FunctionSignature {
                            params: Vec::new(),
                            returns: Vec::new(),
                        },
                        function_kind: FunctionKind::Local,
                        calling_convention: CallingConvention::WebAssembly,
                    },
                    resumption_condition: None,
                })?;
                Ok(ExecutionStepResult::Yielded)
            },
            Ok(wrt_runtime::stackless::ExecutionResult::Waiting(resource_id)) => {
                // Still waiting for resource
                // Create or update waitable for this resource
                let waitable_handle = self
                    .waitable_registry
                    .register_waitable(task.component_id, Some(resource_id as u64))?;

                // Register task as waiting on this waitable
                self.waitable_registry.add_waiting_task(waitable_handle, task.id)?;

                // Update task's waiting list
                task.waiting_on_waitables = Some(vec![waitable_handle]);

                // Update async yield point with new resource
                task.execution_context.create_async_yield_point(
                    engine.get_instruction_pointer()?,
                    resource_id as u64,
                )?;

                Ok(ExecutionStepResult::Waiting)
            },
            Ok(wrt_runtime::stackless::ExecutionResult::FuelExhausted) => {
                // Fuel exhausted - yield
                Ok(ExecutionStepResult::Yielded)
            },
            Err(e) => {
                // Execution error
                Err(e)
            },
        }
    }

    /// Serialize WebAssembly values to bytes
    fn serialize_values(&self, values: &[wrt_foundation::Value]) -> Result<Vec<u8>> {
        let mut result = Vec::new();

        for value in values {
            match value {
                &wrt_foundation::Value::I32(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::I64(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::F32(v) => {
                    result.extend_from_slice(&v.to_bits().to_le_bytes());
                },
                &wrt_foundation::Value::F64(v) => {
                    result.extend_from_slice(&v.to_bits().to_le_bytes());
                },
                wrt_foundation::Value::V128(v) => {
                    result.extend_from_slice(&v.bytes);
                },
                wrt_foundation::Value::FuncRef(opt_ref) => {
                    match opt_ref {
                        Some(func_ref) => {
                            result.extend_from_slice(&[1u8]); // Non-null marker
                            result.extend_from_slice(&func_ref.index.to_le_bytes());
                        },
                        None => {
                            result.extend_from_slice(&[0u8]); // Null marker
                        },
                    }
                },
                wrt_foundation::Value::ExternRef(opt_ref) => {
                    match opt_ref {
                        Some(extern_ref) => {
                            result.extend_from_slice(&[1u8]); // Non-null marker
                            result.extend_from_slice(&extern_ref.index.to_le_bytes());
                        },
                        None => {
                            result.extend_from_slice(&[0u8]); // Null marker
                        },
                    }
                },
                &wrt_foundation::Value::Ref(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                wrt_foundation::Value::I16x8(v) => {
                    result.extend_from_slice(&v.bytes);
                },
                wrt_foundation::Value::Own(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                wrt_foundation::Value::Borrow(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                wrt_foundation::Value::Stream(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                wrt_foundation::Value::Future(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                wrt_foundation::Value::Void => {
                    // Void has no data representation
                },
                wrt_foundation::Value::StructRef(opt_ref) => match opt_ref {
                    Some(_) => result.extend_from_slice(&[1u8]),
                    None => result.extend_from_slice(&[0u8]),
                },
                wrt_foundation::Value::ArrayRef(opt_ref) => match opt_ref {
                    Some(_) => result.extend_from_slice(&[1u8]),
                    None => result.extend_from_slice(&[0u8]),
                },
                wrt_foundation::Value::ExnRef(opt_ref) => match opt_ref {
                    Some(v) => {
                        result.extend_from_slice(&[1u8]);
                        result.extend_from_slice(&v.to_le_bytes());
                    },
                    None => result.extend_from_slice(&[0u8]),
                },
                &wrt_foundation::Value::Bool(v) => {
                    result.extend_from_slice(&[if v { 1u8 } else { 0u8 }]);
                },
                &wrt_foundation::Value::S8(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::U8(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::S16(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::U16(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::S32(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::U32(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::S64(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::U64(v) => {
                    result.extend_from_slice(&v.to_le_bytes());
                },
                &wrt_foundation::Value::Char(v) => {
                    result.extend_from_slice(&(v as u32).to_le_bytes());
                },
                wrt_foundation::Value::String(s) => {
                    let bytes = s.as_bytes();
                    result.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    result.extend_from_slice(bytes);
                },
                wrt_foundation::Value::I31Ref(opt_ref) => match opt_ref {
                    Some(v) => {
                        result.extend_from_slice(&[1u8]);
                        result.extend_from_slice(&v.to_le_bytes());
                    },
                    None => result.extend_from_slice(&[0u8]),
                },
                wrt_foundation::Value::List(_)
                | wrt_foundation::Value::Tuple(_)
                | wrt_foundation::Value::Record(_)
                | wrt_foundation::Value::Variant(_, _)
                | wrt_foundation::Value::Option(_)
                | wrt_foundation::Value::Result(_)
                | wrt_foundation::Value::Flags(_)
                | wrt_foundation::Value::Enum(_) => {
                    // Complex type - serialize length as 0 for now
                    result.extend_from_slice(&0u32.to_le_bytes());
                },
            }
        }

        Ok(result)
    }

    /// Get current execution state for debugging/monitoring
    pub fn get_execution_state(&self, task_id: TaskId) -> Result<ExecutionStateInfo> {
        let task = self
            .tasks
            .get(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

        Ok(ExecutionStateInfo {
            task_id,
            component_id: task.component_id,
            asil_mode: task.execution_context.asil_config.mode,
            stack_depth: task.execution_context.stack_depth,
            max_stack_depth: task.execution_context.max_stack_depth,
            fuel_consumed: task.execution_context.context_fuel_consumed.load(Ordering::Acquire),
            has_yield_point: task.execution_context.last_yield_point.is_some(),
            has_component_instance: task.execution_context.component_instance.is_some(),
            error_state: task.execution_context.error_state,
        })
    }

    /// Enable fuel debt/credit system with configuration
    pub fn enable_debt_credit_system(&mut self, config: Option<DebtCreditConfig>) -> Result<()> {
        let config = config.unwrap_or_default();

        // Determine debt policy based on configuration
        let debt_policy = if !config.enabled {
            DebtPolicy::NeverAllow
        } else if config.max_debt == 0 {
            DebtPolicy::Unlimited
        } else if config.credit_rate > 0 {
            DebtPolicy::Forgiveness {
                rate: config.credit_rate,
            }
        } else {
            DebtPolicy::Strict
        };

        // Use capped credit restriction by default
        let credit_restriction = CreditRestriction::Capped;

        let system = FuelDebtCreditSystem::new(debt_policy, credit_restriction)?;

        self.debt_credit_system = Some(system);
        Ok(())
    }

    /// Check if a task can incur debt
    pub fn can_incur_debt(&self, task_id: TaskId, amount: u64) -> bool {
        if let Some(system) = &self.debt_credit_system {
            if let Some(task) = self.tasks.get(&task_id) {
                // Get debt policy based on ASIL mode
                let policy = match task.execution_context.asil_config.mode {
                    ASILExecutionMode::D { .. } | ASILExecutionMode::AsilD => {
                        DebtPolicy::NeverAllow
                    },
                    ASILExecutionMode::C { .. } | ASILExecutionMode::AsilC => {
                        DebtPolicy::LimitedDebt { max_debt: 1000 }
                    },
                    ASILExecutionMode::B { .. } | ASILExecutionMode::AsilB => {
                        DebtPolicy::ModerateDebt {
                            max_debt: 5000,
                            interest_rate: 0.05,
                        }
                    },
                    ASILExecutionMode::A { .. } | ASILExecutionMode::AsilA => {
                        DebtPolicy::FlexibleDebt {
                            soft_limit: 10000,
                            hard_limit: 20000,
                            interest_rate: 0.10,
                        }
                    },
                    ASILExecutionMode::QM => DebtPolicy::FlexibleDebt {
                        soft_limit: 50000,
                        hard_limit: 100000,
                        interest_rate: 0.15,
                    },
                };

                system.can_incur_debt(task_id, amount, &policy)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Incur fuel debt for a task
    pub fn incur_fuel_debt(&mut self, task_id: TaskId, amount: u64) -> Result<()> {
        let system = self
            .debt_credit_system
            .as_mut()
            .ok_or_else(|| Error::validation_invalid_state("Debt/credit system not enabled"))?;

        let task = self
            .tasks
            .get(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

        // Get debt policy based on ASIL mode
        let policy = match task.execution_context.asil_config.mode {
            ASILExecutionMode::QM => DebtPolicy::FlexibleDebt {
                soft_limit: 10000,
                hard_limit: 20000,
                interest_rate: 0.10,
            },
            ASILExecutionMode::AsilA => DebtPolicy::FlexibleDebt {
                soft_limit: 10000,
                hard_limit: 20000,
                interest_rate: 0.10,
            },
            ASILExecutionMode::AsilB => DebtPolicy::ModerateDebt {
                max_debt: 5000,
                interest_rate: 0.05,
            },
            ASILExecutionMode::AsilC => DebtPolicy::LimitedDebt { max_debt: 1000 },
            ASILExecutionMode::AsilD => DebtPolicy::NeverAllow,
            ASILExecutionMode::D { .. } => DebtPolicy::NeverAllow,
            ASILExecutionMode::C { .. } => DebtPolicy::LimitedDebt { max_debt: 1000 },
            ASILExecutionMode::B { .. } => DebtPolicy::ModerateDebt {
                max_debt: 5000,
                interest_rate: 0.05,
            },
            ASILExecutionMode::A { .. } => DebtPolicy::FlexibleDebt {
                soft_limit: 10000,
                hard_limit: 20000,
                interest_rate: 0.10,
            },
        };

        system.incur_debt(task_id, amount, &policy)
    }

    /// Grant fuel credit to a component
    pub fn grant_fuel_credit(
        &mut self,
        component_id: ComponentInstanceId,
        amount: u64,
        restrictions: Option<CreditRestriction>,
    ) -> Result<()> {
        let system = self
            .debt_credit_system
            .as_mut()
            .ok_or_else(|| Error::validation_invalid_state("Debt/credit system not enabled"))?;

        let restrictions = restrictions.unwrap_or(CreditRestriction::None);
        system.grant_credit(component_id, amount, restrictions)
    }

    /// Check debt/credit balance for a task
    pub fn get_debt_credit_balance(&self, task_id: TaskId) -> DebtCreditBalance {
        if let Some(system) = &self.debt_credit_system {
            if let Some(task) = self.tasks.get(&task_id) {
                let debt = system.get_task_debt(task_id).unwrap_or(0);
                let credit = system.get_component_credit(task.component_id);
                DebtCreditBalance {
                    task_id,
                    component_id: task.component_id.into(),
                    current_debt: debt,
                    available_credit: credit,
                    net_balance: credit as i64 - debt as i64,
                }
            } else {
                DebtCreditBalance::default_for_task(task_id)
            }
        } else {
            DebtCreditBalance::default_for_task(task_id)
        }
    }

    /// Allocate additional fuel to a task and handle debt repayment
    pub fn allocate_additional_fuel(
        &mut self,
        task_id: TaskId,
        additional_fuel: u64,
    ) -> Result<u64> {
        let task = self
            .tasks
            .get_mut(&task_id)
            .ok_or_else(|| Error::validation_invalid_input("Task not found"))?;

        let mut fuel_to_allocate = additional_fuel;

        // First, check if task has debt
        if let Some(system) = &mut self.debt_credit_system {
            let current_debt = system.get_task_debt(task_id)?;

            if current_debt > 0 {
                // Calculate how much fuel goes to debt repayment
                let debt_payment = fuel_to_allocate.min(current_debt);

                // Repay debt with interest
                let interest_rate = match task.execution_context.asil_config.mode {
                    ASILExecutionMode::QM => 0.10,       // 10% interest for QM
                    ASILExecutionMode::AsilA => 0.10,    // 10% interest for ASIL-A
                    ASILExecutionMode::AsilB => 0.05,    // 5% interest for ASIL-B
                    ASILExecutionMode::AsilC => 0.02,    // 2% interest for ASIL-C
                    ASILExecutionMode::AsilD => 0.0, // No interest for ASIL-D (shouldn't have debt)
                    ASILExecutionMode::D { .. } => 0.0, // No interest for ASIL-D (shouldn't have debt)
                    ASILExecutionMode::C { .. } => 0.02, // 2% interest for ASIL-C
                    ASILExecutionMode::B { .. } => 0.05, // 5% interest for ASIL-B
                    ASILExecutionMode::A { .. } => 0.10, // 10% interest for ASIL-A
                };

                system.repay_debt(task_id, debt_payment, interest_rate)?;

                // Reduce fuel available for allocation
                fuel_to_allocate = fuel_to_allocate.saturating_sub(debt_payment);
            }
        }

        // Allocate remaining fuel to task budget
        task.fuel_budget = task.fuel_budget.saturating_add(fuel_to_allocate);

        Ok(fuel_to_allocate)
    }

    /// Get debt/credit system statistics
    pub fn get_debt_credit_stats(&self) -> Option<DebtCreditStats> {
        self.debt_credit_system.as_ref().map(|system| DebtCreditStats {
            current_debt: 0,        // TODO: Track current debt properly
            total_credit_earned: 0, // TODO: Track total credit earned
            debt_violations: 0,     // TODO: Track debt violations
        })
    }
}

impl Default for FuelAsyncExecutor {
    fn default() -> Self {
        Self::new().expect("Failed to create default FuelAsyncExecutor")
    }
}

/// Status information for an async task
#[derive(Debug, Clone)]
pub struct AsyncTaskStatus {
    pub id: TaskId,
    pub component_id: ComponentInstanceId,
    pub state: AsyncTaskState,
    pub fuel_budget: u64,
    pub fuel_consumed: u64,
    pub priority: Priority,
    pub verification_level: VerificationLevel,
}

/// Global fuel status for the async executor
#[derive(Debug, Clone)]
pub struct GlobalAsyncFuelStatus {
    pub limit: u64,
    pub consumed: u64,
    pub enforcement_enabled: bool,
    pub active_tasks: usize,
    pub ready_tasks: usize,
}

impl GlobalAsyncFuelStatus {
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.consumed)
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.limit == 0 {
            0.0
        } else {
            (self.consumed as f64 / self.limit as f64) * 100.0
        }
    }
}

/// Polling statistics for monitoring executor performance
#[derive(Debug, Default, Clone)]
pub struct PollingStatistics {
    pub total_polls: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_yielded: u64,
    pub wakes_coalesced: usize,
}

/// Active fuel monitoring for real-time fuel consumption tracking
#[derive(Debug)]
pub struct FuelMonitor {
    /// Current fuel consumption rate (fuel per ms)
    current_rate: AtomicU64,
    /// Peak fuel consumption rate observed
    peak_rate: AtomicU64,
    /// Total fuel consumed in current monitoring window
    window_fuel_consumed: AtomicU64,
    /// Monitoring window start time (in fuel units for determinism)
    window_start: AtomicU64,
    /// ASIL-specific fuel thresholds
    asil_thresholds: ASILFuelThresholds,
    /// Fuel consumption history for trend analysis
    consumption_history: Mutex<BoundedVec<FuelConsumptionRecord, 128>>,
    /// Active alerts for fuel issues
    active_alerts: Mutex<BoundedVec<FuelAlert, 32>>,
}

/// ASIL-specific fuel thresholds
#[derive(Debug, Clone)]
pub struct ASILFuelThresholds {
    /// ASIL-D: Strict deterministic fuel limit per task
    asil_d_task_limit: u64,
    /// ASIL-C: Isolated fuel budget per component
    asil_c_component_limit: u64,
    /// ASIL-B: Resource-limited fuel per time slice
    asil_b_slice_limit: u64,
    /// ASIL-A: Basic fuel warning threshold
    asil_a_warning_threshold: u64,
}

/// ASIL fuel enforcement policy
#[derive(Debug, Clone)]
pub struct ASILFuelEnforcementPolicy {
    /// Enable strict enforcement (fail fast)
    pub strict_enforcement: bool,
    /// Enable fuel borrowing between tasks
    pub allow_fuel_borrowing: bool,
    /// Enable emergency fuel reserves
    pub emergency_reserves_enabled: bool,
    /// Emergency reserve fuel amount
    pub emergency_reserve_amount: u64,
    /// ASIL-specific policies
    pub asil_policies: ASILSpecificPolicies,
}

/// ASIL-specific enforcement policies
#[derive(Debug, Clone)]
pub struct ASILSpecificPolicies {
    /// ASIL-D: Deterministic fuel allocation
    pub asil_d: ASILDPolicy,
    /// ASIL-C: Isolated fuel pools
    pub asil_c: ASILCPolicy,
    /// ASIL-B: Bounded fuel consumption
    pub asil_b: ASILBPolicy,
    /// ASIL-A: Flexible fuel management
    pub asil_a: ASILAPolicy,
}

/// ASIL-D specific fuel policy
#[derive(Debug, Clone)]
pub struct ASILDPolicy {
    /// Fuel quantum for deterministic allocation
    pub fuel_quantum: u64,
    /// Maximum fuel per execution step
    pub max_step_fuel: u64,
    /// Enforce deterministic fuel ordering
    pub enforce_deterministic_ordering: bool,
    /// Preallocation required
    pub require_preallocation: bool,
}

/// ASIL-C specific fuel policy
#[derive(Debug, Clone)]
pub struct ASILCPolicy {
    /// Component fuel isolation enabled
    pub component_isolation: bool,
    /// Maximum inter-component fuel transfer
    pub max_transfer_amount: u64,
    /// Temporal fuel windows
    pub temporal_window_ms: u64,
}

/// ASIL-B specific fuel policy
#[derive(Debug, Clone)]
pub struct ASILBPolicy {
    /// Fuel budget per time slice
    pub slice_fuel_budget: u64,
    /// Allow fuel rollover between slices
    pub allow_rollover: bool,
    /// Maximum rollover percentage
    pub max_rollover_percent: u32,
}

/// ASIL-A specific fuel policy
#[derive(Debug, Clone)]
pub struct ASILAPolicy {
    /// Soft limit before warnings
    pub soft_limit: u64,
    /// Hard limit before failure
    pub hard_limit: u64,
    /// Grace period for exceeding soft limit
    pub grace_period_ms: u64,
}

/// Fuel consumption record for history tracking
#[derive(Debug, Clone)]
pub struct FuelConsumptionRecord {
    /// Timestamp (in fuel units for determinism)
    timestamp: u64,
    /// Fuel consumed in this period
    fuel_consumed: u64,
    /// Number of tasks active
    active_tasks: u32,
    /// ASIL mode of highest priority task
    highest_asil_mode: ASILExecutionMode,
}

/// Fuel alert for monitoring
#[derive(Debug, Clone)]
pub enum FuelAlert {
    /// Task approaching fuel limit
    TaskApproachingLimit {
        task_id: TaskId,
        remaining_fuel: u64,
    },
    /// Component exceeding budget
    ComponentExceedingBudget {
        component_id: ComponentInstanceId,
        overage: u64,
    },
    /// Global fuel consumption spike
    ConsumptionSpike { rate: u64, threshold: u64 },
    /// ASIL violation detected
    ASILViolation {
        mode: ASILExecutionMode,
        violation_type: String,
    },
}

/// Fuel enforcement decision
#[derive(Debug, Clone)]
pub enum FuelEnforcementDecision {
    /// Allow fuel consumption
    Allow,
    /// Deny fuel consumption
    Deny { reason: String },
    /// Allow with warning
    AllowWithWarning { warning: String },
    /// Allow with fuel transfer from another component
    AllowWithTransfer {
        transfer_amount: u64,
        source_component: Option<ComponentInstanceId>,
    },
    /// Allow with rollover from previous time slice
    AllowWithRollover { rollover_amount: u64 },
    /// Require task to yield before continuing
    RequireYield { reason: String },
    /// Allow with debt (must be repaid)
    AllowWithDebt { debt_amount: u64 },
}
