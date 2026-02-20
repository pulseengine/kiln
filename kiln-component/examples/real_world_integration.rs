//! Real-World Integration Example
//!
//! This example demonstrates how to integrate the unified execution engine
//! into a practical WebAssembly application with multiple components.

use std::collections::HashMap;

use wrt_component::{
    AgentConfiguration, AgentId, AgentRegistry, ExecutionMode, HybridModeFlags,
    RuntimeBridgeConfig, UnifiedExecutionAgent, UnifiedExecutionState, Value,
};

/// A WebAssembly application manager using unified execution engines
pub struct WasmApplicationManager {
    /// Registry for managing execution engines
    agent_registry: AgentRegistry,

    /// Mapping of component names to engine IDs
    component_agents: HashMap<String, AgentId>,

    /// Application configuration
    config: ApplicationConfig,
}

/// Configuration for the WebAssembly application
#[derive(Debug, Clone)]
pub struct ApplicationConfig {
    /// Maximum memory per component (bytes)
    pub max_memory_per_component: usize,

    /// Maximum call depth
    pub max_call_depth: usize,

    /// Enable async execution
    pub enable_async: bool,

    /// Enable CFI protection
    pub enable_cfi: bool,

    /// Enable memory optimization (stackless)
    pub enable_memory_optimization: bool,

    /// Execution timeout (milliseconds)
    pub execution_timeout_ms: u64,
}

/// Application component types
#[derive(Debug, Clone)]
pub enum ComponentType {
    /// User interface component
    UserInterface,

    /// Business logic component
    BusinessLogic,

    /// Data processing component
    DataProcessing,

    /// I/O operations component
    IoOperations,

    /// Security-critical component
    SecurityCritical,
}

/// Component execution result
#[derive(Debug)]
pub struct ComponentResult {
    pub component_name: String,
    pub execution_time_ms: u64,
    pub memory_used: usize,
    pub result_value: Value,
    pub success: bool,
}

impl WasmApplicationManager {
    /// Create a new application manager
    pub fn new(config: ApplicationConfig) -> Self {
        Self {
            agent_registry: AgentRegistry::new(),
            component_agents: HashMap::new(),
            config,
        }
    }

    /// Register a WebAssembly component
    pub fn register_component(
        &mut self,
        name: String,
        component_type: ComponentType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Registering component '{}' of type {:?}",
            name, component_type
        );

        // Choose appropriate execution mode based on component type
        let execution_mode = self.determine_execution_mode(&component_type);

        // Create agent configuration
        let agent_config = AgentConfiguration {
            max_memory: self.config.max_memory_per_component,
            max_call_depth: self.config.max_call_depth,
            execution_mode,
            bounded_execution: true,
            initial_fuel: Some(10000), // Prevent infinite loops
            runtime_config: RuntimeBridgeConfig::default(),
        };

        // Create unified agent for this component
        let agent_id = self.agent_registry.create_unified_agent(agent_config)?;

        // Store the mapping
        self.component_agents.insert(name.clone(), agent_id);

