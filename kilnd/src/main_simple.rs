//! # WebAssembly Runtime Daemon (wrtd) - Simple Demo
//!
//! A demonstration of how wrtd works with different runtime modes.
//! This is a simplified version showing the concepts without full wrt integration.

#![cfg_attr(not(feature = "std-runtime"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Conditional imports based on runtime mode
#[cfg(feature = "std-runtime")]
use std::{
    env, 
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

#[cfg(feature = "std-runtime")]
use crate::bounded_wrtd_infra::{
    BoundedServiceMap, BoundedLogEntryVec, WrtdProvider,
    new_service_map, new_log_entry_vec
};

#[cfg(feature = "alloc-runtime")]
extern crate alloc;

#[cfg(feature = "alloc-runtime")]
use std::{
    string::{String, ToString},
};

#[cfg(feature = "alloc-runtime")]
use crate::bounded_wrtd_infra::{
    BoundedServiceMap, BoundedLogEntryVec, WrtdProvider,
    new_service_map, new_log_entry_vec
};

#[cfg(feature = "nostd-runtime")]
use heapless::{
    String,
    FnvIndexMap as Map,
};

#[cfg(feature = "nostd-runtime")]
use crate::bounded_wrtd_infra::{
    BoundedLogEntryVec, WrtdProvider, new_log_entry_vec
};

/// Configuration for the runtime daemon
#[derive(Debug, Clone)]
pub struct WrtdConfig {
    /// Maximum fuel (execution steps) allowed
    pub max_fuel: u64,
    /// Maximum memory usage in bytes
    pub max_memory: usize,
    /// Enable verbose logging
    pub verbose: bool,
}

impl Default for WrtdConfig {
    fn default() -> Self {
        Self {
            max_fuel: 10_000,
            max_memory: 64 * 1024, // 64KB
            verbose: false,
        }
    }
}

/// Runtime statistics
#[derive(Debug, Clone, Default)]
pub struct RuntimeStats {
    /// Modules executed
    pub modules_executed: u32,
    /// Total fuel consumed
    pub fuel_consumed: u64,
    /// Peak memory usage
    pub peak_memory: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Standard runtime implementation with full std library support
#[cfg(feature = "std-runtime")]
pub mod std_runtime {
    use super::*;
    
    /// Standard runtime with full std support
    pub struct StdRuntime {
        config: WrtdConfig,
        stats: Mutex<RuntimeStats>,
        module_cache: Mutex<BoundedServiceMap<BoundedLogEntryVec<u8>>>,
    }
    
    impl StdRuntime {
        /// Create a new standard runtime
        pub fn new(config: WrtdConfig) -> Self {
            Self {
                config,
                stats: Mutex::new(RuntimeStats::default()),
                module_cache: Mutex::new(new_service_map()),
            }
        }
        
        /// Execute a WebAssembly module (placeholder implementation)
        pub fn execute_module(&self, module_path: &str, function: &str) -> Result<String, String> {
            let start = Instant::now();
            
            // Load module (placeholder)
            let module_bytes = match fs::read(module_path) {
                Ok(bytes) => bytes,
                Err(e) => return Err(format!("Failed to read module: {}", e)),
            };
            
            if self.config.verbose {
                println!("Loaded module: {} bytes", module_bytes.len();
            }
            
            // Cache the module
            {
                let mut cache = self.module_cache.lock().unwrap();
                cache.insert(module_path.to_string(), module_bytes.clone();
            }
            
            // Simulate execution
            let fuel_used = module_bytes.len() as u64 / 100; // Rough estimate
            let memory_used = module_bytes.len() * 2; // Estimate
            
            if fuel_used > self.config.max_fuel {
                return Err(format!("Fuel limit exceeded: {} > {}", fuel_used, self.config.max_fuel;
            }
            
            if memory_used > self.config.max_memory {
                return Err(format!("Memory limit exceeded: {} > {}", memory_used, self.config.max_memory;
            }
            
            // Update stats
            {
                let mut stats = self.stats.lock().unwrap();
                stats.modules_executed += 1;
                stats.fuel_consumed += fuel_used;
                stats.peak_memory = stats.peak_memory.max(memory_used;
                stats.execution_time_ms += start.elapsed().as_millis() as u64;
            }
            
            Ok(format!("Executed function '{}' successfully. Fuel used: {}, Memory used: {}", 
                      function, fuel_used, memory_used))
        }
        
        /// Get runtime statistics
        pub fn stats(&self) -> RuntimeStats {
            self.stats.lock().unwrap().clone()
        }
        
        /// List cached modules
        pub fn list_modules(&self) -> Vec<String> {
            self.module_cache.lock().unwrap().keys().cloned().collect()
        }
    }
    
    /// Main function for std runtime
    pub fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
        println!("WRT Daemon - Standard Runtime Mode");
        println!("=================================");
        
        let args: Vec<String> = env::args().collect();
        if args.len() < 3 {
            println!("Usage: {} <module.wasm> <function>", args[0]);
            return Ok();
        }
        
        let module_path = &args[1];
        let function = &args[2];
        
        let config = WrtdConfig {
            max_fuel: 1_000_000,
            max_memory: 64 * 1024 * 1024, // 64MB
            verbose: args.contains(&"--verbose".to_string()),
        };
        
        let runtime = StdRuntime::new(config;
        
        match runtime.execute_module(module_path, function) {
            Ok(result) => {
                println!("✓ {}", result);
                let stats = runtime.stats);
                println!("📊 Stats: {} modules, {} fuel, {}KB peak memory", 
                        stats.modules_executed, stats.fuel_consumed, stats.peak_memory / 1024;
            }
            Err(e) => {
                eprintln!("✗ Error: {}", e);
                std::process::exit(1;
            }
        }
        
        Ok(())
    }
}

/// Binary std/no_std choice
#[cfg(feature = "alloc-runtime")]
pub mod alloc_runtime {
    use super::*;
    
    /// Binary std/no_std choice
    pub struct AllocRuntime {
        config: WrtdConfig,
        stats: RuntimeStats,
        module_cache: BoundedServiceMap<BoundedLogEntryVec<u8>>,
    }
    
    impl AllocRuntime {
        /// Binary std/no_std choice
        pub fn new(config: WrtdConfig) -> Self {
            Self {
                config,
                stats: RuntimeStats::default(),
                module_cache: new_service_map(),
            }
        }
        
        /// Execute a WebAssembly module (placeholder implementation)
        pub fn execute_module(&mut self, module_data: &[u8], function: &str) -> Result<String, String> {
            if self.config.verbose {
                // Binary std/no_std choice
                // For now, just proceed silently
            }
            
            let fuel_used = module_data.len() as u64 / 50; // More conservative than std
            let memory_used = module_data.len();
            
            if fuel_used > self.config.max_fuel {
                return Err("Fuel limit exceeded".to_string());
            }
            
            if memory_used > self.config.max_memory {
                return Err("Memory limit exceeded".to_string());
            }
            
            // Update stats
            self.stats.modules_executed += 1;
            self.stats.fuel_consumed += fuel_used;
            self.stats.peak_memory = self.stats.peak_memory.max(memory_used;
            
            Ok(format!("Executed '{}' (alloc mode)", function))
        }
        
        /// Get runtime statistics
        pub fn stats(&self) -> &RuntimeStats {
            &self.stats
        }
    }
    
    /// Binary std/no_std choice
    pub fn main() -> Result<(), &'static str> {
        // Binary std/no_std choice
        // This would typically be called with pre-loaded module data
        
        let config = WrtdConfig {
            max_fuel: 100_000,
            max_memory: 1024 * 1024, // 1MB
            verbose: false,
        };
        
        let mut runtime = AllocRuntime::new(config;
        
        // Simulate a small WASM module
        let fake_module = vec![0x00, 0x61, 0x73, 0x6d]; // WASM magic number
        
        match runtime.execute_module(&fake_module, "start") {
            Ok(_) => {
                let stats = runtime.stats);
                // Success - in real implementation this would signal back to host
                Ok(())
            }
            Err(_) => Err("Execution failed"),
        }
    }
}

/// No-std runtime implementation for bare metal systems
#[cfg(feature = "nostd-runtime")]
pub mod nostd_runtime {
    use super::*;
    
    /// No-std runtime for bare metal systems
    pub struct NoStdRuntime {
        config: WrtdConfig,
        stats: RuntimeStats,
        // Using bounded collections with fixed capacity
        execution_log: BoundedLogEntryVec<u8>, // Log execution events
    }
    
    impl NoStdRuntime {
        /// Create a new no-std runtime
        pub fn new(config: WrtdConfig) -> Self {
            Self {
                config,
                stats: RuntimeStats::default(),
                execution_log: new_log_entry_vec(),
            }
        }
        
        /// Execute a WebAssembly module (placeholder implementation)
        pub fn execute_module(&mut self, module_data: &[u8], _function: &str) -> Result<u32, u8> {
            // Very conservative limits for bare metal
            let fuel_used = module_data.len() as u64 / 10;
            let memory_used = module_data.len();
            
            if fuel_used > self.config.max_fuel {
                return Err(1); // Error code: fuel exceeded
            }
            
            if memory_used > self.config.max_memory {
                return Err(2); // Error code: memory exceeded
            }
            
            // Log execution (with fixed capacity)
            let _ = self.execution_log.push(1); // Log execution event
            
            // Update stats
            self.stats.modules_executed += 1;
            self.stats.fuel_consumed += fuel_used;
            self.stats.peak_memory = self.stats.peak_memory.max(memory_used;
            
            Ok(fuel_used as u32)
        }
        
        /// Get runtime statistics
        pub fn stats(&self) -> &RuntimeStats {
            &self.stats
        }
    }
    
    /// Main function for no-std runtime
    pub fn main() -> Result<(), u8> {
        let config = WrtdConfig {
            max_fuel: 10_000,
            max_memory: 64 * 1024, // 64KB
            verbose: false,
        };
        
        let mut runtime = NoStdRuntime::new(config;
        
        // Simulate a tiny WASM module for bare metal
        let fake_module = [0x00, 0x61, 0x73, 0x6d]; // WASM magic number
        
        match runtime.execute_module(&fake_module, "start") {
            Ok(fuel_used) => {
                // Success - in bare metal this might set a status register
                Ok(())
            }
            Err(error_code) => Err(error_code),
        }
    }
}

// Main entry point - delegates to the appropriate runtime
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "std-runtime")]
    {
        std_runtime::main()
    }
    
    #[cfg(feature = "alloc-runtime")]
    {
        alloc_runtime::main().map_err(|e| e.into())
    }
    
    #[cfg(feature = "nostd-runtime")]
    {
        nostd_runtime::main().map_err(|e| format!("Error code: {}", e))?;
        Ok(())
    }
}