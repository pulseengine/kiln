====================================
Developer Tooling & Local Checks
====================================

This page provides an overview of the development tools, coding standards, and local checks configured for this project. Developers should familiarize themselves with these tools to ensure code quality, consistency, and adherence to safety guidelines.

.. contents:: On this page
   :local:
   :depth: 2

Configuration Files
-------------------

The following configuration files define standards and tool behavior across the workspace:

*   ``.editorconfig``: Ensures consistent editor settings (indentation, line endings) across different IDEs.
*   ``.gitattributes``: Enforces LF line endings and UTF-8 encoding for various file types.
*   `rust-toolchain.toml`: Pins the project to a specific Rust stable toolchain version (e.g., 1.78.0) for reproducible builds.
*   `rustfmt.toml`: Defines the code formatting rules enforced by `rustfmt`.
*   `deny.toml`: Configures `cargo-deny` for checking licenses, duplicate dependencies, security advisories, and allowed sources.
*   `cspell.json`: Contains the configuration and custom dictionary for `cspell` spell checking.
*   `Cargo.toml` (workspace and per-crate):
    *   `[profile.release]` and `[profile.test]` set `panic = "abort"`.
    *   `[lints.rust]` and `[lints.clippy]` define a strict set of allowed/denied lints. Key settings include:

        *   `rust.unsafe_code = "forbid"` (enforced by `#![forbid(unsafe_code)]` in lib/main files).
        *   `rust.missing_docs = "deny"`.
        *   `clippy::pedantic = "warn"` (most pedantic lints enabled).
        *   Many specific clippy lints are set to `deny` or `warn` (e.g., `unwrap_used`, `float_arithmetic`, `transmute_ptr_to_ref`).

Local Development Workflow & Checks
-----------------------------------

The `cargo-kiln` unified build tool provides convenient commands for common development tasks and running checks. Install it with:

.. code-block:: bash

   cargo install --path cargo-kiln

.. _dev-formatting:

Code Formatting
~~~~~~~~~~~~~~~

*   **Tool**: `rustfmt`
*   **Configuration**: `rustfmt.toml`
*   **Usage**:
    *   To format all code: ``cargo-kiln check``
    *   To check if code is formatted: ``cargo-kiln check --strict`` (run by CI)

.. _dev-linting:

Linting with Clippy
~~~~~~~~~~~~~~~~~~~

*   **Tool**: `clippy`
*   **Configuration**: `[lints.clippy]` in `Cargo.toml` files.
*   **Usage**:
    *   Run clippy checks: ``cargo-kiln check --strict`` (all warnings treated as errors)
    *   Clippy is also run as part of ``cargo-kiln ci``.

.. _dev-file-checks:

Project File & Header Checks
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

*   **Tool**: Integrated into `cargo-kiln` build system.
*   **Usage**:
    *   Check for presence of essential project files (README, LICENSE, etc.): ``cargo-kiln verify --detailed``
    *   Check file headers (copyright, license, SPDX) and `#![forbid(unsafe_code)]`: ``cargo-kiln verify --detailed``
    *   These are also run as part of ``cargo-kiln ci``.

.. _dev-dependency-checks:

Dependency Management & Audit
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

*   **Dependency Policy (`cargo-deny`)**:
    *   **Tool**: `cargo-deny`
    *   **Configuration**: `deny.toml`
    *   **Usage**: ``cargo-kiln verify --asil c`` (also part of ``cargo-kiln ci``)
*   **Unused Dependencies (`cargo-udeps`)**:
    *   **Tool**: `cargo-udeps` (requires installation: `cargo install cargo-udeps --locked`)
    *   **Usage**: ``cargo-kiln check --strict`` (includes dependency analysis)
*   **Security Advisories (`cargo-audit`)**:
    *   **Tool**: `cargo-audit` (requires installation: `cargo install cargo-audit --locked`)
    *   **Usage**: ``cargo-kiln verify --asil c`` (includes security audit)

.. _dev-geiger:

Unsafe Code Detection
~~~~~~~~~~~~~~~~~~~~~

*   **Tool**: `cargo-geiger`
*   **Usage**: ``cargo-kiln verify --detailed`` (also part of ``cargo-kiln ci``)
    This tool scans for `unsafe` Rust code usage and provides statistics.

.. _dev-spell-check:

Spell Checking
~~~~~~~~~~~~~~

*   **Tool**: `cspell` (requires installation: `npm install -g cspell`)
*   **Configuration**: `cspell.json`
*   **Usage**: ``cargo-kiln verify --detailed`` (includes spell checking)

.. _dev-testing:

Running Tests
~~~~~~~~~~~~~

*   **Unit & Integration Tests**: ``cargo-kiln test`` (runs comprehensive test suite)
*   **Main CI Check Suite**: ``cargo-kiln ci``
    *   Includes: build, formatting, file presence checks, headers, clippy, dependency analysis, unsafe code detection, tests, documentation.
*   **Full Verification Suite**: ``cargo-kiln verify --asil d``
    *   Includes everything in `ci` plus:

        *   Miri checks: Runs tests under Miri to detect undefined behavior.
        *   KANI formal verification: ``cargo-kiln kani-verify --asil-profile d``
        *   Coverage analysis: ``cargo-kiln coverage --html``
        *   Matrix verification: ``cargo-kiln verify-matrix --report``

.. _dev-safety-verification:

Safety Verification (SCORE Framework)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

*   **Tool**: Custom `xtask` commands implementing SCORE-inspired safety verification
*   **Configuration**: `requirements.toml` 
*   **Usage**:
    *   Quick safety dashboard: ``cargo-kiln verify --detailed``
    *   Check requirements traceability: ``cargo-kiln verify --asil c``
    *   Full safety verification: ``cargo-kiln verify --asil d``
    *   Generate safety reports: ``cargo-kiln verify-matrix --report``
*   **Features**: ASIL compliance monitoring, requirements traceability, test coverage analysis
*   **Documentation**: :doc:`safety_verification` - Complete guide to safety verification tools

CI Pipeline Overview
--------------------

The CI pipeline (defined in `.github/workflows/ci.yml`) automates most of these checks. Key jobs include:

*   **Check**: Basic build checks with ``cargo-kiln build``.
*   **Test Suite**: Runs ``cargo-kiln test``.
*   **Compliance Checks**: Runs ``cargo-kiln ci`` which covers formatting, headers, clippy, dependency analysis, unsafe code detection, file presence, tests, doc builds, and verification.
*   **Safety Verification**: Runs ``cargo-kiln verify --asil d``.
*   **Matrix Verification**: Runs ``cargo-kiln verify-matrix --report``.
*   **CI Simulation**: Runs ``cargo-kiln simulate-ci`` for local testing.

This ensures that code merged into the main branch adheres to the defined quality and safety standards.