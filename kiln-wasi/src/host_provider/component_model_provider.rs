//! Component model provider for WASI integration
//!
//! This module provides the `ComponentModelProvider` that integrates WASI host
//! functions with the WRT component model using proven patterns from wrt-host
//! and wrt-component. Uses safety-aware allocation and respects configured
//! safety levels.

#[cfg(not(feature = "std"))]
use alloc::vec;
use core::any::Any;
// Import String type
#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec;

#[cfg(feature = "std")]
use wrt_format::component::ExternType;
// Use foundation Value type for compatibility with host functions
use wrt_host::HostFunctionHandler;

use crate::{
    capabilities::WasiCapabilities,
    host_provider::resource_manager::WasiResourceManager,
    prelude::*,
    wasi_safety_level,
    HostFunction,
};
#[cfg(not(feature = "std"))]
type WasiHostString = BoundedString<256, wrt_foundation::safe_memory::NoStdProvider<1024>>;

// Helper to create strings
#[cfg(not(feature = "std"))]
fn make_string(s: &str) -> Result<WasiHostString> {
    let provider = safe_managed_alloc!(1024, CrateId::Wasi)?;
    BoundedString::try_from_str(s, provider)
        .map_err(|_| Error::wasi_invalid_argument("Failed to create bounded string"))
}

/// Convert foundation Value types to `value_compat` Value types
#[cfg(feature = "std")]
#[allow(dead_code)]
fn convert_foundation_values_to_compat(args: FoundationValueVec) -> Result<Vec<crate::Value>> {
    let mut converted = Vec::new();
    for arg in args {
        match arg {
            wrt_foundation::values::Value::I32(v) => converted.push(crate::Value::S32(v)),
            wrt_foundation::values::Value::I64(v) => converted.push(crate::Value::S64(v)),
            wrt_foundation::values::Value::F32(v) => converted.push(crate::Value::F32(v.value())),
            wrt_foundation::values::Value::F64(v) => converted.push(crate::Value::F64(v.value())),
            _ => {
                return Err(Error::wasi_invalid_argument(
                    "Unsupported value type for conversion",
                ))
            },
        }
    }
    Ok(converted)
}

/// Convert `value_compat` Value types back to foundation Value types
#[cfg(feature = "std")]
#[allow(dead_code)]
fn convert_compat_values_to_foundation(values: Vec<crate::Value>) -> Result<FoundationValueVec> {
    let mut converted = Vec::new();
    for value in values {
        match value {
            crate::Value::Bool(v) => {
                converted.push(wrt_foundation::values::Value::I32(i32::from(v)));
            },
            crate::Value::U8(v) => converted.push(wrt_foundation::values::Value::I32(i32::from(v))),
            crate::Value::U16(v) => converted.push(wrt_foundation::values::Value::I32(i32::from(v))),
            crate::Value::U32(v) => converted.push(wrt_foundation::values::Value::I64(i64::from(v))),
            crate::Value::U64(v) => converted.push(wrt_foundation::values::Value::I64(v as i64)),
            crate::Value::S8(v) => converted.push(wrt_foundation::values::Value::I32(i32::from(v))),
            crate::Value::S16(v) => converted.push(wrt_foundation::values::Value::I32(i32::from(v))),
            crate::Value::S32(v) => converted.push(wrt_foundation::values::Value::I32(v)),
            crate::Value::S64(v) => converted.push(wrt_foundation::values::Value::I64(v)),
            crate::Value::F32(v) => converted.push(wrt_foundation::values::Value::F32(
                wrt_foundation::values::FloatBits32::from_float(v),
            )),
            crate::Value::F64(v) => converted.push(wrt_foundation::values::Value::F64(
                wrt_foundation::values::FloatBits64::from_float(v),
            )),
            crate::Value::List(_list) => {
                // For lists, we'll need to convert to an appropriate foundation type
                // This is a simplification - real implementation would need proper list support
                return Err(Error::wasi_invalid_argument(
                    "List values not yet supported in conversion",
                ));
            },
            _ => {
                return Err(Error::wasi_invalid_argument(
                    "Unsupported value type for conversion",
                ))
            },
        }
    }
    Ok(converted)
}

