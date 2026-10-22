//! Zero-Configuration Memory Management Macros
//!
//! This module provides the convenience macros that make the Kiln memory system
//! extremely easy to use - achieving A+ grade usability.
//!
//! SW-REQ-ID: REQ_USABILITY_001 - Zero boilerplate APIs

/// Zero-configuration memory allocation macro
///
/// **DEPRECATED**: Use `safe_managed_alloc!` instead for better safety
/// guarantees.
///
/// This macro provides the simplest possible interface for budget-aware memory
/// allocation.
///
/// # Examples
///
/// ```rust
/// # fn main() -> kiln_error::Result<()> {
/// use kiln_foundation::{
///     safe_managed_alloc,
///     BoundedVec,
///     CrateId,
/// };
///
/// // Simple allocation with automatic cleanup
/// let provider = safe_managed_alloc!(1024, CrateId::Component)?;
///
/// // Use provider for bounded collections
/// let mut vec: BoundedVec<u32, 16, _> = BoundedVec::new(provider)?;
/// vec.push(42)?;
///
/// // Memory automatically returned on drop
/// # Ok(())
/// # }
/// ```
///
/// # Safety
///
/// This macro ensures:
/// - Budget checking at compile time where possible
/// - Runtime budget enforcement
/// - Automatic cleanup via RAII
/// - Impossible to bypass the memory system
///
/// # Migration
///
/// Replace `safe_managed_alloc!(size, crate_id)` with
/// `safe_managed_alloc!(size, crate_id)?`
#[deprecated(
    since = "0.1.0",
    note = "Use `safe_managed_alloc!` instead for better safety guarantees"
)]
#[macro_export]
macro_rules! managed_alloc {
    ($size:expr, $crate_id:expr) => {{
        // An init failure surfaces as the allocation error below.
        let _ = $crate::memory_init::MemoryInitializer::initialize();

        // Create allocation through modern capability factory
        $crate::kiln_memory_system::CapabilityKilnFactory::create_provider::<$size>($crate_id)
    }};
}

/// Create a memory provider with specific size and crate
///
/// This is a more explicit version of `safe_managed_alloc!` for when you need
/// to specify the exact provider type.
///
/// # Examples
///
/// ```rust
/// # fn main() -> kiln_error::Result<()> {
/// use kiln_foundation::{
///     create_provider,
///     safe_memory::NoStdProvider,
///     CrateId,
/// };
///
/// // Create typed provider (the macro currently always allocates a 1024-byte
/// // provider, so the provider type must be `NoStdProvider<1024>`)
/// let guard = create_provider!(NoStdProvider<1024>, CrateId::Foundation)?;
/// # let _ = guard;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! create_provider {
    ($provider_type:ty, $crate_id:expr) => {{
        // An init failure surfaces as the allocation error below.
        let _ = $crate::memory_init::MemoryInitializer::initialize();
        // MIGRATION NOTE: create_typed_provider moved to capability system
        // This macro now requires explicit size specification in provider type
        $crate::kiln_memory_system::CapabilityKilnFactory::create_provider::<1024>($crate_id)
    }};
}

/// Hierarchical budget allocation macro
///
/// Creates a hierarchical budget with sub-allocations for complex systems.
///
/// # Examples
///
/// ```rust
/// # fn main() -> kiln_error::Result<()> {
/// use kiln_foundation::{
///     hierarchical_budget,
///     CrateId,
///     MemoryPriority,
/// };
///
/// let budget = hierarchical_budget! {
///     crate_id: CrateId::Component,
///     total: 1_000_000,
///     sub_budgets: [
///         ("interpreter", 400_000, MemoryPriority::Critical),
///         ("compiler", 300_000, MemoryPriority::High),
///         ("cache", 200_000, MemoryPriority::Normal),
///         ("temp", 100_000, MemoryPriority::Low),
///     ]
/// };
/// # let _ = budget;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! hierarchical_budget {
    {
        crate_id: $crate_id:expr,
        total: $total:expr,
        sub_budgets: [
            $(($name:literal, $size:expr, $priority:expr)),* $(,)?
        ]
    } => {{
        let mut budget = $crate::hierarchical_budgets::HierarchicalBudget::<8>::new($crate_id, $total);

        $(
            budget.add_sub_budget($name, $size, $priority)?;
        )*

        budget
    }};
}

