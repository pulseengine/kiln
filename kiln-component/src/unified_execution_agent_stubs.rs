//! Minimal stubs for unified execution engine compilation
//! These stubs allow the unified engine to compile without all dependencies

// For no_std, override prelude's bounded::BoundedVec with StaticVec
#[cfg(not(feature = "std"))]
use wrt_foundation::collections::StaticVec as BoundedVec;

use wrt_foundation::{
    bounded::BoundedString, budget_aware_provider::CrateId, prelude::*, safe_managed_alloc,
};

// Import BoundedVec only for std - no_std uses StaticVec alias above
#[cfg(feature = "std")]
use wrt_foundation::bounded::BoundedVec;

use crate::{bounded_component_infra::ComponentProvider, prelude::WrtComponentValue, types::Value};

/// Canonical ABI processor stub
#[derive(Debug, Default, Clone)]
pub struct CanonicalAbi;

impl CanonicalAbi {
    pub fn new() -> Self {
        Self
    }
}

/// Canonical options stub
#[derive(Debug, Default, Clone)]
pub struct CanonicalOptions;

/// Resource handle stub
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceHandle(pub u32);

/// Resource lifecycle manager stub
#[derive(Debug, Clone)]
pub struct ResourceLifecycleManager {
    next_handle: u32,
}

impl Default for ResourceLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceLifecycleManager {
    pub fn new() -> Self {
        Self { next_handle: 1 }
    }

    pub fn create_resource(
        &mut self,
        _type_id: u32,
        _data: WrtComponentValue<ComponentProvider>,
    ) -> wrt_error::Result<ResourceHandle> {
        let handle = ResourceHandle(self.next_handle);
        self.next_handle += 1;
        Ok(handle)
    }

    pub fn drop_resource(&mut self, _handle: ResourceHandle) -> wrt_error::Result<()> {
        Ok(())
    }

    pub fn borrow_resource(
        &mut self,
        _handle: ResourceHandle,
    ) -> wrt_error::Result<&WrtComponentValue<ComponentProvider>> {
        // Return a dummy value - in real implementation this would be tracked
        static DUMMY: WrtComponentValue<ComponentProvider> = WrtComponentValue::Bool(false);
        Ok(&DUMMY)
    }

    pub fn transfer_ownership(
        &mut self,
        _handle: ResourceHandle,
        _new_owner: u32,
    ) -> wrt_error::Result<()> {
        Ok(())
    }
}

/// Runtime bridge configuration stub
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuntimeBridgeConfig;

/// Component runtime bridge stub
#[derive(Debug, Clone)]
pub struct ComponentRuntimeBridge;

impl Default for ComponentRuntimeBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentRuntimeBridge {
    pub fn new() -> Self {
        Self
    }

    pub fn with_config(_config: RuntimeBridgeConfig) -> Self {
        Self
    }

    pub fn execute_component_function(
        &mut self,
        _instance_id: u32,
        _function_name: &str,
        _args: &[WrtComponentValue<ComponentProvider>],
    ) -> core::result::Result<WrtComponentValue<ComponentProvider>, wrt_error::Error> {
        // Return a dummy successful result
        Ok(WrtComponentValue::<ComponentProvider>::U32(42))
    }

    pub fn register_component_instance(
        &mut self,
        _component_id: u32,
        _module_name: String,
        _function_count: u32,
        _memory_size: u32,
    ) -> core::result::Result<u32, wrt_error::Error> {
        Ok(1)
    }

    #[cfg(feature = "std")]
    pub fn register_host_function<F>(
        &mut self,
        _name: String,
        _signature: crate::component_instantiation::FunctionSignature,
        _func: F,
    ) -> core::result::Result<usize, wrt_error::Error>
    where
        F: Fn(
                &[WrtComponentValue<ComponentProvider>],
            )
                -> core::result::Result<WrtComponentValue<ComponentProvider>, wrt_error::Error>
            + Send
            + Sync
            + 'static,
    {
        Ok(0)
    }

    #[cfg(not(feature = "std"))]
    pub fn register_host_function(
        &mut self,
        _name: BoundedString<64>,
        _signature: crate::component_instantiation::FunctionSignature,
        _func: fn(
            &[WrtComponentValue<ComponentProvider>],
        )
            -> core::result::Result<WrtComponentValue<ComponentProvider>, wrt_error::Error>,
    ) -> core::result::Result<usize, wrt_error::Error> {
        Ok(0)
    }
}

