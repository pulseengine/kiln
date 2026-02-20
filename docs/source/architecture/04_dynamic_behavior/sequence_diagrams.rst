.. _sequence_diagrams:

Sequence Diagrams
==================

This section provides detailed sequence diagrams showing the dynamic interactions between
components in PulseEngine (Kiln Edition) across different runtime environments and scenarios.

.. warning::
   **Design Documentation**: These sequence diagrams show the intended design and interactions.
   Many of these sequences are not yet implemented. Component instantiation, function execution,
   and resource management are under development.

.. arch_component:: ARCH_COMP_SEQ_001
   :title: Component Interaction Sequences
   :status: design
   :version: 1.0
   :rationale: Document runtime interaction patterns for different environments

   Comprehensive sequence diagrams showing intended component interactions during
   component loading, execution, and resource management across environments.
   
   **Note**: These are design diagrams. Actual implementation is in progress.

Component Instantiation Sequence
---------------------------------

Standard Environment (std)
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Component Instantiation Sequence - std Environment

   @startuml
   participant "User Code" as User
   participant "Runtime" as Runtime
   participant "Decoder" as Decoder
   participant "Component" as Component
   participant "Memory Manager" as Memory
   participant "Resource Table" as Resources

   User -> Runtime: instantiate(wasm_bytes)
   activate Runtime
   
   Runtime -> Decoder: parse(wasm_bytes)
   activate Decoder
   
   Decoder -> Decoder: validate_format()
   Decoder -> Decoder: parse_sections()
   note right: Dynamic Vec<Section> allocation
   
   Decoder --> Runtime: ParsedComponent
   deactivate Decoder
   
   Runtime -> Component: create_from_parsed()
   activate Component
   
   Component -> Memory: allocate_linear_memory(size)
   activate Memory
   Memory -> Memory: Vec::with_capacity(size)
   note right: Dynamic heap allocation
   Memory --> Component: LinearMemory
   deactivate Memory
   
   Component -> Resources: create_resource_table()
   activate Resources
   Resources -> Resources: HashMap::new()
   note right: Dynamic HashMap allocation
   Resources --> Component: ResourceTable
   deactivate Resources
   
   Component -> Component: resolve_imports()
   Component -> Component: initialize_exports()
   
   Component --> Runtime: ComponentInstance
   deactivate Component
   
   Runtime --> User: ComponentId
   deactivate Runtime
   @enduml

No-Alloc Environment (no_std+no_alloc)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Component Instantiation Sequence - no_std+no_alloc Environment

   @startuml
   participant "User Code" as User
   participant "Runtime" as Runtime
   participant "Decoder" as Decoder
   participant "Component" as Component
   participant "Memory Manager" as Memory
   participant "Resource Table" as Resources

   User -> Runtime: instantiate(wasm_bytes)
   activate Runtime
   
   Runtime -> Decoder: parse(wasm_bytes)
   activate Decoder
   
   Decoder -> Decoder: validate_format()
   Decoder -> Decoder: parse_sections()
   note right: heapless::Vec<Section, 64> fixed allocation
   
   alt Section count > 64
       Decoder --> Runtime: Error::TooManySections
   else Section count <= 64
       Decoder --> Runtime: ParsedComponent
   end
   deactivate Decoder
   
   Runtime -> Component: create_from_parsed()
   activate Component
   
   Component -> Memory: allocate_linear_memory(size)
   activate Memory
   
   alt size > 65536
       Memory --> Component: Error::MemoryTooLarge
   else size <= 65536
       Memory -> Memory: [u8; 65536] static allocation
       Memory --> Component: BoundedMemory
   end
   deactivate Memory
   
   Component -> Resources: create_resource_table()
   activate Resources
   Resources -> Resources: heapless::FnvIndexMap::new()
   note right: Fixed-size resource slots
   Resources --> Component: BoundedResourceTable
   deactivate Resources
   
   Component -> Component: resolve_imports()
   Component -> Component: initialize_exports()
   
   Component --> Runtime: ComponentInstance
   deactivate Component
   
   Runtime --> User: ComponentId
   deactivate Runtime
   @enduml

