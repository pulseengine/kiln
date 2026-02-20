=============================
Formal Verification Guide
=============================

This guide explains how to work with KANI formal verification in Kiln development, write verification properties, and integrate with the CI/CD pipeline.

.. contents:: On this page
   :local:
   :depth: 2

Overview
========

Kiln uses KANI (Rust Model Checker) to mathematically prove safety properties. This provides the highest level of assurance for safety-critical code by exhaustively exploring all possible execution paths within specified bounds.

Quick Start
===========

Installation
------------

.. code-block:: bash

   # Install KANI
   cargo install --locked kani-verifier
   cargo kani setup

   # Install required Rust toolchain
   rustup toolchain install nightly-2024-01-01
   rustup component add rust-src --toolchain nightly-2024-01-01

Running Verification
---------------------

.. code-block:: bash

   # Check readiness
   ./scripts/check-kani-status.sh

   # Run all verifications (ASIL-C profile)
   ./scripts/kani-verify.sh

   # Run specific package
   ./scripts/kani-verify.sh --package kiln-integration-tests

   # Run with ASIL-D (maximum verification)
   ./scripts/kani-verify.sh --profile asil-d

Writing Verification Properties
===============================

Basic Structure
---------------

Every verification module follows this pattern:

.. code-block:: rust

   //! Verification module documentation
   
   #![cfg(any(doc, kani, feature = "kani"))]
   #![deny(clippy::all)]
   #![warn(missing_docs)]
   #![forbid(unsafe_code)]
   
   use kiln_test_registry::prelude::*;
   
   #[cfg(kani)]
   use kani;
   
   // Property implementation
   #[cfg(kani)]
   pub fn verify_my_property() {
       // Property logic here
   }
   
   // KANI harness
   #[cfg(kani)]
   #[kani::proof]
   fn kani_verify_my_property() {
       verify_my_property();
   }
   
   // Fallback test
   #[cfg(test)]
   mod tests {
       #[test]
       fn test_my_property_basic() {
           // Basic test implementation
       }
   }

Property Guidelines
-------------------

**1. Use Bounded Model Checking**

.. code-block:: rust

   pub fn verify_bounded_operation() {
       // Generate arbitrary input within bounds
       let size: usize = kani::any();
       kani::assume(size <= MAX_VERIFICATION_SIZE);
       kani::assume(size > 0);
       
       // Test the property
       let result = bounded_operation(size);
       assert!(result.is_ok(), "Operation within bounds must succeed");
   }

**2. Document Assumptions**

.. code-block:: rust

   pub fn verify_memory_allocation() {
       let budget: usize = kani::any();
       
       // Assumption: Budget is within reasonable limits
       kani::assume(budget <= MAX_VERIFICATION_MEMORY);
       // Justification: Real systems have finite memory
       // Impact: Ensures verification termination
       
       let provider = NoStdProvider::<1024>::new();
       // Test property...
   }

**3. Test Both Success and Failure Cases**

.. code-block:: rust

   pub fn verify_allocation_properties() {
       let budget: usize = kani::any();
       kani::assume(budget <= MAX_VERIFICATION_MEMORY);
       kani::assume(budget > 0);
       
       let provider = NoStdProvider::<{ MAX_VERIFICATION_MEMORY }>::new();
       
       // Property 1: Allocation within budget succeeds
       let valid_size: usize = kani::any();
       kani::assume(valid_size <= budget);
       kani::assume(valid_size > 0);
       
       let result = provider.allocate(valid_size);
       assert!(result.is_ok(), "Valid allocation must succeed");
       
       // Property 2: Allocation exceeding budget fails
       let invalid_size = budget + 1;
       let result = provider.allocate(invalid_size);
       assert!(result.is_err(), "Invalid allocation must fail");
   }

Verification Categories
=======================

Memory Safety
-------------

Verify memory allocation, bounds checking, and lifecycle:

