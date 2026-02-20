//! Platform-agnostic software watchdog for monitoring WASM execution.
//!
//! This module provides a lightweight software watchdog that can monitor
//! WASM module execution and detect hangs or excessive runtime.
//!
//! This module requires the `std` feature since it uses std::thread and
//! std::time.

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    collections::BTreeMap,
    string::String,
    sync::Arc,
};
#[cfg(feature = "std")]
use alloc::{
    collections::BTreeMap,
    string::String,
    sync::Arc,
};
use core::{
    sync::atomic::{
        AtomicBool,
        AtomicU64,
        Ordering,
    },
    time::Duration,
};

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_sync::{
    WrtMutex,
    WrtRwLock,
};

/// Watchdog configuration
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// Default timeout for watched operations
    pub default_timeout:   Duration,
    /// Check interval for the watchdog thread
    pub check_interval:    Duration,
    /// Whether to automatically kill timed-out tasks
    pub auto_kill:         bool,
    /// Maximum number of concurrent watched tasks
    pub max_watched_tasks: usize,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            default_timeout:   Duration::from_secs(60),
            check_interval:    Duration::from_millis(100),
            auto_kill:         false,
            max_watched_tasks: 1000,
        }
    }
}

/// Watchdog action to take on timeout
#[derive(Debug)]
pub enum WatchdogAction {
    /// Just log the timeout
    Log,
    /// Kill the task (platform-specific)
    Kill,
}

impl Clone for WatchdogAction {
    fn clone(&self) -> Self {
        match self {
            WatchdogAction::Log => WatchdogAction::Log,
            WatchdogAction::Kill => WatchdogAction::Kill,
        }
    }
}

/// Watched task identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WatchedTaskId(pub u64);

/// Information about a watched task
#[derive(Debug)]
struct WatchedTask {
    id:             WatchedTaskId,
    name:           String,
    _start_time:    u64, // Timestamp in milliseconds
    timeout:        Duration,
    last_heartbeat: WrtMutex<u64>, // Timestamp in milliseconds
    action:         WatchdogAction,
    active:         AtomicBool,
}

/// Software watchdog for monitoring WASM execution
pub struct SoftwareWatchdog {
    /// Configuration
    config:          WatchdogConfig,
    /// Watched tasks
    tasks:           Arc<WrtRwLock<BTreeMap<WatchedTaskId, Arc<WatchedTask>>>>,
    /// Next task ID
    next_task_id:    AtomicU64,
    /// Watchdog thread handle
    #[cfg(feature = "std")]
    watchdog_thread: WrtMutex<Option<std::thread::JoinHandle<()>>>,
    /// Running flag
    running:         Arc<AtomicBool>,
}

impl SoftwareWatchdog {
    /// Create new software watchdog
    pub fn new(config: WatchdogConfig) -> Self {
        Self {
            config,
            tasks: Arc::new(WrtRwLock::new(BTreeMap::new())),
            next_task_id: AtomicU64::new(1),
            #[cfg(feature = "std")]
            watchdog_thread: WrtMutex::new(None),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the watchdog monitoring thread
    pub fn start(&self) -> Result<()> {
        if self.running.load(Ordering::Acquire) {
            return Ok(());
        }

        self.running.store(true, Ordering::Release);

        let tasks = self.tasks.clone();
        let running = self.running.clone();
        let check_interval = self.config.check_interval;
        let auto_kill = self.config.auto_kill;

        let thread = std::thread::spawn(move || {
            while running.load(Ordering::Acquire) {
                // Check all tasks
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let tasks_snapshot = tasks.read().clone();

                for task in tasks_snapshot.values() {
                    if !task.active.load(Ordering::Acquire) {
                        continue;
                    }

                    let last_heartbeat = *task.last_heartbeat.lock();
                    let elapsed_ms = now.saturating_sub(last_heartbeat);
                    let elapsed = Duration::from_millis(elapsed_ms);

                    if elapsed > task.timeout {
                        // Timeout detected
                        eprintln!(
                            "Watchdog: Task '{name}' (ID: {id:?}) timed out after {elapsed:?}",
                            name = task.name,
                            id = task.id,
                            elapsed = elapsed
                        );

                        // Execute action
                        match &task.action {
                            WatchdogAction::Log => {
                                // Already logged above
                            },
                            WatchdogAction::Kill => {
                                if auto_kill {
                                    // Platform-specific kill logic would go here
                                    eprintln!("Watchdog: Would kill task {name}", name = task.name);
                                }
                            },
                        }

                        // Mark as inactive after timeout
                        task.active.store(false, Ordering::Release);
                    }
                }

                std::thread::sleep(check_interval);
            }
        });

        *self.watchdog_thread.lock() = Some(thread);
        Ok(())
    }

    /// Stop the watchdog
    pub fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::Release);

        if let Some(thread) = self.watchdog_thread.lock().take() {
            let _ = thread.join();
        }

        Ok(())
    }