Function Execution Sequence
---------------------------

Successful Function Execution
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Function Execution Sequence - Success Case

   @startuml
   participant "User Code" as User
   participant "Runtime" as Runtime
   participant "Component" as Component
   participant "Execution Engine" as Engine
   participant "Memory" as Memory
   participant "Host Functions" as Host

   User -> Runtime: execute(component_id, "function_name", args)
   activate Runtime
   
   Runtime -> Runtime: validate_component_id()
   Runtime -> Runtime: validate_args()
   
   Runtime -> Component: get_component(component_id)
   activate Component
   Component --> Runtime: ComponentInstance
   deactivate Component
   
   Runtime -> Engine: execute_function(component, function, args)
   activate Engine
   
   Engine -> Engine: setup_execution_frame()
   Engine -> Engine: push_args_to_stack()
   
   loop For each instruction
       Engine -> Engine: decode_instruction()
       
       alt Memory instruction
           Engine -> Memory: read_bytes(offset, length)
           activate Memory
           Memory -> Memory: bounds_check(offset, length)
           Memory --> Engine: &[u8]
           deactivate Memory
       else Host function call
           Engine -> Host: call_host_function(name, args)
           activate Host
           Host -> Host: execute_native_function()
           Host --> Engine: Result<Value>
           deactivate Host
       else Regular instruction
           Engine -> Engine: execute_instruction()
       end
   end
   
   Engine -> Engine: pop_result_from_stack()
   Engine --> Runtime: Result<Value>
   deactivate Engine
   
   Runtime --> User: Value
   deactivate Runtime
   @enduml

Function Execution with Error Handling
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Function Execution Sequence - Error Handling

   @startuml
   participant "User Code" as User
   participant "Runtime" as Runtime
   participant "Component" as Component
   participant "Execution Engine" as Engine
   participant "Memory" as Memory
   participant "Error Handler" as Error

   User -> Runtime: execute(component_id, "function_name", args)
   activate Runtime
   
   Runtime -> Engine: execute_function(component, function, args)
   activate Engine
   
   Engine -> Engine: setup_execution_frame()
   
   loop For each instruction
       Engine -> Engine: decode_instruction()
       
       alt Memory bounds violation
           Engine -> Memory: read_bytes(offset, length)
           activate Memory
           Memory -> Memory: bounds_check(offset, length)
           Memory --> Engine: Error::OutOfBounds
           deactivate Memory
           
           Engine -> Error: handle_memory_error()
           activate Error
           Error -> Error: create_error_context()
           Error -> Error: capture_execution_state()
           Error --> Engine: ExecutionError
           deactivate Error
           
           Engine --> Runtime: Err(ExecutionError)
       else Stack overflow
           Engine -> Engine: check_stack_depth()
           
           alt depth > max_depth
               Engine -> Error: handle_stack_overflow()
               activate Error
               Error --> Engine: ExecutionError
               deactivate Error
               Engine --> Runtime: Err(ExecutionError)
           end
       end
   end
   
   deactivate Engine
   
   Runtime -> Runtime: cleanup_failed_execution()
   Runtime --> User: Err(KilnError)
   deactivate Runtime
   @enduml

Resource Management Sequence
----------------------------

