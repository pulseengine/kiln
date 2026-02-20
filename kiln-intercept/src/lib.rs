// WRT - wrt-intercept
// Module: Function Call Interception Layer
// SW-REQ-ID: REQ_017
// SW-REQ-ID: REQ_VERIFY_005
//
// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! # Interception Layer for WebAssembly Component Linking
//!
//! This crate provides a flexible interception mechanism for WebAssembly
//! component linking in the WebAssembly Runtime (WRT). It allows
//! intercepting function calls between components and between components
//! and the host.
//!
//! ## Overview
//!
//! The interception layer works by providing hooks that can be inserted
//! at function call boundaries. This enables various use cases:
//!
//! - Security monitoring and firewalls
//! - Performance optimization with shared-nothing or shared-everything
//!   strategies
//! - Logging and telemetry
//! - Debugging and tracing
//!
//! ## Creating Custom Interceptors
//!
//! To create a custom interceptor:
//!
//! 1. Implement the `LinkInterceptorStrategy` trait
//! 2. Add your strategy to a `LinkInterceptor` instance
//! 3. Attach the interceptor to components or host environments
//!
//! See the `strategies` module for examples of built-in interceptor strategies.
//!
//! ```rust,no_run
//! # use std::sync::Arc;
//! # use wrt_intercept::{LinkInterceptor, LinkInterceptorStrategy};
//! # use wrt_error::Result;
//! # use wrt_foundation::values::Value;
//! # struct MyStrategy;
//! # impl LinkInterceptorStrategy for MyStrategy {
//! #     fn before_call(&self, _source: &str, _target: &str, _function: &str, args: &[Value]) -> Result<Vec<Value>> {
//! #         Ok(args.to_vec())
//! #     }
//! #     fn after_call(&self, _source: &str, _target: &str, _function: &str, _args: &[Value], result: Result<Vec<Value>>) -> Result<Vec<Value>> {
//! #         result
//! #     }
//! #     fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
//! #         Arc::new(MyStrategy)
//! #     }
//! # }
//!
//! // Create your custom strategy
//! let my_strategy = Arc::new(MyStrategy;
//!
//! // Create an interceptor and add your strategy
//! let mut interceptor = LinkInterceptor::new("my_interceptor";
//! interceptor.add_strategy(my_strategy;
//!
//! // Attach to component or host
//! // component.with_interceptor(Arc::new(interceptor;
//! ```

#![forbid(unsafe_code)] // Rule 2
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![warn(clippy::missing_panics_doc)]

// Use the prelude for consistent imports

// Binary std/no_std choice
extern crate alloc;

// Binary std/no_std choice
// from wrt-foundation

// Include prelude for unified imports
pub mod prelude;

// Bounded infrastructure for intercept collections
pub mod bounded_intercept;

// Include built-in interception module
pub mod builtins;

// Built-in strategy implementations
pub mod strategies;

// Include verification module conditionally, but exclude during coverage builds
#[cfg(all(not(coverage), any(doc, feature = "kani")))]
pub mod verify;

// Re-export from prelude for convenience
pub use prelude::*;

