// WRT - wrt-component
// Module: Error Context Canonical Built-ins
// SW-REQ-ID: REQ_ERROR_CONTEXT_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! Error Context Canonical Built-ins
//!
//! This module provides implementation of the `error-context.*` built-in functions
//! required by the WebAssembly Component Model for managing error contexts and
//! debugging information.


extern crate alloc;

#[cfg(feature = "std")]
use std::{boxed::Box, collections::HashMap, cell::RefCell as AtomicRefCell, string::String, vec::Vec};

use wrt_error::{Error, ErrorCategory, Result};
use wrt_foundation::{
    bounded::{BoundedMap, BoundedString},
    component_value::ComponentValue,
    safe_memory::NoStdProvider,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ToBytes, ReadStream, WriteStream},
    verification::Checksum,
    MemoryProvider,
};


use crate::async_::async_types::{ErrorContext, ErrorContextHandle};

// Constants for no_std environments
#[cfg(not(any(feature = "std", )))]
const MAX_ERROR_CONTEXTS: usize = 64;
#[cfg(not(any(feature = "std", )))]
const MAX_DEBUG_MESSAGE_SIZE: usize = 512;
#[cfg(not(any(feature = "std", )))]
const MAX_STACK_FRAMES: usize = 32;
#[cfg(not(any(feature = "std", )))]
const MAX_METADATA_ENTRIES: usize = 16;
#[cfg(not(any(feature = "std", )))]
const MAX_METADATA_KEY_SIZE: usize = 64;

/// Error context identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorContextId(pub u64);

impl ErrorContextId {
    pub fn new() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = 
            core::sync::atomic::AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst))
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for ErrorContextId {
    fn default() -> Self {
        Self::new()
    }
}

/// Error severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning", 
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    pub fn as_u32(&self) -> u32 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Error => 2,
            Self::Critical => 3,
        }
    }

    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Info),
            1 => Some(Self::Warning),
            2 => Some(Self::Error),
            3 => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Stack frame information for error contexts
#[derive(Debug, Clone)]
pub struct StackFrame {
    #[cfg(feature = "std")]
    pub function_name: String,
    #[cfg(not(any(feature = "std", )))]
    pub function_name: BoundedString<MAX_DEBUG_MESSAGE_SIZE>,
    
    #[cfg(feature = "std")]
    pub file_name: Option<String>,
    #[cfg(not(any(feature = "std", )))]
    pub file_name: Option<BoundedString<MAX_DEBUG_MESSAGE_SIZE>>,
    
    pub line_number: Option<u32>,
    pub column_number: Option<u32>,
}