    /// Watch a new task
    pub fn watch_task(
        &self,
        name: impl Into<String>,
        timeout: Option<Duration>,
        action: WatchdogAction,
    ) -> Result<WatchdogHandle<'_>> {
        let task_id = WatchedTaskId(self.next_task_id.fetch_add(1, Ordering::AcqRel));
        // Get current timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let task = Arc::new(WatchedTask {
            id: task_id,
            name: name.into(),
            _start_time: now,
            timeout: timeout.unwrap_or(self.config.default_timeout),
            last_heartbeat: WrtMutex::new(now),
            action,
            active: AtomicBool::new(true),
        });

        // Check limit
        {
            let tasks = self.tasks.read();
            if tasks.len() >= self.config.max_watched_tasks {
                return Err(Error::runtime_execution_error(
                    "Maximum watched tasks limit reached",
                ));
            }
        }

        self.tasks.write().insert(task_id, task.clone());

        Ok(WatchdogHandle {
            task:     Some(task),
            watchdog: self,
        })
    }

    /// Send heartbeat for a task
    pub fn heartbeat(&self, task_id: WatchedTaskId) -> Result<()> {
        let tasks = self.tasks.read();
        let task = tasks
            .get(&task_id)
            .ok_or_else(|| Error::new(ErrorCategory::Validation, 1, "Task not found"))?;

        if task.active.load(Ordering::Acquire) {
            // Update heartbeat timestamp
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            *task.last_heartbeat.lock() = now;
            Ok(())
        } else {
            Err(Error::runtime_execution_error("Task is not active"))
        }
    }

    /// Cancel watching a task
    pub fn cancel_task(&self, task_id: WatchedTaskId) -> Result<()> {
        if let Some(task) = self.tasks.write().remove(&task_id) {
            task.active.store(false, Ordering::Release);
            Ok(())
        } else {
            Err(Error::new(ErrorCategory::Validation, 1, "Task not found"))
        }
    }
}

/// RAII handle for watched tasks
pub struct WatchdogHandle<'a> {
    task:     Option<Arc<WatchedTask>>,
    watchdog: &'a SoftwareWatchdog,
}

impl<'a> WatchdogHandle<'a> {
    /// Send a heartbeat
    pub fn heartbeat(&self) -> Result<()> {
        if let Some(task) = &self.task {
            self.watchdog.heartbeat(task.id)
        } else {
            Err(Error::runtime_execution_error("Task is not active"))
        }
    }

    /// Get the task ID
    pub fn task_id(&self) -> Option<WatchedTaskId> {
        self.task.as_ref().map(|t| t.id)
    }

    /// Cancel and consume the handle
    pub fn cancel(mut self) -> Result<()> {
        if let Some(task) = self.task.take() {
            self.watchdog.cancel_task(task.id)
        } else {
            Ok(())
        }
    }
}

impl<'a> Drop for WatchdogHandle<'a> {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            let _ = self.watchdog.cancel_task(task.id);
        }
    }
}

/// Integration with WASM execution
pub trait WatchdogIntegration {
    /// Start watching a WASM module execution
    fn watch_wasm_execution(
        &self,
        module_name: &str,
        timeout: Duration,
    ) -> Result<WatchdogHandle<'_>>;

    /// Create a scoped watchdog for a function
    fn watch_function<F, R>(&self, name: &str, timeout: Duration, f: F) -> Result<R>
    where
        F: FnOnce(&WatchdogHandle) -> Result<R>;
}

impl WatchdogIntegration for SoftwareWatchdog {
    fn watch_wasm_execution(
        &self,
        module_name: &str,
        timeout: Duration,
    ) -> Result<WatchdogHandle<'_>> {
        self.watch_task(
            format!("WASM module: {module_name}"),
            Some(timeout),
            WatchdogAction::Log,
        )
    }

    fn watch_function<F, R>(&self, name: &str, timeout: Duration, f: F) -> Result<R>
    where
        F: FnOnce(&WatchdogHandle) -> Result<R>,
    {
        let handle = self.watch_task(name, Some(timeout), WatchdogAction::Log)?;
        let result = f(&handle);
        handle.cancel()?;
        result
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[test]
    fn test_watchdog_basic() {
        let config = WatchdogConfig {
            default_timeout:   Duration::from_millis(100),
            check_interval:    Duration::from_millis(10),
            auto_kill:         false,
            max_watched_tasks: 10,
        };

        let watchdog = SoftwareWatchdog::new(config);
        watchdog.start().unwrap();

        // Watch a task
        let handle = watchdog.watch_task("test_task", None, WatchdogAction::Log).unwrap();

        // Send heartbeats
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(50));
            handle.heartbeat().unwrap();
        }

        // Cancel the task
        handle.cancel().unwrap();

        watchdog.stop().unwrap();
    }

    #[test]
    fn test_watchdog_timeout() {
        let config = WatchdogConfig {
            default_timeout:   Duration::from_millis(50),
            check_interval:    Duration::from_millis(10),
            auto_kill:         true,
            max_watched_tasks: 10,
        };

        let watchdog = SoftwareWatchdog::new(config);
        watchdog.start().unwrap();

        // Watch a task with kill action
        let _handle = watchdog
            .watch_task(
                "timeout_task",
                Some(Duration::from_millis(50)),
                WatchdogAction::Kill,
            )
            .unwrap();

        // Don't send heartbeats, let it timeout
        std::thread::sleep(Duration::from_millis(100));

        // Task should have timed out (verified through logs)

        watchdog.stop().unwrap();
    }

    #[test]
    fn test_watchdog_integration() {
        let watchdog = SoftwareWatchdog::new(WatchdogConfig::default());
        watchdog.start().unwrap();

        // Test function watching
        let result = watchdog
            .watch_function("test_function", Duration::from_secs(1), |handle| {
                // Simulate some work with heartbeats
                for i in 0..3 {
                    std::thread::sleep(Duration::from_millis(100));
                    handle.heartbeat()?;
                }
                Ok::<_, Error>(42)
            })
            .unwrap();

        assert_eq!(result, 42);

        watchdog.stop().unwrap();
    }
}
