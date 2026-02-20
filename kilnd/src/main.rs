//! # WebAssembly Runtime Daemon (wrtd)
//!
//! A minimal daemon process for WebAssembly module execution with support for
//! both std and no_std environments. Uses only internal WRT capabilities to
//! minimize dependencies.
//!
//! ## Features
//!
//! - **Minimal Dependencies**: Uses only internal WRT crates
//! - **Binary std/no_std**: Single binary that detects runtime capabilities
//! - **Internal Logging**: Uses wrt-logging for structured output
//! - **Runtime Detection**: Automatically selects appropriate execution mode
//!
//! ## Usage
//!
//! ```bash
//! # Standard mode (with filesystem access)
//! wrtd --std module.wasm --function start
//!
//! # No-std mode (embedded/bare metal)
//! wrtd --no-std --data <hex-bytes> --function start
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

// Simple global allocator for no_std mode - use a static buffer
#[cfg(all(not(feature = "std"), feature = "enable-panic-handler"))]
use linked_list_allocator::LockedHeap;

#[cfg(all(not(feature = "std"), feature = "enable-panic-handler"))]
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Static heap memory for the allocator
#[cfg(all(not(feature = "std"), feature = "enable-panic-handler"))]
static mut HEAP: [u8; 64 * 1024] = [0; 64 * 1024]; // 64KB heap

// Conditional imports based on std feature
// Core imports available in both modes
use core::str;
#[cfg(feature = "std")]
use std::{
    env,
    fs,
    process,
};

// Internal WRT dependencies (always available)
use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
// Conditional imports for WRT allocator
#[cfg(all(feature = "std", feature = "safety-critical"))]
use wrt_foundation::allocator::{
    CrateId,
    WrtVec,
};
use wrt_logging::{
    LogLevel,
    MinimalLogHandler,
};

// Bounded infrastructure for static memory allocation
#[cfg(feature = "std")]
pub mod bounded_wrtd_infra;

// Safety-critical memory limits
#[cfg(feature = "safety-critical")]
pub mod memory_limits;

// Optional WRT execution capabilities (only in std mode with wrt-execution
// feature) Engine type moved to wrt::engine module
// Module type is available through wrt prelude

// WASI host function support
// Component model support - temporarily disabled
// #[cfg(feature = "component-model")]
#[cfg(feature = "component-model")]
use wrt_decoder::component::decode_component;
// Enhanced host function registry
#[cfg(feature = "wrt-execution")]
use wrt_host::CallbackRegistry;
#[cfg(feature = "wasi")]
use wrt_wasi::{
    ComponentModelProvider,
    WasiCapabilities,
    WasiDispatcher,
    WasiHostProvider,
    set_global_wasi_args,
};

/// Configuration for the runtime daemon
#[derive(Debug, Clone)]
pub struct WrtdConfig {
    /// Maximum fuel (execution steps) allowed
    pub max_fuel: u64,
    /// Maximum memory usage in bytes
    pub max_memory: usize,
    /// Function to execute
    pub function_name: Option<String>,
    /// Module data (for no_std mode)
    pub module_data: Option<&'static [u8]>,
    /// Module path (for std mode)
    #[cfg(feature = "std")]
    pub module_path: Option<String>,
    /// Enable WASI support
    #[cfg(feature = "wasi")]
    pub enable_wasi: bool,
    /// WASI version to use
    #[cfg(feature = "wasi")]
    pub wasi_version: WasiVersion,
    /// Environment variables to expose to WASI
    #[cfg(feature = "wasi")]
    pub wasi_env_vars: Vec<String>,
    /// Arguments to pass to WASI program
    #[cfg(feature = "wasi")]
    pub wasi_args: Vec<String>,
    /// Enable component model support
    #[cfg(feature = "component-model")]
    pub enable_component_model: bool,
    /// Component interfaces to enable
    #[cfg(feature = "component-model")]
    pub component_interfaces: Vec<String>,
    /// Memory profiling enabled
    pub enable_memory_profiling: bool,
    /// Platform-specific optimizations
    pub enable_platform_optimizations: bool,
    /// WASI capabilities
    #[cfg(feature = "wasi")]
    pub wasi_capabilities: Option<WasiCapabilities>,
}

impl Default for WrtdConfig {
    fn default() -> Self {
        Self {
            max_fuel: 1_000_000, // 1M fuel for large components
            max_memory: 64 * 1024 * 1024, // 64MB default
            function_name: None,
            module_data: None,
            #[cfg(feature = "std")]
            module_path: None,
            #[cfg(feature = "wasi")]
            enable_wasi: false,
            #[cfg(feature = "wasi")]
            wasi_version: WasiVersion::Preview2,
            #[cfg(feature = "wasi")]
            wasi_capabilities: None,
            #[cfg(feature = "wasi")]
            wasi_env_vars: Vec::new(),
            #[cfg(feature = "wasi")]
            wasi_args: Vec::new(),
            #[cfg(feature = "component-model")]
            enable_component_model: true,
            #[cfg(feature = "component-model")]
            component_interfaces: Vec::new(),
            enable_memory_profiling: false,
            enable_platform_optimizations: true,
        }
    }
}

/// WASI version selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasiVersion {
    /// WASI Preview 2 (component model)
    Preview2,
}

/// Memory profiler for tracking resource usage
#[derive(Debug)]
pub struct MemoryProfiler {
    enabled:       bool,
    peak_usage:    usize,
    current_usage: usize,
}

