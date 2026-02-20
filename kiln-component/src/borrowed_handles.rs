//! Borrowed Handles with Lifetime Tracking for WebAssembly Component Model
//!
//! This module implements proper `own<T>` and `borrow<T>` handle semantics
//! with lifetime tracking and ownership validation according to the Component Model.

#[cfg(not(feature = "std"))]
use core::{fmt, mem, marker::PhantomData, sync::atomic::{AtomicU32, AtomicU64, Ordering}};
#[cfg(feature = "std")]
use std::{fmt, mem, marker::PhantomData, sync::atomic::{AtomicU32, AtomicU64, Ordering}};

#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec, sync::Arc};

use wrt_foundation::{
    bounded::{ BoundedString},
    prelude::*,
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::{
    task_manager::TaskId,
    resource_lifecycle_management::{ResourceId, ComponentId},
    types::Value,
};

use wrt_error::{Error, ErrorCategory, Result};

/// Maximum number of borrowed handles in no_std
const MAX_BORROWED_HANDLES: usize = 512;

/// Maximum lifetime stack depth
const MAX_LIFETIME_DEPTH: usize = 32;

/// Handle type for owned resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnHandle<T> {
    /// Raw handle value
    pub raw: u32,
    
    /// Generation counter to detect stale handles
    pub generation: u32,
    
    /// Type marker
    _phantom: PhantomData<T>,
}

/// Handle type for borrowed resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BorrowHandle<T> {
    /// Raw handle value
    pub raw: u32,
    
    /// Generation counter to detect stale handles
    pub generation: u32,
    
    /// Borrow ID for tracking
    pub borrow_id: BorrowId,
    
    /// Type marker
    _phantom: PhantomData<T>,
}

/// Unique identifier for a borrow operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BorrowId(pub u64;

/// Lifetime scope for borrowed handles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifetimeScope(pub u32;

/// Handle lifetime tracker
#[derive(Debug)]
pub struct HandleLifetimeTracker {
    /// Active owned handles
    #[cfg(feature = "std")]
    owned_handles: Vec<OwnedHandleEntry>,
    #[cfg(not(any(feature = "std", )))]
    owned_handles: BoundedVec<OwnedHandleEntry, MAX_BORROWED_HANDLES>,
    
    /// Active borrowed handles
    #[cfg(feature = "std")]
    borrowed_handles: Vec<BorrowedHandleEntry>,
    #[cfg(not(any(feature = "std", )))]
    borrowed_handles: BoundedVec<BorrowedHandleEntry, MAX_BORROWED_HANDLES>,
    
    /// Lifetime scope stack
    #[cfg(feature = "std")]
    scope_stack: Vec<LifetimeScopeEntry>,
    #[cfg(not(any(feature = "std", )))]
    scope_stack: BoundedVec<LifetimeScopeEntry, MAX_LIFETIME_DEPTH>,
    
    /// Next handle ID
    next_handle_id: AtomicU32,
    
    /// Next borrow ID
    next_borrow_id: AtomicU64,
    
    /// Next scope ID
    next_scope_id: AtomicU32,
    
    /// Tracker statistics
    stats: LifetimeStats,
}

/// Entry for an owned handle
#[derive(Debug, Clone)]
pub struct OwnedHandleEntry {
    /// Handle value
    pub handle: u32,
    
    /// Generation counter
    pub generation: u32,
    
    /// Associated resource ID
    pub resource_id: ResourceId,
    
    /// Owning component
    pub owner: ComponentId,
    
    /// Type name for debugging
    pub type_name: BoundedString<64>,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Number of active borrows
    pub active_borrows: u32,
    
    /// Whether this handle has been dropped
    pub dropped: bool,
}

/// Entry for a borrowed handle
#[derive(Debug, Clone)]
pub struct BorrowedHandleEntry {
    /// Borrow ID
    pub borrow_id: BorrowId,
    
    /// Source owned handle
    pub source_handle: u32,
    
    /// Source generation
    pub source_generation: u32,
    
    /// Borrowed handle value
    pub borrowed_handle: u32,
    
    /// Borrow generation
    pub borrow_generation: u32,
    
    /// Lifetime scope
    pub scope: LifetimeScope,
    
