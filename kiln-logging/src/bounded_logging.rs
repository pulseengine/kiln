// Enhanced Bounded Logging Infrastructure
// This is the bounded logging implementation for the component module

extern crate alloc;
use alloc::{
    string::String,
    vec::Vec,
};

// Always import Error and Result regardless of feature flags
use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    traits::{
        Checksummable,
        FromBytes,
        ReadStream,
        ToBytes,
        WriteStream,
    },
    verification::Checksum,
    BoundedCapacity, // Import required trait for BoundedVec methods
    MemoryProvider,
};

use crate::{
    bounded_log_infra::{
        new_log_entry_vec,
        new_logger_vec,
        BoundedLogEntryVec,
        BoundedLoggerVec,
    },
    level::LogLevel,
};

/// Bounded logging limits configuration
///
/// This structure defines the resource limits for the bounded logging system
/// to ensure that logging operations do not exceed platform resource
/// constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedLoggingLimits {
    /// Maximum total size of the log buffer in bytes
    pub max_log_buffer_size:    usize,
    /// Maximum size of a single log message in bytes
    pub max_log_message_size:   usize,
    /// Maximum number of concurrent loggers allowed
    pub max_concurrent_loggers: usize,
    /// Maximum number of log entries that can be stored
    pub max_log_entries:        usize,
    /// Log entry retention time in milliseconds before automatic cleanup
    pub retention_time_ms:      u64,
    /// Number of entries that trigger automatic buffer flush
    pub flush_threshold:        usize,
}

impl Default for BoundedLoggingLimits {
    fn default() -> Self {
        Self {
            max_log_buffer_size:    64 * 1024, // 64KB
            max_log_message_size:   1024,      // 1KB per message
            max_concurrent_loggers: 16,        // 16 concurrent loggers
            max_log_entries:        1000,      // 1000 log entries
            retention_time_ms:      300_000,   // 5 minutes
            flush_threshold:        100,       // Flush after 100 entries
        }
    }
}

impl BoundedLoggingLimits {
    /// Create limits for embedded platforms
    #[must_use]
    pub fn embedded() -> Self {
        Self {
            max_log_buffer_size:    8 * 1024, // 8KB
            max_log_message_size:   256,      // 256B per message
            max_concurrent_loggers: 4,        // 4 concurrent loggers
            max_log_entries:        100,      // 100 log entries
            retention_time_ms:      60_000,   // 1 minute
            flush_threshold:        20,       // Flush after 20 entries
        }
    }

    /// Create limits for QNX platforms
    #[must_use]
    pub fn qnx() -> Self {
        Self {
            max_log_buffer_size:    32 * 1024, // 32KB
            max_log_message_size:   512,       // 512B per message
            max_concurrent_loggers: 8,         // 8 concurrent loggers
            max_log_entries:        500,       // 500 log entries
            retention_time_ms:      180_000,   // 3 minutes
            flush_threshold:        50,        // Flush after 50 entries
        }
    }

    /// Validate limits are reasonable
    pub fn validate(&self) -> Result<()> {
        if self.max_log_buffer_size == 0 {
            return Err(Error::invalid_input("max_log_buffer_size cannot be zero"));
        }
        if self.max_log_message_size == 0 {
            return Err(Error::invalid_input("max_log_message_size cannot be zero"));
        }
        if self.max_log_message_size > self.max_log_buffer_size {
            return Err(Error::invalid_input(
                "max_log_message_size cannot exceed max_log_buffer_size",
            ));
        }
        if self.max_concurrent_loggers == 0 {
            return Err(Error::invalid_input(
                "max_concurrent_loggers cannot be zero",
            ));
        }
        Ok(())
    }
}

/// Logger identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LoggerId(pub u32);

/// Component instance identifier for logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ComponentLoggingId(pub u32);

/// Bounded log entry
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundedLogEntry {
    /// Unique identifier for this log entry
    pub id:           u64,
    /// Timestamp when this entry was created
    pub timestamp:    u64,
    /// Log level for this entry
    pub level:        LogLevel,
    /// Logger that created this entry
    pub logger_id:    LoggerId,
    /// Component that generated this entry
    pub component_id: ComponentLoggingId,
    /// Log message content
    pub message:      String,
    /// Additional metadata for this entry
    pub metadata:     LogMetadata,
}

