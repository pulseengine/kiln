#![cfg(feature = "runtime-memory")]

use wrt_error::Error;
use wrt_foundation::{
    bounded::{BoundedVec, MAX_DWARF_FILE_TABLE},
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::bounded_debug_infra;
/// Runtime memory inspection implementation
/// Provides safe memory access and heap analysis capabilities
use crate::runtime_api::{DebugMemory, RuntimeState};

/// Memory region information
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Start address
    pub start: u32,
    /// Size in bytes
    pub size: u32,
    /// Region type
    pub region_type: MemoryRegionType,
    /// Is writable
    pub writable: bool,
    /// Human-readable name
    pub name: &'static str,
}

/// Type of memory region
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionType {
    /// WebAssembly linear memory
    LinearMemory,
    /// Stack area
    Stack,
    /// Heap area
    Heap,
    /// Global variables
    Globals,
    /// Read-only data
    ReadOnly,
    /// Code (not directly accessible)
    Code,
}

/// Binary std/no_std choice
#[derive(Debug, Clone)]
pub struct HeapAllocation {
    /// Allocation address
    pub address: u32,
    /// Size in bytes
    pub size: u32,
    /// Binary std/no_std choice
    pub allocated: bool,
    /// Allocation ID (if available)
    pub id: Option<u32>,
}

/// Memory inspector for runtime debugging
pub struct MemoryInspector<'a> {
    /// Memory regions
    regions: BoundedVec<MemoryRegion, 16, crate::bounded_debug_infra::DebugProvider>,
    /// Binary std/no_std choice
    allocations:
        BoundedVec<HeapAllocation, MAX_DWARF_FILE_TABLE, crate::bounded_debug_infra::DebugProvider>,
    /// Reference to debug memory interface
    memory: Option<&'a dyn DebugMemory>,
}

impl<'a> MemoryInspector<'a> {
    /// Create a new memory inspector
    pub fn new() -> Result<Self, Error> {
        let regions_provider = safe_managed_alloc!(65536, CrateId::Runtime)?;
        let allocations_provider = safe_managed_alloc!(65536, CrateId::Runtime)?;

        Ok(Self {
            regions: BoundedVec::new(regions_provider)
                .map_err(|_| Error::allocation_failed("Failed to create regions vector"))?,
            allocations: BoundedVec::new(allocations_provider)
                .map_err(|_| Error::allocation_failed("Failed to create allocations vector"))?,
            memory: None,
        })
    }

    /// Attach to runtime memory
    pub fn attach(&mut self, memory: &'a dyn DebugMemory) {
        self.memory = Some(memory);
    }

    /// Register a memory region
    pub fn add_region(&mut self, region: MemoryRegion) -> Result<(), ()> {
        self.regions.push(region).map_err(|_| ())
    }

    /// Binary std/no_std choice
    pub fn add_allocation(&mut self, alloc: HeapAllocation) -> Result<(), ()> {
        self.allocations.push(alloc).map_err(|_| ())
    }

    /// Find which region contains an address
    pub fn find_region(&self, addr: u32) -> Option<&MemoryRegion> {
        self.regions
            .iter()
            .find(|r| addr >= r.start && addr < r.start.saturating_add(r.size))
    }

    /// Check if an address is valid
    pub fn is_valid_address(&self, addr: u32) -> bool {
        if let Some(memory) = self.memory {
            memory.is_valid_address(addr)
        } else {
            self.find_region(addr).is_some()
        }
    }

    /// Read memory safely
    pub fn read_memory(&self, addr: u32, len: usize) -> Option<MemoryView<'_>> {
        let memory = self.memory?;
        let data = memory.read_bytes(addr, len)?;

