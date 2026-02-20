// Allow common WASI-related casts which are expected in syscall interfaces
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::needless_borrow)]

//! # WRT WASI Implementation
//!
//! WASI (WebAssembly System Interface) Preview2 implementation for the WRT
//! WebAssembly runtime. This crate provides WASI host functions that integrate
//! seamlessly with the WRT component model and resource management system.
//!
//! ## Features
//!
//! - **WASI Preview2**: Complete implementation of WASI Preview2 interfaces
//! - **Component Model**: Native integration with WebAssembly Component Model
//! - **Resource Management**: Built on WRT's proven resource management
//!   patterns
//! - **Memory Safety**: Uses WRT's safe memory allocation system
//! - **Platform Abstraction**: Works across `std/no_std` environments
//! - **Preview3 Preparation**: Foundation for future WASI Preview3 features
//!
//! ## Supported WASI Interfaces
//!
//! ### WASI Preview2
//! - `wasi:filesystem` - File and directory operations
//! - `wasi:cli` - Command line arguments and environment variables
//! - `wasi:clocks` - Time and monotonic clock access
//! - `wasi:io` - Stream I/O operations
//! - `wasi:random` - Random number generation
//!
//! ### Future (Preview3 Preparation)
//! - `wasi:sockets` - Network socket operations
//! - Async/await support
//! - Threading primitives
//!
//! ## Usage
//!
//! ```rust,ignore
//! use wrt_wasi::{WasiCapabilities, ComponentModelProvider};
//! use wrt_component::ComponentLinker;
//!
//! // Create WASI capabilities
//! let mut capabilities = WasiCapabilities::minimal();
//! capabilities.filesystem.add_allowed_path("/tmp");
//! capabilities.environment.args_access = true;
//!
//! // Create WASI provider
//! let provider = ComponentModelProvider::new(capabilities)?;
//!
//! // Link with component
//! let mut linker = ComponentLinker::new()?;
//! linker.link_wasi_provider(&provider)?;
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Re-export core WRT types for convenience
pub use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
// Safety configuration for WASI
use wrt_foundation::safety_features::runtime;
pub use wrt_foundation::{
    resource::Resource,
    MemoryProvider,
};

// Re-export safety-aware allocation macros
pub use wrt_foundation::{
    safe_managed_alloc,
    safety_aware_alloc,
    CrateId,
};
pub use wrt_host::{
    CallbackRegistry,
    HostFunctionHandler,
};

/// WASI-specific crate ID for memory allocation tracking
pub const WASI_CRATE_ID: CrateId = CrateId::Wasi;

/// Get the current safety level for WASI operations
#[must_use] 
pub const fn wasi_safety_level() -> &'static str {
    runtime::current_safety_level()
}

/// Get maximum allocation size for WASI operations based on safety level
#[must_use] 
pub const fn wasi_max_allocation_size() -> usize {
    runtime::max_allocation_size()
}

// Temporary component model values
#[cfg(feature = "std")]
pub mod component_values;
pub mod value_compat;

// Unified WASI dispatcher
#[cfg(feature = "preview2")]
pub mod dispatcher;
#[cfg(feature = "preview2")]
pub use dispatcher::WasiDispatcher;
// Re-export types for two-phase dispatch protocol
#[cfg(feature = "preview2")]
pub use dispatcher::{AllocationRequest, DispatchResult};
// Re-export global args/env functions for use by wrtd
#[cfg(all(feature = "preview2", feature = "std"))]
pub use dispatcher::{
    set_global_wasi_args,
    get_global_wasi_args,
    set_global_wasi_env,
};
// Capability-aware value system
pub mod value_capability_aware;

// Re-export the Value type for compatibility
// Re-export the capability-aware value type
pub use value_capability_aware::CapabilityAwareValue;
pub use value_compat::Value;

// WASI Preview2 interfaces
#[cfg(feature = "preview2")]
pub mod preview2 {
    //! WASI Preview2 interface implementations

    #[cfg(feature = "wasi-filesystem")]
    pub mod filesystem;

    #[cfg(feature = "wasi-cli")]
    pub mod cli;

    #[cfg(feature = "wasi-cli")]
    pub mod cli_capability_aware;

    #[cfg(feature = "wasi-clocks")]
    pub mod clocks;

    #[cfg(feature = "wasi-io")]
    pub mod io;

    #[cfg(feature = "wasi-random")]
    pub mod random;

    #[cfg(feature = "wasi-sockets")]
    pub mod sockets;
}

// Preview3 preparation layer
#[cfg(feature = "preview3-prep")]
pub mod preview3 {
    //! WASI Preview3 preparation layer
    //!
    //! This module provides the foundation for future WASI Preview3 features
    //! including async/await, threading, and advanced I/O operations.

    pub mod preparation;
}

// Host provider for component model integration
pub mod host_provider {
    //! Host provider implementations for WASI integration

    pub mod component_model_provider;
    pub mod resource_manager;
}

// Import ExternType for no_std
#[cfg(not(feature = "std"))]
use host_provider::component_model_provider::ExternType;

