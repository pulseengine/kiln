// Threading built-ins implementation for the WebAssembly Component Model
//
// This module implements the threading-related built-in functions:
// - threading.spawn: Spawn a new thread
// - threading.join: Join a thread (wait for its completion)
// - threading.sync: Create a synchronization primitive

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    vec::Vec,
};

use wrt_error::{Error, Result};
#[cfg(feature = "std")]
use wrt_foundation::{builtin::BuiltinType, component_value::ComponentValue};
#[cfg(not(feature = "std"))]
use wrt_foundation::{collections::StaticVec as BoundedVec, safe_memory::NoStdProvider};

#[cfg(not(feature = "std"))]
use crate::types::Value as ComponentValue;

#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    ThreadingSpawn,
    ThreadingJoin,
    ThreadingSync,
}

use super::BuiltinHandler;

/// Helper function to handle RwLock read operations safely for ASIL-D
/// compliance
#[cfg(feature = "std")]
fn safe_read_lock<T>(lock: &RwLock<T>) -> Result<std::sync::RwLockReadGuard<T>> {
    lock.read().map_err(|_| wrt_error::Error::threading_error("Lock poisoned"))
}

/// Helper function to handle RwLock write operations safely for ASIL-D
/// compliance
#[cfg(feature = "std")]
fn safe_write_lock<T>(lock: &RwLock<T>) -> Result<std::sync::RwLockWriteGuard<T>> {
    lock.write().map_err(|_| wrt_error::Error::threading_error("Lock poisoned"))
}

/// Helper function to handle Mutex operations safely for ASIL-D compliance
#[cfg(feature = "std")]
fn safe_mutex_lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<T>> {
    mutex.lock().map_err(|_| wrt_error::Error::threading_error("Mutex poisoned"))
}

/// Thread handle identifier
type ThreadId = u64;

/// Synchronization primitive identifier
type SyncId = u64;

/// Thread execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThreadState {
    /// Thread is running
    Running,
    /// Thread has completed successfully with a result
    Completed,
    /// Thread has terminated with an error
    Error,
}

/// Thread handle for tracking thread state and results
#[cfg(feature = "std")]
struct ThreadHandle {
    /// Thread join handle
    handle: Option<thread::JoinHandle<Result<Vec<ComponentValue>>>>,
    /// Thread state
    state: Arc<RwLock<ThreadState>>,
    /// Thread result (available when completed)
    result: Arc<RwLock<Option<Vec<ComponentValue>>>>,
    /// Thread error (available when error)
    error: Arc<RwLock<Option<String>>>,
}

/// Synchronization primitive types
enum SyncPrimitive {
    /// Mutex with optional data
    Mutex {
        /// Inner mutex
        lock: Mutex<Option<Vec<ComponentValue>>>,
    },
    /// Condition variable with a mutex
    CondVar {
        /// Inner mutex
        lock: Mutex<Option<Vec<ComponentValue>>>,
        /// Condition variable
        cvar: Condvar,
    },
    /// Read-write lock
    RwLock {
        /// Inner read-write lock
        lock: RwLock<Option<Vec<ComponentValue>>>,
    },
}

/// Thread management system
#[cfg(feature = "std")]
#[derive(Default)]
pub struct ThreadManager {
    /// Next available thread ID
    next_thread_id: AtomicU64,
    /// Map of thread ID to thread handle
    threads: RwLock<HashMap<ThreadId, ThreadHandle>>,
    /// Next available sync ID
    next_sync_id: AtomicU64,
    /// Map of sync ID to synchronization primitive
    sync_primitives: RwLock<HashMap<SyncId, SyncPrimitive>>,
}

#[cfg(feature = "std")]
impl ThreadManager {
    /// Create a new thread manager
    pub fn new() -> Self {
        Self {
            next_thread_id: AtomicU64::new(1),
            threads: RwLock::new(HashMap::new()),
            next_sync_id: AtomicU64::new(1),
            sync_primitives: RwLock::new(HashMap::new()),
        }
    }

