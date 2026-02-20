==========================
Interface Catalog
==========================

**Teaching Point**: Interfaces define contracts between components. This catalog shows the actual interfaces implemented in the codebase.

Core Runtime Interfaces
-----------------------

Stackless Engine Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Stackless Engine
   :id: ARCH_IF_001
   :component: ARCH_COMP_001
   :file: kiln-runtime/src/stackless/engine.rs
   :type: provided
   :stability: stable

**Purpose**: Defines the stackless WebAssembly execution engine that doesn't rely on host call stack.

**Actual Implementation**:

.. code-block:: rust

   pub struct StacklessEngine {
       pub(crate) exec_stack: StacklessStack,
       fuel: Option<u64>,
       stats: ExecutionStats,
       callbacks: Arc<Mutex<StacklessCallbackRegistry>>,
       max_call_depth: Option<usize>,
       pub(crate) instance_count: usize,
       verification_level: VerificationLevel,
   }

   impl ControlContext for StacklessEngine {
       fn push_control_value(&mut self, value: Value) -> Result<()>;
       fn pop_control_value(&mut self) -> Result<Value>;
       fn get_block_depth(&self) -> usize;
       fn enter_block(&mut self, block_type: Block) -> Result<()>;
       fn exit_block(&mut self) -> Result<Block>;
       fn branch(&mut self, target: BranchTarget) -> Result<()>;
       fn return_function(&mut self) -> Result<()>;
       fn call_function(&mut self, func_idx: u32) -> Result<()>;
       fn call_indirect(&mut self, table_idx: u32, type_idx: u32) -> Result<()>;
   }

**Environment Variations**:

- **std**: Full async support with `Arc<Mutex<T>>`
- **no_std + alloc**: Bounded collections with `RefCell`
- **no_std + no_alloc**: Static execution with compile-time bounds

Platform Memory Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Platform Memory
   :id: ARCH_IF_020
   :component: ARCH_COMP_002
   :file: kiln-platform/src/memory.rs
   :type: provided
   :stability: stable

**Purpose**: Provides platform-specific memory allocation and management.

**PageAllocator Trait**:

.. code-block:: rust

   pub trait PageAllocator: Debug + Send + Sync {
       fn allocate(
           &mut self,
           initial_pages: u32,
           maximum_pages: Option<u32>,
       ) -> Result<(NonNull<u8>, usize)>;
       
       fn grow(&mut self, current_pages: u32, additional_pages: u32) -> Result<()>;
       
       unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) -> Result<()>;
   }

**MemoryProvider Trait**:

.. code-block:: rust

   pub trait MemoryProvider: Send + Sync {
       fn capacity(&self) -> usize;
       fn verification_level(&self) -> VerificationLevel;
       fn with_verification_level(level: VerificationLevel) -> Self;
   }

**Safe Memory Abstractions**:

.. code-block:: rust

   pub struct SafeMemoryHandler<P: MemoryProvider> {
       provider: P,
       verification_level: VerificationLevel,
   }
   
   pub struct Slice<'a> {
       data: &'a [u8],
       checksum: Checksum,
       verification_level: VerificationLevel,
   }
   
   pub struct SliceMut<'a> {
       data: &'a mut [u8],
       checksum: Checksum,
       verification_level: VerificationLevel,
   }

**Platform Implementations**:

.. list-table:: Platform Memory Implementations
   :header-rows: 1

   * - Platform
     - Implementation
     - Features
   * - Linux
     - ``LinuxAllocator``
     - mmap, guard pages, MTE support
   * - macOS  
     - ``MacOsAllocator``
     - vm_allocate, direct syscalls
   * - QNX
     - ``QnxAllocator``
     - shm_open, partition support
   * - No-std
     - ``NoStdProvider<N>``
     - Static arrays, compile-time bounds

Security and Control Flow Interfaces
-----------------------------------

CFI Control Flow Operations Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: CFI Control Flow Operations
   :id: ARCH_IF_101
   :component: ARCH_COMP_101
   :file: kiln-instructions/src/cfi_control_ops.rs
   :type: provided
   :stability: stable

**Purpose**: Provides Control Flow Integrity protection for WebAssembly execution.

**Actual Implementation**:

.. code-block:: rust

   pub trait CfiControlFlowOps {
       fn call_indirect_with_cfi(
           &mut self,
           type_idx: u32,
           table_idx: u32,
           protection: &CfiControlFlowProtection,
           context: &mut CfiExecutionContext,
       ) -> Result<CfiProtectedBranchTarget>;
       
       fn return_with_cfi(
           &mut self,
           protection: &CfiControlFlowProtection,
           context: &mut CfiExecutionContext,
       ) -> Result<()>;
       
       fn branch_with_cfi(
           &mut self,
           label_idx: u32,
           conditional: bool,
           protection: &CfiControlFlowProtection,
           context: &mut CfiExecutionContext,
       ) -> Result<CfiProtectedBranchTarget>;
   }

   pub struct CfiExecutionEngine {
       cfi_ops: DefaultCfiControlFlowOps,
       cfi_protection: CfiControlFlowProtection,
       cfi_context: CfiExecutionContext,
       violation_policy: CfiViolationPolicy,
       statistics: CfiEngineStatistics,
   }

Async Runtime Interface
~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Async Runtime
   :id: ARCH_IF_102
   :component: ARCH_COMP_102
   :file: kiln-component/src/async_/async_runtime.rs
   :type: provided
   :stability: stable

**Purpose**: Provides async/await capabilities for WebAssembly Component Model.

**Actual Implementation**:

.. code-block:: rust

   pub struct AsyncExecutionEngine {
       scheduler: TaskScheduler,
       runtime_bridge: AsyncRuntimeBridge,
       resource_cleanup: AsyncResourceCleanup,
       context_manager: AsyncContextManager,
   }

   pub trait AsyncCanonicalLift<T> {
       async fn async_lift(&self, bytes: &[u8]) -> Result<T>;
       fn can_lift_sync(&self, bytes: &[u8]) -> bool;
   }

   pub trait AsyncCanonicalLower<T> {
       async fn async_lower(&self, value: T) -> Result<Vec<u8>>;
       fn can_lower_sync(&self, value: &T) -> bool;
   }

Threading Management Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Threading Management
   :id: ARCH_IF_103
   :component: ARCH_COMP_103
   :file: kiln-component/src/threading/task_manager.rs
   :type: provided
   :stability: stable

**Purpose**: Comprehensive task and thread management for WebAssembly components.

**Actual Implementation**:

.. code-block:: rust

   pub struct TaskManager {
       task_registry: BoundedHashMap<TaskId, TaskInfo, 2048>,
       scheduler: PriorityTaskScheduler,
       resource_limits: TaskResourceLimits,
       cancellation: TaskCancellation,
   }

   pub struct ThreadSpawnFuel {
       fuel_pool: FuelPool,
       thread_limits: ThreadLimits,
       platform_config: PlatformThreadConfig,
       thread_tracker: ThreadTracker,
   }

Debug Infrastructure Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Debug Infrastructure
   :id: ARCH_IF_104
   :component: ARCH_COMP_104
   :file: kiln-debug/src/lib.rs
   :type: provided
   :stability: stable

**Purpose**: Comprehensive debugging support with DWARF and WIT integration.

**Actual Implementation**:

.. code-block:: rust

   pub trait RuntimeDebugger {
       fn attach(&mut self, instance: &mut ModuleInstance) -> Result<DebugSession>;
       fn set_breakpoint(&mut self, address: Address) -> Result<BreakpointId>;
       fn remove_breakpoint(&mut self, id: BreakpointId) -> Result<()>;
       fn step(&mut self, mode: StepMode) -> Result<ExecutionState>;
       fn continue_execution(&mut self) -> Result<ExecutionState>;
       fn get_stack_trace(&self) -> Result<StackTrace>;
       fn inspect_variable(&self, name: &str) -> Result<VariableValue>;
       fn read_memory(&self, address: Address, size: usize) -> Result<Vec<u8>>;
   }

   pub struct DwarfDebugInfo {
       debug_info: DebugInfo,
       debug_line: DebugLine,
       debug_str: DebugStr,
       debug_abbrev: DebugAbbrev,
       debug_loc: DebugLoc,
       debug_frame: DebugFrame,
   }

Component Model Interfaces
--------------------------

Component Instance Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Component Instance
   :id: ARCH_IF_030
   :component: ARCH_COMP_003
   :file: kiln-component/src/component_traits.rs
   :type: provided
   :stability: stable

**Actual Trait**:

.. code-block:: rust

   pub trait ComponentInstance {
       fn new(runtime: Arc<dyn ComponentRuntime>) -> KilnResult<Self> where Self: Sized;
       fn add_import(&mut self, name: String, instance: Arc<dyn ComponentInstance>) -> KilnResult<()>;
       fn get_export(&self, name: &str) -> Option<Arc<dyn Any>>;
       fn instantiate(&mut self) -> KilnResult<()>;
   }

Host Function Interface
~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Host Function
   :id: ARCH_IF_031
   :component: ARCH_COMP_003
   :file: kiln-component/src/component_traits.rs
   :type: required
   :stability: stable

**Purpose**: Allows host environment to provide functions to WASM.

.. code-block:: rust

   pub trait HostFunction: Send + Sync {
       fn call(&self, args: &[Value]) -> KilnResult<Vec<Value>>;
       fn signature(&self) -> &FuncType;
   }

Platform Abstraction Interfaces
-------------------------------

Page Allocator Interface
~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Page Allocator
   :id: ARCH_IF_050
   :component: ARCH_COMP_005
   :file: kiln-platform/src/memory.rs
   :type: provided
   :stability: stable

**Teaching Point**: This abstracts memory page management across different OSes:

.. code-block:: rust

   pub trait PageAllocator: Send + Sync {
       fn allocate(&mut self, pages: usize) -> Result<*mut u8, Error>;
       fn deallocate(&mut self, ptr: *mut u8, pages: usize) -> Result<(), Error>;
       fn grow(&mut self, ptr: *mut u8, old_pages: usize, new_pages: usize) -> Result<*mut u8, Error>;
       fn protect(&mut self, ptr: *mut u8, pages: usize, prot: Protection) -> Result<(), Error>;
   }

**Platform Implementations**:

- Linux: ``mmap``/``munmap`` with ``PROT_MTE`` support
- macOS: ``mmap`` with guard pages
- QNX: Arena allocator with partitions
- Bare-metal: Static buffer allocation

Synchronization Interface
~~~~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Futex-like Operations
   :id: ARCH_IF_051
   :component: ARCH_COMP_005
   :file: kiln-platform/src/sync.rs
   :type: provided
   :stability: stable

.. code-block:: rust

   pub trait FutexLike: Send + Sync {
       fn wait(&self, addr: &AtomicU32, expected: u32, timeout: Option<Duration>) -> Result<(), Error>;
       fn wake(&self, addr: &AtomicU32, count: u32) -> Result<u32, Error>;
   }

Internal Interfaces
-------------------

Instruction Traits
~~~~~~~~~~~~~~~~~~

.. arch_interface:: Pure Instruction
   :id: ARCH_IF_060
   :component: ARCH_COMP_011
   :file: kiln-instructions/src/instruction_traits.rs
   :type: internal
   :stability: stable

**Purpose**: Common behavior for all instructions.

.. code-block:: rust

   pub trait PureInstruction {
       fn execute<C: InstructionContext>(&self, context: &mut C) -> Result<(), Error>;
       fn get_opcode(&self) -> u8;
   }

Verification Interface
~~~~~~~~~~~~~~~~~~~~~~

.. arch_interface:: Validatable
   :id: ARCH_IF_070
   :component: ARCH_COMP_002
   :file: kiln-foundation/src/traits.rs
   :type: internal
   :stability: stable

.. code-block:: rust

   pub trait Validatable {
       fn validate(&self) -> Result<(), ValidationError>;
       fn validate_with_level(&self, level: VerificationLevel) -> Result<(), ValidationError>;
   }

Interface Compatibility Matrix
------------------------------

.. list-table:: Feature-Based Interface Availability
   :header-rows: 1

   * - Interface
     - std
     - no_std + alloc
     - no_std + no_alloc
   * - EngineBehavior
     - ✓ Full
     - ✓ Full
     - ✓ Limited instances
   * - MemoryProvider
     - ✓ Dynamic
     - ✓ Dynamic
     - ✓ Static only
   * - ComponentInstance
     - ✓ Full
     - ✓ Full
     - ✓ Bounded
   * - PageAllocator
     - ✓ OS-based
     - ✓ OS-based
     - ✓ Static
   * - FutexLike
     - ✓ Native
     - ✓ Emulated
     - ✗ Spin-only

Cross-References
----------------

- **Component Definitions**: See :doc:`../01_architectural_design/components`
- **API Contracts**: See :doc:`api_contracts`
- **Usage Examples**: See component-specific examples in :doc:`/examples/index`