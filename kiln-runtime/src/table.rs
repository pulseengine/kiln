//! WebAssembly table implementation.
//!
//! This module provides an implementation of WebAssembly tables,
//! which store function references or externref values.

// alloc is imported in lib.rs with proper feature gates

use kiln_foundation::{
    bounded::BoundedVec,
    safe_memory::NoStdMemoryProvider,
    types::{
        Limits as KilnLimits,
        RefType as KilnRefType,
        TableType as KilnTableType,
        ValueType as KilnValueType,
    },
    values::{
        ExternRef as KilnExternRef,
        FuncRef as KilnFuncRef,
        Value as KilnValue,
    },
    // Use clean collections instead of runtime allocator types
    verification::VerificationLevel,
};

// Platform-aware memory provider for table operations (kept for no_std compatibility)
#[allow(dead_code)]
type TableProvider = kiln_foundation::safe_memory::NoStdProvider<8192>;

use std::format;

// Import the TableOperations trait from kiln-instructions
use kiln_instructions::table_ops::TableOperations;

use crate::prelude::{
    Arc,
    BoundedCapacity,
    Debug,
    Eq,
    Error,
    ErrorCategory,
    Ord,
    PartialEq,
    Result,
    RuntimeString,
    TryFrom,
};

// Sync primitives for interior mutability
use std::sync::Mutex;

/// Invalid index error code
const INVALID_INDEX: u16 = 4004;
/// Index too large error code  
const INDEX_TOO_LARGE: u16 = 4005;

/// Safe conversion from WebAssembly u32 index to Rust usize
///
/// # Arguments
///
/// * `index` - WebAssembly index as u32
///
/// # Returns
///
/// Ok(usize) if conversion is safe, error otherwise
fn wasm_index_to_usize(index: u32) -> Result<usize> {
    usize::try_from(index).map_err(|_| Error::runtime_execution_error("Index conversion failed"))
}

/// Safe conversion from Rust usize to WebAssembly u32
///
/// # Arguments
///
/// * `size` - Rust size as usize
///
/// # Returns
///
/// Ok(u32) if conversion is safe, error otherwise  
fn usize_to_wasm_u32(size: usize) -> Result<u32> {
    u32::try_from(size).map_err(|_| {
        Error::new(
            ErrorCategory::Runtime,
            INDEX_TOO_LARGE,
            "Size too large for WebAssembly u32",
        )
    })
}

/// Type alias for the inner elements storage.
/// Uses Vec for std builds to preserve GC ref identity (Arc pointers)
/// through table.get/set operations. BoundedVec serialization loses Arc
/// identity which breaks ref.eq and GC field access after table round-trips.
type TableElements = Vec<Option<KilnValue>>;

/// A WebAssembly table is a vector of opaque values of a single type.
/// Uses interior mutability (Mutex) for thread-safe element access.
pub struct Table {
    /// The table type, using the canonical `KilnTableType`
    pub ty:                 KilnTableType,
    /// The table elements - wrapped in Mutex for interior mutability
    /// This allows setting elements through Arc<Table> references
    elements: Mutex<TableElements>,
    /// A debug name for the table (optional)
    pub debug_name:         Option<RuntimeString>,
    /// Verification level for table operations
    pub verification_level: VerificationLevel,
}

impl Debug for Table {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let elements_len = self.elements.lock().map(|e| e.len()).unwrap_or(0);

        f.debug_struct("Table")
            .field("ty", &self.ty)
            .field("elements_len", &elements_len)
            .field("debug_name", &self.debug_name)
            .field("verification_level", &self.verification_level)
            .finish()
    }
}

impl Clone for Table {
    fn clone(&self) -> Self {
        let source_elements = self.elements.lock()
            .expect("Table mutex poisoned during clone");
        Self {
            ty:                 self.ty.clone(),
            elements:           Mutex::new(source_elements.clone()),
            debug_name:         self.debug_name.clone(),
            verification_level: self.verification_level,
        }
    }
}