impl MemoryProfiler {
    /// Creates a new memory profiler instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            enabled:       true,
            peak_usage:    0,
            current_usage: 0,
        })
    }

    /// Records a memory allocation of the given size
    pub fn record_allocation(&mut self, size: usize) {
        if self.enabled {
            self.current_usage += size;
            if self.current_usage > self.peak_usage {
                self.peak_usage = self.current_usage;
            }
        }
    }

    /// Records a memory deallocation of the given size
    pub fn record_deallocation(&mut self, size: usize) {
        if self.enabled && self.current_usage >= size {
            self.current_usage -= size;
        }
    }

    /// Returns the peak memory usage recorded
    pub fn peak_usage(&self) -> usize {
        self.peak_usage
    }

    /// Returns the current memory usage
    pub fn current_usage(&self) -> usize {
        self.current_usage
    }
}

/// Runtime statistics
#[derive(Debug, Clone, Default)]
pub struct RuntimeStats {
    /// Modules executed
    pub modules_executed:          u32,
    /// Components executed
    pub components_executed:       u32,
    /// Total fuel consumed
    pub fuel_consumed:             u64,
    /// Peak memory usage
    pub peak_memory:               usize,
    /// WASI functions called
    pub wasi_functions_called:     u64,
    /// Host functions registered
    pub host_functions_registered: usize,
    /// Cross-component calls
    pub cross_component_calls:     u32,
}

/// Simple log handler that uses minimal output
pub struct WrtdLogHandler;

impl MinimalLogHandler for WrtdLogHandler {
    fn handle_minimal_log(&self, level: LogLevel, message: &'static str) -> Result<()> {
        // In std mode, use println!; in no_std mode, this would need platform-specific
        // output
        #[cfg(feature = "std")]
        {
            let prefix = match level {
                LogLevel::Trace => "TRACE",
                LogLevel::Debug => "DEBUG",
                LogLevel::Info => "INFO",
                LogLevel::Warn => "WARN",
                LogLevel::Error => "ERROR",
                LogLevel::Critical => "CRITICAL",
            };
            println!("[{}] {}", prefix, message);
        }

        #[cfg(not(feature = "std"))]
        {
            // In no_std mode, we can't easily print to console
            // This would typically write to a hardware register, LED, or serial port
            let _ = (level, message); // Suppress unused warnings
        }

        Ok(())
    }
}

/// WASM execution engine abstraction
pub struct WrtdEngine {
    config:                 WrtdConfig,
    stats:                  RuntimeStats,
    logger:                 WrtdLogHandler,
    /// Host function registry for all host functions
    #[cfg(feature = "wrt-execution")]
    #[allow(dead_code)]
    host_registry:          CallbackRegistry,
    /// WASI provider for WASI functions
    #[cfg(feature = "wasi")]
    wasi_provider:          Option<Box<dyn WasiHostProvider>>,
    /// Component registry for component model
    #[cfg(feature = "component-model")]
    // component_registry:     Option<ComponentRegistry>, // Disabled
    /// Memory profiler
    memory_profiler:        Option<MemoryProfiler>,
    /// Platform optimizations enabled
    platform_optimizations: bool,
}

impl WrtdEngine {
    /// Create a new engine with the given configuration
    pub fn new(config: WrtdConfig) -> Result<Self> {
        let mut engine = Self {
            config,
            stats: RuntimeStats::default(),
            logger: WrtdLogHandler,
            #[cfg(feature = "wrt-execution")]
            host_registry: CallbackRegistry::new(),
            #[cfg(feature = "wasi")]
            wasi_provider: None,
            #[cfg(feature = "component-model")]
            // component_registry: None, // Disabled
            memory_profiler: None,
            platform_optimizations: false,
        };

        // Initialize platform optimizations
        engine.init_platform_optimizations()?;

        // Initialize memory system (required for WASI and other subsystems)
        wrt_foundation::memory_init::init_wrt_memory()?;

        // Initialize memory profiling if enabled
        if engine.config.enable_memory_profiling {
            engine.init_memory_profiling()?;
        }

        // Initialize WASI if enabled
        #[cfg(feature = "wasi")]
        if engine.config.enable_wasi {
            engine.init_wasi()?;
        }

        // Initialize component model if enabled
        #[cfg(feature = "component-model")]
        if engine.config.enable_component_model {
            engine.init_component_model()?;
        }

        Ok(engine)
    }

    /// Initialize platform optimizations
    fn init_platform_optimizations(&mut self) -> Result<()> {
        if self.config.enable_platform_optimizations {
            let _ = self
                .logger
                .handle_minimal_log(LogLevel::Info, "Enabling platform optimizations");

            // Initialize platform-specific features
            #[cfg(feature = "wrt-execution")]
            // PlatformMemory::init_optimizations().map_err(|_| { // Disabled
            Ok(()).map_err(|_: ()| {
                Error::runtime_error("Failed to initialize platform memory optimizations")
            })?;

            self.platform_optimizations = true;
            let _ =
                self.logger.handle_minimal_log(LogLevel::Info, "Platform optimizations enabled");
        }
        Ok(())
    }

