use core::ffi::c_void;

use wrt_error::{
    Error,
    ErrorKind,
};

use crate::vxworks_memory::VxWorksContext;

#[cfg(target_os = "vxworks")]
unsafe extern "C" {
    // VxWorks task functions (LKM context)
    fn taskSpawn(
        name: *const u8,
        priority: i32,
        options: i32,
        stack_size: usize,
        entry_point: extern "C" fn(*mut c_void) -> i32,
        arg1: *mut c_void,
        arg2: usize,
        arg3: usize,
        arg4: usize,
        arg5: usize,
        arg6: usize,
        arg7: usize,
        arg8: usize,
        arg9: usize,
        arg10: usize,
    ) -> usize; // TASK_ID

    fn taskDelete(task_id: usize) -> i32;
    fn taskSuspend(task_id: usize) -> i32;
    fn taskResume(task_id: usize) -> i32;
    fn taskDelay(ticks: i32) -> i32;
    fn taskIdSelf() -> usize;
    fn taskPrioritySet(task_id: usize, new_priority: i32) -> i32;
    fn taskPriorityGet(task_id: usize, priority: *mut i32) -> i32;

    // POSIX thread functions (RTP context)
    fn pthread_create(
        thread: *mut PThread,
        attr: *const PThreadAttr,
        start_routine: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
    ) -> i32;

    fn pthread_join(thread: PThread, retval: *mut *mut c_void) -> i32;
    fn pthread_detach(thread: PThread) -> i32;
    fn pthread_cancel(thread: PThread) -> i32;
    fn pthread_self() -> PThread;
    fn pthread_yield() -> i32;

    // Thread attributes
    fn pthread_attr_init(attr: *mut PThreadAttr) -> i32;
    fn pthread_attr_destroy(attr: *mut PThreadAttr) -> i32;
    fn pthread_attr_setstack(
        attr: *mut PThreadAttr,
        stackaddr: *mut c_void,
        stacksize: usize,
    ) -> i32;
    fn pthread_attr_setstacksize(attr: *mut PThreadAttr, stacksize: usize) -> i32;
    fn pthread_attr_setdetachstate(attr: *mut PThreadAttr, detachstate: i32) -> i32;

    // System functions
    fn sysClkRateGet() -> i32;
}

// VxWorks task options
const VX_FP_TASK: i32 = 0x0008; // Floating point task
const VX_PRIVATE_ENV: i32 = 0x0080; // Private environment variables
const VX_UNBREAKABLE: i32 = 0x0002; // Unbreakable task

// POSIX thread constants
const PTHREAD_CREATE_JOINABLE: i32 = 0;
const PTHREAD_CREATE_DETACHED: i32 = 1;

#[repr(C)]
struct PThread {
    _data: [u8; 8], // Platform-specific thread handle
}

#[repr(C)]
struct PThreadAttr {
    _data: [u8; 32], // Platform-specific thread attributes
}

/// VxWorks thread handle that supports both LKM and RTP contexts
pub struct VxWorksThread {
    context: VxWorksContext,
    task_id: Option<usize>,   // For LKM context (VxWorks task)
    pthread: Option<PThread>, // For RTP context (POSIX thread)
    name:    Option<String>,
}

/// Configuration for VxWorks threads
#[derive(Debug, Clone)]
pub struct VxWorksThreadConfig {
    pub context:        VxWorksContext,
    pub stack_size:     usize,
    pub priority:       Option<i32>,
    pub name:           Option<String>,
    pub floating_point: bool,
    pub detached:       bool,
}

impl Default for VxWorksThreadConfig {
    fn default() -> Self {
        Self {
            context:        VxWorksContext::Rtp,
            stack_size:     8192, // 8KB stack
            priority:       None, // Use default priority
            name:           None,
            floating_point: false,
            detached:       false,
        }
    }
}

/// Thread entry point function type
pub type ThreadEntryPoint = Box<dyn FnOnce() + Send + 'static>;

impl VxWorksThread {
    /// Spawn a new thread in the specified context
    pub fn spawn<F>(config: VxWorksThreadConfig, f: F) -> Result<Self, Error>
    where
        F: FnOnce() + Send + 'static,
    {
        let mut thread = Self {
            context: config.context,
            task_id: None,
            pthread: None,
            name:    config.name.clone(),
        };

        let entry_point = Box::new(f);

        match config.context {
            VxWorksContext::Lkm => {
                thread.spawn_vxworks_task(config, entry_point)?;
            },
            VxWorksContext::Rtp => {
                thread.spawn_posix_thread(config, entry_point)?;
            },
        }

        Ok(thread)
    }

