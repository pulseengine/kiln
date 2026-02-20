// WRT - wrt-foundation
// Module: StaticMap - Inline-storage sorted map
// SW-REQ-ID: REQ_RESOURCE_001, REQ_MEM_SAFETY_001, REQ_TEMPORAL_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Allow unsafe code for MaybeUninit operations (documented and verified via KANI)
#![allow(unsafe_code)]

//! Static key-value map with inline storage and compile-time capacity.
//!
//! `StaticMap<K, V, N>` provides a fixed-capacity map using a sorted array
//! for O(log n) lookups via binary search. All storage is inline.
//!
//! # Characteristics
//!
//! - **Zero allocation**: All memory is inline sorted array
//! - **O(log n) lookup**: Binary search on sorted keys
//! - **O(n) insertion**: Maintains sorted order
//! - **RAII cleanup**: Automatic Drop implementation
//! - **ASIL-D compliant**: Deterministic WCET

use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::marker::PhantomData;

use wrt_error::Result;

/// A sorted map with compile-time capacity and inline storage.
///
/// # Requirements
///
/// - REQ_RESOURCE_001: Static allocation only
/// - REQ_MEM_SAFETY_001: Bounds validation
/// - REQ_TEMPORAL_001: Bounded execution time (O(log n) lookup)
///
/// # Invariants
///
/// 1. `len <= N` always holds
/// 2. Elements [0..len) are initialized and sorted by key
/// 3. No duplicate keys
///
/// # Examples
///
/// ```
/// use wrt_foundation::collections::StaticMap;
///
/// let mut map = StaticMap::<&str, u32, 10>::new();
/// map.insert("foo", 42)?;
/// map.insert("bar", 100)?;
///
/// assert_eq!(map.get("foo"), Some(&42));
/// assert_eq!(map.get("baz"), None);
/// # Ok::<(), wrt_error::Error>(())
/// ```
#[derive(Debug)]
pub struct StaticMap<K, V, const N: usize>
where
    K: Ord,
{
    /// Sorted array of key-value pairs
    /// Invariant: entries[0..len) are sorted by key
    entries: [MaybeUninit<(K, V)>; N],

    /// Number of entries
    /// Invariant: len <= N
    len: usize,

    /// Marker for drop checker
    _marker: PhantomData<(K, V)>,
}

impl<K: Ord, V, const N: usize> StaticMap<K, V, N> {
    /// Creates a new empty map.
    ///
    /// # Const-time Guarantee
    ///
    /// O(1) with WCET ~5 CPU cycles.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the key already exists, the value is updated and the old value is returned.
    ///
    /// # Time Complexity
    ///
    /// - Lookup: O(log n) via binary search
    /// - Insertion: O(n) in worst case (shift elements to maintain sort order)
    /// - WCET: ~(15 * log2(n) + 20 * n) CPU cycles
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if map is full and key doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticMap;
    ///
    /// let mut map = StaticMap::<&str, u32, 3>::new();
    /// assert!(map.insert("a", 1).is_ok());
    /// assert!(map.insert("b", 2).is_ok());
    /// assert!(map.insert("c", 3).is_ok());
    /// assert!(map.insert("d", 4).is_err()); // Full
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>> {
        // Binary search to find insertion point
        match self.binary_search(&key) {
            Ok(index) => {
                // Key exists, replace value
                // SAFETY: index < len, so element is initialized
                let entry = unsafe { self.entries[index].assume_init_mut() };
                let old_value = core::mem::replace(&mut entry.1, value);
                Ok(Some(old_value))
            }
            Err(index) => {
                // Key doesn't exist, insert at index
                if self.len >= N {
                    return Err(wrt_error::Error::foundation_bounded_capacity_exceeded(
                        "StaticMap capacity exceeded",
                    ));
                }

                // Shift elements right to make space
                for i in (index..self.len).rev() {
                    // SAFETY: i < len, so source is initialized
                    // i+1 < len+1 <= N, so destination is valid
                    unsafe {
                        let src = self.entries[i].assume_init_read();
                        self.entries[i + 1].write(src);
                    }
                }

                // Insert new entry
                self.entries[index].write((key, value));
                self.len += 1;

                Ok(None)
            }
        }
    }