    /// Initialize memory profiling
    fn init_memory_profiling(&mut self) -> Result<()> {
        let _ = self.logger.handle_minimal_log(LogLevel::Info, "Initializing memory profiling");

        self.memory_profiler = Some(
            MemoryProfiler::new()
                .map_err(|_| Error::runtime_error("Failed to initialize memory profiler"))?,
        );

        let _ = self.logger.handle_minimal_log(LogLevel::Info, "Memory profiling initialized");
        Ok(())
    }

    /// Initialize WASI host functions
    #[cfg(feature = "wasi")]
    fn init_wasi(&mut self) -> Result<()> {
        let _ = self
            .logger
            .handle_minimal_log(LogLevel::Info, "Initializing WASI host functions");

        // Get WASI capabilities or create minimal set
        let mut capabilities = if let Some(caps) = self.config.wasi_capabilities.clone() {
            caps
        } else {
            WasiCapabilities::minimal().map_err(|_| {
                Error::runtime_error("Failed to create minimal WASI capabilities")
            })?
        };

        // Configure environment variables
        for env_var in &self.config.wasi_env_vars {
            let _ = capabilities.environment.add_allowed_var(env_var);
        }

        // Enable args access if args are provided
        if !self.config.wasi_args.is_empty() {
            capabilities.environment.args_access = true;
        }

        // Enable random access for components that need it
        capabilities.random.secure_random = true;
        capabilities.random.pseudo_random = true;

        // Enable stdout/stderr for component output
        capabilities.io.stdout_access = true;
        capabilities.io.stderr_access = true;

        // Create WASI Preview2 provider
        let mut provider = match self.config.wasi_version {
            WasiVersion::Preview2 => {
                ComponentModelProvider::new(capabilities).map_err(|_| {
                    Error::runtime_error("Failed to create WASI Preview 2 provider")
                })?
            },
        };

        // Register WASI functions with host registry
        provider.register_with_registry(&mut self.host_registry)
            .map_err(|_| Error::runtime_error("Failed to register WASI functions"))?;

        // Update stats
        let function_count = provider.function_count();
        self.stats.host_functions_registered += function_count;

        self.wasi_provider = Some(Box::new(provider));

        let _ = self.logger.handle_minimal_log(LogLevel::Info, "WASI host functions registered");
        Ok(())
    }

    /// Initialize component model support
    #[cfg(feature = "component-model")]
    fn init_component_model(&mut self) -> Result<()> {
        let _ = self.logger.handle_minimal_log(LogLevel::Info, "Initializing component model");

        // Component model initialization
        // Component registry and linker will be created on-demand during execution
        let _ = self.logger.handle_minimal_log(LogLevel::Info, "Component model initialized");
        Ok(())
    }

    /// Detect if the binary is a WebAssembly component or module
    fn detect_component_format(&self, data: &[u8]) -> Result<bool> {
        if data.len() < 8 {
            return Ok(false);
        }

        // Check for WASM magic number (0x00 0x61 0x73 0x6D)
        if &data[0..4] == [0x00, 0x61, 0x73, 0x6D] {
            // Check version to distinguish component vs module
            let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

            // Version 1 = traditional module
            // Component model uses different version encoding
            Ok(version != 1)
        } else {
            // Not a WASM binary at all
            Err(Error::parse_error("Invalid WebAssembly binary format"))
        }
    }

