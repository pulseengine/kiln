//! GC Object representation and layout
//!
//! Defines the in-memory representation of garbage-collected objects
//! including structs and arrays.

use wrt_error::{Error, Result};
use wrt_foundation::types::{FieldType, ValueType};

/// Size of object header in bytes
pub const HEADER_SIZE: usize = 8;

/// Object header - metadata for garbage-collected objects
///
/// Layout (8 bytes total):
/// - Byte 0: Flags (mark bit, object kind)
/// - Bytes 1-3: Reserved / padding
/// - Bytes 4-7: Size in bytes (including header)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ObjectHeader {
    /// Object flags (bit 0 = mark bit, bits 1-2 = object kind)
    pub flags: u8,
    /// Reserved for future use
    pub reserved: [u8; 3],
    /// Total object size including header
    pub size: u32,
}

/// Object kind stored in header flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ObjectKind {
    /// A struct instance
    Struct = 0,
    /// An array instance
    Array = 1,
    /// An i31 value (not actually stored in heap, but useful for type checks)
    I31 = 2,
}

impl ObjectHeader {
    /// Create a new object header
    #[inline]
    pub const fn new(kind: ObjectKind, size: u32) -> Self {
        Self {
            flags: kind as u8,
            reserved: [0; 3],
            size,
        }
    }

    /// Get the object kind
    #[inline]
    pub const fn kind(&self) -> ObjectKind {
        match self.flags & 0x03 {
            0 => ObjectKind::Struct,
            1 => ObjectKind::Array,
            2 => ObjectKind::I31,
            _ => ObjectKind::Struct, // Default fallback
        }
    }

    /// Check if the object is marked (for GC)
    #[inline]
    pub const fn is_marked(&self) -> bool {
        (self.flags & 0x80) != 0
    }

    /// Set the mark bit
    #[inline]
    pub fn set_marked(&mut self, marked: bool) {
        if marked {
            self.flags |= 0x80;
        } else {
            self.flags &= !0x80;
        }
    }

    /// Get the total object size including header
    #[inline]
    pub const fn size(&self) -> u32 {
        self.size
    }

    /// Get the payload size (size minus header)
    #[inline]
    pub const fn payload_size(&self) -> u32 {
        self.size.saturating_sub(HEADER_SIZE as u32)
    }
}

/// Reference to a GC object in the heap
#[derive(Debug, Clone, Copy)]
pub struct GcObjectRef<'a> {
    /// Pointer to the object data in the heap
    data: &'a [u8],
    /// Type index for this object
    type_idx: u32,
}

impl<'a> GcObjectRef<'a> {
    /// Create a new GC object reference
    ///
    /// # Safety
    /// The data slice must contain a valid object header followed by payload.
    pub fn new(data: &'a [u8], type_idx: u32) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(Error::memory_error("Object data too small for header"));
        }
        Ok(Self { data, type_idx })
    }

    /// Get the object header
    pub fn header(&self) -> ObjectHeader {
        // Safe because we verified length in new()
        ObjectHeader {
            flags: self.data[0],
            reserved: [self.data[1], self.data[2], self.data[3]],
            size: u32::from_le_bytes([
                self.data[4],
                self.data[5],
                self.data[6],
                self.data[7],
            ]),
        }
    }

    /// Get the type index
    #[inline]
    pub const fn type_idx(&self) -> u32 {
        self.type_idx
    }

    /// Get the payload bytes (after header)
    pub fn payload(&self) -> &[u8] {
        &self.data[HEADER_SIZE..]
    }
}

/// Mutable reference to a GC object in the heap
pub struct GcObjectMut<'a> {
    /// Mutable pointer to the object data in the heap
    data: &'a mut [u8],
    /// Type index for this object
    type_idx: u32,
}

impl<'a> GcObjectMut<'a> {
    /// Create a new mutable GC object reference
    pub fn new(data: &'a mut [u8], type_idx: u32) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(Error::memory_error("Object data too small for header"));
        }
        Ok(Self { data, type_idx })
    }

    /// Get the object header
    pub fn header(&self) -> ObjectHeader {
        ObjectHeader {
            flags: self.data[0],
            reserved: [self.data[1], self.data[2], self.data[3]],
            size: u32::from_le_bytes([
                self.data[4],
                self.data[5],
                self.data[6],
                self.data[7],
            ]),
        }
    }

    /// Set the object header
    pub fn set_header(&mut self, header: ObjectHeader) {
        self.data[0] = header.flags;
        self.data[1] = header.reserved[0];
        self.data[2] = header.reserved[1];
        self.data[3] = header.reserved[2];
        let size_bytes = header.size.to_le_bytes();
        self.data[4] = size_bytes[0];
        self.data[5] = size_bytes[1];
        self.data[6] = size_bytes[2];
        self.data[7] = size_bytes[3];
    }

    /// Get the type index
    #[inline]
    pub const fn type_idx(&self) -> u32 {
        self.type_idx
    }

    /// Get the payload bytes (after header)
    pub fn payload(&self) -> &[u8] {
        &self.data[HEADER_SIZE..]
    }

    /// Get mutable payload bytes
    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.data[HEADER_SIZE..]
    }
}

