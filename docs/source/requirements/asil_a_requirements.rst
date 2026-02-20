=======================
ASIL-A Requirements
=======================

This document defines the specific requirements for achieving ASIL-A compliance per ISO 26262.

.. contents:: On this page
   :local:
   :depth: 2

ASIL-A Overview
---------------

ASIL-A is the lowest safety integrity level in ISO 26262, suitable for functions where failure poses minimal risk to vehicle occupants. The Kiln project targets ASIL-A as the initial safety certification level.

**Key ASIL-A Requirements:**
- Basic hazard analysis and risk assessment
- Systematic development process
- Basic verification and validation
- Fault detection capability
- Quality management system

Memory Safety Requirements (ASIL-A)
------------------------------------

REQ_ASIL_A_MEM_001: Memory Allocation Safety
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_MEM_001  
**Title:** Capability-Based Memory Allocation  
**Priority:** High  
**Status:** Implemented  

**Requirement:**  
The Kiln runtime SHALL use capability-based memory allocation to prevent unauthorized memory access and ensure deterministic memory usage patterns.

**Implementation:**  
- Located in: `/kiln-foundation/src/capabilities/`
- Verified by: KANI harness `kani_verify_memory_budget_never_exceeded`
- Test coverage: Memory allocation tests in `kiln-foundation/tests/`

**Rationale:**  
Capability-based allocation provides the deterministic memory management required for ASIL-A certification while preventing common memory safety issues.

REQ_ASIL_A_MEM_002: Memory Budget Enforcement
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_MEM_002  
**Title:** Compile-Time Memory Budget Verification  
**Priority:** High  
**Status:** Implemented  

**Requirement:**  
The Kiln runtime SHALL enforce memory budgets at compile-time to prevent memory exhaustion during runtime execution.

**Implementation:**  
- Located in: `/kiln-foundation/src/budget_verification.rs`
- Macros: `safe_managed_alloc!` with budget checking
- Configuration: `CRATE_BUDGETS` constant arrays

**Verification:**  
- KANI harness: `kani_verify_hierarchical_budget_consistency`
- Build-time verification through `cargo-kiln` tool

REQ_ASIL_A_MEM_003: No Dynamic Memory Allocation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_MEM_003  
**Title:** Static Memory Layout Enforcement  
**Priority:** High  
**Status:** Partially Implemented  

**Requirement:**  
The Kiln runtime SHALL NOT perform dynamic memory allocation during execution in safety-critical contexts.

**Implementation:**  
- `NoStdProvider` using fixed-size arrays
- Bounded collections with compile-time capacity limits
- `#![no_std]` compilation for safety-critical targets

**Gap Analysis:**  
- Global capability context still uses `HashMap` in std mode
- Needs static array replacement for full compliance

Error Handling Requirements (ASIL-A)
-------------------------------------

REQ_ASIL_A_ERR_001: Systematic Error Classification
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_ERR_001  
**Title:** ASIL-Level Error Classification  
**Priority:** Medium  
**Status:** Implemented  

**Requirement:**  
The Kiln runtime SHALL classify all errors according to their ASIL level and handle them appropriately.

**Implementation:**  
- Located in: `/kiln-error/src/asil.rs`
- Enum: `AsilLevel` with QM through ASIL-D levels
- Error classification in `Error` struct

**Verification:**  
- Tests in: `/kiln-error/tests/asil_tests.rs`
- ASIL-tagged test framework

REQ_ASIL_A_ERR_002: Fault Detection Coverage
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_ERR_002  
**Title:** Basic Fault Detection Mechanisms  
**Priority:** Medium  
**Status:** In Progress  

**Requirement:**  
The Kiln runtime SHALL detect and respond to basic fault conditions including memory violations and resource exhaustion.

**Implementation Plan:**  
- Memory bounds checking in all allocations
- Resource limit monitoring
- Graceful degradation on fault detection

Verification Requirements (ASIL-A)
-----------------------------------

REQ_ASIL_A_VER_001: Formal Verification Coverage
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_VER_001  
**Title:** KANI Formal Verification for Critical Paths  
**Priority:** High  
**Status:** Implemented  

**Requirement:**  
The Kiln runtime SHALL use formal verification (KANI) to prove correctness of safety-critical memory operations.

**Implementation:**  
- KANI configuration: `/kiln-tests/integration/Kani.toml`
- Verification profiles for each ASIL level
- 7+ existing KANI harnesses for memory operations

**Target Coverage:**  
- Memory allocation and deallocation
- Capability verification logic
- Budget enforcement mechanisms
- Atomic operations and synchronization

REQ_ASIL_A_VER_002: Test Coverage Requirements
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_VER_002  
**Title:** Systematic Test Coverage for ASIL-A Components  
**Priority:** Medium  
**Status:** In Progress  

**Requirement:**  
The Kiln runtime SHALL achieve comprehensive test coverage for all ASIL-A classified components with systematic boundary testing.

**Target Metrics:**  
- Line coverage: ≥95% for safety-critical paths
- Branch coverage: ≥90% for decision points
- MC/DC coverage: ≥85% for complex Boolean expressions

Process Requirements (ASIL-A)
------------------------------

REQ_ASIL_A_PROC_001: Safety Development Lifecycle
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_PROC_001  
**Title:** ISO 26262 Compliant Development Process  
**Priority:** High  
**Status:** In Progress  

**Requirement:**  
The Kiln runtime development SHALL follow ISO 26262 safety development lifecycle processes.

**Implementation Plan:**  
- Safety requirements specification (this document)
- Hazard analysis and risk assessment
- Safety case development
- Verification and validation procedures

REQ_ASIL_A_PROC_002: Change Control Process
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
**ID:** REQ_ASIL_A_PROC_002  
**Title:** Safety-Critical Code Change Management  
**Priority:** Medium  
**Status:** Planned  

**Requirement:**  
All changes to safety-critical Kiln components SHALL be subject to controlled change management with safety impact assessment.

**Implementation Plan:**  
- Git-based change tracking
- Safety review process for critical components
- Regression testing requirements

Gap Analysis for ASIL-A Compliance
-----------------------------------

**Critical Gaps (Must Fix):**

1. **Dynamic Memory in std Mode**
   - Issue: Global capability context uses `HashMap`
   - Fix: Replace with static arrays
   - Timeline: Week 4-5

2. **Incomplete Fault Detection**
   - Issue: Limited fault detection mechanisms
   - Fix: Implement systematic fault detection
   - Timeline: Week 5-6

**Minor Gaps (Should Fix):**

1. **Documentation Completeness**
   - Issue: Some safety requirements lack detailed implementation links
   - Fix: Complete traceability matrix
   - Timeline: Week 7-8

2. **Process Documentation**
   - Issue: Safety development process needs formalization
   - Fix: Create process documentation
   - Timeline: Week 6-8

Compliance Status Summary
-------------------------

**Ready for ASIL-A:**
- ✅ Capability-based memory system
- ✅ Formal verification infrastructure (KANI)
- ✅ Error classification system
- ✅ Budget enforcement mechanisms

**Needs Work for ASIL-A:**
- 🔄 Complete elimination of dynamic allocation
- 🔄 Systematic fault detection implementation
- 🔄 Process documentation completion
- 🔄 Full test coverage achievement

**Timeline to ASIL-A Readiness:**
- **Week 4-6:** Technical gap resolution
- **Week 7-8:** Process and documentation completion
- **Week 9-10:** Independent review and assessment
