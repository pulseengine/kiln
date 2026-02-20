# WebAssembly Component Model Implementation Plan

## Executive Summary

This document provides a comprehensive implementation plan for proper Component Model support in Kiln. The current implementation is **Flickschusterei** (patchwork) - infrastructure exists but execution paths are hollow. Hello World works only because it uses primitives that bypass the Component Model entirely.

### Current State Assessment

| Component | Status | Reality |
|-----------|--------|---------|
| Canonical ABI (lift/lower) | EXISTS | **NEVER CALLED** - execution bypasses it |
| Component Instantiation | EXISTS | Core modules run directly, no component linking |
| Type Validation | EXISTS | Infrastructure defined, never validates at call boundaries |
| Resource System | EXISTS | Just integer tags, no ownership semantics |
| Component Linking | EXISTS | Components don't link - imports resolve to stubs |

### Why Hello World Works

The hello-world component prints successfully because:
1. `wasi:cli/stdout.get-stdout` returns primitive i32 (handle)
2. `wasi:io/streams.blocking-write-and-flush` takes primitive i32 args (handle, ptr, len)
3. All types are primitives that map directly to core WASM values
4. NO lift/lower operations needed for primitives

**ANY component using strings, lists, records, variants, or resources will fail.**

---

## Phase 1: Canonical ABI Integration into Execution Path

**Goal**: Route all component function calls through proper lift/lower operations.

### 1.1 Current Problem

In `kiln-runtime/src/stackless/engine.rs`, WASI calls are dispatched directly:
```rust
// Line ~3397
wasip2_host.dispatch(module_name, field_name, args, Some(&mut mem_buffer))
```

This bypasses `CanonicalExecutor::execute_canon_lower()` entirely. The `args` here are already `Value::I32` pulled from the WASM stack - not `ComponentValue` types.

### 1.2 Required Changes

#### Step 1: Create Canonical Call Context
```rust
// New file: kiln-runtime/src/canonical_call.rs
pub struct CanonicalCallContext<'a> {
    pub memory: &'a mut [u8],
    pub realloc: Option<u32>,  // realloc function index
    pub post_return: Option<u32>,
    pub string_encoding: StringEncoding,
}

pub struct CanonicalCall {
    pub interface: String,
    pub function: String,
    pub param_types: Vec<ComponentType>,
    pub result_types: Vec<ComponentType>,
}
```

#### Step 2: Modify Import Dispatch in stackless/engine.rs

**Before** (current):
```rust
fn execute_import(&mut self, ...) {
    // Pull values from WASM stack
    let args = self.pop_values(param_count);
    // Call WASI directly
    wasip2_host.dispatch(module_name, field_name, args, memory)
}
```

**After** (proposed):
```rust
fn execute_import(&mut self, ...) {
    // 1. Get canonical function definition for this import
    let canon_def = self.get_canonical_definition(module_name, field_name)?;

    // 2. Pull core WASM values from stack
    let core_values = self.pop_values(canon_def.core_param_count);

    // 3. LIFT: Convert core values to component values using Canonical ABI
    let component_values = self.canonical_abi.lift_values(
        &canon_def.param_types,
        &core_values,
        self.memory.as_ref(),
    )?;

    // 4. Dispatch to host
    let results = wasip2_host.dispatch(module_name, field_name, component_values, memory)?;

    // 5. LOWER: Convert component results back to core values
    let core_results = self.canonical_abi.lower_values(
        &canon_def.result_types,
        &results,
        self.memory.as_mut(),
    )?;

    // 6. Push results to WASM stack
    self.push_values(&core_results);
}
```

#### Step 3: Implement lift_values/lower_values

The `CanonicalABI` struct in `kiln-component/src/canonical_abi/canonical_abi.rs` has individual `lift_*` and `lower_*` methods, but needs batch operations:

```rust
impl CanonicalABI {
    /// Lift multiple core values to component values
    pub fn lift_values<M: CanonicalMemory>(
        &self,
        types: &[ComponentType],
        core_values: &[Value],
        memory: &M,
    ) -> Result<Vec<ComponentValue>> {
        // For primitives: direct conversion
        // For strings/lists: read ptr+len from stack, lift from memory
        // For records: read field offsets, lift each field
        // etc.
    }

    /// Lower multiple component values to core values
    pub fn lower_values<M: CanonicalMemory>(
        &self,
        types: &[ComponentType],
        component_values: &[ComponentValue],
        memory: &mut M,
    ) -> Result<Vec<Value>> {
        // For primitives: direct conversion
        // For strings/lists: call realloc, write to memory, return ptr+len
        // etc.
    }
}
```

### 1.3 Files to Modify

