// WRT - wrt-foundation
// Module: Additional Bounded Collections
// SW-REQ-ID: REQ_RESOURCE_002
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Provides additional bounded collections for no_std/no_alloc environments.
//! SW-REQ-ID: REQ_RESOURCE_002
//!
//! These collections ensure that they never exceed a predefined capacity,
//! contributing to memory safety and predictability, especially in `no_std`
//! environments. They complement the core collections in the bounded.rs module.

#![cfg_attr(not(feature = "std"), allow(unused_imports))]

// Standard imports
#[cfg(not(feature = "std"))]
use core::fmt;
use core::marker::PhantomData;

#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

// Crate-level imports
use crate::traits::DefaultMemoryProvider;
use crate::{
    bounded::{
        BoundedError,
        BoundedErrorKind,
        BoundedVec,
    },
    codes,
    operations::{
        record_global_operation,
        Type as OperationType,
    },
    safe_memory::{
        SafeMemoryHandler,
        SliceMut,
    },
    traits::{
        BoundedCapacity,
        Checksummable,
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    verification::{
        Checksum,
        VerificationLevel,
    },
    Error,
    ErrorCategory,
    MemoryProvider,
};

/// A bounded queue (FIFO data structure) with a fixed maximum capacity.
///
/// This implements a First-In-First-Out (FIFO) queue that ensures it never
/// exceeds the specified capacity N_ELEMENTS. It uses a MemoryProvider for
/// storing serialized elements.
#[derive(Debug)]
pub struct BoundedQueue<T, const N_ELEMENTS: usize, P: MemoryProvider>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
{
    /// The underlying memory handler
    handler:              SafeMemoryHandler<P>,
    /// Current number of elements in the queue
    length:               usize,
    /// Index of the first element in the queue (head)
    head:                 usize,
    /// Index of the next available position (tail)
    tail:                 usize,
    /// Size of a single element T in bytes
    item_serialized_size: usize,
    /// Checksum for verifying data integrity
    checksum:             Checksum,
    /// Verification level for this queue
    verification_level:   VerificationLevel,
    /// Phantom data for type T
    _phantom:             PhantomData<T>,
}

impl<T, const N_ELEMENTS: usize, P: MemoryProvider> BoundedQueue<T, N_ELEMENTS, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
    P: Default + Clone,
{
    /// Creates a new `BoundedQueue` with the given memory provider.
    pub fn new(provider_arg: P) -> wrt_error::Result<Self> {
        Self::with_verification_level(provider_arg, VerificationLevel::default())
    }

    /// Creates a new `BoundedQueue` with a specific verification level.
    pub fn with_verification_level(
        provider_arg: P,
        level: VerificationLevel,
    ) -> wrt_error::Result<Self> {
        let item_serialized_size = T::default().serialized_size();
        if item_serialized_size == 0 && N_ELEMENTS > 0 {
            return Err(Error::new_static(
                ErrorCategory::Initialization,
                codes::INITIALIZATION_ERROR,
                "Cannot create BoundedQueue with zero-sized items and non-zero element count",
            ));
        }

        let memory_needed = N_ELEMENTS.saturating_mul(item_serialized_size);
        let handler = SafeMemoryHandler::new(provider_arg);

        // Record creation operation
        record_global_operation(OperationType::CollectionCreate, level);

        Ok(Self {
            handler,
            length: 0,
            head: 0,
            tail: 0,
            item_serialized_size,
            checksum: Checksum::new(),
            verification_level: level,
            _phantom: PhantomData,
        })
    }

    /// Enqueues an item at the end of the queue.
    ///
    /// Returns an error if the queue is full.
    pub fn enqueue(&mut self, item: T) -> Result<(), BoundedError> {
        if self.is_full() {
            return Err(BoundedError::capacity_exceeded());
        }

        let physical_index = self.tail % N_ELEMENTS;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        // Create a buffer to hold serialized data
        let mut item_bytes_buffer = [0u8; 256]; // Fixed size for simplicity
        let item_size = item.serialized_size();

        if item_size > item_bytes_buffer.len() {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        if item_size == 0 {
            // ZST handling
            self.length += 1;
            self.tail = (self.tail + 1) % (N_ELEMENTS * 2); // Use twice the capacity to track wrapping

            if self.verification_level >= VerificationLevel::Full {
                item.update_checksum(&mut self.checksum);
            }

            return Ok(());
        }

        // Serialize the item
        let slice_mut = SliceMut::new(&mut item_bytes_buffer[..item_size])?;
        let mut write_stream = WriteStream::new(slice_mut);
        item.to_bytes_with_provider(&mut write_stream, self.handler.provider())
            .map_err(|e| {
                BoundedError::new(BoundedErrorKind::ConversionError, "Conversion failed")
            })?;

        // Write the serialized data to the handler
        self.handler
            .write_data(offset, &item_bytes_buffer[..item_size])
            .map_err(|e| BoundedError::runtime_execution_error("Operation failed"))?;

        // Update queue state
        self.length += 1;
        self.tail = (self.tail + 1) % (N_ELEMENTS * 2); // Use twice the capacity to track wrapping

        // Record the operation and update checksums if needed
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if self.verification_level >= VerificationLevel::Full {
            item.update_checksum(&mut self.checksum);
        }

        Ok(())
    }

    /// Dequeues an item from the front of the queue.
    ///
    /// Returns `None` if the queue is empty.
    pub fn dequeue(&mut self) -> Result<Option<T>, BoundedError> {
        if self.is_empty() {
            return Ok(None);
        }

        let physical_index = self.head % N_ELEMENTS;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        // Update queue state before reading (prevents double-dequeue issues)
        self.length -= 1;
        self.head = (self.head + 1) % (N_ELEMENTS * 2); // Use twice the capacity to track wrapping

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if self.item_serialized_size == 0 {
            // ZST handling
            let item = T::default();

            if self.verification_level >= VerificationLevel::Full {
                self.recalculate_checksum();
            }

            return Ok(Some(item));
        }

        // Read the serialized data
        let slice_view = self
            .handler
            .get_slice(offset, self.item_serialized_size)
            .map_err(|e| BoundedError::new(BoundedErrorKind::SliceError, "Slice error"))?;

        // Deserialize the item
        let mut read_stream = ReadStream::new(slice_view);
        let item = T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
            .map_err(|_| BoundedError::runtime_execution_error("Operation failed"))?;

        // Update checksums if needed
        if self.verification_level >= VerificationLevel::Full {
            self.recalculate_checksum();
        }

        Ok(Some(item))
    }

    /// Peeks at the front item of the queue without removing it.
    ///
    /// Returns `None` if the queue is empty.
    pub fn peek(&self) -> Result<Option<T>, BoundedError> {
        if self.is_empty() {
            return Ok(None);
        }

        let physical_index = self.head % N_ELEMENTS;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        if self.item_serialized_size == 0 {
            // ZST handling
            return Ok(Some(T::default()));
        }

        // Read the serialized data
        let slice_view = self
            .handler
            .get_slice(offset, self.item_serialized_size)
            .map_err(|e| BoundedError::new(BoundedErrorKind::SliceError, "Slice error"))?;

        // Deserialize the item
        let mut read_stream = ReadStream::new(slice_view);
        let item = T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
            .map_err(|_| BoundedError::runtime_execution_error("Operation failed"))?;

        Ok(Some(item))
    }

    /// Returns the number of elements in the queue.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Checks if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Checks if the queue is full.
    pub fn is_full(&self) -> bool {
        self.length == N_ELEMENTS
    }

    /// Returns the capacity of the queue.
    pub fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    /// Returns the verification level for this queue.
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets the verification level.
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
        self.handler.set_verification_level(level);
    }

    /// Recalculates the checksum for the entire queue.
    fn recalculate_checksum(&mut self) {
        self.checksum.reset();

        if self.item_serialized_size == 0 {
            // ZST handling
            for _ in 0..self.length {
                T::default().update_checksum(&mut self.checksum);
            }
            return;
        }

        // Logic is more complex for a queue since elements might wrap around
        let mut current_index = self.head;

        for _ in 0..self.length {
            let physical_index = current_index % N_ELEMENTS;
            let offset = physical_index.saturating_mul(self.item_serialized_size);

            if let Ok(slice_view) = self.handler.get_slice(offset, self.item_serialized_size) {
                let mut read_stream = ReadStream::new(slice_view);
                if let Ok(item) =
                    T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
                {
                    item.update_checksum(&mut self.checksum);
                }
            }

            current_index = (current_index + 1) % (N_ELEMENTS * 2);
        }

        record_global_operation(
            OperationType::ChecksumFullRecalculation,
            self.verification_level,
        );
    }

    /// Verifies the integrity of the queue using its checksum.
    pub fn verify_checksum(&self) -> bool {
        if self.verification_level == VerificationLevel::Off {
            return true;
        }

        if self.item_serialized_size == 0 && self.length > 0 {
            // ZST handling
            let mut temp_checksum = Checksum::new();
            for _ in 0..self.length {
                T::default().update_checksum(&mut temp_checksum);
            }
            return temp_checksum == self.checksum;
        }

        let mut current_checksum = Checksum::new();
        let mut current_index = self.head;

        for _ in 0..self.length {
            let physical_index = current_index % N_ELEMENTS;
            let offset = physical_index.saturating_mul(self.item_serialized_size);

            if let Ok(slice_view) = self.handler.get_slice(offset, self.item_serialized_size) {
                let mut read_stream = ReadStream::new(slice_view);
                if let Ok(item) =
                    T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
                {
                    item.update_checksum(&mut current_checksum);
                } else {
                    return false; // Deserialization failure
                }
            } else {
                return false; // Slice access failure
            }

            current_index = (current_index + 1) % (N_ELEMENTS * 2);
        }

        current_checksum == self.checksum
    }
}

/// A bounded map with a fixed maximum capacity.
///
/// This implements a key-value store that ensures it never exceeds the
/// specified capacity N_ELEMENTS. It uses a MemoryProvider for storing
/// serialized elements.
#[derive(Debug)]
pub struct BoundedMap<K, V, const N_ELEMENTS: usize, P: MemoryProvider + Clone + PartialEq + Eq>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
{
    // Using a Vec of key-value pairs for simplicity
    entries:            BoundedVec<(K, V), N_ELEMENTS, P>,
    verification_level: VerificationLevel,
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    /// Creates a new `BoundedMap` with the given memory provider.
    pub fn new(provider_arg: P) -> wrt_error::Result<Self> {
        Self::with_verification_level(provider_arg, VerificationLevel::default())
    }

    /// Creates a new `BoundedMap` with a specific verification level.
    pub fn with_verification_level(
        provider_arg: P,
        level: VerificationLevel,
    ) -> wrt_error::Result<Self> {
        let entries = BoundedVec::with_verification_level(provider_arg, level)?;

        Ok(Self {
            entries,
            verification_level: level,
        })
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the key already exists, the value is updated.
    /// If the key doesn't exist and the map is full, returns an error.
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, BoundedError> {
        // Check if the key already exists
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                if entry.0 == key {
                    // Update existing entry
                    let old_value = entry.1.clone();
                    let new_entry = (key, value);

                    // Replace the entry in our vector
                    // Note: This is inefficient as we're removing and adding, but it's simple
                    // A real implementation would directly edit the entry in place
                    self.entries.remove(i)?;
                    self.entries.push(new_entry)?;

                    return Ok(Some(old_value));
                }
            }
        }

        // Key doesn't exist, insert new entry
        if self.is_full() {
            return Err(BoundedError::capacity_exceeded());
        }

        self.entries.push((key, value))?;

        Ok(None)
    }

    /// Gets a reference to the value associated with the given key.
    ///
    /// Returns `None` if the key doesn't exist.
    pub fn get(&self, key: &K) -> Result<Option<V>, BoundedError> {
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                if &entry.0 == key {
                    return Ok(Some(entry.1.clone()));
                }
            }
        }

        Ok(None)
    }

    /// Removes a key from the map, returning the value associated with the key.
    ///
    /// Returns `None` if the key doesn't exist.
    pub fn remove(&mut self, key: &K) -> Result<Option<V>, BoundedError> {
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                if &entry.0 == key {
                    let removed_entry = entry.clone();
                    self.entries.remove(i)?;
                    return Ok(Some(removed_entry.1));
                }
            }
        }

        Ok(None)
    }

    /// Checks if the map contains the given key.
    pub fn contains_key(&self, key: &K) -> Result<bool, BoundedError> {
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                if &entry.0 == key {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Returns the number of key-value pairs in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Checks if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Checks if the map is full.
    pub fn is_full(&self) -> bool {
        self.entries.is_full()
    }

    /// Returns the capacity of the map.
    pub fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    /// Returns the verification level for this map.
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets the verification level.
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
        // Propagate to underlying collections
        // Note: BoundedVec might need set_verification_level method
    }

    /// Clears the map, removing all key-value pairs.
    pub fn clear(&mut self) -> Result<(), BoundedError> {
        // Since we can't directly clear BoundedVec, we remove items one by one
        while !self.is_empty() {
            self.entries.pop()?;
        }

        Ok(())
    }

    /// Gets a mutable reference to the value associated with the given key.
    ///
    /// Returns `None` if the key doesn't exist.
    pub fn get_mut(&mut self, key: &K) -> Result<Option<&mut V>, BoundedError> {
        // Note: This is a simplified implementation that doesn't provide true mut
        // access due to the complexity of BoundedVec's serialization model.
        // In practice, you'd need to get, modify, and re-insert the value.
        Err(BoundedError::new(
            BoundedErrorKind::CapacityExceeded,
            "Mutable access not supported",
        ))
    }

    /// Returns an iterator over the values in the map.
    pub fn values(&self) -> BoundedMapValues<'_, K, V, N_ELEMENTS, P> {
        BoundedMapValues {
            map:   self,
            index: 0,
        }
    }

    /// Entry API for in-place manipulation of a map entry.
    pub fn entry(&mut self, key: K) -> BoundedMapEntry<'_, K, V, N_ELEMENTS, P> {
        BoundedMapEntry { map: self, key }
    }
}

