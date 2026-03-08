use kiln_foundation::bounded::MAX_DWARF_FILE_TABLE;

/// File table support for resolving file indices to paths
/// Provides the missing 2% for source file path resolution
use crate::strings::DebugString;

/// A file entry in the DWARF file table
#[derive(Debug, Clone, Copy, Default)]
pub struct FileEntry<'a> {
    /// File path (may be relative or absolute)
    pub path: DebugString<'a>,
    /// Directory index (0 = current directory)
    pub dir_index: u32,
    /// Last modification time (0 = unknown)
    pub mod_time: u64,
    /// File size in bytes (0 = unknown)
    pub size: u64,
}

// Implement required traits for BoundedVec compatibility

impl<'a> PartialEq for FileEntry<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.dir_index == other.dir_index
            && self.mod_time == other.mod_time
            && self.size == other.size
    }
}

impl<'a> Eq for FileEntry<'a> {}

impl<'a> kiln_foundation::traits::Checksummable for FileEntry<'a> {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        self.path.update_checksum(checksum);
        checksum.update_slice(&self.dir_index.to_le_bytes());
        checksum.update_slice(&self.mod_time.to_le_bytes());
        checksum.update_slice(&self.size.to_le_bytes());
    }
}

impl<'a> kiln_foundation::traits::ToBytes for FileEntry<'a> {
    fn serialized_size(&self) -> usize {
        // path (DebugString) + dir_index (u32) + mod_time (u64) + size (u64)
        self.path.serialized_size() + 4 + 8 + 8
    }

    fn to_bytes_with_provider<'b, P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'b>,
        provider: &P,
    ) -> kiln_error::Result<()> {
        self.path.to_bytes_with_provider(writer, provider)?;
        writer.write_u32_le(self.dir_index)?;
        writer.write_u64_le(self.mod_time)?;
        writer.write_u64_le(self.size)?;
        Ok(())
    }
}

impl<'a> kiln_foundation::traits::FromBytes for FileEntry<'a> {
    fn from_bytes_with_provider<'b, P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'b>,
        provider: &P,
    ) -> kiln_error::Result<Self> {
        Ok(Self {
            path: DebugString::from_bytes_with_provider(reader, provider)?,
            dir_index: reader.read_u32_le()?,
            mod_time: reader.read_u64_le()?,
            size: reader.read_u64_le()?,
        })
    }
}

/// File table for resolving file indices to paths
///
/// Uses fixed-size arrays for no_std compatibility. DebugString is a zero-copy
/// reference type that cannot survive serialization round-trips, so we store
/// entries directly in arrays rather than using BoundedVec.
pub struct FileTable<'a> {
    /// Directory entries
    directories: [Option<DebugString<'a>>; MAX_DWARF_FILE_TABLE],
    /// Number of valid directory entries
    directory_count: usize,
    /// File entries
    files: [Option<FileEntry<'a>>; MAX_DWARF_FILE_TABLE],
    /// Number of valid file entries
    file_count: usize,
}

impl<'a> core::fmt::Debug for FileTable<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FileTable")
            .field("directory_count", &self.directory_count)
            .field("file_count", &self.file_count)
            .finish()
    }
}

impl<'a> Default for FileTable<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> FileTable<'a> {
    /// Create a new empty file table
    pub fn new() -> Self {
        Self {
            directories: [None; MAX_DWARF_FILE_TABLE],
            directory_count: 0,
            files: [None; MAX_DWARF_FILE_TABLE],
            file_count: 0,
        }
    }

    /// Add a directory entry
    /// Returns the 1-based index (DWARF convention: 0 = compilation directory)
    pub fn add_directory(&mut self, dir: DebugString<'a>) -> kiln_error::Result<u32> {
        if self.directory_count >= MAX_DWARF_FILE_TABLE {
            return Err(kiln_error::Error::memory_error("Directory table full"));
        }
        self.directories[self.directory_count] = Some(dir);
        self.directory_count += 1;
        Ok(self.directory_count as u32)
    }

    /// Add a file entry
    /// Returns the 1-based index (DWARF convention: 0 = no file)
    pub fn add_file(&mut self, file: FileEntry<'a>) -> kiln_error::Result<u32> {
        if self.file_count >= MAX_DWARF_FILE_TABLE {
            return Err(kiln_error::Error::memory_error("File table full"));
        }
        self.files[self.file_count] = Some(file);
        self.file_count += 1;
        Ok(self.file_count as u32)
    }

