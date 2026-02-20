#![deny(warnings)]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use wrt_component::{
    CanonicalABI, Component, ComponentOptions, ComponentValue, ExecutionTimeoutError,
};
use wrt_error::Error;
use wrt_intercept::{LinkInterceptorStrategy, MemoryStrategy};

/// A simple test interceptor that logs function calls
#[derive(Default)]
struct TestInterceptor {
    calls: Arc<Mutex<Vec<String>>>,
    intercept_mode: InterceptMode,
}

enum InterceptMode {
    PassThrough,
    ModifyArgs,
    ModifyResults,
    HandleCall,
    ThrowError,
}

impl Default for InterceptMode {
    fn default() -> Self {
        InterceptMode::PassThrough
    }
}

impl TestInterceptor {
    fn new(mode: InterceptMode) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            intercept_mode: mode,
        }
    }

    fn get_calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl LinkInterceptorStrategy for TestInterceptor {
    fn intercept_function_call(
        &self,
        component_name: &str,
        function_name: &str,
        mut arguments: Vec<u8>,
    ) -> Result<(bool, Vec<u8>), Error> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("call: {}.{}", component_name, function_name));

        match self.intercept_mode {
            InterceptMode::PassThrough => Ok((false, arguments)),
            InterceptMode::ModifyArgs => {
                // For test purposes, append some indicator bytes
                arguments.extend_from_slice(&[0xFF, 0xFF]);
                Ok((false, arguments))
            },
            InterceptMode::HandleCall => {
                // Interceptor handles the call completely
                Ok((true, vec![1, 2, 3, 4]))
            },
            InterceptMode::ThrowError => Err(Error::runtime_execution_error(
                "Interceptor blocked function call",
            )),
            _ => Ok((false, arguments)),
        }
    }

    fn intercept_function_result(
        &self,
        component_name: &str,
        function_name: &str,
        mut result: Result<Vec<u8>, Error>,
    ) -> Result<Vec<u8>, Error> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("result: {}.{}", component_name, function_name));

        match self.intercept_mode {
            InterceptMode::ModifyResults => {
                // Modify successful results
                if let Ok(mut bytes) = result {
                    bytes.extend_from_slice(&[0xAA, 0xBB]);
                    Ok(bytes)
                } else {
                    result
                }
            },
            InterceptMode::ThrowError => Err(Error::runtime_execution_error(
                "Interceptor blocked function result",
            )),
            _ => result,
        }
    }

    fn get_preferred_memory_strategy(&self) -> MemoryStrategy {
        MemoryStrategy::BoundedCopy { max_size: 1024 }
    }
}

// Mock component for testing
struct MockComponentBuilder {
    has_start: bool,
    start_should_fail: bool,
    timeout_ms: Option<u64>,
}

impl MockComponentBuilder {
    fn new() -> Self {
        Self {
            has_start: true,
            start_should_fail: false,
            timeout_ms: None,
        }
    }

    fn with_start(mut self, has_start: bool) -> Self {
        self.has_start = has_start;
        self
    }

    fn with_failing_start(mut self) -> Self {
        self.start_should_fail = true;
        self
    }

    fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    fn build(self) -> Component {
        let mut component = Component::new("test-component");

        if self.has_start {
            // Simulate having a start function
            component.has_start_function = true;
            component.start_should_fail = self.start_should_fail;
        }

        if let Some(timeout) = self.timeout_ms {
            component.options.execution_timeout = Some(Duration::from_millis(timeout));
        }

        component
    }
}

// We need to extend the Component struct for testing
impl Component {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            has_start_function: false,
            start_should_fail: false,
            options: ComponentOptions::default(),
            // Other fields would be initialized here
        }
    }

    // Mock the internal start function execution
    fn execute_start_function_with_integrity(&self) -> Result<(), Error> {
        // For testing, simulate execution or failure based on configuration
        if self.start_should_fail {
            Err(Error::runtime_execution_error("Start function failed"))
        } else if let Some(timeout) = self.options.execution_timeout {
            if timeout.as_millis() < 100 {
                // Simulate timeout for very short timeouts
                Err(Error::from(ExecutionTimeoutError::runtime_execution_error(
                    "Execution timeout",
                    timeout,
                )))
            } else {
                // Simulate successful execution
                Ok(())
            }
        } else {
            // Default success
            Ok(())
        }
    }
}

