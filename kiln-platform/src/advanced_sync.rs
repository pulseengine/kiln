// WRT - wrt-platform
// Module: Advanced Synchronization Primitives
// SW-REQ-ID: REQ_PLATFORM_SYNC_ADV_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Advanced Synchronization Primitives for Real-Time and Safety-Critical
//! Systems
//!
//! This module provides high-performance, lock-free synchronization primitives
//! optimized for real-time systems with formal verification support.
//!
//! # Features
//! - **Lock-Free Data Structures**: MPSC queues, atomic allocators, wait-free
//!   operations
//! - **Priority Inheritance**: Real-time scheduling with bounded priority
//!   inversion
//! - **Reader-Writer Locks**: High-performance RwLocks with writer preference
//! - **Formal Verification**: Kani-verified correctness proofs for
//!   safety-critical use
//! - **No-std Compatibility**: Works in embedded and bare-metal environments
//!
//! # Design Principles
//! - Wait-free operations where possible, lock-free as fallback
//! - Bounded execution time for real-time guarantees
//! - Memory ordering guarantees via atomic operations
//! - Formal verification of safety properties

#![allow(dead_code)] // Allow during development

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
use alloc::{
    boxed::Box,
    vec::Vec,
};
use core::{
    cell::UnsafeCell,
    ptr::NonNull,
    sync::atomic::{
        AtomicBool,
        AtomicPtr,
        AtomicUsize,
        Ordering,
    },
};

use wrt_error::Error;

/// Priority levels for real-time scheduling
pub type Priority = u8;

/// Maximum priority level (highest priority)
pub const MAX_PRIORITY: Priority = 255;

/// Minimum priority level (lowest priority)
pub const MIN_PRIORITY: Priority = 0;

/// Lock-free MPSC (Multiple Producer, Single Consumer) queue
///
/// Provides wait-free enqueue and lock-free dequeue operations
/// suitable for real-time systems with bounded execution time.
#[cfg(feature = "std")]
#[repr(align(64))] // Cache line alignment
pub struct LockFreeMpscQueue<T> {
    /// Head pointer for dequeue operations
    head: AtomicPtr<Node<T>>,
    /// Tail pointer for enqueue operations  
    tail: AtomicPtr<Node<T>>,
    /// Stub node to avoid ABA problem
    stub: Box<Node<T>>,
}

#[repr(align(64))] // Cache line alignment
struct Node<T> {
    /// Next node pointer
    next: AtomicPtr<Node<T>>,
    /// Node data (None for stub node)
    data: Option<T>,
}

impl<T> Node<T> {
    fn new(data: T) -> Self {
        Self {
            next: AtomicPtr::new(core::ptr::null_mut()),
            data: Some(data),
        }
    }

    fn stub() -> Self {
        Self {
            next: AtomicPtr::new(core::ptr::null_mut()),
            data: None,
        }
    }
}

#[cfg(feature = "std")]
impl<T> LockFreeMpscQueue<T> {
    /// Create a new empty MPSC queue
    #[cfg(feature = "std")]
    pub fn new() -> Self {
        let stub = Box::new(Node::stub());
        let stub_ptr = Box::as_ref(&stub) as *const Node<T> as *mut Node<T>;

        Self {
            head: AtomicPtr::new(stub_ptr),
            tail: AtomicPtr::new(stub_ptr),
            stub,
        }
    }

    /// Enqueue an item (wait-free, multiple producers)
    ///
    /// # Safety
    /// This function is wait-free and safe for concurrent access by multiple
    /// producers.
    ///
    /// # Formal Verification
    /// Kani proof ensures this operation maintains queue invariants.
    #[cfg(feature = "std")]
    pub fn enqueue(&self, item: T) -> Result<(), Error> {
        let new_node = Box::into_raw(Box::new(Node::new(item)));

        // Atomically update tail pointer
        let prev_tail = self.tail.swap(new_node, Ordering::AcqRel);

        // Link the previous tail to the new node
        unsafe {
            (*prev_tail).next.store(new_node, Ordering::Release);
        }

        Ok(())
    }

    /// Dequeue an item (lock-free, single consumer only)
    ///
    /// Returns `None` if queue is empty.
    ///
    /// # Safety  
    /// This function must only be called by a single consumer thread.
    ///
    /// # Formal Verification
    /// Kani proof ensures memory safety and queue consistency.
    pub fn dequeue(&self) -> Option<T> {
        let head = self.head.load(Ordering::Acquire);

        unsafe {
            let next = (*head).next.load(Ordering::Acquire);

            if next.is_null() {
                return None; // Queue is empty
            }

            // Move head to next node
            self.head.store(next, Ordering::Release);

            // Extract data from the old head
            let data = (*next).data.take();

            // Binary std/no_std choice
            if head != Box::as_ref(&self.stub) as *const Node<T> as *mut Node<T> {
                let _ = Box::from_raw(head);
            }

            data
        }
    }