/// Strategy pattern for intercepting component linking
#[cfg(feature = "std")]
pub trait LinkInterceptorStrategy: Send + Sync {
    /// Called before a function call is made
    ///
    /// # Arguments
    ///
    /// * `source` - Identifier of the calling component
    /// * `target` - Identifier of the target component or host
    /// * `function` - Name of the function being called
    /// * `args` - Arguments passed to the function
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Value>>` - Potentially modified arguments to use for the
    ///   call
    ///
    /// # Errors
    ///
    /// Returns an error if argument modification fails
    fn before_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
    ) -> Result<Vec<Value>>;

    /// Called after a function call completes
    ///
    /// # Arguments
    ///
    /// * `source` - Identifier of the calling component
    /// * `target` - Identifier of the target component or host
    /// * `function` - Name of the function being called
    /// * `args` - Original arguments passed to the function
    /// * `result` - Result of the function call
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Value>>` - Potentially modified result
    ///
    /// # Errors
    ///
    /// Returns an error if result modification fails
    fn after_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
        result: Result<Vec<Value>>,
    ) -> Result<Vec<Value>>;

    /// Determines if the normal execution should be bypassed
    ///
    /// If this returns true, the interceptor will skip the actual function call
    /// and use the result from `before_call` as the return value.
    ///
    /// # Returns
    ///
    /// * `bool` - Whether to bypass normal execution
    fn should_bypass(&self) -> bool {
        false
    }

    /// Determines if the strategy should intercept canonical ABI operations
    ///
    /// # Returns
    ///
    /// * `bool` - Whether to intercept canonical operations
    fn should_intercept_canonical(&self) -> bool {
        false
    }

    /// Intercepts a lift operation in the canonical ABI
    ///
    /// # Arguments
    ///
    /// * `ty` - The value type being lifted
    /// * `addr` - The memory address from which to lift
    /// * `memory_bytes` - The memory bytes to read from
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<u8>>>` - Serialized value if lifting was handled,
    ///   None if it should proceed normally
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or memory access fails
    #[cfg(feature = "std")]
    fn intercept_lift(
        &self,
        _ty: &ValType<wrt_foundation::NoStdProvider<64>>,
        _addr: u32,
        _memory_bytes: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Intercepts a lower operation in the canonical ABI
    ///
    /// # Arguments
    ///
    /// * `value_type` - The type of the value being lowered
    /// * `value_data` - The serialized value being lowered
    /// * `addr` - The memory address to which to lower
    /// * `memory_bytes` - The memory bytes to write to
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - True if the lowering was handled, false if it should
    ///   proceed normally
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization or memory write fails
    #[cfg(feature = "std")]
    fn intercept_lower(
        &self,
        _value_type: &ValType<wrt_foundation::NoStdProvider<64>>,
        _value_data: &[u8],
        _addr: u32,
        _memory_bytes: &mut [u8],
    ) -> Result<bool> {
        Ok(false)
    }

    /// Determines if the strategy should intercept component function calls
    ///
    /// # Returns
    ///
    /// * `bool` - Whether to intercept component function calls
    fn should_intercept_function(&self) -> bool {
        false
    }

    /// Intercepts a function call in the component model
    ///
    /// # Arguments
    ///
    /// * `function_name` - The name of the function being called
    /// * `arg_types` - The types of the arguments
    /// * `arg_data` - The serialized argument values
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<u8>>>` - Serialized result values if call was
    ///   handled, None if it should proceed normally
    ///
    /// # Errors
    ///
    /// Returns an error if function execution or serialization fails
    #[cfg(feature = "std")]
    fn intercept_function_call(
        &self,
        _function_name: &str,
        _arg_types: &[ValType<wrt_foundation::NoStdProvider<64>>],
        _arg_data: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Intercepts the result of a function call in the component model
    ///
    /// # Arguments
    ///
    /// * `function_name` - The name of the function that was called
    /// * `result_types` - The types of the results
    /// * `result_data` - The serialized result values
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<u8>>>` - Modified serialized results if modified,
    ///   None if they should be returned as is
    ///
    /// # Errors
    ///
    /// Returns an error if result modification or serialization fails
    #[cfg(feature = "std")]
    fn intercept_function_result(
        &self,
        _function_name: &str,
        _result_types: &[ValType<wrt_foundation::NoStdProvider<64>>],
        _result_data: &[u8],
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Intercepts a resource operation
    ///
    /// # Arguments
    ///
    /// * `handle` - The resource handle
    /// * `operation` - The operation being performed
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<u8>>>` - Serialized result value if operation was
    ///   handled, None if it should proceed normally
    ///
    /// # Errors
    ///
    /// Returns an error if resource operation or serialization fails
    fn intercept_resource_operation(
        &self,
        _handle: u32,
        _operation: &ResourceCanonicalOperation,
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Gets the preferred memory strategy for a resource or canonical operation
    ///
    /// # Arguments
    ///
    /// * `handle` - The resource handle, or 0 for canonical operations
    ///
    /// # Returns
    ///
    /// * `Option<u8>` - The preferred memory strategy (0=ZeroCopy,
    ///   1=BoundedCopy, 2=FullIsolation), or None to use the default
    fn get_memory_strategy(&self, _handle: u32) -> Option<u8> {
        None
    }

    /// Called before a component start function is executed
    ///
    /// # Arguments
    ///
    /// * `component_name` - The name of the component whose start function is
    ///   executed
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<u8>>>` - Serialized values to use as the result,
    ///   None to execute normally
    ///
    /// # Errors
    ///
    /// Returns an error if initialization or serialization fails
    fn before_start(&self, _component_name: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Called after a component start function has executed
    ///
    /// # Arguments
    ///
    /// * `component_name` - The name of the component whose start function was
    ///   executed
    /// * `result_types` - The types of the results
    /// * `result_data` - The serialized result values, if any
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<u8>>>` - Modified serialized values to use as the
    ///   final result, None to use the original result
    ///
    /// # Errors
    ///
    /// Returns an error if result modification or serialization fails
    #[cfg(feature = "std")]
    fn after_start(
        &self,
        _component_name: &str,
        _result_types: &[ValType<wrt_foundation::NoStdProvider<64>>],
        _result_data: Option<&[u8]>,
    ) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Clones this strategy
    ///
    /// # Returns
    ///
    /// * `Arc<dyn LinkInterceptorStrategy>` - A cloned version of this strategy
    fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy>;

    /// Process results after interception
    ///
    /// # Arguments
    ///
    /// * `component_name` - The name of the component
    /// * `func_name` - The name of the function
    /// * `args` - The original arguments to the function
    /// * `results` - The results of the function call
    ///
    /// # Returns
    ///
    /// * `Result<Option<Vec<Modification>>>` - Optional modifications to apply
    ///
    /// # Errors
    ///
    /// Returns an error if result processing fails
    #[cfg(feature = "std")]
    fn process_results(
        &self,
        _component_name: &str,
        _func_name: &str,
        _args: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
        _results: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
    ) -> Result<Option<Vec<Modification>>> {
        // Default implementation returns no modifications
        Ok(None)
    }
}

/// Simplified strategy pattern for intercepting component linking in `no_std`
/// environments
#[cfg(not(feature = "std"))]
pub trait LinkInterceptorStrategy: Send + Sync {
    /// Called before a function call is made
    ///
    /// # Errors
    /// Returns error if interception fails
    fn before_call(&self, source: &str, target: &str, function: &str, args: &[Value])
        -> Result<()>;

    /// Called after a function call completes
    ///
    /// # Errors
    /// Returns error if interception fails
    fn after_call(
        &self,
        source: &str,
        target: &str,
        function: &str,
        args: &[Value],
        result: Result<()>,
    ) -> Result<()>;

    /// Determines if the normal execution should be bypassed
    fn should_bypass(&self) -> bool {
        false
    }

    /// Determines if the strategy should intercept canonical ABI operations
    fn should_intercept_canonical(&self) -> bool {
        false
    }

    /// Determines if the strategy should intercept component function calls
    fn should_intercept_function(&self) -> bool {
        false
    }

    /// Intercepts a resource operation
    ///
    /// # Errors
    /// Returns error if resource interception fails
    fn intercept_resource_operation(
        &self,
        _handle: u32,
        _operation: &ResourceCanonicalOperation,
    ) -> Result<()> {
        Ok(())
    }

    /// Gets the preferred memory strategy for a resource or canonical operation
    fn get_memory_strategy(&self, _handle: u32) -> Option<u8> {
        None
    }

    /// Called before a component start function is executed
    ///
    /// # Errors
    ///
    /// Returns an error if interception processing fails
    fn before_start(&self, _component_name: &str) -> Result<()> {
        Ok(())
    }

    /// Called after a component start function has executed
    ///
    /// # Errors
    ///
    /// Returns an error if post-processing fails
    fn after_start(&self, _component_name: &str, _result_data: Option<&[u8]>) -> Result<()> {
        Ok(())
    }
}

/// Main interceptor to manage connections between components/host
#[derive(Clone)]
pub struct LinkInterceptor {
    /// Name of this interceptor for identification
    #[cfg(feature = "std")]
    name:           String,
    #[cfg(not(feature = "std"))]
    name:           &'static str,
    /// Collection of strategies to apply
    #[cfg(feature = "std")]
    pub strategies: Vec<Arc<dyn LinkInterceptorStrategy>>,
}

impl LinkInterceptor {
    /// Creates a new interceptor with the given name
    ///
    /// # Arguments
    ///
    /// * `name` - Identifier for this interceptor
    ///
    /// # Returns
    ///
    /// * `Self` - A new `LinkInterceptor` instance
    #[cfg_attr(not(feature = "std"), allow(unused_variables))]
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            #[cfg(feature = "std")]
            name:                               name.to_string(),
            #[cfg(not(feature = "std"))]
            name:                               "default",
            #[cfg(feature = "std")]
            strategies:                         Vec::new(),
        }
    }

    /// Adds a strategy to this interceptor
    ///
    /// Strategies are applied in the order they are added.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The strategy to add
    #[cfg(feature = "std")]
    pub fn add_strategy(&mut self, strategy: Arc<dyn LinkInterceptorStrategy>) {
        self.strategies.push(strategy);
    }

    /// Intercepts a function call
    ///
    /// This method applies all strategies in sequence, potentially
    /// modifying arguments and results.
    ///
    /// # Arguments
    ///
    /// * `target` - Identifier of the target component or host
    /// * `function` - Name of the function being called
    /// * `args` - Arguments to the function
    /// * `call_fn` - Function that performs the actual call
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Value>>` - The result of the function call after
    ///   interception
    ///
    /// # Errors
    ///
    /// Returns an error if interception or function call fails
    #[cfg(feature = "std")]
    pub fn intercept_call<F>(
        &self,
        target: &str,
        function: &str,
        args: &[Value],
        call_fn: F,
    ) -> Result<Vec<Value>>
    where
        F: FnOnce(Vec<Value>) -> Result<Vec<Value>>,
    {
        let mut modified_args = args.to_vec();

        // Apply before_call interceptors
        for strategy in &self.strategies {
            modified_args = strategy.before_call(&self.name, target, function, &modified_args)?;

            // Early return if strategy bypasses execution
            if strategy.should_bypass() {
                return Ok(modified_args);
            }
        }

        // Execute the actual call
        let mut result = call_fn(modified_args);

        // Apply after_call interceptors in reverse order
        for strategy in self.strategies.iter().rev() {
            result = strategy.after_call(&self.name, target, function, args, result);
        }

        result
    }

    /// Gets the name of this interceptor
    ///
    /// # Returns
    ///
    /// * `&str` - The interceptor name
    #[must_use]
    pub fn name(&self) -> &str {
        #[cfg(feature = "std")]
        return &self.name;
        #[cfg(not(feature = "std"))]
        return self.name;
    }

    /// Gets the first strategy in this interceptor
    ///
    /// This is a convenience method for accessing the primary strategy.
    /// If multiple strategies are present, only the first one is returned.
    ///
    /// # Returns
    ///
    /// * `Option<&dyn LinkInterceptorStrategy>` - The first strategy, or None
    ///   if none exists
    #[cfg(feature = "std")]
    #[must_use] 
    pub fn get_strategy(&self) -> Option<&dyn LinkInterceptorStrategy> {
        self.strategies.first().map(std::convert::AsRef::as_ref)
    }

    /// Processes results after interception
    ///
    /// # Arguments
    ///
    /// * `component_name` - The name of the component
    /// * `func_name` - The name of the function
    /// * `args` - The original arguments to the function
    /// * `results` - The results of the function call
    ///
    /// # Returns
    ///
    /// * `Result<InterceptionResult>` - The processed interception result
    ///
    /// # Errors
    ///
    /// Returns an error if result processing fails
    #[cfg(feature = "std")]
    pub fn post_intercept(
        &self,
        component_name: &str,
        func_name: &str,
        args: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
        results: &[ComponentValue<wrt_foundation::NoStdProvider<64>>],
    ) -> Result<InterceptionResult> {
        // Create default result (no modifications)
        let mut result = InterceptionResult {
            modified:      false,
            modifications: Vec::new(),
        };

        // Process with each strategy
        for strategy in &self.strategies {
            // Apply strategy-specific post-processing
            if let Some(modifications) =
                strategy.process_results(component_name, func_name, args, results)?
            {
                result.modified = true;
                result.modifications.extend(modifications);
            }
        }

        Ok(result)
    }

    /// Applies modifications to serialized data
    ///
    /// # Arguments
    ///
    /// * `serialized_data` - The original serialized data
    /// * `modifications` - The modifications to apply
    ///
    /// # Returns
    ///
    /// * `Result<Vec<u8>>` - The modified serialized data
    ///
    /// # Errors
    ///
    /// Returns an error if modification application fails
    #[cfg(feature = "std")]
    pub fn apply_modifications(
        &self,
        serialized_data: &[u8],
        modifications: &[Modification],
    ) -> Result<Vec<u8>> {
        let mut modified_data = serialized_data.to_vec();

        for modification in modifications {
            match modification {
                Modification::None => {
                    // No modification needed
                },
                Modification::Replace { offset, data } => {
                    let end_offset = offset + data.len();
                    if end_offset > modified_data.len() {
                        return Err(Error::runtime_execution_error(
                            "Replace range exceeds data length",
                        ));
                    }

                    // Fixed version without borrowing issues
                    let start = *offset;
                    let end = start + data.len();
                    modified_data[start..end].copy_from_slice(data);
                },
                Modification::Insert { offset, data } => {
                    let start = *offset;
                    if start > modified_data.len() {
                        return Err(Error::new(
                            wrt_error::ErrorCategory::Validation,
                            wrt_error::codes::VALIDATION_ERROR,
                            "Insert offset exceeds data length",
                        ));
                    }

                    modified_data.splice(start..start, data.iter().copied());
                },
                Modification::Remove { offset, length } => {
                    let start = *offset;
                    let end = start + length;
                    if end > modified_data.len() {
                        return Err(Error::runtime_execution_error(
                            "Remove range exceeds data length",
                        ));
                    }

                    modified_data.drain(start..end);
                },
            }
        }

        Ok(modified_data)
    }
}

/// Result of an interception operation
#[derive(Debug, Clone)]
pub struct InterceptionResult {
    /// Whether the data has been modified
    pub modified:      bool,
    /// Modifications to apply to the serialized data
    #[cfg(feature = "std")]
    pub modifications: Vec<Modification>,
    /// Modifications to apply to the serialized data
    #[cfg(not(feature = "std"))]
    pub modifications: wrt_foundation::bounded::BoundedVec<
        Modification,
        32,
        wrt_foundation::safe_memory::NoStdProvider<8192>,
    >,
}

/// Modification to apply to serialized data
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum Modification {
    /// No modification (default)
    #[default]
    None,
    /// Replace data at an offset
    Replace {
        /// Byte offset where the replacement starts
        offset: usize,
        /// New data to insert at the offset
        #[cfg(feature = "std")]
        data:   Vec<u8>,
        /// New data to insert at the offset
        #[cfg(not(feature = "std"))]
        data: wrt_foundation::bounded::BoundedVec<
            u8,
            256,
            wrt_foundation::safe_memory::NoStdProvider<4096>,
        >,
    },
    /// Insert data at an offset
    Insert {
        /// Byte offset where to insert data
        offset: usize,
        /// Data to insert at the offset
        #[cfg(feature = "std")]
        data:   Vec<u8>,
        /// New data to insert at the offset
        #[cfg(not(feature = "std"))]
        data: wrt_foundation::bounded::BoundedVec<
            u8,
            256,
            wrt_foundation::safe_memory::NoStdProvider<4096>,
        >,
    },
    /// Remove data at an offset with a given length
    Remove {
        /// Byte offset where to start removing data
        offset: usize,
        /// Number of bytes to remove
        length: usize,
    },
}


// Implement required traits for BoundedVec compatibility
impl wrt_foundation::traits::Checksummable for Modification {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        match self {
            Modification::None => {
                checksum.update_slice(&[0u8]);
            },
            Modification::Replace { offset, data } => {
                checksum.update_slice(&offset.to_le_bytes());
                checksum.update_slice(&[1u8]); // tag for Replace variant
                #[cfg(feature = "std")]
                checksum.update_slice(data);
                #[cfg(not(feature = "std"))]
                for i in 0..data.len() {
                    if let Ok(byte) = data.get(i) {
                        checksum.update_slice(&[byte]);
                    }
                }
            },
            Modification::Insert { offset, data } => {
                checksum.update_slice(&offset.to_le_bytes());
                checksum.update_slice(&[2u8]); // tag for Insert variant
                #[cfg(feature = "std")]
                checksum.update_slice(data);
                #[cfg(not(feature = "std"))]
                for i in 0..data.len() {
                    if let Ok(byte) = data.get(i) {
                        checksum.update_slice(&[byte]);
                    }
                }
            },
            Modification::Remove { offset, length } => {
                checksum.update_slice(&offset.to_le_bytes());
                checksum.update_slice(&length.to_le_bytes());
                checksum.update_slice(&[3u8]); // tag for Remove variant
            },
        }
    }
}

impl wrt_foundation::traits::ToBytes for Modification {
    fn serialized_size(&self) -> usize {
        match self {
            Modification::None => 1, // Just the tag
            Modification::Replace { data, .. } | Modification::Insert { data, .. } => {
                1 + 8 + 4 + data.len() // tag + offset + length + data
            },
            Modification::Remove { .. } => {
                1 + 8 + 8 // tag + offset + length
            },
        }
    }

    fn to_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<()> {
        match self {
            Modification::None => {
                writer.write_u8(0)?; // Tag for None
            },
            Modification::Replace { offset, data } => {
                writer.write_u8(1)?; // Tag for Replace
                writer.write_usize_le(*offset)?;
                #[allow(clippy::cast_possible_truncation)]
                writer.write_u32_le(data.len() as u32)?;
                #[cfg(feature = "std")]
                writer.write_all(data)?;
                #[cfg(not(feature = "std"))]
                for i in 0..data.len() {
                    if let Ok(byte) = data.get(i) {
                        writer.write_u8(byte)?;
                    }
                }
            },
            Modification::Insert { offset, data } => {
                writer.write_u8(2)?; // Tag for Insert
                writer.write_usize_le(*offset)?;
                #[allow(clippy::cast_possible_truncation)]
                writer.write_u32_le(data.len() as u32)?;
                #[cfg(feature = "std")]
                writer.write_all(data)?;
                #[cfg(not(feature = "std"))]
                for i in 0..data.len() {
                    if let Ok(byte) = data.get(i) {
                        writer.write_u8(byte)?;
                    }
                }
            },
            Modification::Remove { offset, length } => {
                writer.write_u8(3)?; // Tag for Remove
                writer.write_usize_le(*offset)?;
                writer.write_usize_le(*length)?;
            },
        }
        Ok(())
    }
}

impl wrt_foundation::traits::FromBytes for Modification {
    fn from_bytes_with_provider<P: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'_>,
        _provider: &P,
    ) -> wrt_error::Result<Self> {
        use wrt_error::Error;

        let tag = reader.read_u8()?;

        match tag {
            0 => Ok(Modification::None),
            1 => {
                // Replace variant
                let offset = reader.read_usize_le()?;
                let data_len = reader.read_u32_le()? as usize;

                #[cfg(feature = "std")]
                let data = {
                    let mut vec = Vec::with_capacity(data_len);
                    for _ in 0..data_len {
                        vec.push(reader.read_u8()?);
                    }
                    vec
                };
                #[cfg(not(feature = "std"))]
                let data = {
                    use wrt_foundation::{
                        bounded::BoundedVec,
                        budget_aware_provider::CrateId,
                        safe_managed_alloc,
                    };
                    let provider = safe_managed_alloc!(4096, CrateId::Intercept).map_err(|_| {
                        Error::memory_error("Failed to allocate for modification data")
                    })?;
                    let mut bounded_data = BoundedVec::new(provider)
                        .map_err(|_| Error::memory_error("Failed to create bounded vec"))?;
                    for _ in 0..data_len {
                        bounded_data
                            .push(reader.read_u8()?)
                            .map_err(|_| Error::memory_error("Failed to push to bounded vec"))?;
                    }
                    bounded_data
                };

                Ok(Modification::Replace { offset, data })
            },
            2 => {
                // Insert variant
                let offset = reader.read_usize_le()?;
                let data_len = reader.read_u32_le()? as usize;

                #[cfg(feature = "std")]
                let data = {
                    let mut vec = Vec::with_capacity(data_len);
                    for _ in 0..data_len {
                        vec.push(reader.read_u8()?);
                    }
                    vec
                };
                #[cfg(not(feature = "std"))]
                let data = {
                    use wrt_foundation::{
                        bounded::BoundedVec,
                        budget_aware_provider::CrateId,
                        safe_managed_alloc,
                    };
                    let provider = safe_managed_alloc!(4096, CrateId::Intercept).map_err(|_| {
                        Error::memory_error("Failed to allocate for modification data")
                    })?;
                    let mut bounded_data = BoundedVec::new(provider)
                        .map_err(|_| Error::memory_error("Failed to create bounded vec"))?;
                    for _ in 0..data_len {
                        bounded_data
                            .push(reader.read_u8()?)
                            .map_err(|_| Error::memory_error("Failed to push to bounded vec"))?;
                    }
                    bounded_data
                };

                Ok(Modification::Insert { offset, data })
            },
            3 => {
                // Remove variant
                let offset = reader.read_usize_le()?;
                let length = reader.read_usize_le()?;

                Ok(Modification::Remove { offset, length })
            },
            _ => Err(Error::parse_error("Unknown Modification variant")),
        }
    }
}

#[cfg(feature = "std")]
impl std::fmt::Debug for LinkInterceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkInterceptor")
            .field("name", &self.name)
            .field("strategies_count", &self.strategies.len())
            .finish()
    }
}

#[cfg(not(feature = "std"))]
impl core::fmt::Debug for LinkInterceptor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LinkInterceptor").field("name", &self.name).finish()
    }
}

// Duplicate Debug implementation removed

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "std"))]
    use alloc::{vec, vec::Vec};

    struct TestStrategy {
        bypass:        bool,
        modify_args:   bool,
        modify_result: bool,
    }

    #[cfg(feature = "std")]
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
            if self.modify_result {
                match result {
                    Ok(_) => Ok(vec![Value::I32(99)]),
                    Err(e) => Err(e),
                }
            } else {
                result
            }
        }

        fn should_bypass(&self) -> bool {
            self.bypass
        }

        fn should_intercept_canonical(&self) -> bool {
            false
        }

        fn should_intercept_function(&self) -> bool {
            false
        }

        fn clone_strategy(&self) -> Arc<dyn LinkInterceptorStrategy> {
            Arc::new(Self {
                bypass: self.bypass,
                modify_args: self.modify_args,
                modify_result: self.modify_result,
            })
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_interceptor_passthrough() {
        let strategy = Arc::new(TestStrategy {
            bypass:        false,
            modify_args:   false,
            modify_result: false,
        });

        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let result = interceptor.intercept_call("target", "func", &[Value::I32(10)], |args| {
            assert_eq!(args, vec![Value::I32(10)]);
            Ok(vec![Value::I32(20)])
        });

        assert_eq!(result.unwrap(), vec![Value::I32(20)]);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_interceptor_modify_args() {
        let strategy = Arc::new(TestStrategy {
            bypass:        false,
            modify_args:   true,
            modify_result: false,
        });

        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let result = interceptor.intercept_call("target", "func", &[Value::I32(10)], |args| {
            assert_eq!(args, vec![Value::I32(42)]);
            Ok(vec![Value::I32(20)])
        });

        assert_eq!(result.unwrap(), vec![Value::I32(20)]);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_interceptor_modify_result() {
        let strategy = Arc::new(TestStrategy {
            bypass:        false,
            modify_args:   false,
            modify_result: true,
        });

        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let result = interceptor.intercept_call("target", "func", &[Value::I32(10)], |args| {
            assert_eq!(args, vec![Value::I32(10)]);
            Ok(vec![Value::I32(20)])
        });

        assert_eq!(result.unwrap(), vec![Value::I32(99)]);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_interceptor_bypass() {
        let strategy = Arc::new(TestStrategy {
            bypass:        true,
            modify_args:   true,
            modify_result: false,
        });

        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy);

        let result = interceptor.intercept_call("target", "func", &[Value::I32(10)], |_| {
            panic!("This should not be called");
        });

        assert_eq!(result.unwrap(), vec![Value::I32(42)]);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_multiple_strategies() {
        let strategy1 = Arc::new(TestStrategy {
            bypass:        false,
            modify_args:   true,
            modify_result: false,
        });

        let strategy2 = Arc::new(TestStrategy {
            bypass:        false,
            modify_args:   false,
            modify_result: true,
        });

        let mut interceptor = LinkInterceptor::new("test");
        interceptor.add_strategy(strategy1);
        interceptor.add_strategy(strategy2);

        let result = interceptor.intercept_call("target", "func", &[Value::I32(10)], |args| {
            assert_eq!(args, vec![Value::I32(42)]);
            Ok(vec![Value::I32(20)])
        });

        assert_eq!(result.unwrap(), vec![Value::I32(99)]);
    }
}

// Panic handler disabled to avoid conflicts with other crates
// The main wrt crate should provide the panic handler
// #[cfg(all(not(feature = "std"), not(test), not(feature =
// "disable-panic-handler")))] #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