impl PartialEq for Table {
    fn eq(&self, other: &Self) -> bool {
        if self.ty != other.ty
            || self.debug_name != other.debug_name
            || self.verification_level != other.verification_level
        {
            return false;
        }
        let self_elements = self.elements.lock()
            .expect("Table mutex poisoned during comparison");
        let other_elements = other.elements.lock()
            .expect("Table mutex poisoned during comparison");
        *self_elements == *other_elements
    }
}

impl Eq for Table {}

impl Default for Table {
    fn default() -> Self {
        use kiln_foundation::types::{
            Limits,
            TableType,
        };
        let table_type = TableType {
            element_type: KilnRefType::Funcref,
            limits:       Limits {
                min: 0,
                max: Some(1),
            },
            table64: false,
        };
        Self::new(table_type).expect("Failed to create default Table")
    }
}

impl kiln_foundation::traits::Checksummable for Table {
    fn update_checksum(&self, checksum: &mut kiln_foundation::verification::Checksum) {
        let element_type_byte = match self.ty.element_type {
            KilnRefType::Funcref => 0u8,
            KilnRefType::Externref => 1u8,
            KilnRefType::Gc(_) => 2u8,
        };
        checksum.update_slice(&element_type_byte.to_le_bytes());
        checksum.update_slice(&self.ty.limits.min.to_le_bytes());
        if let Some(max) = self.ty.limits.max {
            checksum.update_slice(&max.to_le_bytes());
        }
    }
}

impl kiln_foundation::traits::ToBytes for Table {
    fn serialized_size(&self) -> usize {
        16 // simplified
    }

    fn to_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        &self,
        writer: &mut kiln_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> Result<()> {
        let element_type_byte = match self.ty.element_type {
            KilnRefType::Funcref => 0u8,
            KilnRefType::Externref => 1u8,
            KilnRefType::Gc(_) => 2u8,
        };
        writer.write_all(&element_type_byte.to_le_bytes())?;
        writer.write_all(&self.ty.limits.min.to_le_bytes())
    }
}

impl kiln_foundation::traits::FromBytes for Table {
    fn from_bytes_with_provider<P: kiln_foundation::MemoryProvider>(
        reader: &mut kiln_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> Result<Self> {
        let mut bytes = [0u8; 1];
        reader.read_exact(&mut bytes)?;
        let element_type = match bytes[0] {
            0 => kiln_foundation::types::RefType::Funcref,
            _ => kiln_foundation::types::RefType::Externref,
        };

        let mut min_bytes = [0u8; 4];
        reader.read_exact(&mut min_bytes)?;
        let min = u32::from_le_bytes(min_bytes);

        use kiln_foundation::types::{
            Limits,
            TableType,
        };
        let table_type = TableType {
            element_type,
            limits: Limits {
                min,
                max: Some(min + 1),
            },
            table64: false,
        };
        Self::new(table_type)
    }
}

impl Table {
    /// Creates a new table with the specified type.
    /// Elements are initialized to a type-appropriate null value.
    pub fn new(ty: KilnTableType) -> Result<Self> {
        // Determine the type-appropriate null value for initialization
        let init_val = match &ty.element_type {
            KilnRefType::Funcref => Some(KilnValue::FuncRef(None)),
            KilnRefType::Externref => Some(KilnValue::ExternRef(None)),
            KilnRefType::Gc(gc) => {
                use kiln_foundation::types::HeapType;
                match gc.heap_type {
                    HeapType::Func | HeapType::NoFunc | HeapType::Concrete(_) => Some(KilnValue::FuncRef(None)),
                    HeapType::Extern | HeapType::NoExtern => Some(KilnValue::ExternRef(None)),
                    HeapType::I31 | HeapType::Eq | HeapType::Any | HeapType::None => Some(KilnValue::I31Ref(None)),
                    HeapType::Struct => Some(KilnValue::StructRef(None)),
                    HeapType::Array => Some(KilnValue::ArrayRef(None)),
                    HeapType::Exn => Some(KilnValue::ExnRef(None)),
                }
            },
        };

        let initial_size = wasm_index_to_usize(ty.limits.min)?;

        #[cfg(feature = "tracing")]
        kiln_foundation::tracing::trace!(elements = initial_size, "Creating Table Vec");

        // Use Vec for direct value storage — preserves GC ref identity (Arc pointers)
        let elements: TableElements = vec![init_val; initial_size];

        Ok(Self {
            ty,
            elements: Mutex::new(elements),
            verification_level: VerificationLevel::default(),
            debug_name: None,
        })
    }

