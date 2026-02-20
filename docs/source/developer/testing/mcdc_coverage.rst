============================
MC/DC Coverage for Kiln
============================

.. image:: ../../_static/icons/testing.svg
   :width: 64px
   :align: center
   :alt: MC/DC Coverage Icon

Modified Condition/Decision Coverage (MC/DC) is a critical requirement for safety-critical systems (ASIL-D, SIL-3). This guide explains how to achieve and measure MC/DC coverage in the Kiln project.

.. contents:: On this page
   :local:
   :depth: 2

Overview
--------

MC/DC is a white-box testing technique that ensures:

- Every condition in a decision has been shown to independently affect the decision's outcome
- Each condition has been evaluated to both true and false
- Each decision has been evaluated to both true and false
- Every point of entry and exit has been invoked

This coverage level is required for:

- **ISO 26262 ASIL-D**: Automotive functional safety
- **DO-178C DAL-A**: Aviation software
- **IEC 61508 SIL-3/4**: Industrial safety systems

Requirements
------------

MC/DC coverage in Rust requires:

1. **Rust Nightly**: MC/DC requires nightly Rust with specific features
2. **LLVM 18+**: MC/DC support requires LLVM 18 or later
3. **cargo-llvm-cov**: Version 0.5.0+ with MC/DC support

Setup
-----

Install Nightly Rust with Coverage Components
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   rustup install nightly
   rustup component add llvm-tools-preview --toolchain nightly
   
   # Install cargo-llvm-cov with MC/DC support
   cargo +nightly install cargo-llvm-cov --version ">=0.5.0"

Configure MC/DC Coverage
~~~~~~~~~~~~~~~~~~~~~~~~

Create ``.cargo/config.toml`` for MC/DC-specific settings:

.. code-block:: toml

   [build]
   rustflags = ["-C", "instrument-coverage", "-C", "llvm-args=-enable-mcdc"]
   
   [target.'cfg(all())']
   rustflags = ["-C", "instrument-coverage", "-C", "llvm-args=-enable-mcdc"]

MC/DC Test Design
-----------------

Basic Principles
~~~~~~~~~~~~~~~

For MC/DC to be effective, tests must exercise all condition combinations where each condition independently affects the outcome.

**Example**: For condition ``(a && b) || (c && d)``

MC/DC requires testing combinations where changing each variable independently changes the result:

.. code-block:: rust

   #[cfg(test)]
   mod mcdc_tests {
       use super::*;

       #[test]
       fn test_complex_condition_mcdc() {
           // For condition: (a && b) || (c && d)
           // MC/DC requires testing all combinations where each condition
           // independently affects the outcome
           
           // Test cases for MC/DC coverage:
           assert!(evaluate(true, true, false, false));   // a && b = true
           assert!(!evaluate(false, true, false, false)); // a affects outcome
           assert!(!evaluate(true, false, false, false)); // b affects outcome
           assert!(evaluate(false, false, true, true));   // c && d = true
           assert!(!evaluate(false, false, false, true)); // c affects outcome
           assert!(!evaluate(false, false, true, false)); // d affects outcome
       }
   }

Kiln-Specific MC/DC Patterns
---------------------------

Memory Safety Conditions
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod memory_safety_mcdc {
       use super::*;
       
       #[test]
       fn test_allocation_safety_mcdc() {
           // Test condition: (size > 0) && (size <= budget) && (provider.available())
           
           let budget = 1024;
           let mut provider = TestProvider::new(budget);
           
           // MC/DC test cases:
           // Case 1: All true
           assert!(safe_allocate(512, budget, &provider).is_ok());
           
           // Case 2: size > 0 affects outcome (false)
           assert!(safe_allocate(0, budget, &provider).is_err());
           
           // Case 3: size <= budget affects outcome (false)
           assert!(safe_allocate(2048, budget, &provider).is_err());
           
           // Case 4: provider.available() affects outcome (false)
           provider.set_unavailable();
           assert!(safe_allocate(512, budget, &provider).is_err());
       }
   }

