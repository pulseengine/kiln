//! WASI instance provider for Component Model
//!
//! Creates component instances that satisfy WASI Preview 2 imports.
//! This is a stub implementation - instances don't have actual WASI logic yet.

use crate::bounded_component_infra::ComponentProvider;
use crate::instantiation::{ExportValue, FunctionExport, InstanceImport};
use crate::prelude::WrtComponentType;
use wrt_error::Result;

#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::boxed::Box;

/// Provides WASI component instances
///
/// Each WASI interface (e.g., wasi:cli/stdout, wasi:io/streams) is represented
/// as a component instance that exports the functions defined in the WIT interface.
///
/// This implementation creates instances with actual exported functions that call
/// the WASI host implementations.
#[derive(Debug)]
pub struct WasiInstanceProvider {
    /// Next instance ID to allocate
    /// Starting at 100 to avoid conflicts with component-internal instances
    next_instance_id: u32,
    /// Next function index to allocate
    next_function_index: u32,
}

impl WasiInstanceProvider {
    /// Create a new WASI instance provider
    pub fn new() -> Result<Self> {
        Ok(Self {
            next_instance_id: 100,
            next_function_index: 1000,
        })
    }

    /// Create a WASI instance for the given interface with actual function exports
    ///
    /// # Arguments
    /// * `interface_name` - Full interface name (e.g., "wasi:cli/stdout@0.2.0")
    ///
    /// # Returns
    /// InstanceImport with exported functions that call WASI host implementations
    pub fn create_instance(&mut self, interface_name: &str) -> Result<InstanceImport> {
        // Allocate instance ID
        let id = self.next_instance_id;
        self.next_instance_id += 1;

        #[cfg(feature = "std")]
        println!(
            "[WASI-PROVIDER] Creating instance {} for {}",
            id, interface_name
        );

        // Create instance with exports based on interface
        let mut instance = InstanceImport {
            #[cfg(feature = "std")]
            exports: std::collections::BTreeMap::new(),
            #[cfg(not(feature = "std"))]
            exports: Default::default(),
        };

        // Populate exports based on interface name
        match interface_name {
            name if name.starts_with("wasi:cli/stdout") => {
                self.add_stdout_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/stdin") => {
                self.add_stdin_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/stderr") => {
                self.add_stderr_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/environment") => {
                self.add_environment_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/exit") => {
                self.add_exit_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:io/streams") => {
                self.add_streams_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:io/error") => {
                self.add_error_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:io/poll") => {
                self.add_poll_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:filesystem/types") => {
                self.add_filesystem_types_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:filesystem/preopens") => {
                self.add_filesystem_preopens_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:clocks/monotonic-clock") => {
                self.add_monotonic_clock_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:clocks/wall-clock") => {
                self.add_wall_clock_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:random/random") => {
                self.add_random_exports(&mut instance)?;
            },
            // Terminal interfaces for C/C++ runtime support
            name if name.starts_with("wasi:cli/terminal-stdin") => {
                self.add_terminal_stdin_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/terminal-stdout") => {
                self.add_terminal_stdout_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/terminal-stderr") => {
                self.add_terminal_stderr_exports(&mut instance)?;
            },
            name if name.starts_with("wasi:cli/terminal-input") => {
                // Resource-only interface - no function exports needed
                // The resource-drop is handled by canonical ABI
            },
            name if name.starts_with("wasi:cli/terminal-output") => {
                // Resource-only interface - no function exports needed
                // The resource-drop is handled by canonical ABI
            },
            _ => {
                #[cfg(feature = "std")]
                println!(
                    "[WASI-PROVIDER] Warning: No exports for unknown interface '{}'",
                    interface_name
                );
            },
        }

        #[cfg(feature = "std")]
        println!(
            "[WASI-PROVIDER] Instance {} created with {} exports",
            id,
            instance.exports.len()
        );

        Ok(instance)
    }