    /// Execute a component using the component model
    #[cfg(feature = "component-model")]
    fn execute_component(&mut self, data: &[u8]) -> Result<()> {
        let _ = self
            .logger
            .handle_minimal_log(LogLevel::Info, "Executing WebAssembly component");

        #[cfg(feature = "component-model")]
        {
            // Initialize the memory system before decoding component
            use wrt_foundation::memory_init::MemoryInitializer;
            MemoryInitializer::initialize()
                .map_err(|_| Error::runtime_error("Failed to initialize memory system"))?;

            // Decode the component from binary data and immediately Box it to avoid stack overflow
            let mut parsed_component = Box::new(decode_component(data)
                .map_err(|_| Error::parse_error("Failed to parse component binary"))?);

            let _ = self.logger.handle_minimal_log(
                LogLevel::Info,
                "Component parsed successfully"
            );

            eprintln!("DEBUG: About to call ComponentInstance::from_parsed");

            // Create and initialize component instance (passes by reference to avoid stack overflow)
            // This includes executing start functions and transitioning to Running state
            // Note: WASI functions are already registered in host_registry from init_wasi()
            use wrt_component::components::component_instantiation::ComponentInstance;

            eprintln!("DEBUG: Calling from_parsed...");
            // Wrap host_registry in Arc for passing to component
            use std::sync::Arc;
            let registry_arc = Arc::new(self.host_registry.clone());
            let mut instance = ComponentInstance::from_parsed(0, &mut *parsed_component, Some(registry_arc))
                .map_err(|e| {
                    eprintln!("[DEBUG] ComponentInstance::from_parsed failed: {:?}", e);
                    Error::runtime_error("Failed to create and initialize component instance")
                })?;
            // parsed_component is now dropped - we only keep runtime instance

            // Wire up WASI dispatcher as the host import handler for this component
            // This is CRITICAL - without this, WASI functions won't work
            #[cfg(feature = "wasi")]
            if self.config.enable_wasi {
                use wrt_wasi::WasiDispatcher;
                match WasiDispatcher::with_defaults() {
                    Ok(dispatcher) => {
                        instance.set_host_handler(Box::new(dispatcher));
                        let _ = self.logger.handle_minimal_log(
                            LogLevel::Info,
                            "WASI dispatcher connected to component instance"
                        );
                    }
                    Err(_e) => {
                        let _ = self.logger.handle_minimal_log(
                            LogLevel::Warn,
                            "Failed to create WASI dispatcher for component"
                        );
                    }
                }

                // NOTE: WASI args allocation is now done ON-DEMAND when get-arguments
                // is called (see wrt-runtime engine.rs ON-DEMAND ALLOCATION).
                // Pre-allocating before _start caused memory collisions where the
                // component's allocator reused our memory.
            }

            let _ = self.logger.handle_minimal_log(
                LogLevel::Info,
                "Component initialized and running successfully"
            );

            // Check for WASI CLI entry point and invoke it
            // Debug: print available exports
            #[cfg(feature = "std")]
            {
                println!("\n=== Available Exports ===");
                println!("Total exports: {}", instance.exports.len());
                for (idx, export) in instance.exports.iter().enumerate() {
                    println!("  Export[{}]: \"{}\"", idx, export.name);
                }
                println!();
            }

            // Find wasi:cli/run export with any version
            let run_export = instance.exports.iter()
                .find(|e| e.name.starts_with("wasi:cli/run@"))
                .map(|e| e.name.clone());

            if let Some(export_name) = run_export {
                #[cfg(feature = "std")]
                eprintln!("[INFO] Calling {} entry point", export_name);
                let _ = self.logger.handle_minimal_log(
                    LogLevel::Info,
                    "Calling wasi:cli/run entry point"
                );

                // TODO: Pass actual command-line arguments from config
                // For now, pass empty args (WASI components can get args from WASI functions)
                let args = vec![];

                // Pass the host_registry so component can call WASI functions
                #[cfg(feature = "wrt-execution")]
                let result = instance.call_function(&export_name, &args, Some(&self.host_registry));
                #[cfg(not(feature = "wrt-execution"))]
                let result = instance.call_function(&export_name, &args);

                match result {
                    Ok(_results) => {
                        let _ = self.logger.handle_minimal_log(
                            LogLevel::Info,
                            "Component executed successfully"
                        );
                    }
                    Err(e) => {
                        #[cfg(feature = "std")]
                        {
                            eprintln!("Component execution error: {}", e);
                        }
                        let _ = self.logger.handle_minimal_log(
                            LogLevel::Error,
                            "Component execution failed"
                        );
                        // Propagate the error - don't swallow it!
                        // Following the project's "fail loud and early" principle.
                        return Err(e);
                    }
                }
            } else {
                let _ = self.logger.handle_minimal_log(
                    LogLevel::Info,
                    "No wasi:cli/run entry point found - component initialized only"
                );
            }

            return Ok(());
        }

        #[cfg(not(feature = "component-model"))]
        {
            return Err(Error::runtime_error("Component model support not enabled"));
        }

        #[allow(unreachable_code)]
        if false {

            // Note: WASI functions are already registered with host_registry via init_wasi()
            // Component can look them up when resolving imports
        } else {
            return Err(Error::runtime_error("Component model not initialized"));
        }

        Ok(())
    }

