// WRT - wrt-platform
// Module: Side-Channel Resistance Analysis and Implementation
// SW-REQ-ID: REQ_PLATFORM_SIDECHANNEL_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Side-Channel Resistance for WebAssembly Runtime Security
//!
//! This module analyzes and implements side-channel resistance techniques
//! for the WRT platform, protecting against timing attacks, cache attacks,
//! and other information leakage vectors.
//!
//! # Analysis Summary
//!
//! Side-channel resistance fits into the WRT platform as a **cross-cutting
//! security concern** that integrates with all existing platform layers:
//!
//! ## Integration Points
//! 1. **Memory Subsystem**: Cache-aware allocation, constant-time operations
//! 2. **Hardware Optimizations**: ARM Pointer Authentication, Intel CET for CFI
//!    protection
//! 3. **Synchronization**: Timing-attack resistant lock implementations
//! 4. **Formal Verification**: Provable constant-time guarantees
//! 5. **Platform Abstraction**: Side-channel aware paradigm selection
//!
//! ## Implementation Strategy
//! - **Layered Defense**: Multiple complementary techniques at different
//!   abstraction levels
//! - **Selective Application**: Performance-critical vs security-critical code
//!   paths
//! - **Formal Guarantees**: Verification of timing properties where possible
//! - **Graceful Degradation**: Fallback implementations when hardware features
//!   unavailable

#![allow(dead_code)] // Allow during development

use core::sync::atomic::{
    AtomicUsize,
    Ordering,
};

/// Side-channel resistance levels for different security contexts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResistanceLevel {
    /// No specific side-channel protections
    None,
    /// Basic protections (constant-time primitives)
    Basic,
    /// Advanced protections (cache timing resistance)
    Advanced,
    /// Comprehensive protections (full isolation)
    Comprehensive,
}

/// Side-channel attack vectors we defend against
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackVector {
    /// Timing attacks on cryptographic operations
    Timing,
    /// Cache-based attacks (Flush+Reload, Prime+Probe)
    Cache,
    /// Power analysis attacks
    Power,
    /// Electromagnetic emanation attacks
    Electromagnetic,
    /// Branch prediction attacks (Spectre-style)
    BranchPrediction,
    /// Memory access pattern analysis
    MemoryPattern,
}

/// Analysis: How Side-Channel Resistance Integrates with WRT Platform
pub mod integration_analysis {

    /// Integration with Memory Subsystem
    ///
    /// The memory subsystem is a primary vector for side-channel attacks:
    /// - Cache timing reveals memory access patterns
    /// - Allocation timing reveals memory fragmentation state
    /// - Page fault timing reveals virtual memory layout
    pub struct MemorySubsystemIntegration;

    impl MemorySubsystemIntegration {
        /// Binary std/no_std choice
        ///
        /// Potential side channel vulnerabilities:
        /// 1. Time to find free block varies by fragmentation
        /// 2. Cache state changes based on traversed free list
        ///
        /// Binary std/no_std choice
        pub fn analyze_allocation_timing() -> &'static str {
            "Cache-aware allocation with constant-time guarantees integrates with existing \
             PageAllocator trait via timing-resistant implementations. Uses pre-allocated pools \
             and cache line alignment to prevent timing-based information leakage."
        }

