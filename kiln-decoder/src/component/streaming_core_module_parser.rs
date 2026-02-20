//! Streaming Core Module Section Parser for WebAssembly Component Model
//!
//! This module provides ASIL-compliant streaming parsing of Core Module
//! sections within Component binaries. It uses the unified capability-based
//! memory allocation system and operates without loading entire modules into
//! memory.
//!
//! # ASIL Compliance
//!
//! This implementation works across all ASIL levels using the unified provider
//! system:
//! - The BoundedVec types adapt their behavior based on the current ASIL level
//! - The NoStdProvider internally chooses appropriate allocation strategies
//! - All limits are enforced at compile time with runtime validation
//! - Single implementation that works for QM, ASIL-A, ASIL-B, ASIL-C, and
//!   ASIL-D
//!
//! # Architecture
//!
//! The parser uses a streaming approach where:
//! 1. Only section headers are read into memory
//! 2. Module data is processed incrementally
//! 3. Memory allocation is controlled via the capability system
//! 4. All operations are bounded and deterministic

// Environment setup
#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use alloc::vec::Vec;

use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_format::{
    binary::{WASM_MAGIC, WASM_VERSION, read_leb128_u32},
    module::Module,
};

// Use the same provider size as the streaming decoder
type CoreModule = Module;

#[cfg(feature = "std")]
use wrt_format::component::Component;
use wrt_foundation::{
    BoundedVec, VerificationLevel, budget_aware_provider::CrateId, safe_memory::NoStdProvider,
};

// Import the unified bounded decoder infrastructure
#[cfg(not(feature = "std"))]
use crate::bounded_decoder_infra::{
    BoundedModuleVec, MAX_MODULES_PER_COMPONENT, create_decoder_provider,
};

// For std mode, provide basic constants
#[cfg(feature = "std")]
const MAX_MODULES_PER_COMPONENT: usize = 1024;

/// Maximum size of a single core module (16MB, ASIL constraint)
pub const MAX_CORE_MODULE_SIZE: usize = 16 * 1024 * 1024;

/// Decoder provider type for consistent allocation
type DecoderProvider = NoStdProvider<65536>;

/// Core Module Section streaming parser
///
/// This parser processes Core Module sections within Component binaries using
/// a streaming approach that minimizes memory allocation and provides
/// deterministic behavior across all ASIL levels using the unified provider
/// system.
pub struct StreamingCoreModuleParser<'a> {
    /// Binary data being parsed
    data: &'a [u8],
    /// Current parsing offset
    offset: usize,
    /// Verification level for parsing strictness
    verification_level: VerificationLevel,
}

/// Core Module Section parsing result
#[derive(Debug)]
pub struct CoreModuleSection {
    /// Number of modules parsed
    pub module_count: u32,
    /// Total bytes consumed
    pub bytes_consumed: usize,
    /// Modules parsed using capability-managed storage (simplified for core
    /// module streaming)
    modules: Vec<CoreModule>, /* Use Vec for now until Module implements all required
                               * traits */
}

impl<'a> StreamingCoreModuleParser<'a> {
    /// Create a new streaming core module parser
    ///
    /// # Arguments
    /// * `data` - The binary data containing the core module section
    /// * `verification_level` - Level of validation to perform
    ///
    /// # Returns
    /// A new parser instance ready to process core modules
    pub fn new(data: &'a [u8], verification_level: VerificationLevel) -> Result<Self> {
        if data.is_empty() {
            return Err(Error::runtime_execution_error("Streaming parser error"));
        }

        // ASIL constraint: Verify data size constraints
        if data.len() > MAX_CORE_MODULE_SIZE {
            return Err(Error::validation_error(
                "Core module size exceeds maximum allowed",
            ));
        }

        Ok(Self {
            data,
            offset: 0,
            verification_level,
        })
    }

