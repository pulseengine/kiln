//! Linux-specific thread pool implementation with cgroups support.
//!
//! This module provides a thread pool that uses Linux cgroups v2 for resource
//! isolation and control, along with real-time scheduling capabilities.

use core::{
    fmt::{
        self,
        Debug,
    },
    sync::atomic::{
        AtomicBool,
        AtomicU64,
        AtomicUsize,
        Ordering,
    },
    time::Duration,
};
use std::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{
        String,
        ToString,
    },
    sync::Arc,
    vec::Vec,
};

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};
use wrt_sync::{
    WrtMutex,
    WrtRwLock,
};

use crate::threading::{
    CpuSet,
    PlatformThreadHandle,
    PlatformThreadPool,
    ThreadHandle,
    ThreadPoolConfig,
    ThreadPoolStats,
    ThreadPriority,
    ThreadStats,
    WasmTask,
};

/// Linux scheduling policies
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedPolicy {
    /// Normal scheduling
    Normal     = 0,
    /// FIFO real-time scheduling
    Fifo       = 1,
    /// Round-robin real-time scheduling
    RoundRobin = 2,
    /// Batch scheduling
    Batch      = 3,
    /// Idle scheduling
    Idle       = 5,
    /// Deadline scheduling
    Deadline   = 6,
}

/// FFI declarations for Linux thread and cgroup management
#[allow(non_camel_case_types)]
mod ffi {
    use core::ffi::{
        c_char,
        c_int,
        c_long,
        c_void,
    };

    pub type pthread_t = c_long;
    pub type cpu_set_t = [u64; 16]; // 1024 bits for CPU mask

    #[repr(C)]
    pub struct sched_param {
        pub sched_priority: c_int,
    }

    #[repr(C)]
    pub struct sched_attr {
        pub size:           u32,
        pub sched_policy:   u32,
        pub sched_flags:    u64,
        pub sched_nice:     i32,
        pub sched_priority: u32,
        pub sched_runtime:  u64,
        pub sched_deadline: u64,
        pub sched_period:   u64,
    }

    // SAFETY: Edition 2024 requires unsafe extern blocks
    unsafe extern "C" {
        // Thread creation and management
        pub fn pthread_create(
            thread: *mut pthread_t,
            attr: *const c_void,
            start_routine: extern "C" fn(*mut c_void) -> *mut c_void,
            arg: *mut c_void,
        ) -> c_int;

        pub fn pthread_join(thread: pthread_t, retval: *mut *mut c_void) -> c_int;
        pub fn pthread_cancel(thread: pthread_t) -> c_int;
        pub fn pthread_self() -> pthread_t;

        // Scheduling
        pub fn sched_setscheduler(pid: i32, policy: c_int, param: *const sched_param) -> c_int;
        pub fn sched_getscheduler(pid: i32) -> c_int;
        pub fn sched_setattr(pid: i32, attr: *const sched_attr, flags: u32) -> c_int;
        pub fn sched_getattr(pid: i32, attr: *mut sched_attr, size: u32, flags: u32) -> c_int;

        // CPU affinity
        pub fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const cpu_set_t) -> c_int;
        pub fn sched_getaffinity(pid: i32, cpusetsize: usize, mask: *mut cpu_set_t) -> c_int;

        // Nice value
        pub fn nice(inc: c_int) -> c_int;

        // System calls for cgroups
        pub fn open(pathname: *const c_char, flags: c_int, mode: u32) -> c_int;
        pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
        pub fn close(fd: c_int) -> c_int;
        pub fn mkdir(pathname: *const c_char, mode: u32) -> c_int;

        // Process info
        pub fn getpid() -> i32;
        pub fn gettid() -> i32;
    }

    // CPU set manipulation macros as functions
    pub fn CPU_ZERO(set: *mut cpu_set_t) {
        unsafe {
            for i in 0..16 {
                (*set)[i] = 0;
            }
        }
    }

    pub fn CPU_SET(cpu: usize, set: *mut cpu_set_t) {
        if cpu < 1024 {
            unsafe {
                (*set)[cpu / 64] |= 1u64 << (cpu % 64);
            }
        }
    }

    pub fn CPU_ISSET(cpu: usize, set: *const cpu_set_t) -> bool {
        if cpu < 1024 {
            unsafe { ((*set)[cpu / 64] & (1u64 << (cpu % 64))) != 0 }
        } else {
            false
        }
    }
}

/// Cgroup controller for resource management
struct CgroupController {
    /// Cgroup path
    path:  String,
    /// Whether we created this cgroup
    owned: bool,
}