Resource Allocation and Access
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Resource Management Sequence - std Environment

   @startuml
   participant "Component A" as CompA
   participant "Component B" as CompB
   participant "Resource Manager" as ResMgr
   participant "Resource Table" as ResTable
   participant "Memory Pool" as Pool

   CompA -> ResMgr: allocate_resource<FileHandle>()
   activate ResMgr
   
   ResMgr -> ResTable: find_available_slot()
   activate ResTable
   
   alt std environment
       ResTable -> ResTable: HashMap::insert()
       note right: Dynamic allocation
   else no_alloc environment
       ResTable -> ResTable: check_pool_availability()
       alt pool full
           ResTable --> ResMgr: Error::PoolExhausted
       end
   end
   
   ResTable --> ResMgr: ResourceId(42)
   deactivate ResTable
   
   ResMgr -> Pool: allocate_storage(size_hint)
   activate Pool
   Pool --> ResMgr: ResourceSlot
   deactivate Pool
   
   ResMgr -> ResMgr: initialize_resource()
   ResMgr --> CompA: ResourceId(42)
   deactivate ResMgr
   
   note over CompA: Component A stores ResourceId
   
   CompB -> ResMgr: get_resource<FileHandle>(42)
   activate ResMgr
   
   ResMgr -> ResTable: lookup(ResourceId(42))
   activate ResTable
   ResTable -> ResTable: validate_type<FileHandle>()
   ResTable --> ResMgr: &FileHandle
   deactivate ResTable
   
   ResMgr --> CompB: &FileHandle
   deactivate ResMgr
   
   CompA -> ResMgr: deallocate_resource(42)
   activate ResMgr
   
   ResMgr -> ResTable: remove(ResourceId(42))
   activate ResTable
   ResTable -> Pool: return_to_pool(slot)
   activate Pool
   Pool --> ResTable: Ok()
   deactivate Pool
   ResTable --> ResMgr: Ok()
   deactivate ResTable
   
   ResMgr --> CompA: Ok()
   deactivate ResMgr
   @enduml

Resource Contention Handling
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Resource Contention Sequence

   @startuml
   participant "Component A" as CompA
   participant "Component B" as CompB
   participant "Resource Manager" as ResMgr
   participant "Lock Manager" as LockMgr
   participant "Resource Table" as ResTable

   CompA -> ResMgr: lock_resource(ResourceId(42))
   activate ResMgr
   
   ResMgr -> LockMgr: acquire_exclusive_lock(42)
   activate LockMgr
   LockMgr -> LockMgr: check_current_locks()
   LockMgr --> ResMgr: Ok(LockHandle)
   deactivate LockMgr
   
   ResMgr --> CompA: Ok(LockHandle)
   deactivate ResMgr
   
   CompB -> ResMgr: lock_resource(ResourceId(42))
   activate ResMgr
   
   ResMgr -> LockMgr: acquire_exclusive_lock(42)
   activate LockMgr
   LockMgr -> LockMgr: check_current_locks()
   
   alt Resource already locked
       LockMgr --> ResMgr: Error::ResourceLocked
       ResMgr --> CompB: Error::ResourceLocked
   else Deadlock detection enabled
       LockMgr -> LockMgr: check_deadlock_potential()
       alt Would cause deadlock
           LockMgr --> ResMgr: Error::PotentialDeadlock
           ResMgr --> CompB: Error::PotentialDeadlock
       end
   end
   deactivate LockMgr
   deactivate ResMgr
   
   CompA -> ResMgr: unlock_resource(LockHandle)
   activate ResMgr
   ResMgr -> LockMgr: release_lock(LockHandle)
   activate LockMgr
   LockMgr -> LockMgr: notify_waiting_components()
   LockMgr --> ResMgr: Ok()
   deactivate LockMgr
   ResMgr --> CompA: Ok()
   deactivate ResMgr
   @enduml

Memory Management Sequence
--------------------------

