//! Zero-Cost Hybrid Platform Abstraction
//!
//! This module provides a compile-time zero-cost abstraction that supports
//! multiple platform paradigms:
//! - Traditional POSIX-style platforms (Linux, macOS, QNX)
//! - Security-focused embedded platforms (Tock OS)
//! - Real-time embedded platforms (Zephyr, bare metal)
//!
//! Performance guarantee: All abstractions compile down to direct platform
//! calls with zero runtime overhead through monomorphization and inlining.

use core::marker::PhantomData;

use wrt_error::Error;

/// Platform paradigm marker types for compile-time dispatch
pub mod paradigm {
    /// Binary std/no_std choice
    pub struct Posix;

    /// Binary std/no_std choice
    pub struct SecurityFirst;

    /// Real-time systems with deterministic behavior
    pub struct RealTime;

    /// Bare metal systems with minimal abstraction
    pub struct BareMetal;
}

/// Zero-cost platform abstraction that compiles to platform-specific code
pub trait PlatformAbstraction<P> {
    /// Binary std/no_std choice
    type Allocator: super::memory::PageAllocator;
    /// Platform-specific synchronization type
    type Synchronizer: super::sync::FutexLike;
    /// Platform-specific configuration type
    type Config;

    /// Binary std/no_std choice
    fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error>;

    /// Create platform-specific synchronizer (compiles to direct constructor)  
    fn create_synchronizer(config: &Self::Config) -> Result<Self::Synchronizer, Error>;
}

/// Unified platform configuration that adapts to platform paradigm
#[derive(Debug, Clone)]
pub struct PlatformConfig<P> {
    /// Binary std/no_std choice
    pub max_pages: u32,

    /// Enable guard pages (POSIX platforms)
    pub guard_pages: bool,

    /// Binary std/no_std choice
    pub static_allocation_size: Option<usize>,

    /// Real-time priority settings (RealTime platforms)
    pub rt_priority: Option<u32>,

    /// Security isolation level (SecurityFirst platforms)
    pub isolation_level: Option<IsolationLevel>,

    /// Platform paradigm marker (zero-sized)
    _paradigm: PhantomData<P>,
}

/// Security isolation levels for security-first platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// No isolation (trusted environment)
    None,
    /// Process-level isolation
    Process,
    /// Hardware-enforced isolation (MPU/MMU)
    Hardware,
    /// Formal verification required
    Verified,
}

impl<P> Default for PlatformConfig<P> {
    fn default() -> Self {
        Self {
            max_pages:              1024,
            guard_pages:            true,
            static_allocation_size: None,
            rt_priority:            None,
            isolation_level:        None,
            _paradigm:              PhantomData,
        }
    }
}

impl<P> PlatformConfig<P> {
    /// Create new configuration for specific paradigm
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum pages (universal setting)
    pub fn with_max_pages(mut self, pages: u32) -> Self {
        self.max_pages = pages;
        self
    }

    /// Enable guard pages (POSIX platforms only, ignored elsewhere)
    pub fn with_guard_pages(mut self, enable: bool) -> Self {
        self.guard_pages = enable;
        self
    }
}

impl PlatformConfig<paradigm::SecurityFirst> {
    /// Binary std/no_std choice
    pub fn with_static_allocation(mut self, size: usize) -> Self {
        self.static_allocation_size = Some(size);
        self
    }

    /// Set isolation level (SecurityFirst platforms)
    pub fn with_isolation_level(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = Some(level);
        self
    }
}

impl PlatformConfig<paradigm::RealTime> {
    /// Set real-time priority (RealTime platforms)
    pub fn with_rt_priority(mut self, priority: u32) -> Self {
        self.rt_priority = Some(priority);
        self
    }
}

/// Unified platform provider that adapts at compile-time
pub struct UnifiedPlatform<P> {
    config: PlatformConfig<P>,
}

impl<P> UnifiedPlatform<P> {
    /// Create new unified platform instance
    pub fn new(config: PlatformConfig<P>) -> Self {
        Self { config }
    }

    /// Get platform configuration
    pub fn config(&self) -> &PlatformConfig<P> {
        &self.config
    }
}

// POSIX Platform Implementations (Linux, macOS, QNX, VxWorks RTP)
#[cfg(any(
    all(feature = "platform-linux", target_os = "linux"),
    all(feature = "platform-macos", target_os = "macos"),
    all(feature = "platform-qnx", target_os = "nto"),
    all(feature = "platform-vxworks", target_os = "vxworks")
))]
mod posix_impl {
    use super::*;

    // Linux POSIX implementation
    #[cfg(all(feature = "platform-linux", target_os = "linux"))]
    impl PlatformAbstraction<paradigm::Posix> for UnifiedPlatform<paradigm::Posix> {
        type Allocator = crate::LinuxAllocator;
        type Config = PlatformConfig<paradigm::Posix>;
        type Synchronizer = crate::LinuxFutex;

        #[inline(always)] // Zero-cost: compiles to direct constructor call
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            crate::LinuxAllocatorBuilder::new()
                .with_maximum_pages(config.max_pages)
                .with_guard_pages(config.guard_pages)
                .build()
        }

