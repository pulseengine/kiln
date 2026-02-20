================================================
WebAssembly Component Model Binary Format in Kiln
================================================

.. image:: _static/icons/validation_process.svg
   :width: 64px
   :align: right
   :alt: Validation Process Icon

This document describes the implementation of the WebAssembly Component Model Binary Format in Kiln, following the official `Component Model Binary Format specification <https://github.com/WebAssembly/component-model/blob/main/design/mvp/Binary.md>`_.

.. contents:: Table of Contents
   :local:
   :depth: 2

Overview
========

The WebAssembly Component Model binary format builds upon the core WebAssembly binary format, adding a new layer identifier to distinguish components from modules. The top-level production is ``component`` and the convention is that a file with the ``.wasm`` extension may contain either a core module or a component.

Kiln Implementation Status
-------------------------

The current Kiln implementation provides foundational support for the Component Model binary format with the following implementation components:

- **Decoder**: Implemented in ``kiln-decoder/src/component/decode.rs`` 
- **Section Parsers**: Defined in ``kiln-decoder/src/component/parse.rs``
- **Data Structures**: Defined in ``kiln-format/src/component.rs``
- **Binary Constants**: Defined in ``kiln-format/src/binary.rs``

Many aspects of the specification are still in development, with placeholder implementations that will be completed in future versions.

Binary Format Structure
=======================

Component Definitions
---------------------

The specification defines:

.. code-block:: text

   component ::= <preamble> s*:<section>*
   preamble  ::= <magic> <version> <layer>
   magic     ::= 0x00 0x61 0x73 0x6D
   version   ::= 0x0d 0x00
   layer     ::= 0x01 0x00

**Kiln Implementation**:

The Kiln implementation uses different version and layer encoding:

.. code-block:: text

   // Component Model magic bytes (same as core: \0asm)
   pub const COMPONENT_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];
   
   // Component Model binary format version - 2 bytes version, 2 bytes layer
   // Version 1.0, Layer 1
   pub const COMPONENT_VERSION: [u8; 4] = [0x01, 0x00, 0x01, 0x00];

**Discrepancy**: The specification uses version ``[0x0d, 0x00]`` with layer ``[0x01, 0x00]``, while Kiln implements version ``[0x01, 0x00]`` with layer ``[0x01, 0x00]``. Additionally, Kiln combines these into a single 4-byte field rather than two separate 2-byte fields.

The implementation in ``kiln-decoder/src/component/decode.rs`` verifies only the first 8 bytes (magic + version), without distinguishing between version and layer as separate fields.

Section Definitions
-------------------

The specification defines the following section types:

.. code-block:: text

   section ::=    section_0(<core:custom>)         => ϵ
              | m: section_1(<core:module>)         => [core-prefix(m)]
              | i*:section_2(vec(<core:instance>))  => core-prefix(i)*
              | t*:section_3(vec(<core:type>))      => core-prefix(t)*
              | c: section_4(<component>)           => [c]
              | i*:section_5(vec(<instance>))       => i*
              | a*:section_6(vec(<alias>))          => a*
              | t*:section_7(vec(<type>))           => t*
              | c*:section_8(vec(<canon>))          => c*
              | s: section_9(<start>)               => [s]
              | i*:section_10(vec(<import>))        => i*
              | e*:section_11(vec(<export>))        => e*
              | v*:section_12(vec(<value>))         => v*

**Kiln Implementation**:

Kiln defines section IDs in ``kiln-format/src/binary.rs``:

.. code-block:: text

   pub const COMPONENT_CUSTOM_SECTION_ID: u8 = 0x00;
   pub const COMPONENT_CORE_MODULE_SECTION_ID: u8 = 0x01;
   pub const COMPONENT_CORE_INSTANCE_SECTION_ID: u8 = 0x02;
   pub const COMPONENT_CORE_TYPE_SECTION_ID: u8 = 0x03;
   pub const COMPONENT_COMPONENT_SECTION_ID: u8 = 0x04;
   pub const COMPONENT_INSTANCE_SECTION_ID: u8 = 0x05;
   pub const COMPONENT_ALIAS_SECTION_ID: u8 = 0x06;
   pub const COMPONENT_TYPE_SECTION_ID: u8 = 0x07;
   pub const COMPONENT_CANON_SECTION_ID: u8 = 0x08;
   pub const COMPONENT_START_SECTION_ID: u8 = 0x09;
   pub const COMPONENT_IMPORT_SECTION_ID: u8 = 0x0A;
   pub const COMPONENT_EXPORT_SECTION_ID: u8 = 0x0B;
   pub const COMPONENT_VALUE_SECTION_ID: u8 = 0x0C;

The section parsing is implemented in ``kiln-decoder/src/component/decode.rs``, which iterates through sections and delegates to appropriate parsers in ``kiln-decoder/src/component/parse.rs``.

Instance Definitions
====================

Core Instance Definitions
-------------------------

The specification defines:

.. code-block:: text

   core:instance       ::= ie:<core:instanceexpr>                             => (instance ie)
   core:instanceexpr   ::= 0x00 m:<moduleidx> arg*:vec(<core:instantiatearg>) => (instantiate m arg*)
                         | 0x01 e*:vec(<core:inlineexport>)                   => e*

**Kiln Implementation**:

Kiln implements core instance definitions in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub struct CoreInstance {
       /// Instance expression
       pub instance_expr: CoreInstanceExpr,
   }

   pub enum CoreInstanceExpr {
       /// Instantiate a core module
       Instantiate {
           /// Module index
           module_idx: u32,
           /// Instantiation arguments
           args: Vec<CoreInstantiateArg>,
       },
       /// Collection of inlined exports
       InlineExports(Vec<CoreInlineExport>),
   }

The binary parsing is implemented in ``kiln-decoder/src/component/parse.rs`` in the ``parse_core_instance_section`` and ``parse_core_instance_expr`` functions.

Component Instance Definitions
------------------------------

The specification defines:

.. code-block:: text

   instance            ::= ie:<instanceexpr>                                  => (instance ie)
   instanceexpr        ::= 0x00 c:<componentidx> arg*:vec(<instantiatearg>)   => (instantiate c arg*)
                         | 0x01 e*:vec(<inlineexport>)                        => e*
   sort                ::= 0x00 cs:<core:sort>                                => core cs
                         | 0x01                                               => func
                         | 0x02                                               => value
                         | 0x03                                               => type
                         | 0x04                                               => component
                         | 0x05                                               => instance

**Kiln Implementation**:

Kiln implements component instance definitions in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub struct Instance {
       /// Instance expression
       pub instance_expr: InstanceExpr,
   }

   pub enum InstanceExpr {
       /// Instantiate a component
       Instantiate {
           /// Component index
           component_idx: u32,
           /// Instantiation arguments
           args: Vec<InstantiateArg>,
       },
       /// Collection of inlined exports
       InlineExports(Vec<InlineExport>),
   }

   pub enum Sort {
       /// Core reference
       Core(CoreSort),
       /// Function reference
       Function,
       /// Value reference
       Value,
       /// Type reference
       Type,
       /// Component reference
       Component,
       /// Instance reference
       Instance,
   }

The sorts are defined in ``kiln-format/src/binary.rs`` with values matching the specification:

.. code-block:: text

   pub const COMPONENT_SORT_CORE: u8 = 0x00;
   pub const COMPONENT_SORT_FUNC: u8 = 0x01;
   pub const COMPONENT_SORT_VALUE: u8 = 0x02;
   pub const COMPONENT_SORT_TYPE: u8 = 0x03;
   pub const COMPONENT_SORT_COMPONENT: u8 = 0x04;
   pub const COMPONENT_SORT_INSTANCE: u8 = 0x05;

Type Definitions
================

