// Conditional imports for different environments
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::vec::Vec;

use wrt_error::Result;
// For pure no_std mode, we'll make validation work with bounded collections
#[cfg(not(any(feature = "std")))]
use wrt_foundation::{
    BoundedCapacity,
    BoundedVec,
};

/// Trait for types that can be validated
pub trait Validatable {
    /// Validate that this object is well-formed
    fn validate(&self) -> Result<()>;
}

/// Simple validation helper for Option<T> where T is Validatable
impl<T: Validatable> Validatable for Option<T> {
    fn validate(&self) -> Result<()> {
        match self {
            Some(value) => value.validate(),
            None => Ok(()),
        }
    }
}

/// Simple validation helper for Vec<T> where T is Validatable
#[cfg(feature = "std")]
impl<T: Validatable> Validatable for Vec<T> {
    fn validate(&self) -> Result<()> {
        for item in self {
            item.validate()?;
        }
        Ok(())
    }
}

/// Simple validation helper for BoundedVec<T> where T is Validatable
#[cfg(not(any(feature = "std",)))]
impl<T, const N: usize, P> Validatable for BoundedVec<T, N, P>
where
    T: Validatable
        + wrt_foundation::traits::Checksummable
        + wrt_foundation::traits::ToBytes
        + wrt_foundation::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: wrt_foundation::MemoryProvider + Clone + PartialEq + Eq,
{
    fn validate(&self) -> Result<()> {
        // For BoundedVec, we need to iterate through valid elements
        for i in 0..self.len() {
            if let Ok(item) = self.get(i) {
                item.validate()?;
            }
        }
        Ok(())
    }
}