| File | Changes |
|------|---------|
| `kiln-runtime/src/stackless/engine.rs` | Add canonical call integration at import dispatch |
| `kiln-component/src/canonical_abi/canonical_abi.rs` | Add `lift_values`, `lower_values` batch methods |
| `kiln-component/src/canonical_executor.rs` | Implement `execute_canon_lift` properly |
| `kiln-runtime/src/wasip2_host.rs` | Accept `ComponentValue` instead of `Value` |

### 1.4 Test Cases

1. **String Parameter Test**: Component that calls `wasi:filesystem/preopens.get-directories()` which returns `list<tuple<descriptor, string>>`
2. **String Return Test**: Component that exports a function returning a string
3. **Record Test**: Component using `wasi:http/types.fields` which uses records

---

## Phase 2: Component Linking Infrastructure

**Goal**: Enable components to import from other components, not just host functions.

### 2.1 Current Problem

The `ComponentLinker` in `kiln-component/src/components/component_instantiation.rs` defines structures but:
- `instantiate()` creates standalone instances
- No instance-to-instance linking
- All imports resolve to host or fail

### 2.2 Required Changes

#### Step 1: Instance Registry with Export Mapping

```rust
pub struct ComponentRegistry {
    instances: HashMap<InstanceId, ComponentInstance>,
    exports: HashMap<(InstanceId, String), ExportDefinition>,
    instance_order: Vec<InstanceId>,  // Topological order for initialization
}

impl ComponentRegistry {
    /// Register an export that can satisfy imports
    pub fn register_export(
        &mut self,
        instance: InstanceId,
        name: &str,
        export: ExportDefinition,
    ) -> Result<()>;

    /// Find which instance provides a given import
    pub fn resolve_import(
        &self,
        module: &str,
        name: &str,
    ) -> Result<(InstanceId, ExportDefinition)>;
}
```

#### Step 2: Cross-Instance Function Calls

When a component calls an imported function that's provided by another component:

```rust
fn execute_component_import(&mut self, import: &ResolvedImport) -> Result<Vec<Value>> {
    // 1. Get target instance
    let target = self.registry.get_instance(import.provider_id)?;

    // 2. Get target function
    let func = target.get_export(&import.provider_export)?;

    // 3. Marshal arguments (lift from caller, lower to callee)
    let callee_args = self.marshal_args_to_callee(
        &self.current_memory(),
        &target.memory(),
        &args,
        &func.param_types,
    )?;

    // 4. Execute in target context
    let results = self.execute_in_context(target, func, callee_args)?;

    // 5. Marshal results back (lift from callee, lower to caller)
    let caller_results = self.marshal_results_to_caller(
        &target.memory(),
        &self.current_memory(),
        &results,
        &func.result_types,
    )?;

    Ok(caller_results)
}
```

### 2.3 Instantiation Order

Components must be instantiated in dependency order:

```rust
impl ComponentRegistry {
    pub fn instantiate_all(&mut self, components: &[ComponentBinary]) -> Result<()> {
        // 1. Build dependency graph
        let deps = self.build_dependency_graph(components)?;

        // 2. Topological sort
        let order = topological_sort(&deps)?;

        // 3. Instantiate in order
        for component_id in order {
            let component = &components[component_id];

            // Resolve imports from already-instantiated components
            let imports = self.resolve_imports(component)?;

            // Create instance
            let instance = self.create_instance(component, imports)?;

            // Register exports for later components
            self.register_exports(instance)?;
        }

        Ok(())
    }
}
```

### 2.4 Files to Modify

| File | Changes |
|------|---------|
| `kiln-component/src/components/component_instantiation.rs` | Implement `ComponentRegistry` |
| `kiln-component/src/components/component_linker.rs` | Add dependency resolution |
| `kiln-runtime/src/stackless/engine.rs` | Add cross-instance dispatch |

---

## Phase 3: Resource System (own<T>, borrow<T>)

**Goal**: Implement proper resource ownership and borrowing semantics.

### 3.1 Current Problem

Resources are just u32 handles with no ownership tracking:
```rust
// Current implementation in kiln-component/src/resource_management.rs
pub struct ResourceHandle(pub u32);
```

This allows:
- Using a dropped resource (use-after-free)
- Leaking resources (no automatic cleanup)
- Borrowing from a dropped owner (dangling borrow)

### 3.2 Required Changes

#### Step 1: Resource Table per Component Instance

```rust
/// Resource entry with ownership information
pub struct ResourceEntry {
    /// Resource type ID
    pub type_id: ResourceTypeId,
    /// The actual representation (impl-defined)
    pub rep: u32,
    /// Is this an owned handle or borrowed?
    pub is_owned: bool,
    /// For owned: number of active borrows
    pub borrow_count: u32,
    /// For borrowed: the owning handle
    pub owner: Option<ResourceHandle>,
    /// For borrowed: the task/scope that created the borrow
    pub borrow_scope: Option<TaskId>,
}

pub struct ResourceTable {
    entries: Vec<Option<ResourceEntry>>,
    free_list: Vec<u32>,
}
```

