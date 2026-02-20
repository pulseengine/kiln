=================
Migration Guides
=================

This section contains migration guides for various refactoring and improvement efforts in the Kiln codebase.

.. contents:: Table of Contents
   :local:
   :depth: 2

Atomic Memory Operations Migration
----------------------------------

Background
~~~~~~~~~~

A critical vulnerability was identified in the Kiln memory safety implementation: bit flips can occur in the time window between writing data to memory and calculating/updating its checksum. This vulnerability can lead to undetected memory corruption, as the checksum doesn't accurately reflect the actual data state.

To address this issue, a new ``AtomicMemoryOps`` component has been implemented that ensures atomic operations for memory writes and checksum calculations using mutex-based synchronization.

Operations That Should Be Updated
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The following types of operations should be migrated to use the new ``AtomicMemoryOps`` implementation:

1. Direct calls to ``SliceMut::data_mut()`` followed by ``SliceMut::update_checksum()``
2. Memory write operations in ``SafeMemoryHandler``
3. Any critical data integrity verification operations in:

   - ``BoundedVec``
   - ``BoundedStack``
   - ``Memory`` operations (especially ``Memory::write`` and ``Memory::copy``)
   - Resource management operations
   - Component data serialization/deserialization

Migration Steps
~~~~~~~~~~~~~~~

**1. Update Import Statements**

Add the following import to your module::

    use kiln_foundation::prelude::{AtomicMemoryOps, AtomicMemoryExt};

**2. For Existing SafeMemoryHandler Users**

Convert your existing ``SafeMemoryHandler<P>`` to an ``AtomicMemoryOps<P>``::

    // Before
    let handler = SafeMemoryHandler::new(provider);

    // After
    let atomic_ops = handler.into_atomic_ops().unwrap();
    // OR directly from provider
    let atomic_ops = provider.into_atomic_ops().unwrap();

**3. Replace Write + Checksum Patterns**

Replace sequences of write + checksum operations with atomic alternatives::

    // Before
    let mut slice = handler.get_slice_mut(offset, len)?;
    let slice_data = slice.data_mut()?;
    slice_data.copy_from_slice(data);
    slice.update_checksum();

    // After
    atomic_ops.atomic_write_with_checksum(offset, data)?;

**4. Replace Copy Operations**

Update copy operations to use atomic alternatives::

    // Before
    handler.copy_within(src_offset, dst_offset, len)?;

    // After
    atomic_ops.atomic_copy_within(src_offset, dst_offset, len)?;

**5. For Memory Implementation**

The ``Memory`` struct should adopt atomic operations for critical write operations::

    // Before
    self.data.write_data(addr, data)?;
    if self.verification_level == VerificationLevel::Full {
        self.data.verify_integrity()?;
    }

    // After
    let atomic_ops = self.data.into_atomic_ops()?;
    atomic_ops.atomic_write_with_checksum(addr, data)?;

VerificationLevel Compatibility
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The ``AtomicMemoryOps`` implementation respects the existing ``VerificationLevel`` settings:

- ``VerificationLevel::Off``: No checksumming performed
- ``VerificationLevel::Sampling``: Checksums calculated probabilistically
- ``VerificationLevel::Standard``: Checksums calculated at key operations
- ``VerificationLevel::Full``: Checksums always calculated and verified

Thread Safety Considerations
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

- ``AtomicMemoryOps`` is thread-safe for both read and write operations
- The internal mutex ensures that only one thread can perform write operations at a time
- Multiple concurrent reads are still allowed when using borrow_slice
- The implementation minimizes lock contention by releasing locks as soon as operations complete

Performance Implications
~~~~~~~~~~~~~~~~~~~~~~~~

Using ``AtomicMemoryOps`` adds a small overhead due to mutex synchronization, but this is necessary to ensure data integrity. The performance impact should be minimal for most use cases, especially compared to the safety benefits.

No_std Compatibility
~~~~~~~~~~~~~~~~~~~~

The ``AtomicMemoryOps`` implementation is fully compatible with no_std environments:

- Uses ``core::sync::atomic`` and ``kiln_sync::mutex`` for synchronization
- Does not require heap allocations
- Maintains compact memory footprint

Testing Your Migration
~~~~~~~~~~~~~~~~~~~~~~

After migrating to ``AtomicMemoryOps``, verify that:

1. All critical memory operations with checksumming use atomic operations
2. Data integrity verification still passes
3. Unit tests continue to pass
4. Performance remains acceptable