    /// Spawn a new thread
    ///
    /// # Arguments
    ///
    /// * `function_id` - ID of the function to call
    /// * `args` - Arguments to pass to the function
    /// * `executor` - Function to execute the component function
    ///
    /// # Returns
    ///
    /// The thread ID of the spawned thread
    pub fn spawn<F>(
        &self,
        function_id: u32,
        args: Vec<ComponentValue>,
        executor: F,
    ) -> Result<ThreadId>
    where
        F: FnOnce(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + 'static,
    {
        // Get new thread ID
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::SeqCst);

        // Create shared state
        let state = Arc::new(RwLock::new(ThreadState::Running));
        let result = Arc::new(RwLock::new(None));
        let error = Arc::new(RwLock::new(None));

        // Capture state for the thread
        let thread_state = state.clone();
        let thread_result = result.clone();
        let thread_error = error.clone();

        // Spawn the actual thread
        let handle = thread::spawn(move || {
            // Execute the function
            let fn_result = executor(function_id, args);

            match fn_result {
                Ok(values) => {
                    // Store the result
                    if let Ok(mut guard) = thread_result.write() {
                        *guard = Some(values);
                    }
                    if let Ok(mut guard) = thread_state.write() {
                        *guard = ThreadState::Completed;
                    }
                    let result = thread_result
                        .read()
                        .ok()
                        .and_then(|guard| guard.clone())
                        .unwrap_or_default();
                    Ok(result)
                },
                Err(e) => {
                    // Store the error
                    if let Ok(mut guard) = thread_error.write() {
                        *guard = Some(e.to_string());
                    }
                    if let Ok(mut guard) = thread_state.write() {
                        *guard = ThreadState::Error;
                    }
                    Err(e)
                },
            }
        });

        // Store the thread handle
        let thread_handle = ThreadHandle {
            handle: Some(handle),
            state,
            result,
            error,
        };

        if let Ok(mut threads) = self.threads.write() {
            threads.insert(thread_id, thread_handle);
        } else {
            return Err(Error::runtime_execution_error(
                "Failed to acquire thread lock",
            ));
        }

        Ok(thread_id)
    }

    /// Join a thread (wait for it to complete)
    ///
    /// # Arguments
    ///
    /// * `thread_id` - ID of the thread to join
    ///
    /// # Returns
    ///
    /// The result of the thread execution
    pub fn join(&self, thread_id: ThreadId) -> Result<Vec<ComponentValue>> {
        // Find the thread
        let mut threads = self.threads.write().unwrap();
        let thread = threads
            .get_mut(&thread_id)
            .ok_or_else(|| Error::component_not_found("Thread not found"))?;

        // Check if thread is already joined
        if thread.handle.is_none() {
            // Check state to see if it completed or had an error
            let state = *thread.state.read().unwrap();

            match state {
                ThreadState::Completed => {
                    // Return the cached result
                    let result = thread
                        .result
                        .read()
                        .unwrap()
                        .clone()
                        .ok_or_else(|| Error::threading_error("Thread result unavailable"))?;
                    Ok(result)
                },
                ThreadState::Error => {
                    // Return the cached error
                    let err_msg = thread
                        .error
                        .read()
                        .unwrap()
                        .clone()
                        .unwrap_or_else(|| "Unknown thread error".to_owned());
                    Err(Error::threading_error(&err_msg))
                },
                ThreadState::Running => {
                    // This shouldn't happen if handle is None
                    Err(Error::threading_error(
                        "Thread is still running but handle is missing",
                    ))
                },
            }
        } else {
            // Take the handle out to join it
            let handle = thread
                .handle
                .take()
                .ok_or_else(|| Error::threading_error("Thread handle unavailable"))?;

            // Join the thread
            match handle.join() {
                Ok(result) => result,
                Err(_) => {
                    // Thread panicked
                    *thread.state.write().unwrap() = ThreadState::Error;
                    *thread.error.write().unwrap() = Some("Thread panicked".to_owned());
                    Err(Error::threading_error("Thread panicked during execution"))
                },
            }
        }
    }