    /// Execute a traditional WebAssembly module
    fn execute_traditional_module(&mut self, data: &[u8]) -> Result<()> {
        let _ = self.logger.handle_minimal_log(LogLevel::Info, "Executing WebAssembly module");

        // Execute with actual WRT engine if available
        #[cfg(all(feature = "std", feature = "wrt-execution"))]
        {
            let _ = self
                .logger
                .handle_minimal_log(LogLevel::Info, "Using real WRT execution engine");

            // Initialize the memory system before creating engine
            use wrt_foundation::memory_init::MemoryInitializer;
            MemoryInitializer::initialize()
                .map_err(|_| Error::runtime_error("Failed to initialize memory system"))?;
            use wrt_runtime::engine::{
                CapabilityAwareEngine,
                CapabilityEngine,
                EnginePreset,
            };

            // Determine engine preset from features
            let preset = if cfg!(feature = "asil-d") {
                EnginePreset::AsilD
            } else if cfg!(feature = "asil-c") {
                EnginePreset::AsilC
            } else if cfg!(feature = "asil-b") {
                EnginePreset::AsilB
            } else if cfg!(feature = "asil-a") {
                EnginePreset::AsilA
            } else if cfg!(feature = "qm") {
                EnginePreset::QM
            } else {
                EnginePreset::QM // Default to QM
            };

            // Create engine with appropriate capabilities
            let mut engine = CapabilityAwareEngine::with_preset(preset)
                .map_err(|_e| Error::runtime_error("Failed to create engine"))?;

            // Wire up WASI dispatcher as the host import handler
            // This is the SINGLE dispatch path for ALL host function calls
            #[cfg(feature = "wasi")]
            if self.config.enable_wasi {
                match WasiDispatcher::with_defaults() {
                    Ok(dispatcher) => {
                        engine.set_host_handler(Box::new(dispatcher));
                        let _ = self.logger.handle_minimal_log(LogLevel::Info, "WASI dispatcher connected");
                    }
                    Err(_e) => {
                        let _ = self.logger.handle_minimal_log(
                            LogLevel::Warn,
                            "Failed to create WASI dispatcher",
                        );
                        return Err(Error::runtime_error("WASI dispatcher creation failed"));
                    }
                }
            }

            // Legacy WASI registration (to be removed once dispatcher is verified working)
            #[cfg(feature = "wasi")]
            if self.config.enable_wasi {
                if let Err(_e) = engine.enable_wasi() {
                    let _ = self.logger.handle_minimal_log(
                        LogLevel::Warn,
                        "WASI not available for this ASIL level",
                    );
                } else {
                    let _ = self.logger.handle_minimal_log(LogLevel::Info, "WASI support enabled");
                }
            }

            // Register example host functions for demonstration
            if matches!(preset, EnginePreset::QM | EnginePreset::AsilA) {
                // Only in less restrictive modes
                let _ = engine.register_host_function("env", "host_print", |args: &[wrt_foundation::values::Value]| -> Result<Vec<wrt_foundation::values::Value>> {
                    // Simple host function that "prints" a value (in practice would log it)
                    if let Some(wrt_foundation::values::Value::I32(_val)) = args.get(0) {
                        // In a real implementation, this would print to stdout or log
                        // For now, just return success
                    }
                    Ok(vec![])
                }).unwrap_or(());

                let _ = self
                    .logger
                    .handle_minimal_log(LogLevel::Info, "Example host functions registered");
            }

            // Load module
            let module_handle = engine.load_module(data)
                .map_err(|_| Error::runtime_execution_error("Failed to load module"))?;

            // Instantiate
            let instance = engine.instantiate(module_handle)
                .map_err(|_| Error::runtime_execution_error("Failed to instantiate module"))?;

            // Execute function - try common entry points
            let function_name = self.config.function_name.as_deref().unwrap_or("_start");
            let _ = self.logger.handle_minimal_log(LogLevel::Info, "Executing function");

            // Check if function exists before execution
            if !engine.has_function(instance, function_name).map_err(|_e| {
                Error::runtime_function_not_found("Failed to check function existence")
            })? {
                let _ = self
                    .logger
                    .handle_minimal_log(LogLevel::Error, "Function not found in module exports");
                return Err(Error::runtime_function_not_found("Function not found"));
            }

            let results = engine
                .execute(instance, function_name, &[])
                .map_err(|_| Error::runtime_execution_error("Function execution failed"))?;

            // Display execution results
            if !results.is_empty() {
                println!("\n✓ Function '{}' returned {} value(s):", function_name, results.len());
                for (i, value) in results.iter().enumerate() {
                    println!("  [{}] {:?}", i, value);
                }
            } else {
                println!("\n✓ Function '{}' completed (no return values)", function_name);
            }

            self.stats.modules_executed += 1;
        }

        // Fallback simulation for demo/no-std modes
        #[cfg(not(all(feature = "std", feature = "wrt-execution")))]
        {
            let _ = self
                .logger
                .handle_minimal_log(LogLevel::Info, "Simulating execution of function");

            // Validate module structure
            if data.len() < 8 {
                return Err(Error::parse_error("Module too small to be valid WASM"));
            }

            // Check for WASM magic number (0x00 0x61 0x73 0x6D)
            if &data[0..4] != [0x00, 0x61, 0x73, 0x6D] {
                return Err(Error::parse_error("Invalid WASM magic number"));
            }

            // Simulate successful execution
            let _ = self.logger.handle_minimal_log(LogLevel::Info, "Module validation successful");
        }

        Ok(())
    }

    /// Load module data with bounded allocations
    #[cfg(feature = "std")]
    fn load_module_bounded(&self) -> Result<Vec<u8>> {
        const MAX_MODULE_SIZE: usize = 8 * 1024 * 1024; // 8 MiB limit

        if let Some(ref path) = self.config.module_path {
            // Check file size first
            let metadata = fs::metadata(path)
                .map_err(|_| Error::system_io_error("Failed to read module metadata"))?;

            let file_size = metadata.len() as usize;
            if file_size > MAX_MODULE_SIZE {
                return Err(Error::runtime_execution_error("Module file too large"));
            }

            // For safety-critical mode, use bounded allocation
            #[cfg(feature = "safety-critical")]
            {
                let mut module_data: WrtVec<u8, { CrateId::Wrtd as u8 }, MAX_MODULE_SIZE> =
                    WrtVec::new();

                // Read file in chunks to stay within bounds
                let mut file = fs::File::open(path)
                    .map_err(|_| Error::system_io_error("Failed to open module file"))?;

                use std::io::Read;
                let mut buffer = [0u8; 4096];
                loop {
                    let bytes_read = file
                        .read(&mut buffer)
                        .map_err(|_| Error::system_io_error("Failed to read module data"))?;

                    if bytes_read == 0 {
                        break;
                    }

                    for &byte in &buffer[..bytes_read] {
                        module_data.push(byte).map_err(|_| {
                            Error::runtime_execution_error("Module data capacity exceeded")
                        })?;
                    }
                }

                Ok(module_data.into_vec())
            }

            // For non-safety-critical mode, use standard loading but with size check
            #[cfg(not(feature = "safety-critical"))]
            {
                fs::read(path).map_err(|_| Error::system_io_error("Failed to read module"))
            }
        } else if let Some(data) = &self.config.module_data {
            if data.len() > MAX_MODULE_SIZE {
                return Err(Error::runtime_execution_error("Module data too large"));
            }
            Ok(data.to_vec())
        } else {
            Err(Error::parse_error("No module path or data provided"))
        }
    }