/// Log metadata for tracking and filtering
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogMetadata {
    /// Module path where the log originated
    pub module:       Option<String>,
    /// Source file where the log originated
    pub file:         Option<String>,
    /// Line number where the log originated
    pub line:         Option<u32>,
    /// Thread ID that generated this log
    pub thread_id:    Option<u32>,
    /// Safety level (0-255, higher is more critical)
    pub safety_level: u8,
}

// Implement required traits for BoundedLogEntry

impl Checksummable for BoundedLogEntry {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Update checksum with all fields
        checksum.update_slice(&self.id.to_le_bytes());
        checksum.update_slice(&self.timestamp.to_le_bytes());
        checksum.update_slice(&[self.level as u8]);
        self.logger_id.update_checksum(checksum);
        self.component_id.update_checksum(checksum);
        checksum.update_slice(&(self.message.len() as u32).to_le_bytes());
        checksum.update_slice(self.message.as_bytes());
        self.metadata.update_checksum(checksum);
    }
}

impl ToBytes for BoundedLogEntry {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        stream: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        // Write all fields in order
        stream.write_u64_le(self.id)?;
        stream.write_u64_le(self.timestamp)?;
        stream.write_u8(self.level as u8)?;
        stream.write_u32_le(self.logger_id.0)?;
        stream.write_u32_le(self.component_id.0)?;
        // Write string length then content
        stream.write_u32_le(self.message.len() as u32)?;
        stream.write_all(self.message.as_bytes())?;

        // Write metadata
        self.metadata.to_bytes_with_provider(stream, _provider)?;

        Ok(())
    }

    fn serialized_size(&self) -> usize {
        8 + // id
        8 + // timestamp
        1 + // level
        4 + // logger_id
        4 + // component_id
        4 + self.message.len() + // string length + content
        self.metadata.serialized_size()
    }
}

impl FromBytes for BoundedLogEntry {
    fn from_bytes_with_provider<P: MemoryProvider>(
        stream: &mut ReadStream,
        _provider: &P,
    ) -> Result<Self> {
        let id = stream.read_u64_le()?;
        let timestamp = stream.read_u64_le()?;
        let level_byte = stream.read_u8()?;
        let level = match level_byte {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            4 => LogLevel::Error,
            5 => LogLevel::Critical,
            _ => return Err(Error::invalid_input("Invalid log level byte")),
        };
        let logger_id = LoggerId(stream.read_u32_le()?);
        let component_id = ComponentLoggingId(stream.read_u32_le()?);
        // Read string length then content
        let message_len = stream.read_u32_le()? as usize;
        let mut message_bytes = alloc::vec![0u8; message_len];
        stream.read_exact(&mut message_bytes)?;
        let message = String::from_utf8(message_bytes)
            .map_err(|_| Error::invalid_input("Invalid UTF-8 in log message"))?;
        let metadata = LogMetadata::from_bytes_with_provider(stream, _provider)?;

        Ok(Self {
            id,
            timestamp,
            level,
            logger_id,
            component_id,
            message,
            metadata,
        })
    }
}

// Implement traits for LoggerId

impl Checksummable for LoggerId {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(&self.0.to_le_bytes());
    }
}

impl ToBytes for LoggerId {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        stream: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        stream.write_u32_le(self.0)?;
        Ok(())
    }

    fn serialized_size(&self) -> usize {
        4
    }
}

impl FromBytes for LoggerId {
    fn from_bytes_with_provider<P: MemoryProvider>(
        stream: &mut ReadStream,
        _provider: &P,
    ) -> Result<Self> {
        Ok(LoggerId(stream.read_u32_le()?))
    }
}

// Implement traits for ComponentLoggingId

impl Checksummable for ComponentLoggingId {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update_slice(&self.0.to_le_bytes());
    }
}

impl ToBytes for ComponentLoggingId {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        stream: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        stream.write_u32_le(self.0)?;
        Ok(())
    }

    fn serialized_size(&self) -> usize {
        4
    }
}

impl FromBytes for ComponentLoggingId {
    fn from_bytes_with_provider<P: MemoryProvider>(
        stream: &mut ReadStream,
        _provider: &P,
    ) -> Result<Self> {
        Ok(ComponentLoggingId(stream.read_u32_le()?))
    }
}

// Implement traits for LogMetadata

impl Checksummable for LogMetadata {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Update checksum for Option<String> fields
        if let Some(ref module) = self.module {
            checksum.update_slice(&[1u8]); // Present marker
            checksum.update_slice(&(module.len() as u32).to_le_bytes());
            checksum.update_slice(module.as_bytes());
        } else {
            checksum.update_slice(&[0u8]); // Not present marker
        }

