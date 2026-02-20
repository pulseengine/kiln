.. _interface_architecture:

Interface Architecture with Visual Models
=========================================

This section presents the interface architecture of Pulseengine (Kiln Edition) using visual models
and PlantUML diagrams, focusing on the architectural design rather than implementation details.

Interface Layer Overview
------------------------

.. uml::
   :caption: High-Level Interface Architecture

   @startuml
   !include _common.puml
   
   package "External Layer" {
     interface "Runtime API" as RuntimeAPI
     interface "CLI Interface" as CLI
     interface "Host Functions" as HostAPI
   }
   
   package "Component Layer" {
     interface "Component API" as CompAPI
     interface "Resource Manager" as ResManager
     interface "Memory Manager" as MemManager
   }
   
   package "Foundation Layer" {
     interface "Memory Provider" as MemProvider
     interface "Type System" as TypeSystem
     interface "Error System" as ErrorSystem
   }
   
   package "Platform Layer" {
     interface "Platform Provider" as PlatformProvider
     interface "Sync Provider" as SyncProvider
   }
   
   RuntimeAPI ..> CompAPI : uses
   RuntimeAPI ..> MemManager : uses
   CompAPI ..> ResManager : uses
   CompAPI ..> MemProvider : uses
   ResManager ..> MemProvider : uses
   MemManager ..> MemProvider : implements
   MemProvider ..> PlatformProvider : uses
   
   note right of RuntimeAPI
     Primary external interface
     for WebAssembly execution
   end note
   
   note bottom of PlatformProvider
     Abstracts OS-specific
     functionality
   end note
   @enduml

Memory Provider Interface Architecture
--------------------------------------

**Purpose**: The Memory Provider interface is the cornerstone of memory management across all environments.

**Design Rationale**: 
- Provides a unified abstraction over different memory implementations
- Enables compile-time selection of memory strategy based on environment
- Maintains safety through bounds checking and error handling

.. uml::
   :caption: Memory Provider Interface Design

   @startuml
   !include _common.puml
   
   interface MemoryProvider {
     +len(): usize
     +is_empty(): bool
     +read_bytes(offset: usize, length: usize): Result<&[u8]>
     +write_bytes(offset: usize, data: &[u8]): Result<()>
     +as_slice(): &[u8]
     +as_mut_slice(): &mut [u8]
   }
   
   class StandardMemory {
     -data: Vec<u8>
     -max_size: Option<usize>
   }
   
   class BoundedMemory {
     -data: [u8; 65536]
     -size: usize
   }
   
   class MappedMemory {
     -base_ptr: *mut u8
     -size: usize
     -protection: MemoryProtection
   }
   
   MemoryProvider <|.. StandardMemory : implements
   MemoryProvider <|.. BoundedMemory : implements
   MemoryProvider <|.. MappedMemory : implements
   
   note top of MemoryProvider
     Core abstraction for all
     memory operations
   end note
   
   note left of StandardMemory
     std environment:
     Dynamic allocation
   end note
   
   note right of BoundedMemory
     no_alloc environment:
     Fixed-size allocation
   end note
   
   note bottom of MappedMemory
     Platform-specific:
     Memory-mapped regions
   end note
   @enduml

**Interface Contracts**:

.. uml::
   :caption: Memory Provider Contract Enforcement

   @startuml
   !include _common.puml
   
   participant Client
   participant MemoryProvider
   participant BoundsChecker
   participant Storage
   
   == Read Operation ==
   Client -> MemoryProvider: read_bytes(offset, length)
   MemoryProvider -> BoundsChecker: check_bounds(offset, length, total_size)
   
   alt bounds valid
     BoundsChecker --> MemoryProvider: Ok
     MemoryProvider -> Storage: get_slice(offset, length)
     Storage --> MemoryProvider: &[u8]
     MemoryProvider --> Client: Ok(&[u8])
   else bounds invalid
     BoundsChecker --> MemoryProvider: Error
     MemoryProvider --> Client: Err(OutOfBounds)
   end
   
   == Write Operation ==
   Client -> MemoryProvider: write_bytes(offset, data)
   MemoryProvider -> BoundsChecker: check_bounds(offset, data.len(), total_size)
   
   alt bounds valid
     BoundsChecker --> MemoryProvider: Ok
     MemoryProvider -> Storage: copy_from_slice(offset, data)
     Storage --> MemoryProvider: success
     MemoryProvider --> Client: Ok(())
   else bounds invalid
     BoundsChecker --> MemoryProvider: Error
     MemoryProvider --> Client: Err(OutOfBounds)
   end
   @enduml

Component Instance Interface
----------------------------

**Purpose**: Defines how components are instantiated and managed throughout their lifecycle.