    /// Borrowing component
    pub borrower: ComponentId,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Whether this borrow is still valid
    pub valid: bool,
}

/// Entry for a lifetime scope
#[derive(Debug, Clone)]
pub struct LifetimeScopeEntry {
    /// Scope ID
    pub scope: LifetimeScope,
    
    /// Parent scope
    pub parent: Option<LifetimeScope>,
    
    /// Owning component
    pub component: ComponentId,
    
    /// Task that created this scope
    pub task: TaskId,
    
    /// Borrows created in this scope
    #[cfg(feature = "std")]
    pub borrows: Vec<BorrowId>,
    #[cfg(not(any(feature = "std", )))]
    pub borrows: BoundedVec<BorrowId, MAX_BORROWED_HANDLES>,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Whether this scope is still active
    pub active: bool,
}

/// Statistics for handle lifetime tracking
#[derive(Debug, Clone)]
pub struct LifetimeStats {
    /// Total owned handles created
    pub owned_created: u64,
    
    /// Total owned handles dropped
    pub owned_dropped: u64,
    
    /// Total borrowed handles created
    pub borrowed_created: u64,
    
    /// Total borrowed handles invalidated
    pub borrowed_invalidated: u64,
    
    /// Current active owned handles
    pub active_owned: u32,
    
    /// Current active borrowed handles
    pub active_borrowed: u32,
    
    /// Current active scopes
    pub active_scopes: u32,
    
    /// Borrow validation failures
    pub validation_failures: u64,
}

/// Borrow validation result
#[derive(Debug, Clone)]
pub enum BorrowValidation {
    /// Borrow is valid
    Valid,
    
    /// Source handle no longer exists
    SourceNotFound,
    
    /// Source handle has been dropped
    SourceDropped,
    
    /// Generation mismatch (stale handle)
    GenerationMismatch,
    
    /// Scope has ended
    ScopeEnded,
    
    /// Component permission denied
    PermissionDenied,
}

/// Handle conversion error
#[derive(Debug, Clone)]
pub enum HandleConversionError {
    /// Invalid handle value
    InvalidHandle,
    
    /// Type mismatch
    TypeMismatch,
    
    /// Handle has been dropped
    HandleDropped,
    
    /// Borrow validation failed
    BorrowValidationFailed(BorrowValidation),
}

impl<T> OwnHandle<T> {
    /// Create a new owned handle
    pub fn new(raw: u32, generation: u32) -> Self {
        Self {
            raw,
            generation,
            _phantom: PhantomData,
        }
    }
    
    /// Get the raw handle value
    pub fn raw(&self) -> u32 {
        self.raw
    }
    
    /// Get the generation
    pub fn generation(&self) -> u32 {
        self.generation
    }
    
    /// Convert to a Value for serialization
    pub fn to_value(&self) -> Value {
        Value::Own(self.raw)
    }
    
    /// Create from a Value
    pub fn from_value(value: &Value) -> core::result::Result<Self, HandleConversionError> {
        match value {
            Value::Own(handle) => Ok(Self::new(*handle, 0)), // Generation would be validated separately
            _ => Err(HandleConversionError::TypeMismatch),
        }
    }
}

impl<T> BorrowHandle<T> {
    /// Create a new borrowed handle
    pub fn new(raw: u32, generation: u32, borrow_id: BorrowId) -> Self {
        Self {
            raw,
            generation,
            borrow_id,
            _phantom: PhantomData,
        }
    }
    
    /// Get the raw handle value
    pub fn raw(&self) -> u32 {
        self.raw
    }
    
    /// Get the generation
    pub fn generation(&self) -> u32 {
        self.generation
    }
    
    /// Get the borrow ID
    pub fn borrow_id(&self) -> BorrowId {
        self.borrow_id
    }
    
    /// Convert to a Value for serialization
    pub fn to_value(&self) -> Value {
        Value::Borrow(self.raw)
    }
    
    /// Create from a Value
    pub fn from_value(value: &Value, borrow_id: BorrowId) -> core::result::Result<Self, HandleConversionError> {
        match value {
            Value::Borrow(handle) => Ok(Self::new(*handle, 0, borrow_id)), // Generation would be validated separately
            _ => Err(HandleConversionError::TypeMismatch),
        }
    }
}