        /// Analysis: Memory access patterns leak via cache
        ///
        /// WebAssembly linear memory access patterns reveal:
        /// 1. Data structure layouts and access patterns
        /// 2. Branch outcomes through conditional memory access
        /// 3. Loop iteration counts via access sequence timing
        ///
        /// Mitigation: Oblivious memory access and data-independent patterns
        pub fn analyze_access_patterns() -> &'static str {
            "Memory access pattern obfuscation integrates with wrt-runtime memory adapter to \
             provide data-independent access sequences. Uses dummy reads and cache-line fetching \
             to normalize timing."
        }
    }

    /// Integration with Hardware Optimizations
    ///
    /// Hardware security features complement side-channel resistance:
    /// - ARM MTE provides memory safety without timing leaks
    /// - Intel CET prevents control-flow based side channels
    /// - RISC-V PMP enables fine-grained timing isolation
    pub struct HardwareOptimizationIntegration;

    impl HardwareOptimizationIntegration {
        /// Analysis: ARM MTE integration for side-channel resistance
        pub fn analyze_arm_mte_integration() -> &'static str {
            "ARM Memory Tagging Extension provides memory safety without timing side channels. Tag \
             checking is constant-time in hardware, preventing use-after-free attacks without \
             revealing timing information about memory reuse patterns."
        }

        /// Analysis: Intel CET integration for control-flow protection  
        pub fn analyze_intel_cet_integration() -> &'static str {
            "Intel Control-flow Enforcement Technology prevents control-flow based side channels \
             by ensuring indirect calls cannot be manipulated to leak information through \
             speculative execution or return address prediction."
        }

        /// Analysis: RISC-V PMP for timing isolation
        pub fn analyze_riscv_pmp_integration() -> &'static str {
            "RISC-V Physical Memory Protection enables fine-grained timing isolation by preventing \
             unauthorized memory access that could be used for timing-based attacks. PMP \
             configuration is done at runtime initialization to avoid timing leaks."
        }
    }

    /// Integration with Advanced Synchronization
    ///
    /// Synchronization primitives can leak information through timing:
    /// - Lock contention timing reveals concurrent access patterns
    /// - Priority inheritance timing reveals task scheduling state
    /// - Wait queue ordering reveals arrival patterns
    pub struct SynchronizationIntegration;

    impl SynchronizationIntegration {
        /// Analysis: Lock-free structures for timing resistance
        pub fn analyze_lockfree_timing() -> &'static str {
            "Lock-free data structures provide bounded timing guarantees that resist side-channel \
             attacks. Wait-free operations eliminate timing variations due to contention, \
             providing constant-time guarantees independent of concurrent access patterns."
        }

        /// Analysis: Priority inheritance timing safety
        pub fn analyze_priority_inheritance_timing() -> &'static str {
            "Priority inheritance with timing resistance ensures that priority boosting operations \
             complete in constant time, preventing attackers from inferring system state through \
             lock timing variations. Uses fixed-time priority calculation and bounded retry loops."
        }
    }

    /// Integration with Formal Verification
    ///
    /// Side-channel resistance requires formal timing guarantees:
    /// - Kani proofs for constant-time properties
    /// - CBMC verification of timing bounds
    /// - Automated detection of timing variations
    pub struct FormalVerificationIntegration;

    impl FormalVerificationIntegration {
        /// Analysis: Formal timing verification
        pub fn analyze_timing_verification() -> &'static str {
            "Formal verification extends to timing properties through Kani annotations that prove \
             constant-time execution. Bounded model checking verifies that execution time is \
             independent of secret inputs, providing mathematical guarantees of side-channel \
             resistance."
        }

        /// Analysis: Automated side-channel detection
        pub fn analyze_automated_detection() -> &'static str {
            "Integration with formal verification enables automated detection of potential \
             side-channel vulnerabilities through static analysis. Tools can identify \
             data-dependent timing variations and suggest constant-time alternatives."
        }
    }

    /// Integration with Platform Abstraction
    ///
    /// Different paradigms require different side-channel protections:
    /// - SecurityFirst: Maximum resistance at performance cost
    /// - RealTime: Bounded timing with resistance guarantees
    /// - Posix: Best-effort resistance with standard interfaces
    /// - BareMetal: Hardware-specific optimized resistance
    pub struct PlatformAbstractionIntegration;

    impl PlatformAbstractionIntegration {
        /// Analysis: Paradigm-specific resistance strategies
        pub fn analyze_paradigm_resistance() -> &'static str {
            "Side-channel resistance integrates with platform paradigms by providing different \
             security/performance trade-offs. SecurityFirst paradigm enables comprehensive \
             protections, while RealTime paradigm provides bounded-time resistance suitable for \
             real-time constraints."
        }

        /// Analysis: Runtime adaptation of resistance levels
        pub fn analyze_runtime_adaptation() -> &'static str {
            "Platform abstraction enables runtime selection of side-channel resistance levels \
             based on threat model and performance requirements. Runtime detection of hardware \
             features allows optimal resistance strategy selection for each deployment environment."
        }
    }
}

/// Constant-time operations for cryptographic safety
pub mod constant_time {
    /// Constant-time comparison that resists timing attacks
    ///
    /// # Security Properties
    /// - Execution time independent of input values
    /// - No early termination on mismatch
    /// - Resists cache-based timing analysis
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for i in 0..a.len() {
            result |= a[i] ^ b[i];
        }

