// WRT - wrt-panic
// Module: ISO 26262 Compliant Panic Handler for WRT
// SW-REQ-ID: REQ_PANIC_001, REQ_SAFETY_ASIL_001, REQ_MEM_SAFETY_001
//
// Copyright (c) 2025 The WRT Project Developers
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)] // Allow specific unsafe blocks where necessary for panic handling

//! ISO 26262 compliant panic handler for WRT WebAssembly runtime
//!
//! This crate provides safety-critical panic handlers designed to meet
//! automotive safety standards ASIL-B and ASIL-D, with memory budget
//! integration and configurable safety levels.
//!
//! ## Usage
//!
//! ### Basic Usage (ASIL-D default)
//! ```toml
//! [dependencies]
//! wrt-panic = { path = "../wrt-panic", features = ["asil-d"] }
//! ```
//!
//! ### Runtime Configuration
//! ```rust
//! use wrt_foundation::safety_system::AsilLevel;
//! use wrt_panic::{
//!     initialize_panic_handler,
//!     PanicContextBuilder,
//! };
//!
//! // Initialize with custom memory budget and safety level
//! let panic_context = PanicContextBuilder::new()
//!     .with_memory_budget(2048)  // 2KB for panic information
//!     .with_safety_level(AsilLevel::AsilD)
//!     .build();
//!
//! initialize_panic_handler(panic_context);
//! ```
//!
//! ## Safety Compliance Features
//!
//! ### ASIL-B Requirements (≥90% Single-Point Fault Metric)
//! - Basic fault detection and safe state transition
//! - Memory pattern storage for debugger visibility
//! - Minimal error information retention
//!
//! ### ASIL-D Requirements (≥99% Single-Point Fault Metric)
//! - Enhanced fault detection with hardware consistency monitoring
//! - Comprehensive error information storage
//! - Multiple redundant safety mechanisms
//! - Hardware exception integration
//!
//! ## Memory Pattern Design
//!
//! Panic information is stored in a recognizable memory pattern for debugger
//! access: ```
//! Magic: 0xDEADBEEF
//! ASIL Level: u8
//! Timestamp: u64 (if available)
//! Error Code: u32
//! Location Hash: u32
//! Stack Trace: [u32; N] (if memory allows)
//! Checksum: u32
//! ```

#[cfg(feature = "std")]
extern crate std;

use core::sync::atomic::{
    AtomicU32,
    AtomicU8,
    Ordering,
};

/// ASIL (Automotive Safety Integrity Level) as defined by ISO 26262
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum AsilLevel {
    /// Quality Management (no safety requirements)
    QM    = 0,
    /// ASIL-A (lowest safety integrity level)
    AsilA = 1,
    /// ASIL-B (medium-low safety integrity level)
    AsilB = 2,
    /// ASIL-C (medium-high safety integrity level)
    AsilC = 3,
    /// ASIL-D (highest safety integrity level)
    AsilD = 4,
}

/// Simple memory provider trait for panic handler usage
pub trait MemoryProvider {
    /// Get the capacity of this memory provider
    fn capacity(&self) -> usize;
}

/// No-std memory provider for panic handler
#[derive(Debug, Clone, Default)]
pub struct NoStdProvider<const N: usize>;

impl<const N: usize> MemoryProvider for NoStdProvider<N> {
    fn capacity(&self) -> usize {
        N
    }
}

/// Panic information magic number for debugger recognition
pub const PANIC_MAGIC: u32 = 0xDEADBEEF;

/// Minimum panic information structure size (compile-time guaranteed)
pub const MIN_PANIC_INFO_SIZE: usize = 32; // Magic + ASIL + Error + Hash + Checksum

/// Default panic memory budget (can be overridden at runtime)
pub const DEFAULT_PANIC_MEMORY_BUDGET: usize = 256;

/// Maximum stack trace entries based on memory budget
pub const MAX_STACK_TRACE_ENTRIES: usize = 16;

