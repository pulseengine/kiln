use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    time::Duration,
};
#[cfg(feature = "std")]
use std::thread;

use wrt_foundation::{
    BoundedVec, bounded_collections::BoundedMap, component_value::ComponentValue,
};
use wrt_platform::{
    advanced_sync::{Priority, PriorityInheritanceMutex},
    sync::{FutexLike, SpinFutex},
};

use crate::{
    ComponentInstanceId, ResourceHandle,
    canonical_abi::canonical_options::CanonicalOptions,
    execution_engine::{TimeBoundedConfig, TimeBoundedContext, TimeBoundedOutcome},
    post_return::{CleanupTask, CleanupTaskType, PostReturnRegistry},
    threading::{
        task_manager::{TaskId, TaskManager, TaskState},
        thread_spawn::{
            ComponentThreadManager, ThreadConfiguration, ThreadHandle, ThreadId, ThreadResult,
            ThreadSpawnError, ThreadSpawnErrorKind, ThreadSpawnRequest, ThreadSpawnResult,
        },
    },
    types::ValType,
};

const MAX_FUEL_PER_THREAD: u64 = 1_000_000;
const FUEL_CHECK_INTERVAL: u64 = 1000;
const FUEL_PER_MS: u64 = 100;

/// Thread execution context with fuel tracking
#[derive(Debug)]
pub struct FuelTrackedThreadContext {
    pub thread_id: ThreadId,
    pub component_id: ComponentInstanceId,
    pub initial_fuel: u64,
    pub remaining_fuel: AtomicU64,
    pub consumed_fuel: AtomicU64,
    pub fuel_exhausted: AtomicBool,
    pub check_interval: u64,
    pub last_check: AtomicU64,
}

impl FuelTrackedThreadContext {
    pub fn new(thread_id: ThreadId, component_id: ComponentInstanceId, initial_fuel: u64) -> Self {
        Self {
            thread_id,
            component_id,
            initial_fuel,
            remaining_fuel: AtomicU64::new(initial_fuel),
            consumed_fuel: AtomicU64::new(0),
            fuel_exhausted: AtomicBool::new(false),
            check_interval: FUEL_CHECK_INTERVAL,
            last_check: AtomicU64::new(0),
        }
    }

    pub fn consume_fuel(&self, amount: u64) -> core::result::Result<(), ThreadSpawnError> {
        let current_fuel = self.remaining_fuel.load(Ordering::Acquire);

        if current_fuel < amount {
            self.fuel_exhausted.store(true, Ordering::Release);
            return Err(ThreadSpawnError {
                kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                message: "Thread fuel exhausted",
            });
        }

        self.remaining_fuel.fetch_sub(amount, Ordering::AcqRel);
        self.consumed_fuel.fetch_add(amount, Ordering::AcqRel);

        // Check if we should perform a fuel check
        let consumed = self.consumed_fuel.load(Ordering::Acquire);
        let last_check = self.last_check.load(Ordering::Acquire);

        if consumed - last_check >= self.check_interval {
            self.last_check.store(consumed, Ordering::Release);
            self.check_fuel_status()?;
        }

        Ok(())
    }

