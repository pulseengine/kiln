================================
Building Your First Component
================================

.. epigraph::

   "Components are the future of WebAssembly."
   
   -- The WebAssembly Component Model Working Group

Remember the old days when sharing code meant "here's my library, good luck with the ABI"? The WebAssembly Component Model changes everything. Components are self-contained, composable, and language-agnostic. Let's build one!

.. warning::
   **Component Model Under Development**: This example demonstrates the intended Component Model API design. 
   Component parsing and instantiation are currently under active development in PulseEngine.
   
   **Current Status**: WIT interface definitions and component types are implemented, 
   but component execution is not yet functional.

.. admonition:: What You'll Learn
   :class: note

   - What makes a component different from a module
   - How to define component interfaces with WIT
   - Building components with ``cargo-component``
   - Composing multiple components together
   - Real-world component patterns

Components vs Modules: The Big Picture 🖼️
------------------------------------------

Think of it this way:

- **Module**: A single WebAssembly file with functions (like a .o file)
- **Component**: A complete package with interfaces, types, and dependencies (like a .dll or .so)

.. code-block:: text
   :caption: The difference visualized

   Module:                          Component:
   ┌─────────────┐                 ┌──────────────────────┐
   │  Functions  │                 │  Defined Interface   │
   │  - add()    │                 │  ┌────────────────┐  │
   │  - mult()   │      vs         │  │ imports: []    │  │
   │  - ...      │                 │  │ exports:       │  │
   └─────────────┘                 │  │  - calculate() │  │
                                   │  └────────────────┘  │
                                   │  + Types & Resources │
                                   └──────────────────────┘

Your First WIT Interface 📝
---------------------------

WIT (WebAssembly Interface Types) is how we define component interfaces. It's like a header file, but better:

.. code-block:: wit
   :caption: calculator.wit
   :linenos:

   package example:calculator@0.1.0;
   
   /// A simple calculator component
   interface types {
       /// Calculation operations
       enum operation {
           add,
           subtract,
           multiply,
           divide,
       }
       
       /// A calculation request
       record calculation {
           left: f64,
           right: f64,
           op: operation,
       }
       
       /// Possible calculation errors
       variant calc-error {
           divide-by-zero,
           overflow,
           invalid-operation(string),
       }
   }
   
   /// The main calculator interface
   interface calculator {
       use types.{calculation, calc-error};
       
       /// Perform a calculation
       calculate: func(calc: calculation) -> result<f64, calc-error>;
       
       /// Get calculation history
       get-history: func() -> list<calculation>;
       
       /// Clear history
       clear-history: func();
   }
   
   /// The calculator world (what we export)
   world calculator-world {
       import print: func(msg: string);
       export calculator;
   }

Building the Component 🔨
-------------------------

Step 1: Set Up Your Project
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Install cargo-component
   cargo install cargo-component
   
   # Create a new component project
   cargo component new calculator --lib
   cd calculator

Step 2: Add Your WIT File
~~~~~~~~~~~~~~~~~~~~~~~~~

Place the WIT file in your project:

.. code-block:: bash

   mkdir wit
   # Copy the calculator.wit file to wit/calculator.wit

Step 3: Implement the Component
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust
   :caption: src/lib.rs
   :linenos:

   use exports::example::calculator::types::{Calculation, CalcError, Operation};
   
   wit_bindgen::generate!({
       world: "calculator-world",
       exports: {
           "example:calculator/calculator": Calculator,
       },
   });
   
   struct Calculator;
   
   // Thread-local storage for history (components are single-threaded)
   std::thread_local! {
       static HISTORY: std::cell::RefCell<Vec<Calculation>> = 
           std::cell::RefCell::new(Vec::new());
   }
   
   impl exports::example::calculator::calculator::Guest for Calculator {
       fn calculate(calc: Calculation) -> Result<f64, CalcError> {
           // Add to history
           HISTORY.with(|h| h.borrow_mut().push(calc.clone()));
           
           // Perform calculation
           match calc.op {
               Operation::Add => Ok(calc.left + calc.right),
               Operation::Subtract => Ok(calc.left - calc.right),
               Operation::Multiply => Ok(calc.left * calc.right),
               Operation::Divide => {
                   if calc.right == 0.0 {
                       Err(CalcError::DivideByZero)
                   } else {
                       Ok(calc.left / calc.right)
                   }
               }
           }
       }
       
       fn get_history() -> Vec<Calculation> {
           HISTORY.with(|h| h.borrow().clone())
       }
       
       fn clear_history() {
           HISTORY.with(|h| h.borrow_mut().clear());
           
           // Use the imported print function
           print("History cleared!");
       }
   }
   
   // Required by wit-bindgen
   export!(Calculator);

