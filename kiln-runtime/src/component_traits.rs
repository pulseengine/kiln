use crate::prelude::*;
use wrt_foundation::{
    safe_memory::{SafeStack, SafeSlice},
    Value, VerificationLevel,
};

// Type aliases with proper memory provider
pub type ComponentType = wrt_foundation::component::ComponentType<wrt_foundation::safe_memory::NoStdProvider<1024>>;
pub type ExternType = wrt_foundation::component::ExternType<wrt_foundation::safe_memory::NoStdProvider<1024>>;
pub type SafeStackValue = wrt_foundation::safe_memory::SafeStack<Value, 64, wrt_foundation::safe_memory::NoStdProvider<1024>>;
pub type FuncType = wrt_foundation::types::FuncType;

/// Represents a runtime component instance
#[cfg(feature = "std")]
pub trait ComponentInstance {
    /// Execute a function by name with the given arguments
    fn execute_function(&self, name: &str, args: &[Value]) -> Result<SafeStackValue>;

    /// Read from exported memory
    fn read_memory(&self, name: &str, offset: u32, size: u32) -> Result<SafeSlice<'_>>;

    /// Write to exported memory
    fn write_memory(&mut self, name: &str, offset: u32, bytes: &[u8]) -> Result<()>;

    /// Get the type of an export
    fn get_export_type(&self, name: &str) -> Result<ExternType>;


}

/// Represents a host function implementation
#[cfg(feature = "std")]
pub trait HostFunction {
    /// Call the host function with the given arguments
    fn call(&self, args: &[Value]) -> Result<SafeStackValue>;

    /// Get the function's type
    fn get_type(&self) -> FuncType;

}

/// Represents a host function factory
#[cfg(feature = "std")]
pub trait HostFunctionFactory {
    /// Create a host function implementation
    fn create_function(&self, name: &str, ty: &FuncType) -> Result<Box<dyn HostFunction>>;
}

// Re-export HostImportHandler from wrt-foundation for convenience
#[cfg(feature = "std")]
pub use wrt_foundation::HostImportHandler;

/// Represents a component runtime environment
#[cfg(feature = "std")]
pub trait ComponentRuntime {
    /// Create a new runtime instance
    fn new() -> Self
    where
        Self: Sized;

    /// Register a host function factory
    fn register_host_factory(&mut self, factory: Box<dyn HostFunctionFactory>);

    /// Instantiate a component
    fn instantiate(&self, component_type: &ComponentType) -> Result<Box<dyn ComponentInstance>>;

    /// Register a specific host function
    fn register_host_function<F>(&mut self, name: &str, ty: FuncType, function: F) -> Result<()>
    where
        F: Fn(&[Value]) -> Result<wrt_foundation::bounded::BoundedVec<Value, 16, wrt_foundation::safe_memory::NoStdProvider<1024>>> + 'static + Send + Sync;

    /// Set the verification level for memory operations
    fn set_verification_level(&mut self, level: VerificationLevel) -> Result<()>;

    /// Get the current verification level
    fn verification_level(&self) -> VerificationLevel;
}