#[cfg(feature = "std")]
fn make_string(s: &str) -> String {
    s.to_string()
}

// Simple ExternType replacement for no_std mode
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, PartialEq)]
pub enum ExternType {
    Function {
        params: wrt_foundation::BoundedVec<
            ValType,
            16,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
        results: wrt_foundation::BoundedVec<
            ValType,
            16,
            wrt_foundation::safe_memory::NoStdProvider<1024>,
        >,
    },
}

// Simple ValType for no_std mode
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
}

#[cfg(not(feature = "std"))]
impl Default for ValType {
    fn default() -> Self {
        ValType::I32
    }
}

// Implement required traits for BoundedVec compatibility
#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::Checksummable for ValType {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update_slice(&[*self as u8]);
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::ToBytes for ValType {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

#[cfg(not(feature = "std"))]
impl wrt_foundation::traits::FromBytes for ValType {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        match reader.read_u8()? {
            0 => Ok(ValType::I32),
            1 => Ok(ValType::I64),
            2 => Ok(ValType::F32),
            3 => Ok(ValType::F64),
            _ => Err(wrt_error::Error::parse_error(
                "Invalid ValType discriminant",
            )),
        }
    }
}

/// Component model provider for WASI Preview2
///
/// This provider integrates WASI host functions with the WRT component model,
/// using the same patterns as other WRT host providers.
#[derive(Debug)]
pub struct ComponentModelProvider {
    /// WASI capabilities for this provider
    capabilities:      WasiCapabilities,
    /// Resource manager for WASI handles
    _resource_manager: WasiResourceManager,
    /// Cached host functions
    #[cfg(feature = "std")]
    cached_functions: Option<Vec<HostFunction>>,
    #[cfg(not(feature = "std"))]
    cached_functions: Option<u8>, // Simplified for no_std mode
}

// Helper types for no_std mode
// Remove type aliases and handle vectors dynamically

// Helper function to create empty value vector
#[cfg(not(feature = "std"))]
fn empty_value_vec() -> wrt_error::Result<impl Iterator<Item = Value>> {
    // Return empty iterator for no_std mode
    let empty_slice: &[Value] = &[];
    Ok(empty_slice.iter().cloned())
}

// Helper function to create empty type vector
#[cfg(not(feature = "std"))]
fn empty_type_vec() -> wrt_error::Result<impl Iterator<Item = ValType>> {
    // Return empty iterator for no_std mode
    let empty_slice: &[ValType] = &[];
    Ok(empty_slice.iter().cloned())
}

// Type alias for Value vectors from foundation
#[cfg(feature = "std")]
#[allow(dead_code)]
type FoundationValueVec = Vec<wrt_foundation::values::Value>;

// Simplified no_std types - just use placeholder
#[cfg(not(feature = "std"))]
type FoundationValueVec = ();

// Macro to create value vector in both std and no_std modes
macro_rules! value_vec {
    () => {{
        #[cfg(feature = "std")]
        {
            vec![]
        }
        #[cfg(not(feature = "std"))]
        {
            // Simplified for no_std - just return empty iterator result
            // This is a placeholder implementation
            return Err(Error::runtime_execution_error(
                "Value vectors not supported in no_std mode",
            ));
        }
    }};
}

// Macro to create type vector in both std and no_std modes
macro_rules! type_vec {
    () => {{
        #[cfg(feature = "std")]
        {
            vec![]
        }
        #[cfg(not(feature = "std"))]
        {
            // Simplified for no_std
            return Err(Error::runtime_execution_error(
                "Type vectors not supported in no_std mode",
            ));
        }
    }};
}

