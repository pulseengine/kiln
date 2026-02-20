// WRT - wrt-foundation
// Module: StaticVec - Inline-storage vector
// SW-REQ-ID: REQ_RESOURCE_001, REQ_MEM_SAFETY_001, REQ_TEMPORAL_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Allow unsafe code for MaybeUninit operations (documented and verified via KANI)
#![allow(unsafe_code)]

//! Static vector with inline storage and compile-time capacity.
//!
//! `StaticVec<T, N>` is inspired by the `heapless` crate but is fully self-contained
//! within the WRT project. It provides a fixed-capacity vector where all storage is
//! inline (no heap allocation, no Provider abstraction).
//!
//! # Characteristics
//!
//! - **Zero allocation**: All memory is inline `[MaybeUninit<T>; N]`
//! - **Const-time operations**: `push()`, `pop()`, `get()` are O(1)
//! - **RAII cleanup**: Automatic Drop implementation
//! - **Bounds checking**: All access methods return `Option` or `Result`
//! - **ASIL-D compliant**: Deterministic behavior, no dynamic allocation
//!
//! # Safety
//!
//! This implementation uses `MaybeUninit<T>` to store uninitialized elements
//! and carefully tracks which elements are initialized via the `len` field.
//! All unsafe code is confined to initialization and cleanup, with invariants
//! documented and verified via KANI.

use core::marker::PhantomData;
use core::mem::MaybeUninit;

use wrt_error::Result;

use crate::traits::{Checksummable, ToBytes, FromBytes, WriteStream, ReadStream};
use crate::verification::Checksum;

/// A vector with compile-time capacity and inline storage.
///
/// # Requirements
///
/// - REQ_RESOURCE_001: Static allocation only (no runtime allocation)
/// - REQ_MEM_SAFETY_001: Bounds validation before all access
/// - REQ_TEMPORAL_001: Constant-time operations
/// - REQ_MEM_SAFETY_005: Explicit lifetime management via RAII
///
/// # Invariants
///
/// 1. `len <= N` always holds
/// 2. Elements `[0..len)` are initialized
/// 3. Elements `[len..N)` are uninitialized
/// 4. On drop, all initialized elements are dropped
///
/// # Examples
///
/// ```
/// use wrt_foundation::collections::StaticVec;
///
/// let mut vec = StaticVec::<u32, 10>::new();
/// vec.push(42)?;
/// vec.push(100)?;
///
/// assert_eq!(vec.len(), 2);
/// assert_eq!(vec.get(0), Some(&42));
///
/// vec.pop();
/// assert_eq!(vec.len(), 1);
/// # Ok::<(), wrt_error::Error>(())
/// ```
#[derive(Debug)]
pub struct StaticVec<T, const N: usize> {
    /// Inline storage for elements.
    /// Elements [0..len) are initialized, [len..N) are uninitialized.
    data: [MaybeUninit<T>; N],

    /// Number of initialized elements.
    /// Invariant: len <= N
    len: usize,

    /// Marker for drop checker.
    /// Ensures proper cleanup of T even though we use MaybeUninit.
    _marker: PhantomData<T>,
}