#### Step 2: Canonical Built-ins

```rust
impl ResourceTable {
    /// canon resource.new - create a new owned resource
    pub fn resource_new(&mut self, type_id: ResourceTypeId, rep: u32) -> ResourceHandle {
        let entry = ResourceEntry {
            type_id,
            rep,
            is_owned: true,
            borrow_count: 0,
            owner: None,
            borrow_scope: None,
        };
        self.allocate(entry)
    }

    /// canon resource.drop - drop an owned resource
    pub fn resource_drop(&mut self, handle: ResourceHandle) -> Result<u32> {
        let entry = self.get_mut(handle)?;

        // Cannot drop if borrows exist
        if entry.borrow_count > 0 {
            return Err(Error::resource_error("Cannot drop resource with active borrows"));
        }

        if !entry.is_owned {
            return Err(Error::resource_error("Cannot drop borrowed resource"));
        }

        let rep = entry.rep;
        self.deallocate(handle);
        Ok(rep)
    }

    /// Create a borrow from an owned resource
    pub fn borrow(&mut self, owner: ResourceHandle, scope: TaskId) -> Result<ResourceHandle> {
        let owner_entry = self.get_mut(owner)?;

        if !owner_entry.is_owned {
            return Err(Error::resource_error("Cannot borrow from borrowed resource"));
        }

        owner_entry.borrow_count += 1;

        let borrow_entry = ResourceEntry {
            type_id: owner_entry.type_id,
            rep: owner_entry.rep,
            is_owned: false,
            borrow_count: 0,
            owner: Some(owner),
            borrow_scope: Some(scope),
        };

        Ok(self.allocate(borrow_entry))
    }

    /// End a borrow (called when borrow scope ends)
    pub fn end_borrow(&mut self, handle: ResourceHandle) -> Result<()> {
        let entry = self.get(handle)?;

        if entry.is_owned {
            return Err(Error::resource_error("Cannot end borrow on owned resource"));
        }

        let owner = entry.owner.ok_or_else(|| Error::resource_error("Borrow missing owner"))?;

        self.deallocate(handle);

        // Decrement owner's borrow count
        let owner_entry = self.get_mut(owner)?;
        owner_entry.borrow_count -= 1;

        Ok(())
    }
}
```

#### Step 3: Integrate with Canonical ABI

When lifting/lowering resource handles:

```rust
impl CanonicalABI {
    pub fn lift_own<M: CanonicalMemory>(
        &self,
        memory: &M,
        offset: u32,
        resource_table: &mut ResourceTable,
    ) -> Result<ResourceHandle> {
        let handle_value = memory.read_u32_le(offset)?;
        let handle = ResourceHandle(handle_value);

        // Validate the handle exists and is owned
        let entry = resource_table.get(handle)?;
        if !entry.is_owned {
            return Err(Error::resource_error("Expected owned resource"));
        }

        Ok(handle)
    }

    pub fn lower_borrow<M: CanonicalMemory>(
        &self,
        memory: &mut M,
        offset: u32,
        handle: ResourceHandle,
        scope: TaskId,
        resource_table: &mut ResourceTable,
    ) -> Result<()> {
        // Create a borrow for the callee's scope
        let borrow_handle = resource_table.borrow(handle, scope)?;
        memory.write_u32_le(offset, borrow_handle.0)
    }
}
```

### 3.3 Files to Modify

| File | Changes |
|------|---------|
| `kiln-component/src/resource_management.rs` | Implement `ResourceTable` with ownership |
| `kiln-component/src/canonical_abi/canonical_abi.rs` | Add `lift_own`, `lift_borrow`, `lower_own`, `lower_borrow` |
| `kiln-runtime/src/wasip2_host.rs` | Use resource table for WASI handles |

---

## Phase 4: Multi-Component Composition

**Goal**: Support composing multiple components with proper type checking.

### 4.1 Current Problem

Components are treated as standalone. No composition:
- No WIT type compatibility checking
- No interface matching
- No component graph management

### 4.2 Required Changes

#### Step 1: WIT Type Registry

```rust
/// Registry of component types for validation
pub struct TypeRegistry {
    /// Named interfaces
    interfaces: HashMap<String, InterfaceDefinition>,
    /// Type definitions
    types: HashMap<TypeId, TypeDefinition>,
}

pub struct InterfaceDefinition {
    pub name: String,
    pub version: Version,
    pub types: HashMap<String, TypeId>,
    pub functions: HashMap<String, FunctionType>,
}
```