Step 4: Build It!
~~~~~~~~~~~~~~~~~

.. code-block:: bash

   # Build the component
   cargo component build --release
   
   # The component is at:
   # target/wasm32-wasi/release/calculator.wasm

Using Your Component 🎮
-----------------------

Now let's use our calculator component from a host application:

.. code-block:: rust
   :caption: examples/use_calculator.rs (Target API - Under Development)

   // This code shows the intended Component Model API design
   // Current implementation status: WIT interface definitions exist, component execution in development
   
   use kiln::component::*;  // Not yet implemented
   use kiln::{Config, Engine, Store};  // Infrastructure exists, execution engine in progress
   
   bindgen!({  // Target API - component bindings under development
       world: "calculator-world",
       async: false,
   });
   
   fn main() -> Result<()> {
       // TARGET API: Configure the engine for component model
       let mut config = Config::new();  // Configuration infrastructure exists
       config.wasm_component_model(true);  // Not yet implemented
       let engine = Engine::new(&config)?;  // Basic engine infrastructure exists
       
       // TARGET API: Load the component
       let component = Component::from_file(  // Not yet implemented
           &engine,
           "target/wasm32-wasi/release/calculator.wasm"
       )?;
       
       // TARGET API: Create a store with our state
       struct State {
           prints: Vec<String>,
       }
       
       let mut store = Store::new(&engine, State { prints: Vec::new() });  // Not yet implemented
       
       // TARGET API: Create a linker and add our imports
       let mut linker = Linker::new(&engine);  // Not yet implemented
       
       // TARGET API: Provide the print function
       linker.func_wrap("print", |mut store: StoreContextMut<State>, msg: String| {  // Not yet implemented
           store.data_mut().prints.push(msg);
           println!("Component says: {}", store.data().prints.last().unwrap());
       })?;
       
       // TARGET API: Instantiate the component
       let instance = linker.instantiate(&mut store, &component)?;  // Not yet implemented
       let calculator = CalculatorWorld::new(&mut store, &instance)?;  // Not yet implemented
       
       // TARGET API: Use it!
       let calc = Calculation {  // Type definitions implemented
           left: 10.0,
           right: 5.0,
           op: Operation::Add,
       };
       
       // TARGET API: Call component functions
       match calculator.example_calculator_calculator()  // Not yet implemented
           .call_calculate(&mut store, &calc)? {
           Ok(result) => println!("10 + 5 = {}", result),
           Err(e) => println!("Error: {:?}", e),
       }
       
       // TARGET API: Check history
       let history = calculator.example_calculator_calculator()  // Not yet implemented
           .call_get_history(&mut store)?;
       println!("History has {} calculations", history.len());
       
       Ok(())
   }

Component Composition 🧩
------------------------

The real power comes from composing components:

.. code-block:: wit
   :caption: composed-app.wit

   package example:math-app@0.1.0;
   
   world app {
       // Import the calculator
       import example:calculator/calculator@0.1.0;
       
       // Import a grapher component
       import example:grapher/graph-display@0.1.0;
       
       // Export our app interface
       export run: func();
   }

Real-World Example: Plugin System 🔌
------------------------------------

Let's build a plugin system using components:

.. code-block:: rust
   :caption: Plugin host system
   :linenos:

   use wasmtime::component::*;
   use std::collections::HashMap;
   
   /// A plugin host that can load calculator plugins
   struct PluginHost {
       engine: Engine,
       plugins: HashMap<String, Component>,
   }
   
   impl PluginHost {
       fn new() -> Result<Self> {
           let mut config = Config::new();
           config.wasm_component_model(true);
           
           Ok(Self {
               engine: Engine::new(&config)?,
               plugins: HashMap::new(),
           })
       }
       
       fn load_plugin(&mut self, name: &str, path: &str) -> Result<()> {
           let component = Component::from_file(&self.engine, path)?;
           self.plugins.insert(name.to_string(), component);
           Ok(())
       }
       
       fn execute_calculation(
           &self,
           plugin_name: &str,
           calc: Calculation
       ) -> Result<f64> {
           let component = self.plugins.get(plugin_name)
               .ok_or_else(|| anyhow!("Plugin not found"))?;
           
           let mut store = Store::new(&self.engine, ());
           let linker = Linker::new(&self.engine);
           
           // Add required imports...
           let instance = linker.instantiate(&mut store, component)?;
           let calculator = CalculatorWorld::new(&mut store, &instance)?;
           
           calculator.example_calculator_calculator()
               .call_calculate(&mut store, &calc)?
               .map_err(|e| anyhow!("Calculation error: {:?}", e))
       }
   }

Advanced Patterns 🎓
--------------------

**Resource Handles:**

.. code-block:: wit
   :caption: Resources in WIT

   interface graphics {
       resource canvas {
           constructor(width: u32, height: u32);
           draw-line: func(x1: u32, y1: u32, x2: u32, y2: u32);
           clear: func();
           get-pixels: func() -> list<u8>;
       }
       
       create-canvas: func(width: u32, height: u32) -> canvas;
   }

**Streaming Data:**

.. code-block:: wit

   interface streaming {
       resource data-stream {
           read: func(max-bytes: u32) -> list<u8>;
           write: func(data: list<u8>) -> result<u32, string>;
           close: func();
       }
   }

Testing Components 🧪
---------------------

.. code-block:: rust
   :caption: Component testing

   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_calculator_operations() {
           // Create test fixtures
           let calculations = vec![
               (10.0, 5.0, Operation::Add, Ok(15.0)),
               (10.0, 5.0, Operation::Subtract, Ok(5.0)),
               (10.0, 5.0, Operation::Multiply, Ok(50.0)),
               (10.0, 0.0, Operation::Divide, Err(CalcError::DivideByZero)),
           ];
           
           for (left, right, op, expected) in calculations {
               let calc = Calculation { left, right, op };
               let result = Calculator::calculate(calc);
               
               match (result, expected) {
                   (Ok(r), Ok(e)) => assert_eq!(r, e),
                   (Err(_), Err(_)) => (), // Both errors, ok
                   _ => panic!("Unexpected result"),
               }
           }
       }
   }

Common Pitfalls 🕳️
-------------------

.. admonition:: Watch Out!
   :class: warning

   1. **State Management**: Components are single-threaded, use thread_local!
   2. **Memory Ownership**: Data crossing boundaries is copied, not referenced
   3. **Version Conflicts**: Be explicit about interface versions
   4. **Import Dependencies**: Missing imports = runtime errors

Best Practices ✨
-----------------

.. admonition:: Do This!
   :class: tip

   1. **Small Interfaces**: Keep WIT interfaces focused and minimal
   2. **Version Everything**: Use semantic versioning in your packages
   3. **Document in WIT**: Use /// comments - they become API docs
   4. **Test Compositions**: Test components both alone and together

Your Turn! 🎯
-------------

Try these challenges:

1. **Add Scientific Functions**: Extend the calculator with sin, cos, sqrt
2. **Create a Logger Component**: Build a component that other components can use for logging
3. **Build a State Machine**: Make a component that manages state transitions

Next Steps 🚶
-------------

- Dive deeper into the component model: :doc:`component/index`
- Learn about resources: :doc:`foundation/resources`
- Explore advanced composition: :doc:`advanced/index`

Remember: Components aren't just a feature - they're the future of portable, composable software. Welcome aboard! 🚂