# Engine Architecture Documentation

## Overview

The WRT engine architecture provides a layered approach to WebAssembly execution with clear separation of concerns between core functionality and optional capabilities.

## Engine Hierarchy

```
┌─────────────────────────────────────────┐
│            Application Layer            │
├─────────────────────────────────────────┤
│          Capability Layer               │
│  ┌─────────────────┐ ┌─────────────────┐│
│  │ CapabilityAware │ │  WastEngine     ││
│  │    Engine       │ │                 ││
│  └─────────────────┘ └─────────────────┘│
├─────────────────────────────────────────┤
│             Core Engine Layer           │
│  ┌─────────────────┐ ┌─────────────────┐│
│  │  StacklessEngine│ │   CoreEngine    ││
│  │                 │ │                 ││
│  └─────────────────┘ └─────────────────┘│
├─────────────────────────────────────────┤
│            Foundation Layer             │
│  ┌─────────────────┐ ┌─────────────────┐│
│  │ Memory Manager  │ │ Module System   ││
│  │                 │ │                 ││
│  └─────────────────┘ └─────────────────┘│
└─────────────────────────────────────────┘
```

## Core Components

### 1. StacklessEngine (Core)
- **Purpose**: Pure WebAssembly execution without capability overhead
- **Use Cases**: Testing, benchmarking, minimal deployments
- **Dependencies**: Only foundation components
- **Memory**: Direct allocation without capability checks
- **Performance**: Highest - no capability verification overhead

### 2. CapabilityAwareEngine (Enhanced)
- **Purpose**: Production engine with full capability system
- **Use Cases**: Production deployments, security-critical environments
- **Dependencies**: Core engine + capability system
- **Memory**: Capability-verified allocation
- **Performance**: Production-ready with security guarantees

### 3. WastEngine (Testing)
- **Purpose**: WAST test execution and validation
- **Use Cases**: Test suite execution, compliance verification
- **Dependencies**: Core engine + WAST parser
- **Memory**: Test-optimized allocation
- **Performance**: Test-focused with comprehensive reporting

## Factory Pattern Implementation

### Engine Factory
```rust
pub enum EngineType {
    Stackless,
    CapabilityAware,
    Wast,
}

pub struct EngineFactory;

impl EngineFactory {
    pub fn create(engine_type: EngineType) -> Result<Box<dyn Engine>> {
        match engine_type {
            EngineType::Stackless => Ok(Box::new(StacklessEngine::new()?)),
            EngineType::CapabilityAware => Ok(Box::new(CapabilityAwareEngine::new()?)),
            EngineType::Wast => Ok(Box::new(WastEngine::new()?)),
        }
    }
}
```

### Memory Provider Factory
```rust
pub enum MemoryProviderType {
    Basic,
    CapabilityAware,
    Test,
}

pub struct MemoryProviderFactory;

impl MemoryProviderFactory {
    pub fn create(provider_type: MemoryProviderType) -> Result<Box<dyn MemoryProvider>> {
        match provider_type {
            MemoryProviderType::Basic => {
                let provider = safe_managed_alloc!(65536, CrateId::Runtime)?;
                Ok(Box::new(provider))
            },
            MemoryProviderType::CapabilityAware => {
                Ok(Box::new(CapabilityWrtFactory::new_with_budget()?))
            },
            MemoryProviderType::Test => {
                let provider = safe_managed_alloc!(131072, CrateId::Test)?;
                Ok(Box::new(provider))
            }
        }
    }
}
```

## Separation of Concerns

### Core Engine Responsibilities
- ✅ WebAssembly module loading and validation
- ✅ Instruction execution and interpretation
- ✅ Memory management (basic)
- ✅ Function calls and stack management
- ✅ Value conversion and type checking

### Capability Layer Responsibilities
- ✅ Security policy enforcement
- ✅ Resource quota management  
- ✅ Access control verification
- ✅ Audit logging and monitoring
- ✅ Multi-tenant isolation

### Testing Layer Responsibilities
- ✅ WAST directive processing
- ✅ Test result validation
- ✅ Compliance verification
- ✅ Performance benchmarking
- ✅ Error analysis and reporting

## Initialization Patterns

### 1. Lazy Initialization (Recommended)
```rust
pub struct LazyEngine {
    engine: Option<Box<dyn Engine>>,
    engine_type: EngineType,
}

impl LazyEngine {
    pub fn new(engine_type: EngineType) -> Self {
        Self { engine: None, engine_type }
    }
    
    fn get_or_create(&mut self) -> Result<&mut dyn Engine> {
        if self.engine.is_none() {
            self.engine = Some(EngineFactory::create(self.engine_type)?);
        }
        Ok(self.engine.as_mut().unwrap().as_mut())
    }
}
```

### 2. Early Initialization
```rust
pub fn initialize_engine_system() -> Result<()> {
    // Initialize memory system first
    wrt_foundation::memory::initialize()?;
    
    // Pre-create engine instances
    ENGINE_POOL.lock()?.initialize()?;
    
    Ok(())
}
```

## Usage Guidelines

### When to Use StacklessEngine
- ✅ Unit testing and benchmarking
- ✅ Embedded environments with memory constraints
- ✅ High-performance scenarios without security requirements
- ✅ Development and debugging

### When to Use CapabilityAwareEngine  
- ✅ Production deployments
- ✅ Multi-tenant environments
- ✅ Security-critical applications
- ✅ Compliance-required scenarios (ASIL-D)

### When to Use WastEngine
- ✅ Test suite execution
- ✅ Compliance verification
- ✅ Educational and research purposes
- ✅ WebAssembly specification validation

## Performance Characteristics

| Engine Type | Startup Time | Memory Usage | Execution Speed | Security Level |
|-------------|--------------|--------------|-----------------|----------------|
| Stackless   | Fast         | Minimal      | Fastest         | Basic          |
| Capability  | Medium       | Moderate     | Fast            | High           |
| Wast        | Medium       | High         | Medium          | Test-focused   |

## Future Architecture Improvements

### Planned Enhancements
1. **Plugin Architecture**: Modular capability plugins
2. **Engine Pools**: Pre-allocated engine instances
3. **Hot Swapping**: Runtime engine type switching
4. **Telemetry Integration**: Built-in performance monitoring
5. **Async Support**: Full async/await engine variants

### API Stability
- ✅ Core engine API: Stable
- ✅ Factory patterns: Stable  
- ⚠️  Capability API: Evolving
- ⚠️  WAST API: Extending

This architecture provides a solid foundation for scalable, maintainable, and testable WebAssembly runtime development.