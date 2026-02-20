// WRT - wrt-runtime
// Module: Platform Abstraction Interface (PAI)
// SW-REQ-ID: REQ_PLATFORM_ABSTRACTION_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Platform Abstraction Interface (PAI) for ASIL-compliant runtime
//! 
//! This module provides a clean abstraction layer between the runtime
//! and platform-specific implementations. It ensures deterministic
//! compilation and proper feature gating for safety-critical systems.
//!
//! # Design Principles
//! - Zero-cost abstractions through compile-time dispatch
//! - Clear separation between platform-dependent and independent code
//! - Deterministic feature combinations for ASIL compliance
//! - No implicit dependencies on platform features

#![forbid(unsafe_code)] // Platform abstractions must be safe

use core::marker::PhantomData;
use wrt_error::{Error, ErrorCategory, Result};

// Box for trait objects
#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;

// Re-export ASIL levels from foundation stubs temporarily
use super::foundation_stubs::AsilLevel;

// Platform-agnostic atomic types
#[cfg(not(feature = "platform-sync"))]
mod atomic_fallback {
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicBool, Ordering};
    
    /// Fallback atomic u32 for platforms without native atomics
    pub struct AtomicU32 {
        lock: AtomicBool,
        value: UnsafeCell<u32>,
    }
    
    unsafe impl Sync for AtomicU32 {}
    unsafe impl Send for AtomicU32 {}
    
    impl AtomicU32 {
        pub const fn new(value: u32) -> Self {
            Self {
                lock: AtomicBool::new(false),
                value: UnsafeCell::new(value),
            }
        }
        
        pub fn load(&self, _ordering: Ordering) -> u32 {
            // Spin lock for safety
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            let value = unsafe { *self.value.get() };
            self.lock.store(false, Ordering::Release);
            value
        }
        
        pub fn store(&self, value: u32, _ordering: Ordering) {
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            unsafe { *self.value.get() = value; }
            self.lock.store(false, Ordering::Release);
        }
        
        pub fn compare_exchange(
            &self,
            current: u32,
            new: u32,
            _success: Ordering,
            _failure: Ordering,
        ) -> core::result::Result<u32, u32> {
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            let old_value = unsafe { *self.value.get() };
            if old_value == current {
                unsafe { *self.value.get() = new; }
                self.lock.store(false, Ordering::Release);
                Ok(old_value)
            } else {
                self.lock.store(false, Ordering::Release);
                Err(old_value)
            }
        }
        
        pub fn fetch_add(&self, value: u32, _ordering: Ordering) -> u32 {
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            let old_value = unsafe { *self.value.get() };
            unsafe { *self.value.get() = old_value.wrapping_add(value); }
            self.lock.store(false, Ordering::Release);
            old_value
        }
    }
    
    /// Fallback atomic u64 for platforms without native atomics
    pub struct AtomicU64 {
        lock: AtomicBool,
        value: UnsafeCell<u64>,
    }
    
    unsafe impl Sync for AtomicU64 {}
    unsafe impl Send for AtomicU64 {}
    
    impl AtomicU64 {
        pub const fn new(value: u64) -> Self {
            Self {
                lock: AtomicBool::new(false),
                value: UnsafeCell::new(value),
            }
        }
        
        pub fn load(&self, _ordering: Ordering) -> u64 {
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            let value = unsafe { *self.value.get() };
            self.lock.store(false, Ordering::Release);
            value
        }
        
        pub fn store(&self, value: u64, _ordering: Ordering) {
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            unsafe { *self.value.get() = value; }
            self.lock.store(false, Ordering::Release);
        }
        
        pub fn fetch_add(&self, value: u64, _ordering: Ordering) -> u64 {
            while self.lock.compare_exchange_weak(
                false, true, Ordering::Acquire, Ordering::Relaxed
            ).is_err() {
                core::hint::spin_loop();
            }
            let old_value = unsafe { *self.value.get() };
            unsafe { *self.value.get() = old_value.wrapping_add(value); }
            self.lock.store(false, Ordering::Release);
            old_value
        }
    }
    
    /// Fallback atomic usize
    pub type AtomicUsize = core::sync::atomic::AtomicUsize;
    
    /// Re-export ordering
    // Ordering is already imported above
}

// Platform abstraction layer
#[cfg(feature = "platform-sync")]
pub use wrt_platform::sync::{AtomicU32, AtomicU64, AtomicUsize, Ordering as PlatformOrdering};

#[cfg(not(feature = "platform-sync"))]
pub use self::atomic_fallback::{AtomicU32, AtomicU64, AtomicUsize, Ordering as PlatformOrdering};

// Duration abstraction
#[cfg(feature = "std")]
pub use std::time::Duration;

#[cfg(all(not(feature = "std"), feature = "platform-sync"))]
pub use wrt_platform::sync::Duration;

#[cfg(all(not(feature = "std"), not(feature = "platform-sync")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duration {
    secs: u64,
    nanos: u32,
}

