=================================
Documentation Style Guide
=================================

.. warning::
   **Living Document**: This style guide evolves with project needs. Last updated: |today|

This guide establishes consistent writing standards for all PulseEngine documentation to ensure clarity, accuracy, and professional quality.

.. contents:: On this page
   :local:
   :depth: 2

Core Principles
===============

ALCOA-C Compliance
------------------

All documentation follows ALCOA-C principles:

- **Attributable**: Clear authorship and revision history
- **Legible**: Clear formatting and structure
- **Contemporaneous**: Written at time of work
- **Original**: First-hand information
- **Accurate**: Technically correct and verified
- **Complete**: All necessary information included

Accuracy Over Marketing
-----------------------

1. **No Overstatement**: Never claim features that don't exist
2. **Clear Status**: Always indicate development/certification status
3. **Honest Limitations**: Document what doesn't work
4. **Evidence-Based**: Support claims with references

Voice and Tone
==============

Professional Technical Writing
------------------------------

**DO:**
- Use active voice: "The runtime executes modules"
- Be direct: "This fails because..."
- Stay factual: "Performance improves by 20%"

**DON'T:**
- Use passive voice: "Modules are executed by..."
- Be vague: "This might fail..."
- Make unsubstantiated claims: "Blazingly fast"

Consistent Terminology
----------------------

**Product Names:**
- PulseEngine (Kiln Edition) - Full product name
- PulseEngine - Acceptable short form
- Kiln - When referring to technical components

**Never use:**
- Mixed terminology in same document
- "WebAssembly Runtime" as product name
- Inconsistent capitalization

Status Indicators
=================

Development Status
------------------

Always include clear status indicators:

.. code-block:: rst

   .. warning::
      **Development Status**: This feature is under active development.
      Current completion: ~15%. See :doc:`/overview/implementation_status` for details.

.. code-block:: rst

   .. note::
      **API Stability**: This API is experimental and subject to change.

Certification Status
--------------------

For safety-critical features:

.. code-block:: rst

   .. warning::
      **Certification Status**: PulseEngine is NOT currently certified.
      This documentation supports preparation for future certification.

Document Structure
==================

Standard Sections
-----------------

Every documentation page should include:

1. **Title** - Clear, descriptive heading
2. **Status Warning** - If applicable
3. **Introduction** - Brief overview (2-3 sentences)
4. **Table of Contents** - For pages > 1 screen
5. **Main Content** - Organized with clear headings
6. **See Also** - Related documentation links

Example Structure
-----------------

.. code-block:: rst

   ========================
   Component Model Overview
   ========================

   .. warning::
      **Development Status**: Component Model implementation is ~20% complete.

   This document describes PulseEngine's WebAssembly Component Model 
   implementation, providing type-safe composition of WebAssembly modules.

   .. contents:: On this page
      :local:
      :depth: 2

   Introduction
   ============

   The Component Model enables...

   [Main content sections...]

   See Also
   ========

   - :doc:`component_types` - Type system details
   - :doc:`../examples/component/index` - Component examples

Writing Guidelines
==================

Clarity Rules
-------------

1. **One Concept Per Paragraph**
2. **Examples After Explanations**
3. **Define Before Use** - Explain terms on first use
4. **Concrete Over Abstract** - Use specific examples

Code Examples
-------------

**Requirements:**
- Must compile (unless showing errors)
- Include all imports
- Add context comments
- Show expected output

.. code-block:: rst

   .. code-block:: rust

      // Demonstrate safe memory allocation
      use kiln_foundation::safe_memory::SafeVec;

      let mut vec = SafeVec::new(provider)?;
      vec.push(42)?;  // Returns error if allocation fails
      
      assert_eq!(vec[0], 42);

Lists and Tables
----------------

Use tables for structured comparison:

.. code-block:: rst

   .. list-table:: Implementation Status
      :widths: 40 20 40
      :header-rows: 1

      * - Component
        - Status
        - Notes
      * - Execution Engine
        - 15%
        - Basic instruction set only
      * - Component Model
        - 20%
        - Type definitions complete

Language Standards
==================

Technical Precision
-------------------

**Good:** "The memory allocator returns an error when allocation exceeds 64KB"

**Bad:** "The memory allocator might have issues with large allocations"

Avoid Ambiguity
---------------

**Replace vague terms:**
- "Soon" → "Target: Q2 2025"
- "Fast" → "< 10ms latency"
- "Small" → "< 1MB binary size"
- "Many" → "Supports up to 64 instances"

Safety Language
===============

For safety-critical documentation:

Required Terminology
--------------------

- **shall** - Mandatory requirement
- **should** - Recommended practice
- **may** - Optional feature
- **will** - Declaration of intent

Example Usage
-------------

.. code-block:: rst

   The runtime **shall** validate all memory accesses.
   
   Applications **should** check return values for errors.
   
   The host **may** provide custom allocators.

Cross-References
================

Internal Links
--------------

Always use Sphinx references:

.. code-block:: rst

   See :doc:`/safety_manual/index` for safety documentation.
   
   The :ref:`memory-model` section explains allocation.
   
   API details in :doc:`../api/kiln-runtime/lib`.

External Links
--------------

Include context for external references:

.. code-block:: rst

   Based on `WebAssembly Component Model`_ specification.
   
   .. _WebAssembly Component Model: https://github.com/WebAssembly/component-model

Templates
=========

The following templates are available:

- :doc:`templates/api_reference` - API documentation
- :doc:`templates/user_guide` - User-facing guides  
- :doc:`templates/design_document` - Technical designs
- :doc:`templates/safety_document` - Safety-critical docs

Version Control
===============

Documentation Changes
---------------------

1. **Atomic Commits** - One concept per commit
2. **Clear Messages** - Describe what and why
3. **Issue References** - Link to tracking issues

Example Commit
--------------

.. code-block:: text

   docs: clarify component model implementation status
   
   - Add explicit percentage complete (20%)
   - Remove misleading "ready for use" language
   - Add reference to implementation roadmap
   
   Fixes #1234

Review Process
==============

Documentation Review
--------------------

All documentation requires:

1. **Technical Review** - Accuracy verification
2. **Style Review** - Consistency check
3. **Safety Review** - For safety-critical content

Review Checklist
----------------

- [ ] No false claims or overstatements
- [ ] Status indicators present and accurate
- [ ] Consistent terminology throughout
- [ ] Code examples compile and run
- [ ] Cross-references work correctly
- [ ] Follows style guide standards

Common Issues
=============

Avoid These Mistakes
--------------------

1. **Mixed Tense** - Use present tense consistently
2. **Buried Status** - Put warnings at top
3. **Orphaned Pages** - Always link from index
4. **Stale Examples** - Test code regularly
5. **Undefined Acronyms** - Define on first use

Quick Reference
===============

Formatting Cheatsheet
---------------------

.. code-block:: rst

   **Bold** for emphasis
   ``code`` for inline code
   :doc:`path` for internal links
   
   .. warning::
      Important warnings
   
   .. note::
      Helpful information
      
   .. code-block:: rust
      // Code examples