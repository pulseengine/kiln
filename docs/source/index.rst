=======================================
PulseEngine (Kiln Edition) Documentation
=======================================

.. raw:: html

   <div class="landing-header">
      <img src="_static/icons/logo.svg" alt="Kiln Logo" class="landing-logo">
      <p class="landing-subtitle">Safety-critical WebAssembly infrastructure for embedded systems</p>
   </div>

.. warning::
   This documentation is for PulseEngine (Kiln Edition) version |version|. For other versions, use the version switcher in the navigation bar.

.. note::
   **Development Status**: PulseEngine provides foundational WebAssembly infrastructure. The instruction execution engine and component model are under active development.

User Documentation
------------------

.. grid:: 3
   :gutter: 3

   .. grid-item-card:: Overview
      :link: overview/index
      :link-type: doc

      Product introduction, features, and status

   .. grid-item-card:: Getting Started
      :link: getting_started/index
      :link-type: doc

      Quick start guide and installation

   .. grid-item-card:: User Guide
      :link: user_guide/index
      :link-type: doc

      How to use PulseEngine in your applications

   .. grid-item-card:: Examples
      :link: examples/index
      :link-type: doc

      Code examples and practical tutorials

   .. grid-item-card:: Platform Guides
      :link: platform_guides/index
      :link-type: doc

      Platform-specific usage guides

   .. grid-item-card:: Architecture
      :link: architecture/index
      :link-type: doc

      System design and component model

API Reference
-------------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: Core Runtime API
      :link: kiln-runtime/lib
      :link-type: doc

      Core runtime components and interfaces

   .. grid-item-card:: Component Model API
      :link: kiln-component/lib
      :link-type: doc

      Component model implementation

   .. grid-item-card:: Host Integration API
      :link: kiln-host/lib
      :link-type: doc

      Host function bindings and callbacks

   .. grid-item-card:: All Crates
      :link: api/index
      :link-type: doc

      Complete API documentation for all PulseEngine crates

Qualification Material
----------------------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: Requirements
      :link: requirements/index
      :link-type: doc

      Functional, safety, and qualification requirements

   .. grid-item-card:: Qualification Plan
      :link: qualification/plan
      :link-type: doc

      ISO 26262 ASIL-D qualification preparation (not certified)

   .. grid-item-card:: Test Documentation
      :link: qualification/index
      :link-type: doc

      Test cases, coverage, and verification

   .. grid-item-card:: Safety Analysis
      :link: qualification/safety_analysis
      :link-type: doc

      FMEA, FTA, and safety assessments

Developer Documentation
------------------------

.. grid:: 3
   :gutter: 3

   .. grid-item-card:: Development Setup
      :link: developer/setup/index
      :link-type: doc

      Environment setup and toolchain installation

   .. grid-item-card:: Contributing
      :link: developer/contributing/index
      :link-type: doc

      Guidelines for contributing to PulseEngine

   .. grid-item-card:: Build System
      :link: developer/build_system/index
      :link-type: doc

      Cargo workspace and build configuration

   .. grid-item-card:: Testing
      :link: developer/testing/index
      :link-type: doc

      Test strategies and coverage requirements

   .. grid-item-card:: Internals
      :link: developer/internals/index
      :link-type: doc

      Technical deep-dives and implementation details

   .. grid-item-card:: Tooling
      :link: developer/tooling/index
      :link-type: doc

      cargo-kiln unified build tool, development workflows, and safety verification

Reference Documentation
-----------------------

.. grid:: 2
   :gutter: 3

   .. grid-item-card:: Safety Manual
      :link: safety_manual/index
      :link-type: doc

      Comprehensive safety documentation and compliance guidance

   .. grid-item-card:: Binary Format
      :link: binary
      :link-type: doc

      WebAssembly binary format specifications

   .. grid-item-card:: Changelog
      :link: changelog.md
      :link-type: doc

      Release notes and version history

.. toctree::
   :hidden:
   :caption: User Documentation

   overview/index
   overview/implementation_status
   getting_started/index
   user_guide/index
   examples/index
   platform_guides/index
   architecture/index

.. toctree::
   :hidden:
   :caption: API Reference

   kiln-runtime/lib
   kiln-component/lib
   kiln-host/lib
   kiln-instructions/lib
   kiln-logging/lib
   api/index

.. toctree::
   :hidden:
   :caption: Safety & Qualification

   safety_manual/index
   requirements/index
   qualification/index

.. toctree::
   :hidden:
   :caption: Developer Documentation

   developer/setup/index
   developer/contributing/index
   developer/build_system/index
   developer/testing/index
   developer/internals/index
   developer/tooling/index

.. toctree::
   :hidden:
   :caption: Reference

   safety_manual/index
   binary
   changelog.md
   
.. comment:: _generated_symbols.rst - Generated during build process

.. include:: _generated_coverage_summary.rst

.. include:: _generated_safety_summary.rst

Indices and tables
==================

* :ref:`genindex`
* :ref:`search`