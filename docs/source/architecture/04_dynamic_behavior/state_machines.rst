.. _state_machines:

State Machines and Lifecycle Management
========================================

This section documents the state machines that govern component lifecycle and runtime behavior
in Pulseengine (Kiln Edition), showing how state transitions work across different environments.

.. arch_component:: ARCH_COMP_STATE_001
   :title: Component Lifecycle State Machine
   :status: implemented
   :version: 1.0
   :rationale: Ensure consistent component lifecycle management across environments

   State machine that governs component instantiation, execution, and cleanup
   across std, no_std+alloc, and no_std+no_alloc environments.

Component Lifecycle State Machine
----------------------------------

State Definitions
~~~~~~~~~~~~~~~~~

From ``kiln-component/src/component.rs:89-156``:

.. code-block:: rust

   /// Component lifecycle states
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum ComponentState {
       /// Component bytes loaded but not parsed
       Loaded,
       /// Component parsed and validated
       Parsed,
       /// Component instantiated with imports resolved
       Instantiated,
       /// Component ready for function execution
       Running,
       /// Component suspended (can be resumed)
       Suspended,
       /// Component terminated (cannot be resumed)
       Terminated,
       /// Component in error state
       Error(ComponentErrorCode),
   }

   /// Component with type-state tracking
   pub struct Component<S = ComponentState> {
       inner: ComponentInner,
       state: S,
   }

   /// Type-safe state transitions
   pub struct Loaded;
   pub struct Parsed;
   pub struct Instantiated;
   pub struct Running;
   pub struct Suspended;
   pub struct Terminated;

**State Transition Diagram**:

.. uml::
   :caption: Component Lifecycle State Machine

   @startuml
   [*] --> Loaded : load_bytes()
   Loaded --> Parsed : parse()
   Loaded --> Error : parse_error
   Parsed --> Instantiated : instantiate()
   Parsed --> Error : instantiation_error
   Instantiated --> Running : start()
   Instantiated --> Error : start_error
   Running --> Suspended : suspend()
   Running --> Terminated : terminate()
   Running --> Error : execution_error
   Suspended --> Running : resume()
   Suspended --> Terminated : terminate()
   Error --> Terminated : cleanup()
   Terminated --> [*]
   @enduml

State Transition Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Type-safe state transitions (``kiln-component/src/component.rs:178-245``):

.. code-block:: rust

   impl Component<Loaded> {
       /// Create a new component from bytes
       pub fn from_bytes(bytes: &[u8]) -> Result<Self, ComponentError> {
           let inner = ComponentInner::from_bytes(bytes)?;
           Ok(Component {
               inner,
               state: Loaded,
           })
       }
       
       /// Parse the component (Loaded -> Parsed)
       pub fn parse(self) -> Result<Component<Parsed>, ComponentError> {
           let parsed_inner = self.inner.parse()?;
           Ok(Component {
               inner: parsed_inner,
               state: Parsed,
           })
       }
   }

   impl Component<Parsed> {
       /// Instantiate the component (Parsed -> Instantiated)
       pub fn instantiate(
           self, 
           imports: &ImportMap
       ) -> Result<Component<Instantiated>, ComponentError> {
           let instantiated_inner = self.inner.instantiate(imports)?;
           Ok(Component {
               inner: instantiated_inner,
               state: Instantiated,
           })
       }
   }

   impl Component<Instantiated> {
       /// Start the component (Instantiated -> Running)
       pub fn start(self) -> Result<Component<Running>, ComponentError> {
           let running_inner = self.inner.start()?;
           Ok(Component {
               inner: running_inner,
               state: Running,
           })
       }
   }

   impl Component<Running> {
       /// Execute a function (only available in Running state)
       pub fn execute(
           &mut self, 
           function: &str, 
           args: &[ComponentValue]
       ) -> Result<ComponentValue, ExecutionError> {
           self.inner.execute(function, args)
       }
       
       /// Suspend the component (Running -> Suspended)
       pub fn suspend(self) -> Result<Component<Suspended>, ComponentError> {
           let suspended_inner = self.inner.suspend()?;
           Ok(Component {
               inner: suspended_inner,
               state: Suspended,
           })
       }
   }

Runtime Execution State Machine
-------------------------------

Execution Engine States
~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-runtime/src/execution.rs:123-189``:

.. code-block:: rust

   /// Runtime execution states
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum ExecutionState {
       /// Runtime idle, ready to execute
       Idle,
       /// Executing a function
       Executing {
           component_id: ComponentId,
           function_name: FunctionName,
           depth: usize,
       },
       /// Waiting for host function callback
       HostCall {
           host_function: HostFunctionId,
           callback_data: CallbackData,
       },
       /// Suspended execution (can be resumed)
       Suspended {
           checkpoint: ExecutionCheckpoint,
       },
       /// Execution completed successfully
       Completed {
           result: ComponentValue,
       },
       /// Execution failed with error
       Failed {
           error: ExecutionError,
           recovery_point: Option<ExecutionCheckpoint>,
       },
   }

   /// Execution context with state management
   pub struct ExecutionContext {
       state: ExecutionState,
       stack: ExecutionStack,
       #[cfg(any(feature = "std", feature = "alloc"))]
       call_history: BoundedVec<CallFrame>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       call_history: heapless::Vec<CallFrame, 256>,
   }

**Execution State Diagram**:

.. uml::
   :caption: Runtime Execution State Machine

   @startuml
   [*] --> Idle
   Idle --> Executing : execute_function()
   Executing --> HostCall : host_function_call()
   Executing --> Completed : function_return()
   Executing --> Suspended : yield_execution()
   Executing --> Failed : execution_error()
   HostCall --> Executing : host_callback_complete()
   HostCall --> Failed : host_callback_error()
   Suspended --> Executing : resume_execution()
   Suspended --> Failed : resume_error()
   Completed --> Idle : reset()
   Failed --> Idle : reset()
   Failed --> [*] : terminate()
   @enduml

State Transition Guards and Actions
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

From ``kiln-runtime/src/execution.rs:234-289``:

.. code-block:: rust

   impl ExecutionContext {
       /// Execute function with state transition validation
       pub fn execute_function(
           &mut self,
           component_id: ComponentId,
           function_name: &str,
           args: &[ComponentValue],
       ) -> Result<ComponentValue, ExecutionError> {
           // Guard: Must be in Idle state
           match self.state {
               ExecutionState::Idle => {}
               _ => return Err(ExecutionError::InvalidState {
                   current: self.state,
                   expected: ExecutionState::Idle,
               }),
           }
           
           // Transition to Executing state
           self.state = ExecutionState::Executing {
               component_id,
               function_name: function_name.into(),
               depth: 0,
           };
           
           // Execute with error handling
           match self.execute_internal(component_id, function_name, args) {
               Ok(result) => {
                   // Transition to Completed state
                   self.state = ExecutionState::Completed { result: result.clone() };
                   Ok(result)
               }
               Err(error) => {
                   // Transition to Failed state
                   self.state = ExecutionState::Failed {
                       error: error.clone(),
                       recovery_point: self.create_checkpoint(),
                   };
                   Err(error)
               }
           }
       }
       
       /// Suspend execution with checkpoint creation
       pub fn suspend(&mut self) -> Result<ExecutionCheckpoint, ExecutionError> {
           match self.state {
               ExecutionState::Executing { .. } => {
                   let checkpoint = self.create_checkpoint()?;
                   self.state = ExecutionState::Suspended {
                       checkpoint: checkpoint.clone(),
                   };
                   Ok(checkpoint)
               }
               _ => Err(ExecutionError::InvalidState {
                   current: self.state,
                   expected: ExecutionState::Executing { 
                       component_id: ComponentId(0), 
                       function_name: "any".into(),
                       depth: 0,
                   },
               }),
           }
       }
   }

Memory Management State Machine
-------------------------------

Memory Region States
~~~~~~~~~~~~~~~~~~~~

From ``kiln-foundation/src/safe_memory.rs:189-256``:

.. code-block:: rust

   /// Memory region state
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum MemoryState {
       /// Memory allocated but not initialized
       Allocated {
           size: usize,
       },
       /// Memory initialized and ready for use
       Initialized {
           size: usize,
           protection: MemoryProtection,
       },
       /// Memory mapped to component
       Mapped {
           component_id: ComponentId,
           base_address: usize,
           size: usize,
       },
       /// Memory protected (read-only or no-access)
       Protected {
           protection: MemoryProtection,
       },
       /// Memory deallocated (invalid)
       Deallocated,
   }

   /// Memory region with state tracking
   pub struct ManagedMemory {
       region: MemoryRegion,
       state: MemoryState,
       #[cfg(any(feature = "std", feature = "alloc"))]
       metadata: BoundedMap<BoundedString, BoundedString>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       metadata: heapless::FnvIndexMap<&'static str, &'static str, 16>,
   }

