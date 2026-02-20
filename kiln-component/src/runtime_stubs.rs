// Runtime stubs for component module development
// These provide the interface to the runtime module's types

use crate::foundation_stubs::{LargeVec, MediumVec, SafetyContext, SmallVec};
use crate::platform_stubs::ComprehensivePlatformLimits;
use crate::prelude::{Box, Vec};

// Basic value type stub
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    V128(u128),
}

// Execution context stub
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub component_id: ComponentId,
    pub instance_id: InstanceId,
    pub safety_context: SafetyContext,
}

impl ExecutionContext {
    pub fn new(
        component_id: ComponentId,
        instance_id: InstanceId,
        safety_context: SafetyContext,
    ) -> Self {
        Self {
            component_id,
            instance_id,
            safety_context,
        }
    }
}

// Memory adapter stub
pub trait UnifiedMemoryAdapter: Send + Sync {
    fn allocate(&mut self, size: usize) -> core::result::Result<&mut [u8], wrt_error::Error>;
    fn deallocate(&mut self, ptr: &mut [u8]) -> core::result::Result<(), wrt_error::Error>;
    fn available_memory(&self) -> usize;
    fn total_memory(&self) -> usize;
}

pub struct GenericMemoryAdapter {
    total_memory: usize,
    allocated: usize,
}

impl GenericMemoryAdapter {
    pub fn new(total_memory: usize) -> core::result::Result<Self, wrt_error::Error> {
        Ok(Self {
            total_memory,
            allocated: 0,
        })
    }
}

impl UnifiedMemoryAdapter for GenericMemoryAdapter {
    fn allocate(&mut self, size: usize) -> core::result::Result<&mut [u8], wrt_error::Error> {
        if self.allocated + size > self.total_memory {
            return Err(wrt_error::Error::resource_exhausted("Out of memory"));
        }
        self.allocated += size;
        // This is a stub - real implementation would return actual memory
        Err(wrt_error::Error::runtime_error("Memory allocation stub"))
    }

    fn deallocate(&mut self, _ptr: &mut [u8]) -> core::result::Result<(), wrt_error::Error> {
        Ok(())
    }

    fn available_memory(&self) -> usize {
        self.total_memory - self.allocated
    }

    fn total_memory(&self) -> usize {
        self.total_memory
    }
}

// Function and execution stubs
#[derive(Debug, Clone)]
pub struct Function {
    pub id: FunctionId,
    pub signature: FunctionSignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub params: SmallVec<ValueType>,
    pub results: SmallVec<ValueType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    I32,
    I64,
    F32,
    F64,
    V128,
}

// Component and instance identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u32);

// CFI engine stub
pub struct CfiEngine {
    validation_enabled: bool,
}

impl CfiEngine {
    pub fn new(_limits: &ExecutionLimits) -> core::result::Result<Self, wrt_error::Error> {
        Ok(Self {
            validation_enabled: true,
        })
    }

    pub fn validate_call(
        &self,
        _function: &Function,
    ) -> core::result::Result<(), wrt_error::Error> {
        if self.validation_enabled {
            // Stub validation always passes
            Ok(())
        } else {
            Ok(())
        }
    }
}

// Execution limits stub
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    pub max_stack_depth: usize,
    pub max_value_stack: usize,
    pub max_locals: usize,
    pub max_function_calls: usize,
}

impl ExecutionLimits {
    pub fn from_platform(platform_limits: &ComprehensivePlatformLimits) -> Self {
        Self {
            max_stack_depth: platform_limits.max_stack_bytes / 1024, // Rough estimate
            max_value_stack: 10000,
            max_locals: 1000,
            max_function_calls: 10000,
        }
    }
}

// Execution engine stub
pub struct ExecutionEngine {
    limits: ExecutionLimits,
    value_stack: LargeVec<Value>,
    call_stack: MediumVec<CallFrame>,
    locals: SmallVec<Value>,
    cfi_engine: CfiEngine,
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_id: FunctionId,
    pub locals_start: usize,
    pub locals_count: usize,
}

impl ExecutionEngine {
    pub fn new(
        platform_limits: &ComprehensivePlatformLimits,
    ) -> core::result::Result<Self, wrt_error::Error> {
        let limits = ExecutionLimits::from_platform(platform_limits);
        let cfi_engine = CfiEngine::new(&limits)?;

        Ok(Self {
            limits,
            value_stack: LargeVec::new(),
            call_stack: MediumVec::new(),
            locals: SmallVec::new(),
            cfi_engine,
        })
    }

    pub fn execute_function(
        &mut self,
        function: &Function,
        args: &[Value],
    ) -> core::result::Result<Vec<Value>, wrt_error::Error> {
        // Validate execution against limits
        if self.call_stack.len() >= self.limits.max_stack_depth {
            return Err(wrt_error::Error::runtime_stack_overflow(
                "Call stack depth exceeded",
            ));
        }

        // CFI validation
        self.cfi_engine.validate_call(function)?;

        // Stub execution - just return empty result
        Ok(Vec::new())
    }
}

// WASM configuration stub
#[derive(Debug, Clone)]
pub struct WasmConfiguration {
    pub memory_limits: MemoryLimits,
    pub execution_limits: ExecutionLimits,
}

#[derive(Debug, Clone)]
pub struct MemoryLimits {
    pub max_memory: usize,
    pub max_pages: u32,
}

impl WasmConfiguration {
    pub fn new(platform_limits: &ComprehensivePlatformLimits) -> Self {
        Self {
            memory_limits: MemoryLimits {
                max_memory: platform_limits.max_wasm_linear_memory,
                max_pages: (platform_limits.max_wasm_linear_memory / 65536) as u32,
            },
            execution_limits: ExecutionLimits::from_platform(platform_limits),
        }
    }
}
