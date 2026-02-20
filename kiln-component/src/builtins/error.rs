// Error context built-ins implementation for the WebAssembly Component Model
//
// This module implements the error-related built-in functions:
// - error.new: Create a new error context
// - error.trace: Get the trace from an error context
//
// Note: Full functionality requires std feature for Arc/Mutex

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::{
    boxed::Box,
    collections::HashMap,
    string::String,
    sync::{Arc, Mutex},
    vec::Vec,
};

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_foundation::component_value::ComponentValue;
#[cfg(not(feature = "std"))]
use wrt_foundation::{collections::StaticVec as BoundedVec, safe_memory::NoStdProvider};

// Define a stub BuiltinType for no_std
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinType {
    ErrorNew,
    ErrorTrace,
}

#[cfg(feature = "std")]
use wrt_foundation::builtin::BuiltinType;

use super::BuiltinHandler;

/// Error context object
#[derive(Clone, Debug)]
pub struct ErrorContext {
    /// Error message
    message: String,
    /// Optional trace information
    trace: Vec<String>,
    /// Optional additional metadata
    metadata: HashMap<String, String>,
}

impl ErrorContext {
    /// Create a new error context with the given message
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            trace: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a trace entry to the error context
    pub fn add_trace(&mut self, trace_entry: &str) {
        self.trace.push(trace_entry.to_string());
    }

    /// Get the error message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Get the trace information
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Add metadata to the error context
    pub fn add_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    /// Get metadata value for a key
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Store for error contexts
#[derive(Default)]
pub struct ErrorContextStore {
    /// Map of error context ID to error context
    contexts: HashMap<u64, ErrorContext>,
    /// Next available error context ID
    next_id: u64,
}

impl ErrorContextStore {
    /// Create a new error context store
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new error context and return its ID
    pub fn create_error(&mut self, message: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.contexts.insert(id, ErrorContext::new(message));
        id
    }

    /// Get an error context by ID
    pub fn get_error(&self, id: u64) -> Option<&ErrorContext> {
        self.contexts.get(&id)
    }

    /// Get a mutable reference to an error context by ID
    pub fn get_error_mut(&mut self, id: u64) -> Option<&mut ErrorContext> {
        self.contexts.get_mut(&id)
    }

    /// Drop an error context by ID
    pub fn drop_error(&mut self, id: u64) -> bool {
        self.contexts.remove(&id).is_some()
    }
}

/// Handler for error.new built-in
#[derive(Clone)]
pub struct ErrorNewHandler {
    /// Store for error contexts
    store: Arc<Mutex<ErrorContextStore>>,
}

impl ErrorNewHandler {
    /// Create a new error.new handler
    pub fn new(store: Arc<Mutex<ErrorContextStore>>) -> Self {
        Self { store }
    }
}

impl BuiltinHandler for ErrorNewHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ErrorNew
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.len() != 1 {
            return Err(Error::validation_invalid_input("Error occurred"));
        }

        // Extract error message
        let message = match &args[0] {
            ComponentValue::String(s) => s.as_str(),
            _ => return Err(Error::runtime_execution_error("Error occurred")),
        };

        // Create a new error context
        let id = self.store.lock().unwrap().create_error(message);

        // Return the error context ID
        Ok(vec![ComponentValue::U64(id)])
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Handler for error.trace built-in
#[derive(Clone)]
pub struct ErrorTraceHandler {
    /// Store for error contexts
    store: Arc<Mutex<ErrorContextStore>>,
}

impl ErrorTraceHandler {
    /// Create a new error.trace handler
    pub fn new(store: Arc<Mutex<ErrorContextStore>>) -> Self {
        Self { store }
    }
}

impl BuiltinHandler for ErrorTraceHandler {
    fn builtin_type(&self) -> BuiltinType {
        BuiltinType::ErrorTrace
    }

    fn execute(&self, args: &[ComponentValue]) -> Result<Vec<ComponentValue>> {
        // Validate arguments
        if args.len() != 2 {
            return Err(Error::validation_invalid_input("Missing error message"));
        }

        // Extract error context ID
        let error_id = match args[0] {
            ComponentValue::U64(id) => id,
            _ => {
                return Err(Error::runtime_execution_error("Error occurred"));
            },
        };

        // Extract trace message
        let trace_message = match &args[1] {
            ComponentValue::String(s) => s.as_str(),
            _ => {
                return Err(Error::new(
                    ErrorCategory::Type,
                    codes::TYPE_MISMATCH_ERROR,
                    "Error message needed",
                ));
            },
        };

        // Add trace to the error context
        let mut store = self.store.lock().unwrap();
        let error_context = store
            .get_error_mut(error_id)
            .ok_or_else(|| Error::resource_not_found("Error occurred"))?;
        error_context.add_trace(trace_message);

        // No return value
        Ok(vec![])
    }

    fn clone_handler(&self) -> Box<dyn BuiltinHandler> {
        Box::new(self.clone())
    }
}

/// Create handlers for error built-ins
pub fn create_error_handlers() -> Vec<Box<dyn BuiltinHandler>> {
    let store = Arc::new(Mutex::new(ErrorContextStore::new()));
    vec![
        Box::new(ErrorNewHandler::new(store.clone())),
        Box::new(ErrorTraceHandler::new(store)),
    ]
}
