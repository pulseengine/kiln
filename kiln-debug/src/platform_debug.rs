//! Platform-aware debug infrastructure
//!
//! Provides debug capabilities that adapt to platform limitations and
//! resources.

use wrt_error::Error;

// Stub imports for platform limits - will be replaced during integration
mod platform_stubs {
    /// Comprehensive platform limits configuration
    ///
    /// This structure defines platform-specific resource limits that constrain
    /// system operation and debug capabilities. These limits are used to ensure
    /// that debug operations do not exceed platform resource constraints.
    pub struct ComprehensivePlatformLimits {
        /// Maximum total memory available on the platform (bytes)
        pub max_total_memory: usize,
        /// Maximum memory overhead allowed for debug features (bytes)
        pub max_debug_overhead: usize,
        /// Platform identifier for platform-specific optimizations
        pub platform_id: PlatformId,
    }

    /// Platform identifier enumeration
    ///
    /// Identifies the target platform to enable platform-specific optimizations
    /// and resource management strategies.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PlatformId {
        /// Linux-based platforms with standard resources
        Linux,
        /// QNX real-time operating system
        QNX,
        /// macOS platforms with Darwin kernel
        MacOS,
        /// VxWorks real-time operating system
        VxWorks,
        /// Zephyr RTOS for embedded systems
        Zephyr,
        /// Tock secure embedded operating system
        Tock,
        /// Generic embedded platforms with limited resources
        Embedded,
        /// Unknown or unspecified platform
        Unknown,
    }

    impl Default for ComprehensivePlatformLimits {
        fn default() -> Self {
            Self {
                max_total_memory: 1024 * 1024 * 1024,
                max_debug_overhead: 64 * 1024 * 1024,
                platform_id: PlatformId::Unknown,
            }
        }
    }
}

pub use platform_stubs::{ComprehensivePlatformLimits, PlatformId};

/// Debug capability levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugLevel {
    /// No debug capabilities
    None = 0,
    /// Basic profiling only
    BasicProfile = 1,
    /// Full debug with breakpoints and inspection
    FullDebug = 2,
}

/// Platform-specific debug limits
#[derive(Debug, Clone)]
pub struct PlatformDebugLimits {
    /// Maximum number of debug sections
    pub max_debug_sections: usize,
    /// Maximum size of DWARF section in bytes
    pub max_dwarf_section_size: usize,
    /// Maximum number of breakpoints
    pub max_breakpoints: usize,
    /// Maximum number of stack traces
    pub max_stack_traces: usize,
    /// Debug level supported on this platform
    pub debug_level: DebugLevel,
}

impl PlatformDebugLimits {
    /// Create debug limits from platform limits and desired debug level
    pub fn from_platform_limits(
        limits: &ComprehensivePlatformLimits,
        debug_level: DebugLevel,
    ) -> Self {
        let debug_overhead = match debug_level {
            DebugLevel::None => 0,
            DebugLevel::BasicProfile => limits.max_total_memory / 50, // 2% overhead
            DebugLevel::FullDebug => limits.max_total_memory / 10,    // 10% overhead
        };

        // Cap the debug overhead at the platform limit
        let debug_overhead = debug_overhead.min(limits.max_debug_overhead);

        Self {
            max_debug_sections: if debug_overhead > 0 { 64 } else { 0 },
            max_dwarf_section_size: if debug_overhead > 1024 * 1024 {
                1024 * 1024
            } else {
                debug_overhead / 2
            },
            max_breakpoints: match debug_level {
                DebugLevel::None => 0,
                DebugLevel::BasicProfile => 10,
                DebugLevel::FullDebug => {
                    // Scale breakpoints based on available memory
                    if limits.max_total_memory > 256 * 1024 * 1024 {
                        10000 // High-memory systems
                    } else if limits.max_total_memory > 64 * 1024 * 1024 {
                        1000 // Medium-memory systems
                    } else {
                        100 // Low-memory systems
                    }
                },
            },
            max_stack_traces: match debug_level {
                DebugLevel::None => 0,
                DebugLevel::BasicProfile => 10,
                DebugLevel::FullDebug => {
                    // Scale stack traces based on available memory
                    if limits.max_total_memory > 256 * 1024 * 1024 {
                        1000 // High-memory systems
                    } else if limits.max_total_memory > 64 * 1024 * 1024 {
                        100 // Medium-memory systems
                    } else {
                        10 // Low-memory systems
                    }
                },
            },
            debug_level,
        }
    }

    /// Create minimal debug limits for embedded systems
    pub fn minimal_embedded() -> Self {
        Self {
            max_debug_sections: 8,
            max_dwarf_section_size: 32 * 1024, // 32KB
            max_breakpoints: 4,
            max_stack_traces: 4,
            debug_level: DebugLevel::BasicProfile,
        }
    }