    /// Creates a new table with the specified capacity and element type
    ///
    /// # Arguments
    ///
    /// * `capacity` - The initial capacity of the table
    /// * `element_type` - The element type for the table
    ///
    /// # Returns
    ///
    /// A new table instance
    ///
    /// # Errors
    ///
    /// Returns an error if the table cannot be created
    pub fn with_capacity(capacity: u32, element_type: &KilnRefType) -> Result<Self> {
        let table_type = KilnTableType {
            element_type: *element_type,
            limits:       KilnLimits {
                min: capacity,
                max: Some(capacity),
            },
            table64: false,
        };
        Self::new(table_type)
    }

    /// Gets the size of the table
    ///
    /// # Returns
    ///
    /// The current size of the table
    #[must_use]
    pub fn size(&self) -> u32 {
        let len = self.elements.lock().map(|e| e.len()).unwrap_or(0);
        usize_to_wasm_u32(len).unwrap_or(0)
    }

    /// Gets an element from the table
    ///
    /// # Arguments
    ///
    /// * `idx` - The index to get
    ///
    /// # Returns
    ///
    /// The element at the given index or None if it hasn't been set
    ///
    /// # Errors
    ///
    /// Returns an error if the index is out of bounds
    pub fn get(&self, idx: u32) -> Result<Option<KilnValue>> {
        let idx_usize = wasm_index_to_usize(idx)?;

        let elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        if idx_usize >= elements.len() {
            return Err(Error::invalid_function_index("Table access out of bounds"));
        }

        // Direct Vec access — returns clone of the value, preserving Arc identity
        Ok(elements[idx_usize].clone())
    }

    /// Sets an element at the specified index
    ///
    /// # Arguments
    ///
    /// * `idx` - The index to set
    /// * `value` - The value to set
    ///
    /// # Returns
    ///
    /// Ok(()) if the set was successful
    ///
    /// # Errors
    ///
    /// Returns an error if the index is out of bounds or if the value type
    /// doesn't match the table element type
    pub fn set(&mut self, idx: u32, value: Option<KilnValue>) -> Result<()> {
        let idx_usize = wasm_index_to_usize(idx)?;

        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        if idx_usize >= elements.len() {
            return Err(Error::invalid_function_index("Table access out of bounds"));
        }

        if let Some(ref val) = value {
            let val_matches = matches!(
                (&val, &self.ty.element_type),
                (KilnValue::FuncRef(_), KilnRefType::Funcref)
                | (KilnValue::FuncRef(_), KilnRefType::Gc(_))
                | (KilnValue::ExternRef(_), KilnRefType::Externref)
                | (KilnValue::ExternRef(_), KilnRefType::Gc(_))
                | (KilnValue::I31Ref(_), KilnRefType::Gc(_))
                | (KilnValue::StructRef(_), KilnRefType::Gc(_))
                | (KilnValue::ArrayRef(_), KilnRefType::Gc(_))
            );
            if !val_matches {
                return Err(Error::validation_error(
                    "Element value type doesn't match table element type",
                ));
            }
        }
        // Direct Vec assignment — preserves Arc identity for GC refs
        elements[idx_usize] = value;
        Ok(())
    }

    /// Sets an element at the specified index through a shared reference.
    pub fn set_shared(&self, idx: u32, value: Option<KilnValue>) -> Result<()> {
        let idx_usize = wasm_index_to_usize(idx)?;

        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        if idx_usize >= elements.len() {
            return Err(Error::invalid_function_index("Table access out of bounds"));
        }

        // Direct Vec assignment — preserves Arc identity for GC refs
        elements[idx_usize] = value;
        Ok(())
    }