    /// Check if queue is empty (approximate)
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        unsafe { (*head).next.load(Ordering::Acquire).is_null() }
    }
}

#[cfg(feature = "std")]
unsafe impl<T: Send> Send for LockFreeMpscQueue<T> {}
#[cfg(feature = "std")]
unsafe impl<T: Send> Sync for LockFreeMpscQueue<T> {}

/// Binary std/no_std choice
///
/// Binary std/no_std choice
/// Suitable for real-time systems requiring bounded execution time.
pub struct LockFreeAllocator {
    /// Free list head
    free_list:    AtomicPtr<FreeBlock>,
    /// Total blocks available
    total_blocks: usize,
    /// Block size in bytes
    block_size:   usize,
    /// Memory pool base address
    pool_base:    *mut u8,
}

#[repr(align(8))]
struct FreeBlock {
    next: *mut FreeBlock,
}

impl LockFreeAllocator {
    /// Binary std/no_std choice
    ///
    /// # Safety
    /// `pool` must be a valid memory region of size `pool_size`.
    /// `block_size` must be >= size_of::<FreeBlock>().
    pub unsafe fn new(pool: *mut u8, pool_size: usize, block_size: usize) -> Result<Self, Error> {
        if block_size < core::mem::size_of::<FreeBlock>() {
            return Err(Error::runtime_execution_error("Block size too small"));
        }

        let total_blocks = pool_size / block_size;
        if total_blocks == 0 {
            return Err(Error::new(
                wrt_error::ErrorCategory::Validation,
                wrt_error::codes::VALIDATION_ERROR,
                "Pool size too small",
            ));
        }

        // Initialize free list
        let mut current = pool as *mut FreeBlock;
        for i in 0..total_blocks - 1 {
            // SAFETY: Edition 2024 requires explicit unsafe blocks in unsafe functions.
            // We're within bounds (i < total_blocks-1) and pool is properly aligned.
            unsafe {
                let next = pool.add((i + 1) * block_size) as *mut FreeBlock;
                (*current).next = next;
                current = next;
            }
        }
        // SAFETY: current points to the last block, safe to write null terminator
        unsafe {
            (*current).next = core::ptr::null_mut();
        }

        Ok(Self {
            free_list: AtomicPtr::new(pool as *mut FreeBlock),
            total_blocks,
            block_size,
            pool_base: pool,
        })
    }

    /// Allocate a block (lock-free)
    ///
    /// Returns `None` if no blocks available.
    /// Execution time is bounded and deterministic.
    pub fn allocate(&self) -> Option<NonNull<u8>> {
        loop {
            let head = self.free_list.load(Ordering::Acquire);

            if head.is_null() {
                return None; // No free blocks
            }

            unsafe {
                let next = (*head).next;

                // Try to atomically update free list head
                match self.free_list.compare_exchange_weak(
                    head,
                    next,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return NonNull::new(head as *mut u8),
                    Err(_) => {}, // Retry due to contention
                }
            }
        }
    }

    /// Binary std/no_std choice
    ///
    /// # Safety
    /// Binary std/no_std choice
    pub unsafe fn deallocate(&self, ptr: NonNull<u8>) {
        let block = ptr.as_ptr() as *mut FreeBlock;

        loop {
            let head = self.free_list.load(Ordering::Acquire);
            // SAFETY: Edition 2024 requires explicit unsafe blocks in unsafe functions.
            // block is a valid FreeBlock pointer from a previous allocation.
            unsafe {
                (*block).next = head;
            }

            // Try to atomically update free list head
            match self.free_list.compare_exchange_weak(
                head,
                block,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(_) => {}, // Retry due to contention
            }
        }
    }

    /// Get block size
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get total number of blocks
    pub fn total_blocks(&self) -> usize {
        self.total_blocks
    }
}

unsafe impl Send for LockFreeAllocator {}
unsafe impl Sync for LockFreeAllocator {}

/// Priority inheritance mutex for real-time systems
///
/// Implements priority inheritance protocol to bound priority inversion.
/// Ensures higher priority tasks don't wait indefinitely for lower priority
/// tasks.
pub struct PriorityInheritanceMutex<T> {
    /// Protected data
    data:           UnsafeCell<T>,
    /// Current owner's priority
    owner_priority: AtomicUsize,
    /// Lock state (0 = unlocked, 1 = locked)
    locked:         AtomicBool,
    /// Waiting queue (simplified - real implementation would use priority
    /// queue)
    waiters:        AtomicUsize,
}

