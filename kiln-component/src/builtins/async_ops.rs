// Async operations built-ins implementation for the WebAssembly Component Model
//
// This module implements the async-related built-in functions:
// - async.new: Create a new async value
// - async.get: Get the value from an async value once resolved
// - async.poll: Poll an async value for completion
// - async.wait: Wait for an async value to complete

#[cfg(all(feature = "component-model-async", not(feature = "std")))]
use alloc::{boxed::Box, vec::Vec};
#[cfg(all(feature = "component-model-async", feature = "std"))]
use std::{
    boxed::Box,
    collections::HashMap,
    sync::{Arc, Mutex},
    vec::Vec,
};

#[cfg(feature = "component-model-async")]
use wrt_error::{Error, Result};
#[cfg(all(feature = "component-model-async", feature = "std"))]
use wrt_foundation::{builtin::BuiltinType, component_value::ComponentValue};
#[cfg(all(feature = "component-model-async", not(feature = "std")))]
use wrt_foundation::{collections::StaticVec as BoundedVec, safe_memory::NoStdProvider};

#[cfg(all(feature = "component-model-async", not(feature = "std")))]
use crate::types::Value as ComponentValue;

#[cfg(all(feature = "component-model-async", not(feature = "std")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    AsyncNew,
    AsyncGet,
    AsyncPoll,
    AsyncWait,
}

#[cfg(feature = "component-model-async")]
use crate::builtins::BuiltinHandler;

#[cfg(feature = "component-model-async")]
/// Status of an async computation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncStatus {
    /// The computation is still pending
    Pending,
    /// The computation has been resolved with a result
    Ready,
    /// The computation has failed
    Failed,
}

#[cfg(feature = "component-model-async")]
/// Storage for async value information
pub struct AsyncValueStore {
    /// Map of async IDs to their values
    values: HashMap<u32, AsyncValue>,
    /// Next available async ID
    next_id: u32,
}

#[cfg(feature = "component-model-async")]
/// Information about an async computation
pub struct AsyncValue {
    /// Current status of the computation
    status: AsyncStatus,
    /// Result value (if available)
    result: Option<Vec<ComponentValue>>,
    /// Error message (if failed)
    error: Option<String>,
}

#[cfg(feature = "component-model-async")]
impl AsyncValueStore {
    /// Create a new async value store
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            next_id: 1, // Start at 1, 0 is reserved
        }
    }

    /// Generate a new async ID
    pub fn generate_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Create a new async value with the given status
    pub fn create_async(&mut self, status: AsyncStatus) -> u32 {
        let id = self.generate_id();
        self.values.insert(
            id,
            AsyncValue {
                status,
                result: None,
                error: None,
            },
        );
        id
    }

    /// Set the result of an async computation
    pub fn set_result(&mut self, id: u32, result: Vec<ComponentValue>) -> Result<()> {
        match self.values.get_mut(&id) {
            Some(async_value) => {
                async_value.status = AsyncStatus::Ready;
                async_value.result = Some(result);

                Ok(())
            },
            None => Err(Error::component_not_found("Error occurred")),
        }
    }

    /// Set an error for an async computation
    pub fn set_error(&mut self, id: u32, error: String) -> Result<()> {
        match self.values.get_mut(&id) {
            Some(async_value) => {
                async_value.status = AsyncStatus::Failed;
                async_value.error = Some(error);

                Ok(())
            },
            None => Err(Error::component_not_found("Error occurred")),
        }
    }

    /// Get the status of an async computation
    pub fn get_status(&self, id: u32) -> Result<AsyncStatus> {
        match self.values.get(&id) {
            Some(async_value) => Ok(async_value.status.clone()),
            None => Err(Error::component_not_found("Error occurred")),
        }
    }

    /// Get the result of an async computation
    pub fn get_result(&self, id: u32) -> Result<Vec<ComponentValue>> {
        match self.values.get(&id) {
            Some(async_value) => {
                if async_value.status == AsyncStatus::Ready {
                    async_value.result.clone().ok_or_else(|| Error::async_error("Error occurred"))
                } else if async_value.status == AsyncStatus::Failed {
                    Err(Error::async_error(
                        &async_value
                            .error
                            .clone()
                            .unwrap_or_else(|| "Async operation failed".to_owned()),
                    ))
                } else {
                    Err(Error::async_error("Error occurred"))
                }
            },
            None => Err(Error::component_not_found("Error occurred")),
        }
    }

    /// Check if an async value exists
    pub fn has_async(&self, id: u32) -> bool {
        self.values.contains_key(&id)
    }

    /// Remove an async value
    pub fn remove_async(&mut self, id: u32) -> Result<()> {
        if self.values.remove(&id).is_some() {
            Ok(())
        } else {
            Err(Error::component_not_found("Error occurred"))
        }
    }
}

#[cfg(feature = "component-model-async")]
/// Handler for the async.new built-in function
pub struct AsyncNewHandler {
    /// Store containing async values
    async_store: Arc<Mutex<AsyncValueStore>>,
}

#[cfg(feature = "component-model-async")]
impl AsyncNewHandler {
    /// Create a new async.new handler
    pub fn new(async_store: Arc<Mutex<AsyncValueStore>>) -> Self {
        Self { async_store }
    }
}

