//! Streaming WebAssembly validator with platform limit checking
//!
//! Provides single-pass WASM validation with immediate limit checking against
//! platform capabilities.

use wrt_error::{Error, ErrorCategory, codes};
use wrt_foundation::{
    NoStdProvider,
    bounded::BoundedVec,
    traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream},
    verification::Checksum,
};

#[cfg(feature = "std")]
extern crate std;

// Stub imports for platform limits - will be replaced during integration
mod platform_stubs {
    /// Comprehensive platform limits configuration
    ///
    /// This structure defines platform-specific resource limits that constrain
    /// WebAssembly execution and validation. These limits ensure that WASM
    /// modules do not exceed platform capabilities.
    pub struct ComprehensivePlatformLimits {
        /// Maximum total memory available on the platform (bytes)
        pub max_total_memory: usize,
        /// Maximum WebAssembly linear memory allowed (bytes)
        pub max_wasm_linear_memory: usize,
        /// Maximum stack size in bytes for function calls
        pub max_stack_bytes: usize,
        /// Maximum number of components that can be loaded simultaneously
        pub max_components: usize,
        /// Platform identifier for platform-specific optimizations
        pub platform_id: PlatformId,
    }

    /// Platform identifier enumeration
    ///
    /// Identifies the target platform to enable platform-specific optimizations
    /// and resource management strategies for WebAssembly execution.
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
                max_wasm_linear_memory: 256 * 1024 * 1024,
                max_stack_bytes: 1024 * 1024,
                max_components: 256,
                platform_id: PlatformId::Unknown,
            }
        }
    }
}

// Stub imports for Agent D's runtime work - will be replaced during integration
mod runtime_stubs {
    /// WebAssembly module configuration
    ///
    /// Contains resource usage information extracted from a WASM module
    /// during validation, used for runtime resource planning.
    #[derive(Debug, Clone)]
    pub struct WasmConfiguration {
        /// Initial memory size in WASM pages (64KB each)
        pub initial_memory: u32,
        /// Maximum memory size in WASM pages, if specified
        pub maximum_memory: Option<u32>,
        /// Estimated stack usage in bytes for function calls
        pub estimated_stack_usage: u32,
        /// Total number of functions defined in the module
        pub function_count: u32,
        /// Total number of imports required by the module
        pub import_count: u32,
        /// Total number of exports provided by the module
        pub export_count: u32,
    }
}

pub use platform_stubs::{ComprehensivePlatformLimits, PlatformId};
pub use runtime_stubs::WasmConfiguration;

/// WASM section types for validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    /// Custom section
    #[default]
    Custom,
    /// Type section
    Type,
    /// Import section
    Import,
    /// Function section
    Function,
    /// Table section
    Table,
    /// Memory section
    Memory(MemorySection),
    /// Global section
    Global,
    /// Export section
    Export,
    /// Start section
    Start,
    /// Element section
    Element,
    /// Code section
    Code(CodeSection),
    /// Data section
    Data,
}

// Trait implementations for Section to work with BoundedVec
impl Checksummable for Section {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Simple checksum based on discriminant
        let discriminant = match self {
            Section::Custom => 0u8,
            Section::Type => 1u8,
            Section::Import => 2u8,
            Section::Function => 3u8,
            Section::Table => 4u8,
            Section::Memory(_) => 5u8,
            Section::Global => 6u8,
            Section::Export => 7u8,
            Section::Start => 8u8,
            Section::Element => 9u8,
            Section::Code(_) => 10u8,
            Section::Data => 11u8,
        };
        checksum.update(discriminant);
    }
}