    /// Grows the table by the given number of elements through a shared reference.
    /// This method provides interior mutability for use when the table is
    /// wrapped in an Arc.
    ///
    /// # Arguments
    ///
    /// * `delta` - The number of elements to grow by
    /// * `init_value` - The value to initialize new elements with
    ///
    /// # Returns
    ///
    /// The previous size of the table
    ///
    /// # Errors
    ///
    /// Returns an error if the table cannot be grown
    pub fn grow_shared(&self, delta: u32, init_value_from_arg: KilnValue) -> Result<u32> {
        let init_val_matches = matches!(
            (&init_value_from_arg, &self.ty.element_type),
            (KilnValue::FuncRef(_), KilnRefType::Funcref)
            | (KilnValue::ExternRef(_), KilnRefType::Externref)
            | (KilnValue::ExternRef(_), KilnRefType::Gc(_))
            | (KilnValue::I31Ref(_), KilnRefType::Gc(_))
            | (KilnValue::StructRef(_), KilnRefType::Gc(_))
            | (KilnValue::ArrayRef(_), KilnRefType::Gc(_))
        );
        if !init_val_matches {
            return Err(Error::validation_error(
                "Grow operation init value type doesn't match table element type",
            ));
        }

        let old_size = self.size();
        let new_size = old_size
            .checked_add(delta)
            .ok_or_else(|| Error::runtime_execution_error("Table size overflow"))?;

        if let Some(max) = self.ty.limits.max {
            if new_size > max {
                // As per spec, grow should return -1 (or an error indicating failure)
                return Err(Error::new(
                    ErrorCategory::Runtime,
                    kiln_error::codes::CAPACITY_EXCEEDED,
                    "Table size exceeds maximum limit",
                ));
            }
        }

        // Lock elements and push new values
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        for _ in 0..delta {
            elements.push(Some(init_value_from_arg.clone()));
        }

        Ok(old_size)
    }

    /// Fill a range of elements with a given value through a shared reference.
    /// This method provides interior mutability for use when the table is
    /// wrapped in an Arc.
    pub fn fill_elements_shared(
        &self,
        offset: usize,
        value: Option<KilnValue>,
        len: usize,
    ) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        // Verify bounds - use checked arithmetic to prevent overflow
        let end = offset.checked_add(len)
            .ok_or_else(|| Error::runtime_trap("out of bounds table access"))?;
        if end > elements.len() {
            return Err(Error::runtime_trap("out of bounds table access"));
        }

        // Handle empty fill (after bounds check per spec)
        if len == 0 {
            return Ok(());
        }

        // Fill the range directly
        for i in offset..offset + len {
            elements[i] = value.clone();
        }

        Ok(())
    }

    /// Copy elements from one region of a table to another through a shared reference.
    /// This method provides interior mutability for use when the table is
    /// wrapped in an Arc.
    pub fn copy_elements_shared(&self, dst: usize, src: usize, len: usize) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        // Verify bounds - use checked arithmetic to prevent overflow
        let src_end = src.checked_add(len)
            .ok_or_else(|| Error::runtime_trap("out of bounds table access"))?;
        let dst_end = dst.checked_add(len)
            .ok_or_else(|| Error::runtime_trap("out of bounds table access"))?;
        if src_end > elements.len() || dst_end > elements.len() {
            return Err(Error::runtime_trap("out of bounds table access"));
        }

        // Handle the case where no elements to copy (AFTER bounds check per spec)
        if len == 0 {
            return Ok(());
        }

        // Copy source elements to temp buffer, then write to destination
        let temp: Vec<Option<KilnValue>> = (0..len)
            .map(|i| elements[src + i].clone())
            .collect();

        for (i, val) in temp.into_iter().enumerate() {
            elements[dst + i] = val;
        }