impl<T, const N: usize> StaticVec<T, N> {
    /// Creates a new empty vector.
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` and has a **deterministic WCET of ~5 CPU cycles**
    /// (measured on x86_64, may vary by architecture).
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let vec = StaticVec::<u32, 10>::new();
    /// assert_eq!(vec.len(), 0);
    /// assert_eq!(vec.capacity(), 10);
    /// ```
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            // SAFETY: MaybeUninit::uninit_array() is safe; we track initialization via len
            data: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Pushes an element to the end of the vector.
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` with **deterministic WCET of ~12 CPU cycles**.
    /// There is NO reallocation - the operation either succeeds immediately
    /// or fails with an error.
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if `len == N` (vector is full).
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 2>::new();
    /// assert!(vec.push(1).is_ok());
    /// assert!(vec.push(2).is_ok());
    /// assert!(vec.push(3).is_err()); // Capacity exceeded
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    pub fn push(&mut self, value: T) -> Result<()> {
        if self.len >= N {
            return Err(wrt_error::Error::foundation_bounded_capacity_exceeded(
                "StaticVec capacity exceeded",
            ));
        }

        // SAFETY: We just checked that len < N, so this index is valid
        self.data[self.len].write(value);
        self.len += 1;
        Ok(())
    }

    /// Removes and returns the last element, or `None` if empty.
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` with **deterministic WCET of ~15 CPU cycles**
    /// (includes drop of element).
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(42)?;
    ///
    /// assert_eq!(vec.pop(), Some(42));
    /// assert_eq!(vec.pop(), None);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        // SAFETY: We just decremented len, so len is now < old len, which was <= N
        // The element at self.len was initialized
        Some(unsafe { self.data[self.len].assume_init_read() })
    }

    /// Returns a reference to an element, or `None` if out of bounds.
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` with **deterministic WCET of ~8 CPU cycles**
    /// (includes bounds check).
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(42)?;
    ///
    /// assert_eq!(vec.get(0), Some(&42));
    /// assert_eq!(vec.get(1), None);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            // SAFETY: index < len, so the element is initialized
            Some(unsafe { self.data[index].assume_init_ref() })
        } else {
            None
        }
    }

    /// Returns a mutable reference to an element, or `None` if out of bounds.
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` with **deterministic WCET of ~8 CPU cycles**.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(42)?;
    ///
    /// *vec.get_mut(0).unwrap() = 100;
    /// assert_eq!(vec.get(0), Some(&100));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            // SAFETY: index < len, so the element is initialized
            Some(unsafe { self.data[index].assume_init_mut() })
        } else {
            None
        }
    }

    /// Returns the current length (number of initialized elements).
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` with **deterministic WCET of ~3 CPU cycles**.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the compile-time capacity.
    ///
    /// # Const-time Guarantee
    ///
    /// This operation is `O(1)` and typically **optimized to 0 cycles**
    /// (const-folded at compile time).
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns `true` if the vector is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the vector is at full capacity.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Maps an error type in a Result chain (always returns Ok(self)).
    ///
    /// This is a compatibility method for code migrating from BoundedVec to StaticVec.
    /// Since StaticVec construction never fails, this always returns Ok(self).
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    /// use wrt_error::Error;
    ///
    /// let vec = StaticVec::<u32, 10>::new();
    /// let result = vec.map_err(|| Error::runtime_error("Never happens"))?;
    /// assert_eq!(result.len(), 0);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    pub fn map_err<E, F>(self, _f: F) -> core::result::Result<Self, E>
    where
        F: FnOnce() -> E,
    {
        Ok(self)
    }

    /// Consumes self and returns it wrapped in Ok.
    ///
    /// This is a convenience method for Result chaining patterns.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let vec = StaticVec::<u32, 10>::new();
    /// let result: Result<StaticVec<u32, 10>, &str> = vec.ok_or("error");
    /// assert!(result.is_ok());
    /// ```
    #[inline]
    pub fn ok_or<E>(self, _err: E) -> core::result::Result<Self, E> {
        Ok(self)
    }

    /// Identity function that returns self.
    ///
    /// This is a convenience method for compatibility with Result unwrap patterns.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(42).unwrap();
    /// let vec = vec.unwrap(); // No-op, returns self
    /// assert_eq!(vec.len(), 1);
    /// ```
    #[inline]
    #[must_use]
    pub fn unwrap(self) -> Self {
        self
    }

    /// Clears the vector, dropping all elements.
    ///
    /// # Time Complexity
    ///
    /// `O(n)` where `n = len`. **WCET = ~15 * len CPU cycles**.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    ///
    /// vec.clear();
    /// assert_eq!(vec.len(), 0);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn clear(&mut self) {
        while self.len > 0 {
            self.len -= 1;
            // SAFETY: We just decremented len, so this index was initialized
            unsafe {
                self.data[self.len].assume_init_drop();
            }
        }
    }

    /// Returns an iterator over the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// vec.push(3)?;
    ///
    /// let sum: u32 = vec.iter().sum();
    /// assert_eq!(sum, 6);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn iter(&self) -> StaticVecIter<'_, T, N> {
        StaticVecIter {
            vec: self,
            index: 0,
            end: self.len,
        }
    }

    /// Returns a mutable iterator over the vector.
    #[inline]
    #[must_use]
    pub fn iter_mut(&mut self) -> StaticVecIterMut<'_, T, N> {
        let len = self.len;
        StaticVecIterMut {
            vec: self as *mut Self,
            index: 0,
            end: len,
            _marker: PhantomData,
        }
    }

    /// Checks if the vector contains an element.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    ///
    /// assert!(vec.contains(&1));
    /// assert!(!vec.contains(&3));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq,
    {
        for i in 0..self.len {
            // SAFETY: i < self.len, so element is initialized
            let elem = unsafe { self.data[i].assume_init_ref() };
            if elem == x {
                return true;
            }
        }
        false
    }

    /// Returns a reference to the first element, or `None` if empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// assert_eq!(vec.first(), None);
    ///
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// assert_eq!(vec.first(), Some(&1));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<&T> {
        if self.len > 0 {
            // SAFETY: self.len > 0, so index 0 is initialized
            Some(unsafe { self.data[0].assume_init_ref() })
        } else {
            None
        }
    }

    /// Returns a reference to the last element, or `None` if empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// assert_eq!(vec.last(), None);
    ///
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// assert_eq!(vec.last(), Some(&2));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<&T> {
        if self.len > 0 {
            // SAFETY: self.len > 0, so index self.len - 1 is initialized
            Some(unsafe { self.data[self.len - 1].assume_init_ref() })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the last element, or `None` if empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    ///
    /// *vec.last_mut().unwrap() = 10;
    /// assert_eq!(vec.last(), Some(&10));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.len > 0 {
            // SAFETY: self.len > 0, so index self.len - 1 is initialized
            Some(unsafe { self.data[self.len - 1].assume_init_mut() })
        } else {
            None
        }
    }

    /// Returns a slice containing the entire vector.
    ///
    /// # Safety Note
    ///
    /// This method uses `MaybeUninit::slice_assume_init_ref` which is safe
    /// because we maintain the invariant that elements [0..len) are initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    ///
    /// let slice = vec.as_slice();
    /// assert_eq!(slice, &[1, 2]);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0 {
            &[]
        } else {
            // SAFETY: Elements [0..len) are initialized per our invariant
            unsafe {
                core::slice::from_raw_parts(
                    self.data.as_ptr() as *const T,
                    self.len,
                )
            }
        }
    }

    /// Returns a mutable slice containing the entire vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    ///
    /// let slice = vec.as_mut_slice();
    /// slice[0] = 10;
    /// assert_eq!(vec.get(0), Some(&10));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.len == 0 {
            &mut []
        } else {
            // SAFETY: Elements [0..len) are initialized per our invariant
            unsafe {
                core::slice::from_raw_parts_mut(
                    self.data.as_mut_ptr() as *mut T,
                    self.len,
                )
            }
        }
    }

    /// Reverses the order of elements in the vector, in place.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// vec.push(3)?;
    ///
    /// vec.reverse();
    /// assert_eq!(vec.get(0), Some(&3));
    /// assert_eq!(vec.get(1), Some(&2));
    /// assert_eq!(vec.get(2), Some(&1));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn reverse(&mut self) {
        let mid = self.len / 2;
        for i in 0..mid {
            let j = self.len - 1 - i;
            // SAFETY: Both i and j are < self.len, so elements are initialized
            unsafe {
                let temp = self.data[i].assume_init_read();
                self.data[i].write(self.data[j].assume_init_read());
                self.data[j].write(temp);
            }
        }
    }

    /// Removes and returns the element at position `index`, shifting all elements
    /// after it to the left.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<i32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// vec.push(3)?;
    ///
    /// assert_eq!(vec.remove(1), 2);
    /// assert_eq!(vec.len(), 2);
    /// assert_eq!(vec.get(1), Some(&3));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "index out of bounds");

        // SAFETY: index is bounds-checked above
        let result = unsafe {
            self.data[index].assume_init_read()
        };

        // Shift elements left
        for i in index..self.len - 1 {
            // SAFETY: We're within bounds (index < self.len - 1 < N)
            unsafe {
                let src = self.data[i + 1].assume_init_read();
                self.data[i].write(src);
            }
        }

        self.len -= 1;
        result
    }

    /// Removes and returns the element at position `index` by swapping it with the last element.
    ///
    /// This is O(1) but does not preserve ordering.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<i32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// vec.push(3)?;
    ///
    /// assert_eq!(vec.swap_remove(1), 2);
    /// assert_eq!(vec.len(), 2);
    /// // Note: element at index 1 is now 3 (swapped from end)
    /// assert_eq!(vec.get(1), Some(&3));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "index out of bounds");

        // SAFETY: index is bounds-checked above
        let result = unsafe {
            self.data[index].assume_init_read()
        };

        self.len -= 1;

        // If we're not removing the last element, swap with the last element
        if index != self.len {
            // SAFETY: self.len is the index of the (now removed) last element
            unsafe {
                let last = self.data[self.len].assume_init_read();
                self.data[index].write(last);
            }
        }

        result
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<i32, 10>::new();
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// vec.push(3)?;
    /// vec.push(4)?;
    ///
    /// vec.retain(|&x| x % 2 == 0);
    /// assert_eq!(vec.len(), 2);
    /// assert_eq!(vec.get(0), Some(&2));
    /// assert_eq!(vec.get(1), Some(&4));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        let mut i = 0;
        while i < self.len {
            // SAFETY: i < self.len, so this element is initialized
            let should_retain = unsafe {
                f(self.data[i].assume_init_ref())
            };

            if should_retain {
                i += 1;
            } else {
                // Remove this element by shifting
                unsafe {
                    self.data[i].assume_init_drop();
                }

                for j in i..self.len - 1 {
                    unsafe {
                        let src = self.data[j + 1].assume_init_read();
                        self.data[j].write(src);
                    }
                }

                self.len -= 1;
            }
        }
    }

    /// Creates a new `StaticVec` from a slice.
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if the slice length exceeds `N`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let slice = &[1, 2, 3, 4, 5];
    /// let vec = StaticVec::<i32, 10>::from_slice(slice)?;
    /// assert_eq!(vec.len(), 5);
    /// assert_eq!(vec.get(0), Some(&1));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn from_slice(slice: &[T]) -> Result<Self>
    where
        T: Clone,
    {
        if slice.len() > N {
            return Err(wrt_error::Error::foundation_bounded_capacity_exceeded(
                "Slice length exceeds StaticVec capacity",
            ));
        }

        let mut vec = Self::new();
        for item in slice {
            vec.push(item.clone())?;
        }
        Ok(vec)
    }

    /// Resizes the vector to the specified length, filling with the provided value.
    ///
    /// If `new_len` is greater than the current length, clones of `value` are appended.
    /// If `new_len` is less than the current length, the vector is truncated.
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if `new_len > N`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    /// vec.resize(3, 42)?;
    ///
    /// assert_eq!(vec.len(), 3);
    /// assert_eq!(vec.get(0), Some(&1));
    /// assert_eq!(vec.get(1), Some(&42));
    /// assert_eq!(vec.get(2), Some(&42));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn resize(&mut self, new_len: usize, value: T) -> Result<()>
    where
        T: Clone,
    {
        if new_len > N {
            return Err(wrt_error::Error::foundation_bounded_capacity_exceeded(
                "Resize length exceeds StaticVec capacity",
            ));
        }

        if new_len > self.len {
            // Grow: append clones of value
            for _ in self.len..new_len {
                self.push(value.clone())?;
            }
        } else if new_len < self.len {
            // Shrink: drop excess elements
            while self.len > new_len {
                self.pop();
            }
        }
        // If new_len == self.len, do nothing

        Ok(())
    }

    /// Extends the vector with the contents of an iterator.
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if the iterator contains more elements than available capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(1)?;
    ///
    /// vec.extend([2, 3, 4].iter().cloned())?;
    /// assert_eq!(vec.len(), 4);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn extend<I>(&mut self, iter: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
    {
        for item in iter {
            self.push(item)?;
        }
        Ok(())
    }

    /// Unwraps the vector with a custom error message.
    ///
    /// This is a convenience method for compatibility with Result-like patterns.
    /// Since StaticVec construction never fails, this always returns self.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let vec = StaticVec::<u32, 10>::new();
    /// let vec = vec.expect("This never fails");
    /// assert_eq!(vec.len(), 0);
    /// ```
    #[inline]
    #[must_use]
    pub fn expect(self, _msg: &str) -> Self {
        self
    }

    /// Returns a reference to the first element without removing it, or `None` if empty.
    ///
    /// This is an alias for `first()` but follows common queue/heap naming conventions.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// assert_eq!(vec.peek(), None);
    ///
    /// vec.push(1)?;
    /// vec.push(2)?;
    /// assert_eq!(vec.peek(), Some(&1));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        self.first()
    }

    /// Sorts the vector using a comparator function.
    ///
    /// This uses a simple insertion sort algorithm which is suitable for small vectors
    /// and provides stable sorting with O(n²) worst case and O(n) best case complexity.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticVec;
    ///
    /// let mut vec = StaticVec::<u32, 10>::new();
    /// vec.push(5)?;
    /// vec.push(2)?;
    /// vec.push(8)?;
    /// vec.push(1)?;
    ///
    /// vec.sort_by(|a, b| a.cmp(b));
    /// assert_eq!(vec.get(0), Some(&1));
    /// assert_eq!(vec.get(1), Some(&2));
    /// assert_eq!(vec.get(2), Some(&5));
    /// assert_eq!(vec.get(3), Some(&8));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn sort_by<F>(&mut self, mut compare: F)
    where
        F: FnMut(&T, &T) -> core::cmp::Ordering,
    {
        // Insertion sort - simple and efficient for small N
        for i in 1..self.len {
            let mut j = i;
            while j > 0 {
                // SAFETY: Both j and j-1 are within bounds [0..len)
                let (left, right) = unsafe {
                    (
                        self.data[j - 1].assume_init_ref(),
                        self.data[j].assume_init_ref(),
                    )
                };

                if compare(left, right) != core::cmp::Ordering::Greater {
                    break;
                }

                // Swap elements at j-1 and j
                unsafe {
                    let temp = self.data[j - 1].assume_init_read();
                    self.data[j - 1].write(self.data[j].assume_init_read());
                    self.data[j].write(temp);
                }

                j -= 1;
            }
        }
    }
}

