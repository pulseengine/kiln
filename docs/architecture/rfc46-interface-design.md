# RFC #46 Host Intrinsic Interface Design

This document describes the full RFC #46 host intrinsic interface as implemented by Kiln, mapping each intrinsic to its backing implementation on gale (RTOS) and std targets.

## Architecture

The mapping is:

```
RFC #46 host intrinsics --> kiln async executor --> gale RTOS primitives
```

Specifically:
- `thread_new/switch_to/thread_index` --> gale RTOS task creation and cooperative scheduling
- `waitable_set_new/wait/poll` --> kiln async executor polling gale's event loop
- `stream_read/write` --> kiln async futures backed by gale IPC or shared memory channels
- `context0_get/set` --> gale task-local storage
- `resource_new/rep/drop` --> kiln bounded resource table (generation-counted handles)
- `cabi_realloc` --> kiln memory pool allocation from pre-assigned budget
- `task_return/cancel` --> kiln async task completion signaling to gale scheduler

### Control Flow

1. Meld lowers component to core module(s) at build time
2. Lowered module imports RFC #46 intrinsics
3. Kiln provides intrinsic implementations
4. Kiln's async executor bridges wasm cooperative multitasking to gale's preemptive scheduler
5. Gale provides the actual OS primitives (tasks, IPC, memory protection)

For std targets (no gale): kiln uses `std::thread` and `tokio`/`async-std` as the backing runtime instead of gale.

---

## Thread Management

### `thread_new(context: i32, table: i32, func: i32) -> thread_id: i32`

Kiln creates a logical thread backed by a gale RTOS task.

- **context**: The initial state value for the new thread. Stored in the thread's task-local context variable (accessible via `context0_get`).
- **table**: The table index containing the function reference to execute.
- **func**: The function index within the table identifying the wasm function to run as the thread's entry point.
- **Returns**: A `thread_id` handle that can be used with `thread_switch_to` and tracked by the executor.

**gale backing**: `gale_task_create(priority, stack_size, entry_fn)` allocates a new RTOS task. The wasm function is wrapped in a trampoline that sets up the execution context (linear memory base, table pointers, context value) before entering the wasm code. The gale task ID is stored in kiln's thread table, which maps logical `thread_id` to gale task handles.

**std backing**: A new `tokio::task` (or `std::thread` in synchronous mode) is spawned. The wasm function is executed within the spawned task's context. The OS thread/task handle is stored in the thread table.

**Constraints**:
- Thread creation is bounded by the component instance's thread budget (configurable at instantiation time).
- Each `thread_new` call decrements the thread budget. When the budget is zero, `thread_new` traps.
- The table and func indices are validated against the module's table and function sections before task creation.
- On gale, the task stack is allocated from the component's pre-assigned memory pool (not from global heap).

### `thread_switch_to(thread_id: i32)`

Cooperative yield from the current thread to the specified target thread.

- **thread_id**: The logical thread to switch to, as returned by `thread_new`.

**gale backing**: Kiln saves the current wasm execution state (program counter, value stack, call stack, fuel counter) into the current thread's context block. Then kiln signals gale to schedule the target task via `gale_task_yield_to(target_task_handle)`. Gale's preemptive scheduler activates the target task, which resumes from its saved wasm execution state.

**std backing**: Uses async yield (`tokio::task::yield_now()`) followed by executor-directed wake of the target task's waker. The executor maintains a ready queue and ensures the target task is scheduled next.

**Constraints**:
- `thread_switch_to` validates that `thread_id` belongs to the same component instance. Switching to a thread in a different component is a sandbox violation and causes an immediate trap.
- Switching to a thread that has already completed or been cancelled traps.
- The current thread's execution state must be fully saved before the switch. Partial saves cause state corruption on resume.

### `thread_index() -> thread_id: i32`

Returns the current logical thread's ID.

**gale backing**: Reads the thread ID from gale's task-local storage (`gale_tls_get(KILN_THREAD_ID_KEY)`).

**std backing**: Reads from `thread_local!` storage or the async task's local context.

---

## Context Variables

### `context0_get() -> value: i32`

Returns the per-thread context variable value.

**gale backing**: Reads from gale's task-local storage (`gale_tls_get(KILN_CONTEXT0_KEY)`). Each gale task has its own TLS area, so context values are naturally isolated between threads.

**std backing**: Reads from `thread_local!` storage (synchronous mode) or `tokio::task_local!` (async mode).

### `context0_set(value: i32)`

Sets the per-thread context variable value.

