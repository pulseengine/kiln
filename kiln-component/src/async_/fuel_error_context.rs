//! Error context propagation for async operations
//!
//! This module provides error context tracking and propagation across
//! async boundaries, enabling detailed error reporting with fuel costs.

use core::fmt::{self, Display};

use wrt_foundation::{
    CrateId,
    bounded::BoundedString,
    collections::StaticVec as BoundedVec,
    operations::{Type as OperationType, record_global_operation},
    safe_managed_alloc,
    traits::{Checksummable, FromBytes, ToBytes},
    verification::VerificationLevel,
};

use crate::{
    async_::fuel_async_executor::{AsyncTaskState, FuelAsyncTask},
    prelude::*,
};

/// Maximum error context chain depth
const MAX_ERROR_CONTEXT_DEPTH: usize = 16;

/// Maximum length for error messages
const MAX_ERROR_MESSAGE_LENGTH: usize = 256;

/// Fuel costs for error operations
const ERROR_CREATE_FUEL: u64 = 5;
const ERROR_CHAIN_FUEL: u64 = 3;
const ERROR_CONTEXT_FUEL: u64 = 2;

/// Error context information
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Component that generated the error
    pub component_id: u64,
    /// Task that was executing when error occurred
    pub task_id: Option<u64>,
    /// Location in the code (file:line)
    pub location: BoundedString<128>,
    /// Additional context information
    pub context: BoundedString<MAX_ERROR_MESSAGE_LENGTH>,
    /// Fuel consumed up to this error
    pub fuel_consumed: u64,
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            component_id: 0,
            task_id: None,
            location: BoundedString::from_str_truncate("")
                .unwrap_or_else(|_| panic!("Failed to create default location")),
            context: BoundedString::from_str_truncate("")
                .unwrap_or_else(|_| panic!("Failed to create default context")),
            fuel_consumed: 0,
        }
    }
}

impl PartialEq for ErrorContext {
    fn eq(&self, other: &Self) -> bool {
        self.component_id == other.component_id && self.task_id == other.task_id
    }
}

impl Eq for ErrorContext {}

impl Checksummable for ErrorContext {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.component_id.update_checksum(checksum);
        self.fuel_consumed.update_checksum(checksum);
    }
}

impl ToBytes for ErrorContext {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.component_id.to_bytes_with_provider(writer, provider)?;
        self.fuel_consumed.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for ErrorContext {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        _reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self::default())
    }
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(
        component_id: u64,
        task_id: Option<u64>,
        location: &str,
        context: &str,
        fuel_consumed: u64,
    ) -> Result<Self> {
        let location_provider = safe_managed_alloc!(512, CrateId::Component)?;
        let context_provider = safe_managed_alloc!(2048, CrateId::Component)?;

        let bounded_location = BoundedString::from_str_truncate(location).map_err(|_| {
            wrt_error::Error::runtime_execution_error("Failed to create location string")
        })?;

        let bounded_context = BoundedString::from_str_truncate(context).map_err(|_| {
            wrt_error::Error::runtime_execution_error("Failed to create context string")
        })?;

        Ok(Self {
            component_id,
            task_id,
            location: bounded_location,
            context: bounded_context,
            fuel_consumed,
        })
    }
}

/// Enhanced error type with context chain
#[derive(Debug)]
pub struct ContextualError {
    /// The original error
    pub error: Error,
    /// Chain of error contexts
    pub context_chain: BoundedVec<ErrorContext, MAX_ERROR_CONTEXT_DEPTH>,
    /// Total fuel consumed across all contexts
    pub total_fuel_consumed: u64,
    /// Verification level for fuel tracking
    pub verification_level: VerificationLevel,
}

impl ContextualError {
    /// Create a new contextual error
    pub fn new(error: Error, verification_level: VerificationLevel) -> Result<Self> {
        let context_chain = BoundedVec::new();

        // Record error creation
        record_global_operation(OperationType::Other, verification_level);

        Ok(Self {
            error,
            context_chain,
            total_fuel_consumed: ERROR_CREATE_FUEL,
            verification_level,
        })
    }

    /// Add context to the error
    pub fn with_context(mut self, context: ErrorContext) -> Result<Self> {
        // Consume fuel for adding context
        let fuel_cost =
            OperationType::fuel_cost_for_operation(OperationType::Other, self.verification_level)?;

        self.total_fuel_consumed = self
            .total_fuel_consumed
            .saturating_add(ERROR_CONTEXT_FUEL)
            .saturating_add(fuel_cost);

        // Add to context chain
        self.context_chain.push(context)?;

        Ok(self)
    }

    /// Chain another error
    pub fn chain(mut self, other: ContextualError) -> Result<Self> {
        // Consume fuel for chaining
        let fuel_cost =
            OperationType::fuel_cost_for_operation(OperationType::Other, self.verification_level)?;

        self.total_fuel_consumed = self
            .total_fuel_consumed
            .saturating_add(ERROR_CHAIN_FUEL)
            .saturating_add(fuel_cost)
            .saturating_add(other.total_fuel_consumed);

        // Merge context chains
        for context in other.context_chain.iter() {
            if self.context_chain.len() < MAX_ERROR_CONTEXT_DEPTH {
                self.context_chain.push(context.clone())?;
            }
        }

        Ok(self)
    }

    /// Get the root cause error
    pub fn root_cause(&self) -> &Error {
        &self.error
    }

    /// Get the most recent context
    pub fn latest_context(&self) -> Option<&ErrorContext> {
        self.context_chain.last()
    }