#[cfg(all(not(feature = "std"), not(feature = "platform-sync")))]
impl Duration {
    pub const fn from_millis(millis: u64) -> Self {
        Self {
            secs: millis / 1000,
            nanos: ((millis % 1000) * 1_000_000) as u32,
        }
    }
    
    pub const fn as_millis(&self) -> u128 {
        self.secs as u128 * 1000 + self.nanos as u128 / 1_000_000
    }
}

/// Platform identification enum with ASIL mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformId {
    /// Linux - General purpose OS (QM)
    Linux,
    /// QNX - Safety-critical RTOS (ASIL-B)
    QNX,
    /// Embedded - Bare metal or RTOS (ASIL-D)
    Embedded,
    /// macOS - Development platform (QM)
    MacOS,
    /// Windows - Development platform (QM)
    Windows,
    /// Unknown platform (QM with restrictions)
    Unknown,
}

impl PlatformId {
    /// Get the default ASIL level for this platform
    pub const fn default_asil_level(&self) -> AsilLevel {
        match self {
            PlatformId::Linux => AsilLevel::QM,
            PlatformId::QNX => AsilLevel::B,
            PlatformId::Embedded => AsilLevel::D,
            PlatformId::MacOS => AsilLevel::QM,
            PlatformId::Windows => AsilLevel::QM,
            PlatformId::Unknown => AsilLevel::QM,
        }
    }
    
    /// Check if platform supports dynamic allocation
    pub const fn supports_dynamic_allocation(&self) -> bool {
        match self {
            PlatformId::Linux | PlatformId::MacOS | PlatformId::Windows => true,
            PlatformId::QNX => true, // Limited support
            PlatformId::Embedded | PlatformId::Unknown => false,
        }
    }
}

impl Default for PlatformId {
    fn default() -> Self {
        #[cfg(target_os = "linux")]
        return PlatformId::Linux;
        #[cfg(target_os = "macos")]
        return PlatformId::MacOS;
        #[cfg(target_os = "windows")]
        return PlatformId::Windows;
        #[cfg(target_os = "nto")]
        return PlatformId::QNX;
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows", target_os = "nto")))]
        return PlatformId::Embedded;
    }
}

/// Comprehensive platform limits stub
#[derive(Debug, Clone)]
pub struct ComprehensivePlatformLimits {
    pub platform_id: PlatformId,
    pub max_total_memory: usize,
    pub max_wasm_linear_memory: usize,
    pub max_stack_bytes: usize,
    pub max_components: usize,
    pub max_debug_overhead: usize,
    pub asil_level: AsilLevel,
}

impl Default for ComprehensivePlatformLimits {
    fn default() -> Self {
        Self {
            platform_id: PlatformId::default(),
            max_total_memory: 1024 * 1024 * 1024, // 1GB
            max_wasm_linear_memory: 256 * 1024 * 1024, // 256MB
            max_stack_bytes: 8 * 1024 * 1024, // 8MB
            max_components: 256,
            max_debug_overhead: 64 * 1024 * 1024, // 64MB
            asil_level: AsilLevel::QM,
        }
    }
}

impl ComprehensivePlatformLimits {
    /// Create new limits with specified memory
    pub fn with_memory(max_memory: usize) -> Self {
        Self {
            max_total_memory: max_memory,
            max_wasm_linear_memory: max_memory / 4,
            max_stack_bytes: max_memory / 128,
            ..Default::default()
        }
    }
    
    /// Create minimal limits for embedded systems
    pub fn minimal() -> Self {
        Self {
            platform_id: PlatformId::Embedded,
            max_total_memory: 256 * 1024, // 256KB
            max_wasm_linear_memory: 64 * 1024, // 64KB
            max_stack_bytes: 16 * 1024, // 16KB
            max_components: 8,
            max_debug_overhead: 0,
            asil_level: AsilLevel::D,
        }
    }
}

/// Platform limit provider trait stub
pub trait ComprehensiveLimitProvider: Send + Sync {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, wrt_error::Error>;
    fn platform_id(&self) -> PlatformId;
}

/// Default platform limit provider
#[derive(Default)]
pub struct DefaultLimitProvider;

impl ComprehensiveLimitProvider for DefaultLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, wrt_error::Error> {
        Ok(ComprehensivePlatformLimits::default())
    }
    
    fn platform_id(&self) -> PlatformId {
        PlatformId::default()
    }
}

/// Linux limit provider stub
pub struct LinuxLimitProvider;

impl ComprehensiveLimitProvider for LinuxLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, wrt_error::Error> {
        Ok(ComprehensivePlatformLimits {
            platform_id: PlatformId::Linux,
            max_total_memory: 8 * 1024 * 1024 * 1024, // 8GB
            max_wasm_linear_memory: 2 * 1024 * 1024 * 1024, // 2GB
            max_stack_bytes: 32 * 1024 * 1024, // 32MB
            max_components: 1024,
            max_debug_overhead: 512 * 1024 * 1024, // 512MB
            asil_level: AsilLevel::QM,
        })
    }
    
    fn platform_id(&self) -> PlatformId {
        PlatformId::Linux
    }
}