        Some(MemoryView {
            address: addr,
            data,
            region: self.find_region(addr),
        })
    }

    /// Read a null-terminated string
    pub fn read_cstring(&self, addr: u32, max_len: usize) -> Option<CStringView<'_>> {
        let memory = self.memory?;

        // Find string length
        let mut len = 0;
        for i in 0..max_len {
            if let Some(byte) = memory.read_bytes(addr + i as u32, 1) {
                if byte[0] == 0 {
                    break;
                }
                len += 1;
            } else {
                return None;
            }
        }

        let data = memory.read_bytes(addr, len)?;
        Some(CStringView {
            address: addr,
            data,
        })
    }

    /// Get heap statistics
    pub fn heap_stats(&self) -> HeapStats {
        let mut stats = HeapStats {
            total_allocations: 0,
            active_allocations: 0,
            total_bytes: 0,
            allocated_bytes: 0,
            largest_allocation: 0,
            fragmentation: 0.0,
        };

        for alloc in self.allocations.iter() {
            stats.total_allocations += 1;
            stats.total_bytes += alloc.size;

            if alloc.allocated {
                stats.active_allocations += 1;
                stats.allocated_bytes += alloc.size;
                stats.largest_allocation = stats.largest_allocation.max(alloc.size);
            }
        }

        // Simple fragmentation calculation
        if stats.total_bytes > 0 {
            let free_bytes = stats.total_bytes - stats.allocated_bytes;
            stats.fragmentation = (free_bytes as f32) / (stats.total_bytes as f32);
        }

        stats
    }

    /// Binary std/no_std choice
    pub fn find_allocation(&self, addr: u32) -> Option<&HeapAllocation> {
        self.allocations.iter().find(|alloc| {
            alloc.allocated
                && addr >= alloc.address
                && addr < alloc.address.saturating_add(alloc.size)
        })
    }

    /// Dump memory in hex format
    pub fn dump_hex(&self, addr: u32, len: usize) -> MemoryDump {
        MemoryDump {
            inspector: self,
            address: addr,
            length: len,
        }
    }

    /// Analyze stack usage
    pub fn analyze_stack(&self, state: &dyn RuntimeState) -> StackAnalysis {
        let sp = state.sp();

        // Find stack region
        let stack_region = self.regions.iter().find(|r| r.region_type == MemoryRegionType::Stack);

        let (stack_base, stack_size) = if let Some(region) = stack_region {
            (region.start + region.size, region.size)
        } else {
            // Assume default WASM stack
            (0x10000, 0x10000) // 64KB at 64KB offset
        };

        let used = stack_base.saturating_sub(sp);
        let free = sp.saturating_sub(stack_base - stack_size);

        StackAnalysis {
            stack_pointer: sp,
            stack_base,
            stack_size,
            used_bytes: used,
            free_bytes: free,
            usage_percent: (used as f32 / stack_size as f32) * 100.0,
        }
    }
}

/// View of memory contents
pub struct MemoryView<'a> {
    /// Start address
    pub address: u32,
    /// Memory data
    pub data: &'a [u8],
    /// Region containing this memory
    pub region: Option<&'a MemoryRegion>,
}

/// View of a C-style string
pub struct CStringView<'a> {
    /// String address
    pub address: u32,
    /// String data (without null terminator)
    pub data: &'a [u8],
}

impl<'a> CStringView<'a> {
    /// Get as UTF-8 string if valid
    pub fn as_str(&self) -> Option<&str> {
        core::str::from_utf8(self.data).ok()
    }
}

/// Heap statistics
#[derive(Debug, Clone)]
pub struct HeapStats {
    /// Binary std/no_std choice
    pub total_allocations: u32,
    /// Binary std/no_std choice
    pub active_allocations: u32,
    /// Total heap size
    pub total_bytes: u32,
    /// Binary std/no_std choice
    pub allocated_bytes: u32,
    /// Binary std/no_std choice
    pub largest_allocation: u32,
    /// Fragmentation ratio (0.0 - 1.0)
    pub fragmentation: f32,
}

/// Stack usage analysis
#[derive(Debug, Clone)]
pub struct StackAnalysis {
    /// Current stack pointer
    pub stack_pointer: u32,
    /// Stack base address
    pub stack_base: u32,
    /// Total stack size
    pub stack_size: u32,
    /// Bytes used
    pub used_bytes: u32,
    /// Bytes free
    pub free_bytes: u32,
    /// Usage percentage
    pub usage_percent: f32,
}

/// Memory dump helper
pub struct MemoryDump<'a> {
    inspector: &'a MemoryInspector<'a>,
    address: u32,
    length: usize,
}

