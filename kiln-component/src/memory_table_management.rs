//! Component memory and table management
//!
//! This module provides memory and table management for WebAssembly components,
//! including isolation, sharing, and lifecycle management.

#[cfg(not(feature = "std"))]
use core::{fmt, mem, slice};
#[cfg(feature = "std")]
use std::{fmt, mem, slice};

#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec};

#[cfg(feature = "std")]
use wrt_foundation::{collections::StaticVec as BoundedVec, component_value::ComponentValue, prelude::*};

#[cfg(not(feature = "std"))]
use wrt_foundation::{
    collections::StaticVec as BoundedVec,
    safe_memory::NoStdProvider,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
    BoundedString,
};

use crate::{
    adapter::CoreValType,
    types::{ValType, Value},
};

// Memory provider type removed - using capability-based allocation

/// Maximum number of memories in no_std environments
const MAX_MEMORIES: usize = 16;

/// Maximum number of tables in no_std environments
const MAX_TABLES: usize = 16;

/// Maximum number of memory pages in no_std environments
const MAX_MEMORY_PAGES: usize = 1024; // 64MB with 64KB pages

/// WebAssembly page size (64KB)
const WASM_PAGE_SIZE: usize = 65536;

/// Component memory manager
pub struct ComponentMemoryManager {
    /// Managed memories
    #[cfg(feature = "std")]
    memories: Vec<ComponentMemory>,
    #[cfg(not(any(feature = "std", )))]
    memories: BoundedVec<ComponentMemory, MAX_MEMORIES>,

    /// Memory sharing policies
    #[cfg(feature = "std")]
    sharing_policies: Vec<MemorySharingPolicy>,
    #[cfg(not(any(feature = "std", )))]
    sharing_policies: BoundedVec<MemorySharingPolicy, MAX_MEMORIES>,

    /// Binary std/no_std choice
    total_allocated: usize,
    /// Maximum allowed memory
    max_memory: usize,
}

/// Component table manager
pub struct ComponentTableManager {
    /// Managed tables
    #[cfg(feature = "std")]
    tables: Vec<ComponentTable>,
    #[cfg(not(any(feature = "std", )))]
    tables: BoundedVec<ComponentTable, MAX_TABLES>,

    /// Table sharing policies
    #[cfg(feature = "std")]
    sharing_policies: Vec<TableSharingPolicy>,
    #[cfg(not(any(feature = "std", )))]
    sharing_policies: BoundedVec<TableSharingPolicy, MAX_TABLES>,
}

/// Component memory instance
#[derive(Debug, Clone)]
pub struct ComponentMemory {
    /// Memory ID
    pub id: u32,
    /// Memory data
    #[cfg(feature = "std")]
    pub data: Vec<u8>,
    #[cfg(not(any(feature = "std", )))]
    pub data: BoundedVec<u8, { MAX_MEMORY_PAGES * WASM_PAGE_SIZE }>,
    /// Memory limits
    pub limits: MemoryLimits,
    /// Shared flag
    pub shared: bool,
    /// Owner component instance
    pub owner: Option<u32>,
    /// Access permissions
    pub permissions: MemoryPermissions,
}

/// Memory limits
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryLimits {
    /// Minimum size in pages
    pub min: u32,
    /// Maximum size in pages (if any)
    pub max: Option<u32>,
}

/// Memory access permissions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryPermissions {
    /// Read permission
    pub read: bool,
    /// Write permission
    pub write: bool,
    /// Execute permission
    pub execute: bool,
}

/// Memory sharing policy
#[derive(Debug, Clone)]
pub struct MemorySharingPolicy {
    /// Memory ID
    pub memory_id: u32,
    /// Sharing mode
    pub mode: SharingMode,
    /// Allowed component instances
    #[cfg(feature = "std")]
    pub allowed_instances: Vec<u32>,
    #[cfg(not(any(feature = "std", )))]
    pub allowed_instances: BoundedVec<u32, 32>,
}