        #[inline(always)] // Zero-cost: compiles to direct constructor call
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            crate::LinuxFutexBuilder::new().build()
        }
    }

    // macOS POSIX implementation
    #[cfg(all(feature = "platform-macos", target_os = "macos"))]
    impl PlatformAbstraction<paradigm::Posix> for UnifiedPlatform<paradigm::Posix> {
        type Allocator = crate::MacOsAllocator;
        type Config = PlatformConfig<paradigm::Posix>;
        type Synchronizer = crate::MacOsFutex;

        #[inline(always)]
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            Ok(crate::MacOsAllocatorBuilder::new()
                .with_maximum_pages(config.max_pages)
                .with_guard_pages(config.guard_pages)
                .build())
        }

        #[inline(always)]
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            Ok(crate::MacOsFutexBuilder::new().build())
        }
    }

    // QNX POSIX implementation
    #[cfg(all(feature = "platform-qnx", target_os = "nto"))]
    impl PlatformAbstraction<paradigm::Posix> for UnifiedPlatform<paradigm::Posix> {
        type Allocator = crate::QnxAllocator;
        type Config = PlatformConfig<paradigm::Posix>;
        type Synchronizer = crate::QnxFutex;

        #[inline(always)]
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            crate::QnxAllocatorBuilder::new()
                .with_maximum_pages(config.max_pages)
                .with_guard_pages(config.guard_pages)
                .build()
        }

        #[inline(always)]
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            crate::QnxFutexBuilder::new().build()
        }
    }

    // VxWorks POSIX implementation (RTP user-space applications)
    #[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
    impl PlatformAbstraction<paradigm::Posix> for UnifiedPlatform<paradigm::Posix> {
        type Allocator = crate::VxWorksAllocator;
        type Config = PlatformConfig<paradigm::Posix>;
        type Synchronizer = crate::VxWorksFutex;

        #[inline(always)]
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            crate::VxWorksAllocatorBuilder::new()
                .context(crate::vxworks_memory::VxWorksContext::Rtp) // RTP for POSIX-like usage
                .max_pages(config.max_pages as usize)
                .enable_guard_pages(config.guard_pages)
                .use_dedicated_partition(false) // Use system heap for RTP
                .build()
        }

        #[inline(always)]
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            crate::VxWorksFutexBuilder::new(crate::vxworks_memory::VxWorksContext::Rtp).build()
        }
    }
}

// Real-Time Platform Implementation (Zephyr, VxWorks LKM)
#[cfg(any(
    feature = "platform-zephyr",
    all(feature = "platform-vxworks", target_os = "vxworks")
))]
mod realtime_impl {
    use super::*;

    // Zephyr Real-Time implementation
    #[cfg(feature = "platform-zephyr")]
    impl PlatformAbstraction<paradigm::RealTime> for UnifiedPlatform<paradigm::RealTime> {
        type Allocator = crate::ZephyrAllocator;
        type Config = PlatformConfig<paradigm::RealTime>;
        type Synchronizer = crate::ZephyrFutex;

        #[inline(always)] // Zero-cost: compiles to direct Zephyr calls
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            let mut builder = crate::ZephyrAllocatorBuilder::new()
                .with_maximum_pages(config.max_pages)
                .with_memory_domains(true)
                .with_guard_regions(config.guard_pages);

            // Apply real-time specific configuration
            if let Some(_priority) = config.rt_priority {
                // Binary std/no_std choice
                builder = builder.with_memory_domains(true);
            }

            Ok(builder.build())
        }

        #[inline(always)]
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            Ok(crate::ZephyrFutexBuilder::new().build())
        }
    }

    // VxWorks Real-Time implementation (LKM kernel modules)
    #[cfg(all(feature = "platform-vxworks", target_os = "vxworks"))]
    impl PlatformAbstraction<paradigm::RealTime> for UnifiedPlatform<paradigm::RealTime> {
        type Allocator = crate::VxWorksAllocator;
        type Config = PlatformConfig<paradigm::RealTime>;
        type Synchronizer = crate::VxWorksFutex;

        #[inline(always)] // Zero-cost: compiles to direct VxWorks calls
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            let mut builder = crate::VxWorksAllocatorBuilder::new()
                .context(crate::vxworks_memory::VxWorksContext::Lkm) // LKM for real-time usage
                .max_pages(config.max_pages as usize)
                .enable_guard_pages(config.guard_pages)
                .use_dedicated_partition(true); // Binary std/no_std choice

            // Apply real-time specific configuration
            if config.rt_priority.is_some() {
                // Configure memory partition for real-time use
                builder = builder.partition_size(config.max_pages as usize * crate::WASM_PAGE_SIZE);
            }

            builder.build()
        }

        #[inline(always)]
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            crate::VxWorksFutexBuilder::new(crate::vxworks_memory::VxWorksContext::Lkm).build()
        }
    }
}

// Security-First Platform Implementation (Tock OS)
#[cfg(feature = "platform-tock")]
mod security_impl {
    use super::*;

