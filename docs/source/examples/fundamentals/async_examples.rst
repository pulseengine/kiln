===========================
Async Programming Examples
===========================

.. image:: ../../_static/icons/execution_flow.svg
   :width: 64px
   :align: right
   :alt: Async Icon

.. epigraph::

   "Async programming is like ordering coffee - you don't wait around for it to brew, you do other things and come back when it's ready."
   
   -- Kiln Async Team

The Kiln async system lets you write asynchronous code that works seamlessly across std, no_std+alloc, and pure no_std environments. Whether you're running on a desktop or a microcontroller with 64KB of RAM, the same async patterns work everywhere.

.. contents:: Async Examples
   :local:
   :depth: 3

Quick Start
===========

Basic Async Function
--------------------

Let's start with the simplest possible async example:

.. code-block:: rust

   use kiln_foundation::{with_async, ExecutorError};
   
   async fn greet(name: &str) -> String {
       format!("Hello, {}!", name)
   }
   
   fn main() -> Result<(), ExecutorError> {
       // Use the built-in fallback executor
       let greeting = with_async(greet("Kiln"))?;
       println!("{}", greeting); // Prints: Hello, Kiln!
       Ok(())
   }

.. admonition:: What's Happening Here?
   :class: note

   1. ``with_async()`` runs the async function using the current executor
   2. If no custom executor is registered, it uses the built-in fallback executor
   3. The fallback executor works in any environment - even pure no_std!

Checking Executor Status
------------------------

Before diving into complex async code, it's good to know what executor you're using:

.. code-block:: rust

   use kiln_foundation::{is_using_fallback, current_executor};
   
   fn main() {
       if is_using_fallback() {
           println!("Using built-in fallback executor");
           println!("Consider registering a custom executor for better performance");
       } else {
           println!("Using custom executor");
       }
       
       let executor = current_executor();
       println!("Executor running: {}", executor.is_running());
   }

Using the AsyncRuntime
======================

For more complex async operations, use the ``AsyncRuntime`` wrapper:

Basic Runtime Usage
-------------------

.. code-block:: rust

   use kiln_foundation::{AsyncRuntime, ExecutorError};
   
   async fn fetch_data(id: u32) -> Result<String, &'static str> {
       // Simulate async work
       if id == 42 {
           Ok("The answer to everything".to_string())
       } else {
           Err("Data not found")
       }
   }
   
   fn main() -> Result<(), ExecutorError> {
       let runtime = AsyncRuntime::new();
       
       // Execute async operation
       let result = runtime.block_on(async {
           match fetch_data(42).await {
               Ok(data) => {
                   println!("Success: {}", data);
                   Ok(())
               }
               Err(e) => {
                   eprintln!("Error: {}", e);
                   Err("Failed to fetch data")
               }
           }
       })?;
       
       Ok(())
   }

Spawning Background Tasks
-------------------------

The async system supports spawning tasks that run in the background:

.. code-block:: rust

   use kiln_foundation::{current_executor, ExecutorError};
   use core::future::Future;
   use core::pin::Pin;
   
   async fn background_task(task_id: u32) {
       println!("Background task {} starting", task_id);
       
       // Simulate some work
       for i in 0..3 {
           println!("Task {} step {}", task_id, i);
           // In a real implementation, you'd yield to other tasks here
       }
       
       println!("Background task {} complete", task_id);
   }
   
   fn main() -> Result<(), ExecutorError> {
       let executor = current_executor();
       
       // Spawn multiple background tasks
       for i in 1..=3 {
           let future = Box::pin(async move {
               background_task(i).await;
           });
           executor.spawn(future)?;
       }
       
       // Poll the executor to run tasks
       for _ in 0..10 {
           executor.poll_once()?;
       }
       
       Ok(())
   }

Working with Futures
====================

Creating Custom Futures
------------------------

Sometimes you need to create your own futures for specific use cases:

.. code-block:: rust

   use core::future::Future;
   use core::pin::Pin;
   use core::task::{Context, Poll};
   
   /// A future that counts down from a given number
   pub struct CountdownFuture {
       count: u32,
   }
   
   impl CountdownFuture {
       pub fn new(count: u32) -> Self {
           Self { count }
       }
   }
   
   impl Future for CountdownFuture {
       type Output = &'static str;
       
       fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
           if self.count == 0 {
               Poll::Ready("Countdown complete!")
           } else {
               println!("Counting down: {}", self.count);
               self.count -= 1;
               Poll::Pending
           }
       }
   }
   
   // Usage
   use kiln_foundation::with_async;
   
   fn main() -> Result<(), ExecutorError> {
       let countdown = CountdownFuture::new(3);
       let result = with_async(countdown)?;
       println!("{}", result);
       Ok(())
   }

Combining Multiple Futures
---------------------------

You can combine multiple async operations:

.. code-block:: rust

   async fn fetch_user(id: u32) -> Result<String, &'static str> {
       match id {
           1 => Ok("Alice".to_string()),
           2 => Ok("Bob".to_string()),
           _ => Err("User not found"),
       }
   }
   
   async fn fetch_user_posts(user: &str) -> Result<Vec<String>, &'static str> {
       match user {
           "Alice" => Ok(vec!["Hello World".to_string(), "Rust is great".to_string()]),
           "Bob" => Ok(vec!["async/await rocks".to_string()]),
           _ => Err("No posts found"),
       }
   }
   
   async fn get_user_info(id: u32) -> Result<(String, Vec<String>), &'static str> {
       // Sequential async operations
       let user = fetch_user(id).await?;
       let posts = fetch_user_posts(&user).await?;
       Ok((user, posts))
   }
   
   fn main() -> Result<(), ExecutorError> {
       let result = with_async(async {
           match get_user_info(1).await {
               Ok((user, posts)) => {
                   println!("User: {}", user);
                   println!("Posts: {:?}", posts);
               }
               Err(e) => eprintln!("Error: {}", e),
           }
       })?;
       
       Ok(())
   }

Custom Executor Integration
===========================

Registering a Custom Executor
------------------------------

Here's how to create and register a simple custom executor:

.. code-block:: rust

   use kiln_foundation::{
       WrtExecutor, ExecutorError, TaskHandle, BoxedFuture,
       register_executor, current_executor
   };
   use core::future::Future;
   
   /// Simple custom executor that logs all operations
   struct LoggingExecutor {
       name: &'static str,
   }
   
   impl LoggingExecutor {
       fn new(name: &'static str) -> Self {
           Self { name }
       }
   }
   
   impl WrtExecutor for LoggingExecutor {
       fn spawn(&self, future: BoxedFuture<'_, ()>) -> Result<TaskHandle, ExecutorError> {
           println!("[{}] Spawning task", self.name);
           // In a real implementation, you'd store and execute the future
           Ok(TaskHandle { id: 1, waker: None })
       }
       
       fn block_on<F: Future>(&self, future: F) -> Result<F::Output, ExecutorError> {
           println!("[{}] Blocking on future", self.name);
           // In a real implementation, you'd run the future to completion
           // For this example, we'll return an error
           Err(ExecutorError::NotSupported)
       }
       
       fn poll_once(&self) -> Result<(), ExecutorError> {
           println!("[{}] Polling tasks", self.name);
           Ok(())
       }
       
       fn is_running(&self) -> bool {
           true
       }
       
       fn shutdown(&self) -> Result<(), ExecutorError> {
           println!("[{}] Shutting down", self.name);
           Ok(())
       }
   }
   
   fn main() -> Result<(), ExecutorError> {
       // Create and register custom executor
       let custom_executor = Box::new(LoggingExecutor::new("MyExecutor"));
       register_executor(custom_executor)?;
       
       // Now all async operations will use our custom executor
       let executor = current_executor();
       println!("Using custom executor: {}", !is_using_fallback());
       
       // Try to spawn a task (will log the operation)
       let future = Box::pin(async {
           println!("Task is running!");
       });
       executor.spawn(future)?;
       
       Ok(())
   }