    /// Add stdout interface exports (wasi:cli/stdout@0.2.0)
    fn add_stdout_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-stdout() -> output-stream
        let get_stdout_index = self.next_function_index;
        self.next_function_index += 1;

        let provider = ComponentProvider::default();
        let signature = WrtComponentType::Unit(provider)?;

        let func_export = FunctionExport {
            signature,
            index: get_stdout_index,
        };

        #[cfg(feature = "std")]
        instance.exports.insert(
            "get-stdout".to_string(),
            Box::new(ExportValue::Function(func_export)),
        );

        #[cfg(not(feature = "std"))]
        {
            use wrt_foundation::bounded::BoundedString;
            let name = BoundedString::try_from_str("get-stdout")?;
            instance
                .exports
                .push((name, Box::new(ExportValue::Function(func_export))))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many exports"))?;
        }

        #[cfg(feature = "std")]
        println!(
            "[WASI-PROVIDER] Added export: get-stdout (index {})",
            get_stdout_index
        );

        Ok(())
    }

    /// Add stdin interface exports (wasi:cli/stdin@0.2.0)
    fn add_stdin_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-stdin() -> input-stream
        self.add_simple_export(instance, "get-stdin")
    }

    /// Add stderr interface exports (wasi:cli/stderr@0.2.0)
    fn add_stderr_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-stderr() -> output-stream
        self.add_simple_export(instance, "get-stderr")
    }

    /// Add environment interface exports (wasi:cli/environment@0.2.0)
    fn add_environment_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-environment() -> list<tuple<string, string>>
        self.add_simple_export(instance, "get-environment")?;
        // Export: get-arguments() -> list<string>
        self.add_simple_export(instance, "get-arguments")
    }

    /// Add exit interface exports (wasi:cli/exit@0.2.0)
    fn add_exit_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: exit(status: result) -> ()
        self.add_simple_export(instance, "exit")
    }

    /// Add streams interface exports (wasi:io/streams@0.2.0)
    fn add_streams_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: [method]output-stream.blocking-write-and-flush
        self.add_simple_export(instance, "[method]output-stream.blocking-write-and-flush")?;
        // Export: [method]input-stream.blocking-read
        self.add_simple_export(instance, "[method]input-stream.blocking-read")?;
        // Export: [method]output-stream.blocking-flush
        self.add_simple_export(instance, "[method]output-stream.blocking-flush")?;
        // Export: [method]output-stream.write
        self.add_simple_export(instance, "[method]output-stream.write")?;
        // Export: [method]input-stream.read
        self.add_simple_export(instance, "[method]input-stream.read")
    }

    /// Add error interface exports (wasi:io/error@0.2.0)
    fn add_error_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: [method]error.to-debug-string
        self.add_simple_export(instance, "[method]error.to-debug-string")
    }

    /// Add poll interface exports (wasi:io/poll@0.2.0)
    fn add_poll_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: poll(pollables: list<pollable>) -> list<u32>
        self.add_simple_export(instance, "poll")
    }

    /// Add filesystem types interface exports (wasi:filesystem/types@0.2.0)
    fn add_filesystem_types_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: [method]descriptor.read-via-stream
        self.add_simple_export(instance, "[method]descriptor.read-via-stream")?;
        // Export: [method]descriptor.write-via-stream
        self.add_simple_export(instance, "[method]descriptor.write-via-stream")?;
        // Export: [method]descriptor.stat
        self.add_simple_export(instance, "[method]descriptor.stat")?;
        // Export: [method]descriptor.open-at
        self.add_simple_export(instance, "[method]descriptor.open-at")
    }

    /// Add filesystem preopens interface exports (wasi:filesystem/preopens@0.2.0)
    fn add_filesystem_preopens_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-directories() -> list<tuple<descriptor, string>>
        self.add_simple_export(instance, "get-directories")
    }

    /// Add monotonic clock interface exports (wasi:clocks/monotonic-clock@0.2.0)
    fn add_monotonic_clock_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: now() -> instant
        self.add_simple_export(instance, "now")?;
        // Export: resolution() -> duration
        self.add_simple_export(instance, "resolution")
    }

    /// Add wall clock interface exports (wasi:clocks/wall-clock@0.2.0)
    fn add_wall_clock_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: now() -> datetime
        self.add_simple_export(instance, "now")?;
        // Export: resolution() -> datetime
        self.add_simple_export(instance, "resolution")
    }

    /// Add random interface exports (wasi:random/random@0.2.0)
    fn add_random_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-random-bytes(len: u64) -> list<u8>
        self.add_simple_export(instance, "get-random-bytes")?;
        // Export: get-random-u64() -> u64
        self.add_simple_export(instance, "get-random-u64")
    }

    /// Add terminal-stdin interface exports (wasi:cli/terminal-stdin@0.2.0)
    /// Returns option<terminal-input> to indicate if stdin is a terminal
    fn add_terminal_stdin_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-terminal-stdin() -> option<terminal-input>
        self.add_simple_export(instance, "get-terminal-stdin")
    }

    /// Add terminal-stdout interface exports (wasi:cli/terminal-stdout@0.2.0)
    /// Returns option<terminal-output> to indicate if stdout is a terminal
    fn add_terminal_stdout_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-terminal-stdout() -> option<terminal-output>
        self.add_simple_export(instance, "get-terminal-stdout")
    }

    /// Add terminal-stderr interface exports (wasi:cli/terminal-stderr@0.2.0)
    /// Returns option<terminal-output> to indicate if stderr is a terminal
    fn add_terminal_stderr_exports(&mut self, instance: &mut InstanceImport) -> Result<()> {
        // Export: get-terminal-stderr() -> option<terminal-output>
        self.add_simple_export(instance, "get-terminal-stderr")
    }

    /// Helper to add a simple function export
    fn add_simple_export(&mut self, instance: &mut InstanceImport, name: &str) -> Result<()> {
        let func_index = self.next_function_index;
        self.next_function_index += 1;

        let provider = ComponentProvider::default();
        let signature = WrtComponentType::Unit(provider)?;

        let func_export = FunctionExport {
            signature,
            index: func_index,
        };

        #[cfg(feature = "std")]
        instance.exports.insert(
            name.to_string(),
            Box::new(ExportValue::Function(func_export)),
        );

        #[cfg(not(feature = "std"))]
        {
            use wrt_foundation::bounded::BoundedString;
            let bounded_name = BoundedString::try_from_str(name)?;
            instance
                .exports
                .push((bounded_name, Box::new(ExportValue::Function(func_export))))
                .map_err(|_| wrt_error::Error::resource_exhausted("Too many exports"))?;
        }

        #[cfg(feature = "std")]
        println!(
            "[WASI-PROVIDER] Added export: {} (index {})",
            name, func_index
        );

        Ok(())
    }

    /// Get the list of supported WASI interfaces
    ///
    /// Returns interface names that this provider can satisfy
    #[allow(dead_code)]
    pub fn supported_interfaces() -> &'static [&'static str] {
        &[
            "wasi:cli/environment@0.2.0",
            "wasi:cli/exit@0.2.0",
            "wasi:cli/stdin@0.2.0",
            "wasi:cli/stdout@0.2.0",
            "wasi:cli/stderr@0.2.0",
            "wasi:io/error@0.2.0",
            "wasi:io/poll@0.2.0",
            "wasi:io/streams@0.2.0",
            "wasi:clocks/monotonic-clock@0.2.0",
            "wasi:clocks/wall-clock@0.2.0",
            "wasi:filesystem/types@0.2.0",
            "wasi:filesystem/preopens@0.2.0",
            "wasi:random/random@0.2.0",
        ]
    }
}

impl Default for WasiInstanceProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create WasiInstanceProvider")
    }
}
