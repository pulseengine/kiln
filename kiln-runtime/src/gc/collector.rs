//! Tracing Garbage Collector
//!
//! Implements a mark-and-sweep garbage collector for WebAssembly GC objects.
//! Designed for no_std compatibility with fixed-size data structures.

use wrt_error::{Error, Result};

use super::{
    heap::GcHeap,
    object::{ObjectKind, HEADER_SIZE},
    GcRef, GC_OBJECT_ALIGNMENT,
};

/// Maximum size of the mark stack (for no_std compatibility)
const MAX_MARK_STACK: usize = 1024;

/// GC statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct GcStats {
    /// Number of collections performed
    pub collections: u64,
    /// Total bytes reclaimed
    pub bytes_reclaimed: u64,
    /// Total objects reclaimed
    pub objects_reclaimed: u64,
    /// Last collection time in microseconds (if available)
    pub last_collection_us: u64,
}

/// Garbage collector state
#[derive(Debug)]
pub struct GcCollector {
    /// Mark stack for traversal (fixed-size for no_std)
    mark_stack: [GcRef; MAX_MARK_STACK],
    /// Current mark stack pointer
    mark_stack_ptr: usize,
    /// Collection statistics
    stats: GcStats,
    /// Allocation threshold for triggering GC (bytes)
    threshold: usize,
    /// Bytes allocated since last collection
    bytes_since_collection: usize,
}

impl Default for GcCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl GcCollector {
    /// Create a new garbage collector
    pub const fn new() -> Self {
        Self {
            mark_stack: [GcRef::null(); MAX_MARK_STACK],
            mark_stack_ptr: 0,
            stats: GcStats {
                collections: 0,
                bytes_reclaimed: 0,
                objects_reclaimed: 0,
                last_collection_us: 0,
            },
            threshold: 64 * 1024, // 64KB default threshold
            bytes_since_collection: 0,
        }
    }

    /// Set the allocation threshold for triggering GC
    pub fn set_threshold(&mut self, threshold: usize) {
        self.threshold = threshold;
    }

    /// Get the allocation threshold
    pub const fn threshold(&self) -> usize {
        self.threshold
    }

    /// Get collection statistics
    pub const fn stats(&self) -> &GcStats {
        &self.stats
    }

    /// Record an allocation (for threshold tracking)
    pub fn record_allocation(&mut self, size: usize) {
        self.bytes_since_collection += size;
    }

    /// Check if GC should be triggered based on allocation threshold
    pub const fn should_collect(&self) -> bool {
        self.bytes_since_collection >= self.threshold
    }

    /// Push a reference onto the mark stack
    fn push_mark(&mut self, gc_ref: GcRef) -> Result<()> {
        if gc_ref.is_null() {
            return Ok(());
        }

        if self.mark_stack_ptr >= MAX_MARK_STACK {
            return Err(Error::memory_error("Mark stack overflow"));
        }

        self.mark_stack[self.mark_stack_ptr] = gc_ref;
        self.mark_stack_ptr += 1;
        Ok(())
    }

    /// Pop a reference from the mark stack
    fn pop_mark(&mut self) -> Option<GcRef> {
        if self.mark_stack_ptr == 0 {
            return None;
        }

        self.mark_stack_ptr -= 1;
        let gc_ref = self.mark_stack[self.mark_stack_ptr];
        self.mark_stack[self.mark_stack_ptr] = GcRef::null();
        Some(gc_ref)
    }

