//! Platform Abstraction Interface (PAI) for ASIL-compliant systems
//!
//! This module provides a clean abstraction layer between the runtime
//! and platform-specific implementations. It ensures deterministic
//! compilation and proper feature gating for safety-critical systems.
//!
//! # Design Principles
//! - Zero-cost abstractions through compile-time dispatch
//! - Clear separation between platform-dependent and independent code
//! - Deterministic feature combinations for ASIL compliance
//! - Dependency injection pattern for platform services
//! - No implicit dependencies on platform features

#![forbid(unsafe_code)] // Platform abstractions must be safe

use core::marker::PhantomData;
use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_error::Result;use crate::safety_system::{SafetyContext, AsilLevel};

// Box for trait objects when allocation is available
#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use alloc::boxed::Box;

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
            PlatformId::QNX => AsilLevel::AsilB,
            PlatformId::Embedded => AsilLevel::AsilD,
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

/// Comprehensive platform limits for resource management
#[derive(Debug, Clone)]
pub struct PlatformLimits {
    pub platform_id: PlatformId,
    pub max_total_memory: usize,
    pub max_wasm_linear_memory: usize,
    pub max_stack_bytes: usize,
    pub max_components: usize,
    pub max_debug_overhead: usize,
    pub asil_level: AsilLevel,
}

impl Default for PlatformLimits {
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

impl PlatformLimits {
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
            asil_level: AsilLevel::AsilD,
        }
    }
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
    /// High-resolution timers
    HighResolutionTimers,
    /// Thread-local storage
    ThreadLocalStorage,
}

/// Time provider trait for platform-agnostic time operations
pub trait TimeProvider: Send + Sync {
    /// Get current time in nanoseconds since epoch
    fn current_time_ns(&self) -> u64;
    
    /// Get high-resolution timestamp for performance measurement
    fn timestamp(&self) -> u64 {
        self.current_time_ns()
    }
}

/// Task executor trait for platform-agnostic threading
pub trait TaskExecutor: Send + Sync {
    type TaskHandle: Send + Sync;
    type Error: core::fmt::Debug;
    
    /// Spawn a new task
    fn spawn_task<F>(&self, task: F) -> core::result::Result<Self::TaskHandle, Self::Error>
    where
        F: FnOnce() -> Result<()> + Send + 'static;
    
    /// Join a task and wait for completion
    fn join_task(&self, handle: Self::TaskHandle) -> core::result::Result<Result<()>, Self::Error>;
}

/// Limit provider trait for discovering platform capabilities
pub trait LimitProvider: Send + Sync {
    /// Discover platform limits and capabilities
    fn discover_limits(&self) -> Result<PlatformLimits>;
    
    /// Get platform identifier
    fn platform_id(&self) -> PlatformId;
}

/// Main platform abstraction interface
pub trait PlatformInterface: Send + Sync {
    /// Get platform limits
    fn limits(&self) -> &PlatformLimits;
    
    /// Check if a feature is supported
    fn supports_feature(&self, feature: PlatformFeature) -> bool;
    
    /// Get time provider
    fn time_provider(&self) -> &dyn TimeProvider;
    
    /// Get task executor (if available)
    /// Get task executor (if available) - returns None for minimal platforms
    #[cfg(any(feature = "std", feature = "alloc"))]
    fn task_executor(&self) -> Option<&dyn TaskExecutor<TaskHandle = Box<dyn Send + Sync>, Error = Error>>;
    
    /// Get task executor (not available in pure no_std)
    #[cfg(not(any(feature = "std", feature = "alloc")))]
    fn task_executor(&self) -> Option<()> { None }
    
    /// Get safety context for this platform
    fn safety_context(&self) -> SafetyContext;
}

/// Default time provider for no_std environments
#[derive(Debug)]
pub struct NoStdTimeProvider {
    counter: core::sync::atomic::AtomicU64,
}