Embassy Integration Example
---------------------------

For embedded systems, you can integrate with Embassy:

.. code-block:: rust

   #[cfg(feature = "embassy")]
   mod embassy_integration {
       use kiln_foundation::{WrtExecutor, ExecutorError, TaskHandle, BoxedFuture};
       use embassy_executor::Executor;
       use core::future::Future;
       
       pub struct EmbassyAdapter {
           executor: &'static Executor,
       }
       
       impl EmbassyAdapter {
           pub fn new(executor: &'static Executor) -> Self {
               Self { executor }
           }
       }
       
       impl WrtExecutor for EmbassyAdapter {
           fn spawn(&self, future: BoxedFuture<'_, ()>) -> Result<TaskHandle, ExecutorError> {
               self.executor.spawner().spawn(async move {
                   future.await;
               }).map_err(|_| ExecutorError::OutOfResources)?;
               
               Ok(TaskHandle { id: 0, waker: None })
           }
           
           fn block_on<F: Future>(&self, _future: F) -> Result<F::Output, ExecutorError> {
               // Embassy doesn't support block_on in no_std environments
               Err(ExecutorError::NotSupported)
           }
           
           fn poll_once(&self) -> Result<(), ExecutorError> {
               // Embassy handles polling internally
               Ok(())
           }
           
           fn is_running(&self) -> bool { true }
           fn shutdown(&self) -> Result<(), ExecutorError> { Ok(()) }
       }
       
       pub fn setup_embassy(executor: &'static Executor) -> Result<(), ExecutorError> {
           let adapter = Box::new(EmbassyAdapter::new(executor));
           kiln_foundation::register_executor(adapter)
       }
   }

Component Model Async Integration
=================================

When both ``async-api`` and ``component-model-async`` features are enabled, you can work with Component Model async types:

Working with Component Futures
-------------------------------

.. code-block:: rust

   #[cfg(all(feature = "async-api", feature = "component-model-async"))]
   mod component_async {
       use kiln_foundation::{
           ComponentFuture, ComponentFutureBridge, FutureHandle,
           AsyncRuntime, ValType
       };
       
       async fn work_with_component_future() -> Result<u32, ExecutorError> {
           // Create a Component Model future
           let component_future = ComponentFuture::new(
               FutureHandle(42),
               ValType::I32
           );
           
           // Bridge it to a Rust future
           let rust_future = ComponentFutureBridge::new(component_future);
           
           // Await the result
           let result = rust_future.await?;
           Ok(result)
       }
       
       pub fn example() -> Result<(), ExecutorError> {
           let runtime = AsyncRuntime::new();
           runtime.block_on(async {
               match work_with_component_future().await {
                   Ok(value) => println!("Component future result: {}", value),
                   Err(e) => eprintln!("Error: {:?}", e),
               }
           })
       }
   }

Working with Component Streams
------------------------------

.. code-block:: rust

   #[cfg(all(feature = "async-api", feature = "component-model-async"))]
   mod component_streams {
       use kiln_foundation::{
           ComponentStream, ComponentStreamBridge, StreamHandle,
           ValType
       };
       
       async fn process_stream() -> Result<Vec<u32>, ExecutorError> {
           let mut component_stream = ComponentStream::new(
               StreamHandle(1),
               ValType::I32
           );
           
           // Write some test data
           component_stream.try_write(1).unwrap();
           component_stream.try_write(2).unwrap();
           component_stream.try_write(3).unwrap();
           component_stream.close_write();
           
           let mut stream_bridge = ComponentStreamBridge::new(component_stream);
           let mut results = Vec::new();
           
           // Read all values from the stream
           loop {
               match stream_bridge.poll_next(&mut Context::from_waker(&waker)) {
                   Poll::Ready(Some(value)) => results.push(value),
                   Poll::Ready(None) => break, // Stream closed
                   Poll::Pending => continue,
               }
           }
           
           Ok(results)
       }
   }