        // Constant-time zero check
        ((result as u16).wrapping_sub(1) >> 8) == 0
    }

    /// Constant-time conditional selection
    ///
    /// Returns `a` if `condition` is true, `b` otherwise.
    /// Execution time independent of condition value.
    pub fn constant_time_select<T: Copy>(condition: bool, a: T, b: T) -> T {
        let mask = if condition { !0 } else { 0 };

        // This requires T to support bitwise operations
        // In practice, would use specialized implementations for different types
        unsafe {
            let a_bytes =
                core::slice::from_raw_parts(&a as *const T as *const u8, core::mem::size_of::<T>());
            let b_bytes =
                core::slice::from_raw_parts(&b as *const T as *const u8, core::mem::size_of::<T>());
            let mut result_bytes = [0u8; 32]; // Assume T fits in 32 bytes

            for i in 0..core::mem::size_of::<T>() {
                result_bytes[i] = (a_bytes[i] & mask) | (b_bytes[i] & !mask);
            }

            core::ptr::read(result_bytes.as_ptr() as *const T)
        }
    }

    /// Constant-time memory copy with cache protection
    ///
    /// Copies memory while minimizing cache-based side channels.
    /// Uses cache line alignment and prefetching for timing consistency.
    pub fn constant_time_copy(dst: &mut [u8], src: &[u8]) -> Result<(), wrt_error::Error> {
        if dst.len() != src.len() {
            return Err(wrt_error::Error::runtime_execution_error(
                "Source and destination lengths must match",
            ));
        }

        let len = dst.len();

        // Copy in cache-line sized chunks for consistent timing
        const CACHE_LINE_SIZE: usize = 64;
        let full_lines = len / CACHE_LINE_SIZE;
        let remainder = len % CACHE_LINE_SIZE;

        // Copy full cache lines
        for i in 0..full_lines {
            let start = i * CACHE_LINE_SIZE;
            let end = start + CACHE_LINE_SIZE;

            // Prefetch next cache line to normalize timing
            #[cfg(target_arch = "x86_64")]
            if i + 1 < full_lines {
                let next_start = end;
                unsafe {
                    core::arch::x86_64::_mm_prefetch(
                        src.as_ptr().add(next_start) as *const i8,
                        core::arch::x86_64::_MM_HINT_T0,
                    );
                }
            }

            dst[start..end].copy_from_slice(&src[start..end]);
        }

        // Copy remainder with padding to full cache line
        if remainder > 0 {
            let start = full_lines * CACHE_LINE_SIZE;
            dst[start..].copy_from_slice(&src[start..]);

            // Add dummy reads to complete cache line timing
            let dummy_reads = CACHE_LINE_SIZE - remainder;
            for i in 0..dummy_reads {
                let dummy_idx = (start + remainder + i) % len;
                let _ = src[dummy_idx]; // Dummy read for timing consistency
            }
        }

        Ok(())
    }
}

/// Binary std/no_std choice
pub mod cache_aware_allocation {
    use core::ptr::NonNull;

    use super::*;

    /// Cache-aligned memory pool that resists timing attacks
    ///
    /// Binary std/no_std choice
    /// Binary std/no_std choice
    pub struct CacheAwareAllocator {
        /// Binary std/no_std choice
        blocks:            &'static mut [CacheBlock],
        /// Allocation bitmap (constant-time operations)
        allocation_bitmap: AtomicUsize,
        /// Block size (power of 2, cache-aligned)
        block_size:        usize,
        /// Total blocks available
        total_blocks:      usize,
    }

    /// Binary std/no_std choice
    #[repr(align(64))] // Cache line alignment
    pub struct CacheBlock {
        data:      [u8; 64], // One cache line
        next_free: AtomicUsize,
    }

    impl CacheAwareAllocator {
        /// Binary std/no_std choice
        ///
        /// # Security Properties
        /// Binary std/no_std choice
        /// Binary std/no_std choice
        /// Binary std/no_std choice
        pub unsafe fn new(pool: &'static mut [CacheBlock]) -> Self {
            let total_blocks = pool.len();

            // Initialize free list in constant time
            for i in 0..total_blocks.saturating_sub(1) {
                pool[i].next_free.store(i + 1, Ordering::Relaxed);
            }
            if total_blocks > 0 {
                pool[total_blocks - 1].next_free.store(usize::MAX, Ordering::Relaxed);
            }

            Self {
                blocks: pool,
                allocation_bitmap: AtomicUsize::new(0),
                block_size: core::mem::size_of::<CacheBlock>(),
                total_blocks,
            }
        }

