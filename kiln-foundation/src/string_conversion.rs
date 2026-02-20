//! String conversion traits for component model integration
//!
//! This module provides conversion traits between different string types used
//! in the WRT framework, particularly for component model integration where
//! strings need to be converted between format, runtime, and component representations.

use crate::{
    bounded::BoundedString,
    MemoryProvider,
    traits::{BoundedCapacity, Checksummable, ToBytes, FromBytes},
    wrt_error::Result,
};
use wrt_error::{Error, Result};
use wrt_error::Result;
#[cfg(feature = "std")]
use std::string::String;
#[cfg(not(feature = "std"))]
use alloc::string::String;

/// Trait for converting between different string representations
pub trait StringConversion<T> {
    /// Convert from this string type to the target type
    fn convert_to(&self) -> Result<T>;
    
    /// Convert from the target type to this string type
    fn convert_from(value: &T) -> Result<Self>
    where
        Self: Sized;
}

/// Component string type for WebAssembly Component Model
/// This provides a safe abstraction for string handling in component contexts
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentString<const N: usize, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    inner: BoundedString<N, P>,
}

impl<const N: usize, P> Default for ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq + Default,
{
    fn default() -> Self {
        let provider = P::default());
        Self {
            inner: BoundedString::new(provider).unwrap_or_else(|_| BoundedString::default()),
        }
    }
}

impl<const N: usize, P> ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    /// Create a new ComponentString with the given provider
    pub fn new(provider: P) -> Result<Self> {
        let inner = BoundedString::new(provider)?;
        Ok(Self { inner })
    }
    
    /// Create from a string slice, truncating if necessary
    pub fn from_str_truncate(s: &str, provider: P) -> Result<Self> {
        let inner = BoundedString::from_str_truncate(s, provider)?;
        Ok(Self { inner })
    }
    
    /// Get the string content as a &str
    pub fn as_str(&self) -> wrt_error::Result<&str> {
        self.inner.as_str()
    }
    
    /// Get the length of the string
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    
    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    
    /// Get the capacity of the string
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

impl<const N: usize, P> Checksummable for ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        self.inner.update_checksum(checksum;
    }
}

impl<const N: usize, P> ToBytes for ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    fn serialized_size(&self) -> usize {
        self.inner.serialized_size()
    }
    
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut crate::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.inner.to_bytes_with_provider(writer, provider)
    }
}

impl<const N: usize, P> FromBytes for ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut crate::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self>
    where
        Self: Sized,
    {
        let inner = BoundedString::from_bytes_with_provider(reader, provider)?;
        Ok(Self { inner })
    }
}

/// Conversion from std::string::String to ComponentString
impl<const N: usize, P> StringConversion<String> for ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    fn convert_to(&self) -> Result<String> {
        let s = self.as_str().map_err(|_| Error::type_conversion_error("Failed to get string content"))?;
        Ok(s.to_string())
    }
    
    fn convert_from(value: &String) -> Result<Self>
    where
        Self: Sized,
    {
        // Need to provide a default provider - this is a limitation
        // In practice, callers should use from_str_truncate with explicit provider
        Err(Error::not_supported_unsupported_operation("Use from_str_truncate with explicit provider"))
    }
}

/// Conversion from BoundedString to ComponentString
impl<const N: usize, const M: usize, P> StringConversion<BoundedString<M, P>> for ComponentString<N, P>
where
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    fn convert_to(&self) -> Result<BoundedString<M, P>> {
        let content = self.as_str().map_err(|_| Error::type_conversion_error("Failed to get string content"))?;
        let provider = self.inner.provider().clone();
        BoundedString::from_str_truncate(content, provider)
            .map_err(|_| Error::type_conversion_error("Failed to create BoundedString"))
    }
    
    fn convert_from(value: &BoundedString<M, P>) -> Result<Self>
    where
        Self: Sized,
    {
        let content = value.as_str().map_err(|_| Error::type_conversion_error("Failed to get string content"))?;
        let provider = value.provider().clone();
        Self::from_str_truncate(content, provider)
    }
}

/// String adapter for component model contexts
pub struct ComponentStringAdapter;