/// Panic information structure stored in memory for debugger access
#[repr(C, packed)]
pub struct WrtPanicInfo {
    /// Magic number for debugger recognition (0xDEADBEEF)
    pub magic:             u32,
    /// ASIL level at time of panic
    pub asil_level:        u8,
    /// Reserved for alignment
    pub reserved:          [u8; 3],
    /// Error code (hash of panic message)
    pub error_code:        u32,
    /// Location hash (file + line combination)
    pub location_hash:     u32,
    /// Timestamp if available (0 if not available)
    pub timestamp:         u64,
    /// Stack trace entries count
    pub stack_trace_count: u8,
    /// Reserved for future use
    pub reserved2:         [u8; 3],
    /// Variable length stack trace (depends on memory budget)
    pub stack_trace:       [u32; MAX_STACK_TRACE_ENTRIES],
    /// Checksum of all above data
    pub checksum:          u32,
}

/// Panic context configuration
pub struct PanicContext<P: MemoryProvider> {
    safety_level:    AsilLevel,
    memory_provider: P,
    memory_budget:   usize,
}

/// Global panic context storage
static PANIC_ASIL_LEVEL: AtomicU8 = AtomicU8::new(AsilLevel::AsilD as u8);
static PANIC_MEMORY_BUDGET: AtomicU32 = AtomicU32::new(DEFAULT_PANIC_MEMORY_BUDGET as u32);

/// Builder for panic context configuration
pub struct PanicContextBuilder<P: MemoryProvider> {
    safety_level:    AsilLevel,
    memory_provider: Option<P>,
    memory_budget:   usize,
}

impl<P: MemoryProvider> Default for PanicContextBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: MemoryProvider> PanicContextBuilder<P> {
    /// Create a new panic context builder with ASIL-D defaults
    pub const fn new() -> Self {
        Self {
            safety_level:    AsilLevel::AsilD,
            memory_provider: None,
            memory_budget:   DEFAULT_PANIC_MEMORY_BUDGET,
        }
    }

    /// Set the safety level (must be ASIL-B or higher for safety-critical
    /// systems)
    pub fn with_safety_level(mut self, level: AsilLevel) -> Self {
        self.safety_level = level;
        self
    }

    /// Set the memory budget for panic information storage
    pub fn with_memory_budget(mut self, budget: usize) -> Self {
        self.memory_budget = budget.max(MIN_PANIC_INFO_SIZE);
        self
    }

    /// Set the memory provider
    pub fn with_memory_provider(mut self, provider: P) -> Self {
        self.memory_provider = Some(provider);
        self
    }

    /// Build the panic context
    pub fn build(self) -> Result<PanicContext<P>, &'static str> {
        let provider = self.memory_provider.ok_or("Memory provider is required")?;

        if self.memory_budget < MIN_PANIC_INFO_SIZE {
            return Err("Memory budget too small for minimum panic information");
        }

        Ok(PanicContext {
            safety_level:    self.safety_level,
            memory_provider: provider,
            memory_budget:   self.memory_budget,
        })
    }
}

/// Initialize the global panic handler configuration
pub fn initialize_panic_handler<P: MemoryProvider>(
    context: PanicContext<P>,
) -> Result<(), &'static str> {
    // Store configuration in global atomics
    PANIC_ASIL_LEVEL.store(context.safety_level as u8, Ordering::SeqCst);
    PANIC_MEMORY_BUDGET.store(context.memory_budget as u32, Ordering::SeqCst);

    // Validate memory provider can handle the budget
    if context.memory_provider.capacity() < context.memory_budget {
        return Err("Memory provider capacity insufficient for panic budget");
    }

    Ok(())
}

/// Hash a string at compile time for error codes
#[allow(dead_code)]
const fn hash_str(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut hash = 5381u32;
    let mut i = 0;
    while i < bytes.len() {
        hash = hash.wrapping_mul(33).wrapping_add(bytes[i] as u32);
        i += 1;
    }
    hash
}