        Ok(())
    }

    /// Initialize a range of elements in the table through a shared reference.
    /// This method provides interior mutability for use when the table is
    /// wrapped in an Arc.
    pub fn init_shared(&self, offset: u32, init_data: &[Option<KilnValue>]) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        if offset as usize + init_data.len() > elements.len() {
            return Err(Error::runtime_out_of_bounds(
                "Table initialization out of bounds",
            ));
        }
        for (i, val_opt) in init_data.iter().enumerate() {
            if let Some(val) = val_opt {
                let val_matches = matches!((&val, &self.ty.element_type), (KilnValue::FuncRef(_), KilnRefType::Funcref) | (KilnValue::ExternRef(_), KilnRefType::Externref) | (KilnValue::ExternRef(_), KilnRefType::Gc(_)));
                if !val_matches {
                    return Err(Error::validation_error("Table init value type mismatch"));
                }
            }
            elements[(offset as usize) + i] = val_opt.clone();
        }
        Ok(())
    }

    /// Grows the table by the given number of elements
    ///
    /// # Arguments
    ///
    /// * `delta` - The number of elements to grow by
    /// * `init_value` - The value to initialize new elements with
    ///
    /// # Returns
    ///
    /// The previous size of the table
    ///
    /// # Errors
    ///
    /// Returns an error if the table cannot be grown
    pub fn grow(&mut self, delta: u32, init_value_from_arg: KilnValue) -> Result<u32> {
        let init_val_matches = matches!(
            (&init_value_from_arg, &self.ty.element_type),
            (KilnValue::FuncRef(_), KilnRefType::Funcref)
            | (KilnValue::ExternRef(_), KilnRefType::Externref)
            | (KilnValue::ExternRef(_), KilnRefType::Gc(_))
            | (KilnValue::I31Ref(_), KilnRefType::Gc(_))
            | (KilnValue::StructRef(_), KilnRefType::Gc(_))
            | (KilnValue::ArrayRef(_), KilnRefType::Gc(_))
        );
        if !init_val_matches {
            return Err(Error::validation_error(
                "Grow operation init value type doesn't match table element type",
            ));
        }

        let old_size = self.size();
        let new_size = old_size
            .checked_add(delta)
            .ok_or_else(|| Error::runtime_execution_error("Table size overflow"))?;

        if let Some(max) = self.ty.limits.max {
            if new_size > max {
                // As per spec, grow should return -1 (or an error indicating failure)
                // For now, let's return an error. The runtime execution might interpret this.
                return Err(Error::new(
                    ErrorCategory::Runtime,
                    kiln_error::codes::CAPACITY_EXCEEDED,
                    "Table size exceeds maximum limit",
                ));
            }
        }

        // Lock elements and push new values
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        for _ in 0..delta {
            elements.push(Some(init_value_from_arg.clone()));
        }
        // Update the min limit in the table type if it changes due to growth (spec is a
        // bit unclear if ty should reflect current size) For now, ty.limits.min
        // reflects the *initial* min. Current size is self.size().

        Ok(old_size)
    }

    /// Sets a function reference in the table
    ///
    /// # Arguments
    ///
    /// * `idx` - The index to set
    /// * `func_idx` - The function index to set
    ///
    /// # Returns
    ///
    /// Ok(()) if the operation was successful
    ///
    /// # Errors
    ///
    /// Returns an error if the index is out of bounds or the table element type
    /// isn't a funcref
    pub fn set_func(&mut self, idx: u32, func_idx: u32) -> Result<()> {
        if !matches!(self.ty.element_type, KilnRefType::Funcref) {
            return Err(Error::runtime_execution_error(
                "Table element type must be funcref",
            ));
        }
        self.set(
            idx,
            Some(KilnValue::FuncRef(Some(KilnFuncRef::from_index(func_idx)))),
        )
    }

    /// Initialize a range of elements in the table
    ///
    /// # Arguments
    ///
    /// * `offset` - The starting offset
    /// * `init` - The elements to initialize with
    ///
    /// # Returns
    ///
    /// Ok(()) if successful
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails
    pub fn init(&mut self, offset: u32, init_data: &[Option<KilnValue>]) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        if offset as usize + init_data.len() > elements.len() {
            return Err(Error::runtime_out_of_bounds(
                "Table initialization out of bounds",
            ));
        }
        for (i, val_opt) in init_data.iter().enumerate() {
            if let Some(val) = val_opt {
                let val_matches = matches!((&val, &self.ty.element_type), (KilnValue::FuncRef(_), KilnRefType::Funcref) | (KilnValue::ExternRef(_), KilnRefType::Externref) | (KilnValue::ExternRef(_), KilnRefType::Gc(_)));
                if !val_matches {
                    return Err(Error::validation_error("Table init value type mismatch"));
                }
            }
            elements[(offset as usize) + i] = val_opt.clone();
        }
        Ok(())
    }

    /// Copy elements from one region of a table to another
    pub fn copy_elements(&mut self, dst: usize, src: usize, len: usize) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        // Verify bounds - use checked arithmetic to prevent overflow
        let src_end = src.checked_add(len)
            .ok_or_else(|| Error::runtime_trap("out of bounds table access"))?;
        let dst_end = dst.checked_add(len)
            .ok_or_else(|| Error::runtime_trap("out of bounds table access"))?;
        if src_end > elements.len() || dst_end > elements.len() {
            return Err(Error::runtime_trap("out of bounds table access"));
        }

        // Handle the case where regions don't overlap or no elements to copy (AFTER bounds check per spec)
        if len == 0 {
            return Ok(());
        }

        // Copy source elements to temp buffer, then write to destination
        let temp: Vec<Option<KilnValue>> = (0..len)
            .map(|i| elements[src + i].clone())
            .collect();
        for (i, val) in temp.into_iter().enumerate() {
            elements[dst + i] = val;
        }

        Ok(())
    }

    /// Fill a range of elements with a given value
    pub fn fill_elements(
        &mut self,
        offset: usize,
        value: Option<KilnValue>,
        len: usize,
    ) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        let end = offset.checked_add(len)
            .ok_or_else(|| Error::runtime_trap("out of bounds table access"))?;
        if end > elements.len() {
            return Err(Error::runtime_trap("out of bounds table access"));
        }

        if len == 0 {
            return Ok(());
        }

        for i in offset..offset + len {
            elements[i] = value.clone();
        }

        Ok(())
    }

    /// Sets the verification level for this table
    ///
    /// # Arguments
    ///
    /// * `level` - The verification level to set
    pub fn set_verification_level(&mut self, level: VerificationLevel) {
        self.verification_level = level;
        // Note: BoundedVec doesn't have set_verification_level method
        // The verification level is tracked at the Table level
    }

    /// Gets the current verification level for this table
    ///
    /// # Returns
    ///
    /// The current verification level
    #[must_use]
    pub fn verification_level(&self) -> VerificationLevel {
        self.verification_level
    }

    /// Sets an element at the given index.
    pub fn init_element(&mut self, idx: usize, value: Option<KilnValue>) -> Result<()> {
        let mut elements = self.elements.lock()
            .map_err(|_| Error::runtime_error("Failed to lock table elements"))?;

        // Check bounds
        if idx >= elements.len() {
            return Err(Error::runtime_trap("out of bounds table access"));
        }

        // Set the element directly
        elements[idx] = value;

        Ok(())
    }

    /// Get safety statistics for this table instance
    ///
    /// This returns detailed statistics about table usage and safety checks
    ///
    /// # Returns
    ///
    /// A string containing the statistics
    pub fn safety_stats(&self) -> kiln_foundation::bounded::BoundedString<256> {
        let stats_text = "Table Safety Stats: [Runtime table]";
        kiln_foundation::bounded::BoundedString::try_from_str(stats_text)
            .unwrap_or_default()
    }
}

