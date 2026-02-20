//! QNX-specific memory partition management.
//!
//! Provides a memory partition manager for QNX Neutrino RTOS to create
//! isolated WebAssembly execution environments with dedicated memory resources.
//!
//! This module interfaces with QNX's partition APIs to enable multi-tenant
//! WebAssembly execution with strong isolation guarantees.

use core::{
    fmt::{
        self,
        Debug,
    },
    sync::atomic::{
        AtomicU32,
        Ordering,
    },
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// FFI declarations for QNX system calls related to memory partitions
#[allow(non_camel_case_types)]
#[cfg(all(feature = "platform-qnx", target_os = "nto"))]
mod ffi {
    use core::ffi::c_void;

    // QNX-specific types
    pub type qnx_pid_t = i32;
    pub type qnx_int_t = i32;
    pub type qnx_size_t = usize;
    pub type mem_partition_id_t = u32;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MemPartitionFlags {
        /// No special flags
        None            = 0,
        /// Create a hierarchical partition
        Hierarchical    = 1,
        /// Create a memory-isolated partition
        MemoryIsolation = 2,
        /// Create a container partition
        Container       = 4,
    }

    // SAFETY: Edition 2024 requires unsafe extern blocks
    unsafe extern "C" {
        // Memory partition management functions
        pub fn mem_partition_create(
            flags: u32,
            name: *const u8,
            parent: mem_partition_id_t,
        ) -> mem_partition_id_t;

        pub fn mem_partition_destroy(id: mem_partition_id_t) -> qnx_int_t;

        pub fn mem_partition_getid() -> mem_partition_id_t;

        pub fn mem_partition_setcurrent(id: mem_partition_id_t) -> qnx_int_t;

        // Memory partition configuration
        pub fn mem_partition_config(
            id: mem_partition_id_t,
            cmd: qnx_int_t,
            param: *mut c_void,
            size: qnx_size_t,
        ) -> qnx_int_t;

        // Process attachment to partitions
        pub fn mem_partition_attach_process(id: mem_partition_id_t, pid: qnx_pid_t) -> qnx_int_t;

        pub fn mem_partition_detach_process(id: mem_partition_id_t, pid: qnx_pid_t) -> qnx_int_t;
    }
}

// Mock implementation for non-QNX targets for build compatibility
#[cfg(not(all(feature = "platform-qnx", target_os = "nto")))]
mod ffi {
    use core::ffi::c_void;

    // Mock types
    pub type qnx_pid_t = i32;
    pub type qnx_int_t = i32;
    pub type qnx_size_t = usize;
    pub type mem_partition_id_t = u32;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MemPartitionFlags {
        None            = 0,
        Hierarchical    = 1,
        MemoryIsolation = 2,
        Container       = 4,
    }

    // Mock functions for build compatibility
    #[allow(unused)]
    pub unsafe fn mem_partition_create(
        _flags: u32,
        _name: *const u8,
        _parent: mem_partition_id_t,
    ) -> mem_partition_id_t {
        1 // Return dummy valid id
    }

    #[allow(unused)]
    pub unsafe fn mem_partition_destroy(_id: mem_partition_id_t) -> qnx_int_t {
        0 // Success
    }

    #[allow(unused)]
    pub unsafe fn mem_partition_getid() -> mem_partition_id_t {
        0 // System partition
    }

    #[allow(unused)]
    pub unsafe fn mem_partition_setcurrent(_id: mem_partition_id_t) -> qnx_int_t {
        0 // Success
    }

    #[allow(unused)]
    pub unsafe fn mem_partition_config(
        _id: mem_partition_id_t,
        _cmd: qnx_int_t,
        _param: *mut c_void,
        _size: qnx_size_t,
    ) -> qnx_int_t {
        0 // Success
    }

    #[allow(unused)]
    pub unsafe fn mem_partition_attach_process(
        _id: mem_partition_id_t,
        _pid: qnx_pid_t,
    ) -> qnx_int_t {
        0 // Success
    }

    #[allow(unused)]
    pub unsafe fn mem_partition_detach_process(
        _id: mem_partition_id_t,
        _pid: qnx_pid_t,
    ) -> qnx_int_t {
        0 // Success
    }

    // Binary std/no_std choice
    #[allow(unused)]
    pub unsafe fn malloc(_size: qnx_size_t) -> *mut c_void {
        core::ptr::null_mut()
    }

    #[allow(unused)]
    pub unsafe fn free(_ptr: *mut c_void) {}
}

/// QNX memory partition flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnxPartitionFlags {
    /// Standard partition with no special flags
    Standard        = 0,
    /// Hierarchical partition (can have child partitions)
    Hierarchical    = 1,
    /// Memory-isolated partition (stronger memory isolation)
    MemoryIsolation = 2,
    /// Container partition (for full isolation)
    Container       = 4,
}

impl From<QnxPartitionFlags> for u32 {
    fn from(flags: QnxPartitionFlags) -> Self {
        flags as u32
    }
}

