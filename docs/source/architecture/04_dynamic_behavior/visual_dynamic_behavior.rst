.. _visual_dynamic_behavior:

Visual Dynamic Behavior Models
==============================

This section presents the dynamic behavior of Pulseengine (Kiln Edition) through visual models,
focusing on state machines, activity flows, and interaction patterns rather than code details.

Component Lifecycle State Machine
---------------------------------

**Purpose**: The component lifecycle manages the transition from raw WebAssembly bytes to an executing component instance.

**Design Rationale**:
- Type-safe state transitions prevent invalid operations
- Clear error states enable proper recovery
- Suspension support allows for resource management

.. uml::
   :caption: Complete Component Lifecycle State Machine

   @startuml
   !include _common.puml
   
   [*] --> Loaded : load_bytes()
   
   state Loaded {
     Loaded : Contains raw WASM bytes
     Loaded : No validation performed
   }
   
   Loaded --> Parsed : parse()
   Loaded --> ParseError : parse_failed()
   
   state Parsed {
     Parsed : Structure validated
     Parsed : Sections decoded
     Parsed : Types extracted
   }
   
   Parsed --> Instantiated : instantiate(imports)
   Parsed --> InstantiationError : instantiation_failed()
   
   state Instantiated {
     Instantiated : Imports resolved
     Instantiated : Memory allocated
     Instantiated : Exports available
   }
   
   Instantiated --> Running : start()
   Instantiated --> StartError : start_failed()
   
   state Running {
     Running : Executing functions
     Running : Processing requests
     Running : Managing resources
     
     Running --> Running : execute_function()
   }
   
   Running --> Suspended : suspend()
   Running --> Terminated : terminate()
   Running --> RuntimeError : execution_failed()
   
   state Suspended {
     Suspended : State preserved
     Suspended : Resources held
     Suspended : Not processing
   }
   
   Suspended --> Running : resume()
   Suspended --> Terminated : terminate()
   
   state Terminated {
     Terminated : Resources released
     Terminated : Memory freed
     Terminated : Cannot resume
   }
   
   state ErrorStates {
     ParseError : Invalid WASM format
     InstantiationError : Import resolution failed
     StartError : Initialization failed
     RuntimeError : Execution trapped
   }
   
   ParseError --> Terminated : cleanup()
   InstantiationError --> Terminated : cleanup()
   StartError --> Terminated : cleanup()
   RuntimeError --> Terminated : cleanup()
   RuntimeError --> Suspended : recover()
   
   Terminated --> [*]
   
   note right of Running
     Only state where
     functions can execute
   end note
   
   note left of Suspended
     Can be resumed
     or terminated
   end note
   @enduml

Execution Engine Activity Flow
------------------------------

**Purpose**: Shows how the execution engine processes WebAssembly instructions with budget management.

**Key Concepts**:
- Instruction execution is bounded by CPU budget
- Stack depth is monitored to prevent overflow
- Host calls transition to external execution

.. uml::
   :caption: Execution Engine Activity Diagram

   @startuml
   !include _common.puml
   
   start
   
   :Initialize Execution Context;
   :Set CPU Budget;
   :Clear Stack;
   
   while (Has Instructions?) is (yes)
     :Fetch Next Instruction;
     
     :Check CPU Budget;
     if (Budget Exceeded?) then (yes)
       :Suspend Execution;
       :Save Context;
       stop
     else (no)
     endif
     
     :Check Stack Depth;
     if (Stack Overflow?) then (yes)
       :Raise Stack Error;
       :Cleanup Stack;
       stop
     else (no)
     endif
     
     :Decode Instruction;
     
     switch (Instruction Type)
     case (Control Flow)
       :Update Program Counter;
       :Manage Block Stack;
     case (Memory Access)
       :Validate Memory Bounds;
       if (Out of Bounds?) then (yes)
         :Raise Memory Error;
         stop
       else (no)
         :Perform Memory Operation;
       endif
     case (Function Call)
       :Push Call Frame;
       if (Recursive Limit?) then (yes)
         :Raise Recursion Error;
         stop
       else (no)
         :Enter Function;
       endif
     case (Host Call)
       :Save WASM Context;
       :Execute Host Function;
       :Restore WASM Context;
     case (Arithmetic)
       :Pop Operands;
       :Perform Operation;
       :Push Result;
     endswitch
     
     :Update Instruction Counter;
     :Update Execution Time;
     
   endwhile (no)
   
   :Function Return;
   :Pop Result from Stack;
   
   stop
   @enduml