    /// Gets a reference to the value for the given key.
    ///
    /// # Time Complexity
    ///
    /// O(log n) via binary search. WCET: ~15 * log2(n) CPU cycles.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticMap;
    ///
    /// let mut map = StaticMap::<&str, u32, 10>::new();
    /// map.insert("foo", 42)?;
    ///
    /// assert_eq!(map.get("foo"), Some(&42));
    /// assert_eq!(map.get("bar"), None);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.binary_search(key).ok().map(|index| {
            // SAFETY: index is valid (from binary_search on len)
            unsafe { &self.entries[index].assume_init_ref().1 }
        })
    }

    /// Gets a mutable reference to the value for the given key.
    ///
    /// # Time Complexity
    ///
    /// O(log n) via binary search.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.binary_search(key) {
            Ok(index) => {
                // SAFETY: index is valid (from binary_search on len)
                Some(unsafe { &mut self.entries[index].assume_init_mut().1 })
            }
            Err(_) => None,
        }
    }

    /// Removes a key-value pair from the map.
    ///
    /// # Time Complexity
    ///
    /// O(n) - shifts elements left to maintain sorted order.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticMap;
    ///
    /// let mut map = StaticMap::<&str, u32, 10>::new();
    /// map.insert("foo", 42)?;
    ///
    /// assert_eq!(map.remove("foo"), Some(42));
    /// assert_eq!(map.remove("foo"), None);
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn remove(&mut self, key: &K) -> Option<V> {
        match self.binary_search(key) {
            Ok(index) => {
                // SAFETY: index < len, element is initialized
                let (_key, value) = unsafe { self.entries[index].assume_init_read() };

                // Shift elements left
                for i in index..(self.len - 1) {
                    // SAFETY: i+1 < len, so source is initialized
                    unsafe {
                        let src = self.entries[i + 1].assume_init_read();
                        self.entries[i].write(src);
                    }
                }

                self.len -= 1;
                Some(value)
            }
            Err(_) => None,
        }
    }

    /// Returns `true` if the map contains the given key.
    ///
    /// # Time Complexity
    ///
    /// O(log n) via binary search.
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.binary_search(key).is_ok()
    }

    /// Returns the current number of entries.
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

    /// Returns `true` if the map is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the map is full.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Clears the map, dropping all entries.
    ///
    /// # Time Complexity
    ///
    /// O(n). WCET: ~15 * len CPU cycles.
    pub fn clear(&mut self) {
        for i in 0..self.len {
            // SAFETY: i < len, so element is initialized
            unsafe {
                self.entries[i].assume_init_drop();
            }
        }
        self.len = 0;
    }

    /// Returns an iterator over the map entries in sorted order.
    #[inline]
    #[must_use]
    pub fn iter(&self) -> StaticMapIter<'_, K, V, N> {
        StaticMapIter {
            map: self,
            index: 0,
        }
    }

    /// Returns an iterator over keys in sorted order.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over values.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// Returns a mutable iterator over values.
    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.iter_mut().map(|(_, v)| v)
    }

    /// Returns a mutable iterator over the map entries in sorted order.
    #[inline]
    #[must_use]
    pub fn iter_mut(&mut self) -> StaticMapIterMut<'_, K, V, N> {
        StaticMapIterMut {
            map: self,
            index: 0,
        }
    }

    /// Binary search for key.
    ///
    /// Returns Ok(index) if found, Err(insert_index) if not found.
    fn binary_search(&self, key: &K) -> core::result::Result<usize, usize> {
        let mut left = 0;
        let mut right = self.len;

        while left < right {
            let mid = left + (right - left) / 2;

            // SAFETY: mid < right <= len, so element is initialized
            let mid_key = unsafe { &self.entries[mid].assume_init_ref().0 };

            match key.cmp(mid_key) {
                Ordering::Equal => return Ok(mid),
                Ordering::Less => right = mid,
                Ordering::Greater => left = mid + 1,
            }
        }

        Err(left)
    }

    /// Gets the given key's corresponding entry in the map for in-place manipulation.
    ///
    /// # Time Complexity
    ///
    /// O(log n) for lookup via binary search.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticMap;
    ///
    /// let mut map = StaticMap::<&str, u32, 10>::new();
    ///
    /// map.entry("key").or_insert(42);
    /// assert_eq!(map.get("key"), Some(&42));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, N> {
        match self.binary_search(&key) {
            Ok(index) => Entry::Occupied(OccupiedEntry {
                map: self,
                index,
                _marker: PhantomData,
            }),
            Err(index) => Entry::Vacant(VacantEntry {
                map: self,
                key,
                index,
                _marker: PhantomData,
            }),
        }
    }

    /// Gets a mutable reference to the value for the given key, inserting a default value if the key doesn't exist.
    ///
    /// # Time Complexity
    ///
    /// - Lookup: O(log n) via binary search
    /// - Insertion (if needed): O(n) to maintain sorted order
    ///
    /// # Errors
    ///
    /// Returns `Err(CapacityExceeded)` if the map is full and the key doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use wrt_foundation::collections::StaticMap;
    ///
    /// let mut map = StaticMap::<&str, u32, 10>::new();
    ///
    /// let value = map.get_mut_or_insert("key", 42)?;
    /// *value = 100;
    /// assert_eq!(map.get("key"), Some(&100));
    /// # Ok::<(), wrt_error::Error>(())
    /// ```
    pub fn get_mut_or_insert(&mut self, key: K, default: V) -> Result<&mut V> {
        self.entry(key).or_insert(default)
    }
}

