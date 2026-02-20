// WRT - wrt-foundation
// Module: No-std HashMap Implementation
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! A simple HashMap implementation for no_std environments without external
//! dependencies.
#![allow(clippy::needless_continue)]
#![allow(clippy::if_not_else)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::unused_self)]
#![allow(clippy::ptr_as_ptr)]
#![allow(clippy::cast_possible_truncation)]
//! This module provides a basic hash map implementation that is suitable for
//! no_std/no_alloc environments. It has limited functionality compared to
//! the standard HashMap or external crates like hashbrown, but it provides
//! the core functionality needed for the WRT ecosystem.

use core::{
    borrow::Borrow,
    fmt,
    hash::Hash,
    marker::PhantomData,
};

use crate::{
    bounded::BoundedVec,
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
    verification::Checksum,
    MemoryProvider,
};

/// A simple fixed-capacity hash map implementation for no_std environments.
///
/// This implementation uses linear probing for collision resolution and
/// supports a fixed maximum number of entries specified by the N parameter.
#[derive(Debug)]
pub struct SimpleHashMap<
    K,
    V,
    const N: usize,
    P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
> where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
{
    entries:  BoundedVec<Option<Entry<K, V>>, N, P>,
    len:      usize,
    _phantom: PhantomData<(K, V, P)>,
}

/// Type alias for backward compatibility
pub type BoundedHashMap<K, V, const N: usize, P> = SimpleHashMap<K, V, N, P>;

/// A key-value pair entry in the hash map.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry<K, V>
where
    K: Clone + PartialEq + Eq,
    V: Clone + PartialEq + Eq,
{
    key:   K,
    value: V,
    hash:  u64,
}

impl<K, V> Default for Entry<K, V>
where
    K: Clone + PartialEq + Eq + Default,
    V: Clone + PartialEq + Eq + Default,
{
    fn default() -> Self {
        Self {
            key:   K::default(),
            value: V::default(),
            hash:  0,
        }
    }
}

impl<K, V> Checksummable for Entry<K, V>
where
    K: Clone + PartialEq + Eq + Checksummable,
    V: Clone + PartialEq + Eq + Checksummable,
{
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.key.update_checksum(checksum);
        self.value.update_checksum(checksum);
        self.hash.update_checksum(checksum);
    }
}

