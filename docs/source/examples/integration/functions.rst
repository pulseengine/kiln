==============
Host Functions
==============

This example demonstrates how to create and register host functions for WebAssembly modules.

.. note::
   **Implementation Status**: Host function interface is under development.
   This shows the planned API design.

Overview
--------

Host functions allow WebAssembly modules to call native code, providing access to system resources and APIs.

Basic Example
-------------

.. code-block:: rust
   :caption: Basic host function registration

   use kiln::prelude::*;

   // Define a host function
   fn print_message(message: &str) -> Result<()> {
       println!("WASM says: {}", message);
       Ok(())
   }

   // Register with the runtime
   let mut host = HostRegistry::new();
   host.register("console", "log", print_message)?;

   // Use in module instantiation
   let instance = ModuleInstance::new(module, host.imports())?;

Next Steps
----------

See :doc:`../basic_component` for complete component integration examples.