    /// Execute a WebAssembly module or component
    pub fn execute_module(&mut self) -> Result<()> {
        let _ = self.logger.handle_minimal_log(LogLevel::Info, "Starting module execution");

        // Determine execution mode and module source with bounded allocations
        #[cfg(feature = "std")]
        let module_data = self.load_module_bounded()?;

        #[cfg(not(feature = "std"))]
        let module_data = self
            .config
            .module_data
            .ok_or_else(|| Error::parse_error("No module data provided for no_std execution"))?;

        // Check if this is a component or module
        let is_component = self.detect_component_format(&module_data)?;

        // Get module size for resource estimation
        #[cfg(feature = "std")]
        let module_size = module_data.len();
        #[cfg(not(feature = "std"))]
        let module_size = module_data.len();

        // Estimate resource usage
        let estimated_fuel = (module_size as u64) / 10; // Conservative estimate
        let estimated_memory = module_size * 2; // Memory overhead estimate

        // Check limits
        if estimated_fuel > self.config.max_fuel {
            return Err(Error::runtime_execution_error(
                "Estimated fuel exceeds maximum limit",
            ));
        }

        if estimated_memory > self.config.max_memory {
            return Err(Error::new(
                ErrorCategory::Resource,
                codes::CAPACITY_EXCEEDED,
                "Estimated memory exceeds maximum limit",
            ));
        }

        // Route execution based on binary type
        if is_component {
            // Execute as WebAssembly component
            #[cfg(feature = "component-model")]
            {
                self.execute_component(&module_data)?;
            }
            #[cfg(not(feature = "component-model"))]
            {
                return Err(Error::runtime_error("Component model support not enabled"));
            }
        } else {
            // Execute as traditional WebAssembly module
            self.execute_traditional_module(&module_data)?;
        }

        // Update statistics
        self.stats.modules_executed += 1;
        self.stats.fuel_consumed += estimated_fuel;
        self.stats.peak_memory = self.stats.peak_memory.max(estimated_memory);

        let _ = self
            .logger
            .handle_minimal_log(LogLevel::Info, "Module execution completed successfully");
        Ok(())
    }

    /// Get current statistics
    pub const fn stats(&self) -> &RuntimeStats {
        &self.stats
    }

    /// Get memory profiler if enabled
    pub fn memory_profiler(&self) -> Option<&MemoryProfiler> {
        self.memory_profiler.as_ref()
    }
}

/// Simple argument parser for minimal dependencies
#[cfg(feature = "std")]
pub struct SimpleArgs {
    /// Module path for std mode
    pub module_path: Option<String>,
    /// Function name to execute
    pub function_name: Option<String>,
    /// Maximum fuel
    pub max_fuel: Option<u64>,
    /// Maximum memory
    pub max_memory: Option<usize>,
    /// Force no-std mode
    pub force_nostd: bool,
    /// Enable WASI support
    #[cfg(feature = "wasi")]
    pub enable_wasi: bool,
    /// WASI version
    #[cfg(feature = "wasi")]
    pub wasi_version: Option<WasiVersion>,
    /// WASI filesystem paths
    #[cfg(feature = "wasi")]
    pub wasi_fs_paths: Vec<String>,
    /// WASI environment variables to expose
    #[cfg(feature = "wasi")]
    pub wasi_env_vars: Vec<String>,
    /// WASI program arguments
    #[cfg(feature = "wasi")]
    pub wasi_args: Vec<String>,
    /// Enable component model
    #[cfg(feature = "component-model")]
    pub enable_component_model: bool,
    /// Component interfaces to register
    #[cfg(feature = "component-model")]
    pub component_interfaces: Vec<String>,
    /// Enable memory profiling
    pub enable_memory_profiling: bool,
    /// Enable platform optimizations
    pub enable_platform_optimizations: bool,
}

#[cfg(feature = "std")]
impl SimpleArgs {
    /// Parse command line arguments without external dependencies
    pub fn parse() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        let mut result = Self {
            module_path: None,
            function_name: None,
            max_fuel: None,
            max_memory: None,
            force_nostd: false,
            #[cfg(feature = "wasi")]
            enable_wasi: true,
            #[cfg(feature = "wasi")]
            wasi_version: None,
            #[cfg(feature = "wasi")]
            wasi_fs_paths: Vec::new(),
            #[cfg(feature = "wasi")]
            wasi_env_vars: Vec::new(),
            #[cfg(feature = "wasi")]
            wasi_args: Vec::new(),
            #[cfg(feature = "component-model")]
            enable_component_model: true,
            #[cfg(feature = "component-model")]
            component_interfaces: Vec::new(),
            enable_memory_profiling: false,
            enable_platform_optimizations: true,
        };