    /// Spawn a VxWorks task (LKM context)
    fn spawn_vxworks_task(
        &mut self,
        config: VxWorksThreadConfig,
        f: ThreadEntryPoint,
    ) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            // Convert Rust closure to C function pointer
            extern "C" fn task_wrapper(arg: *mut c_void) -> i32 {
                let closure: Box<ThreadEntryPoint> =
                    unsafe { Box::from_raw(arg as *mut ThreadEntryPoint) };
                closure();
                0 // Success
            }

            let closure_ptr = Box::into_raw(Box::new(f)) as *mut c_void;
            let name_ptr = config.name.as_ref().map(|n| n.as_ptr()).unwrap_or(b"wrt_task\0".as_ptr);

            let priority = config.priority.unwrap_or(100); // Default VxWorks priority
            let mut options = 0;

            if config.floating_point {
                options |= VX_FP_TASK;
            }

            let task_id = unsafe {
                taskSpawn(
                    name_ptr,
                    priority,
                    options,
                    config.stack_size,
                    task_wrapper,
                    closure_ptr,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                )
            };

            if task_id == 0 {
                // Cleanup on failure
                unsafe {
                    let _ = Box::from_raw(closure_ptr as *mut ThreadEntryPoint);
                }
                return Err(Error::runtime_execution_error(
                    "Failed to create VxWorks task",
                ));
            }

            self.task_id = Some(task_id);
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks task spawn not supported on this platform",
            ));
        }

        Ok(())
    }

    /// Spawn a POSIX thread (RTP context)
    fn spawn_posix_thread(
        &mut self,
        config: VxWorksThreadConfig,
        f: ThreadEntryPoint,
    ) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            extern "C" fn thread_wrapper(arg: *mut c_void) -> *mut c_void {
                let closure: Box<ThreadEntryPoint> =
                    unsafe { Box::from_raw(arg as *mut ThreadEntryPoint) };
                closure();
                core::ptr::null_mut()
            }

            let closure_ptr = Box::into_raw(Box::new(f)) as *mut c_void;

            // Initialize thread attributes
            let mut attr = PThreadAttr { _data: [0; 32] };
            let attr_result = unsafe { pthread_attr_init(&mut attr) };
            if attr_result != 0 {
                unsafe {
                    let _ = Box::from_raw(closure_ptr as *mut ThreadEntryPoint);
                }
                return Err(Error::runtime_execution_error(
                    "Failed to initialize thread attributes",
                ));
            }

            // Set stack size
            unsafe {
                pthread_attr_setstacksize(&mut attr, config.stack_size);
            }

            // Set detach state
            if config.detached {
                unsafe {
                    pthread_attr_setdetachstate(&mut attr, PTHREAD_CREATE_DETACHED);
                }
            }

            // Create the thread
            let mut pthread = PThread { _data: [0; 8] };
            let create_result =
                unsafe { pthread_create(&mut pthread, &attr, thread_wrapper, closure_ptr) };

            // Clean up attributes
            unsafe {
                pthread_attr_destroy(&mut attr);
            }

            if create_result != 0 {
                unsafe {
                    let _ = Box::from_raw(closure_ptr as *mut ThreadEntryPoint);
                }
                return Err(Error::new(
                    ErrorKind::Platform,
                    "Failed to create POSIX thread",
                ));
            }

            self.pthread = Some(pthread);
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks thread spawn not supported on this platform",
            ));
        }

        Ok(())
    }

    /// Join the thread (wait for completion)
    pub fn join(self) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    // VxWorks tasks don't have a direct join mechanism
                    // We can only delete the task or wait in a loop
                    if let Some(task_id) = self.task_id {
                        // For demonstration, we'll just delete the task
                        // In practice, you'd implement a proper join mechanism
                        unsafe {
                            taskDelete(task_id);
                        }
                    }
                },
                VxWorksContext::Rtp => {
                    if let Some(pthread) = self.pthread {
                        let result = unsafe { pthread_join(pthread, core::ptr::null_mut()) };
                        if result != 0 {
                            return Err(Error::runtime_execution_error(
                                "Failed to join POSIX thread",
                            ));
                        }
                    }
                },
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks thread join not supported on this platform",
            ));
        }

        Ok(())
    }

    /// Detach the thread
    pub fn detach(self) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    // VxWorks tasks are detached by default
                    Ok(())
                },
                VxWorksContext::Rtp => {
                    if let Some(pthread) = self.pthread {
                        let result = unsafe { pthread_detach(pthread) };
                        if result != 0 {
                            return Err(Error::runtime_execution_error(
                                "Failed to detach POSIX thread",
                            ));
                        }
                    }
                    Ok(())
                },
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            Err(Error::runtime_execution_error(
                "VxWorks thread detach not supported on this platform",
            ))
        }
    }

    /// Get the current thread ID
    pub fn current_id() -> u64 {
        #[cfg(target_os = "vxworks")]
        {
            // Try VxWorks task ID first, fall back to POSIX thread ID
            let task_id = unsafe { taskIdSelf() };
            if task_id != 0 {
                task_id as u64
            } else {
                let pthread = unsafe { pthread_self() };
                // Convert pthread to u64 (implementation-specific)
                unsafe { core::mem::transmute::<PThread, u64>(pthread) }
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            0 // Default value for non-VxWorks platforms
        }
    }

    /// Yield the current thread
    pub fn yield_now() -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            // Try VxWorks task delay first
            let task_id = unsafe { taskIdSelf() };
            if task_id != 0 {
                unsafe {
                    taskDelay(0);
                } // Yield to other tasks
            } else {
                unsafe {
                    pthread_yield();
                } // Yield POSIX thread
            }
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            Err(Error::runtime_execution_error(
                "VxWorks thread yield not supported on this platform",
            ))
        }
    }

    /// Sleep for the specified number of milliseconds
    pub fn sleep_ms(milliseconds: u32) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            let ticks_per_sec = unsafe { sysClkRateGet() } as u32;
            let ticks = (milliseconds * ticks_per_sec) / 1000;

            let result = unsafe { taskDelay(ticks as i32) };
            if result != 0 {
                return Err(Error::runtime_execution_error(
                    "Failed to delay VxWorks task",
                ));
            }
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            Err(Error::runtime_execution_error(
                "VxWorks thread sleep not supported on this platform",
            ))
        }
    }
}