Memory Allocation and Protection
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Memory Management Sequence

   @startuml
   participant "Component" as Comp
   participant "Memory Manager" as MemMgr
   participant "Platform Memory" as Platform
   participant "Protection System" as Protection

   Comp -> MemMgr: allocate_memory(64KB, RW)
   activate MemMgr
   
   MemMgr -> MemMgr: validate_size(64KB)
   MemMgr -> MemMgr: check_memory_limits()
   
   alt std environment
       MemMgr -> Platform: allocate_pages(16) // 64KB / 4KB
       activate Platform
       Platform -> Platform: mmap() or VirtualAlloc()
       Platform --> MemMgr: MemoryRegion
       deactivate Platform
   else no_alloc environment
       MemMgr -> MemMgr: allocate_from_static_pool()
       alt static pool exhausted
           MemMgr --> Comp: Error::OutOfMemory
       end
   end
   
   MemMgr -> Protection: set_protection(region, RW)
   activate Protection
   
   alt Platform supports memory protection
       Protection -> Platform: mprotect(region, RW)
       activate Platform
       Platform --> Protection: Ok()
       deactivate Platform
   else No memory protection
       Protection -> Protection: track_protection_flags()
   end
   
   Protection --> MemMgr: Ok()
   deactivate Protection
   
   MemMgr -> MemMgr: register_memory_region()
   MemMgr --> Comp: LinearMemory
   deactivate MemMgr
   
   note over Comp: Component uses memory
   
   Comp -> MemMgr: change_protection(region, RO)
   activate MemMgr
   
   MemMgr -> Protection: set_protection(region, RO)
   activate Protection
   Protection -> Platform: mprotect(region, RO)
   activate Platform
   Platform --> Protection: Ok()
   deactivate Platform
   Protection --> MemMgr: Ok()
   deactivate Protection
   
   MemMgr --> Comp: Ok()
   deactivate MemMgr
   @enduml

Cross-Environment Error Propagation
-----------------------------------

Error Context Building
~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Error Propagation Sequence Across Components

   @startuml
   participant "User Code" as User
   participant "Runtime" as Runtime
   participant "Component" as Component
   participant "Memory" as Memory
   participant "Error Context" as ErrorCtx

   User -> Runtime: execute(component_id, function, args)
   activate Runtime
   
   Runtime -> Component: execute_function(function, args)
   activate Component
   
   Component -> Memory: read_bytes(offset, length)
   activate Memory
   
   Memory -> Memory: bounds_check(offset, length)
   
   alt Out of bounds access
       Memory -> ErrorCtx: create_memory_error()
       activate ErrorCtx
       ErrorCtx -> ErrorCtx: capture_memory_context()
       note right: offset, length, memory_size
       ErrorCtx --> Memory: MemoryError::OutOfBounds
       deactivate ErrorCtx
       
       Memory --> Component: Err(MemoryError)
   end
   deactivate Memory
   
   Component -> ErrorCtx: add_component_context()
   activate ErrorCtx
   ErrorCtx -> ErrorCtx: wrap_error_with_context()
   note right: component_id, function_name
   ErrorCtx --> Component: ComponentError
   deactivate ErrorCtx
   
   Component --> Runtime: Err(ComponentError)
   deactivate Component
   
   Runtime -> ErrorCtx: add_runtime_context()
   activate ErrorCtx
   ErrorCtx -> ErrorCtx: create_error_chain()
   note right: execution_state, call_stack
   ErrorCtx --> Runtime: KilnError
   deactivate ErrorCtx
   
   Runtime --> User: Err(KilnError)
   deactivate Runtime
   @enduml

Environment-Specific Timing Differences
---------------------------------------

Performance Comparison Sequence
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Performance Timing - Component Instantiation

   @startuml
   participant "std\nEnvironment" as Std
   participant "no_std+alloc\nEnvironment" as NoStdAlloc
   participant "no_std+no_alloc\nEnvironment" as NoAlloc

   note over Std, NoAlloc: Component Instantiation Timing Comparison

   Std -> Std: parse_component()
   note right Std: ~5ms\n(dynamic allocation)
   
   NoStdAlloc -> NoStdAlloc: parse_component()
   note right NoStdAlloc: ~6ms\n(BTreeMap overhead)
   
   NoAlloc -> NoAlloc: parse_component()
   note right NoAlloc: ~3ms\n(stack allocation)
   
   Std -> Std: allocate_memory()
   note right Std: ~2ms\n(heap allocation)
   
   NoStdAlloc -> NoStdAlloc: allocate_memory()
   note right NoStdAlloc: ~2.5ms\n(heap allocation)
   
   NoAlloc -> NoAlloc: allocate_memory()
   note right NoAlloc: ~0.1ms\n(static allocation)
   
   Std -> Std: setup_resources()
   note right Std: ~1ms\n(HashMap creation)
   
   NoStdAlloc -> NoStdAlloc: setup_resources()
   note right NoStdAlloc: ~1.5ms\n(BTreeMap creation)
   
   NoAlloc -> NoAlloc: setup_resources()
   note right NoAlloc: ~0.2ms\n(fixed pools)
   
   note over Std: Total: ~8ms
   note over NoStdAlloc: Total: ~10ms  
   note over NoAlloc: Total: ~3.3ms
   @enduml