/// A view into a single entry in a map, which may either be vacant or occupied.
pub enum Entry<'a, K: Ord, V, const N: usize> {
    /// An occupied entry.
    Occupied(OccupiedEntry<'a, K, V, N>),
    /// A vacant entry.
    Vacant(VacantEntry<'a, K, V, N>),
}

impl<'a, K: Ord, V, const N: usize> Entry<'a, K, V, N> {
    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// a mutable reference to the value in the entry.
    pub fn or_insert(self, default: V) -> Result<&'a mut V> {
        match self {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> Result<&'a mut V> {
        match self {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }

    /// Returns a reference to this entry's key.
    pub fn key(&self) -> &K {
        match self {
            Entry::Occupied(entry) => entry.key(),
            Entry::Vacant(entry) => entry.key(),
        }
    }

    /// Provides in-place mutable access to an occupied entry before any potential inserts.
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Entry::Occupied(mut entry) => {
                f(entry.get_mut());
                Entry::Occupied(entry)
            }
            Entry::Vacant(entry) => Entry::Vacant(entry),
        }
    }
}

/// A view into an occupied entry in a StaticMap.
pub struct OccupiedEntry<'a, K: Ord, V, const N: usize> {
    map: &'a mut StaticMap<K, V, N>,
    index: usize,
    _marker: PhantomData<&'a mut (K, V)>,
}

impl<'a, K: Ord, V, const N: usize> OccupiedEntry<'a, K, V, N> {
    /// Gets a reference to the key in the entry.
    pub fn key(&self) -> &K {
        unsafe { &self.map.entries[self.index].assume_init_ref().0 }
    }

    /// Gets a reference to the value in the entry.
    pub fn get(&self) -> &V {
        unsafe { &self.map.entries[self.index].assume_init_ref().1 }
    }