/// Component value conversion stubs
impl From<WrtComponentValue<ComponentProvider>> for Value {
    fn from(cv: WrtComponentValue<ComponentProvider>) -> Self {
        match cv {
            WrtComponentValue::Bool(b) => Value::Bool(b),
            WrtComponentValue::U8(v) => Value::U8(v),
            WrtComponentValue::U16(v) => Value::U16(v),
            WrtComponentValue::U32(v) => Value::U32(v),
            WrtComponentValue::U64(v) => Value::U64(v),
            WrtComponentValue::S8(v) => Value::S8(v),
            WrtComponentValue::S16(v) => Value::S16(v),
            WrtComponentValue::S32(v) => Value::S32(v),
            WrtComponentValue::S64(v) => Value::S64(v),
            WrtComponentValue::F32(v) => Value::F32(v.to_f32()),
            WrtComponentValue::F64(v) => Value::F64(v.to_f64()),
            WrtComponentValue::Char(c) => Value::Char(c),
            #[cfg(feature = "std")]
            WrtComponentValue::String(s) => {
                let provider = safe_managed_alloc!(2048, CrateId::Component)
                    .unwrap_or_else(|_| NoStdProvider::default());
                let bounded_str = wrt_foundation::bounded::BoundedString::try_from_str(&s)
                    .unwrap_or_else(|_| panic!("Failed to convert string"));
                Value::String(bounded_str)
            },
            #[cfg(not(any(feature = "std",)))]
            WrtComponentValue::String(s) => {
                let _provider = safe_managed_alloc!(2048, CrateId::Component)
                    .unwrap_or_else(|_| NoStdProvider::default());
                let bounded_str = wrt_foundation::bounded::BoundedString::try_from_str(s.as_str())
                    .unwrap_or_else(|_| panic!("Failed to convert string"));
                Value::String(bounded_str)
            },
            _ => Value::Bool(false), // Fallback
        }
    }
}

impl From<Value> for WrtComponentValue<ComponentProvider> {
    fn from(v: Value) -> Self {
        match v {
            Value::Bool(b) => WrtComponentValue::Bool(b),
            Value::U8(v) => WrtComponentValue::U8(v),
            Value::U16(v) => WrtComponentValue::U16(v),
            Value::U32(v) => WrtComponentValue::U32(v),
            Value::U64(v) => WrtComponentValue::U64(v),
            Value::S8(v) => WrtComponentValue::S8(v),
            Value::S16(v) => WrtComponentValue::S16(v),
            Value::S32(v) => WrtComponentValue::S32(v),
            Value::S64(v) => WrtComponentValue::S64(v),
            Value::F32(v) => WrtComponentValue::F32(wrt_foundation::FloatBits32::from_f32(v)),
            Value::F64(v) => WrtComponentValue::F64(wrt_foundation::FloatBits64::from_f64(v)),
            Value::Char(c) => WrtComponentValue::Char(c),
            #[cfg(feature = "std")]
            Value::String(s) => {
                let string =
                    s.as_str().unwrap_or_else(|_| panic!("Failed to get string slice")).to_string();
                WrtComponentValue::String(string)
            },
            #[cfg(not(any(feature = "std",)))]
            Value::String(s) => {
                match s.as_str() {
                    Ok(str_ref) => {
                        // Convert BoundedString to String
                        WrtComponentValue::String(str_ref.into())
                    },
                    Err(_) => WrtComponentValue::Bool(false), // Fallback on error
                }
            },
            _ => WrtComponentValue::Bool(false), // Fallback
        }
    }
}

#[cfg(feature = "async")]
pub mod async_stubs {
    use super::*;

    /// Async read result stub
    #[derive(Debug, Clone)]
    pub enum AsyncReadResult {
        Ready(Vec<u8>),
        Pending,
        Error(String),
    }

    /// Future handle stub
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FutureHandle(pub u32);

    impl FutureHandle {
        /// Create a new future handle
        pub const fn new(id: u32) -> Self {
            Self(id)
        }

        /// Extract the inner value
        pub const fn into_inner(self) -> u32 {
            self.0
        }
    }

    /// Future state stub
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FutureState {
        Pending,
        Ready,
        Error,
    }

    /// Stream handle stub
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct StreamHandle(pub u32);