impl NoStdTimeProvider {
    pub const fn new() -> Self {
        Self {
            counter: core::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl Default for NoStdTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeProvider for NoStdTimeProvider {
    fn current_time_ns(&self) -> u64 {
        // Simple counter-based time for no_std environments
        self.counter.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
    }
}

/// Standard time provider for std environments  
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct StdTimeProvider;

#[cfg(feature = "std")]
impl TimeProvider for StdTimeProvider {
    fn current_time_ns(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
}

/// Default limit provider
#[derive(Debug, Default)]
pub struct DefaultLimitProvider {
    limits: PlatformLimits,
}

impl LimitProvider for DefaultLimitProvider {
    fn discover_limits(&self) -> Result<PlatformLimits> {
        Ok(self.limits.clone())
    }
    
    fn platform_id(&self) -> PlatformId {
        self.limits.platform_id
    }
}

/// Minimal platform implementation for no_std environments
#[derive(Debug)]
pub struct MinimalPlatform {
    limits: PlatformLimits,
    time_provider: NoStdTimeProvider,
    safety_context: SafetyContext,
}

impl MinimalPlatform {
    pub fn new() -> Self {
        let limits = PlatformLimits::minimal);
        Self {
            safety_context: SafetyContext::new(limits.asil_level),
            limits,
            time_provider: NoStdTimeProvider::new(),
        }
    }
    
    pub fn with_limits(limits: PlatformLimits) -> Self {
        Self {
            safety_context: SafetyContext::new(limits.asil_level),
            time_provider: NoStdTimeProvider::new(),
            limits,
        }
    }
}

impl Default for MinimalPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformInterface for MinimalPlatform {
    fn limits(&self) -> &PlatformLimits {
        &self.limits
    }
    
    fn supports_feature(&self, feature: PlatformFeature) -> bool {
        match feature {
            PlatformFeature::NativeAtomics => false, // Using fallback
            PlatformFeature::FormalVerification => true, // Always support verification
            PlatformFeature::HighResolutionTimers => false, // Counter-based
            _ => false,
        }
    }
    
    fn time_provider(&self) -> &dyn TimeProvider {
        &self.time_provider
    }
    
    fn task_executor(&self) -> Option<&dyn TaskExecutor<TaskHandle = Box<dyn Send + Sync>, Error = Error>> {
        None // No threading in minimal platform
    }
    
    fn safety_context(&self) -> SafetyContext {
        self.safety_context
    }
}

/// Standard platform implementation for std environments
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct StdPlatform {
    limits: PlatformLimits,
    time_provider: StdTimeProvider,
    safety_context: SafetyContext,
}

#[cfg(feature = "std")]
impl StdPlatform {
    pub fn new() -> Self {
        let limits = PlatformLimits::default());
        Self {
            safety_context: SafetyContext::new(limits.asil_level),
            limits,
            time_provider: StdTimeProvider,
        }
    }
    
    pub fn with_limits(limits: PlatformLimits) -> Self {
        Self {
            safety_context: SafetyContext::new(limits.asil_level),
            time_provider: StdTimeProvider,
            limits,
        }
    }
}

#[cfg(feature = "std")]
impl Default for StdPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl PlatformInterface for StdPlatform {
    fn limits(&self) -> &PlatformLimits {
        &self.limits
    }
    
    fn supports_feature(&self, feature: PlatformFeature) -> bool {
        match feature {
            PlatformFeature::NativeAtomics => true,
            PlatformFeature::MemoryMappedFiles => true,
            PlatformFeature::GuardPages => true,
            PlatformFeature::FormalVerification => true,
            PlatformFeature::HighResolutionTimers => true,
            PlatformFeature::ThreadLocalStorage => true,
            _ => false,
        }
    }
    
    fn time_provider(&self) -> &dyn TimeProvider {
        &self.time_provider
    }
    
    fn task_executor(&self) -> Option<&dyn TaskExecutor<TaskHandle = Box<dyn Send + Sync>, Error = Error>> {
        None // TODO: Implement std task executor
    }
    
    fn safety_context(&self) -> SafetyContext {
        self.safety_context
    }
}

/// Create platform interface based on compile-time features and runtime needs
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn create_platform_interface() -> Result<Box<dyn PlatformInterface>> {
    #[cfg(feature = "std")]
    {
        Ok(Box::new(StdPlatform::new()))
    }
    
    #[cfg(all(not(feature = "std"), feature = "alloc"))]
    {
        Ok(Box::new(MinimalPlatform::new()))
    }
}

/// Create platform interface for pure no_std environments
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub fn create_platform_interface() -> Result<&'static dyn PlatformInterface> {
    static MINIMAL_PLATFORM: MinimalPlatform = MinimalPlatform {
        limits: PlatformLimits {
            platform_id: PlatformId::Embedded,
            max_total_memory: 256 * 1024,
            max_wasm_linear_memory: 64 * 1024,
            max_stack_bytes: 16 * 1024,
            max_components: 8,
            max_debug_overhead: 0,
            asil_level: AsilLevel::AsilD,
        },
        time_provider: NoStdTimeProvider::new(),
        safety_context: SafetyContext::new(AsilLevel::AsilD),
    };
    Ok(&MINIMAL_PLATFORM)
}

/// Convenience function to get current time through platform abstraction
pub fn current_time_ns() -> u64 {
    #[cfg(feature = "std")]
    {
        StdTimeProvider.current_time_ns()
    }
    
    #[cfg(not(feature = "std"))]
    {
        static TIME_PROVIDER: NoStdTimeProvider = NoStdTimeProvider::new();
        TIME_PROVIDER.current_time_ns()
    }
}