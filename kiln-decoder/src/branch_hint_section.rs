//! WebAssembly Branch Hint Custom Section Parser
//!
//! This module requires the `alloc` feature.
//!
//! This module implements parsing for the "metadata.code.branch_hint" custom
//! section as defined in the WebAssembly Branch Hinting proposal. This section
//! contains performance hints that suggest which branches are more likely to be
//! taken.
//!
//! # Custom Section Format
//!
//! The branch hint section has the following structure:
//! ```text
//! branch_hint_section ::= func_count:u32 func_hint*
//! func_hint ::= func_idx:u32 hint_count:u32 branch_hint*
//! branch_hint ::= instruction_offset:u32 hint_value:u8
//! ```
//!
//! Where hint_value is:
//! - 0x00: likely_false (branch is unlikely to be taken)
//! - 0x01: likely_true (branch is likely to be taken)

// Core/std library imports

#[cfg(feature = "std")]
extern crate alloc;
#[cfg(feature = "std")]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::collections::HashMap;

// External crates
use wrt_error::{Error, ErrorCategory, Result, codes};
use wrt_format::binary::{read_leb128_u32, read_u8};
use wrt_foundation::traits::{Checksummable, FromBytes, ReadStream, ToBytes, WriteStream};
// NoStdProvider import removed - not used
use wrt_foundation::verification::Checksum;

// Internal modules
use crate::prelude::*;

/// Safe conversion from Rust usize to WebAssembly u32 for LEB128 encoding
///
/// # Arguments
///
/// * `size` - Rust size as usize
///
/// # Returns
///
/// Ok(u32) if conversion is safe, error otherwise  
fn usize_to_wasm_u32(size: usize) -> Result<u32> {
    u32::try_from(size)
        .map_err(|_| Error::parse_error("Size exceeds u32 limit for LEB128 encoding"))
}

/// Branch hint value indicating the likelihood of a branch being taken
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BranchHintValue {
    /// Branch is unlikely to be taken (0x00)
    #[default]
    LikelyFalse = 0,
    /// Branch is likely to be taken (0x01)
    LikelyTrue = 1,
}

impl BranchHintValue {
    /// Create a BranchHintValue from a byte value
    pub fn from_byte(value: u8) -> Result<Self> {
        match value {
            0x00 => Ok(BranchHintValue::LikelyFalse),
            0x01 => Ok(BranchHintValue::LikelyTrue),
            _ => Err(Error::runtime_execution_error("Invalid branch hint value")),
        }
    }

    /// Convert to byte representation
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Check if this hint indicates the branch is likely to be taken
    pub fn is_likely_taken(self) -> bool {
        matches!(self, BranchHintValue::LikelyTrue)
    }
}

// Default is now derived

// Implement required traits for BoundedVec compatibility
impl Checksummable for BranchHintValue {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(&[self.to_byte()]);
    }
}

impl ToBytes for BranchHintValue {
    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(self.to_byte())
    }

    fn serialized_size(&self) -> usize {
        1
    }
}

impl FromBytes for BranchHintValue {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        Self::from_byte(byte).map_err(|_e| {
            wrt_error::Error::new(
                wrt_error::ErrorCategory::Parse,
                wrt_error::codes::INVALID_VALUE_TYPE,
                "Invalid branch hint byte",
            )
        })
    }
}

/// A single branch hint for a specific instruction
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchHint {
    /// Byte offset of the instruction within the function body
    pub instruction_offset: u32,
    /// Hint about whether the branch is likely to be taken
    pub hint_value: BranchHintValue,
}

impl BranchHint {
    /// Create a new branch hint
    pub fn new(instruction_offset: u32, hint_value: BranchHintValue) -> Self {
        Self {
            instruction_offset,
            hint_value,
        }
    }

    /// Check if this hint suggests the branch should be optimized for the taken
    /// path
    pub fn optimize_for_taken(&self) -> bool {
        self.hint_value.is_likely_taken()
    }
}

/// Branch hints for a specific function
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FunctionBranchHints {
    /// Function index within the module
    pub function_index: u32,
    /// Map from instruction offset to branch hint
    #[cfg(feature = "std")]
    pub hints: HashMap<u32, BranchHintValue>,
    #[cfg(not(feature = "std"))]
    pub hints: BTreeMap<u32, BranchHintValue>,
}

impl FunctionBranchHints {
    /// Create new function branch hints
    pub fn new(function_index: u32) -> Self {
        Self {
            function_index,
            #[cfg(feature = "std")]
            hints: HashMap::new(),
            #[cfg(not(feature = "std"))]
            hints: BTreeMap::new(),
        }
    }

