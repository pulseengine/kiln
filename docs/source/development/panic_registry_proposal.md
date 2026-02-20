# Panic Registry Implementation Proposal

## Overview

This document proposes an enhanced approach to tracking panics in the Kiln codebase by:

1. Maintaining the existing CSV-based panic registry for easy reading and editing
2. Adding a new RST-based representation using sphinx-needs for qualification documentation
3. Implementing validation to ensure all panic documentation includes safety impact and tracking information

## Implementation Details

### 1. Enhanced `update_panic_registry.rs`

The planned `cargo-kiln update-panic-registry` command will be extended to:

- Continue to generate the CSV file as before
- Additionally generate an RST file with sphinx-needs directives
- Parse safety impact levels and categorize them (LOW/MEDIUM/HIGH)
- Automatically create KILNQ-XXX tracking IDs for entries without them

### 2. sphinx-needs Configuration

The `docs/source/conf.py` file has been updated with:

- A new `panic` directive type with `KILNQ-` prefix
- Additional option specs for panic-specific fields (safety_impact, status, handling_strategy)
- Tags for filtering panic entries by safety impact level (low, medium, high)
- A custom template for displaying panic entries

### 3. Documentation Updates

The panic documentation guidelines have been updated to:

- Explain both CSV and RST formats
- Provide guidance on how to use the sphinx-needs representation
- Maintain the existing CSV table for backward compatibility

### 4. Validation Enhancements

The `check_panics` command has been enhanced to:

- Check for undocumented panics as before
- Additionally validate that panic documentation includes required fields:
  - Safety impact: `LOW/MEDIUM/HIGH` - Brief explanation
  - Tracking: `KILNQ-XXX` (qualification tracking ID)
- Generate appropriate documentation templates when run with `--fix`

## Benefits

This approach provides several advantages:

1. **Dual Representation**: Maintains the simple CSV format while adding structured qualification documentation
2. **Integration with Qualification**: Enables cross-referencing panic points in qualification documentation
3. **Automated Validation**: Ensures consistent documentation of safety impact and tracking IDs
4. **Improved Reporting**: Enables filtering and searching panic entries by various attributes
5. **Traceability**: Links panic points to qualification requirements through the KilnQ-XXX IDs

## Usage

For developers:

1. Document panics in Rust code following the established format
2. Run `cargo-kiln check-panics --fix` to add templates for undocumented panics (planned)
3. Run `cargo-kiln update-panic-registry` to update both CSV and RST files (planned)

For documentation readers:

1. CSV format remains available for quick reference
2. RST format provides enhanced filtering and cross-referencing capabilities
3. Sphinx-needs features enable integration with qualification matrices

## Example of Generated RST Content

```rst
.. _panic-registry:

Panic Registry
==============

This document contains all documented panic conditions in the Kiln codebase.
Each panic is tracked as a qualification requirement using sphinx-needs.

.. contents:: Table of Contents
   :local:
   :depth: 2

Summary
-------

* Total panic points: 14
* Status:
  * Todo: 14
  * In Progress: 0
  * Resolved: 0

The original CSV version of this registry is maintained at:
`docs/source/development/panic_registry.csv`_

.. csv-table:: Panic Registry CSV
   :file: panic_registry.csv
   :header-rows: 1
   :widths: 20, 15, 5, 20, 5, 10, 10, 15

Panic Details
------------

.. qual:: f32_nearest
   :id: KILNQ-0001
   :status: Todo
   :implementation: 
   :tags: panic, medium

   **File:** kiln/src/execution.rs
   **Line:** 389
   **Function:** f32_nearest
   **Safety Impact:** MEDIUM - Incorrect numerical results
   **Last Updated:** 2025-04-25

   This function will panic if the provided value is not an F32 value.
```

## Next Steps

1. Run the updated `xtask update-panic-registry` command to generate the initial RST file
2. Update qualification documentation to reference panic points using sphinx-needs
3. Begin filling in missing safety impact assessments and tracking IDs

This implementation enables the Kiln project to maintain better traceability between panic points and qualification requirements, while keeping the existing CSV format for ease of maintenance. 