    impl StreamHandle {
        /// Create a new stream handle
        pub const fn new(id: u32) -> Self {
            Self(id)
        }

        /// Extract the inner value
        pub const fn into_inner(self) -> u32 {
            self.0
        }
    }

    /// Stream state stub
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum StreamState {
        Open,
        Closed,
        Error,
    }

    /// Component future stub
    #[derive(Debug)]
    pub struct Future;

    /// Stream stub
    #[derive(Debug)]
    pub struct Stream;
}

#[cfg(feature = "async")]
pub use async_stubs::*;

#[cfg(feature = "cfi")]
pub mod cfi_stubs {
    use super::*;

    /// CFI control flow operations stub
    #[derive(Debug, Default)]
    pub struct DefaultCfiControlFlowOps;

    /// CFI control flow protection stub
    #[derive(Debug, Default, Clone)]
    pub struct CfiControlFlowProtection {
        pub software_config: CfiSoftwareConfig,
    }

    /// CFI software config stub
    #[derive(Debug, Default, Clone)]
    pub struct CfiSoftwareConfig {
        pub max_shadow_stack_depth: usize,
    }

    impl Default for CfiSoftwareConfig {
        fn default() -> Self {
            Self {
                max_shadow_stack_depth: 1024,
            }
        }
    }

    /// CFI execution context stub
    #[derive(Debug, Clone)]
    pub struct CfiExecutionContext {
        pub current_function: u32,
        pub current_instruction: u32,
        pub shadow_stack: BoundedVec<u32, 1024>,
        pub violation_count: u32,
        pub landing_pad_expectations: BoundedVec<LandingPadExpectation, 16>,
        pub metrics: CfiMetrics,
    }

    /// Landing pad expectation stub
    #[derive(Debug, Clone)]
    pub struct LandingPadExpectation {
        pub function_index: u32,
        pub instruction_offset: u32,
        pub deadline: Option<u64>,
    }

    /// CFI metrics stub
    #[derive(Debug, Default, Clone)]
    pub struct CfiMetrics {
        pub landing_pads_validated: u64,
        pub shadow_stack_operations: u64,
    }

    /// CFI protected branch target stub
    #[derive(Debug, Clone)]
    pub struct CfiProtectedBranchTarget {
        pub target: u32,
        pub protection: CfiProtection,
    }

    /// CFI protection stub
    #[derive(Debug, Default, Clone)]
    pub struct CfiProtection {
        pub landing_pad: Option<CfiLandingPad>,
    }

    /// CFI landing pad stub
    #[derive(Debug, Clone)]
    pub struct CfiLandingPad {
        pub label: u32,
    }

    impl DefaultCfiControlFlowOps {
        pub fn call_indirect_with_cfi(
            &mut self,
            _type_idx: u32,
            _table_idx: u32,
            _protection: &CfiControlFlowProtection,
            _context: &mut CfiExecutionContext,
        ) -> core::result::Result<CfiProtectedBranchTarget, wrt_error::Error> {
            Ok(CfiProtectedBranchTarget {
                target: 0,
                protection: CfiProtection::default(),
            })
        }

        pub fn return_with_cfi(
            &mut self,
            _protection: &CfiControlFlowProtection,
            _context: &mut CfiExecutionContext,
        ) -> core::result::Result<(), wrt_error::Error> {
            Ok(())
        }

        pub fn branch_with_cfi(
            &mut self,
            _label_idx: u32,
            _conditional: bool,
            _protection: &CfiControlFlowProtection,
            _context: &mut CfiExecutionContext,
        ) -> core::result::Result<CfiProtectedBranchTarget, wrt_error::Error> {
            Ok(CfiProtectedBranchTarget {
                target: _label_idx,
                protection: CfiProtection::default(),
            })
        }
    }

    impl CfiExecutionContext {
        pub fn new() -> wrt_error::Result<Self> {
            Ok(Self {
                current_function: 0,
                current_instruction: 0,
                shadow_stack: {
                    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                    BoundedVec::new().unwrap()
                },
                violation_count: 0,
                landing_pad_expectations: {
                    let provider = safe_managed_alloc!(65536, CrateId::Component)?;
                    BoundedVec::new().unwrap()
                },
                metrics: CfiMetrics::default(),
            })
        }
    }
}

#[cfg(feature = "cfi")]
pub use cfi_stubs::*;