    /// Consume fuel for specific WebAssembly instruction types
    /// This integrates with the instruction-level fuel system
    pub fn consume_instruction_fuel(
        &self,
        instruction_type: wrt_runtime::stackless::engine::InstructionFuelType,
        verification_level: wrt_foundation::verification::VerificationLevel,
    ) -> core::result::Result<(), ThreadSpawnError> {
        // Map instruction type to operation type for fuel calculation
        let op_type = match instruction_type {
            wrt_runtime::stackless::engine::InstructionFuelType::SimpleConstant => {
                wrt_foundation::operations::Type::WasmSimpleConstant
            },
            wrt_runtime::stackless::engine::InstructionFuelType::LocalAccess => {
                wrt_foundation::operations::Type::WasmLocalAccess
            },
            wrt_runtime::stackless::engine::InstructionFuelType::GlobalAccess => {
                wrt_foundation::operations::Type::WasmGlobalAccess
            },
            wrt_runtime::stackless::engine::InstructionFuelType::SimpleArithmetic => {
                wrt_foundation::operations::Type::WasmSimpleArithmetic
            },
            wrt_runtime::stackless::engine::InstructionFuelType::ComplexArithmetic => {
                wrt_foundation::operations::Type::WasmComplexArithmetic
            },
            wrt_runtime::stackless::engine::InstructionFuelType::FloatArithmetic => {
                wrt_foundation::operations::Type::WasmFloatArithmetic
            },
            wrt_runtime::stackless::engine::InstructionFuelType::Comparison => {
                wrt_foundation::operations::Type::WasmComparison
            },
            wrt_runtime::stackless::engine::InstructionFuelType::SimpleControl => {
                wrt_foundation::operations::Type::WasmSimpleControl
            },
            wrt_runtime::stackless::engine::InstructionFuelType::ComplexControl => {
                wrt_foundation::operations::Type::WasmComplexControl
            },
            wrt_runtime::stackless::engine::InstructionFuelType::FunctionCall => {
                wrt_foundation::operations::Type::WasmFunctionCall
            },
            wrt_runtime::stackless::engine::InstructionFuelType::MemoryLoad => {
                wrt_foundation::operations::Type::WasmMemoryLoad
            },
            wrt_runtime::stackless::engine::InstructionFuelType::MemoryStore => {
                wrt_foundation::operations::Type::WasmMemoryStore
            },
            wrt_runtime::stackless::engine::InstructionFuelType::MemoryManagement => {
                wrt_foundation::operations::Type::WasmMemoryManagement
            },
            wrt_runtime::stackless::engine::InstructionFuelType::TableAccess => {
                wrt_foundation::operations::Type::WasmTableAccess
            },
            wrt_runtime::stackless::engine::InstructionFuelType::TypeConversion => {
                wrt_foundation::operations::Type::WasmTypeConversion
            },
            wrt_runtime::stackless::engine::InstructionFuelType::SimdOperation => {
                wrt_foundation::operations::Type::WasmSimdOperation
            },
            wrt_runtime::stackless::engine::InstructionFuelType::AtomicOperation => {
                wrt_foundation::operations::Type::WasmAtomicOperation
            },
        };

        // Calculate fuel cost with verification level
        let fuel_cost =
            wrt_foundation::operations::Type::fuel_cost_for_operation(op_type, verification_level)
                .map_err(|_| ThreadSpawnError {
                    kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                    message: "Fuel calculation error",
                })?;

        // Record the operation for tracking
        wrt_foundation::operations::record_global_operation(op_type, verification_level);

        // Consume the calculated fuel
        self.consume_fuel(fuel_cost)
    }

    pub fn check_fuel_status(&self) -> core::result::Result<(), ThreadSpawnError> {
        if self.fuel_exhausted.load(Ordering::Acquire) {
            return Err(ThreadSpawnError {
                kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                message: "Component not found",
            });
        }

        let remaining = self.remaining_fuel.load(Ordering::Acquire);
        if remaining == 0 {
            self.fuel_exhausted.store(true, Ordering::Release);
            return Err(ThreadSpawnError {
                kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                message: "Component not found",
            });
        }

        Ok(())
    }

    pub fn add_fuel(&self, amount: u64) -> u64 {
        self.remaining_fuel.fetch_add(amount, Ordering::AcqRel)
    }

    pub fn get_remaining_fuel(&self) -> u64 {
        self.remaining_fuel.load(Ordering::Acquire)
    }

    pub fn get_consumed_fuel(&self) -> u64 {
        self.consumed_fuel.load(Ordering::Acquire)
    }
}

/// Extended thread configuration with fuel settings
#[derive(Debug, Clone)]
pub struct FuelThreadConfiguration {
    pub base_config: ThreadConfiguration,
    pub initial_fuel: Option<u64>,
    pub fuel_per_ms: u64,
    pub allow_fuel_extension: bool,
    pub fuel_check_interval: u64,
}

impl Default for FuelThreadConfiguration {
    fn default() -> Self {
        Self {
            base_config: ThreadConfiguration::default(),
            initial_fuel: Some(MAX_FUEL_PER_THREAD),
            fuel_per_ms: FUEL_PER_MS,
            allow_fuel_extension: false,
            fuel_check_interval: FUEL_CHECK_INTERVAL,
        }
    }
}

/// Thread manager with integrated fuel tracking
pub struct FuelTrackedThreadManager {
    base_manager: ComponentThreadManager,
    thread_contexts: BoundedMap<ThreadId, FuelTrackedThreadContext, 512>,
    time_bounds: BoundedMap<ThreadId, TimeBoundedContext, 512>,
    global_fuel_limit: AtomicU64,
    global_fuel_consumed: AtomicU64,
    fuel_enforcement: AtomicBool,
}

