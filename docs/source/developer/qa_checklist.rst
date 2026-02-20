=================================
Documentation Quality Checklist
=================================

.. warning::
   **Required Review**: All documentation must pass this checklist before merge.

This checklist ensures documentation meets PulseEngine quality standards and ALCOA-C principles.

.. contents:: Checklist Sections
   :local:
   :depth: 2

Pre-Submission Checklist
========================

Content Accuracy
----------------

**Technical Accuracy**

- [ ] All code examples compile and run
- [ ] API signatures match actual implementation
- [ ] Feature status accurately reflects code state
- [ ] No false claims about capabilities
- [ ] Performance metrics verified with benchmarks

**Status Indicators**

- [ ] Development status warnings present where needed
- [ ] Implementation percentages match reality
- [ ] Certification status clearly marked as "preparation only"
- [ ] Future features marked as "planned" not "available"

Writing Standards
-----------------

**Style Compliance**

- [ ] Active voice used throughout
- [ ] Present tense for current features
- [ ] No vague quantifiers (">90%", "might", "could")
- [ ] All acronyms defined on first use
- [ ] Consistent product naming (PulseEngine)

**Structure**

- [ ] Clear document title
- [ ] Table of contents for long pages
- [ ] Logical section hierarchy
- [ ] "See Also" section with relevant links
- [ ] No orphaned pages (linked from index)

Technical Requirements
----------------------

**Code Examples**

- [ ] All imports included
- [ ] Error handling shown
- [ ] Expected output documented
- [ ] Context comments explain purpose
- [ ] Follows project coding standards

**Cross-References**

- [ ] Internal links use Sphinx format
- [ ] External links include context
- [ ] No broken references
- [ ] API links point to correct versions
- [ ] Related documents linked

Safety Documentation
====================

Additional Requirements
-----------------------

For safety-critical documentation:

**Compliance**

- [ ] Safety level clearly stated (ASIL-D, SIL 3, etc.)
- [ ] Assumptions documented
- [ ] Constraints specified
- [ ] Integration requirements listed
- [ ] Traceability to requirements

**Language Standards**

- [ ] Uses "shall/should/may" correctly
- [ ] No ambiguous requirements
- [ ] Measurable criteria provided
- [ ] Test methods specified
- [ ] Verification approach documented

Review Process
==============

Review Types
------------

.. list-table:: Required Reviews by Document Type
   :widths: 30 35 35
   :header-rows: 1

   * - Document Type
     - Technical Review
     - Safety Review
   * - API Reference
     - Code owner
     - Not required
   * - User Guide
     - Developer + User
     - Not required
   * - Safety Manual
     - Safety engineer
     - Required
   * - Architecture
     - Architect + Developer
     - If safety-related

Review Checklist
----------------

**Technical Review**

- [ ] Technically accurate
- [ ] Code examples tested
- [ ] Links verified
- [ ] Terminology consistent
- [ ] Follows templates

**Editorial Review**

- [ ] Grammar and spelling correct
- [ ] Style guide followed
- [ ] Formatting consistent
- [ ] Images have alt text
- [ ] Tables properly formatted

**Safety Review** (when required)

- [ ] Safety claims verified
- [ ] Assumptions reasonable
- [ ] Constraints complete
- [ ] No safety gaps introduced
- [ ] Traceability maintained

Automated Checks
================

CI Pipeline Validation
----------------------

The following checks run automatically:

1. **Sphinx Build**
   - No warnings or errors
   - All references resolve
   - Valid RST syntax

2. **Link Checker**
   - No broken internal links
   - External links reachable
   - Anchors exist

3. **Code Example Testing**
   - Rust examples compile
   - No undefined symbols
   - Follows style guide

4. **Spell Check**
   - Technical terms in dictionary
   - No obvious typos
   - Consistent spelling

Manual Verification
-------------------

Items requiring human review:

- Technical accuracy
- Clarity and completeness
- Appropriate detail level
- User perspective
- Safety implications

Common Issues
=============

Frequent Problems
-----------------

1. **Overstatement**
   - Claiming features not implemented
   - Missing development warnings
   - Incorrect completion percentages

2. **Poor Structure**
   - Missing table of contents
   - Illogical section order
   - No related links

3. **Code Issues**
   - Examples don't compile
   - Missing error handling
   - No context provided

4. **Style Violations**
   - Passive voice
   - Inconsistent tense
   - Undefined acronyms

Resolution Guide
----------------

**Before Submission:**

1. Run through entire checklist
2. Test all code examples
3. Verify all claims
4. Check all links
5. Review in rendered form

**After Review Feedback:**

1. Address all comments
2. Re-test changed examples
3. Update related documents
4. Note changes in PR
5. Request re-review

Quality Metrics
===============

Documentation Health
--------------------

Track these metrics:

- **Coverage**: All features documented
- **Accuracy**: No false claims
- **Clarity**: User comprehension rate
- **Completeness**: No missing sections
- **Currency**: Updated with code

Improvement Process
-------------------

1. Collect user feedback
2. Track documentation bugs
3. Regular accuracy audits
4. Style guide updates
5. Template improvements

Tools and Resources
===================

Validation Tools
----------------

- ``cargo-kiln docs --private`` - Build and check documentation
- ``cargo-kiln docs --open`` - Build and preview locally
- ``cargo-kiln verify --detailed`` - Run comprehensive verification
- ``cargo test --doc`` - Test Rust examples

References
----------

- :doc:`style_guide` - Writing standards
- :doc:`templates/index` - Document templates
- :doc:`contributing/documentation` - Contribution guide
- `Sphinx Documentation`_ - RST reference

.. _Sphinx Documentation: https://www.sphinx-doc.org/