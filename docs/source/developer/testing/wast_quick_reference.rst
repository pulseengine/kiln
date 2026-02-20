===========================
WAST Testing Quick Reference
===========================

This is a quick reference guide for using the Kiln WAST test infrastructure.

.. contents:: Quick Navigation
   :local:
   :depth: 2

Quick Start
===========

Basic Usage
-----------

.. code-block:: bash

   # Run WAST tests
   cargo test -p kiln wast_tests_new
   
   # Run with external testsuite
   export WASM_TESTSUITE=/path/to/testsuite
   cargo test -p kiln test_wast_files
   
   # Run example tests
   cargo test -p kiln wast_integration_examples

Programmatic Usage
------------------

.. code-block:: rust

   use kiln::tests::wast_test_runner::WastTestRunner;
   
   let mut runner = WastTestRunner::new();
   let stats = runner.run_wast_content(wast_content)?;
   println!("Passed: {}, Failed: {}", stats.passed, stats.failed);

WAST Directive Reference
=========================

Core Test Directives
---------------------

**assert_return**
  Tests function calls return expected values
  
  .. code-block:: wast
  
     (assert_return (invoke "add" (i32.const 1) (i32.const 2)) (i32.const 3))

**assert_trap**
  Tests execution traps with specific messages
  
  .. code-block:: wast
  
     (assert_trap (invoke "div" (i32.const 1) (i32.const 0)) "integer divide by zero")

**assert_invalid**
  Tests modules fail validation
  
  .. code-block:: wast
  
     (assert_invalid (module (func (result i32) i64.const 1)) "type mismatch")

**assert_malformed**
  Tests binary format is malformed
  
  .. code-block:: wast
  
     (assert_malformed (module binary "") "unexpected end")

Integration Directives
-----------------------

**register**
  Registers modules for import
  
  .. code-block:: wast
  
     (register "M1" $module1)

**invoke**
  Standalone function call
  
  .. code-block:: wast
  
     (invoke "function" (i32.const 42))

Configuration
=============

Resource Limits
---------------

.. code-block:: rust

   runner.set_resource_limits(ResourceLimits {
       max_stack_depth: 1024,
       max_memory_size: 64 << 20,  // 64MB
       max_execution_steps: 1_000_000,
   });

Environment Variables
---------------------

.. code-block:: bash

   # WebAssembly testsuite path
   export WASM_TESTSUITE=/path/to/testsuite
   
   # Testsuite commit (set by build script)
   export WASM_TESTSUITE_COMMIT=abc123

Test Statistics
===============

Available Metrics
-----------------

.. code-block:: rust

   pub struct WastTestStats {
       pub assert_return_count: usize,
       pub assert_trap_count: usize,
       pub assert_invalid_count: usize,
       pub assert_malformed_count: usize,
       pub assert_unlinkable_count: usize,
       pub assert_exhaustion_count: usize,
       pub register_count: usize,
       pub passed: usize,
       pub failed: usize,
   }

Common Patterns
===============

Float Testing
-------------

.. code-block:: wast

   (assert_return (invoke "f32_add" (f32.const 1.5) (f32.const 2.5)) (f32.const 4.0))
   (assert_return (invoke "f32_nan") (f32.const nan))

Memory Testing
--------------

.. code-block:: wast

   (module (memory 1))
   (invoke "store" (i32.const 0) (i32.const 42))
   (assert_return (invoke "load" (i32.const 0)) (i32.const 42))

Control Flow
------------

.. code-block:: wast

   (assert_return (invoke "if_test" (i32.const 1)) (i32.const 1))
   (assert_return (invoke "loop_test" (i32.const 5)) (i32.const 15))

Error Handling
==============

Common Error Types
------------------

**Trap Errors**
  - "integer divide by zero"
  - "integer overflow"
  - "unreachable"
  - "out of bounds"

**Validation Errors**
  - "type mismatch"
  - "unknown import"
  - "invalid"

**Format Errors**
  - "malformed"
  - "unexpected end"
  - "invalid encoding"

**Linking Errors**
  - "unknown import"
  - "incompatible import"

Debugging
=========

Debug Output
------------

