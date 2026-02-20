==========================
Layered Architecture
==========================

**Teaching Point**: A layered architecture ensures clean dependencies and separation of concerns. Each layer only depends on layers below it.

Architecture Layers
-------------------

.. uml::

   @startuml
   !define LAYER(name,desc) rectangle name as desc
   
   LAYER(APP, "Application Layer\n(kilnd, user apps)")
   LAYER(LIB, "Library Facade\n(kiln)")
   LAYER(COMP, "Component Model\n(kiln-component)")
   LAYER(RUNTIME, "Runtime Layer\n(kiln-runtime, kiln-instructions)")
   LAYER(DECODE, "Decoding Layer\n(kiln-decoder)")
   LAYER(FOUND, "Foundation Layer\n(kiln-foundation, kiln-error)")
   LAYER(PLATFORM, "Platform Layer\n(kiln-platform, kiln-sync)")
   
   APP --> LIB
   LIB --> COMP
   LIB --> RUNTIME
   COMP --> RUNTIME
   RUNTIME --> DECODE
   RUNTIME --> FOUND
   DECODE --> FOUND
   COMP --> FOUND
   FOUND --> PLATFORM
   @enduml

Layer Responsibilities
----------------------

Platform Layer (Bottom)
~~~~~~~~~~~~~~~~~~~~~~~

**Crates**: ``kiln-platform``, ``kiln-sync``

**Purpose**: Abstracts OS and hardware differences

**Key Responsibilities**:
- Memory page allocation
- Synchronization primitives  
- Hardware security features
- Platform detection

**Environment Support**:

.. code-block:: rust

   // Platform traits work across all environments
   pub trait PageAllocator { /* ... */ }
   pub trait FutexLike { /* ... */ }
   
   // Different implementations selected at compile time
   #[cfg(target_os = "linux")]
   type PlatformAllocator = LinuxAllocator;
   
   #[cfg(all(not(feature = "std"), not(feature = "alloc")))]
   type PlatformAllocator = StaticAllocator;

Foundation Layer
~~~~~~~~~~~~~~~~

**Crates**: ``kiln-foundation``, ``kiln-error``, ``kiln-math``

**Purpose**: Core types and utilities used throughout the system

**Key Components**:
- Error handling system
- Safe memory abstractions
- Bounded collections for no_alloc
- Basic math operations
- Type definitions

**Teaching Point**: This layer must work in all environments:

.. code-block:: rust

   // Example: BoundedVec works everywhere
   #[cfg(feature = "alloc")]
   pub type DefaultVec<T> = Vec<T>;
   
   #[cfg(not(feature = "alloc"))]
   pub type DefaultVec<T> = BoundedVec<T, 1024, NoStdProvider<16384>>;

Decoding Layer
~~~~~~~~~~~~~~

**Crates**: ``kiln-decoder``, ``kiln-format``

**Purpose**: Parse and validate WebAssembly binaries

**Responsibilities**:
- Binary format parsing
- Structural validation
- Module representation
- Custom section handling

**no_alloc Support**:

.. code-block:: rust

   // Decoder works with fixed buffers in no_alloc
   pub struct NoAllocDecoder<const N: usize> {
       sections: BoundedVec<Section, MAX_SECTIONS>,
   }

Runtime Layer
~~~~~~~~~~~~~

**Crates**: ``kiln-runtime``, ``kiln-instructions``

**Purpose**: Execute WebAssembly code

**Components**:
- Execution engine
- Instruction implementations
- Memory management
- Global/table management
- Stack management

Component Model Layer
~~~~~~~~~~~~~~~~~~~~~

**Crates**: ``kiln-component``

**Purpose**: Implement WebAssembly Component Model

**Features**:
- Component instantiation
- Interface type handling
- Canonical ABI
- Resource management

Library Facade Layer
~~~~~~~~~~~~~~~~~~~~

**Crates**: ``kiln``

**Purpose**: Unified API for users

**Provides**:
- Simple interface
- Feature configuration
- Default implementations
- Convenience functions

Application Layer (Top)
~~~~~~~~~~~~~~~~~~~~~~~

**Crates**: ``kilnd``, user applications

**Examples**:
- CLI daemon (kilnd)
- Embedded applications
- Cloud services
- Test harnesses

Cross-Layer Communication
-------------------------

**Teaching Point**: Layers communicate through well-defined interfaces:

.. code-block:: rust

   // Lower layer provides trait
   // kiln-platform/src/lib.rs
   pub trait PageAllocator {
       fn allocate(&mut self, pages: usize) -> Result<*mut u8>;
   }
   
   // Higher layer uses trait
   // kiln-runtime/src/memory.rs
   pub struct Memory<A: PageAllocator> {
       allocator: A,
       pages: Vec<Page>,
   }

Dependency Rules
----------------

.. arch_constraint:: Layering Rules
   :id: ARCH_CON_LAYER_001
   :priority: high
   
   1. Dependencies only go downward
   2. No circular dependencies between layers
   3. Each layer has a clear interface
   4. Platform-specific code isolated to platform layer

Benefits of Layering
--------------------

1. **Testability**: Each layer can be tested independently
2. **Portability**: Platform changes isolated to one layer
3. **Maintainability**: Clear boundaries and responsibilities
4. **Flexibility**: Layers can be replaced or extended

Cross-References
----------------

- **Component Details**: See :doc:`components`
- **Interface Definitions**: See :doc:`../03_interfaces/interface_catalog`
- **Platform Details**: See :doc:`../platform_layer`