        if let Some(ref file) = self.file {
            checksum.update_slice(&[1u8]);
            checksum.update_slice(&(file.len() as u32).to_le_bytes());
            checksum.update_slice(file.as_bytes());
        } else {
            checksum.update_slice(&[0u8]);
        }

        if let Some(line) = self.line {
            checksum.update_slice(&[1u8]);
            checksum.update_slice(&line.to_le_bytes());
        } else {
            checksum.update_slice(&[0u8]);
        }

        if let Some(thread_id) = self.thread_id {
            checksum.update_slice(&[1u8]);
            checksum.update_slice(&thread_id.to_le_bytes());
        } else {
            checksum.update_slice(&[0u8]);
        }

        checksum.update_slice(&[self.safety_level]);
    }
}

impl ToBytes for LogMetadata {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        stream: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        // Write Option<String> fields
        match &self.module {
            Some(s) => {
                stream.write_u8(1)?; // Present
                stream.write_u32_le(s.len() as u32)?;
                stream.write_all(s.as_bytes())?;
            },
            None => {
                stream.write_u8(0)?; // Not present
            },
        }

        match &self.file {
            Some(s) => {
                stream.write_u8(1)?;
                stream.write_u32_le(s.len() as u32)?;
                stream.write_all(s.as_bytes())?;
            },
            None => {
                stream.write_u8(0)?;
            },
        }

        match self.line {
            Some(v) => {
                stream.write_u8(1)?;
                stream.write_u32_le(v)?;
            },
            None => {
                stream.write_u8(0)?;
            },
        }

        match self.thread_id {
            Some(v) => {
                stream.write_u8(1)?;
                stream.write_u32_le(v)?;
            },
            None => {
                stream.write_u8(0)?;
            },
        }

        stream.write_u8(self.safety_level)?;

        Ok(())
    }

    fn serialized_size(&self) -> usize {
        let mut size = 0;

        // Option<String> fields
        size += 1; // Present/not present marker
        if let Some(ref s) = self.module {
            size += 4 + s.len(); // Length + content
        }

        size += 1;
        if let Some(ref s) = self.file {
            size += 4 + s.len();
        }

        size += 1;
        if self.line.is_some() {
            size += 4;
        }

        size += 1;
        if self.thread_id.is_some() {
            size += 4;
        }

        size += 1; // safety_level

        size
    }
}

impl FromBytes for LogMetadata {
    fn from_bytes_with_provider<P: MemoryProvider>(
        stream: &mut ReadStream,
        _provider: &P,
    ) -> Result<Self> {
        let module = if stream.read_u8()? == 1 {
            let len = stream.read_u32_le()? as usize;
            let mut bytes = alloc::vec![0u8; len];
            stream.read_exact(&mut bytes)?;
            Some(String::from_utf8(bytes).map_err(|_| Error::invalid_input("Invalid UTF-8"))?)
        } else {
            None
        };

        let file = if stream.read_u8()? == 1 {
            let len = stream.read_u32_le()? as usize;
            let mut bytes = alloc::vec![0u8; len];
            stream.read_exact(&mut bytes)?;
            Some(String::from_utf8(bytes).map_err(|_| Error::invalid_input("Invalid UTF-8"))?)
        } else {
            None
        };

        let line = if stream.read_u8()? == 1 { Some(stream.read_u32_le()?) } else { None };

        let thread_id = if stream.read_u8()? == 1 { Some(stream.read_u32_le()?) } else { None };

        let safety_level = stream.read_u8()?;

        Ok(Self {
            module,
            file,
            line,
            thread_id,
            safety_level,
        })
    }
}

/// Bounded log buffer for storing log entries
pub struct BoundedLogBuffer {
    entries:         BoundedLogEntryVec,
    max_entries:     usize,
    buffer_size:     usize,
    max_buffer_size: usize,
    next_entry_id:   u64,
}

impl BoundedLogBuffer {
    /// Create a new bounded log buffer
    ///
    /// # Arguments
    /// * `max_entries` - Maximum number of log entries to store
    /// * `max_buffer_size` - Maximum total buffer size in bytes
    ///
    /// # Errors
    /// Returns an error if the log entry vector cannot be created
    pub fn new(max_entries: usize, max_buffer_size: usize) -> Result<Self> {
        let entries = new_log_entry_vec()?;
        Ok(Self {
            entries,
            max_entries,
            buffer_size: 0,
            max_buffer_size,
            next_entry_id: 1,
        })
    }