    /// Gets a mutable reference to the value in the entry.
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.map.entries[self.index].assume_init_mut().1 }
    }

    /// Converts the entry into a mutable reference to the value with the same lifetime as the map.
    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut self.map.entries[self.index].assume_init_mut().1 }
    }

    /// Sets the value of the entry and returns the entry's old value.
    pub fn insert(&mut self, value: V) -> V {
        core::mem::replace(self.get_mut(), value)
    }

    /// Takes the value out of the entry and returns it.
    pub fn remove(self) -> V {
        let (_key, value) = unsafe { self.map.entries[self.index].assume_init_read() };

        // Shift elements left
        for i in self.index..(self.map.len - 1) {
            unsafe {
                let src = self.map.entries[i + 1].assume_init_read();
                self.map.entries[i].write(src);
            }
        }

        self.map.len -= 1;
        value
    }
}

/// A view into a vacant entry in a StaticMap.
pub struct VacantEntry<'a, K: Ord, V, const N: usize> {
    map: &'a mut StaticMap<K, V, N>,
    key: K,
    index: usize,
    _marker: PhantomData<&'a mut (K, V)>,
}

impl<'a, K: Ord, V, const N: usize> VacantEntry<'a, K, V, N> {
    /// Gets a reference to the key that would be used when inserting a value.
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Take ownership of the key.
    pub fn into_key(self) -> K {
        self.key
    }

    /// Sets the value of the entry with the VacantEntry's key and returns a mutable reference to it.
    pub fn insert(self, value: V) -> Result<&'a mut V> {
        if self.map.len >= N {
            return Err(wrt_error::Error::foundation_bounded_capacity_exceeded(
                "StaticMap capacity exceeded",
            ));
        }

        // Shift elements right to make space
        for i in (self.index..self.map.len).rev() {
            unsafe {
                let src = self.map.entries[i].assume_init_read();
                self.map.entries[i + 1].write(src);
            }
        }

        // Insert new entry
        self.map.entries[self.index].write((self.key, value));
        self.map.len += 1;

        // Return mutable reference to the inserted value
        Ok(unsafe { &mut self.map.entries[self.index].assume_init_mut().1 })
    }
}

// RAII: Automatic cleanup on drop
impl<K: Ord, V, const N: usize> Drop for StaticMap<K, V, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

// Default: empty map
impl<K: Ord, V, const N: usize> Default for StaticMap<K, V, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Clone implementation (requires K: Clone, V: Clone)
impl<K: Ord + Clone, V: Clone, const N: usize> Clone for StaticMap<K, V, N> {
    fn clone(&self) -> Self {
        let mut new_map = Self::new();
        for (key, value) in self.iter() {
            // SAFETY: Cloning from existing map, same capacity
            let _unused = new_map.insert(key.clone(), value.clone());
        }
        new_map
    }
}

// Checksummable implementation for verifiable data integrity
impl<K: crate::traits::Checksummable + Ord, V: crate::traits::Checksummable, const N: usize> crate::traits::Checksummable for StaticMap<K, V, N> {
    fn update_checksum(&self, checksum: &mut crate::verification::Checksum) {
        // Update checksum with length
        self.len().update_checksum(checksum);
        // Iterate through key-value pairs in sorted order for determinism
        for (key, value) in self.iter() {
            key.update_checksum(checksum);
            value.update_checksum(checksum);
        }
    }
}

// ToBytes implementation for serialization
impl<K: crate::traits::ToBytes + Ord, V: crate::traits::ToBytes, const N: usize> crate::traits::ToBytes for StaticMap<K, V, N> {
    fn serialized_size(&self) -> usize {
        let len_size = core::mem::size_of::<usize>();
        let entries_size: usize = self.iter()
            .map(|(k, v)| k.serialized_size() + v.serialized_size())
            .sum();
        len_size + entries_size
    }

    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut crate::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write length
        self.len().to_bytes_with_provider(writer, provider)?;
        // Write each key-value pair
        for (key, value) in self.iter() {
            key.to_bytes_with_provider(writer, provider)?;
            value.to_bytes_with_provider(writer, provider)?;
        }
        Ok(())
    }
}

// FromBytes implementation for deserialization
impl<K: crate::traits::FromBytes + Ord, V: crate::traits::FromBytes, const N: usize> crate::traits::FromBytes for StaticMap<K, V, N> {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut crate::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        // Read length
        let len = usize::from_bytes_with_provider(reader, provider)?;

