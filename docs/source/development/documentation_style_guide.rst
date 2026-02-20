==========================
Documentation Style Guide
==========================

This guide establishes consistent style and tone standards for all PulseEngine (Kiln Edition) documentation.

.. contents:: Table of Contents
   :local:
   :depth: 2

General Principles
==================

Accuracy First
--------------

* **Never claim features that don't exist** - Use "planned", "under development", or "not implemented"
* **Qualify all statements** - Use "currently", "as of version X", "planned for"
* **Update regularly** - Documentation must reflect actual code state

Tone and Voice
--------------

* **Professional but approachable** - Technical accuracy without being overly academic
* **Direct and concise** - Get to the point quickly
* **Honest about limitations** - Clearly state what doesn't work or isn't complete

Naming Conventions
==================

Project Name
------------

* **Official name**: PulseEngine (Kiln Edition)
* **Short forms**: PulseEngine or Kiln
* **Never use**: SentryPulse Engine, SPE_kiln, or other legacy names

Component Names
---------------

* Use exact crate names when referencing code: ``kiln-runtime``, ``kiln-component``
* Use descriptive names in prose: "the runtime", "the decoder"
* Capitalize proper nouns: WebAssembly, Component Model

Status Indicators
=================

Implementation Status
---------------------

Use these standard terms:

* **Implemented** - Feature is complete and tested
* **Partial** - Core functionality exists but incomplete
* **Design** - Architecture defined but not implemented
* **Planned** - On roadmap but no current work
* **Not Implemented** - Placeholder or stub code only

Example::

   .. warning::
      **Implementation Status**: The Component Model runtime is partially 
      implemented. Core parsing works but instantiation is under development.

Development Status Warnings
---------------------------

Place warnings prominently at the top of sections::

   .. warning::
      **Development Status**: This feature is under active development and 
      not ready for production use.

Formatting Standards
====================

reStructuredText Conventions
----------------------------

Headers
~~~~~~~

Use consistent header hierarchy::

   ================
   Document Title
   ================

   Major Section
   =============

   Subsection
   ----------

   Sub-subsection
   ~~~~~~~~~~~~~~

Code Examples
~~~~~~~~~~~~~

* Always test code examples before including them
* Mark untested or conceptual code clearly::

   .. code-block:: rust
      :caption: Conceptual Example (Not Yet Implemented)

      // This shows the intended API design
      let component = Component::parse(bytes)?;

Lists
~~~~~

* Use bullet lists for unordered items
* Use numbered lists only for sequential steps
* Indent nested lists consistently

Cross-References
~~~~~~~~~~~~~~~~

* Use ``:doc:`` for internal documentation links
* Use ``:ref:`` for section references
* Always verify links work::

   See :doc:`/getting_started/installation` for setup instructions.

Common Patterns
===============

Feature Documentation
---------------------

When documenting a feature:

1. State what it does (present tense if implemented)
2. Show current implementation status
3. Provide working example (or mark as conceptual)
4. List limitations or known issues
5. Reference related documentation

Module Documentation
--------------------

For each module:

1. Brief description of purpose
2. Implementation status warning if needed
3. Key types and traits
4. Usage examples
5. Cross-references to related modules

API Documentation
-----------------

* Document all public APIs
* Include at least one example per public function
* Note any safety requirements or panics
* Specify error conditions

Safety Documentation
====================

Certification Claims
--------------------

Always qualify certification statements::

   .. warning::
      **Certification Status**: PulseEngine is designed for safety-critical 
      systems but is NOT currently certified to any safety standard. The 
      architecture supports future ISO 26262 certification.

ASIL Requirements
-----------------

* State ASIL level as "targeted" or "designed for", not "compliant"
* Reference specific ISO 26262 clauses when applicable
* Distinguish between framework support and actual compliance

Writing Checklist
=================

Before Publishing
-----------------

.. checklist::

   □ All code examples compile and run
   □ Implementation status is accurate
   □ No false claims about features
   □ Links and cross-references verified
   □ Consistent project naming throughout
   □ Safety claims properly qualified
   □ Technical accuracy reviewed

Review Questions
----------------

* Could a user successfully use this feature based on the docs?
* Are limitations and incomplete features clearly marked?
* Would this mislead someone evaluating the project?
* Is the implementation status current?

Common Mistakes to Avoid
========================

Don't Do This
-------------

* ❌ "PulseEngine provides complete WebAssembly execution"
* ❌ "Fully compliant with ISO 26262"
* ❌ "Install from crates.io with ``cargo install kiln``"
* ❌ Using different project names in the same document
* ❌ Copying boilerplate without updating details

Do This Instead
---------------

* ✅ "PulseEngine provides WebAssembly infrastructure with execution engine under development"
* ✅ "Designed to support ISO 26262 certification"
* ✅ "Install from source (not yet published to crates.io)"
* ✅ Consistent "PulseEngine (Kiln Edition)" naming
* ✅ Tailored, accurate content for each section

Templates
=========

New Feature Documentation
-------------------------

.. code-block:: rst

   Feature Name
   ============

   Brief description of what this feature provides.

   .. warning::
      **Implementation Status**: [Implemented|Partial|Design|Planned]
      
      Additional context about current state.

   Overview
   --------

   Detailed explanation of the feature's purpose and design.

   Usage
   -----

   .. code-block:: rust
      :caption: Basic Usage

      // Working example code here

   Limitations
   -----------

   * Known limitation 1
   * Known limitation 2

   See Also
   --------

   * :doc:`related_feature`
   * :ref:`specific-section`

Maintenance Tasks
=================

Regular Reviews
---------------

* **Monthly**: Check implementation status markers
* **Per Release**: Update all version-specific information
* **Quarterly**: Full documentation accuracy audit

Update Triggers
---------------

Update documentation when:

* New features are implemented
* Implementation status changes
* APIs change
* Limitations are discovered or resolved
* Safety requirements change