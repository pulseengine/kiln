//! WASI CLI interface implementation
//!
//! Implements the `wasi:cli` interface for command line arguments and
//! environment variables using WRT's platform abstractions.

use core::any::Any;

// Import capability-aware functions
use crate::preview2::cli_capability_aware::{
    wasi_cli_get_arguments_bridge,
    wasi_cli_get_environment_bridge,
    wasi_get_initial_cwd_bridge,
};
use crate::{
    capabilities::WasiEnvironmentCapabilities,
    prelude::*,
    Value,
};

/// WASI get arguments operation
///
/// Implements `wasi:cli/environment.get-arguments` using capability-aware
/// allocation
///
/// # Errors
///
/// Returns an error if the capability-aware bridge function fails.
pub fn wasi_cli_get_arguments(target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    // Use capability-aware bridge function for memory safety
    wasi_cli_get_arguments_bridge(target, args)
}

/// WASI get environment operation
///
/// Implements `wasi:cli/environment.get-environment` using capability-aware
/// allocation
///
/// # Errors
///
/// Returns an error if the capability-aware bridge function fails.
pub fn wasi_cli_get_environment(target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    // Use capability-aware bridge function for memory safety
    wasi_cli_get_environment_bridge(target, args)
}

/// WASI get initial working directory operation
///
/// Implements `wasi:cli/environment.initial-cwd` using capability-aware
/// allocation
///
/// # Errors
///
/// Returns an error if the capability-aware bridge function fails.
pub fn wasi_get_initial_cwd(target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    // Use capability-aware bridge function for memory safety
    wasi_get_initial_cwd_bridge(target, args)
}

/// WASI get terminal stdin information
///
/// Implements `wasi:cli/terminal-stdin.get-terminal-stdin` for terminal
/// detection
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn wasi_get_terminal_stdin(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        use std::io::{
            self,
            IsTerminal,
        };

        // Check if stdin is a terminal
        let is_terminal = io::stdin().is_terminal();

        if is_terminal {
            // Return some terminal handle (simplified)
            Ok(vec![Value::Option(Some(Box::new(Value::U32(0))))])
        } else {
            Ok(vec![Value::Option(None)])
        }
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, typically no terminal
        Ok(vec![Value::Option(None)])
    }
}

/// WASI get terminal stdout information
///
/// Implements `wasi:cli/terminal-stdout.get-terminal-stdout` for terminal
/// detection
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn wasi_get_terminal_stdout(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        use std::io::{
            self,
            IsTerminal,
        };

        // Check if stdout is a terminal
        let is_terminal = io::stdout().is_terminal();

        if is_terminal {
            // Return some terminal handle (simplified)
            Ok(vec![Value::Option(Some(Box::new(Value::U32(1))))])
        } else {
            Ok(vec![Value::Option(None)])
        }
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, typically no terminal
        Ok(vec![Value::Option(None)])
    }
}

/// WASI get terminal stderr information
///
/// Implements `wasi:cli/terminal-stderr.get-terminal-stderr` for terminal
/// detection
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn wasi_get_terminal_stderr(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    #[cfg(feature = "std")]
    {
        use std::io::{
            self,
            IsTerminal,
        };

        // Check if stderr is a terminal
        let is_terminal = io::stderr().is_terminal();

        if is_terminal {
            // Return some terminal handle (simplified)
            Ok(vec![Value::Option(Some(Box::new(Value::U32(2))))])
        } else {
            Ok(vec![Value::Option(None)])
        }
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, typically no terminal
        Ok(vec![Value::Option(None)])
    }
}

/// Get filtered environment variables based on capabilities
///
/// Helper function to filter environment variables based on WASI capabilities
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn get_filtered_environment(
    capabilities: &WasiEnvironmentCapabilities,
) -> Result<Vec<(String, String)>> {
    let mut filtered_vars = Vec::with_capacity(0);

    if !capabilities.environ_access {
        return Ok(filtered_vars);
    }

    #[cfg(feature = "std")]
    {
        use std::env;

        for (key, value) in env::vars() {
            if capabilities.is_env_var_allowed(&key) {
                filtered_vars.push((key, value));
            }
        }
    }

    Ok(filtered_vars)
}

/// Get filtered command line arguments based on capabilities
///
/// Helper function to filter arguments based on WASI capabilities
///
/// # Errors
///
/// This function is infallible and always returns `Ok`.
pub fn get_filtered_arguments(capabilities: &WasiEnvironmentCapabilities) -> Result<Vec<String>> {
    if !capabilities.args_access {
        return Ok(Vec::with_capacity(0));
    }

    #[cfg(feature = "std")]
    {
        use std::env;
        Ok(env::args().collect())
    }

    #[cfg(not(feature = "std"))]
    {
        Ok(Vec::with_capacity(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasi_get_arguments() -> Result<()> {
        // Initialize memory system for testing
        let _ = wrt_foundation::memory_init::MemoryInitializer::initialize();

        let result = wasi_cli_get_arguments(&mut (), vec![])?;
        assert_eq!(result.len(), 1);

        // Should return a list
        if let Value::List(_args) = &result[0] {
            // Arguments should be a list of strings
            // In test environment, may be empty or contain test runner args
        } else {
            panic!("Expected list of arguments");
        }

        Ok(())
    }

    #[test]
    fn test_wasi_get_environment() -> Result<()> {
        // Initialize memory system for testing
        let _ = wrt_foundation::memory_init::MemoryInitializer::initialize();

        let result = wasi_cli_get_environment(&mut (), vec![])?;
        assert_eq!(result.len(), 1);

        // Should return a list of tuples
        if let Value::List(env_vars) = &result[0] {
            // Each environment variable should be a tuple of (key, value)
            for env_var in env_vars {
                if let Value::Tuple(tuple) = env_var {
                    assert_eq!(tuple.len(), 2);
                    // Both should be strings
                    assert!(matches!(tuple[0], Value::String(_)));
                    assert!(matches!(tuple[1], Value::String(_)));
                }
            }
        } else {
            panic!("Expected list of environment variables");
        }

        Ok(())
    }

    #[test]
    fn test_wasi_get_initial_cwd() -> Result<()> {
        // Initialize memory system for testing
        let _ = wrt_foundation::memory_init::MemoryInitializer::initialize();

        let result = wasi_get_initial_cwd(&mut (), vec![])?;
        assert_eq!(result.len(), 1);

        // Should return an option
        if let Value::Option(cwd_opt) = &result[0] {
            // May be Some(string) or None depending on environment
            if let Some(cwd_value) = cwd_opt {
                assert!(matches!(**cwd_value, Value::String(_)));
            }
        } else {
            panic!("Expected option for current working directory");
        }

        Ok(())
    }

    #[test]
    fn test_filtered_environment() -> Result<()> {
        let mut capabilities = WasiEnvironmentCapabilities::minimal()?;
        capabilities.environ_access = true;
        capabilities.add_allowed_var("PATH")?;

        let filtered = get_filtered_environment(&capabilities)?;

        // Should only contain PATH if it exists in environment
        for (key, _value) in &filtered {
            assert_eq!(key, "PATH");
        }

        Ok(())
    }

    #[test]
    fn test_filtered_arguments() -> Result<()> {
        let mut capabilities = WasiEnvironmentCapabilities::minimal()?;
        capabilities.args_access = true;

        let _filtered = get_filtered_arguments(&capabilities)?;

        // Should return arguments when access is allowed
        // In test environment, may contain test runner arguments

        // Test with access disabled
        capabilities.args_access = false;
        let no_args = get_filtered_arguments(&capabilities)?;
        assert!(no_args.is_empty());

        Ok(())
    }
}
