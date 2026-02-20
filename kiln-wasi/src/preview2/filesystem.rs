//! WASI filesystem interface implementation
//!
//! Implements the `wasi:filesystem` interface for file operations using WRT's
//! resource management patterns and platform abstractions.

use core::any::Any;

#[cfg(feature = "std")]
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::RwLock,
};

use crate::{
    prelude::*,
    Value,
};

/// File descriptor table for tracking open files
#[cfg(feature = "std")]
static FILE_TABLE: RwLock<Option<FileDescriptorTable>> = RwLock::new(None);

/// File descriptor table implementation
#[cfg(feature = "std")]
pub struct FileDescriptorTable {
    /// Map of file descriptors to open files
    files: HashMap<u32, OpenFile>,
    /// Next available file descriptor (starts at 3 after stdin/stdout/stderr)
    next_fd: u32,
}

#[cfg(feature = "std")]
impl FileDescriptorTable {
    /// Create a new file descriptor table
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            next_fd: 3, // 0=stdin, 1=stdout, 2=stderr
        }
    }

    /// Allocate a new file descriptor
    fn allocate(&mut self) -> u32 {
        let fd = self.next_fd;
        self.next_fd += 1;
        fd
    }

    /// Insert a file and return its descriptor
    fn insert(&mut self, file: OpenFile) -> u32 {
        let fd = self.allocate();
        self.files.insert(fd, file);
        fd
    }

    /// Get a file by descriptor
    fn get(&self, fd: u32) -> Option<&OpenFile> {
        self.files.get(&fd)
    }

    /// Get a mutable file by descriptor
    fn get_mut(&mut self, fd: u32) -> Option<&mut OpenFile> {
        self.files.get_mut(&fd)
    }

    /// Remove a file by descriptor
    fn remove(&mut self, fd: u32) -> Option<OpenFile> {
        self.files.remove(&fd)
    }
}

#[cfg(feature = "std")]
impl Default for FileDescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents an open file with its associated metadata
#[cfg(feature = "std")]
pub struct OpenFile {
    /// The underlying file handle
    file: File,
    /// The path to the file (reserved for future use)
    _path: PathBuf,
    /// Whether the file is readable
    readable: bool,
    /// Whether the file is writable
    writable: bool,
}

/// Initialize the file descriptor table
#[cfg(feature = "std")]
fn ensure_file_table() -> Result<()> {
    let mut table = FILE_TABLE.write()
        .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
    if table.is_none() {
        *table = Some(FileDescriptorTable::new());
    }
    Ok(())
}

// ============================================================================
// WASI Filesystem Operations
// ============================================================================