impl StackFrame {
    #[cfg(feature = "std")]
    pub fn new(function_name: String) -> Self {
        Self {
            function_name,
            file_name: None,
            line_number: None,
            column_number: None,
        }
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn new(function_name: &str) -> Result<Self> {
        let bounded_name = BoundedString::try_from_str(function_name)
            .map_err(|_| Error::memory_allocation_failed("Function name too long for no_std environment"))?;
        Ok(Self {
            function_name: bounded_name,
            file_name: None,
            line_number: None,
            column_number: None,
        })
    }

    #[cfg(feature = "std")]
    pub fn with_location(mut self, file_name: String, line: u32, column: u32) -> Self {
        self.file_name = Some(file_name);
        self.line_number = Some(line);
        self.column_number = Some(column);
        self
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn with_location(mut self, file_name: &str, line: u32, column: u32) -> Result<Self> {
        let bounded_file = BoundedString::try_from_str(file_name)
            .map_err(|_| Error::memory_allocation_failed("File name too long for no_std environment"))?;
        self.file_name = Some(bounded_file);
        self.line_number = Some(line);
        self.column_number = Some(column);
        Ok(self)
    }

    pub fn function_name(&self) -> &str {
        #[cfg(feature = "std")]
        return &self.function_name;
        #[cfg(not(any(feature = "std", )))]
        return self.function_name.as_str();
    }

    pub fn file_name(&self) -> Option<&str> {
        match &self.file_name {
            #[cfg(feature = "std")]
            Some(name) => Some(name),
            #[cfg(not(any(feature = "std", )))]
            Some(name) => Some(name.as_str()),
            None => None,
        }
    }
}

impl PartialEq for StackFrame {
    fn eq(&self, other: &Self) -> bool {
        self.function_name() == other.function_name() &&
        self.file_name() == other.file_name() &&
        self.line_number == other.line_number &&
        self.column_number == other.column_number
    }
}

impl Eq for StackFrame {}

impl Default for StackFrame {
    fn default() -> Self {
        #[cfg(feature = "std")]
        {
            Self {
                function_name: String::new(),
                file_name: None,
                line_number: None,
                column_number: None,
            }
        }
        #[cfg(not(any(feature = "std", )))]
        {
            Self {
                function_name: BoundedString::new(),
                file_name: None,
                line_number: None,
                column_number: None,
            }
        }
    }
}

impl Checksummable for StackFrame {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.function_name().update_checksum(checksum);
        if let Some(file) = self.file_name() {
            file.update_checksum(checksum);
        }
        self.line_number.update_checksum(checksum);
        self.column_number.update_checksum(checksum);
    }
}

impl ToBytes for StackFrame {
    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.function_name().to_bytes_with_provider(writer, provider)?;
        match self.file_name() {
            Some(name) => {
                true.to_bytes_with_provider(writer, provider)?;
                name.to_bytes_with_provider(writer, provider)?;
            }
            None => {
                false.to_bytes_with_provider(writer, provider)?;
            }
        }
        self.line_number.to_bytes_with_provider(writer, provider)?;
        self.column_number.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for StackFrame {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        #[cfg(feature = "std")]
        {
            let function_name = String::from_bytes_with_provider(reader, provider)?;
            let has_file = bool::from_bytes_with_provider(reader, provider)?;
            let file_name = if has_file {
                Some(String::from_bytes_with_provider(reader, provider)?)
            } else {
                None
            };
            let line_number = Option::<u32>::from_bytes_with_provider(reader, provider)?;
            let column_number = Option::<u32>::from_bytes_with_provider(reader, provider)?;
            Ok(Self {
                function_name,
                file_name,
                line_number,
                column_number,
            })
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let function_name = BoundedString::<MAX_DEBUG_MESSAGE_SIZE>::from_bytes_with_provider(reader, provider)?;
            let has_file = bool::from_bytes_with_provider(reader, provider)?;
            let file_name = if has_file {
                Some(BoundedString::<MAX_DEBUG_MESSAGE_SIZE>::from_bytes_with_provider(reader, provider)?)
            } else {
                None
            };
            let line_number = Option::<u32>::from_bytes_with_provider(reader, provider)?;
            let column_number = Option::<u32>::from_bytes_with_provider(reader, provider)?;
            Ok(Self {
                function_name,
                file_name,
                line_number,
                column_number,
            })
        }
    }
}

/// Error context implementation with debugging information
#[derive(Debug, Clone)]
pub struct ErrorContextImpl {
    pub id: ErrorContextId,
    pub handle: ErrorContextHandle,
    pub severity: ErrorSeverity,
    
    #[cfg(feature = "std")]
    pub debug_message: String,
    #[cfg(not(any(feature = "std", )))]
    pub debug_message: BoundedString<MAX_DEBUG_MESSAGE_SIZE>,
    
    #[cfg(feature = "std")]
    pub stack_trace: Vec<StackFrame>,
    #[cfg(not(any(feature = "std", )))]
    pub stack_trace: BoundedVec<StackFrame, MAX_STACK_FRAMES>,
    
    #[cfg(feature = "std")]
    pub metadata: HashMap<String, ComponentValue>,
    #[cfg(not(any(feature = "std", )))]
    pub metadata: BoundedMap<BoundedString<MAX_METADATA_KEY_SIZE>, ComponentValue, MAX_METADATA_ENTRIES>,
    
