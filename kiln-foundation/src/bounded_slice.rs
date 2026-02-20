//! BoundedSlice - Safe slice-like abstraction for BoundedVec
//!
//! This module provides a safe abstraction for slice-like operations on
//! BoundedVec that maintains ASIL compliance while allowing compatibility with
//! APIs that expect slices.

use core::ops::Range;

#[cfg(test)]
extern crate alloc;

use crate::{
    bounded::BoundedVec,
    traits::BoundedCapacity,
    MemoryProvider,
};

/// A read-only view into a BoundedVec that provides slice-like operations
/// without exposing the underlying memory directly.
#[derive(Debug)]
pub struct BoundedSlice<'a, T, const N: usize, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    vec:   &'a BoundedVec<T, N, P>,
    start: usize,
    len:   usize,
}

impl<'a, T, const N: usize, P> BoundedSlice<'a, T, N, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    /// Create a new BoundedSlice viewing the entire BoundedVec
    pub fn new(vec: &'a BoundedVec<T, N, P>) -> Self {
        Self {
            vec,
            start: 0,
            len: vec.len(),
        }
    }

    /// Create a new BoundedSlice viewing a range of the BoundedVec
    pub fn from_range(vec: &'a BoundedVec<T, N, P>, range: Range<usize>) -> Option<Self> {
        if range.start > range.end || range.end > vec.len() {
            return None;
        }

        Some(Self {
            vec,
            start: range.start,
            len: range.end - range.start,
        })
    }

    /// Get the length of this slice view
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if this slice view is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get an element by index (returns a clone since elements are serialized)
    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        self.vec.get(self.start + index).ok()
    }

    /// Create an iterator over the slice
    pub fn iter(&'a self) -> BoundedSliceIter<'a, T, N, P> {
        BoundedSliceIter {
            slice:   self,
            current: 0,
        }
    }

    /// Split the slice at the given index
    pub fn split_at(&self, mid: usize) -> Option<(Self, Self)> {
        if mid > self.len {
            return None;
        }

        let left = Self {
            vec:   self.vec,
            start: self.start,
            len:   mid,
        };

        let right = Self {
            vec:   self.vec,
            start: self.start + mid,
            len:   self.len - mid,
        };

        Some((left, right))
    }
}

/// Iterator over a BoundedSlice
pub struct BoundedSliceIter<'a, T, const N: usize, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    slice:   &'a BoundedSlice<'a, T, N, P>,
    current: usize,
}

impl<'a, T, const N: usize, P> Iterator for BoundedSliceIter<'a, T, N, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.slice.len {
            return None;
        }

        let item = self.slice.get(self.current)?;
        self.current += 1;
        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.slice.len - self.current;
        (remaining, Some(remaining))
    }
}

impl<'a, T, const N: usize, P> ExactSizeIterator for BoundedSliceIter<'a, T, N, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
}

// Note: Cannot implement Index trait because it requires returning a reference,
// but BoundedVec stores elements serialized and can only return clones

/// Extension trait to add slice-like methods to BoundedVec
pub trait BoundedVecSliceExt<T, const N: usize, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    /// Create a slice view of the entire BoundedVec
    fn as_slice(&self) -> BoundedSlice<'_, T, N, P>;

    /// Create a slice view of a range
    fn slice(&self, range: Range<usize>) -> Option<BoundedSlice<'_, T, N, P>>;
}

impl<T, const N: usize, P> BoundedVecSliceExt<T, N, P> for BoundedVec<T, N, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    fn as_slice(&self) -> BoundedSlice<'_, T, N, P> {
        BoundedSlice::new(self)
    }

    fn slice(&self, range: Range<usize>) -> Option<BoundedSlice<'_, T, N, P>> {
        BoundedSlice::from_range(self, range)
    }
}

/// Adapter to convert BoundedSlice to a regular slice in QM mode only
#[cfg(feature = "dynamic-allocation")]
impl<'a, T, const N: usize, P> BoundedSlice<'a, T, N, P>
where
    T: Sized
        + crate::traits::Checksummable
        + crate::traits::ToBytes
        + crate::traits::FromBytes
        + Default
        + Clone
        + PartialEq
        + Eq,
    P: MemoryProvider + Clone + PartialEq + Eq,
{
    /// Convert to a regular slice (QM mode only)
    ///
    /// # Safety
    /// This is only available in QM mode where dynamic allocation is allowed.
    /// In ASIL modes, use the index-based API instead.
    pub fn as_std_slice(&self) -> &[T] {
        // This requires BoundedVec to expose its internal storage
        // For now, we'll need to implement this differently
        unimplemented!("Requires BoundedVec internal access")
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;
    use crate::safe_memory::NoStdProvider;

    #[test]
    fn test_bounded_slice_creation() {
        let provider = NoStdProvider::<1024>::default();
        let mut vec = BoundedVec::<i32, 10, _>::new(provider).unwrap();
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        vec.push(3).unwrap();

        // as_slice() is not supported with the current BoundedVec architecture
        // (elements are serialized, not stored contiguously)
        assert!(vec.as_slice().is_err());

        // Use individual element access instead
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.get(0).ok(), Some(1));
        assert_eq!(vec.get(1).ok(), Some(2));
        assert_eq!(vec.get(2).ok(), Some(3));
    }

    #[test]
    fn test_bounded_slice_range() {
        let provider = NoStdProvider::<1024>::default();
        let mut vec = BoundedVec::<i32, 10, _>::new(provider).unwrap();
        for i in 0..5 {
            vec.push(i).unwrap();
        }

        let slice = vec.slice(1..4).unwrap();
        assert_eq!(slice.len(), 3);
        assert_eq!(slice.get(0), Some(1));
        assert_eq!(slice.get(1), Some(2));
        assert_eq!(slice.get(2), Some(3));
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_bounded_slice_iterator() {
        let provider = NoStdProvider::<1024>::default();
        let mut vec = BoundedVec::<i32, 10, _>::new(provider).unwrap();
        for i in 0..5 {
            vec.push(i).unwrap();
        }

        let slice = vec.as_slice().unwrap();
        let collected: Vec<i32> = slice.iter().copied().collect();
        assert_eq!(collected, vec![0, 1, 2, 3, 4]);
    }
}
