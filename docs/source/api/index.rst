API Documentation
=================

This section contains the API documentation for all PulseEngine libraries and components.

.. warning::
   **Development Status**: Many APIs shown here represent the intended design. 
   Implementation status varies by component - see individual crate documentation for details.

.. note::
   The following references are automatically generated during the complete documentation build process.
   Missing references are normal if you're viewing a partial build without Rust documentation generation.

.. toctree::
   :maxdepth: 2
   :caption: Core Libraries:

   kiln-error <../_generated_rust_docs/kiln-error/lib>
   kiln-foundation <../_generated_rust_docs/kiln-foundation/lib>
   kiln-sync <../_generated_rust_docs/kiln-sync/lib>
   kiln-math <../_generated_rust_docs/kiln-math/lib>

.. toctree::
   :maxdepth: 2
   :caption: Format and Parsing:

   kiln-format <../_generated_rust_docs/kiln-format/lib>
   kiln-decoder <../_generated_rust_docs/kiln-decoder/lib>

.. toctree::
   :maxdepth: 2
   :caption: Runtime and Execution:

   kiln-instructions <../_generated_rust_docs/kiln-instructions/lib>

.. toctree::
   :maxdepth: 2
   :caption: Platform Support:

   kiln-platform <../_generated_rust_docs/kiln-platform/lib>

.. toctree::
   :maxdepth: 2
   :caption: Host Integration:

   kiln-host <../_generated_rust_docs/kiln-host/lib>
   kiln-intercept <../_generated_rust_docs/kiln-intercept/lib>
   kiln-logging <../_generated_rust_docs/kiln-logging/lib>

.. note::
   Additional crate documentation will be enabled progressively as we resolve 
   build dependencies and improve the rust documentation generation pipeline.
   
   Planned additions:
   - kiln-foundation (core types and collections)
   - kiln-runtime (execution engine)
   - kiln-component (Component Model implementation)
   - kiln-platform (platform abstraction)
   - kiln-decoder (binary format parsing)
   - And more... 