    pub error_code: Option<u32>,
    pub source_error: Option<Box<ErrorContextImpl>>,
}

impl ErrorContextImpl {
    #[cfg(feature = "std")]
    pub fn new(message: String, severity: ErrorSeverity) -> Self {
        Self {
            id: ErrorContextId::new(),
            handle: ErrorContextHandle::new(),
            severity,
            debug_message: message,
            stack_trace: Vec::new(),
            metadata: HashMap::new(),
            error_code: None,
            source_error: None,
        }
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn new(message: &str, severity: ErrorSeverity) -> Result<Self> {
        let bounded_message = BoundedString::try_from_str(message)
            .map_err(|_| Error::memory_allocation_failed("Debug message too long for no_std environment"))?;
        Ok(Self {
            id: ErrorContextId::new(),
            handle: ErrorContextHandle::new(),
            severity,
            debug_message: bounded_message,
            stack_trace: BoundedVec::new(),
            metadata: BoundedMap::new(safe_managed_alloc!(4096, CrateId::Component)?)?,
            error_code: None,
            source_error: None,
        })
    }

    pub fn with_error_code(mut self, code: u32) -> Self {
        self.error_code = Some(code);
        self
    }

    pub fn with_source_error(mut self, source: ErrorContextImpl) -> Self {
        self.source_error = Some(Box::new(source));
        self
    }

    #[cfg(feature = "std")]
    pub fn add_stack_frame(&mut self, frame: StackFrame) {
        self.stack_trace.push(frame);
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn add_stack_frame(&mut self, frame: StackFrame) -> Result<()> {
        self.stack_trace.push(frame)
            .map_err(|_| Error::memory_allocation_failed("Stack trace full"))?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn set_metadata(&mut self, key: String, value: ComponentValue) {
        self.metadata.insert(key, value);
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn set_metadata(&mut self, key: &str, value: ComponentValue) -> Result<()> {
        let bounded_key = BoundedString::try_from_str(key)
            .map_err(|_| Error::memory_allocation_failed("Metadata key too long for no_std environment"))?;
        self.metadata.insert(bounded_key, value)
            .map_err(|_| Error::memory_allocation_failed("Metadata storage full"))?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn get_metadata(&self, key: &str) -> Option<&ComponentValue> {
        self.metadata.get(key)
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn get_metadata(&self, key: &str) -> Option<&ComponentValue> {
        if let Ok(bounded_key) = BoundedString::try_from_str(key) {
            self.metadata.get(&bounded_key)
        } else {
            None
        }
    }

    pub fn debug_message(&self) -> &str {
        #[cfg(feature = "std")]
        return &self.debug_message;
        #[cfg(not(any(feature = "std", )))]
        return self.debug_message.as_str();
    }

    pub fn stack_frame_count(&self) -> usize {
        self.stack_trace.len()
    }

    pub fn get_stack_frame(&self, index: usize) -> Option<&StackFrame> {
        self.stack_trace.get(index)
    }

    #[cfg(feature = "std")]
    pub fn format_stack_trace(&self) -> String {
        let mut output = String::new();
        for (i, frame) in self.stack_trace.iter().enumerate() {
            output.push_str(&format!("  #{}: {}", i, frame.function_name);
            if let Some(file) = frame.file_name() {
                output.push_str(&format!(" at {}:{}", file, frame.line_number.unwrap_or(0)));
            }
            output.push('\n');
        }
        output
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn format_stack_trace(&self) -> core::result::Result<BoundedString<1024>> {
        let mut output = BoundedString::new();
        for (i, frame) in self.stack_trace.iter().enumerate() {
            // Binary std/no_std choice
            output.push_str("  #").map_err(|_| Error::memory_allocation_failed("Stack trace format buffer full"))?;
            output.push_str(": ").map_err(|_| Error::memory_allocation_failed("Stack trace format buffer full"))?;
            output.push_str(frame.function_name()).map_err(|_| Error::memory_allocation_failed("Stack trace format buffer full"))?;
            output.push('\n').map_err(|_| Error::memory_allocation_failed("Stack trace format buffer full"))?;
        }
        Ok(output)
    }
}

/// Global registry for error contexts
static ERROR_CONTEXT_REGISTRY: AtomicRefCell<Option<ErrorContextRegistry>> = 
    AtomicRefCell::new(None);

/// Registry that manages all error contexts
#[derive(Debug)]
pub struct ErrorContextRegistry {
    #[cfg(feature = "std")]
    contexts: HashMap<ErrorContextId, ErrorContextImpl>,
    #[cfg(not(any(feature = "std", )))]
    contexts: BoundedMap<ErrorContextId, ErrorContextImpl, MAX_ERROR_CONTEXTS>,
}

impl ErrorContextRegistry {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            contexts: HashMap::new(),
            #[cfg(not(any(feature = "std", )))]
            contexts: BoundedMap::new(
                safe_managed_alloc!(4096, CrateId::Component)?
            ).map_err(|_| Error::memory_allocation_failed("Failed to create context map"))?,
        }
    }

    pub fn register_context(&mut self, context: ErrorContextImpl) -> Result<ErrorContextId> {
        let id = context.id;
        #[cfg(feature = "std")]
        {
            self.contexts.insert(id, context);
            Ok(id)
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.contexts.insert(id, context)
                .map_err(|_| Error::memory_allocation_failed("Error context registry full"))?;
            Ok(id)
        }
    }

    pub fn get_context(&self, id: ErrorContextId) -> Option<&ErrorContextImpl> {
        self.contexts.get(&id)
    }

    pub fn get_context_mut(&mut self, id: ErrorContextId) -> Option<&mut ErrorContextImpl> {
        self.contexts.get_mut(&id)
    }

    pub fn remove_context(&mut self, id: ErrorContextId) -> Option<ErrorContextImpl> {
        self.contexts.remove(&id)
    }

    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }
}

impl Default for ErrorContextRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Error context built-ins providing canonical functions
pub struct ErrorContextBuiltins;

impl ErrorContextBuiltins {
    /// Initialize the global error context registry
    pub fn initialize() -> Result<()> {
        let mut registry_ref = ERROR_CONTEXT_REGISTRY.try_borrow_mut()
            .map_err(|_| Error::runtime_invalid_state("Error context registry borrow failed")
            ))?;
        *registry_ref = Some(ErrorContextRegistry::new();
        Ok(()
    }

    /// Get the global registry
    fn with_registry<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&ErrorContextRegistry) -> R,
    {
        let registry_ref = ERROR_CONTEXT_REGISTRY.try_borrow()
            .map_err(|_| Error::runtime_invalid_state("Error context registry borrow failed")
            ))?;
        let registry = registry_ref.as_ref()
            .ok_or_else(|| Error::runtime_invalid_state("Error context registry not initialized")
            ))?;
        Ok(f(registry)
    }

    /// Get the global registry mutably
    fn with_registry_mut<F, R>(f: F) -> Result<R>
    where
        F: FnOnce(&mut ErrorContextRegistry) -> Result<R>,
    {
        let mut registry_ref = ERROR_CONTEXT_REGISTRY.try_borrow_mut()
            .map_err(|_| Error::runtime_invalid_state("Error context registry borrow failed")
            ))?;
        let registry = registry_ref.as_mut()
            .ok_or_else(|| Error::runtime_invalid_state("Error context registry not initialized")
            ))?;
        f(registry)
    }

    /// `error-context.new` canonical built-in
    /// Creates a new error context
    #[cfg(feature = "std")]
    pub fn error_context_new(message: String, severity: ErrorSeverity) -> Result<ErrorContextId> {
        let context = ErrorContextImpl::new(message, severity;
        Self::with_registry_mut(|registry| {
            registry.register_context(context)
        })?
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn error_context_new(message: &str, severity: ErrorSeverity) -> Result<ErrorContextId> {
        let context = ErrorContextImpl::new(message, severity)?;
        Self::with_registry_mut(|registry| {
            registry.register_context(context)
        })?
    }

    /// `error-context.debug-message` canonical built-in
    /// Gets the debug message from an error context
    #[cfg(feature = "std")]
    pub fn error_context_debug_message(context_id: ErrorContextId) -> Result<String> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.debug_message.clone()
            } else {
                String::new()
            }
        })
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn error_context_debug_message(context_id: ErrorContextId) -> Result<BoundedString<MAX_DEBUG_MESSAGE_SIZE>> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.debug_message.clone()
            } else {
                BoundedString::new()
            }
        })
    }

    /// `error-context.drop` canonical built-in
    /// Drops an error context
    pub fn error_context_drop(context_id: ErrorContextId) -> Result<()> {
        Self::with_registry_mut(|registry| {
            registry.remove_context(context_id;
            Ok(()
        })?
    }

    /// Get error context severity
    pub fn error_context_severity(context_id: ErrorContextId) -> Result<ErrorSeverity> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.severity
            } else {
                ErrorSeverity::Error
            }
        })
    }

    /// Get error context error code if set
    pub fn error_context_error_code(context_id: ErrorContextId) -> Result<Option<u32>> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.error_code
            } else {
                None
            }
        })
    }

    /// Get stack trace from error context
    #[cfg(feature = "std")]
    pub fn error_context_stack_trace(context_id: ErrorContextId) -> Result<String> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.format_stack_trace()
            } else {
                String::new()
            }
        })
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn error_context_stack_trace(context_id: ErrorContextId) -> core::result::Result<BoundedString<1024>> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.format_stack_trace()
            } else {
                Ok(BoundedString::new()
            }
        })?
    }

    /// Add a stack frame to an error context
    #[cfg(feature = "std")]
    pub fn error_context_add_stack_frame(
        context_id: ErrorContextId, 
        function_name: String,
        file_name: Option<String>,
        line: Option<u32>,
        column: Option<u32>
    ) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(context) = registry.get_context_mut(context_id) {
                let mut frame = StackFrame::new(function_name;
                if let (Some(file), Some(line_num)) = (file_name, line) {
                    frame = frame.with_location(file, line_num, column.unwrap_or(0;
                }
                context.add_stack_frame(frame;
                Ok(()
            } else {
                Err(Error::runtime_execution_error("Error occurred"
            })?;
            }
        })?
    }

    #[cfg(not(any(feature = "std")))]
    pub fn error_context_add_stack_frame(
        context_id: ErrorContextId, 
        function_name: &str,
        file_name: Option<&str>,
        line: Option<u32>,
        column: Option<u32>
    ) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(context) = registry.get_context_mut(context_id) {
                let mut frame = StackFrame::new(function_name)?;
                if let (Some(file), Some(line_num)) = (file_name, line) {
                    frame = frame.with_location(file, line_num, column.unwrap_or(0))?;
                }
                context.add_stack_frame(frame)?;
                Ok(()
            } else {
                Err(Error::runtime_execution_error("Error occurred"
            })?;
            }
        })?
    }

    /// Set metadata on an error context
    #[cfg(feature = "std")]
    pub fn error_context_set_metadata(
        context_id: ErrorContextId,
        key: String,
        value: ComponentValue
    ) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(context) = registry.get_context_mut(context_id) {
                context.set_metadata(key, value;
                Ok(()
            } else {
                Err(Error::runtime_execution_error("Error occurred"
            })?;
            }
        })?
    }

    #[cfg(not(any(feature = "std")))]
    pub fn error_context_set_metadata(
        context_id: ErrorContextId,
        key: &str,
        value: ComponentValue
    ) -> Result<()> {
        Self::with_registry_mut(|registry| {
            if let Some(context) = registry.get_context_mut(context_id) {
                context.set_metadata(key, value)?;
                Ok(()
            } else {
                Err(Error::runtime_execution_error("Error occurred"
            })?;
            }
        })?
    }

    /// Get metadata from an error context
    pub fn error_context_get_metadata(
        context_id: ErrorContextId,
        key: &str
    ) -> Result<Option<ComponentValue>> {
        Self::with_registry(|registry| {
            if let Some(context) = registry.get_context(context_id) {
                context.get_metadata(key).cloned()
            } else {
                None
            }
        })
    }
}