The specification defines various component types. Kiln implements them in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub enum ComponentTypeDefinition {
       /// Component type
       Component {
           /// Component imports
           imports: Vec<(String, String, ExternType)>,
           /// Component exports
           exports: Vec<(String, ExternType)>,
       },
       /// Instance type
       Instance {
           /// Instance exports
           exports: Vec<(String, ExternType)>,
       },
       /// Function type
       Function {
           /// Parameter types
           params: Vec<(String, ValType)>,
           /// Result types
           results: Vec<ValType>,
       },
       /// Value type
       Value(ValType),
       /// Resource type
       Resource {
           /// Resource representation type
           representation: ResourceRepresentation,
           /// Whether the resource is nullable
           nullable: bool,
       },
   }

Value Types
-----------

The specification defines numerous value types. Kiln implements them in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub enum ValType {
       /// Boolean type
       Bool,
       /// 8-bit signed integer
       S8,
       /// 8-bit unsigned integer
       U8,
       /// 16-bit signed integer
       S16,
       /// 16-bit unsigned integer
       U16,
       /// 32-bit signed integer
       S32,
       /// 32-bit unsigned integer
       U32,
       /// 64-bit signed integer
       S64,
       /// 64-bit unsigned integer
       U64,
       /// 32-bit float
       F32,
       /// 64-bit float
       F64,
       /// Unicode character
       Char,
       /// String type
       String,
       /// Reference type
       Ref(u32),
       /// Record type with named fields
       Record(Vec<(String, ValType)>),
       /// Variant type
       Variant(Vec<(String, Option<ValType>)>),
       /// List type
       List(Box<ValType>),
       /// Fixed-length list type with element type and length
       FixedList(Box<ValType>, u32),
       /// Tuple type
       Tuple(Vec<ValType>),
       /// Flags type
       Flags(Vec<String>),
       /// Enum type
       Enum(Vec<String>),
       /// Option type
       Option(Box<ValType>),
       /// Result type (ok only)
       Result(Box<ValType>),
       /// Result type (error only)
       ResultErr(Box<ValType>),
       /// Result type (ok and error)
       ResultBoth(Box<ValType>, Box<ValType>),
       /// Own a resource
       Own(u32),
       /// Borrow a resource
       Borrow(u32),
       /// Error context type
       ErrorContext,
   }

The binary type codes are defined in ``kiln-format/src/binary.rs``:

.. code-block:: text

   pub const COMPONENT_VALTYPE_BOOL: u8 = 0x7F;
   pub const COMPONENT_VALTYPE_S8: u8 = 0x7E;
   pub const COMPONENT_VALTYPE_U8: u8 = 0x7D;
   // ...and so on

Resource Types
--------------

Kiln implements resource types with a custom representation:

.. code-block:: text

   pub enum ResourceRepresentation {
       /// 32-bit integer handle
       Handle32,
       /// 64-bit integer handle
       Handle64,
       /// Record representation
       Record(Vec<String>),
       /// Aggregate representation
       Aggregate(Vec<u32>),
   }

This differs from the specification, which has a simpler representation focused on abstract vs. concrete resources.

Alias Definitions
=================

The specification defines different forms of aliases. Kiln implements them in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub enum AliasTarget {
       /// Core WebAssembly export from an instance
       CoreInstanceExport {
           /// Instance index
           instance_idx: u32,
           /// Export name
           name: String,
           /// Kind of the target
           kind: CoreSort,
       },
       /// Export from a component instance
       InstanceExport {
           /// Instance index
           instance_idx: u32,
           /// Export name
           name: String,
           /// Kind of the target
           kind: Sort,
       },
       /// Outer definition from an enclosing component (forwarding from parent)
       Outer {
           /// Count of components to traverse outward
           count: u32,
           /// Kind of the target
           kind: Sort,
           /// Index within the kind
           idx: u32,
       },
   }

The parsing is implemented in ``parse_alias_section`` and ``parse_alias_target`` in ``kiln-decoder/src/component/parse.rs``.

Canonical Function Definitions
==============================

