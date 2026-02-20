//! Bounded Infrastructure for Host
//!
//! This module provides bounded alternatives for host collections
//! to ensure static memory allocation throughout the host interface.

#[cfg(any(feature = "std", feature = "alloc"))]
use wrt_foundation::capabilities::CapabilityAwareProvider;
use wrt_foundation::{
    bounded::{
        BoundedString,
        BoundedVec,
    },
    bounded_collections::BoundedMap as BoundedHashMap,
    capability_context,
    safe_capability_alloc,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    CrateId,
};

/// Budget-aware memory size for host (64KB)
pub const HOST_MEMORY_SIZE: usize = 65536;

/// Default provider type for host (always use NoStdProvider for consistency)
pub type HostProvider = NoStdProvider<HOST_MEMORY_SIZE>;

/// Maximum number of host functions
pub const MAX_HOST_FUNCTIONS: usize = 1024;

/// Maximum number of callbacks
pub const MAX_CALLBACKS: usize = 256;

/// Maximum number of host modules
pub const MAX_HOST_MODULES: usize = 64;

/// Maximum host function name length
pub const MAX_HOST_FUNCTION_NAME_LEN: usize = 256;

/// Maximum host module name length
pub const MAX_HOST_MODULE_NAME_LEN: usize = 128;

/// Maximum host ID length
pub const MAX_HOST_ID_LEN: usize = 128;

/// Maximum number of host instances
pub const MAX_HOST_INSTANCES: usize = 32;

/// Maximum number of function arguments
pub const MAX_FUNCTION_ARGS: usize = 128;

/// Maximum number of function results
pub const MAX_FUNCTION_RESULTS: usize = 128;

/// Maximum number of environment variables
pub const MAX_ENV_VARS: usize = 256;

/// Create a provider for host operations (unified implementation)
pub fn create_host_provider() -> wrt_error::Result<HostProvider> {
    // Use the standardized provider for consistency
    safe_managed_alloc!(HOST_MEMORY_SIZE, CrateId::Host).map_err(|_| {
        wrt_error::Error::platform_memory_allocation_failed("Failed to allocate host provider")
    })
}

/// Maximum environment variable name length
pub const MAX_ENV_VAR_NAME_LEN: usize = 256;

/// Maximum environment variable value length
pub const MAX_ENV_VAR_VALUE_LEN: usize = 1024;

/// Maximum number of host resource handles
pub const MAX_HOST_RESOURCE_HANDLES: usize = 1024;

/// Maximum number of function pointers
pub const MAX_FUNCTION_POINTERS: usize = 512;

/// Bounded vector for host functions
pub type BoundedHostFunctionVec<T> = BoundedVec<T, MAX_HOST_FUNCTIONS, HostProvider>;

/// Bounded vector for callbacks
pub type BoundedCallbackVec<T> = BoundedVec<T, MAX_CALLBACKS, HostProvider>;

/// Bounded vector for host modules
pub type BoundedHostModuleVec<T> = BoundedVec<T, MAX_HOST_MODULES, HostProvider>;

/// Bounded string for host function names
pub type BoundedHostFunctionName = BoundedString<MAX_HOST_FUNCTION_NAME_LEN>;

/// Bounded string for host module names
pub type BoundedHostModuleName = BoundedString<MAX_HOST_MODULE_NAME_LEN>;

/// Bounded string for host ID
pub type BoundedHostId = BoundedString<MAX_HOST_ID_LEN>;

/// Bounded vector for host instances
pub type BoundedHostInstanceVec<T> = BoundedVec<T, MAX_HOST_INSTANCES, HostProvider>;

/// Bounded vector for function arguments
pub type BoundedArgsVec<T> = BoundedVec<T, MAX_FUNCTION_ARGS, HostProvider>;

/// Bounded vector for function results
pub type BoundedResultsVec<T> = BoundedVec<T, MAX_FUNCTION_RESULTS, HostProvider>;

/// Bounded map for host functions
pub type BoundedHostFunctionMap<V> =
    BoundedHashMap<BoundedHostFunctionName, V, MAX_HOST_FUNCTIONS, HostProvider>;

/// Bounded map for callbacks
pub type BoundedCallbackMap<V> = BoundedHashMap<
    u32, // Callback ID
    V,
    MAX_CALLBACKS,
    HostProvider,
>;

