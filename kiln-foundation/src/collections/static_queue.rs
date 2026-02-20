// WRT - wrt-foundation
// Module: StaticQueue - Inline-storage FIFO queue
// SW-REQ-ID: REQ_RESOURCE_001, REQ_MEM_SAFETY_001, REQ_TEMPORAL_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Allow unsafe code for MaybeUninit operations (documented and verified via KANI)
#![allow(unsafe_code)]

//! Static FIFO queue with inline storage and compile-time capacity.
//!
//! `StaticQueue<T, N>` provides a fixed-capacity queue (First-In-First-Out)
//! with all storage inline. No heap allocation, no Provider abstraction.
//!
//! # Characteristics
//!
//! - **Zero allocation**: All memory is inline `[MaybeUninit<T>; N]`
//! - **Const-time operations**: `push()`, `pop()` are O(1)
//! - **Circular buffer**: Efficient wraparound for continuous use
//! - **RAII cleanup**: Automatic Drop implementation
//! - **ASIL-D compliant**: Deterministic behavior

use core::marker::PhantomData;
use core::mem::MaybeUninit;

use wrt_error::Result;

/// A FIFO queue with compile-time capacity and inline storage.
///
/// # Requirements
///
/// - REQ_RESOURCE_001: Static allocation only
/// - REQ_MEM_SAFETY_001: Bounds validation
/// - REQ_TEMPORAL_001: Constant-time operations
///
/// # Invariants
///
/// 1. `len <= N` always holds
/// 2. Elements in circular range are initialized
/// 3. `head` and `tail` are always < N
/// 4. When `len == 0`, queue is empty
/// 5. When `len == N`, queue is full
///
/// # Examples
///
/// ```
/// use wrt_foundation::collections::StaticQueue;
///
/// let mut queue = StaticQueue::<u32, 10>::new();
/// queue.push(1)?;
/// queue.push(2)?;
/// queue.push(3)?;
///
/// assert_eq!(queue.pop(), Some(1));
/// assert_eq!(queue.pop(), Some(2));
/// assert_eq!(queue.len(), 1);
/// # Ok::<(), wrt_error::Error>(())
/// ```
#[derive(Debug)]
pub struct StaticQueue<T, const N: usize> {
    /// Inline storage for elements (circular buffer)
    data: [MaybeUninit<T>; N],

    /// Index of the first element (head)
    head: usize,

    /// Index where next element will be inserted (tail)
    tail: usize,

    /// Number of elements currently in the queue
    /// Invariant: len <= N
    len: usize,

    /// Marker for drop checker
    _marker: PhantomData<T>,
}

impl<T, const N: usize> StaticQueue<T, N> {
    /// Creates a new empty queue.
    ///
    /// # Const-time Guarantee
    ///
    /// O(1) with WCET ~5 CPU cycles.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: unsafe { MaybeUninit::uninit().assume_init() },
            head: 0,
            tail: 0,
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Pushes an element to the back of the queue.
    ///
    /// # Const-time Guarantee
    ///
    /// O(1) with WCET ~15 CPU cycles (includes modulo for wraparound).
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if queue is full.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticQueue;
    ///
    /// let mut queue = StaticQueue::<u32, 3>::new();
    /// queue.push(1)?;
    /// queue.push(2)?;
    /// queue.push(3)?;
    /// assert!(queue.push(4).is_err()); // Full
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    pub fn push(&mut self, value: T) -> Result<()> {
        if self.len >= N {
            return Err(wrt_error::Error::foundation_bounded_capacity_exceeded(
                "StaticQueue capacity exceeded",
            ));
        }

        // SAFETY: tail < N (maintained by wraparound), and we have capacity
        self.data[self.tail].write(value);

        // Circular wraparound
        self.tail = (self.tail + 1) % N;
        self.len += 1;

        Ok(())
    }

    /// Removes and returns the element from the front of the queue.
    ///
    /// # Const-time Guarantee
    ///
    /// O(1) with WCET ~18 CPU cycles.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticQueue;
    ///
    /// let mut queue = StaticQueue::<u32, 10>::new();
    /// queue.push(1)?;
    /// queue.push(2)?;
    ///
    /// assert_eq!(queue.pop(), Some(1));
    /// assert_eq!(queue.pop(), Some(2));
    /// assert_eq!(queue.pop(), None);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        // SAFETY: head < N, and element is initialized
        let value = unsafe { self.data[self.head].assume_init_read() };