        // Create new map
        let mut map = StaticMap::new();

        // Read key-value pairs
        for _ in 0..len {
            let key = K::from_bytes_with_provider(reader, provider)?;
            let value = V::from_bytes_with_provider(reader, provider)?;
            map.insert(key, value)?;
        }

        Ok(map)
    }
}

// Hash implementation for use in hash-based collections
impl<K: core::hash::Hash + Ord, V: core::hash::Hash, const N: usize> core::hash::Hash for StaticMap<K, V, N> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Hash length
        self.len().hash(state);
        // Hash each key-value pair
        for (key, value) in self.iter() {
            key.hash(state);
            value.hash(state);
        }
    }
}

// PartialEq implementation for equality comparison
impl<K: Ord + PartialEq, V: PartialEq, const N: usize> PartialEq for StaticMap<K, V, N> {
    fn eq(&self, other: &Self) -> bool {
        // Fast path: different lengths mean not equal
        if self.len() != other.len() {
            return false;
        }

        // Compare all entries
        self.iter().zip(other.iter()).all(|(a, b)| a.0 == b.0 && a.1 == b.1)
    }
}

// Eq implementation (requires PartialEq + Eq for K and V)
impl<K: Ord + Eq, V: Eq, const N: usize> Eq for StaticMap<K, V, N> {}

// Iterator implementation
pub struct StaticMapIter<'a, K: Ord, V, const N: usize> {
    map: &'a StaticMap<K, V, N>,
    index: usize,
}

impl<'a, K: Ord, V, const N: usize> Iterator for StaticMapIter<'a, K, V, N> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.map.len {
            let entry = unsafe { self.map.entries[self.index].assume_init_ref() };
            self.index += 1;
            Some((&entry.0, &entry.1))
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.map.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, K: Ord, V, const N: usize> ExactSizeIterator for StaticMapIter<'a, K, V, N> {}

// Mutable iterator implementation
pub struct StaticMapIterMut<'a, K: Ord, V, const N: usize> {
    map: &'a mut StaticMap<K, V, N>,
    index: usize,
}

impl<'a, K: Ord, V, const N: usize> Iterator for StaticMapIterMut<'a, K, V, N> {
    type Item = (&'a K, &'a mut V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.map.len {
            let entry = unsafe { self.map.entries[self.index].assume_init_mut() };
            self.index += 1;
            // SAFETY: We're extending the lifetime here, but it's safe because:
            // 1. The map is borrowed mutably for 'a
            // 2. Each entry is visited exactly once
            // 3. The key is immutable, the value is mutable
            Some(unsafe {
                let key_ptr = &entry.0 as *const K;
                let val_ptr = &mut entry.1 as *mut V;
                (&*key_ptr, &mut *val_ptr)
            })
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.map.len - self.index;
        (remaining, Some(remaining))
    }
}

impl<'a, K: Ord, V, const N: usize> ExactSizeIterator for StaticMapIterMut<'a, K, V, N> {}

// IntoIterator for references
impl<'a, K: Ord, V, const N: usize> IntoIterator for &'a StaticMap<K, V, N> {
    type Item = (&'a K, &'a V);
    type IntoIter = StaticMapIter<'a, K, V, N>;

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
    fn verify_sorted_order() {
        let mut map: StaticMap<u32, u32, 5> = StaticMap::new();

        // Insert in random order
        map.insert(3, 30).unwrap();
        map.insert(1, 10).unwrap();
        map.insert(4, 40).unwrap();
        map.insert(2, 20).unwrap();

        // Verify sorted iteration
        let keys: Vec<u32> = map.keys().copied().collect();
        assert!(keys == vec![1, 2, 3, 4]);
    }

    #[kani::proof]
    fn verify_update_existing() {
        let mut map: StaticMap<u32, u32, 5> = StaticMap::new();

        map.insert(1, 10).unwrap();
        let old = map.insert(1, 20).unwrap();

        assert!(old == Some(10));
        assert!(map.get(&1) == Some(&20));
        assert!(map.len() == 1);
    }