The specification defines canonical operations for function lifting and lowering. Kiln implements an extended version in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub enum CanonOperation {
       /// Lift a core function to the component ABI
       Lift {
           /// Core function index
           func_idx: u32,
           /// Type index for the lifted function
           type_idx: u32,
           /// Options for lifting
           options: LiftOptions,
       },
       /// Lower a component function to the core ABI
       Lower {
           /// Component function index
           func_idx: u32,
           /// Options for lowering
           options: LowerOptions,
       },
       /// Resource operations
       Resource(ResourceOperation),
       /// Reallocation operation
       Realloc {
           /// Function index for memory allocation
           alloc_func_idx: u32,
           /// Memory index to use
           memory_idx: u32,
       },
       /// Post-return operation (cleanup)
       PostReturn {
           /// Function index for post-return cleanup
           func_idx: u32,
       },
       /// Memory copy operation (optional optimization)
       MemoryCopy {
           /// Source memory index
           src_memory_idx: u32,
           /// Destination memory index
           dst_memory_idx: u32,
           /// Function index for the copy operation
           func_idx: u32,
       },
       /// Async operation (stackful lift)
       Async {
           /// Function index for the async operation
           func_idx: u32,
           /// Type index for the async function
           type_idx: u32,
           /// Options for async operations
           options: AsyncOptions,
       },
   }

The parsing is implemented in ``parse_canon_section`` and related functions in ``kiln-decoder/src/component/parse.rs``.

Start Definitions
=================

The specification defines:

.. code-block:: text

   start ::= f:<funcidx> arg*:vec(<valueidx>) r:<u32> => (start f (value arg)* (result (value))ʳ)

**Kiln Implementation**:

Kiln implements the start definition in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub struct Start {
       /// Function index
       pub func_idx: u32,
       /// Value arguments
       pub args: Vec<u32>,
       /// Number of results
       pub results: u32,
   }

**Implementation Status**: The ``parse_start_section`` function in ``kiln-decoder/src/component/parse.rs`` currently returns a "not implemented" error, indicating this feature is planned but not yet implemented.

Import and Export Definitions
=============================

The specification defines import and export declarations. Kiln implements them in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub struct Import {
       /// Import name in namespace.name format
       pub name: ImportName,
       /// Type of the import
       pub ty: ExternType,
   }

   pub struct Export {
       /// Export name in "name" format
       pub name: ExportName,
       /// Sort of the exported item
       pub sort: Sort,
       /// Index within the sort
       pub idx: u32,
       /// Declared type (optional)
       pub ty: Option<ExternType>,
   }

Kiln has extended name structures:

.. code-block:: text

   pub struct ImportName {
       /// Namespace of the import
       pub namespace: String,
       /// Name of the import
       pub name: String,
       /// Nested namespaces (if any)
       pub nested: Vec<String>,
       /// Package reference (if any)
       pub package: Option<PackageReference>,
   }
   
   pub struct ExportName {
       /// Basic name
       pub name: String,
       /// Whether this export is a resource
       pub is_resource: bool,
       /// Semver compatibility string
       pub semver: Option<String>,
       /// Integrity hash for content verification
       pub integrity: Option<String>,
       /// Nested namespaces (if any)
       pub nested: Vec<String>,
   }

The parsing is implemented in ``parse_import_section`` and ``parse_export_section`` in ``kiln-decoder/src/component/parse.rs``.

Value Definitions
=================

The specification defines detailed value encoding rules. Kiln implements a simplified version in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub struct Value {
       /// Type of the value
       pub ty: ValType,
       /// Encoded value data
       pub data: Vec<u8>,
       /// Value expression (if available)
       pub expression: Option<ValueExpression>,
       /// Value name (if available from custom sections)
       pub name: Option<String>,
   }

   pub enum ValueExpression {
       /// Reference to an item in component
       ItemRef {
           /// Sort of the referenced item
           sort: Sort,
           /// Index within the sort
           idx: u32,
       },
       /// Global initialization expression
       GlobalInit {
           /// Global index
           global_idx: u32,
       },
       /// Function call expression
       FunctionCall {
           /// Function index
           func_idx: u32,
           /// Arguments (indices to other values)
           args: Vec<u32>,
       },
       /// Direct constant value
       Const(ConstValue),
   }

