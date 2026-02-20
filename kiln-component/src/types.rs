//! Types for the WebAssembly Component Model
//!
//! This module provides component model type definitions.

#[cfg(all(feature = "std", feature = "safety-critical"))]
use wrt_foundation::allocator::{CrateId, WrtVec};
use wrt_foundation::{
    bounded::BoundedString,
    collections::StaticVec,
    traits::{Checksummable, FromBytes, ToBytes},
};

/// Type alias for backward compatibility - BoundedVec is now StaticVec
type BoundedVec<T, const N: usize> = StaticVec<T, N>;

#[cfg(feature = "component-model-async")]
use crate::async_::async_types::{FutureHandle, StreamHandle};
use crate::{
    components::{component::Component, component_instantiation::ComponentMemory},
    instantiation::{ModuleInstance, ResolvedExport, ResolvedImport, ResourceTable},
    prelude::*,
};

// Fallback types when async features are not enabled
#[cfg(not(feature = "component-model-async"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamHandle(pub u32);

#[cfg(not(feature = "component-model-async"))]
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

#[cfg(not(feature = "component-model-async"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FutureHandle(pub u32);

#[cfg(not(feature = "component-model-async"))]
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

// Fallback TaskId when threading features are not enabled
#[cfg(not(feature = "component-model-threading"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub u32);

#[cfg(not(feature = "component-model-threading"))]
impl TaskId {
    /// Create a new task identifier
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }
}

/// Metadata for component instance tracking
#[derive(Debug, Clone, Default)]
pub struct ComponentMetadata {
    /// Number of function calls made
    pub function_calls: u64,
    /// Creation timestamp (in microseconds since some epoch)
    pub created_at: u64,
    /// Last access timestamp (in microseconds)
    pub last_accessed: u64,
}

/// Canonical ComponentInstance definition for ASIL-D type safety
///
/// This is the single source of truth for ComponentInstance across the codebase
/// to ensure type consistency and prevent safety violations.
///
/// SW-REQ-ID: REQ_TYPE_001 - Unified type definitions
/// SW-REQ-ID: REQ_SAFETY_002 - Type safety enforcement
pub struct ComponentInstance {
    /// Unique instance ID
    pub id: u32,
    /// Reference to the component definition
    pub component: Component,
    /// Current instance state
    pub state: ComponentInstanceState,
    /// Resource manager for this instance
    pub resource_manager: Option<crate::resource_management::ResourceManager>,
    /// Instance memory (if allocated)
    pub memory: Option<ComponentMemory>,
    /// Instance metadata for tracking
    pub metadata: ComponentMetadata,
    /// Type index for runtime type resolution (Step 2)
    #[cfg(feature = "std")]
    pub type_index: std::collections::HashMap<u32, wrt_format::component::ComponentType>,
    #[cfg(not(feature = "std"))]
    pub type_index: (), // Placeholder for no_std
    /// Function table for this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub functions: WrtVec<
        crate::components::component_instantiation::ComponentFunction,
        { CrateId::Component as u8 },
        128,
    >,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub functions: Vec<crate::components::component_instantiation::ComponentFunction>,
    #[cfg(not(any(feature = "std",)))]
    pub functions: BoundedVec<crate::components::component_instantiation::ComponentFunction, 128>,
    /// Resolved imports for this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub imports: WrtVec<ResolvedImport, { CrateId::Component as u8 }, 256>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub imports: Vec<ResolvedImport>,
    #[cfg(not(any(feature = "std",)))]
    pub imports: BoundedVec<ResolvedImport, 256>,
    /// Resolved exports from this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub exports: WrtVec<ResolvedExport, { CrateId::Component as u8 }, 256>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub exports: Vec<ResolvedExport>,
    #[cfg(not(any(feature = "std",)))]
    pub exports: BoundedVec<ResolvedExport, 256>,
    /// Resource tables for this instance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub resource_tables: WrtVec<ResourceTable, { CrateId::Component as u8 }, 16>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub resource_tables: Vec<ResourceTable>,
    #[cfg(not(any(feature = "std",)))]
    pub resource_tables: BoundedVec<ResourceTable, 16>,
    /// Module instances embedded in this component
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub module_instances: WrtVec<ModuleInstance, { CrateId::Component as u8 }, 64>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub module_instances: Vec<ModuleInstance>,
    #[cfg(not(any(feature = "std",)))]
    pub module_instances: BoundedVec<ModuleInstance, 64>,
    /// Nested component instances (child components instantiated within this component)
    /// Maps from component instance index to the instantiated ComponentInstance
    #[cfg(all(feature = "std", feature = "safety-critical"))]
    pub nested_component_instances:
        WrtVec<NestedComponentInstance, { CrateId::Component as u8 }, 16>,
    #[cfg(all(feature = "std", not(feature = "safety-critical")))]
    pub nested_component_instances: Vec<NestedComponentInstance>,
    #[cfg(not(any(feature = "std",)))]
    pub nested_component_instances: BoundedVec<NestedComponentInstance, 16>,
    /// Runtime engine for executing WASM functions (if available)
    #[cfg(feature = "wrt-execution")]
    pub runtime_engine: Option<Box<wrt_runtime::engine::CapabilityAwareEngine>>,
    /// Main module instance handle for execution
    #[cfg(feature = "wrt-execution")]
    pub main_instance_handle: Option<wrt_runtime::engine::InstanceHandle>,
}