**Design Rationale**:
- Separates component metadata from execution state
- Enables different storage strategies for different environments
- Provides clear lifecycle boundaries

.. uml::
   :caption: Component Instance Interface Architecture

   @startuml
   !include _common.puml
   
   interface ComponentInstance {
     +metadata(): &ComponentMetadata
     +execute(function: &str, args: &[Value]): Result<Value>
     +exports(): &ExportTable
     +imports(): &ImportTable
     +memory(): Option<&MemoryProvider>
     +memory_mut(): Option<&mut MemoryProvider>
   }
   
   class ComponentMetadata {
     +id: ComponentId
     +name: String
     +version: Version
     +capabilities: Capabilities
   }
   
   class ExportTable {
     +functions: Map<String, FunctionExport>
     +memories: Map<String, MemoryExport>
     +tables: Map<String, TableExport>
     +globals: Map<String, GlobalExport>
   }
   
   class ImportTable {
     +functions: Map<String, FunctionImport>
     +memories: Map<String, MemoryImport>
     +required: Set<ImportRequirement>
   }
   
   abstract class ComponentStorage {
     {abstract} +store_component(id: ComponentId, component: Component)
     {abstract} +get_component(id: ComponentId): Option<&Component>
     {abstract} +remove_component(id: ComponentId): Option<Component>
   }
   
   class DynamicStorage {
     -components: HashMap<ComponentId, Component>
   }
   
   class BoundedStorage {
     -components: [Option<Component>; 256]
     -id_map: IndexMap<ComponentId, usize>
   }
   
   ComponentInstance ..> ComponentMetadata : contains
   ComponentInstance ..> ExportTable : provides
   ComponentInstance ..> ImportTable : requires
   ComponentStorage <|-- DynamicStorage
   ComponentStorage <|-- BoundedStorage
   
   note top of ComponentInstance
     Core interface for
     component management
   end note
   @enduml

Resource Management Interface Hierarchy
---------------------------------------

**Purpose**: Provides a type-safe, environment-adaptive resource management system.

**Design Philosophy**:
- Resources are strongly typed and tracked by ID
- Different environments use different allocation strategies
- Lifecycle is explicitly managed with clear ownership

.. uml::
   :caption: Resource Management Interface Design

   @startuml
   !include _common.puml
   
   interface ResourceManager {
     +allocate<T>(resource: T): Result<ResourceId>
     +get<T>(id: ResourceId): Result<&T>
     +get_mut<T>(id: ResourceId): Result<&mut T>
     +deallocate(id: ResourceId): Result<()>
     +transfer_ownership(id: ResourceId, to: ComponentId): Result<()>
   }
   
   interface ResourceTable {
     +insert(id: ResourceId, resource: Resource): Result<()>
     +lookup(id: ResourceId): Option<&Resource>
     +remove(id: ResourceId): Option<Resource>
     +capacity(): usize
     +len(): usize
   }
   
   interface ResourceStrategy {
     +allocation_strategy<T>(): AllocationStrategy
     +validate_allocation<T>(size_hint: Option<usize>): Result<()>
     +handle_deallocation(id: ResourceId): Result<()>
   }
   
   class ResourceId {
     -value: u32
     +new(): ResourceId
     +as_u32(): u32
   }
   
   class Resource {
     -type_id: TypeId
     -data: ResourceData
     -owner: ComponentId
     -refcount: u32
   }
   
   enum AllocationStrategy {
     Dynamic
     Pool(pool_id: usize)
     Stack
   }
   
   ResourceManager ..> ResourceTable : uses
   ResourceManager ..> ResourceStrategy : consults
   ResourceTable ..> Resource : stores
   Resource ..> ResourceId : identified by
   ResourceStrategy ..> AllocationStrategy : determines
   
   note right of ResourceManager
     High-level resource
     management API
   end note
   
   note bottom of AllocationStrategy
     Environment-specific
     allocation strategies
   end note
   @enduml

Interface Interaction Patterns
------------------------------

**Cross-Layer Communication**:

.. uml::
   :caption: Interface Layer Communication Pattern

   @startuml
   !include _common.puml
   
   actor "External Caller" as Caller
   
   box "External Layer" #LightBlue
     participant "Runtime API" as Runtime
   end box
   
   box "Component Layer" #LightGreen
     participant "Component Manager" as CompMgr
     participant "Resource Manager" as ResMgr
   end box
   
   box "Foundation Layer" #LightYellow
     participant "Memory Provider" as MemProv
     participant "Type Registry" as TypeReg
   end box
   
   box "Platform Layer" #LightPink
     participant "Platform Provider" as Platform
   end box
   
   == Component Instantiation Flow ==
   Caller -> Runtime: instantiate(wasm_bytes)
   Runtime -> CompMgr: create_component(bytes)
   CompMgr -> TypeReg: validate_types(component)
   TypeReg --> CompMgr: validation_result
   
   CompMgr -> MemProv: allocate_memory(size)
   MemProv -> Platform: request_memory(size)
   Platform --> MemProv: memory_region
   MemProv --> CompMgr: LinearMemory
   
   CompMgr -> ResMgr: register_component(id, component)
   ResMgr --> CompMgr: registration_result
   
   CompMgr --> Runtime: ComponentId
   Runtime --> Caller: ComponentId
   
   note over Runtime, Platform
     Each layer only communicates
     with adjacent layers
   end note
   @enduml

Error Propagation Through Interfaces
------------------------------------

**Design Principle**: Errors are enriched with context as they propagate up through interface layers.

.. uml::
   :caption: Error Context Enrichment Pattern

   @startuml
   !include _common.puml
   
   class Error {
     +kind: ErrorKind
     +message: String
     +source: Option<Error>
   }
   
   class MemoryError {
     +OutOfBounds(offset: usize, length: usize)
     +AllocationFailed(requested: usize)
     +ProtectionViolation(address: usize)
   }
   
   class ComponentError {
     +Memory(MemoryError)
     +Validation(ValidationError)
     +Instantiation(String)
   }
   
   class RuntimeError {
     +Component(ComponentError)
     +Execution(ExecutionError)
     +Resource(ResourceError)
   }
   
   Error <|-- MemoryError
   Error <|-- ComponentError
   Error <|-- RuntimeError
   
   ComponentError o-- MemoryError : wraps
   RuntimeError o-- ComponentError : wraps
   
   note top of Error
     Base error type with
     context chain support
   end note
   
   note right of RuntimeError
     Top-level errors exposed
     to external callers
   end note
   @enduml

Interface Evolution Strategy
----------------------------

**Versioning and Compatibility**:

.. uml::
   :caption: Interface Versioning Strategy

   @startuml
   !include _common.puml
   
   package "Interface v1.0" {
     interface "MemoryProvider_v1" as MP1 {
       +read_bytes(offset, length): Result<&[u8]>
       +write_bytes(offset, data): Result<()>
     }
   }
   
   package "Interface v2.0" {
     interface "MemoryProvider_v2" as MP2 {
       +read_bytes(offset, length): Result<&[u8]>
       +write_bytes(offset, data): Result<()>
       +protect(protection: Protection): Result<()>
     }
   }
   
   class "CompatibilityAdapter" as Adapter {
     -inner: MemoryProvider_v1
     +read_bytes(offset, length): Result<&[u8]>
     +write_bytes(offset, data): Result<()>
     +protect(protection: Protection): Result<()>
   }
   
   MP1 <|.. Adapter : adapts
   MP2 <|.. Adapter : implements
   
   note bottom of Adapter
     Provides backward compatibility
     by adapting v1 to v2 interface
   end note
   @enduml

Interface Testing Strategy
--------------------------

**Contract Testing Approach**:

.. uml::
   :caption: Interface Contract Testing

   @startuml
   !include _common.puml
   
   abstract class "ContractTest<T>" as Contract {
     {abstract} +test_invariants(impl: T)
     {abstract} +test_preconditions(impl: T)
     {abstract} +test_postconditions(impl: T)
     {abstract} +test_error_conditions(impl: T)
   }
   
   class "MemoryProviderContract" as MemContract {
     +test_invariants(impl: MemoryProvider)
     +test_preconditions(impl: MemoryProvider)
     +test_postconditions(impl: MemoryProvider)
     +test_error_conditions(impl: MemoryProvider)
   }
   
   class "StandardMemoryTest" as StdTest {
     -memory: StandardMemory
     +run_all_tests()
   }
   
   class "BoundedMemoryTest" as BoundedTest {
     -memory: BoundedMemory
     +run_all_tests()
   }
   
   Contract <|-- MemContract
   MemContract <.. StdTest : uses
   MemContract <.. BoundedTest : uses
   
   note right of Contract
     Generic contract testing
     framework for interfaces
   end note
   
   note bottom of MemContract
     Specific contract tests
     for MemoryProvider interface
   end note
   @enduml

Summary
-------

This visual approach to interface documentation:

1. **Shows Structure**: Clear visualization of interface relationships
2. **Explains Design**: Rationale for architectural decisions
3. **Demonstrates Patterns**: Common interaction patterns
4. **Maintains Accuracy**: Can be generated from code annotations
5. **Supports Evolution**: Shows how interfaces can evolve

The key advantage is that these diagrams can be automatically generated from code annotations,
ensuring they remain synchronized with the actual implementation.