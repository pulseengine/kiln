//! Runtime Memory Profiling and Debugging
//!
//! This module provides comprehensive runtime profiling and debugging
//! capabilities for the WRT memory system, enabling detailed analysis of memory
//! usage patterns, allocation tracking, and performance profiling. It
//! complements the existing memory inspection capabilities with advanced
//! profiling features.

#![cfg(feature = "memory-profiling")]

#[cfg(any(feature = "std", feature = "alloc"))]
extern crate alloc;

#[cfg(feature = "std")]
use alloc::collections::BTreeMap;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
#[cfg(feature = "std")]
use std::sync::{Mutex, OnceLock};

use wrt_error::Result;
#[cfg(not(feature = "std"))]
use wrt_foundation::no_std_hashmap::BoundedHashMap;
use wrt_foundation::{
    CrateId, NoStdProvider,
    bounded::{BoundedString, BoundedVec},
    verification::Checksum,
    wrt_provider,
};

use crate::{bounded_debug_infra, runtime_memory::MemoryInspector};

/// Maximum number of allocation records to track
const MAX_ALLOCATION_RECORDS: usize = 512;

/// Maximum number of performance samples
const MAX_PERF_SAMPLES: usize = 128;

/// Maximum call stack depth to record
const MAX_CALL_STACK_DEPTH: usize = 8;

/// Global profiling state
static PROFILING_ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCATION_TRACKING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Allocation tracking record
#[derive(Debug, Clone, PartialEq)]
pub struct AllocationRecord {
    /// Unique allocation ID
    pub id: u32,
    /// Crate that made the allocation
    pub crate_id: CrateId,
    /// Size of allocation in bytes
    pub size: usize,
    /// Timestamp when allocated (microseconds since start)
    pub timestamp: u64,
    /// Call stack at allocation time (simplified)
    pub call_stack:
        BoundedVec<u64, MAX_CALL_STACK_DEPTH, NoStdProvider<{ MAX_CALL_STACK_DEPTH * 8 }>>,
    /// Allocation type
    pub alloc_type: AllocationType,
    /// Whether this allocation is still active
    pub active: bool,
    /// Tag for custom categorization
    pub tag: BoundedString<32>,
}

/// Type of memory allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocationType {
    /// Heap allocation
    Heap,
    /// Stack allocation
    Stack,
    /// Static memory pool
    Static,
    /// Shared memory
    Shared,
    /// Bounded collection
    Bounded,
    /// Provider allocation
    Provider,
}

