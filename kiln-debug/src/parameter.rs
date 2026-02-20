use wrt_foundation::{
    bounded::{BoundedVec, MAX_DWARF_ABBREV_CACHE},
    budget_aware_provider::CrateId,
    memory_sizing::LargeProvider,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
};

/// Parameter and type information support
/// Provides the missing 2% for parameter information
use crate::strings::DebugString;

/// Basic type information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicType {
    /// Void type
    Void,
    /// Boolean type
    Bool,
    /// Signed integer (size in bytes)
    SignedInt(u8),
    /// Unsigned integer (size in bytes)
    UnsignedInt(u8),
    /// Floating point (size in bytes)
    Float(u8),
    /// Pointer to another type
    Pointer,
    /// Reference to another type
    Reference,
    /// Array type
    Array,
    /// Structure type
    Struct,
    /// Unknown/complex type
    Unknown,
}

impl BasicType {
    /// Parse from DWARF encoding and size
    pub fn from_encoding(encoding: u8, byte_size: u8) -> Self {
        match encoding {
            0x00 => Self::Void,                   // DW_ATE_address
            0x01 => Self::Pointer,                // DW_ATE_address
            0x02 => Self::Bool,                   // DW_ATE_boolean
            0x04 => Self::Float(byte_size),       // DW_ATE_float
            0x05 => Self::SignedInt(byte_size),   // DW_ATE_signed
            0x07 => Self::UnsignedInt(byte_size), // DW_ATE_unsigned
            0x08 => Self::UnsignedInt(byte_size), // DW_ATE_unsigned_char
            0x10 => Self::Reference,              // DW_ATE_UTF (reused for references)
            _ => Self::Unknown,
        }
    }

    /// Get a string representation of the type
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "bool",
            Self::SignedInt(1) => "i8",
            Self::SignedInt(2) => "i16",
            Self::SignedInt(4) => "i32",
            Self::SignedInt(8) => "i64",
            Self::UnsignedInt(1) => "u8",
            Self::UnsignedInt(2) => "u16",
            Self::UnsignedInt(4) => "u32",
            Self::UnsignedInt(8) => "u64",
            Self::Float(4) => "f32",
            Self::Float(8) => "f64",
            Self::Pointer => "ptr",
            Self::Reference => "ref",
            Self::Array => "array",
            Self::Struct => "struct",
            _ => "unknown",
        }
    }

    /// Convert to a u8 representation for serialization
    pub fn to_u8(&self) -> u8 {
        match self {
            Self::Void => 0,
            Self::Bool => 1,
            Self::SignedInt(size) => 2 + *size,
            Self::UnsignedInt(size) => 10 + *size,
            Self::Float(size) => 18 + *size,
            Self::Pointer => 26,
            Self::Reference => 27,
            Self::Array => 28,
            Self::Struct => 29,
            Self::Unknown => 30,
        }
    }
}

/// Function parameter information
#[derive(Debug, Clone)]
pub struct Parameter<'a> {
    /// Parameter name
    pub name: Option<DebugString<'a>>,
    /// Parameter type
    pub param_type: BasicType,
    /// Source file index where declared
    pub file_index: u16,
    /// Source line where declared
    pub line: u32,
    /// Parameter position (0-based)
    pub position: u16,
    /// Is this a variadic parameter?
    pub is_variadic: bool,
}

// Implement required traits for BoundedVec compatibility
impl<'a> Default for Parameter<'a> {
    fn default() -> Self {
        Self {
            name: None,
            param_type: BasicType::Unknown,
            file_index: 0,
            line: 0,
            position: 0,
            is_variadic: false,
        }
    }
}

impl<'a> PartialEq for Parameter<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.param_type == other.param_type
            && self.file_index == other.file_index
            && self.line == other.line
            && self.position == other.position
            && self.is_variadic == other.is_variadic
    }
}

impl<'a> Eq for Parameter<'a> {}

impl<'a> wrt_foundation::traits::Checksummable for Parameter<'a> {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        if let Some(ref name) = self.name {
            checksum.update(1);
            name.update_checksum(checksum);
        } else {
            checksum.update(0);
        }
        checksum.update(self.param_type.to_u8());
        checksum.update_slice(&self.file_index.to_le_bytes());
        checksum.update_slice(&self.line.to_le_bytes());
        checksum.update_slice(&self.position.to_le_bytes());
        checksum.update(self.is_variadic as u8);
    }
}

impl<'a> wrt_foundation::traits::ToBytes for Parameter<'a> {
    fn to_bytes_with_provider<'b, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'b>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        // Write name option
        match &self.name {
            Some(name) => {
                writer.write_u8(1)?;
                name.to_bytes_with_provider(writer, provider)?;
            },
            None => {
                writer.write_u8(0)?;
            },
        }
        writer.write_u8(self.param_type.to_u8())?;
        writer.write_u16_le(self.file_index)?;
        writer.write_u32_le(self.line)?;
        writer.write_u16_le(self.position)?;
        writer.write_u8(self.is_variadic as u8)?;
        Ok(())
    }
}