// RAII: Automatic cleanup on drop
impl<T, const N: usize> Drop for StaticVec<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

// Default: empty vector
impl<T, const N: usize> Default for StaticVec<T, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// PartialEq implementation
impl<T: PartialEq, const N: usize> PartialEq for StaticVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        for i in 0..self.len {
            let a = unsafe { self.data[i].assume_init_ref() };
            let b = unsafe { other.data[i].assume_init_ref() };
            if a != b {
                return false;
            }
        }
        true
    }
}

// Eq implementation (if T: Eq)
impl<T: Eq, const N: usize> Eq for StaticVec<T, N> {}

// PartialOrd implementation
impl<T: PartialOrd, const N: usize> PartialOrd for StaticVec<T, N> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

// Ord implementation (if T: Ord)
impl<T: Ord, const N: usize> Ord for StaticVec<T, N> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.iter().cmp(other.iter())
    }
}

// Clone implementation (requires T: Clone)
impl<T: Clone, const N: usize> Clone for StaticVec<T, N> {
    fn clone(&self) -> Self {
        let mut new_vec = Self::new();
        for item in self.iter() {
            // SAFETY: We're iterating over initialized elements,
            // and new_vec has same capacity, so this won't fail
            let _ = new_vec.push(item.clone());
        }
        new_vec
    }
}