// WASI capabilities and security model
pub mod capabilities;

// Neural network support (preview-agnostic)
#[cfg(feature = "wasi-nn")]
pub mod nn;

// WIT interface bindings
#[cfg(feature = "preview2")]
pub mod wit_bindings;

// Re-export main types for convenience
#[cfg(feature = "wasi-nn")]
pub use capabilities::WasiNeuralNetworkCapabilities;
pub use capabilities::{
    WasiCapabilities,
    WasiEnvironmentCapabilities,
    WasiFileSystemCapabilities,
};
#[cfg(feature = "preview2")]
pub use host_provider::component_model_provider::{
    ComponentModelProvider,
    WasiProviderBuilder,
};
#[cfg(feature = "preview2")]
pub use host_provider::resource_manager::WasiResourceManager;

/// WASI version enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasiVersion {
    /// WASI Preview2 (component model)
    Preview2,
    /// WASI Preview3 (future - async/threading)
    #[cfg(feature = "preview3-prep")]
    Preview3,
}

impl Default for WasiVersion {
    fn default() -> Self {
        Self::Preview2
    }
}

/// WASI host provider trait for integration with different execution engines
pub trait WasiHostProvider {
    /// Get the number of functions provided
    fn function_count(&self) -> usize;

    /// Get the WASI version supported by this provider
    fn version(&self) -> WasiVersion;

    /// Get the capabilities enabled for this provider
    fn capabilities(&self) -> &WasiCapabilities;
}

/// Simple host function representation for WASI
#[derive(Clone)]
pub struct HostFunction {
    /// Function name
    #[cfg(feature = "std")]
    pub name:        String,
    #[cfg(not(feature = "std"))]
    pub name: wrt_foundation::BoundedString<256, wrt_foundation::safe_memory::NoStdProvider<1024>>,
    /// Function handler
    pub handler:     HostFunctionHandler,
    /// External type (for component model integration)
    #[cfg(feature = "std")]
    pub extern_type: wrt_format::component::ExternType,
    #[cfg(not(feature = "std"))]
    pub extern_type: ExternType,
}

impl core::fmt::Debug for HostFunction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HostFunction")
            .field("name", &self.name)
            .field("handler", &"<CloneableFn>")
            .field("extern_type", &"<ExternType>")
            .finish()
    }
}

/// Error types specific to WASI operations
pub mod error {
    

    /// WASI-specific error codes
    pub mod codes {
        /// WASI permission denied
        pub const WASI_PERMISSION_DENIED: u16 = 0x2001;
        /// WASI file not found
        pub const WASI_FILE_NOT_FOUND: u16 = 0x2002;
        /// WASI invalid file descriptor
        pub const WASI_INVALID_FD: u16 = 0x2003;
        /// WASI capability not available
        pub const WASI_CAPABILITY_UNAVAILABLE: u16 = 0x2004;
        /// WASI resource limit exceeded
        pub const WASI_RESOURCE_LIMIT: u16 = 0x2005;
    }

    /// WASI-specific error kinds
    pub mod kinds {
        /// WASI permission error
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct WasiPermissionError(pub &'static str);

        /// WASI file system error
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct WasiFileSystemError(pub &'static str);

        /// WASI resource error
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct WasiResourceError(pub &'static str);
    }
}

// Prelude for common imports
pub mod prelude {
    //! Common imports for WASI implementations

    // Re-export component values
    pub use wrt_foundation::Value;
    // Re-export commonly used WRT foundation types
    pub use wrt_foundation::{
        capability_context,
        safe_capability_alloc,
        BoundedMap,
        BoundedString,
        BoundedVec,
        CrateId,
    };
    // Re-export platform abstractions
    pub use wrt_platform::{
        memory::MemoryProvider as PlatformMemoryProvider,
        time::PlatformTime,
    };

    pub use super::error::{
        codes,
        kinds,
    };
    pub use super::{
        CallbackRegistry,
        Error,
        ErrorCategory,
        HostFunction,
        // ComponentLinker,
        HostFunctionHandler,
        MemoryProvider,
        Resource,
        Result,
        WasiCapabilities,
        WasiHostProvider,
        WasiVersion,
    };
    #[cfg(feature = "preview2")]
    pub use super::{
        ComponentModelProvider,
        WasiResourceManager,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasi_version_default() {
        assert_eq!(WasiVersion::default(), WasiVersion::Preview2);
    }

    #[test]
    fn test_error_codes_are_unique() {
        use error::codes::*;

        let codes = [
            WASI_PERMISSION_DENIED,
            WASI_FILE_NOT_FOUND,
            WASI_INVALID_FD,
            WASI_CAPABILITY_UNAVAILABLE,
            WASI_RESOURCE_LIMIT,
        ];

        // Ensure all codes are unique
        for (i, &code1) in codes.iter().enumerate() {
            for &code2 in codes.iter().skip(i + 1) {
                assert_ne!(code1, code2, "Error codes must be unique");
            }
        }
    }
}
