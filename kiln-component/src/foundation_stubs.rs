// WRT - wrt-component
// Module: Foundation Integration Stubs
// SW-REQ-ID: REQ_INTEGRATION_STUBS_001, REQ_COMPONENT_FOUNDATION_001
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Foundation stubs for component module development
// These provide the interface to the foundation module's types

use crate::prelude::Vec;

// Temporary stubs for bounded collections from Agent A's work
pub type SmallVec<T> = Vec<T>;
pub type MediumVec<T> = Vec<T>;
pub type LargeVec<T> = Vec<T>;

// Safety context stub
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsilLevel {
    QM = 0,
    AsilA = 1,
    AsilB = 2,
    AsilC = 3,
    AsilD = 4,
}

// Aliases for compatibility
pub const ASIL_A: AsilLevel = AsilLevel::AsilA;
pub const ASIL_B: AsilLevel = AsilLevel::AsilB;
pub const ASIL_C: AsilLevel = AsilLevel::AsilC;
pub const ASIL_D: AsilLevel = AsilLevel::AsilD;

#[derive(Debug, Clone)]
pub struct SafetyContext {
    pub compile_time_asil: AsilLevel,
    pub runtime_asil: Option<AsilLevel>,
}

impl SafetyContext {
    pub const fn new(compile_time: AsilLevel) -> Self {
        Self {
            compile_time_asil: compile_time,
            runtime_asil: None,
        }
    }

    pub fn effective_asil(&self) -> AsilLevel {
        self.runtime_asil.unwrap_or(self.compile_time_asil)
    }
}

// Memory provider stubs
pub trait UnifiedMemoryProvider: Send + Sync {
    fn allocate(&mut self, size: usize) -> core::result::Result<&mut [u8], wrt_error::Error>;
    fn deallocate(&mut self, ptr: &mut [u8]) -> core::result::Result<(), wrt_error::Error>;
    fn available_memory(&self) -> usize;
    fn total_memory(&self) -> usize;
}

pub struct NoStdProvider<const SIZE: usize> {
    buffer: [u8; SIZE],
    allocated: usize,
}

impl<const SIZE: usize> NoStdProvider<SIZE> {
    pub fn new() -> Self {
        Self {
            buffer: [0; SIZE],
            allocated: 0,
        }
    }
}

impl<const SIZE: usize> Default for NoStdProvider<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> UnifiedMemoryProvider for NoStdProvider<SIZE> {
    fn allocate(&mut self, size: usize) -> core::result::Result<&mut [u8], wrt_error::Error> {
        if self.allocated + size > SIZE {
            return Err(wrt_error::Error::resource_exhausted("Out of memory"));
        }
        let start = self.allocated;
        self.allocated += size;
        Ok(&mut self.buffer[start..self.allocated])
    }

    fn deallocate(&mut self, _ptr: &mut [u8]) -> core::result::Result<(), wrt_error::Error> {
        // Simple implementation - could reset if ptr is at end
        Ok(())
    }

    fn available_memory(&self) -> usize {
        SIZE - self.allocated
    }

    fn total_memory(&self) -> usize {
        SIZE
    }
}

// Error types from Agent A
pub use wrt_error::Error;

// Threading stubs for component model
/// Thread identifier type for component threading
pub type ThreadId = u32;

/// Thread execution statistics
#[derive(Debug, Clone, Default)]
pub struct ThreadExecutionStats {
    pub execution_time: u64,
    pub cycles_used: u64,
    pub memory_used: usize,
}

/// Thread state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

/// Thread manager for component model threading
#[derive(Debug)]
pub struct ThreadManager {
    thread_count: u32,
    max_threads: u32,
}

impl ThreadManager {
    pub fn new(max_threads: u32) -> Self {
        Self {
            thread_count: 0,
            max_threads,
        }
    }

    pub fn spawn_thread(&mut self) -> core::result::Result<ThreadId, Error> {
        if self.thread_count >= self.max_threads {
            return Err(Error::resource_exhausted("Maximum threads reached"));
        }

        let thread_id = self.thread_count;
        self.thread_count += 1;
        Ok(thread_id)
    }

    pub fn get_thread_stats(
        &self,
        _thread_id: ThreadId,
    ) -> core::result::Result<ThreadExecutionStats, Error> {
        Ok(ThreadExecutionStats::default())
    }

    pub fn get_thread_state(
        &self,
        _thread_id: ThreadId,
    ) -> core::result::Result<ThreadState, Error> {
        Ok(ThreadState::Ready)
    }

    pub fn terminate_thread(&mut self, _thread_id: ThreadId) -> core::result::Result<(), Error> {
        if self.thread_count > 0 {
            self.thread_count -= 1;
        }
        Ok(())
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new(64) // Default maximum of 64 threads
    }
}
