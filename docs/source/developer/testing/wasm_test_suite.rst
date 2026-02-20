=======================
WASM Test Suite (WAST)
=======================

The Kiln project includes a comprehensive WAST (WebAssembly Text) test infrastructure that provides full WebAssembly specification compliance testing. This infrastructure integrates with the existing ``kiln-test-registry`` framework and supports testing across std, no_std+alloc, and no_std environments.

.. contents:: Table of Contents
   :local:
   :depth: 2

Overview
========

The WAST test infrastructure consists of several key components:

1. **WastTestRunner**: Core test execution engine
2. **Test Registry Integration**: Leverages existing test framework
3. **Comprehensive Directive Support**: All WAST directive types
4. **Multi-Environment Support**: std, no_std+alloc, no_std compatibility
5. **Intelligent Test Categorization**: Automatic test grouping and execution strategies

Architecture
============

Test Infrastructure Components
------------------------------

.. code-block:: rust

   // Core test runner
   pub struct WastTestRunner {
       module_registry: HashMap<String, Module>,  // std only
       current_module: Option<Module>,
       stats: WastTestStats,
       resource_limits: ResourceLimits,
   }

The WAST test infrastructure is built around these key concepts:

**Test Categories**
~~~~~~~~~~~~~~~~~~~

1. **Correctness Tests** (45,678+ tests)
   - ``assert_return`` directives
   - Functional validation of WebAssembly operations
   - Parallel execution supported

2. **Error Handling Tests** (6,496+ tests)
   - ``assert_trap``: Runtime trap validation
   - ``assert_invalid``: Module validation failures
   - ``assert_malformed``: Binary format errors
   - ``assert_unlinkable``: Linking failures

3. **Integration Tests** (105+ tests)
   - ``register``: Multi-module linking
   - Cross-module communication
   - Sequential execution required

4. **Resource Tests** (15+ tests)
   - ``assert_exhaustion``: Resource limit validation
   - Stack overflow, memory exhaustion
   - Isolated execution environment

WAST Directive Support
======================

The infrastructure supports all major WAST directive types:

Core Directives
---------------

**assert_return**
~~~~~~~~~~~~~~~~~

Tests that function calls return expected values:

.. code-block:: wast

   (module
     (func (export "add") (param i32 i32) (result i32)
       (i32.add (local.get 0) (local.get 1))))
   
   (assert_return (invoke "add" (i32.const 1) (i32.const 2)) (i32.const 3))

**assert_trap**
~~~~~~~~~~~~~~~

Tests that execution traps with specific error messages:

.. code-block:: wast

   (assert_trap (invoke "div_s" (i32.const 1) (i32.const 0)) "integer divide by zero")

**assert_invalid**
~~~~~~~~~~~~~~~~~~

Tests that modules fail validation:

.. code-block:: wast

   (assert_invalid
     (module (func (throw 0)))
     "unknown tag 0")

**assert_malformed**
~~~~~~~~~~~~~~~~~~~~

Tests that binary format is malformed:

.. code-block:: wast

   (assert_malformed (module binary "") "unexpected end")

Integration Directives
-----------------------

**register**
~~~~~~~~~~~~

Registers modules for import by other modules:

.. code-block:: wast

   (module $M1 (export "func" (func ...)))
   (register "M1" $M1)
   (module $M2 (import "M1" "func" (func ...)))

**invoke**
~~~~~~~~~~

Standalone function invocation:

.. code-block:: wast

   (invoke "function_name" (i32.const 42))

Usage Guide
===========

Basic Test Execution
--------------------

**Running WAST Tests**

.. code-block:: bash

   # Run all WAST tests
   cargo test -p kiln wast_tests_new
   
   # Run with external testsuite
   export WASM_TESTSUITE=/path/to/testsuite
   cargo test -p kiln test_wast_files

**Using the Test Runner Programmatically**

