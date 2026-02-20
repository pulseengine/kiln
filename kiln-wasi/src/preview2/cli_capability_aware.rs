//! Capability-aware WASI CLI interface implementation
//!
//! Implements the `wasi:cli` interface using capability-based memory allocation
//! for ASIL compliance.

extern crate alloc;

use core::any::Any;

use wrt_foundation::{
    budget_aware_provider::CrateId,
    capabilities::MemoryOperation,
    memory_init::get_global_capability_context,
};

use crate::{
    prelude::*,
    value_capability_aware::{
        CapabilityAwareValue,
        WasiValueBox,
    },
    Value,
};

/// Capability-aware WASI get arguments operation
///
/// Implements `wasi:cli/environment.get-arguments` using capability-based
/// allocation
///
/// # Errors
///
/// Returns an error if:
/// - The global capability context cannot be acquired
/// - The allocation capability verification fails
/// - Value conversion fails during list creation
pub fn wasi_cli_get_arguments_capability_aware(
    _target: &mut dyn Any,
    _args: Vec<CapabilityAwareValue>,
) -> Result<Vec<CapabilityAwareValue>> {
    // Verify we have allocation capability
    let context = get_global_capability_context()?;
    let operation = MemoryOperation::Allocate { size: 1024 }; // Reasonable estimate for args
    context.verify_operation(CrateId::Wasi, &operation)?;

    // Get command line arguments using platform abstraction
    #[cfg(feature = "std")]
    {
        use std::env;

        // Create capability-aware argument list
        let mut wasi_args = alloc::vec::Vec::new();

        for arg in env::args() {
            let arg_value = CapabilityAwareValue::string_from_str(&arg)?;
            wasi_args.push(arg_value);
        }

        // Create capability-aware list
        let args_list = CapabilityAwareValue::list_from_vec(wasi_args)?;
        Ok(vec![args_list])
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, return empty args
        let empty_args = CapabilityAwareValue::list_from_vec(vec![])?;
        Ok(vec![empty_args])
    }
}

/// Capability-aware WASI get environment operation
///
/// Implements `wasi:cli/environment.get-environment` using capability-based
/// allocation
///
/// # Errors
///
/// Returns an error if:
/// - The global capability context cannot be acquired
/// - The allocation capability verification fails
/// - Value conversion fails during list creation
pub fn wasi_cli_get_environment_capability_aware(
    _target: &mut dyn Any,
    _args: Vec<CapabilityAwareValue>,
) -> Result<Vec<CapabilityAwareValue>> {
    // Verify we have allocation capability
    let context = get_global_capability_context()?;
    let operation = MemoryOperation::Allocate { size: 2048 }; // Reasonable estimate for env vars
    context.verify_operation(CrateId::Wasi, &operation)?;

    // Get environment variables using platform abstraction
    #[cfg(feature = "std")]
    {
        use std::env;

        let mut env_vars = alloc::vec::Vec::new();

        // Iterate through environment variables
        for (key, value) in env::vars() {
            // Create capability-aware tuple of (key, value)
            let key_value = CapabilityAwareValue::string_from_str(&key)?;
            let value_value = CapabilityAwareValue::string_from_str(&value)?;

            let env_tuple = CapabilityAwareValue::tuple_from_vec(vec![key_value, value_value])?;

            env_vars.push(env_tuple);
        }

        let env_list = CapabilityAwareValue::list_from_vec(env_vars)?;
        Ok(vec![env_list])
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, return empty environment
        let empty_env = CapabilityAwareValue::list_from_vec(vec![])?;
        Ok(vec![empty_env])
    }
}

/// Capability-aware WASI get initial working directory operation
///
/// Implements `wasi:cli/environment.initial-cwd` using capability-based
/// allocation
///
/// # Errors
///
/// Returns an error if:
/// - The global capability context cannot be acquired
/// - The allocation capability verification fails
/// - Value conversion fails during option creation
pub fn wasi_get_initial_cwd_capability_aware(
    _target: &mut dyn Any,
    _args: Vec<CapabilityAwareValue>,
) -> Result<Vec<CapabilityAwareValue>> {
    // Verify we have allocation capability
    let context = get_global_capability_context()?;
    let operation = MemoryOperation::Allocate { size: 512 }; // Reasonable estimate for path
    context.verify_operation(CrateId::Wasi, &operation)?;

    #[cfg(feature = "std")]
    {
        use std::env;

        if let Ok(cwd) = env::current_dir() {
            let cwd_string = cwd.to_string_lossy();
            let cwd_value = CapabilityAwareValue::string_from_str(&cwd_string)?;
            let cwd_boxed = WasiValueBox::new(cwd_value)?;
            let cwd_option =
                CapabilityAwareValue::option_from_value(Some(cwd_boxed.into_inner()))?;
            Ok(vec![cwd_option])
        } else {
            // Return None if current directory cannot be determined
            let none_option = CapabilityAwareValue::option_from_value(None)?;
            Ok(vec![none_option])
        }
    }

    #[cfg(not(feature = "std"))]
    {
        // In no_std environment, return None for current directory
        let none_option = CapabilityAwareValue::option_from_value(None)?;
        Ok(vec![none_option])
    }
}