Error Handling Patterns
========================

Graceful Error Handling
------------------------

Always handle async errors gracefully:

.. code-block:: rust

   use kiln_foundation::{ExecutorError, with_async};
   
   async fn risky_operation(should_fail: bool) -> Result<&'static str, &'static str> {
       if should_fail {
           Err("Something went wrong!")
       } else {
           Ok("Success!")
       }
   }
   
   fn main() {
       // Good: Explicit error handling
       match with_async(risky_operation(false)) {
           Ok(Ok(result)) => println!("Operation succeeded: {}", result),
           Ok(Err(app_error)) => eprintln!("Application error: {}", app_error),
           Err(ExecutorError::TaskPanicked) => eprintln!("Task panicked!"),
           Err(ExecutorError::OutOfResources) => eprintln!("Too many concurrent tasks"),
           Err(e) => eprintln!("Executor error: {:?}", e),
       }
   }

Timeout Handling
----------------

Implement timeouts for async operations:

.. code-block:: rust

   use core::future::Future;
   use core::pin::Pin;
   use core::task::{Context, Poll};
   
   /// A future that times out after a certain number of polls
   pub struct TimeoutFuture<F> {
       future: Pin<Box<F>>,
       remaining_polls: u32,
   }
   
   impl<F: Future> TimeoutFuture<F> {
       pub fn new(future: F, max_polls: u32) -> Self {
           Self {
               future: Box::pin(future),
               remaining_polls: max_polls,
           }
       }
   }
   
   impl<F: Future> Future for TimeoutFuture<F> {
       type Output = Result<F::Output, &'static str>;
       
       fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
           if self.remaining_polls == 0 {
               return Poll::Ready(Err("Timeout"));
           }
           
           self.remaining_polls -= 1;
           
           match self.future.as_mut().poll(cx) {
               Poll::Ready(output) => Poll::Ready(Ok(output)),
               Poll::Pending => Poll::Pending,
           }
       }
   }
   
   // Usage
   async fn slow_operation() -> &'static str {
       // This would normally take a long time
       "Finally done!"
   }
   
   fn main() -> Result<(), ExecutorError> {
       let timeout_future = TimeoutFuture::new(slow_operation(), 5);
       
       match with_async(timeout_future)? {
           Ok(result) => println!("Success: {}", result),
           Err(timeout_err) => eprintln!("Timeout: {}", timeout_err),
       }
       
       Ok(())
   }

Performance Patterns
====================

Task Pooling
------------

Limit concurrent tasks to avoid resource exhaustion:

.. code-block:: rust

   use kiln_foundation::{current_executor, ExecutorError};
   use kiln_foundation::bounded_collections::BoundedVec;
   
   const MAX_CONCURRENT_TASKS: usize = 8;
   
   async fn process_item(id: u32) -> Result<String, &'static str> {
       // Simulate processing
       Ok(format!("Processed item {}", id))
   }
   
   fn process_items_concurrently(items: &[u32]) -> Result<(), ExecutorError> {
       let executor = current_executor();
       let mut active_tasks = BoundedVec::<TaskHandle, MAX_CONCURRENT_TASKS>::new();
       
       for &item_id in items {
           // Wait if we've hit the task limit
           while active_tasks.len() >= MAX_CONCURRENT_TASKS {
               // In a real implementation, you'd wait for tasks to complete
               executor.poll_once()?;
               // Remove completed tasks (simplified)
               active_tasks.clear(); // In reality, you'd track completion
           }
           
           // Spawn new task
           let future = Box::pin(async move {
               match process_item(item_id).await {
                   Ok(result) => println!("{}", result),
                   Err(e) => eprintln!("Error processing {}: {}", item_id, e),
               }
           });
           
           let handle = executor.spawn(future)?;
           active_tasks.push(handle).unwrap(); // Safe due to length check above
       }
       
       // Wait for remaining tasks to complete
       while !active_tasks.is_empty() {
           executor.poll_once()?;
           // Remove completed tasks (simplified)
           active_tasks.clear();
       }
       
       Ok(())
   }