// Implement required traits for AllocationType to use in HashMap
#[cfg(not(feature = "std"))]
impl Default for AllocationType {
    fn default() -> Self {
        Self::Heap
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for AllocationType {
    fn calculate_checksum(&self) -> wrt_foundation::verification::Checksum {
        wrt_foundation::verification::Checksum::new(*self as u32)
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for AllocationType {
    fn to_bytes(
        &self,
    ) -> wrt_foundation::bounded::BoundedVec<u8, 32, crate::bounded_debug_infra::DebugProvider>
    {
        let mut vec = wrt_foundation::bounded::BoundedVec::new(
            wrt_provider!(32, CrateId::Debug).unwrap_or_default(),
        )
        .expect("Failed to create bounded vector");
        let _ = vec.push(*self as u8);
        vec
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::FromBytes for AllocationType {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let bytes = reader.read_bytes(1)?;
        match bytes[0] {
            0 => Ok(Self::Heap),
            1 => Ok(Self::Stack),
            2 => Ok(Self::Static),
            3 => Ok(Self::Shared),
            4 => Ok(Self::Bounded),
            5 => Ok(Self::Provider),
            _ => Err(wrt_error::Error::parse_error(
                "Invalid AllocationType value",
            )),
        }
    }
}

/// Memory access pattern record
#[derive(Debug, Clone)]
pub struct AccessRecord {
    /// Memory location accessed
    pub address: usize,
    /// Access type
    pub access_type: AccessType,
    /// Size of access
    pub size: usize,
    /// Timestamp
    pub timestamp: u64,
    /// Crate that performed access
    pub crate_id: CrateId,
}

/// Type of memory access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// Read access
    Read,
    /// Write access
    Write,
    /// Read-modify-write
    ReadWrite,
}

/// Performance profiling sample
#[derive(Debug, Clone)]
pub struct PerformanceSample {
    /// Operation being profiled
    pub operation: BoundedString<64>,
    /// Start timestamp (microseconds)
    pub start_time: u64,
    /// Duration (microseconds)
    pub duration: u64,
    /// Memory allocated during operation
    pub memory_allocated: usize,
    /// Memory freed during operation
    pub memory_freed: usize,
    /// Crate performing operation
    pub crate_id: CrateId,
}

/// Memory leak detection information
#[derive(Debug, Clone)]
pub struct LeakInfo {
    /// Allocation that appears to be leaked
    pub allocation: AllocationRecord,
    /// Confidence score (0-100)
    pub confidence: u8,
    /// Reason for suspicion
    pub reason: BoundedString<128>,
}

/// Memory profiler for runtime debugging
pub struct MemoryProfiler<'a> {
    /// Active allocation records
    allocations: BoundedVec<
        AllocationRecord,
        MAX_ALLOCATION_RECORDS,
        NoStdProvider<{ MAX_ALLOCATION_RECORDS * 128 }>,
    >,
    /// Access pattern records
    access_records: BoundedVec<AccessRecord, 256, NoStdProvider<{ 256 * 32 }>>,
    /// Performance samples
    perf_samples:
        BoundedVec<PerformanceSample, MAX_PERF_SAMPLES, NoStdProvider<{ MAX_PERF_SAMPLES * 128 }>>,
    /// Next allocation ID
    next_alloc_id: AtomicU32,
    /// Start timestamp for relative timing
    start_time: u64,
    /// Total allocations tracked
    total_allocations: AtomicUsize,
    /// Total deallocations tracked
    total_deallocations: AtomicUsize,
    /// Reference to memory inspector for integration
    memory_inspector: Option<&'a MemoryInspector<'a>>,
}

impl<'a> MemoryProfiler<'a> {
    /// Create a new memory profiler
    pub fn new() -> Self {
        Self {
            allocations: BoundedVec::new(
                wrt_provider!({ { MAX_ALLOCATION_RECORDS * 32 } }, CrateId::Debug)
                    .unwrap_or_default(),
            )
            .expect("Failed to create allocations vector"),
            access_records: BoundedVec::new(
                wrt_provider!({ { MAX_ALLOCATION_RECORDS * 32 } }, CrateId::Debug)
                    .unwrap_or_default(),
            )
            .expect("Failed to create access records vector"),
            perf_samples: BoundedVec::new(
                wrt_provider!({ { MAX_PERF_SAMPLES * 32 } }, CrateId::Debug).unwrap_or_default(),
            )
            .expect("Failed to create perf samples vector"),
            next_alloc_id: AtomicU32::new(1),
            start_time: Self::get_timestamp(),
            total_allocations: AtomicUsize::new(0),
            total_deallocations: AtomicUsize::new(0),
            memory_inspector: None,
        }
    }

    /// Attach to a memory inspector for integrated debugging
    pub fn attach_inspector(&mut self, inspector: &'a MemoryInspector<'a>) {
        self.memory_inspector = Some(inspector);
    }

    /// Enable profiling
    pub fn enable_profiling() {
        PROFILING_ENABLED.store(true, Ordering::SeqCst);
    }

    /// Disable profiling
    pub fn disable_profiling() {
        PROFILING_ENABLED.store(false, Ordering::SeqCst);
    }

    /// Check if profiling is enabled
    pub fn is_profiling_enabled() -> bool {
        PROFILING_ENABLED.load(Ordering::SeqCst)
    }

    /// Enable allocation tracking
    pub fn enable_allocation_tracking() {
        ALLOCATION_TRACKING_ENABLED.store(true, Ordering::SeqCst);
    }

    /// Disable allocation tracking
    pub fn disable_allocation_tracking() {
        ALLOCATION_TRACKING_ENABLED.store(false, Ordering::SeqCst);
    }

    /// Check if allocation tracking is enabled
    pub fn is_allocation_tracking_enabled() -> bool {
        ALLOCATION_TRACKING_ENABLED.load(Ordering::SeqCst)
    }

    /// Track a new allocation
    pub fn track_allocation(
        &mut self,
        crate_id: CrateId,
        size: usize,
        alloc_type: AllocationType,
        tag: &str,
    ) -> wrt_error::Result<u32> {
        if !Self::is_allocation_tracking_enabled() {
            return Ok(0);
        }

        let id = self.next_alloc_id.fetch_add(1, Ordering::SeqCst);
        self.total_allocations.fetch_add(1, Ordering::SeqCst);

        let record = AllocationRecord {
            id,
            crate_id,
            size,
            timestamp: self.get_relative_timestamp(),
            call_stack: self.capture_call_stack()?,
            alloc_type,
            active: true,
            tag: BoundedString::try_from(tag)?,
        };

        // Store allocation record
        if self.allocations.len() >= MAX_ALLOCATION_RECORDS {
            // Remove oldest inactive allocation
            self.evict_oldest_inactive()?;
        }
        self.allocations.push(record)?;

        Ok(id)
    }

    /// Track a deallocation
    pub fn track_deallocation(&mut self, alloc_id: u32) -> wrt_error::Result<()> {
        if !Self::is_allocation_tracking_enabled() {
            return Ok();
        }

        self.total_deallocations.fetch_add(1, Ordering::SeqCst);

        // Mark allocation as inactive
        for alloc in self.allocations.iter_mut() {
            if alloc.id == alloc_id {
                alloc.active = false;
                break;
            }
        }

        Ok(())
    }

    /// Track a memory access
    pub fn track_access(
        &mut self,
        address: usize,
        access_type: AccessType,
        size: usize,
        crate_id: CrateId,
    ) -> wrt_error::Result<()> {
        if !Self::is_profiling_enabled() {
            return Ok();
        }

        let record = AccessRecord {
            address,
            access_type,
            size,
            timestamp: self.get_relative_timestamp(),
            crate_id,
        };

        if self.access_records.len() >= 256 {
            // Remove oldest record
            self.access_records.remove(0);
        }
        self.access_records.push(record)?;

        Ok(())
    }

    /// Start profiling an operation
    pub fn start_profiling(&self, operation: &str, crate_id: CrateId) -> ProfilingHandle {
        ProfilingHandle {
            operation: BoundedString::try_from(operation).unwrap_or_default(),
            start_time: self.get_relative_timestamp(),
            initial_allocations: self.total_allocations.load(Ordering::SeqCst),
            initial_deallocations: self.total_deallocations.load(Ordering::SeqCst),
            crate_id,
        }
    }

    /// Complete profiling and record sample
    pub fn complete_profiling(&mut self, handle: ProfilingHandle) -> wrt_error::Result<()> {
        if !Self::is_profiling_enabled() {
            return Ok();
        }

        let duration = self.get_relative_timestamp() - handle.start_time;
        let allocations =
            self.total_allocations.load(Ordering::SeqCst) - handle.initial_allocations;
        let deallocations =
            self.total_deallocations.load(Ordering::SeqCst) - handle.initial_deallocations;

        let sample = PerformanceSample {
            operation: handle.operation,
            start_time: handle.start_time,
            duration,
            memory_allocated: allocations * 64, // Estimate average allocation size
            memory_freed: deallocations * 64,
            crate_id: handle.crate_id,
        };

        if self.perf_samples.len() >= MAX_PERF_SAMPLES {
            self.perf_samples.remove(0);
        }
        self.perf_samples.push(sample)?;

        Ok(())
    }

    /// Detect potential memory leaks
    pub fn detect_leaks(
        &self,
    ) -> wrt_error::Result<BoundedVec<LeakInfo, 16, NoStdProvider<{ 16 * 256 }>>> {
        let mut leaks =
            BoundedVec::new(wrt_provider!({ 16 * 256 }, CrateId::Debug).unwrap_or_default())?;
        let current_time = self.get_relative_timestamp();

        for alloc in self.allocations.iter() {
            if !alloc.active {
                continue;
            }

            // Check various leak indicators
            let age = current_time - alloc.timestamp;
            let mut confidence = 0u8;
            let mut reason = BoundedString::<128, crate::bounded_debug_infra::DebugProvider>::new(
                wrt_provider!(128, CrateId::Debug).unwrap_or_default(),
            )?;

            // Long-lived allocation
            if age > 60_000_000 {
                // 1 minute in microseconds
                confidence += 30;
                let _ = reason.push_str("Long-lived allocation");
            }

            // Large allocation
            if alloc.size > 1024 * 1024 {
                // 1MB
                confidence += 20;
                if !reason.is_empty() {
                    let _ = reason.push_str(", ");
                }
                let _ = reason.push_str("Large allocation");
            }

            // No recent access (simplified check)
            if self.no_recent_access(alloc.id, 30_000_000)? {
                confidence += 40;
                if !reason.is_empty() {
                    let _ = reason.push_str(", ");
                }
                let _ = reason.push_str("No recent access");
            }

            if confidence >= 50 {
                let leak = LeakInfo {
                    allocation: alloc.clone(),
                    confidence,
                    reason,
                };
                let _ = leaks.push(leak);
            }
        }

        Ok(leaks)
    }

    /// Generate memory profiling report
    pub fn generate_profile_report(&self) -> wrt_error::Result<ProfileReport> {
        // Create bounded maps for stats
        #[cfg(feature = "std")]
        let mut crate_stats = BTreeMap::new();
        #[cfg(feature = "std")]
        let mut type_stats = BTreeMap::new();

        #[cfg(not(feature = "std"))]
        let mut crate_stats = BoundedHashMap::<CrateId, usize, 32, NoStdProvider<{ 32 * 64 }>>::new(
            wrt_provider!({ 32 * 64 }, CrateId::Debug).unwrap_or_default(),
        );
        #[cfg(not(feature = "std"))]
        let mut type_stats =
            BoundedHashMap::<AllocationType, usize, 16, NoStdProvider<{ 16 * 64 }>>::new(
                wrt_provider!({ 16 * 64 }, CrateId::Debug).unwrap_or_default(),
            );

        // Analyze active allocations
        for alloc in self.allocations.iter() {
            if alloc.active {
                *crate_stats.entry(alloc.crate_id).or_insert(0) += alloc.size;
                *type_stats.entry(alloc.alloc_type).or_insert(0) += alloc.size;
            }
        }

        // Analyze access patterns
        let access_pattern = self.analyze_access_patterns()?;

        // Detect hotspots
        let hotspots = self.detect_memory_hotspots()?;

        // Performance analysis
        let perf_analysis = self.analyze_performance()?;

        Ok(ProfileReport {
            timestamp: self.get_relative_timestamp(),
            total_allocations: self.total_allocations.load(Ordering::SeqCst),
            total_deallocations: self.total_deallocations.load(Ordering::SeqCst),
            active_allocations: self.allocations.iter().filter(|a| a.active).count(),
            crate_breakdown: crate_stats,
            type_breakdown: type_stats,
            access_patterns: access_pattern,
            memory_hotspots: hotspots,
            performance_metrics: perf_analysis,
            detected_leaks: self.detect_leaks()?,
        })
    }

    /// Check if allocation has been accessed recently
    fn no_recent_access(&self, alloc_id: u32, threshold: u64) -> wrt_error::Result<bool> {
        // Simplified implementation - in production, would track actual access patterns
        Ok(true)
    }

    /// Analyze access patterns
    fn analyze_access_patterns(&self) -> wrt_error::Result<AccessPatternSummary> {
        let mut read_count = 0;
        let mut write_count = 0;
        let mut sequential_count = 0;
        let mut random_count = 0;

        let mut last_address = 0;
        for (i, access) in self.access_records.iter().enumerate() {
            match access.access_type {
                AccessType::Read => read_count += 1,
                AccessType::Write => write_count += 1,
                AccessType::ReadWrite => {
                    read_count += 1;
                    write_count += 1;
                },
            }

            if i > 0 {
                let diff = if access.address > last_address {
                    access.address - last_address
                } else {
                    last_address - access.address
                };

                if diff <= 64 {
                    sequential_count += 1;
                } else {
                    random_count += 1;
                }
            }
            last_address = access.address;
        }

        Ok(AccessPatternSummary {
            total_reads: read_count,
            total_writes: write_count,
            sequential_accesses: sequential_count,
            random_accesses: random_count,
            read_write_ratio: if write_count > 0 { (read_count * 100) / write_count } else { 100 },
        })
    }

    /// Detect memory hotspots
    fn detect_memory_hotspots(
        &self,
    ) -> wrt_error::Result<BoundedVec<MemoryHotspot, 8, NoStdProvider<{ 8 * 32 }>>> {
        let mut hotspots =
            BoundedVec::new(wrt_provider!({ 8 * 32 }, CrateId::Debug).unwrap_or_default())?;

        // Group accesses by address range
        #[cfg(feature = "std")]
        let mut access_counts = BTreeMap::new();
        #[cfg(not(feature = "std"))]
        let mut access_counts = BoundedHashMap::<usize, usize, 64, NoStdProvider<{ 64 * 32 }>>::new(
            wrt_provider!({ 64 * 32 }, CrateId::Debug).unwrap_or_default(),
        );

        for access in self.access_records.iter() {
            let region = access.address / 4096; // 4KB pages
            *access_counts.entry(region).or_insert(0) += 1;
        }

        // Find top hotspots
        let mut sorted: BoundedVec<(usize, usize), 32, NoStdProvider<{ 32 * 16 }>> =
            BoundedVec::new(wrt_provider!({ 32 * 16 }, CrateId::Debug).unwrap_or_default())?;
        for (region, count) in access_counts {
            let _ = sorted.push((region, count));
        }

        // Simple bubble sort for top entries
        for i in 0..sorted.len().min(8) {
            for j in i + 1..sorted.len() {
                if sorted.get(j).unwrap().1 > sorted.get(i).unwrap().1 {
                    sorted.swap(i, j);
                }
            }
        }

        // Create hotspot entries
        for i in 0..sorted.len().min(8) {
            if let Some(&(region, count)) = sorted.get(i) {
                let hotspot = MemoryHotspot {
                    address_range_start: region * 4096,
                    address_range_end: (region + 1) * 4096,
                    access_count: count,
                    predominant_type: AccessType::Read, // Simplified
                };
                let _ = hotspots.push(hotspot);
            }
        }

        Ok(hotspots)
    }

    /// Analyze performance metrics
    fn analyze_performance(&self) -> wrt_error::Result<PerformanceAnalysis> {
        let mut total_duration = 0u64;
        let mut total_allocations = 0usize;
        let mut total_deallocations = 0usize;

        #[cfg(feature = "std")]
        let mut operation_times = BTreeMap::new();
        #[cfg(not(feature = "std"))]
        let mut operation_times =
            BoundedHashMap::<BoundedString<64>, (u64, u32), 32, NoStdProvider<{ 32 * 96 }>>::new(
                wrt_provider!({ 32 * 96 }, CrateId::Debug).unwrap_or_default(),
            );

        for sample in self.perf_samples.iter() {
            total_duration += sample.duration;
            total_allocations += sample.memory_allocated;
            total_deallocations += sample.memory_freed;

            let entry = operation_times.entry(sample.operation.clone()).or_insert((0u64, 0u32));
            entry.0 += sample.duration;
            entry.1 += 1;
        }

        // Find slowest operations
        let mut slowest_ops =
            BoundedVec::<(BoundedString<64>, u64), 5, NoStdProvider<{ 5 * 72 }>>::new(
                wrt_provider!({ 5 * 72 }, CrateId::Debug).unwrap_or_default(),
            )?;
        for (op, (total_time, count)) in operation_times {
            let avg_time = total_time / count as u64;
            let _ = slowest_ops.push((op, avg_time));
        }

        // Sort by average time
        for i in 0..slowest_ops.len() {
            for j in i + 1..slowest_ops.len() {
                if slowest_ops.get(j).unwrap().1 > slowest_ops.get(i).unwrap().1 {
                    slowest_ops.swap(i, j);
                }
            }
        }

        Ok(PerformanceAnalysis {
            avg_operation_time: if !self.perf_samples.is_empty() {
                total_duration / self.perf_samples.len() as u64
            } else {
                0
            },
            memory_churn_rate: (total_allocations + total_deallocations) as u64
                / total_duration.max(1),
            slowest_operations: slowest_ops,
        })
    }

    /// Capture simplified call stack
    fn capture_call_stack(
        &self,
    ) -> wrt_error::Result<
        BoundedVec<u64, MAX_CALL_STACK_DEPTH, NoStdProvider<{ MAX_CALL_STACK_DEPTH * 8 }>>,
    > {
        // In a real implementation, this would use platform-specific
        // stack unwinding. For now, return a dummy stack.
        let mut stack = BoundedVec::new(
            wrt_provider!({ MAX_CALL_STACK_DEPTH * 8 }, CrateId::Debug).unwrap_or_default(),
        )?;
        let _ = stack.push(0x1000); // Dummy addresses
        let _ = stack.push(0x2000);
        Ok(stack)
    }

    /// Get timestamp relative to profiler start
    fn get_relative_timestamp(&self) -> u64 {
        Self::get_timestamp() - self.start_time
    }

    /// Get current timestamp in microseconds
    fn get_timestamp() -> u64 {
        // In no_std, this would use a platform-specific timer
        // For now, use a simple counter
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        COUNTER.fetch_add(1000, Ordering::SeqCst) as u64
    }

    /// Evict oldest inactive allocation
    fn evict_oldest_inactive(&mut self) -> wrt_error::Result<()> {
        let mut oldest_idx = None;
        let mut oldest_time = u64::MAX;

        for (i, alloc) in self.allocations.iter().enumerate() {
            if !alloc.active && alloc.timestamp < oldest_time {
                oldest_time = alloc.timestamp;
                oldest_idx = Some(i);
            }
        }

        if let Some(idx) = oldest_idx {
            self.allocations.remove(idx);
        }

        Ok(())
    }
}

/// Handle for profiling operations
pub struct ProfilingHandle {
    operation: BoundedString<64>,
    start_time: u64,
    initial_allocations: usize,
    initial_deallocations: usize,
    crate_id: CrateId,
}

/// Memory profiling report
#[derive(Debug, Clone)]
pub struct ProfileReport {
    /// Report timestamp
    pub timestamp: u64,
    /// Total allocations made
    pub total_allocations: usize,
    /// Total deallocations made
    pub total_deallocations: usize,
    /// Currently active allocations
    pub active_allocations: usize,
    /// Memory usage by crate
    #[cfg(feature = "std")]
    pub crate_breakdown: BTreeMap<CrateId, usize>,
    #[cfg(not(feature = "std"))]
    pub crate_breakdown: BoundedHashMap<CrateId, usize, 32, NoStdProvider<{ 32 * 64 }>>,
    /// Memory usage by type
    #[cfg(feature = "std")]
    pub type_breakdown: BTreeMap<AllocationType, usize>,
    #[cfg(not(feature = "std"))]
    pub type_breakdown: BoundedHashMap<AllocationType, usize, 16, NoStdProvider<{ 16 * 64 }>>,
    /// Access pattern analysis
    pub access_patterns: AccessPatternSummary,
    /// Memory hotspots
    pub memory_hotspots: BoundedVec<MemoryHotspot, 8, NoStdProvider<{ 8 * 32 }>>,
    /// Performance metrics
    pub performance_metrics: PerformanceAnalysis,
    /// Detected memory leaks
    pub detected_leaks: BoundedVec<LeakInfo, 16, NoStdProvider<{ 16 * 256 }>>,
}

/// Access pattern summary
#[derive(Debug, Clone)]
pub struct AccessPatternSummary {
    /// Total read operations
    pub total_reads: usize,
    /// Total write operations
    pub total_writes: usize,
    /// Sequential access count
    pub sequential_accesses: usize,
    /// Random access count
    pub random_accesses: usize,
    /// Read/write ratio percentage
    pub read_write_ratio: usize,
}

/// Memory hotspot information
#[derive(Debug, Clone)]
pub struct MemoryHotspot {
    /// Start of address range
    pub address_range_start: usize,
    /// End of address range
    pub address_range_end: usize,
    /// Number of accesses
    pub access_count: usize,
    /// Most common access type
    pub predominant_type: AccessType,
}

/// Performance analysis results
#[derive(Debug, Clone)]
pub struct PerformanceAnalysis {
    /// Average operation time in microseconds
    pub avg_operation_time: u64,
    /// Memory allocation/deallocation rate per microsecond
    pub memory_churn_rate: u64,
    /// Slowest operations by average time
    pub slowest_operations: BoundedVec<(BoundedString<64>, u64), 5, NoStdProvider<{ 5 * 72 }>>,
}

/// Memory profiler instance
// ASIL-D safe: Use thread-safe static with lazy initialization
#[cfg(feature = "std")]
static MEMORY_PROFILER: OnceLock<Mutex<MemoryProfiler<'static>>> = OnceLock::new();

#[cfg(not(feature = "std"))]
use core::sync::atomic::AtomicPtr;
#[cfg(not(feature = "std"))]
static MEMORY_PROFILER: AtomicPtr<MemoryProfiler<'static>> = AtomicPtr::new(core::ptr::null_mut);

/// Initialize the memory profiler (ASIL-D safe)
pub fn init_profiler() -> wrt_error::Result<()> {
    #[cfg(feature = "std")]
    {
        MEMORY_PROFILER.get_or_init(|| Mutex::new(MemoryProfiler::new()));
    }
    #[cfg(not(feature = "std"))]
    {
        // For no_std, we use a simpler approach - just store a raw pointer
        // This is safe because we only initialize once
        let profiler = Box::leak(Box::new(MemoryProfiler::new()));
        MEMORY_PROFILER.store(profiler as *mut _, Ordering::SeqCst);
    }
    Ok(())
}

/// Get mutable reference to profiler (ASIL-D safe)
pub fn with_profiler<F, R>(f: F) -> wrt_error::Result<R>
where
    F: FnOnce(&mut MemoryProfiler<'static>) -> wrt_error::Result<R>,
{
    // ASIL-D safe: Use safe lock access without unsafe
    #[cfg(feature = "std")]
    {
        match MEMORY_PROFILER.get() {
            Some(profiler_mutex) => match profiler_mutex.lock() {
                Ok(mut profiler) => f(&mut *profiler),
                Err(_) => Err(wrt_error::Error::memory_error("Failed to lock profiler")),
            },
            None => Err(wrt_error::Error::memory_error("Profiler not initialized")),
        }
    }
    #[cfg(not(feature = "std"))]
    {
        let ptr = MEMORY_PROFILER.load(Ordering::SeqCst);
        if ptr.is_null() {
            Err(wrt_error::Error::memory_error("Profiler not initialized"))
        } else {
            // SAFETY: We only store valid pointers from Box::leak
            // and the profiler has 'static lifetime
            #[allow(unsafe_code)]
            unsafe {
                f(&mut *ptr)
            }
        }
    }
}

/// Macro for tracking allocations
#[macro_export]
macro_rules! track_allocation {
    ($crate_id:expr, $size:expr, $type:expr, $tag:expr) => {{
        $crate::memory_profiling::with_profiler(|profiler| {
            profiler.track_allocation($crate_id, $size, $type, $tag)
        })
    }};
}

/// Macro for tracking deallocations
#[macro_export]
macro_rules! track_deallocation {
    ($alloc_id:expr) => {{ $crate::memory_profiling::with_profiler(|profiler| profiler.track_deallocation($alloc_id)) }};
}

/// Macro for profiling operations
#[macro_export]
macro_rules! profile_operation {
    ($operation:expr, $crate_id:expr, $body:expr) => {{
        let _handle = $crate::memory_profiling::with_profiler(|profiler| {
            Ok(profiler.start_profiling($operation, $crate_id))
        });

        let _result = $body;

        if let Ok(handle) = _handle {
            let _ = $crate::memory_profiling::with_profiler(|profiler| {
                profiler.complete_profiling(handle)
            });
        }

        _result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_profiler_basic() {
        init_profiler().unwrap();
        MemoryProfiler::enable_allocation_tracking();

        // Track an allocation
        let alloc_id = with_profiler(|profiler| {
            profiler.track_allocation(
                CrateId::Runtime,
                1024,
                AllocationType::Heap,
                "test_allocation",
            )
        })
        .unwrap();

        assert!(alloc_id > 0);

        // Track deallocation
        with_profiler(|profiler| profiler.track_deallocation(alloc_id)).unwrap();

        // Generate report
        let report = with_profiler(|profiler| profiler.generate_profile_report()).unwrap();

        assert_eq!(report.total_allocations, 1);
        assert_eq!(report.total_deallocations, 1);
    }

    #[test]
    fn test_leak_detection() {
        init_profiler().unwrap();
        MemoryProfiler::enable_allocation_tracking();

        // Create allocation without deallocation
        let _alloc_id = with_profiler(|profiler| {
            profiler.track_allocation(
                CrateId::Component,
                1024 * 1024 * 2, // 2MB
                AllocationType::Heap,
                "potential_leak",
            )
        })
        .unwrap();

        // Detect leaks
        let leaks = with_profiler(|profiler| profiler.detect_leaks()).unwrap();

        // Should detect the large allocation as potential leak
        assert!(!leaks.is_empty());
    }

    #[test]
    fn test_profiling() {
        init_profiler().unwrap();
        MemoryProfiler::enable_profiling();

        // Profile an operation
        let result = profile_operation!("test_operation", CrateId::Foundation, {
            // Simulate some work
            let mut sum = 0;
            for i in 0..100 {
                sum += i;
            }
            sum
        });

        assert_eq!(result, 4950);

        // Check that profiling was recorded
        let report = with_profiler(|profiler| profiler.generate_profile_report()).unwrap();

        assert!(!report.performance_metrics.slowest_operations.is_empty());
    }
}