    /// Create limits for production systems (minimal debugging)
    pub fn production() -> Self {
        Self {
            max_debug_sections: 0,
            max_dwarf_section_size: 0,
            max_breakpoints: 0,
            max_stack_traces: 1, // Allow minimal crash reporting
            debug_level: DebugLevel::None,
        }
    }

    /// Create limits for development systems (full debugging)
    pub fn development(memory_size: usize) -> Self {
        let mock_limits = ComprehensivePlatformLimits {
            max_total_memory: memory_size,
            max_debug_overhead: memory_size / 8, // 12.5% for debug
            platform_id: PlatformId::Unknown,
        };

        Self::from_platform_limits(&mock_limits, DebugLevel::FullDebug)
    }
}

/// Platform-aware debug manager
pub struct PlatformDebugManager {
    /// Debug limits for this platform
    limits: PlatformDebugLimits,
    /// Current memory usage for debug features
    current_debug_memory: usize,
    /// Number of active debug sections
    active_sections: usize,
    /// Number of active breakpoints
    active_breakpoints: usize,
    /// Number of active stack traces
    active_stack_traces: usize,
}

impl PlatformDebugManager {
    /// Create new platform debug manager
    pub fn new(limits: PlatformDebugLimits) -> Self {
        Self {
            limits,
            current_debug_memory: 0,
            active_sections: 0,
            active_breakpoints: 0,
            active_stack_traces: 0,
        }
    }

    /// Check if we can allocate more debug memory
    pub fn can_allocate_debug_memory(&self, size: usize) -> bool {
        match self.limits.debug_level {
            DebugLevel::None => false,
            _ => self.current_debug_memory + size <= self.limits.max_dwarf_section_size,
        }
    }

    /// Allocate debug memory if possible
    pub fn allocate_debug_memory(&mut self, size: usize) -> Result<(), Error> {
        if !self.can_allocate_debug_memory(size) {
            return Err(Error::resource_exhausted("Debug memory limit exceeded"));
        }

        self.current_debug_memory += size;
        Ok(())
    }

    /// Free debug memory
    pub fn free_debug_memory(&mut self, size: usize) {
        self.current_debug_memory = self.current_debug_memory.saturating_sub(size);
    }

    /// Check if we can add more debug sections
    pub fn can_add_debug_section(&self) -> bool {
        match self.limits.debug_level {
            DebugLevel::None => false,
            _ => self.active_sections < self.limits.max_debug_sections,
        }
    }

    /// Add a debug section
    pub fn add_debug_section(&mut self) -> Result<(), Error> {
        if !self.can_add_debug_section() {
            return Err(Error::resource_exhausted("Debug section limit exceeded"));
        }

        self.active_sections += 1;
        Ok(())
    }

    /// Remove a debug section
    pub fn remove_debug_section(&mut self) {
        self.active_sections = self.active_sections.saturating_sub(1);
    }

    /// Check if we can add more breakpoints
    pub fn can_add_breakpoint(&self) -> bool {
        self.active_breakpoints < self.limits.max_breakpoints
    }

    /// Add a breakpoint
    pub fn add_breakpoint(&mut self) -> Result<(), Error> {
        if !self.can_add_breakpoint() {
            return Err(Error::resource_exhausted("Breakpoint limit exceeded"));
        }

        self.active_breakpoints += 1;
        Ok(())
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&mut self) {
        self.active_breakpoints = self.active_breakpoints.saturating_sub(1);
    }

    /// Check if we can add more stack traces
    pub fn can_add_stack_trace(&self) -> bool {
        self.active_stack_traces < self.limits.max_stack_traces
    }

    /// Add a stack trace
    pub fn add_stack_trace(&mut self) -> Result<(), Error> {
        if !self.can_add_stack_trace() {
            return Err(Error::resource_exhausted("Stack trace limit exceeded"));
        }

        self.active_stack_traces += 1;
        Ok(())
    }

    /// Remove a stack trace
    pub fn remove_stack_trace(&mut self) {
        self.active_stack_traces = self.active_stack_traces.saturating_sub(1);
    }

    /// Get current debug memory usage
    pub fn debug_memory_usage(&self) -> usize {
        self.current_debug_memory
    }

    /// Get available debug memory
    pub fn available_debug_memory(&self) -> usize {
        self.limits.max_dwarf_section_size.saturating_sub(self.current_debug_memory)
    }

    /// Get debug level
    pub fn debug_level(&self) -> DebugLevel {
        self.limits.debug_level
    }

    /// Get debug limits
    pub fn limits(&self) -> &PlatformDebugLimits {
        &self.limits
    }

    /// Reset all debug resources
    pub fn reset(&mut self) {
        self.current_debug_memory = 0;
        self.active_sections = 0;
        self.active_breakpoints = 0;
        self.active_stack_traces = 0;
    }
}