.. code-block:: rust

   use kiln::tests::wast_test_runner::WastTestRunner;
   
   // Create test runner
   let mut runner = WastTestRunner::new();
   
   // Set resource limits
   runner.set_resource_limits(ResourceLimits {
       max_stack_depth: 2048,
       max_memory_size: 128 * 1024 * 1024,
       max_execution_steps: 10_000_000,
   });
   
   // Run WAST file (std only)
   #[cfg(feature = "std")]
   {
       let stats = runner.run_wast_file(&path)?;
       println!("Tests: {} passed, {} failed", stats.passed, stats.failed);
   }
   
   // Run WAST content (works in no_std)
   let wast_content = "(module (func (export \"test\") (result i32) i32.const 42))
                       (assert_return (invoke \"test\") (i32.const 42))";
   let stats = runner.run_wast_content(wast_content)?;

Test Registry Integration
-------------------------

The WAST infrastructure integrates with ``kiln-test-registry``:

.. code-block:: rust

   // Register WAST tests
   wast_test_runner::register_wast_tests();
   
   // Access global registry
   let registry = TestRegistry::global();
   
   // Run filtered tests
   registry.run_filtered_tests(Some("wast"), None, true);

Advanced Configuration
======================

Resource Limits
---------------

Configure resource limits for exhaustion testing:

.. code-block:: rust

   let limits = ResourceLimits {
       max_stack_depth: 1024,        // Maximum call stack depth
       max_memory_size: 64 << 20,    // 64MB memory limit
       max_execution_steps: 1_000_000, // Fuel limit
   };
   runner.set_resource_limits(limits);

Error Classification
--------------------

The infrastructure includes intelligent error classification:

.. code-block:: rust

   // Trap keywords
   ["divide by zero", "integer overflow", "unreachable", "out of bounds"]
   
   // Validation keywords  
   ["type mismatch", "unknown", "invalid", "malformed"]
   
   // Linking keywords
   ["unknown import", "incompatible import", "missing"]
   
   // Exhaustion keywords
   ["stack overflow", "out of fuel", "limit exceeded"]

Test Execution Strategies
=========================

Parallel Execution
------------------

Correctness tests (``assert_return``) support parallel execution:

.. code-block:: rust

   // Group tests by instruction type
   let correctness_groups = group_by_instruction_type(correctness_tests);
   
   // Execute in parallel
   correctness_groups.par_iter().for_each(|group| {
       let mut engine = StacklessEngine::new();
       group.tests.iter().for_each(|test| {
           run_assert_return_test(&mut engine, test);
       });
   });

Sequential Execution
-------------------

Error and integration tests require sequential execution:

.. code-block:: rust

   // Sequential execution for state-dependent tests
   for test in error_tests {
       let mut engine = StacklessEngine::new();
       run_error_test(&mut engine, test);
       // Engine dropped, clean state for next test
   }

No_std Compatibility
====================

Environment Support
-------------------

The WAST infrastructure supports three environments:

1. **std**: Full functionality including file I/O and module registry
2. **no_std + alloc**: Core functionality with bounded collections
3. **no_std**: Core functionality with static bounds

**Conditional Compilation**

.. code-block:: rust

   // File operations (std only)
   #[cfg(feature = "std")]
   pub fn run_wast_file(&mut self, path: &Path) -> Result<WastTestStats>
   
   // Content parsing (all environments)
   pub fn run_wast_content(&mut self, content: &str) -> Result<WastTestStats>
   
   // Module registry (std only)
   #[cfg(feature = "std")]
   module_registry: HashMap<String, Module>,

Test Statistics
===============

Comprehensive Statistics
------------------------

The test runner provides detailed statistics:

.. code-block:: rust

   #[derive(Debug, Default, Clone)]
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

Performance Metrics
------------------

Track test execution performance:

.. code-block:: bash

   # Example output
   ✅ i32.wast - 1,234 directives passed, 5 failed
   ✅ f32.wast - 2,567 directives passed, 0 failed
   ❌ memory.wast - 345 directives passed, 12 failed
   
   External testsuite: 45 files passed, 3 failed
   Final runner stats: WastTestStats {
       assert_return_count: 35678,
       assert_trap_count: 2134,
       passed: 37234,
       failed: 578
   }

Best Practices
==============

Test Organization
-----------------

1. **Categorize by Test Type**: Group tests by directive type for optimal execution
2. **Use Resource Limits**: Set appropriate limits for exhaustion testing
3. **Handle Error Cases**: Implement comprehensive error classification
4. **Support All Environments**: Ensure no_std compatibility where possible

Error Handling
--------------

1. **Trap Classification**: Use keyword matching for trap validation
2. **Tolerance for Floats**: Implement proper NaN and precision handling
3. **Resource Cleanup**: Ensure clean state between tests
4. **Graceful Degradation**: Handle missing features in no_std environments

Performance Optimization
------------------------

1. **Parallel Execution**: Use parallel execution for independent tests
2. **Smart Filtering**: Filter tests based on capability detection
3. **Resource Management**: Configure appropriate resource limits
4. **Batching**: Group related tests for better cache locality

Troubleshooting
===============

Common Issues
-------------

**Environment Variable Not Set**

.. code-block:: bash

   # Set testsuite path
   export WASM_TESTSUITE=/path/to/testsuite
   
   # Or use external directory
   ln -s /path/to/testsuite external/testsuite

**Compilation Errors**

.. code-block:: bash

   # Check feature compatibility
   cargo check --features std
   cargo check --no-default-features --features alloc
   cargo check --no-default-features

**Test Failures**

.. code-block:: bash

   # Run with debug output
   RUST_LOG=debug cargo test wast_tests_new -- --nocapture
   
   # Run specific test
   cargo test -p kiln test_external_testsuite -- --nocapture

Debug Information
-----------------

Enable detailed logging for debugging:

.. code-block:: rust

   // Debug directive execution
   match self.execute_directive(&mut engine, &mut directive) {
       Ok(info) => {
           println!("✓ {} ({})", info.directive_name, info.test_type);
       }
       Err(e) => {
           eprintln!("✗ Failed: {}", e);
       }
   }

Integration Examples
===================

Custom Test Execution
---------------------

.. code-block:: rust

   use kiln::tests::wast_test_runner::{WastTestRunner, WastTestType};
   
   fn run_custom_wast_test() -> Result<()> {
       let mut runner = WastTestRunner::new();
       
       let wast_content = r#"
           (module
             (func (export "factorial") (param i32) (result i32)
               local.get 0
               i32.const 1
               i32.le_s
               if (result i32)
                 i32.const 1
               else
                 local.get 0
                 local.get 0
                 i32.const 1
                 i32.sub
                 call 0
                 i32.mul
               end))
           
           (assert_return (invoke "factorial" (i32.const 5)) (i32.const 120))
           (assert_return (invoke "factorial" (i32.const 0)) (i32.const 1))
       "#;
       
       let stats = runner.run_wast_content(wast_content)?;
       assert_eq!(stats.passed, 2);
       assert_eq!(stats.failed, 0);
       
       Ok(())
   }

Test Suite Integration
---------------------

.. code-block:: rust

   use kiln_test_registry::{TestSuite, TestResult};
   
   fn create_wast_test_suite() -> TestSuite {
       let mut suite = TestSuite::new("WAST Compliance");
       
       suite.add_test("basic_arithmetic", || {
           let mut runner = WastTestRunner::new();
           let content = "(module (func (export \"add\") (param i32 i32) (result i32) 
                                    (i32.add (local.get 0) (local.get 1))))
                          (assert_return (invoke \"add\" (i32.const 1) (i32.const 2)) (i32.const 3))";
           
           match runner.run_wast_content(content) {
               Ok(stats) if stats.failed == 0 => TestResult::success(),
               Ok(stats) => TestResult::failure(format!("{} tests failed", stats.failed)),
               Err(e) => TestResult::failure(e.to_string()),
           }
       })?;
       
       suite
   }

Future Enhancements
==================

Planned Improvements
-------------------

1. **Test Parallelization**: Further optimize parallel execution strategies
2. **Coverage Analysis**: Integration with test coverage reporting
3. **Performance Benchmarking**: Automated performance regression detection
4. **Extended Proposals**: Support for cutting-edge WebAssembly proposals
5. **Test Generation**: Automated test case generation from specifications

Extensibility
-------------

The infrastructure is designed for extensibility:

.. code-block:: rust

   // Custom directive handlers
   impl WastTestRunner {
       pub fn add_custom_directive_handler<F>(&mut self, handler: F)
       where
           F: Fn(&WastDirective) -> Result<WastDirectiveInfo>,
       {
           // Custom directive handling logic
       }
   }

Contributing
============

Test Development Guidelines
---------------------------

When adding new WAST functionality:

1. **Support All Environments**: Ensure std, no_std+alloc, and no_std compatibility
2. **Add Comprehensive Tests**: Include unit tests for all new functionality  
3. **Update Documentation**: Document new features and usage patterns
4. **Performance Considerations**: Optimize for the expected test load
5. **Error Handling**: Implement robust error classification and reporting

Code Review Checklist
---------------------

- [ ] All directive types properly handled
- [ ] No_std compatibility maintained
- [ ] Error messages are descriptive
- [ ] Resource limits respected
- [ ] Test statistics updated correctly
- [ ] Documentation updated
- [ ] Performance impact assessed

Conclusion
==========

The Kiln WAST test infrastructure provides comprehensive WebAssembly specification compliance testing with:

- **Complete directive support** for all WAST test types
- **Multi-environment compatibility** across std, no_std+alloc, and no_std
- **Intelligent test categorization** and execution strategies
- **Integration with existing test framework** for seamless adoption
- **Comprehensive error handling** and classification
- **Performance optimization** through parallel execution
- **Detailed statistics and reporting** for test analysis

This infrastructure ensures that Kiln maintains high WebAssembly specification compliance while supporting diverse deployment environments from embedded systems to server applications.