    /// Check if a thread has completed
    ///
    /// # Arguments
    ///
    /// * `thread_id` - ID of the thread to check
    ///
    /// # Returns
    ///
    /// `true` if the thread has completed, `false` if it's still running
    pub fn is_thread_completed(&self, thread_id: ThreadId) -> Result<bool> {
        // Find the thread
        let threads = self.threads.read().unwrap();
        let thread = threads
            .get(&thread_id)
            .ok_or_else(|| Error::component_not_found("Component not found"))?;

        // Check the state
        let state = *thread.state.read().unwrap();
        Ok(state != ThreadState::Running)
    }

    /// Create a new mutex
    ///
    /// # Returns
    ///
    /// The ID of the created mutex
    pub fn create_mutex(&self) -> SyncId {
        let sync_id = self.next_sync_id.fetch_add(1, Ordering::SeqCst);

        let mutex = SyncPrimitive::Mutex {
            lock: Mutex::new(None),
        };

        self.sync_primitives.write().unwrap().insert(sync_id, mutex);
        sync_id
    }

    /// Create a new condition variable
    ///
    /// # Returns
    ///
    /// The ID of the created condition variable
    pub fn create_condvar(&self) -> SyncId {
        let sync_id = self.next_sync_id.fetch_add(1, Ordering::SeqCst);

        let condvar = SyncPrimitive::CondVar {
            lock: Mutex::new(None),
            cvar: Condvar::new(),
        };

        self.sync_primitives.write().unwrap().insert(sync_id, condvar);
        sync_id
    }

    /// Create a new read-write lock
    ///
    /// # Returns
    ///
    /// The ID of the created read-write lock
    pub fn create_rwlock(&self) -> SyncId {
        let sync_id = self.next_sync_id.fetch_add(1, Ordering::SeqCst);

        let rwlock = SyncPrimitive::RwLock {
            lock: RwLock::new(None),
        };

        self.sync_primitives.write().unwrap().insert(sync_id, rwlock);
        sync_id
    }

    /// Lock a mutex
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the mutex to lock
    /// * `data` - Optional data to store in the mutex
    ///
    /// # Returns
    ///
    /// The previous data in the mutex, if any
    pub fn lock_mutex(
        &self,
        sync_id: SyncId,
        data: Option<Vec<ComponentValue>>,
    ) -> Result<Option<Vec<ComponentValue>>> {
        // Find the sync primitive
        let primitives = self.sync_primitives.read().unwrap();

        match primitives.get(&sync_id) {
            Some(SyncPrimitive::Mutex { lock }) => {
                // Lock the mutex
                let mut guard = lock.lock().unwrap();

                // Swap the data
                let previous = guard.take();
                *guard = data;

                Ok(previous)
            },
            Some(_) => Err(Error::component_not_found("Component not found")),
            None => Err(Error::component_not_found("Component not found")),
        }
    }

    /// Wait on a condition variable
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the condition variable to wait on
    /// * `predicate` - Predicate to wait for
    ///
    /// # Returns
    ///
    /// The data stored in the condition variable
    pub fn wait_condvar(
        &self,
        sync_id: SyncId,
        predicate: Box<dyn Fn(&Option<Vec<ComponentValue>>) -> bool + Send>,
    ) -> Result<Option<Vec<ComponentValue>>> {
        // Find the sync primitive
        let primitives = self.sync_primitives.read().unwrap();

        match primitives.get(&sync_id) {
            Some(SyncPrimitive::CondVar { lock, cvar }) => {
                // Lock the mutex
                let mut guard = lock.lock().unwrap();

                // Wait until the predicate is satisfied
                while !predicate(&guard) {
                    guard = cvar.wait(guard).unwrap();
                }

                // Return the data
                Ok(guard.clone())
            },
            Some(_) => Err(Error::runtime_execution_error("Thread operation failed")),
            None => Err(Error::component_not_found("Sync primitive not found")),
        }
    }