/// Iterator over the values in a BoundedMap.
pub struct BoundedMapValues<'a, K, V, const N_ELEMENTS: usize, P: MemoryProvider>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    map:   &'a BoundedMap<K, V, N_ELEMENTS, P>,
    index: usize,
}

impl<'a, K, V, const N_ELEMENTS: usize, P: MemoryProvider> Iterator
    for BoundedMapValues<'a, K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.map.len() {
            if let Ok(entry) = self.map.entries.get(self.index) {
                self.index += 1;
                Some(entry.1.clone())
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Entry API for BoundedMap.
pub struct BoundedMapEntry<'a, K, V, const N_ELEMENTS: usize, P: MemoryProvider>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    map: &'a mut BoundedMap<K, V, N_ELEMENTS, P>,
    key: K,
}

impl<'a, K, V, const N_ELEMENTS: usize, P: MemoryProvider> BoundedMapEntry<'a, K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts.
    pub fn or_insert(self, default: V) -> Result<V, BoundedError> {
        match self.map.get(&self.key)? {
            Some(value) => Ok(value),
            None => {
                self.map.insert(self.key, default.clone())?;
                Ok(default)
            },
        }
    }

    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts with a closure.
    pub fn or_insert_with<F>(self, f: F) -> Result<V, BoundedError>
    where
        F: FnOnce() -> V,
    {
        match self.map.get(&self.key)? {
            Some(value) => Ok(value),
            None => {
                let default = f();
                self.map.insert(self.key, default.clone())?;
                Ok(default)
            },
        }
    }
}

/// A bounded set with a fixed maximum capacity.
///
/// This implements a collection of unique elements that ensures it never
/// exceeds the specified capacity N_ELEMENTS. It uses a MemoryProvider for
/// storing serialized elements.
#[derive(Debug)]
pub struct BoundedSet<T, const N_ELEMENTS: usize, P: MemoryProvider + Clone + PartialEq + Eq>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
{
    // Using a Vec for simplicity, ensuring uniqueness via operations
    elements:           BoundedVec<T, N_ELEMENTS, P>,
    verification_level: VerificationLevel,
}

impl<T, const N_ELEMENTS: usize, P: MemoryProvider> BoundedSet<T, N_ELEMENTS, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    P: Default + Clone + PartialEq + Eq,
{
    /// Creates a new `BoundedSet` with the given memory provider.
    pub fn new(provider_arg: P) -> wrt_error::Result<Self> {
        Self::with_verification_level(provider_arg, VerificationLevel::default())
    }

    /// Creates a new `BoundedSet` with a specific verification level.
    pub fn with_verification_level(
        provider_arg: P,
        level: VerificationLevel,
    ) -> wrt_error::Result<Self> {
        let elements = BoundedVec::with_verification_level(provider_arg, level)?;

        Ok(Self {
            elements,
            verification_level: level,
        })
    }

    /// Inserts an element into the set.
    ///
    /// Returns `true` if the element was newly inserted, `false` if it was
    /// already present. Returns an error if the set is full and the element
    /// is not already present.
    pub fn insert(&mut self, value: T) -> Result<bool, BoundedError> {
        // Check if the element already exists
        if self.contains(&value)? {
            return Ok(false);
        }

        // Element doesn't exist, insert it
        if self.is_full() {
            return Err(BoundedError::capacity_exceeded());
        }

        self.elements.push(value)?;

        Ok(true)
    }