The parsing is implemented in ``parse_value_section`` and related functions in ``kiln-decoder/src/component/parse.rs``.

Name Section
============

The specification defines a name section for components similar to the core WebAssembly name section. Kiln has an initial implementation in ``kiln-decoder/src/component_name_section.rs`` that parses the component name but does not yet support the full specification's naming capabilities for all component elements.

Current Implementation Differences Summary
==========================================

1. **Version Field Format**: Kiln uses ``[0x01, 0x00, 0x01, 0x00]`` while the specification uses ``[0x0D, 0x00]`` for version followed by ``[0x01, 0x00]`` for layer.

2. **Incomplete Section Implementations**: Many section parsers are currently placeholder implementations or only partially implemented:
   - Start section parser explicitly returns "not implemented"
   - Value section parser has incomplete value encoding/decoding
   - Resource types have a different representation structure

3. **Extended Structure**: Kiln implements additional fields and structures beyond the specification:
   - Extended import/export name structures with packaging and nesting information
   - Additional canonical operation types for async functions and memory management
   - Value expressions have multiple forms beyond the specification's direct encoding

4. **Name Section Implementation**: The component name section implementation is simpler than specified, focusing primarily on the component name rather than all of the possible naming information.

5. **Validation**: Many of the validation rules specified in the binary format documentation are not yet fully implemented.

Binary Format Parsing Process
=============================

The Kiln component binary parsing process in ``kiln-decoder/src/component/decode.rs`` follows these steps:

1. Verify the magic number (``\0asm``)
2. Check the version bytes (currently checking only for 8 total bytes)
3. Process each section:
   a. Read section ID and size
   b. Extract section bytes
   c. Delegate to the appropriate section parser

Each section parser in ``kiln-decoder/src/component/parse.rs`` is responsible for:
1. Reading the count of elements in the section
2. Parsing each element according to its binary format
3. Returning a vector of the parsed elements

Next Steps in Implementation
============================

Key areas for future development of the Kiln binary format implementation:

1. **Align Version Handling**: Update to match the specification's separate version and layer fields
2. **Complete Parsers**: Implement the remaining placeholder parsers, particularly for start sections and value encoding
3. **Validation Rules**: Add full validation according to the specification
4. **Resource Type Handling**: Align resource type implementation with the specification
5. **Name Section**: Complete the name section implementation 
6. **Value Encoding/Decoding**: Implement complete value encoding and decoding according to the specification

Current Implementation Status
-----------------------------

The Kiln implementation differs from the specification in several key aspects:

**Version Field Discrepancy**

The specification defines:

.. code-block:: text

   version   ::= 0x0D 0x00
   layer     ::= 0x01 0x00

But Kiln implements:

.. code-block:: text

   // Component Model binary format version - version 0.1
   COMPONENT_VERSION: [0x01, 0x00, 0x00, 0x01]

This means Kiln uses a 4-byte field structured as "version + layer", with the first 2 bytes representing the version (0x01, 0x00) and the last 2 bytes representing the layer (0x00, 0x01).

Instance Definitions
====================

Core Instance Definitions
-------------------------

The specification defines:

.. code-block:: text

   core:instance       ::= ie:<core:instanceexpr>                             => (instance ie)
   core:instanceexpr   ::= 0x00 m:<moduleidx> arg*:vec(<core:instantiatearg>) => (instantiate m arg*)
                         | 0x01 e*:vec(<core:inlineexport>)                   => e*
   core:instantiatearg ::= n:<core:name> 0x12 i:<instanceidx>                 => (with n (instance i))
   core:sortidx        ::= sort:<core:sort> idx:<u32>                         => (sort idx)
   core:sort           ::= 0x00                                               => func
                         | 0x01                                               => table
                         | 0x02                                               => memory
                         | 0x03                                               => global
                         | 0x10                                               => type
                         | 0x11                                               => module
                         | 0x12                                               => instance
   core:inlineexport   ::= n:<core:name> si:<core:sortidx>                    => (export n si)

Kiln implements the core sort values as constants:

.. code-block:: text

   COMPONENT_CORE_SORT_FUNC: 0x00
   COMPONENT_CORE_SORT_TABLE: 0x01
   COMPONENT_CORE_SORT_MEMORY: 0x02
   COMPONENT_CORE_SORT_GLOBAL: 0x03
   COMPONENT_CORE_SORT_TYPE: 0x10
   COMPONENT_CORE_SORT_MODULE: 0x11
   COMPONENT_CORE_SORT_INSTANCE: 0x12

The data structure in Kiln:

.. code-block:: text

   pub enum CoreInstanceExpr {
       /// Instantiate a core module
       Instantiate {
           /// Module index
           module_idx: u32,
           /// Instantiation arguments
           args: Vec<CoreInstantiateArg>,
       },
       /// Collection of inlined exports
       InlineExports(Vec<CoreInlineExport>),
   }

Component Instance Definitions
------------------------------

The specification defines:

.. code-block:: text

   instance            ::= ie:<instanceexpr>                                  => (instance ie)
   instanceexpr        ::= 0x00 c:<componentidx> arg*:vec(<instantiatearg>)   => (instantiate c arg*)
                         | 0x01 e*:vec(<inlineexport>)                        => e*
   instantiatearg      ::= n:<name>  si:<sortidx>                             => (with n si)
   name                ::= n:<core:name>                                      => n
   sortidx             ::= sort:<sort> idx:<u32>                              => (sort idx)
   sort                ::= 0x00 cs:<core:sort>                                => core cs
                         | 0x01                                               => func
                         | 0x02                                               => value
                         | 0x03                                               => type
                         | 0x04                                               => component
                         | 0x05                                               => instance
   inlineexport        ::= n:<exportname> si:<sortidx>                        => (export n si)

Kiln implements these sort values as constants:

.. code-block:: text

   COMPONENT_SORT_CORE: 0x00
   COMPONENT_SORT_FUNC: 0x01
   COMPONENT_SORT_VALUE: 0x02
   COMPONENT_SORT_TYPE: 0x03
   COMPONENT_SORT_COMPONENT: 0x04
   COMPONENT_SORT_INSTANCE: 0x05

Component Type Definitions
==========================

The Kiln implementation provides support for the following component type definitions with data structures in ``kiln-format/src/component.rs``:

.. code-block:: text

   pub enum ComponentTypeDefinition {
       /// Component type
       Component {
           /// Component imports
           imports: Vec<(String, String, ExternType)>,
           /// Component exports
           exports: Vec<(String, ExternType)>,
       },
       /// Instance type
       Instance {
           /// Instance exports
           exports: Vec<(String, ExternType)>,
       },
       /// Function type
       Function {
           /// Parameter types
           params: Vec<(String, ValType)>,
           /// Result types
           results: Vec<ValType>,
       },
       /// Value type
       Value(ValType),
       /// Resource type
       Resource {
           /// Resource representation type
           representation: ResourceRepresentation,
           /// Whether the resource is nullable
           nullable: bool,
       },
   }

This implements the specification's component type definitions, though the binary parsing is not yet complete for all types.

Value Types
-----------

The Kiln implementation supports the following value types:

.. code-block:: text

   pub enum ValType {
       /// Boolean type
       Bool,
       /// 8-bit signed integer
       S8,
       /// 8-bit unsigned integer
       U8,
       /// 16-bit signed integer
       S16,
       /// 16-bit unsigned integer
       U16,
       /// 32-bit signed integer
       S32,
       /// 32-bit unsigned integer
       U32,
       /// 64-bit signed integer
       S64,
       /// 64-bit unsigned integer
       U64,
       /// 32-bit float
       F32,
       /// 64-bit float
       F64,
       /// Unicode character
       Char,
       /// String type
       String,
       /// Reference type
       Ref(u32),
       /// Record type with named fields
       Record(Vec<(String, ValType)>),
       /// Variant type
       Variant(Vec<(String, Option<ValType>)>),
       /// List type
       List(Box<ValType>),
       /// Tuple type
       Tuple(Vec<ValType>),
       /// Flags type
       Flags(Vec<String>),
       /// Enum type
       Enum(Vec<String>),
       /// Option type
       Option(Box<ValType>),
       /// Result type (ok only)
       Result(Box<ValType>),
       /// Result type (error only)
       ResultErr(Box<ValType>),
       /// Result type (ok and error)
       ResultBoth(Box<ValType>, Box<ValType>),
       /// Own a resource
       Own(u32),
       /// Borrow a resource
       Borrow(u32),
   }

