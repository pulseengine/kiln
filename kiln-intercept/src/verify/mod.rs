//! Verification module for the Kani model checker
//!
//! This module contains verification functions and proofs
//! for the Kani model checker to verify the correctness of
//! the interceptor implementation.

// Only compile Kani verification code when documentation is being generated
// or when explicitly running cargo kani. This prevents interference with
// coverage testing.
#[cfg(all(feature = "kani", feature = "std"))]
pub mod proofs {
    use std::{
        sync::Arc,
        vec::Vec,
    };

    use wrt_error::Result;
    use wrt_foundation::values::Value;

    use crate::{
        LinkInterceptor,
        LinkInterceptorStrategy,
    };

    /// A simple strategy for verification
    struct TestStrategy {
        modify_args: bool,
    }

    impl LinkInterceptorStrategy for TestStrategy {
        fn before_call(
            &self,
            _source: &str,
            _target: &str,
            _function: &str,
            args: &[Value],
        ) -> Result<Vec<Value>> {
            if self.modify_args {
                Ok(vec![Value::I32(42)])
            } else {
                Ok(args.to_vec())
            }
        }

        fn after_call(
            &self,
            _source: &str,
            _target: &str,
            _function: &str,
            _args: &[Value],
            result: Result<Vec<Value>>,
        ) -> Result<Vec<Value>> {
            result
        }

        fn should_bypass(&self) -> bool {
            false
        }

        fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
            Arc::new(Self {
                modify_args: self.modify_args,
            })
        }
    }

    /// Verify that the interceptor properly modifies arguments
    #[cfg_attr(feature = "kani", kani::proof)]
    pub fn verify_interceptor_modifies_args() {
        let strategy = Arc::new(TestStrategy { modify_args: true });
        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let args = vec![Value::I32(10)];
        let result = interceptor.intercept_call("target", "func", args.clone(), |modified_args| {
            // The strategy should have modified the args
            assert!(modified_args.len() == 1);
            assert!(matches!(modified_args[0], Value::I32(42)));
            Ok(vec![Value::I64(20)])
        });

        assert!(result.is_ok());
    }

    /// Verify that the interceptor passes through arguments when not modified
    #[cfg_attr(feature = "kani", kani::proof)]
    pub fn verify_interceptor_passthrough() {
        let strategy = Arc::new(TestStrategy { modify_args: false });
        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let args = vec![Value::I32(10)];
        let result = interceptor.intercept_call("target", "func", args.clone(), |modified_args| {
            // The strategy should not have modified the args
            assert!(modified_args.len() == args.len());
            assert!(matches!(modified_args[0], Value::I32(10)));
            Ok(vec![Value::I64(20)])
        });

        assert!(result.is_ok());
    }

    /// Verify that multiple strategies are applied in order
    #[cfg_attr(feature = "kani", kani::proof)]
    pub fn verify_multiple_strategies() {
        let strategy1 = Arc::new(TestStrategy { modify_args: true });
        let strategy2 = Arc::new(TestStrategy { modify_args: false });

        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy1);
        interceptor.add_strategy(strategy2);

        let args = vec![Value::I32(10)];
        let result = interceptor.intercept_call("target", "func", args.clone(), |modified_args| {
            // The first strategy should have modified the args
            // The second strategy should have passed them through
            assert!(modified_args.len() == 1);
            assert!(matches!(modified_args[0], Value::I32(42)));
            Ok(vec![Value::I64(20)])
        });

        assert!(result.is_ok());
    }

    /// Verify that the interceptor passes errors through
    #[cfg_attr(feature = "kani", kani::proof)]
    pub fn verify_error_passthrough() {
        let strategy = Arc::new(TestStrategy { modify_args: false });
        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let args = vec![Value::I32(10)];
        let result = interceptor.intercept_call("target", "func", args.clone(), |_| {
            Err(wrt_error::Error::runtime_execution_error("Test error"))
        });

        assert!(result.is_err());
    }
}

// Expose the verification module in docs but not for normal compilation
#[cfg(all(feature = "kani", feature = "std"))]
pub use proofs::*;