impl FuelTrackedThreadManager {
    pub fn new() -> ThreadSpawnResult<Self> {
        let provider =
            wrt_foundation::safe_managed_alloc!(65536, wrt_foundation::CrateId::Component)
                .map_err(|_| wrt_error::Error::resource_exhausted("Memory allocation failed"))?;

        Ok(Self {
            base_manager: ComponentThreadManager::new()?,
            thread_contexts: BoundedMap::new(),
            time_bounds: BoundedMap::new(),
            global_fuel_limit: AtomicU64::new(u64::MAX),
            global_fuel_consumed: AtomicU64::new(0),
            fuel_enforcement: AtomicBool::new(true),
        })
    }

    pub fn set_global_fuel_limit(&self, limit: u64) {
        self.global_fuel_limit.store(limit, Ordering::SeqCst);
    }

    pub fn set_fuel_enforcement(&self, enforce: bool) {
        self.fuel_enforcement.store(enforce, Ordering::SeqCst);
    }

    pub fn spawn_thread_with_fuel(
        &mut self,
        request: ThreadSpawnRequest,
        fuel_config: FuelThreadConfiguration,
    ) -> ThreadSpawnResult<ThreadHandle> {
        // Check global fuel availability
        if self.fuel_enforcement.load(Ordering::Acquire) {
            let initial_fuel = fuel_config.initial_fuel.unwrap_or(MAX_FUEL_PER_THREAD);
            let global_consumed = self.global_fuel_consumed.load(Ordering::Acquire);
            let global_limit = self.global_fuel_limit.load(Ordering::Acquire);

            if global_consumed + initial_fuel > global_limit {
                return Err(ThreadSpawnError {
                    kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                    message: "Global fuel limit would be exceeded".to_owned(),
                });
            }
        }

        // Create time-bounded config
        let time_config = TimeBoundedConfig {
            time_limit_ms: fuel_config.base_config.stack_size.checked_div(fuel_config.fuel_per_ms),
            allow_extension: fuel_config.allow_fuel_extension,
            fuel_limit: fuel_config.initial_fuel,
        };

        // Spawn the thread
        let handle = self.base_manager.spawn_thread(request.clone())?;

        // Create fuel context
        let fuel_context = FuelTrackedThreadContext::new(
            handle.thread_id,
            request.component_id,
            fuel_config.initial_fuel.unwrap_or(MAX_FUEL_PER_THREAD),
        );

        // Create time-bounded context
        let time_context = TimeBoundedContext::new(time_config);

        // Store contexts
        self.thread_contexts.insert(handle.thread_id, fuel_context).map_err(|_| {
            ThreadSpawnError {
                kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                message: "Too many thread contexts".to_owned(),
            }
        })?;

        self.time_bounds
            .insert(handle.thread_id, time_context)
            .map_err(|_| ThreadSpawnError {
                kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                message: "Too many time bound contexts".to_owned(),
            })?;

        // Update global fuel consumed
        if self.fuel_enforcement.load(Ordering::Acquire) {
            let initial_fuel = fuel_config.initial_fuel.unwrap_or(MAX_FUEL_PER_THREAD);
            self.global_fuel_consumed.fetch_add(initial_fuel, Ordering::AcqRel);
        }

        Ok(handle)
    }

    pub fn consume_thread_fuel(&self, thread_id: ThreadId, amount: u64) -> ThreadSpawnResult<()> {
        if !self.fuel_enforcement.load(Ordering::Acquire) {
            return Ok();
        }

        let context = self.thread_contexts.get(&thread_id).ok_or_else(|| ThreadSpawnError {
            kind: ThreadSpawnErrorKind::ThreadNotFound,
            message: "Component not found",
        })?;

        context.consume_fuel(amount)?;

        // Also check time bounds
        if let Some(time_context) = self.time_bounds.get(&thread_id) {
            time_context.check_time_bounds().map_err(|e| ThreadSpawnError {
                kind: ThreadSpawnErrorKind::ResourceLimitExceeded,
                message: "Component not found",
            })?;
        }

        Ok(())
    }

    pub fn add_thread_fuel(&mut self, thread_id: ThreadId, amount: u64) -> ThreadSpawnResult<u64> {
        let context = self.thread_contexts.get(&thread_id).ok_or_else(|| ThreadSpawnError {
            kind: ThreadSpawnErrorKind::ThreadNotFound,
            message: "Component not found",
        })?;

        let new_fuel = context.add_fuel(amount);
        Ok(new_fuel)
    }

