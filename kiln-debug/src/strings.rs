use super::error::{DebugError, DebugResult};
/// String extraction from DWARF .debug_str section
/// Binary std/no_std choice
use crate::cursor::DwarfCursor;

/// String table providing access to .debug_str section data
#[derive(Debug, Clone)]
pub struct StringTable<'a> {
    data: &'a [u8],
}

/// A reference to a string in the debug string table
/// Provides zero-copy access to string data
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugString<'a> {
    data: &'a str,
}

// Implement required traits for BoundedVec compatibility

impl<'a> wrt_foundation::traits::Checksummable for DebugString<'a> {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(self.data.as_bytes());
    }
}

impl<'a> wrt_foundation::traits::ToBytes for DebugString<'a> {
    fn to_bytes_with_provider<'b, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'b>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        // Write length followed by string data
        writer.write_u32_le(self.data.len() as u32)?;
        writer.write_all(self.data.as_bytes())?;
        Ok(())
    }
}

impl<'a> wrt_foundation::traits::FromBytes for DebugString<'a> {
    fn from_bytes_with_provider<'b, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'b>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        // This is tricky because we need to return a reference with lifetime 'a
        // In practice, this should not be called for DebugString as it's a zero-copy
        // type We'll return a default value for now
        let _ = reader.read_u32_le()?; // Read and ignore length
        Ok(Self::default())
    }
}

impl<'a> StringTable<'a> {
    /// Create a new string table from .debug_str section data
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Extract a null-terminated string at the given offset
    /// Returns None if offset is out of bounds or string is invalid UTF-8
    pub fn get_string(&self, offset: u32) -> Option<DebugString<'a>> {
        let offset = offset as usize;
        if offset >= self.data.len() {
            return None;
        }

        let remaining = &self.data[offset..];
        let end = remaining.iter().position(|&b| b == 0)?;
        let string_bytes = &remaining[..end];

        let string_str = core::str::from_utf8(string_bytes).ok()?;
        Some(DebugString { data: string_str })
    }

    /// Get string length in bytes (excluding null terminator)
    pub fn get_string_length(&self, offset: u32) -> Option<usize> {
        let offset = offset as usize;
        if offset >= self.data.len() {
            return None;
        }

        let remaining = &self.data[offset..];
        remaining.iter().position(|&b| b == 0)
    }

    /// Iterator over all strings in the table
    pub fn strings(&self) -> StringTableIterator<'a> {
        StringTableIterator {
            data: self.data,
            offset: 0,
        }
    }

    /// Check if the string table is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get the total size of the string table
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

impl<'a> DebugString<'a> {
    /// Get the string data as a &str
    pub fn as_str(&self) -> &str {
        self.data
    }

    /// Get the length of the string in bytes
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Check if the string starts with a given prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.data.starts_with(prefix)
    }

    /// Check if the string ends with a given suffix
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.data.ends_with(suffix)
    }

    /// Check if the string contains a substring
    pub fn contains(&self, substring: &str) -> bool {
        self.data.contains(substring)
    }
}

/// Iterator over strings in a string table
pub struct StringTableIterator<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for StringTableIterator<'a> {
    type Item = (u32, DebugString<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let current_offset = self.offset as u32;
        let remaining = &self.data[self.offset..];

        let end = remaining.iter().position(|&b| b == 0)?;
        let string_bytes = &remaining[..end];

        if let Ok(string_str) = core::str::from_utf8(string_bytes) {
            let debug_string = DebugString { data: string_str };
            self.offset += end + 1; // Skip null terminator
            Some((current_offset, debug_string))
        } else {
            // Skip invalid UTF-8 string
            self.offset += end + 1;
            self.next()
        }
    }
}

/// Helper function to read a string reference from DWARF data
/// Used for DW_FORM_strp attributes
#[allow(dead_code)]
pub fn read_string_ref(cursor: &mut DwarfCursor) -> DebugResult<u32> {
    Ok(cursor.read_u32()?)
}