/// Table sharing policy
#[derive(Debug, Clone)]
pub struct TableSharingPolicy {
    /// Table ID
    pub table_id: u32,
    /// Sharing mode
    pub mode: SharingMode,
    /// Allowed component instances
    #[cfg(feature = "std")]
    pub allowed_instances: Vec<u32>,
    #[cfg(not(any(feature = "std", )))]
    pub allowed_instances: BoundedVec<u32, 32>,
}

/// Resource sharing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharingMode {
    /// Private to single instance
    Private,
    /// Shared read-only
    ReadOnly,
    /// Shared read-write
    ReadWrite,
    /// Copy-on-write
    CopyOnWrite,
}

/// Component table instance
#[derive(Debug, Clone)]
pub struct ComponentTable {
    /// Table ID
    pub id: u32,
    /// Table elements
    #[cfg(feature = "std")]
    pub elements: Vec<TableElement>,
    #[cfg(not(any(feature = "std", )))]
    pub elements: BoundedVec<TableElement, 65536>, // 64K elements max
    /// Element type
    pub element_type: CoreValType,
    /// Table limits
    pub limits: TableLimits,
    /// Owner component instance
    pub owner: Option<u32>,
}

/// Table limits
#[derive(Debug, Clone, PartialEq)]
pub struct TableLimits {
    /// Minimum size
    pub min: u32,
    /// Maximum size (if any)
    pub max: Option<u32>,
}

/// Table element
#[derive(Debug, Clone)]
pub enum TableElement {
    /// Null reference
    Null,
    /// Function reference
    FuncRef(u32),
    /// External reference
    ExternRef(ComponentValue),
}

/// Memory access result
#[derive(Debug, Clone)]
pub struct MemoryAccess {
    /// Whether access was successful
    pub success: bool,
    /// Bytes read/written
    pub bytes_accessed: usize,
    /// Error message if failed
    pub error: Option<BoundedString<256>>,
}