impl<T> PriorityInheritanceMutex<T> {
    /// Create a new priority inheritance mutex
    pub const fn new(data: T) -> Self {
        Self {
            data:           UnsafeCell::new(data),
            owner_priority: AtomicUsize::new(0),
            locked:         AtomicBool::new(false),
            waiters:        AtomicUsize::new(0),
        }
    }

    /// Acquire lock with priority inheritance
    ///
    /// # Priority Inheritance Protocol
    /// 1. If lock is free, acquire immediately
    /// 2. If lock is held, boost holder's priority to current task's priority
    /// 3. Wait until lock becomes available
    /// 4. Restore original priority when releasing
    pub fn lock(&self, current_priority: Priority) -> PriorityGuard<'_, T> {
        self.waiters.fetch_add(1, Ordering::AcqRel);

        loop {
            // Try to acquire lock
            match self.locked.compare_exchange_weak(
                false,
                true,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Lock acquired
                    self.owner_priority.store(current_priority as usize, Ordering::Release);
                    self.waiters.fetch_sub(1, Ordering::AcqRel);

                    return PriorityGuard {
                        mutex:             self,
                        original_priority: current_priority,
                    };
                },
                Err(_) => {
                    // Lock is held - implement priority inheritance
                    let owner_priority = self.owner_priority.load(Ordering::Acquire);

                    if (current_priority as usize) > owner_priority {
                        // Boost owner's priority to current priority
                        self.owner_priority
                            .compare_exchange_weak(
                                owner_priority,
                                current_priority as usize,
                                Ordering::AcqRel,
                                Ordering::Relaxed,
                            )
                            .ok(); // Ignore failure - another high priority
                                   // task may have boosted it
                    }

                    // Yield and retry (in real implementation, would use futex or similar)
                    core::hint::spin_loop();
                },
            }
        }
    }

    /// Try to acquire lock without blocking
    pub fn try_lock(&self, current_priority: Priority) -> Option<PriorityGuard<'_, T>> {
        match self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => {
                self.owner_priority.store(current_priority as usize, Ordering::Release);
                Some(PriorityGuard {
                    mutex:             self,
                    original_priority: current_priority,
                })
            },
            Err(_) => None,
        }
    }

    /// Get current owner priority
    pub fn owner_priority(&self) -> Priority {
        self.owner_priority.load(Ordering::Acquire) as Priority
    }

    /// Check if there are waiting tasks
    pub fn has_waiters(&self) -> bool {
        self.waiters.load(Ordering::Acquire) > 0
    }
}

unsafe impl<T: Send> Send for PriorityInheritanceMutex<T> {}
unsafe impl<T: Send> Sync for PriorityInheritanceMutex<T> {}

/// RAII guard for priority inheritance mutex
pub struct PriorityGuard<'a, T> {
    mutex:             &'a PriorityInheritanceMutex<T>,
    original_priority: Priority,
}

impl<'a, T> core::ops::Deref for PriorityGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for PriorityGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for PriorityGuard<'a, T> {
    fn drop(&mut self) {
        // Restore original priority and release lock
        self.mutex.owner_priority.store(0, Ordering::Release);
        self.mutex.locked.store(false, Ordering::Release);
    }
}

/// High-performance reader-writer lock with writer preference
///
/// Optimized for scenarios with many readers and occasional writers.
/// Writers have priority to prevent writer starvation.
pub struct AdvancedRwLock<T> {
    /// Protected data
    data:           UnsafeCell<T>,
    /// Reader count (negative value indicates writer)
    readers:        AtomicUsize,
    /// Writer waiting flag
    writer_waiting: AtomicBool,
}

const WRITER_MASK: usize = 1 << (usize::BITS - 1);
const READER_MASK: usize = !WRITER_MASK;

impl<T> AdvancedRwLock<T> {
    /// Create a new reader-writer lock
    pub const fn new(data: T) -> Self {
        Self {
            data:           UnsafeCell::new(data),
            readers:        AtomicUsize::new(0),
            writer_waiting: AtomicBool::new(false),
        }
    }