.. code-block:: rust

   pub fn verify_memory_bounds() {
       let capacity: usize = kani::any();
       kani::assume(capacity <= MAX_VERIFICATION_CAPACITY);
       kani::assume(capacity > 0);
       
       let provider = NoStdProvider::<1024>::new();
       let mut collection = BoundedVec::new(provider);
       
       // Fill to capacity
       for i in 0..capacity {
           let item: u32 = kani::any();
           let result = collection.push(item);
           assert!(result.is_ok(), "Push within capacity must succeed");
       }
       
       // Verify capacity is respected
       let overflow_item: u32 = kani::any();
       let result = collection.push(overflow_item);
       assert!(result.is_err(), "Push beyond capacity must fail");
   }

Concurrency Safety
------------------

Verify thread safety and atomic operations:

.. code-block:: rust

   pub fn verify_atomic_operations() {
       let initial: u32 = kani::any();
       let increment: u32 = kani::any();
       
       // Prevent overflow
       kani::assume(initial <= u32::MAX - increment);
       
       let provider = NoStdProvider::<1024>::new();
       let mut atomic_region = AtomicMemoryRegion::new(16, provider);
       
       // Store initial value
       atomic_region.store_u32(0, initial);
       
       // Perform atomic increment
       let old_value = atomic_region.fetch_and_add_u32(0, increment).unwrap();
       
       // Verify atomic semantics
       assert_eq!(old_value, initial);
       assert_eq!(atomic_region.load_u32(0), initial + increment);
   }

Resource Management
-------------------

Verify resource lifecycle and isolation:

.. code-block:: rust

   pub fn verify_resource_uniqueness() {
       let resource_count: usize = kani::any();
       kani::assume(resource_count <= MAX_VERIFICATION_RESOURCES);
       
       let provider = NoStdProvider::<4096>::new();
       let mut resource_ids = BoundedVec::new(provider);
       
       // Generate unique resource IDs
       for _ in 0..resource_count {
           let new_id: u32 = kani::any();
           
           // Check uniqueness
           for existing_id in resource_ids.iter() {
               assert_ne!(new_id, *existing_id, "Resource IDs must be unique");
           }
           
           let _ = resource_ids.push(new_id).ok();
       }
   }

Integration Testing
===================

Running in CI/CD
-----------------

The CI pipeline automatically runs formal verification:

**Pull Requests**: Quick verification (ASIL-A, ~5 minutes)

.. code-block:: yaml

   - name: Quick Verification
     run: ./scripts/kani-verify.sh --profile asil-a

**Main Branch**: Comprehensive verification (ASIL-C, ~20 minutes)

.. code-block:: yaml

   - name: Comprehensive Verification
     run: ./scripts/kani-verify.sh --profile asil-c

**Scheduled**: Maximum verification (ASIL-D, ~45 minutes)

.. code-block:: yaml

   - name: Maximum Verification
     run: ./scripts/kani-verify.sh --profile asil-d --verbose

Local Testing
-------------

Simulate CI verification locally:

.. code-block:: bash

   # Simulate quick verification (PR-style)
   ./scripts/simulate-ci.sh

   # Run comprehensive verification locally
   ./scripts/kani-verify.sh --profile asil-c --verbose

   # Test specific properties during development
   cargo kani --harness kani_verify_my_new_property

Debugging Failed Proofs
========================

Understanding Failures
-----------------------

When a proof fails, KANI provides detailed information:

.. code-block:: text

   VERIFICATION FAILED
   
   Check ID: kani_verify_memory_allocation
   Description: "Memory allocation within budget"
   
   Failed assertion: assertion failed at line 42
   Property: allocation within budget must succeed
   
   Counterexample available at: /tmp/kani-trace-xyz

Generating Counterexamples
--------------------------

.. code-block:: bash

   # Generate concrete playback for failed proof
   cargo kani --harness kani_verify_memory_allocation \
              --concrete-playbook inplace

   # This creates concrete values that trigger the failure
   # Use these values to write a targeted unit test

Creating Regression Tests
-------------------------

Convert counterexamples into unit tests:

.. code-block:: rust

   #[test]
   fn test_counterexample_regression() {
       // Values from KANI counterexample
       let budget = 1024;
       let allocation_size = 1025; // This triggered the failure
       
       let provider = NoStdProvider::<1024>::new();
       let result = provider.allocate(allocation_size);
       
       // This should fail as expected
       assert!(result.is_err(), "Allocation beyond budget should fail");
   }

