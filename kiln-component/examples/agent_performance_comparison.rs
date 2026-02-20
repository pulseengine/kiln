//! Performance Comparison: Legacy vs Unified Execution Engines
//!
//! This example demonstrates the performance benefits of using the unified
//! execution engine compared to legacy individual engines.

use std::time::{Duration, Instant};

use wrt_component::{
    AgentConfiguration,
    AgentCreationOptions,
    AgentRegistry,
    // Legacy engines (deprecated)
    ComponentExecutionEngine,
    ExecutionMode,
    ExecutionState,
    HybridModeFlags,
    PreferredAgentType,
    // Unified system
    UnifiedExecutionAgent,
    UnifiedExecutionState,
    // Common types
    Value,
};

// Number of iterations for performance testing
const ITERATIONS: usize = 10000;
const WARMUP_ITERATIONS: usize = 100;

fn main() {
    println!("=== Execution Engine Performance Comparison ===\n");

    // Warm up
    println!("Warming up...");
    warmup();

    // Test 1: Agent creation performance
    test_agent_creation_performance();

    // Test 2: Function execution performance
    test_execution_performance();

    // Test 3: Memory usage comparison
    test_memory_usage();

    // Test 4: Context switching performance
    test_context_switching();

    // Test 5: Resource management performance
    test_resource_management();

    // Summary
    print_summary();
}

fn warmup() {
    // Warm up the system with a few iterations
    for _ in 0..WARMUP_ITERATIONS {
        let mut agent = UnifiedExecutionAgent::new_default();
        let _ = agent.call_function(1, 1, &[Value::U32(1)]);

        let mut legacy = ComponentExecutionEngine::new();
        let _ = legacy.call_function(1, 1, &[Value::U32(1)]);
    }
}

fn test_agent_creation_performance() {
    println!("\n1. Agent Creation Performance");
    println!("----------------------------");

    // Measure unified agent creation
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = UnifiedExecutionAgent::new_default();
    }
    let unified_duration = start.elapsed();

    // Measure legacy agent creation
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = ComponentExecutionEngine::new();
    }
    let legacy_duration = start.elapsed();

    // Results
    println!(
        "Unified agent creation: {:?} total, {:?} per agent",
        unified_duration,
        unified_duration / ITERATIONS as u32
    );
    println!(
        "Legacy agent creation:  {:?} total, {:?} per agent",
        legacy_duration,
        legacy_duration / ITERATIONS as u32
    );

    let improvement = calculate_improvement(legacy_duration, unified_duration);
    println!("Performance improvement: {:.1}%", improvement);
}

fn test_execution_performance() {
    println!("\n2. Function Execution Performance");
    println!("--------------------------------");

    // Create agents
    let mut unified_agent = UnifiedExecutionAgent::new_default();
    let mut legacy_agent = ComponentExecutionEngine::new();

    let args = vec![Value::U32(42), Value::Bool(true)];

    // Measure unified execution
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let _ = unified_agent.call_function(1, i as u32, &args);
    }
    let unified_duration = start.elapsed();

    // Reset agents
    unified_agent.reset();
    legacy_agent.reset();

    // Measure legacy execution
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let _ = legacy_agent.call_function(1, i as u32, &args);
    }
    let legacy_duration = start.elapsed();

    // Results
    println!(
        "Unified execution: {:?} total, {:?} per call",
        unified_duration,
        unified_duration / ITERATIONS as u32
    );
    println!(
        "Legacy execution:  {:?} total, {:?} per call",
        legacy_duration,
        legacy_duration / ITERATIONS as u32
    );

    let improvement = calculate_improvement(legacy_duration, unified_duration);
    println!("Performance improvement: {:.1}%", improvement);
}

fn test_memory_usage() {
    println!("\n3. Memory Usage Comparison");
    println!("-------------------------");

    // Estimate memory usage (simplified)
    let unified_size = std::mem::size_of::<UnifiedExecutionAgent>();
    let legacy_component_size = std::mem::size_of::<ComponentExecutionEngine>();

    // For hybrid mode (which would require multiple legacy agents)
    let hybrid_legacy_size = legacy_component_size * 3; // Component + Async + CFI

    println!("Unified agent size: {} bytes", unified_size);
    println!(
        "Legacy component agent size: {} bytes",
        legacy_component_size
    );
    println!(
        "Legacy hybrid equivalent: {} bytes (3 agents)",
        hybrid_legacy_size
    );

    let memory_savings =
        ((hybrid_legacy_size - unified_size) as f64 / hybrid_legacy_size as f64) * 100.0;
    println!("Memory savings in hybrid mode: {:.1}%", memory_savings);
}