Memory Management State Machine
-------------------------------

**Purpose**: Tracks memory region lifecycle from allocation through deallocation.

**Design Principles**:
- Memory protection states prevent unauthorized access
- Explicit state transitions ensure proper cleanup
- Platform-specific features are abstracted

.. uml::
   :caption: Memory Region State Machine

   @startuml
   !include _common.puml
   
   [*] --> Unallocated
   
   state Unallocated {
     Unallocated : No memory reserved
     Unallocated : Zero resource usage
   }
   
   Unallocated --> Allocated : allocate(size)
   
   state Allocated {
     Allocated : Memory reserved
     Allocated : Not yet usable
     Allocated : May be unmapped
   }
   
   Allocated --> Initialized : initialize(pattern)
   Allocated --> Deallocated : deallocate()
   
   state Initialized {
     Initialized : Memory accessible
     Initialized : Read/Write allowed
     Initialized : Default protection
   }
   
   Initialized --> Mapped : map_to_component(id)
   Initialized --> Protected : protect(flags)
   Initialized --> Deallocated : deallocate()
   
   state Mapped {
     Mapped : Bound to component
     Mapped : Actively used
     Mapped : Access tracked
   }
   
   Mapped --> Protected : protect(flags)
   Mapped --> Initialized : unmap()
   
   state Protected {
     Protected : Access restricted
     Protected : May be read-only
     Protected : Or no-access
     
     state ProtectionLevels {
       ReadOnly : Write forbidden
       NoAccess : All access forbidden
       ExecuteOnly : Data access forbidden
     }
   }
   
   Protected --> Initialized : unprotect()
   Protected --> Deallocated : deallocate()
   
   state Deallocated {
     Deallocated : Memory released
     Deallocated : No longer valid
     Deallocated : Access forbidden
   }
   
   Deallocated --> [*]
   
   note right of Mapped
     Component owns this
     memory region
   end note
   
   note left of Protected
     Platform-specific
     protection mechanisms
   end note
   @enduml

Resource Allocation Sequence
----------------------------

**Purpose**: Shows how resources are allocated differently across environments.

.. uml::
   :caption: Multi-Environment Resource Allocation

   @startuml
   !include _common.puml
   
   actor Component
   participant "Resource Manager" as ResMgr
   participant "Allocation Strategy" as Strategy
   participant "Memory Backend" as Backend
   
   == Allocation Request ==
   Component -> ResMgr: allocate<FileHandle>()
   ResMgr -> ResMgr: determine_environment()
   
   alt std environment
     ResMgr -> Strategy: use_dynamic_strategy()
     Strategy -> Backend: HashMap::insert()
     Backend --> Strategy: Ok(slot)
   else no_std + alloc
     ResMgr -> Strategy: use_btree_strategy()
     Strategy -> Backend: BTreeMap::insert()
     Backend --> Strategy: Ok(slot)
   else no_std + no_alloc
     ResMgr -> Strategy: use_pool_strategy()
     Strategy -> Backend: check_pool_capacity()
     alt pool has space
       Backend --> Strategy: Ok(slot_index)
       Strategy -> Backend: pool[slot_index] = resource
     else pool full
       Backend --> Strategy: Err(PoolExhausted)
       Strategy --> ResMgr: Err(AllocationFailed)
       ResMgr --> Component: Err(ResourceError)
     end
   end
   
   Strategy --> ResMgr: Ok(ResourceId)
   ResMgr -> ResMgr: track_allocation(id)
   ResMgr --> Component: Ok(ResourceId)
   
   note over Strategy
     Strategy selection based on
     compile-time features
   end note
   @enduml