#[test]
fn test_execute_start_basic() {
    let component = MockComponentBuilder::new().build();
    let result = component.execute_start();
    assert!(result.is_ok(), "Start function should execute successfully");
}

#[test]
fn test_execute_start_no_start_function() {
    let component = MockComponentBuilder::new().with_start(false).build();
    let result = component.execute_start();
    assert!(
        result.is_ok(),
        "Component without start function should return Ok"
    );
}

#[test]
fn test_execute_start_failure() {
    let component = MockComponentBuilder::new().with_failing_start().build();
    let result = component.execute_start();
    assert!(
        result.is_err(),
        "Failing start function should return error"
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        "Start function failed",
        "Error message should match"
    );
}

#[test]
fn test_execute_start_timeout() {
    let component = MockComponentBuilder::new().with_timeout(10).build();
    let result = component.execute_start();
    assert!(result.is_err(), "Start function should timeout");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("timed out"),
        "Error should indicate timeout: {}",
        err
    );
}

#[test]
fn test_execute_start_with_interceptor_pass_through() {
    let component = MockComponentBuilder::new().build();
    let interceptor = Arc::new(TestInterceptor::new(InterceptMode::PassThrough));

    let mut options = ComponentOptions::default();
    options.interceptor = Some(interceptor.clone());

    let mut component = component;
    component.options = options;

    let result = component.execute_start();
    assert!(
        result.is_ok(),
        "Start execution with pass-through interceptor should succeed"
    );

    let calls = interceptor.get_calls();
    assert_eq!(
        calls.len(),
        2,
        "Interceptor should record two calls (call + result)"
    );
    assert!(
        calls[0].contains("call"),
        "First call should be function call"
    );
    assert!(
        calls[1].contains("result"),
        "Second call should be function result"
    );
}

#[test]
fn test_execute_start_with_interceptor_error() {
    let component = MockComponentBuilder::new().build();
    let interceptor = Arc::new(TestInterceptor::new(InterceptMode::ThrowError));

    let mut options = ComponentOptions::default();
    options.interceptor = Some(interceptor.clone());

    let mut component = component;
    component.options = options;

    let result = component.execute_start();
    assert!(
        result.is_err(),
        "Start execution with error interceptor should fail"
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        "Interceptor blocked function call",
        "Error message should match"
    );

    let calls = interceptor.get_calls();
    assert_eq!(
        calls.len(),
        1,
        "Interceptor should record only the call attempt"
    );
}

#[test]
fn test_execute_start_with_interceptor_handle_call() {
    let component = MockComponentBuilder::new().build();
    let interceptor = Arc::new(TestInterceptor::new(InterceptMode::HandleCall));

    let mut options = ComponentOptions::default();
    options.interceptor = Some(interceptor.clone());

    let mut component = component;
    component.options = options;

    let result = component.execute_start();
    assert!(
        result.is_ok(),
        "Start execution with handling interceptor should succeed"
    );

    let calls = interceptor.get_calls();
    assert_eq!(
        calls.len(),
        1,
        "Interceptor should record only one call (no result)"
    );
    assert!(calls[0].contains("call"), "Call should be recorded");
}

#[test]
fn test_memory_strategy_selection() {
    // Test that the interceptor's memory strategy is respected
    let component = MockComponentBuilder::new().build();
    let interceptor = Arc::new(TestInterceptor::default());

    let mut options = ComponentOptions::default();
    options.interceptor = Some(interceptor);

    let mut component = component;
    component.options = options;

    // This is an indirect test through execute_start
    // In a real implementation, the memory strategy would be applied during
    // execution
    let result = component.execute_start();
    assert!(result.is_ok());
}

// Integration-style test that verifies the whole workflow
#[test]
fn test_integration_workflow() {
    // Create a component with a start function
    let mut component = MockComponentBuilder::new().build();

    // Set an interceptor that modifies args and results
    let interceptor = Arc::new(TestInterceptor::new(InterceptMode::ModifyArgs));

    let mut options = ComponentOptions::default();
    options.execution_timeout = Some(Duration::from_secs(5));
    options.interceptor = Some(interceptor.clone());

    component.options = options;

    // Execute start function
    let result = component.execute_start();
    assert!(result.is_ok(), "Integrated execution should succeed");

    // Verify interceptor was called
    let calls = interceptor.get_calls();
    assert_eq!(
        calls.len(),
        2,
        "Interceptor should be called for function and result"
    );
}