/// Memory region validation macro
///
/// Creates compile-time validated memory regions.
///
/// # Examples
///
/// ```rust
/// use kiln_foundation::memory_region;
///
/// // Compile-time validated region
/// let region = memory_region!(start: 0, size: 4096);
/// assert_eq!(region.size(), 4096);
/// ```
#[macro_export]
macro_rules! memory_region {
    (start: $start:expr,size: $size:expr) => {{ $crate::enforcement::MemoryRegion::<{ $start }, { $size }>::new() }};
}

/// Capability-based allocation token macro
///
/// Creates allocation tokens with compile-time size verification.
///
/// # Examples
///
/// ```rust
/// # fn main() -> kiln_error::Result<()> {
/// use kiln_foundation::{
///     allocation_token,
///     memory_init::{
///         get_global_capability_context,
///         MemoryInitializer,
///     },
///     CrateId,
/// };
///
/// MemoryInitializer::initialize()?;
/// let context = get_global_capability_context()?;
///
/// let token = allocation_token!(size: 1024, crate_id: CrateId::Foundation);
/// let guard = token.allocate(context)?;
/// # let _ = guard;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! allocation_token {
    (size: $size:expr,crate_id: $crate_id:expr) => {{ $crate::enforcement::AllocationToken::<{ $size }>::new($crate_id) }};
}

/// Auto-sizing provider creation macro
///
/// Automatically determines the best provider size based on usage patterns.
///
/// # Examples
///
/// ```rust
/// # fn main() -> kiln_error::Result<()> {
/// use kiln_foundation::{auto_provider, CrateId};
///
/// // Automatically sized for typical usage
/// let guard = auto_provider!(CrateId::Component, typical_usage: "bounded_collections")?;
/// # let _ = guard;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! auto_provider {
    ($crate_id:expr,typical_usage: "bounded_collections") => {{ $crate::safe_managed_alloc!(4096, $crate_id) }};
    ($crate_id:expr,typical_usage: "string_processing") => {{ $crate::safe_managed_alloc!(8192, $crate_id) }};
    ($crate_id:expr,typical_usage: "temporary_buffers") => {{ $crate::safe_managed_alloc!(2048, $crate_id) }};
    ($crate_id:expr,typical_usage: "large_data") => {{ $crate::safe_managed_alloc!(16384, $crate_id) }};
    ($crate_id:expr) => {{ $crate::safe_managed_alloc!(4096, $crate_id) }};
}

/// Development/debug macro with enhanced monitoring
///
/// Provides additional debugging and monitoring features for development.
///
/// # Examples
///
/// ```rust
/// # fn main() -> kiln_error::Result<()> {
/// use kiln_foundation::{
///     debug_alloc,
///     CrateId,
/// };
///
/// // The macro propagates allocation errors with `?` itself and yields the provider
/// let guard = debug_alloc!(1024, CrateId::Component, "parsing logic");
/// # let _ = guard;
/// # Ok(())
/// # }
/// ```
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_alloc {
    ($size:expr, $crate_id:expr, $purpose:literal) => {{
        // Log allocation for debugging
        $crate::debug_println!(
            "Allocating {} bytes for {} in {}",
            $size,
            $purpose,
            $crate::memory_coordinator::CrateIdentifier::name(&$crate_id)
        );

        let guard = $crate::safe_managed_alloc!($size, $crate_id)?;

        // Track allocation in debug mode
        $crate::monitoring::debug_track_allocation($crate_id, $size, $purpose);

        guard
    }};
}

/// Release mode equivalent of debug_alloc (no-op)
#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug_alloc {
    ($size:expr, $crate_id:expr, $purpose:literal) => {{ $crate::safe_managed_alloc!($size, $crate_id) }};
}

/// Conditional debug printing macro
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {{
        #[cfg(feature = "std")]
        println!($($arg)*);
    }};
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {{}};
}