    /// Signal a condition variable
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the condition variable to signal
    /// * `data` - Optional data to store in the condition variable
    ///
    /// # Returns
    ///
    /// The previous data in the condition variable, if any
    pub fn signal_condvar(
        &self,
        sync_id: SyncId,
        data: Option<Vec<ComponentValue>>,
    ) -> Result<Option<Vec<ComponentValue>>> {
        // Find the sync primitive
        let primitives = self.sync_primitives.read().unwrap();

        match primitives.get(&sync_id) {
            Some(SyncPrimitive::CondVar { lock, cvar }) => {
                // Lock the mutex
                let mut guard = lock.lock().unwrap();

                // Swap the data
                let previous = guard.take();
                *guard = data;

                // Signal one waiting thread
                cvar.notify_one();

                Ok(previous)
            },
            Some(_) => Err(Error::runtime_execution_error("Thread operation failed")),
            None => Err(Error::component_not_found("Sync primitive not found")),
        }
    }

    /// Acquire a read lock
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the read-write lock to read from
    ///
    /// # Returns
    ///
    /// The data in the read-write lock, if any
    pub fn read_rwlock(&self, sync_id: SyncId) -> Result<Option<Vec<ComponentValue>>> {
        // Find the sync primitive
        let primitives = self.sync_primitives.read().unwrap();

        match primitives.get(&sync_id) {
            Some(SyncPrimitive::RwLock { lock }) => {
                // Acquire a read lock
                let guard = lock.read().unwrap();

                // Return a clone of the data
                Ok(guard.clone())
            },
            Some(_) => Err(Error::runtime_execution_error("Thread operation failed")),
            None => Err(Error::component_not_found("Sync primitive not found")),
        }
    }

    /// Acquire a write lock
    ///
    /// # Arguments
    ///
    /// * `sync_id` - ID of the read-write lock to write to
    /// * `data` - Optional data to store in the read-write lock
    ///
    /// # Returns
    ///
    /// The previous data in the read-write lock, if any
    pub fn write_rwlock(
        &self,
        sync_id: SyncId,
        data: Option<Vec<ComponentValue>>,
    ) -> Result<Option<Vec<ComponentValue>>> {
        // Find the sync primitive
        let primitives = self.sync_primitives.read().unwrap();

        match primitives.get(&sync_id) {
            Some(SyncPrimitive::RwLock { lock }) => {
                // Acquire a write lock
                let mut guard = lock.write().unwrap();

                // Swap the data
                let previous = guard.take();
                *guard = data;

                Ok(previous)
            },
            Some(_) => Err(Error::runtime_execution_error("Thread operation failed")),
            None => Err(Error::component_not_found("Sync primitive not found")),
        }
    }
}

/// Handler for threading.spawn built-in
#[cfg(feature = "std")]
#[derive(Clone)]
pub struct ThreadingSpawnHandler {
    /// Thread manager
    thread_manager: Arc<ThreadManager>,
    /// Function to execute the component function
    executor: Arc<dyn Fn(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + Sync>,
}

#[cfg(feature = "std")]
impl ThreadingSpawnHandler {
    /// Create a new threading.spawn handler
    ///
    /// # Arguments
    ///
    /// * `thread_manager` - Thread manager to use
    /// * `executor` - Function to execute the component function
    pub fn new(
        thread_manager: Arc<ThreadManager>,
        executor: Arc<
            dyn Fn(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + Sync,
        >,
    ) -> Self {
        Self {
            thread_manager,
            executor,
        }
    }
}

#[cfg(feature = "std")]
impl BuiltinHandler for ThreadingSpawnHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ThreadingSpawn
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.len() < 1 {
            return Err(Error::threading_error(
                "threading.spawn requires at least 1 argument",
            ));
        }

