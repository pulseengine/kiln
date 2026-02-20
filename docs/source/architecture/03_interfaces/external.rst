.. _external_interfaces:

External Interfaces
===================

This section documents the external interfaces of Pulseengine (Kiln Edition), defining how
external systems and users interact with the runtime across different environments.

.. arch_interface:: ARCH_IF_EXT_001
   :title: Public API Interface
   :status: stable
   :version: 1.0
   :rationale: Provide consistent external API across all environments

   The primary public interface for external systems to interact with the WebAssembly runtime,
   maintaining API compatibility across std, no_std+alloc, and no_std+no_alloc environments.

Primary Runtime Interface
-------------------------

Core Runtime API
~~~~~~~~~~~~~~~~

The main external interface is provided through ``kiln/src/lib.rs``:

.. code-block:: rust

   // Primary runtime interface - available in all environments
   pub struct Runtime {
       inner: RuntimeInner,
   }

   impl Runtime {
       /// Create a new runtime instance
       /// Available in std and no_std+alloc environments
       #[cfg(any(feature = "std", feature = "alloc"))]
       pub fn new() -> Result<Self, RuntimeError> {
           Ok(Self {
               inner: RuntimeInner::new_dynamic()?,
           })
       }

       /// Create a bounded runtime instance  
       /// Available in all environments, required for no_alloc
       pub fn new_bounded() -> Result<Self, RuntimeError> {
           Ok(Self {
               inner: RuntimeInner::new_bounded()?,
           })
       }

       /// Load and instantiate a WebAssembly component
       pub fn instantiate(&mut self, bytes: &[u8]) -> Result<ComponentId, RuntimeError> {
           self.inner.instantiate(bytes)
       }

       /// Execute a function in a component
       pub fn execute(
           &mut self, 
           component_id: ComponentId, 
           function: &str, 
           args: &[Value]
       ) -> Result<Value, RuntimeError> {
           self.inner.execute(component_id, function, args)
       }
   }

**Environment Compatibility**:
- ``Runtime::new()``: Available in std and no_std+alloc ✅
- ``Runtime::new_bounded()``: Available in all environments ✅
- All other methods: Available in all environments ✅

Component Interface
~~~~~~~~~~~~~~~~~~~

Component loading interface from ``kiln-component/src/lib.rs``:

.. code-block:: rust

   /// Component loading and management
   pub struct Component {
       inner: ComponentInner,
   }

   impl Component {
       /// Load component from bytes
       pub fn from_bytes(bytes: &[u8]) -> Result<Self, ComponentError> {
           let parsed = Parser::parse(bytes)?;
           Ok(Self {
               inner: ComponentInner::new(parsed)?,
           })
       }

       /// Get component exports
       pub fn exports(&self) -> &ExportMap {
           &self.inner.exports
       }

       /// Get component imports  
       pub fn imports(&self) -> &ImportMap {
           &self.inner.imports
       }
   }

   // Environment-specific storage adaptations
   pub struct ExportMap {
       #[cfg(feature = "std")]
       exports: HashMap<String, Export>,
       
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       exports: BTreeMap<String, Export>,
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       exports: heapless::FnvIndexMap<String, Export, 128>,
   }

Command Line Interface
----------------------

.. arch_interface:: ARCH_IF_EXT_002
   :title: Command Line Interface  
   :status: stable
   :version: 1.0
   :rationale: Provide command-line access to runtime functionality

   Command-line interface for development, testing, and production deployment.

KILND CLI Tool
~~~~~~~~~~~~~

The ``kilnd`` binary provides external command-line access (``kilnd/src/main.rs:34-89``):

.. code-block:: rust

   #[derive(Debug, Parser)]
   #[command(name = "kilnd", about = "Pulseengine (Kiln Edition) CLI")]
   pub struct Cli {
       #[command(subcommand)]
       pub command: Commands,
   }

   #[derive(Debug, Subcommand)]
   pub enum Commands {
       /// Run a WebAssembly module
       Run {
           /// Path to WebAssembly file
           #[arg(short, long)]
           file: PathBuf,
           
           /// Function to execute
           #[arg(short = 'f', long, default_value = "main")]
           function: String,
           
           /// Use bounded runtime (no heap allocation)
           #[arg(long)]
           bounded: bool,
       },
       
       /// Inspect a WebAssembly module
       Inspect {
           /// Path to WebAssembly file
           #[arg(short, long)]
           file: PathBuf,
       },
       
       /// Validate a WebAssembly module
       Validate {
           /// Path to WebAssembly file  
           #[arg(short, long)]
           file: PathBuf,
       },
   }

