//! GC Heap management
//!
//! Provides a capability-based managed heap for garbage-collected objects.
//! Designed for no_std compatibility with fixed-size allocation.

use wrt_error::{Error, Result};
use wrt_foundation::{
    bounded::BoundedVec,
    safe_memory::NoStdProvider,
};

use super::{
    object::{GcObjectMut, GcObjectRef, ObjectHeader, ObjectKind, HEADER_SIZE},
    GcRef, GC_OBJECT_ALIGNMENT,
};

/// Default heap size (256KB for embedded systems)
const DEFAULT_HEAP_SIZE: usize = 256 * 1024;

/// Maximum number of type entries for tracking
const MAX_TYPE_ENTRIES: usize = 256;

/// GC Heap - manages garbage-collected object allocation
///
/// The heap uses a simple bump allocator with a free list for reclaimed space.
/// Objects are aligned to 8 bytes for 64-bit compatibility.
#[derive(Debug)]
pub struct GcHeap<const SIZE: usize = DEFAULT_HEAP_SIZE> {
    /// The heap memory
    memory: [u8; SIZE],
    /// Current allocation pointer (next free offset)
    alloc_ptr: usize,
    /// Total bytes allocated (for statistics)
    bytes_allocated: usize,
    /// Number of live objects
    object_count: usize,
}

impl<const SIZE: usize> Default for GcHeap<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> GcHeap<SIZE> {
    /// Create a new empty GC heap
    pub const fn new() -> Self {
        Self {
            memory: [0u8; SIZE],
            // Start at offset 1 so that offset 0 can represent null
            alloc_ptr: GC_OBJECT_ALIGNMENT,
            bytes_allocated: 0,
            object_count: 0,
        }
    }

    /// Get the total heap size
    #[inline]
    pub const fn size(&self) -> usize {
        SIZE
    }