        println!(
            "Successfully registered component '{}' with agent ID {:?}",
            name, agent_id
        );
        Ok(())
    }

    /// Execute a function in a specific component
    pub fn execute_component_function(
        &mut self,
        component_name: &str,
        function_name: &str,
        args: Vec<Value>,
    ) -> Result<ComponentResult, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();

        // Get the agent for this component
        let agent_id = self
            .component_agents
            .get(component_name)
            .ok_or_else(|| format!("Component '{}' not found", component_name))?;

        println!(
            "Executing function '{}' in component '{}'",
            function_name, component_name
        );

        // Simple function name to index mapping (in real app, this would be more
        // sophisticated)
        let function_index = self.function_name_to_index(function_name);
        let instance_id = 1; // Simplified for demo

        // Execute the function
        let result =
            self.agent_registry.call_function(*agent_id, instance_id, function_index, &args);

        let execution_time = start_time.elapsed();

        // Create result
        let component_result = ComponentResult {
            component_name: component_name.to_string(),
            execution_time_ms: execution_time.as_millis() as u64,
            memory_used: 0, // Would be tracked by agent in real implementation
            result_value: result.unwrap_or(Value::Bool(false)),
            success: result.is_ok(),
        };

        if component_result.success {
            println!(
                "✅ Function '{}' executed successfully in {}ms",
                function_name, component_result.execution_time_ms
            );
        } else {
            println!("❌ Function '{}' failed", function_name);
        }

        Ok(component_result)
    }

    /// Execute a complex workflow across multiple components
    pub fn execute_workflow(
        &mut self,
        workflow_name: &str,
        input_data: Value,
    ) -> Result<Vec<ComponentResult>, Box<dyn std::error::Error>> {
        println!("\n🚀 Executing workflow: {}", workflow_name);

        let mut results = Vec::new();

        match workflow_name {
            "user_data_processing" => {
                // Step 1: Validate input in security component
                results.push(self.execute_component_function(
                    "security",
                    "validate_input",
                    vec![input_data.clone()],
                )?);

                // Step 2: Process data in business logic component
                results.push(self.execute_component_function(
                    "business_logic",
                    "process_user_data",
                    vec![input_data.clone()],
                )?);

                // Step 3: Store results in data component
                results.push(self.execute_component_function(
                    "data_processing",
                    "store_processed_data",
                    vec![Value::String("processed_data".to_string())],
                )?);

                // Step 4: Update UI
                results.push(self.execute_component_function(
                    "ui",
                    "update_display",
                    vec![Value::Bool(true)],
                )?);
            },

            "batch_data_processing" => {
                // Use async execution for batch processing
                results.push(self.execute_component_function(
                    "data_processing",
                    "start_batch_job",
                    vec![input_data],
                )?);

                // Simulate monitoring the batch job
                for i in 0..3 {
                    results.push(self.execute_component_function(
                        "data_processing",
                        "check_batch_status",
                        vec![Value::U32(i)],
                    )?);
                }
            },

            _ => return Err(format!("Unknown workflow: {}", workflow_name).into()),
        }

        let total_time: u64 = results.iter().map(|r| r.execution_time_ms).sum();
        let success_count = results.iter().filter(|r| r.success).count();

        println!("📊 Workflow '{}' completed:", workflow_name);
        println!("   Total execution time: {}ms", total_time);
        println!("   Successful steps: {}/{}", success_count, results.len());

        Ok(results)
    }

    /// Get application statistics
    pub fn get_statistics(&self) -> ApplicationStatistics {
        let registry_stats = self.agent_registry.statistics();
        let migration_stats = self.agent_registry.migration_status();

        ApplicationStatistics {
            total_components: self.component_agents.len(),
            active_agents: registry_stats.active_agents,
            unified_agents: registry_stats.unified_agents_created,
            legacy_agents: registry_stats.legacy_agents_created,
            completed_migrations: migration_stats.completed_migrations,
        }
    }

    /// Migrate all legacy agents to unified (if any)
    pub fn migrate_legacy_agents(&mut self) -> Result<u32, Box<dyn std::error::Error>> {
        println!("🔄 Migrating legacy agents to unified...");
        let migrated = self.agent_registry.migrate_all_agents()?;
        println!("✅ Migrated {} agents", migrated);
        Ok(migrated)
    }

    // Private helper methods

    fn determine_execution_mode(&self, component_type: &ComponentType) -> ExecutionMode {
        match component_type {
            ComponentType::UserInterface => {
                // UI components benefit from async execution
                if self.config.enable_async {
                    ExecutionMode::Asynchronous
                } else {
                    ExecutionMode::Synchronous
                }
            },

            ComponentType::BusinessLogic => {
                // Business logic can use hybrid mode for flexibility
                ExecutionMode::Hybrid(HybridModeFlags {
                    async_enabled: self.config.enable_async,
                    stackless_enabled: false,
                    cfi_enabled: false,
                })
            },

            ComponentType::DataProcessing => {
                // Data processing benefits from memory optimization
                if self.config.enable_memory_optimization {
                    ExecutionMode::Stackless
                } else {
                    ExecutionMode::Synchronous
                }
            },

            ComponentType::IoOperations => {
                // I/O operations are typically async
                if self.config.enable_async {
                    ExecutionMode::Asynchronous
                } else {
                    ExecutionMode::Synchronous
                }
            },

            ComponentType::SecurityCritical => {
                // Security components need CFI protection
                if self.config.enable_cfi {
                    ExecutionMode::CfiProtected
                } else {
                    ExecutionMode::Hybrid(HybridModeFlags {
                        async_enabled: false,
                        stackless_enabled: true,
                        cfi_enabled: false,
                    })
                }
            },
        }
    }

    fn function_name_to_index(&self, function_name: &str) -> u32 {
        // Simple hash-based mapping (in real app, use proper function registry)
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };

        let mut hasher = DefaultHasher::new();
        function_name.hash(&mut hasher);
        (hasher.finish() % 1000) as u32
    }
}