    /// Add a branch hint for an instruction
    pub fn add_hint(&mut self, instruction_offset: u32, hint_value: BranchHintValue) -> Result<()> {
        self.hints.insert(instruction_offset, hint_value);
        Ok(())
    }

    /// Get a branch hint for a specific instruction offset
    pub fn get_hint(&self, instruction_offset: u32) -> Option<BranchHintValue> {
        self.hints.get(&instruction_offset).copied()
    }

    /// Get all hints as an iterator
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &BranchHintValue)> {
        self.hints.iter()
    }

    /// Get number of hints
    pub fn len(&self) -> usize {
        self.hints.len()
    }

    /// Check if no hints are present
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Complete branch hint section containing hints for all functions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchHintSection {
    /// Map from function index to branch hints
    #[cfg(feature = "std")]
    pub function_hints: HashMap<u32, FunctionBranchHints>,
    #[cfg(not(feature = "std"))]
    pub function_hints: BTreeMap<u32, FunctionBranchHints>,
}

impl BranchHintSection {
    /// Create a new empty branch hint section
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "std")]
            function_hints: HashMap::new(),
            #[cfg(not(feature = "std"))]
            function_hints: BTreeMap::new(),
        }
    }

    /// Add function branch hints
    pub fn add_function_hints(&mut self, hints: FunctionBranchHints) -> Result<()> {
        self.function_hints.insert(hints.function_index, hints);
        Ok(())
    }

    /// Get branch hints for a specific function
    pub fn get_function_hints(&self, function_index: u32) -> Option<&FunctionBranchHints> {
        self.function_hints.get(&function_index)
    }

    /// Get a specific branch hint
    pub fn get_hint(
        &self,
        function_index: u32,
        instruction_offset: u32,
    ) -> Option<BranchHintValue> {
        self.get_function_hints(function_index)
            .and_then(|hints| hints.get_hint(instruction_offset))
    }

    /// Get number of functions with hints
    pub fn function_count(&self) -> usize {
        self.function_hints.len()
    }

    /// Check if section is empty
    pub fn is_empty(&self) -> bool {
        self.function_count() == 0
    }

    /// Get total number of hints across all functions
    pub fn total_hint_count(&self) -> usize {
        self.function_hints.values().map(|h| h.len()).sum()
    }
}

impl Default for BranchHintSection {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse the branch hint custom section from binary data
pub fn parse_branch_hint_section(data: &[u8]) -> Result<BranchHintSection> {
    let mut offset = 0;
    let mut section = BranchHintSection::new();

    // Read function count
    let (func_count, consumed) = read_leb128_u32(data, offset)?;
    offset += consumed;

    // Parse each function's hints
    for _ in 0..func_count {
        // Read function index
        let (func_idx, consumed) = read_leb128_u32(data, offset)?;
        offset += consumed;

        let mut function_hints = FunctionBranchHints::new(func_idx);

        // Read hint count for this function
        let (hint_count, consumed) = read_leb128_u32(data, offset)?;
        offset += consumed;

        // Parse each hint
        for _ in 0..hint_count {
            // Read instruction offset
            let (instruction_offset, consumed) = read_leb128_u32(data, offset)?;
            offset += consumed;

            // Read hint value
            let (hint_byte, consumed) = read_u8(data, offset)?;
            offset += consumed;

            let hint_value = BranchHintValue::from_byte(hint_byte)?;
            function_hints.add_hint(instruction_offset, hint_value)?;
        }

        section.add_function_hints(function_hints)?;
    }

    Ok(section)
}

/// Encode branch hint section to binary data
#[cfg(feature = "std")]
pub fn encode_branch_hint_section(section: &BranchHintSection) -> Result<Vec<u8>> {
    use crate::prelude::write_leb128_u32 as format_write_leb128_u32;
    let mut data = Vec::new();

    // Write function count
    data.extend_from_slice(&format_write_leb128_u32(usize_to_wasm_u32(
        section.function_count(),
    )?));

    // Write each function's hints
    for (func_idx, hints) in &section.function_hints {
        data.extend_from_slice(&format_write_leb128_u32(*func_idx));
        data.extend_from_slice(&format_write_leb128_u32(usize_to_wasm_u32(hints.len())?));

        for (offset, hint) in hints.iter() {
            data.extend_from_slice(&format_write_leb128_u32(*offset));
            data.push(hint.to_byte());
        }
    }

    Ok(data)
}

/// Branch hint section name constant
pub const BRANCH_HINT_SECTION_NAME: &str = "metadata.code.branch_hint";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_hint_value() {
        assert_eq!(
            BranchHintValue::from_byte(0x00).unwrap(),
            BranchHintValue::LikelyFalse
        );
        assert_eq!(
            BranchHintValue::from_byte(0x01).unwrap(),
            BranchHintValue::LikelyTrue
        );
        assert!(BranchHintValue::from_byte(0x02).is_err());

        assert_eq!(BranchHintValue::LikelyFalse.to_byte(), 0x00);
        assert_eq!(BranchHintValue::LikelyTrue.to_byte(), 0x01);

        assert!(!BranchHintValue::LikelyFalse.is_likely_taken());
        assert!(BranchHintValue::LikelyTrue.is_likely_taken());
    }