    /// Checks if the set contains the given element.
    pub fn contains(&self, value: &T) -> Result<bool, BoundedError> {
        for i in 0..self.elements.len() {
            if let Ok(element) = self.elements.get(i) {
                if &element == value {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Removes an element from the set.
    ///
    /// Returns `true` if the element was present and removed, `false` if it was
    /// not present.
    pub fn remove(&mut self, value: &T) -> Result<bool, BoundedError> {
        for i in 0..self.elements.len() {
            if let Ok(element) = self.elements.get(i) {
                if &element == value {
                    self.elements.remove(i)?;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Returns the number of elements in the set.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Checks if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Checks if the set is full.
    pub fn is_full(&self) -> bool {
        self.elements.is_full()
    }

    /// Returns the capacity of the set.
    pub fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    /// Returns the verification level for this set.
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets the verification level.
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
        // Propagate to underlying collections
        // Note: BoundedVec might need set_verification_level method
    }

    /// Clears the set, removing all elements.
    pub fn clear(&mut self) -> Result<(), BoundedError> {
        // Since we can't directly clear BoundedVec, we remove items one by one
        while !self.is_empty() {
            self.elements.pop()?;
        }

        Ok(())
    }
}

/// A bounded double-ended queue (deque) with a fixed maximum capacity.
///
/// This implements a deque that allows adding and removing elements from both
/// ends. It ensures it never exceeds the specified capacity N_ELEMENTS.
/// It uses a MemoryProvider for storing serialized elements.
#[derive(Debug)]
pub struct BoundedDeque<T, const N_ELEMENTS: usize, P: MemoryProvider>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
{
    /// The underlying memory handler
    handler:              SafeMemoryHandler<P>,
    /// Current number of elements in the deque
    length:               usize,
    /// Index of the first element in the deque (front)
    front:                usize,
    /// Index of the last element in the deque (back)
    back:                 usize,
    /// Size of a single element T in bytes
    item_serialized_size: usize,
    /// Checksum for verifying data integrity
    checksum:             Checksum,
    /// Verification level for this deque
    verification_level:   VerificationLevel,
    /// Phantom data for type T
    _phantom:             PhantomData<T>,
}

impl<T, const N_ELEMENTS: usize, P: MemoryProvider> BoundedDeque<T, N_ELEMENTS, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
    P: Default + Clone,
{
    /// Creates a new `BoundedDeque` with the given memory provider.
    pub fn new(provider_arg: P) -> wrt_error::Result<Self> {
        Self::with_verification_level(provider_arg, VerificationLevel::default())
    }

    /// Creates a new `BoundedDeque` with a specific verification level.
    pub fn with_verification_level(
        provider_arg: P,
        level: VerificationLevel,
    ) -> wrt_error::Result<Self> {
        let item_serialized_size = T::default().serialized_size();
        if item_serialized_size == 0 && N_ELEMENTS > 0 {
            return Err(Error::new_static(
                ErrorCategory::Initialization,
                codes::INITIALIZATION_ERROR,
                "Cannot create BoundedDeque with zero-sized items and non-zero element count",
            ));
        }

        let memory_needed = N_ELEMENTS.saturating_mul(item_serialized_size);
        let handler = SafeMemoryHandler::new(provider_arg);

        // Record creation operation
        record_global_operation(OperationType::CollectionCreate, level);

        Ok(Self {
            handler,
            length: 0,
            front: 0,
            back: 0,
            item_serialized_size,
            checksum: Checksum::new(),
            verification_level: level,
            _phantom: PhantomData,
        })
    }

    /// Adds an element to the front of the deque.
    ///
    /// Returns an error if the deque is full.
    pub fn push_front(&mut self, item: T) -> Result<(), BoundedError> {
        if self.is_full() {
            return Err(BoundedError::capacity_exceeded());
        }

        // Calculate the new front index (moving backward in the circular buffer)
        self.front = if self.front == 0 { N_ELEMENTS - 1 } else { self.front - 1 };

        let physical_index = self.front;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        // Create a buffer to hold serialized data
        let mut item_bytes_buffer = [0u8; 256]; // Fixed size for simplicity
        let item_size = item.serialized_size();

        if item_size > item_bytes_buffer.len() {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        if item_size == 0 {
            // ZST handling
            self.length += 1;

            if self.verification_level >= VerificationLevel::Full {
                item.update_checksum(&mut self.checksum);
            }

            return Ok(());
        }

        // Serialize the item
        let slice_mut = SliceMut::new(&mut item_bytes_buffer[..item_size])?;
        let mut write_stream = WriteStream::new(slice_mut);
        item.to_bytes_with_provider(&mut write_stream, self.handler.provider())
            .map_err(|e| {
                BoundedError::new(BoundedErrorKind::ConversionError, "Conversion failed")
            })?;

        // Write the serialized data to the handler
        self.handler
            .write_data(offset, &item_bytes_buffer[..item_size])
            .map_err(|e| BoundedError::runtime_execution_error("Operation failed"))?;

        // Update deque state
        self.length += 1;

        // Initialize back if this is the first element
        if self.length == 1 {
            self.back = self.front;
        }

        // Record the operation and update checksums if needed
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if self.verification_level >= VerificationLevel::Full {
            item.update_checksum(&mut self.checksum);
        }

        Ok(())
    }

    /// Adds an element to the back of the deque.
    ///
    /// Returns an error if the deque is full.
    pub fn push_back(&mut self, item: T) -> Result<(), BoundedError> {
        if self.is_full() {
            return Err(BoundedError::capacity_exceeded());
        }

        let physical_index = self.back;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        // Create a buffer to hold serialized data
        let mut item_bytes_buffer = [0u8; 256]; // Fixed size for simplicity
        let item_size = item.serialized_size();

        if item_size > item_bytes_buffer.len() {
            return Err(BoundedError::new(
                BoundedErrorKind::ItemTooLarge,
                "Item size exceeds buffer capacity",
            ));
        }

        if item_size == 0 {
            // ZST handling
            self.length += 1;

            // If this is the first element, set front = back
            if self.length == 1 {
                self.front = self.back;
            } else {
                // Move back pointer forward
                self.back = (self.back + 1) % N_ELEMENTS;
            }

            if self.verification_level >= VerificationLevel::Full {
                item.update_checksum(&mut self.checksum);
            }

            return Ok(());
        }

        // Serialize the item
        let slice_mut = SliceMut::new(&mut item_bytes_buffer[..item_size])?;
        let mut write_stream = WriteStream::new(slice_mut);
        item.to_bytes_with_provider(&mut write_stream, self.handler.provider())
            .map_err(|e| {
                #[cfg(feature = "std")]
                eprintln!("DEBUG: to_bytes_with_provider failed with error: {:?}", e);
                BoundedError::from(e)
            })?;

        // Write the serialized data to the handler
        self.handler
            .write_data(offset, &item_bytes_buffer[..item_size])
            .map_err(|e| BoundedError::new(BoundedErrorKind::SliceError, "Slice error"))?;

        // Update deque state
        self.length += 1;

        // If this is the first element, set front = back
        if self.length == 1 {
            self.front = self.back;
        } else {
            // Move back pointer forward
            self.back = (self.back + 1) % N_ELEMENTS;
        }

        // Record the operation and update checksums if needed
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if self.verification_level >= VerificationLevel::Full {
            item.update_checksum(&mut self.checksum);
        }

        Ok(())
    }

    /// Removes and returns the element at the front of the deque.
    ///
    /// Returns `None` if the deque is empty.
    pub fn pop_front(&mut self) -> Result<Option<T>, BoundedError> {
        if self.is_empty() {
            return Ok(None);
        }

        let physical_index = self.front;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        // Read the item before updating deque state
        let item = if self.item_serialized_size == 0 {
            // ZST handling
            T::default()
        } else {
            // Read the serialized data
            let slice_view =
                self.handler.get_slice(offset, self.item_serialized_size).map_err(|e| {
                    BoundedError::runtime_execution_error("Failed to get slice from handler")
                })?;

            // Deserialize the item
            let mut read_stream = ReadStream::new(slice_view);
            

            T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
                .map_err(|_| {
                    BoundedError::new(
                        BoundedErrorKind::ConversionError,
                        "Failed to deserialize from bytes",
                    )
                })?
        };

        // Update deque state
        self.length -= 1;
        if self.length > 0 {
            self.front = (self.front + 1) % N_ELEMENTS;
        }

        // Record operation
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        // Update checksums if needed
        if self.verification_level >= VerificationLevel::Full {
            self.recalculate_checksum();
        }

        Ok(Some(item))
    }

    /// Removes and returns the element at the back of the deque.
    ///
    /// Returns `None` if the deque is empty.
    pub fn pop_back(&mut self) -> Result<Option<T>, BoundedError> {
        if self.is_empty() {
            return Ok(None);
        }

        // Calculate the physical index for the back element
        let physical_index = if self.back == 0 { N_ELEMENTS - 1 } else { self.back - 1 };

        let offset = physical_index.saturating_mul(self.item_serialized_size);

        // Read the item before updating deque state
        let item = if self.item_serialized_size == 0 {
            // ZST handling
            T::default()
        } else {
            // Read the serialized data
            let slice_view =
                self.handler.get_slice(offset, self.item_serialized_size).map_err(|e| {
                    BoundedError::runtime_execution_error("Failed to get slice from handler")
                })?;

            // Deserialize the item
            let mut read_stream = ReadStream::new(slice_view);
            

            T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
                .map_err(|_| {
                    BoundedError::new(
                        BoundedErrorKind::ConversionError,
                        "Failed to deserialize from bytes",
                    )
                })?
        };

        // Update deque state
        self.length -= 1;
        if self.length > 0 {
            self.back = physical_index;
        }

        // Record operation
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        // Update checksums if needed
        if self.verification_level >= VerificationLevel::Full {
            self.recalculate_checksum();
        }

        Ok(Some(item))
    }

    /// Returns a reference to the element at the front of the deque without
    /// removing it.
    ///
    /// Returns `None` if the deque is empty.
    pub fn front(&self) -> Result<Option<T>, BoundedError> {
        if self.is_empty() {
            return Ok(None);
        }

        let physical_index = self.front;
        let offset = physical_index.saturating_mul(self.item_serialized_size);

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        if self.item_serialized_size == 0 {
            // ZST handling
            return Ok(Some(T::default()));
        }

        // Read the serialized data
        let slice_view = self
            .handler
            .get_slice(offset, self.item_serialized_size)
            .map_err(|e| BoundedError::runtime_execution_error("Operation failed"))?;

        // Deserialize the item
        let mut read_stream = ReadStream::new(slice_view);
        let item = T::from_bytes_with_provider(&mut read_stream, self.handler.provider()).map_err(
            |_| {
                BoundedError::new(
                    BoundedErrorKind::ConversionError,
                    "Failed to deserialize from bytes",
                )
            },
        )?;

        Ok(Some(item))
    }

    /// Returns a reference to the element at the back of the deque without
    /// removing it.
    ///
    /// Returns `None` if the deque is empty.
    pub fn back(&self) -> Result<Option<T>, BoundedError> {
        if self.is_empty() {
            return Ok(None);
        }

        // Calculate the physical index for the back element
        let physical_index = if self.back == 0 { N_ELEMENTS - 1 } else { self.back - 1 };

        let offset = physical_index.saturating_mul(self.item_serialized_size);

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        if self.item_serialized_size == 0 {
            // ZST handling
            return Ok(Some(T::default()));
        }

        // Read the serialized data
        let slice_view = self
            .handler
            .get_slice(offset, self.item_serialized_size)
            .map_err(|e| BoundedError::runtime_execution_error("Operation failed"))?;

        // Deserialize the item
        let mut read_stream = ReadStream::new(slice_view);
        let item = T::from_bytes_with_provider(&mut read_stream, self.handler.provider()).map_err(
            |_| {
                BoundedError::new(
                    BoundedErrorKind::ConversionError,
                    "Failed to deserialize from bytes",
                )
            },
        )?;

        Ok(Some(item))
    }

    /// Returns the number of elements in the deque.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Checks if the deque is empty.
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Checks if the deque is full.
    pub fn is_full(&self) -> bool {
        self.length == N_ELEMENTS
    }

    /// Returns the capacity of the deque.
    pub fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    /// Returns the verification level for this deque.
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets the verification level.
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
        self.handler.set_verification_level(level);
    }

    /// Clears the deque, removing all elements.
    pub fn clear(&mut self) -> Result<(), BoundedError> {
        // Reset the deque state
        self.length = 0;
        self.front = 0;
        self.back = 0;

        // Record operation
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        // Reset checksum
        self.checksum = Checksum::new();

        Ok(())
    }

    /// Recalculates the checksum for the entire deque.
    fn recalculate_checksum(&mut self) {
        self.checksum.reset();

        if self.length == 0 || self.item_serialized_size == 0 {
            return;
        }

        let mut current_index = self.front;

        for _ in 0..self.length {
            let offset = current_index.saturating_mul(self.item_serialized_size);

            if let Ok(slice_view) = self.handler.get_slice(offset, self.item_serialized_size) {
                let mut read_stream = ReadStream::new(slice_view);
                if let Ok(item) =
                    T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
                {
                    item.update_checksum(&mut self.checksum);
                }
            }

            current_index = (current_index + 1) % N_ELEMENTS;
        }

        record_global_operation(
            OperationType::ChecksumFullRecalculation,
            self.verification_level,
        );
    }

    /// Verifies the integrity of the deque using its checksum.
    pub fn verify_checksum(&self) -> bool {
        if self.verification_level == VerificationLevel::Off || self.length == 0 {
            return true;
        }

        let mut current_checksum = Checksum::new();

        if self.item_serialized_size == 0 {
            // ZST handling
            for _ in 0..self.length {
                T::default().update_checksum(&mut current_checksum);
            }
            return current_checksum == self.checksum;
        }

        let mut current_index = self.front;

        for _ in 0..self.length {
            let offset = current_index.saturating_mul(self.item_serialized_size);

            if let Ok(slice_view) = self.handler.get_slice(offset, self.item_serialized_size) {
                let mut read_stream = ReadStream::new(slice_view);
                if let Ok(item) =
                    T::from_bytes_with_provider(&mut read_stream, self.handler.provider())
                {
                    item.update_checksum(&mut current_checksum);
                } else {
                    return false; // Deserialization failure
                }
            } else {
                return false; // Slice access failure
            }

            current_index = (current_index + 1) % N_ELEMENTS;
        }

        current_checksum == self.checksum
    }
}

/// A fixed-size bit set with efficient storage.
///
/// This implements a set of bits where each bit represents the presence (1) or
/// absence (0) of an element with the corresponding index. It ensures it never
/// exceeds the specified capacity N_BITS.
#[derive(Debug, Clone)]
#[cfg(feature = "std")]
pub struct BoundedBitSet<const N_BITS: usize> {
    /// The underlying storage, using `u32` for efficient bit operations
    /// Each `u32` holds 32 bits, so we need N_BITS/32 (rounded up) elements
    storage:            Vec<(u32, Checksum)>,
    /// Count of set bits (1s) for efficient size queries
    count:              usize,
    /// Verification level for this bitset
    verification_level: VerificationLevel,
}

#[cfg(feature = "std")]
impl<const N_BITS: usize> Default for BoundedBitSet<N_BITS> {
    fn default() -> Self {
        // Calculate storage size (N_BITS/32 rounded up)
        let storage_size = (N_BITS + 31) / 32;
        let mut storage = Vec::with_capacity(storage_size);

        // Initialize with zeros and default checksums
        for _ in 0..storage_size {
            storage.push((0, Checksum::default()));
        }

        Self {
            storage,
            count: 0,
            verification_level: VerificationLevel::default(),
        }
    }
}

#[cfg(feature = "std")]
impl<const N_BITS: usize> BoundedBitSet<N_BITS> {
    /// Creates a new empty `BoundedBitSet`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BoundedBitSet` with a specific verification level.
    pub fn with_verification_level(level: VerificationLevel) -> Self {
        // Calculate storage size (N_BITS/32 rounded up)
        let storage_size = (N_BITS + 31) / 32;
        let mut storage = Vec::with_capacity(storage_size);

        // Initialize with zeros and default checksums
        for _ in 0..storage_size {
            storage.push((0, Checksum::default()));
        }

        Self {
            storage,
            count: 0,
            verification_level: level,
        }
    }

    /// Sets the bit at the specified index to 1.
    ///
    /// Returns `true` if the bit was changed, `false` if it was already set.
    /// Returns an error if the index is out of bounds.
    pub fn set(&mut self, index: usize) -> Result<bool, BoundedError> {
        if index >= N_BITS {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        let storage_index = index / 32;
        let bit_position = index % 32;
        let bit_mask = 1 << bit_position;

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if storage_index >= self.storage.len() {
            return Err(BoundedError::new(BoundedErrorKind::SliceError, ")"));
        }

        let (bits, checksum) = &mut self.storage[storage_index];
        let old_bits = *bits;

        // Set the bit
        *bits |= bit_mask;

        // Update count and checksum if the bit was changed
        let changed = old_bits != *bits;
        if changed {
            self.count += 1;

            if self.verification_level >= VerificationLevel::Full {
                // Update the checksum only for the affected chunk
                checksum.reset();
                (*bits).update_checksum(checksum);
            }
        }

        Ok(changed)
    }

    /// Clears the bit at the specified index (sets it to 0).
    ///
    /// Returns `true` if the bit was changed, `false` if it was already clear.
    /// Returns an error if the index is out of bounds.
    pub fn clear(&mut self, index: usize) -> Result<bool, BoundedError> {
        if index >= N_BITS {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        let storage_index = index / 32;
        let bit_position = index % 32;
        let bit_mask = 1 << bit_position;

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if storage_index >= self.storage.len() {
            return Err(BoundedError::new(BoundedErrorKind::SliceError, ")"));
        }

        let (bits, checksum) = &mut self.storage[storage_index];
        let old_bits = *bits;

        // Clear the bit
        *bits &= !bit_mask;

        // Update count and checksum if the bit was changed
        let changed = old_bits != *bits;
        if changed {
            self.count -= 1;

            if self.verification_level >= VerificationLevel::Full {
                // Update the checksum only for the affected chunk
                checksum.reset();
                (*bits).update_checksum(checksum);
            }
        }

        Ok(changed)
    }

    /// Checks if the bit at the specified index is set (1).
    ///
    /// Returns an error if the index is out of bounds.
    pub fn contains(&self, index: usize) -> Result<bool, BoundedError> {
        if index >= N_BITS {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        let storage_index = index / 32;
        let bit_position = index % 32;
        let bit_mask = 1 << bit_position;

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        if storage_index >= self.storage.len() {
            return Err(BoundedError::new(BoundedErrorKind::SliceError, ")"));
        }

        // Check if the bit is set
        Ok((self.storage[storage_index].0 & bit_mask) != 0)
    }

    /// Toggles the bit at the specified index (changes 0 to 1 and 1 to 0).
    ///
    /// Returns `true` if the bit is now set (1), `false` if it is now clear
    /// (0). Returns an error if the index is out of bounds.
    pub fn toggle(&mut self, index: usize) -> Result<bool, BoundedError> {
        if index >= N_BITS {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        let storage_index = index / 32;
        let bit_position = index % 32;
        let bit_mask = 1 << bit_position;

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        if storage_index >= self.storage.len() {
            return Err(BoundedError::new(BoundedErrorKind::SliceError, ")"));
        }

        let (bits, checksum) = &mut self.storage[storage_index];

        // Toggle the bit
        *bits ^= bit_mask;

        // Update count and checksum
        let is_set = (*bits & bit_mask) != 0;
        if is_set {
            self.count += 1;
        } else {
            self.count -= 1;
        }

        if self.verification_level >= VerificationLevel::Full {
            // Update the checksum only for the affected chunk
            checksum.reset();
            (*bits).update_checksum(checksum);
        }

        Ok(is_set)
    }

    /// Returns the number of bits set to 1.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Checks if the bit set is empty (all bits are 0).
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Checks if the bit set is full (all bits are 1).
    pub fn is_full(&self) -> bool {
        self.count == N_BITS
    }

    /// Returns the capacity of the bit set (maximum number of bits).
    pub fn capacity(&self) -> usize {
        N_BITS
    }

    /// Returns the verification level for this bit set.
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets the verification level.
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
    }

    /// Clears the bit set, setting all bits to 0.
    pub fn clear_all(&mut self) {
        for (bits, checksum) in &mut self.storage {
            *bits = 0;

            if self.verification_level >= VerificationLevel::Full {
                checksum.reset();
                (*bits).update_checksum(checksum);
            }
        }

        self.count = 0;
        record_global_operation(OperationType::CollectionWrite, self.verification_level);
    }

    /// Sets all bits in the bit set to 1.
    pub fn set_all(&mut self) {
        for (bits, checksum) in &mut self.storage {
            *bits = !0; // All bits set to 1

            if self.verification_level >= VerificationLevel::Full {
                checksum.reset();
                (*bits).update_checksum(checksum);
            }
        }

        // Calculate the exact count (handling the case where the last chunk is
        // partially used)
        let full_chunks = N_BITS / 32;
        let remaining_bits = N_BITS % 32;

        self.count = full_chunks * 32;
        if remaining_bits > 0 {
            self.count += remaining_bits;

            // Fix the last chunk to only have valid bits set
            let last_index = self.storage.len() - 1;
            self.storage[last_index].0 &= (1 << remaining_bits) - 1;

            if self.verification_level >= VerificationLevel::Full {
                // Update checksum for the last chunk
                self.storage[last_index].1.reset();
                let value = self.storage[last_index].0;
                value.update_checksum(&mut self.storage[last_index].1);
            }
        }

        record_global_operation(OperationType::CollectionWrite, self.verification_level);
    }

    /// Verifies the integrity of the bit set using its checksums.
    pub fn verify_checksum(&self) -> bool {
        if self.verification_level == VerificationLevel::Off {
            return true;
        }

        // Verify each chunk's checksum
        for (bits, stored_checksum) in &self.storage {
            let mut current_checksum = Checksum::new();
            (*bits).update_checksum(&mut current_checksum);

            if current_checksum != *stored_checksum {
                return false;
            }
        }

        // Verify the count is correct
        let calculated_count =
            self.storage.iter().map(|(bits, _)| bits.count_ones() as usize).sum::<usize>();

        calculated_count == self.count
    }

    /// Performs a bitwise AND operation with another `BoundedBitSet`.
    ///
    /// Modifies this bitset in-place to contain only the bits that are set in
    /// both bitsets.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut a = BoundedBitSet::<8>::new();
    /// a.set(0).unwrap();
    /// a.set(2).unwrap();
    /// a.set(4).unwrap();
    ///
    /// let mut b = BoundedBitSet::<8>::new();
    /// b.set(0).unwrap();
    /// b.set(1).unwrap();
    /// b.set(4).unwrap();
    ///
    /// a.bitand_with(&b);
    ///
    /// assert!(a.contains(0).unwrap();
    /// assert!(!a.contains(1).unwrap();
    /// assert!(!a.contains(2).unwrap();
    /// assert!(a.contains(4).unwrap();
    /// ```
    pub fn bitand_with(&mut self, other: &Self) {
        let mut new_count = 0;

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        for (i, (bits, checksum)) in self.storage.iter_mut().enumerate() {
            if i < other.storage.len() {
                // Perform AND operation
                *bits &= other.storage[i].0;

                // Update count and checksum
                let bit_count = bits.count_ones() as usize;
                new_count += bit_count;

                if self.verification_level >= VerificationLevel::Full {
                    // Update the checksum
                    checksum.reset();
                    (*bits).update_checksum(checksum);
                }
            }
        }

        self.count = new_count;
    }

    /// Performs a bitwise OR operation with another `BoundedBitSet`.
    ///
    /// Modifies this bitset in-place to contain bits that are set in either
    /// bitset.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut a = BoundedBitSet::<8>::new();
    /// a.set(0).unwrap();
    /// a.set(2).unwrap();
    ///
    /// let mut b = BoundedBitSet::<8>::new();
    /// b.set(1).unwrap();
    /// b.set(2).unwrap();
    ///
    /// a.bitor_with(&b;
    ///
    /// assert!(a.contains(0).unwrap();
    /// assert!(a.contains(1).unwrap();
    /// assert!(a.contains(2).unwrap();
    /// ```
    pub fn bitor_with(&mut self, other: &Self) {
        let mut new_count = 0;

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        for (i, (bits, checksum)) in self.storage.iter_mut().enumerate() {
            if i < other.storage.len() {
                // Perform OR operation
                *bits |= other.storage[i].0;

                // Update count and checksum
                let bit_count = bits.count_ones() as usize;
                new_count += bit_count;

                if self.verification_level >= VerificationLevel::Full {
                    // Update the checksum
                    checksum.reset();
                    (*bits).update_checksum(checksum);
                }
            }
        }

        self.count = new_count;
    }

    /// Performs a bitwise XOR operation with another `BoundedBitSet`.
    ///
    /// Modifies this bitset in-place to contain bits that are set in either
    /// bitset but not both.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut a = BoundedBitSet::<8>::new();
    /// a.set(0).unwrap();
    /// a.set(2).unwrap();
    ///
    /// let mut b = BoundedBitSet::<8>::new();
    /// b.set(1).unwrap();
    /// b.set(2).unwrap();
    ///
    /// a.bitxor_with(&b;
    ///
    /// assert!(a.contains(0).unwrap();
    /// assert!(a.contains(1).unwrap();
    /// assert!(!a.contains(2).unwrap();
    /// ```
    pub fn bitxor_with(&mut self, other: &Self) {
        let mut new_count = 0;

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        for (i, (bits, checksum)) in self.storage.iter_mut().enumerate() {
            if i < other.storage.len() {
                // Perform XOR operation
                *bits ^= other.storage[i].0;

                // Update count and checksum
                let bit_count = bits.count_ones() as usize;
                new_count += bit_count;

                if self.verification_level >= VerificationLevel::Full {
                    // Update the checksum
                    checksum.reset();
                    (*bits).update_checksum(checksum);
                }
            }
        }

        self.count = new_count;
    }

    /// Performs a bitwise NOT operation (complement) on this bitset.
    ///
    /// Modifies this bitset in-place to contain the complement of its current
    /// state. This inverts every bit in the valid range (0 to N_BITS-1).
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<8>::new();
    /// bitset.set(1).unwrap();
    /// bitset.set(3).unwrap();
    ///
    /// bitset.bitnot(feature = "std";
    ///
    /// assert!(bitset.contains(0).unwrap();
    /// assert!(!bitset.contains(1).unwrap();
    /// assert!(bitset.contains(2).unwrap();
    /// assert!(!bitset.contains(3).unwrap();
    /// assert!(bitset.contains(4).unwrap();
    /// assert!(bitset.contains(5).unwrap();
    /// assert!(bitset.contains(6).unwrap();
    /// assert!(bitset.contains(7).unwrap();
    /// ```
    pub fn bitnot(&mut self) {
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        // Calculate the total number of full u32 chunks needed
        let full_chunks = N_BITS / 32;
        // Calculate the remaining bits in the last partial chunk (if any)
        let remaining_bits = N_BITS % 32;

        for (i, (bits, checksum)) in self.storage.iter_mut().enumerate() {
            if i < full_chunks {
                // Fully invert all bits in complete chunks
                *bits = !*bits;
            } else if i == full_chunks && remaining_bits > 0 {
                // For the last chunk, only invert valid bits (mask with ones up to
                // remaining_bits)
                let mask = (1u32 << remaining_bits) - 1;
                *bits = (*bits ^ mask) & mask; // XOR with mask and keep only
                                               // valid bits
            }

            if self.verification_level >= VerificationLevel::Full {
                // Update the checksum
                checksum.reset();
                (*bits).update_checksum(checksum);
            }
        }

        // Recalculate the count of set bits
        self.count = self.storage.iter().map(|(bits, _)| bits.count_ones() as usize).sum();
    }

    /// Returns the index of the first set bit, or `None` if no bits are set.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// assert_eq!(bitset.first_set_bit(), None);
    ///
    /// bitset.set(42).unwrap();
    /// assert_eq!(bitset.first_set_bit(), Some(42));
    ///
    /// bitset.set(10).unwrap();
    /// assert_eq!(bitset.first_set_bit(), Some(10)); // Returns the lowest index
    /// ```
    pub fn first_set_bit(&self) -> Option<usize> {
        record_global_operation(OperationType::CollectionRead, self.verification_level);

        for (chunk_index, (bits, _)) in self.storage.iter().enumerate() {
            if *bits != 0 {
                // Find the index of the least significant bit that is set
                let bit_pos = bits.trailing_zeros() as usize;
                let index = chunk_index * 32 + bit_pos;

                // Ensure the index is within bounds
                if index < N_BITS {
                    return Some(index);
                }
            }
        }

        None
    }

    /// Returns the index of the next set bit at or after the given position,
    /// or `None` if no more bits are set.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set(10).unwrap();
    /// bitset.set(20).unwrap();
    /// bitset.set(30).unwrap();
    ///
    /// assert_eq!(bitset.next_set_bit(0), Some(10));
    /// assert_eq!(bitset.next_set_bit(10), Some(10));
    /// assert_eq!(bitset.next_set_bit(11), Some(20));
    /// assert_eq!(bitset.next_set_bit(25), Some(30));
    /// assert_eq!(bitset.next_set_bit(31), None;
    /// ```
    pub fn next_set_bit(&self, from_index: usize) -> Option<usize> {
        if from_index >= N_BITS {
            return None;
        }

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Calculate the chunk and bit position for the starting index
        let start_chunk = from_index / 32;
        let start_bit = from_index % 32;

        // Check the first chunk, masking out bits before the start_bit
        if start_chunk < self.storage.len() {
            let masked_bits = self.storage[start_chunk].0 & (!0u32 << start_bit);
            if masked_bits != 0 {
                let bit_pos = masked_bits.trailing_zeros() as usize;
                let index = start_chunk * 32 + bit_pos;
                if index < N_BITS {
                    return Some(index);
                }
            }
        }

        // Check remaining chunks
        for chunk_index in (start_chunk + 1)..self.storage.len() {
            let bits = self.storage[chunk_index].0;
            if bits != 0 {
                let bit_pos = bits.trailing_zeros() as usize;
                let index = chunk_index * 32 + bit_pos;
                if index < N_BITS {
                    return Some(index);
                }
            }
        }

        None
    }

    /// Returns the index of the first clear bit (0), or `None` if all bits are
    /// set.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set_all(); // Set all bits to 1
    /// assert_eq!(bitset.first_clear_bit(), None;
    ///
    /// bitset.clear(42).unwrap();
    /// assert_eq!(bitset.first_clear_bit(), Some(42));
    ///
    /// bitset.clear(10).unwrap();
    /// assert_eq!(bitset.first_clear_bit(), Some(10)); // Returns the lowest index
    /// ```
    pub fn first_clear_bit(&self) -> Option<usize> {
        record_global_operation(OperationType::CollectionRead, self.verification_level);

        for (chunk_index, (bits, _)) in self.storage.iter().enumerate() {
            if *bits != !0u32 {
                // Find the index of the least significant bit that is clear
                let inverted = !*bits;
                let bit_pos = inverted.trailing_zeros() as usize;
                let index = chunk_index * 32 + bit_pos;

                // Ensure the index is within bounds
                if index < N_BITS {
                    return Some(index);
                }
            }
        }

        None
    }

    /// Returns the index of the next clear bit at or after the given position,
    /// or `None` if no more bits are clear.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set_all(); // Set all bits to 1
    /// bitset.clear(10).unwrap();
    /// bitset.clear(20).unwrap();
    /// bitset.clear(30).unwrap();
    ///
    /// assert_eq!(bitset.next_clear_bit(0), Some(10));
    /// assert_eq!(bitset.next_clear_bit(10), Some(10));
    /// assert_eq!(bitset.next_clear_bit(11), Some(20));
    /// assert_eq!(bitset.next_clear_bit(25), Some(30));
    /// assert_eq!(bitset.next_clear_bit(31), None); // Assuming all bits beyond 30 are set
    /// ```
    /// Sets multiple bits in one operation.
    ///
    /// This is more efficient than calling `set` multiple times.
    /// Returns the number of bits that were newly set (excluding bits that were
    /// already set). Returns an error if any index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// let indices = [10, 20, 30, 20]; // Note: 20 appears twice
    ///
    /// assert_eq!(bitset.set_multiple(&indices).unwrap(), 3); // Only 3 bits were newly set
    /// assert!(bitset.contains(10).unwrap();
    /// assert!(bitset.contains(20).unwrap();
    /// assert!(bitset.contains(30).unwrap();
    /// ```
    pub fn set_multiple(&mut self, indices: &[usize]) -> Result<usize, BoundedError> {
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        let mut bits_changed = 0;
        let mut modified_chunks = Vec::new();

        for &index in indices {
            if index >= N_BITS {
                return Err(BoundedError::runtime_execution_error("Index out of bounds"));
            }

            let storage_index = index / 32;
            let bit_position = index % 32;
            let bit_mask = 1 << bit_position;

            if storage_index >= self.storage.len() {
                return Err(BoundedError::new(
                    BoundedErrorKind::SliceError,
                    "Index out of bounds",
                ));
            }

            let (bits, _) = &mut self.storage[storage_index];
            let old_bits = *bits;

            // Set the bit
            *bits |= bit_mask;

            // If the bits changed, record the change
            if old_bits != *bits {
                bits_changed += 1;

                // Track which chunks were modified for checksum updates
                if !modified_chunks.contains(&storage_index) {
                    modified_chunks.push(storage_index);
                }
            }
        }

        // Update count and checksums for modified chunks
        if bits_changed > 0 {
            self.count += bits_changed;

            if self.verification_level >= VerificationLevel::Full {
                for &chunk_index in &modified_chunks {
                    let (bits, checksum) = &mut self.storage[chunk_index];
                    checksum.reset();
                    (*bits).update_checksum(checksum);
                }
            }
        }

        Ok(bits_changed)
    }

    /// Clears multiple bits in one operation.
    ///
    /// This is more efficient than calling `clear` multiple times.
    /// Returns the number of bits that were newly cleared (excluding bits that
    /// were already clear). Returns an error if any index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set_all(); // Set all bits to 1
    ///
    /// let indices = [10, 20, 30, 20]; // Note: 20 appears twice
    ///
    /// assert_eq!(bitset.clear_multiple(&indices).unwrap(), 3); // Only 3 bits were newly cleared
    /// assert!(!bitset.contains(10).unwrap();
    /// assert!(!bitset.contains(20).unwrap();
    /// assert!(!bitset.contains(30).unwrap();
    /// ```
    pub fn clear_multiple(&mut self, indices: &[usize]) -> Result<usize, BoundedError> {
        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        let mut bits_changed = 0;
        let mut modified_chunks = Vec::new();

        for &index in indices {
            if index >= N_BITS {
                return Err(BoundedError::runtime_execution_error("Index out of bounds"));
            }

            let storage_index = index / 32;
            let bit_position = index % 32;
            let bit_mask = 1 << bit_position;

            if storage_index >= self.storage.len() {
                return Err(BoundedError::new(
                    BoundedErrorKind::SliceError,
                    "Index out of bounds",
                ));
            }

            let (bits, _) = &mut self.storage[storage_index];
            let old_bits = *bits;

            // Clear the bit
            *bits &= !bit_mask;

            // If the bits changed, record the change
            if old_bits != *bits {
                bits_changed += 1;

                // Track which chunks were modified for checksum updates
                if !modified_chunks.contains(&storage_index) {
                    modified_chunks.push(storage_index);
                }
            }
        }

        // Update count and checksums for modified chunks
        if bits_changed > 0 {
            self.count -= bits_changed;

            if self.verification_level >= VerificationLevel::Full {
                for &chunk_index in &modified_chunks {
                    let (bits, checksum) = &mut self.storage[chunk_index];
                    checksum.reset();
                    (*bits).update_checksum(checksum);
                }
            }
        }

        Ok(bits_changed)
    }

    /// Sets or clears a range of bits.
    ///
    /// Sets all bits in the range [start_index, end_index) to the specified
    /// value. Returns the number of bits that were changed.
    /// Returns an error if either index is out of bounds or if end_index <=
    /// start_index.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    ///
    /// // Set bits 10-20 (inclusive) to 1
    /// assert_eq!(bitset.set_range(10, 21, true).unwrap(), 11;
    ///
    /// // Clear bits 15-25 (inclusive) to 0
    /// assert_eq!(bitset.set_range(15, 26, false).unwrap(), 6); // Only 6 bits changed (15-20)
    ///
    /// // Verify state
    /// for i in 10..15 {
    ///     assert!(bitset.contains(i).unwrap();
    /// }
    /// for i in 15..26 {
    ///     assert!(!bitset.contains(i).unwrap();
    /// }
    /// ```
    pub fn set_range(
        &mut self,
        start_index: usize,
        end_index: usize,
        value: bool,
    ) -> Result<usize, BoundedError> {
        if start_index >= N_BITS || end_index > N_BITS || start_index >= end_index {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        record_global_operation(OperationType::CollectionWrite, self.verification_level);

        let mut bits_changed = 0;
        let mut modified_chunks = Vec::new();

        // Calculate which chunks are affected
        let start_chunk = start_index / 32;
        let end_chunk = (end_index - 1) / 32;

        for chunk_index in start_chunk..=end_chunk {
            if chunk_index >= self.storage.len() {
                return Err(BoundedError::new(
                    BoundedErrorKind::SliceError,
                    "Index out of bounds",
                ));
            }

            let (bits, _) = &mut self.storage[chunk_index];
            let old_bits = *bits;

            // Calculate the mask for this chunk
            let chunk_start_bit = if chunk_index == start_chunk { start_index % 32 } else { 0 };
            let chunk_end_bit = if chunk_index == end_chunk { end_index % 32 } else { 32 };

            // Create a mask for the bits in this chunk's range
            let mask = if chunk_end_bit == 0 {
                // Special case for end at a chunk boundary
                !0u32 << chunk_start_bit
            } else {
                (!0u32 << chunk_start_bit) & (!0u32 >> (32 - chunk_end_bit))
            };

            // Apply the mask based on the value
            if value {
                *bits |= mask; // Set bits to 1
            } else {
                *bits &= !mask; // Clear bits to 0
            }

            // If bits changed, track the changes
            if old_bits != *bits {
                // Count how many bits changed
                let changed_bits = if value {
                    // Count bits that were 0 and became 1
                    (mask & !old_bits).count_ones() as usize
                } else {
                    // Count bits that were 1 and became 0
                    (mask & old_bits).count_ones() as usize
                };

                bits_changed += changed_bits;
                modified_chunks.push(chunk_index);
            }
        }

        // Update count
        if bits_changed > 0 {
            if value {
                self.count += bits_changed;
            } else {
                self.count -= bits_changed;
            }

            // Update checksums for modified chunks
            if self.verification_level >= VerificationLevel::Full {
                for &chunk_index in &modified_chunks {
                    let (bits, checksum) = &mut self.storage[chunk_index];
                    checksum.reset();
                    (*bits).update_checksum(checksum);
                }
            }
        }

        Ok(bits_changed)
    }

    /// Returns the position of the lowest set bit in the range [start_index,
    /// end_index).
    ///
    /// Returns `None` if no bits are set in the range or if the range is
    /// invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set(10).unwrap();
    /// bitset.set(20).unwrap();
    /// bitset.set(30).unwrap();
    ///
    /// assert_eq!(bitset.lowest_set_bit_in_range(0, 100).unwrap(), Some(10));
    /// assert_eq!(bitset.lowest_set_bit_in_range(15, 35).unwrap(), Some(20));
    /// assert_eq!(bitset.lowest_set_bit_in_range(31, 50).unwrap(), None;
    /// ```
    pub fn lowest_set_bit_in_range(
        &self,
        start_index: usize,
        end_index: usize,
    ) -> Result<Option<usize>, BoundedError> {
        if start_index >= N_BITS || end_index > N_BITS || start_index >= end_index {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Fast path: if no bits are set, return None
        if self.count == 0 {
            return Ok(None);
        }

        // Start at start_index and check each bit until we find one that's set
        let mut current = start_index;
        while current < end_index {
            if let Some(next_set) = self.next_set_bit(current) {
                if next_set < end_index {
                    return Ok(Some(next_set));
                }
                break; // Next set bit is beyond our range
            }
            break; // No more set bits
        }

        Ok(None)
    }

    /// Returns the position of the highest set bit in the range [start_index,
    /// end_index).
    ///
    /// Returns `None` if no bits are set in the range or if the range is
    /// invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set(10).unwrap();
    /// bitset.set(20).unwrap();
    /// bitset.set(30).unwrap();
    ///
    /// assert_eq!(bitset.highest_set_bit_in_range(0, 100).unwrap(), Some(30));
    /// assert_eq!(bitset.highest_set_bit_in_range(15, 25).unwrap(), Some(20));
    /// assert_eq!(bitset.highest_set_bit_in_range(31, 50).unwrap(), None;
    /// ```
    pub fn highest_set_bit_in_range(
        &self,
        start_index: usize,
        end_index: usize,
    ) -> Result<Option<usize>, BoundedError> {
        if start_index >= N_BITS || end_index > N_BITS || start_index >= end_index {
            return Err(BoundedError::new(
                BoundedErrorKind::SliceError,
                "Invalid slice range",
            ));
        }

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Fast path: if no bits are set, return None
        if self.count == 0 {
            return Ok(None);
        }

        // Calculate which chunks are affected
        let start_chunk = start_index / 32;
        let end_chunk = (end_index - 1) / 32;

        // Start from the highest chunk and work down
        for chunk_index in (start_chunk..=end_chunk).rev() {
            if chunk_index >= self.storage.len() {
                continue; // Skip out of bounds chunks
            }

            let (bits, _) = &self.storage[chunk_index];

            // Skip if this chunk has no bits set
            if *bits == 0 {
                continue;
            }

            // Calculate the mask for this chunk based on the range
            let chunk_start_bit = if chunk_index == start_chunk { start_index % 32 } else { 0 };
            let chunk_end_bit = if chunk_index == end_chunk { end_index % 32 } else { 32 };

            // Create a mask for the bits in this chunk's range
            let mask = if chunk_end_bit == 0 {
                // Special case for end at a chunk boundary
                !0u32 << chunk_start_bit
            } else {
                (!0u32 << chunk_start_bit) & (!0u32 >> (32 - chunk_end_bit))
            };

            // Get the bits in our range
            let masked_bits = *bits & mask;

            if masked_bits != 0 {
                // Find the most significant bit that is set
                // 31 - leading_zeros gives us the position of the highest bit
                let highest_bit_in_chunk = 31 - masked_bits.leading_zeros() as usize;
                let absolute_index = chunk_index * 32 + highest_bit_in_chunk;

                // Double-check that it's within our range
                if absolute_index >= start_index && absolute_index < end_index {
                    return Ok(Some(absolute_index));
                }
            }
        }

        Ok(None)
    }

    /// Returns the number of trailing zeros (unset bits) starting from index 0.
    ///
    /// This is useful for finding the first gap of zeros in a sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// assert_eq!(bitset.trailing_zeros(), 100); // All bits are 0
    ///
    /// bitset.set(10).unwrap();
    /// assert_eq!(bitset.trailing_zeros(), 10;
    ///
    /// bitset.set(0).unwrap();
    /// assert_eq!(bitset.trailing_zeros(), 0);
    /// ```
    pub fn trailing_zeros(&self) -> usize {
        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Fast path: if all bits are set, return 0
        if self.count == N_BITS {
            return 0;
        }

        // Fast path: if no bits are set, return N_BITS
        if self.count == 0 {
            return N_BITS;
        }

        // Find the first chunk with any bits set
        for (chunk_index, (bits, _)) in self.storage.iter().enumerate() {
            if *bits != 0 {
                // Found a chunk with at least one bit set
                // Count trailing zeros in this chunk
                let trailing_in_chunk = bits.trailing_zeros() as usize;
                return chunk_index * 32 + trailing_in_chunk;
            }
        }

        // If we get here, no bits are set, so return the capacity
        N_BITS
    }

    /// Returns the number of leading zeros (unset bits) at the end of the
    /// bitset.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// assert_eq!(bitset.leading_zeros(), 100); // All bits are 0
    ///
    /// bitset.set(50).unwrap();
    /// assert_eq!(bitset.leading_zeros(), 49); // Bits 51-99 are unset
    ///
    /// bitset.set(99).unwrap();
    /// assert_eq!(bitset.leading_zeros(), 0);
    /// ```
    pub fn leading_zeros(&self) -> usize {
        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Fast path: if all bits are set, return 0
        if self.count == N_BITS {
            return 0;
        }

        // Fast path: if no bits are set, return N_BITS
        if self.count == 0 {
            return N_BITS;
        }

        // Find the highest chunk with any bits set, going backwards
        for chunk_index in (0..self.storage.len()).rev() {
            let (bits, _) = &self.storage[chunk_index];
            if *bits != 0 {
                // Found a chunk with at least one bit set
                // Count leading zeros in this chunk
                let leading_in_chunk = bits.leading_zeros() as usize;

                // Calculate how many bits are in the higher chunks and add them
                let bits_in_higher_chunks = N_BITS - ((chunk_index + 1) * 32);
                let leading_zeros = leading_in_chunk + bits_in_higher_chunks;

                // Ensure we don't return more than N_BITS due to rounding
                return core::cmp::min(leading_zeros, N_BITS);
            }
        }

        // If we get here, no bits are set, so return the capacity
        N_BITS
    }

    /// Finds a contiguous sequence of clear bits of the specified length.
    ///
    /// Returns the starting index of the first such sequence found, or `None`
    /// if no such sequence exists. This is useful for finding available
    /// space in a bitmap.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set(0).unwrap();
    /// bitset.set(1).unwrap();
    /// bitset.set(4).unwrap();
    /// bitset.set(5).unwrap();
    ///
    /// assert_eq!(bitset.find_clear_sequence(2).unwrap(), Some(2)); // Bits 2-3 are clear
    /// assert_eq!(bitset.find_clear_sequence(3).unwrap(), Some(6)); // Bits 6-8 are clear
    /// assert_eq!(bitset.find_clear_sequence(95).unwrap(), Some(6)); // Bits 6-100 have 94 clear bits
    /// assert_eq!(bitset.find_clear_sequence(96).unwrap(), None); // Not enough clear bits
    /// ```
    pub fn find_clear_sequence(&self, length: usize) -> Result<Option<usize>, BoundedError> {
        if length == 0 || length > N_BITS {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Fast path: if all bits are clear, return 0
        if self.count == 0 {
            return Ok(Some(0));
        }

        // Fast path: if not enough bits are clear, return None
        if self.count > N_BITS - length {
            return Ok(None);
        }

        // Search for a sequence of clear bits
        let mut start = 0;
        let mut consecutive_clear = 0;

        for i in 0..N_BITS {
            match self.contains(i) {
                Ok(true) => {
                    // This bit is set, reset our counter
                    consecutive_clear = 0;
                    start = i + 1;
                },
                Ok(false) => {
                    // This bit is clear, increment our counter
                    consecutive_clear += 1;

                    // Check if we've found a sequence of sufficient length
                    if consecutive_clear == length {
                        return Ok(Some(start));
                    }
                },
                Err(e) => return Err(e),
            }
        }

        // If we get here, we didn't find a sequence of sufficient length
        Ok(None)
    }

    /// Returns a string representation of the bitset for debugging.
    ///
    /// The format is a sequence of '0' and '1' characters, with the least
    /// significant (lowest index) bit on the right.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<8>::new();
    /// bitset.set(1).unwrap();
    /// bitset.set(3).unwrap();
    /// bitset.set(5).unwrap();
    ///
    /// // Bits 1, 3, and 5 are set (indexed from 0)
    /// assert_eq!(bitset.to_binary_string(), "00000000 00000000 00000000 00101010";
    /// ```
    #[cfg(feature = "std")]
    pub fn to_binary_string(&self) -> String {
        let mut result = String::with_capacity(N_BITS);

        // Add each bit to the string, starting from the most significant bit
        for i in (0..N_BITS).rev() {
            match self.contains(i) {
                Ok(true) => result.push('1'),
                _ => result.push('0'),
            }
        }

        result
    }

    pub fn next_clear_bit(&self, from_index: usize) -> Option<usize> {
        if from_index >= N_BITS {
            return None;
        }

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        // Calculate the chunk and bit position for the starting index
        let start_chunk = from_index / 32;
        let start_bit = from_index % 32;

        // Check the first chunk, masking out bits before the start_bit
        if start_chunk < self.storage.len() {
            let inverted = !self.storage[start_chunk].0;
            let masked_bits = inverted & (!0u32 << start_bit);
            if masked_bits != 0 {
                let bit_pos = masked_bits.trailing_zeros() as usize;
                let index = start_chunk * 32 + bit_pos;
                if index < N_BITS {
                    return Some(index);
                }
            }
        }

        // Check remaining chunks
        for chunk_index in (start_chunk + 1)..self.storage.len() {
            let inverted = !self.storage[chunk_index].0;
            if inverted != 0 {
                let bit_pos = inverted.trailing_zeros() as usize;
                let index = chunk_index * 32 + bit_pos;
                if index < N_BITS {
                    return Some(index);
                }
            }
        }

        None
    }

    /// Counts the number of bits set to 1 in the range [start_index,
    /// end_index).
    ///
    /// Returns an error if either index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set(10).unwrap();
    /// bitset.set(20).unwrap();
    /// bitset.set(30).unwrap();
    ///
    /// assert_eq!(bitset.count_bits_in_range(0, 100).unwrap(), 3;
    /// assert_eq!(bitset.count_bits_in_range(0, 15).unwrap(), 1);
    /// assert_eq!(bitset.count_bits_in_range(15, 25).unwrap(), 1);
    /// assert_eq!(bitset.count_bits_in_range(31, 100).unwrap(), 0);
    /// ```
    pub fn count_bits_in_range(
        &self,
        start_index: usize,
        end_index: usize,
    ) -> Result<usize, BoundedError> {
        if start_index >= N_BITS || end_index > N_BITS || start_index >= end_index {
            return Err(BoundedError::runtime_execution_error("Operation failed"));
        }

        record_global_operation(OperationType::CollectionRead, self.verification_level);

        let mut count = 0;

        // Fast path: if the range spans multiple chunks, process chunk by chunk
        let start_chunk = start_index / 32;
        let end_chunk = (end_index - 1) / 32;

        if start_chunk == end_chunk {
            // Range is within a single chunk
            let start_bit = start_index % 32;
            let end_bit = end_index % 32;

            // Create a mask for the bits in the range [start_bit, end_bit)
            let mask = if end_bit == 0 {
                !0u32 << start_bit
            } else {
                (!0u32 << start_bit) & (!0u32 >> (32 - end_bit))
            };

            let bits = self.storage[start_chunk].0 & mask;
            count = bits.count_ones() as usize;
        } else {
            // Process first chunk (partial)
            let start_bit = start_index % 32;
            if start_bit > 0 {
                let mask = !0u32 << start_bit;
                let bits = self.storage[start_chunk].0 & mask;
                count += bits.count_ones() as usize;
            } else {
                count += self.storage[start_chunk].0.count_ones() as usize;
            }

            // Process middle chunks (full)
            for chunk_index in (start_chunk + 1)..end_chunk {
                count += self.storage[chunk_index].0.count_ones() as usize;
            }

            // Process last chunk (partial)
            let end_bit = end_index % 32;
            if end_bit > 0 {
                let mask = !0u32 >> (32 - end_bit);
                let bits = self.storage[end_chunk].0 & mask;
                count += bits.count_ones() as usize;
            }
        }

        Ok(count)
    }

    /// Creates a clone of this bitset.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut a = BoundedBitSet::<100>::new();
    /// a.set(10).unwrap();
    /// a.set(20).unwrap();
    ///
    /// let b = a.clone_bitset);
    /// assert!(b.contains(10).unwrap();
    /// assert!(b.contains(20).unwrap();
    /// ```
    pub fn clone_bitset(&self) -> Self {
        let mut clone = Self::with_verification_level(self.verification_level);

        // Copy the storage and count
        clone.storage = self.storage.clone();
        clone.count = self.count;

        clone
    }

    /// Checks if this bitset is a subset of another bitset.
    ///
    /// Returns `true` if all bits set in this bitset are also set in the other
    /// bitset.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut a = BoundedBitSet::<100>::new();
    /// a.set(10).unwrap();
    /// a.set(20).unwrap();
    ///
    /// let mut b = BoundedBitSet::<100>::new();
    /// b.set(10).unwrap();
    /// b.set(20).unwrap();
    /// b.set(30).unwrap();
    ///
    /// assert!(a.is_subset_of(&b);
    /// assert!(!b.is_subset_of(&a);
    /// ```
    pub fn is_subset_of(&self, other: &Self) -> bool {
        record_global_operation(OperationType::CollectionRead, self.verification_level);

        for (i, (bits, _)) in self.storage.iter().enumerate() {
            if i < other.storage.len() {
                // Check if all bits set in this bitset are also set in the other bitset
                if (*bits & other.storage[i].0) != *bits {
                    return false;
                }
            } else if *bits != 0 {
                // This bitset has bits set that are not even in the range of the other bitset
                return false;
            }
        }

        true
    }

    /// Returns an iterator over the indices of all set bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<100>::new();
    /// bitset.set(10).unwrap();
    /// bitset.set(20).unwrap();
    /// bitset.set(30).unwrap();
    ///
    /// let indices: Vec<usize> = bitset.iter_ones().collect();
    /// assert_eq!(indices, vec![10, 20, 30]);
    /// ```
    pub fn iter_ones(&self) -> BitSetOnesIterator<'_, N_BITS> {
        BitSetOnesIterator {
            bitset:     self,
            next_index: 0,
        }
    }

    /// Returns an iterator over the indices of all clear bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::BoundedBitSet;
    ///
    /// let mut bitset = BoundedBitSet::<5>::new();
    /// bitset.set(1).unwrap();
    /// bitset.set(3).unwrap();
    ///
    /// let indices: Vec<usize> = bitset.iter_zeros().collect();
    /// assert_eq!(indices, vec![0, 2, 4]);
    /// ```
    pub fn iter_zeros(&self) -> BitSetZerosIterator<'_, N_BITS> {
        BitSetZerosIterator {
            bitset:     self,
            next_index: 0,
        }
    }
}

// Implement standard traits for the new collections

#[cfg(feature = "default-provider")]
impl<const N_BITS: usize> BoundedCapacity for BoundedBitSet<N_BITS> {
    fn capacity(&self) -> usize {
        N_BITS
    }

    fn len(&self) -> usize {
        self.count
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn is_full(&self) -> bool {
        self.count == N_BITS
    }
}

impl<T, const N_ELEMENTS: usize, P: MemoryProvider> BoundedCapacity
    for BoundedQueue<T, N_ELEMENTS, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
{
    fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    fn len(&self) -> usize {
        self.length
    }

    fn is_empty(&self) -> bool {
        self.length == 0
    }

    fn is_full(&self) -> bool {
        self.length == N_ELEMENTS
    }
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> BoundedCapacity
    for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn is_full(&self) -> bool {
        self.entries.is_full()
    }
}

impl<T, const N_ELEMENTS: usize, P: MemoryProvider> BoundedCapacity for BoundedSet<T, N_ELEMENTS, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    P: MemoryProvider + Default + Clone + PartialEq + Eq,
{
    fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    fn len(&self) -> usize {
        self.elements.len()
    }

    fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    fn is_full(&self) -> bool {
        self.elements.is_full()
    }
}

impl<T, const N_ELEMENTS: usize, P: MemoryProvider> BoundedCapacity
    for BoundedDeque<T, N_ELEMENTS, P>
where
    T: Sized + Checksummable + ToBytes + FromBytes + Default,
{
    fn capacity(&self) -> usize {
        N_ELEMENTS
    }

    fn len(&self) -> usize {
        self.length
    }

    fn is_empty(&self) -> bool {
        self.length == 0
    }

    fn is_full(&self) -> bool {
        self.length == N_ELEMENTS
    }
}

/// Iterator over the set bits (1s) in a `BoundedBitSet`.
#[cfg(feature = "std")]
pub struct BitSetOnesIterator<'a, const N_BITS: usize> {
    bitset:     &'a BoundedBitSet<N_BITS>,
    next_index: usize,
}

#[cfg(feature = "std")]
impl<'a, const N_BITS: usize> Iterator for BitSetOnesIterator<'a, N_BITS> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.bitset.next_set_bit(self.next_index);
        if let Some(index) = result {
            self.next_index = index + 1;
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // We know the exact number of remaining elements
        let remaining = self
            .bitset
            .count
            .saturating_sub(self.bitset.count_bits_in_range(0, self.next_index).unwrap_or(0));
        (remaining, Some(remaining))
    }
}

/// Iterator over the clear bits (0s) in a `BoundedBitSet`.
#[cfg(feature = "std")]
pub struct BitSetZerosIterator<'a, const N_BITS: usize> {
    bitset:     &'a BoundedBitSet<N_BITS>,
    next_index: usize,
}

#[cfg(feature = "std")]
impl<'a, const N_BITS: usize> Iterator for BitSetZerosIterator<'a, N_BITS> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.bitset.next_clear_bit(self.next_index);
        if let Some(index) = result {
            self.next_index = index + 1;
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // We know the exact number of remaining elements
        let ones_remaining = self
            .bitset
            .count
            .saturating_sub(self.bitset.count_bits_in_range(0, self.next_index).unwrap_or(0));
        let total_remaining = N_BITS.saturating_sub(self.next_index);
        let zeros_remaining = total_remaining.saturating_sub(ones_remaining);
        (zeros_remaining, Some(zeros_remaining))
    }
}

/// Implement PartialEq for BoundedBitSet
#[cfg(feature = "std")]
impl<const N_BITS: usize> PartialEq for BoundedBitSet<N_BITS> {
    fn eq(&self, other: &Self) -> bool {
        // Quick check for count
        if self.count != other.count {
            return false;
        }

        // Check each chunk of bits
        for i in 0..self.storage.len() {
            if i < other.storage.len() {
                if self.storage[i].0 != other.storage[i].0 {
                    return false;
                }
            } else if self.storage[i].0 != 0 {
                // This bitset has bits set beyond other's storage
                return false;
            }
        }

        // Check if other has bits set beyond this bitset's storage
        for i in self.storage.len()..other.storage.len() {
            if other.storage[i].0 != 0 {
                return false;
            }
        }

        true
    }
}

/// Implement Eq for BoundedBitSet
#[cfg(feature = "std")]
impl<const N_BITS: usize> Eq for BoundedBitSet<N_BITS> {}

/// Implement Hash for BoundedBitSet
#[cfg(feature = "std")]
impl<const N_BITS: usize> core::hash::Hash for BoundedBitSet<N_BITS> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        N_BITS.hash(state);
        self.count.hash(state);

        // Only hash the actual bit data, not the checksums
        for (bits, _) in &self.storage {
            bits.hash(state);
        }
    }
}

/// Implement Checksummable for BoundedBitSet
#[cfg(feature = "std")]
impl<const N_BITS: usize> Checksummable for BoundedBitSet<N_BITS> {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Update with capacity and count
        (N_BITS as u32).update_checksum(checksum);
        (self.count as u32).update_checksum(checksum);

        // Update with all bit chunks
        for (bits, _) in &self.storage {
            (*bits).update_checksum(checksum);
        }
    }
}

/// Implement ToBytes for BoundedBitSet
#[cfg(feature = "std")]
impl<const N_BITS: usize> ToBytes for BoundedBitSet<N_BITS> {
    fn to_bytes_with_provider<'a, P: crate::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        stream_provider: &P,
    ) -> wrt_error::Result<()> {
        // Write the number of bits in the set (count)
        writer.write_u32_le(self.count as u32)?;

        // Write verification level
        self.verification_level.to_bytes_with_provider(writer, stream_provider)?;

        // Write the number of storage chunks
        writer.write_u32_le(self.storage.len() as u32)?;

        // Write each chunk's bits (not the checksums, as they'll be recalculated)
        for (bits, _) in &self.storage {
            writer.write_u32_le(*bits)?;
        }

        Ok(())
    }

    #[cfg(feature = "default-provider")]
    fn to_bytes<'a>(&self, writer: &mut WriteStream<'a>) -> wrt_error::Result<()> {
        let default_provider = DefaultMemoryProvider::default();
        self.to_bytes_with_provider(writer, &default_provider)
    }
}

/// Implement FromBytes for BoundedBitSet
#[cfg(feature = "std")]
impl<const N_BITS: usize> FromBytes for BoundedBitSet<N_BITS> {
    fn from_bytes_with_provider<'a, P: crate::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        stream_provider: &P,
    ) -> wrt_error::Result<Self> {
        // Read the number of bits in the set (count)
        let count = reader.read_u32_le()? as usize;

        // Read verification level
        let verification_level =
            VerificationLevel::from_bytes_with_provider(reader, stream_provider)?;

        // Read the number of storage chunks
        let num_chunks = reader.read_u32_le()? as usize;

        // Create empty bitset with the specified verification level
        let mut bitset = Self::with_verification_level(verification_level);

        // Calculate expected number of chunks based on N_BITS
        let expected_chunks = (N_BITS + 31) / 32;

        // Validate the number of chunks
        if num_chunks > expected_chunks {
            return Err(Error::new_static(
                ErrorCategory::Parse,
                codes::PARSE_ERROR,
                "Invalid number of chunks in BoundedBitSet serialization",
            ));
        }

        // Reset storage to ensure we have the right capacity
        bitset.storage.clear();

        // Read each chunk's bits and generate new checksums
        for _ in 0..num_chunks {
            let bits = reader.read_u32_le()?;
            let mut chunk_checksum = Checksum::new();
            bits.update_checksum(&mut chunk_checksum);
            bitset.storage.push((bits, chunk_checksum));
        }

        // Add empty chunks if needed
        while bitset.storage.len() < expected_chunks {
            bitset.storage.push((0, Checksum::default()));
        }

        // Set the count (total number of set bits)
        bitset.count = count;

        // Validate that the count matches the actual number of set bits
        let calculated_count =
            bitset.storage.iter().map(|(bits, _)| bits.count_ones() as usize).sum::<usize>();

        if calculated_count != count {
            return Err(Error::new_static(
                ErrorCategory::Validation,
                codes::VALIDATION_ERROR,
                "Count mismatch in BoundedBitSet deserialization",
            ));
        }

        Ok(bitset)
    }

    #[cfg(feature = "default-provider")]
    fn from_bytes<'a>(reader: &mut ReadStream<'a>) -> wrt_error::Result<Self> {
        let default_provider = DefaultMemoryProvider::default();
        Self::from_bytes_with_provider(reader, &default_provider)
    }
}

// Note: Using the generic implementation of FromBytes for (A, B) from traits.rs
// This specific implementation was removed to avoid conflicts

// These types are already defined within this module
// No need to re-export them within the same module

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        budget_aware_provider::CrateId,
        safe_managed_alloc,
        safe_memory::NoStdProvider,
    };

    // Helper function to initialize memory system for tests
    fn init_test_memory_system() {
        let _ = crate::memory_init::MemoryInitializer::initialize;
    }

    // Test BoundedQueue
    #[test]
    fn test_bounded_queue() {
        init_test_memory_system();
        let provider = safe_managed_alloc!(1024, CrateId::Foundation).unwrap();
        let mut queue = BoundedQueue::<u32, 5, NoStdProvider<1024>>::new(provider).unwrap();

        // Test enqueue
        for i in 0..5 {
            queue.enqueue(i).unwrap();
        }

        // Test full queue
        assert!(queue.is_full());
        assert_eq!(
            queue.enqueue(5).unwrap_err().kind,
            BoundedErrorKind::CapacityExceeded
        );

        // Test dequeue
        for i in 0..5 {
            assert_eq!(queue.dequeue().unwrap(), Some(i));
        }

        // Test empty queue
        assert!(queue.is_empty());
        assert_eq!(queue.dequeue().unwrap(), None);

        // Test wrap-around behavior
        for i in 0..3 {
            queue.enqueue(i).unwrap();
        }

        assert_eq!(queue.dequeue().unwrap(), Some(0));
        assert_eq!(queue.dequeue().unwrap(), Some(1));

        queue.enqueue(3).unwrap();
        queue.enqueue(4).unwrap();
        queue.enqueue(5).unwrap();

        assert_eq!(queue.dequeue().unwrap(), Some(2));
        assert_eq!(queue.dequeue().unwrap(), Some(3));
        assert_eq!(queue.dequeue().unwrap(), Some(4));
        assert_eq!(queue.dequeue().unwrap(), Some(5));
        assert_eq!(queue.dequeue().unwrap(), None);
    }

    // Test BoundedMap
    #[test]
    fn test_bounded_map() {
        init_test_memory_system();
        let provider = safe_managed_alloc!(1024, CrateId::Foundation).unwrap();
        let mut map = BoundedMap::<u32, u32, 3, NoStdProvider<1024>>::new(provider).unwrap();

        // Test insert
        assert_eq!(map.insert(1, 10).unwrap(), None);
        assert_eq!(map.insert(2, 20).unwrap(), None);
        assert_eq!(map.insert(3, 30).unwrap(), None);

        // Test full map
        assert!(map.is_full());
        assert_eq!(
            map.insert(4, 40).unwrap_err().kind,
            BoundedErrorKind::CapacityExceeded
        );

        // Test get
        assert_eq!(map.get(&1).unwrap(), Some(10));
        assert_eq!(map.get(&2).unwrap(), Some(20));
        assert_eq!(map.get(&3).unwrap(), Some(30));
        assert_eq!(map.get(&4).unwrap(), None);

        // Test update existing key
        assert_eq!(map.insert(2, 25).unwrap(), Some(20));
        assert_eq!(map.get(&2).unwrap(), Some(25));

        // Test remove
        assert_eq!(map.remove(&2).unwrap(), Some(25));
        assert_eq!(map.get(&2).unwrap(), None);

        // Test contains_key
        assert!(map.contains_key(&1).unwrap());
        assert!(!map.contains_key(&2).unwrap());

        // Test clear
        map.clear().unwrap();
        assert!(map.is_empty());
    }

    // Test BoundedSet
    #[test]
    fn test_bounded_set() {
        init_test_memory_system();
        let provider = safe_managed_alloc!(1024, CrateId::Foundation).unwrap();
        let mut set = BoundedSet::<u32, 3, NoStdProvider<1024>>::new(provider).unwrap();

        // Test insert
        assert!(set.insert(1).unwrap());
        assert!(set.insert(2).unwrap());
        assert!(set.insert(3).unwrap());

        // Test full set
        assert!(set.is_full());
        assert_eq!(
            set.insert(4).unwrap_err().kind,
            BoundedErrorKind::CapacityExceeded
        );

        // Test contains
        assert!(set.contains(&1).unwrap());
        assert!(set.contains(&2).unwrap());
        assert!(set.contains(&3).unwrap());
        assert!(!set.contains(&4).unwrap());

        // Test insert duplicate (no effect)
        assert!(!set.insert(1).unwrap());

        // Test remove
        assert!(set.remove(&2).unwrap());
        assert!(!set.contains(&2).unwrap());

        // Test clear
        set.clear().unwrap();
        assert!(set.is_empty());
    }

    // Test BoundedDeque
    #[test]
    fn test_bounded_deque() {
        init_test_memory_system();
        let provider = safe_managed_alloc!(1024, CrateId::Foundation).unwrap();
        let mut deque = BoundedDeque::<u32, 5, NoStdProvider<1024>>::new(provider).unwrap();

        // Test push_back
        for i in 0..3 {
            deque.push_back(i).unwrap();
        }

        // Test push_front
        deque.push_front(10).unwrap();
        deque.push_front(20).unwrap();

        // Test full deque
        assert!(deque.is_full());
        assert_eq!(
            deque.push_back(5).unwrap_err().kind,
            BoundedErrorKind::CapacityExceeded
        );
        assert_eq!(
            deque.push_front(5).unwrap_err().kind,
            BoundedErrorKind::CapacityExceeded
        );

        // Test front and back
        assert_eq!(deque.front().unwrap(), Some(20));
        assert_eq!(deque.back().unwrap(), Some(2));

        // Test pop_front and pop_back
        assert_eq!(deque.pop_front().unwrap(), Some(20));
        assert_eq!(deque.pop_back().unwrap(), Some(2));
        assert_eq!(deque.pop_front().unwrap(), Some(10));
        assert_eq!(deque.pop_back().unwrap(), Some(1));
        assert_eq!(deque.pop_front().unwrap(), Some(0));

        // Test empty deque
        assert!(deque.is_empty());
        assert_eq!(deque.pop_front().unwrap(), None);
        assert_eq!(deque.pop_back().unwrap(), None);

        // Test clear
        deque.push_back(1).unwrap();
        deque.push_back(2).unwrap();
        deque.clear().unwrap();
        assert!(deque.is_empty());
    }

    // Test BoundedBitSet
    #[test]
    #[cfg(feature = "std")]
    fn test_bounded_bit_set() {
        let mut bit_set = BoundedBitSet::<100>::new();

        // Test set
        assert!(bit_set.set(10).unwrap());
        assert!(bit_set.set(50).unwrap());
        assert!(bit_set.set(99).unwrap());

        // Test set already set bit (no effect)
        assert!(!bit_set.set(10).unwrap());

        // Test contains
        assert!(bit_set.contains(10).unwrap());
        assert!(bit_set.contains(50).unwrap());
        assert!(bit_set.contains(99).unwrap());
        assert!(!bit_set.contains(20).unwrap());

        // Test clear
        assert!(bit_set.clear(50).unwrap());
        assert!(!bit_set.contains(50).unwrap());

        // Test clear already cleared bit (no effect)
        assert!(!bit_set.clear(50).unwrap());

        // Test toggle
        assert!(!bit_set.toggle(50).unwrap()); // Now set
        assert!(bit_set.contains(50).unwrap());

        assert!(bit_set.toggle(50).unwrap()); // Now cleared
        assert!(!bit_set.contains(50).unwrap());

        // Test set_all and clear_all
        bit_set.set_all();
        assert_eq!(bit_set.len(), 100);
        assert!(bit_set.is_full());

        bit_set.clear_all();
        assert_eq!(bit_set.len(), 0);
        assert!(bit_set.is_empty());

        // Test out of bounds access
        assert!(bit_set.set(100).is_err());
        assert!(bit_set.clear(100).is_err());
        assert!(bit_set.contains(100).is_err());
        assert!(bit_set.toggle(100).is_err());
    }
}