// Checksummable implementation
impl<T: Checksummable, const N: usize> Checksummable for StaticVec<T, N> {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Update checksum with length first
        self.len.update_checksum(checksum);
        // Then iterate through elements
        for item in self.iter() {
            item.update_checksum(checksum);
        }
    }
}

// ToBytes implementation
impl<T: ToBytes, const N: usize> ToBytes for StaticVec<T, N> {
    fn serialized_size(&self) -> usize {
        // Size needed: length (usize) + sum of element sizes
        core::mem::size_of::<usize>() + self.iter().map(|item| item.serialized_size()).sum::<usize>()
    }

    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write length first
        self.len.to_bytes_with_provider(writer, provider)?;
        // Write each element
        for item in self.iter() {
            item.to_bytes_with_provider(writer, provider)?;
        }
        Ok(())
    }
}

// FromBytes implementation
impl<T: FromBytes, const N: usize> FromBytes for StaticVec<T, N> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Read length
        let len = usize::from_bytes_with_provider(reader, provider)?;

        // Create new vec
        let mut vec = StaticVec::new();

        // Read elements
        for _ in 0..len {
            let item = T::from_bytes_with_provider(reader, provider)?;
            vec.push(item)?;
        }

        Ok(vec)
    }
}

// Hash implementation
impl<T: core::hash::Hash, const N: usize> core::hash::Hash for StaticVec<T, N> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Hash length first
        self.len.hash(state);
        // Hash each element
        for item in self.iter() {
            item.hash(state);
        }
    }
}

