//! Built-in interception for WebAssembly Component Model
//!
//! This module provides facilities for intercepting built-in function calls
//! in the WebAssembly Component Model implementation.

#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(feature = "std")]
use wrt_foundation::component_value::{
    ComponentValue,
    ValType,
};
#[cfg(feature = "std")]
use wrt_foundation::values::Value;
#[allow(unused_imports)] // Used in feature-gated code
use wrt_error::Error;

use crate::prelude::{
    str,
    BuiltinType,
    Debug,
};

/// Context for built-in interception
///
/// This struct provides context for built-in interception, including
/// information about the caller, the built-in being called, and any
/// additional context needed for making interception decisions.
#[derive(Debug, Clone)]
pub struct InterceptContext {
    /// The name of the component making the built-in call
    #[cfg(feature = "std")]
    pub component_name: String,
    /// The name of the component making the built-in call (static in `no_std`)
    #[cfg(not(feature = "std"))]
    pub component_name: &'static str,
    /// The built-in function being called
    pub builtin_type:   BuiltinType,
    /// The host environment's unique identifier
    #[cfg(feature = "std")]
    pub host_id:        String,
    /// The host environment's unique identifier (static in `no_std`)
    #[cfg(not(feature = "std"))]
    pub host_id:        &'static str,
    /// Additional context data (if any)
    #[cfg(feature = "std")]
    pub context_data:   std::collections::BTreeMap<String, Value>,
}

impl InterceptContext {
    /// Create a new interception context
    ///
    /// # Arguments
    ///
    /// * `component_name` - The name of the calling component
    /// * `builtin_type` - The built-in function being called
    /// * `host_id` - The host identifier
    ///
    /// # Returns
    ///
    /// A new `InterceptContext` instance
    #[must_use]
    pub fn new(component_name: &str, builtin_type: BuiltinType, host_id: &str) -> Self {
        #[cfg(not(feature = "std"))]
        let _ = (component_name, host_id);
        Self {
            #[cfg(feature = "std")]
            component_name: component_name.to_string(),
            #[cfg(not(feature = "std"))]
            component_name: "default",
            builtin_type,
            #[cfg(feature = "std")]
            host_id: host_id.to_string(),
            #[cfg(not(feature = "std"))]
            host_id: "default",
            #[cfg(feature = "std")]
            context_data: std::collections::BTreeMap::new(),
        }
    }

    /// Add context data (only available with `std` feature)
    #[cfg(feature = "std")]
    pub fn add_data(&mut self, key: &str, value: Value) {
        self.context_data.insert(key.to_string(), value);
    }

    /// Get context data (only available with `std` feature)
    #[cfg(feature = "std")]
    #[must_use] 
    pub fn get_data(&self, key: &str) -> Option<&Value> {
        self.context_data.get(key)
    }
}

/// Serialization helper for built-in arguments
///
/// This struct provides methods for serializing and deserializing
/// arguments and results for built-in function calls.
#[cfg(feature = "std")]
pub struct BuiltinSerialization;

#[cfg(feature = "std")]
impl BuiltinSerialization {
    /// Serialize component values to bytes
    ///
    /// # Arguments
    ///
    /// * `values` - The values to serialize
    ///
    /// # Returns
    ///
    /// A `Result` containing the serialized bytes or an error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Serialization of any value fails
    /// - Unsupported value type is encountered
    pub fn serialize(
        values: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
    ) -> wrt_error::Result<Vec<u8>> {
        // Simple implementation for now - convert to bytes
        let mut result = Vec::new();
        for value in values {
            let bytes = match value {
                ComponentValue::S32(v) => v.to_le_bytes().to_vec(),
                ComponentValue::S64(v) => v.to_le_bytes().to_vec(),
                ComponentValue::F32(v) => v.0.to_le_bytes().to_vec(),
                ComponentValue::F64(v) => v.0.to_le_bytes().to_vec(),
                _ => {
                    return Err(Error::runtime_execution_error(
                        "unsupported component value type for serialization",
                    ))
                },
            };
            result.extend(bytes);
        }
        Ok(result)
    }

