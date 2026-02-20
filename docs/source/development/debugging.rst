Runtime Debug Features Guide
============================

This guide explains how to use the new runtime debugging features in kiln-debug.

Overview
--------

The runtime debug features extend kiln-debug beyond static analysis to provide full interactive debugging capabilities when integrated with a WebAssembly runtime.

Feature Structure
-----------------

.. code-block:: toml

    [features]
    # Static features (no runtime needed)
    static-debug = ["line-info", "debug-info", "function-info"]

    # Runtime features (requires integration)
    runtime-inspection = ["static-debug"]         # Read runtime state
    runtime-variables = ["runtime-inspection"]    # Variable values
    runtime-memory = ["runtime-inspection"]       # Memory inspection
    runtime-control = ["runtime-inspection"]      # Execution control
    runtime-breakpoints = ["runtime-control"]     # Breakpoints
    runtime-stepping = ["runtime-control"]        # Step debugging
    runtime-debug = ["runtime-variables", "runtime-memory", "runtime-breakpoints", "runtime-stepping"]

Integration with Kiln Runtime
----------------------------

1. Implement Runtime Interfaces
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Your runtime must implement these traits:

.. code-block:: rust

    use kiln_debug::{RuntimeState, DebugMemory};

    impl RuntimeState for YourRuntime {
        fn pc(&self) -> u32 { /* current program counter */ }
        fn sp(&self) -> u32 { /* stack pointer */ }
        fn fp(&self) -> Option<u32> { /* frame pointer if available */ }
        fn read_local(&self, index: u32) -> Option<u64> { /* local variable */ }
        fn read_stack(&self, offset: u32) -> Option<u64> { /* stack value */ }
        fn current_function(&self) -> Option<u32> { /* function index */ }
    }

    impl DebugMemory for YourRuntime {
        fn read_bytes(&self, addr: u32, len: usize) -> Option<&[u8]> {
            // Safe memory access
        }
        fn is_valid_address(&self, addr: u32) -> bool {
            // Address validation
        }
    }

2. Attach Debugger
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

    use kiln_debug::{DebuggableRuntime, DefaultDebugger};

    impl DebuggableRuntime for YourRuntime {
        fn attach_debugger(&mut self, debugger: Box<dyn RuntimeDebugger>) {
            self.debugger = Some(debugger);
        }
        // ... other methods
    }

    // Usage
    let debugger = Box::new(DefaultDebugger::new());
    runtime.attach_debugger(debugger);

3. Hook Execution
~~~~~~~~~~~~~~~~~

.. code-block:: rust

    // In your interpreter loop
    fn execute_instruction(&mut self, instr: Instruction) -> Result<()> {
        #[cfg(feature = "runtime-debug")]
        if let Some(debugger) = &mut self.debugger {
            match debugger.on_instruction(self.pc, self) {
                DebugAction::Continue => {},
                DebugAction::Break => return Ok(()),
                DebugAction::StepLine => self.single_step = true,
                // Handle other actions...
            }
        }
        
        // Normal execution
        match instr {
            // ...
        }
    }

Usage Examples
--------------

Variable Inspection
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

    use kiln_debug::{VariableInspector, VariableDefinition, DwarfLocation};

    // Create inspector
    let mut inspector = VariableInspector::new();

    // Add variable from DWARF (normally parsed from .debug_info)
    inspector.add_variable(VariableDefinition {
        name: Some(debug_str.get_string(name_offset)),
        var_type: BasicType::SignedInt(4),
        location: DwarfLocation::Register(0), // Local 0
        scope: VariableScope {
            start_pc: 0x1000,
            end_pc: 0x2000,
            depth: 0,
        },
        file_index: 1,
        line: 42,
    })?;

    // At runtime: get live variables
    let live_vars = inspector.get_live_variables(pc, &runtime_state, &memory);

    for var in live_vars.iter() {
        if let Some(value) = &var.value {
            println!("{}: {}", 
                var.name.as_ref().map(|n| n.as_str()).unwrap_or("<unnamed>"),
                format_value(value));
        }
    }

Memory Inspection
~~~~~~~~~~~~~~~~~

.. code-block:: rust

    use kiln_debug::{MemoryInspector, MemoryRegion, MemoryRegionType};

    let mut inspector = MemoryInspector::new();
    inspector.attach(&runtime_memory);

    // Register memory regions
    inspector.add_region(MemoryRegion {
        start: 0x0,
        size: 0x10000,
        region_type: MemoryRegionType::LinearMemory,
        writable: true,
        name: "main",
    })?;

    // Read string from memory
    if let Some(cstring) = inspector.read_cstring(0x1000, 256) {
        println!("String at 0x1000: {}", cstring.as_str().unwrap_or("<invalid>"));
    }

    // Hex dump
    inspector.dump_hex(0x2000, 64).display(|s| {
        print!("{}", s);
        Ok(())
    })?;

    // Analyze heap
    let stats = inspector.heap_stats();
    println!("Heap: {} allocations, {} bytes used", 
             stats.active_allocations, 
             stats.allocated_bytes);

Breakpoint Management
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

    use kiln_debug::{BreakpointManager, BreakpointCondition};

    let mut bp_manager = BreakpointManager::new();

    // Set breakpoint at address
    let bp1 = bp_manager.add_breakpoint(0x1234)?;

    // Set breakpoint at source location
    let bp2 = bp_manager.add_line_breakpoint(
        file_index,  // From file table
        line_number, // Line 42
        address      // Resolved address
    )?;

    // Conditional breakpoint
    bp_manager.set_condition(bp1, BreakpointCondition::HitCount(3))?;

    // Check during execution
    if let Some(bp) = bp_manager.should_break(pc, &runtime_state) {
        println!("Hit breakpoint {} at 0x{:x}", bp.id.0, bp.address);
        // Handle breakpoint...
    }