    /// Add a new log entry to the buffer
    ///
    /// # Arguments
    /// * `entry` - The log entry to add
    ///
    /// # Errors
    /// Returns an error if the entry cannot be added
    pub fn add_entry(&mut self, mut entry: BoundedLogEntry) -> Result<()> {
        let entry_size = entry.message.len()
            + entry.metadata.module.as_ref().map_or(0, |s| s.len())
            + entry.metadata.file.as_ref().map_or(0, |s| s.len())
            + 64; // Base overhead

        // Check if adding this entry would exceed buffer size
        if self.buffer_size + entry_size > self.max_buffer_size {
            self.make_space(entry_size)?;
        }

        // Check if we're at max entries
        if self.entries.len() >= self.max_entries {
            self.remove_oldest_entry();
        }

        entry.id = self.next_entry_id;
        self.next_entry_id = self.next_entry_id.wrapping_add(1);

        self.buffer_size += entry_size;
        self.entries.push(entry)?;

        Ok(())
    }

    fn make_space(&mut self, required_size: usize) -> Result<()> {
        while self.buffer_size + required_size > self.max_buffer_size && !self.entries.is_empty() {
            self.remove_oldest_entry();
        }

        if self.buffer_size + required_size > self.max_buffer_size {
            return Err(Error::OUT_OF_MEMORY);
        }

        Ok(())
    }

    fn remove_oldest_entry(&mut self) {
        if let Ok(entry) = self.entries.get(0) {
            let entry_size = entry.message.len()
                + entry.metadata.module.as_ref().map_or(0, |s| s.len())
                + entry.metadata.file.as_ref().map_or(0, |s| s.len())
                + 64;
            self.buffer_size = self.buffer_size.saturating_sub(entry_size);
        }

        if !self.entries.is_empty() {
            let _ = self.entries.remove(0);
        }
    }

    /// Get all log entries
    pub fn get_entries(&self) -> Vec<BoundedLogEntry> {
        let mut entries = Vec::new();
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                entries.push(entry);
            }
        }
        entries
    }

    /// Get log entries filtered by level
    pub fn get_entries_by_level(&self, level: LogLevel) -> Vec<BoundedLogEntry> {
        let mut filtered = Vec::new();
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                if entry.level == level {
                    filtered.push(entry);
                }
            }
        }
        filtered
    }

    /// Get log entries filtered by component
    pub fn get_entries_by_component(
        &self,
        component_id: ComponentLoggingId,
    ) -> Vec<BoundedLogEntry> {
        let mut filtered = Vec::new();
        for i in 0..self.entries.len() {
            if let Ok(entry) = self.entries.get(i) {
                if entry.component_id == component_id {
                    filtered.push(entry);
                }
            }
        }
        filtered
    }

    /// Clear all log entries
    pub fn clear(&mut self) {
        let _ = self.entries.clear();
        self.buffer_size = 0;
    }

    /// Get number of log entries
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if buffer is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get current buffer size in bytes
    #[must_use]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

/// Bounded logger instance
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedLogger {
    /// Unique identifier for this logger
    pub id:            LoggerId,
    /// Component this logger belongs to
    pub component_id:  ComponentLoggingId,
    /// Human-readable name for this logger
    pub name:          String,
    /// Minimum log level for this logger
    pub min_level:     LogLevel,
    /// Whether this logger is enabled
    pub enabled:       bool,
    /// Number of messages logged by this instance
    pub message_count: u64,
}

impl BoundedLogger {
    /// Create a new bounded logger
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this logger
    /// * `component_id` - Component this logger belongs to
    /// * `name` - Human-readable name for this logger
    /// * `min_level` - Minimum log level for this logger
    #[must_use]
    pub fn new(
        id: LoggerId,
        component_id: ComponentLoggingId,
        name: String,
        min_level: LogLevel,
    ) -> Self {
        Self {
            id,
            component_id,
            name,
            min_level,
            enabled: true,
            message_count: 0,
        }
    }

    /// Check if this logger should log at the given level
    #[must_use]
    pub fn should_log(&self, level: LogLevel) -> bool {
        self.enabled && level >= self.min_level
    }

    /// Increment the message count for this logger
    pub fn increment_message_count(&mut self) {
        self.message_count = self.message_count.wrapping_add(1);
    }
}

impl Default for BoundedLogger {
    fn default() -> Self {
        Self {
            id:            LoggerId(0),
            component_id:  ComponentLoggingId(0),
            name:          String::new(),
            min_level:     LogLevel::Info,
            enabled:       false,
            message_count: 0,
        }
    }
}