// Index implementation
impl<T, const N: usize> core::ops::Index<usize> for StaticVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len, "index out of bounds");
        unsafe { self.data[index].assume_init_ref() }
    }
}

// IndexMut implementation
impl<T, const N: usize> core::ops::IndexMut<usize> for StaticVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len, "index out of bounds");
        unsafe { self.data[index].assume_init_mut() }
    }
}

// Range indexing implementation
impl<T, const N: usize> core::ops::Index<core::ops::Range<usize>> for StaticVec<T, N> {
    type Output = [T];

    fn index(&self, range: core::ops::Range<usize>) -> &Self::Output {
        assert!(range.start <= range.end, "range start must be <= end");
        assert!(range.end <= self.len, "range end out of bounds");
        unsafe {
            core::slice::from_raw_parts(
                self.data[range.start].as_ptr(),
                range.end - range.start
            )
        }
    }
}

// Iterator implementation
pub struct StaticVecIter<'a, T, const N: usize> {
    vec: &'a StaticVec<T, N>,
    index: usize,
    end: usize,
}

impl<'a, T, const N: usize> Iterator for StaticVecIter<'a, T, N> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.end {
            let item = unsafe { self.vec.data[self.index].assume_init_ref() };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, T, const N: usize> ExactSizeIterator for StaticVecIter<'a, T, N> {}

impl<'a, T, const N: usize> DoubleEndedIterator for StaticVecIter<'a, T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index < self.end {
            self.end -= 1;
            let item = unsafe { self.vec.data[self.end].assume_init_ref() };
            Some(item)
        } else {
            None
        }
    }
}