/// QNX memory partition configuration commands
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QnxPartitionConfigCmd {
    /// Set memory size limits
    SetMemorySize     = 1,
    /// Set CPU limits
    SetCpuLimits      = 2,
    /// Set scheduler policy
    SetSchedPolicy    = 3,
    /// Set security policy
    SetSecurityPolicy = 4,
}

/// Memory size configuration for QNX memory partitions
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemorySizeConfig {
    /// Minimum guaranteed memory in bytes
    pub min_size:      usize,
    /// Maximum allowed memory in bytes
    pub max_size:      usize,
    /// Reserved memory in bytes
    pub reserved_size: usize,
}

/// Configuration for QnxMemoryPartition
#[derive(Debug, Clone)]
pub struct QnxPartitionConfig {
    /// Name of the partition
    pub name:              &'static str,
    /// Partition flags
    pub flags:             QnxPartitionFlags,
    /// Whether to use the system partition as parent
    pub use_system_parent: bool,
    /// Memory size configuration
    pub memory_size:       Option<MemorySizeConfig>,
}

impl Default for QnxPartitionConfig {
    fn default() -> Self {
        Self {
            name:              "wrt_partition",
            flags:             QnxPartitionFlags::Standard,
            use_system_parent: true,
            memory_size:       None,
        }
    }
}

/// Builder for QnxMemoryPartition
#[derive(Debug, Default)]
pub struct QnxMemoryPartitionBuilder {
    config: QnxPartitionConfig,
}

impl QnxMemoryPartitionBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the partition name
    pub fn with_name(mut self, name: &'static str) -> Self {
        self.config.name = name;
        self
    }

    /// Set partition flags
    pub fn with_flags(mut self, flags: QnxPartitionFlags) -> Self {
        self.config.flags = flags;
        self
    }

    /// Configure whether to use the system partition as parent
    pub fn with_system_parent(mut self, use_system: bool) -> Self {
        self.config.use_system_parent = use_system;
        self
    }

    /// Set memory size configuration
    pub fn with_memory_size(mut self, min: usize, max: usize, reserved: usize) -> Self {
        self.config.memory_size = Some(MemorySizeConfig {
            min_size:      min,
            max_size:      max,
            reserved_size: reserved,
        });
        self
    }

    /// Build the QnxMemoryPartition with the configured settings
    pub fn build(self) -> Result<QnxMemoryPartition> {
        QnxMemoryPartition::new(self.config)
    }
}

/// Memory partition manager for QNX Neutrino
#[derive(Debug)]
pub struct QnxMemoryPartition {
    /// Configuration settings for the partition
    config:       QnxPartitionConfig,
    /// Partition ID
    partition_id: AtomicU32,
    /// Parent partition ID
    parent_id:    u32,
    /// Whether the partition has been created
    created:      bool,
}

impl QnxMemoryPartition {
    /// Create a new QnxMemoryPartition with the specified configuration
    pub fn new(config: QnxPartitionConfig) -> Result<Self> {
        // Get the parent partition ID
        let parent_id = if config.use_system_parent {
            unsafe { ffi::mem_partition_getid() }
        } else {
            0 // Use default parent
        };

        // Create a new partition
        let partition_id = unsafe {
            ffi::mem_partition_create(config.flags.into(), config.name.as_ptr(), parent_id)
        };

        if partition_id == 0 {
            return Err(Error::runtime_execution_error(
                "Failed to create QNX memory partition",
            ));
        }

        // Configure memory size if specified
        if let Some(mem_size) = &config.memory_size {
            let result = unsafe {
                ffi::mem_partition_config(
                    partition_id,
                    QnxPartitionConfigCmd::SetMemorySize as i32,
                    mem_size as *const _ as *mut _,
                    core::mem::size_of::<MemorySizeConfig>(),
                )
            };

            if result != 0 {
                // Clean up the partition if configuration fails
                unsafe {
                    ffi::mem_partition_destroy(partition_id);
                }

                return Err(Error::new(
                    ErrorCategory::Platform,
                    1,
                    "Failed to configure QNX partition memory size",
                ));
            }
        }

        Ok(Self {
            config,
            partition_id: AtomicU32::new(partition_id),
            parent_id,
            created: true,
        })
    }

    /// Get the partition ID
    pub fn id(&self) -> u32 {
        self.partition_id.load(Ordering::Acquire)
    }

    /// Activate this partition for the current thread
    pub fn activate(&self) -> Result<()> {
        if !self.created {
            return Err(Error::runtime_execution_error(
                "Cannot activate destroyed QNX partition",
            ));
        }

        let result =
            unsafe { ffi::mem_partition_setcurrent(self.partition_id.load(Ordering::Acquire)) };

        if result != 0 {
            return Err(Error::new(
                ErrorCategory::Platform,
                1,
                "Failed to activate QNX memory partition",
            ));
        }

        Ok(())
    }

    /// Restore the parent partition
    pub fn restore_parent(&self) -> Result<()> {
        let result = unsafe { ffi::mem_partition_setcurrent(self.parent_id) };

        if result != 0 {
            return Err(Error::runtime_execution_error(
                "Failed to restore parent QNX partition",
            ));
        }

        Ok(())
    }

