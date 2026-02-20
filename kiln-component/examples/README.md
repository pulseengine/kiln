# WRT Unified Execution Agent Examples

This directory contains examples demonstrating the unified execution agent system in the WRT (WebAssembly Runtime) project.

## Overview

The unified execution agent system consolidates multiple specialized execution engines into a single, configurable agent that supports:

- **Synchronous execution** - Traditional WebAssembly component execution
- **Asynchronous execution** - Non-blocking operations and concurrent tasks
- **Stackless execution** - Memory-efficient execution for constrained environments
- **CFI-protected execution** - Control Flow Integrity for security-critical applications
- **Hybrid execution** - Combining multiple execution capabilities

## Examples

### 1. `unified_agent_demo.rs`
**Basic demonstration of unified agent capabilities**

Shows how to:
- Create agents with different execution modes
- Execute functions in sync, async, stackless, and CFI-protected modes
- Use hybrid execution combining multiple capabilities
- Manage agents through the AgentRegistry
- Migrate from legacy agents to unified agents

**Run with:**
```bash
cargo run --example unified_agent_demo
```

### 2. `agent_performance_comparison.rs`
**Performance comparison between legacy and unified agents**

Demonstrates:
- Agent creation performance improvements
- Function execution speed comparison
- Memory usage reduction in hybrid modes
- Context switching efficiency
- Resource management performance

**Run with:**
```bash
cargo run --example agent_performance_comparison --release
```

### 3. `real_world_integration.rs`
**Practical application using unified agents**

Shows a complete WebAssembly application with:
- Multiple components with different execution requirements
- Automatic execution mode selection based on component type
- Complex workflows coordinating multiple components
- Statistics tracking and monitoring
- Legacy agent migration

**Run with:**
```bash
cargo run --example real_world_integration
```

## Key Benefits Demonstrated

✅ **Reduced Code Duplication** - Single agent replaces multiple specialized engines  
✅ **Improved Performance** - Optimized execution paths and reduced memory overhead  
✅ **Enhanced Flexibility** - Hybrid modes combine multiple capabilities  
✅ **Simplified API** - Consistent interface across all execution types  
✅ **Better Maintainability** - Single codebase for all execution logic  
✅ **Seamless Migration** - Automated tools for legacy agent transition  

## Migration from Legacy Agents

The unified system provides a clear migration path from individual legacy agents:

**Legacy → Unified Mapping:**
- `ComponentExecutionEngine` → `ExecutionMode::Synchronous`
- `AsyncExecutionEngine` → `ExecutionMode::Asynchronous`  
- `StacklessEngine` → `ExecutionMode::Stackless`
- `CfiExecutionEngine` → `ExecutionMode::CfiProtected`
- Multiple agents → `ExecutionMode::Hybrid`

## Getting Started

1. **Basic Usage:**
```rust
use wrt_component::{UnifiedExecutionAgent, AgentConfiguration};

let agent = UnifiedExecutionAgent::new(AgentConfiguration::default());
```

2. **With Agent Registry:**
```rust
use wrt_component::{AgentRegistry, AgentCreationOptions, PreferredAgentType};

let mut registry = AgentRegistry::new();
let agent_id = registry.create_agent(AgentCreationOptions::default())?;
```

3. **Hybrid Mode:**
```rust
use wrt_component::{ExecutionMode, HybridModeFlags};

let agent = UnifiedExecutionAgent::new_hybrid(HybridModeFlags {
    async_enabled: true,
    stackless_enabled: true,
    cfi_enabled: true,
});
```

See the [Migration Guide](../AGENT_MIGRATION_GUIDE.md) for complete migration instructions.