Concurrency and Synchronization
-------------------------------

Multi-Component Execution
~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Concurrent Component Execution

   @startuml
   participant "Runtime" as Runtime
   participant "Component A" as CompA
   participant "Component B" as CompB
   participant "Shared Resource" as Shared
   participant "Sync Manager" as Sync

   par Component A execution
       Runtime -> CompA: execute("process_data")
       activate CompA
       CompA -> Shared: request_access()
       activate Shared
       Shared -> Sync: acquire_lock()
       activate Sync
       
   and Component B execution
       Runtime -> CompB: execute("transform_data")  
       activate CompB
       CompB -> Shared: request_access()
       Shared -> Sync: acquire_lock()
       
       alt Resource available
           Sync --> Shared: Ok(Lock)
           Shared --> CompB: Ok(Access)
       else Resource locked
           Sync --> Shared: Error::WouldBlock
           Shared --> CompB: Error::ResourceBusy
           CompB -> CompB: retry_with_backoff()
       end
   end
   
   CompA -> CompA: process_data_with_resource()
   CompA -> Shared: release_access()
   Shared -> Sync: release_lock()
   Sync -> Sync: notify_waiting_components()
   Sync --> Shared: Ok()
   deactivate Sync
   Shared --> CompA: Ok()
   deactivate Shared
   CompA --> Runtime: Result
   deactivate CompA
   
   Sync -> CompB: lock_available()
   activate CompB
   CompB -> Shared: request_access()
   activate Shared
   Shared --> CompB: Ok(Access)
   CompB -> CompB: transform_data_with_resource()
   CompB --> Runtime: Result
   deactivate CompB
   deactivate Shared
   @enduml

Testing and Verification Sequences
----------------------------------

Cross-Environment Test Execution
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. uml::
   :caption: Cross-Environment Test Verification

   @startuml
   participant "Test Runner" as Test
   participant "std Runtime" as StdRT
   participant "no_alloc Runtime" as NoAllocRT
   participant "Test Component" as TestComp

   Test -> Test: load_test_component()
   
   par std environment test
       Test -> StdRT: instantiate(test_wasm)
       activate StdRT
       StdRT -> TestComp: create_with_dynamic_memory()
       activate TestComp
       TestComp --> StdRT: ComponentInstance
       StdRT -> StdRT: execute_test_suite()
       StdRT --> Test: TestResults(std)
       deactivate StdRT
       deactivate TestComp
       
   and no_alloc environment test
       Test -> NoAllocRT: instantiate(test_wasm)
       activate NoAllocRT
       NoAllocRT -> TestComp: create_with_bounded_memory()
       activate TestComp
       TestComp --> NoAllocRT: ComponentInstance
       NoAllocRT -> NoAllocRT: execute_test_suite()
       NoAllocRT --> Test: TestResults(no_alloc)
       deactivate NoAllocRT
       deactivate TestComp
   end
   
   Test -> Test: compare_results(std, no_alloc)
   Test -> Test: verify_behavioral_equivalence()
   
   alt Results equivalent
       Test -> Test: mark_test_passed()
   else Results differ
       Test -> Test: analyze_difference()
       Test -> Test: report_incompatibility()
   end
   @enduml

Cross-References
-----------------

.. seealso::

   * :doc:`state_machines` for component state management
   * :doc:`interaction_flows` for high-level interaction patterns
   * :doc:`../03_interfaces/internal` for detailed interface specifications
   * :doc:`../05_resource_management/resource_overview` for resource management details