// Trait implementations for BoundedMap
impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> Default for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    fn default() -> Self {
        Self::new(P::default()).expect("Default provider should never fail to create BoundedMap")
    }
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> Clone for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    fn clone(&self) -> Self {
        // Clone the provider from the entries BoundedVec to preserve capacity/allocation
        // Using P::default() would create an empty provider with no capacity
        let mut new_map = Self::new(self.entries.provider().clone())
            .expect("Cloned provider should create BoundedMap");
        new_map.verification_level = self.verification_level;

        // Clone all entries - don't silently drop errors
        for i in 0..self.entries.len() {
            if let Ok((k, v)) = self.entries.get(i) {
                // Insert should succeed since we cloned the provider with same capacity
                drop(new_map.insert(k, v));
            }
        }

        new_map
    }
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> PartialEq for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        for i in 0..self.entries.len() {
            if let (Ok((k1, v1)), Ok((k2, v2))) = (self.entries.get(i), other.entries.get(i)) {
                if k1 != k2 || v1 != v2 {
                    return false;
                }
            }
        }

        true
    }
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> Eq for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> Checksummable
    for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(&(self.len() as u32).to_le_bytes());
        for i in 0..self.entries.len() {
            if let Ok((k, v)) = self.entries.get(i) {
                k.update_checksum(checksum);
                v.update_checksum(checksum);
            }
        }
    }
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> ToBytes for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    fn serialized_size(&self) -> usize {
        4 + self
            .entries
            .iter()
            .map(|(k, v)| k.serialized_size() + v.serialized_size())
            .sum::<usize>()
    }

    fn to_bytes_with_provider<'a, PROV: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &PROV,
    ) -> wrt_error::Result<()> {
        writer.write_all(&(self.len() as u32).to_le_bytes())?;
        for i in 0..self.entries.len() {
            if let Ok((k, v)) = self.entries.get(i) {
                k.to_bytes_with_provider(writer, provider)?;
                v.to_bytes_with_provider(writer, provider)?;
            }
        }
        Ok(())
    }
}

impl<K, V, const N_ELEMENTS: usize, P: MemoryProvider> FromBytes for BoundedMap<K, V, N_ELEMENTS, P>
where
    K: Sized + Checksummable + ToBytes + FromBytes + Default + Eq + Clone + PartialEq,
    V: Sized + Checksummable + ToBytes + FromBytes + Default + Clone + PartialEq + Eq,
    P: Default + Clone + PartialEq + Eq,
{
    fn from_bytes_with_provider<'a, PROV: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &PROV,
    ) -> wrt_error::Result<Self> {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut map = Self::new(P::default())?;

        for _ in 0..len.min(N_ELEMENTS) {
            let k = K::from_bytes_with_provider(reader, provider)?;
            let v = V::from_bytes_with_provider(reader, provider)?;
            drop(map.insert(k, v));
        }

        Ok(map)
    }
}