/// Convert `CapabilityAwareValue` back to legacy Value for bridge functions
///
/// This implements basic conversion for the value types used by CLI functions.
/// Complex nested types may lose some capability information.
fn convert_capability_value_to_legacy(value: CapabilityAwareValue) -> Result<Value> {
    match value {
        CapabilityAwareValue::Bool(b) => Ok(Value::Bool(b)),
        CapabilityAwareValue::U8(v) => Ok(Value::U8(v)),
        CapabilityAwareValue::U16(v) => Ok(Value::U16(v)),
        CapabilityAwareValue::U32(v) => Ok(Value::U32(v)),
        CapabilityAwareValue::U64(v) => Ok(Value::U64(v)),
        CapabilityAwareValue::S8(v) => Ok(Value::S8(v)),
        CapabilityAwareValue::S16(v) => Ok(Value::S16(v)),
        CapabilityAwareValue::S32(v) => Ok(Value::S32(v)),
        CapabilityAwareValue::S64(v) => Ok(Value::S64(v)),
        CapabilityAwareValue::F32(v) => Ok(Value::F32(v)),
        CapabilityAwareValue::F64(v) => Ok(Value::F64(v)),
        CapabilityAwareValue::Char(c) => Ok(Value::String(c.to_string())),
        CapabilityAwareValue::String(bounded_str) => {
            let str_result = bounded_str.as_str();
            match str_result {
                Ok(s) => Ok(Value::String(s.to_string())),
                Err(_) => Ok(Value::String(String::new())), // Fallback for invalid string
            }
        },
        CapabilityAwareValue::List(bounded_vec) => {
            let mut legacy_list = Vec::new();
            for item in bounded_vec.iter() {
                legacy_list.push(convert_capability_value_to_legacy(item.clone())?);
            }
            Ok(Value::List(legacy_list))
        },
        CapabilityAwareValue::Tuple(bounded_vec) => {
            let mut legacy_tuple = Vec::new();
            for item in bounded_vec.iter() {
                legacy_tuple.push(convert_capability_value_to_legacy(item.clone())?);
            }
            Ok(Value::Tuple(legacy_tuple))
        },
        CapabilityAwareValue::Option(opt) => match opt {
            Some(boxed_value) => {
                let converted = convert_capability_value_to_legacy(boxed_value.into_inner())?;
                Ok(Value::Option(Some(Box::new(converted))))
            },
            None => Ok(Value::Option(None)),
        },
        CapabilityAwareValue::Record(bounded_vec) => {
            let mut legacy_record = Vec::new();
            for (key, item) in bounded_vec.iter() {
                let converted_value = convert_capability_value_to_legacy(item.clone())?;
                let key_str = match key.as_str() {
                    Ok(s) => s.to_string(),
                    Err(_) => String::new(), // Fallback for invalid key
                };
                legacy_record.push((key_str, converted_value));
            }
            Ok(Value::Record(legacy_record))
        },
        // For complex types that can't be easily converted, return a placeholder
        _ => Ok(Value::U32(0)), // Fallback for unsupported types
    }
}

/// Bridge function to convert legacy CLI functions to capability-aware versions
///
/// # Errors
///
/// Returns an error if value conversion or the underlying capability-aware
/// function fails.
pub fn wasi_cli_get_arguments_bridge(target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    // Convert legacy values to capability-aware values
    let mut capability_args = alloc::vec::Vec::new();
    for arg in args {
        capability_args.push(arg.try_into()?);
    }

    // Call capability-aware function
    let result = wasi_cli_get_arguments_capability_aware(target, capability_args)?;

    // Convert back to legacy values for compatibility
    // Note: This is a temporary bridge - eventually all code should use
    // CapabilityAwareValue
    let mut legacy_result = alloc::vec::Vec::new();
    for value in result {
        let converted = convert_capability_value_to_legacy(value)?;
        legacy_result.push(converted);
    }

    Ok(legacy_result)
}

/// Bridge function for environment variables
///
/// # Errors
///
/// Returns an error if value conversion or the underlying capability-aware
/// function fails.
pub fn wasi_cli_get_environment_bridge(
    target: &mut dyn Any,
    args: Vec<Value>,
) -> Result<Vec<Value>> {
    // Convert to capability-aware and back
    let mut capability_args = alloc::vec::Vec::new();
    for arg in args {
        capability_args.push(arg.try_into()?);
    }

    let result = wasi_cli_get_environment_capability_aware(target, capability_args)?;

    // Convert back to legacy values
    let mut legacy_result = alloc::vec::Vec::new();
    for value in result {
        let converted = convert_capability_value_to_legacy(value)?;
        legacy_result.push(converted);
    }

    Ok(legacy_result)
}

/// Bridge function for current working directory
///
/// # Errors
///
/// Returns an error if value conversion or the underlying capability-aware
/// function fails.
pub fn wasi_get_initial_cwd_bridge(target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    // Convert to capability-aware and back
    let mut capability_args = alloc::vec::Vec::new();
    for arg in args {
        capability_args.push(arg.try_into()?);
    }

    let result = wasi_get_initial_cwd_capability_aware(target, capability_args)?;

    // Convert back to legacy values
    let mut legacy_result = alloc::vec::Vec::new();
    for value in result {
        let converted = convert_capability_value_to_legacy(value)?;
        legacy_result.push(converted);
    }

    Ok(legacy_result)
}

#[cfg(test)]
mod tests {
    use wrt_foundation::memory_init::MemoryInitializer;

    use super::*;

    #[test]
    fn test_capability_aware_cli_functions() {
        // Initialize memory system
        let _ = MemoryInitializer::initialize();

        // Test arguments function
        let mut dummy_target = ();
        let empty_args = vec![];

        let result = wasi_cli_get_arguments_capability_aware(&mut dummy_target, empty_args);
        assert!(result.is_ok(), "Arguments function should succeed");

        // Test environment function
        let empty_args = vec![];
        let result = wasi_cli_get_environment_capability_aware(&mut dummy_target, empty_args);
        assert!(result.is_ok(), "Environment function should succeed");

        // Test current directory function
        let empty_args = vec![];
        let result = wasi_get_initial_cwd_capability_aware(&mut dummy_target, empty_args);
        assert!(result.is_ok(), "Current directory function should succeed");
    }
}