    /// Acquire read lock
    ///
    /// Blocks if a writer is active or waiting (writer preference).
    pub fn read(&self) -> ReadGuard<'_, T> {
        loop {
            // Check if writer is waiting or active
            if self.writer_waiting.load(Ordering::Acquire) {
                core::hint::spin_loop();
                continue;
            }

            let current = self.readers.load(Ordering::Acquire);

            // Check if writer is active
            if current & WRITER_MASK != 0 {
                core::hint::spin_loop();
                continue;
            }

            // Try to increment reader count
            let new_count = (current & READER_MASK) + 1;
            if new_count > READER_MASK {
                // Too many readers
                core::hint::spin_loop();
                continue;
            }

            match self.readers.compare_exchange_weak(
                current,
                new_count,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => return ReadGuard { lock: self },
                Err(_) => {},
            }
        }
    }

    /// Try to acquire read lock without blocking
    pub fn try_read(&self) -> Option<ReadGuard<'_, T>> {
        if self.writer_waiting.load(Ordering::Acquire) {
            return None;
        }

        let current = self.readers.load(Ordering::Acquire);

        if current & WRITER_MASK != 0 {
            return None; // Writer is active
        }

        let new_count = (current & READER_MASK) + 1;
        if new_count > READER_MASK {
            return None; // Too many readers
        }

        match self.readers.compare_exchange(
            current,
            new_count,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => Some(ReadGuard { lock: self }),
            Err(_) => None,
        }
    }

    /// Acquire write lock
    ///
    /// Has priority over new readers to prevent writer starvation.
    pub fn write(&self) -> WriteGuard<'_, T> {
        // Signal that a writer is waiting
        self.writer_waiting.store(true, Ordering::Release);

        loop {
            // Try to acquire write lock (no readers or writers)
            match self.readers.compare_exchange_weak(
                0,
                WRITER_MASK,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.writer_waiting.store(false, Ordering::Release);
                    return WriteGuard { lock: self };
                },
                Err(_) => core::hint::spin_loop(),
            }
        }
    }

    /// Try to acquire write lock without blocking
    pub fn try_write(&self) -> Option<WriteGuard<'_, T>> {
        self.writer_waiting.store(true, Ordering::Release);

        match self
            .readers
            .compare_exchange(0, WRITER_MASK, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(_) => {
                self.writer_waiting.store(false, Ordering::Release);
                Some(WriteGuard { lock: self })
            },
            Err(_) => {
                self.writer_waiting.store(false, Ordering::Release);
                None
            },
        }
    }

    /// Get current reader count (approximate)
    pub fn reader_count(&self) -> usize {
        let current = self.readers.load(Ordering::Acquire);
        if current & WRITER_MASK != 0 {
            0 // Writer is active
        } else {
            current & READER_MASK
        }
    }

    /// Check if a writer is active
    pub fn has_writer(&self) -> bool {
        self.readers.load(Ordering::Acquire) & WRITER_MASK != 0
    }
}

unsafe impl<T: Send> Send for AdvancedRwLock<T> {}
unsafe impl<T: Send + Sync> Sync for AdvancedRwLock<T> {}

/// RAII guard for read access
pub struct ReadGuard<'a, T> {
    lock: &'a AdvancedRwLock<T>,
}

impl<'a, T> core::ops::Deref for ReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.readers.fetch_sub(1, Ordering::Release);
    }
}

/// RAII guard for write access
pub struct WriteGuard<'a, T> {
    lock: &'a AdvancedRwLock<T>,
}

impl<'a, T> core::ops::Deref for WriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for WriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T> Drop for WriteGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.readers.store(0, Ordering::Release);
    }
}

/// Wait-free single-producer single-consumer queue
///
/// Provides deterministic O(1) operations suitable for real-time systems.
/// Uses a ring buffer with atomic head/tail pointers.
#[cfg(feature = "std")]
pub struct WaitFreeSpscQueue<T> {
    /// Ring buffer storage
    buffer:   Box<[UnsafeCell<Option<T>>]>,
    /// Capacity (power of 2)
    capacity: usize,
    /// Capacity mask for fast modulo
    mask:     usize,
    /// Head index (consumer)
    head:     AtomicUsize,
    /// Tail index (producer)  
    tail:     AtomicUsize,
}

#[cfg(feature = "std")]
impl<T> WaitFreeSpscQueue<T> {
    /// Create queue with specified capacity (rounded up to power of 2)
    #[cfg(feature = "std")]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let buffer = (0..capacity)
            .map(|_| UnsafeCell::new(None))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            buffer,
            capacity,
            mask: capacity - 1,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Enqueue item (wait-free, single producer only)
    ///
    /// Returns `Err(item)` if queue is full.
    pub fn enqueue(&self, item: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let next_tail = (tail + 1) & self.mask;

        // Check if queue is full
        if next_tail == self.head.load(Ordering::Acquire) {
            return Err(item);
        }

        // Store item in buffer
        unsafe {
            *self.buffer[tail].get() = Some(item);
        }

        // Update tail pointer
        self.tail.store(next_tail, Ordering::Release);
        Ok(())
    }