impl fmt::Debug for ComponentInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentInstance")
            .field("id", &self.id)
            .field("component", &self.component)
            .field("state", &self.state)
            .field("resource_manager", &self.resource_manager)
            .field("memory", &self.memory)
            .field("metadata", &self.metadata)
            .field("functions", &self.functions.len())
            .field("imports", &self.imports.len())
            .field("exports", &self.exports.len())
            .field("resource_tables", &self.resource_tables.len())
            .field("module_instances", &self.module_instances.len())
            .field(
                "nested_component_instances",
                &self.nested_component_instances.len(),
            )
            .finish()
    }
}

impl ComponentInstance {
    /// Get a core WebAssembly module instance by index
    ///
    /// Returns the module instance at the given index, or None if the index is out of bounds.
    ///
    /// # Arguments
    /// * `index` - The index of the module instance to retrieve
    ///
    /// # Returns
    /// An optional reference to the module instance
    pub fn get_core_module_instance(&self, index: usize) -> Option<&ModuleInstance> {
        self.module_instances.get(index)
    }

    /// Get a nested component instance by its instance index
    ///
    /// Returns the nested component instance at the given index, or None if not found.
    ///
    /// # Arguments
    /// * `instance_index` - The instance index within this component's instance space
    ///
    /// # Returns
    /// An optional reference to the nested component instance
    #[cfg(feature = "std")]
    pub fn get_nested_component_instance(
        &self,
        instance_index: u32,
    ) -> Option<&NestedComponentInstance> {
        self.nested_component_instances
            .iter()
            .find(|n| n.instance_index == instance_index)
    }

    /// Get a mutable reference to a nested component instance by its instance index
    #[cfg(feature = "std")]
    pub fn get_nested_component_instance_mut(
        &mut self,
        instance_index: u32,
    ) -> Option<&mut NestedComponentInstance> {
        self.nested_component_instances
            .iter_mut()
            .find(|n| n.instance_index == instance_index)
    }

    /// Get the number of nested component instances
    pub fn nested_component_instance_count(&self) -> usize {
        self.nested_component_instances.len()
    }

    /// Set the host import handler for WASI and other host functions
    ///
    /// This should be called after creating the ComponentInstance to enable
    /// WASI support. The handler routes all host function calls through
    /// a unified dispatch mechanism.
    ///
    /// # Arguments
    /// * `handler` - The host import handler implementation (e.g., WasiDispatcher)
    ///
    /// # Example
    /// ```ignore
    /// let mut instance = ComponentInstance::from_parsed(...)?;
    /// let dispatcher = WasiDispatcher::with_defaults()?;
    /// instance.set_host_handler(Box::new(dispatcher));
    /// ```
    #[cfg(all(feature = "std", feature = "wrt-execution"))]
    pub fn set_host_handler(&mut self, handler: Box<dyn wrt_foundation::HostImportHandler>) {
        if let Some(ref mut engine) = self.runtime_engine {
            engine.set_host_handler(handler);
        }
    }