/// Extension trait for `Arc<Table>` to simplify access to table operations
pub trait ArcTableExt {
    /// Get the size of the table
    fn size(&self) -> u32;

    /// Get an element from the table
    fn get(&self, idx: u32) -> Result<Option<KilnValue>>;

    /// Set an element in the table
    fn set(&self, idx: u32, value: Option<KilnValue>) -> Result<()>;

    /// Grow the table by a given number of elements
    fn grow(&self, delta: u32, init_value: KilnValue) -> Result<u32>;

    /// Set a function reference in the table
    fn set_func(&self, idx: u32, func_idx: u32) -> Result<()>;

    /// Initialize a range of elements from a vector
    fn init(&self, offset: u32, init: &[Option<KilnValue>]) -> Result<()>;

    /// Copy elements from one range to another
    fn copy(&self, dst: u32, src: u32, len: u32) -> Result<()>;

    /// Fill a range of elements with a value
    fn fill(&self, offset: u32, len: u32, value: Option<KilnValue>) -> Result<()>;
}

impl ArcTableExt for Arc<Table> {
    fn size(&self) -> u32 {
        self.as_ref().size()
    }

    fn get(&self, idx: u32) -> Result<Option<KilnValue>> {
        self.as_ref().get(idx)
    }

    fn set(&self, idx: u32, value: Option<KilnValue>) -> Result<()> {
        // Use interior mutability via Mutex — preserves Arc identity
        self.as_ref().set_shared(idx, value)
    }

