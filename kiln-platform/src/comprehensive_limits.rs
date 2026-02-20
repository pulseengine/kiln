//! Comprehensive Platform Limit Discovery
//!
//! Provides comprehensive platform limit discovery capabilities across
//! different operating systems and runtime environments.

use wrt_error::Error;

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

#[cfg(feature = "std")]
use alloc::boxed::Box;

// Stub imports for foundation module - will be replaced during integration
mod foundation_stubs {
    /// ASIL (Automotive Safety Integrity Level) classification
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AsilLevel {
        /// Quality Management (no ASIL)
        QM,
        /// ASIL A (lowest safety level)
        AsilA,
        /// ASIL B (medium-low safety level)
        AsilB,
        /// ASIL C (medium-high safety level)
        AsilC,
        /// ASIL D (highest safety level)
        AsilD,
    }
}

pub use foundation_stubs::AsilLevel;

/// Platform identification enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformId {
    /// Linux platform
    Linux,
    /// QNX platform  
    QNX,
    /// macOS platform
    MacOS,
    /// VxWorks platform
    VxWorks,
    /// Zephyr RTOS
    Zephyr,
    /// Tock OS
    Tock,
    /// Generic embedded platform
    Embedded,
    /// Unknown platform
    Unknown,
}

/// Comprehensive platform limits structure
#[derive(Debug, Clone)]
pub struct ComprehensivePlatformLimits {
    /// Platform identifier
    pub platform_id:            PlatformId,
    /// Maximum total memory available to the runtime
    pub max_total_memory:       usize,
    /// Maximum WebAssembly linear memory
    pub max_wasm_linear_memory: usize,
    /// Maximum stack bytes
    pub max_stack_bytes:        usize,
    /// Maximum number of components
    pub max_components:         usize,
    /// Maximum debug overhead memory
    pub max_debug_overhead:     usize,
    /// ASIL level for safety-critical systems
    pub asil_level:             AsilLevel,
}

impl Default for ComprehensivePlatformLimits {
    fn default() -> Self {
        Self {
            platform_id:            PlatformId::Unknown,
            max_total_memory:       1024 * 1024 * 1024, // 1GB
            max_wasm_linear_memory: 256 * 1024 * 1024,  // 256MB
            max_stack_bytes:        1024 * 1024,        // 1MB
            max_components:         256,
            max_debug_overhead:     64 * 1024 * 1024, // 64MB
            asil_level:             AsilLevel::QM,
        }
    }
}

/// Trait for comprehensive limit providers
pub trait ComprehensiveLimitProvider: Send + Sync {
    /// Discover platform limits
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, Error>;

    /// Get platform identifier
    fn platform_id(&self) -> PlatformId;
}

/// Linux limit provider implementation
pub struct LinuxLimitProvider;

impl ComprehensiveLimitProvider for LinuxLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, Error> {
        let mut limits = ComprehensivePlatformLimits {
            platform_id: PlatformId::Linux,
            ..ComprehensivePlatformLimits::default()
        };

        #[cfg(feature = "std")]
        {
            // Read /proc/meminfo for memory information
            if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
                if let Some(total_memory) = parse_meminfo_value(&meminfo, "MemTotal:") {
                    limits.max_total_memory = (total_memory * 1024).min(limits.max_total_memory);
                    // Reserve 25% for system, use 75% for WebAssembly
                    limits.max_wasm_linear_memory = (limits.max_total_memory * 3) / 4;
                }
            }

            // Check for container limits (Docker, cgroups)
            if let Ok(cgroup_memory) =
                std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes")
            {
                if let Ok(limit) = cgroup_memory.trim().parse::<usize>() {
                    if limit < limits.max_total_memory {
                        limits.max_total_memory = limit;
                        limits.max_wasm_linear_memory = (limit * 3) / 4;
                    }
                }
            }

            // Check for environment variables
            if let Ok(max_mem) = std::env::var("WRT_MAX_MEMORY") {
                if let Ok(limit) = max_mem.parse::<usize>() {
                    limits.max_total_memory = limit;
                    limits.max_wasm_linear_memory = (limit * 3) / 4;
                }
            }
        }

        // Set conservative stack limits for Linux
        limits.max_stack_bytes = 8 * 1024 * 1024; // 8MB
        limits.max_components = 512;
        limits.max_debug_overhead = limits.max_total_memory / 10; // 10% for debug

        Ok(limits)
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::Linux
    }
}

/// QNX limit provider implementation
pub struct QnxLimitProvider;

impl ComprehensiveLimitProvider for QnxLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, Error> {
        let limits = ComprehensivePlatformLimits {
            platform_id:            PlatformId::QNX,
            max_total_memory:       512 * 1024 * 1024, // 512MB conservative for QNX
            max_wasm_linear_memory: 256 * 1024 * 1024, // 256MB
            max_stack_bytes:        2 * 1024 * 1024,   // 2MB stack
            max_components:         128,               // Conservative for embedded
            max_debug_overhead:     32 * 1024 * 1024,  // 32MB debug
            asil_level:             AsilLevel::AsilB,  // Assume automotive grade
        };

        Ok(limits)
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::QNX
    }
}

/// macOS limit provider implementation  
pub struct MacOsLimitProvider;

impl ComprehensiveLimitProvider for MacOsLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, Error> {
        let mut limits = ComprehensivePlatformLimits {
            platform_id: PlatformId::MacOS,
            ..ComprehensivePlatformLimits::default()
        };

        #[cfg(all(feature = "std", target_os = "macos"))]
        {
            // Query system memory via sysctl
            // In a real implementation, this would use sysctl calls
            limits.max_total_memory = 8 * 1024 * 1024 * 1024; // 8GB typical
            limits.max_wasm_linear_memory = 4 * 1024 * 1024 * 1024; // 4GB
        }