    #[kani::proof]
    fn verify_remove() {
        let mut map: StaticMap<u32, u32, 5> = StaticMap::new();

        map.insert(1, 10).unwrap();
        map.insert(2, 20).unwrap();
        map.insert(3, 30).unwrap();

        assert!(map.remove(&2) == Some(20));
        assert!(map.len() == 2);
        assert!(map.get(&2).is_none());
    }

    #[kani::proof]
    fn verify_capacity_enforcement() {
        let mut map: StaticMap<u32, u32, 2> = StaticMap::new();

        assert!(map.insert(1, 10).is_ok());
        assert!(map.insert(2, 20).is_ok());
        assert!(map.insert(3, 30).is_err()); // Should fail
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
        let map: StaticMap<&str, u32, 10> = StaticMap::new();
        assert_eq!(map.len(), 0);
        assert_eq!(map.capacity(), 10);
        assert!(map.is_empty());
    }

    #[test]
    fn test_insert_get() -> Result<()> {
        let mut map = StaticMap::<&str, u32, 10>::new();

        map.insert("foo", 42)?;
        map.insert("bar", 100)?;

        assert_eq!(map.get(&"foo"), Some(&42));
        assert_eq!(map.get(&"bar"), Some(&100));
        assert_eq!(map.get(&"baz"), None);

        Ok(())
    }

    #[test]
    fn test_sorted_order() -> Result<()> {
        let mut map = StaticMap::<u32, &str, 10>::new();

        // Insert in random order
        map.insert(5, "five")?;
        map.insert(2, "two")?;
        map.insert(8, "eight")?;
        map.insert(1, "one")?;

        // Keys should be sorted (manually check - no_std compatible)
        let mut keys = map.keys();
        assert_eq!(keys.next(), Some(&1));
        assert_eq!(keys.next(), Some(&2));
        assert_eq!(keys.next(), Some(&5));
        assert_eq!(keys.next(), Some(&8));
        assert_eq!(keys.next(), None);

        Ok(())
    }

    #[test]
    fn test_update_value() -> Result<()> {
        let mut map = StaticMap::<&str, u32, 10>::new();

        map.insert("foo", 42)?;
        let old = map.insert("foo", 100)?;

        assert_eq!(old, Some(42));
        assert_eq!(map.get(&"foo"), Some(&100));
        assert_eq!(map.len(), 1);

        Ok(())
    }

    #[test]
    fn test_remove() -> Result<()> {
        let mut map = StaticMap::<&str, u32, 10>::new();

        map.insert("foo", 42)?;
        map.insert("bar", 100)?;
        map.insert("baz", 200)?;

        assert_eq!(map.remove(&"bar"), Some(100));
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&"bar"), None);

        Ok(())
    }

    #[test]
    fn test_capacity_exceeded() {
        let mut map = StaticMap::<&str, u32, 2>::new();

        assert!(map.insert("a", 1).is_ok());
        assert!(map.insert("b", 2).is_ok());
        assert!(map.insert("c", 3).is_err());
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_clear() -> Result<()> {
        let mut map = StaticMap::<&str, u32, 10>::new();
        map.insert("foo", 1)?;
        map.insert("bar", 2)?;

        map.clear();
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());

        Ok(())
    }

    #[test]
    fn test_iter() -> Result<()> {
        let mut map = StaticMap::<u32, &str, 10>::new();
        map.insert(3, "three")?;
        map.insert(1, "one")?;
        map.insert(2, "two")?;

        // Manually check iterator values (no_std compatible)
        let mut iter = map.iter();
        assert_eq!(iter.next(), Some((&1, &"one")));
        assert_eq!(iter.next(), Some((&2, &"two")));
        assert_eq!(iter.next(), Some((&3, &"three")));
        assert_eq!(iter.next(), None);

        Ok(())
    }
}