#### Step 2: Import/Export Type Checking

```rust
impl ComponentLinker {
    /// Validate that an export satisfies an import
    fn check_type_compatibility(
        &self,
        import: &ImportDefinition,
        export: &ExportDefinition,
    ) -> Result<()> {
        match (import, export) {
            (ImportDefinition::Function(import_fn), ExportDefinition::Function(export_fn)) => {
                // Check parameter types match
                if import_fn.params.len() != export_fn.params.len() {
                    return Err(Error::type_error("Parameter count mismatch"));
                }

                for (imp, exp) in import_fn.params.iter().zip(export_fn.params.iter()) {
                    self.check_subtype(exp, imp)?;  // contravariance
                }

                // Check return types match
                for (imp, exp) in import_fn.results.iter().zip(export_fn.results.iter()) {
                    self.check_subtype(imp, exp)?;  // covariance
                }

                Ok(())
            }
            _ => Err(Error::type_error("Import/export kind mismatch"))
        }
    }
}
```

#### Step 3: Component Composition DSL

```rust
/// Builder for component composition
pub struct CompositionBuilder {
    components: HashMap<String, ComponentBinary>,
    links: Vec<Link>,
}

pub struct Link {
    pub from_component: String,
    pub from_export: String,
    pub to_component: String,
    pub to_import: String,
}

impl CompositionBuilder {
    pub fn add_component(&mut self, name: &str, binary: &[u8]) -> Result<&mut Self>;
    pub fn link(&mut self, link: Link) -> Result<&mut Self>;
    pub fn build(&self) -> Result<ComposedComponent>;
}
```

### 4.3 Files to Create/Modify

| File | Changes |
|------|---------|
| `kiln-component/src/wit/type_registry.rs` | NEW: Type registry |
| `kiln-component/src/wit/compatibility.rs` | NEW: Type compatibility checking |
| `kiln-component/src/composition.rs` | NEW: Composition builder |
| `kiln-component/src/components/component_linker.rs` | Integrate type checking |

---

## Implementation Order and Dependencies

```
Phase 1 (Canonical ABI Integration)
    │
    ├── 1.1 Batch lift/lower operations
    ├── 1.2 Call context with realloc
    └── 1.3 Integration into stackless/engine.rs
          │
          v
Phase 2 (Component Linking)
    │
    ├── 2.1 Instance registry
    ├── 2.2 Cross-instance calls
    └── 2.3 Dependency ordering
          │
          v
Phase 3 (Resource System)
    │
    ├── 3.1 Resource table
    ├── 3.2 own/borrow semantics
    └── 3.3 Integration with ABI
          │
          v
Phase 4 (Multi-Component Composition)
    │
    ├── 4.1 Type registry
    ├── 4.2 Compatibility checking
    └── 4.3 Composition builder
```

---

## Verification Milestones

### Milestone 1: String Support
- [ ] Component can receive a string parameter from host
- [ ] Component can return a string to host
- [ ] String encoding (UTF-8) is correctly handled

### Milestone 2: List Support
- [ ] Component can receive a list parameter
- [ ] Component can return a list
- [ ] Nested lists work correctly

### Milestone 3: Record/Variant Support
- [ ] Component can use records in function signatures
- [ ] Component can use variants (enums with payloads)
- [ ] Option and Result types work correctly

### Milestone 4: Resource Ownership
- [ ] `own<T>` correctly transfers ownership
- [ ] `borrow<T>` correctly creates borrows
- [ ] Dropped borrows decrement owner count
- [ ] Cannot drop owner while borrows exist

### Milestone 5: Component Linking
- [ ] Component A can import function from Component B
- [ ] Dependency order is respected
- [ ] Type checking prevents mismatched links

### Milestone 6: Full WASI Preview 2
- [ ] wasi:filesystem works (uses strings, records, resources)
- [ ] wasi:http works (complex types)
- [ ] wasi:sockets works (complex types)

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing hello-world | HIGH | Keep primitive passthrough, add lift/lower for complex types |
| Performance regression | MEDIUM | Optimize primitive case to avoid unnecessary conversions |
| Memory allocation in lift/lower | MEDIUM | Use realloc properly, pool allocations |
| Complex type edge cases | HIGH | Extensive testing with real WASI components |

---

## Conclusion

The current Kiln Component Model implementation is a facade. Making it real requires:

1. **Route all calls through Canonical ABI** - not just documentation, actual execution
2. **Implement component linking** - instances resolving imports from other instances
3. **Real resource ownership** - not just integer tags
4. **Type validation** - at link time and call time

This plan provides a roadmap to transform the existing infrastructure into a working Component Model implementation.
