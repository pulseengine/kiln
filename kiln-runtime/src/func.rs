//! WebAssembly function type implementation
//!
//! This module provides the implementation for WebAssembly function types.

use kiln_foundation::{
    budget_aware_provider::CrateId,
    safe_managed_alloc,
};

use crate::prelude::Debug;
use crate::prelude::RuntimeFuncType;

/// Placeholder Function type for runtime functions
#[derive(Debug, Clone)]
pub struct Function {
    /// Function type signature
    pub func_type:      RuntimeFuncType,
    /// Function body (placeholder)
    pub body: kiln_foundation::bounded::BoundedVec<
        u8,
        4096,
        kiln_foundation::safe_memory::NoStdProvider<8192>,
    >,
    /// Function index in the module (optional)
    pub function_index: Option<u32>,
}

impl Function {
    /// Create a new function
    pub fn new(func_type: RuntimeFuncType) -> Result<Self, kiln_error::Error> {
        let provider = safe_managed_alloc!(8192, CrateId::Runtime)?;
        Ok(Self {
            func_type,
            body: kiln_foundation::bounded::BoundedVec::new(provider)?,
            function_index: None,
        })
    }

    /// Create a new function with an index
    pub fn new_with_index(
        func_type: RuntimeFuncType,
        index: u32,
    ) -> Result<Self, kiln_error::Error> {
        let provider = safe_managed_alloc!(8192, CrateId::Runtime)?;
        Ok(Self {
            func_type,
            body: kiln_foundation::bounded::BoundedVec::new(provider)?,
            function_index: Some(index),
        })
    }

    /// Get the function index
    pub fn index(&self) -> Option<u32> {
        self.function_index
    }

    /// Set the function index
    pub fn set_index(&mut self, index: u32) {
        self.function_index = Some(index);
    }
}