    impl PlatformAbstraction<paradigm::SecurityFirst> for UnifiedPlatform<paradigm::SecurityFirst> {
        type Allocator = crate::TockAllocator;
        type Config = PlatformConfig<paradigm::SecurityFirst>;
        type Synchronizer = crate::TockFutex;

        #[inline(always)] // Zero-cost: compiles to direct Tock calls
        fn create_allocator(config: &Self::Config) -> Result<Self::Allocator, Error> {
            let mut builder = crate::TockAllocatorBuilder::new()
                .with_maximum_pages(config.max_pages)
                .with_verification_level(match config.isolation_level {
                    Some(IsolationLevel::None) => crate::VerificationLevel::Off,
                    Some(IsolationLevel::Process) => crate::VerificationLevel::Standard,
                    Some(IsolationLevel::Hardware | IsolationLevel::Verified) => {
                        crate::VerificationLevel::Full
                    },
                    None => crate::VerificationLevel::Full,
                });

            // Binary std/no_std choice
            if let Some(size) = config.static_allocation_size {
                // In a real implementation, this would use a static buffer
                // Binary std/no_std choice
                builder = builder.with_maximum_pages((size / crate::WASM_PAGE_SIZE) as u32);
            }

            builder.build()
        }

        #[inline(always)]
        fn create_synchronizer(_config: &Self::Config) -> Result<Self::Synchronizer, Error> {
            crate::TockFutexBuilder::new().build()
        }
    }
}

/// Compile-time platform selection helper
pub mod platform_select {
    use super::*;

    /// Auto-select best available platform paradigm at compile-time
    /// Priority order: SecurityFirst > RealTime > Posix
    #[cfg(feature = "platform-tock")]
    pub type Auto = paradigm::SecurityFirst;

    /// Auto-select: Real-time platform when Zephyr available
    #[cfg(all(feature = "platform-zephyr", not(feature = "platform-tock")))]
    pub type Auto = paradigm::RealTime;

    /// Auto-select: POSIX platform when available
    #[cfg(all(
        any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto"),
            all(feature = "platform-vxworks", target_os = "vxworks")
        ),
        not(feature = "platform-tock"),
        not(feature = "platform-zephyr")
    ))]
    pub type Auto = paradigm::Posix;

    /// Fallback when no platform features are enabled
    #[cfg(not(any(
        feature = "platform-tock",
        feature = "platform-zephyr",
        all(feature = "platform-linux", target_os = "linux"),
        all(feature = "platform-macos", target_os = "macos"),
        all(feature = "platform-qnx", target_os = "nto"),
        all(feature = "platform-vxworks", target_os = "vxworks")
    )))]
    pub type Auto = paradigm::Posix;

    /// Helper function to create auto-selected platform
    pub fn create_auto_platform() -> UnifiedPlatform<Auto> {
        UnifiedPlatform::new(PlatformConfig::new())
    }
}

/// Convenience type aliases for common configurations
pub type PosixPlatform = UnifiedPlatform<paradigm::Posix>;
/// Security-focused platform with hardware isolation
pub type SecurityPlatform = UnifiedPlatform<paradigm::SecurityFirst>;
/// Real-time platform with deterministic behavior
pub type RealtimePlatform = UnifiedPlatform<paradigm::RealTime>;
/// Bare metal platform with minimal abstractions
pub type BaremetalPlatform = UnifiedPlatform<paradigm::BareMetal>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = PlatformConfig::<paradigm::Posix>::new()
            .with_max_pages(2048)
            .with_guard_pages(true);

        assert_eq!(config.max_pages, 2048);
        assert!(config.guard_pages);
    }

    #[test]
    fn test_security_config() {
        let config = PlatformConfig::<paradigm::SecurityFirst>::new()
            .with_max_pages(1024)
            .with_static_allocation(64 * 1024)
            .with_isolation_level(IsolationLevel::Hardware);

        assert_eq!(config.max_pages, 1024);
        assert_eq!(config.static_allocation_size, Some(64 * 1024));
        assert_eq!(config.isolation_level, Some(IsolationLevel::Hardware));
    }

    #[test]
    fn test_realtime_config() {
        let config = PlatformConfig::<paradigm::RealTime>::new()
            .with_max_pages(512)
            .with_rt_priority(10);

        assert_eq!(config.max_pages, 512);
        assert_eq!(config.rt_priority, Some(10));
    }

    #[cfg(any(
        all(feature = "platform-linux", target_os = "linux"),
        all(feature = "platform-macos", target_os = "macos"),
        all(feature = "platform-qnx", target_os = "nto")
    ))]
    #[test]
    fn test_posix_platform_creation() {
        let platform = PosixPlatform::new(PlatformConfig::new());

        // Binary std/no_std choice
        let _config = platform.config();
        assert_eq!(_config.max_pages, 1024);
    }

    #[cfg(feature = "platform-zephyr")]
    #[test]
    fn test_realtime_platform_creation() {
        let platform = RealtimePlatform::new(PlatformConfig::new().with_rt_priority(5));

        // Binary std/no_std choice
        let _config = platform.config();
        assert_eq!(_config.rt_priority, Some(5));
    }
}