    /// Perform a full garbage collection
    ///
    /// # Arguments
    /// * `heap` - The GC heap to collect
    /// * `roots` - Iterator over root references
    pub fn collect<'a, const SIZE: usize>(
        &mut self,
        heap: &mut GcHeap<SIZE>,
        roots: impl Iterator<Item = GcRef>,
    ) -> Result<()> {
        // Phase 1: Clear all marks
        heap.clear_marks();
        self.mark_stack_ptr = 0;

        // Phase 2: Mark from roots
        for root in roots {
            self.mark_from_root(heap, root)?;
        }

        // Phase 3: Process mark stack until empty
        while let Some(gc_ref) = self.pop_mark() {
            self.trace_object(heap, gc_ref)?;
        }

        // Phase 4: Sweep (note: current implementation is non-compacting)
        // In a full implementation, we would:
        // 1. Scan the heap for unmarked objects
        // 2. Add them to a free list or compact memory
        // For now, we just track stats

        // Update statistics
        self.stats.collections += 1;
        self.bytes_since_collection = 0;

        Ok(())
    }

    /// Mark an object and add it to the work stack
    fn mark_from_root<const SIZE: usize>(
        &mut self,
        heap: &mut GcHeap<SIZE>,
        gc_ref: GcRef,
    ) -> Result<()> {
        if gc_ref.is_null() {
            return Ok(());
        }

        // Skip if already marked
        if heap.is_marked(gc_ref) {
            return Ok(());
        }

        // Mark and add to work stack
        heap.mark(gc_ref)?;
        self.push_mark(gc_ref)?;

        Ok(())
    }

    /// Trace references within an object
    fn trace_object<const SIZE: usize>(
        &mut self,
        heap: &mut GcHeap<SIZE>,
        gc_ref: GcRef,
    ) -> Result<()> {
        // First, extract the information we need without holding the borrow
        let (kind, payload_offset, payload_len) = {
            let obj = heap.get(gc_ref)?;
            let header = obj.header();
            let offset = gc_ref.offset().unwrap() as usize + HEADER_SIZE;
            (header.kind(), offset, obj.payload().len())
        };

        match kind {
            ObjectKind::Struct => {
                // Scan struct fields for references
                self.scan_payload_for_refs(heap, payload_offset, payload_len)?;
            }
            ObjectKind::Array => {
                // Scan array elements for references
                // Skip the length field (first 4 bytes)
                if payload_len > 4 {
                    self.scan_payload_for_refs(heap, payload_offset + 4, payload_len - 4)?;
                }
            }
            ObjectKind::I31 => {
                // i31 values don't contain references
            }
        }

        Ok(())
    }

    /// Scan a payload for potential references (conservative)
    fn scan_payload_for_refs<const SIZE: usize>(
        &mut self,
        heap: &mut GcHeap<SIZE>,
        payload_offset: usize,
        payload_len: usize,
    ) -> Result<()> {
        // Collect potential refs first to avoid borrow conflicts
        let mut potential_refs = [GcRef::null(); 64];
        let mut ref_count = 0;

        // Scan 4-byte aligned positions for potential GcRefs
        let mut offset = payload_offset;
        let end = payload_offset + payload_len;

        while offset + 4 <= end && ref_count < 64 {
            // Read 4 bytes directly from heap memory
            let value = heap.read_u32_at(offset)?;

            // Check if this could be a valid GC reference
            // (non-zero, aligned, within heap bounds)
            if value != 0 && value % GC_OBJECT_ALIGNMENT as u32 == 0 {
                let potential_ref = GcRef::from_offset(value);

                // Validate it's a real object by checking if get() succeeds
                if heap.get(potential_ref).is_ok() {
                    potential_refs[ref_count] = potential_ref;
                    ref_count += 1;
                }
            }

            offset += 4;
        }

        // Now process the collected refs
        for i in 0..ref_count {
            let potential_ref = potential_refs[i];
            if !heap.is_marked(potential_ref) {
                heap.mark(potential_ref)?;
                self.push_mark(potential_ref)?;
            }
        }

        Ok(())
    }

    /// Collect garbage with an explicit root set
    pub fn collect_with_roots<const SIZE: usize>(
        &mut self,
        heap: &mut GcHeap<SIZE>,
        roots: &[GcRef],
    ) -> Result<()> {
        self.collect(heap, roots.iter().copied())
    }
}

/// Root set for garbage collection
///
/// Tracks all root references that should keep objects alive.
#[derive(Debug)]
pub struct RootSet {
    /// Root references (fixed-size for no_std)
    roots: [GcRef; 256],
    /// Number of active roots
    count: usize,
}

impl Default for RootSet {
    fn default() -> Self {
        Self::new()
    }
}

impl RootSet {
    /// Create a new empty root set
    pub const fn new() -> Self {
        Self {
            roots: [GcRef::null(); 256],
            count: 0,
        }
    }

    /// Add a root reference
    pub fn add(&mut self, gc_ref: GcRef) -> Result<()> {
        if self.count >= 256 {
            return Err(Error::memory_error("Root set full"));
        }

        self.roots[self.count] = gc_ref;
        self.count += 1;
        Ok(())
    }

    /// Remove a root reference (swap-remove for efficiency)
    pub fn remove(&mut self, gc_ref: GcRef) {
        for i in 0..self.count {
            if self.roots[i] == gc_ref {
                self.count -= 1;
                self.roots[i] = self.roots[self.count];
                self.roots[self.count] = GcRef::null();
                return;
            }
        }
    }

    /// Clear all roots
    pub fn clear(&mut self) {
        for i in 0..self.count {
            self.roots[i] = GcRef::null();
        }
        self.count = 0;
    }

    /// Get the number of roots
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Iterate over roots
    pub fn iter(&self) -> impl Iterator<Item = GcRef> + '_ {
        self.roots[..self.count].iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_set() {
        let mut roots = RootSet::new();
        assert!(roots.is_empty());

        let ref1 = GcRef::from_offset(8);
        let ref2 = GcRef::from_offset(16);

        roots.add(ref1).unwrap();
        roots.add(ref2).unwrap();
        assert_eq!(roots.len(), 2);

        roots.remove(ref1);
        assert_eq!(roots.len(), 1);

        roots.clear();
        assert!(roots.is_empty());
    }

    #[test]
    fn test_gc_collector_basic() {
        let mut heap = GcHeap::<4096>::new();
        let mut collector = GcCollector::new();
        let mut roots = RootSet::new();

        // Allocate some objects
        let obj1 = heap.alloc_struct(0, &[4, 4]).unwrap();
        let obj2 = heap.alloc_struct(0, &[4]).unwrap();

        // Only obj1 is a root
        roots.add(obj1).unwrap();

        // Run collection
        collector.collect_with_roots(&mut heap, &[obj1]).unwrap();

        // obj1 should be marked, obj2 should not
        assert!(heap.is_marked(obj1));
        assert!(!heap.is_marked(obj2)); // Not reachable, would be collected
    }

    #[test]
    fn test_gc_threshold() {
        let mut collector = GcCollector::new();

        collector.set_threshold(1000);
        assert_eq!(collector.threshold(), 1000);

        collector.record_allocation(500);
        assert!(!collector.should_collect());

        collector.record_allocation(600);
        assert!(collector.should_collect());
    }

    #[test]
    fn test_gc_stats() {
        let mut heap = GcHeap::<4096>::new();
        let mut collector = GcCollector::new();

        assert_eq!(collector.stats().collections, 0);

        collector.collect_with_roots(&mut heap, &[]).unwrap();

        assert_eq!(collector.stats().collections, 1);
    }
}