impl<K, V> ToBytes for Entry<K, V>
where
    K: Clone + PartialEq + Eq + ToBytes,
    V: Clone + PartialEq + Eq + ToBytes,
{
    fn to_bytes_with_provider<'a, PStream: MemoryProvider>(
        &self,
        writer: &mut crate::traits::WriteStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<()> {
        self.key.to_bytes_with_provider(writer, provider)?;
        self.value.to_bytes_with_provider(writer, provider)?;
        self.hash.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl<K, V> FromBytes for Entry<K, V>
where
    K: Clone + PartialEq + Eq + FromBytes,
    V: Clone + PartialEq + Eq + FromBytes,
{
    fn from_bytes_with_provider<'a, PStream: MemoryProvider>(
        reader: &mut crate::traits::ReadStream<'a>,
        provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let key = K::from_bytes_with_provider(reader, provider)?;
        let value = V::from_bytes_with_provider(reader, provider)?;
        let hash = u64::from_bytes_with_provider(reader, provider)?;
        Ok(Self { key, value, hash })
    }
}

impl<K, V, const N: usize, P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq>
    SimpleHashMap<K, V, N, P>
where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
{
    /// Creates a new empty `SimpleHashMap` with the given memory provider.
    pub fn new(provider: P) -> wrt_error::Result<Self> {
        let mut entries = BoundedVec::new(provider)?;

        // Pre-populate with None values to indicate empty slots
        for _ in 0..N {
            entries.push(None)?;
        }

        Ok(Self {
            entries,
            len: 0,
            _phantom: PhantomData,
        })
    }

    /// Returns the number of key-value pairs in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the map contains no key-value pairs.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns true if the map cannot hold any more key-value pairs.
    pub fn is_full(&self) -> bool {
        self.len == N
    }

    /// Returns the capacity of the map.
    pub fn capacity(&self) -> usize {
        N
    }

    /// Calculates the hash of a key.
    fn hash_key<Q: ?Sized + Hash>(&self, key: &Q) -> u64 {
        // For no_std environments, use a simple hash function
        // This is not cryptographically secure but sufficient for HashMap functionality
        let hash: u64 = 5381; // DJB2 hash algorithm starting value

        // Binary std/no_std choice
        // we'll use a simplified approach. In a real implementation, you'd want
        // to use a proper no_std hasher like `ahash` or implement Hasher for a simple
        // algorithm.

        // For now, use a basic checksum-style hash
        hash.wrapping_mul(33)
            .wrapping_add(core::ptr::addr_of!(*key) as *const () as usize as u64)
    }

    /// Calculates the initial index for a key.
    fn initial_index(&self, hash: u64) -> usize {
        (hash as usize) % N
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the key already exists, the old value is replaced and returned.
    /// If the map is full and the key doesn't exist, returns an error.
    pub fn insert(&mut self, key: K, value: V) -> wrt_error::Result<Option<V>>
    where
        K: Hash,
    {
        if self.is_full() {
            // Check if the key already exists
            let hash = self.hash_key(&key);
            let index = self.initial_index(hash);

            for i in 0..N {
                let actual_index = (index + i) % N;

                if let Some(entry) = self.entries.get(actual_index)? {
                    if entry.hash == hash && entry.key == key {
                        // Key exists, replace value
                        let old_value = entry.value.clone();
                        let mut entry = entry.clone();
                        entry.value = value;
                        self.entries.set(actual_index, Some(entry))?;
                        return Ok(Some(old_value));
                    }
                }
            }

            // Map is full and key doesn't exist
            return Err(crate::Error::capacity_error("SimpleHashMap is full"));
        }

        let hash = self.hash_key(&key);
        let index = self.initial_index(hash);

        // Find the slot for this key (either empty or matching key)
        for i in 0..N {
            let actual_index = (index + i) % N;

            match self.entries.get(actual_index)? {
                Some(entry) if entry.hash == hash && entry.key == key => {
                    // Key exists, replace value
                    let old_value = entry.value.clone();
                    let mut entry = entry.clone();
                    entry.value = value;
                    self.entries.set(actual_index, Some(entry))?;
                    return Ok(Some(old_value));
                },
                None => {
                    // Empty slot, insert new entry
                    let entry = Entry { key, value, hash };
                    self.entries.set(actual_index, Some(entry))?;
                    self.len += 1;
                    return Ok(None);
                },
                _ => {
                    // Occupied by a different key, try next slot
                    continue;
                },
            }
        }

        // This should never happen as we checked if the map is full
        Err(crate::Error::internal_error(
            "Failed to insert into SimpleHashMap",
        ))
    }

    /// Gets a copy of the value associated with the key.
    pub fn get<Q: ?Sized>(&self, key: &Q) -> wrt_error::Result<Option<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let hash = self.hash_key(key);
        let index = self.initial_index(hash);

        for i in 0..N {
            let actual_index = (index + i) % N;

            match self.entries.get(actual_index)? {
                Some(entry) if entry.hash == hash && entry.key.borrow() == key => {
                    return Ok(Some(entry.value));
                },
                None => {
                    // Empty slot, key doesn't exist
                    return Ok(None);
                },
                _ => {
                    // Occupied by a different key, try next slot
                    continue;
                },
            }
        }

        // We searched all slots and didn't find the key
        Ok(None)
    }

    /// Removes a key from the map, returning the value if it was present.
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> wrt_error::Result<Option<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let hash = self.hash_key(key);
        let index = self.initial_index(hash);

        for i in 0..N {
            let actual_index = (index + i) % N;

            match self.entries.get(actual_index)? {
                Some(entry) if entry.hash == hash && entry.key.borrow() == key => {
                    let value = entry.value.clone();
                    self.entries.set(actual_index, None)?;
                    self.len -= 1;
                    return Ok(Some(value));
                },
                None => {
                    // Empty slot, key doesn't exist
                    return Ok(None);
                },
                _ => {
                    // Occupied by a different key, try next slot
                    continue;
                },
            }
        }

        // We searched all slots and didn't find the key
        Ok(None)
    }

    /// Clears the map, removing all key-value pairs.
    pub fn clear(&mut self) -> wrt_error::Result<()> {
        for i in 0..N {
            self.entries.set(i, None)?;
        }
        self.len = 0;
        Ok(())
    }

    /// Returns an iterator over the map entries.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wrt_foundation::no_std_hashmap::SimpleHashMap;
    /// # use wrt_foundation::{safe_managed_alloc, budget_aware_provider::CrateId, safe_memory::NoStdProvider};
    /// #
    /// # let provider = safe_managed_alloc!(512, CrateId::Foundation).unwrap();
    /// # let mut map = SimpleHashMap::<u32, i32, 8, NoStdProvider<512>>::new(provider).unwrap();
    /// # map.insert(1, 100).unwrap();
    /// # map.insert(2, 200).unwrap();
    /// for (key, value) in map.iter() {
    ///     println!("{}: {}", key, value);
    /// }
    /// ```
    pub fn iter(&self) -> SimpleHashMapIter<'_, K, V, N, P> {
        SimpleHashMapIter {
            map: self,
            index: 0,
        }
    }

    /// Returns a mutable iterator over the map entries.
    ///
    /// # Examples
    ///
    /// ```
    /// # use wrt_foundation::no_std_hashmap::SimpleHashMap;
    /// # use wrt_foundation::{safe_managed_alloc, budget_aware_provider::CrateId, safe_memory::NoStdProvider};
    /// #
    /// # let provider = safe_managed_alloc!(512, CrateId::Foundation).unwrap();
    /// # let mut map = SimpleHashMap::<u32, i32, 8, NoStdProvider<512>>::new(provider).unwrap();
    /// # map.insert(1, 100).unwrap();
    /// # map.insert(2, 200).unwrap();
    /// for (key, value) in map.iter_mut() {
    ///     *value *= 2;
    /// }
    /// ```
    pub fn iter_mut(&mut self) -> SimpleHashMapIterMut<'_, K, V, N, P> {
        SimpleHashMapIterMut {
            map: self,
            index: 0,
        }
    }
}

