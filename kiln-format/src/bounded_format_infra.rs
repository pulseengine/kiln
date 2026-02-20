//! Bounded Infrastructure for Format
//!
//! This module provides bounded alternatives for format collections
//! to ensure static memory allocation throughout the format structures.

use wrt_error::Result;
use wrt_foundation::{
    bounded::{BoundedString, BoundedVec},
    no_std_hashmap::BoundedHashMap,
    safe_memory::NoStdProvider,
};

/// Budget-aware memory provider for format (32KB)
pub type FormatProvider = NoStdProvider<32768>;

/// Maximum number of function parameters
pub const MAX_FUNCTION_PARAMS: usize = 128;

/// Maximum number of function results
pub const MAX_FUNCTION_RESULTS: usize = 128;

/// Maximum number of struct fields
pub const MAX_STRUCT_FIELDS: usize = 256;

/// Maximum number of variant cases
pub const MAX_VARIANT_CASES: usize = 256;

/// Maximum number of enum cases
pub const MAX_ENUM_CASES: usize = 256;

/// Maximum number of union types
pub const MAX_UNION_TYPES: usize = 256;

/// Maximum number of type parameters
pub const MAX_TYPE_PARAMS: usize = 32;

/// Maximum module name length
pub const MAX_MODULE_NAME_LENGTH: usize = 256;

/// Maximum field name length
pub const MAX_FIELD_NAME_LENGTH: usize = 128;

/// Maximum package URL length
pub const MAX_PACKAGE_URL_LENGTH: usize = 512;

/// Maximum interface definitions
pub const MAX_INTERFACES: usize = 64;

/// Maximum world imports/exports
pub const MAX_WORLD_ITEMS: usize = 128;

/// Bounded vector for function parameters
pub type BoundedParamsVec<T> = BoundedVec<T, MAX_FUNCTION_PARAMS, FormatProvider>;

/// Bounded vector for function results
pub type BoundedResultsVec<T> = BoundedVec<T, MAX_FUNCTION_RESULTS, FormatProvider>;

/// Bounded vector for struct fields
pub type BoundedFieldsVec<T> = BoundedVec<T, MAX_STRUCT_FIELDS, FormatProvider>;

/// Bounded vector for variant cases
pub type BoundedCasesVec<T> = BoundedVec<T, MAX_VARIANT_CASES, FormatProvider>;

/// Bounded vector for enum cases
pub type BoundedEnumCasesVec<T> = BoundedVec<T, MAX_ENUM_CASES, FormatProvider>;

/// Bounded vector for union types
pub type BoundedUnionVec<T> = BoundedVec<T, MAX_UNION_TYPES, FormatProvider>;

/// Bounded vector for type parameters
pub type BoundedTypeParamsVec<T> = BoundedVec<T, MAX_TYPE_PARAMS, FormatProvider>;

/// Bounded string for module names
pub type BoundedModuleName = BoundedString<MAX_MODULE_NAME_LENGTH>;

/// Bounded string for field names
pub type BoundedFieldName = BoundedString<MAX_FIELD_NAME_LENGTH>;

/// Bounded string for package URLs
pub type BoundedPackageUrl = BoundedString<MAX_PACKAGE_URL_LENGTH>;

/// Bounded map for interface definitions
pub type BoundedInterfaceMap<V> =
    BoundedHashMap<BoundedModuleName, V, MAX_INTERFACES, FormatProvider>;

/// Bounded map for world items
pub type BoundedWorldMap<V> = BoundedHashMap<BoundedModuleName, V, MAX_WORLD_ITEMS, FormatProvider>;

/// Create a new bounded params vector
pub fn new_params_vec<T>() -> Result<BoundedParamsVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = FormatProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded results vector
pub fn new_results_vec<T>() -> Result<BoundedResultsVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = FormatProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded fields vector
pub fn new_fields_vec<T>() -> Result<BoundedFieldsVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = FormatProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded cases vector
pub fn new_cases_vec<T>() -> Result<BoundedCasesVec<T>>
where
    T: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = FormatProvider::default();
    BoundedVec::new(provider)
}

/// Create a new bounded module name
pub fn new_module_name() -> Result<BoundedModuleName> {
    let _provider = FormatProvider::default();
    Ok(BoundedString::from_str_truncate("")?)
}

/// Create a bounded module name from str
pub fn bounded_module_from_str(s: &str) -> Result<BoundedModuleName> {
    let _provider = FormatProvider::default();
    Ok(BoundedString::try_from_str(s)?)
}

/// Create a new bounded field name
pub fn new_field_name() -> Result<BoundedFieldName> {
    let _provider = FormatProvider::default();
    Ok(BoundedString::from_str_truncate("")?)
}

/// Create a bounded field name from str
pub fn bounded_field_from_str(s: &str) -> Result<BoundedFieldName> {
    let _provider = FormatProvider::default();
    Ok(BoundedString::try_from_str(s)?)
}

/// Create a new bounded interface map
pub fn new_interface_map<V>() -> Result<BoundedInterfaceMap<V>>
where
    V: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = FormatProvider::default();
    BoundedHashMap::new(provider)
}

/// Create a new bounded world map
pub fn new_world_map<V>() -> Result<BoundedWorldMap<V>>
where
    V: wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
{
    let provider = FormatProvider::default();
    BoundedHashMap::new(provider)
}