// Implement WRT traits for BoundedLogger
impl wrt_foundation::traits::Checksummable for BoundedLogger {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        // Update checksum with all fields
        checksum.update_slice(&self.id.0.to_le_bytes());
        checksum.update_slice(&self.component_id.0.to_le_bytes());
        checksum.update_slice(&(self.name.len() as u32).to_le_bytes());
        checksum.update_slice(self.name.as_bytes());
        checksum.update_slice(&[self.min_level as u8]);
        checksum.update_slice(&[self.enabled as u8]);
        checksum.update_slice(&self.message_count.to_le_bytes());
    }
}

impl ToBytes for BoundedLogger {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        stream: &mut WriteStream,
        _provider: &P,
    ) -> Result<()> {
        // Write all fields in order
        self.id.to_bytes_with_provider(stream, _provider)?;
        self.component_id.to_bytes_with_provider(stream, _provider)?;

        // Write string length then content
        stream.write_u32_le(self.name.len() as u32)?;
        stream.write_all(self.name.as_bytes())?;

        stream.write_u8(self.min_level as u8)?;
        stream.write_bool(self.enabled)?;
        stream.write_u64_le(self.message_count)?;

        Ok(())
    }

    fn serialized_size(&self) -> usize {
        self.id.serialized_size() +
        self.component_id.serialized_size() +
        4 + self.name.len() + // string length + content
        1 + // min_level
        1 + // enabled
        8 // message_count
    }
}

impl FromBytes for BoundedLogger {
    fn from_bytes_with_provider<P: MemoryProvider>(
        stream: &mut ReadStream,
        provider: &P,
    ) -> Result<Self> {
        let id = LoggerId::from_bytes_with_provider(stream, provider)?;
        let component_id = ComponentLoggingId::from_bytes_with_provider(stream, provider)?;

        // Read string length then content
        let name_len = stream.read_u32_le()? as usize;
        let mut name_bytes = alloc::vec![0u8; name_len];
        stream.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|_| Error::invalid_input("Invalid UTF-8 in logger name"))?;

        let min_level_byte = stream.read_u8()?;
        let min_level = match min_level_byte {
            0 => LogLevel::Trace,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            4 => LogLevel::Error,
            5 => LogLevel::Critical,
            _ => return Err(Error::invalid_input("Invalid log level byte")),
        };

        let enabled = stream.read_bool()?;
        let message_count = stream.read_u64_le()?;

        Ok(Self {
            id,
            component_id,
            name,
            min_level,
            enabled,
            message_count,
        })
    }
}

/// Bounded logging manager
pub struct BoundedLoggingManager {
    limits:           BoundedLoggingLimits,
    buffer:           BoundedLogBuffer,
    loggers:          BoundedLoggerVec<BoundedLogger>,
    next_logger_id:   u32,
    total_messages:   u64,
    dropped_messages: u64,
    flush_pending:    bool,
}

impl BoundedLoggingManager {
    /// Create a new bounded logging manager
    pub fn new(limits: BoundedLoggingLimits) -> Result<Self> {
        limits.validate()?;

        let buffer = BoundedLogBuffer::new(limits.max_log_entries, limits.max_log_buffer_size)?;
        let loggers = new_logger_vec()?;

        Ok(Self {
            limits,
            buffer,
            loggers,
            next_logger_id: 1,
            total_messages: 0,
            dropped_messages: 0,
            flush_pending: false,
        })
    }

    /// Register a new logger
    pub fn register_logger(
        &mut self,
        component_id: ComponentLoggingId,
        name: String,
        min_level: LogLevel,
    ) -> Result<LoggerId> {
        // Check logger limit
        if self.loggers.len() >= self.limits.max_concurrent_loggers {
            return Err(Error::TOO_MANY_COMPONENTS);
        }

        let logger_id = LoggerId(self.next_logger_id);
        self.next_logger_id = self.next_logger_id.wrapping_add(1);

        let logger = BoundedLogger::new(logger_id, component_id, name, min_level);
        self.loggers.push(logger)?;

        Ok(logger_id)
    }