/// Bounded map for environment variables
pub type BoundedEnvMap = BoundedHashMap<
    BoundedString<MAX_ENV_VAR_NAME_LEN>,
    BoundedString<MAX_ENV_VAR_VALUE_LEN>,
    MAX_ENV_VARS,
    HostProvider,
>;

/// Bounded vector for host resource handles
pub type BoundedHostResourceVec<T> = BoundedVec<T, MAX_HOST_RESOURCE_HANDLES, HostProvider>;

/// Bounded vector for function pointers
pub type BoundedFunctionPointerVec<T> = BoundedVec<T, MAX_FUNCTION_POINTERS, HostProvider>;

/// Create a new bounded host function vector
pub fn new_host_function_vec<T>() -> wrt_error::Result<BoundedHostFunctionVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded callback vector
pub fn new_callback_vec<T>() -> wrt_error::Result<BoundedCallbackVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded host module vector
pub fn new_host_module_vec<T>() -> wrt_error::Result<BoundedHostModuleVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded host function name
pub fn new_host_function_name() -> wrt_error::Result<BoundedHostFunctionName> {
    let provider = create_host_provider()?;
    BoundedString::<MAX_HOST_FUNCTION_NAME_LEN>::try_from_str("")
        .map_err(|_| wrt_error::Error::platform_memory_allocation_failed("Failed to create empty bounded string"))
}

/// Create a bounded host function name from str
pub fn bounded_host_function_name_from_str(s: &str) -> wrt_error::Result<BoundedHostFunctionName> {
    let provider = create_host_provider()?;
    BoundedString::<MAX_HOST_FUNCTION_NAME_LEN>::try_from_str(s)
        .map_err(|_| wrt_error::Error::validation_error("String too long for bounded host function name"))
}

/// Create a new bounded host module name
pub fn new_host_module_name() -> wrt_error::Result<BoundedHostModuleName> {
    let provider = create_host_provider()?;
    BoundedString::<MAX_HOST_MODULE_NAME_LEN>::try_from_str("")
        .map_err(|_| wrt_error::Error::platform_memory_allocation_failed("Failed to create empty bounded string"))
}

/// Create a bounded host module name from str
pub fn bounded_host_module_name_from_str(s: &str) -> wrt_error::Result<BoundedHostModuleName> {
    let provider = create_host_provider()?;
    BoundedString::<MAX_HOST_MODULE_NAME_LEN>::try_from_str(s)
        .map_err(|_| wrt_error::Error::validation_error("String too long for bounded host module name"))
}

/// Create a new bounded host ID
pub fn new_host_id() -> wrt_error::Result<BoundedHostId> {
    let provider = create_host_provider()?;
    BoundedString::<MAX_HOST_ID_LEN>::try_from_str("")
        .map_err(|_| wrt_error::Error::platform_memory_allocation_failed("Failed to create empty bounded string"))
}

/// Create a bounded host ID from str
pub fn bounded_host_id_from_str(s: &str) -> wrt_error::Result<BoundedHostId> {
    let provider = create_host_provider()?;
    BoundedString::<MAX_HOST_ID_LEN>::try_from_str(s)
        .map_err(|_| wrt_error::Error::validation_error("String too long for bounded host ID"))
}

/// Create a new bounded host instance vector
pub fn new_host_instance_vec<T>() -> wrt_error::Result<BoundedHostInstanceVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded args vector
pub fn new_args_vec<T>() -> wrt_error::Result<BoundedArgsVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded results vector
pub fn new_results_vec<T>() -> wrt_error::Result<BoundedResultsVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded host function map
pub fn new_host_function_map<V>() -> wrt_error::Result<BoundedHostFunctionMap<V>>
where
    V: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedHashMap::new(provider)
}

/// Create a new bounded callback map
pub fn new_callback_map<V>() -> wrt_error::Result<BoundedCallbackMap<V>>
where
    V: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedHashMap::new(provider)
}

/// Create a new bounded environment map
pub fn new_env_map() -> wrt_error::Result<BoundedEnvMap> {
    let provider = create_host_provider()?;
    BoundedHashMap::new(provider)
}

/// Create a new bounded host resource vector
pub fn new_host_resource_vec<T>() -> wrt_error::Result<BoundedHostResourceVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}

/// Create a new bounded function pointer vector
pub fn new_function_pointer_vec<T>() -> wrt_error::Result<BoundedFunctionPointerVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = create_host_provider()?;
    BoundedVec::new(provider)
}