/// A GC object that owns its memory (for stack-allocated testing)
#[derive(Debug, Clone)]
pub struct GcObject {
    /// The raw bytes of the object
    data: [u8; 256],
    /// Actual size used
    len: usize,
    /// Type index
    type_idx: u32,
}

impl GcObject {
    /// Create a new struct object
    pub fn new_struct(type_idx: u32, field_sizes: &[usize]) -> Result<Self> {
        let payload_size: usize = field_sizes.iter().sum();
        let total_size = HEADER_SIZE + payload_size;

        if total_size > 256 {
            return Err(Error::memory_error("Object too large"));
        }

        let mut data = [0u8; 256];

        // Write header
        let header = ObjectHeader::new(ObjectKind::Struct, total_size as u32);
        data[0] = header.flags;
        let size_bytes = header.size.to_le_bytes();
        data[4] = size_bytes[0];
        data[5] = size_bytes[1];
        data[6] = size_bytes[2];
        data[7] = size_bytes[3];

        Ok(Self {
            data,
            len: total_size,
            type_idx,
        })
    }

    /// Create a new array object
    pub fn new_array(type_idx: u32, element_size: usize, length: u32) -> Result<Self> {
        // Arrays store: length (4 bytes) + elements
        let payload_size = 4 + (element_size * length as usize);
        let total_size = HEADER_SIZE + payload_size;

        if total_size > 256 {
            return Err(Error::memory_error("Array too large"));
        }

        let mut data = [0u8; 256];

        // Write header
        let header = ObjectHeader::new(ObjectKind::Array, total_size as u32);
        data[0] = header.flags;
        let size_bytes = header.size.to_le_bytes();
        data[4] = size_bytes[0];
        data[5] = size_bytes[1];
        data[6] = size_bytes[2];
        data[7] = size_bytes[3];

        // Write array length
        let len_bytes = length.to_le_bytes();
        data[HEADER_SIZE] = len_bytes[0];
        data[HEADER_SIZE + 1] = len_bytes[1];
        data[HEADER_SIZE + 2] = len_bytes[2];
        data[HEADER_SIZE + 3] = len_bytes[3];

        Ok(Self {
            data,
            len: total_size,
            type_idx,
        })
    }

    /// Get the object as a reference
    pub fn as_ref(&self) -> GcObjectRef<'_> {
        GcObjectRef {
            data: &self.data[..self.len],
            type_idx: self.type_idx,
        }
    }

    /// Get the object as a mutable reference
    pub fn as_mut(&mut self) -> GcObjectMut<'_> {
        GcObjectMut {
            data: &mut self.data[..self.len],
            type_idx: self.type_idx,
        }
    }
}

/// Calculate the size of a value type in bytes
pub fn value_type_size(vt: &ValueType) -> usize {
    match vt {
        ValueType::I32 | ValueType::F32 => 4,
        ValueType::I64 | ValueType::F64 => 8,
        ValueType::V128 => 16,
        // References are stored as 4-byte offsets
        ValueType::FuncRef
        | ValueType::ExternRef
        | ValueType::ExnRef
        | ValueType::AnyRef
        | ValueType::EqRef
        | ValueType::I31Ref
        | ValueType::StructRef(_)
        | ValueType::ArrayRef(_) => 4,
        _ => 4, // Default for unknown types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_header() {
        let header = ObjectHeader::new(ObjectKind::Struct, 24);
        assert_eq!(header.kind(), ObjectKind::Struct);
        assert_eq!(header.size(), 24);
        assert!(!header.is_marked());

        let mut header = header;
        header.set_marked(true);
        assert!(header.is_marked());
    }

    #[test]
    fn test_gc_object_struct() {
        let obj = GcObject::new_struct(0, &[4, 8, 4]).unwrap();
        let obj_ref = obj.as_ref();
        assert_eq!(obj_ref.type_idx(), 0);
        assert_eq!(obj_ref.header().kind(), ObjectKind::Struct);
        assert_eq!(obj_ref.payload().len(), 16); // 4 + 8 + 4
    }

    #[test]
    fn test_gc_object_array() {
        let obj = GcObject::new_array(1, 4, 10).unwrap();
        let obj_ref = obj.as_ref();
        assert_eq!(obj_ref.type_idx(), 1);
        assert_eq!(obj_ref.header().kind(), ObjectKind::Array);
        // Payload: 4 bytes length + 10 * 4 bytes elements = 44
        assert_eq!(obj_ref.payload().len(), 44);
    }
}