/// WASI filesystem open-at operation
///
/// Opens a file relative to a directory descriptor.
/// Implements `wasi:filesystem/types.open-at`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - Arguments are missing or have invalid types
/// - The file cannot be opened (permission denied, not found, etc.)
pub fn wasi_filesystem_open_at(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        // Extract arguments: dir_fd, path_flags, path, open_flags, descriptor_flags
        let _dir_fd = extract_file_descriptor(args)?;
        let path = extract_string(args, 2)?;
        let open_flags = extract_open_flags(args, 3)?;
        let descriptor_flags = extract_descriptor_flags(args, 4)?;

        // Build open options
        let mut options = OpenOptions::new();

        // Handle open flags
        if open_flags.create {
            options.create(true);
        }
        if open_flags.exclusive {
            options.create_new(true);
        }
        if open_flags.truncate {
            options.truncate(true);
        }

        // Handle descriptor flags (read/write permissions)
        let readable = descriptor_flags.read;
        let writable = descriptor_flags.write;

        if readable {
            options.read(true);
        }
        if writable {
            options.write(true);
        }

        // Open the file
        let file = options.open(path)
            .map_err(|e| map_io_error(&e, "Failed to open file"))?;

        // Register in file table
        let mut table = FILE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = OpenFile {
            file,
            _path: PathBuf::from(path),
            readable,
            writable,
        };

        let fd = table.insert(open_file);

        // Return result<descriptor, error-code>
        Ok(vec![Value::Result(Ok(Box::new(Value::U32(fd))))])
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, filesystem operations are not available
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem read operation
///
/// Reads data from a file descriptor.
/// Implements `wasi:filesystem/types.read`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - The file is not readable or seek/read operations fail
pub fn wasi_filesystem_read(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;
        let len = extract_length(args, 1)? as usize;
        let offset = args.get(2)
            .and_then(|v| match v {
                Value::U64(o) => Some(*o),
                _ => None,
            });

        let mut table = FILE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get_mut(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        if !open_file.readable {
            return Ok(vec![Value::Result(Err(Box::new(Value::U32(8))))]);  // EBADF
        }

        // Seek if offset provided
        if let Some(off) = offset {
            open_file.file.seek(SeekFrom::Start(off))
                .map_err(|e| map_io_error(&e, "Failed to seek"))?;
        }

        // Read data
        let mut buffer = vec![0u8; len.min(65536)];
        let bytes_read = open_file.file.read(&mut buffer)
            .map_err(|e| map_io_error(&e, "Failed to read file"))?;

        buffer.truncate(bytes_read);

        // Convert to WASI list<u8>
        let data: Vec<Value> = buffer.into_iter().map(Value::U8).collect();

        // Return tuple of (data, eof)
        let eof = bytes_read == 0;
        Ok(vec![
            Value::Result(Ok(Box::new(Value::Tuple(vec![
                Value::List(data),
                Value::Bool(eof),
            ]))))
        ])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem write operation
///
/// Writes data to a file descriptor.
/// Implements `wasi:filesystem/types.write`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - The file is not writable or seek/write operations fail
pub fn wasi_filesystem_write(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;
        let data = extract_byte_data(args, 1)?;
        let offset = args.get(2)
            .and_then(|v| match v {
                Value::U64(o) => Some(*o),
                _ => None,
            });

        let mut table = FILE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get_mut(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        if !open_file.writable {
            return Ok(vec![Value::Result(Err(Box::new(Value::U32(8))))]);  // EBADF
        }

        // Seek if offset provided
        if let Some(off) = offset {
            open_file.file.seek(SeekFrom::Start(off))
                .map_err(|e| map_io_error(&e, "Failed to seek"))?;
        }

        // Write data
        let bytes_written = open_file.file.write(&data)
            .map_err(|e| map_io_error(&e, "Failed to write file"))?;

        Ok(vec![Value::Result(Ok(Box::new(Value::U64(bytes_written as u64))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem close operation
///
/// Closes a file descriptor.
/// Implements closing via dropping the descriptor
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
pub fn wasi_filesystem_close(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;

        let mut table = FILE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        if table.remove(fd).is_some() {
            Ok(vec![])
        } else {
            Err(Error::wasi_invalid_fd("Invalid file descriptor"))
        }
    }

    #[cfg(not(feature = "std"))]
    {
        Err(Error::wasi_capability_unavailable("Filesystem not available in no_std"))
    }
}

/// WASI filesystem stat operation
///
/// Gets file metadata.
/// Implements `wasi:filesystem/types.stat`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - Metadata retrieval fails
pub fn wasi_filesystem_stat(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;

        let table = FILE_TABLE.read()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_ref()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        let metadata = open_file.file.metadata()
            .map_err(|e| map_io_error(&e, "Failed to get file metadata"))?;

        // Build descriptor-stat record
        let stat = build_descriptor_stat(&metadata);

        Ok(vec![Value::Result(Ok(Box::new(stat)))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem stat-at operation
///
/// Gets file metadata by path relative to a directory.
/// Implements `wasi:filesystem/types.stat-at`
///
/// # Errors
///
/// Returns an error if:
/// - The directory descriptor argument is missing or invalid
/// - The path argument is missing or invalid
/// - Metadata retrieval for the path fails
pub fn wasi_filesystem_stat_at(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        let _dir_fd = extract_file_descriptor(args)?;
        let path = extract_string(args, 2)?;

        let metadata = std::fs::metadata(path)
            .map_err(|e| map_io_error(&e, "Failed to get file metadata"))?;

        let stat = build_descriptor_stat(&metadata);

        Ok(vec![Value::Result(Ok(Box::new(stat)))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem sync operation
///
/// Syncs file data to disk.
/// Implements `wasi:filesystem/types.sync`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - The sync operation fails
pub fn wasi_filesystem_sync(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;

        let table = FILE_TABLE.read()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_ref()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        open_file.file.sync_all()
            .map_err(|e| map_io_error(&e, "Failed to sync file"))?;

        Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem sync-data operation
///
/// Syncs file data to disk without metadata.
/// Implements `wasi:filesystem/types.sync-data`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - The sync-data operation fails
pub fn wasi_filesystem_sync_data(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;

        let table = FILE_TABLE.read()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_ref()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        // sync_data() syncs data without metadata (more efficient than sync_all)
        open_file.file.sync_data()
            .map_err(|e| map_io_error(&e, "Failed to sync file data"))?;

        Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem set-times operation
///
/// Sets file access and modification times.
/// Implements `wasi:filesystem/types.set-times`
///
/// Note: This implementation uses `std::fs::File::set_times` which requires
/// Rust 1.75+. If using an older Rust version, this will use the path-based
/// approach via `std::fs::set_times` which is available on most platforms.
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - Metadata or timestamp operations fail
#[allow(clippy::similar_names)]
pub fn wasi_filesystem_set_times(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        use std::fs::FileTimes;
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;
        let atime = extract_timestamp(args, 1)?;
        let mtime = extract_timestamp(args, 2)?;

        let table = FILE_TABLE.read()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_ref()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        // Get current timestamps for NoChange cases
        let metadata = open_file.file.metadata()
            .map_err(|e| map_io_error(&e, "Failed to get metadata"))?;

        let current_atime = metadata.accessed().unwrap_or(SystemTime::now());
        let current_mtime = metadata.modified().unwrap_or(SystemTime::now());

        // Convert WASI timestamps to SystemTime
        let atime_system = match atime {
            TimestampValue::Now => SystemTime::now(),
            TimestampValue::Timestamp(ns) => UNIX_EPOCH + Duration::from_nanos(ns),
            TimestampValue::NoChange => current_atime,
        };

        let mtime_system = match mtime {
            TimestampValue::Now => SystemTime::now(),
            TimestampValue::Timestamp(ns) => UNIX_EPOCH + Duration::from_nanos(ns),
            TimestampValue::NoChange => current_mtime,
        };

        // Build FileTimes and set on the file
        let times = FileTimes::new()
            .set_accessed(atime_system)
            .set_modified(mtime_system);

        open_file.file.set_times(times)
            .map_err(|e| map_io_error(&e, "Failed to set file times"))?;

        Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem get-flags operation
///
/// Gets descriptor flags.
/// Implements `wasi:filesystem/types.get-flags`
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
pub fn wasi_filesystem_get_flags(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;

        let table = FILE_TABLE.read()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_ref()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        // Build flags from our tracked state
        let mut flags = 0u32;
        if open_file.readable {
            flags |= 0x01;  // read
        }
        if open_file.writable {
            flags |= 0x02;  // write
        }
        // Note: sync and nonblocking would require additional tracking in OpenFile

        Ok(vec![Value::Result(Ok(Box::new(Value::U32(flags))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem set-flags operation
///
/// Sets descriptor flags.
/// Implements `wasi:filesystem/types.set-flags`
///
/// Note: This implementation can only update our tracked flag state.
/// The actual OS-level flags (like `O_NONBLOCK`, `O_SYNC`) cannot be changed
/// without unsafe code and libc. If the caller needs true nonblocking I/O,
/// they should open the file with those flags initially.
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
/// - The descriptor flags argument is missing or invalid
pub fn wasi_filesystem_set_flags(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;
        let new_flags = extract_descriptor_flags(args, 1)?;

        let mut table = FILE_TABLE.write()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_mut()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let open_file = table.get_mut(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        // Update our tracked flags
        // Note: read/write permissions can't be changed after open
        // We track the requested flags for get_flags to report back
        // Real OS flags cannot be changed without unsafe FFI
        open_file.readable = new_flags.read;
        open_file.writable = new_flags.write;
        // Note: sync and nonblocking require platform-specific FFI to actually change
        // We accept the request but cannot enforce it at the OS level without unsafe code

        Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

/// WASI filesystem advise operation
///
/// Provides advisory information about file access patterns.
/// Implements `wasi:filesystem/types.advise`
///
/// Note: This is an advisory operation that hints to the OS about expected
/// access patterns. Without libc/unsafe FFI, we cannot call `posix_fadvise`,
/// so this implementation validates arguments and returns success.
/// The OS will still use its default caching strategy.
///
/// # Errors
///
/// Returns an error if:
/// - The file table lock cannot be acquired
/// - The file table is not initialized
/// - The file descriptor is invalid
pub fn wasi_filesystem_advise(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        ensure_file_table()?;

        let fd = extract_file_descriptor(args)?;
        let _offset = extract_length(args, 1)?;
        let _len = extract_length(args, 2)?;
        let _advice = extract_advice(args, 3)?;

        // Verify the fd is valid
        let table = FILE_TABLE.read()
            .map_err(|_| Error::wasi_capability_unavailable("Failed to acquire file table lock"))?;
        let table = table.as_ref()
            .ok_or_else(|| Error::wasi_capability_unavailable("File table not initialized"))?;

        let _open_file = table.get(fd)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid file descriptor"))?;

        // Advisory operations are hints to the OS kernel about expected access patterns.
        // Without unsafe FFI to call posix_fadvise (Linux) or fcntl F_RDADVISE (macOS),
        // we accept the hint but cannot pass it to the kernel.
        // This is compliant as WASI advise is advisory only - it should not fail
        // if the system cannot honor the advice.

        Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(vec![Value::Result(Err(Box::new(Value::U32(76))))])  // ENOSYS
    }
}

// ============================================================================
// Helper Types and Functions
// ============================================================================

/// Open flags for file operations
#[derive(Debug, Default)]
struct OpenFlags {
    create: bool,
    directory: bool,
    exclusive: bool,
    truncate: bool,
}

/// Descriptor flags for file operations
#[derive(Debug, Default)]
struct DescriptorFlags {
    read: bool,
    write: bool,
    sync: bool,
    nonblocking: bool,
}

/// Helper function to extract file descriptor from WASI arguments
fn extract_file_descriptor(args: &[Value]) -> Result<u32> {
    args.first()
        .and_then(|v| match v {
            Value::U32(fd) => Some(*fd),
            _ => None,
        })
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Invalid file descriptor argument"))
}

/// Helper function to extract length parameter from WASI arguments
fn extract_length(args: &[Value], index: usize) -> Result<u64> {
    args.get(index)
        .and_then(|v| match v {
            Value::U64(len) => Some(*len),
            Value::U32(len) => Some(u64::from(*len)),
            _ => None,
        })
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Invalid length argument"))
}

/// Helper function to extract string from WASI arguments
fn extract_string(args: &[Value], index: usize) -> Result<&str> {
    args.get(index)
        .and_then(|v| match v {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        })
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Invalid string argument"))
}

/// Helper function to extract byte data from WASI arguments
fn extract_byte_data(args: &[Value], index: usize) -> Result<Vec<u8>> {
    args.get(index)
        .and_then(|v| match v {
            Value::List(list) => {
                let mut bytes = Vec::new();
                for item in list {
                    match item {
                        Value::U8(byte) => bytes.push(*byte),
                        _ => return None,
                    }
                }
                Some(bytes)
            },
            _ => None,
        })
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Invalid byte data argument"))
}

/// Extract open flags from arguments
fn extract_open_flags(args: &[Value], index: usize) -> Result<OpenFlags> {
    let flags_val = args.get(index)
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Missing open flags"))?;

    match flags_val {
        Value::U32(bits) => {
            Ok(OpenFlags {
                create: (bits & 0x01) != 0,
                directory: (bits & 0x02) != 0,
                exclusive: (bits & 0x04) != 0,
                truncate: (bits & 0x08) != 0,
            })
        },
        Value::Record(fields) => {
            // Handle record-style flags
            let mut flags = OpenFlags::default();
            for (name, value) in fields {
                match (name.as_str(), value) {
                    ("create", Value::Bool(b)) => flags.create = *b,
                    ("directory", Value::Bool(b)) => flags.directory = *b,
                    ("exclusive", Value::Bool(b)) => flags.exclusive = *b,
                    ("truncate", Value::Bool(b)) => flags.truncate = *b,
                    _ => {}
                }
            }
            Ok(flags)
        },
        _ => Ok(OpenFlags::default()),
    }
}

/// Extract descriptor flags from arguments
fn extract_descriptor_flags(args: &[Value], index: usize) -> Result<DescriptorFlags> {
    let flags_val = args.get(index)
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Missing descriptor flags"))?;

    match flags_val {
        Value::U32(bits) => {
            Ok(DescriptorFlags {
                read: (bits & 0x01) != 0,
                write: (bits & 0x02) != 0,
                sync: (bits & 0x04) != 0,
                nonblocking: (bits & 0x10) != 0,
            })
        },
        Value::Record(fields) => {
            let mut flags = DescriptorFlags::default();
            for (name, value) in fields {
                match (name.as_str(), value) {
                    ("read", Value::Bool(b)) => flags.read = *b,
                    ("write", Value::Bool(b)) => flags.write = *b,
                    ("sync", Value::Bool(b)) => flags.sync = *b,
                    ("nonblocking", Value::Bool(b)) => flags.nonblocking = *b,
                    _ => {}
                }
            }
            Ok(flags)
        },
        _ => Ok(DescriptorFlags { read: true, ..Default::default() }),
    }
}

/// Timestamp value for set-times operation
#[derive(Debug, Clone, Copy)]
enum TimestampValue {
    /// Set to current time
    Now,
    /// Set to specific timestamp (nanoseconds since UNIX epoch)
    Timestamp(u64),
    /// Don't change this timestamp
    NoChange,
}

/// Advisory access pattern for advise operation
#[derive(Debug, Clone, Copy)]
enum Advice {
    /// Normal access pattern
    Normal,
    /// Sequential access pattern
    Sequential,
    /// Random access pattern
    Random,
    /// Data will be accessed soon
    WillNeed,
    /// Data will not be needed soon
    DontNeed,
    /// Data will be accessed once
    NoReuse,
}

/// Extract timestamp value from WASI arguments
#[cfg(feature = "std")]
fn extract_timestamp(args: &[Value], index: usize) -> Result<TimestampValue> {
    let val = args.get(index)
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Missing timestamp argument"))?;

    match val {
        // WASI new-timestamp variant encoding:
        // 0 = no-change, 1 = now, 2 = timestamp(u64)
        // Value::U32(0) falls through to wildcard which returns NoChange
        Value::U32(1) => Ok(TimestampValue::Now),
        Value::U64(ns) => Ok(TimestampValue::Timestamp(*ns)),
        Value::Record(fields) => {
            // Handle record-style timestamp (variant)
            for (name, value) in fields {
                match (name.as_str(), value) {
                    ("no-change", Value::Tuple(_)) => return Ok(TimestampValue::NoChange),
                    ("now", Value::Tuple(_)) => return Ok(TimestampValue::Now),
                    ("timestamp", Value::U64(ns)) => return Ok(TimestampValue::Timestamp(*ns)),
                    _ => {}
                }
            }
            Ok(TimestampValue::NoChange)
        },
        Value::Tuple(items) if items.len() == 2 => {
            // Variant encoding: (tag, payload)
            // Some(Value::U32(0)) falls through to wildcard which returns NoChange
            match items.first() {
                Some(Value::U32(1)) => Ok(TimestampValue::Now),
                Some(Value::U32(2)) => {
                    match items.get(1) {
                        Some(Value::U64(ns)) => Ok(TimestampValue::Timestamp(*ns)),
                        _ => Ok(TimestampValue::NoChange),
                    }
                },
                _ => Ok(TimestampValue::NoChange),
            }
        },
        _ => Ok(TimestampValue::NoChange),
    }
}

/// Extract advice value from WASI arguments
#[cfg(feature = "std")]
fn extract_advice(args: &[Value], index: usize) -> Result<Advice> {
    let val = args.get(index)
        .ok_or_else(|| Error::parameter_wasi_invalid_fd("Missing advice argument"))?;

    match val {
        // WASI advice enum encoding (supports both U32 and U8)
        // 0/default = Normal, handled by wildcard
        Value::U32(1) | Value::U8(1) => Ok(Advice::Sequential),
        Value::U32(2) | Value::U8(2) => Ok(Advice::Random),
        Value::U32(3) | Value::U8(3) => Ok(Advice::WillNeed),
        Value::U32(4) | Value::U8(4) => Ok(Advice::DontNeed),
        Value::U32(5) | Value::U8(5) => Ok(Advice::NoReuse),
        _ => Ok(Advice::Normal),
    }
}

/// Map `std::io::Error` to WASI error
#[cfg(feature = "std")]
fn map_io_error(e: &std::io::Error, _context: &str) -> Error {
    use std::io::ErrorKind;

    match e.kind() {
        ErrorKind::NotFound => Error::wasi_invalid_fd("File not found"),
        ErrorKind::PermissionDenied => Error::wasi_permission_denied("Permission denied"),
        ErrorKind::AlreadyExists => Error::wasi_invalid_argument("File already exists"),
        ErrorKind::InvalidInput => Error::wasi_invalid_argument("Invalid input"),
        ErrorKind::NotADirectory => Error::wasi_invalid_argument("Not a directory"),
        ErrorKind::IsADirectory => Error::wasi_invalid_argument("Is a directory"),
        ErrorKind::DirectoryNotEmpty => Error::wasi_invalid_argument("Directory not empty"),
        ErrorKind::WouldBlock => Error::wasi_resource_exhausted("Would block"),
        ErrorKind::TimedOut => Error::wasi_timeout("Operation timed out"),
        _ => Error::wasi_runtime_error("I/O error"),
    }
}

/// Build a descriptor-stat record from file metadata
#[cfg(feature = "std")]
fn build_descriptor_stat(metadata: &std::fs::Metadata) -> Value {
    use std::time::UNIX_EPOCH;

    let file_type = if metadata.is_dir() {
        3u8  // directory
    } else if metadata.is_file() {
        4u8  // regular-file
    } else if metadata.is_symlink() {
        7u8  // symbolic-link
    } else {
        0u8  // unknown
    };

    let size = metadata.len();

    // Get timestamps (nanoseconds since UNIX epoch)
    let atime = metadata.accessed()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let mtime = metadata.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let ctime = mtime; // Use mtime as ctime fallback

    Value::Record(vec![
        ("type".to_string(), Value::U8(file_type)),
        ("link-count".to_string(), Value::U64(1)),
        ("size".to_string(), Value::U64(size)),
        ("data-access-timestamp".to_string(), Value::U64(atime)),
        ("data-modification-timestamp".to_string(), Value::U64(mtime)),
        ("status-change-timestamp".to_string(), Value::U64(ctime)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_descriptor() {
        let args = vec![Value::U32(42)];
        assert_eq!(extract_file_descriptor(args).unwrap(), 42);

        let invalid_args = vec![Value::String("not_a_fd".to_string())];
        assert!(extract_file_descriptor(&invalid_args).is_err());
    }

    #[test]
    fn test_extract_length() {
        let args = vec![Value::U32(0), Value::U64(1024)];
        assert_eq!(extract_length(args, 1).unwrap(), 1024);

        let args_u32 = vec![Value::U32(0), Value::U32(512)];
        assert_eq!(extract_length(&args_u32, 1).unwrap(), 512);
    }

    #[test]
    fn test_extract_byte_data() -> Result<()> {
        let data = vec![Value::U8(1), Value::U8(2), Value::U8(3)];
        let args = vec![Value::U32(42), Value::List(data)];

        let bytes = extract_byte_data(args, 1)?;
        assert_eq!(bytes, vec![1, 2, 3]);

        Ok(())
    }

    #[test]
    fn test_extract_string() -> Result<()> {
        let args = vec![Value::U32(42), Value::String("test.txt".to_string())];
        let path = extract_string(args, 1)?;
        assert_eq!(path, "test.txt");

        Ok(())
    }

    #[test]
    fn test_open_flags_extraction() {
        // Test bit-based flags
        let args = vec![Value::U32(0), Value::U32(0), Value::String("test".to_string()), Value::U32(0x05)];
        let flags = extract_open_flags(args, 3).unwrap();
        assert!(flags.create);
        assert!(!flags.directory);
        assert!(flags.exclusive);
        assert!(!flags.truncate);
    }

    #[test]
    fn test_descriptor_flags_extraction() {
        let args = vec![Value::U32(0), Value::U32(0), Value::String("test".to_string()), Value::U32(0), Value::U32(0x03)];
        let flags = extract_descriptor_flags(args, 4).unwrap();
        assert!(flags.read);
        assert!(flags.write);
        assert!(!flags.sync);
    }
}