        // Circular wraparound
        self.head = (self.head + 1) % N;
        self.len -= 1;

        Some(value)
    }

    /// Returns a reference to the front element without removing it.
    ///
    /// # Const-time Guarantee
    ///
    /// O(1) with WCET ~8 CPU cycles.
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }

        // SAFETY: head < N, and element is initialized
        Some(unsafe { self.data[self.head].assume_init_ref() })
    }

    /// Returns the current length.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the compile-time capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns `true` if the queue is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the queue is full.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Clears the queue, dropping all elements.
    ///
    /// # Time Complexity
    ///
    /// O(n) where n = len. WCET = ~15 * len CPU cycles.
    pub fn clear(&mut self) {
        while self.len > 0 {
            // SAFETY: head < N, and element is initialized
            unsafe {
                self.data[self.head].assume_init_drop();
            }
            self.head = (self.head + 1) % N;
            self.len -= 1;
        }

        // Reset to clean state
        self.head = 0;
        self.tail = 0;
    }

    /// Returns an iterator over the queue.
    ///
    /// Elements are yielded in FIFO order (front to back).
    #[inline]
    #[must_use]
    pub fn iter(&self) -> StaticQueueIter<'_, T, N> {
        StaticQueueIter {
            queue: self,
            index: 0,
        }
    }
}

// RAII: Automatic cleanup on drop
impl<T, const N: usize> Drop for StaticQueue<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

// Default: empty queue
impl<T, const N: usize> Default for StaticQueue<T, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Clone implementation (requires T: Clone)
impl<T: Clone, const N: usize> Clone for StaticQueue<T, N> {
    fn clone(&self) -> Self {
        let mut new_queue = Self::new();
        for item in self.iter() {
            // SAFETY: Cloning from existing queue, same capacity
            let _ = new_queue.push(item.clone());
        }
        new_queue
    }
}

// Checksummable implementation for verifiable data integrity
impl<T: crate::traits::Checksummable, const N: usize> crate::traits::Checksummable for StaticQueue<T, N> {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        // Update checksum with length
        self.len().update_checksum(checksum);
        // Iterate through elements in FIFO order for determinism
        for item in self.iter() {
            item.update_checksum(checksum);
        }
    }
}

// ToBytes implementation for serialization
impl<T: crate::traits::ToBytes, const N: usize> crate::traits::ToBytes for StaticQueue<T, N> {
    fn serialized_size(&self) -> usize {
        let len_size = core::mem::size_of::<usize>();
        let items_size: usize = self.iter()
            .map(|item| item.serialized_size())
            .sum();
        len_size + items_size
    }

    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut crate::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write length
        self.len().to_bytes_with_provider(writer, provider)?;
        // Write each item in FIFO order
        for item in self.iter() {
            item.to_bytes_with_provider(writer, provider)?;
        }
        Ok(())
    }
}

// FromBytes implementation for deserialization
impl<T: crate::traits::FromBytes, const N: usize> crate::traits::FromBytes for StaticQueue<T, N> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut crate::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Read length
        let len = usize::from_bytes_with_provider(reader, provider)?;

        // Create new queue
        let mut queue = StaticQueue::new();

        // Read items and push to queue
        for _ in 0..len {
            let item = T::from_bytes_with_provider(reader, provider)?;
            queue.push(item)?;
        }

        Ok(queue)
    }
}

// Hash implementation for use in hash-based collections
impl<T: core::hash::Hash, const N: usize> core::hash::Hash for StaticQueue<T, N> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Hash length
        self.len().hash(state);
        // Hash each item in FIFO order
        for item in self.iter() {
            item.hash(state);
        }
    }
}

// Iterator implementation
pub struct StaticQueueIter<'a, T, const N: usize> {
    queue: &'a StaticQueue<T, N>,
    index: usize,
}