impl CgroupController {
    /// Create or attach to a cgroup
    fn new(name: &str) -> Result<Self> {
        let path = format!("/sys/fs/cgroup/{}", name);

        // Try to create the cgroup directory
        let path_cstr = format!("{}\0", path);
        let result = unsafe { ffi::mkdir(path_cstr.as_ptr() as *const i8, 0o755) };

        let owned = result == 0; // We created it

        Ok(Self { path, owned })
    }

    /// Write a value to a cgroup file
    fn write_file(&self, filename: &str, value: &str) -> Result<()> {
        let filepath = format!("{}/{}\0", self.path, filename);
        let fd = unsafe { ffi::open(filepath.as_ptr() as *const i8, 1, 0) }; // O_WRONLY

        if fd < 0 {
            return Err(Error::runtime_execution_error(
                "Failed to create thread: resource limits exceeded",
            ));
        }

        let written = unsafe { ffi::write(fd, value.as_ptr() as *const _, value.len()) };

        unsafe {
            ffi::close(fd);
        }

        if written < 0 {
            return Err(Error::new(
                ErrorCategory::Platform,
                1,
                "Failed to write to cgroup file",
            ));
        }

        Ok(())
    }

    /// Add current thread to cgroup
    fn add_thread(&self) -> Result<()> {
        let tid = unsafe { ffi::gettid() };
        self.write_file("cgroup.threads", &tid.to_string())
    }

    /// Set memory limit
    fn set_memory_limit(&self, bytes: usize) -> Result<()> {
        self.write_file("memory.max", &bytes.to_string())
    }

    /// Set CPU quota (microseconds per period)
    fn set_cpu_quota(&self, quota_us: u64, period_us: u64) -> Result<()> {
        self.write_file("cpu.max", &format!("{} {}", quota_us, period_us))
    }
}

impl Drop for CgroupController {
    fn drop(&mut self) {
        if self.owned {
            // Best effort cleanup - remove cgroup directory
            // In practice, this requires the cgroup to be empty
        }
    }
}

/// Linux thread handle
struct LinuxThreadHandle {
    /// Thread ID
    tid:     ffi::pthread_t,
    /// Task being executed
    task:    Arc<WrtMutex<Option<WasmTask>>>,
    /// `Result` storage
    result:  Arc<WrtMutex<Option<Result<Vec<u8>>>>>,
    /// Running flag
    running: Arc<AtomicBool>,
    /// Thread statistics
    stats:   Arc<WrtMutex<ThreadStats>>,
    /// Cgroup controller
    cgroup:  Option<Arc<CgroupController>>,
}

impl PlatformThreadHandle for LinuxThreadHandle {
    fn join(self: Box<Self>) -> Result<Vec<u8>> {
        // Join the thread
        let mut retval: *mut core::ffi::c_void = core::ptr::null_mut();
        let result = unsafe { ffi::pthread_join(self.tid, &mut retval) };

        if result != 0 {
            return Err(Error::runtime_execution_error(
                "Failed to set thread priority",
            ));
        }

        // Get the result
        let result = self.result.lock();
        match &*result {
            Some(Ok(data)) => Ok(data.clone()),
            Some(Err(e)) => Err(e.clone()),
            None => Err(Error::new(
                ErrorCategory::Platform,
                1,
                "No result available",
            )),
        }
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    fn get_stats(&self) -> Result<ThreadStats> {
        Ok(self.stats.lock().clone())
    }
}

/// Thread context passed to pthread
struct ThreadContext {
    /// Task to execute
    task:     WasmTask,
    /// `Result` storage
    result:   Arc<WrtMutex<Option<Result<Vec<u8>>>>>,
    /// Running flag
    running:  Arc<AtomicBool>,
    /// Stats
    stats:    Arc<WrtMutex<ThreadStats>>,
    /// Executor function
    executor: Arc<dyn Fn(WasmTask) -> Result<Vec<u8>> + Send + Sync>,
    /// Cgroup controller
    cgroup:   Option<Arc<CgroupController>>,
}

/// Linux thread pool implementation
pub struct LinuxThreadPool {
    /// Configuration
    config:         ThreadPoolConfig,
    /// Base cgroup for the pool
    base_cgroup:    Option<CgroupController>,
    /// Active threads
    active_threads: Arc<WrtRwLock<BTreeMap<u64, Box<dyn PlatformThreadHandle>>>>,
    /// Thread statistics
    stats:          Arc<WrtMutex<ThreadPoolStats>>,
    /// Next thread ID
    next_thread_id: AtomicU64,
    /// Shutdown flag
    shutdown:       AtomicBool,
    /// Task executor
    executor:       Arc<dyn Fn(WasmTask) -> Result<Vec<u8>> + Send + Sync>,
}

impl LinuxThreadPool {
    /// Create new Linux thread pool
    pub fn new(config: ThreadPoolConfig) -> Result<Self> {
        // Try to create base cgroup if we have memory limits
        let base_cgroup = if config.memory_limit_per_thread.is_some() {
            match CgroupController::new("wasm_threads") {
                Ok(cgroup) => {
                    // Set overall memory limit for the pool
                    if let Some(limit) = config.memory_limit_per_thread {
                        let total_limit = limit.saturating_mul(config.max_threads);
                        let _ = cgroup.set_memory_limit(total_limit);
                    }
                    Some(cgroup)
                },
                Err(_) => None, // Cgroups not available, continue without
            }
        } else {
            None
        };

        // Create a simple executor for now
        let executor = Arc::new(|_task: WasmTask| -> Result<Vec<u8>> { Ok(vec![]) });

        Ok(Self {
            config,
            base_cgroup,
            active_threads: Arc::new(WrtRwLock::new(BTreeMap::new())),
            stats: Arc::new(WrtMutex::new(ThreadPoolStats::default())),
            next_thread_id: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            executor,
        })
    }

