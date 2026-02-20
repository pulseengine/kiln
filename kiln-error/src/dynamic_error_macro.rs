// Dynamic error creation macro for cases where category/code are runtime values

/// Macro to create errors with dynamic category and code
/// 
/// This macro helps migrate patterns where error category and code
/// are determined at runtime, which can't use const fn factory methods.
///
/// # Examples
/// ```
/// use wrt_error::{create_error, ErrorCategory, codes};
/// 
/// let category = ErrorCategory::Memory;
/// let code = codes::MEMORY_ERROR;
/// let error = create_error!(category, code, "Dynamic error";
/// ```
#[macro_export]
macro_rules! create_error {
    ($category:expr, $code:expr, $message:expr) => {{
        use $crate::{Error, ErrorCategory, codes};
        
        // Match on category and code to use appropriate factory method
        match ($category, $code) {
            // Memory errors
            (ErrorCategory::Memory, codes::MEMORY_ERROR) => Error::memory_error($message),
            (ErrorCategory::Memory, codes::MEMORY_ALLOCATION_FAILED) => Error::memory_allocation_failed($message),
            (ErrorCategory::Memory, codes::OUT_OF_BOUNDS_ERROR) => Error::out_of_bounds($message),
            (ErrorCategory::Memory, codes::MEMORY_OUT_OF_BOUNDS) => Error::memory_out_of_bounds($message),
            
            // Runtime errors
            (ErrorCategory::Runtime, codes::EXECUTION_ERROR) => Error::runtime_execution_error($message),
            (ErrorCategory::Runtime, codes::STACK_OVERFLOW) => Error::runtime_stack_overflow($message),
            (ErrorCategory::Runtime, codes::STACK_UNDERFLOW) => Error::runtime_stack_underflow($message),
            (ErrorCategory::Runtime, codes::FUNCTION_NOT_FOUND) => Error::function_not_found($message),
            
            // Resource errors
            (ErrorCategory::Resource, codes::RESOURCE_EXHAUSTED) => Error::resource_exhausted($message),
            (ErrorCategory::Resource, codes::RESOURCE_NOT_FOUND) => Error::resource_not_found($message),
            (ErrorCategory::Resource, codes::INVALID_FUNCTION_INDEX) => Error::invalid_function_index($message),
            
            // Type errors
            (ErrorCategory::Type, codes::TYPE_MISMATCH_ERROR) => Error::type_mismatch_error($message),
            (ErrorCategory::Type, codes::INVALID_TYPE) => Error::invalid_type_error($message),
            (ErrorCategory::Type, codes::TYPE_CONVERSION_ERROR) => Error::type_conversion_error($message),
            
            // Safety errors
            (ErrorCategory::Safety, codes::SAFETY_VIOLATION) => Error::safety_violation($message),
            (ErrorCategory::Safety, codes::CFI_VIOLATION) => Error::cfi_violation($message),
            
            // Component errors
            (ErrorCategory::Component, codes::COMPONENT_NOT_FOUND) => Error::component_not_found($message),
            (ErrorCategory::Component, codes::COMPONENT_INSTANTIATION_ERROR) => Error::component_instantiation_error($message),
            
            // Async errors
            (ErrorCategory::AsyncRuntime, codes::ASYNC_ERROR) => Error::async_error($message),
            
            // System errors
            (ErrorCategory::System, codes::SYSTEM_ERROR) => Error::system_error($message),
            (ErrorCategory::System, codes::CONFIGURATION_ERROR) => Error::configuration_error($message),
            
            // Parse errors
            (ErrorCategory::Parse, codes::PARSE_ERROR) => Error::parse_error($message),
            
            // Validation errors
            (ErrorCategory::Validation, codes::INVALID_VALUE) => Error::invalid_value($message),
            (ErrorCategory::Validation, codes::VALIDATION_ERROR) => Error::validation_error($message),
            
            // Timeout errors
            (ErrorCategory::Runtime, codes::OPERATION_TIMEOUT) => Error::timeout_error($message),
            
            // Additional factory method mappings
            (ErrorCategory::Capacity, codes::CAPACITY_LIMIT_EXCEEDED) => Error::capacity_limit_exceeded($message),
            (ErrorCategory::Component, codes::DUPLICATE_OPERATION) => Error::component_already_exists($message),
            (ErrorCategory::Component, codes::COMPONENT_LINKING_ERROR) => Error::component_linking_error($message),
            (ErrorCategory::RuntimeTrap, codes::RUNTIME_TRAP_ERROR) => Error::runtime_trap_error($message),
            (ErrorCategory::RuntimeTrap, codes::INTEGER_OVERFLOW) => Error::trap_integer_overflow($message),
            (ErrorCategory::RuntimeTrap, codes::DIVISION_BY_ZERO) => Error::trap_divide_by_zero($message),
            (ErrorCategory::Platform, codes::MEMORY_ERROR) => Error::platform_memory_error($message),
            (ErrorCategory::Platform, codes::THREADING_ERROR) => Error::platform_thread_error($message),
            (ErrorCategory::Platform, codes::PLATFORM_ERROR) => Error::platform_error($message),
            (ErrorCategory::Security, codes::ACCESS_DENIED) => Error::access_denied($message),
            (ErrorCategory::Security, codes::SECURITY_VIOLATION) => Error::security_violation($message),
            (ErrorCategory::Initialization, codes::INITIALIZATION_ERROR) => Error::initialization_error($message),
            (ErrorCategory::NotSupported, codes::UNSUPPORTED) => Error::not_supported($message),
            (ErrorCategory::NotSupported, codes::VALIDATION_UNSUPPORTED_FEATURE) => Error::feature_not_supported($message),
            (ErrorCategory::Runtime, codes::RUNTIME_ERROR) => Error::runtime_error($message),
            (ErrorCategory::Runtime, codes::INVALID_STATE) => Error::invalid_state_error($message),
            (ErrorCategory::Validation, codes::VALIDATION_FAILURE) => Error::validation_failed($message),
            (ErrorCategory::Validation, codes::INVALID_ARGUMENT) => Error::invalid_argument($message),
            (ErrorCategory::Type, codes::TYPE_ERROR) => Error::type_error($message),
            (ErrorCategory::Type, codes::CONVERSION_ERROR) => Error::conversion_error($message),
            (ErrorCategory::Memory, codes::MEMORY_CORRUPTION_DETECTED) => Error::memory_corruption_detected($message),
            (ErrorCategory::Memory, codes::BUFFER_TOO_SMALL) => Error::buffer_overflow($message),
            (ErrorCategory::Resource, codes::RESOURCE_ERROR) => Error::resource_error($message),
            (ErrorCategory::Resource, codes::RESOURCE_LIMIT_EXCEEDED) => Error::resource_limit_exceeded($message),
            (ErrorCategory::Io, codes::IO_ERROR) => Error::io_error($message),
            (ErrorCategory::Io, codes::RESOURCE_NOT_FOUND) => Error::file_not_found($message),
            
            // For any unmatched combination, fall back to runtime_execution_error
            // This ensures the macro always produces a valid error
            _ => Error::runtime_execution_error($message),
        }
    }};
}