    pub fn get_thread_fuel_status(
        &self,
        thread_id: ThreadId,
    ) -> ThreadSpawnResult<ThreadFuelStatus> {
        let context = self.thread_contexts.get(&thread_id).ok_or_else(|| ThreadSpawnError {
            kind: ThreadSpawnErrorKind::ThreadNotFound,
            message: "Component not found",
        })?;

        Ok(ThreadFuelStatus {
            thread_id,
            initial_fuel: context.initial_fuel,
            remaining_fuel: context.get_remaining_fuel(),
            consumed_fuel: context.get_consumed_fuel(),
            fuel_exhausted: context.fuel_exhausted.load(Ordering::Acquire),
        })
    }

    pub fn join_thread_with_fuel(
        &mut self,
        thread_id: ThreadId,
    ) -> ThreadSpawnResult<FuelTrackedThreadResult> {
        let result = self.base_manager.join_thread(thread_id)?;

        let fuel_status = self.get_thread_fuel_status(thread_id).ok();

        // Clean up contexts
        self.thread_contexts.remove(&thread_id);
        self.time_bounds.remove(&thread_id);

        // Update global fuel consumed (return unused fuel)
        if let Some(status) = fuel_status {
            if self.fuel_enforcement.load(Ordering::Acquire) {
                self.global_fuel_consumed.fetch_sub(status.remaining_fuel, Ordering::AcqRel);
            }
        }

        Ok(FuelTrackedThreadResult {
            result,
            fuel_status,
        })
    }

    pub fn get_global_fuel_status(&self) -> GlobalFuelStatus {
        GlobalFuelStatus {
            limit: self.global_fuel_limit.load(Ordering::Acquire),
            consumed: self.global_fuel_consumed.load(Ordering::Acquire),
            enforcement_enabled: self.fuel_enforcement.load(Ordering::Acquire),
        }
    }

    pub fn execute_with_fuel_tracking<F, R>(
        &self,
        thread_id: ThreadId,
        fuel_per_operation: u64,
        operation: F,
    ) -> ThreadSpawnResult<R>
    where
        F: FnOnce() -> R,
    {
        // Consume fuel before operation
        self.consume_thread_fuel(thread_id, fuel_per_operation)?;

        // Execute the operation
        let result = operation();

        Ok(result)
    }
}

impl Default for FuelTrackedThreadManager {
    fn default() -> Self {
        // Just use new() and panic if it fails, since this is Default trait
        Self::new().expect("Failed to create default FuelTrackedThreadManager")
    }
}

#[derive(Debug, Clone)]
pub struct ThreadFuelStatus {
    pub thread_id: ThreadId,
    pub initial_fuel: u64,
    pub remaining_fuel: u64,
    pub consumed_fuel: u64,
    pub fuel_exhausted: bool,
}

#[derive(Debug, Clone)]
pub struct FuelTrackedThreadResult {
    pub result: ThreadResult,
    pub fuel_status: Option<ThreadFuelStatus>,
}

#[derive(Debug, Clone)]
pub struct GlobalFuelStatus {
    pub limit: u64,
    pub consumed: u64,
    pub enforcement_enabled: bool,
}

impl GlobalFuelStatus {
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

/// Helper functions for creating fuel-aware thread configurations
pub fn create_fuel_thread_config(initial_fuel: u64) -> FuelThreadConfiguration {
    FuelThreadConfiguration {
        base_config: ThreadConfiguration::default(),
        initial_fuel: Some(initial_fuel),
        fuel_per_ms: FUEL_PER_MS,
        allow_fuel_extension: false,
        fuel_check_interval: FUEL_CHECK_INTERVAL,
    }
}

pub fn create_unlimited_fuel_thread_config() -> FuelThreadConfiguration {
    FuelThreadConfiguration {
        base_config: ThreadConfiguration::default(),
        initial_fuel: None,
        fuel_per_ms: 0,
        allow_fuel_extension: true,
        fuel_check_interval: u64::MAX,
    }
}

/// Integration with component execution
pub trait FuelAwareExecution {
    fn execute_with_fuel<F, R>(&self, fuel: u64, f: F) -> core::result::Result<R, ThreadSpawnError>
    where
        F: FnOnce() -> R;

    fn check_fuel_before_operation(
        &self,
        required_fuel: u64,
    ) -> core::result::Result<(), ThreadSpawnError>;
}
