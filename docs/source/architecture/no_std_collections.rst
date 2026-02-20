.. _no_std_collections:

no_std/no_alloc Collections
===========================

Overview
--------

This document describes the bounded collection types designed for ``no_std`` and ``no_alloc`` environments in the Kiln system. These collections provide memory-safe alternatives to standard library collections, with strict capacity limits, verification mechanisms, and platform-specific optimizations.

Design Goals
-----------

The bounded collections were designed with the following goals:

1. **Memory Safety**: Prevent dynamic memory allocation in ``no_std`` environments
2. **Predictable Resource Usage**: Fixed capacity prevents unbounded memory growth
3. **Verification**: Support for checksumming and validation at different levels
4. **Platform Optimization**: Leverage platform-specific features for better performance
5. **Compatibility**: Provide API similar to ``heapless`` crate for easy adoption

Core Bounded Collections
-----------------------

.. list-table::
   :widths: 25 75
   :header-rows: 1

   * - Collection
     - Description
   * - ``BoundedVec<T, N, P>``
     - Fixed-capacity vector that stores elements contiguously
   * - ``BoundedStack<T, N, P>``
     - Fixed-capacity stack with LIFO semantics
   * - ``BoundedQueue<T, N, P>``
     - Fixed-capacity queue with FIFO semantics
   * - ``BoundedMap<K, V, N, P>``
     - Fixed-capacity key-value store
   * - ``BoundedSet<T, N, P>``
     - Fixed-capacity collection of unique elements
   * - ``BoundedDeque<T, N, P>``
     - Fixed-capacity double-ended queue
   * - ``BoundedBitSet<N>``
     - Fixed-capacity bit set with efficient storage
   * - ``BoundedString<N, P>``
     - Fixed-capacity UTF-8 string
   * - ``WasmName<N, P>``
     - Fixed-capacity string for WebAssembly identifiers

Type Parameters
--------------

All bounded collections share a common set of type parameters:

- ``T``: Element type (must implement traits like ``Checksummable``, ``ToBytes``, ``FromBytes``, etc.)
- ``N``: Capacity (const generic parameter)
- ``P``: Memory provider type (implements ``MemoryProvider`` trait)

Builder Pattern
--------------

A builder pattern is provided to simplify the creation of bounded collections:

.. code-block:: rust

    // Create a BoundedVec with the builder pattern
    let vec_builder = BoundedBuilder::<u32, 10, NoStdProvider>::new()
        .with_verification_level(VerificationLevel::Critical);
        
    let mut vec = vec_builder.build_vec().unwrap();

    // Create a BoundedString with a builder
    let string_builder = StringBuilder::<128, NoStdProvider>::new()
        .with_content("Hello, world!")
        .with_truncation(true);
        
    let string = string_builder.build_string().unwrap();

    // Create an optimized platform provider
    let provider_builder = PlatformOptimizedProviderBuilder::new()
        .with_size(4096)
        .with_verification_level(VerificationLevel::Full)
        .with_optimization(MemoryOptimization::HardwareAcceleration);
        
    let provider = provider_builder.build();

Memory Providers
---------------

Memory providers abstract the storage mechanism for collections:

- ``NoStdProvider``: Basic provider for all environments
- ``MacOSOptimizedProvider``: Provider optimized for macOS
- ``LinuxOptimizedProvider``: Provider optimized for Linux

Platform-Specific Optimizations
-------------------------------

The bounded collections leverage platform-specific optimizations for better performance:

1. **Hardware Acceleration**:
   - Uses SIMD instructions where available
   - Vectorized operations for bulk data transfer

2. **Alignment Optimization**:
   - Aligns memory access to platform-preferred boundaries
   - Reduces cache misses

3. **Memory Protection**:
   - Uses platform memory protection mechanisms
   - Enhances security for sensitive data

4. **Secure Zeroing**:
   - Ensures sensitive data is properly cleared
   - Prevents optimization removal by compilers

Verification Levels
------------------

Collections support different verification levels:

- ``Off``: No verification, maximum performance
- ``Critical``: Verify only critical operations
- ``Full``: Verify all operations and maintain checksums
- ``Redundant``: Add redundant verifications for safety-critical systems

Usage Examples
-------------

Basic Usage
~~~~~~~~~~

.. code-block:: rust

    // Create a queue with standard provider
    let provider = NoStdProvider::new(1024, VerificationLevel::Critical);
    let mut queue = BoundedQueue::<u32, 5, NoStdProvider>::new(provider).unwrap();
    
    // Add elements
    for i in 0..5 {
        queue.enqueue(i).unwrap();
    }
    
    // Dequeue elements
    while let Some(value) = queue.dequeue().unwrap() {
        println!("Got: {}", value);
    }

Platform-Optimized Usage
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

    // Create a platform-optimized queue (macOS example)
    let mut queue = OptimizedQueue::<u32, 100>::new(
        MacOSOptimizedProvider::new(1024, VerificationLevel::Critical)
    ).unwrap();
    
    // Operations are the same as the standard version
    for i in 0..100 {
        queue.enqueue(i).unwrap();
    }

Performance Considerations
-------------------------

The bounded collections are designed for predictable performance:

1. **Fixed-Time Operations**:
   - Most operations have O(1) time complexity
   - No hidden allocations that could cause unpredictable delays

2. **Memory Usage**:
   - Memory usage is known at compile time
   - No runtime allocation or deallocation overhead

3. **Verification Overhead**:
   - Higher verification levels add some performance overhead
   - Critical systems may prefer higher verification at the cost of speed

Safety Features
--------------

1. **Memory Safety**:
   - No dynamic allocation
   - Bounds checking on all operations
   - Panic-free error handling

2. **Data Integrity**:
   - Checksum verification to detect corruption
   - Different verification levels for different needs

3. **Secure Operations**:
   - Platform-specific secure memory operations
   - Proper clearing of sensitive data

Testing Strategy
---------------

The bounded collections are tested at multiple levels:

1. **Unit Tests**:
   - Verify individual operation correctness
   - Test error conditions

2. **Integration Tests**:
   - Test interaction between collections
   - Verify platform optimizations

3. **Performance Tests**:
   - Benchmark against standard implementations
   - Verify optimization effectiveness