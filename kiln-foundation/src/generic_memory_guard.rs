//! Generic RAII Memory Guard
//!
//! This module provides a generic RAII guard that can work with any memory
//! provider and coordinator implementation, making it reusable across different
//! projects.
//!
//! SW-REQ-ID: REQ_MEM_003 - Automatic cleanup
//! SW-REQ-ID: REQ_ERROR_004 - Safe resource management

use core::{
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{
        Deref,
        DerefMut,
    },
};

use crate::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// Trait for memory providers that can be managed by the guard
pub trait ManagedMemoryProvider: Sized {
    /// Get the size of this provider's allocation
    fn allocation_size(&self) -> usize;
}

/// Trait for memory coordinators that can track allocations
pub trait MemoryCoordinator<CrateId> {
    /// Allocation identifier type
    type AllocationId: Copy;

    /// Register a new allocation
    fn register_allocation(&self, crate_id: CrateId, size: usize) -> Result<Self::AllocationId>;

    /// Return an allocation
    fn return_allocation(
        &self,
        crate_id: CrateId,
        id: Self::AllocationId,
        size: usize,
    ) -> Result<()>;
}

/// Generic RAII guard for automatic memory management
///
/// This guard works with any provider P and coordinator C, making it
/// completely generic and reusable.
///
/// # Type Parameters
///
/// * `P` - The memory provider type
/// * `C` - The coordinator type
/// * `I` - The crate identifier type
pub struct GenericMemoryGuard<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    /// The memory provider, wrapped for controlled cleanup
    provider:      ManuallyDrop<P>,
    /// Reference to the coordinator
    coordinator:   &'static C,
    /// The crate that owns this allocation
    crate_id:      I,
    /// Allocation identifier
    allocation_id: C::AllocationId,
    /// Size of the allocation (cached for cleanup)
    size:          usize,
    /// Whether cleanup has been performed
    cleaned_up:    bool,
    /// Phantom data for lifetime management
    _phantom:      PhantomData<(C, I)>,
}

impl<P, C, I> GenericMemoryGuard<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    /// Create a new memory guard
    ///
    /// # Arguments
    ///
    /// * `provider` - The memory provider to manage
    /// * `coordinator` - The coordinator for tracking allocations
    /// * `crate_id` - The crate requesting the allocation
    pub fn new(provider: P, coordinator: &'static C, crate_id: I) -> Result<Self> {
        let size = provider.allocation_size();

        // Register with coordinator
        let allocation_id = coordinator.register_allocation(crate_id, size)?;

        // Record allocation in monitoring system
        crate::monitoring::MEMORY_MONITOR.record_allocation(size);

        Ok(Self {
            provider: ManuallyDrop::new(provider),
            coordinator,
            crate_id,
            allocation_id,
            size,
            cleaned_up: false,
            _phantom: PhantomData,
        })
    }

    /// Get a reference to the provider
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Get a mutable reference to the provider
    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }

    /// Get the crate ID
    pub fn crate_id(&self) -> I {
        self.crate_id
    }

    /// Get the allocation size
    pub fn size(&self) -> usize {
        self.size
    }

    // Memory providers are managed through capability system with automatic RAII
    // cleanup Use capability-driven memory management through MemoryFactory
    // instead
}

impl<P, C, I> Drop for GenericMemoryGuard<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    fn drop(&mut self) {
        if self.cleaned_up {
            return;
        }

        // Return allocation to coordinator
        // Intentionally ignore errors in Drop to avoid panic
        let _ = self.coordinator.return_allocation(self.crate_id, self.allocation_id, self.size);

        // Record deallocation in monitoring system
        crate::monitoring::MEMORY_MONITOR.record_deallocation(self.size);

        // Drop the provider
        #[allow(unsafe_code)]
        unsafe {
            ManuallyDrop::drop(&mut self.provider);
        }

        self.cleaned_up = true;
    }
}

// Deref implementations for ergonomic access
impl<P, C, I> Deref for GenericMemoryGuard<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}

impl<P, C, I> DerefMut for GenericMemoryGuard<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.provider
    }
}

/// Builder pattern for creating memory guards with nice API
pub struct MemoryGuardBuilder<P, C, I>
where
    C: 'static,
{
    provider:    Option<P>,
    coordinator: Option<&'static C>,
    crate_id:    Option<I>,
    _phantom:    PhantomData<(P, C, I)>,
}

impl<P, C, I> Default for MemoryGuardBuilder<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<P, C, I> MemoryGuardBuilder<P, C, I>
where
    P: ManagedMemoryProvider,
    C: MemoryCoordinator<I> + 'static,
    I: Copy,
{
    pub fn new() -> Self {
        Self {
            provider:    None,
            coordinator: None,
            crate_id:    None,
            _phantom:    PhantomData,
        }
    }

    pub fn provider(mut self, provider: P) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn coordinator(mut self, coordinator: &'static C) -> Self {
        self.coordinator = Some(coordinator);
        self
    }

    pub fn crate_id(mut self, id: I) -> Self {
        self.crate_id = Some(id);
        self
    }

    pub fn build(self) -> Result<GenericMemoryGuard<P, C, I>> {
        let provider = self
            .provider
            .ok_or_else(|| Error::initialization_error("Provider not specified"))?;

        let coordinator = self
            .coordinator
            .ok_or_else(|| Error::initialization_error("Coordinator not specified"))?;

        let crate_id = self
            .crate_id
            .ok_or_else(|| Error::initialization_error("Crate ID not specified"))?;

        GenericMemoryGuard::new(provider, coordinator, crate_id)
    }
}

/// Convenience type alias for common use cases
pub type MemoryGuard<P, I> =
    GenericMemoryGuard<P, crate::memory_coordinator::GenericMemoryCoordinator<I, 32>, I>;

