==========================
Qualification Requirements
==========================

.. image:: ../_static/icons/qualification.svg
   :width: 64px
   :align: right
   :alt: Qualification Requirements Icon

This document defines the qualification requirements for PulseEngine (Kiln Edition). These requirements specify how the system must be qualified for use in safety-critical applications.

.. contents:: On this page
   :local:
   :depth: 2

Status Overview
---------------

.. commenting out needpie directives until they can be fixed
..
.. .. needpie::
..    :labels: Active, Implemented, Not Started
..    :filter: id =~ "QUAL_.*" and status != "removed"

Documentation Requirements
--------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "QUAL_DOCS_.*"
..    :style: table
..    :columns: id, title, status

Testing Requirements
--------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "QUAL_TEST_.*"
..    :style: table
..    :columns: id, title, status

Safety Verification Requirements
--------------------------------

.. commenting out needfilter directives until they can be fixed
..
.. .. needfilter::
..    :filter: id =~ "QUAL_SAFETY_.*"
..    :style: table
..    :columns: id, title, status

Panic Documentation
-------------------

The following table shows all documented panic points in the system:

.. needtable::
   :columns: id;title;item_status;safety_impact
   :filter: id.startswith("KILNQ-")
   :style: table

Qualification Documentation
---------------------------

For more information on qualification materials, see:

* :doc:`../qualification/index` - Qualification overview
* :doc:`../qualification/plan` - Qualification plan
* :doc:`../qualification/safety_analysis` - Safety analysis report
* :doc:`../qualification/panic_registry` - Panic registry 