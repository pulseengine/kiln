=====================
Hello World with PulseEngine
============================

.. epigraph::

   "Hello world. This is a test."
   
   -- Every developer's first words

Welcome to PulseEngine! Let's start with the classic "Hello, World!" - but with a WebAssembly twist. This example will show you how to create, compile, and run your first WebAssembly module using PulseEngine.

.. warning::
   **Work in Progress**: This example shows the intended API design for PulseEngine. 
   The core execution engine is currently under development (15% complete). 
   
   **Current Status**: Infrastructure and type definitions are implemented, but 
   module instantiation and function execution are not yet functional.

.. admonition:: What You'll Learn
   :class: note

   - How to write a simple WebAssembly module in Rust
   - How to compile Rust to WebAssembly
   - How to run WebAssembly with PulseEngine (under development)
   - How to pass data between host and guest

Prerequisites
-------------

Before we start, make sure you have:

.. code-block:: bash

   # Rust toolchain
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # WebAssembly target
   rustup target add wasm32-unknown-unknown
   
   # PulseEngine command-line tool (from source)
   git clone https://github.com/pulseengine/kiln
   cd kiln
   cargo build --bin kilnd

Let's Build Something! 🔨
-------------------------

Step 1: Create Your Project
~~~~~~~~~~~~~~~~~~~~~~~~~~~

First, let's create a new Rust project:

.. code-block:: bash

   cargo new --lib hello-kiln
   cd hello-kiln

Step 2: Configure for WebAssembly
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Update your ``Cargo.toml`` to build a WebAssembly module:

.. code-block:: toml
   :caption: Cargo.toml

   [package]
   name = "hello-kiln"
   version = "0.1.0"
   edition = "2021"

   [lib]
   crate-type = ["cdylib"]

   [dependencies]
   # No dependencies needed for hello world!

Step 3: Write Your First Module
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Now for the fun part - let's write some code:

.. code-block:: rust
   :caption: src/lib.rs
   :linenos:

   //! The simplest WebAssembly module - it adds two numbers!
   
   /// Add two numbers together
   /// This will be callable from the host
   #[no_mangle]
   pub extern "C" fn add(a: i32, b: i32) -> i32 {
       a + b
   }
   
   /// Say hello to someone
   /// This demonstrates string handling
   #[no_mangle]
   pub extern "C" fn greet(name_ptr: *const u8, name_len: usize) -> *const u8 {
       unsafe {
           // Convert the input to a Rust string
           let name_bytes = std::slice::from_raw_parts(name_ptr, name_len);
           let name = std::str::from_utf8_unchecked(name_bytes);
           
           // Create our greeting
           let greeting = format!("Hello, {}! Welcome to PulseEngine!", name);
           
           // Leak the string so it persists after this function returns
           // In a real app, you'd want proper memory management!
           let leaked = Box::leak(greeting.into_boxed_str());
           leaked.as_ptr()
       }
   }
   
   /// Get the length of the last greeting
   /// (This is a simple way to handle string returns)
   #[no_mangle]
   pub extern "C" fn last_greeting_len() -> usize {
       // In a real implementation, you'd track this properly
       // For now, let's just return a reasonable length
       30
   }

.. warning::

   The string handling here is simplified for the example. In production code, you'd want proper memory management and safety checks!

Step 4: Build Your Module
~~~~~~~~~~~~~~~~~~~~~~~~~

Time to compile to WebAssembly:

.. code-block:: bash

   cargo build --target wasm32-unknown-unknown --release

Your WebAssembly module is now at:
``target/wasm32-unknown-unknown/release/hello_kiln.wasm``

Step 5: Run It! 🚀
~~~~~~~~~~~~~~~~~~

Let's create a simple runner to test our module:

.. code-block:: rust
   :caption: examples/run_hello.rs (Intended API - Under Development)

   // This code shows the target API design
   // Current implementation status: Infrastructure exists, execution engine in progress
   
   use kiln::prelude::*;
   
   fn main() -> Result<(), Box<dyn std::error::Error>> {
       // TARGET API: Load the WebAssembly module
       let bytes = include_bytes!("../target/wasm32-unknown-unknown/release/hello_kiln.wasm");
       let module = Module::from_bytes(bytes)?;  // Not yet implemented
       
       // TARGET API: Create an instance  
       let instance = ModuleInstance::new(module, imports)?;  // Not yet implemented
       
       // TARGET API: Call functions
       let add_fn = instance.get_export("add").expect("add function not found");
       let result = add_fn.call(&[Value::I32(5), Value::I32(3)])?;  // Not yet implemented
       println!("5 + 3 = {:?}", result[0]);
       
       // Note: This represents the planned API design
       // Current status: Type definitions exist, execution engine under development
       println!("Greeting: Hello, PulseEngine User! Welcome to PulseEngine!");
       
       Ok(())
   }

Or use the command-line tool (when execution engine is complete):

.. code-block:: bash

   # Planned CLI usage (under development)
   kilnd run target/wasm32-unknown-unknown/release/hello_kiln.wasm
   
   # Module inspection (partially implemented)
   kilnd inspect target/wasm32-unknown-unknown/release/hello_kiln.wasm

You should see output like:

.. code-block:: text

   Module: hello_kiln.wasm
   Exports:
     - add: [i32, i32] -> [i32]
     - greet: [i32, i32] -> [i32]
     - last_greeting_len: [] -> [i32]

What Just Happened? 🤔
----------------------

Let's break down what we just did:

1. **Created a Library**: We made a Rust library that compiles to WebAssembly
2. **Exported Functions**: The ``#[no_mangle]`` and ``extern "C"`` make our functions callable from the host
3. **Handled Data**: We showed basic number operations and (simplified) string handling
4. **Compiled to WASM**: Rust's toolchain made it easy to target WebAssembly
5. **Target API**: We showed how modules will be loaded and executed with PulseEngine (execution engine under development)

Common Gotchas 🎣
-----------------

.. admonition:: Watch Out For These!
   :class: warning

   **Memory Management**: WebAssembly modules have their own linear memory. Passing complex data types requires careful coordination.
   
   **String Handling**: Strings need special handling since WebAssembly only understands numbers. You'll usually pass pointers and lengths.
   
   **No Standard Library**: By default, WebAssembly doesn't have access to system calls. You need to explicitly import what you need.

Next Steps 🎯
-------------

Now that you've got your first module running:

1. **Try the Component Model**: Check out :doc:`basic_component` to see the modern way of building WebAssembly
2. **Learn Memory Management**: See :doc:`foundation/safe_memory` for memory handling examples
3. **Add Host Functions**: Learn how to give your modules superpowers in :doc:`host/functions`

.. admonition:: Challenge
   :class: tip

   Can you modify the example to:
   - Add a ``multiply`` function?
   - Create a function that returns the larger of two numbers?
   - Make a function that counts the vowels in a string?

Remember: Every expert was once a beginner. You've just taken your first step into the world of WebAssembly with PulseEngine! 🎉