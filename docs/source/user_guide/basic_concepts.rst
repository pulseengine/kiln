==============
Basic Concepts
==============

Understanding the fundamental concepts of WebAssembly and Kiln.

WebAssembly Overview
====================

WebAssembly (WASM) is a binary instruction format for a stack-based virtual machine designed as a portable compilation target for programming languages.

Key Properties
--------------

* **Safe**: Memory-safe, sandboxed execution environment
* **Fast**: Near-native performance with predictable execution
* **Portable**: Runs on any platform with a WebAssembly runtime
* **Compact**: Efficient binary format for fast loading

Kiln Runtime Model
=================

Kiln provides a safety-critical WebAssembly runtime with:

Modules and Instances
---------------------

* **Module**: A compiled WebAssembly binary containing code and metadata
* **Instance**: A runtime instantiation of a module with allocated memory and state
* **Imports**: External functions and resources provided by the host
* **Exports**: Functions and values exposed by the module

Memory Model
------------

* **Linear Memory**: A contiguous array of bytes that grows in page-sized chunks
* **Stack**: Operand stack for instruction execution (managed internally)
* **Globals**: Module-scoped variables with defined mutability
* **Tables**: Arrays of references (functions, externrefs)

Execution Model
---------------

* **Instructions**: Stack-based operations that manipulate values
* **Functions**: Callable units of code with parameters and return values
* **Control Flow**: Structured control with blocks, loops, and conditionals
* **Traps**: Runtime errors that halt execution safely

Component Model
===============

The Component Model extends WebAssembly with higher-level abstractions:

Components vs Modules
---------------------

* **Modules**: Low-level WebAssembly with basic import/export
* **Components**: High-level units with rich interface types
* **Interfaces**: Strongly-typed contracts between components
* **Worlds**: Complete interface definitions for component interaction

Interface Types
---------------

* **Primitive Types**: Integers, floats, booleans, characters
* **Compound Types**: Records, variants, tuples, lists
* **Resource Types**: Handles to host-managed resources
* **Function Types**: Type-safe function signatures

Safety Features
===============

Kiln emphasizes safety through multiple mechanisms:

Memory Safety
-------------

* **Bounds Checking**: All memory accesses are validated
* **Type Safety**: Operations are checked for type compatibility
* **Stack Overflow Protection**: Execution stack has configurable limits
* **Resource Limits**: Configurable limits on memory, computation, and other resources

Error Handling
--------------

* **Graceful Degradation**: Runtime continues after recoverable errors
* **Detailed Error Information**: Clear error messages with context
* **No Undefined Behavior**: All edge cases have defined behavior
* **Isolation**: Errors in one module don't affect others

Security Model
--------------

* **Sandboxing**: Modules cannot access host resources without explicit grants
* **Capability-based Security**: Access to resources requires specific capabilities
* **Resource Accounting**: Track and limit resource consumption
* **Audit Trail**: Log security-relevant operations

Performance Characteristics
===========================

Kiln is designed for predictable, high-performance execution:

Deterministic Execution
-----------------------

* **Bounded Execution Time**: All operations have known worst-case timing
* **Reproducible Results**: Same inputs always produce same outputs
* **No Hidden Allocations**: Memory usage is explicit and controllable
* **Fuel-based Execution**: Optional step counting for execution limits

Optimization Features
---------------------

* **Ahead-of-Time Compilation**: Pre-compile modules for faster startup
* **Just-in-Time Compilation**: Dynamic optimization for hot paths
* **Memory Pooling**: Reuse allocated memory to reduce allocation overhead
* **Platform-Specific Optimizations**: Leverage platform features when available

Deployment Models
=================

Kiln supports various deployment scenarios:

Embedded Systems
----------------

* **no_std Support**: Run without standard library on bare metal
* **Minimal Resource Usage**: Configurable memory and CPU limits
* **Real-time Guarantees**: Predictable execution timing
* **Safety Certification**: Suitable for safety-critical applications

Server Applications
-------------------

* **High Throughput**: Efficient handling of many concurrent modules
* **Resource Isolation**: Prevent modules from interfering with each other
* **Dynamic Loading**: Load and unload modules at runtime
* **Monitoring and Metrics**: Detailed runtime statistics and profiling

Edge Computing
--------------

* **Fast Startup**: Quick module instantiation for responsive services
* **Small Footprint**: Minimal runtime overhead
* **Portable Deployment**: Same binaries run across different edge devices
* **Offline Operation**: No dependency on external services

Next Steps
==========

* Learn how to :doc:`running_modules` with Kiln
* Understand :doc:`configuration` options
* Explore :doc:`../examples/index` for practical usage
* See :doc:`../platform_guides/index` for platform-specific guidance