/// Helper trait for types that can be converted to Error
/// 
/// This helps migrate patterns where custom error types are used
pub trait IntoWrtError {
    /// Convert to a WRT Error using appropriate factory method
    fn into_wrt_error(self) -> Error;
}

// Example implementations for common patterns
#[cfg(feature = "std")]
impl IntoWrtError for std::io::Error {
    fn into_wrt_error(self) -> Error {
        match self.kind() {
            std::io::ErrorKind::OutOfMemory => Error::memory_allocation_failed("IO out of memory"),
            std::io::ErrorKind::NotFound => Error::resource_not_found("IO resource not found"),
            std::io::ErrorKind::PermissionDenied => Error::access_denied("IO permission denied"),
            std::io::ErrorKind::TimedOut => Error::timeout_error("IO operation timed out"),
            _ => Error::system_error("IO error"),
        }
    }
}

// Helper for threading errors seen in the codebase
pub struct ThreadingError(pub String;

impl IntoWrtError for ThreadingError {
    fn into_wrt_error(self) -> Error {
        Error::threading_error("Threading error")
    }
}

// Helper for async errors seen in the codebase  
pub struct AsyncError(pub String;

impl IntoWrtError for AsyncError {
    fn into_wrt_error(self) -> Error {
        Error::async_error("Async error")
    }
}