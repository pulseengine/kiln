# RFC #46 C API Reference

> Extracted from [bytecodealliance/rfcs#46](https://github.com/bytecodealliance/rfcs/pull/46)
> (dicej/rfcs branch `lower-component`, as of 2026-03-18)

## Types

```c
typedef struct { uint32_t event; uint32_t waitable; uint32_t payload; } wait_result_t;
typedef struct { uint32_t writer; uint32_t reader; } writer_reader_pair_t;
typedef struct { uint32_t result; size_t count; } result_and_count_t;
typedef struct { uint32_t status; uint32_t task; } task_result_t;
typedef struct { uint32_t code; uint32_t waitable_set; } task_status_t;
```

## Constants

```c
// Events
#define EVENT_NONE 0
#define EVENT_SUBTASK 1
#define EVENT_STREAM_READ 2
#define EVENT_STREAM_WRITE 3
#define EVENT_FUTURE_READ 4
#define EVENT_FUTURE_WRITE 5
#define EVENT_CANCELLED 6

// Copy results
#define COPY_RESULT_BLOCKED 0xFFFFFFFF
#define COPY_RESULT_COMPLETED 0
#define COPY_RESULT_DROPPED 1
#define COPY_RESULT_CANCELLED 2

// Task status
#define TASK_STATUS_STARTING 0
#define TASK_STATUS_STARTED 1
#define TASK_STATUS_RETURNED 2
#define TASK_STATUS_START_CANCELLED 3
#define TASK_STATUS_RETURN_CANCELLED 4

// Callback codes
#define CALLBACK_CODE_EXIT 0
#define CALLBACK_CODE_YIELD 1
#define CALLBACK_CODE_WAIT 2
```

## Guest→Host Intrinsics (imported by lowered module)

### Thread Management
```c
uint32_t thread_new(void *context, uint32_t table, uint32_t func);
void thread_switch_to(uint32_t thread);
uint32_t thread_index();
```

### Context Variables
```c
uint32_t context0_get();
void context0_set(uint32_t value);
```

### Waitable Sets
```c
uint32_t waitable_set_new();
uint32_t waitable_join(uint32_t waitable, uint32_t set);
wait_result_t waitable_set_wait(uint32_t set);
wait_result_t waitable_set_poll(uint32_t set);
```

### Streams (typed per element)
```c
writer_reader_pair_t stream_<type>_new();
result_and_count_t stream_<type>_read(uint32_t stream, uint32_t memory, uint32_t* buffer, size_t length);
result_and_count_t stream_<type>_write(uint32_t stream, uint32_t memory, uint32_t* buffer, size_t length);
```

### Resources (per interface)
```c
uint32_t __wasm_import_<iface>_<resource>_new(uint32_t v);
uint32_t __wasm_import_<iface>_<resource>_rep(uint32_t handle);
void __wasm_import_<iface>_<resource>_drop(int32_t handle);
```

### Async Task Lifecycle
```c
void __wasm_export_<iface>_<func>__task_return(uint32_t value);
void __wasm_export_<iface>_<func>__task_cancel();
```

## Guest→Host Non-Intrinsic Imports

```c
// Resource constructor/method/drop (from imported interface)
uint32_t import_<iface>_constructor_<resource>(uint32_t v);
uint32_t import_<iface>_<resource>_get(uint32_t handle);
void import_<iface>_<resource>_drop(uint32_t handle);

// Async function import
task_result_t import_<iface>_<func>(uint32_t memory, uint8_t *v_ptr, size_t v_len, uint32_t s, uint32_t *return_ptr);
```

## Host→Guest Exports (called by host)

```c
// Memory allocation
void *cabi_realloc(uint32_t memory, void *ptr, size_t old_size, size_t align, size_t new_size);

// Resource constructor/method/destructor
uint32_t <iface>_constructor_<resource>(uint32_t v);
uint32_t <iface>_<resource>_get(uint32_t handle);
void <iface>_<resource>_dtor(uint32_t handle);

// Async export (returns task status for cooperative scheduling)
task_status_t <iface>_<func>(uint32_t memory, uint8_t *v_ptr, size_t v_len, uint32_t s, uint32_t *return_ptr);
task_status_t <iface>_<func>_callback(uint32_t event, uint32_t waitable, uint32_t payload);
```

## Embedder Bindings API (host-side runtime interface)

### Store Management
```c
store_t store_new(void *data);
void *store_data(store_t store);
void store_drop(store_t store);
```

### Linker
```c
linker_t linker_new();
void linker_add(linker_t linker, const char *module, const char *name, host_function_t func);
void linker_drop(linker_t linker);
```

### Instance Management
```c
instance_t instance_new(store_t store, linker_t linker, uint8_t *module);
void instance_call(store_t store, instance_t instance, const char *name,
                   value_t *param_ptr, size_t param_len,
                   value_t *result_ptr, size_t result_len);
value_t instance_get_global(store_t store, instance_t instance, uint32_t index);
void instance_set_global(store_t store, instance_t instance, uint32_t index, value_t value);
memory_result_t instance_get_memory(store_t store, instance_t instance, uint32_t index);
```

### Fiber Management
```c
fiber_t fiber_new(store_t store, void *context, void (*func)(void *));
void fiber_resume(store_t store, fiber_t fiber);
void fiber_suspend();
```

### Types
```c
typedef struct { void *ptr; } store_t;
typedef struct { void *ptr; } linker_t;
typedef struct { void *ptr; } instance_t;
typedef struct { void *ptr; } fiber_t;
typedef struct { uint8_t *ptr; size_t len; } memory_result_t;
typedef union { uint32_t u32; uint64_t u64; float f32; double f64; } value_t;
typedef void (*host_function_t)(store_t store, instance_t instance,
                                value_t *param_ptr, size_t param_len,
                                value_t *result_ptr, size_t result_len);
```