/// Application performance statistics
#[derive(Debug)]
pub struct ApplicationStatistics {
    pub total_components: usize,
    pub active_agents: u32,
    pub unified_agents: u32,
    pub legacy_agents: u32,
    pub completed_migrations: u32,
}

impl Default for ApplicationConfig {
    fn default() -> Self {
        Self {
            max_memory_per_component: 1024 * 1024, // 1MB
            max_call_depth: 128,
            enable_async: true,
            enable_cfi: false, // Enable in production for security-critical apps
            enable_memory_optimization: true,
            execution_timeout_ms: 5000, // 5 seconds
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Real-World WebAssembly Application Example ===\n");

    // Create application manager with configuration
    let config = ApplicationConfig {
        enable_cfi: true, // Enable CFI protection for demo
        ..ApplicationConfig::default()
    };

    let mut app_manager = WasmApplicationManager::new(config);

    // Register application components
    app_manager.register_component("ui".to_string(), ComponentType::UserInterface)?;

    app_manager.register_component("business_logic".to_string(), ComponentType::BusinessLogic)?;

    app_manager.register_component("data_processing".to_string(), ComponentType::DataProcessing)?;

    app_manager.register_component("security".to_string(), ComponentType::SecurityCritical)?;

    // Show initial statistics
    let stats = app_manager.get_statistics();
    println!("\n📈 Initial Statistics:");
    println!("   Components registered: {}", stats.total_components);
    println!("   Active agents: {}", stats.active_agents);
    println!("   Unified agents: {}", stats.unified_agents);

    // Execute individual component functions
    println!("\n🔧 Testing Individual Components:");

    let validation_result = app_manager.execute_component_function(
        "security",
        "validate_user_input",
        vec![Value::String("test_input".to_string())],
    )?;

    let business_result = app_manager.execute_component_function(
        "business_logic",
        "calculate_result",
        vec![Value::U32(42), Value::F64(3.14)],
    )?;

    // Execute complex workflows
    println!("\n🏗️ Executing Complex Workflows:");

    let workflow_results = app_manager.execute_workflow(
        "user_data_processing",
        Value::String("user_data_payload".to_string()),
    )?;

    let batch_results = app_manager.execute_workflow("batch_data_processing", Value::U32(1000))?;

    // Show final statistics
    let final_stats = app_manager.get_statistics();
    println!("\n📊 Final Statistics:");
    println!("   Total components: {}", final_stats.total_components);
    println!("   Active agents: {}", final_stats.active_agents);
    println!("   Unified agents: {}", final_stats.unified_agents);
    println!("   Legacy agents: {}", final_stats.legacy_agents);

    println!("\n✅ Application completed successfully!");
    println!("\nKey Benefits Demonstrated:");
    println!("  🔹 Unified agents handle different component types seamlessly");
    println!("  🔹 Execution modes are automatically chosen based on component requirements");
    println!("  🔹 Complex workflows coordinate multiple components efficiently");
    println!("  🔹 Security-critical components get appropriate protection (CFI)");
    println!("  🔹 Memory-intensive components use stackless execution");
    println!("  🔹 UI components use async execution for responsiveness");

    Ok(())
}
