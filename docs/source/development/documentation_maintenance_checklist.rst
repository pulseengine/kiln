====================================
Documentation Maintenance Checklist
====================================

This checklist ensures documentation remains accurate and useful as PulseEngine evolves.

.. contents:: Table of Contents
   :local:
   :depth: 2

Pre-Release Checklist
=====================

Before Each Release
-------------------

.. checklist::

   **Implementation Status Updates**
   
   □ Review all "under development" markers
   □ Update "partial" implementations that are now complete
   □ Add warnings for new experimental features
   □ Verify example code still compiles
   □ Check that limitations sections are current

   **Version Information**
   
   □ Update version numbers in installation guides
   □ Update compatibility matrices
   □ Review and update system requirements
   □ Update changelog/release notes references

   **Safety Documentation**
   
   □ Verify all safety claims are properly qualified
   □ Ensure no false certification claims
   □ Update ASIL compliance preparations
   □ Review safety manual for accuracy

   **Cross-References**
   
   □ Test all internal documentation links
   □ Verify external links still work
   □ Update moved or renamed sections
   □ Fix any broken references

Regular Maintenance
===================

Weekly Tasks
------------

.. checklist::

   □ Review recent code changes for documentation impact
   □ Update API documentation for new public functions
   □ Check for new TODOs in documentation
   □ Verify CI documentation build passes

Monthly Tasks
-------------

.. checklist::

   □ **Accuracy Audit**
      - Scan for outdated implementation claims
      - Verify feature descriptions match code
      - Update development status markers
   
   □ **Consistency Check**
      - Ensure project naming is consistent
      - Verify terminology usage
      - Check formatting standards
   
   □ **Example Code Review**
      - Test all code examples
      - Update for API changes
      - Mark conceptual vs working examples

Quarterly Tasks
---------------

.. checklist::

   □ **Comprehensive Review**
      - Full documentation accuracy audit
      - Review all warnings and notices
      - Update architecture diagrams
      - Verify all features documented
   
   □ **User Feedback Integration**
      - Review documentation issues/feedback
      - Clarify confusing sections
      - Add missing information
   
   □ **Safety Documentation**
      - Review ISO 26262 alignment
      - Update safety requirements
      - Verify hazard analysis current

New Feature Documentation
=========================

When Adding Features
--------------------

.. checklist::

   □ Create feature documentation following style guide
   □ Add implementation status warning if not complete
   □ Include working examples (or mark as conceptual)
   □ Document all new public APIs
   □ Add to feature matrix with correct status
   □ Update architecture documentation if needed
   □ Add cross-references to related features
   □ Update the main feature list

When Modifying Features
-----------------------

.. checklist::

   □ Update all affected documentation
   □ Revise examples to match new behavior  
   □ Update limitations or remove if fixed
   □ Check for outdated cross-references
   □ Update implementation status if changed
   □ Add migration notes if breaking changes

Critical Documentation Areas
============================

Always Verify These Sections
----------------------------

1. **Installation Guide** (``getting_started/installation.rst``)
   
   .. checklist::
      □ Build commands work
      □ Prerequisites are current
      □ No false package manager claims
      □ Platform notes accurate

2. **Feature Overview** (``overview/features.rst``)
   
   .. checklist::
      □ Implementation status accurate
      □ No false "complete" claims
      □ Development warnings present
      □ Limitations documented

3. **Safety Manual** (``safety_manual/index.rst``)
   
   .. checklist::
      □ Certification status disclaimer
      □ No false compliance claims
      □ ASIL levels marked as "targeted"
      □ SEooC assumptions current

4. **Architecture Docs** (``architecture/``)
   
   .. checklist::
      □ Component status markers
      □ Design vs implementation clear
      □ Sequence diagrams marked appropriately
      □ Test coverage metrics current

Quick Audit Commands
====================

Find Potential Issues
---------------------

Search for common problems::

   # Find "fully implemented" claims
   grep -r "fully implemented" docs/
   grep -r "complete implementation" docs/
   
   # Find outdated project names
   grep -r "SentryPulse" docs/
   grep -r "SPE_kiln" docs/
   
   # Find missing status warnings
   grep -r "\.rst:" docs/ | xargs grep -L "Implementation Status\|Development Status"
   
   # Find TODO markers
   grep -r "TODO\|FIXME\|XXX" docs/

   # Check for crates.io references
   grep -r "crates\.io" docs/
   grep -r "cargo install kiln" docs/

Documentation Health Metrics
============================

Track These Metrics
-------------------

* **False Claim Count**: Should be zero
* **Broken Links**: Should be zero  
* **Outdated Examples**: Should be zero
* **Missing Status Warnings**: Track and reduce
* **Documentation Coverage**: % of public APIs documented
* **Example Coverage**: % of features with working examples

Red Flags
---------

Immediate action needed if you find:

* ❗ Unqualified safety certification claims
* ❗ "Fully implemented" for partial features
* ❗ Installation instructions that don't work
* ❗ Code examples that don't compile
* ❗ Missing implementation status warnings
* ❗ Inconsistent project naming

Documentation Debt Tracking
===========================

Track Technical Debt
--------------------

Maintain a list of:

* Sections needing updates
* Missing documentation
* Conceptual examples needing implementation
* Diagrams needing refresh
* Cross-references to verify

Priority Levels
---------------

1. **Critical**: False claims, broken installation
2. **High**: Missing status warnings, outdated examples  
3. **Medium**: Incomplete sections, missing cross-refs
4. **Low**: Formatting issues, style inconsistencies

Post-Incident Updates
=====================

After Bug Fixes
---------------

.. checklist::

   □ Update limitations sections
   □ Remove warnings for fixed issues
   □ Update troubleshooting guides
   □ Add to changelog

After Security Issues
---------------------

.. checklist::

   □ Update security considerations
   □ Document mitigation steps
   □ Update safety documentation
   □ Review related examples

Automation Opportunities
========================

Consider Automating
-------------------

* Link checking in CI
* Example code compilation tests
* Terminology consistency checks
* Status marker validation
* Cross-reference verification

Manual Review Required
----------------------

* Implementation accuracy
* Safety claim qualification  
* Feature completeness assessment
* User experience evaluation
* Technical accuracy verification