    /// Pre-allocate memory for WASI arguments using cabi_realloc
    ///
    /// This must be called after setting the host handler but before calling
    /// entry points that use `get-arguments` or `get-environment`. The canonical
    /// ABI requires pre-allocated memory for returning list types.
    ///
    /// # Returns
    /// Ok(()) if pre-allocation succeeded or no engine is available,
    /// Err if cabi_realloc fails
    #[cfg(all(feature = "std", feature = "wrt-execution", feature = "wasi"))]
    pub fn pre_allocate_wasi_args(&mut self) -> wrt_error::Result<()> {
        if let (Some(engine), Some(handle)) = (&mut self.runtime_engine, self.main_instance_handle)
        {
            engine.pre_allocate_wasi_args(handle)
        } else {
            // No engine or handle - nothing to pre-allocate
            Ok(())
        }
    }
}

/// State of a component instance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentInstanceState {
    /// Instance is initialized but not started
    Initialized,
    /// Instance is running
    Running,
    /// Instance is paused
    Paused,
    /// Instance has been stopped or exited
    Stopped,
    /// Instance encountered an error
    Error,
}

impl Default for ComponentInstanceState {
    fn default() -> Self {
        Self::Initialized
    }
}

/// A nested component instance within a parent component
///
/// This represents a component that was instantiated as part of another
/// component's instantiation process. The nested instance has its own
/// exports that can be aliased by the parent.
///
/// Note: This type intentionally does not implement Clone because
/// ComponentInstance contains runtime state that should not be duplicated.
#[derive(Debug)]
pub struct NestedComponentInstance {
    /// Index of this instance within the parent component's instance space
    pub instance_index: u32,
    /// Index of the component definition this instance was created from
    pub component_index: u32,
    /// The instantiated component instance (boxed to avoid recursive size issues)
    #[cfg(feature = "std")]
    pub instance: Box<ComponentInstance>,
    /// Placeholder for no_std (nested components not supported without std)
    #[cfg(not(feature = "std"))]
    pub instance: (),
    /// Exports provided by this nested instance that can be aliased by parent
    #[cfg(feature = "std")]
    pub exports: std::collections::HashMap<String, NestedExportRef>,
    #[cfg(not(feature = "std"))]
    pub exports: (),
}

/// Reference to an export from a nested component instance
#[derive(Debug, Clone)]
pub struct NestedExportRef {
    /// Kind of the export (function, value, type, instance, etc.)
    pub kind: NestedExportKind,
    /// Index within the nested instance's export space
    pub index: u32,
}

/// Kind of export from a nested component instance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedExportKind {
    /// Function export
    Function,
    /// Value export
    Value,
    /// Type export
    Type,
    /// Instance export
    Instance,
    /// Component export
    Component,
}

/// Component model value type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum ValType {
    /// Boolean type
    #[default]
    Bool,
    /// Signed 8-bit integer
    S8,
    /// Unsigned 8-bit integer  
    U8,
    /// Signed 16-bit integer
    S16,
    /// Unsigned 16-bit integer
    U16,
    /// Signed 32-bit integer
    S32,
    /// Unsigned 32-bit integer
    U32,
    /// Signed 64-bit integer
    S64,
    /// Unsigned 64-bit integer
    U64,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
    /// Character type
    Char,
    /// String type
    String,
    /// List type with element type
    List(Box<ValType>),
    /// Record type with named fields
    Record(Record),
    /// Tuple type with element types (boxed to break recursive cycle)
    Tuple(Box<Tuple>),
    /// Variant type with alternatives
    Variant(Variant),
    /// Enum type with cases
    Enum(Enum),
    /// Option type with payload
    Option(Box<ValType>),
    /// Result type with ok/error types
    Result(Result_),
    /// Flags type with bitfields
    Flags(Flags),
    /// Resource handle
    Own(u32),
    /// Borrowed resource
    Borrow(u32),
    /// Stream type with element type
    Stream(Box<ValType>),
    /// Future type with value type
    Future(Box<ValType>),
}

/// Record type definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Record {
    #[cfg(feature = "std")]
    pub fields: Vec<Field>,
    #[cfg(not(any(feature = "std",)))]
    pub fields: BoundedVec<Field, 64>,
}

/// Field in a record
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    #[cfg(feature = "std")]
    pub name: String,
    #[cfg(not(any(feature = "std",)))]
    pub name: BoundedString<64>,
    pub ty: Box<ValType>, // Boxed to break recursive type cycle
}

