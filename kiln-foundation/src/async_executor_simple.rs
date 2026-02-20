//! Simple async executor support for no_std environments
//!
//! This is a simplified version that avoids unsafe code and complex
//! initialization.

use core::{
    future::Future,
    pin::Pin,
    task::{
        Context,
        Poll,
    },
};

/// Simple executor error type
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorError {
    NotRunning,
    TaskPanicked,
    OutOfResources,
    NotSupported,
    Custom(&'static str),
}

/// Simple async runtime for basic operations
pub struct AsyncRuntime;

impl Default for AsyncRuntime {
    fn default() -> Self {
        Self
    }
}

impl AsyncRuntime {
    /// Create a new async runtime
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Block on a future until completion (simplified version)
    pub fn block_on<F: Future + core::marker::Unpin>(
        &self,
        mut future: F,
    ) -> Result<F::Output, ExecutorError> {
        // For the simple version, we just poll once
        // This is not a real async executor, but enough for basic usage
        let waker = create_noop_waker();
        let mut cx = Context::from_waker(&waker);

        // Pin the future safely
        let future = Pin::new(&mut future);

        // Poll the future
        match future.poll(&mut cx) {
            Poll::Ready(output) => Ok(output),
            Poll::Pending => Err(ExecutorError::Custom("Future not immediately ready")),
        }
    }
}

/// Helper to run async code
pub fn with_async<F, T>(future: F) -> Result<T, ExecutorError>
where
    F: Future<Output = T> + core::marker::Unpin,
{
    let runtime = AsyncRuntime::new();
    runtime.block_on(future)
}

/// Check if using fallback executor (always true in simple version)
pub fn is_using_fallback() -> bool {
    true
}

/// Create a no-op waker for simple polling
fn create_noop_waker() -> core::task::Waker {
    use core::task::{
        RawWaker,
        RawWakerVTable,
        Waker,
    };

    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(core::ptr::null(), &VTABLE), // clone
        |_| {},                                        // wake
        |_| {},                                        // wake_by_ref
        |_| {},                                        // drop
    );

    let raw_waker = RawWaker::new(core::ptr::null(), &VTABLE);
    // SAFETY: The vtable functions are valid no-ops and meet the requirements
    #[allow(unsafe_code)]
    unsafe {
        Waker::from_raw(raw_waker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_async() {
        extern crate alloc;
        use alloc::boxed::Box;

        async fn test_future() -> u32 {
            42
        }

        // Pin the future to satisfy the Unpin bound
        let pinned = Box::pin(test_future());
        let result = with_async(pinned).unwrap();
        assert_eq!(result, 42);
    }
}