    /// Get the number of bytes currently allocated
    #[inline]
    pub const fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
    }

    /// Get the number of free bytes remaining
    #[inline]
    pub const fn bytes_free(&self) -> usize {
        SIZE.saturating_sub(self.alloc_ptr)
    }

    /// Get the number of live objects
    #[inline]
    pub const fn object_count(&self) -> usize {
        self.object_count
    }

    /// Allocate a struct object
    ///
    /// Returns a GcRef to the newly allocated object.
    pub fn alloc_struct(&mut self, type_idx: u32, field_sizes: &[usize]) -> Result<GcRef> {
        let payload_size: usize = field_sizes.iter().sum();
        self.alloc_object(ObjectKind::Struct, type_idx, payload_size)
    }

    /// Allocate an array object
    ///
    /// Returns a GcRef to the newly allocated object.
    pub fn alloc_array(
        &mut self,
        type_idx: u32,
        element_size: usize,
        length: u32,
    ) -> Result<GcRef> {
        // Arrays store: length (4 bytes) + elements
        let payload_size = 4 + (element_size * length as usize);
        let gc_ref = self.alloc_object(ObjectKind::Array, type_idx, payload_size)?;

        // Write array length to payload
        if let Some(offset) = gc_ref.offset() {
            let len_offset = offset as usize + HEADER_SIZE;
            if len_offset + 4 <= SIZE {
                let len_bytes = length.to_le_bytes();
                self.memory[len_offset] = len_bytes[0];
                self.memory[len_offset + 1] = len_bytes[1];
                self.memory[len_offset + 2] = len_bytes[2];
                self.memory[len_offset + 3] = len_bytes[3];
            }
        }

        Ok(gc_ref)
    }

    /// Allocate a raw object with given kind and payload size
    fn alloc_object(
        &mut self,
        kind: ObjectKind,
        type_idx: u32,
        payload_size: usize,
    ) -> Result<GcRef> {
        let total_size = HEADER_SIZE + payload_size;
        let aligned_size = align_up(total_size, GC_OBJECT_ALIGNMENT);

        // Check if we have enough space
        if self.alloc_ptr + aligned_size > SIZE {
            return Err(Error::memory_error("GC heap out of memory"));
        }

        let offset = self.alloc_ptr;

        // Write object header
        let header = ObjectHeader::new(kind, total_size as u32);
        self.memory[offset] = header.flags;
        self.memory[offset + 1] = 0; // reserved
        self.memory[offset + 2] = 0;
        self.memory[offset + 3] = 0;
        let size_bytes = (total_size as u32).to_le_bytes();
        self.memory[offset + 4] = size_bytes[0];
        self.memory[offset + 5] = size_bytes[1];
        self.memory[offset + 6] = size_bytes[2];
        self.memory[offset + 7] = size_bytes[3];

        // Zero-initialize payload
        for i in (offset + HEADER_SIZE)..(offset + total_size) {
            self.memory[i] = 0;
        }

        // Update allocation state
        self.alloc_ptr += aligned_size;
        self.bytes_allocated += aligned_size;
        self.object_count += 1;

        Ok(GcRef::from_offset(offset as u32))
    }

    /// Get an immutable reference to an object
    pub fn get(&self, gc_ref: GcRef) -> Result<GcObjectRef<'_>> {
        let offset = gc_ref
            .offset()
            .ok_or_else(|| Error::memory_error("Null GC reference"))?;

        let offset = offset as usize;
        if offset + HEADER_SIZE > SIZE {
            return Err(Error::memory_error("GC reference out of bounds"));
        }

        // Read header to get size
        let size = u32::from_le_bytes([
            self.memory[offset + 4],
            self.memory[offset + 5],
            self.memory[offset + 6],
            self.memory[offset + 7],
        ]) as usize;

        if offset + size > SIZE {
            return Err(Error::memory_error("Object extends beyond heap"));
        }

        // Extract type_idx from next 4 bytes after header (we store it there)
        // Actually, we don't store type_idx in the object - we need to track it separately
        // For now, use 0 as placeholder - the caller must provide it
        GcObjectRef::new(&self.memory[offset..offset + size], 0)
    }

    /// Get a mutable reference to an object
    pub fn get_mut(&mut self, gc_ref: GcRef) -> Result<GcObjectMut<'_>> {
        let offset = gc_ref
            .offset()
            .ok_or_else(|| Error::memory_error("Null GC reference"))?;

        let offset = offset as usize;
        if offset + HEADER_SIZE > SIZE {
            return Err(Error::memory_error("GC reference out of bounds"));
        }

        // Read header to get size
        let size = u32::from_le_bytes([
            self.memory[offset + 4],
            self.memory[offset + 5],
            self.memory[offset + 6],
            self.memory[offset + 7],
        ]) as usize;

        if offset + size > SIZE {
            return Err(Error::memory_error("Object extends beyond heap"));
        }

        GcObjectMut::new(&mut self.memory[offset..offset + size], 0)
    }

    /// Read a field from a struct object
    ///
    /// # Arguments
    /// * `gc_ref` - Reference to the struct
    /// * `field_offset` - Byte offset of the field within the payload
    /// * `field_size` - Size of the field in bytes
    pub fn read_struct_field(
        &self,
        gc_ref: GcRef,
        field_offset: usize,
        field_size: usize,
    ) -> Result<&[u8]> {
        let offset = gc_ref
            .offset()
            .ok_or_else(|| Error::memory_error("Null GC reference"))?;

        let obj_offset = offset as usize;
        if obj_offset + HEADER_SIZE > SIZE {
            return Err(Error::memory_error("GC reference out of bounds"));
        }

        // Get object size
        let size = u32::from_le_bytes([
            self.memory[obj_offset + 4],
            self.memory[obj_offset + 5],
            self.memory[obj_offset + 6],
            self.memory[obj_offset + 7],
        ]) as usize;

        let payload_start = obj_offset + HEADER_SIZE;
        let field_start = payload_start + field_offset;
        let field_end = field_start + field_size;

        if field_end > obj_offset + size {
            return Err(Error::memory_error("Field access out of bounds"));
        }

        Ok(&self.memory[field_start..field_end])
    }

    /// Write a field to a struct object
    pub fn write_struct_field(
        &mut self,
        gc_ref: GcRef,
        field_offset: usize,
        data: &[u8],
    ) -> Result<()> {
        let offset = gc_ref
            .offset()
            .ok_or_else(|| Error::memory_error("Null GC reference"))?;

        let obj_offset = offset as usize;
        let payload_start = obj_offset + HEADER_SIZE;
        let write_start = payload_start + field_offset;
        let write_end = write_start + data.len();

        // Get object size for bounds check
        let size = u32::from_le_bytes([
            self.memory[obj_offset + 4],
            self.memory[obj_offset + 5],
            self.memory[obj_offset + 6],
            self.memory[obj_offset + 7],
        ]) as usize;

        if write_end > obj_offset + size {
            return Err(Error::memory_error("Field write out of bounds"));
        }

        self.memory[write_start..write_end].copy_from_slice(data);
        Ok(())
    }

    /// Get array length
    pub fn array_len(&self, gc_ref: GcRef) -> Result<u32> {
        let obj = self.get(gc_ref)?;
        let payload = obj.payload();

        if payload.len() < 4 {
            return Err(Error::memory_error("Invalid array object"));
        }

        Ok(u32::from_le_bytes([
            payload[0],
            payload[1],
            payload[2],
            payload[3],
        ]))
    }

    /// Read an array element
    pub fn read_array_element(
        &self,
        gc_ref: GcRef,
        index: u32,
        element_size: usize,
    ) -> Result<&[u8]> {
        let length = self.array_len(gc_ref)?;
        if index >= length {
            return Err(Error::memory_error("Array index out of bounds"));
        }

        let offset = gc_ref.offset().unwrap(); // Safe: already validated in array_len
        let obj_offset = offset as usize;

        // Get object size for bounds check
        let size = u32::from_le_bytes([
            self.memory[obj_offset + 4],
            self.memory[obj_offset + 5],
            self.memory[obj_offset + 6],
            self.memory[obj_offset + 7],
        ]) as usize;

        // Skip header (8 bytes) + length (4 bytes)
        let elem_offset = obj_offset + HEADER_SIZE + 4 + (index as usize * element_size);
        let elem_end = elem_offset + element_size;

        if elem_end > obj_offset + size {
            return Err(Error::memory_error("Array element access out of bounds"));
        }

        Ok(&self.memory[elem_offset..elem_end])
    }

    /// Write an array element
    pub fn write_array_element(
        &mut self,
        gc_ref: GcRef,
        index: u32,
        element_size: usize,
        data: &[u8],
    ) -> Result<()> {
        let length = self.array_len(gc_ref)?;
        if index >= length {
            return Err(Error::memory_error("Array index out of bounds"));
        }

        let offset = gc_ref.offset().unwrap();
        let obj_offset = offset as usize;

        // Skip header (8 bytes) + length (4 bytes)
        let elem_offset = obj_offset + HEADER_SIZE + 4 + (index as usize * element_size);
        let elem_end = elem_offset + element_size;

        // Get object size for bounds check
        let size = u32::from_le_bytes([
            self.memory[obj_offset + 4],
            self.memory[obj_offset + 5],
            self.memory[obj_offset + 6],
            self.memory[obj_offset + 7],
        ]) as usize;

        if elem_end > obj_offset + size {
            return Err(Error::memory_error("Array element write out of bounds"));
        }

        if data.len() != element_size {
            return Err(Error::memory_error("Data size mismatch"));
        }

        self.memory[elem_offset..elem_end].copy_from_slice(data);
        Ok(())
    }

    /// Clear all mark bits (preparation for GC mark phase)
    pub fn clear_marks(&mut self) {
        let mut offset = GC_OBJECT_ALIGNMENT;

        while offset < self.alloc_ptr {
            if offset + HEADER_SIZE <= SIZE {
                // Clear mark bit
                self.memory[offset] &= !0x80;

                // Get object size to skip to next
                let size = u32::from_le_bytes([
                    self.memory[offset + 4],
                    self.memory[offset + 5],
                    self.memory[offset + 6],
                    self.memory[offset + 7],
                ]) as usize;

                offset += align_up(size, GC_OBJECT_ALIGNMENT);
            } else {
                break;
            }
        }
    }

    /// Mark an object as reachable
    pub fn mark(&mut self, gc_ref: GcRef) -> Result<()> {
        if let Some(offset) = gc_ref.offset() {
            let offset = offset as usize;
            if offset + HEADER_SIZE <= SIZE {
                self.memory[offset] |= 0x80;
            }
        }
        Ok(())
    }

    /// Check if an object is marked
    pub fn is_marked(&self, gc_ref: GcRef) -> bool {
        if let Some(offset) = gc_ref.offset() {
            let offset = offset as usize;
            if offset < SIZE {
                return (self.memory[offset] & 0x80) != 0;
            }
        }
        false
    }

    /// Read a u32 value at a specific offset (for GC scanning)
    pub fn read_u32_at(&self, offset: usize) -> Result<u32> {
        if offset + 4 > SIZE {
            return Err(Error::memory_error("Read offset out of bounds"));
        }
        Ok(u32::from_le_bytes([
            self.memory[offset],
            self.memory[offset + 1],
            self.memory[offset + 2],
            self.memory[offset + 3],
        ]))
    }
}