/// QNX limit provider stub
pub struct QnxLimitProvider;

impl ComprehensiveLimitProvider for QnxLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, wrt_error::Error> {
        Ok(ComprehensivePlatformLimits {
            platform_id: PlatformId::QNX,
            max_total_memory: 512 * 1024 * 1024, // 512MB
            max_wasm_linear_memory: 128 * 1024 * 1024, // 128MB
            max_stack_bytes: 4 * 1024 * 1024, // 4MB
            max_components: 64,
            max_debug_overhead: 16 * 1024 * 1024, // 16MB
            asil_level: AsilLevel::B,
        })
    }
    
    fn platform_id(&self) -> PlatformId {
        PlatformId::QNX
    }
}

/// Platform abstraction interface for runtime
pub trait PlatformInterface: Send + Sync {
    /// Get platform limits
    fn limits(&self) -> &ComprehensivePlatformLimits;
    
    /// Check if a feature is supported
    fn supports_feature(&self, feature: PlatformFeature) -> bool;
    
    /// Get platform-specific configuration
    fn get_config<T: PlatformConfig>(&self) -> Option<&T>;
}

/// Platform features that may or may not be available
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFeature {
    /// Native atomic operations
    NativeAtomics,
    /// Memory-mapped files
    MemoryMappedFiles,
    /// Guard pages for memory protection
    GuardPages,
    /// Hardware memory tagging
    MemoryTagging,
    /// Real-time scheduling
    RealtimeScheduling,
    /// Formal verification support
    FormalVerification,
}

/// Marker trait for platform-specific configurations
pub trait PlatformConfig: Send + Sync + 'static {}

/// Create platform abstraction based on compile-time features
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn create_platform_interface() -> Result<Box<dyn PlatformInterface>> {
    #[cfg(feature = "platform-sync")]
    {
        // Use full platform abstraction from wrt-platform
        create_full_platform_interface()
    }
    
    #[cfg(not(feature = "platform-sync"))]
    {
        // Use minimal fallback implementation
        Ok(Box::new(MinimalPlatform::new()))
    }
}

/// Create platform interface for pure no_std environments
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub fn create_platform_interface() -> Result<&'static dyn PlatformInterface> {
    // In pure no_std, return a static reference
    static MINIMAL_PLATFORM: MinimalPlatform = MinimalPlatform {
        limits: ComprehensivePlatformLimits {
            platform_id: PlatformId::Embedded,
            max_total_memory: 256 * 1024,
            max_wasm_linear_memory: 64 * 1024,
            max_stack_bytes: 16 * 1024,
            max_components: 8,
            max_debug_overhead: 0,
            asil_level: AsilLevel::D,
        },
    };
    Ok(&MINIMAL_PLATFORM)
}

/// Minimal platform implementation for no_std without platform features
struct MinimalPlatform {
    limits: ComprehensivePlatformLimits,
}

impl MinimalPlatform {
    fn new() -> Self {
        Self {
            limits: ComprehensivePlatformLimits::minimal(),
        }
    }
}

impl PlatformInterface for MinimalPlatform {
    fn limits(&self) -> &ComprehensivePlatformLimits {
        &self.limits
    }
    
    fn supports_feature(&self, feature: PlatformFeature) -> bool {
        match feature {
            PlatformFeature::NativeAtomics => false, // Using fallback
            PlatformFeature::FormalVerification => true, // Always support verification
            _ => false,
        }
    }
    
    fn get_config<T: PlatformConfig>(&self) -> Option<&T> {
        None // No platform-specific config in minimal mode
    }
}

#[cfg(feature = "platform-sync")]
fn create_full_platform_interface() -> Result<Box<dyn PlatformInterface>> {
    // This would integrate with wrt-platform's full abstraction
    // For now, return error to indicate implementation needed
    Err(Error::not_implemented_error("Full platform interface integration pending"))
}

/// Create platform-appropriate limit provider (legacy compatibility)
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn create_limit_provider() -> Box<dyn ComprehensiveLimitProvider> {
    match PlatformId::default() {
        PlatformId::Linux => Box::new(LinuxLimitProvider),
        PlatformId::QNX => Box::new(QnxLimitProvider),
        _ => Box::new(DefaultLimitProvider),
    }
}

/// Create platform limit provider for pure no_std
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub fn create_limit_provider() -> &'static dyn ComprehensiveLimitProvider {
    static DEFAULT_PROVIDER: DefaultLimitProvider = DefaultLimitProvider;
    &DEFAULT_PROVIDER
}

/// Platform-agnostic time functions
pub fn current_time_ns() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
    
    #[cfg(all(not(feature = "std"), feature = "platform-sync"))]
    {
        wrt_platform::time::current_time_ns()
    }
    
    #[cfg(all(not(feature = "std"), not(feature = "platform-sync")))]
    {
        // Fallback: use a simple counter for no_std environments
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
    }
}