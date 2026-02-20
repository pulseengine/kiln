=================
Logging Subsystem
=================

The logging subsystem implements the WASI logging API and provides platform-specific backends.

.. spec:: Logging Architecture
   :id: SPEC_004
   :links: REQ_015, REQ_016
   
   The logging architecture consists of:
   
   1. WASI logging component interface
   2. Host logging handler registration
   3. Platform-specific backends
   4. Log level filtering and routing
   
   **Logging Flow Sequence**
   
   The following sequence diagram illustrates how logging flows from a WebAssembly module through the runtime:
   
   .. uml:: ../../_static/logging_flow.puml
      :alt: Kiln Logging Flow
      :width: 100%
   
   When a WebAssembly module calls a logging function, the following steps occur:
   
   1. WebAssembly module calls the WASI logging interface function (`log`, `logTrace`, etc.)
   2. The WASI logging interface implementation in Kiln converts the call to an engine operation
   3. The StacklessEngine creates a LogOperation object with level, message, and optional component ID
   4. The operation is passed to the CallbackRegistry via handle_log()
   5. The CallbackRegistry invokes the registered log handler (if any)
   6. The host logging system (terminal, file, syslog, etc.) processes the log message

.. impl:: Logging Implementation
   :id: IMPL_007
   :status: implemented
   :links: SPEC_004, REQ_015
   
   The logging implementation provides:
   
   1. Standard log levels (Trace, Debug, Info, Warn, Error, Critical)
   2. Registration of custom log handlers
   3. Default stderr fallback
   4. Component-specific context tracking
   
   Key components:
   
   - ``LogLevel`` - Enum with standard log levels (Trace, Debug, Info, Warn, Error, Critical)
   - ``LogOperation`` - Struct containing level, message, and optional component ID
   - ``CallbackRegistry`` - Central registry for handling WebAssembly component operations
   
   Key methods include:
   - ``register_log_handler(handler)`` - Registers a custom log handler
   - ``handle_log(operation)`` - Internal method to process log messages
   - ``LogOperation::with_component(level, message, component_id)`` - Creates a log operation with component context 