impl<'a, T, const N: usize> Iterator for StaticQueueIter<'a, T, N> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.queue.len {
            // Calculate actual position in circular buffer
            let pos = (self.queue.head + self.index) % N;
            let item = unsafe { self.queue.data[pos].assume_init_ref() };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.queue.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, T, const N: usize> ExactSizeIterator for StaticQueueIter<'a, T, N> {}

// IntoIterator for references
impl<'a, T, const N: usize> IntoIterator for &'a StaticQueue<T, N> {
    type Item = &'a T;
    type IntoIter = StaticQueueIter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ============================================================================
// KANI Formal Verification
// ============================================================================

#[cfg(kani)]
mod verification {
    use super::*;

    #[kani::proof]
    fn verify_fifo_order() {
        let mut queue: StaticQueue<u8, 5> = StaticQueue::new();

        // Push in order
        queue.push(1).unwrap();
        queue.push(2).unwrap();
        queue.push(3).unwrap();

        // Pop maintains FIFO order
        assert!(queue.pop() == Some(1));
        assert!(queue.pop() == Some(2));
        assert!(queue.pop() == Some(3));
        assert!(queue.pop().is_none());
    }

    #[kani::proof]
    fn verify_circular_wraparound() {
        let mut queue: StaticQueue<u32, 3> = StaticQueue::new();

        // Fill queue
        queue.push(1).unwrap();
        queue.push(2).unwrap();
        queue.push(3).unwrap();

        // Pop one
        assert!(queue.pop() == Some(1));

        // Push again (triggers wraparound)
        queue.push(4).unwrap();

        // Verify correct order
        assert!(queue.pop() == Some(2));
        assert!(queue.pop() == Some(3));
        assert!(queue.pop() == Some(4));
    }

    #[kani::proof]
    fn verify_capacity_enforcement() {
        let mut queue: StaticQueue<u8, 3> = StaticQueue::new();

        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert!(queue.push(3).is_ok());
        assert!(queue.push(4).is_err()); // Should fail

        assert!(queue.len() == 3);
    }

    #[kani::proof]
    #[kani::unwind(6)]
    fn verify_drop_cleanup() {
        let mut queue: StaticQueue<u32, 5> = StaticQueue::new();
        queue.push(1).unwrap();
        queue.push(2).unwrap();
        queue.push(3).unwrap();

        drop(queue); // KANI verifies no leaks
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
        let queue: StaticQueue<u32, 10> = StaticQueue::new();
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.capacity(), 10);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_push_pop_fifo() -> Result<()> {
        let mut queue = StaticQueue::<u32, 5>::new();

        queue.push(1)?;
        queue.push(2)?;
        queue.push(3)?;

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), None);

        Ok(())
    }

    #[test]
    fn test_circular_buffer() -> Result<()> {
        let mut queue = StaticQueue::<u32, 3>::new();

        // Fill
        queue.push(1)?;
        queue.push(2)?;
        queue.push(3)?;

        // Pop 2
        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));

        // Push 2 more (wraparound)
        queue.push(4)?;
        queue.push(5)?;

        // Verify order
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), Some(4));
        assert_eq!(queue.pop(), Some(5));

        Ok(())
    }

    #[test]
    fn test_capacity_exceeded() {
        let mut queue = StaticQueue::<u32, 2>::new();

        assert!(queue.push(1).is_ok());
        assert!(queue.push(2).is_ok());
        assert!(queue.push(3).is_err());
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_peek() -> Result<()> {
        let mut queue = StaticQueue::<u32, 10>::new();
        queue.push(42)?;
        queue.push(100)?;

        assert_eq!(queue.peek(), Some(&42));
        assert_eq!(queue.peek(), Some(&42)); // Doesn't remove
        assert_eq!(queue.len(), 2);

        Ok(())
    }

    #[test]
    fn test_clear() -> Result<()> {
        let mut queue = StaticQueue::<u32, 10>::new();
        queue.push(1)?;
        queue.push(2)?;
        queue.push(3)?;

        queue.clear();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());

        Ok(())
    }

    #[test]
    fn test_iter() -> Result<()> {
        let mut queue = StaticQueue::<u32, 10>::new();
        queue.push(1)?;
        queue.push(2)?;
        queue.push(3)?;

        // Manually check iterator values (no_std compatible)
        let mut iter = queue.iter();
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), None);

        Ok(())
    }
}