/// Convenience functions for working with error contexts
pub mod error_context_helpers {
    use super::*;

    /// Create an error context from a standard error
    #[cfg(feature = "std")]
    pub fn from_error(error: &Error) -> Result<ErrorContextId> {
        let message = error.message().to_string());
        let severity = match error.category() {
            ErrorCategory::InvalidInput | ErrorCategory::Type => ErrorSeverity::Warning,
            ErrorCategory::Runtime | ErrorCategory::Memory => ErrorSeverity::Error,
            _ => ErrorSeverity::Critical,
        };
        
        let context_id = ErrorContextBuiltins::error_context_new(message, severity)?;
        ErrorContextBuiltins::error_context_set_metadata(
            context_id,
            "error_code".to_owned(),
            ComponentValue::I32(error.code() as i32)
        )?;
        Ok(context_id)
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn from_error(error: &Error) -> Result<ErrorContextId> {
        let severity = match error.category() {
            ErrorCategory::InvalidInput | ErrorCategory::Type => ErrorSeverity::Warning,
            ErrorCategory::Runtime | ErrorCategory::Memory => ErrorSeverity::Error,
            _ => ErrorSeverity::Critical,
        };
        
        let context_id = ErrorContextBuiltins::error_context_new(error.message(), severity)?;
        ErrorContextBuiltins::error_context_set_metadata(
            context_id,
            "error_code",
            ComponentValue::I32(error.code() as i32)
        )?;
        Ok(context_id)
    }