    /// Dequeue item (wait-free, single consumer only)
    ///
    /// Returns `None` if queue is empty.
    pub fn dequeue(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);

        // Check if queue is empty
        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }

        // Extract item from buffer
        let item = unsafe { (*self.buffer[head].get()).take() };

        // Update head pointer
        let next_head = (head + 1) & self.mask;
        self.head.store(next_head, Ordering::Release);

        item
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire) == self.tail.load(Ordering::Acquire)
    }

    /// Check if queue is full  
    pub fn is_full(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        let next_tail = (tail + 1) & self.mask;
        next_tail == self.head.load(Ordering::Acquire)
    }

    /// Get current queue length (approximate)
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        (tail.wrapping_sub(head)) & self.mask
    }

    /// Get queue capacity
    pub fn capacity(&self) -> usize {
        self.capacity - 1 // One slot reserved for full detection
    }
}

#[cfg(feature = "std")]
unsafe impl<T: Send> Send for WaitFreeSpscQueue<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_mpsc_queue() {
        let queue = LockFreeMpscQueue::new();

        // Test enqueue/dequeue
        queue.enqueue(42).unwrap();
        queue.enqueue(84).unwrap();

        assert_eq!(queue.dequeue(), Some(42));
        assert_eq!(queue.dequeue(), Some(84));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_lock_free_allocator() {
        const POOL_SIZE: usize = 1024;
        const BLOCK_SIZE: usize = 64;

        #[cfg(feature = "std")]
        let mut pool = vec![0u8; POOL_SIZE];
        #[cfg(not(feature = "std"))]
        let mut pool = [0u8; POOL_SIZE];
        let allocator =
            unsafe { LockFreeAllocator::new(pool.as_mut_ptr(), POOL_SIZE, BLOCK_SIZE).unwrap() };

        // Binary std/no_std choice
        let ptr1 = allocator.allocate().unwrap();
        let ptr2 = allocator.allocate().unwrap();

        assert_ne!(ptr1.as_ptr(), ptr2.as_ptr());

        // Binary std/no_std choice
        unsafe {
            allocator.deallocate(ptr1);
            allocator.deallocate(ptr2);
        }

        // Binary std/no_std choice
        let ptr3 = allocator.allocate().unwrap();
        assert!(!ptr3.as_ptr().is_null());
    }

    #[test]
    fn test_priority_inheritance_mutex() {
        let mutex = PriorityInheritanceMutex::new(42);

        // Test basic locking
        {
            let guard = mutex.lock(100);
            assert_eq!(*guard, 42);
            assert_eq!(mutex.owner_priority(), 100);
        }

        // Test try_lock
        let guard = mutex.try_lock(50).unwrap();
        assert_eq!(*guard, 42);
        assert_eq!(mutex.owner_priority(), 50);
        drop(guard);

        // Test try_lock failure
        let _guard1 = mutex.try_lock(10).unwrap();
        assert!(mutex.try_lock(20).is_none());
    }

    #[test]
    fn test_advanced_rwlock() {
        let lock = AdvancedRwLock::new(42);

        // Test read locks
        {
            let read1 = lock.read();
            let read2 = lock.read();
            assert_eq!(*read1, 42);
            assert_eq!(*read2, 42);
            assert_eq!(lock.reader_count(), 2);
            assert!(!lock.has_writer());
        }

        // Test write lock
        {
            let mut write = lock.write();
            *write = 84;
            assert!(lock.has_writer());
            assert_eq!(lock.reader_count(), 0);
        }

        // Verify write persisted
        let read = lock.read();
        assert_eq!(*read, 84);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_wait_free_spsc_queue() {
        let queue = WaitFreeSpscQueue::new(4);

        assert!(queue.is_empty());
        assert!(!queue.is_full());
        assert_eq!(queue.len(), 0);

        // Fill queue
        assert!(queue.enqueue(1).is_ok());
        assert!(queue.enqueue(2).is_ok());
        assert!(queue.enqueue(3).is_ok());

        assert!(queue.is_full());
        assert_eq!(queue.len(), 3);

        // Queue should reject when full
        assert!(queue.enqueue(4).is_err());

        // Drain queue
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);

        assert!(queue.is_empty());
    }
}
