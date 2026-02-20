//! Safe threading built-ins using the new platform-aware architecture.
//!
//! This module provides WebAssembly threading built-ins that leverage the
//! platform-specific thread pools and safety mechanisms from wrt-platform.

use std::{boxed::Box, string::ToString, sync::Arc, vec::Vec};

use wrt_error::{Error, Result, kinds::ThreadingError};
#[cfg(feature = "std")]
use wrt_foundation::{builtin::BuiltinType, component_value::ComponentValue};
use wrt_platform::threading::{ThreadPoolConfig, ThreadPriority, ThreadingLimits};

#[cfg(feature = "threading")]
use wrt_platform::wasm_thread_manager::{WasmModuleInfo, WasmThreadManager};

use super::BuiltinHandler;

/// Safe threading spawn handler using platform thread manager
#[derive(Clone)]
pub struct SafeThreadingSpawnHandler {
    /// Platform-aware thread manager
    thread_manager: Arc<WasmThreadManager>,
    /// Module ID for this component
    module_id: u64,
}

impl SafeThreadingSpawnHandler {
    /// Create new safe threading spawn handler
    pub fn new(thread_manager: Arc<WasmThreadManager>, module_id: u64) -> Self {
        Self {
            thread_manager,
            module_id,
        }
    }
}

impl BuiltinHandler for SafeThreadingSpawnHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ThreadingSpawn
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.is_empty() {
            return Err(Error::runtime_execution_error("Error occurred".to_owned()));
        }

        // Extract function ID
        let function_id = match args[0] {
            ComponentValue::U32(id) => id,
            _ => {
                return Err(Error::component_thread_spawn_failed(
                    "Invalid function ID type",
                ));
            },
        };

        // Extract function arguments
        let function_args = args[1..].to_vec();

        // Extract optional priority (if provided as second argument)
        let priority = if args.len() > 1 {
            match &args[1] {
                ComponentValue::U32(p) => match p {
                    0 => Some(ThreadPriority::Idle),
                    1 => Some(ThreadPriority::Low),
                    2 => Some(ThreadPriority::Normal),
                    3 => Some(ThreadPriority::High),
                    4 => Some(ThreadPriority::Realtime),
                    _ => Some(ThreadPriority::Normal),
                },
                _ => None,
            }
        } else {
            None
        };

        // Create spawn request
        let request = wrt_platform::threading::ThreadSpawnRequest {
            module_id: self.module_id,
            function_id,
            args: function_args,
            priority,
            stack_size: None, // Use defaults
        };

        // Spawn thread with safety checks
        match self.thread_manager.spawn_thread(request) {
            Ok(thread_id) => Ok(vec![ComponentValue::U64(thread_id)]),
            Err(e) => Err(Error::component_thread_spawn_failed("Thread spawn failed")),
        }
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Safe threading join handler
#[derive(Clone)]
pub struct SafeThreadingJoinHandler {
    /// Platform-aware thread manager
    thread_manager: Arc<WasmThreadManager>,
}

impl SafeThreadingJoinHandler {
    /// Create new safe threading join handler
    pub fn new(thread_manager: Arc<WasmThreadManager>) -> Self {
        Self { thread_manager }
    }
}

impl BuiltinHandler for SafeThreadingJoinHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ThreadingJoin
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.len() != 1 {
            return Err(Error::runtime_execution_error("Error occurred".to_owned()));
        }

        // Extract thread ID
        let thread_id = match args[0] {
            ComponentValue::U64(id) => id,
            _ => {
                return Err(Error::component_thread_spawn_failed(
                    "Invalid thread ID type",
                ));
            },
        };

        // Join the thread
        match self.thread_manager.join_thread(thread_id) {
            Ok(result) => match result {
                wrt_platform::wasm_thread_manager::ThreadExecutionResult::Success(values) => {
                    Ok(values)
                },
                wrt_platform::wasm_thread_manager::ThreadExecutionResult::Error(msg) => {
                    Err(Error::component_thread_spawn_failed(&msg))
                },
                wrt_platform::wasm_thread_manager::ThreadExecutionResult::Cancelled => {
                    Err(Error::threading_error("Error occurred"))
                },
                wrt_platform::wasm_thread_manager::ThreadExecutionResult::Timeout => {
                    Err(Error::threading_error("Error occurred"))
                },
            },
            Err(e) => Err(Error::component_thread_spawn_failed("Thread join failed")),
        }
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Safe threading status handler
#[derive(Clone)]
pub struct SafeThreadingStatusHandler {
    /// Platform-aware thread manager
    thread_manager: Arc<WasmThreadManager>,
}

impl SafeThreadingStatusHandler {
    /// Create new safe threading status handler
    pub fn new(thread_manager: Arc<WasmThreadManager>) -> Self {
        Self { thread_manager }
    }
}