**gale backing**: Writes to gale's task-local storage (`gale_tls_set(KILN_CONTEXT0_KEY, value)`).

**std backing**: Writes to `thread_local!` or `tokio::task_local!`.

**Constraints**:
- Context values are per-thread and per-component-instance. One component's context is not visible to another.
- The context variable persists across calls within the same thread's execution lifetime.

---

## Waitable Sets (Kiln Async Executor)

### `waitable_set_new() -> set_id: i32`

Kiln allocates an executor poll set.

- **Returns**: A `set_id` identifying the new waitable set.

**gale backing**: Kiln creates an internal poll set data structure. The poll set maintains a list of registered waitables (streams, futures, subtasks) and their readiness state. On gale, each poll set is associated with a gale event group (`gale_event_group_create()`) that aggregates readiness signals from multiple sources.

**std backing**: Creates a `futures::stream::FuturesUnordered` or equivalent poll aggregator backed by the async runtime's event loop.

**Constraints**:
- Waitable sets are bounded by the component's waitable set budget.
- Each waitable set has a maximum capacity for joined waitables (configurable).

### `waitable_join(waitable: i32, set: i32) -> join_id: i32`

Register a waitable (stream endpoint, future, or subtask) with a waitable set.

- **waitable**: The waitable handle (stream reader, future reader, or subtask handle).
- **set**: The waitable set to join.
- **Returns**: A `join_id` that uniquely identifies this waitable within the set. Used to correlate events from `waitable_set_wait`/`waitable_set_poll`.

**gale backing**: Adds the waitable's readiness signal to the set's gale event group. When the waitable becomes ready (data available on stream, future resolved, subtask completed), it signals the event group via `gale_event_group_set()`.

**std backing**: Adds the waitable's future to the `FuturesUnordered` aggregator. The waker is configured to signal readiness to the poll set.

**Constraints**:
- A waitable can only be joined to one set at a time. Joining to a second set traps.
- The waitable handle is validated for ownership (must belong to the same component instance).

### `waitable_set_wait(set: i32) -> (event_type: i32, payload: i32)`

Blocking wait for any waitable in the set to become ready.

- **set**: The waitable set to wait on.
- **Returns**: A pair of `(event_type, payload)` where `event_type` is one of the EVENT constants (EVENT_SUBTASK=1, EVENT_STREAM_READ=2, EVENT_STREAM_WRITE=3, EVENT_FUTURE_READ=4, EVENT_FUTURE_WRITE=5, EVENT_CANCELLED=6) and `payload` is the `join_id` of the waitable that fired.

**gale backing**: Kiln yields the current thread to gale via `gale_event_group_wait(event_group, timeout)`. Gale suspends the calling task until any event in the group fires. When woken, kiln inspects the event group bits to determine which waitable(s) are ready and returns the first ready event.

**std backing**: Calls `poll` on the `FuturesUnordered` aggregator within the async runtime. If no events are ready, the current async task is suspended (yielded) until one becomes ready.

**Constraints**:
- Waiting on an empty set (no joined waitables) traps immediately in all modes.
- On single-threaded runtimes without an async executor, `waitable_set_wait` traps because blocking the sole thread would deadlock.
- The event group wait on gale has a configurable timeout to prevent permanent blocking (integrated with watchdog).

### `waitable_set_poll(set: i32) -> (event_type: i32, payload: i32)`

Non-blocking check of all waitables in the set.

- **set**: The waitable set to poll.
- **Returns**: Same format as `waitable_set_wait`. Returns `(EVENT_NONE=0, 0)` if no events are ready.

**gale backing**: Calls `gale_event_group_poll(event_group)` which checks readiness bits without blocking.

**std backing**: Calls `poll` with `Poll::Pending` pass-through (single poll iteration without yielding).

**Constraints**:
- Polling an empty set returns `(EVENT_NONE, 0)` (not a trap, unlike `wait`).
- Polling must complete in bounded time (O(n) where n is the number of joined waitables).

---

## Streams

### `stream_<type>_new() -> (writer: i32, reader: i32)`

Kiln allocates a bounded channel for typed stream data.

- **Returns**: A packed i64 value containing the writer handle (low 32 bits) and reader handle (high 32 bits), per the RFC encoding.

**gale backing**: Allocates a bounded ring buffer from the component's memory pool. The ring buffer is in shared memory accessible to both the writer and reader tasks. Readiness notifications use gale IPC signals (`gale_signal_create()`). The writer signals "data available" and the reader signals "space available" to implement backpressure.