impl<'a> MemoryDump<'a> {
    /// Write hex dump to output
    pub fn display<F>(&self, mut writer: F) -> Result<(), core::fmt::Error>
    where
        F: FnMut(&str) -> Result<(), core::fmt::Error>,
    {
        let mut addr = self.address & !0xF; // Align to 16 bytes
        let end = self.address + self.length as u32;

        while addr < end {
            // Address
            let mut hex_buf = [0u8; 8];
            writer(format_hex_u32(addr, &mut hex_buf))?;
            writer(": ")?;

            // Hex bytes
            for i in 0..16 {
                if i == 8 {
                    writer(" ")?;
                }

                let byte_addr = addr + i;
                if byte_addr >= self.address && byte_addr < end {
                    if let Some(view) = self.inspector.read_memory(byte_addr, 1) {
                        let mut buf = [0u8; 2];
                        writer(format_hex_u8(view.data[0], &mut buf))?;
                    } else {
                        writer("??")?;
                    }
                } else {
                    writer("  ")?;
                }
                writer(" ")?;
            }

            writer(" |")?;

            // ASCII representation
            for i in 0..16 {
                let byte_addr = addr + i;
                if byte_addr >= self.address && byte_addr < end {
                    if let Some(view) = self.inspector.read_memory(byte_addr, 1) {
                        let ch = view.data[0];
                        if ch >= 0x20 && ch < 0x7F {
                            writer(core::str::from_utf8(&[ch]).unwrap_or("?"))?;
                        } else {
                            writer(".")?;
                        }
                    } else {
                        writer("?")?;
                    }
                } else {
                    writer(" ")?;
                }
            }

            writer("|\n")?;
            addr += 16;
        }

        Ok(())
    }
}

// Formatting helpers
fn format_hex_u8(n: u8, buf: &mut [u8; 2]) -> &str {
    let high = (n >> 4) & 0xF;
    let low = n & 0xF;

    buf[0] = if high < 10 { b'0' + high } else { b'a' + high - 10 };
    buf[1] = if low < 10 { b'0' + low } else { b'a' + low - 10 };

    core::str::from_utf8(buf).unwrap_or("??")
}

fn format_hex_u32(mut n: u32, buf: &mut [u8; 8]) -> &str {
    for i in (0..8).rev() {
        let digit = (n & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        n >>= 4;
    }
    core::str::from_utf8(buf).unwrap_or("????????")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_regions() -> Result<(), Error> {
        let mut inspector = MemoryInspector::new()?;

        // Add memory regions
        inspector
            .add_region(MemoryRegion {
                start: 0x0,
                size: 0x10000,
                region_type: MemoryRegionType::LinearMemory,
                writable: true,
                name: "main",
            })
            .unwrap();

        inspector
            .add_region(MemoryRegion {
                start: 0x10000,
                size: 0x10000,
                region_type: MemoryRegionType::Stack,
                writable: true,
                name: "stack",
            })
            .unwrap();

        // Test region lookup
        assert!(inspector.find_region(0x5000).is_some());
        assert!(inspector.find_region(0x15000).is_some());
        assert!(inspector.find_region(0x30000).is_none());

        Ok(())
    }

    #[test]
    fn test_heap_stats() -> Result<(), Error> {
        let mut inspector = MemoryInspector::new()?;

        // Binary std/no_std choice
        inspector
            .add_allocation(HeapAllocation {
                address: 0x1000,
                size: 256,
                allocated: true,
                id: Some(1),
            })
            .unwrap();

        inspector
            .add_allocation(HeapAllocation {
                address: 0x2000,
                size: 512,
                allocated: true,
                id: Some(2),
            })
            .unwrap();

        inspector
            .add_allocation(HeapAllocation {
                address: 0x3000,
                size: 128,
                allocated: false,
                id: Some(3),
            })
            .unwrap();

        let stats = inspector.heap_stats();
        assert_eq!(stats.total_allocations, 3);
        assert_eq!(stats.active_allocations, 2);
        assert_eq!(stats.allocated_bytes, 768);
        assert_eq!(stats.largest_allocation, 512);

        Ok(())
    }
}