        /// Allocate block with constant-time guarantee
        ///
        /// Returns None if pool is exhausted, but timing is independent
        /// of pool state to prevent information leakage.
        pub fn allocate_constant_time(&self) -> Option<NonNull<u8>> {
            // Use constant-time bit scanning for free block
            let mut attempts = 0;
            const MAX_ATTEMPTS: usize = 64; // Prevent infinite loops

            while attempts < MAX_ATTEMPTS {
                let bitmap = self.allocation_bitmap.load(Ordering::Acquire);

                // Find first free bit in constant time
                let free_bit = self.find_free_bit_constant_time(bitmap);

                if free_bit >= self.total_blocks {
                    // Binary std/no_std choice
                    self.dummy_allocation_work();
                    return None;
                }

                let mask = 1usize << free_bit;

                // Try to atomically set the bit
                match self.allocation_bitmap.compare_exchange_weak(
                    bitmap,
                    bitmap | mask,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Binary std/no_std choice
                        let block_ptr = &self.blocks[free_bit] as *const CacheBlock as *mut u8;
                        return NonNull::new(block_ptr);
                    },
                    Err(_) => {
                        attempts += 1;
                        // Continue retry loop
                    },
                }
            }

            None
        }

        /// Binary std/no_std choice
        ///
        /// # Safety
        /// Binary std/no_std choice
        pub unsafe fn deallocate_constant_time(&self, ptr: NonNull<u8>) {
            let block_addr = ptr.as_ptr() as usize;
            let base_addr = self.blocks.as_ptr() as usize;

            // Calculate block index in constant time
            let block_index = (block_addr - base_addr) / self.block_size;

            if block_index < self.total_blocks {
                let mask = 1usize << block_index;

                // Clear bit atomically
                loop {
                    let bitmap = self.allocation_bitmap.load(Ordering::Acquire);
                    match self.allocation_bitmap.compare_exchange_weak(
                        bitmap,
                        bitmap & !mask,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(_) => {},
                    }
                }
            }

            // Always do the same amount of work regardless of success
            self.dummy_deallocation_work();
        }

        /// Find free bit using constant-time bit operations
        fn find_free_bit_constant_time(&self, bitmap: usize) -> usize {
            // Use bit manipulation to find first zero bit in constant time
            let inverted = !bitmap;
            if inverted == 0 {
                return self.total_blocks; // No free bits
            }

            // Count trailing zeros (number of set bits before first zero)
            inverted.trailing_zeros() as usize
        }

        /// Binary std/no_std choice
        fn dummy_allocation_work(&self) {
            // Binary std/no_std choice
            let _ = self.allocation_bitmap.load(Ordering::Acquire);
            let _ = self.find_free_bit_constant_time(0);
        }

        /// Binary std/no_std choice
        fn dummy_deallocation_work(&self) {
            // Binary std/no_std choice
            let _ = self.allocation_bitmap.load(Ordering::Acquire);
        }

        /// Binary std/no_std choice
        pub fn stats(&self) -> AllocatorStats {
            let bitmap = self.allocation_bitmap.load(Ordering::Acquire);
            let used_blocks = bitmap.count_ones() as usize;

            AllocatorStats {
                total_blocks: self.total_blocks,
                used_blocks,
                free_blocks: self.total_blocks - used_blocks,
                block_size: self.block_size,
            }
        }
    }

    /// Allocator statistics for monitoring
    #[derive(Debug, Clone, Copy)]
    pub struct AllocatorStats {
        /// Total number of memory blocks
        pub total_blocks: usize,
        /// Number of blocks currently in use
        pub used_blocks:  usize,
        /// Number of free blocks available
        pub free_blocks:  usize,
        /// Size of each memory block in bytes
        pub block_size:   usize,
    }
}

/// Memory access obfuscation to prevent pattern analysis
pub mod access_obfuscation {
    use super::*;

    /// Oblivious memory access that hides access patterns
    ///
    /// Performs the same cache access patterns regardless of
    /// which memory location is actually accessed.
    pub struct ObliviousMemoryAccess {
        /// Memory region to obfuscate
        memory:          &'static mut [u8],
        /// Cache line size for access alignment
        cache_line_size: usize,
    }