    /// Get a file entry by index (1-based as per DWARF spec)
    pub fn get_file(&self, index: u16) -> Option<FileEntry<'a>> {
        if index == 0 {
            return None; // 0 means no file in DWARF
        }
        let i = (index - 1) as usize;
        if i < self.file_count {
            self.files[i]
        } else {
            None
        }
    }

    /// Get a directory by index (0 = compilation directory)
    pub fn get_directory(&self, index: u32) -> Option<DebugString<'a>> {
        if index == 0 {
            return None; // 0 = compilation directory (not stored here)
        }
        let i = (index - 1) as usize;
        if i < self.directory_count {
            self.directories[i]
        } else {
            None
        }
    }

    /// Get the full path for a file
    /// Returns directory + "/" + filename (or just filename if no directory)
    pub fn get_full_path(&self, file_index: u16) -> Option<FilePath<'a>> {
        let file = self.get_file(file_index)?;

        if file.dir_index == 0 {
            // File is relative to compilation directory
            Some(FilePath {
                directory: None,
                filename: file.path,
            })
        } else {
            // File has explicit directory
            let directory = self.get_directory(file.dir_index)?;
            Some(FilePath {
                directory: Some(directory),
                filename: file.path,
            })
        }
    }

    /// Get the number of files in the table
    pub fn file_count(&self) -> usize {
        self.file_count
    }

    /// Get the number of directories in the table
    pub fn directory_count(&self) -> usize {
        self.directory_count
    }
}

/// Represents a resolved file path
#[derive(Debug, Clone)]
pub struct FilePath<'a> {
    /// Directory component (None = relative to compilation directory)
    pub directory: Option<DebugString<'a>>,
    /// Filename component
    pub filename: DebugString<'a>,
}

impl<'a> FilePath<'a> {
    /// Check if this is a relative path
    pub fn is_relative(&self) -> bool {
        self.directory.is_none() || !self.directory.as_ref().unwrap().as_str().starts_with('/')
    }

    /// Get the filename only (no directory)
    pub fn filename(&self) -> &str {
        self.filename.as_str()
    }

    /// Format as a path string (directory/filename)
    /// Binary std/no_std choice
    /// so this is primarily for display purposes
    pub fn display<F>(&self, mut writer: F) -> core::result::Result<(), core::fmt::Error>
    where
        F: FnMut(&str) -> core::result::Result<(), core::fmt::Error>,
    {
        if let Some(ref dir) = self.directory {
            writer(dir.as_str())?;
            writer("/")?;
        }
        writer(self.filename.as_str())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::StringTable;

    #[cfg(all(not(feature = "std"), any(feature = "alloc", test)))]
    extern crate alloc;

    #[cfg(not(feature = "std"))]
    use alloc::string::String;
    #[cfg(feature = "std")]
    use std::string::String;

    #[test]
    fn test_file_table() {
        // Create mock string data
        let string_data = b"\0src\0lib\0main.rs\0utils.rs\0tests\0";
        let string_table = StringTable::new(string_data);

        let mut file_table = FileTable::new();

        // Add directories
        let src_dir = string_table.get_string(1).unwrap();
        let lib_dir = string_table.get_string(5).unwrap();
        let tests_dir = string_table.get_string(25).unwrap();

        assert_eq!(file_table.add_directory(src_dir), Ok(1));
        assert_eq!(file_table.add_directory(lib_dir), Ok(2));
        assert_eq!(file_table.add_directory(tests_dir), Ok(3));

        // Add files
        let main_rs = FileEntry {
            path: string_table.get_string(9).unwrap(),
            dir_index: 1,
            mod_time: 0,
            size: 0,
        };

        let utils_rs = FileEntry {
            path: string_table.get_string(17).unwrap(),
            dir_index: 1,
            mod_time: 0,
            size: 0,
        };

        assert_eq!(file_table.add_file(main_rs), Ok(1));
        assert_eq!(file_table.add_file(utils_rs), Ok(2));

        // Test retrieval
        assert_eq!(file_table.file_count(), 2);
        assert_eq!(file_table.directory_count(), 3);

        // Test full path resolution
        let path1 = file_table.get_full_path(1).unwrap();
        assert_eq!(path1.filename(), "main.rs");
        assert_eq!(path1.directory.unwrap().as_str(), "src");

        let path2 = file_table.get_full_path(2).unwrap();
        assert_eq!(path2.filename(), "utils.rs");
        assert_eq!(path2.directory.unwrap().as_str(), "src");
    }

    #[test]
    fn test_file_path_display() {
        let string_data = b"\0src\0main.rs\0";
        let string_table = StringTable::new(string_data);

        let path = FilePath {
            directory: Some(string_table.get_string(1).unwrap()),
            filename: string_table.get_string(5).unwrap(),
        };

        let mut output = String::new();
        path.display(|s| {
            output.push_str(s);
            Ok(())
        })
        .unwrap();

        assert_eq!(output, "src/main.rs");
    }
}