    /// Parse the core module section using streaming approach
    ///
    /// This method processes the core module section without loading entire
    /// modules into memory, using the unified capability-based allocation
    /// system.
    ///
    /// # Returns
    /// A CoreModuleSection containing parsed modules and metadata
    pub fn parse(&mut self) -> Result<CoreModuleSection> {
        // Read the number of core modules
        let (module_count, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        // ASIL constraint: Validate module count
        if module_count > MAX_MODULES_PER_COMPONENT as u32 {
            return Err(Error::validation_error(
                "Too many core modules in component",
            ));
        }

        // Initialize storage (simplified for now)
        let mut modules = Vec::new();

        // Parse each core module
        for i in 0..module_count {
            let module = self.parse_single_core_module(i)?;
            self.store_module(&mut modules, module)?;
        }

        Ok(CoreModuleSection {
            module_count,
            bytes_consumed: self.offset,
            modules,
        })
    }

    // Method removed - using Vec directly for now

    /// Parse a single core module from the binary stream
    fn parse_single_core_module(&mut self, module_index: u32) -> Result<CoreModule> {
        // Read module size
        let (module_size, bytes_read) = read_leb128_u32(self.data, self.offset)?;
        self.offset += bytes_read;

        // ASIL constraint: Validate module size
        if module_size > MAX_CORE_MODULE_SIZE as u32 {
            return Err(Error::validation_error("Core module size exceeds maximum"));
        }

        // Ensure we have enough data
        if self.offset + module_size as usize > self.data.len() {
            return Err(Error::parse_error(
                "Core module extends beyond section data",
            ));
        }

        // Extract module binary data
        let module_data = &self.data[self.offset..self.offset + module_size as usize];

        // Validate WASM header
        if module_data.len() < 8 {
            return Err(Error::parse_invalid_binary(
                "Core module too small for WASM header",
            ));
        }

        if module_data[0..4] != WASM_MAGIC {
            return Err(Error::parse_invalid_binary(
                "Invalid WASM magic number in core module",
            ));
        }

        let version_bytes = [
            module_data[4],
            module_data[5],
            module_data[6],
            module_data[7],
        ];
        if version_bytes != WASM_VERSION {
            return Err(Error::runtime_execution_error("Streaming parser error"));
        }

        // Use existing streaming decoder for the core module
        let module = self.parse_core_module_streaming(module_data)?;

        // Update offset
        self.offset += module_size as usize;

        Ok(module)
    }

    /// Parse core module using streaming decoder
    fn parse_core_module_streaming(&self, module_data: &[u8]) -> Result<CoreModule> {
        // Use the existing streaming decoder infrastructure
        // Convert the result type to match our unified Module type
        #[cfg(feature = "std")]
        {
            crate::streaming_decoder::decode_module_streaming(module_data)
        }
        #[cfg(not(feature = "std"))]
        {
            // For no_std, we need to convert from the specific provider type
            // to the unified Module type
            let module_with_provider =
                crate::streaming_decoder::decode_module_streaming(module_data)?;
            // Convert to unified Module type (this might require implementing conversion)
            // For now, return error indicating this needs proper conversion
            Err(Error::runtime_not_implemented(
                "Core module parsing in no_std requires type conversion implementation",
            ))
        }
    }

    /// Store a parsed module in the storage
    fn store_module(&self, modules: &mut Vec<CoreModule>, module: CoreModule) -> Result<()> {
        modules.push(module);
        Ok(())
    }

    /// Get current parsing offset
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get remaining bytes in the section
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }
}

impl CoreModuleSection {
    /// Get the number of parsed modules
    pub fn module_count(&self) -> u32 {
        self.module_count
    }

    /// Get total bytes consumed during parsing
    pub fn bytes_consumed(&self) -> usize {
        self.bytes_consumed
    }

    /// Get a module by index (ASIL-safe)
    pub fn get_module(&self, index: usize) -> Option<&CoreModule> {
        self.modules.get(index)
    }

    /// Iterate over all modules (ASIL-safe)
    pub fn iter_modules(&self) -> impl ExactSizeIterator<Item = &CoreModule> {
        self.modules.iter()
    }

    /// Get the number of modules as usize
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if the section is empty
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_section() {
        let data = &[0u8]; // Zero modules

        let mut parser = StreamingCoreModuleParser::new(data, VerificationLevel::Standard).unwrap();

        let result = parser.parse().unwrap();
        assert_eq!(result.module_count(), 0);
        assert_eq!(result.bytes_consumed(), 1);
        assert!(result.is_empty());
    }

    #[test]
    fn test_invalid_module_count() {
        // Create data with too many modules
        let module_count = (MAX_MODULES_PER_COMPONENT + 1) as u32;
        let mut data = Vec::new();

        // Write LEB128 encoded module count
        let mut count = module_count;
        while count >= 0x80 {
            data.push(((count & 0x7F) as u8) | 0x80);
            count >>= 7;
        }
        data.push(count as u8);

        let mut parser =
            StreamingCoreModuleParser::new(&data, VerificationLevel::Standard).unwrap();

        assert!(parser.parse().is_err());
    }

    #[test]
    fn test_invalid_module_size() {
        let mut data = Vec::new();
        data.push(1); // One module

        // Write oversized module size
        let oversized = MAX_CORE_MODULE_SIZE as u32 + 1;
        let mut size = oversized;
        while size >= 0x80 {
            data.push(((size & 0x7F) as u8) | 0x80);
            size >>= 7;
        }
        data.push(size as u8);

        let mut parser =
            StreamingCoreModuleParser::new(&data, VerificationLevel::Standard).unwrap();

        assert!(parser.parse().is_err());
    }

    #[test]
    fn test_parser_offset_tracking() {
        let data = &[0u8]; // Zero modules

        let mut parser = StreamingCoreModuleParser::new(data, VerificationLevel::Standard).unwrap();

        assert_eq!(parser.offset(), 0);
        assert_eq!(parser.remaining(), 1);

        let result = parser.parse().unwrap();
        assert_eq!(parser.offset(), 1);
        assert_eq!(parser.remaining(), 0);
        assert_eq!(result.bytes_consumed(), 1);
    }
}
