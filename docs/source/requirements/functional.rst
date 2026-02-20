=======================
Functional Requirements
=======================

.. image:: ../_static/icons/functional.svg
   :width: 64px
   :align: right
   :alt: Functional Requirements Icon

This document defines the functional requirements for PulseEngine (Kiln Edition). These requirements specify what the system should do to accomplish its mission.

.. contents:: On this page
   :local:
   :depth: 2

Functional Requirements Status
------------------------------

.. 
   Pie chart temporarily removed due to syntax issues
   
   .. needpie::
      :labels: Active, Implemented, Not Started
      :status: id =~ "REQ_[^S].*" and status != "removed" and id !~ "REQ_SAFETY_.*|REQ_MEM_SAFETY_.*|REQ_VERIFY_.*|REQ_RESOURCE_.*"

WebAssembly Core Requirements
-----------------------------

.. commenting out needfilters until they can be fixed
.. 
.. .. needfilter::
..    :filter: id =~ "REQ_CORE_.*"
..    :style: table
..    :columns: id, title, status

Platform Requirements
---------------------

Component Model Requirements
----------------------------

.. commenting out needfilters until they can be fixed
.. 
.. .. needfilter::
..    :filter: id =~ "REQ_COMP_.*"
..    :style: table
..    :columns: id, title, status

Performance Requirements
------------------------

.. commenting out needfilters until they can be fixed
.. 
.. .. needfilter::
..    :filter: id =~ "REQ_PERF_.*"
..    :style: table
..    :columns: id, title, status

Error Handling Requirements
---------------------------

.. commenting out needfilters until they can be fixed
.. 
.. .. needfilter::
..    :filter: id =~ "REQ_ERROR_.*"
..    :style: table
..    :columns: id, title, status

API Requirements
----------------

.. commenting out needfilters until they can be fixed
.. 
.. .. needfilter::
..    :filter: id =~ "REQ_API_.*"
..    :style: table
..    :columns: id, title, status

Quality Assurance Requirements
------------------------------

.. commenting out needfilters until they can be fixed
.. 
.. .. needfilter::
..    :filter: id =~ "REQ_QA_.*"
..    :style: table
..    :columns: id, title, status

Implementation Details
----------------------

For information on how these functional requirements are implemented, see:

* :doc:`../architecture` - System architecture
* :doc:`../api/index` - API documentation 