Configuration and Tuning
=========================

Kani.toml Configuration
-----------------------

.. code-block:: toml

   [kani]
   # Basic settings
   enable-unstable = true
   solver = "cadical"
   parallel = 4
   default-unwind = 5
   
   # Profile-specific settings
   [profile.development]
   default-unwind = 3
   parallel = 2
   
   [profile.production]
   default-unwind = 7
   parallel = 8
   check-undefined-behavior = true

Harness-Specific Configuration
------------------------------

.. code-block:: toml

   [[harness]]
   name = "kani_verify_complex_property"
   unwind = 10  # Higher limit for complex loops
   profile = "asil-d"

Performance Optimization
------------------------

**1. Minimize Unwind Limits**

.. code-block:: rust

   // Good: Bounded loop
   for i in 0..kani::any::<usize>() {
       kani::assume(i < 10); // Limit iterations
       // loop body
   }

**2. Use Efficient Assumptions**

.. code-block:: rust

   // Good: Early assumptions
   let size: usize = kani::any();
   kani::assume(size <= 1024);  // Bound immediately
   kani::assume(size > 0);      // Avoid edge cases

**3. Limit Input Domains**

.. code-block:: rust

   // Good: Constrained inputs
   let value: u8 = kani::any();  // Smaller domain than u32
   kani::assume(value <= 100);   // Further constraint

Best Practices
==============

Property Design
---------------

1. **One Property Per Function**: Each verification function should test exactly one property
2. **Clear Property Statements**: Use descriptive assertion messages
3. **Comprehensive Coverage**: Test both positive and negative cases
4. **Realistic Bounds**: Choose bounds that reflect real usage

Code Organization
-----------------

1. **Separate Modules**: Keep verification code in dedicated modules
2. **Consistent Naming**: Use `verify_property_name` pattern
3. **Documentation**: Document each property's purpose and assumptions
4. **Version Control**: Include verification code in code reviews

Integration with Development
----------------------------

1. **Write Properties Early**: Add verification alongside implementation
2. **Test Incrementally**: Run verification during development
3. **Review Assumptions**: Regularly validate verification assumptions
4. **Monitor Performance**: Track verification time and resource usage

Common Pitfalls
===============

Avoiding Issues
---------------

**1. Unbounded Loops**

.. code-block:: rust

   // Bad: Unbounded loop
   while condition {
       // This may not terminate in verification
   }
   
   // Good: Bounded loop
   for _ in 0..kani::any::<usize>() {
       kani::assume(condition);
       // loop body
   }

**2. Overly Complex Properties**

.. code-block:: rust

   // Bad: Testing multiple properties
   pub fn verify_everything() {
       // Tests memory, concurrency, and resources
   }
   
   // Good: Focused property
   pub fn verify_memory_allocation() {
       // Tests only memory allocation
   }

**3. Missing Assumptions**

.. code-block:: rust

   // Bad: No bounds
   let size: usize = kani::any();
   let buffer = allocate(size); // May cause verification timeout
   
   // Good: Bounded input
   let size: usize = kani::any();
   kani::assume(size <= MAX_ALLOCATION_SIZE);
   let buffer = allocate(size);

Error Recovery
--------------

If verification fails or times out:

1. **Check Assumptions**: Ensure all inputs are properly bounded
2. **Reduce Complexity**: Split complex properties into simpler ones
3. **Lower Unwind Limits**: Reduce loop iteration bounds
4. **Use Sampling**: Apply verification to subset of inputs

Conclusion
==========

Formal verification with KANI provides mathematical proof of safety properties, giving the highest level of confidence in critical code. By following these guidelines, you can effectively write, maintain, and debug verification properties as part of the Kiln development process.

**Key Takeaways:**

- Use bounded model checking with realistic assumptions
- Write focused properties that test single invariants
- Integrate verification into the development workflow
- Debug failures systematically using counterexamples
- Optimize for both correctness and performance

For more details on the overall formal verification architecture, see :doc:`../../safety/formal_verification`.