**Usage Examples**:

.. code-block:: bash

   # Standard execution
   kilnd run --file module.wasm --function main
   
   # Bounded execution (no heap allocation)
   kilnd run --file module.wasm --function main --bounded
   
   # Module inspection
   kilnd inspect --file module.wasm
   
   # Module validation
   kilnd validate --file module.wasm

Host Function Interface
-----------------------

.. arch_interface:: ARCH_IF_EXT_003
   :title: Host Function Interface
   :status: stable  
   :version: 1.0
   :rationale: Enable WebAssembly modules to call host-provided functions

   Interface for registering and calling host functions from WebAssembly modules.

Host Function Registration
~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-host/src/host.rs:67-123``:

.. code-block:: rust

   /// Host function provider interface
   pub trait HostFunction {
       fn call(&self, args: &[Value]) -> Result<Value, HostError>;
   }

   pub struct HostFunctionRegistry {
       #[cfg(feature = "std")]
       functions: HashMap<String, Box<dyn HostFunction>>,
       
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       functions: BTreeMap<String, Box<dyn HostFunction>>,
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       functions: heapless::FnvIndexMap<&'static str, &'static dyn HostFunction, 64>,
   }

   impl HostFunctionRegistry {
       /// Register a host function
       pub fn register<F>(&mut self, name: impl Into<String>, func: F) -> Result<(), HostError>
       where
           F: HostFunction + 'static,
       {
           #[cfg(any(feature = "std", feature = "alloc"))]
           {
               self.functions.insert(name.into(), Box::new(func));
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               // Static registration for no_alloc
               let name_str = name.into();
               let static_name = leak_str(name_str); // Convert to &'static str
               self.functions.insert(static_name, &func)?;
           }
           
           Ok(())
       }
   }

**Environment Adaptations**:
- std/no_std+alloc: Dynamic registration with owned strings ✅
- no_std+no_alloc: Static registration with string constants ✅

Memory Interface
----------------

.. arch_interface:: ARCH_IF_EXT_004
   :title: Memory Management Interface
   :status: stable
   :version: 1.0  
   :rationale: Provide controlled access to WebAssembly linear memory

   External interface for accessing and managing WebAssembly linear memory.

Memory Access API
~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/safe_memory.rs:178-234``:

.. code-block:: rust

   /// Safe memory access interface
   pub struct LinearMemory {
       provider: Box<dyn MemoryProvider>,
   }

   impl LinearMemory {
       /// Read bytes from memory with bounds checking
       pub fn read_bytes(&self, offset: usize, length: usize) -> Result<&[u8], MemoryError> {
           if offset.saturating_add(length) > self.provider.len() {
               return Err(MemoryError::OutOfBounds { offset, length });
           }
           self.provider.read_bytes(offset, length)
       }

       /// Write bytes to memory with bounds checking  
       pub fn write_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), MemoryError> {
           if offset.saturating_add(data.len()) > self.provider.len() {
               return Err(MemoryError::OutOfBounds { 
                   offset, 
                   length: data.len() 
               });
           }
           self.provider.write_bytes(offset, data)
       }

       /// Get memory size
       pub fn size(&self) -> usize {
           self.provider.len()
       }

       /// Grow memory (if supported by environment)
       #[cfg(any(feature = "std", feature = "alloc"))]
       pub fn grow(&mut self, pages: usize) -> Result<usize, MemoryError> {
           self.provider.grow(pages)
       }
   }

Error Interface
---------------

.. arch_interface:: ARCH_IF_EXT_005
   :title: Error Reporting Interface
   :status: stable
   :version: 1.0
   :rationale: Provide consistent error reporting across environments

   Unified error interface that works in no_std environments.

Error Types and Handling
~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-error/src/errors.rs:45-123``:

.. code-block:: rust

   /// Primary error type for external consumers
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum KilnError {
       /// Component-related errors
       Component(ComponentError),
       /// Runtime execution errors  
       Runtime(RuntimeError),
       /// Memory access errors
       Memory(MemoryError),
       /// Host function errors
       Host(HostError),
       /// Validation errors
       Validation(ValidationError),
   }

   impl core::fmt::Display for KilnError {
       fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
           match self {
               Self::Component(e) => write!(f, "Component error: {}", e),
               Self::Runtime(e) => write!(f, "Runtime error: {}", e),
               Self::Memory(e) => write!(f, "Memory error: {}", e),
               Self::Host(e) => write!(f, "Host error: {}", e),
               Self::Validation(e) => write!(f, "Validation error: {}", e),
           }
       }
   }

   // std environment support
   #[cfg(feature = "std")]
   impl std::error::Error for KilnError {
       fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
           match self {
               Self::Component(e) => Some(e),
               Self::Runtime(e) => Some(e),
               Self::Memory(e) => Some(e),
               Self::Host(e) => Some(e),
               Self::Validation(e) => Some(e),
           }
       }
   }

Platform Integration Interface
------------------------------

.. arch_interface:: ARCH_IF_EXT_006
   :title: Platform Integration Interface
   :status: stable
   :version: 1.0
   :rationale: Enable integration with different operating systems and RTOSes

   Platform-specific integration points for different deployment environments.

Platform Abstraction
~~~~~~~~~~~~~~~~~~~~

From ``kiln-platform/src/platform_abstraction.rs:67-134``:

.. code-block:: rust

   /// Platform-specific functionality interface
   pub trait PlatformProvider {
       type Memory: MemoryProvider;
       type Sync: SyncProvider;
       
       /// Create platform memory instance
       fn create_memory(&self, size: usize) -> Result<Self::Memory, PlatformError>;
       
       /// Create platform synchronization primitives
       fn create_sync(&self) -> Result<Self::Sync, PlatformError>;
       
       /// Get platform capabilities
       fn capabilities(&self) -> PlatformCapabilities;
   }

   // Platform-specific implementations
   #[cfg(target_os = "linux")]
   pub struct LinuxPlatform;

   #[cfg(target_os = "macos")]  
   pub struct MacOSPlatform;

   #[cfg(target_os = "qnx")]
   pub struct QnxPlatform;

   // Embedded platforms
   pub struct ZephyrPlatform;
   pub struct TockPlatform;

**Supported Platforms**:
- Linux: Full std support ✅
- macOS: Full std support ✅  
- QNX: no_std+alloc support ✅
- Zephyr RTOS: no_std+no_alloc support ✅
- Tock OS: no_std+no_alloc support ✅

External API Guarantees
-----------------------

API Stability
~~~~~~~~~~~~~

**Semantic Versioning**: The external API follows semantic versioning:
- Major version changes: Breaking API changes
- Minor version changes: Backward-compatible additions
- Patch version changes: Bug fixes and performance improvements

**Environment Compatibility**: 
- All external APIs work in all supported environments ✅
- Environment-specific APIs clearly marked with ``#[cfg(...)]`` ✅
- Fallback implementations provided for no_alloc environments ✅

Performance Characteristics
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table:: API Performance Guarantees
   :header-rows: 1
   :widths: 30 20 20 30

   * - Operation
     - std
     - no_std+alloc  
     - no_std+no_alloc
   * - Component loading
     - O(n)
     - O(n)
     - O(n) with bounds check
   * - Function execution
     - O(1) dispatch
     - O(1) dispatch
     - O(1) dispatch
   * - Memory access
     - O(1)
     - O(1)
     - O(1) with bounds check
   * - Host function call
     - O(1) lookup
     - O(log n) lookup
     - O(1) array lookup

Usage Examples
--------------

Complete External Usage Example
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln::{Runtime, Value, ComponentError};
   
   fn main() -> Result<(), ComponentError> {
       // Works in all environments
       let mut runtime = Runtime::new_bounded()?;
       
       // Load WebAssembly component
       let wasm_bytes = include_bytes!("module.wasm");
       let component_id = runtime.instantiate(wasm_bytes)?;
       
       // Execute function  
       let args = [Value::I32(42), Value::I32(24)];
       let result = runtime.execute(component_id, "add", &args)?;
       
       println!("Result: {:?}", result);
       Ok(())
   }

Cross-References
-----------------

.. seealso::

   * :doc:`internal` for internal component interfaces
   * :doc:`api_contracts` for detailed API contracts and specifications
   * :doc:`../01_architectural_design/components` for component implementation details
   * :doc:`../04_dynamic_behavior/interaction_flows` for external interaction patterns