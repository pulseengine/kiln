//! Runtime Platform Detection
//!
//! Provides runtime detection of platform capabilities for dynamic scenarios
//! where compile-time selection is not sufficient. This is useful for:
//! - Applications that need to adapt to different deployment environments
//! - Libraries that want to provide best-effort platform support
//! - Testing and benchmarking across multiple platforms

use wrt_error::Error;

/// Platform capabilities detected at runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapabilities {
    /// Memory management capabilities
    pub memory:   MemoryCapabilities,
    /// Synchronization capabilities  
    pub sync:     SyncCapabilities,
    /// Security features available
    pub security: SecurityCapabilities,
    /// Real-time guarantees available
    pub realtime: RealtimeCapabilities,
}

/// Memory management capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCapabilities {
    /// Binary std/no_std choice
    pub dynamic_allocation:     bool,
    /// Supports memory protection (mprotect-style)
    pub memory_protection:      bool,
    /// Supports guard pages
    pub guard_pages:            bool,
    /// Has hardware memory tagging (ARM MTE, Intel MPX, etc.)
    pub hardware_tagging:       bool,
    /// Binary std/no_std choice
    pub max_memory:             Option<usize>,
    /// Binary std/no_std choice
    pub allocation_granularity: usize,
}

/// Synchronization capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncCapabilities {
    /// Supports futex-style blocking synchronization
    pub futex_support:      bool,
    /// Supports cross-process synchronization
    pub cross_process_sync: bool,
    /// Supports timeout-based operations
    pub timeout_support:    bool,
    /// Has hardware atomic operations beyond basic CAS
    pub hardware_atomics:   bool,
    /// Maximum number of waiters supported
    pub max_waiters:        Option<u32>,
}

/// Security capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityCapabilities {
    /// Has hardware memory isolation (MPU/MMU)
    pub hardware_isolation:  bool,
    /// Supports process-level isolation
    pub process_isolation:   bool,
    /// Has capability-based security
    pub capability_security: bool,
    /// Supports formal verification
    pub formal_verification: bool,
    /// Has trusted execution environment
    pub trusted_execution:   bool,
}

/// Real-time capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealtimeCapabilities {
    /// Provides deterministic timing guarantees
    pub deterministic_timing:  bool,
    /// Supports priority-based scheduling
    pub priority_scheduling:   bool,
    /// Has preemption control
    pub preemption_control:    bool,
    /// Maximum interrupt latency in nanoseconds
    pub max_interrupt_latency: Option<u64>,
    /// Supports deadline scheduling
    pub deadline_scheduling:   bool,
}

/// Runtime platform detector
pub struct PlatformDetector {
    cached_capabilities: Option<PlatformCapabilities>,
}

impl PlatformDetector {
    /// Create new platform detector
    pub fn new() -> Self {
        Self {
            cached_capabilities: None,
        }
    }

    /// Detect platform capabilities (cached after first call)
    pub fn detect(&mut self) -> Result<PlatformCapabilities, Error> {
        if let Some(capabilities) = self.cached_capabilities {
            return Ok(capabilities);
        }

        let capabilities = self.detect_capabilities()?;
        self.cached_capabilities = Some(capabilities);
        Ok(capabilities)
    }

    /// Force re-detection of capabilities
    pub fn refresh(&mut self) -> Result<PlatformCapabilities, Error> {
        self.cached_capabilities = None;
        self.detect()
    }

    /// Detect capabilities from current platform
    fn detect_capabilities(&self) -> Result<PlatformCapabilities, Error> {
        let memory = self.detect_memory_capabilities()?;
        let sync = self.detect_sync_capabilities()?;
        let security = self.detect_security_capabilities()?;
        let realtime = self.detect_realtime_capabilities()?;

        Ok(PlatformCapabilities {
            memory,
            sync,
            security,
            realtime,
        })
    }