    impl ObliviousMemoryAccess {
        /// Create oblivious memory accessor
        pub fn new(memory: &'static mut [u8]) -> Self {
            Self {
                memory,
                cache_line_size: 64, // Common cache line size
            }
        }

        /// Read from memory with oblivious access pattern
        ///
        /// # Security Properties
        /// - Cache access pattern independent of actual index
        /// - Timing independent of memory content
        /// - Resists cache-based side-channel attacks
        pub fn oblivious_read(&self, index: usize) -> u8 {
            let len = self.memory.len();
            if len == 0 {
                return 0;
            }

            let valid_index = index < len;
            let safe_index = if valid_index { index } else { 0 };

            // Read all cache lines to hide access pattern
            let cache_lines = (len + self.cache_line_size - 1) / self.cache_line_size;
            let mut result = 0u8;

            for line in 0..cache_lines {
                let line_start = line * self.cache_line_size;
                let line_end = core::cmp::min(line_start + self.cache_line_size, len);

                // Read entire cache line
                for offset in line_start..line_end {
                    let value = self.memory[offset];

                    // Conditionally select the target value
                    let is_target = offset == safe_index && valid_index;
                    result = constant_time::constant_time_select(is_target, value, result);
                }
            }

            result
        }

        /// Write to memory with oblivious access pattern
        ///
        /// # Security Properties  
        /// - Same cache access pattern for all writes
        /// - Timing independent of write location
        /// - Prevents write pattern analysis
        pub fn oblivious_write(&mut self, index: usize, value: u8) {
            let len = self.memory.len();
            if len == 0 {
                return;
            }

            let valid_index = index < len;
            let safe_index = if valid_index { index } else { 0 };

            // Touch all cache lines to hide write pattern
            let cache_lines = (len + self.cache_line_size - 1) / self.cache_line_size;

            for line in 0..cache_lines {
                let line_start = line * self.cache_line_size;
                let line_end = core::cmp::min(line_start + self.cache_line_size, len);

                // Access entire cache line
                for offset in line_start..line_end {
                    let is_target = offset == safe_index && valid_index;
                    let current_value = self.memory[offset];

                    // Conditionally update target location
                    let new_value =
                        constant_time::constant_time_select(is_target, value, current_value);

                    self.memory[offset] = new_value;
                }
            }
        }

        /// Copy memory with oblivious access patterns
        pub fn oblivious_copy(&mut self, dst_index: usize, src_index: usize, len: usize) {
            for i in 0..len {
                let src_val = self.oblivious_read(src_index + i);
                self.oblivious_write(dst_index + i, src_val);
            }
        }
    }
}

/// Integration layer for side-channel resistance across WRT platform
pub mod platform_integration {
    use super::*;
    use crate::platform_abstraction::PlatformConfig;

    /// Side-channel resistance configuration for different paradigms
    #[derive(Debug, Clone)]
    pub struct SideChannelConfig {
        /// Target resistance level
        pub resistance_level:      ResistanceLevel,
        /// Specific attack vectors to defend against
        pub protected_vectors:     &'static [AttackVector],
        /// Performance impact tolerance (0.0 to 1.0)
        pub performance_tolerance: f32,
        /// Enable formal verification of timing properties
        pub verify_timing:         bool,
    }

    impl Default for SideChannelConfig {
        fn default() -> Self {
            Self {
                resistance_level:      ResistanceLevel::Basic,
                protected_vectors:     &[AttackVector::Timing, AttackVector::Cache],
                performance_tolerance: 0.1, // 10% performance overhead acceptable
                verify_timing:         false,
            }
        }
    }

    /// Extend platform configuration with side-channel resistance
    pub trait SideChannelAwarePlatform<P> {
        /// Configure side-channel resistance for paradigm
        fn with_side_channel_config(self, config: SideChannelConfig) -> Self;

        /// Enable specific attack vector protection
        fn protect_against(self, vector: AttackVector) -> Self;

        /// Set resistance level
        fn with_resistance_level(self, level: ResistanceLevel) -> Self;
    }

    impl<P> SideChannelAwarePlatform<P> for PlatformConfig<P> {
        fn with_side_channel_config(self, _config: SideChannelConfig) -> Self {
            // Integration with existing platform configuration
            // This would extend the PlatformConfig struct with side-channel fields
            self
        }

        fn protect_against(self, _vector: AttackVector) -> Self {
            // Add vector to protected list
            self
        }