    #[test]
    fn test_branch_hint() {
        let hint = BranchHint::new(42, BranchHintValue::LikelyTrue);
        assert_eq!(hint.instruction_offset, 42);
        assert_eq!(hint.hint_value, BranchHintValue::LikelyTrue);
        assert!(hint.optimize_for_taken());

        let hint = BranchHint::new(100, BranchHintValue::LikelyFalse);
        assert!(!hint.optimize_for_taken());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_function_branch_hints() {
        let mut hints = FunctionBranchHints::new(5);
        assert_eq!(hints.function_index, 5);
        assert!(hints.is_empty());

        hints.add_hint(10, BranchHintValue::LikelyTrue).unwrap();
        hints.add_hint(20, BranchHintValue::LikelyFalse).unwrap();

        assert_eq!(hints.len(), 2);
        assert!(!hints.is_empty());

        assert_eq!(hints.get_hint(10), Some(BranchHintValue::LikelyTrue));
        assert_eq!(hints.get_hint(20), Some(BranchHintValue::LikelyFalse));
        assert_eq!(hints.get_hint(30), None);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_branch_hint_section() {
        let mut section = BranchHintSection::new();
        assert!(section.is_empty());

        let mut func_hints = FunctionBranchHints::new(0);
        func_hints.add_hint(5, BranchHintValue::LikelyTrue).unwrap();
        func_hints.add_hint(15, BranchHintValue::LikelyFalse).unwrap();
        section.add_function_hints(func_hints).unwrap();

        let mut func_hints = FunctionBranchHints::new(2);
        func_hints.add_hint(25, BranchHintValue::LikelyTrue).unwrap();
        section.add_function_hints(func_hints).unwrap();

        assert_eq!(section.function_count(), 2);
        assert_eq!(section.total_hint_count(), 3);

        assert_eq!(section.get_hint(0, 5), Some(BranchHintValue::LikelyTrue));
        assert_eq!(section.get_hint(0, 15), Some(BranchHintValue::LikelyFalse));
        assert_eq!(section.get_hint(2, 25), Some(BranchHintValue::LikelyTrue));
        assert_eq!(section.get_hint(1, 5), None);
        assert_eq!(section.get_hint(0, 30), None);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_encode_round_trip() {
        // Create a test section
        let mut section = BranchHintSection::new();

        let mut func0_hints = FunctionBranchHints::new(0);
        func0_hints.add_hint(10, BranchHintValue::LikelyTrue).unwrap();
        func0_hints.add_hint(20, BranchHintValue::LikelyFalse).unwrap();
        section.add_function_hints(func0_hints).unwrap();

        let mut func2_hints = FunctionBranchHints::new(2);
        func2_hints.add_hint(30, BranchHintValue::LikelyTrue).unwrap();
        section.add_function_hints(func2_hints).unwrap();

        // Encode to binary
        let encoded = encode_branch_hint_section(&section).unwrap();

        // Parse back from binary
        let parsed = parse_branch_hint_section(&encoded).unwrap();

        // Verify round-trip
        assert_eq!(parsed.function_count(), 2);
        assert_eq!(parsed.total_hint_count(), 3);
        assert_eq!(parsed.get_hint(0, 10), Some(BranchHintValue::LikelyTrue));
        assert_eq!(parsed.get_hint(0, 20), Some(BranchHintValue::LikelyFalse));
        assert_eq!(parsed.get_hint(2, 30), Some(BranchHintValue::LikelyTrue));
    }

    #[test]
    fn test_parse_empty_section() {
        // Empty section: just function count = 0
        let data = &[0x00];
        let section = parse_branch_hint_section(data).unwrap();

        assert!(section.is_empty());
        assert_eq!(section.function_count(), 0);
        assert_eq!(section.total_hint_count(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_parse_malformed_data() {
        // Truncated data
        let data = &[0x01]; // function count = 1, but no function data
        assert!(parse_branch_hint_section(data).is_err());

        // Invalid hint value
        let data = &[
            0x01, // function count = 1
            0x00, // function index = 0
            0x01, // hint count = 1
            0x05, // instruction offset = 5
            0x02, // invalid hint value
        ];
        assert!(parse_branch_hint_section(data).is_err());
    }
}