        limits.max_stack_bytes = 16 * 1024 * 1024; // 16MB stack
        limits.max_components = 1024; // macOS can handle more
        limits.max_debug_overhead = limits.max_total_memory / 8; // 12.5% for debug

        Ok(limits)
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::MacOS
    }
}

/// Embedded platform limit provider
pub struct EmbeddedLimitProvider {
    /// Configured memory size
    pub memory_size: usize,
    /// ASIL level for the embedded system
    pub asil_level:  AsilLevel,
}

impl EmbeddedLimitProvider {
    /// Create new embedded limit provider
    pub fn new(memory_size: usize, asil_level: AsilLevel) -> Self {
        Self {
            memory_size,
            asil_level,
        }
    }
}

impl ComprehensiveLimitProvider for EmbeddedLimitProvider {
    fn discover_limits(&self) -> Result<ComprehensivePlatformLimits, Error> {
        let limits = ComprehensivePlatformLimits {
            platform_id:            PlatformId::Embedded,
            max_total_memory:       self.memory_size,
            max_wasm_linear_memory: (self.memory_size * 2) / 3, // 66% for WASM
            max_stack_bytes:        self.memory_size / 16,      // 6.25% for stack
            max_components:         16,                         // Very limited for embedded
            max_debug_overhead:     self.memory_size / 20,      // 5% for debug
            asil_level:             self.asil_level,
        };

        Ok(limits)
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::Embedded
    }
}

/// Platform limit discoverer - main entry point
pub struct PlatformLimitDiscoverer {
    /// Cached limits
    cached_limits: Option<ComprehensivePlatformLimits>,
}

impl PlatformLimitDiscoverer {
    /// Create new platform limit discoverer
    pub fn new() -> Self {
        Self {
            cached_limits: None,
        }
    }

    /// Discover platform limits with caching
    pub fn discover(&mut self) -> Result<ComprehensivePlatformLimits, Error> {
        if let Some(ref limits) = self.cached_limits {
            return Ok(limits.clone());
        }

        #[cfg(feature = "std")]
        let limits = {
            let provider: alloc::boxed::Box<dyn ComprehensiveLimitProvider> =
                self.create_provider()?;
            provider.discover_limits()?
        };

        #[cfg(not(feature = "std"))]
        let limits = {
            let provider = self.create_provider()?;
            provider.discover_limits()?
        };

        self.cached_limits = Some(limits.clone());

        Ok(limits)
    }

    /// Create appropriate provider for current platform
    #[cfg(feature = "std")]
    fn create_provider(&self) -> Result<alloc::boxed::Box<dyn ComprehensiveLimitProvider>, Error> {
        #[cfg(target_os = "linux")]
        return Ok(alloc::boxed::Box::new(LinuxLimitProvider));

        #[cfg(target_os = "nto")]
        return Ok(alloc::boxed::Box::new(QnxLimitProvider));

        #[cfg(target_os = "macos")]
        return Ok(alloc::boxed::Box::new(MacOsLimitProvider));

        #[cfg(not(any(target_os = "linux", target_os = "nto", target_os = "macos")))]
        return Ok(alloc::boxed::Box::new(EmbeddedLimitProvider::new(
            64 * 1024 * 1024, // 64MB default
            AsilLevel::QM,
        )));
    }

    /// Create appropriate provider for current platform (no_std version)
    #[cfg(not(feature = "std"))]
    fn create_provider(&self) -> Result<EmbeddedLimitProvider, Error> {
        Ok(EmbeddedLimitProvider::new(
            64 * 1024 * 1024, // 64MB default
            AsilLevel::QM,
        ))
    }
}

impl Default for PlatformLimitDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
fn parse_meminfo_value(meminfo: &str, key: &str) -> Option<usize> {
    meminfo
        .lines()
        .find(|line| line.starts_with(key))
        .and_then(|line| line.split_whitespace().nth(1).and_then(|value| value.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ComprehensivePlatformLimits::default();
        assert_eq!(limits.platform_id, PlatformId::Unknown);
        assert!(limits.max_total_memory > 0);
        assert!(limits.max_wasm_linear_memory > 0);
        assert!(limits.max_stack_bytes > 0);
        assert!(limits.max_components > 0);
    }

    #[test]
    fn test_embedded_provider() {
        let provider = EmbeddedLimitProvider::new(1024 * 1024, AsilLevel::AsilC);
        let limits = provider.discover_limits().unwrap();

        assert_eq!(limits.platform_id, PlatformId::Embedded);
        assert_eq!(limits.max_total_memory, 1024 * 1024);
        assert!(limits.max_wasm_linear_memory < limits.max_total_memory);
        assert!(limits.max_stack_bytes < limits.max_total_memory);
    }

    #[test]
    fn test_discoverer() {
        let mut discoverer = PlatformLimitDiscoverer::new();
        let limits1 = discoverer.discover().unwrap();
        let limits2 = discoverer.discover().unwrap();

        // Should be cached and identical
        assert_eq!(limits1.platform_id, limits2.platform_id);
        assert_eq!(limits1.max_total_memory, limits2.max_total_memory);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_meminfo() {
        let meminfo = "MemTotal:       16384000 kB\nMemFree:         8192000 kB\n";
        let value = parse_meminfo_value(meminfo, "MemTotal:");
        assert_eq!(value, Some(16_384_000));

        let value = parse_meminfo_value(meminfo, "MemFree:");
        assert_eq!(value, Some(8_192_000));

        let value = parse_meminfo_value(meminfo, "NonExistent:");
        assert_eq!(value, None);
    }
}