impl ComponentMemoryManager {
    /// Create a new memory manager
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            memories: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            memories: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|_| wrt_error::Error::resource_exhausted("Failed to create memory manager"))?
            },
            #[cfg(feature = "std")]
            sharing_policies: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            sharing_policies: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|_| wrt_error::Error::resource_exhausted("Failed to create sharing policies"))?
            },
            total_allocated: 0,
            max_memory: 256 * 1024 * 1024, // 256MB default
        })
    }

    /// Set maximum memory limit
    pub fn set_max_memory(&mut self, max_memory: usize) {
        self.max_memory = max_memory;
    }

    /// Create a new memory instance
    pub fn create_memory(
        &mut self,
        limits: MemoryLimits,
        shared: bool,
        owner: Option<u32>,
    ) -> wrt_error::Result<u32> {
        let memory_id = self.memories.len() as u32;

        // Check memory limits
        let initial_size = limits.min as usize * WASM_PAGE_SIZE;
        if self.total_allocated + initial_size > self.max_memory {
            return Err(wrt_error::Error::resource_exhausted("Memory limit exceeded"))
            ;
        }

        // Create memory data
        #[cfg(feature = "std")]
        let data = vec![0u8; initial_size];
        #[cfg(not(any(feature = "std", )))]
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        #[cfg(not(any(feature = "std", )))]
        let mut data = BoundedVec::new();
        #[cfg(not(any(feature = "std", )))]
        {
            for _ in 0..initial_size {
                data.push(0u8).map_err(|_| {
                    wrt_error::Error::resource_exhausted("Memory allocation failed")
                })?;
                })?;
            }
        }

        let memory = ComponentMemory {
            id: memory_id,
            data,
            limits,
            shared,
            owner,
            permissions: MemoryPermissions::default(),
        };

        #[cfg(feature = "std")]
        {
            self.memories.push(memory);
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.memories.push(memory).map_err(|_| {
                wrt_error::Error::resource_exhausted("Too many memories")
            })?;
            })?;
        }

        self.total_allocated += initial_size;
        Ok(memory_id)
    }

    /// Get memory by ID
    pub fn get_memory(&self, memory_id: u32) -> Option<&ComponentMemory> {
        self.memories.get(memory_id as usize)
    }

    /// Get mutable memory by ID
    pub fn get_memory_mut(&mut self, memory_id: u32) -> Option<&mut ComponentMemory> {
        self.memories.get_mut(memory_id as usize)
    }

    /// Read from memory
    pub fn read_memory(
        &self,
        memory_id: u32,
        offset: u32,
        size: u32,
        instance_id: Option<u32>,
    ) -> wrt_error::Result<Vec<u8>> {
        let memory = self
            .get_memory(memory_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        // Check permissions
        if !self.check_read_permission(memory_id, instance_id)? {
            return Err(wrt_error::Error::runtime_error("Read permission denied"))
            ;
        }

        // Check bounds
        let end_offset = offset as usize + size as usize;
        if end_offset > memory.data.len() {
            return Err(wrt_error::Error::validation_invalid_input("Invalid input"))
            ;
        }

        // Read data
        #[cfg(feature = "std")]
        {
            Ok(memory.data[offset as usize..end_offset].to_vec()
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let mut result = Vec::new();
            for i in offset as usize..end_offset {
                result.push(memory.data[i]);
            }
            Ok(result)
        }
    }

    /// Write to memory
    pub fn write_memory(
        &mut self,
        memory_id: u32,
        offset: u32,
        data: &[u8],
        instance_id: Option<u32>,
    ) -> wrt_error::Result<MemoryAccess> {
        // Check permissions first
        if !self.check_write_permission(memory_id, instance_id)? {
            let provider = safe_managed_alloc!(512, CrateId::Component)?;
            return Ok(MemoryAccess {
                success: false,
                bytes_accessed: 0,
                error: Some(BoundedString::try_from_str("Write permission denied").unwrap_or_default()),
            };
        }

        let memory = self
            .get_memory_mut(memory_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        // Check bounds
        let end_offset = offset as usize + data.len();
        if end_offset > memory.data.len() {
            let provider = safe_managed_alloc!(512, CrateId::Component)?;
            return Ok(MemoryAccess {
                success: false,
                bytes_accessed: 0,
                error: Some(
                    BoundedString::try_from_str("Memory access out of bounds").unwrap_or_default(),
                ),
            };
        }

        // Write data
        for (i, &byte) in data.iter().enumerate() {
            memory.data[offset as usize + i] = byte;
        }

        Ok(MemoryAccess { success: true, bytes_accessed: data.len(), error: None })
    }

    /// Grow memory
    pub fn grow_memory(
        &mut self,
        memory_id: u32,
        pages: u32,
        instance_id: Option<u32>,
    ) -> wrt_error::Result<u32> {
        let memory = self
            .get_memory_mut(memory_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        // Check permissions
        if !self.check_write_permission(memory_id, instance_id)? {
            return Err(wrt_error::Error::runtime_error("Write permission denied"))
            ;
        }

        let current_pages = memory.data.len() / WASM_PAGE_SIZE;
        let new_pages = current_pages + pages as usize;

        // Check limits
        if let Some(max) = memory.limits.max {
            if new_pages > max as usize {
                return Err(wrt_error::Error::validation_invalid_input("Invalid input"))
                ;
            }
        }

        // Check global memory limit
        let additional_size = pages as usize * WASM_PAGE_SIZE;
        if self.total_allocated + additional_size > self.max_memory {
            return Err(wrt_error::Error::resource_exhausted("Memory limit exceeded"))
            ;
        }

        // Grow memory
        let old_size = memory.data.len();
        #[cfg(feature = "std")]
        {
            memory.data.resize(old_size + additional_size, 0);
        }
        #[cfg(not(any(feature = "std", )))]
        {
            for _ in 0..additional_size {
                memory.data.push(0u8).map_err(|_| {
                    wrt_error::Error::resource_exhausted("Memory allocation failed")
                })?;
                })?;
            }
        }

        self.total_allocated += additional_size;
        Ok(current_pages as u32)
    }

    /// Check read permission
    fn check_read_permission(&self, memory_id: u32, instance_id: Option<u32>) -> wrt_error::Result<bool> {
        let memory = self
            .get_memory(memory_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        if !memory.permissions.read {
            return Ok(false;
        }

        // Check sharing policy
        for policy in &self.sharing_policies {
            if policy.memory_id == memory_id {
                return self.check_instance_allowed(&policy.allowed_instances, instance_id;
            }
        }

        // If no policy, check ownership
        match (memory.owner, instance_id) {
            (Some(owner), Some(instance)) => Ok(owner == instance),
            (None, _) => Ok(true),        // Unowned memory is accessible
            (Some(_), None) => Ok(false), // Owned memory needs instance
        }
    }

    /// Check write permission
    fn check_write_permission(&self, memory_id: u32, instance_id: Option<u32>) -> wrt_error::Result<bool> {
        let memory = self
            .get_memory(memory_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        if !memory.permissions.write {
            return Ok(false;
        }

        // Check sharing policy
        for policy in &self.sharing_policies {
            if policy.memory_id == memory_id {
                match policy.mode {
                    SharingMode::Private => {
                        return Ok(memory.owner == instance_id;
                    }
                    SharingMode::ReadOnly => {
                        return Ok(false); // No write access in read-only mode
                    }
                    SharingMode::ReadWrite | SharingMode::CopyOnWrite => {
                        return self.check_instance_allowed(&policy.allowed_instances, instance_id;
                    }
                }
            }
        }

        // If no policy, check ownership
        match (memory.owner, instance_id) {
            (Some(owner), Some(instance)) => Ok(owner == instance),
            (None, _) => Ok(true),
            (Some(_), None) => Ok(false),
        }
    }

    /// Check if instance is allowed
    fn check_instance_allowed(
        &self,
        allowed_instances: &[u32],
        instance_id: Option<u32>,
    ) -> wrt_error::Result<bool> {
        match instance_id {
            Some(id) => Ok(allowed_instances.contains(&id)),
            None => Ok(false),
        }
    }

    /// Set memory sharing policy
    pub fn set_sharing_policy(&mut self, policy: MemorySharingPolicy) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            self.sharing_policies.push(policy);
            Ok(()
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.sharing_policies.push(policy).map_err(|_| {
                wrt_error::Error::resource_exhausted("Too many sharing policies")
            })?;
            })
        }
    }

    /// Binary std/no_std choice
    pub fn total_allocated(&self) -> usize {
        self.total_allocated
    }

    /// Get memory count
    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }
}

impl ComponentTableManager {
    /// Create a new table manager
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(feature = "std")]
            tables: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            tables: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|_| wrt_error::Error::resource_exhausted("Failed to create table manager"))?
            },
            #[cfg(feature = "std")]
            sharing_policies: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            sharing_policies: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().map_err(|_| wrt_error::Error::resource_exhausted("Failed to create sharing policies"))?
            },
        })
    }

    /// Create a new table
    pub fn create_table(
        &mut self,
        element_type: CoreValType,
        limits: TableLimits,
        owner: Option<u32>,
    ) -> wrt_error::Result<u32> {
        let table_id = self.tables.len() as u32;

        // Create table elements
        #[cfg(feature = "std")]
        let elements = vec![TableElement::Null; limits.min as usize];
        #[cfg(not(any(feature = "std", )))]
        let provider = safe_managed_alloc!(65536, CrateId::Component)?;
        #[cfg(not(any(feature = "std", )))]
        let mut elements = BoundedVec::new();
        #[cfg(not(any(feature = "std", )))]
        {
            for _ in 0..limits.min {
                elements.push(TableElement::Null).map_err(|_| {
                    wrt_error::Error::resource_exhausted("Table allocation failed")
                })?;
                })?;
            }
        }

        let table = ComponentTable { id: table_id, elements, element_type, limits, owner };

        #[cfg(feature = "std")]
        {
            self.tables.push(table);
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.tables.push(table).map_err(|_| {
                wrt_error::Error::resource_exhausted("Too many tables")
            })?;
            })?;
        }

        Ok(table_id)
    }

    /// Get table by ID
    pub fn get_table(&self, table_id: u32) -> Option<&ComponentTable> {
        self.tables.get(table_id as usize)
    }

    /// Get mutable table by ID
    pub fn get_table_mut(&mut self, table_id: u32) -> Option<&mut ComponentTable> {
        self.tables.get_mut(table_id as usize)
    }

    /// Get table element
    pub fn get_element(&self, table_id: u32, index: u32) -> wrt_error::Result<&TableElement> {
        let table = self
            .get_table(table_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        table.elements.get(index as usize).ok_or_else(|| {
            wrt_error::Error::validation_invalid_input("Invalid input")
            )
        })
    }

    /// Set table element
    pub fn set_element(
        &mut self,
        table_id: u32,
        index: u32,
        element: TableElement,
    ) -> wrt_error::Result<()> {
        let table = self
            .get_table_mut(table_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        if index as usize >= table.elements.len() {
            return Err(wrt_error::Error::validation_invalid_input("Invalid input"))
            ;
        }

        table.elements[index as usize] = element;
        Ok(()
    }

    /// Grow table
    pub fn grow_table(&mut self, table_id: u32, size: u32, init: TableElement) -> wrt_error::Result<u32> {
        let table = self
            .get_table_mut(table_id)
            .ok_or_else(|| wrt_error::Error::validation_invalid_input("Invalid input"))
            ))?;

        let current_size = table.elements.len();
        let new_size = current_size + size as usize;

        // Check limits
        if let Some(max) = table.limits.max {
            if new_size > max as usize {
                return Err(wrt_error::Error::validation_invalid_input("Invalid input"))
            ;
            }
        }

        // Grow table
        #[cfg(feature = "std")]
        {
            table.elements.resize(new_size, init;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            for _ in 0..size {
                table.elements.push(init.clone()).map_err(|_| {
                    wrt_error::Error::resource_exhausted("Table allocation failed")
                })?;
                })?;
            }
        }

        Ok(current_size as u32)
    }

    /// Set table sharing policy
    pub fn set_sharing_policy(&mut self, policy: TableSharingPolicy) -> wrt_error::Result<()> {
        #[cfg(feature = "std")]
        {
            self.sharing_policies.push(policy);
            Ok(()
        }
        #[cfg(not(any(feature = "std", )))]
        {
            self.sharing_policies.push(policy).map_err(|_| {
                wrt_error::Error::resource_exhausted("Too many sharing policies")
            })?;
            })
        }
    }

    /// Get table count
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }
}

impl Default for ComponentMemoryManager {
    fn default() -> Self {
        // NOTE: Default trait cannot return Result, so we panic on allocation failure
        // In practice, use ComponentMemoryManager::new() for proper error handling
        Self::new().expect("Failed to create default ComponentMemoryManager")
    }
}

impl Default for ComponentTableManager {
    fn default() -> Self {
        // NOTE: Default trait cannot return Result, so we panic on allocation failure
        // In practice, use ComponentTableManager::new() for proper error handling
        Self::new().expect("Failed to create default ComponentTableManager")
    }
}

impl Default for MemoryPermissions {
    fn default() -> Self {
        Self { read: true, write: true, execute: false }
    }
}

impl fmt::Display for SharingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SharingMode::Private => write!(f, "private"),
            SharingMode::ReadOnly => write!(f, "readonly"),
            SharingMode::ReadWrite => write!(f, "readwrite"),
            SharingMode::CopyOnWrite => write!(f, "copyonwrite"),
        }