/// Align a value up to the given alignment
#[inline]
const fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heap_alloc_struct() {
        let mut heap = GcHeap::<1024>::new();

        let gc_ref = heap.alloc_struct(0, &[4, 8, 4]).unwrap();
        assert!(!gc_ref.is_null());

        let obj = heap.get(gc_ref).unwrap();
        assert_eq!(obj.header().kind(), ObjectKind::Struct);
    }

    #[test]
    fn test_heap_alloc_array() {
        let mut heap = GcHeap::<1024>::new();

        let gc_ref = heap.alloc_array(1, 4, 10).unwrap();
        assert!(!gc_ref.is_null());

        let length = heap.array_len(gc_ref).unwrap();
        assert_eq!(length, 10);
    }

    #[test]
    fn test_struct_field_access() {
        let mut heap = GcHeap::<1024>::new();

        // Allocate struct with two i32 fields
        let gc_ref = heap.alloc_struct(0, &[4, 4]).unwrap();

        // Write first field
        heap.write_struct_field(gc_ref, 0, &42i32.to_le_bytes())
            .unwrap();

        // Write second field
        heap.write_struct_field(gc_ref, 4, &100i32.to_le_bytes())
            .unwrap();

        // Read fields back
        let field1 = heap.read_struct_field(gc_ref, 0, 4).unwrap();
        assert_eq!(i32::from_le_bytes(field1.try_into().unwrap()), 42);

        let field2 = heap.read_struct_field(gc_ref, 4, 4).unwrap();
        assert_eq!(i32::from_le_bytes(field2.try_into().unwrap()), 100);
    }

    #[test]
    fn test_array_element_access() {
        let mut heap = GcHeap::<1024>::new();

        // Allocate array of 5 i32 elements
        let gc_ref = heap.alloc_array(0, 4, 5).unwrap();

        // Write elements
        for i in 0..5 {
            heap.write_array_element(gc_ref, i, 4, &((i * 10) as i32).to_le_bytes())
                .unwrap();
        }

        // Read elements back
        for i in 0..5 {
            let elem = heap.read_array_element(gc_ref, i, 4).unwrap();
            let value = i32::from_le_bytes(elem.try_into().unwrap());
            assert_eq!(value, (i * 10) as i32);
        }
    }

    #[test]
    fn test_gc_marks() {
        let mut heap = GcHeap::<1024>::new();

        let gc_ref1 = heap.alloc_struct(0, &[4]).unwrap();
        let gc_ref2 = heap.alloc_struct(0, &[4]).unwrap();

        // Initially not marked
        assert!(!heap.is_marked(gc_ref1));
        assert!(!heap.is_marked(gc_ref2));

        // Mark first object
        heap.mark(gc_ref1).unwrap();
        assert!(heap.is_marked(gc_ref1));
        assert!(!heap.is_marked(gc_ref2));

        // Clear marks
        heap.clear_marks();
        assert!(!heap.is_marked(gc_ref1));
        assert!(!heap.is_marked(gc_ref2));
    }

    #[test]
    fn test_heap_out_of_memory() {
        let mut heap = GcHeap::<64>::new();

        // First allocation should succeed
        let _ = heap.alloc_struct(0, &[4]).unwrap();

        // Fill up the heap
        let result = heap.alloc_array(0, 4, 100);
        assert!(result.is_err());
    }
}
