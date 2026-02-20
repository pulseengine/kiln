============================
Async Executor Integration
============================

.. image:: ../_static/icons/component_model.svg
   :width: 64px
   :align: right
   :alt: Async Icon

.. epigraph::

   "Asynchronous programming is like juggling - it looks impossible until you understand the pattern."
   
   -- Kiln Development Team

The Kiln provides a pluggable async executor system that allows you to integrate your preferred async runtime while maintaining compatibility across std, no_std+alloc, and pure no_std environments.

.. contents:: On this page
   :local:
   :depth: 3

Overview
========

The pluggable executor system consists of three key components:

1. **WrtExecutor Trait**: The interface that all executors must implement
2. **Executor Registry**: Global registration system for managing executors
3. **Fallback Executor**: Built-in minimal executor for when no external executor is provided

Key Features
------------

.. list-table:: Executor System Features
   :header-rows: 1
   :widths: 30 70

   * - Feature
     - Description
   * - **Zero Dependencies**
     - Fallback executor works without any external crates
   * - **Platform Agnostic**
     - Works across desktop, embedded, and real-time systems
   * - **Type Safe**
     - Trait-based design ensures compatibility
   * - **Gradual Adoption**
     - Start with fallback, upgrade to advanced executor later
   * - **no_std Compatible**
     - Full functionality in resource-constrained environments

Architecture
============

Executor Trait
---------------

The core ``WrtExecutor`` trait defines the interface that all executors must implement:

.. code-block:: rust

   pub trait WrtExecutor: Send + Sync {
       /// Spawn a future onto the executor
       fn spawn(&self, future: BoxedFuture<'_, ()>) -> Result<TaskHandle, ExecutorError>;
       
       /// Block on a future until completion
       fn block_on<F: Future>(&self, future: F) -> Result<F::Output, ExecutorError>;
       
       /// Poll all ready tasks once (for cooperative executors)
       fn poll_once(&self) -> Result<(), ExecutorError>;
       
       /// Check if the executor is still running
       fn is_running(&self) -> bool;
       
       /// Shutdown the executor gracefully
       fn shutdown(&self) -> Result<(), ExecutorError>;
   }

Executor Registry
-----------------

The registry provides a global mechanism for managing executor instances:

.. code-block:: rust

   // Register a custom executor
   register_executor(Box::new(MyExecutor))?;
   
   // Get the current executor (custom or fallback)
   let executor = current_executor();
   
   // Check if using fallback
   if is_using_fallback() {
       println!("Using built-in fallback executor");
   }

Fallback Executor
-----------------

The built-in fallback executor provides basic async functionality without dependencies:

- **Task Limit**: Supports up to 32 concurrent tasks
- **Execution Model**: Simple polling-based execution
- **Memory Usage**: Uses bounded collections for task storage
- **Compatibility**: Works in pure no_std environments

Using the Async API
===================

Basic Usage
-----------

The async API works automatically with either the fallback or a registered executor:

.. code-block:: rust

   use kiln_foundation::{current_executor, with_async};
   
   // Simple async function
   async fn hello_async() -> &'static str {
       "Hello from async!"
   }
   
   fn main() -> Result<(), ExecutorError> {
       // Use the current executor (fallback by default)
       let result = with_async(hello_async())?;
       println!("Result: {}", result);
       Ok(())
   }

Using AsyncRuntime
------------------

The ``AsyncRuntime`` provides a convenient wrapper for async operations:

.. code-block:: rust

   use kiln_foundation::AsyncRuntime;
   
   let runtime = AsyncRuntime::new();
   
   // Execute async operations
   let result = runtime.execute_async(async {
       // Your async code here
       42
   }).await?;

Component Model Integration
---------------------------

When both ``async-api`` and ``component-model-async`` features are enabled, you can bridge Component Model async types with Rust futures:

.. code-block:: rust

   use kiln_foundation::{ComponentFuture, ComponentFutureBridge, ComponentAsyncExt};
   
   // Convert Component Model future to Rust future
   let component_future = ComponentFuture::new(handle, value_type);
   let rust_future = ComponentFutureBridge::new(component_future);
   let result = rust_future.await?;

Implementing Custom Executors
=============================

Embassy Integration
-------------------

For embedded systems, integrate with the Embassy executor:

.. code-block:: rust

   use embassy_executor::Executor;
   use kiln_foundation::{WrtExecutor, ExecutorError, TaskHandle, BoxedFuture};
   
   struct EmbassyAdapter {
       executor: &'static Executor,
   }
   
   impl WrtExecutor for EmbassyAdapter {
       fn spawn(&self, future: BoxedFuture<'_, ()>) -> Result<TaskHandle, ExecutorError> {
           self.executor.spawner().spawn(async move {
               future.await;
           }).map_err(|_| ExecutorError::OutOfResources)?;
           
           Ok(TaskHandle { id: 0, waker: None })
       }
       
       fn block_on<F: Future>(&self, _future: F) -> Result<F::Output, ExecutorError> {
           // Embassy doesn't support block_on in no_std
           Err(ExecutorError::NotSupported)
       }
       
       fn is_running(&self) -> bool { true }
       fn shutdown(&self) -> Result<(), ExecutorError> { Ok(()) }
   }
   
   // Register the Embassy adapter
   fn init_embassy(executor: &'static Executor) {
       let adapter = Box::new(EmbassyAdapter { executor });
       register_executor(adapter).expect("Failed to register Embassy");
   }

Tokio Integration
-----------------

For desktop applications, integrate with Tokio:

.. code-block:: rust

   #[cfg(feature = "std")]
   use tokio::runtime::Runtime;
   
   struct TokioAdapter {
       runtime: Runtime,
   }
   
   impl WrtExecutor for TokioAdapter {
       fn spawn(&self, future: BoxedFuture<'_, ()>) -> Result<TaskHandle, ExecutorError> {
           self.runtime.spawn(future);
           Ok(TaskHandle { id: 0, waker: None })
       }
       
       fn block_on<F: Future>(&self, future: F) -> Result<F::Output, ExecutorError> {
           Ok(self.runtime.block_on(future))
       }
       
       fn is_running(&self) -> bool { true }
       fn shutdown(&self) -> Result<(), ExecutorError> { Ok(()) }
   }

Custom Executor
---------------

For specialized needs, implement a custom executor:

.. code-block:: rust

   struct CustomExecutor {
       task_queue: Mutex<BoundedVec<Task, 64>>,
       running: AtomicBool,
   }
   
   impl WrtExecutor for CustomExecutor {
       fn spawn(&self, future: BoxedFuture<'_, ()>) -> Result<TaskHandle, ExecutorError> {
           let mut queue = self.task_queue.lock();
           let id = generate_task_id();
           
           queue.push(Task::new(id, future))
               .map_err(|_| ExecutorError::OutOfResources)?;
               
           Ok(TaskHandle { id, waker: None })
       }
       
       fn block_on<F: Future>(&self, future: F) -> Result<F::Output, ExecutorError> {
           // Implement blocking execution
           // ...
       }
       
       // Implement other required methods
       // ...
   }

Configuration
=============

Feature Flags
-------------

Enable async support through Cargo features:

.. code-block:: toml

   [dependencies]
   kiln-foundation = { version = "0.1", features = ["async-api"] }
   
   # For Component Model async integration
   kiln-foundation = { version = "0.1", features = ["async-api", "component-model-async"] }

Environment-Specific Configuration
----------------------------------

The async system adapts to your environment:

.. list-table:: Environment Support
   :header-rows: 1
   :widths: 20 40 40

   * - Environment
     - Recommended Executor
     - Notes
   * - **Desktop (std)**
     - Tokio, async-std
     - Full async ecosystem available
   * - **Embedded (no_std)**
     - Embassy, custom
     - Optimized for resource constraints
   * - **Real-time (QNX)**
     - Custom implementation
     - Deterministic scheduling required
   * - **Bare metal**
     - Fallback executor
     - Minimal overhead, polling-based

Performance Considerations
==========================

Task Limits
-----------

The fallback executor has a built-in task limit to prevent resource exhaustion:

.. code-block:: rust

   // Maximum concurrent tasks in fallback executor
   pub const MAX_TASKS: usize = 32;

For applications requiring more concurrent tasks, use a custom executor.

Memory Usage
------------

.. list-table:: Memory Usage Comparison
   :header-rows: 1
   :widths: 30 25 45

   * - Executor Type
     - Memory Overhead
     - Notes
   * - **Fallback**
     - ~2KB stack
     - Fixed allocation, no heap
   * - **Embassy**
     - ~1KB stack
     - Highly optimized for embedded
   * - **Tokio**
     - ~8KB+ heap
     - Full-featured, desktop-oriented
   * - **Custom**
     - Varies
     - Depends on implementation