    fn grow(&self, delta: u32, init_value: KilnValue) -> Result<u32> {
        // Use interior mutability via Mutex
        self.as_ref().grow_shared(delta, init_value)
    }

    fn set_func(&self, idx: u32, func_idx: u32) -> Result<()> {
        // Use interior mutability via Mutex
        self.as_ref().set_shared(idx, Some(KilnValue::FuncRef(Some(
            kiln_foundation::values::FuncRef::from_index(func_idx),
        ))))
    }

    fn init(&self, offset: u32, init: &[Option<KilnValue>]) -> Result<()> {
        self.as_ref().init_shared(offset, init)
    }

    fn copy(&self, dst: u32, src: u32, len: u32) -> Result<()> {
        self.as_ref().copy_elements_shared(dst as usize, src as usize, len as usize)
    }

    fn fill(&self, offset: u32, len: u32, value: Option<KilnValue>) -> Result<()> {
        self.as_ref().fill_elements_shared(offset as usize, value, len as usize)
    }
}

/// Table manager to handle multiple tables for `TableOperations` trait
#[derive(Debug)]
pub struct TableManager {
    tables: kiln_foundation::bounded::BoundedVec<Table, 1024, TableProvider>,
}

impl TableManager {
    /// Create a new table manager
    pub fn new() -> Result<Self> {
        Ok(Self {
            tables: kiln_foundation::bounded::BoundedVec::new(TableProvider::default())?,
        })
    }

    /// Add a table to the manager
    pub fn add_table(&mut self, table: Table) -> u32 {
        let index = self.tables.len() as u32;
        self.tables.push(table).expect("Failed to add table to manager");
        index
    }

    /// Get a table by index
    pub fn get_table(&self, index: u32) -> Result<Table> {
        let table = self
            .tables
            .get(index as usize)
            .map_err(|_| Error::invalid_function_index("Table index out of bounds"))?;
        Ok(table)
    }

    /// Get a mutable table by index
    pub fn get_table_mut(&mut self, index: u32) -> Result<&mut Table> {
        if index as usize >= self.tables.len() {
            return Err(Error::invalid_function_index("Table index out of bounds"));
        }
        // Since BoundedVec doesn't have get_mut, we need to work around this
        // For now, return an error indicating this operation is not supported
        Err(Error::runtime_error(
            "Mutable table access not supported with current BoundedVec implementation",
        ))
    }

    /// Get the number of tables
    pub fn table_count(&self) -> u32 {
        self.tables.len() as u32
    }
}

impl Default for TableManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default TableManager")
    }
}

impl Clone for TableManager {
    fn clone(&self) -> Self {
        Self {
            tables: self.tables.clone(),
        }
    }
}

// TableOperations trait implementation is temporarily disabled due to complex
// type conversions This will be re-enabled once the Value types are properly
// unified across crates