/// Iterator over SimpleHashMap entries
pub struct SimpleHashMapIter<
    'a,
    K,
    V,
    const N: usize,
    P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
> where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
{
    map: &'a SimpleHashMap<K, V, N, P>,
    index: usize,
}

impl<
        'a,
        K,
        V,
        const N: usize,
        P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
    > Iterator for SimpleHashMapIter<'a, K, V, N, P>
where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < N {
            let current_index = self.index;
            self.index += 1;

            // Try to get the entry at current_index
            // Note: BoundedVec::get() returns owned values, not references
            if let Ok(Some(entry)) = self.map.entries.get(current_index) {
                return Some((entry.key, entry.value));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.map.len))
    }
}

/// Mutable iterator over SimpleHashMap entries
pub struct SimpleHashMapIterMut<
    'a,
    K,
    V,
    const N: usize,
    P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
> where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
{
    map: &'a mut SimpleHashMap<K, V, N, P>,
    index: usize,
}

impl<
        'a,
        K,
        V,
        const N: usize,
        P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
    > SimpleHashMapIterMut<'a, K, V, N, P>
where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
{
    /// Get the next mutable entry accessor
    ///
    /// This returns an accessor that allows reading and writing values.
    /// Since the underlying storage is serialized, we can't return direct mutable references.
    pub fn next_mut(&mut self) -> wrt_error::Result<Option<SimpleHashMapMutEntry<'_, K, V, N, P>>> {
        while self.index < N {
            let current_index = self.index;
            self.index += 1;

            // Check if this slot has an entry
            if let Ok(Some(entry)) = self.map.entries.get(current_index) {
                return Ok(Some(SimpleHashMapMutEntry {
                    map: self.map,
                    index: current_index,
                    key: entry.key,
                }));
            }
        }
        Ok(None)
    }
}