/// Tuple type definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tuple {
    #[cfg(feature = "std")]
    pub types: Vec<ValType>,
    #[cfg(not(any(feature = "std",)))]
    pub types: BoundedVec<ValType, 32>,
}

/// Variant type definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Variant {
    #[cfg(feature = "std")]
    pub cases: Vec<Case>,
    #[cfg(not(any(feature = "std",)))]
    pub cases: BoundedVec<Case, 64>,
}

/// Case in a variant
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Case {
    #[cfg(feature = "std")]
    pub name: String,
    #[cfg(not(any(feature = "std",)))]
    pub name: BoundedString<64>,
    pub ty: Option<Box<ValType>>, // Boxed to break recursive cycle
    pub refines: Option<u32>,
}

/// Enum type definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Enum {
    #[cfg(feature = "std")]
    pub cases: Vec<String>,
    #[cfg(not(any(feature = "std",)))]
    pub cases: BoundedVec<BoundedString<64>, 64>,
}

/// Result type definition (renamed to avoid conflict with std::result::Result)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Result_ {
    pub ok: Option<Box<ValType>>,
    pub err: Option<Box<ValType>>,
}

/// Flags type definition
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Flags {
    #[cfg(feature = "std")]
    pub labels: Vec<String>,
    #[cfg(not(any(feature = "std",)))]
    pub labels: BoundedVec<BoundedString<64>, 64>,
}

/// Component model value
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean value
    Bool(bool),
    /// Signed 8-bit integer
    S8(i8),
    /// Unsigned 8-bit integer
    U8(u8),
    /// Signed 16-bit integer
    S16(i16),
    /// Unsigned 16-bit integer
    U16(u16),
    /// Signed 32-bit integer
    S32(i32),
    /// Unsigned 32-bit integer
    U32(u32),
    /// Signed 64-bit integer
    S64(i64),
    /// Unsigned 64-bit integer
    U64(u64),
    /// 32-bit floating point
    F32(f32),
    /// 64-bit floating point
    F64(f64),
    /// Character value
    Char(char),
    /// String value
    String(BoundedString<1024>),
    /// List value - Boxed to break recursive type cycle
    #[cfg(feature = "std")]
    List(Box<Vec<Value>>),
    #[cfg(not(any(feature = "std",)))]
    List(Box<BoundedVec<Value, 256>>),
    /// Record value - Boxed to break recursive type cycle
    #[cfg(feature = "std")]
    Record(Box<Vec<Value>>),
    #[cfg(not(any(feature = "std",)))]
    Record(Box<BoundedVec<Value, 64>>),
    /// Tuple value - Boxed to break recursive type cycle
    #[cfg(feature = "std")]
    Tuple(Box<Vec<Value>>),
    #[cfg(not(any(feature = "std",)))]
    Tuple(Box<BoundedVec<Value, 32>>),
    /// Variant value
    Variant {
        discriminant: u32,
        value: Option<Box<Value>>,
    },
    /// Enum value
    Enum(u32),
    /// Option value
    Option(Option<Box<Value>>),
    /// Result value
    Result(core::result::Result<Option<Box<Value>>, Box<Value>>),
    /// Flags value
    Flags(u32),
    /// Owned resource
    Own(u32),
    /// Borrowed resource
    Borrow(u32),
    /// Stream handle
    Stream(StreamHandle),
    /// Future handle
    Future(FutureHandle),
}

impl Default for Value {
    fn default() -> Self {
        Value::Bool(false)
    }
}

// Manual Eq implementation for Value
// This is safe for WebAssembly use cases where we treat NaN as equal to itself
// for the purposes of value equality in collections and comparisons.
impl Eq for Value {}