impl ToBytes for Section {
    fn serialized_size(&self) -> usize {
        match self {
            Section::Memory(mem) => 1 + 4 + if mem.maximum.is_some() { 4 } else { 0 },
            Section::Code(_code) => 1 + 4 + 4,
            _ => 1, // Just the discriminant
        }
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        // Write section discriminant
        let discriminant = match self {
            Section::Custom => 0u8,
            Section::Type => 1u8,
            Section::Import => 2u8,
            Section::Function => 3u8,
            Section::Table => 4u8,
            Section::Memory(_) => 5u8,
            Section::Global => 6u8,
            Section::Export => 7u8,
            Section::Start => 8u8,
            Section::Element => 9u8,
            Section::Code(_) => 10u8,
            Section::Data => 11u8,
        };
        writer.write_u8(discriminant)?;

        // Write section-specific data
        match self {
            Section::Memory(mem) => {
                writer.write_u32_le(mem.initial)?;
                if let Some(max) = mem.maximum {
                    writer.write_u32_le(max)?;
                }
            },
            Section::Code(code) => {
                writer.write_u32_le(code.function_count)?;
                writer.write_u32_le(code.estimated_stack_usage)?;
            },
            _ => {}, // No additional data
        }

        Ok(())
    }
}

impl FromBytes for Section {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let discriminant = reader.read_u8()?;
        Ok(match discriminant {
            0 => Section::Custom,
            1 => Section::Type,
            2 => Section::Import,
            3 => Section::Function,
            4 => Section::Table,
            5 => {
                let initial = reader.read_u32_le()?;
                let maximum =
                    if reader.remaining_len() >= 4 { Some(reader.read_u32_le()?) } else { None };
                Section::Memory(MemorySection { initial, maximum })
            },
            6 => Section::Global,
            7 => Section::Export,
            8 => Section::Start,
            9 => Section::Element,
            10 => {
                let function_count = reader.read_u32_le()?;
                let estimated_stack_usage = reader.read_u32_le()?;
                Section::Code(CodeSection {
                    function_count,
                    estimated_stack_usage,
                })
            },
            11 => Section::Data,
            _ => Section::Custom, // Default fallback
        })
    }
}

/// Memory section information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySection {
    /// Initial memory size in pages (64KB each)
    pub initial: u32,
    /// Maximum memory size in pages (optional)
    pub maximum: Option<u32>,
}

/// Code section information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSection {
    /// Number of functions
    pub function_count: u32,
    /// Estimated stack usage based on function analysis
    pub estimated_stack_usage: u32,
}

/// WASM requirements derived from validation
#[derive(Debug, Clone)]
pub struct WasmRequirements {
    /// Required linear memory in bytes
    pub required_memory: usize,
    /// Estimated stack usage in bytes
    pub estimated_stack_usage: usize,
    /// Number of functions
    pub function_count: u32,
    /// Number of imports
    pub import_count: u32,
    /// Number of exports
    pub export_count: u32,
    /// Whether the module uses multiple memories
    pub uses_multiple_memories: bool,
}

impl Default for WasmRequirements {
    fn default() -> Self {
        Self {
            required_memory: 0,
            estimated_stack_usage: 8192, // 8KB default
            function_count: 0,
            import_count: 0,
            export_count: 0,
            uses_multiple_memories: false,
        }
    }
}

/// Streaming WebAssembly validator
pub struct StreamingWasmValidator {
    /// Platform limits to validate against
    platform_limits: ComprehensivePlatformLimits,
    /// Current WASM requirements being built
    requirements: WasmRequirements,
    /// Validation state
    state: ValidationState,
}

/// Validation state tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationState {
    /// Waiting for header
    Header,
    /// Processing sections
    Sections,
    /// Validation complete
    Complete,
    /// Validation failed
    Failed,
}

impl StreamingWasmValidator {
    /// Create new streaming validator
    pub fn new(platform_limits: ComprehensivePlatformLimits) -> Self {
        Self {
            platform_limits,
            requirements: WasmRequirements::default(),
            state: ValidationState::Header,
        }
    }