Memory-Efficient Async
----------------------

Use bounded collections to manage memory usage:

.. code-block:: rust

   use kiln_foundation::bounded_collections::{BoundedVec, BoundedQueue};
   
   const BUFFER_SIZE: usize = 32;
   
   /// Async producer-consumer pattern with bounded queues
   pub struct AsyncProducerConsumer<T> {
       queue: BoundedQueue<T, BUFFER_SIZE>,
   }
   
   impl<T> AsyncProducerConsumer<T> {
       pub fn new() -> Self {
           Self {
               queue: BoundedQueue::new(),
           }
       }
       
       pub async fn produce(&mut self, item: T) -> Result<(), &'static str> {
           // In a real implementation, this would wait for space
           self.queue.push(item).map_err(|_| "Queue full")
       }
       
       pub async fn consume(&mut self) -> Option<T> {
           // In a real implementation, this would wait for items
           self.queue.pop()
       }
   }

Best Practices Summary
======================

Do's and Don'ts
---------------

.. list-table:: Async Best Practices
   :header-rows: 1
   :widths: 50 50

   * - ✅ Do
     - ❌ Don't
   * - Use ``with_async()`` for simple cases
     - Call ``.unwrap()`` on async results
   * - Handle all error cases explicitly
     - Block indefinitely without timeouts
   * - Limit concurrent task count
     - Spawn unlimited background tasks
   * - Use bounded collections
     - Allocate unbounded memory
   * - Register executors early
     - Change executors during runtime
   * - Test with fallback executor first
     - Assume custom executor is available

Performance Tips
----------------

1. **Start Simple**: Use the fallback executor for prototyping
2. **Measure First**: Profile before optimizing async code
3. **Bound Resources**: Always limit concurrent tasks and memory usage
4. **Choose Right Executor**: Match executor to your environment
5. **Error Fast**: Fail quickly rather than hanging indefinitely

Memory Guidelines
-----------------

1. **No Hidden Allocations**: Kiln async system never allocates secretly
2. **Bounded Everything**: Use bounded collections for task queues
3. **Stack-First**: Prefer stack allocation over heap when possible
4. **Measure Usage**: Monitor actual memory consumption

Debugging Tips
==============

Common Issues
-------------

.. list-table:: Troubleshooting Async Issues
   :header-rows: 1
   :widths: 40 60

   * - Problem
     - Solution
   * - "Executor not running"
     - Check if executor was properly registered and is_running() returns true
   * - "Out of resources"
     - Reduce MAX_TASKS or implement task pooling
   * - "Task never completes"
     - Add timeout handling and debug polling behavior
   * - "Panics in async code"
     - Add explicit error handling, avoid unwrap()

Logging Async Operations
------------------------

Add logging to track async execution:

.. code-block:: rust

   async fn logged_operation(id: u32) -> Result<String, &'static str> {
       println!("Starting operation {}", id);
       
       // Simulate work
       let result = format!("Result for {}", id);
       
       println!("Completed operation {}", id);
       Ok(result)
   }
   
   fn main() -> Result<(), ExecutorError> {
       let result = with_async(logged_operation(42))?;
       match result {
           Ok(value) => println!("Final result: {}", value),
           Err(e) => eprintln!("Operation failed: {}", e),
       }
       Ok(())
   }

Next Steps
==========

Now that you've learned the basics of Kiln async programming:

1. **Experiment**: Try the examples with different executors
2. **Build**: Create your own async functions and patterns
3. **Integrate**: Choose and integrate an appropriate executor for your platform
4. **Optimize**: Profile and tune performance for your specific use case

For more advanced topics, see:

- :doc:`../../development/async_executor_integration` - Technical implementation details
- :doc:`../../architecture/04_dynamic_behavior/concurrency_model` - Concurrency architecture
- :doc:`../platform/` - Platform-specific async examples
- `Component Model Async Specification <https://github.com/WebAssembly/component-model/blob/main/design/mvp/Async.md>`_