    /// Log a message with bounds checking
    pub fn log_message(
        &mut self,
        logger_id: LoggerId,
        level: LogLevel,
        message: String,
        metadata: LogMetadata,
    ) -> Result<()> {
        // Check message size limit
        if message.len() > self.limits.max_log_message_size {
            self.dropped_messages += 1;
            return Err(Error::invalid_input("Log message too large"));
        }

        // Find the logger and get its component_id
        let (component_id, should_log) = {
            let logger = self
                .loggers
                .iter()
                .find(|logger| logger.id == logger_id)
                .ok_or(Error::COMPONENT_NOT_FOUND)?;

            (logger.component_id, logger.should_log(level))
        };

        // Check if logger should log this level
        if !should_log {
            return Ok(()); // Silently ignore
        }

        // Create log entry
        let entry = BoundedLogEntry {
            id: 0, // Will be set by buffer
            timestamp: self.get_timestamp(),
            level,
            logger_id,
            component_id,
            message,
            metadata,
        };

        // Add to buffer
        if let Ok(()) = self.buffer.add_entry(entry) {
            // Find and update the logger's message count
            for i in 0..self.loggers.len() {
                if let Ok(mut logger) = self.loggers.get(i) {
                    if logger.id == logger_id {
                        logger.increment_message_count();
                        // Need to update the logger in the vec
                        let _ = self.loggers.remove(i);
                        let _ = self.loggers.insert(i, logger);
                        break;
                    }
                }
            }
            self.total_messages += 1;

            // Check if we should flush
            if self.buffer.len() >= self.limits.flush_threshold {
                self.flush_pending = true;
            }
        } else {
            self.dropped_messages += 1;
            return Err(Error::OUT_OF_MEMORY);
        }

        Ok(())
    }

    /// Convenience method for logging with minimal metadata
    pub fn log(&mut self, logger_id: LoggerId, level: LogLevel, message: String) -> Result<()> {
        self.log_message(logger_id, level, message, LogMetadata::default())
    }

    /// Get logger by ID
    pub fn get_logger(&self, logger_id: LoggerId) -> Option<BoundedLogger> {
        for i in 0..self.loggers.len() {
            if let Ok(logger) = self.loggers.get(i) {
                if logger.id == logger_id {
                    return Some(logger);
                }
            }
        }
        None
    }

    /// Get mutable logger by ID
    ///
    /// Note: Since BoundedVec doesn't support iter_mut, this returns a cloned
    /// logger that must be updated back into the collection if modified.
    pub fn get_logger_mut(&mut self, logger_id: LoggerId) -> Option<(usize, BoundedLogger)> {
        for i in 0..self.loggers.len() {
            if let Ok(logger) = self.loggers.get(i) {
                if logger.id == logger_id {
                    return Some((i, logger));
                }
            }
        }
        None
    }

    /// Enable/disable a logger
    pub fn set_logger_enabled(&mut self, logger_id: LoggerId, enabled: bool) -> Result<()> {
        if let Some((index, mut logger)) = self.get_logger_mut(logger_id) {
            logger.enabled = enabled;
            // Update the logger in the vector
            let _ = self.loggers.remove(index);
            let _ = self.loggers.insert(index, logger);
            Ok(())
        } else {
            Err(Error::COMPONENT_NOT_FOUND)
        }
    }

    /// Set minimum log level for a logger
    pub fn set_logger_level(&mut self, logger_id: LoggerId, min_level: LogLevel) -> Result<()> {
        if let Some((index, mut logger)) = self.get_logger_mut(logger_id) {
            logger.min_level = min_level;
            // Update the logger in the vector
            let _ = self.loggers.remove(index);
            let _ = self.loggers.insert(index, logger);
            Ok(())
        } else {
            Err(Error::COMPONENT_NOT_FOUND)
        }
    }

    /// Get log entries
    pub fn get_log_entries(&self) -> Vec<BoundedLogEntry> {
        self.buffer.get_entries()
    }

    /// Get log entries by level
    pub fn get_entries_by_level(&self, level: LogLevel) -> Vec<BoundedLogEntry> {
        self.buffer.get_entries_by_level(level)
    }

    /// Get log entries by component
    pub fn get_entries_by_component(
        &self,
        component_id: ComponentLoggingId,
    ) -> Vec<BoundedLogEntry> {
        self.buffer.get_entries_by_component(component_id)
    }

    /// Clear all log entries
    pub fn clear_logs(&mut self) {
        self.buffer.clear();
        self.flush_pending = false;
    }