/// Helper function to read an inline string from DWARF data
/// Used for DW_FORM_string attributes
#[allow(dead_code)]
pub fn read_inline_string<'a>(cursor: &mut DwarfCursor<'a>) -> DebugResult<DebugString<'a>> {
    let remaining = cursor.remaining_slice();

    let end = remaining.iter().position(|&b| b == 0).ok_or(DebugError::InvalidData)?;

    let string_bytes = &remaining[..end];
    let string_str = core::str::from_utf8(string_bytes).map_err(|_| DebugError::InvalidData)?;

    cursor.advance(end + 1)?; // Skip string + null terminator
    Ok(DebugString { data: string_str })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(not(feature = "std"), any(feature = "alloc", test)))]
    extern crate alloc;

    #[cfg(not(feature = "std"))]
    use alloc::vec::Vec;
    #[cfg(feature = "std")]
    use std::vec::Vec;

    const TEST_STRING_DATA: &[u8] = &[
        0x00, // Empty string at offset 0
        b'h', b'e', b'l', b'l', b'o', 0x00, // "hello" at offset 1
        b'w', b'o', b'r', b'l', b'd', 0x00, // "world" at offset 7
        b'r', b'u', b's', b't', 0x00, // "rust" at offset 13
    ];

    #[test]
    fn test_string_table_creation() {
        let table = StringTable::new(TEST_STRING_DATA);
        assert_eq!(table.size(), TEST_STRING_DATA.len());
        assert!(!table.is_empty());
    }

    #[test]
    fn test_string_extraction() {
        let table = StringTable::new(TEST_STRING_DATA);

        // Test empty string at offset 0
        let empty = table.get_string(0).unwrap();
        assert_eq!(empty.as_str(), "");
        assert!(empty.is_empty());

        // Test "hello" at offset 1
        let hello = table.get_string(1).unwrap();
        assert_eq!(hello.as_str(), "hello");
        assert_eq!(hello.len(), 5);

        // Test "world" at offset 7
        let world = table.get_string(7).unwrap();
        assert_eq!(world.as_str(), "world");
        assert_eq!(world.len(), 5);

        // Test "rust" at offset 13
        let rust = table.get_string(13).unwrap();
        assert_eq!(rust.as_str(), "rust");
        assert_eq!(rust.len(), 4);
    }

    #[test]
    fn test_invalid_offsets() {
        let table = StringTable::new(TEST_STRING_DATA);

        // Out of bounds offset
        assert!(table.get_string(1000).is_none());

        // Offset at end of data
        assert!(table.get_string(TEST_STRING_DATA.len() as u32).is_none());
    }

    #[test]
    fn test_string_methods() {
        let table = StringTable::new(TEST_STRING_DATA);
        let hello = table.get_string(1).unwrap();

        assert!(hello.starts_with("hel"));
        assert!(hello.ends_with("llo"));
        assert!(hello.contains("ell"));
        assert!(!hello.contains("xyz"));
    }

    #[test]
    fn test_string_length() {
        let table = StringTable::new(TEST_STRING_DATA);

        assert_eq!(table.get_string_length(0), Some(0)); // Empty string
        assert_eq!(table.get_string_length(1), Some(5)); // "hello"
        assert_eq!(table.get_string_length(7), Some(5)); // "world"
        assert_eq!(table.get_string_length(13), Some(4)); // "rust"
        assert_eq!(table.get_string_length(1000), None); // Out of bounds
    }

    #[test]
    fn test_string_iterator() {
        let table = StringTable::new(TEST_STRING_DATA);
        let strings: Vec<_> = table.strings().collect();

        assert_eq!(strings.len(), 4);
        assert_eq!(strings[0].0, 0);
        assert_eq!(strings[0].1.as_str(), "");
        assert_eq!(strings[1].0, 1);
        assert_eq!(strings[1].1.as_str(), "hello");
        assert_eq!(strings[2].0, 7);
        assert_eq!(strings[2].1.as_str(), "world");
        assert_eq!(strings[3].0, 13);
        assert_eq!(strings[3].1.as_str(), "rust");
    }

    #[test]
    fn test_inline_string_reading() {
        let data = b"test_string\0more_data";
        let mut cursor = DwarfCursor::new(data);

        let string = read_inline_string(&mut cursor).unwrap();
        assert_eq!(string.as_str(), "test_string");

        // Cursor should be positioned after the null terminator
        assert_eq!(cursor.remaining(), 9); // "more_data" has 9 bytes
    }
}