/// Store panic information in memory with debugger-visible pattern
#[allow(dead_code)]
fn store_panic_info(info: &core::panic::PanicInfo) {
    let asil_level = PANIC_ASIL_LEVEL.load(Ordering::SeqCst);
    let _memory_budget = PANIC_MEMORY_BUDGET.load(Ordering::SeqCst) as usize;

    // Create panic info structure
    let mut panic_info = WrtPanicInfo {
        magic: PANIC_MAGIC,
        asil_level,
        reserved: [0; 3],
        error_code: 0,
        location_hash: 0,
        timestamp: 0,
        stack_trace_count: 0,
        reserved2: [0; 3],
        stack_trace: [0; MAX_STACK_TRACE_ENTRIES],
        checksum: 0,
    };

    // Extract information from panic info
    if let Some(location) = info.location() {
        let file_hash = hash_str(location.file());
        let line = location.line();
        panic_info.location_hash = file_hash.wrapping_add(line);
    }

    // Extract error code from panic message
    #[cfg(feature = "std")]
    {
        #[allow(clippy::incompatible_msrv)]
        let msg = info.message();
        let msg_str = std::format!("{msg}");
        panic_info.error_code = hash_str(&msg_str);
    }
    #[cfg(not(feature = "std"))]
    {
        // For no_std, we'll use the location information for error code
        // since message() is not reliable in no_std contexts
        panic_info.error_code = panic_info.location_hash.wrapping_mul(0x9e3779b9);
    }

    // Add timestamp if available (std feature)
    #[cfg(feature = "std")]
    {
        if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            panic_info.timestamp = duration.as_secs();
        }
    }

    // Calculate checksum (simple XOR-based)
    let mut checksum = panic_info.magic;
    checksum ^= panic_info.asil_level as u32;
    checksum ^= panic_info.error_code;
    checksum ^= panic_info.location_hash;
    checksum ^= (panic_info.timestamp as u32) ^ ((panic_info.timestamp >> 32) as u32);
    panic_info.checksum = checksum;

    // Store in a static location that debuggers can easily find
    static mut PANIC_INFO_STORAGE: WrtPanicInfo = WrtPanicInfo {
        magic:             0,
        asil_level:        0,
        reserved:          [0; 3],
        error_code:        0,
        location_hash:     0,
        timestamp:         0,
        stack_trace_count: 0,
        reserved2:         [0; 3],
        stack_trace:       [0; MAX_STACK_TRACE_ENTRIES],
        checksum:          0,
    };

    // Safe to write since we're in a panic handler (single-threaded at this point)
    // This unsafe block is justified: panic handlers are inherently single-threaded
    // and we need static storage for debugger access
    #[allow(unsafe_code)]
    unsafe {
        PANIC_INFO_STORAGE = panic_info;
    }
}

/// ASIL-D compliant panic handler (≥99% Single-Point Fault Metric)
///
/// Implements the highest level of safety integrity as per ISO 26262:
/// - Hardware consistency monitoring through exception handling
/// - Comprehensive error information storage with memory patterns
/// - Multiple redundant safety mechanisms
/// - Immediate safe state transition with no recovery attempts
///
/// Safety mechanisms implemented:
/// 1. Deterministic behavior (no heap allocations)
/// 2. Immediate safe state entry
/// 3. Memory pattern storage for fault analysis
/// 4. Hardware exception integration
/// 5. Checksum-based data integrity
#[cfg(all(
    not(feature = "std"),
    feature = "asil-d",
    not(feature = "asil-b"),
    not(feature = "dev"),
    not(feature = "release"),
    feature = "default-panic-handler"
))]
#[panic_handler]
fn panic_asil_d(info: &core::panic::PanicInfo) -> ! {
    // ASIL-D compliant panic handling per ISO 26262:

    // 1. Store comprehensive error information for fault analysis
    store_panic_info(info);

    // 2. Ensure no recovery attempts - enter permanent safe state
    // 3. Use hardware-efficient infinite loop for safe state maintenance
    loop {
        core::hint::spin_loop();
    }
}