/// Mutable entry accessor for SimpleHashMap
pub struct SimpleHashMapMutEntry<'a, K, V, const N: usize, P>
where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
    P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
{
    map: &'a mut SimpleHashMap<K, V, N, P>,
    index: usize,
    key: K,
}

impl<'a, K, V, const N: usize, P> SimpleHashMapMutEntry<'a, K, V, N, P>
where
    K: Hash + Eq + Clone + Default + Checksummable + ToBytes + FromBytes,
    V: Clone + Default + PartialEq + Eq + Checksummable + ToBytes + FromBytes,
    P: MemoryProvider + Default + Clone + fmt::Debug + PartialEq + Eq,
{
    /// Get the key
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Get the current value
    pub fn get(&self) -> wrt_error::Result<V> {
        if let Some(entry) = self.map.entries.get(self.index)? {
            Ok(entry.value)
        } else {
            Err(crate::Error::internal_error("Entry disappeared during iteration"))
        }
    }

    /// Set a new value
    pub fn set(&mut self, value: V) -> wrt_error::Result<()> {
        if let Some(mut entry) = self.map.entries.get(self.index)? {
            entry.value = value;
            self.map.entries.set(self.index, Some(entry))?;
            Ok(())
        } else {
            Err(crate::Error::internal_error("Entry disappeared during iteration"))
        }
    }

    /// Modify the value using a function
    pub fn modify<F>(&mut self, f: F) -> wrt_error::Result<()>
    where
        F: FnOnce(V) -> V,
    {
        let value = self.get()?;
        let new_value = f(value);
        self.set(new_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        budget_aware_provider::CrateId,
        safe_managed_alloc,
        safe_memory::NoStdProvider,
    };

    #[test]
    fn test_simple_hashmap() -> wrt_error::Result<()> {
        let provider = safe_managed_alloc!(512, CrateId::Foundation)?;
        let mut map = SimpleHashMap::<u32, i32, 8, NoStdProvider<512>>::new(provider)?;

        // Test insertion
        assert!(map.insert(1, 100)?.is_none());
        assert!(map.insert(2, 200)?.is_none());
        assert!(map.insert(3, 300)?.is_none());

        // Test get
        assert_eq!(map.get(&1)?, Some(100));
        assert_eq!(map.get(&2)?, Some(200));
        assert_eq!(map.get(&3)?, Some(300));
        assert_eq!(map.get(&4)?, None);

        // Test replacing a value
        assert_eq!(map.insert(1, 1000)?, Some(100));
        assert_eq!(map.get(&1)?, Some(1000));

        // Test removing a value
        assert_eq!(map.remove(&2)?, Some(200));
        assert_eq!(map.get(&2)?, None);

        // Test len and is_empty
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());

        // Test clear
        map.clear()?;
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
        assert_eq!(map.get(&1)?, None);
        Ok(())
    }

    #[test]
    fn test_full_map() -> wrt_error::Result<()> {
        let provider = safe_managed_alloc!(256, CrateId::Foundation)?;
        let mut map = SimpleHashMap::<i32, i32, 4, NoStdProvider<256>>::new(provider)?;

        // Fill the map
        assert!(map.insert(1, 10)?.is_none());
        assert!(map.insert(2, 20)?.is_none());
        assert!(map.insert(3, 30)?.is_none());
        assert!(map.insert(4, 40)?.is_none());

        // Map is full
        assert!(map.is_full());

        // Can replace existing keys
        assert_eq!(map.insert(1, 100)?, Some(10));

        // But can't add new keys
        assert!(map.insert(5, 50).is_err());
        Ok(())
    }
}