impl<'a> wrt_foundation::traits::FromBytes for Parameter<'a> {
    fn from_bytes_with_provider<'b, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'b>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let has_name = reader.read_u8()? != 0;
        let name = if has_name {
            Some(DebugString::from_bytes_with_provider(reader, provider)?)
        } else {
            None
        };

        Ok(Self {
            name,
            param_type: BasicType::Unknown, // We'll just use Unknown for deserialization
            file_index: reader.read_u16_le()?,
            line: reader.read_u32_le()?,
            position: reader.read_u16_le()?,
            is_variadic: reader.read_u8()? != 0,
        })
    }
}

/// Collection of parameters for a function
#[derive(Debug)]
pub struct ParameterList<'a> {
    /// Parameters in order
    parameters: BoundedVec<
        Parameter<'a>,
        MAX_DWARF_ABBREV_CACHE,
        NoStdProvider<{ MAX_DWARF_ABBREV_CACHE * 64 }>,
    >,
}

impl<'a> Default for ParameterList<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> ParameterList<'a> {
    /// Create a new empty parameter list
    pub fn new() -> Self {
        Self {
            parameters: {
                let provider = safe_managed_alloc!({ MAX_DWARF_ABBREV_CACHE * 64 }, CrateId::Debug)
                    .unwrap_or_else(|_| {
                        NoStdProvider::<{ MAX_DWARF_ABBREV_CACHE * 64 }>::default()
                    });
                BoundedVec::new(provider).expect("Failed to create parameters BoundedVec")
            },
        }
    }

    /// Add a parameter to the list
    #[allow(clippy::result_unit_err)]
    pub fn add_parameter(&mut self, param: Parameter<'a>) -> Result<(), ()> {
        self.parameters.push(param).map_err(|_| ())
    }

    /// Get all parameters
    pub fn parameters(&self) -> &[Parameter<'a>] {
        self.parameters.as_slice().unwrap_or(&[])
    }

    /// Get parameter count
    pub fn count(&self) -> usize {
        self.parameters.len()
    }

    /// Check if function has variadic parameters
    pub fn is_variadic(&self) -> bool {
        self.parameters.iter().any(|p| p.is_variadic)
    }

    /// Get parameter by position
    pub fn get_by_position(&self, position: u16) -> Option<Parameter<'a>> {
        self.parameters.iter().find(|p| p.position == position)
    }

    /// Format parameter list for display
    pub fn display<F>(&self, mut writer: F) -> core::result::Result<(), core::fmt::Error>
    where
        F: FnMut(&str) -> core::result::Result<(), core::fmt::Error>,
    {
        writer("(")?;

        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                writer(", ")?;
            }

            // Parameter name
            if let Some(ref name) = param.name {
                writer(name.as_str())?;
                writer(": ")?;
            }

            // Parameter type
            writer(param.param_type.type_name())?;

            if param.is_variadic {
                writer("...")?;
            }
        }

        writer(")")?;
        Ok(())
    }
}

/// Inline function information
#[derive(Debug, Clone, Default)]
pub struct InlinedFunction<'a> {
    /// Name of the inlined function
    pub name: Option<DebugString<'a>>,
    /// Abstract origin (reference to original function)
    pub abstract_origin: u32,
    /// Low PC (start address in parent)
    pub low_pc: u32,
    /// High PC (end address in parent)
    pub high_pc: u32,
    /// Call site file
    pub call_file: u16,
    /// Call site line
    pub call_line: u32,
    /// Call site column
    pub call_column: u16,
    /// Depth of inlining (0 = directly inlined into parent)
    pub depth: u8,
}

// Implement required traits for BoundedVec compatibility

impl<'a> PartialEq for InlinedFunction<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.abstract_origin == other.abstract_origin
            && self.low_pc == other.low_pc
            && self.high_pc == other.high_pc
            && self.call_file == other.call_file
            && self.call_line == other.call_line
            && self.call_column == other.call_column
            && self.depth == other.depth
    }
}

impl<'a> Eq for InlinedFunction<'a> {}

impl<'a> wrt_foundation::traits::Checksummable for InlinedFunction<'a> {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        if let Some(ref name) = self.name {
            checksum.update(1);
            name.update_checksum(checksum);
        } else {
            checksum.update(0);
        }
        checksum.update_slice(&self.abstract_origin.to_le_bytes());
        checksum.update_slice(&self.low_pc.to_le_bytes());
        checksum.update_slice(&self.high_pc.to_le_bytes());
        checksum.update_slice(&self.call_file.to_le_bytes());
        checksum.update_slice(&self.call_line.to_le_bytes());
        checksum.update_slice(&self.call_column.to_le_bytes());
        checksum.update(self.depth);
    }
}

impl<'a> wrt_foundation::traits::ToBytes for InlinedFunction<'a> {
    fn to_bytes_with_provider<'b, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'b>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        // Write name option
        match &self.name {
            Some(name) => {
                writer.write_u8(1)?;
                name.to_bytes_with_provider(writer, provider)?;
            },
            None => {
                writer.write_u8(0)?;
            },
        }
        writer.write_u32_le(self.abstract_origin)?;
        writer.write_u32_le(self.low_pc)?;
        writer.write_u32_le(self.high_pc)?;
        writer.write_u16_le(self.call_file)?;
        writer.write_u32_le(self.call_line)?;
        writer.write_u16_le(self.call_column)?;
        writer.write_u8(self.depth)?;
        Ok(())
    }
}