impl wrt_foundation::traits::ToBytes for Value {
    fn serialized_size(&self) -> usize {
        match self {
            Value::Bool(_) => 2,                                // discriminant + bool
            Value::S8(_) | Value::U8(_) => 2,                   // discriminant + byte
            Value::S16(_) | Value::U16(_) => 3,                 // discriminant + 2 bytes
            Value::S32(_) | Value::U32(_) | Value::F32(_) => 5, // discriminant + 4 bytes
            Value::S64(_) | Value::U64(_) | Value::F64(_) => 9, // discriminant + 8 bytes
            Value::Char(_) => 5,                                // discriminant + 4 bytes
            _ => 1,                                             /* just discriminant for complex
                                                                  * types */
        }
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        use wrt_foundation::traits::WriteStream;

        match self {
            Value::Bool(b) => {
                writer.write_u8(0)?; // discriminant
                writer.write_u8(if *b { 1 } else { 0 })?;
            },
            Value::S8(v) => {
                writer.write_u8(1)?;
                writer.write_i8(*v)?;
            },
            Value::U8(v) => {
                writer.write_u8(2)?;
                writer.write_u8(*v)?;
            },
            Value::S16(v) => {
                writer.write_u8(3)?;
                writer.write_i16_le(*v)?;
            },
            Value::U16(v) => {
                writer.write_u8(4)?;
                writer.write_u16_le(*v)?;
            },
            Value::S32(v) => {
                writer.write_u8(5)?;
                writer.write_i32_le(*v)?;
            },
            Value::U32(v) => {
                writer.write_u8(6)?;
                writer.write_u32_le(*v)?;
            },
            Value::S64(v) => {
                writer.write_u8(7)?;
                writer.write_i64_le(*v)?;
            },
            Value::U64(v) => {
                writer.write_u8(8)?;
                writer.write_u64_le(*v)?;
            },
            Value::F32(v) => {
                writer.write_u8(9)?;
                writer.write_f32_le(*v)?;
            },
            Value::F64(v) => {
                writer.write_u8(10)?;
                writer.write_f64_le(*v)?;
            },
            Value::Char(c) => {
                writer.write_u8(11)?;
                writer.write_u32_le(*c as u32)?;
            },
            // For complex types, just store the discriminant
            _ => {
                writer.write_u8(255)?; // generic complex type discriminant
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for Value {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        use wrt_foundation::traits::ReadStream;

        let discriminant = reader.read_u8()?;

        match discriminant {
            0 => {
                let val = reader.read_u8()?;
                Ok(Value::Bool(val != 0))
            },
            1 => {
                let val = reader.read_i8()?;
                Ok(Value::S8(val))
            },
            2 => {
                let val = reader.read_u8()?;
                Ok(Value::U8(val))
            },
            3 => {
                let val = reader.read_i16_le()?;
                Ok(Value::S16(val))
            },
            4 => {
                let val = reader.read_u16_le()?;
                Ok(Value::U16(val))
            },
            5 => {
                let val = reader.read_i32_le()?;
                Ok(Value::S32(val))
            },
            6 => {
                let val = reader.read_u32_le()?;
                Ok(Value::U32(val))
            },
            7 => {
                let val = reader.read_i64_le()?;
                Ok(Value::S64(val))
            },
            8 => {
                let val = reader.read_u64_le()?;
                Ok(Value::U64(val))
            },
            9 => {
                let val = reader.read_f32_le()?;
                Ok(Value::F32(val))
            },
            10 => {
                let val = reader.read_f64_le()?;
                Ok(Value::F64(val))
            },
            11 => {
                let char_code = reader.read_u32_le()?;
                if let Some(c) = char::from_u32(char_code) {
                    Ok(Value::Char(c))
                } else {
                    Ok(Value::Char('\0'))
                }
            },
            _ => Ok(Value::Bool(false)), // default for complex/unknown types
        }
    }
}

impl wrt_foundation::traits::Checksummable for Value {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        // Simple checksum based on the discriminant and basic content
        let discriminant = match self {
            Value::Bool(_) => 0u8,
            Value::S8(_) => 1u8,
            Value::U8(_) => 2u8,
            Value::S16(_) => 3u8,
            Value::U16(_) => 4u8,
            Value::S32(_) => 5u8,
            Value::U32(_) => 6u8,
            Value::S64(_) => 7u8,
            Value::U64(_) => 8u8,
            Value::F32(_) => 9u8,
            Value::F64(_) => 10u8,
            Value::Char(_) => 11u8,
            Value::String(_) => 12u8,
            _ => 255u8,
        };
        checksum.update(discriminant);

        // For simplicity, just update with basic data
        match self {
            Value::Bool(b) => checksum.update(if *b { 1u8 } else { 0u8 }),
            Value::S8(v) => checksum.update(*v as u8),
            Value::U8(v) => checksum.update(*v),
            Value::S16(v) => checksum.update_slice(&v.to_le_bytes()),
            Value::U16(v) => checksum.update_slice(&v.to_le_bytes()),
            Value::S32(v) => checksum.update_slice(&v.to_le_bytes()),
            Value::U32(v) => checksum.update_slice(&v.to_le_bytes()),
            Value::S64(v) => checksum.update_slice(&v.to_le_bytes()),
            Value::U64(v) => checksum.update_slice(&v.to_le_bytes()),
            Value::F32(v) => checksum.update_slice(&v.to_bits().to_le_bytes()),
            Value::F64(v) => checksum.update_slice(&v.to_bits().to_le_bytes()),
            Value::Char(c) => checksum.update_slice(&(*c as u32).to_le_bytes()),
            Value::String(s) => {
                if let Ok(bytes) = s.as_bytes() {
                    checksum.update_slice(bytes.as_ref());
                }
            },
            _ => {}, // Skip complex types for now
        }
    }
}

/// Component instance identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ComponentInstanceId(pub u32);

impl ComponentInstanceId {
    /// Create a new component instance identifier
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }

    /// Get the inner value as a reference
    pub const fn as_u32(&self) -> u32 {
        self.0
    }

    /// Get the ID value (alias for as_u32 for compatibility)
    pub const fn id(&self) -> u32 {
        self.0
    }
}

impl From<ComponentInstanceId> for u64 {
    fn from(id: ComponentInstanceId) -> Self {
        id.0 as u64
    }
}

impl Checksummable for ComponentInstanceId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for ComponentInstanceId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ComponentInstanceId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Type identifier for generative types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TypeId(pub u32);

impl TypeId {
    /// Create a new type identifier
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }

    /// Get the inner value as a reference
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Checksummable for TypeId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for TypeId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for TypeId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Resource identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ResourceId(pub u32);

impl ResourceId {
    /// Create a new resource identifier
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Extract the inner value
    pub const fn into_inner(self) -> u32 {
        self.0
    }

    /// Get the inner value as a reference
    pub const fn as_u32(&self) -> u32 {
        self.0
    }
}

impl Checksummable for ResourceId {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        self.0.update_checksum(checksum);
    }
}

impl ToBytes for ResourceId {
    fn to_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<()> {
        self.0.to_bytes_with_provider(writer, provider)
    }
}

impl FromBytes for ResourceId {
    fn from_bytes_with_provider<'a, P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        provider: &P,
    ) -> wrt_error::Result<Self> {
        Ok(Self(u32::from_bytes_with_provider(reader, provider)?))
    }
}

/// Component error types
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentError {
    /// Too many generative types for a single instance
    TooManyGenerativeTypes,
    /// Too many type bounds for a single type
    TooManyTypeBounds,
    /// Resource handle already exists
    ResourceHandleAlreadyExists,
    /// Invalid type reference
    InvalidTypeReference(TypeId, TypeId),
    /// Invalid subtype relation
    InvalidSubtypeRelation(TypeId, TypeId),
    /// Component instantiation failed
    InstantiationFailed,
    /// Resource not found
    ResourceNotFound(u32),
    /// Type mismatch
    TypeMismatch,
    /// Import resolution failed
    ImportResolutionFailed,
    /// Export resolution failed
    ExportResolutionFailed,
    /// Memory allocation failed
    AllocationFailed,
}