    /// Remove all loggers for a component
    pub fn remove_component_loggers(&mut self, component_id: ComponentLoggingId) -> usize {
        let _initial_count = self.loggers.len();

        // Manual implementation of retain
        let mut i = 0;
        let mut removed = 0;
        while i < self.loggers.len() {
            if let Ok(logger) = self.loggers.get(i) {
                if logger.component_id == component_id {
                    let _ = self.loggers.remove(i);
                    removed += 1;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        removed
    }

    /// Check if flush is pending
    #[must_use]
    pub fn is_flush_pending(&self) -> bool {
        self.flush_pending
    }

    /// Mark flush as completed
    pub fn mark_flushed(&mut self) {
        self.flush_pending = false;
    }

    /// Get logging statistics
    #[must_use]
    pub fn get_statistics(&self) -> BoundedLoggingStatistics {
        let memory_used = self.buffer.buffer_size();
        let memory_utilization = if self.limits.max_log_buffer_size > 0 {
            (memory_used as f64 / self.limits.max_log_buffer_size as f64) * 100.0
        } else {
            0.0
        };

        BoundedLoggingStatistics {
            registered_loggers: self.loggers.len(),
            active_loggers: self.loggers.iter().filter(|l| l.enabled).count(),
            total_log_entries: self.buffer.len(),
            memory_used,
            memory_utilization,
            total_messages: self.total_messages,
            dropped_messages: self.dropped_messages,
            flush_pending: self.flush_pending,
        }
    }

    /// Validate all logging state
    pub fn validate(&self) -> Result<()> {
        if self.loggers.len() > self.limits.max_concurrent_loggers {
            return Err(Error::TOO_MANY_COMPONENTS);
        }

        if self.buffer.buffer_size() > self.limits.max_log_buffer_size {
            return Err(Error::OUT_OF_MEMORY);
        }

        if self.buffer.len() > self.limits.max_log_entries {
            return Err(Error::OUT_OF_MEMORY);
        }

        Ok(())
    }

    /// Get timestamp (stub implementation)
    fn get_timestamp(&self) -> u64 {
        // In a real implementation, this would use platform-specific timing
        0
    }
}

/// Logging statistics
#[derive(Debug, Clone)]
pub struct BoundedLoggingStatistics {
    /// Number of registered loggers
    pub registered_loggers: usize,
    /// Number of active loggers
    pub active_loggers:     usize,
    /// Total number of log entries stored
    pub total_log_entries:  usize,
    /// Memory used in bytes
    pub memory_used:        usize,
    /// Memory utilization as percentage (0.0-100.0)
    pub memory_utilization: f64,
    /// Total number of messages processed
    pub total_messages:     u64,
    /// Number of messages dropped due to limits
    pub dropped_messages:   u64,
    /// Whether there are pending flush operations
    pub flush_pending:      bool,
}

/// Convenience macros for logging (only available with alloc)
/// Log a debug message
#[macro_export]
macro_rules! log_debug {
    ($manager:expr, $logger_id:expr, $($arg:tt)*) => {
        $manager.log($logger_id, $crate::LogLevel::Debug, alloc::format!($($arg)*))
    };
}

/// Log an info message
#[macro_export]
macro_rules! log_info {
    ($manager:expr, $logger_id:expr, $($arg:tt)*) => {
        $manager.log($logger_id, $crate::LogLevel::Info, alloc::format!($($arg)*))
    };
}

/// Log a warning message
#[macro_export]
macro_rules! log_warning {
    ($manager:expr, $logger_id:expr, $($arg:tt)*) => {
        $manager.log($logger_id, $crate::LogLevel::Warn, alloc::format!($($arg)*))
    };
}

/// Log an error message
#[macro_export]
macro_rules! log_error {
    ($manager:expr, $logger_id:expr, $($arg:tt)*) => {
        $manager.log($logger_id, $crate::LogLevel::Error, alloc::format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn test_bounded_logging_manager_creation() {
        let limits = BoundedLoggingLimits::default();
        let manager = BoundedLoggingManager::new(limits);
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        let stats = manager.get_statistics();
        assert_eq!(stats.registered_loggers, 0);
        assert_eq!(stats.total_log_entries, 0);
    }

    #[test]
    fn test_logger_registration() {
        let limits = BoundedLoggingLimits::default();
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "test-logger".to_string(),
                LogLevel::Info,
            )
            .unwrap();

        assert_eq!(logger_id.0, 1);

        let stats = manager.get_statistics();
        assert_eq!(stats.registered_loggers, 1);
        assert_eq!(stats.active_loggers, 1);
    }

    #[test]
    fn test_log_message() {
        let limits = BoundedLoggingLimits::default();
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "test-logger".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        let result = manager.log(logger_id, LogLevel::Info, "Test message".to_string());
        assert!(result.is_ok());

        let stats = manager.get_statistics();
        assert_eq!(stats.total_log_entries, 1);
        assert_eq!(stats.total_messages, 1);
    }

    #[test]
    fn test_log_level_filtering() {
        let limits = BoundedLoggingLimits::default();
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "test-logger".to_string(),
                LogLevel::Warn, // Only log Warn and Error
            )
            .unwrap();

        // This should be ignored (Debug < Warning)
        let result = manager.log(logger_id, LogLevel::Debug, "Debug message".to_string());
        assert!(result.is_ok());

        // This should be logged (Warning >= Warning)
        let result = manager.log(logger_id, LogLevel::Warn, "Warning message".to_string());
        assert!(result.is_ok());

        let stats = manager.get_statistics();
        assert_eq!(stats.total_log_entries, 1); // Only the warning message
        assert_eq!(stats.total_messages, 1);
    }

    #[test]
    fn test_message_size_limits() {
        let limits = BoundedLoggingLimits {
            max_log_message_size: 10,
            ..BoundedLoggingLimits::default()
        };
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "test-logger".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        // This message is too long (20 chars > 10 limit)
        let result = manager.log(
            logger_id,
            LogLevel::Info,
            "This message is too long".to_string(),
        );
        assert!(result.is_err());

        let stats = manager.get_statistics();
        assert_eq!(stats.dropped_messages, 1);
    }

    #[test]
    fn test_buffer_size_limits() {
        let limits = BoundedLoggingLimits {
            max_log_entries: 2,
            ..BoundedLoggingLimits::default()
        };
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "test-logger".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        // Add three messages (should only keep the last two)
        manager.log(logger_id, LogLevel::Info, "Message 1".to_string()).unwrap();
        manager.log(logger_id, LogLevel::Info, "Message 2".to_string()).unwrap();
        manager.log(logger_id, LogLevel::Info, "Message 3".to_string()).unwrap();

        let entries = manager.get_log_entries();
        assert_eq!(entries.len(), 2);

        // Should have the last two messages
        assert_eq!(entries[0].message, "Message 2");
        assert_eq!(entries[1].message, "Message 3");
    }

    #[test]
    fn test_logger_limits() {
        let limits = BoundedLoggingLimits {
            max_concurrent_loggers: 1,
            ..BoundedLoggingLimits::default()
        };
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        // First logger should succeed
        let result1 = manager.register_logger(
            ComponentLoggingId(1),
            "logger1".to_string(),
            LogLevel::Debug,
        );
        assert!(result1.is_ok());

        // Second logger should fail
        let result2 = manager.register_logger(
            ComponentLoggingId(2),
            "logger2".to_string(),
            LogLevel::Debug,
        );
        assert!(result2.is_err());
    }

    #[test]
    fn test_component_logger_removal() {
        let limits = BoundedLoggingLimits::default();
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger1_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "logger1".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        let logger2_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "logger2".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        let logger3_id = manager
            .register_logger(
                ComponentLoggingId(2),
                "logger3".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        let removed = manager.remove_component_loggers(ComponentLoggingId(1));
        assert_eq!(removed, 2);

        let stats = manager.get_statistics();
        assert_eq!(stats.registered_loggers, 1);

        // Logger3 should still exist
        assert!(manager.get_logger(logger3_id).is_some());
        // Logger1 and Logger2 should be gone
        assert!(manager.get_logger(logger1_id).is_none());
        assert!(manager.get_logger(logger2_id).is_none());
    }

    #[test]
    fn test_log_filtering_by_component() {
        let limits = BoundedLoggingLimits::default();
        let mut manager = BoundedLoggingManager::new(limits).unwrap();

        let logger1_id = manager
            .register_logger(
                ComponentLoggingId(1),
                "logger1".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        let logger2_id = manager
            .register_logger(
                ComponentLoggingId(2),
                "logger2".to_string(),
                LogLevel::Debug,
            )
            .unwrap();

        manager
            .log(
                logger1_id,
                LogLevel::Info,
                "Message from component 1".to_string(),
            )
            .unwrap();
        manager
            .log(
                logger2_id,
                LogLevel::Info,
                "Message from component 2".to_string(),
            )
            .unwrap();
        manager
            .log(
                logger1_id,
                LogLevel::Error,
                "Error from component 1".to_string(),
            )
            .unwrap();

        let component1_entries = manager.get_entries_by_component(ComponentLoggingId(1));
        let component2_entries = manager.get_entries_by_component(ComponentLoggingId(2));

        assert_eq!(component1_entries.len(), 2);
        assert_eq!(component2_entries.len(), 1);
    }
}