**Memory State Transitions**:

.. uml::
   :caption: Memory Management State Machine

   @startuml
   [*] --> Allocated : allocate()
   Allocated --> Initialized : initialize()
   Allocated --> Deallocated : deallocate()
   Initialized --> Mapped : map_to_component()
   Initialized --> Protected : set_protection()
   Initialized --> Deallocated : deallocate()
   Mapped --> Protected : protect()
   Mapped --> Initialized : unmap()
   Protected --> Initialized : unprotect()
   Protected --> Deallocated : deallocate()
   Deallocated --> [*]
   @enduml

Resource Lifecycle State Machine
---------------------------------

Resource States
~~~~~~~~~~~~~~~

From ``kiln-component/src/resources/resource_manager.rs:145-234``:

.. code-block:: rust

   /// Resource lifecycle states
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum ResourceState {
       /// Resource allocated but not initialized
       Allocated {
           resource_id: ResourceId,
           resource_type: ResourceType,
       },
       /// Resource initialized and ready for use
       Ready {
           resource_id: ResourceId,
           last_access: Timestamp,
       },
       /// Resource currently in use
       Active {
           resource_id: ResourceId,
           owner: ComponentId,
           access_count: usize,
       },
       /// Resource temporarily unavailable
       Locked {
           resource_id: ResourceId,
           lock_holder: ComponentId,
       },
       /// Resource marked for cleanup
       Cleanup {
           resource_id: ResourceId,
           cleanup_reason: CleanupReason,
       },
       /// Resource deallocated
       Deallocated {
           resource_id: ResourceId,
           deallocation_time: Timestamp,
       },
   }

   /// Resource manager with state tracking
   pub struct ResourceManager {
       #[cfg(any(feature = "std", feature = "alloc"))]
       resources: BoundedMap<ResourceId, ResourceEntry>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       resources: heapless::FnvIndexMap<ResourceId, ResourceEntry, 256>,
       state_transitions: StateTransitionLog,
   }

Environment-Specific State Adaptations
--------------------------------------

State Storage Patterns
~~~~~~~~~~~~~~~~~~~~~~

Different environments use different state storage strategies:

.. code-block:: rust

   /// Environment-adaptive state storage
   pub struct StateManager {
       #[cfg(feature = "std")]
       state_history: std::collections::VecDeque<StateTransition>,
       
       #[cfg(all(not(feature = "std"), feature = "alloc"))]
       state_history: alloc::collections::VecDeque<StateTransition>,
       
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       state_history: heapless::Deque<StateTransition, 64>,
   }

   impl StateManager {
       /// Record state transition (works in all environments)
       pub fn record_transition(
           &mut self,
           from: ComponentState,
           to: ComponentState,
           trigger: StateTransitionTrigger,
       ) -> Result<(), StateError> {
           let transition = StateTransition {
               from,
               to,
               trigger,
               timestamp: self.get_timestamp(),
           };
           
           #[cfg(any(feature = "std", feature = "alloc"))]
           {
               self.state_history.push_back(transition);
               // Dynamic storage can grow as needed
           }
           
           #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
           {
               // Fixed storage requires overflow handling
               if self.state_history.is_full() {
                   let _ = self.state_history.pop_front(); // Remove oldest
               }
               self.state_history.push_back(transition)
                   .map_err(|_| StateError::HistoryFull)?;
           }
           
           Ok(())
       }
   }

State Persistence
~~~~~~~~~~~~~~~~~

State can be persisted across runtime restarts:

.. code-block:: rust

   /// State checkpoint for persistence
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct StateCheckpoint {
       pub component_states: BoundedVec<(ComponentId, ComponentState)>,
       pub execution_state: ExecutionState,
       pub memory_state: BoundedVec<(MemoryRegion, MemoryState)>,
       pub resource_states: BoundedVec<(ResourceId, ResourceState)>,
       pub timestamp: u64,
   }

   impl StateCheckpoint {
       /// Create checkpoint from current runtime state
       pub fn create(runtime: &Runtime) -> Result<Self, CheckpointError> {
           Ok(StateCheckpoint {
               component_states: runtime.get_component_states()?,
               execution_state: runtime.get_execution_state(),
               memory_state: runtime.get_memory_states()?,
               resource_states: runtime.get_resource_states()?,
               timestamp: runtime.get_timestamp(),
           })
       }
       
       /// Restore runtime state from checkpoint
       pub fn restore(&self, runtime: &mut Runtime) -> Result<(), RestoreError> {
           runtime.restore_component_states(&self.component_states)?;
           runtime.restore_execution_state(self.execution_state)?;
           runtime.restore_memory_states(&self.memory_state)?;
           runtime.restore_resource_states(&self.resource_states)?;
           Ok(())
       }
   }

State Machine Verification
---------------------------

Invariant Checking
~~~~~~~~~~~~~~~~~~

State machines include invariant checking to ensure correctness:

.. code-block:: rust

   /// State invariant validation
   impl Component<Running> {
       fn validate_state_invariants(&self) -> Result<(), InvariantViolation> {
           // Invariant: Running components must have valid memory
           if self.inner.memory.is_none() {
               return Err(InvariantViolation::MissingMemory {
                   component_id: self.inner.id,
                   state: ComponentState::Running,
               });
           }
           
           // Invariant: Running components must have resolved imports
           if !self.inner.imports_resolved {
               return Err(InvariantViolation::UnresolvedImports {
                   component_id: self.inner.id,
                   state: ComponentState::Running,
               });
           }
           
           // Invariant: Running components must have valid export table
           if self.inner.exports.is_empty() {
               return Err(InvariantViolation::EmptyExports {
                   component_id: self.inner.id,
                   state: ComponentState::Running,
               });
           }
           
           Ok(())
       }
   }

Deadlock Prevention
~~~~~~~~~~~~~~~~~~

State transitions include deadlock prevention mechanisms:

.. code-block:: rust

   /// Deadlock prevention in state transitions
   pub struct DeadlockDetector {
       #[cfg(any(feature = "std", feature = "alloc"))]
       dependency_graph: BoundedMap<ComponentId, BoundedVec<ComponentId>>,
       #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
       dependency_graph: heapless::FnvIndexMap<ComponentId, heapless::Vec<ComponentId, 16>, 64>,
   }

   impl DeadlockDetector {
       /// Check for potential deadlock before state transition
       pub fn check_deadlock_potential(
           &self,
           component: ComponentId,
           target_state: ComponentState,
       ) -> Result<(), DeadlockError> {
           match target_state {
               ComponentState::Suspended => {
                   // Check if suspending this component would create deadlock
                   if self.would_create_deadlock(component) {
                       return Err(DeadlockError::PotentialDeadlock {
                           component,
                           blocking_components: self.get_blocking_components(component),
                       });
                   }
               }
               _ => {}
           }
           Ok(())
       }
   }

Cross-Environment Testing
-------------------------

State Machine Test Framework
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

State machines are tested across all environments:

.. code-block:: rust

   // Test from tests/state_machine_test.rs
   #[test]
   fn test_component_lifecycle_all_environments() {
       fn test_lifecycle_impl() -> Result<(), ComponentError> {
           // This test works in all environments
           let component = Component::from_bytes(SAMPLE_WASM)?;
           let parsed = component.parse()?;
           let instantiated = parsed.instantiate(&ImportMap::new())?;
           let mut running = instantiated.start()?;
           
           // Test state transitions
           let result = running.execute("test_function", &[])?;
           assert_eq!(result, ComponentValue::I32(42));
           
           // Test suspension and resumption
           let suspended = running.suspend()?;
           let resumed = suspended.resume()?;
           
           Ok(())
       }
       
       // Test in current environment
       test_lifecycle_impl().unwrap();
   }

Cross-References
-----------------

.. seealso::

   * :doc:`interaction_flows` for component interaction patterns
   * :doc:`sequence_diagrams` for detailed execution sequences
   * :doc:`../01_architectural_design/patterns` for state management patterns
   * :doc:`../05_resource_management/resource_overview` for resource state management