Execution Model
---------------

.. code-block:: rust

   // Cooperative scheduling with fallback executor
   loop {
       executor.poll_once()?;
       
       // Yield to other tasks
       if let Some(sleep_duration) = calculate_sleep() {
           std::thread::sleep(sleep_duration);
       }
   }

Debugging and Monitoring
========================

Executor Status
---------------

Check executor status for debugging:

.. code-block:: rust

   let executor = current_executor();
   
   if !executor.is_running() {
       eprintln!("Warning: Executor is not running");
   }
   
   if is_using_fallback() {
       println!("Using fallback executor (consider registering a custom executor)");
   }

Error Handling
--------------

Handle common executor errors:

.. code-block:: rust

   match executor.spawn(future) {
       Ok(handle) => println!("Task spawned: {:?}", handle),
       Err(ExecutorError::OutOfResources) => {
           eprintln!("Too many concurrent tasks");
       }
       Err(ExecutorError::NotRunning) => {
           eprintln!("Executor has been shut down");
       }
       Err(e) => eprintln!("Executor error: {:?}", e),
   }

Best Practices
==============

Executor Selection
------------------

Choose the right executor for your environment:

1. **Start with Fallback**: Test basic functionality
2. **Evaluate Requirements**: Determine concurrency and performance needs
3. **Select Appropriate Executor**: Based on platform and constraints
4. **Profile and Optimize**: Measure actual performance

Error Handling
--------------

Always handle async errors gracefully:

.. code-block:: rust

   // Good: Explicit error handling
   match with_async(risky_operation()).await {
       Ok(result) => process_result(result),
       Err(ExecutorError::TaskPanicked) => handle_panic(),
       Err(e) => log_error(e),
   }
   
   // Avoid: Unwrapping async results
   // let result = with_async(operation()).unwrap(); // DON'T DO THIS

Resource Management
-------------------

Be mindful of task lifecycle:

.. code-block:: rust

   // Spawn bounded number of tasks
   const MAX_CONCURRENT_DOWNLOADS: usize = 10;
   let semaphore = Semaphore::new(MAX_CONCURRENT_DOWNLOADS);
   
   for url in urls {
       let permit = semaphore.acquire().await?;
       executor.spawn(Box::pin(async move {
           let _permit = permit; // Hold permit until task completes
           download_file(url).await
       }))?;
   }

Migration Guide
===============

From Futures Crate
-------------------

If migrating from the ``futures`` crate:

.. code-block:: rust

   // Before: Using futures crate
   use futures::executor::block_on;
   let result = block_on(my_async_fn());
   
   // After: Using Kiln async API
   use kiln_foundation::with_async;
   let result = with_async(my_async_fn())?;

From Custom Async
------------------

If you have existing async infrastructure:

1. **Implement WrtExecutor**: Wrap your existing executor
2. **Register Early**: Set up executor before any async operations
3. **Test Incrementally**: Migrate async code gradually
4. **Monitor Performance**: Compare with previous implementation

Troubleshooting
===============

Common Issues
-------------

.. list-table:: Common Problems and Solutions
   :header-rows: 1
   :widths: 40 60

   * - Problem
     - Solution
   * - "Executor not running"
     - Check if executor was properly registered
   * - "Out of resources"
     - Reduce concurrent tasks or use custom executor
   * - "Task panicked"
     - Add proper error handling to async functions
   * - "Blocking in async context"
     - Use async alternatives or spawn_blocking

Debugging Tips
--------------

1. **Enable Logging**: Use the logging system to trace executor behavior
2. **Check Task Limits**: Monitor concurrent task count
3. **Profile Memory**: Measure actual memory usage vs. expectations
4. **Test Edge Cases**: Verify behavior under resource pressure

Further Reading
===============

- :doc:`../examples/foundation/async_examples` - Practical examples
- :doc:`../architecture/04_dynamic_behavior/concurrency_model` - Concurrency architecture
- :doc:`no_std_development` - no_std development practices
- `WebAssembly Component Model Async Specification <https://github.com/WebAssembly/component-model/blob/main/design/mvp/Async.md>`_