    /// Format the error with full context chain
    pub fn format_with_context(&self) -> Result<String> {
        let mut output = String::new();

        // Start with the main error
        output.push_str(&format!("Error: {}\n", self.error.message()));

        // Add context chain
        if !self.context_chain.is_empty() {
            output.push_str("\nError Context Chain:\n");
            for (i, context) in self.context_chain.iter().enumerate() {
                output.push_str(&format!(
                    "  [{}] Component {}, Task {:?}\n",
                    i, context.component_id, context.task_id
                ));
                output.push_str(&format!(
                    "      Location: {}\n",
                    context.location.as_str().unwrap_or("<invalid>")
                ));
                output.push_str(&format!(
                    "      Context: {}\n",
                    context.context.as_str().unwrap_or("<invalid>")
                ));
                output.push_str(&format!("      Fuel consumed: {}\n", context.fuel_consumed));
            }
        }

        // Add total fuel consumed
        output.push_str(&format!(
            "\nTotal fuel consumed: {}\n",
            self.total_fuel_consumed
        ));

        Ok(output)
    }
}

impl Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error.message())?;

        if let Some(context) = self.latest_context() {
            write!(
                f,
                " (in component {}, {})",
                context.component_id,
                context.context.as_str().unwrap_or("<invalid>")
            )?;
        }

        Ok(())
    }
}

impl From<ContextualError> for Error {
    fn from(contextual: ContextualError) -> Self {
        contextual.error
    }
}

/// Error propagation helper for async operations
pub struct ErrorPropagator {
    /// Current component ID
    component_id: u64,
    /// Current task ID
    task_id: Option<u64>,
    /// Verification level
    verification_level: VerificationLevel,
}

impl ErrorPropagator {
    /// Create a new error propagator
    pub fn new(
        component_id: u64,
        task_id: Option<u64>,
        verification_level: VerificationLevel,
    ) -> Self {
        Self {
            component_id,
            task_id,
            verification_level,
        }
    }

    /// Wrap an error with context
    pub fn wrap_error(
        &self,
        error: Error,
        location: &str,
        context: &str,
        fuel_consumed: u64,
    ) -> Result<ContextualError> {
        let mut contextual = ContextualError::new(error, self.verification_level)?;

        let error_context = ErrorContext::new(
            self.component_id,
            self.task_id,
            location,
            context,
            fuel_consumed,
        )?;

        contextual.with_context(error_context)
    }

    /// Propagate an error through a component boundary
    pub fn propagate(
        &self,
        error: ContextualError,
        boundary: &str,
        fuel_consumed: u64,
    ) -> Result<ContextualError> {
        let context = ErrorContext::new(
            self.component_id,
            self.task_id,
            boundary,
            "Error propagated through component boundary",
            fuel_consumed,
        )?;

        error.with_context(context)
    }
}

/// Trait for adding error context to Results
pub trait ErrorContextExt<T> {
    /// Add context to an error
    fn context(self, context: &str) -> Result<T>;

    /// Add context with location
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T> ErrorContextExt<T> for Result<T> {
    fn context(self, context: &str) -> Result<T> {
        self.map_err(|e| {
            // Use a generic error message since we can't format with dynamic strings
            Error::runtime_execution_error("Error with context")
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self
    }
}

/// Async-specific error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncErrorKind {
    /// Task was cancelled
    TaskCancelled,
    /// Task exceeded fuel budget
    FuelExhausted,
    /// Task deadlocked
    Deadlock,
    /// Task panicked
    Panic,
    /// Stream closed unexpectedly
    StreamClosed,
    /// Future timed out
    Timeout,
    /// Component not found
    ComponentNotFound,
    /// Resource limit exceeded
    ResourceLimit,
}

impl AsyncErrorKind {
    /// Convert to error code
    pub fn to_code(self) -> u16 {
        match self {
            Self::TaskCancelled => codes::ASYNC_CANCELLED,
            Self::FuelExhausted => codes::RESOURCE_LIMIT_EXCEEDED,
            Self::Deadlock => codes::ASYNC_DEADLOCK,
            Self::Panic => codes::ASYNC_PANIC,
            Self::StreamClosed => codes::ASYNC_STREAM_CLOSED,
            Self::Timeout => codes::ASYNC_TIMEOUT,
            Self::ComponentNotFound => codes::COMPONENT_NOT_FOUND,
            Self::ResourceLimit => codes::RESOURCE_LIMIT_EXCEEDED,
        }
    }

    /// Get description
    pub fn description(self) -> &'static str {
        match self {
            Self::TaskCancelled => "Async task was cancelled",
            Self::FuelExhausted => "Task fuel budget exhausted",
            Self::Deadlock => "Async deadlock detected",
            Self::Panic => "Async task panicked",
            Self::StreamClosed => "Stream closed unexpectedly",
            Self::Timeout => "Async operation timed out",
            Self::ComponentNotFound => "Component not found",
            Self::ResourceLimit => "Resource limit exceeded",
        }
    }
}

/// Create an async error with context
pub fn async_error(
    kind: AsyncErrorKind,
    component_id: u64,
    task_id: Option<u64>,
    additional_context: &str,
) -> Result<ContextualError> {
    let error = Error::new(ErrorCategory::Async, kind.to_code(), kind.description());

    let mut contextual = ContextualError::new(error, VerificationLevel::Basic)?;

    let context = ErrorContext::new(
        component_id,
        task_id,
        "async operation",
        additional_context,
        0,
    )?;

    contextual.with_context(context)
}