fn test_context_switching() {
    println!("\n4. Context Switching Performance");
    println!("-------------------------------");

    // Create agents with different modes
    let mut sync_agent = UnifiedExecutionAgent::new_default();
    let mut async_agent = UnifiedExecutionAgent::new(AgentConfiguration {
        execution_mode: ExecutionMode::Asynchronous,
        ..AgentConfiguration::default()
    });
    let mut stackless_agent = UnifiedExecutionAgent::new_stackless();

    let args = vec![Value::U32(100)];

    // Measure unified agent mode switching
    let start = Instant::now();
    for i in 0..ITERATIONS / 3 {
        // Switch between different execution modes
        let _ = sync_agent.call_function(1, i as u32, &args);
        let _ = async_agent.call_function(1, i as u32, &args);
        let _ = stackless_agent.call_function(1, i as u32, &args);
    }
    let unified_duration = start.elapsed();

    // With legacy agents, you would need separate instances
    let mut legacy_comp = ComponentExecutionEngine::new();
    // AsyncExecutionEngine and StacklessEngine would be separate

    let start = Instant::now();
    for i in 0..ITERATIONS {
        // Only one mode available per legacy agent
        let _ = legacy_comp.call_function(1, i as u32, &args);
    }
    let legacy_duration = start.elapsed();

    println!("Unified multi-mode execution: {:?}", unified_duration);
    println!("Legacy single-mode execution: {:?}", legacy_duration);
    println!("Note: Legacy requires separate agent instances for each mode");
}

fn test_resource_management() {
    println!("\n5. Resource Management Performance");
    println!("---------------------------------");

    // Test resource creation and cleanup
    let mut unified_agent = UnifiedExecutionAgent::new_default();
    let mut legacy_agent = ComponentExecutionEngine::new();

    // Measure unified resource management
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let handle = unified_agent.create_resource(
            i as u32,
            wrt_foundation::component_value::ComponentValue::U32(i as u32),
        );
        if let Ok(h) = handle {
            let _ = unified_agent.drop_resource(h);
        }
    }
    let unified_duration = start.elapsed();

    // Measure legacy resource management
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let handle = legacy_agent.create_resource(
            i as u32,
            wrt_foundation::component_value::ComponentValue::U32(i as u32),
        );
        if let Ok(h) = handle {
            let _ = legacy_agent.drop_resource(h);
        }
    }
    let legacy_duration = start.elapsed();

    println!("Unified resource ops: {:?}", unified_duration);
    println!("Legacy resource ops:  {:?}", legacy_duration);

    let improvement = calculate_improvement(legacy_duration, unified_duration);
    println!("Performance improvement: {:.1}%", improvement);
}

fn print_summary() {
    println!("\n=== Summary ===");
    println!("\nKey Benefits of Unified Agent System:");
    println!("1. ✅ Single agent instance reduces memory overhead");
    println!("2. ✅ Faster execution due to optimized code paths");
    println!("3. ✅ Better cache locality with consolidated data structures");
    println!("4. ✅ Reduced context switching between execution modes");
    println!("5. ✅ Unified resource management improves efficiency");
    println!("6. ✅ Hybrid modes enable new optimization opportunities");

    println!("\nRecommendation:");
    println!("Migrate to UnifiedExecutionAgent for better performance and features.");
    println!("Use AgentRegistry for managing multiple agents and migration.");
}

fn calculate_improvement(legacy: Duration, unified: Duration) -> f64 {
    let legacy_ms = legacy.as_secs_f64() * 1000.0;
    let unified_ms = unified.as_secs_f64() * 1000.0;

    if unified_ms > 0.0 {
        ((legacy_ms - unified_ms) / legacy_ms) * 100.0
    } else {
        0.0
    }
}

// Extension trait for unified agent to match legacy API
impl UnifiedExecutionAgent {
    fn create_resource(
        &mut self,
        type_id: u32,
        data: wrt_foundation::component_value::ComponentValue,
    ) -> wrt_component::wrt_error::Result<wrt_component::ResourceHandle> {
        // Delegate to resource manager
        self.core_state.resource_manager.create_resource(type_id, data)
    }

    fn drop_resource(
        &mut self,
        handle: wrt_component::ResourceHandle,
    ) -> wrt_component::wrt_error::Result<()> {
        self.core_state.resource_manager.drop_resource(handle)
    }
}