Alias Definitions
=================

The specification defines various forms of aliases, and Kiln implements them as:

.. code-block:: text

   pub enum AliasTarget {
       /// Core WebAssembly export from an instance
       CoreInstanceExport {
           /// Instance index
           instance_idx: u32,
           /// Export name
           name: String,
           /// Kind of the target
           kind: CoreSort,
       },
       /// Export from a component instance
       InstanceExport {
           /// Instance index
           instance_idx: u32,
           /// Export name
           name: String,
           /// Kind of the target
           kind: Sort,
       },
       /// Outer definition from an enclosing component (forwarding from parent)
       Outer {
           /// Count of components to traverse outward
           count: u32,
           /// Kind of the target
           kind: Sort,
           /// Index within the kind
           idx: u32,
       },
   }

This differs slightly from the specification, which has more detailed alias forms.

Canonical Function Definitions
==============================

Kiln implements canonical function operations:

.. code-block:: text

   pub enum CanonOperation {
       /// Lift a core function to the component ABI
       Lift {
           /// Core function index
           func_idx: u32,
           /// Type index for the lifted function
           type_idx: u32,
           /// Options for lifting
           options: LiftOptions,
       },
       /// Lower a component function to the core ABI
       Lower {
           /// Component function index
           func_idx: u32,
           /// Options for lowering
           options: LowerOptions,
       },
       /// Resource operations
       Resource(ResourceOperation),
   }

Start Definitions
=================

The specification defines:

.. code-block:: text

   start ::= f:<funcidx> arg*:vec(<valueidx>) r:<u32> => (start f (value arg)* (result (value))ʳ)

Kiln implements this as:

.. code-block:: text

   pub struct Start {
       /// Function index
       pub func_idx: u32,
       /// Value arguments
       pub args: Vec<u32>,
       /// Number of results
       pub results: u32,
   }

However, the parsing is currently incomplete in Kiln, as indicated by the implementation in ``parse_start_section`` which returns a not implemented error.

Import and Export Definitions
=============================

Kiln implements imports and exports with these structures:

.. code-block:: text

   pub struct Import {
       /// Import name in namespace.name format
       pub name: ImportName,
       /// Type of the import
       pub ty: ExternType,
   }

   pub struct Export {
       /// Export name in "name" format
       pub name: ExportName,
       /// Sort of the exported item
       pub sort: Sort,
       /// Index within the sort
       pub idx: u32,
       /// Declared type (optional)
       pub ty: Option<ExternType>,
   }

These implement the specification imports and exports, though with some differences in the naming metadata structure.

Value Definitions
=================

Kiln implements a Value structure, though the binary parsing is still incomplete:

.. code-block:: text

   pub struct Value {
       /// Type of the value
       pub ty: ValType,
       /// Encoded value data
       pub data: Vec<u8>,
   }

The specification defines more detailed value encoding rules which are not yet fully implemented.

Section Parsing Process
=======================

The decoding process in ``kiln-decoder/src/component/decode.rs`` follows these steps:

1. Verify the magic number (``\0asm``)
2. Read the version field
3. Iterate through sections:
   a. Read section ID and size
   b. Extract section bytes
   c. Parse section based on ID

Each section type has a corresponding parser in ``kiln-decoder/src/component/parse.rs``, but many of these are currently placeholders that don't fully implement the specification.

Binary Format Constants
=======================

The binary format constants are defined in ``kiln-format/src/binary.rs``:

.. code-block:: text

   // Component Model magic bytes (same as core: \0asm)
   COMPONENT_MAGIC: [0x00, 0x61, 0x73, 0x6D]

   // Component Model binary format version - version 0.1
   COMPONENT_VERSION: [0x01, 0x00, 0x00, 0x01]

   // Component Model version only (first two bytes of version)
   COMPONENT_VERSION_ONLY: [0x01, 0x00]

   // Component Model layer identifier - distinguishes components from modules
   COMPONENT_LAYER: [0x00, 0x01]

   // Component Model section IDs
   COMPONENT_CUSTOM_SECTION_ID: 0x00
   COMPONENT_CORE_MODULE_SECTION_ID: 0x01
   COMPONENT_CORE_INSTANCE_SECTION_ID: 0x02
   COMPONENT_CORE_TYPE_SECTION_ID: 0x03
   COMPONENT_COMPONENT_SECTION_ID: 0x04
   COMPONENT_INSTANCE_SECTION_ID: 0x05
   COMPONENT_ALIAS_SECTION_ID: 0x06
   COMPONENT_TYPE_SECTION_ID: 0x07
   COMPONENT_CANON_SECTION_ID: 0x08
   COMPONENT_START_SECTION_ID: 0x09
   COMPONENT_IMPORT_SECTION_ID: 0x0A
   COMPONENT_EXPORT_SECTION_ID: 0x0B
   COMPONENT_VALUE_SECTION_ID: 0x0C

Name Section Implementation
===========================

The specification defines a name section for components, similar to the core WebAssembly name section. The Kiln implementation has a partial implementation in ``kiln-decoder/src/component_name_section.rs`` but with some discrepancies:

The specification defines:

.. code-block:: text

   namesec    ::= section_0(namedata)
   namedata   ::= n:<name>                (if n = 'component-name')
                  name:<componentnamesubsec>?
                  sortnames*:<sortnamesubsec>*
   namesubsection_N(B) ::= N:<byte> size:<u32> B     (if size == |B|)

   componentnamesubsec ::= namesubsection_0(<name>)
   sortnamesubsec ::= namesubsection_1(<sortnames>)
   sortnames ::= sort:<sort> names:<namemap>

   namemap ::= names:vec(<nameassoc>)
   nameassoc ::= idx:<u32> name:<name>

Current Implementation Differences Summary
==========================================

1. **Version Implementation**: Kiln uses a 4-byte version field ``[0x01, 0x00, 0x00, 0x01]`` while the specification separates this into a 2-byte version field ``[0x0D, 0x00]`` followed by a 2-byte layer field ``[0x01, 0x00]``.

2. **Placeholder Implementations**: Many section parsers are currently placeholder implementations that will be fully implemented in future versions:
   - ``parse_core_module_section``
   - ``parse_core_instance_section``
   - ``parse_core_type_section``
   - ``parse_component_section``
   - ``parse_instance_section``
   - ``parse_canon_section``
   - ``parse_component_type_section``
   - ``parse_start_section``
   - ``parse_import_section``
   - ``parse_export_section``
   - ``parse_value_section``
   - ``parse_alias_section``

3. **Resource Types Implementation**: The resource type representation in Kiln has a different structure than specified, with specific types for handle32, handle64, record, and aggregate.

4. **Start Function Implementation**: The start function section is defined in the data structure but parsing is explicitly not implemented yet.

5. **Value Encoding/Decoding**: The specification defines detailed value encoding rules which are not yet fully implemented in Kiln.

6. **Name Section Implementation**: The name section implementation in Kiln differs from the specification in structure and completeness.

7. **Validation**: The specification requires detailed validation of each section's contents which is not yet fully implemented in Kiln.

Future Work
===========

The Kiln implementation of the Component Model binary format is under active development. Future work includes:

1. Complete implementation of all section parsers
2. Updating the version field structure to match the specification
3. Full validation according to the specification
4. Complete implementation of value encoding/decoding
5. Resource type handling improvements
6. Name section implementation according to specification
7. Support for experimental features marked with 🪙 in the specification
8. Optimization of parsing and validation 