    /// Validate WebAssembly module in single pass with immediate limit checking
    pub fn validate_single_pass(&mut self, wasm_bytes: &[u8]) -> Result<WasmConfiguration, Error> {
        // Reset state
        self.state = ValidationState::Header;
        self.requirements = WasmRequirements::default();

        // Validate header first
        self.validate_header(wasm_bytes)?;
        self.state = ValidationState::Sections;

        // Parse and validate sections
        let sections = self.parse_sections(wasm_bytes)?;

        for section in sections.iter() {
            self.validate_section(&section)?;
        }

        // Final validation against platform limits
        self.validate_final_requirements()?;

        self.state = ValidationState::Complete;

        // Create configuration from validated requirements
        Ok(WasmConfiguration {
            initial_memory: (self.requirements.required_memory / 65536) as u32,
            maximum_memory: None, // Will be set based on platform limits
            estimated_stack_usage: self.requirements.estimated_stack_usage as u32,
            function_count: self.requirements.function_count,
            import_count: self.requirements.import_count,
            export_count: self.requirements.export_count,
        })
    }

    /// Validate WebAssembly header
    fn validate_header(&self, wasm_bytes: &[u8]) -> Result<(), Error> {
        if wasm_bytes.len() < 8 {
            return Err(Error::parse_error("WASM module too small for header"));
        }

        // Check magic number (0x00 0x61 0x73 0x6D)
        if wasm_bytes[0..4] != [0x00, 0x61, 0x73, 0x6D] {
            return Err(Error::parse_error("Invalid WASM magic number"));
        }

        // Check version (0x01 0x00 0x00 0x00)
        if wasm_bytes[4..8] != [0x01, 0x00, 0x00, 0x00] {
            return Err(Error::parse_error("Unsupported WASM version"));
        }

        Ok(())
    }

    /// Parse sections from WASM module
    fn parse_sections(
        &self,
        wasm_bytes: &[u8],
    ) -> Result<BoundedVec<Section, 32, wrt_foundation::NoStdProvider<2048>>, Error> {
        let provider = wrt_foundation::safe_managed_alloc!(
            2048,
            wrt_foundation::budget_aware_provider::CrateId::Decoder
        )?;
        let mut sections = BoundedVec::new(provider)
            .map_err(|_| Error::runtime_execution_error("Failed to create section vector"))?;
        let mut offset = 8; // Skip header

        while offset < wasm_bytes.len() {
            if offset + 1 >= wasm_bytes.len() {
                break;
            }

            let section_id = wasm_bytes[offset];
            offset += 1;

            // Read section size (LEB128)
            let (section_size, size_bytes) = self.read_leb128_u32(&wasm_bytes[offset..])?;
            offset += size_bytes;

            if offset + section_size as usize > wasm_bytes.len() {
                return Err(Error::parse_error("Section extends beyond available data"));
            }

            let section_data = &wasm_bytes[offset..offset + section_size as usize];
            let section = self.parse_section_type(section_id, section_data)?;

            if sections.push(section).is_err() {
                return Err(Error::resource_exhausted("Too many sections"));
            }

            offset += section_size as usize;
        }

        Ok(sections)
    }

    /// Parse specific section type
    fn parse_section_type(&self, section_id: u8, section_data: &[u8]) -> Result<Section, Error> {
        match section_id {
            0 => Ok(Section::Custom),
            1 => Ok(Section::Type),
            2 => Ok(Section::Import),
            3 => Ok(Section::Function),
            4 => Ok(Section::Table),
            5 => self.parse_memory_section(section_data),
            6 => Ok(Section::Global),
            7 => Ok(Section::Export),
            8 => Ok(Section::Start),
            9 => Ok(Section::Element),
            10 => self.parse_code_section(section_data),
            11 => Ok(Section::Data),
            _ => Err(Error::parse_error("Unknown section type ")),
        }
    }