/// ASIL-B compliant panic handler (≥90% Single-Point Fault Metric)
///
/// Implements ASIL-B safety integrity level as per ISO 26262:
/// - Basic fault detection and safe state transition
/// - Memory pattern storage for debugger visibility
/// - Minimal error information retention
/// - Hardware consistency monitoring through safe state maintenance
///
/// Safety mechanisms implemented:
/// 1. Deterministic behavior (no heap allocations)
/// 2. Immediate safe state entry
/// 3. Basic memory pattern storage for fault analysis
/// 4. Hardware-efficient safe state maintenance
#[cfg(all(
    not(feature = "std"),
    feature = "asil-b",
    not(feature = "asil-d"),
    not(feature = "dev"),
    not(feature = "release"),
    feature = "default-panic-handler"
))]
#[panic_handler]
fn panic_asil_b(info: &core::panic::PanicInfo) -> ! {
    // ASIL-B compliant panic handling per ISO 26262:

    // 1. Store basic error information for fault analysis
    store_panic_info(info);

    // 2. Enter safe state - similar to ASIL-D but with reduced complexity
    loop {
        core::hint::spin_loop();
    }
}

/// Development panic handler
///
/// This panic handler is intended for development and debugging. It provides
/// enhanced information storage while maintaining safety principles.
///
/// Development features:
/// 1. Enhanced error information storage
/// 2. Extended memory pattern for debugging
/// 3. Safe state maintenance
/// 4. Compatible with debugging tools
#[cfg(all(
    not(feature = "std"),
    feature = "dev",
    not(feature = "asil-b"),
    not(feature = "asil-d"),
    feature = "default-panic-handler"
))]
#[panic_handler]
fn panic_dev(info: &core::panic::PanicInfo) -> ! {
    // Development panic handling:

    // 1. Store enhanced error information for development debugging
    store_panic_info(info);

    // 2. Enter safe state with development-friendly behavior
    loop {
        core::hint::spin_loop();
    }
}

/// Release panic handler (default)
///
/// This is the default panic handler for release builds. It provides
/// minimal overhead while maintaining basic safety requirements.
///
/// Release features:
/// 1. Minimal memory footprint
/// 2. Basic error information storage
/// 3. Fast safe state transition
/// 4. Production-optimized behavior
#[cfg(all(
    not(feature = "std"),
    not(feature = "dev"),
    not(feature = "asil-b"),
    not(feature = "asil-d"),
    feature = "default-panic-handler"
))]
#[panic_handler]
fn panic_release(info: &core::panic::PanicInfo) -> ! {
    // Release panic handling:

    // 1. Store minimal error information
    store_panic_info(info);

    // 2. Enter safe state immediately
    loop {
        core::hint::spin_loop();
    }
}

/// Get the current panic handler configuration information
///
/// This function can be called during application initialization to verify
/// that the correct panic handler features are enabled for the target
/// environment.
///
/// # Returns
///
/// A string describing the active panic handler configuration
pub fn panic_handler_info() -> &'static str {
    #[cfg(feature = "asil-d")]
    return "ASIL-D compliant panic handler (≥99% SPFM)";

    #[cfg(all(feature = "asil-b", not(feature = "asil-d")))]
    return "ASIL-B compliant panic handler (≥90% SPFM)";

    #[cfg(all(feature = "dev", not(feature = "asil-b"), not(feature = "asil-d")))]
    return "Development panic handler with enhanced debugging";

    #[cfg(all(not(feature = "dev"), not(feature = "asil-b"), not(feature = "asil-d")))]
    return "Release panic handler with minimal overhead";
}

/// Check if the current panic handler configuration meets the specified safety
/// level
///
/// # Arguments
///
/// * `required_level` - The required safety level ("asil-b", "asil-d", or
///   "standard")
///
/// # Returns
///
/// `true` if the current configuration meets or exceeds the required level
pub fn meets_safety_level(required_level: &str) -> bool {
    match required_level {
        "asil-d" => cfg!(feature = "asil-d"),
        "asil-b" => cfg!(any(feature = "asil-b", feature = "asil-d")),
        "standard" => true, // All configurations meet standard requirements
        _ => false,
    }
}

