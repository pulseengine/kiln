//! Memory Layout Management for WebAssembly Component Model
//!
//! This module provides memory layout calculations and alignment handling
//! for the canonical ABI, ensuring proper data representation across
//! component boundaries.

use wrt_format::component::FormatValType;
use wrt_foundation::{
    budget_aware_provider::CrateId,
    collections::StaticVec as BoundedVec,
    safe_managed_alloc,
    safe_memory::NoStdProvider,
    traits::{Checksummable, FromBytes, ToBytes},
    verification::Checksum,
};

use crate::{bounded_component_infra::ComponentProvider, prelude::*};

/// Maximum alignment requirement for any type
const MAX_ALIGNMENT: usize = 8;

/// Memory layout information for a type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLayout {
    /// Size of the type in bytes
    pub size: usize,
    /// Alignment requirement in bytes
    pub alignment: usize,
}

impl MemoryLayout {
    /// Create a new memory layout
    pub const fn new(size: usize, alignment: usize) -> Self {
        Self { size, alignment }
    }

    /// Calculate the aligned size (size rounded up to alignment)
    pub const fn aligned_size(&self) -> usize {
        align_to(self.size, self.alignment)
    }

    /// Check if this layout fits within another layout
    pub const fn fits_in(&self, other: &MemoryLayout) -> bool {
        self.size <= other.size && self.alignment <= other.alignment
    }
}

/// Calculate memory layout for a WebAssembly component model type
pub fn calculate_layout(ty: &FormatValType) -> MemoryLayout {
    match ty {
        // Primitive types
        FormatValType::Bool => MemoryLayout::new(1, 1),
        FormatValType::S8 | FormatValType::U8 => MemoryLayout::new(1, 1),
        FormatValType::S16 | FormatValType::U16 => MemoryLayout::new(2, 2),
        FormatValType::S32 | FormatValType::U32 => MemoryLayout::new(4, 4),
        FormatValType::S64 | FormatValType::U64 => MemoryLayout::new(8, 8),
        FormatValType::F32 => MemoryLayout::new(4, 4),
        FormatValType::F64 => MemoryLayout::new(8, 8),
        FormatValType::Char => MemoryLayout::new(4, 4), // Unicode scalar value

        // String is represented as pointer + length
        FormatValType::String => MemoryLayout::new(8, 4), // 4-byte pointer + 4-byte length

        // Lists are represented as pointer + length
        FormatValType::List(_) => MemoryLayout::new(8, 4), // 4-byte pointer + 4-byte length

        // Records require calculating layout for all fields
        FormatValType::Record(fields) => calculate_record_layout(fields),

        // Tuples are similar to records
        FormatValType::Tuple(types) => calculate_tuple_layout(types),

        // Variants need space for discriminant + largest payload
        FormatValType::Variant(cases) => calculate_variant_layout(cases),

        // Enums need space for discriminant only
        FormatValType::Enum(cases) => calculate_enum_layout(cases.len()),

        // Options are variants with two cases (none/some)
        FormatValType::Option(inner) => calculate_option_layout(inner),

        // Results are variants with two cases (ok/err)
        FormatValType::Result(result_ty) => calculate_result_layout(Some(result_ty.as_ref()), None),

        // Flags need bit storage
        FormatValType::Flags(names) => calculate_flags_layout(names.len()),

        // Resources are handles (u32)
        FormatValType::Own(_) | FormatValType::Borrow(_) => MemoryLayout::new(4, 4),

        // Other types
        _ => MemoryLayout::new(0, 1), // Unknown types have zero size
    }
}

/// Calculate layout for a record type
fn calculate_record_layout(fields: &[(String, FormatValType)]) -> MemoryLayout {
    let mut offset = 0;
    let mut max_alignment = 1;

    for (_, field_type) in fields {
        let field_layout = calculate_layout(field_type);

        // Align offset to field's alignment requirement
        offset = align_to(offset, field_layout.alignment);
        offset += field_layout.size;

        // Track maximum alignment requirement
        max_alignment = max_alignment.max(field_layout.alignment);
    }

    // Final size must be aligned to the record's alignment
    let final_size = align_to(offset, max_alignment);

    MemoryLayout::new(final_size, max_alignment)
}

/// Calculate layout for a tuple type
fn calculate_tuple_layout(types: &[FormatValType]) -> MemoryLayout {
    let mut offset = 0;
    let mut max_alignment = 1;

    for ty in types {
        let layout = calculate_layout(ty);

        // Align offset to element's alignment requirement
        offset = align_to(offset, layout.alignment);
        offset += layout.size;

        // Track maximum alignment requirement
        max_alignment = max_alignment.max(layout.alignment);
    }

    // Final size must be aligned to the tuple's alignment
    let final_size = align_to(offset, max_alignment);

    MemoryLayout::new(final_size, max_alignment)
}