**std backing**: Creates a bounded MPSC channel (`tokio::sync::mpsc::channel(capacity)` or `crossbeam::channel::bounded(capacity)`).

**Constraints**:
- Channel capacity is bounded by the component's stream budget.
- Each stream endpoint (writer, reader) is a resource handle tracked in the resource table with generation counters.
- The element type determines the element size in the ring buffer.

### `stream_<type>_read(stream: i32, memory: i32, buffer: i32, length: i32) -> result_and_count: i32`

Async read from the stream channel into wasm linear memory.

- **stream**: The reader handle returned by `stream_new`.
- **memory**: The linear memory index to write into.
- **buffer**: The byte offset within linear memory where data should be written.
- **length**: The maximum number of elements to read.
- **Returns**: A packed result containing the copy result code and the actual number of elements read. Result codes: BLOCKED(0xFFFFFFFF) if no data is available, COMPLETED(0) if the read succeeded, DROPPED(1) if the writer has been dropped (end of stream), CANCELLED(2) if the operation was cancelled.

**gale backing**: Kiln checks if data is available in the ring buffer. If data is present, copies elements from the ring buffer to `memory[buffer..buffer + count * elem_size]`, updates the read pointer, and signals "space available" to the writer via gale IPC. If no data is available, returns BLOCKED and the calling wasm code must register the stream reader with a waitable set and wait.

**std backing**: Attempts `try_recv` on the channel. If data is available, copies to linear memory. If empty, returns BLOCKED.

**Constraints**:
- The range `[buffer, buffer + length * element_size)` is validated against the linear memory bounds using checked arithmetic before any data transfer. If the range is invalid, the intrinsic traps.
- The memory index is validated against the module's declared memories.
- Stream read is non-blocking: it returns BLOCKED rather than suspending the thread.

### `stream_<type>_write(stream: i32, memory: i32, buffer: i32, length: i32) -> result_and_count: i32`

Async write from wasm linear memory into the stream channel.

- **stream**: The writer handle returned by `stream_new`.
- **memory**: The linear memory index to read from.
- **buffer**: The byte offset within linear memory where data should be read.
- **length**: The number of elements to write.
- **Returns**: Same format as `stream_read`. BLOCKED if the channel is full, COMPLETED if the write succeeded, DROPPED if the reader has been dropped, CANCELLED if cancelled.

**gale backing**: Kiln checks if space is available in the ring buffer. If space exists, copies elements from `memory[buffer..buffer + count * elem_size]` to the ring buffer, updates the write pointer, and signals "data available" to the reader via gale IPC. If the ring buffer is full, returns BLOCKED.

**std backing**: Attempts `try_send` on the channel. Returns BLOCKED if channel is full.

**Constraints**:
- Same bounds-checking constraints as `stream_read`.
- Backpressure is enforced by the bounded channel: writers cannot produce faster than the channel capacity allows.
- When both writer and reader are on the same thread and the channel is full, the writer returns BLOCKED. The calling wasm code must yield (via `thread_switch_to` or `waitable_set_wait`) to allow the reader to drain the channel. Failure to yield creates a deadlock (LS-EX-2).

---

## Resources

### `thing_new(value: i32) -> handle: i32`

Kiln allocates a new entry in the bounded resource table.

- **value**: The representation value to store (opaque to kiln, meaningful to the component).
- **Returns**: A handle encoding both the slot index and the generation counter.

**Implementation**: The resource table is a bounded array of slots. Each slot contains: the representation value, a generation counter (incremented on each drop), a type tag (identifying which resource type this handle belongs to), and an ownership tag (the creating component instance ID). The handle value is computed as `(generation << INDEX_BITS) | slot_index` where `INDEX_BITS` is sufficient to address all slots.

**Constraints**:
- Table capacity is bounded by the component's resource budget.
- When the table is full, `thing_new` traps with a resource exhaustion error.
- Generation counter overflow detection: if the generation counter reaches its maximum value, the slot is permanently retired (never reused) to prevent aliasing.

### `thing_rep(handle: i32) -> value: i32`

Look up a resource's representation value.

- **handle**: The handle returned by `thing_new`.
- **Returns**: The stored representation value.

**Implementation**: Extracts the slot index and generation from the handle. Validates: (1) slot index is within table bounds, (2) the slot's current generation matches the handle's generation (detects use-after-free), (3) the slot is in the "live" state (not dropped), (4) the calling component matches the owning component (or has been explicitly granted access via resource transfer). If any check fails, traps immediately.

### `thing_drop(handle: i32)`

Drop a resource, freeing its table slot.

- **handle**: The handle to drop.