    /// Deserialize bytes to component values
    ///
    /// # Arguments
    ///
    /// * `bytes` - The bytes to deserialize
    /// * `types` - The types of values to deserialize
    ///
    /// # Returns
    ///
    /// A `Result` containing the deserialized values or an error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Insufficient bytes available for deserialization
    /// - Invalid type encountered
    /// - Deserialization of any value fails
    pub fn deserialize(
        bytes: &[u8],
        types: &[ValType<wrt_foundation::NoStdProvider<64>>],
    ) -> wrt_error::Result<Vec<ComponentValue<wrt_foundation::NoStdProvider<64>>>> {
        let mut result = Vec::new();
        let mut offset = 0;

        for ty in types {
            match ty {
                ValType::S32 => {
                    if offset + 4 > bytes.len() {
                        return Err(Error::new(
                            wrt_error::ErrorCategory::Parse,
                            wrt_error::codes::PARSE_ERROR,
                            "insufficient bytes for S32 deserialization",
                        ));
                    }
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[offset..offset + 4]);
                    result.push(ComponentValue::S32(i32::from_le_bytes(buf)));
                    offset += 4;
                },
                ValType::S64 => {
                    if offset + 8 > bytes.len() {
                        return Err(Error::runtime_execution_error(
                            "insufficient bytes for S64 deserialization",
                        ));
                    }
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&bytes[offset..offset + 8]);
                    result.push(ComponentValue::S64(i64::from_le_bytes(buf)));
                    offset += 8;
                },
                ValType::F32 => {
                    if offset + 4 > bytes.len() {
                        return Err(Error::new(
                            wrt_error::ErrorCategory::Parse,
                            wrt_error::codes::PARSE_ERROR,
                            "insufficient bytes for F32 deserialization",
                        ));
                    }
                    let mut buf = [0u8; 4];
                    buf.copy_from_slice(&bytes[offset..offset + 4]);
                    result.push(ComponentValue::F32(wrt_foundation::FloatBits32(
                        f32::from_le_bytes(buf).to_bits(),
                    )));
                    offset += 4;
                },
                ValType::F64 => {
                    if offset + 8 > bytes.len() {
                        return Err(Error::runtime_execution_error(
                            "insufficient bytes for F64 deserialization",
                        ));
                    }
                    let mut buf = [0u8; 8];
                    buf.copy_from_slice(&bytes[offset..offset + 8]);
                    result.push(ComponentValue::F64(wrt_foundation::FloatBits64(
                        f64::from_le_bytes(buf).to_bits(),
                    )));
                    offset += 8;
                },
                _ => {
                    return Err(Error::new(
                        wrt_error::ErrorCategory::Type,
                        wrt_error::codes::INVALID_TYPE,
                        "unsupported value type for deserialization",
                    ))
                },
            }
        }

        Ok(result)
    }

    // NOTE: The following two functions (serialize_args,
    // supported_serialization_types) were erroneously added by a previous edit
    // and should be removed if they are not part of the original
    // DefaultBuiltinSerialization. For now, I am commenting them out to ensure
    // the build doesn't break, but they should be deleted if not needed.
    // fn serialize_args(
    // &self,
    // context: &InterceptContext,
    // args: &[ComponentValue],
    // types: &[ValType],
    // ) -> Result<Vec<u8>> {
    // let mut bytes = Vec::new();
    // for (index, value) in args.iter().enumerate() {
    // match types.get(index) {
    // Some(ty) => match ty {
    // ValType::S32 => {
    // if let Some(ComponentValue::S32(val)) = args.get(index) {
    // bytes.extend_from_slice(&val.to_le_bytes);
    // }
    // }
    // ValType::S64 => {
    // if let Some(ComponentValue::S64(val)) = args.get(index) {
    // bytes.extend_from_slice(&val.to_le_bytes);
    // }
    // }
    // ValType::F32 => {
    // if let Some(ComponentValue::F32(val)) = args.get(index) {
    // bytes.extend_from_slice(&val.to_le_bytes);
    // }
    // }
    // ValType::F64 => {
    // if let Some(ComponentValue::F64(val)) = args.get(index) {
    // bytes.extend_from_slice(&val.to_le_bytes);
    // }
    // }
    // _ => {
    // return Err(Error::runtime_execution_error("unsupported value type for
    // argument serialization" ))
    // }
    // },
    // None => {
    // return Err(Error::new(
    // wrt_error::ErrorCategory::Type,
    // wrt_error::codes::INVALID_TYPE,
    // "missing type information for argument"))
    // }
    // }
    // }
    // Ok(bytes)
    // }
    //
    // fn supported_serialization_types() -> Vec<ValType> {
    // Example: only basic numeric types are supported for now
    // vec![
    // ValType::S32,
    // ValType::S64,
    // ValType::F32,
    // ValType::F64,
    // ]
    // }
}