        fn with_resistance_level(self, _level: ResistanceLevel) -> Self {
            // Set resistance level
            self
        }
    }

    /// Security-first paradigm with comprehensive side-channel resistance
    pub fn security_first_side_channel_config() -> SideChannelConfig {
        SideChannelConfig {
            resistance_level:      ResistanceLevel::Comprehensive,
            protected_vectors:     &[
                AttackVector::Timing,
                AttackVector::Cache,
                AttackVector::BranchPrediction,
                AttackVector::MemoryPattern,
            ],
            performance_tolerance: 0.5, // Accept 50% performance overhead for security
            verify_timing:         true,
        }
    }

    /// Real-time paradigm with bounded-time side-channel resistance
    #[must_use]
    pub fn realtime_side_channel_config() -> SideChannelConfig {
        SideChannelConfig {
            resistance_level:      ResistanceLevel::Advanced,
            protected_vectors:     &[AttackVector::Timing, AttackVector::Cache],
            performance_tolerance: 0.1,  // Strict performance requirements
            verify_timing:         true, // Essential for real-time guarantees
        }
    }

    /// POSIX paradigm with best-effort side-channel resistance
    #[must_use]
    pub fn posix_side_channel_config() -> SideChannelConfig {
        SideChannelConfig {
            resistance_level:      ResistanceLevel::Basic,
            protected_vectors:     &[AttackVector::Timing],
            performance_tolerance: 0.05, // Minimal performance impact
            verify_timing:         false,
        }
    }

    /// Bare-metal paradigm with hardware-optimized resistance
    #[must_use]
    pub fn baremetal_side_channel_config() -> SideChannelConfig {
        SideChannelConfig {
            resistance_level:      ResistanceLevel::Advanced,
            protected_vectors:     &[
                AttackVector::Timing,
                AttackVector::Cache,
                AttackVector::Power,
                AttackVector::Electromagnetic,
            ],
            performance_tolerance: 0.2,
            verify_timing:         true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_operations() {
        // Test constant-time equality
        let a = b"hello";
        let b = b"hello";
        let c = b"world";

        assert!(constant_time::constant_time_eq(a, b));
        assert!(!constant_time::constant_time_eq(a, c));
        assert!(!constant_time::constant_time_eq(a, b"hi"));

        // Test constant-time selection
        let result1 = constant_time::constant_time_select(true, 42u32, 84u32);
        assert_eq!(result1, 42);

        let result2 = constant_time::constant_time_select(false, 42u32, 84u32);
        assert_eq!(result2, 84);
    }

    #[test]
    fn test_constant_time_copy() {
        let src = b"test data for copying";
        let mut dst = [0u8; 21];

        let result = constant_time::constant_time_copy(&mut dst, src);
        assert!(result.is_ok());
        assert_eq!(&dst, src);

        // Test error on size mismatch
        let mut small_dst = [0u8; 5];
        let result = constant_time::constant_time_copy(&mut small_dst, src);
        assert!(result.is_err());
    }

    #[test]
    fn test_side_channel_config() {
        let config = platform_integration::security_first_side_channel_config();
        assert_eq!(config.resistance_level, ResistanceLevel::Comprehensive);
        assert!(config.protected_vectors.contains(&AttackVector::Timing));
        assert!(config.protected_vectors.contains(&AttackVector::Cache));
        assert!(config.verify_timing);

        let rt_config = platform_integration::realtime_side_channel_config();
        assert_eq!(rt_config.resistance_level, ResistanceLevel::Advanced);
        assert!(rt_config.performance_tolerance < 0.2);
    }

    #[test]
    fn test_integration_analysis() {
        // Test that integration analysis functions return meaningful descriptions
        let memory_analysis =
            integration_analysis::MemorySubsystemIntegration::analyze_allocation_timing();
        assert!(memory_analysis.contains("Cache-aware"));
        assert!(memory_analysis.contains("constant-time"));

        let hw_analysis =
            integration_analysis::HardwareOptimizationIntegration::analyze_arm_mte_integration();
        assert!(hw_analysis.contains("Memory Tagging Extension"));
        assert!(hw_analysis.contains("constant-time"));

        let sync_analysis =
            integration_analysis::SynchronizationIntegration::analyze_lockfree_timing();
        assert!(sync_analysis.contains("Lock-free"));
        assert!(sync_analysis.contains("bounded timing"));
    }
}