Component Validation Conditions
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod component_validation_mcdc {
       use super::*;
       
       #[test]
       fn test_component_safety_mcdc() {
           // Test condition: component.is_valid() && 
           //                 safety_level.is_sufficient() && 
           //                 resources.are_available()
           
           // MC/DC test cases for component safety validation
           let valid_component = create_valid_component();
           let sufficient_safety = SafetyLevel::ASIL_C;
           let available_resources = create_available_resources();
           
           // All conditions true
           assert!(validate_component(&valid_component, sufficient_safety, &available_resources));
           
           // component.is_valid() independently affects outcome
           let invalid_component = create_invalid_component();
           assert!(!validate_component(&invalid_component, sufficient_safety, &available_resources));
           
           // safety_level.is_sufficient() independently affects outcome
           let insufficient_safety = SafetyLevel::QM;
           assert!(!validate_component(&valid_component, insufficient_safety, &available_resources));
           
           // resources.are_available() independently affects outcome
           let unavailable_resources = create_unavailable_resources();
           assert!(!validate_component(&valid_component, sufficient_safety, &unavailable_resources));
       }
   }

Resource Management Conditions
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod resource_mcdc {
       use super::*;
       
       #[test]
       fn test_resource_allocation_mcdc() {
           // Test complex resource allocation condition:
           // (handle.is_valid()) && 
           // (crate_budget.has_capacity(size)) && 
           // (global_budget.has_capacity(size)) &&
           // (!resource_exists(handle))
           
           let valid_handle = ResourceHandle::new(42);
           let mut crate_budget = CrateBudget::new(1024);
           let mut global_budget = GlobalBudget::new(8192);
           let size = 256;
           
           // All conditions true - allocation succeeds
           assert!(allocate_resource(valid_handle, size, &mut crate_budget, &mut global_budget).is_ok());
           
           // handle.is_valid() affects outcome
           let invalid_handle = ResourceHandle::invalid();
           assert!(allocate_resource(invalid_handle, size, &mut crate_budget, &mut global_budget).is_err());
           
           // crate_budget.has_capacity() affects outcome
           crate_budget.consume(1024); // Exhaust crate budget
           assert!(allocate_resource(valid_handle, size, &mut crate_budget, &mut global_budget).is_err());
           
           // global_budget.has_capacity() affects outcome
           crate_budget = CrateBudget::new(1024); // Reset crate budget
           global_budget.consume(8192); // Exhaust global budget
           assert!(allocate_resource(valid_handle, size, &mut crate_budget, &mut global_budget).is_err());
           
           // resource_exists() affects outcome (resource already exists)
           global_budget = GlobalBudget::new(8192); // Reset global budget
           allocate_resource(valid_handle, size, &mut crate_budget, &mut global_budget).unwrap();
           assert!(allocate_resource(valid_handle, size, &mut crate_budget, &mut global_budget).is_err());
       }
   }

Running MC/DC Coverage
---------------------

Basic Coverage Collection
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Run tests with MC/DC coverage
   cargo +nightly llvm-cov --mcdc --html --output-dir mcdc-coverage test

   # Generate MC/DC report in different formats
   cargo +nightly llvm-cov --mcdc --json --output-path mcdc-report.json test
   cargo +nightly llvm-cov --mcdc --lcov --output-path mcdc-report.lcov test

Workspace-Wide Coverage
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Run MC/DC coverage for entire workspace
   cargo +nightly llvm-cov --mcdc --workspace --html --output-dir workspace-mcdc test

   # Exclude non-safety-critical crates
   cargo +nightly llvm-cov --mcdc --workspace \
     --exclude kiln-debug \
     --html --output-dir safety-mcdc test

Safety-Critical Subset
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Focus on safety-critical crates only
   cargo +nightly llvm-cov --mcdc \
     --package kiln-foundation \
     --package kiln-runtime \
     --package kiln-component \
     --package kiln-memory \
     --html --output-dir asil-mcdc test

Configuration for Different ASIL Levels
---------------------------------------

ASIL-A/B Configuration
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: toml

   # .cargo/config.toml for ASIL-A/B
   [build]
   rustflags = [
       "-C", "instrument-coverage",
       "-C", "llvm-args=-enable-mcdc",
       "-C", "llvm-args=-mcdc-level=basic"
   ]

ASIL-C/D Configuration  
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: toml

   # .cargo/config.toml for ASIL-C/D
   [build]
   rustflags = [
       "-C", "instrument-coverage", 
       "-C", "llvm-args=-enable-mcdc",
       "-C", "llvm-args=-mcdc-level=comprehensive",
       "-C", "llvm-args=-mcdc-verification=strict"
   ]

MC/DC Metrics and Reporting
---------------------------

Coverage Thresholds
~~~~~~~~~~~~~~~~~~