**Implementation**: Performs the same validation as `thing_rep`. If valid: marks the slot as "dropped", increments the generation counter, adds the slot index to the free list for future reuse. If the handle has already been dropped (generation mismatch or state is "dropped"), traps with a double-free error.

---

## Memory

### `cabi_realloc(memory: i32, ptr: i32, old_size: i32, align: i32, new_size: i32) -> ptr: i32`

Allocate or reallocate memory from the component instance's pre-assigned memory pool.

- **memory**: The linear memory index.
- **ptr**: The existing pointer (0 for new allocation).
- **old_size**: The current allocation size (0 for new allocation).
- **align**: The required alignment.
- **new_size**: The desired allocation size.
- **Returns**: A pointer to the allocated region within linear memory.

**gale backing**: Kiln maintains a bump allocator or free-list allocator within each component's linear memory. The allocator operates within the linear memory bounds -- it does not allocate host memory. The pre-assigned budget limits total allocatable bytes.

**std backing**: Same linear-memory allocator strategy. The backing linear memory itself may be heap-allocated, but cabi_realloc operates within the wasm address space.

**Constraints**:
- All allocation is within the component's linear memory. cabi_realloc does not call malloc or allocate host memory.
- Alignment must be a power of two. Invalid alignment traps.
- Allocation that would exceed the linear memory bounds traps.
- In ASIL-D mode, cabi_realloc operates only from a pre-reserved region of linear memory established at initialization.

---

## Task Lifecycle

### `task_return(value: i32)`

Signal that the current async task has completed successfully.

- **value**: The return value (encoded per the task's return type).

**gale backing**: Kiln marks the current task as "completed" in the task table. If a parent task is waiting on this subtask (via a waitable set), kiln signals the parent's event group with EVENT_SUBTASK and the subtask's join ID. Gale then schedules the parent task to run. The return value is stored in the task's completion slot for the parent to read.

**std backing**: Resolves the task's completion future, waking any waiters.

**Constraints**:
- Calling `task_return` on a task that has already been cancelled (state START_CANCELLED or RETURN_CANCELLED) traps. The cancellation has already been reported to the parent; delivering a return value would cause double-completion.
- After `task_return`, the task is in a terminal state. No further execution occurs in this task.

### `task_cancel()`

Request cancellation of the current async task.

**gale backing**: Kiln sets the task's state to "cancel requested". On the next yield point (cooperative scheduling boundary), the task checks the cancellation flag and performs cleanup. The parent is notified via EVENT_CANCELLED on its waitable set.

**std backing**: Drops the task's cancellation token, triggering the async cancellation machinery.

**Constraints**:
- Cancellation is cooperative: the task must reach a yield point (e.g., `waitable_set_wait`, `thread_switch_to`, or fuel exhaustion) for cancellation to take effect.
- Resources held by the cancelled task must be cleaned up. Kiln's resource table tracks which resources belong to each task; on cancellation, all owned resources are dropped in reverse creation order.

---

## Event Constants

| Constant            | Value      | Context                     |
|---------------------|------------|-----------------------------|
| EVENT_NONE          | 0          | No event ready (poll result)|
| EVENT_SUBTASK       | 1          | Subtask state changed       |
| EVENT_STREAM_READ   | 2          | Stream data available        |
| EVENT_STREAM_WRITE  | 3          | Stream space available       |
| EVENT_FUTURE_READ   | 4          | Future value ready           |
| EVENT_FUTURE_WRITE  | 5          | Future write slot available  |
| EVENT_CANCELLED     | 6          | Task/waitable cancelled      |

## Copy Result Constants

| Constant          | Value      | Meaning                            |
|-------------------|------------|------------------------------------|
| COPY_RESULT_COMPLETED | 0      | Transfer completed successfully    |
| COPY_RESULT_DROPPED   | 1      | Other end dropped (end of stream)  |
| COPY_RESULT_CANCELLED | 2      | Operation cancelled                |
| COPY_RESULT_BLOCKED   | 0xFFFFFFFF | No data/space available (try later) |

## Task Status Constants

| Constant              | Value | Meaning                          |
|-----------------------|-------|----------------------------------|
| TASK_STATUS_STARTING  | 0     | Task is being initialized        |
| TASK_STATUS_STARTED   | 1     | Task is running                  |
| TASK_STATUS_RETURNED  | 2     | Task completed with return value |
| TASK_STATUS_START_CANCELLED | 3 | Task cancelled before starting  |
| TASK_STATUS_RETURN_CANCELLED | 4 | Task cancelled after starting  |
