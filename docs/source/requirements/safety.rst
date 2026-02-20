===================
Safety Requirements
===================

.. image:: ../_static/icons/safety_features.svg
   :width: 64px
   :align: right
   :alt: Safety Features Icon

This document defines the safety, resource management, and verification requirements for the Kiln runtime. It consolidates all safety-related requirements in one place for easier tracking and management.

.. contents:: On this page
   :local:
   :depth: 2

Status Overview
---------------

.. commenting out needpie directives until they can be fixed
..
.. .. needpie::
..    :labels: Active, Implemented, Not Started
..    :filter: id =~ "REQ_SAFETY_.*" and status != "removed"

Safety Core Requirements
------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_SAFETY_.*"
..    :style: table
..    :columns: id, title, status

Memory Safety Requirements
--------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_MEM_SAFETY_.*"
..    :style: table
..    :columns: id, title, status

Resource Management
-------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_RESOURCE_.*"
..    :style: table
..    :columns: id, title, status

Verification Requirements
-------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_VERIFY_.*"
..    :style: table
..    :columns: id, title, status

WebAssembly Safety Requirements
-------------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_WASM_.*"
..    :style: table
..    :columns: id, title, status

Code Quality Requirements
-------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_CODE_QUALITY_.*"
..    :style: table
..    :columns: id, title, status

Build and Environment Requirements
----------------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "REQ_BUILD_.*|REQ_ENV_.*|REQ_INSTALL_.*"
..    :style: table
..    :columns: id, title, status

Related Documentation
---------------------

For more information on how these safety requirements are implemented and verified, see:

* :doc:`../safety/index` - Safety documentation overview
* :doc:`../safety/mechanisms` - Safety mechanism implementations
* :doc:`../safety/test_cases` - Safety test cases
* :doc:`../safety/constraints` - Safety constraints 