/// Calculate layout for a variant type
fn calculate_variant_layout(cases: &[(String, Option<FormatValType>)]) -> MemoryLayout {
    // Discriminant size based on number of cases
    let discriminant_size = discriminant_size(cases.len());
    let discriminant_alignment = discriminant_size;

    // Find the largest payload
    let mut max_payload_size = 0;
    let mut max_payload_alignment = 1;

    for (_, payload_type) in cases {
        if let Some(ty) = payload_type {
            let payload_layout = calculate_layout(ty);
            max_payload_size = max_payload_size.max(payload_layout.size);
            max_payload_alignment = max_payload_alignment.max(payload_layout.alignment);
        }
    }

    // Variant alignment is max of discriminant and payload alignments
    let variant_alignment = discriminant_alignment.max(max_payload_alignment);

    // Calculate total size
    let payload_offset = align_to(discriminant_size, max_payload_alignment);
    let total_size = payload_offset + max_payload_size;
    let final_size = align_to(total_size, variant_alignment);

    MemoryLayout::new(final_size, variant_alignment)
}

/// Calculate layout for an enum type
fn calculate_enum_layout(num_cases: usize) -> MemoryLayout {
    let size = discriminant_size(num_cases);
    MemoryLayout::new(size, size)
}

/// Calculate layout for an option type
fn calculate_option_layout(inner: &FormatValType) -> MemoryLayout {
    // Option is a variant with none (no payload) and some (with payload)
    let inner_layout = calculate_layout(inner);

    // 1-byte discriminant + payload
    let payload_offset = align_to(1, inner_layout.alignment);
    let total_size = payload_offset + inner_layout.size;
    let alignment = inner_layout.alignment.max(1);
    let final_size = align_to(total_size, alignment);

    MemoryLayout::new(final_size, alignment)
}

/// Calculate layout for a result type
fn calculate_result_layout(
    ok_ty: Option<&FormatValType>,
    err_ty: Option<&FormatValType>,
) -> MemoryLayout {
    // Result is a variant with ok and err cases
    let mut max_payload_size = 0;
    let mut max_payload_alignment = 1;

    if let Some(ty) = ok_ty {
        let layout = calculate_layout(ty);
        max_payload_size = max_payload_size.max(layout.size);
        max_payload_alignment = max_payload_alignment.max(layout.alignment);
    }

    if let Some(ty) = err_ty {
        let layout = calculate_layout(ty);
        max_payload_size = max_payload_size.max(layout.size);
        max_payload_alignment = max_payload_alignment.max(layout.alignment);
    }

    // 1-byte discriminant + payload
    let payload_offset = align_to(1, max_payload_alignment);
    let total_size = payload_offset + max_payload_size;
    let alignment = max_payload_alignment.max(1);
    let final_size = align_to(total_size, alignment);

    MemoryLayout::new(final_size, alignment)
}

/// Calculate layout for flags type
fn calculate_flags_layout(num_flags: usize) -> MemoryLayout {
    // Flags are stored as bit fields
    let num_bytes = num_flags.div_ceil(8);

    // Align to natural size up to 8 bytes
    let alignment = if num_bytes <= 1 {
        1
    } else if num_bytes <= 2 {
        2
    } else if num_bytes <= 4 {
        4
    } else {
        8
    };

    let size = align_to(num_bytes, alignment);
    MemoryLayout::new(size, alignment)
}

/// Determine discriminant size based on number of cases
fn discriminant_size(num_cases: usize) -> usize {
    if num_cases <= 256 {
        1
    } else if num_cases <= 65536 {
        2
    } else {
        4
    }
}

/// Align a value to the specified alignment
const fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

/// Calculate field offsets for a record or struct
pub fn calculate_field_offsets(
    fields: &[(String, FormatValType)],
) -> Vec<(String, usize, MemoryLayout)> {
    let mut result = Vec::new();
    let mut offset = 0;

    for (name, field_type) in fields {
        let layout = calculate_layout(field_type);

        // Align offset to field's alignment requirement
        offset = align_to(offset, layout.alignment);

        result.push((name.clone(), offset, layout));
        offset += layout.size;
    }

    result
}

/// Memory layout optimizer for reducing padding
pub struct LayoutOptimizer;

impl LayoutOptimizer {
    /// Reorder fields to minimize padding (largest alignment first)
    pub fn optimize_field_order(
        fields: &[(String, FormatValType)],
    ) -> Vec<(String, FormatValType)> {
        let mut fields_with_layout: Vec<_> = fields
            .iter()
            .map(|(name, ty)| {
                let layout = calculate_layout(ty);
                (name.clone(), ty.clone(), layout)
            })
            .collect();

        // Sort by alignment (descending) then by size (descending)
        fields_with_layout.sort_by(|a, b| {
            b.2.alignment.cmp(&a.2.alignment).then_with(|| b.2.size.cmp(&a.2.size))
        });

        fields_with_layout.into_iter().map(|(name, ty, _)| (name, ty)).collect()
    }

    /// Calculate padding between two adjacent fields
    pub fn calculate_padding(current_offset: usize, next_alignment: usize) -> usize {
        let aligned_offset = align_to(current_offset, next_alignment);
        aligned_offset - current_offset
    }
}