#[cfg(feature = "component-model-async")]
impl BuiltinHandler for AsyncNewHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::AsyncNew
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate args - async.new takes no arguments
        if !args.is_empty() {
            return Err(Error::component_not_found("Error occurred"));
        }

        // Create a new async value
        let id = {
            let mut store = self.async_store.lock().unwrap();
            store.create_async(AsyncStatus::Pending)
        };

        // Return the async ID
        Ok(vec![ComponentValue::U32(id)])
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            async_store: self.async_store.clone(),
        })
    }
}

#[cfg(feature = "component-model-async")]
/// Handler for the async.get built-in function
pub struct AsyncGetHandler {
    /// Store containing async values
    async_store: Arc<Mutex<AsyncValueStore>>,
}

#[cfg(feature = "component-model-async")]
impl AsyncGetHandler {
    /// Create a new async.get handler
    pub fn new(async_store: Arc<Mutex<AsyncValueStore>>) -> Self {
        Self { async_store }
    }
}

#[cfg(feature = "component-model-async")]
impl BuiltinHandler for AsyncGetHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::AsyncGet
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::component_not_found("Error occurred"));
        }

        // Extract the async ID from args
        let async_id = match &args[0] {
            ComponentValue::U32(id) => *id,
            _ => {
                return Err(Error::runtime_execution_error("Error occurred"));
            },
        };

        // Get the result of the async computation
        let store = self.async_store.lock().unwrap();
        store.get_result(async_id)
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            async_store: self.async_store.clone(),
        })
    }
}

#[cfg(feature = "std")]
/// Handler for the async.poll built-in function
pub struct AsyncPollHandler {
    /// Store containing async values
    async_store: Arc<Mutex<AsyncValueStore>>,
}

#[cfg(feature = "component-model-async")]
impl AsyncPollHandler {
    /// Create a new async.poll handler
    pub fn new(async_store: Arc<Mutex<AsyncValueStore>>) -> Self {
        Self { async_store }
    }
}

#[cfg(feature = "component-model-async")]
impl BuiltinHandler for AsyncPollHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::AsyncPoll
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::component_not_found("Error occurred"));
        }

        // Extract the async ID from args
        let async_id = match &args[0] {
            ComponentValue::U32(id) => *id,
            _ => {
                return Err(Error::runtime_execution_error("Error occurred"));
            },
        };

        // Check the status of the async computation
        let store = self.async_store.lock().unwrap();
        let status = store.get_status(async_id)?;

        // Return the status as a variant
        match status {
            AsyncStatus::Pending => Ok(vec![ComponentValue::U32(0)]), // 0 = pending
            AsyncStatus::Ready => Ok(vec![ComponentValue::U32(1)]),   // 1 = ready
            AsyncStatus::Failed => Ok(vec![ComponentValue::U32(2)]),  // 2 = failed
        }
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            async_store: self.async_store.clone(),
        })
    }
}

#[cfg(feature = "std")]
#[cfg(feature = "std")]
/// Handler for the async.wait built-in function
pub struct AsyncWaitHandler {
    /// Store containing async values
    async_store: Arc<Mutex<AsyncValueStore>>,
}

#[cfg(feature = "component-model-async")]
#[cfg(feature = "std")]
impl AsyncWaitHandler {
    /// Create a new async.wait handler
    pub fn new(async_store: Arc<Mutex<AsyncValueStore>>) -> Self {
        Self { async_store }
    }
}

#[cfg(feature = "component-model-async")]
#[cfg(feature = "std")]
impl BuiltinHandler for AsyncWaitHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::AsyncWait
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate args
        if args.len() != 1 {
            return Err(Error::component_not_found("Error occurred"));
        }

        // Extract the async ID from args
        let async_id = match &args[0] {
            ComponentValue::U32(id) => *id,
            _ => {
                return Err(Error::runtime_execution_error("Error occurred"));
            },
        };

        // Use Component Model polling instead of Rust futures
        loop {
            let store = self.async_store.lock().unwrap();

            match store.get_status(async_id) {
                Ok(AsyncStatus::Ready) => {
                    return store.get_result(async_id);
                },
                Ok(AsyncStatus::Failed) => {
                    return store.get_result(async_id); // Will return the error
                },
                Ok(AsyncStatus::Pending) => {
                    // Drop the lock and yield/sleep briefly
                    drop(store);

                    #[cfg(feature = "std")]
                    std::thread::sleep(std::time::Duration::from_millis(1));

                    // Continue polling
                    continue;
                },
                Err(e) => return Err(e),
            }
        }
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(Self {
            async_store: self.async_store.clone(),
        })
    }
}

#[cfg(feature = "component-model-async")]
/// Create all async-related built-in handlers
pub fn create_async_handlers(
    async_store: Arc<Mutex<AsyncValueStore>>,
) -> Vec<Box<dyn BuiltinHandler>> {
    let mut handlers = vec![
        Box::new(AsyncNewHandler::new(async_store.clone())),
        Box::new(AsyncGetHandler::new(async_store.clone())),
        Box::new(AsyncPollHandler::new(async_store.clone())),
    ];

    #[cfg(feature = "std")]
    handlers.push(Box::new(AsyncWaitHandler::new(async_store)));

    handlers
}