/// The `BuiltinInterceptor` trait defines methods for intercepting and
/// potentially modifying built-in function calls in the WebAssembly
/// Component Model implementation.
#[cfg(feature = "std")]
pub trait BuiltinInterceptor: Send + Sync {
    /// Called before a built-in function is invoked
    ///
    /// # Arguments
    ///
    /// * `context` - The interception context
    /// * `args` - The arguments to the built-in function
    ///
    /// # Returns
    ///
    /// A `Result` containing potentially modified arguments, or a complete
    /// result if the built-in execution should be bypassed
    ///
    /// # Errors
    ///
    /// Returns an error if interception processing fails
    fn before_builtin(
        &self,
        context: &InterceptContext,
        args: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
    ) -> wrt_error::Result<BeforeBuiltinResult>;

    /// Called after a built-in function has been invoked
    ///
    /// # Arguments
    ///
    /// * `context` - The interception context
    /// * `args` - The original arguments to the built-in function
    /// * `result` - The result of the built-in function call
    ///
    /// # Returns
    ///
    /// A `Result` containing potentially modified result values
    ///
    /// # Errors
    ///
    /// Returns an error if result modification fails
    fn after_builtin(
        &self,
        context: &InterceptContext,
        args: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
        result: wrt_error::Result<Vec<ComponentValue<wrt_foundation::NoStdProvider<64>>>>,
    ) -> wrt_error::Result<Vec<ComponentValue<wrt_foundation::NoStdProvider<64>>>>;

    /// Clone this interceptor
    ///
    /// # Returns
    ///
    /// A cloned version of this interceptor
    fn clone_interceptor(&self) -> Arc<dyn BuiltinInterceptor>;
}

/// Result of the `before_builtin` method
#[cfg(feature = "std")]
pub enum BeforeBuiltinResult {
    /// Continue with the built-in execution using the provided arguments
    Continue(Vec<ComponentValue<wrt_foundation::NoStdProvider<64>>>),
    /// Skip the built-in execution and use these values as the result
    Bypass(Vec<ComponentValue<wrt_foundation::NoStdProvider<64>>>),
}

/// Result of the `before_builtin` method (`no_std` version)
#[cfg(not(feature = "std"))]
pub enum BeforeBuiltinResult {
    /// Continue with the built-in execution
    Continue,
    /// Skip the built-in execution
    Bypass,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intercept_context() {
        #[cfg(feature = "std")]
        {
            let context =
                InterceptContext::new("test-component", BuiltinType::ResourceCreate, "test-host");

            assert_eq!(context.component_name, "test-component");
            assert_eq!(context.builtin_type, BuiltinType::ResourceCreate);
            assert_eq!(context.host_id, "test-host");

            let mut context = context;
            context.add_data("test-key", Value::I32(42));
            assert_eq!(context.get_data("test-key"), Some(&Value::I32(42)));
            assert_eq!(context.get_data("non-existent"), None);
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_builtin_serialization() {
        let values = vec![
            ComponentValue::S32(123),
            ComponentValue::S64(456),
            ComponentValue::F32(wrt_foundation::FloatBits32::from_float(1.23)),
            ComponentValue::F64(wrt_foundation::FloatBits64::from_float(4.56)),
        ];

        let serialized_bytes = BuiltinSerialization::serialize(&values).unwrap();

        let types = vec![ValType::S32, ValType::S64, ValType::F32, ValType::F64];

        let deserialized_values =
            BuiltinSerialization::deserialize(&serialized_bytes, &types).unwrap();

        assert_eq!(deserialized_values.len(), values.len());
        assert_eq!(deserialized_values[0], values[0]);
        assert_eq!(deserialized_values[1], values[1]);
        // For floating point, we need to handle potential rounding issues
        if let (ComponentValue::F32(a), ComponentValue::F32(b)) =
            (&deserialized_values[2], &values[2])
        {
            assert!((a.to_f32() - b.to_f32()).abs() < f32::EPSILON);
        } else {
            panic!("Expected F32 values");
        }
        if let (ComponentValue::F64(a), ComponentValue::F64(b)) =
            (&deserialized_values[3], &values[3])
        {
            assert!((a.to_f64() - b.to_f64()).abs() < f64::EPSILON);
        } else {
            panic!("Expected F64 values");
        }
    }
}