        // Extract function ID
        let function_id = match args[0] {
            ComponentValue::U32(id) => id,
            _ => {
                return Err(Error::threading_error(
                    "threading.spawn first argument must be a function ID",
                ));
            },
        };

        // Extract function arguments
        let function_args = args[1..].to_vec();

        // Create a clone of the executor
        let executor = self.executor.clone();

        // Spawn the thread
        let thread_id =
            self.thread_manager.spawn(function_id, function_args, move |id, args| {
                executor(id, args)
            })?;

        // Return the thread ID
        Ok(vec![ComponentValue::U64(thread_id)])
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Handler for threading.join built-in
#[cfg(feature = "std")]
#[derive(Clone)]
pub struct ThreadingJoinHandler {
    /// Thread manager
    thread_manager: Arc<ThreadManager>,
}

#[cfg(feature = "std")]
impl ThreadingJoinHandler {
    /// Create a new threading.join handler
    ///
    /// # Arguments
    ///
    /// * `thread_manager` - Thread manager to use
    pub fn new(thread_manager: Arc<ThreadManager>) -> Self {
        Self { thread_manager }
    }
}

#[cfg(feature = "std")]
impl BuiltinHandler for ThreadingJoinHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ThreadingJoin
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.len() != 1 {
            return Err(Error::threading_error(
                "threading.join requires exactly 1 argument",
            ));
        }

        // Extract thread ID
        let thread_id = match args[0] {
            ComponentValue::U64(id) => id,
            _ => {
                return Err(Error::threading_error(
                    "threading.join argument must be a thread ID",
                ));
            },
        };

        // Join the thread
        self.thread_manager.join(thread_id)
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Handler for threading.sync built-in
#[cfg(feature = "std")]
#[derive(Clone)]
pub struct ThreadingSyncHandler {
    /// Thread manager
    thread_manager: Arc<ThreadManager>,
    /// Flag to track if this is a no-op in no_std mode
    no_std_mode: AtomicBool,
}

#[cfg(feature = "std")]
impl ThreadingSyncHandler {
    /// Create a new threading.sync handler
    ///
    /// # Arguments
    ///
    /// * `thread_manager` - Thread manager to use
    pub fn new(thread_manager: Arc<ThreadManager>) -> Self {
        Self {
            thread_manager,
            no_std_mode: AtomicBool::new(false),
        }
    }
}

#[cfg(feature = "std")]
impl BuiltinHandler for ThreadingSyncHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ThreadingSync
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Check if we're in no_std mode (should never happen if properly feature-gated)
        if self.no_std_mode.load(Ordering::Relaxed) {
            return Err(Error::threading_error(
                "Threading is not supported in no_std mode",
            ));
        }

        // Validate arguments
        if args.len() < 1 {
            return Err(Error::threading_error(
                "threading.sync requires at least 1 argument",
            ));
        }

        // Extract operation type
        let op_type = match &args[0] {
            ComponentValue::String(s) => s.as_str(),
            _ => {
                return Err(Error::threading_error(
                    "threading.sync first argument must be a string",
                ));
            },
        };

        match op_type {
            "create-mutex" => {
                // Create a new mutex
                let mutex_id = self.thread_manager.create_mutex();
                Ok(vec![ComponentValue::U64(mutex_id)])
            },
            "lock-mutex" => {
                // Lock a mutex
                if args.len() < 2 {
                    return Err(Error::threading_error("lock-mutex requires a mutex ID"));
                }

                let mutex_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::threading_error(
                            "lock-mutex requires a mutex ID as second argument",
                        ));
                    },
                };

                // Extract optional data
                let data = if args.len() > 2 { Some(args[2..].to_vec()) } else { None };

                // Lock the mutex
                let previous = self.thread_manager.lock_mutex(mutex_id, data)?;