    /// Parse memory section
    fn parse_memory_section(&self, section_data: &[u8]) -> Result<Section, Error> {
        if section_data.is_empty() {
            return Err(Error::parse_error("Empty memory section"));
        }

        // Read memory count (should be 1 for MVP)
        let (memory_count, mut offset) = self.read_leb128_u32(section_data)?;

        if memory_count == 0 {
            return Err(Error::parse_error("Memory section with zero memories"));
        }

        if memory_count > 1 {
            // Multiple memories - future feature
            return Err(Error::parse_error("Multiple memories not supported"));
        }

        // Read memory limits
        if offset >= section_data.len() {
            return Err(Error::parse_error("Truncated memory section"));
        }

        let limits_flag = section_data[offset];
        offset += 1;

        let (initial, size_bytes) = self.read_leb128_u32(&section_data[offset..])?;
        offset += size_bytes;

        let maximum = if limits_flag & 0x01 != 0 {
            let (max, _) = self.read_leb128_u32(&section_data[offset..])?;
            Some(max)
        } else {
            None
        };

        Ok(Section::Memory(MemorySection { initial, maximum }))
    }

    /// Parse code section
    fn parse_code_section(&self, section_data: &[u8]) -> Result<Section, Error> {
        if section_data.is_empty() {
            return Ok(Section::Code(CodeSection {
                function_count: 0,
                estimated_stack_usage: 0,
            }));
        }

        let (function_count, _) = self.read_leb128_u32(section_data)?;

        // Estimate stack usage based on function count
        // This is a simplified heuristic - real implementation would analyze function
        // bodies
        let estimated_stack_usage = function_count * 512; // 512 bytes per function estimate

        Ok(Section::Code(CodeSection {
            function_count,
            estimated_stack_usage,
        }))
    }

    /// Validate individual section against platform limits
    fn validate_section(&mut self, section: &Section) -> Result<(), Error> {
        match section {
            Section::Memory(mem) => {
                let required = mem.initial as usize * 65536; // Convert pages to bytes

                if required > self.platform_limits.max_wasm_linear_memory {
                    return Err(Error::resource_exhausted("Memory limit exceeded"));
                }

                self.requirements.required_memory = required;
            },
            Section::Code(code) => {
                if code.estimated_stack_usage as usize > self.platform_limits.max_stack_bytes {
                    return Err(Error::resource_exhausted("Stack size limit exceeded"));
                }

                self.requirements.estimated_stack_usage = code.estimated_stack_usage as usize;
                self.requirements.function_count = code.function_count;
            },
            Section::Import => {
                self.requirements.import_count += 1;
            },
            Section::Export => {
                self.requirements.export_count += 1;
            },
            _ => {
                // Other sections don't have immediate resource implications
            },
        }

        Ok(())
    }

    /// Perform final validation of all requirements
    fn validate_final_requirements(&self) -> Result<(), Error> {
        // Check total memory requirement
        let total_memory_need =
            self.requirements.required_memory + self.requirements.estimated_stack_usage;

        if total_memory_need > self.platform_limits.max_total_memory {
            return Err(Error::resource_exhausted("Total memory limit exceeded"));
        }

        // Check function count limits (platform-specific)
        let max_functions = match self.platform_limits.platform_id {
            PlatformId::Embedded => 256,
            PlatformId::QNX => 1024,
            _ => 10000,
        };

        if self.requirements.function_count > max_functions {
            return Err(Error::resource_exhausted("Function count limit exceeded"));
        }

        Ok(())
    }

    /// Read LEB128 unsigned 32-bit integer
    fn read_leb128_u32(&self, data: &[u8]) -> Result<(u32, usize), Error> {
        let mut result = 0u32;
        let mut shift = 0;
        let mut bytes_read = 0;

        for &byte in data.iter().take(5) {
            // Max 5 bytes for u32
            bytes_read += 1;
            result |= ((byte & 0x7F) as u32) << shift;

            if byte & 0x80 == 0 {
                return Ok((result, bytes_read));
            }

            shift += 7;
            if shift >= 32 {
                return Err(Error::parse_error("LEB128 value too large"));
            }
        }

        Err(Error::parse_error("Truncated LEB128 value "))
    }

    /// Get current validation state
    pub fn state(&self) -> ValidationState {
        self.state
    }

    /// Get current requirements
    pub fn requirements(&self) -> &WasmRequirements {
        &self.requirements
    }
}