    /// Create a simple error context with just a message
    #[cfg(feature = "std")]
    pub fn create_simple(message: String) -> Result<ErrorContextId> {
        ErrorContextBuiltins::error_context_new(message, ErrorSeverity::Error)
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn create_simple(message: &str) -> Result<ErrorContextId> {
        ErrorContextBuiltins::error_context_new(message, ErrorSeverity::Error)
    }

    /// Create an error context with stack trace
    #[cfg(feature = "std")]
    pub fn create_with_stack_trace(
        message: String, 
        function_name: String,
        file_name: Option<String>,
        line: Option<u32>
    ) -> Result<ErrorContextId> {
        let context_id = ErrorContextBuiltins::error_context_new(message, ErrorSeverity::Error)?;
        ErrorContextBuiltins::error_context_add_stack_frame(
            context_id, 
            function_name, 
            file_name, 
            line, 
            None
        )?;
        Ok(context_id)
    }

    #[cfg(not(any(feature = "std", )))]
    pub fn create_with_stack_trace(
        message: &str, 
        function_name: &str,
        file_name: Option<&str>,
        line: Option<u32>
    ) -> Result<ErrorContextId> {
        let context_id = ErrorContextBuiltins::error_context_new(message, ErrorSeverity::Error)?;
        ErrorContextBuiltins::error_context_add_stack_frame(
            context_id, 
            function_name, 
            file_name, 
            line, 
            None
        )?;
        Ok(context_id)
    }

}
