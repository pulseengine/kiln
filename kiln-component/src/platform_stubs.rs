// Platform stubs for component module development
// These provide the interface to the platform module's types

use crate::foundation_stubs::AsilLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformId {
    Linux,
    QNX,
    Embedded,
    MacOS,
    Windows,
}

#[derive(Debug, Clone)]
pub struct ComprehensivePlatformLimits {
    pub platform_id: PlatformId,
    pub max_total_memory: usize,
    pub max_wasm_linear_memory: usize,
    pub max_stack_bytes: usize,
    pub max_components: usize,
    pub max_component_instances: usize,
    pub max_debug_overhead: usize,
    pub asil_level: AsilLevel,
}

impl Default for ComprehensivePlatformLimits {
    fn default() -> Self {
        Self {
            platform_id: PlatformId::Linux,
            max_total_memory: 1024 * 1024 * 1024,      // 1GB
            max_wasm_linear_memory: 512 * 1024 * 1024, // 512MB
            max_stack_bytes: 1024 * 1024,              // 1MB
            max_components: 256,
            max_component_instances: 1024,
            max_debug_overhead: 64 * 1024 * 1024, // 64MB
            asil_level: AsilLevel::QM,
        }
    }
}

pub trait ComprehensiveLimitProvider: Send + Sync {
    fn discover_limits(
        &self,
    ) -> core::result::Result<ComprehensivePlatformLimits, wrt_error::Error>;
    fn platform_id(&self) -> PlatformId;
}

pub struct DefaultLimitProvider;

impl ComprehensiveLimitProvider for DefaultLimitProvider {
    fn discover_limits(
        &self,
    ) -> core::result::Result<ComprehensivePlatformLimits, wrt_error::Error> {
        Ok(ComprehensivePlatformLimits::default())
    }

    fn platform_id(&self) -> PlatformId {
        PlatformId::Linux
    }
}

// Debug limits stub
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugLevel {
    None = 0,
    BasicProfile = 1,
    FullDebug = 2,
}

pub struct PlatformDebugLimits {
    pub max_debug_sections: usize,
    pub max_dwarf_section_size: usize,
    pub max_breakpoints: usize,
    pub max_stack_traces: usize,
    pub debug_level: DebugLevel,
}

impl PlatformDebugLimits {
    pub fn from_platform_limits(
        limits: &ComprehensivePlatformLimits,
        debug_level: DebugLevel,
    ) -> Self {
        let debug_overhead = match debug_level {
            DebugLevel::None => 0,
            DebugLevel::BasicProfile => limits.max_total_memory / 50,
            DebugLevel::FullDebug => limits.max_total_memory / 10,
        };

        Self {
            max_debug_sections: if debug_overhead > 0 { 64 } else { 0 },
            max_dwarf_section_size: 1024 * 1024,
            max_breakpoints: if debug_level >= DebugLevel::FullDebug { 10000 } else { 100 },
            max_stack_traces: if debug_level >= DebugLevel::FullDebug { 1000 } else { 10 },
            debug_level,
        }
    }
}

impl PartialOrd for DebugLevel {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DebugLevel {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