impl ComponentModelProvider {
    /// Create a new component model provider with the given capabilities
    /// Allocates function cache using safety-aware allocation
    ///
    /// # Errors
    ///
    /// Returns an error if the resource manager cannot be initialized.
    pub fn new(capabilities: WasiCapabilities) -> Result<Self> {
        let resource_manager = WasiResourceManager::new()?;

        // Initialize function cache (None = not built yet)
        #[cfg(feature = "std")]
        let cached_functions = None;
        #[cfg(not(feature = "std"))]
        let cached_functions = None;

        Ok(Self {
            capabilities,
            _resource_manager: resource_manager,
            cached_functions,
        })
    }

    /// Get the current safety level for this provider
    pub fn safety_level(&self) -> &'static str {
        wasi_safety_level()
    }

    /// Get the capabilities for this provider
    pub fn capabilities(&self) -> &WasiCapabilities {
        &self.capabilities
    }

    /// Register all WASI functions with a callback registry
    ///
    /// This follows the same pattern as other WRT host providers
    ///
    /// # Errors
    ///
    /// Returns an error if building host functions fails.
    pub fn register_with_registry(&mut self, registry: &mut CallbackRegistry) -> Result<()> {
        // Build functions if not already cached
        #[cfg(feature = "std")]
        {
            let functions = self.build_host_functions()?;

            for function in functions {
                let module_name = Self::extract_module_name(&function.name);
                registry.register_host_function(
                    module_name,
                    &function.name,
                    function.handler.clone(),
                );
            }
        }

        #[cfg(not(feature = "std"))]
        {
            // In no_std mode, we can't build dynamic functions
            let _count = self.build_host_functions()?;
            // TODO: Register static functions in no_std mode
        }

        Ok(())
    }

    /// Build all WASI host functions based on enabled capabilities
    #[cfg(feature = "std")]
    fn build_host_functions(&mut self) -> Result<&Vec<HostFunction>> {
        if self.cached_functions.is_some() {
            return Ok(self.cached_functions.as_ref().unwrap());
        }

        let mut functions = Vec::with_capacity(0);

        // Add filesystem functions if capabilities allow
        if self.capabilities.filesystem.read_access {
            functions.push(self.create_filesystem_read_function()?);
        }
        if self.capabilities.filesystem.write_access {
            functions.push(self.create_filesystem_write_function()?);
        }
        if self.capabilities.filesystem.directory_access {
            functions.push(self.create_filesystem_open_function()?);
        }

        // Add CLI functions if capabilities allow
        if self.capabilities.environment.args_access {
            functions.push(self.create_cli_args_function()?);
        }
        if self.capabilities.environment.environ_access {
            functions.push(self.create_cli_environ_function()?);
        }

        // Add clock functions if capabilities allow
        if self.capabilities.clocks.monotonic_access {
            functions.push(self.create_clock_now_function()?);
        }

        // Add I/O functions if capabilities allow
        if self.capabilities.io.stdout_access {
            functions.push(self.create_io_write_function()?);
        }

        // Add random functions if capabilities allow
        if self.capabilities.random.secure_random {
            functions.push(self.create_random_get_function()?);
        }

        // Add neural network functions if capabilities allow
        #[cfg(all(feature = "wasi-nn", feature = "nn-preview2", feature = "std"))]
        if self.capabilities.nn.dynamic_loading {
            functions.extend(self.create_nn_functions()?);
        }

        self.cached_functions = Some(functions);
        Ok(self.cached_functions.as_ref().unwrap())
    }

    /// Build all WASI host functions based on enabled capabilities (no_std
    /// version)
    #[cfg(not(feature = "std"))]
    fn build_host_functions(&mut self) -> Result<u8> {
        // In no_std mode, return a simple count of available functions
        let mut count = 0u8;

        if self.capabilities.filesystem.read_access {
            count += 1;
        }
        if self.capabilities.filesystem.write_access {
            count += 1;
        }
        if self.capabilities.filesystem.directory_access {
            count += 1;
        }
        if self.capabilities.environment.args_access {
            count += 1;
        }
        if self.capabilities.environment.environ_access {
            count += 1;
        }
        if self.capabilities.clocks.monotonic_access {
            count += 1;
        }
        if self.capabilities.io.stdout_access {
            count += 1;
        }
        if self.capabilities.random.secure_random {
            count += 1;
        }
        #[cfg(feature = "wasi-nn")]
        if self.capabilities.nn.dynamic_loading {
            count += 5;
        } // 5 NN functions

        self.cached_functions = Some(count);
        Ok(count)
    }

    /// Create host function for WASI filesystem read operations
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_filesystem_read_function(&self) -> Result<HostFunction> {
        // use crate::preview2::filesystem::wasi_filesystem_read;

        // Use capability-aware value conversion
        

        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:filesystem/types.read"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:filesystem/types.read")?,
            handler: HostFunctionHandler::new(move |_target: &mut dyn Any| {
                // Return capability-aware empty result
                // Return empty result vector
                #[cfg(feature = "std")]
                {
                    Ok(vec![])
                }
                #[cfg(not(feature = "std"))]
                {
                    Ok(value_vec!())
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI filesystem write operations
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_filesystem_write_function(&self) -> Result<HostFunction> {
        // use crate::preview2::filesystem::wasi_filesystem_write;

        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:filesystem/types.write"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:filesystem/types.write")?,
            handler: HostFunctionHandler::new(move |_target: &mut dyn Any| {
                #[cfg(feature = "std")]
                {
                    Ok(vec![])
                }
                #[cfg(not(feature = "std"))]
                {
                    Ok(value_vec!())
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI filesystem open operations
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_filesystem_open_function(&self) -> Result<HostFunction> {
        // use crate::preview2::filesystem::wasi_filesystem_open_at;

        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:filesystem/types.open-at"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:filesystem/types.open-at")?,
            handler: HostFunctionHandler::new(move |_target: &mut dyn Any| {
                #[cfg(feature = "std")]
                {
                    Ok(vec![])
                }
                #[cfg(not(feature = "std"))]
                {
                    Ok(value_vec!())
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI CLI arguments
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_cli_args_function(&self) -> Result<HostFunction> {
        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:cli/environment.get-arguments"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:cli/environment.get-arguments")?,
            handler: HostFunctionHandler::new(move |target: &mut dyn Any| {
                // Placeholder implementation for no_std
                #[cfg(feature = "wasi-cli")]
                {
                    use crate::preview2::cli_capability_aware::wasi_cli_get_arguments_capability_aware;
                    let empty_args = value_vec!();
                    match wasi_cli_get_arguments_capability_aware(target, empty_args) {
                        Ok(_) | Err(_) => Ok(value_vec!()),
                    }
                }
                #[cfg(not(feature = "wasi-cli"))]
                {
                    Ok(vec![])
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI CLI environment variables
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_cli_environ_function(&self) -> Result<HostFunction> {
        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:cli/environment.get-environment"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:cli/environment.get-environment")?,
            handler: HostFunctionHandler::new(move |target: &mut dyn Any| {
                // Placeholder implementation for no_std
                #[cfg(feature = "wasi-cli")]
                {
                    use crate::preview2::cli_capability_aware::wasi_cli_get_environment_capability_aware;
                    let empty_args = value_vec!();
                    match wasi_cli_get_environment_capability_aware(target, empty_args) {
                        Ok(_) | Err(_) => Ok(value_vec!()),
                    }
                }
                #[cfg(not(feature = "wasi-cli"))]
                {
                    Ok(vec![])
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI monotonic clock
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_clock_now_function(&self) -> Result<HostFunction> {
        // use crate::preview2::clocks::wasi_monotonic_clock_now;

        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:clocks/monotonic-clock.now"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:clocks/monotonic-clock.now")?,
            handler: HostFunctionHandler::new(move |_target: &mut dyn Any| {
                #[cfg(feature = "std")]
                {
                    Ok(vec![])
                }
                #[cfg(not(feature = "std"))]
                {
                    Ok(value_vec!())
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI I/O write
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_io_write_function(&self) -> Result<HostFunction> {
        // use crate::preview2::io::wasi_stream_write;

        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:io/streams.write"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:io/streams.write")?,
            handler: HostFunctionHandler::new(move |_target: &mut dyn Any| {
                #[cfg(feature = "std")]
                {
                    Ok(vec![])
                }
                #[cfg(not(feature = "std"))]
                {
                    Ok(value_vec!())
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host function for WASI random number generation
    // Result needed for no_std where make_string() can fail.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn create_random_get_function(&self) -> Result<HostFunction> {
        // use crate::preview2::random::wasi_get_random_bytes;

        Ok(HostFunction {
            #[cfg(feature = "std")]
            name: make_string("wasi:random/random.get-random-bytes"),
            #[cfg(not(feature = "std"))]
            name: make_string("wasi:random/random.get-random-bytes")?,
            handler: HostFunctionHandler::new(move |_target: &mut dyn Any| {
                #[cfg(feature = "std")]
                {
                    Ok(vec![])
                }
                #[cfg(not(feature = "std"))]
                {
                    Ok(value_vec!())
                }
            }),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        })
    }

    /// Create host functions for WASI neural network interface
    #[cfg(all(feature = "wasi-nn", feature = "nn-preview2", feature = "std"))]
    fn create_nn_functions(&self) -> Result<Vec<HostFunction>> {
        use crate::nn::sync_bridge::{
            wasi_nn_compute,
            wasi_nn_get_output,
            wasi_nn_init_execution_context,
            wasi_nn_load,
            wasi_nn_set_input,
        };

        let mut functions = Vec::new();

        // Load function
        functions.push(HostFunction {
            name:        make_string("wasi:nn/inference.load"),
            handler:     HostFunctionHandler::new_with_args(
                |target: &mut dyn Any, args: FoundationValueVec| {
                    // Convert wrt_foundation::Value to crate::Value
                    let converted_args = convert_foundation_values_to_compat(args)?;

                    // Call the actual function
                    let result = wasi_nn_load(target, converted_args)?;

                    // Convert back from crate::Value to wrt_foundation::Value
                    convert_compat_values_to_foundation(result)
                },
            ),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        });

        // Init execution context function
        functions.push(HostFunction {
            name:        make_string("wasi:nn/inference.init-execution-context"),
            handler:     HostFunctionHandler::new_with_args(
                |target: &mut dyn Any, args: FoundationValueVec| {
                    // Convert wrt_foundation::Value to crate::Value
                    let converted_args = convert_foundation_values_to_compat(args)?;

                    // Call the actual function
                    let result = wasi_nn_init_execution_context(target, converted_args)?;

                    // Convert back from crate::Value to wrt_foundation::Value
                    convert_compat_values_to_foundation(result)
                },
            ),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        });

        // Set input function
        functions.push(HostFunction {
            name:        make_string("wasi:nn/inference.set-input"),
            handler:     HostFunctionHandler::new_with_args(
                |target: &mut dyn Any, args: FoundationValueVec| {
                    // Convert wrt_foundation::Value to crate::Value
                    let converted_args = convert_foundation_values_to_compat(args)?;

                    // Call the actual function
                    let result = wasi_nn_set_input(target, converted_args)?;

                    // Convert back from crate::Value to wrt_foundation::Value
                    convert_compat_values_to_foundation(result)
                },
            ),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        });

        // Compute function
        functions.push(HostFunction {
            name:        make_string("wasi:nn/inference.compute"),
            handler:     HostFunctionHandler::new_with_args(
                |target: &mut dyn Any, args: FoundationValueVec| {
                    // Convert wrt_foundation::Value to crate::Value
                    let converted_args = convert_foundation_values_to_compat(args)?;

                    // Call the actual function
                    let result = wasi_nn_compute(target, converted_args)?;

                    // Convert back from crate::Value to wrt_foundation::Value
                    convert_compat_values_to_foundation(result)
                },
            ),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        });

        // Get output function
        functions.push(HostFunction {
            name:        make_string("wasi:nn/inference.get-output"),
            handler:     HostFunctionHandler::new_with_args(
                |target: &mut dyn Any, args: FoundationValueVec| {
                    // Convert wrt_foundation::Value to crate::Value
                    let converted_args = convert_foundation_values_to_compat(args)?;

                    // Call the actual function
                    let result = wasi_nn_get_output(target, converted_args)?;

                    // Convert back from crate::Value to wrt_foundation::Value
                    convert_compat_values_to_foundation(result)
                },
            ),
            extern_type: ExternType::Function {
                params:  type_vec!(),
                results: type_vec!(),
            },
        });

        Ok(functions)
    }

    /// Extract module name from full WASI function name
    fn extract_module_name(function_name: &str) -> &str {
        if let Some(colon_pos) = function_name.find(':') {
            if let Some(slash_pos) = function_name[colon_pos..].find('/') {
                return &function_name[..colon_pos + slash_pos];
            }
        }
        function_name
    }
}

impl WasiHostProvider for ComponentModelProvider {
    /// Get the number of functions provided
    fn function_count(&self) -> usize {
        #[cfg(feature = "std")]
        {
            if let Some(ref functions) = self.cached_functions {
                functions.len()
            } else {
                0
            }
        }
        #[cfg(not(feature = "std"))]
        {
            if let Some(count) = self.cached_functions {
                count as usize
            } else {
                0
            }
        }
    }

    /// Get the WASI version supported by this provider
    fn version(&self) -> WasiVersion {
        WasiVersion::Preview2
    }

    /// Get the capabilities enabled for this provider
    fn capabilities(&self) -> &WasiCapabilities {
        &self.capabilities
    }
}

/// Safety-aware WASI provider factory
pub struct WasiProviderBuilder {
    capabilities: Option<WasiCapabilities>,
    safety_level: Option<&'static str>,
}

impl WasiProviderBuilder {
    /// Create a new WASI provider builder
    #[must_use] 
    pub fn new() -> Self {
        Self {
            capabilities: None,
            safety_level: None,
        }
    }

    /// Set capabilities for the provider
    pub fn with_capabilities(mut self, capabilities: WasiCapabilities) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Set safety level for the provider (overrides capability-based detection)
    pub fn with_safety_level(mut self, level: &'static str) -> Self {
        self.safety_level = Some(level);
        self
    }

    /// Build the WASI provider with safety-aware defaults
    ///
    /// # Errors
    ///
    /// Returns an error if the capabilities cannot be created or the provider
    /// initialization fails.
    pub fn build(self) -> Result<ComponentModelProvider> {
        let capabilities = match self.capabilities {
            Some(caps) => caps,
            None => {
                // Choose default capabilities based on safety level
                match self.safety_level.unwrap_or(wasi_safety_level()) {
                    "maximum-safety" => WasiCapabilities::minimal()?,
                    "static-memory-safety" | "bounded-collections" => WasiCapabilities::sandboxed()?,
                    _ => WasiCapabilities::system_utility()?,
                }
            },
        };

        ComponentModelProvider::new(capabilities)
    }
}

impl Default for WasiProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() -> Result<()> {
        let capabilities = WasiCapabilities::minimal()?;
        let provider = ComponentModelProvider::new(capabilities)?;

        assert_eq!(provider.version(), WasiVersion::Preview2);
        assert!(!provider.capabilities().filesystem.read_access);

        Ok(())
    }

    #[test]
    fn test_extract_module_name() {
        assert_eq!(
            ComponentModelProvider::extract_module_name("wasi:filesystem/types.read"),
            "wasi:filesystem"
        );
        assert_eq!(
            ComponentModelProvider::extract_module_name("wasi:cli/environment.get-arguments"),
            "wasi:cli"
        );
    }

    #[test]
    fn test_safety_level_integration() -> Result<()> {
        let capabilities = WasiCapabilities::minimal()?;
        let provider = ComponentModelProvider::new(capabilities)?;

        // Should return current compile-time safety level
        let safety_level = provider.safety_level();
        assert!(!safety_level.is_empty());

        Ok(())
    }

    #[test]
    fn test_provider_builder() -> Result<()> {
        let provider =
            WasiProviderBuilder::new().with_safety_level("bounded-collections").build()?;

        assert_eq!(provider.version(), WasiVersion::Preview2);

        Ok(())
    }
}
