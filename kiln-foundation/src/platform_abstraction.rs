//! Simple Platform Abstraction for WRT Foundation
//!
//! This module provides essential platform services that can be injected
//! into the runtime, keeping the core runtime platform-agnostic.

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::safety_system::{
    AsilLevel,
    SafetyContext,
};

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;
#[cfg(any(feature = "std", feature = "alloc"))]
use alloc::boxed::Box;

/// Platform limits for resource management
#[derive(Debug, Clone, Copy)]
pub struct PlatformLimits {
    pub max_memory:     usize,
    pub max_stack:      usize,
    pub max_components: usize,
}

impl Default for PlatformLimits {
    fn default() -> Self {
        Self {
            max_memory:     256 * 1024 * 1024, // 256MB
            max_stack:      8 * 1024 * 1024,   // 8MB
            max_components: 256,
        }
    }
}

impl PlatformLimits {
    pub const fn minimal() -> Self {
        Self {
            max_memory:     64 * 1024, // 64KB
            max_stack:      16 * 1024, // 16KB
            max_components: 8,
        }
    }
}

/// Simple time provider trait
pub trait TimeProvider: Send + Sync {
    fn current_time_ns(&self) -> u64;
}

/// No-std time provider using atomic counter
#[derive(Debug)]
pub struct CounterTimeProvider {
    counter: core::sync::atomic::AtomicU64,
}

impl CounterTimeProvider {
    pub const fn new() -> Self {
        Self {
            counter: core::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl Default for CounterTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeProvider for CounterTimeProvider {
    fn current_time_ns(&self) -> u64 {
        self.counter.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
    }
}

/// Std time provider using system time
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct SystemTimeProvider;

#[cfg(feature = "std")]
impl TimeProvider for SystemTimeProvider {
    fn current_time_ns(&self) -> u64 {
        use std::time::{
            SystemTime,
            UNIX_EPOCH,
        };
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
}

/// Simple platform services interface
pub struct PlatformServices {
    pub limits:         PlatformLimits,
    pub time_provider:  &'static dyn TimeProvider,
    pub safety_context: SafetyContext,
}

impl PlatformServices {
    pub fn minimal() -> Self {
        static TIME_PROVIDER: CounterTimeProvider = CounterTimeProvider::new();
        Self {
            limits:         PlatformLimits::minimal(),
            time_provider:  &TIME_PROVIDER,
            safety_context: SafetyContext::new(AsilLevel::AsilD),
        }
    }

    #[cfg(feature = "std")]
    pub fn standard() -> Self {
        static TIME_PROVIDER: SystemTimeProvider = SystemTimeProvider;
        Self {
            limits:         PlatformLimits::default(),
            time_provider:  &TIME_PROVIDER,
            safety_context: SafetyContext::new(AsilLevel::QM),
        }
    }

    #[cfg(not(feature = "std"))]
    pub fn standard() -> Self {
        Self::minimal()
    }
}

impl Default for PlatformServices {
    fn default() -> Self {
        Self::standard()
    }
}

use core::sync::atomic::{
    AtomicPtr,
    Ordering,
};

/// Global platform services instance using safer atomic pointer
static PLATFORM_SERVICES: AtomicPtr<PlatformServices> = AtomicPtr::new(core::ptr::null_mut());

/// Initialize platform services (call once at startup)
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn initialize_platform_services(services: PlatformServices) -> Result<()> {
    let boxed = Box::into_raw(Box::new(services));
    match PLATFORM_SERVICES.compare_exchange(
        core::ptr::null_mut(),
        boxed,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => Ok(()),
        Err(_) => {
            // Someone else already initialized, clean up our allocation
            #[allow(unsafe_code)] // Required for cleanup
            unsafe {
                drop(Box::from_raw(boxed));
            }
            Err(Error::runtime_execution_error(
                "Platform services already initialized",
            ))
        },
    }
}

/// Initialize platform services (no-op for pure no_std)
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub fn initialize_platform_services(_services: PlatformServices) -> Result<()> {
    // In pure no_std, we can't dynamically allocate
    Err(Error::runtime_execution_error(
        "Dynamic platform services not supported in pure no_std",
    ))
}

/// Get platform services (returns minimal defaults if not initialized)
pub fn get_platform_services() -> &'static PlatformServices {
    let ptr = PLATFORM_SERVICES.load(Ordering::Acquire);
    if !ptr.is_null() {
        #[allow(unsafe_code)] // Required for pointer dereference
        unsafe {
            &*ptr
        }
    } else {
        // Return static minimal services as fallback
        static MINIMAL_TIME_PROVIDER: CounterTimeProvider = CounterTimeProvider::new();
        static MINIMAL_SERVICES: PlatformServices = PlatformServices {
            limits:         PlatformLimits::minimal(),
            time_provider:  &MINIMAL_TIME_PROVIDER,
            safety_context: SafetyContext::new(AsilLevel::AsilD),
        };
        &MINIMAL_SERVICES
    }
}

/// Convenience function to get current time
pub fn current_time_ns() -> u64 {
    get_platform_services().time_provider.current_time_ns()
}

/// Convenience function to get platform limits
pub fn get_platform_limits() -> PlatformLimits {
    get_platform_services().limits
}