Stepping Control
~~~~~~~~~~~~~~~~

.. code-block:: rust

    use kiln_debug::{SteppingDebugger, StepMode};

    let mut stepper = SteppingDebugger::new();

    // Populate line mappings from DWARF
    stepper.add_line_mapping(0x1000, 0x1010, LineInfo {
        file_index: 1,
        line: 10,
        column: 0,
        is_stmt: true,
        end_sequence: false,
    })?;

    // Start stepping
    stepper.step(StepMode::Line, current_pc);

    // Check during execution
    match stepper.should_break(pc, &runtime_state) {
        DebugAction::Continue => {},
        DebugAction::Break => {
            println!("Stepped to new line");
            // Show source context...
        }
        // Handle other actions...
    }

    // Track function calls for step-over
    stepper.on_function_entry(func_idx, return_pc);
    stepper.on_function_exit();

Complete Example: Interactive Debugger
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

    struct InteractiveDebugger {
        debug_info: DwarfDebugInfo<'static>,
        var_inspector: VariableInspector<'static>,
        mem_inspector: MemoryInspector<'static>,
        bp_manager: BreakpointManager,
        stepper: SteppingDebugger,
        file_table: FileTable<'static>,
    }

    impl InteractiveDebugger {
        fn on_break(&mut self, pc: u32, state: &dyn RuntimeState, memory: &dyn DebugMemory) {
            // Show location
            if let Some(line_info) = self.debug_info.find_line_info(pc).ok().flatten() {
                let mut output = String::new();
                line_info.format_location(&self.file_table).display(|s| {
                    output.push_str(s);
                    Ok(())
                }).ok();
                println!("Stopped at {}", output);
            }
            
            // Show function
            if let Some(func) = self.debug_info.find_function_info(pc) {
                print!("In function {}", 
                    func.name.as_ref().map(|n| n.as_str()).unwrap_or("<unknown>"));
                if let Some(params) = &func.parameters {
                    params.display(|s| { print!("{}", s); Ok(()) }).ok();
                }
                println!();
            }
            
            // Show local variables
            let vars = self.var_inspector.get_live_variables(pc, state, memory);
            if !vars.is_empty() {
                println!("\nLocal variables:");
                for var in vars.iter() {
                    if let Some(value) = &var.value {
                        let mut val_str = String::new();
                        ValueDisplay { value }.display(|s| {
                            val_str.push_str(s);
                            Ok(())
                        }).ok();
                        println!("  {}: {} = {}", 
                            var.name.as_ref().map(|n| n.as_str()).unwrap_or("?"),
                            var.var_type.type_name(),
                            val_str);
                    }
                }
            }
            
            // Interactive commands
            loop {
                print!("> ");
                let cmd = read_command();
                
                match cmd.as_str() {
                    "c" | "continue" => break,
                    "n" | "next" => {
                        self.stepper.step(StepMode::Over, pc);
                        break;
                    }
                    "s" | "step" => {
                        self.stepper.step(StepMode::Into, pc);
                        break;
                    }
                    "bt" | "backtrace" => self.show_backtrace(state),
                    "mem" => self.show_memory(memory),
                    "q" | "quit" => std::process::exit(0),
                    _ => println!("Unknown command"),
                }
            }
        }
    }

Performance Considerations
--------------------------

Interpreter Mode
~~~~~~~~~~~~~~~~

- Variable inspection: ~5% overhead
- Memory inspection: ~3% overhead  
- Breakpoints: ~10% overhead
- Stepping: ~15% overhead
- **Total with all features**: ~20-30% overhead

Future AOT Mode
~~~~~~~~~~~~~~~

- Debug build: ~20-30% overhead
- Release build: 0% overhead
- Hybrid mode: 0% normally, falls back to interpreter for debugging

Memory Usage
------------

============== =====
Component      Size
============== =====
Variable Inspector    ~4KB per 100 variables
Memory Inspector      ~2KB + region metadata
Breakpoint Manager    ~1KB per 100 breakpoints
Step Controller       ~512 bytes
**Total typical usage**    ~8-16KB
============== =====

Best Practices
---------------

1. **Feature Selection**: Only enable features you need

   .. code-block:: toml

       # Production: static only
       features = ["static-debug"]
       
       # Development: full debugging
       features = ["runtime-debug"]

2. **Lazy Initialization**: Don't parse debug info until needed

   .. code-block:: rust

       if debugging_enabled {
           debug_info.init_info_parser()?;
       }

3. **Conditional Compilation**: Use feature gates

   .. code-block:: rust

       #[cfg(feature = "runtime-debug")]
       self.check_breakpoint(pc)?;

4. **Memory Boundaries**: Always validate addresses

   .. code-block:: rust

       if !memory.is_valid_address(addr) {
           return Err(DebugError::InvalidAddress);
       }

Limitations
-----------

1. **no_std/no_alloc**: All data structures are bounded
2. **Complex Types**: Only basic types supported
3. **DWARF Expressions**: Limited expression evaluation
4. **Optimization**: Optimized code may hide variables

Future Enhancements
-------------------

1. **Expression Evaluation**: ``print x + y``
2. **Watchpoints**: Break on memory changes
3. **Remote Debugging**: Debug over network
4. **Time-Travel**: Record and replay execution
5. **DAP Integration**: VS Code debugging