impl ComponentStringAdapter {
    /// Convert a regular string to a ComponentString with the given provider
    pub fn to_component_string<const N: usize, P>(
        s: &str,
        provider: P,
    ) -> Result<ComponentString<N, P>>
    where
        P: MemoryProvider + Clone + PartialEq + Eq,
    {
        ComponentString::from_str_truncate(s, provider)
    }
    
    /// Convert a ComponentString to a regular string
    pub fn from_component_string<const N: usize, P>(
        component_string: &ComponentString<N, P>,
    ) -> Result<String>
    where
        P: MemoryProvider + Clone + PartialEq + Eq,
    {
        component_string.convert_to()
    }
    
    /// Convert between different sized ComponentStrings
    pub fn resize_component_string<const N: usize, const M: usize, P>(
        source: &ComponentString<N, P>,
    ) -> Result<ComponentString<M, P>>
    where
        P: MemoryProvider + Clone + PartialEq + Eq,
    {
        let content = source.as_str().map_err(|_| Error::type_conversion_error("Failed to get string content"))?;
        let provider = source.inner.provider().clone();
        ComponentString::from_str_truncate(content, provider)
    }
}

/// Utility functions for string conversion in component contexts
pub mod component_utils {
    use super::*;
    
    /// Create a ComponentString from export name
    pub fn export_name_to_component<const N: usize, P>(
        export_name: &str,
        provider: P,
    ) -> Result<ComponentString<N, P>>
    where
        P: MemoryProvider + Clone + PartialEq + Eq,
    {
        ComponentString::from_str_truncate(export_name, provider)
    }
    
    /// Create a BoundedString from ComponentString for export maps
    pub fn component_to_export_key<const N: usize, const M: usize, P>(
        component_string: &ComponentString<N, P>,
    ) -> Result<BoundedString<M, P>>
    where
        P: MemoryProvider + Clone + PartialEq + Eq,
    {
        component_string.convert_to()
    }
    
    /// Batch convert string arrays for component instantiation
    pub fn convert_string_array<const N: usize, P>(
        strings: &[String],
        provider: P,
    ) -> Result<crate::bounded::BoundedVec<ComponentString<N, P>, 64, P>>
    where
        P: MemoryProvider + Clone + PartialEq + Eq,
    {
        let mut result = crate::bounded::BoundedVec::new(provider.clone())?;
        
        for s in strings {
            let component_string = ComponentString::from_str_truncate(s, provider.clone())?;
            result.push(component_string)?;
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safe_memory::NoStdProvider;
    
    #[test]
    fn test_component_string_creation() {
        let provider = NoStdProvider::<1024>::default());
        let component_string = ComponentString::<128, _>::from_str_truncate("test_string", provider).unwrap();
        
        assert_eq!(component_string.as_str().unwrap(), "test_string";
        assert_eq!(component_string.len(), 11;
        assert!(!component_string.is_empty());
    }
    
    #[test]
    fn test_string_conversion() {
        let provider = NoStdProvider::<1024>::default());
        let component_string = ComponentString::<128, _>::from_str_truncate("hello_world", provider).unwrap();
        
        let std_string = component_string.convert_to().unwrap();
        assert_eq!(std_string, "hello_world";
    }
    
    #[test]
    fn test_component_string_adapter() {
        let provider = NoStdProvider::<1024>::default());
        
        // Test to_component_string
        let component_string = ComponentStringAdapter::to_component_string::<256, _>("test", provider).unwrap();
        assert_eq!(component_string.as_str().unwrap(), "test";
        
        // Test from_component_string
        let std_string = ComponentStringAdapter::from_component_string(&component_string).unwrap();
        assert_eq!(std_string, "test";
    }
    
    #[test]
    fn test_string_array_conversion() {
        let provider = NoStdProvider::<1024>::default());
        let strings = vec!["first".to_string(), "second".to_string(), "third".to_string()];
        
        let component_strings = component_utils::convert_string_array::<64, _>(&strings, provider).unwrap();
        
        assert_eq!(component_strings.len(), 3;
        assert_eq!(component_strings.get(0).unwrap().as_str().unwrap(), "first";
        assert_eq!(component_strings.get(1).unwrap().as_str().unwrap(), "second";
        assert_eq!(component_strings.get(2).unwrap().as_str().unwrap(), "third";
    }
}