impl<'a> wrt_foundation::traits::FromBytes for InlinedFunction<'a> {
    fn from_bytes_with_provider<'b, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'b>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        let has_name = reader.read_u8()? != 0;
        let name = if has_name {
            Some(DebugString::from_bytes_with_provider(reader, provider)?)
        } else {
            None
        };

        Ok(Self {
            name,
            abstract_origin: reader.read_u32_le()?,
            low_pc: reader.read_u32_le()?,
            high_pc: reader.read_u32_le()?,
            call_file: reader.read_u16_le()?,
            call_line: reader.read_u32_le()?,
            call_column: reader.read_u16_le()?,
            depth: reader.read_u8()?,
        })
    }
}

/// Collection of inlined functions
#[derive(Debug)]
#[allow(dead_code)]
pub struct InlinedFunctions<'a> {
    /// Inlined function entries
    entries: BoundedVec<
        InlinedFunction<'a>,
        MAX_DWARF_ABBREV_CACHE,
        NoStdProvider<{ MAX_DWARF_ABBREV_CACHE * 128 }>,
    >,
}

impl<'a> Default for InlinedFunctions<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl<'a> InlinedFunctions<'a> {
    /// Create new inlined functions collection
    pub fn new() -> Self {
        Self {
            entries: {
                let provider =
                    safe_managed_alloc!({ MAX_DWARF_ABBREV_CACHE * 128 }, CrateId::Debug)
                        .unwrap_or_else(|_| LargeProvider::default());
                BoundedVec::new(provider).expect("Failed to create entries BoundedVec")
            },
        }
    }

    /// Add an inlined function
    #[allow(clippy::result_unit_err)]
    pub fn add(&mut self, func: InlinedFunction<'a>) -> Result<(), ()> {
        self.entries.push(func).map_err(|_| ())
    }

    /// Find all inlined functions containing the given PC
    pub fn find_at_pc(&self, pc: u32) -> impl Iterator<Item = InlinedFunction<'a>> + '_ {
        self.entries.iter().filter(move |f| pc >= f.low_pc && pc < f.high_pc)
    }

    /// Get all inlined functions
    pub fn all(&self) -> impl Iterator<Item = InlinedFunction<'a>> + '_ {
        self.entries.iter()
    }

    /// Check if any functions are inlined at this PC
    pub fn has_inlined_at(&self, pc: u32) -> bool {
        self.find_at_pc(pc).next().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(not(feature = "std"), any(feature = "alloc", test)))]
    extern crate alloc;

    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec::Vec};
    #[cfg(feature = "std")]
    use std::{string::String, vec::Vec};

    #[test]
    fn test_basic_type_parsing() {
        assert_eq!(BasicType::from_encoding(0x02, 1), BasicType::Bool);
        assert_eq!(BasicType::from_encoding(0x05, 4), BasicType::SignedInt(4));
        assert_eq!(BasicType::from_encoding(0x07, 8), BasicType::UnsignedInt(8));
        assert_eq!(BasicType::from_encoding(0x04, 4), BasicType::Float(4));
    }

    #[test]
    fn test_type_names() {
        assert_eq!(BasicType::SignedInt(4).type_name(), "i32");
        assert_eq!(BasicType::UnsignedInt(8).type_name(), "u64");
        assert_eq!(BasicType::Float(4).type_name(), "f32");
        assert_eq!(BasicType::Bool.type_name(), "bool");
    }

    #[test]
    fn test_parameter_list_display() {
        let mut params = ParameterList::new();

        // Add some test parameters
        let param1 = Parameter {
            name: None,
            param_type: BasicType::SignedInt(4),
            file_index: 0,
            line: 0,
            position: 0,
            is_variadic: false,
        };

        let param2 = Parameter {
            name: None,
            param_type: BasicType::Pointer,
            file_index: 0,
            line: 0,
            position: 1,
            is_variadic: false,
        };

        params.add_parameter(param1).unwrap();
        params.add_parameter(param2).unwrap();

        let mut output = String::new();
        params
            .display(|s| {
                output.push_str(s);
                Ok(())
            })
            .unwrap();

        assert_eq!(output, "(i32, ptr)");
    }

    #[test]
    fn test_inlined_functions() {
        let mut inlined = InlinedFunctions::new();

        let func = InlinedFunction {
            name: None,
            abstract_origin: 0x100,
            low_pc: 0x1000,
            high_pc: 0x1100,
            call_file: 1,
            call_line: 42,
            call_column: 8,
            depth: 0,
        };

        inlined.add(func).unwrap();

        // Test PC lookup
        assert!(inlined.has_inlined_at(0x1050));
        assert!(!inlined.has_inlined_at(0x2000));

        let found: Vec<_> = inlined.find_at_pc(0x1050).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].call_line, 42);
    }
}