    /// Set the task executor
    pub fn set_executor<F>(&mut self, executor: F)
    where
        F: Fn(WasmTask) -> Result<Vec<u8>> + Send + Sync + 'static,
    {
        self.executor = Arc::new(executor);
    }

    /// Map ThreadPriority to Linux nice value (-20 to 19)
    fn map_priority_to_nice(&self, priority: ThreadPriority) -> i32 {
        match priority {
            ThreadPriority::Idle => 19,
            ThreadPriority::Low => 10,
            ThreadPriority::Normal => 0,
            ThreadPriority::High => -10,
            ThreadPriority::Realtime => -20,
        }
    }

    /// Apply CPU affinity
    fn set_cpu_affinity(cpu_set: &CpuSet) -> Result<()> {
        let mut mask: ffi::cpu_set_t = [0; 16];
        ffi::CPU_ZERO(&mut mask);

        // Set CPUs in mask
        for cpu in 0..64 {
            if cpu_set.contains(cpu) {
                ffi::CPU_SET(cpu, &mut mask);
            }
        }

        let result =
            unsafe { ffi::sched_setaffinity(0, core::mem::size_of::<ffi::cpu_set_t>(), &mask) };

        if result != 0 {
            return Err(Error::runtime_execution_error("Thread join failed"));
        }

        Ok(())
    }

    /// Set thread priority using nice value
    fn set_thread_priority(priority: ThreadPriority) -> Result<()> {
        let nice_value = match priority {
            ThreadPriority::Idle => 19,
            ThreadPriority::Low => 10,
            ThreadPriority::Normal => 0,
            ThreadPriority::High => -10,
            ThreadPriority::Realtime => -20,
        };

        // For real-time priorities, use SCHED_FIFO
        if priority == ThreadPriority::Realtime {
            let param = ffi::sched_param { sched_priority: 50 }; // Mid-range RT priority
            let result = unsafe { ffi::sched_setscheduler(0, SchedPolicy::Fifo as i32, &param) };

            if result != 0 {
                // Fall back to nice value if RT scheduling fails
                unsafe {
                    ffi::nice(nice_value);
                }
            }
        } else {
            unsafe {
                ffi::nice(nice_value);
            }
        }

        Ok(())
    }
}

/// Thread entry point
extern "C" fn thread_entry(arg: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    let context = unsafe { Box::from_raw(arg as *mut ThreadContext) };

    // Add thread to cgroup if available
    if let Some(ref cgroup) = context.cgroup {
        let _ = cgroup.add_thread();
    }

    // Set CPU affinity if specified
    if let Some(ref cpu_set) = context.task.cpu_affinity {
        let _ = LinuxThreadPool::set_cpu_affinity(cpu_set);
    }

    // Set thread priority
    let _ = LinuxThreadPool::set_thread_priority(context.task.priority);

    // Mark as running
    context.running.store(true, Ordering::Release);

    // Execute the task
    let result = (context.executor)(context.task);

    // Store result
    *context.result.lock() = Some(result);

    // Mark as not running
    context.running.store(false, Ordering::Release);

    core::ptr::null_mut()
}

impl PlatformThreadPool for LinuxThreadPool {
    fn configure(&mut self, config: ThreadPoolConfig) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn spawn_wasm_thread(&self, task: WasmTask) -> Result<ThreadHandle> {
        // Check if shutting down
        if self.shutdown.load(Ordering::Acquire) {
            return Err(Error::runtime_execution_error(
                "Thread pool is shutting down",
            ));
        }

        // Check thread limit
        let active_count = self.active_threads.read().len();
        if active_count >= self.config.max_threads {
            return Err(Error::new(
                ErrorCategory::Resource,
                1,
                "Thread pool has reached maximum thread limit",
            ));
        }

        // Get thread ID
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::AcqRel);