Concurrent Component Execution
------------------------------

**Purpose**: Illustrates how multiple components interact with shared resources.

**Concurrency Model**:
- Components execute independently
- Shared resources require synchronization
- Platform determines actual parallelism

.. uml::
   :caption: Concurrent Component Interaction

   @startuml
   !include _common.puml
   
   box "Component A Context" #LightBlue
     participant "Component A" as CompA
     participant "Stack A" as StackA
   end box
   
   participant "Scheduler" as Sched
   participant "Shared Memory" as SharedMem
   participant "Lock Manager" as LockMgr
   
   box "Component B Context" #LightGreen
     participant "Component B" as CompB
     participant "Stack B" as StackB
   end box
   
   == Concurrent Execution ==
   
   par Component A execution
     CompA -> StackA: push_frame()
     CompA -> Sched: request_time_slice()
     Sched --> CompA: granted(100ms)
     
     CompA -> SharedMem: read_shared_data()
     SharedMem -> LockMgr: acquire_read_lock()
     LockMgr --> SharedMem: lock_granted
     SharedMem --> CompA: data
     
     CompA -> CompA: process_data()
     
   and Component B execution
     CompB -> StackB: push_frame()
     CompB -> Sched: request_time_slice()
     Sched --> CompB: granted(100ms)
     
     CompB -> SharedMem: write_shared_data()
     SharedMem -> LockMgr: acquire_write_lock()
     
     alt read lock held
       LockMgr --> SharedMem: wait
       note right: Component B blocks
       ... wait for read lock release ...
       LockMgr --> SharedMem: lock_granted
     else no locks held
       LockMgr --> SharedMem: lock_granted
     end
     
     SharedMem --> CompB: write_complete
   end
   
   CompA -> SharedMem: release_read_lock()
   SharedMem -> LockMgr: release_lock()
   
   note over LockMgr
     Prevents data races
     May cause blocking
   end note
   @enduml

Error Recovery Flow
-------------------

**Purpose**: Shows how errors are handled and recovery is attempted.

.. uml::
   :caption: Error Handling and Recovery Activity

   @startuml
   !include _common.puml
   
   start
   
   :Execute Component Function;
   
   if (Error Occurred?) then (yes)
     :Capture Error Context;
     :Save Execution State;
     
     switch (Error Type)
     case (Memory Error)
       :Log Memory Violation;
       if (Recoverable?) then (yes)
         :Reset Memory State;
         :Retry Operation;
       else (no)
         :Mark Component Failed;
       endif
       
     case (Stack Overflow)
       :Unwind Stack;
       :Clear Call Frames;
       if (Component Critical?) then (yes)
         :Attempt Restart;
       else (no)
         :Terminate Component;
       endif
       
     case (Resource Exhaustion)
       :Release Unused Resources;
       :Run Garbage Collection;
       if (Resources Available?) then (yes)
         :Retry Allocation;
       else (no)
         :Queue for Later;
       endif
       
     case (Host Function Error)
       :Check Error Code;
       if (Transient Error?) then (yes)
         :Exponential Backoff;
         :Retry Host Call;
       else (no)
         :Propagate Error;
       endif
     endswitch
     
     if (Recovery Successful?) then (yes)
       :Resume Execution;
     else (no)
       :Build Error Chain;
       :Notify Caller;
       :Cleanup Resources;
       stop
     endif
   else (no)
     :Continue Execution;
   endif
   
   :Return Result;
   
   stop
   @enduml

Platform-Specific Behavior
--------------------------

**Purpose**: Illustrates how behavior adapts to different platform capabilities.