                // Return the previous data
                Ok(previous.unwrap_or_default())
            },
            "create-condvar" => {
                // Create a new condition variable
                let condvar_id = self.thread_manager.create_condvar();
                Ok(vec![ComponentValue::U64(condvar_id)])
            },
            "wait-condvar" => {
                // Wait on a condition variable
                if args.len() < 2 {
                    return Err(Error::threading_error("wait-condvar requires a condvar ID"));
                }

                let condvar_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::threading_error(
                            "wait-condvar requires a condvar ID as second argument",
                        ));
                    },
                };

                // Simple predicate that always waits until signaled
                let predicate = Box::new(|_: &Option<Vec<ComponentValue>>| true);

                // Wait on the condition variable
                let data = self.thread_manager.wait_condvar(condvar_id, predicate)?;

                // Return the data
                Ok(data.unwrap_or_default())
            },
            "signal-condvar" => {
                // Signal a condition variable
                if args.len() < 2 {
                    return Err(Error::threading_error(
                        "signal-condvar requires a condvar ID",
                    ));
                }

                let condvar_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::threading_error(
                            "signal-condvar requires a condvar ID as second argument",
                        ));
                    },
                };

                // Extract optional data
                let data = if args.len() > 2 { Some(args[2..].to_vec()) } else { None };

                // Signal the condition variable
                let previous = self.thread_manager.signal_condvar(condvar_id, data)?;

                // Return the previous data
                Ok(previous.unwrap_or_default())
            },
            "create-rwlock" => {
                // Create a new read-write lock
                let rwlock_id = self.thread_manager.create_rwlock();
                Ok(vec![ComponentValue::U64(rwlock_id)])
            },
            "read-rwlock" => {
                // Read from a read-write lock
                if args.len() < 2 {
                    return Err(Error::threading_error("read-rwlock requires a rwlock ID"));
                }

                let rwlock_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::threading_error(
                            "read-rwlock requires a rwlock ID as second argument",
                        ));
                    },
                };

                // Read from the read-write lock
                let data = self.thread_manager.read_rwlock(rwlock_id)?;

                // Return the data
                Ok(data.unwrap_or_default())
            },
            "write-rwlock" => {
                // Write to a read-write lock
                if args.len() < 2 {
                    return Err(Error::threading_error("write-rwlock requires a rwlock ID"));
                }

                let rwlock_id = match args[1] {
                    ComponentValue::U64(id) => id,
                    _ => {
                        return Err(Error::threading_error(
                            "write-rwlock requires a rwlock ID as second argument",
                        ));
                    },
                };

                // Extract optional data
                let data = if args.len() > 2 { Some(args[2..].to_vec()) } else { None };

                // Write to the read-write lock
                let previous = self.thread_manager.write_rwlock(rwlock_id, data)?;

                // Return the previous data
                Ok(previous.unwrap_or_default())
            },
            _ => Err(Error::runtime_execution_error(&format!(
                "Unknown sync operation: {}",
                op_type
            ))),
        }
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Create handlers for threading built-ins
#[cfg(feature = "std")]
pub fn create_threading_handlers(
    executor: Arc<dyn Fn(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + Sync>,
) -> Vec<Box<dyn BuiltinHandler>> {
    let thread_manager = Arc::new(ThreadManager::new());

    vec![
        Box::new(ThreadingSpawnHandler::new(thread_manager.clone(), executor)),
        Box::new(ThreadingJoinHandler::new(thread_manager.clone())),
        Box::new(ThreadingSyncHandler::new(thread_manager)),
    ]
}

/// Create no-op threading handlers for no_std environments
#[cfg(not(feature = "std"))]
pub fn create_threading_handlers(
    _: Arc<dyn Fn(u32, Vec<ComponentValue>) -> Result<Vec<ComponentValue>> + Send + Sync>,
) -> Vec<Box<dyn BuiltinHandler>> {
    // Threading is not supported in no_std mode
    Vec::new()
}