        let mut i = 1; // Skip program name
        while i < args.len() {
            match args[i].as_str() {
                "--help" | "-h" => {
                    println!("WebAssembly Runtime Daemon (wrtd)");
                    println!("Usage: wrtd [OPTIONS] <module.wasm>");
                    println!();
                    println!("Options:");
                    println!("  --function <name>     Function to execute (default: start)");
                    println!("  --fuel <amount>       Maximum fuel limit");
                    println!("  --memory <bytes>      Maximum memory limit");
                    println!("  --no-std             Force no-std execution mode");
                    println!("  --memory-profile     Enable memory profiling");
                    println!("  --no-platform-opt    Disable platform optimizations");
                    #[cfg(feature = "wasi")]
                    {
                        println!("  --wasi               Enable WASI support");
                        println!("  --wasi-version <v>   WASI version (preview2)");
                        println!("  --wasi-fs <path>     Allow filesystem access to path");
                        println!("  --wasi-env <var>     Expose environment variable to WASI");
                        println!("  --wasi-arg <arg>     Pass argument to WASI program");
                    }
                    #[cfg(feature = "component-model")]
                    {
                        println!("  --component          Enable component model support");
                        println!("  --interface <name>   Register component interface");
                    }
                    println!("  --help               Show this help message");
                    process::exit(0);
                },
                "--function" => {
                    i += 1;
                    if i < args.len() {
                        result.function_name = Some(args[i].clone());
                    }
                },
                "--fuel" => {
                    i += 1;
                    if i < args.len() {
                        result.max_fuel = args[i].parse().ok();
                    }
                },
                "--memory" => {
                    i += 1;
                    if i < args.len() {
                        result.max_memory = args[i].parse().ok();
                    }
                },
                "--no-std" => {
                    result.force_nostd = true;
                },
                "--memory-profile" => {
                    result.enable_memory_profiling = true;
                },
                "--no-platform-opt" => {
                    result.enable_platform_optimizations = false;
                },
                #[cfg(feature = "wasi")]
                "--wasi" => {
                    result.enable_wasi = true;
                },
                #[cfg(feature = "wasi")]
                "--wasi-version" => {
                    i += 1;
                    if i < args.len() {
                        result.wasi_version = match args[i].as_str() {
                            "preview2" => Some(WasiVersion::Preview2),
                            _ => None,
                        };
                    }
                },
                #[cfg(feature = "wasi")]
                "--wasi-fs" => {
                    i += 1;
                    if i < args.len() {
                        result.wasi_fs_paths.push(args[i].clone());
                    }
                },
                #[cfg(feature = "wasi")]
                "--wasi-env" => {
                    i += 1;
                    if i < args.len() {
                        result.wasi_env_vars.push(args[i].clone());
                    }
                },
                #[cfg(feature = "wasi")]
                "--wasi-arg" => {
                    i += 1;
                    if i < args.len() {
                        result.wasi_args.push(args[i].clone());
                    }
                },
                #[cfg(feature = "component-model")]
                "--component" => {
                    result.enable_component_model = true;
                },
                #[cfg(feature = "component-model")]
                "--interface" => {
                    i += 1;
                    if i < args.len() {
                        result.component_interfaces.push(args[i].clone());
                    }
                },
                // Everything after "--" goes to wasi_args
                "--" => {
                    #[cfg(feature = "wasi")]
                    {
                        i += 1;
                        while i < args.len() {
                            result.wasi_args.push(args[i].clone());
                            i += 1;
                        }
                    }
                    break;
                },
                arg if !arg.starts_with("--") => {
                    // First non-flag argument is the module path
                    if result.module_path.is_none() {
                        result.module_path = Some(arg.to_string());
                    } else {
                        // Additional positional arguments go to wasi_args
                        #[cfg(feature = "wasi")]
                        result.wasi_args.push(arg.to_string());
                    }
                },
                _ => {}, // Ignore unknown flags
            }
            i += 1;
        }

        Ok(result)
    }
}

/// Main entry point
#[cfg(feature = "std")]
fn main() -> Result<()> {
    // Run main logic in a thread with 32MB stack to handle deep WebAssembly processing
    // This is necessary because Module struct initialization requires significant stack space
    const STACK_SIZE: usize = 32 * 1024 * 1024; // 32MB

    std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(|| main_with_stack())
        .expect("Failed to spawn main thread")
        .join()
        .expect("Main thread panicked")
}