        // Create per-thread cgroup if needed
        let thread_cgroup = if self.base_cgroup.is_some() {
            match CgroupController::new(&format!("wasm_threads/thread_{}", thread_id)) {
                Ok(mut cgroup) => {
                    // Set memory limit
                    if let Some(limit) = task.memory_limit.or(self.config.memory_limit_per_thread) {
                        let _ = cgroup.set_memory_limit(limit);
                    }

                    // Set CPU quota if deadline is specified
                    if let Some(deadline) = task.deadline {
                        let quota_us = deadline.as_micros() as u64;
                        let period_us = 1_000_000; // 1 second period
                        let _ = cgroup.set_cpu_quota(quota_us, period_us);
                    }

                    Some(Arc::new(cgroup))
                },
                Err(_) => None,
            }
        } else {
            None
        };

        // Create thread context
        let result = Arc::new(WrtMutex::new(None));
        let running = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(WrtMutex::new(ThreadStats::default()));

        let context = Box::new(ThreadContext {
            task,
            result: result.clone(),
            running: running.clone(),
            stats: stats.clone(),
            executor: self.executor.clone(),
            cgroup: thread_cgroup.clone(),
        });

        // Create thread
        let mut tid: ffi::pthread_t = 0;
        let context_ptr = Box::into_raw(context);

        let create_result = unsafe {
            ffi::pthread_create(
                &mut tid,
                core::ptr::null(),
                thread_entry,
                context_ptr as *mut core::ffi::c_void,
            )
        };

        if create_result != 0 {
            // Clean up context on failure
            unsafe {
                let _ = Box::from_raw(context_ptr);
            }
            return Err(Error::runtime_execution_error("Failed to create thread"));
        }

        // Create handle
        let handle = Box::new(LinuxThreadHandle {
            tid,
            task: Arc::new(WrtMutex::new(None)),
            result,
            running,
            stats,
            cgroup: thread_cgroup,
        });

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.active_threads += 1;
            stats.total_spawned += 1;
        }

        Ok(ThreadHandle {
            id:              thread_id,
            platform_handle: handle,
        })
    }

    fn get_stats(&self) -> ThreadPoolStats {
        self.stats.lock().clone()
    }

    fn shutdown(&mut self, timeout: Duration) -> Result<()> {
        // Set shutdown flag
        self.shutdown.store(true, Ordering::Release);

        // Wait for threads to complete
        let start = std::time::Instant::now();
        while self.active_threads.read().len() > 0 && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_thread_pool_basic() {
        let config = ThreadPoolConfig {
            max_threads: 4,
            priority_range: (ThreadPriority::Low, ThreadPriority::High),
            ..Default::default()
        };

        let mut pool = LinuxThreadPool::new(config).unwrap();

        // Set a test executor
        pool.set_executor(|task| Ok(task.args));

        // Spawn a thread
        let task = WasmTask {
            id:           1,
            function_id:  100,
            args:         vec![1, 2, 3, 4],
            priority:     ThreadPriority::Normal,
            stack_size:   None,
            memory_limit: None,
            cpu_affinity: None,
            deadline:     None,
        };

        let handle = pool.spawn_wasm_thread(task).unwrap();

        // Join and verify result
        let result = handle.join().unwrap();
        assert_eq!(result, vec![1, 2, 3, 4]);

        // Check stats
        let stats = pool.get_stats();
        assert_eq!(stats.total_spawned, 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_thread_pool_with_cpu_affinity() {
        let config = ThreadPoolConfig::default();
        let mut pool = LinuxThreadPool::new(config).unwrap();

        pool.set_executor(|_| Ok(vec![]));

        // Create CPU set for CPU 0
        let mut cpu_set = CpuSet::new();
        cpu_set.add(0);

        let task = WasmTask {
            id:           2,
            function_id:  200,
            args:         vec![],
            priority:     ThreadPriority::High,
            stack_size:   Some(4 * 1024 * 1024),
            memory_limit: Some(32 * 1024 * 1024),
            cpu_affinity: Some(cpu_set),
            deadline:     Some(Duration::from_millis(100)),
        };

        let handle = pool.spawn_wasm_thread(task).unwrap();
        let _ = handle.join().unwrap();
    }
}