    /// Detect memory management capabilities
    #[allow(unreachable_code)]
    fn detect_memory_capabilities(&self) -> Result<MemoryCapabilities, Error> {
        #[cfg(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto")
        ))]
        {
            // POSIX-style platforms
            return Ok(MemoryCapabilities {
                dynamic_allocation:     true,
                memory_protection:      true,
                guard_pages:            true,
                hardware_tagging:       self.detect_hardware_tagging(),
                max_memory:             self.detect_max_memory(),
                allocation_granularity: self.detect_page_size(),
            });
        }

        #[cfg(feature = "platform-zephyr")]
        {
            // Real-time embedded platform
            return Ok(MemoryCapabilities {
                dynamic_allocation:     false,           // Binary std/no_std choice
                memory_protection:      true,            // Memory domains provide protection
                guard_pages:            true,            // Guard regions supported
                hardware_tagging:       false,           // Not typical in embedded
                max_memory:             Some(64 * 1024), // Typical embedded limit
                allocation_granularity: 32,              // Typical alignment
            });
        }

        #[cfg(feature = "platform-tock")]
        {
            // Security-first platform
            return Ok(MemoryCapabilities {
                dynamic_allocation:     false,           // Grant-based, not dynamic
                memory_protection:      true,            // MPU provides protection
                guard_pages:            false,           // Not applicable to grants
                hardware_tagging:       false,           // Focus on isolation, not tagging
                max_memory:             Some(16 * 1024), // Very limited embedded memory
                allocation_granularity: 32,              // MPU alignment requirement
            });
        }

        #[cfg(not(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto"),
            feature = "platform-zephyr",
            feature = "platform-tock"
        )))]
        {
            // Fallback - minimal capabilities
            return Ok(MemoryCapabilities {
                dynamic_allocation:     false,
                memory_protection:      false,
                guard_pages:            false,
                hardware_tagging:       false,
                max_memory:             Some(4096),
                allocation_granularity: 1,
            });
        }

        // Unreachable but needed for exhaustiveness
        #[allow(unreachable_code)]
        Err(Error::runtime_execution_error("Platform detection failed"))
    }

    /// Detect synchronization capabilities
    #[allow(unreachable_code)]
    fn detect_sync_capabilities(&self) -> Result<SyncCapabilities, Error> {
        #[cfg(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto")
        ))]
        {
            // POSIX-style platforms
            return Ok(SyncCapabilities {
                futex_support:      true,
                cross_process_sync: true,
                timeout_support:    true,
                hardware_atomics:   true,
                max_waiters:        None, // Typically unlimited
            });
        }

        #[cfg(feature = "platform-zephyr")]
        {
            // Real-time embedded platform
            return Ok(SyncCapabilities {
                futex_support:      true,  // Zephyr has futex
                cross_process_sync: false, // Single-process system
                timeout_support:    true,
                hardware_atomics:   self.detect_embedded_atomics(),
                max_waiters:        Some(32), // Limited by memory
            });
        }

        #[cfg(feature = "platform-tock")]
        {
            // Security-first platform
            return Ok(SyncCapabilities {
                futex_support:      false, // No traditional futex
                cross_process_sync: true,  // IPC-based
                timeout_support:    true,  // Timer-based
                hardware_atomics:   self.detect_embedded_atomics(),
                max_waiters:        Some(8), // Very limited
            });
        }

        #[cfg(not(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto"),
            feature = "platform-zephyr",
            feature = "platform-tock"
        )))]
        {
            // Fallback - minimal capabilities
            return Ok(SyncCapabilities {
                futex_support:      false,
                cross_process_sync: false,
                timeout_support:    false,
                hardware_atomics:   false,
                max_waiters:        Some(1),
            });
        }

        // Unreachable but needed for exhaustiveness
        #[allow(unreachable_code)]
        Err(Error::runtime_execution_error("Platform detection failed"))
    }

    /// Detect security capabilities
    fn detect_security_capabilities(&self) -> Result<SecurityCapabilities, Error> {
        #[cfg(feature = "platform-tock")]
        {
            // Security-first platform (Tock OS)
            Ok(SecurityCapabilities {
                hardware_isolation:  true,  // MPU-based
                process_isolation:   true,  // Core feature
                capability_security: true,  // Grant system
                formal_verification: false, // Depends on specific implementation
                trusted_execution:   false, // Typically not TEE
            })
        }

        #[cfg(all(
            any(
                all(feature = "platform-linux", target_os = "linux"),
                all(feature = "platform-macos", target_os = "macos")
            ),
            not(feature = "platform-tock")
        ))]
        {
            // POSIX platforms (Linux/macOS)
            Ok(SecurityCapabilities {
                hardware_isolation:  true,  // MMU-based
                process_isolation:   true,  // OS feature
                capability_security: false, // Not typical
                formal_verification: false,
                trusted_execution:   self.detect_tee_support(),
            })
        }

        #[cfg(all(
            feature = "platform-qnx",
            target_os = "nto",
            not(feature = "platform-tock")
        ))]
        {
            // QNX - safety-critical RTOS
            Ok(SecurityCapabilities {
                hardware_isolation:  true, // MMU/MPU
                process_isolation:   true, // Strong isolation
                capability_security: false,
                formal_verification: true, // QNX can be formally verified
                trusted_execution:   false,
            })
        }

        #[cfg(all(feature = "platform-zephyr", not(feature = "platform-tock")))]
        {
            // Zephyr - real-time embedded
            Ok(SecurityCapabilities {
                hardware_isolation:  true,  // Memory domains
                process_isolation:   false, // Single process
                capability_security: false,
                formal_verification: false,
                trusted_execution:   false,
            })
        }

        #[cfg(not(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto"),
            feature = "platform-zephyr",
            feature = "platform-tock"
        )))]
        {
            // Fallback - no security features
            Ok(SecurityCapabilities {
                hardware_isolation:  false,
                process_isolation:   false,
                capability_security: false,
                formal_verification: false,
                trusted_execution:   false,
            })
        }
    }

    /// Detect real-time capabilities
    fn detect_realtime_capabilities(&self) -> Result<RealtimeCapabilities, Error> {
        #[cfg(feature = "platform-zephyr")]
        {
            // Real-time embedded platform
            Ok(RealtimeCapabilities {
                deterministic_timing:  true,
                priority_scheduling:   true,
                preemption_control:    true,
                max_interrupt_latency: Some(1_000), // 1 microsecond typical
                deadline_scheduling:   true,
            })
        }

        #[cfg(all(
            feature = "platform-qnx",
            target_os = "nto",
            not(feature = "platform-zephyr")
        ))]
        {
            // QNX - real-time OS
            Ok(RealtimeCapabilities {
                deterministic_timing:  true,
                priority_scheduling:   true,
                preemption_control:    true,
                max_interrupt_latency: Some(5_000), // 5 microseconds typical
                deadline_scheduling:   true,
            })
        }

        #[cfg(all(
            feature = "platform-tock",
            not(feature = "platform-zephyr"),
            not(all(feature = "platform-qnx", target_os = "nto"))
        ))]
        {
            // Tock OS - embedded but not hard real-time
            Ok(RealtimeCapabilities {
                deterministic_timing:  false, // Security over timing
                priority_scheduling:   false,
                preemption_control:    false,
                max_interrupt_latency: Some(10_000), // 10 microseconds
                deadline_scheduling:   false,
            })
        }

        #[cfg(not(any(
            feature = "platform-zephyr",
            all(feature = "platform-qnx", target_os = "nto"),
            feature = "platform-tock"
        )))]
        {
            // General-purpose platforms (Linux/macOS) - limited real-time
            Ok(RealtimeCapabilities {
                deterministic_timing:  false,
                priority_scheduling:   true,
                preemption_control:    false,
                max_interrupt_latency: None, // Highly variable
                deadline_scheduling:   false,
            })
        }
    }

    /// Detect hardware tagging support
    #[allow(dead_code)]
    fn detect_hardware_tagging(&self) -> bool {
        #[cfg(all(
            feature = "platform-linux",
            feature = "linux-mte",
            target_arch = "aarch64",
            target_os = "linux"
        ))]
        {
            // ARM64 MTE support available
            true
        }

        #[cfg(not(all(
            feature = "platform-linux",
            feature = "linux-mte",
            target_arch = "aarch64",
            target_os = "linux"
        )))]
        {
            false
        }
    }

    /// Detect maximum available memory
    #[allow(dead_code)]
    fn detect_max_memory(&self) -> Option<usize> {
        // This would typically query the OS for available memory
        // For now, return reasonable defaults based on platform
        #[cfg(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos")
        ))]
        {
            Some(1024 * 1024 * 1024) // 1GB default
        }

        #[cfg(all(feature = "platform-qnx", target_os = "nto"))]
        {
            Some(512 * 1024 * 1024) // 512MB for embedded QNX
        }

        #[cfg(not(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto")
        )))]
        {
            Some(64 * 1024) // 64KB for embedded systems
        }
    }

    /// Binary std/no_std choice
    #[allow(dead_code)]
    fn detect_page_size(&self) -> usize {
        #[cfg(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto")
        ))]
        {
            4096 // Standard page size
        }

        #[cfg(not(any(
            all(feature = "platform-linux", target_os = "linux"),
            all(feature = "platform-macos", target_os = "macos"),
            all(feature = "platform-qnx", target_os = "nto")
        )))]
        {
            32 // Typical embedded alignment
        }
    }

    /// Detect embedded atomic operation support
    #[allow(dead_code)]
    fn detect_embedded_atomics(&self) -> bool {
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        {
            true // ARM has good atomic support
        }

        #[cfg(target_arch = "riscv32")]
        {
            true // RISC-V atomic extension
        }

        #[cfg(not(any(target_arch = "arm", target_arch = "aarch64", target_arch = "riscv32")))]
        {
            false
        }
    }

    /// Detect TEE (Trusted Execution Environment) support
    #[allow(dead_code)]
    fn detect_tee_support(&self) -> bool {
        // This would check for TrustZone, Intel SGX, AMD SEV, etc.
        // For now, conservatively return false
        false
    }
}