/// Platform-aware WASM validator factory
pub struct PlatformWasmValidatorFactory;

impl PlatformWasmValidatorFactory {
    /// Create validator for current platform
    pub fn create_for_platform() -> Result<StreamingWasmValidator, Error> {
        // In a real implementation, this would detect the current platform
        let limits = ComprehensivePlatformLimits::default();
        Ok(StreamingWasmValidator::new(limits))
    }

    /// Create validator with specific limits
    pub fn create_with_limits(limits: ComprehensivePlatformLimits) -> StreamingWasmValidator {
        StreamingWasmValidator::new(limits)
    }

    /// Create validator for embedded platform
    pub fn create_for_embedded(memory_size: usize) -> StreamingWasmValidator {
        let limits = ComprehensivePlatformLimits {
            max_total_memory: memory_size,
            max_wasm_linear_memory: (memory_size * 2) / 3,
            max_stack_bytes: memory_size / 16,
            max_components: 16,
            platform_id: PlatformId::Embedded,
        };
        StreamingWasmValidator::new(limits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let limits = ComprehensivePlatformLimits::default();
        let validator = StreamingWasmValidator::new(limits);
        assert_eq!(validator.state(), ValidationState::Header);
    }

    #[test]
    fn test_header_validation() {
        let limits = ComprehensivePlatformLimits::default();
        let validator = StreamingWasmValidator::new(limits);

        // Valid WASM header
        let valid_header = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(validator.validate_header(&valid_header).is_ok());

        // Invalid magic
        let invalid_magic = [0xFF, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        assert!(validator.validate_header(&invalid_magic).is_err());

        // Invalid version
        let invalid_version = [0x00, 0x61, 0x73, 0x6D, 0x02, 0x00, 0x00, 0x00];
        assert!(validator.validate_header(&invalid_version).is_err());
    }

    #[test]
    fn test_memory_section_parsing() {
        let validator = StreamingWasmValidator::new(ComprehensivePlatformLimits::default());

        // Memory section: count=1, limits=0, initial=1
        let memory_data = [0x01, 0x00, 0x01];
        let section = validator.parse_memory_section(&memory_data).unwrap();

        if let Section::Memory(mem) = section {
            assert_eq!(mem.initial, 1);
            assert_eq!(mem.maximum, None);
        } else {
            panic!("Wrong section type");
        }
    }

    #[test]
    fn test_leb128_reading() {
        let validator = StreamingWasmValidator::new(ComprehensivePlatformLimits::default());

        // Test reading simple values
        let data = [0x01]; // 1
        let (value, bytes) = validator.read_leb128_u32(&data).unwrap();
        assert_eq!(value, 1);
        assert_eq!(bytes, 1);

        let data = [0x7F]; // 127
        let (value, bytes) = validator.read_leb128_u32(&data).unwrap();
        assert_eq!(value, 127);
        assert_eq!(bytes, 1);

        let data = [0x80, 0x01]; // 128
        let (value, bytes) = validator.read_leb128_u32(&data).unwrap();
        assert_eq!(value, 128);
        assert_eq!(bytes, 2);
    }

    #[test]
    fn test_factory_methods() {
        let validator = PlatformWasmValidatorFactory::create_for_platform().unwrap();
        assert_eq!(validator.state(), ValidationState::Header);

        let embedded_validator = PlatformWasmValidatorFactory::create_for_embedded(1024 * 1024);
        assert_eq!(
            embedded_validator.platform_limits.max_total_memory,
            1024 * 1024
        );
    }

    #[test]
    fn test_requirements_validation() {
        let mut limits = ComprehensivePlatformLimits::default();
        limits.max_wasm_linear_memory = 64 * 1024; // 64KB limit

        let mut validator = StreamingWasmValidator::new(limits);

        // Create memory section that exceeds limit
        let large_memory = MemorySection {
            initial: 2, // 2 pages = 128KB > 64KB limit
            maximum: None,
        };

        let section = Section::Memory(large_memory);
        assert!(validator.validate_section(&section).is_err());
    }
}