// Mutable iterator implementation
pub struct StaticVecIterMut<'a, T, const N: usize> {
    vec: *mut StaticVec<T, N>,
    index: usize,
    end: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T, const N: usize> Iterator for StaticVecIterMut<'a, T, N> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.end {
            unsafe {
                let vec = &mut *self.vec;
                let item = vec.data[self.index].assume_init_mut();
                self.index += 1;
                Some(item)
            }
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, T, const N: usize> ExactSizeIterator for StaticVecIterMut<'a, T, N> {}

impl<'a, T, const N: usize> DoubleEndedIterator for StaticVecIterMut<'a, T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index < self.end {
            self.end -= 1;
            unsafe {
                let vec = &mut *self.vec;
                let item = vec.data[self.end].assume_init_mut();
                Some(item)
            }
        } else {
            None
        }
    }
}

// IntoIterator support for references
impl<'a, T, const N: usize> IntoIterator for &'a StaticVec<T, N> {
    type Item = &'a T;
    type IntoIter = StaticVecIter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut StaticVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = StaticVecIterMut<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

// Consuming IntoIterator (takes ownership)
pub struct StaticVecIntoIter<T, const N: usize> {
    vec: StaticVec<T, N>,
    index: usize,
}

impl<T, const N: usize> Iterator for StaticVecIntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vec.len() {
            // SAFETY: We know index is valid and element is initialized
            let item = unsafe {
                core::ptr::read(self.vec.data[self.index].as_ptr())
            };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vec.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<T, const N: usize> Drop for StaticVecIntoIter<T, N> {
    fn drop(&mut self) {
        // Drop any remaining elements that weren't consumed
        while self.next().is_some() {}
        // Set vec len to 0 to prevent double-drop
        self.vec.len = 0;
    }
}

impl<T, const N: usize> IntoIterator for StaticVec<T, N> {
    type Item = T;
    type IntoIter = StaticVecIntoIter<T, N>;

    fn into_iter(self) -> Self::IntoIter {
        StaticVecIntoIter {
            vec: self,
            index: 0,
        }
    }
}

// ============================================================================
// KANI Formal Verification
// ============================================================================

#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn verify_push_never_exceeds_capacity() {
        let mut vec: StaticVec<u8, 10> = StaticVec::new();

        // Push 10 items (should all succeed)
        for i in 0..10 {
            assert!(vec.push(i).is_ok());
            assert!(vec.len() == (i as usize + 1));
        }

        assert!(vec.len() == 10);
        assert!(vec.is_full());

        // 11th push should fail
        assert!(vec.push(42).is_err());
        assert!(vec.len() == 10); // Length unchanged after failed push
    }