fn main_with_stack() -> Result<()> {
    // Initialize tracing subscriber if tracing feature is enabled
    #[cfg(feature = "tracing")]
    {
        use tracing_subscriber::{EnvFilter, fmt};

        // Set up tracing with environment-based filtering
        // Use RUST_LOG environment variable to control verbosity
        // e.g., RUST_LOG=debug,wrt_runtime=trace
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));

        fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true)
            .init();

        eprintln!("[TRACING] Tracing initialized - use RUST_LOG env var to control output");
    }

    // Parse arguments first to check for --help
    let args = SimpleArgs::parse()?;

    println!("WebAssembly Runtime Daemon (wrtd)");
    println!("===================================");

    // Create configuration from arguments
    let mut config = WrtdConfig::default();
    config.module_path = args.module_path;
    config.function_name = args.function_name;

    if let Some(fuel) = args.max_fuel {
        config.max_fuel = fuel;
    }

    if let Some(memory) = args.max_memory {
        config.max_memory = memory;
    }

    // Apply general configuration options
    config.enable_memory_profiling = args.enable_memory_profiling;
    config.enable_platform_optimizations = args.enable_platform_optimizations;

    // Configure WASI if enabled
    #[cfg(feature = "wasi")]
    {
        config.enable_wasi = args.enable_wasi;
        if let Some(version) = args.wasi_version {
            config.wasi_version = version;
        }

        if config.enable_wasi {
            let mut capabilities = WasiCapabilities::minimal().map_err(|e| {
                eprintln!("Failed to create WASI capabilities: {}", e);
                e
            })?;

            // Add filesystem access paths
            for path in &args.wasi_fs_paths {
                let _ = capabilities.filesystem.add_allowed_path(path);
            }

            // Configure environment variables
            for env_var in &args.wasi_env_vars {
                let _ = capabilities.environment.add_allowed_var(env_var);
            }

            // Enable args access if args are provided
            if !args.wasi_args.is_empty() {
                capabilities.environment.args_access = true;
            }

            // Enable environ access if env vars are specified
            if !args.wasi_env_vars.is_empty() {
                capabilities.environment.environ_access = true;
            }

            config.wasi_capabilities = Some(capabilities);
            config.wasi_env_vars = args.wasi_env_vars.clone();
            config.wasi_args = args.wasi_args.clone();

            println!("✓ WASI enabled:");
            println!("  - Version: {:?}", config.wasi_version);
            println!("  - Filesystem paths: {}", args.wasi_fs_paths.len());
            println!("  - Environment variables: {}", args.wasi_env_vars.len());
            println!("  - Program arguments: {}", args.wasi_args.len());
        }
    }

    // Configure component model if enabled
    #[cfg(feature = "component-model")]
    {
        config.enable_component_model = args.enable_component_model;
        config.component_interfaces = args.component_interfaces.clone();

        if config.enable_component_model {
            println!(
                "✓ Component model enabled with {} interfaces",
                args.component_interfaces.len()
            );
        }
    }

    if config.enable_memory_profiling {
        println!("✓ Memory profiling enabled");
    }

    if !config.enable_platform_optimizations {
        println!("! Platform optimizations disabled");
    }

    // Check if we have a module to execute
    if config.module_path.is_none() {
        println!("Error: No module specified");
        println!("Use --help for usage information");
        process::exit(1);
    }

    // Create and run engine
    let mut engine = WrtdEngine::new(config.clone())?;

    // Set global WASI args for the dispatcher to use
    // The first arg should be the program name (like argv[0] in C)
    #[cfg(feature = "wasi")]
    {
        let module_name = config.module_path.clone().unwrap_or_else(|| "module.wasm".to_string());
        let mut full_args = vec![module_name];
        full_args.extend(config.wasi_args.clone());
        set_global_wasi_args(full_args.clone());
        println!("  - Set {} global WASI args: {:?}", full_args.len(), full_args);
    }

    match engine.execute_module() {
        Ok(()) => {
            let stats = engine.stats();
            println!("✓ Execution completed successfully");
            println!("  Modules executed: {}", stats.modules_executed);
            println!("  Components executed: {}", stats.components_executed);
            println!("  Fuel consumed: {}", stats.fuel_consumed);
            println!("  Peak memory: {} bytes", stats.peak_memory);
            println!(
                "  Host functions registered: {}",
                stats.host_functions_registered
            );
            println!("  WASI functions called: {}", stats.wasi_functions_called);
            println!("  Cross-component calls: {}", stats.cross_component_calls);

            // Display memory profiling if enabled
            if let Some(profiler) = engine.memory_profiler() {
                println!("Memory Profiling:");
                println!("  Peak usage: {} bytes", profiler.peak_usage);
                println!("  Current usage: {} bytes", profiler.current_usage);
            }
        },
        Err(e) => {
            eprintln!("✗ Execution failed: {}", e);
            process::exit(1);
        },
    }

    // Complete memory system initialization
    #[cfg(feature = "std")]
    if let Err(e) = wrt_foundation::memory_init::init_wrt_memory() {
        eprintln!("Warning: Failed to complete memory system: {}", e);
    }

    Ok(())
}

/// Main entry point for no_std mode
#[cfg(not(feature = "std"))]
fn main() {
    // Initialize the allocator if available
    #[cfg(feature = "enable-panic-handler")]
    {
        #[allow(unsafe_code)] // Required for allocator initialization
        unsafe {
            ALLOCATOR.lock().init(HEAP.as_mut_ptr(), HEAP.len());
        }
    }

    // Initialize global memory system for embedded environment
    #[cfg(feature = "std")]
    if let Err(_) = wrt_foundation::memory_init::init_wrt_memory() {
        // Initialize with defaults
        // In no_std, we can't easily print errors, so we enter an error loop
        loop {
            core::hint::spin_loop();
        }
    }

    // In no_std mode, we typically get module data from embedded storage
    // For this demo, we'll use a minimal WASM module
    const DEMO_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6D, // WASM magic
        0x01, 0x00, 0x00, 0x00, // Version 1
    ];

    let mut config = WrtdConfig::default();
    config.module_data = Some(DEMO_MODULE);
    config.function_name = Some("start");
    config.max_fuel = 1000; // Conservative for embedded
    config.max_memory = 4096; // 4KB for embedded

    let mut engine = match WrtdEngine::new(config) {
        Ok(engine) => engine,
        Err(_) => {
            // Engine creation failed, enter error loop
            loop {
                core::hint::spin_loop();
            }
        },
    };

    if let Err(_e) = engine.execute_module() {
        // In no_std mode, we can't easily print errors
        // For embedded applications, this would typically trigger some error handling
        // mechanism For now, we just enter an infinite loop (panic-like
        // behavior)
        loop {
            core::hint::spin_loop();
        }
    }

    // Complete memory system initialization
    #[cfg(feature = "std")]
    let _ = wrt_foundation::memory_init::init_wrt_memory();
}

// Panic handler for no_std builds
#[cfg(all(not(feature = "std"), not(test), feature = "enable-panic-handler"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In real embedded systems, this might:
    // - Write to status registers
    // - Trigger hardware reset
    // - Flash error LED pattern
    loop {}
}