.. list-table:: MC/DC Coverage Requirements by ASIL Level
   :header-rows: 1
   :widths: 20 30 50

   * - ASIL Level
     - MC/DC Requirement
     - Notes
   * - ASIL-A
     - Decision Coverage
     - MC/DC not required
   * - ASIL-B
     - Decision Coverage
     - MC/DC recommended
   * - ASIL-C
     - MC/DC Required
     - ≥95% MC/DC coverage
   * - ASIL-D
     - MC/DC Required
     - ≥100% MC/DC coverage for safety functions

Report Analysis
~~~~~~~~~~~~~~

Key metrics to track:

1. **Decision Coverage**: Percentage of decisions exercised
2. **Condition Coverage**: Percentage of conditions exercised
3. **MC/DC Coverage**: Percentage of conditions with independent effect demonstrated
4. **Branch Coverage**: Percentage of execution branches taken

.. code-block:: bash

   # Generate comprehensive report with metrics
   cargo +nightly llvm-cov --mcdc --summary-only test

Automation and CI Integration
-----------------------------

GitHub Actions Workflow
~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: yaml

   name: MC/DC Coverage
   
   on: [push, pull_request]
   
   jobs:
     mcdc-coverage:
       runs-on: ubuntu-latest
       steps:
         - uses: actions/checkout@v4
         
         - name: Install Rust Nightly
           uses: dtolnay/rust-toolchain@nightly
           with:
             components: llvm-tools-preview
             
         - name: Install cargo-llvm-cov
           run: cargo install cargo-llvm-cov --version ">=0.5.0"
           
         - name: Run MC/DC Coverage
           run: |
             cargo +nightly llvm-cov --mcdc --workspace \
               --exclude kiln-debug \
               --html --output-dir mcdc-coverage test
               
         - name: Upload Coverage Report
           uses: actions/upload-artifact@v4
           with:
             name: mcdc-coverage-report
             path: mcdc-coverage/

Coverage Gates
~~~~~~~~~~~~~

.. code-block:: bash

   #!/bin/bash
   # check-mcdc-coverage.sh
   
   REQUIRED_COVERAGE=95
   
   # Run MC/DC coverage and extract percentage
   COVERAGE=$(cargo +nightly llvm-cov --mcdc --summary-only test | \
             grep "TOTAL" | awk '{print $4}' | sed 's/%//')
   
   if (( $(echo "$COVERAGE < $REQUIRED_COVERAGE" | bc -l) )); then
       echo "MC/DC coverage ($COVERAGE%) below required threshold ($REQUIRED_COVERAGE%)"
       exit 1
   fi
   
   echo "MC/DC coverage ($COVERAGE%) meets requirement"

Best Practices
--------------

Test Design Guidelines
~~~~~~~~~~~~~~~~~~~~~

1. **Independent Conditions**: Ensure each condition can independently affect the outcome
2. **Complete Coverage**: Test all true/false combinations for each condition
3. **Edge Cases**: Include boundary conditions and error cases
4. **Realistic Scenarios**: Use realistic input data and system states

Documentation Requirements
~~~~~~~~~~~~~~~~~~~~~~~~~~

For safety certification, document:

1. **MC/DC Test Design**: Rationale for test case selection
2. **Coverage Analysis**: Analysis of achieved coverage levels
3. **Gap Analysis**: Explanation of any uncovered conditions
4. **Traceability**: Mapping from requirements to MC/DC test cases

Common Pitfalls
~~~~~~~~~~~~~~~

1. **Short-Circuit Evaluation**: Rust's ``&&`` and ``||`` operators short-circuit
2. **Compiler Optimizations**: May eliminate conditions, affecting coverage
3. **Macro Expansion**: Generated code may not achieve desired coverage
4. **Unreachable Code**: Dead code paths cannot achieve MC/DC coverage

Troubleshooting
--------------

Coverage Not Generated
~~~~~~~~~~~~~~~~~~~~~

Check:

1. Nightly Rust version has LLVM 18+ support
2. Coverage flags are properly set in ``.cargo/config.toml``
3. Tests are actually running (not being skipped)

Low Coverage Numbers
~~~~~~~~~~~~~~~~~~~

Investigate:

1. Short-circuit evaluation masking conditions
2. Compiler optimizations eliminating branches  
3. Missing test cases for specific condition combinations
4. Unreachable code paths

See Also
--------

- :doc:`formal_verification_guide` - Formal verification with KANI
- :doc:`../safety/test_cases` - Safety test case documentation
- :doc:`wasm_test_suite` - WebAssembly test suite integration
- :doc:`../../qualification/safety_analysis` - Safety analysis requirements