impl Default for PlatformDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for platform capability queries
impl PlatformCapabilities {
    /// Check if platform supports the minimum requirements for WebAssembly
    /// runtime
    pub fn supports_wasm_runtime(&self) -> bool {
        // Binary std/no_std choice
        // synchronization
        (self.memory.dynamic_allocation || self.memory.max_memory.is_some())
            && (self.sync.futex_support || self.sync.cross_process_sync)
    }

    /// Check if platform is suitable for security-critical applications
    pub fn is_security_suitable(&self) -> bool {
        self.security.hardware_isolation && self.security.process_isolation
    }

    /// Check if platform provides real-time guarantees
    pub fn is_realtime_suitable(&self) -> bool {
        self.realtime.deterministic_timing && self.realtime.priority_scheduling
    }

    /// Get recommended paradigm based on capabilities
    pub fn recommended_paradigm(&self) -> &'static str {
        if self.is_security_suitable() {
            "SecurityFirst"
        } else if self.is_realtime_suitable() {
            "RealTime"
        } else {
            "Posix"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detector_creation() {
        let detector = PlatformDetector::new();
        assert!(detector.cached_capabilities.is_none());
    }

    #[test]
    fn test_capability_detection() {
        let mut detector = PlatformDetector::new();
        let capabilities = detector.detect().unwrap();

        // Basic sanity checks
        assert!(capabilities.memory.allocation_granularity > 0);

        // Test caching
        let capabilities2 = detector.detect().unwrap();
        assert_eq!(
            capabilities.memory.allocation_granularity,
            capabilities2.memory.allocation_granularity
        );
    }

    #[test]
    fn test_wasm_runtime_support() {
        let mut detector = PlatformDetector::new();
        let capabilities = detector.detect().unwrap();

        // Should support WebAssembly runtime on any reasonable platform
        assert!(capabilities.supports_wasm_runtime());
    }

    #[test]
    fn test_paradigm_recommendation() {
        let mut detector = PlatformDetector::new();
        let capabilities = detector.detect().unwrap();

        let paradigm = capabilities.recommended_paradigm();
        assert!(paradigm == "SecurityFirst" || paradigm == "RealTime" || paradigm == "Posix");
    }

    #[test]
    fn test_refresh_detection() {
        let mut detector = PlatformDetector::new();

        // Initial detection
        let _caps1 = detector.detect().unwrap();
        assert!(detector.cached_capabilities.is_some());

        // Refresh should work
        let _caps2 = detector.refresh().unwrap();
        assert!(detector.cached_capabilities.is_some());
    }
}