.. uml::
   :caption: Platform Adaptation State Machine

   @startuml
   !include _common.puml
   
   state "Platform Detection" as Detect {
     [*] --> CheckOS
     CheckOS --> Linux : is_linux()
     CheckOS --> MacOS : is_macos()
     CheckOS --> QNX : is_qnx()
     CheckOS --> Embedded : is_embedded()
   }
   
   state "Linux Platform" as Linux {
     state "Memory Management" as LinuxMem {
       [*] --> MMap
       MMap : Use mmap/munmap
       MMap : Support huge pages
       MMap : NUMA awareness
     }
     
     state "Synchronization" as LinuxSync {
       [*] --> Futex
       Futex : Fast userspace mutex
       Futex : Kernel arbitration
     }
   }
   
   state "Embedded Platform" as Embedded {
     state "Memory Management" as EmbedMem {
       [*] --> StaticAlloc
       StaticAlloc : Fixed memory regions
       StaticAlloc : No dynamic allocation
       StaticAlloc : Compile-time layout
     }
     
     state "Synchronization" as EmbedSync {
       [*] --> DisableInterrupts
       DisableInterrupts : Critical sections
       DisableInterrupts : No OS support
     }
   }
   
   Detect --> Linux
   Detect --> MacOS
   Detect --> QNX
   Detect --> Embedded
   
   Linux --> RuntimeBehavior
   MacOS --> RuntimeBehavior
   QNX --> RuntimeBehavior
   Embedded --> RuntimeBehavior
   
   state "Runtime Behavior" as RuntimeBehavior {
     RuntimeBehavior : Adapted to platform
     RuntimeBehavior : Same API surface
     RuntimeBehavior : Different performance
   }
   
   note right of Linux
     Full OS features
     Dynamic allocation
     Thread support
   end note
   
   note right of Embedded
     No OS features
     Static allocation
     Interrupt-based
   end note
   @enduml

Performance Characteristics
---------------------------

**Purpose**: Visual representation of performance trade-offs across environments.

.. uml::
   :caption: Performance Characteristics by Environment

   @startuml
   !include _common.puml
   
   package "Performance Metrics" {
     
     object "std Environment" as Std {
       allocation_speed = "Fast (malloc)"
       allocation_predictability = "Variable"
       memory_overhead = "8-16 bytes/alloc"
       max_memory = "System limit"
       concurrent_components = "Unlimited"
       synchronization = "OS primitives"
     }
     
     object "no_std + alloc" as NoStdAlloc {
       allocation_speed = "Fast (custom)"
       allocation_predictability = "Variable"
       memory_overhead = "4-8 bytes/alloc"
       max_memory = "Configured limit"
       concurrent_components = "Platform dependent"
       synchronization = "Platform specific"
     }
     
     object "no_std + no_alloc" as NoAlloc {
       allocation_speed = "Instant (pre-allocated)"
       allocation_predictability = "Deterministic"
       memory_overhead = "0 bytes/alloc"
       max_memory = "Compile-time fixed"
       concurrent_components = "Fixed count"
       synchronization = "Spin locks/disabled"
     }
     
   }
   
   note bottom of Std
     Best for: Desktop/Server
     Trade-off: Memory for flexibility
   end note
   
   note bottom of NoStdAlloc
     Best for: Embedded Linux/QNX
     Trade-off: Some flexibility
   end note
   
   note bottom of NoAlloc
     Best for: Hard real-time
     Trade-off: Flexibility for predictability
   end note
   @enduml

Summary
-------

These visual models provide:

1. **Clear State Transitions**: Shows valid state changes and error paths
2. **Behavioral Patterns**: Illustrates how components interact
3. **Environment Adaptation**: Demonstrates multi-environment support
4. **Error Handling**: Shows recovery and cleanup flows
5. **Performance Trade-offs**: Visualizes environment-specific characteristics

The key advantage is these diagrams can be automatically validated against the implementation
through state machine annotations and behavioral tests, ensuring they remain accurate as
the codebase evolves.