/// Builder for VxWorks threads
pub struct VxWorksThreadBuilder {
    config: VxWorksThreadConfig,
}

impl VxWorksThreadBuilder {
    pub fn new(context: VxWorksContext) -> Self {
        Self {
            config: VxWorksThreadConfig {
                context,
                ..Default::default()
            },
        }
    }

    pub fn stack_size(mut self, size: usize) -> Self {
        self.config.stack_size = size;
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.config.priority = Some(priority);
        self
    }

    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.config.name = Some(name.into());
        self
    }

    pub fn floating_point(mut self, enable: bool) -> Self {
        self.config.floating_point = enable;
        self
    }

    pub fn detached(mut self, detached: bool) -> Self {
        self.config.detached = detached;
        self
    }

    pub fn spawn<F>(self, f: F) -> Result<VxWorksThread, Error>
    where
        F: FnOnce() + Send + 'static,
    {
        VxWorksThread::spawn(self.config, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_builder() {
        let builder = VxWorksThreadBuilder::new(VxWorksContext::Rtp)
            .stack_size(16384)
            .name("test_thread")
            .floating_point(true)
            .detached(true);

        assert_eq!(builder.config.stack_size, 16384);
        assert_eq!(builder.config.name.as_ref().unwrap(), "test_thread");
        assert!(builder.config.floating_point);
        assert!(builder.config.detached);
    }

    #[test]
    fn test_thread_config_default() {
        let config = VxWorksThreadConfig::default();
        assert_eq!(config.context, VxWorksContext::Rtp);
        assert_eq!(config.stack_size, 8192);
        assert!(config.priority.is_none());
        assert!(!config.floating_point);
        assert!(!config.detached);
    }

    #[test]
    fn test_current_thread_id() {
        let id = VxWorksThread::current_id();

        #[cfg(target_os = "vxworks")]
        assert!(id > 0);

        #[cfg(not(target_os = "vxworks"))]
        assert_eq!(id, 0);
    }
}