impl fmt::Display for ComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComponentError::TooManyGenerativeTypes => write!(f, "Too many generative types"),
            ComponentError::TooManyTypeBounds => write!(f, "Too many type bounds"),
            ComponentError::ResourceHandleAlreadyExists => {
                write!(f, "Resource handle already exists")
            },
            ComponentError::InvalidTypeReference(type_id, target_type) => {
                write!(
                    f,
                    "Invalid type reference from {:?} to {:?}",
                    type_id, target_type
                )
            },
            ComponentError::InvalidSubtypeRelation(sub_type, super_type) => {
                write!(
                    f,
                    "Invalid subtype relation: {:?} <: {:?}",
                    sub_type, super_type
                )
            },
            ComponentError::InstantiationFailed => write!(f, "Component instantiation failed"),
            ComponentError::ResourceNotFound(handle) => write!(f, "Resource not found: {}", handle),
            ComponentError::TypeMismatch => write!(f, "Type mismatch"),
            ComponentError::ImportResolutionFailed => write!(f, "Import resolution failed"),
            ComponentError::ExportResolutionFailed => write!(f, "Export resolution failed"),
            ComponentError::AllocationFailed => write!(f, "Memory allocation failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ComponentError {}

// Conversion to wrt_error::Error for unified error handling
impl From<ComponentError> for wrt_error::Error {
    fn from(err: ComponentError) -> Self {
        use wrt_error::{ErrorCategory, codes};
        match err {
            ComponentError::TooManyGenerativeTypes => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_RESOURCE_LIFECYCLE_ERROR,
                "Too many generative types for component instance",
            ),
            ComponentError::TooManyTypeBounds => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_CONFIGURATION_INVALID,
                "Too many type bounds for component type",
            ),
            ComponentError::ResourceHandleAlreadyExists => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_HANDLE_REPRESENTATION_ERROR,
                "Resource handle already exists",
            ),
            ComponentError::InvalidTypeReference(_, _) => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_ABI_RUNTIME_ERROR,
                "Invalid type reference in component ABI",
            ),
            ComponentError::InvalidSubtypeRelation(_, _) => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_ABI_RUNTIME_ERROR,
                "Invalid subtype relation in component type system",
            ),
            ComponentError::InstantiationFailed => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_INSTANTIATION_RUNTIME_ERROR,
                "Component instantiation failed",
            ),
            ComponentError::ResourceNotFound(_) => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_HANDLE_REPRESENTATION_ERROR,
                "Component resource not found",
            ),
            ComponentError::TypeMismatch => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_ABI_RUNTIME_ERROR,
                "Component type mismatch",
            ),
            ComponentError::ImportResolutionFailed => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_INSTANTIATION_RUNTIME_ERROR,
                "Component import resolution failed",
            ),
            ComponentError::ExportResolutionFailed => Self::new(
                ErrorCategory::ComponentRuntime,
                codes::COMPONENT_INSTANTIATION_RUNTIME_ERROR,
                "Component export resolution failed",
            ),
            ComponentError::AllocationFailed => Self::new(
                ErrorCategory::Memory,
                codes::MEMORY_OUT_OF_BOUNDS,
                "Component memory allocation failed",
            ),
        }
    }
}

// Implement required traits for BoundedVec compatibility
use wrt_foundation::traits::{ReadStream, WriteStream};

// Macro to implement basic traits for complex types
macro_rules! impl_basic_traits {
    ($type:ty, $default_val:expr) => {
        impl Checksummable for $type {
            fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
                // Simple stub implementation
                0u32.update_checksum(checksum);
            }
        }

        impl ToBytes for $type {
            fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                &self,
                _writer: &mut WriteStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<()> {
                Ok(())
            }
        }

        impl FromBytes for $type {
            fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                _reader: &mut ReadStream<'a>,
                _provider: &PStream,
            ) -> wrt_error::Result<Self> {
                Ok($default_val)
            }
        }
    };
}

// Default implementations for complex types

impl Default for Record {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            fields: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            fields: BoundedVec::new(),
        }
    }
}

impl Default for Field {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            name: String::new(),
            #[cfg(not(any(feature = "std",)))]
            name: BoundedString::from_str_truncate("")
                .unwrap_or_else(|_| panic!("Failed to create default Field name")),
            ty: Box::new(ValType::default()),
        }
    }
}

impl Default for Tuple {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            types: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            types: BoundedVec::new(),
        }
    }
}

impl Default for Variant {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            cases: Vec::new(),
            #[cfg(not(any(feature = "std",)))]
            cases: BoundedVec::new(),
        }
    }
}

impl Default for Case {
    fn default() -> Self {
        Self {
            #[cfg(feature = "std")]
            name: String::new(),
            #[cfg(not(any(feature = "std",)))]
            name: BoundedString::from_str_truncate("")
                .unwrap_or_else(|_| panic!("Failed to create default Case name")),
            ty: None,
            refines: None,
        }
    }
}

// Apply macro to all complex types
impl_basic_traits!(ValType, ValType::default());
impl_basic_traits!(Record, Record::default());
impl_basic_traits!(Field, Field::default());
impl_basic_traits!(Tuple, Tuple::default());
impl_basic_traits!(Variant, Variant::default());
impl_basic_traits!(Case, Case::default());