Example Implementation
~~~~~~~~~~~~~~~~~~~~~~

::

    use kiln_foundation::prelude::{AtomicMemoryOps, AtomicMemoryExt, NoStdProvider};

    // Create a provider
    let provider = NoStdProvider::<1024>::new();

    // Create atomic memory operations handler
    let atomic_ops = provider.into_atomic_ops().unwrap();

    // Write data with atomic checksumming
    let data = [1, 2, 3, 4, 5];
    atomic_ops.atomic_write_with_checksum(0, &data).unwrap();

    // Read data back
    let slice = atomic_ops.borrow_slice(0, data.len()).unwrap();
    let read_data = slice.data().unwrap();

    assert_eq!(read_data, &data);

Package Rename Migration (kiln-foundation)
-----------------------------------------

Background
~~~~~~~~~~

The ``kiln-foundation`` package needs to be renamed to ``kiln-foundation`` for consistency with the broader ecosystem naming conventions. Due to the complexity of the codebase and number of cross-dependencies, an incremental migration approach is required.

Migration Phases
~~~~~~~~~~~~~~~~

**Phase 1: Transition Package Setup** (Completed)

- ✅ Create migration documentation
- ✅ Create ``kiln-foundation`` with updated package metadata
- ✅ Create ``kiln-foundation-transition`` package for backwards compatibility
- ✅ Update workspace configuration in root ``Cargo.toml``

**Phase 2: Prepare Source Migration**

1. Copy one module at a time from ``kiln-foundation`` to ``kiln-foundation`` and fix any errors

   Start with core modules that have minimal dependencies:
   
   - prelude.rs (fix imports)
   - bounded.rs
   - traits.rs
   - types.rs
   - values.rs
   - verification.rs
   
   Then move to more complex modules:
   
   - safe_memory.rs
   - component modules
   - other modules

2. Fix cross-module references and imports:

   - Address issues with ``MAX_WASM_NAME_LENGTH`` and other cfg-gated constants
   - Fix duplicate imports and references
   - Ensure feature flags work correctly

**Phase 3: Fix API Consistency**

Update references in dependent crates one at a time:

- kiln-error
- kiln-sync
- kiln-format
- kiln-decoder
- kiln-runtime
- kiln-component
- kiln-host
- kiln-intercept

Fix import statements from ``kiln_foundation`` to ``kiln_foundation``.

**Phase 4: Testing and Verification**

- Ensure all crates compile successfully
- Run the test suite for each crate
- Run integration tests
- Validate feature combinations

**Phase 5: Final Implementation**

- Remove ``kiln-foundation`` crate completely
- Keep only ``kiln-foundation`` and ``kiln-foundation-transition`` (for backward compatibility)
- Update documentation

Recommendations
~~~~~~~~~~~~~~~

1. **Module-by-Module Approach**: Rather than trying to migrate everything at once, focus on one module at a time, starting with those with the fewest dependencies.

2. **Fix Core Modules First**: The prelude, bounded collections, and basic types should be prioritized as they are used extensively.

3. **Incremental Testing**: After each module is migrated, compile the codebase to catch errors early.

4. **Feature Flag Consistency**: Pay special attention to feature-gated code, ensuring that features are defined consistently across all crates.

5. **Update One Dependent Crate at a Time**: After core modules are working, update dependent crates one by one, starting with the lowest-level ones.

Immediate Action Items
~~~~~~~~~~~~~~~~~~~~~~

1. Fix the ``prelude.rs`` in ``kiln-foundation`` to ensure it correctly handles imports for both std and no_std environments
2. Address the cfg-gated constants like ``MAX_WASM_NAME_LENGTH``
3. Fix duplicate imports and references
4. Update dependent crates to use ``kiln-foundation`` instead of ``kiln-foundation``

This incremental approach will help manage the complexity of the migration and ensure a stable transition to the new naming.

Build System Migration (Justfile to xtasks)
-------------------------------------------

The project has migrated from Justfile to xtasks for improved build management and cross-platform compatibility. See the build system documentation for details.

Memory Subsystem Rework
-----------------------

A comprehensive rework of the memory subsystem has been implemented to improve type consistency and safety. Key changes include:

1. Consistent use of ``u32`` for WebAssembly spec compliance
2. Proper conversion between ``u32`` and ``usize`` for Rust memory operations
3. Improved error handling for memory operations
4. Better support for no_std environments

See the memory rework documentation for implementation details.