/// Platform debug configuration builder
pub struct PlatformDebugConfigBuilder {
    debug_level: DebugLevel,
    memory_override: Option<usize>,
    breakpoint_override: Option<usize>,
    stack_trace_override: Option<usize>,
}

impl PlatformDebugConfigBuilder {
    /// Create new debug config builder
    pub fn new() -> Self {
        Self {
            debug_level: DebugLevel::BasicProfile,
            memory_override: None,
            breakpoint_override: None,
            stack_trace_override: None,
        }
    }

    /// Set debug level
    pub fn with_debug_level(mut self, level: DebugLevel) -> Self {
        self.debug_level = level;
        self
    }

    /// Override maximum debug memory
    pub fn with_max_debug_memory(mut self, size: usize) -> Self {
        self.memory_override = Some(size);
        self
    }

    /// Override maximum breakpoints
    pub fn with_max_breakpoints(mut self, count: usize) -> Self {
        self.breakpoint_override = Some(count);
        self
    }

    /// Override maximum stack traces
    pub fn with_max_stack_traces(mut self, count: usize) -> Self {
        self.stack_trace_override = Some(count);
        self
    }

    /// Build debug limits from platform limits
    pub fn build(self, platform_limits: &ComprehensivePlatformLimits) -> PlatformDebugLimits {
        let mut limits =
            PlatformDebugLimits::from_platform_limits(platform_limits, self.debug_level);

        if let Some(memory) = self.memory_override {
            limits.max_dwarf_section_size = memory;
        }

        if let Some(breakpoints) = self.breakpoint_override {
            limits.max_breakpoints = breakpoints;
        }

        if let Some(stack_traces) = self.stack_trace_override {
            limits.max_stack_traces = stack_traces;
        }

        limits
    }
}

impl Default for PlatformDebugConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_limits_from_platform() {
        let platform_limits = ComprehensivePlatformLimits {
            max_total_memory: 256 * 1024 * 1024,  // 256MB
            max_debug_overhead: 25 * 1024 * 1024, // 25MB
            platform_id: PlatformId::Linux,
        };

        let debug_limits =
            PlatformDebugLimits::from_platform_limits(&platform_limits, DebugLevel::FullDebug);

        assert_eq!(debug_limits.debug_level, DebugLevel::FullDebug);
        assert!(debug_limits.max_breakpoints > 0);
        assert!(debug_limits.max_stack_traces > 0);
        assert!(debug_limits.max_dwarf_section_size > 0);
    }

    #[test]
    fn test_minimal_embedded_limits() {
        let limits = PlatformDebugLimits::minimal_embedded();

        assert_eq!(limits.debug_level, DebugLevel::BasicProfile);
        assert_eq!(limits.max_debug_sections, 8);
        assert_eq!(limits.max_breakpoints, 4);
        assert_eq!(limits.max_stack_traces, 4);
        assert_eq!(limits.max_dwarf_section_size, 32 * 1024);
    }

    #[test]
    fn test_production_limits() {
        let limits = PlatformDebugLimits::production();

        assert_eq!(limits.debug_level, DebugLevel::None);
        assert_eq!(limits.max_debug_sections, 0);
        assert_eq!(limits.max_breakpoints, 0);
        assert_eq!(limits.max_stack_traces, 1); // Minimal crash reporting
    }

    #[test]
    fn test_debug_manager() {
        let limits = PlatformDebugLimits::development(64 * 1024 * 1024);
        let mut manager = PlatformDebugManager::new(limits);

        // Test memory allocation
        assert!(manager.can_allocate_debug_memory(1024));
        assert!(manager.allocate_debug_memory(1024).is_ok());
        assert_eq!(manager.debug_memory_usage(), 1024);

        // Test breakpoint management
        assert!(manager.can_add_breakpoint());
        assert!(manager.add_breakpoint().is_ok());
        assert_eq!(manager.active_breakpoints, 1);

        // Test section management
        assert!(manager.can_add_debug_section());
        assert!(manager.add_debug_section().is_ok());
        assert_eq!(manager.active_sections, 1);

        // Test reset
        manager.reset();
        assert_eq!(manager.debug_memory_usage(), 0);
        assert_eq!(manager.active_breakpoints, 0);
        assert_eq!(manager.active_sections, 0);
    }

    #[test]
    fn test_config_builder() {
        let platform_limits = ComprehensivePlatformLimits::default();

        let limits = PlatformDebugConfigBuilder::new()
            .with_debug_level(DebugLevel::FullDebug)
            .with_max_debug_memory(2 * 1024 * 1024)
            .with_max_breakpoints(50)
            .build(&platform_limits);

        assert_eq!(limits.debug_level, DebugLevel::FullDebug);
        assert_eq!(limits.max_dwarf_section_size, 2 * 1024 * 1024);
        assert_eq!(limits.max_breakpoints, 50);
    }
}