impl HandleLifetimeTracker {
    /// Create new handle lifetime tracker
    pub fn new() -> Result<Self> {
        Self {
            #[cfg(feature = "std")]
            owned_handles: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            owned_handles: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            
            #[cfg(feature = "std")]
            borrowed_handles: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            borrowed_handles: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            
            #[cfg(feature = "std")]
            scope_stack: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            scope_stack: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            
            next_handle_id: AtomicU32::new(1),
            next_borrow_id: AtomicU64::new(1),
            next_scope_id: AtomicU32::new(1),
            stats: LifetimeStats::new(),
        })
    }
    
    /// Create a new owned handle
    pub fn create_owned_handle<T>(
        &mut self,
        resource_id: ResourceId,
        owner: ComponentId,
        type_name: &str,
    ) -> Result<OwnHandle<T>> {
        let handle = self.next_handle_id.fetch_add(1, Ordering::Relaxed;
        let generation = 1; // First generation
        
        let entry = OwnedHandleEntry {
            handle,
            generation,
            resource_id,
            owner,
            type_name: {
                let provider = safe_managed_alloc!(64, CrateId::Component)?;
                BoundedString::from_str_truncate(type_name).unwrap_or_else(|_| {
                    BoundedString::from_str_truncate("").unwrap_or_default()
                })
            },
            created_at: self.get_current_time(),
            active_borrows: 0,
            dropped: false,
        };
        
        self.owned_handles.push(entry).map_err(|_| {
            Error::resource_exhausted("Error occurred")
            )
        })?;
        
        self.stats.owned_created += 1;
        self.stats.active_owned += 1;
        
        Ok(OwnHandle::new(handle, generation)
    }
    
    /// Create a borrowed handle from an owned handle
    pub fn borrow_handle<T>(
        &mut self,
        source: &OwnHandle<T>,
        borrower: ComponentId,
        scope: LifetimeScope,
    ) -> Result<BorrowHandle<T>> {
        // Validate source handle
        let source_entry = self.find_owned_handle(source.raw)?;
        if source_entry.generation != source.generation {
            return Err(Error::runtime_execution_error("Error occurred")
            ;
        }
        
        if source_entry.dropped {
            return Err(Error::runtime_execution_error("Error occurred")
            ;
        }
        
        // Validate scope
        if !self.is_scope_active(scope) {
            return Err(Error::runtime_execution_error("Error occurred")
            ;
        }
        
        let borrow_id = BorrowId(self.next_borrow_id.fetch_add(1, Ordering::Relaxed;
        let borrowed_handle = self.next_handle_id.fetch_add(1, Ordering::Relaxed;
        let borrow_generation = 1;
        
        let entry = BorrowedHandleEntry {
            borrow_id,
            source_handle: source.raw,
            source_generation: source.generation,
            borrowed_handle,
            borrow_generation,
            scope,
            borrower,
            created_at: self.get_current_time(),
            valid: true,
        };
        
        self.borrowed_handles.push(entry).map_err(|_| {
            Error::resource_exhausted("Error occurred")
            )
        })?;
        
        // Update source handle
        let source_entry = self.find_owned_handle_mut(source.raw)?;
        source_entry.active_borrows += 1;
        
        // Add to scope
        self.add_borrow_to_scope(scope, borrow_id)?;
        
        self.stats.borrowed_created += 1;
        self.stats.active_borrowed += 1;
        
        Ok(BorrowHandle::new(borrowed_handle, borrow_generation, borrow_id)
    }
    
    /// Drop an owned handle
    pub fn drop_owned_handle<T>(&mut self, handle: &OwnHandle<T>) -> Result<()> {
        let entry = self.find_owned_handle_mut(handle.raw)?;
        
        if entry.generation != handle.generation {
            return Err(Error::runtime_execution_error("Error occurred")
            ;
        }
        
        if entry.dropped {
            return Ok(()); // Already dropped
        }
        
        // Invalidate all borrows
        for borrowed in &mut self.borrowed_handles {
            if borrowed.source_handle == handle.raw && borrowed.valid {
                borrowed.valid = false;
                self.stats.borrowed_invalidated += 1;
                self.stats.active_borrowed -= 1;
            }
        }
        
        entry.dropped = true;
        self.stats.owned_dropped += 1;
        self.stats.active_owned -= 1;
        
        Ok(()
    }
    
    /// Validate a borrowed handle
    pub fn validate_borrow<T>(&self, handle: &BorrowHandle<T>) -> BorrowValidation {
        // Find borrow entry
        let borrow_entry = match self.find_borrowed_handle(handle.borrow_id) {
            Ok(entry) => entry,
            Err(_) => return BorrowValidation::SourceNotFound,
        };
        
        // Check if borrow is valid
        if !borrow_entry.valid {
            return BorrowValidation::SourceDropped;
        }
        
        // Check generation
        if borrow_entry.borrow_generation != handle.generation {
            return BorrowValidation::GenerationMismatch;
        }
        
        // Check scope
        if !self.is_scope_active(borrow_entry.scope) {
            return BorrowValidation::ScopeEnded;
        }
        
        // Check source handle
        let source_entry = match self.find_owned_handle(borrow_entry.source_handle) {
            Ok(entry) => entry,
            Err(_) => return BorrowValidation::SourceNotFound,
        };
        
        if source_entry.dropped {
            return BorrowValidation::SourceDropped;
        }
        
        if source_entry.generation != borrow_entry.source_generation {
            return BorrowValidation::GenerationMismatch;
        }
        
        BorrowValidation::Valid
    }
    
    /// Create a new lifetime scope
    pub fn create_scope(&mut self, component: ComponentId, task: TaskId) -> Result<LifetimeScope> {
        let scope = LifetimeScope(self.next_scope_id.fetch_add(1, Ordering::Relaxed;
        
        let parent = self.scope_stack.last().map(|entry| entry.scope);
        
        let entry = LifetimeScopeEntry {
            scope,
            parent,
            component,
            task,
            #[cfg(feature = "std")]
            borrows: Vec::new(),
            #[cfg(not(any(feature = "std", )))]
            borrows: {
                let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                BoundedVec::new().unwrap()
            },
            created_at: self.get_current_time(),
            active: true,
        };
        
        self.scope_stack.push(entry).map_err(|_| {
            Error::resource_exhausted("Error occurred")
            )
        })?;
        
        self.stats.active_scopes += 1;
        
        Ok(scope)
    }
    
    /// End a lifetime scope and invalidate all borrows in it
    pub fn end_scope(&mut self, scope: LifetimeScope) -> Result<()> {
        // Find scope entry
        let scope_index = self.scope_stack
            .iter()
            .position(|entry| entry.scope == scope)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;
            })?;
        
        // Get borrows to invalidate
        let scope_entry = &self.scope_stack[scope_index];
        let borrows_to_invalidate = scope_entry.borrows.clone();
        
        // Invalidate all borrows in this scope
        for borrow_id in borrows_to_invalidate {
            if let Ok(borrow_entry) = self.find_borrowed_handle_mut(borrow_id) {
                if borrow_entry.valid {
                    borrow_entry.valid = false;
                    self.stats.borrowed_invalidated += 1;
                    self.stats.active_borrowed -= 1;
                    
                    // Update source handle borrow count
                    if let Ok(source_entry) = self.find_owned_handle_mut(borrow_entry.source_handle) {
                        source_entry.active_borrows = source_entry.active_borrows.saturating_sub(1;
                    }
                }
            }
        }
        
        // Mark scope as inactive
        self.scope_stack[scope_index].active = false;
        
        // Remove scope from stack if it's the top scope
        if scope_index == self.scope_stack.len() - 1 {
            self.scope_stack.remove(scope_index;
            self.stats.active_scopes -= 1;
        }
        
        Ok(()
    }
    
    /// Get current statistics
    pub fn get_stats(&self) -> &LifetimeStats {
        &self.stats
    }
    
    /// Clean up invalid handles and scopes
    pub fn cleanup(&mut self) -> Result<()> {
        // Remove invalid borrowed handles
        #[cfg(feature = "std")]
        {
            self.borrowed_handles.retain(|entry| entry.valid;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let mut i = 0;
            while i < self.borrowed_handles.len() {
                if !self.borrowed_handles[i].valid {
                    self.borrowed_handles.remove(i;
                } else {
                    i += 1;
                }
            }
        }
        
        // Remove inactive scopes
        #[cfg(feature = "std")]
        {
            self.scope_stack.retain(|entry| entry.active;
        }
        #[cfg(not(any(feature = "std", )))]
        {
            let mut i = 0;
            while i < self.scope_stack.len() {
                if !self.scope_stack[i].active {
                    self.scope_stack.remove(i;
                } else {
                    i += 1;
                }
            }
        }
        
        Ok(()
    }
    
    // Private helper methods
    
    fn find_owned_handle(&self, handle: u32) -> Result<&OwnedHandleEntry> {
        self.owned_handles
            .iter()
            .find(|entry| entry.handle == handle)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;
            })
    }
    
    fn find_owned_handle_mut(&mut self, handle: u32) -> Result<&mut OwnedHandleEntry> {
        self.owned_handles
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;
            })
    }
    
    fn find_borrowed_handle(&self, borrow_id: BorrowId) -> Result<&BorrowedHandleEntry> {
        self.borrowed_handles
            .iter()
            .find(|entry| entry.borrow_id == borrow_id)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;
            })
    }
    
    fn find_borrowed_handle_mut(&mut self, borrow_id: BorrowId) -> Result<&mut BorrowedHandleEntry> {
        self.borrowed_handles
            .iter_mut()
            .find(|entry| entry.borrow_id == borrow_id)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;
            })
    }
    
    fn is_scope_active(&self, scope: LifetimeScope) -> bool {
        self.scope_stack
            .iter()
            .any(|entry| entry.scope == scope && entry.active)
    }
    
    fn add_borrow_to_scope(&mut self, scope: LifetimeScope, borrow_id: BorrowId) -> Result<()> {
        let scope_entry = self.scope_stack
            .iter_mut()
            .find(|entry| entry.scope == scope && entry.active)
            .ok_or_else(|| {
                Error::runtime_execution_error("Error occurred")
            })?;
            })?;
        
        scope_entry.borrows.push(borrow_id).map_err(|_| {
            Error::resource_exhausted("Error occurred")
            )
        })?;
        
        Ok(()
    }
    
    fn get_current_time(&self) -> u64 {
        // Simplified time implementation
        0
    }
}

impl LifetimeStats {
    /// Create new lifetime statistics
    pub fn new() -> Self {
        Self {
            owned_created: 0,
            owned_dropped: 0,
            borrowed_created: 0,
            borrowed_invalidated: 0,
            active_owned: 0,
            active_borrowed: 0,
            active_scopes: 0,
            validation_failures: 0,
        }
    }
}

impl Default for HandleLifetimeTracker {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback for Default trait - this should only be used in non-critical paths
            panic!("Failed to create HandleLifetimeTracker with default memory allocation")
        })
    }
}

impl Default for LifetimeStats {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Display for OwnHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "own<{}>({}:{})", 
               core::any::type_name::<T>(), 
               self.raw, 
               self.generation)
    }
}

impl<T> fmt::Display for BorrowHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "borrow<{}>({}:{}, borrow:{})", 
               core::any::type_name::<T>(), 
               self.raw, 
               self.generation,
               self.borrow_id.0)
    }
}

impl fmt::Display for BorrowValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BorrowValidation::Valid => write!(f, "valid"),
            BorrowValidation::SourceNotFound => write!(f, "source not found"),
            BorrowValidation::SourceDropped => write!(f, "source dropped"),
            BorrowValidation::GenerationMismatch => write!(f, "generation mismatch"),
            BorrowValidation::ScopeEnded => write!(f, "scope ended"),
            BorrowValidation::PermissionDenied => write!(f, "permission denied"),
        }
    }
}

/// Convenience function to create a lifetime scope
pub fn with_lifetime_scope<F, R>(
    tracker: &mut HandleLifetimeTracker,
    component: ComponentId,
    task: TaskId,
    f: F,
) -> Result<R>
where
    F: FnOnce(LifetimeScope) -> Result<R>,
{
    let scope = tracker.create_scope(component, task)?;
    let result = f(scope;
    let _ = tracker.end_scope(scope;
    result
}

}