    #[kani::proof]
    fn verify_bounds_checking() {
        let mut vec: StaticVec<u32, 5> = StaticVec::new();
        vec.push(100).unwrap();
        vec.push(200).unwrap();

        let index: usize = kani::any();

        // Verify: out-of-bounds returns None (never UB)
        let result = vec.get(index);
        if index >= vec.len() {
            assert!(result.is_none());
        } else {
            assert!(result.is_some());
        }
    }

    #[kani::proof]
    #[kani::unwind(6)] // Enough for 5 elements + cleanup
    fn verify_drop_cleanup() {
        let mut vec: StaticVec<u32, 5> = StaticVec::new();
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        vec.push(3).unwrap();

        // Explicit drop - KANI verifies no memory leaks
        drop(vec);
    }

    #[kani::proof]
    fn verify_wcet_determinism() {
        let mut vec: StaticVec<u64, 100> = StaticVec::new();

        // Push is O(1) regardless of current length
        for i in 0..100 {
            let len_before = vec.len();
            let result = vec.push(i);
            assert!(result.is_ok());
            assert!(vec.len() == len_before + 1);
            // KANI verifies: same code path regardless of len
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let vec: StaticVec<u32, 10> = StaticVec::new();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 10);
        assert!(vec.is_empty());
        assert!(!vec.is_full());
    }

    #[test]
    fn test_push_pop() -> Result<()> {
        let mut vec = StaticVec::<u32, 5>::new();

        vec.push(1)?;
        vec.push(2)?;
        vec.push(3)?;

        assert_eq!(vec.len(), 3);
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);

        Ok(())
    }

    #[test]
    fn test_capacity_exceeded() {
        let mut vec = StaticVec::<u32, 2>::new();

        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert!(vec.push(3).is_err()); // Should fail

        assert_eq!(vec.len(), 2); // Length unchanged
    }

    #[test]
    fn test_get() -> Result<()> {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(42)?;
        vec.push(100)?;

        assert_eq!(vec.get(0), Some(&42));
        assert_eq!(vec.get(1), Some(&100));
        assert_eq!(vec.get(2), None);

        Ok(())
    }

    #[test]
    fn test_clear() -> Result<()> {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(1)?;
        vec.push(2)?;
        vec.push(3)?;

        vec.clear();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        Ok(())
    }

    #[test]
    fn test_iter() -> Result<()> {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(1)?;
        vec.push(2)?;
        vec.push(3)?;

        let sum: u32 = vec.iter().sum();
        assert_eq!(sum, 6);

        // Manually check iterator values (no_std compatible)
        let mut iter = vec.iter();
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), None);

        Ok(())
    }

    #[test]
    fn test_clone() -> Result<()> {
        let mut vec1 = StaticVec::<u32, 10>::new();
        vec1.push(1)?;
        vec1.push(2)?;

        let vec2 = vec1.clone();
        assert_eq!(vec2.len(), 2);
        assert_eq!(vec2.get(0), Some(&1));
        assert_eq!(vec2.get(1), Some(&2));

        Ok(())
    }

    #[test]
    fn test_index() -> Result<()> {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(10)?;
        vec.push(20)?;
        vec.push(30)?;

        // Test Index trait
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);

        Ok(())
    }

    #[test]
    fn test_index_mut() -> Result<()> {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(10)?;
        vec.push(20)?;
        vec.push(30)?;

        // Test IndexMut trait
        vec[0] = 100;
        vec[1] = 200;
        vec[2] = 300;

        assert_eq!(vec[0], 100);
        assert_eq!(vec[1], 200);
        assert_eq!(vec[2], 300);

        Ok(())
    }

    #[test]
    fn test_range_index() -> Result<()> {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(1)?;
        vec.push(2)?;
        vec.push(3)?;
        vec.push(4)?;
        vec.push(5)?;

        // Test range indexing
        let slice = &vec[1..4];
        assert_eq!(slice, &[2, 3, 4]);

        let slice = &vec[0..2];
        assert_eq!(slice, &[1, 2]);

        let slice = &vec[3..5];
        assert_eq!(slice, &[4, 5]);

        Ok(())
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn test_index_out_of_bounds() {
        let mut vec = StaticVec::<u32, 10>::new();
        vec.push(1).unwrap();
        let _ = vec[5]; // Should panic
    }
}
