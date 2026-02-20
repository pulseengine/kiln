//! Example demonstrating how to use the simple async executor system
//!
//! This example shows:
//! 1. Using the simple async executor
//! 2. Working with basic async/await patterns
//! 3. Integration patterns for async code

#[cfg(feature = "async-api")]
use core::{
    future::Future,
    marker::Unpin,
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};

#[cfg(feature = "async-api")]
use wrt_foundation::{
    is_using_fallback,
    with_async,
    AsyncRuntime,
    ExecutorError,
};

#[cfg(all(feature = "async-api", feature = "std"))]
extern crate alloc;
#[cfg(all(feature = "async-api", feature = "std"))]
use std::boxed::Box;

/// Simple async function for testing
#[cfg(feature = "async-api")]
async fn hello_async() -> &'static str {
    "Hello from simple async!"
}

/// Example future that is immediately ready
#[cfg(feature = "async-api")]
#[derive(Debug)]
struct ReadyFuture {
    value: &'static str,
}

#[cfg(feature = "async-api")]
impl Future for ReadyFuture {
    type Output = &'static str;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.value)
    }
}

#[cfg(feature = "async-api")]
impl Unpin for ReadyFuture {}

#[cfg(feature = "async-api")]
fn main() {
    println!("=== Simple Async Executor Example ===\n");

    // 1. Check initial state - should be using fallback
    println!("1. Initial state:");
    println!("   Using fallback executor: {}", is_using_fallback);

    // 2. Use the simple async runtime
    println!("\n2. Using AsyncRuntime:");
    let runtime = AsyncRuntime::new();

    // Test with a simple ready future
    let ready_future = ReadyFuture {
        value: "Ready immediately!",
    };
    match runtime.block_on(ready_future) {
        Ok(result) => println!("   Ready future result: {}", result),
        Err(e) => println!("   Ready future error: {:?}", e),
    }

    // 3. Using the with_async helper with ready futures
    println!("\n3. Using with_async helper:");

    #[cfg(feature = "std")]
    {
        // Create an async block that's immediately ready
        let async_block = async { "Async block result" };

        // Note: This requires the future to be Unpin, so we'll pin it
        let pinned_future = Box::pin(async_block);
        match with_async(pinned_future) {
            Ok(result) => println!("   Result: {}", result),
            Err(e) => println!("   Error: {:?}", e),
        }

        // 4. Example of what happens with pending futures
        println!("\n4. Pending futures (expected to fail):");
        let pending_future = core::future::pending::<()>();
        let pinned_pending = Box::pin(pending_future);
        match with_async(pinned_pending) {
            Ok(_) => println!("   Unexpected success"),
            Err(e) => println!("   Expected error: {:?}", e),
        }
    }

    #[cfg(not(any(feature = "std",)))]
    {
        println!("   Skipping Box::pin examples (requires alloc feature)");

        // Binary std/no_std choice
        let ready_future2 = ReadyFuture {
            value: "Stack allocated result",
        };
        match with_async(ready_future2) {
            Ok(result) => println!("   Stack result: {}", result),
            Err(e) => println!("   Stack error: {:?}", e),
        }
    }

    println!("\n=== Example Complete ===");
    println!("Note: This simple executor only handles immediately ready futures.");
    println!("For real async execution, integrate with Embassy, tokio, or other runtimes.");
}

#[cfg(not(feature = "async-api"))]
fn main() {
    println!("This example requires the 'async-api' feature to be enabled.");
    println!("Try running with: cargo run --example pluggable_async_example --features async-api");
}
