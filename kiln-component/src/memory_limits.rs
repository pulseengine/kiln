//! Memory budget limits for wrt-component safety-critical operations
//!
//! This module defines compile-time memory limits for all bounded collections
//! used in the wrt-component crate when the safety-critical feature is enabled.

#![cfg(feature = "safety-critical")]

/// Canonical ABI memory limits
pub mod canonical_abi {
    /// Maximum number of values in a tuple
    pub const TUPLE_VALUES_LIMIT: usize = 32;

    /// Maximum number of flags
    pub const FLAGS_LIMIT: usize = 64;

    /// Maximum number of fields in a record
    pub const RECORD_FIELDS_LIMIT: usize = 32;

    /// Maximum size of a fixed list
    pub const FIXED_LIST_LIMIT: usize = 256;

    /// Maximum size of a dynamic list
    pub const DYNAMIC_LIST_LIMIT: usize = 1024;

    /// Maximum number of values in type conversion
    pub const CONVERSION_VALUES_LIMIT: usize = 1024;
}

/// Resource management memory limits
pub mod resources {
    /// Maximum number of resources in a resource table
    pub const RESOURCE_TABLE_LIMIT: usize = 1024;

    /// Maximum number of borrows per resource
    pub const BORROWS_PER_RESOURCE_LIMIT: usize = 32;

    /// Maximum number of resource interceptors
    pub const INTERCEPTORS_LIMIT: usize = 16;
}

/// Component instantiation memory limits
pub mod instantiation {
    /// Maximum number of imports per component
    pub const IMPORTS_LIMIT: usize = 256;

    /// Maximum number of exports per component
    pub const EXPORTS_LIMIT: usize = 256;

    /// Maximum number of resource tables per instance
    pub const RESOURCE_TABLES_LIMIT: usize = 16;

    /// Maximum number of module instances per component
    pub const MODULE_INSTANCES_LIMIT: usize = 64;
}

/// Cross-component communication memory limits
pub mod communication {
    /// Maximum number of component instances in registry
    pub const INSTANCE_REGISTRY_LIMIT: usize = 256;

    /// Maximum number of security policies
    pub const SECURITY_POLICIES_LIMIT: usize = 64;

    /// Maximum number of allowed target components per policy
    pub const ALLOWED_TARGETS_LIMIT: usize = 32;

    /// Maximum number of allowed functions per policy
    pub const ALLOWED_FUNCTIONS_LIMIT: usize = 64;

    /// Maximum size of marshaled parameter data in bytes
    pub const MARSHALED_DATA_LIMIT: usize = 8192;
}
