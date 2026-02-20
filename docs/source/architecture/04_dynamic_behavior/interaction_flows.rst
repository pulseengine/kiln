==========================
Component Interaction Flows
==========================

**Teaching Point**: This section shows how components work together at runtime. Understanding these flows is crucial for debugging and extending the system.

Module Loading and Instantiation
--------------------------------

This flow shows how a WebAssembly module is loaded and prepared for execution:

.. uml::

   @startuml
   actor "Host Application" as Host
   participant "Kiln Facade" as Kiln
   participant "Decoder" as DEC
   participant "Runtime" as RT
   participant "Memory Manager" as MM
   participant "Platform Layer" as PL
   
   Host -> Kiln: load_module(bytes)
   activate Kiln
   
   Kiln -> DEC: decode_module(bytes)
   activate DEC
   note right: Validates binary format
   DEC -> DEC: parse_sections()
   DEC -> DEC: validate_structure()
   DEC --> Kiln: Module
   deactivate DEC
   
   Kiln -> RT: create_instance(module)
   activate RT
   
   RT -> MM: allocate_memory(size)
   activate MM
   
   alt std environment
       MM -> MM: Vec::with_capacity(size)
   else no_std + alloc
       MM -> MM: alloc::vec![0; size]
   else no_std + no_alloc
       MM -> MM: BoundedVec<u8, MAX_SIZE>::new()
   end
   
   MM -> PL: allocate_pages(count)
   activate PL
   PL --> MM: memory_ptr
   deactivate PL
   
   MM --> RT: Memory
   deactivate MM
   
   RT -> RT: initialize_globals()
   RT -> RT: initialize_tables()
   RT --> Kiln: Instance
   deactivate RT
   
   Kiln --> Host: InstanceHandle
   deactivate Kiln
   @enduml

**Key Decision Points**:

1. **Memory Allocation**: Based on environment features
2. **Validation Level**: Configurable per deployment
3. **Platform Calls**: OS-specific optimizations

Function Execution Flow
-----------------------

**Teaching Point**: This shows the actual execution path for WebAssembly functions.

.. uml::

   @startuml
   actor "Host" as H
   participant "Instance" as I
   participant "Engine" as E
   participant "Stack" as S
   participant "Instructions" as INS
   participant "Memory" as M
   
   H -> I: invoke("function", args)
   activate I
   
   I -> E: execute(func_idx, args)
   activate E
   
   E -> S: push_frame(func)
   activate S
   
   alt no_std + no_alloc
       S -> S: BoundedStack::push()
       note right: Fixed capacity check
   else dynamic allocation
       S -> S: Vec::push()
   end
   
   S --> E: frame_ref
   deactivate S
   
   E -> E: push_args(args)
   
   loop for each instruction
       E -> INS: decode_instruction()
       INS -> INS: execute()
       
       alt Memory Operation
           INS -> M: load/store(addr)
           M -> M: bounds_check(addr)
           M --> INS: value
       else Control Flow
           INS -> E: branch(target)
       else Arithmetic
           INS -> S: pop_values()
           INS -> INS: compute()
           INS -> S: push_result()
       end
   end
   
   E -> S: pop_frame()
   E --> I: results
   deactivate E
   
   I --> H: Vec<Value>
   deactivate I
   @enduml

Component Model Instantiation
-----------------------------

**Teaching Point**: Component Model adds another layer of abstraction with imports/exports.

.. uml::

   @startuml
   participant "Host" as H
   participant "Component Runtime" as CR
   participant "Component Instance" as CI
   participant "Runtime Core" as RC
   participant "Type Registry" as TR
   
   H -> CR: instantiate_component(component_def)
   activate CR
   
   CR -> TR: validate_types(component_def.types)
   activate TR
   TR -> TR: check_compatibility()
   TR --> CR: validation_result
   deactivate TR
   
   CR -> CI: new(component_def)
   activate CI
   
   loop for each import
       CI -> H: resolve_import(name)
       H --> CI: import_instance
       CI -> CI: validate_import_type()
   end
   
   CI -> RC: create_core_instances()
   activate RC
   
   loop for each module
       RC -> RC: instantiate_module()
       RC -> RC: link_imports()
   end
   
   RC --> CI: core_instances
   deactivate RC
   
   CI -> CI: wire_components()
   CI --> CR: instance
   deactivate CI
   
   CR --> H: ComponentInstance
   deactivate CR
   @enduml

Memory Growth Handling
----------------------

**Teaching Point**: Memory growth is handled differently in each environment.

.. uml::

   @startuml
   participant "WASM Code" as W
   participant "Memory Instruction" as MI
   participant "Memory Manager" as MM
   participant "Platform Layer" as PL
   
   W -> MI: memory.grow(delta)
   activate MI
   
   MI -> MM: grow_memory(delta_pages)
   activate MM
   
   alt std environment
       MM -> MM: vec.resize(new_size)
       note right: Can use OS virtual memory
   else no_std + alloc
       MM -> MM: realloc_vec(new_size)
   else no_std + no_alloc
       MM -> MM: check_static_limit()
       alt within bounds
           MM -> MM: update_used_size
       else exceeds capacity
           MM --> MI: Error::OutOfMemory
       end
   end
   
   alt growth succeeded
       MM -> PL: remap_pages(old_ptr, new_size)
       PL --> MM: new_ptr
       MM --> MI: old_page_count
   else growth failed
       MM --> MI: -1 (failure)
   end
   
   deactivate MM
   MI --> W: result
   deactivate MI
   @enduml

Inter-Component Communication
-----------------------------

Shows how components communicate through the Component Model:

.. uml::

   @startuml
   participant "Component A" as A
   participant "Canonical ABI" as CABI
   participant "Type Converter" as TC
   participant "Component B" as B
   
   A -> CABI: call_export("func", args)
   activate CABI
   
   CABI -> TC: lower_values(args)
   activate TC
   TC -> TC: convert_to_canonical()
   TC --> CABI: canonical_args
   deactivate TC
   
   CABI -> B: invoke_import("func", canonical_args)
   activate B
   B -> B: execute_function()
   B --> CABI: canonical_results
   deactivate B
   
   CABI -> TC: lift_values(canonical_results)
   activate TC
   TC -> TC: convert_from_canonical()
   TC --> CABI: results
   deactivate TC
   
   CABI --> A: results
   deactivate CABI
   @enduml

Error Propagation Flow
----------------------

**Teaching Point**: Errors are handled consistently across all layers.

.. uml::

   @startuml
   participant "Instruction" as I
   participant "Runtime" as R
   participant "Error Handler" as EH
   participant "Host" as H
   
   I -> I: execute()
   I -> I: detect_error()
   
   I -> EH: create_error(ErrorKind::DivByZero)
   activate EH
   EH -> EH: add_context("instruction", "i32.div")
   EH -> EH: add_context("pc", "0x1234")
   EH --> I: Error
   deactivate EH
   
   I --> R: Err(error)
   R -> R: unwind_stack()
   R -> EH: wrap_error(RuntimeError)
   R --> H: Err(wrapped_error)
   
   H -> H: handle_error()
   alt development mode
       H -> H: log_full_trace()
   else production mode  
       H -> H: log_summary()
   end
   @enduml

Cross-References
----------------

- **State Machines**: See :doc:`state_machines` for component lifecycles
- **Sequence Details**: See :doc:`sequence_diagrams` for detailed timing
- **Implementation**: See actual code in ``kiln-runtime/src/execution.rs``