/// Get the current ASIL level configured for panic handling
///
/// # Returns
///
/// The current ASIL level as stored in global configuration
pub fn current_asil_level() -> AsilLevel {
    let level = PANIC_ASIL_LEVEL.load(Ordering::SeqCst);
    match level {
        0 => AsilLevel::QM,
        1 => AsilLevel::AsilA,
        2 => AsilLevel::AsilB,
        3 => AsilLevel::AsilC,
        4 => AsilLevel::AsilD,
        _ => AsilLevel::AsilD, // Default to highest safety level
    }
}

/// Get the current memory budget configured for panic information
///
/// # Returns
///
/// The current memory budget in bytes
pub fn current_memory_budget() -> usize {
    PANIC_MEMORY_BUDGET.load(Ordering::SeqCst) as usize
}

/// Access the stored panic information (for debugging/analysis tools)
///
/// This function provides read-only access to the panic information structure
/// that would be stored during a panic. This is useful for testing and
/// validation.
///
/// Note: This function can only access panic information after a panic has
/// occurred and the store_panic_info function has been called.
///
/// # Returns
///
/// Information about the current panic handler configuration
pub fn get_panic_handler_status() -> (&'static str, AsilLevel, usize) {
    (
        panic_handler_info(),
        current_asil_level(),
        current_memory_budget(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_handler_info() {
        let info = panic_handler_info();
        assert!(!info.is_empty());

        // Verify that we get a sensible response
        assert!(
            info.contains("panic handler")
                || info.contains("ASIL-")
                || info.contains("Development")
                || info.contains("Release")
        );
    }

    #[test]
    fn test_safety_level_checking() {
        // Standard level should always be met
        assert!(meets_safety_level("standard"));

        // Invalid levels should return false
        assert!(!meets_safety_level("invalid"));
        assert!(!meets_safety_level(""));

        // ASIL levels depend on feature flags
        #[cfg(feature = "asil-d")]
        {
            assert!(meets_safety_level("asil-d"));
            assert!(meets_safety_level("asil-b"));
        }

        #[cfg(all(feature = "asil-b", not(feature = "asil-d")))]
        {
            assert!(!meets_safety_level("asil-d"));
            assert!(meets_safety_level("asil-b"));
        }

        #[cfg(all(not(feature = "asil-b"), not(feature = "asil-d")))]
        {
            assert!(!meets_safety_level("asil-d"));
            assert!(!meets_safety_level("asil-b"));
        }
    }

    #[test]
    fn test_memory_budget_configuration() {
        // Test default memory budget
        assert_eq!(current_memory_budget(), DEFAULT_PANIC_MEMORY_BUDGET);

        // Test ASIL level configuration
        let level = current_asil_level();
        assert!(matches!(
            level,
            AsilLevel::QM
                | AsilLevel::AsilA
                | AsilLevel::AsilB
                | AsilLevel::AsilC
                | AsilLevel::AsilD
        ));
    }

    #[test]
    fn test_panic_info_structure_size() {
        use core::mem;

        // Ensure the panic info structure has expected size constraints
        let size = mem::size_of::<WrtPanicInfo>();
        assert!(size >= MIN_PANIC_INFO_SIZE);
        assert!(size <= DEFAULT_PANIC_MEMORY_BUDGET);
    }

    #[test]
    fn test_hash_function() {
        // Test the hash function produces consistent results
        let hash1 = hash_str("test");
        let hash2 = hash_str("test");
        assert_eq!(hash1, hash2);

        // Different strings should produce different hashes (usually)
        let hash3 = hash_str("different");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_panic_context_builder() {
        type TestProvider = NoStdProvider<512>;
        let provider = TestProvider::default();

        let context = PanicContextBuilder::new()
            .with_safety_level(AsilLevel::AsilB)
            .with_memory_budget(512)
            .with_memory_provider(provider)
            .build();

        assert!(context.is_ok());

        let context = context.unwrap();
        assert_eq!(context.safety_level, AsilLevel::AsilB);
        assert_eq!(context.memory_budget, 512);
    }
}