    /// Attach a process to this partition
    pub fn attach_process(&self, pid: i32) -> Result<()> {
        if !self.created {
            return Err(Error::new(
                ErrorCategory::Platform,
                1,
                "Cannot attach process to destroyed QNX partition",
            ));
        }

        let result = unsafe {
            ffi::mem_partition_attach_process(self.partition_id.load(Ordering::Acquire), pid)
        };

        if result != 0 {
            return Err(Error::runtime_execution_error(
                "Failed to attach process to QNX partition",
            ));
        }

        Ok(())
    }

    /// Detach a process from this partition
    pub fn detach_process(&self, pid: i32) -> Result<()> {
        if !self.created {
            return Err(Error::new(
                ErrorCategory::Platform,
                1,
                "Cannot detach process from destroyed QNX partition",
            ));
        }

        let result = unsafe {
            ffi::mem_partition_detach_process(self.partition_id.load(Ordering::Acquire), pid)
        };

        if result != 0 {
            return Err(Error::runtime_execution_error(
                "Failed to detach process from QNX partition",
            ));
        }

        Ok(())
    }

    /// Execute a function within this partition's context
    pub fn with_partition<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        // Activate this partition
        self.activate()?;

        // Execute the function
        let result = f();

        // Restore the parent partition
        self.restore_parent()?;

        // Return the function result
        result
    }

    /// Manually destroy the partition
    pub fn destroy(&self) -> Result<()> {
        let id = self.partition_id.load(Ordering::Acquire);
        if id != 0 {
            let result = unsafe { ffi::mem_partition_destroy(id) };
            if result != 0 {
                return Err(Error::new(
                    ErrorCategory::Platform,
                    1,
                    "Failed to destroy QNX memory partition",
                ));
            }
            self.partition_id.store(0, Ordering::Release);
        }
        Ok(())
    }
}

impl Drop for QnxMemoryPartition {
    fn drop(&mut self) {
        if self.created {
            let id = self.partition_id.load(Ordering::Acquire);
            if id != 0 {
                unsafe {
                    let _ = ffi::mem_partition_destroy(id);
                }
            }
        }
    }
}

/// A RAII guard for temporarily switching to a partition
pub struct PartitionGuard<'a> {
    /// Reference to the partition
    partition: &'a QnxMemoryPartition,
    /// Whether the guard is active
    active:    bool,
}

impl<'a> PartitionGuard<'a> {
    /// Create a new guard that activates the partition
    pub fn new(partition: &'a QnxMemoryPartition) -> Result<Self> {
        partition.activate()?;
        Ok(Self {
            partition,
            active: true,
        })
    }

    /// Manually deactivate the guard (restore parent partition)
    pub fn deactivate(&mut self) -> Result<()> {
        if self.active {
            self.partition.restore_parent()?;
            self.active = false;
        }
        Ok(())
    }
}

impl<'a> Drop for PartitionGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.partition.restore_parent();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests would only run on QNX, so they're marked as ignore
    // In a real implementation, you might use conditional compilation
    // to only include these tests when targeting QNX

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_partition_basic() {
        // Create a basic partition
        let partition = QnxMemoryPartitionBuilder::new()
            .with_name("test_partition")
            .build()
            .expect("Failed to create partition");

        // Get partition ID
        let id = partition.id();
        assert!(id > 0);

        // Test activation
        let result = partition.activate();
        assert!(result.is_ok());

        // Test restoration
        let result = partition.restore_parent();
        assert!(result.is_ok());

        // Clean up (handled by Drop, but can be done manually)
        let result = partition.destroy();
        assert!(result.is_ok());
    }

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_partition_guard() {
        // Create a partition
        let partition = QnxMemoryPartitionBuilder::new()
            .with_name("guard_test")
            .build()
            .expect("Failed to create partition");

        // Use partition guard
        {
            let guard = PartitionGuard::new(&partition).expect("Failed to create guard");

            // Execute code inside the partition
            // ...

            // Guard will automatically restore parent when it goes out of scope
        }

        // Test manual deactivation
        {
            let mut guard = PartitionGuard::new(&partition).expect("Failed to create guard");

            // Manually deactivate
            let result = guard.deactivate();
            assert!(result.is_ok());
        }
    }

    #[test]
    #[ignore = "Requires QNX system to run"]
    fn test_qnx_partition_with_memory_limits() {
        // Create a partition with memory limits
        let partition = QnxMemoryPartitionBuilder::new()
            .with_name("memory_test")
            .with_flags(QnxPartitionFlags::MemoryIsolation)
            .with_memory_size(
                4 * 1024 * 1024,  // 4MB minimum
                16 * 1024 * 1024, // 16MB maximum
                1 * 1024 * 1024,  // 1MB reserved
            )
            .build()
            .expect("Failed to create partition");

        // Execute a closure within the partition
        let result = partition.with_partition(|| {
            // Binary std/no_std choice
            let ptr = unsafe { ffi::malloc(1024 * 1024) };
            if ptr.is_null() {
                return Err(Error::memory_error("Allocation failed within partition"));
            }
            unsafe { ffi::free(ptr) };
            Ok(())
        });

        assert!(result.is_ok());
    }
}