/// Binary std/no_std choice
#[derive(Debug)]
pub struct CanonicalMemoryPool {
    /// Binary std/no_std choice
    #[cfg(not(any(feature = "std",)))]
    pools: [BoundedVec<MemoryBuffer, 16>; 4],
    #[cfg(feature = "std")]
    pools: [Vec<MemoryBuffer>; 4],
    /// Size classes: 64B, 256B, 1KB, 4KB
    size_classes: [usize; 4],
}

#[derive(Debug)]
struct MemoryBuffer {
    data: Box<[u8]>,
    in_use: bool,
}

impl Clone for MemoryBuffer {
    fn clone(&self) -> Self {
        Self {
            data: self.data.to_vec().into_boxed_slice(),
            in_use: self.in_use,
        }
    }
}

impl Default for MemoryBuffer {
    fn default() -> Self {
        Self {
            data: Box::new([]),
            in_use: false,
        }
    }
}

impl PartialEq for MemoryBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.data.as_ref() == other.data.as_ref() && self.in_use == other.in_use
    }
}

impl Eq for MemoryBuffer {}

impl Checksummable for MemoryBuffer {
    fn update_checksum(&self, checksum: &mut Checksum) {
        // Update checksum with the data buffer contents
        self.data.as_ref().update_checksum(checksum);
        // Include the in_use flag in the checksum
        self.in_use.update_checksum(checksum);
    }
}

impl ToBytes for MemoryBuffer {
    fn to_bytes_with_provider<P: MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream,
        provider: &P,
    ) -> core::result::Result<(), wrt_error::Error> {
        // Write the length of the data buffer
        (self.data.len() as u32).to_bytes_with_provider(writer, provider)?;
        // Write the data buffer contents
        writer.write_all(&self.data).map_err(|_| {
            wrt_error::Error::foundation_memory_provider_failed("Failed to write MemoryBuffer data")
        })?;
        // Write the in_use flag
        self.in_use.to_bytes_with_provider(writer, provider)?;
        Ok(())
    }
}

impl FromBytes for MemoryBuffer {
    fn from_bytes_with_provider<P: MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream,
        provider: &P,
    ) -> core::result::Result<Self, wrt_error::Error> {
        // Read the length of the data buffer
        let len = u32::from_bytes_with_provider(reader, provider)? as usize;
        // Read the data buffer contents
        #[cfg(feature = "std")]
        let mut data = vec![0u8; len];
        #[cfg(not(feature = "std"))]
        let mut data = {
            use alloc::vec;
            vec![0u8; len]
        };
        reader.read_exact(&mut data).map_err(|_| {
            wrt_error::Error::foundation_memory_provider_failed("Failed to read MemoryBuffer data")
        })?;
        // Read the in_use flag
        let in_use = bool::from_bytes_with_provider(reader, provider)?;
        Ok(Self {
            data: data.into_boxed_slice(),
            in_use,
        })
    }
}

impl CanonicalMemoryPool {
    /// Create a new memory pool
    pub fn new() -> wrt_error::Result<Self> {
        Ok(Self {
            #[cfg(not(any(feature = "std",)))]
            pools: [
                {
                    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                    BoundedVec::new().unwrap()
                },
                {
                    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                    BoundedVec::new().unwrap()
                },
                {
                    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                    BoundedVec::new().unwrap()
                },
                {
                    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                    BoundedVec::new().unwrap()
                },
            ],
            #[cfg(feature = "std")]
            pools: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            size_classes: [64, 256, 1024, 4096],
        })
    }

    /// Acquire a buffer of at least the specified size
    pub fn acquire(&mut self, size: usize) -> Option<&mut [u8]> {
        // Find appropriate size class
        let class_idx = self.size_classes.iter().position(|&class_size| class_size >= size)?;

        // Look for available buffer in pool
        #[cfg(not(any(feature = "std",)))]
        {
            let pool = &mut self.pools[class_idx];
            for buffer in pool.iter_mut() {
                if !buffer.in_use {
                    buffer.in_use = true;
                    return Some(&mut buffer.data);
                }
            }
            None // Pool is full in no_std
        }

        #[cfg(feature = "std")]
        {
            let pool = &mut self.pools[class_idx];

            // Find index of existing free buffer
            let free_index = pool.iter().position(|b| !b.in_use);

            if let Some(index) = free_index {
                pool[index].in_use = true;
                return Some(&mut pool[index].data);
            }

            // Allocate new buffer
            let buffer_size = self.size_classes[class_idx];
            let data = vec![0u8; buffer_size].into_boxed_slice();
            pool.push(MemoryBuffer { data, in_use: true });

            pool.last_mut().map(|b| &mut b.data[..])
        }
    }

    /// Release a buffer back to the pool
    pub fn release(&mut self, ptr: *mut u8) {
        for pool in &mut self.pools {
            for buffer in pool.iter_mut() {
                if core::ptr::eq(buffer.data.as_ptr(), ptr) {
                    buffer.in_use = false;
                    return;
                }
            }
        }
    }
}

impl Default for CanonicalMemoryPool {
    fn default() -> Self {
        Self::new().expect("Failed to create CanonicalMemoryPool")
    }
}