impl BuiltinHandler for SafeThreadingStatusHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ThreadingSync // Reuse sync type for status
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.is_empty() {
            return Err(Error::runtime_execution_error("Error occurred".to_owned()));
        }

        // Extract operation type
        let op_type = match &args[0] {
            ComponentValue::String(s) => s.as_str(),
            _ => {
                return Err(Error::component_thread_spawn_failed(
                    "Invalid operation type",
                ));
            },
        };

        match op_type {
            "is-running" => {
                if args.len() != 2 {
                    return Err(Error::runtime_execution_error("Error occurred".to_owned()));
                }

                let thread_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::component_thread_spawn_failed(
                            "Invalid thread ID type",
                        ));
                    },
                };

                match self.thread_manager.is_thread_running(thread_id) {
                    Ok(running) => Ok(vec![ComponentValue::U32(if running { 1 } else { 0 })]),
                    Err(e) => Err(Error::runtime_execution_error("Error occurred")),
                }
            },
            "cancel" => {
                if args.len() != 2 {
                    return Err(Error::runtime_execution_error("Error occurred".to_owned()));
                }

                let thread_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::component_thread_spawn_failed(
                            "Invalid thread ID type",
                        ));
                    },
                };

                match self.thread_manager.cancel_thread(thread_id) {
                    Ok(()) => Ok(vec![ComponentValue::U32(1)]), // Success
                    Err(e) => Err(Error::component_not_found("Error occurred")),
                }
            },
            "health-check" => {
                // Perform health check on all threads
                match self.thread_manager.health_check() {
                    Ok(results) => {
                        let mut response = vec![ComponentValue::U32(results.len() as u32)];
                        for (thread_id, health) in results {
                            response.push(ComponentValue::U64(thread_id));
                            let health_code = match health {
                                wrt_platform::threading::ThreadHealth::Healthy => 0,
                                wrt_platform::threading::ThreadHealth::CpuQuotaExceeded => 1,
                                wrt_platform::threading::ThreadHealth::LifetimeExceeded => 2,
                                wrt_platform::threading::ThreadHealth::Deadlocked => 3,
                                wrt_platform::threading::ThreadHealth::Unresponsive => 4,
                            };
                            response.push(ComponentValue::U32(health_code));
                        }
                        Ok(response)
                    },
                    Err(e) => Err(Error::runtime_execution_error("Error occurred")),
                }
            },
            "active-count" => match self.thread_manager.active_thread_count() {
                Ok(count) => Ok(vec![ComponentValue::U32(count as u32)]),
                Err(e) => Err(Error::runtime_execution_error("Error occurred")),
            },
            "stats" => {
                let stats = self.thread_manager.get_stats();
                Ok(vec![
                    ComponentValue::U32(stats.total_threads as u32),
                    ComponentValue::U64(stats.pool_stats.total_spawned),
                    ComponentValue::U64(stats.pool_stats.total_completed),
                    ComponentValue::U64(stats.pool_stats.total_failed),
                    ComponentValue::U32(stats.modules_registered as u32),
                ])
            },
            _ => Err(Error::runtime_execution_error("Unknown operation type")),
        }
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Create safe threading handlers with platform awareness
#[cfg(feature = "std")]
pub fn create_safe_threading_handlers(
    executor: Arc<dyn Fn(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + Sync>,
    module_info: WasmModuleInfo,
) -> Result<(Arc<WasmThreadManager>, Vec<Box<dyn BuiltinHandler>>)> {
    // Create thread pool configuration based on module requirements
    let config = ThreadPoolConfig {
        max_threads: module_info.max_threads,
        priority_range: (ThreadPriority::Low, ThreadPriority::High),
        memory_limit_per_thread: Some(module_info.memory_limit / module_info.max_threads.max(1)),
        stack_size: 2 * 1024 * 1024, // 2MB default
        max_thread_lifetime: Some(module_info.cpu_quota),
        name_prefix: "wasm-safe",
        ..Default::default()
    };

    // Create threading limits
    let limits = ThreadingLimits {
        max_threads_per_module: module_info.max_threads,
        max_total_threads: module_info.max_threads * 4, // Allow some headroom
        max_thread_lifetime: module_info.cpu_quota,
        cpu_quota_per_thread: module_info.cpu_quota,
        memory_limit_per_module: module_info.memory_limit,
    };

    // Create thread manager
    let thread_manager = Arc::new(WasmThreadManager::new(config, limits, executor)?);

    // Register the module
    thread_manager.register_module(module_info.clone())?;

    // Create handlers
    let handlers: Vec<Box<dyn BuiltinHandler>> = vec![
        Box::new(SafeThreadingSpawnHandler::new(
            thread_manager.clone(),
            module_info.id,
        )),
        Box::new(SafeThreadingJoinHandler::new(thread_manager.clone())),
        Box::new(SafeThreadingStatusHandler::new(thread_manager.clone())),
    ];

    Ok((thread_manager, handlers))
}

/// Create no-op handlers for no_std environments
#[cfg(not(feature = "std"))]
pub fn create_safe_threading_handlers(
    _executor: Arc<dyn Fn(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + Sync>,
    _module_info: WasmModuleInfo,
) -> Result<(Arc<()>, Vec<Box<dyn BuiltinHandler>>)> {
    // Threading is not supported in no_std mode
    Ok((Arc::new(()), Vec::new()))
}