.. code-block:: bash

   # Run with debug output
   RUST_LOG=debug cargo test wast_tests_new -- --nocapture
   
   # Run single test
   cargo test -p kiln example_basic_wast_execution -- --nocapture

Test Analysis
-------------

.. code-block:: rust

   fn analyze_results(stats: &WastTestStats) {
       let total = stats.passed + stats.failed;
       let success_rate = (stats.passed as f64 / total as f64) * 100.0;
       println!("Success rate: {:.1}%", success_rate);
   }

Performance Tips
================

Optimization Strategies
-----------------------

1. **Parallel Execution**: Correctness tests run in parallel
2. **Smart Filtering**: Filter tests by capability
3. **Resource Management**: Set appropriate limits
4. **Batching**: Group related tests

Example Batch Execution
-----------------------

.. code-block:: rust

   // Test multiple WAST contents
   let test_cases = vec![wast1, wast2, wast3];
   for (i, wast) in test_cases.iter().enumerate() {
       let stats = runner.run_wast_content(wast)?;
       println!("Test {}: {} passed", i + 1, stats.passed);
   }

Environment Compatibility
=========================

Feature Support
---------------

+------------------+-------+-------------+--------+
| Feature          | std   | no_std+alloc| no_std |
+==================+=======+=============+========+
| File I/O         | ✅    | ❌          | ❌     |
| Module Registry  | ✅    | ❌          | ❌     |
| Content Parsing  | ✅    | ✅          | ✅     |
| Error Handling   | ✅    | ✅          | ✅     |
| Statistics       | ✅    | ✅          | ✅     |
| Resource Limits  | ✅    | ✅          | ✅     |
+------------------+-------+-------------+--------+

Conditional Usage
-----------------

.. code-block:: rust

   // File operations (std only)
   #[cfg(feature = "std")]
   let stats = runner.run_wast_file(&path)?;
   
   // Content operations (all environments)
   let stats = runner.run_wast_content(content)?;

Common Issues
=============

Troubleshooting
---------------

**"No testsuite found"**
  
  .. code-block:: bash
  
     export WASM_TESTSUITE=/path/to/testsuite
     # or
     ln -s /path/to/testsuite external/testsuite

**"Type mismatch errors"**
  
  Check Value type conversions in convert_wast_arg_core

**"Compilation errors"**
  
  .. code-block:: bash
  
     cargo check --features std
     cargo check --no-default-features

**"Test failures"**
  
  Expected during development - indicates missing implementation

Integration Examples
===================

Test Registry Integration
-------------------------

.. code-block:: rust

   use kiln_test_registry::TestRegistry;
   
   // Register WAST tests
   wast_test_runner::register_wast_tests();
   
   // Run through registry
   let registry = TestRegistry::global();
   registry.run_filtered_tests(Some("wast"), None, true);

Custom Test Suite
-----------------

.. code-block:: rust

   use kiln_test_registry::TestSuite;
   
   let mut suite = TestSuite::new("Custom WAST");
   suite.add_test("arithmetic", || {
       let mut runner = WastTestRunner::new();
       let stats = runner.run_wast_content(wast_content)?;
       if stats.failed == 0 { 
           TestResult::success() 
       } else { 
           TestResult::failure("Tests failed".to_string()) 
       }
   })?;

Best Practices
==============

Code Organization
-----------------

1. **Group by functionality**: Separate arithmetic, memory, control flow tests
2. **Use descriptive names**: Clear test function and variable names  
3. **Handle all environments**: Support std, no_std+alloc, no_std
4. **Comprehensive error handling**: Proper error classification
5. **Performance awareness**: Use parallel execution where possible

Testing Guidelines
------------------

1. **Test behavior, not implementation**
2. **Include edge cases and error conditions**
3. **Use appropriate resource limits**
4. **Verify statistics and results**
5. **Document complex test scenarios**

Links
=====

- **Detailed Documentation**: :doc:`wasm_test_suite`
- **Architecture**: :doc:`../../architecture/testing`
- **Examples**: ``kiln/tests/wast_integration_examples.rs``
- **Test Registry**: :doc:`../../../kiln-test-registry/README`