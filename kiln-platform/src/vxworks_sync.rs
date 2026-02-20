use core::{
    sync::atomic::{
        AtomicU32,
        Ordering,
    },
    time::Duration,
};

use wrt_error::{
    Error,
    ErrorKind,
};

use crate::FutexLike;

#[cfg(target_os = "vxworks")]
unsafe extern "C" {
    // VxWorks semaphore functions (both LKM and RTP)
    fn semBCreate(options: i32, initial_state: i32) -> usize; // SEM_ID
    fn semMCreate(options: i32) -> usize; // Mutex semaphore
    fn semCCreate(options: i32, initial_count: i32) -> usize; // Counting semaphore
    fn semDelete(sem_id: usize) -> i32;
    fn semTake(sem_id: usize, timeout: i32) -> i32;
    fn semGive(sem_id: usize) -> i32;
    fn semFlush(sem_id: usize) -> i32;

    // POSIX semaphores (RTP context)
    fn sem_init(sem: *mut PosixSem, pshared: i32, value: u32) -> i32;
    fn sem_destroy(sem: *mut PosixSem) -> i32;
    fn sem_wait(sem: *mut PosixSem) -> i32;
    fn sem_trywait(sem: *mut PosixSem) -> i32;
    fn sem_timedwait(sem: *mut PosixSem, timeout: *const TimeSpec) -> i32;
    fn sem_post(sem: *mut PosixSem) -> i32;

    // Task/thread functions
    fn taskIdSelf() -> usize;
    fn taskDelay(ticks: i32) -> i32;
    fn sysClkRateGet() -> i32;
}

// VxWorks semaphore options
const SEM_Q_FIFO: i32 = 0x00;
const SEM_Q_PRIORITY: i32 = 0x01;
const SEM_DELETE_SAFE: i32 = 0x04;
const SEM_INVERSION_SAFE: i32 = 0x08;

// Timeout values
const WAIT_FOREVER: i32 = -1;
const NO_WAIT: i32 = 0;

// Error codes
const OK: i32 = 0;
const ERROR: i32 = -1;

#[repr(C)]
struct PosixSem {
    _data: [u8; 16], // Platform-specific semaphore data
}

#[repr(C)]
struct TimeSpec {
    tv_sec:  i64,
    tv_nsec: i64,
}

use super::vxworks_memory::VxWorksContext;

/// VxWorks synchronization primitive supporting both LKM and RTP contexts
pub struct VxWorksFutex {
    context:      VxWorksContext,
    atomic_value: AtomicU32,
    sem_id:       Option<usize>,
    posix_sem:    Option<PosixSem>,
}

impl VxWorksFutex {
    /// Create a new VxWorks futex-like synchronization primitive
    pub fn new(context: VxWorksContext, initial_value: u32) -> Result<Self, Error> {
        let atomic_value = AtomicU32::new(initial_value);

        let mut futex = Self {
            context,
            atomic_value,
            sem_id: None,
            posix_sem: None,
        };

        // Initialize appropriate synchronization primitive based on context
        match context {
            VxWorksContext::Lkm => {
                futex.init_vxworks_semaphore()?;
            },
            VxWorksContext::Rtp => {
                futex.init_posix_semaphore(initial_value)?;
            },
        }

        Ok(futex)
    }

    /// Initialize VxWorks semaphore for LKM context
    fn init_vxworks_semaphore(&mut self) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            // Create a binary semaphore with priority queuing and inversion safety
            let options = SEM_Q_PRIORITY | SEM_DELETE_SAFE | SEM_INVERSION_SAFE;
            let sem_id = unsafe { semBCreate(options, 0) }; // Start empty

            if sem_id == 0 {
                return Err(Error::runtime_execution_error(
                    "Failed to create VxWorks semaphore",
                ));
            }

            self.sem_id = Some(sem_id);
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks semaphore not supported on this platform",
            ));
        }

        Ok(())
    }

    /// Initialize POSIX semaphore for RTP context
    fn init_posix_semaphore(&mut self, initial_value: u32) -> Result<(), Error> {
        #[cfg(target_os = "vxworks")]
        {
            let mut posix_sem = PosixSem { _data: [0; 16] };

            let result = unsafe { sem_init(&mut posix_sem, 0, initial_value) };
            if result != 0 {
                return Err(Error::runtime_execution_error(
                    "Failed to initialize POSIX semaphore",
                ));
            }

            self.posix_sem = Some(posix_sem);
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "POSIX semaphore not supported on this platform",
            ));
        }

        Ok(())
    }

    /// Convert duration to VxWorks ticks
    fn duration_to_ticks(duration: Duration) -> i32 {
        #[cfg(target_os = "vxworks")]
        {
            let ticks_per_sec = unsafe { sysClkRateGet() } as u64;
            let total_ms = duration.as_millis() as u64;

            if total_ms == 0 {
                return NO_WAIT;
            }

            let ticks = (total_ms * ticks_per_sec) / 1000;
            if ticks > i32::MAX as u64 {
                WAIT_FOREVER
            } else {
                ticks as i32
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            // For non-VxWorks platforms, return a reasonable default
            duration.as_millis() as i32
        }
    }

    /// Convert duration to timespec for POSIX operations
    fn duration_to_timespec(duration: Duration) -> TimeSpec {
        TimeSpec {
            tv_sec:  duration.as_secs() as i64,
            tv_nsec: duration.subsec_nanos() as i64,
        }
    }
}

impl FutexLike for VxWorksFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<(), Error> {
        // Check if the atomic value matches expected
        let current = self.atomic_value.load(Ordering::Acquire);
        if current != expected {
            return Ok(()); // Value changed, no need to wait
        }

        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    if let Some(sem_id) = self.sem_id {
                        let timeout_ticks = timeout.map_or(WAIT_FOREVER, Self::duration_to_ticks);

                        let result = unsafe { semTake(sem_id, timeout_ticks) };
                        if result != OK {
                            return Err(Error::runtime_execution_error(
                                "VxWorks semaphore wait failed",
                            ));
                        }
                    }
                },
                VxWorksContext::Rtp => {
                    if let Some(ref posix_sem) = self.posix_sem {
                        match timeout {
                            Some(duration) => {
                                let timespec = Self::duration_to_timespec(duration);
                                let result = unsafe {
                                    sem_timedwait(posix_sem as *const _ as *mut _, &timespec)
                                };
                                if result != 0 {
                                    return Err(Error::new(
                                        ErrorKind::Platform,
                                        "POSIX semaphore timed wait failed",
                                    ));
                                }
                            },
                            None => {
                                let result = unsafe { sem_wait(posix_sem as *const _ as *mut _) };
                                if result != 0 {
                                    return Err(Error::runtime_execution_error(
                                        "POSIX semaphore wait failed",
                                    ));
                                }
                            },
                        }
                    }
                },
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks futex wait not supported on this platform",
            ));
        }

        Ok(())
    }

    fn wake_one(&self) -> Result<u32, Error> {
        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    if let Some(sem_id) = self.sem_id {
                        let result = unsafe { semGive(sem_id) };
                        if result != OK {
                            return Err(Error::runtime_execution_error(
                                "Failed to give VxWorks semaphore",
                            ));
                        }
                        return Ok(1); // Woke one task
                    }
                },
                VxWorksContext::Rtp => {
                    if let Some(ref posix_sem) = self.posix_sem {
                        let result = unsafe { sem_post(posix_sem as *const _ as *mut _) };
                        if result != 0 {
                            return Err(Error::new(
                                ErrorKind::Platform,
                                "Failed to post POSIX semaphore",
                            ));
                        }
                        return Ok(1); // Woke one task
                    }
                },
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks futex wake not supported on this platform",
            ));
        }

        Ok(0)
    }

    fn wake_all(&self) -> Result<u32, Error> {
        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    if let Some(sem_id) = self.sem_id {
                        let result = unsafe { semFlush(sem_id) };
                        if result != OK {
                            return Err(Error::runtime_execution_error(
                                "Failed to flush VxWorks semaphore",
                            ));
                        }
                        // semFlush wakes all waiting tasks, but we don't know how many
                        return Ok(u32::MAX); // Indicate potentially many tasks
                                             // woken
                    }
                },
                VxWorksContext::Rtp => {
                    // POSIX semaphores don't have a direct wake-all operation
                    // We'll post multiple times to simulate it
                    let mut woken = 0;
                    if let Some(ref posix_sem) = self.posix_sem {
                        // Post up to a reasonable number of times
                        for _ in 0..32 {
                            let result = unsafe { sem_post(posix_sem as *const _ as *mut _) };
                            if result == 0 {
                                woken += 1;
                            } else {
                                break;
                            }
                        }
                        return Ok(woken);
                    }
                },
            }
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            return Err(Error::runtime_execution_error(
                "VxWorks futex wake_all not supported on this platform",
            ));
        }

        Ok(0)
    }

    fn load(&self, ordering: Ordering) -> u32 {
        self.atomic_value.load(ordering)
    }

    fn store(&self, value: u32, ordering: Ordering) {
        self.atomic_value.store(value, ordering);
    }

    fn compare_exchange_weak(
        &self,
        current: u32,
        new: u32,
        success: Ordering,
        failure: Ordering,
    ) -> Result<u32, u32> {
        self.atomic_value.compare_exchange_weak(current, new, success, failure)
    }
}

impl Drop for VxWorksFutex {
    fn drop(&mut self) {
        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    if let Some(sem_id) = self.sem_id {
                        unsafe {
                            semDelete(sem_id);
                        }
                    }
                },
                VxWorksContext::Rtp => {
                    if let Some(ref mut posix_sem) = self.posix_sem {
                        unsafe {
                            sem_destroy(posix_sem);
                        }
                    }
                },
            }
        }
    }
}

/// Builder for VxWorks futex
pub struct VxWorksFutexBuilder {
    context:       VxWorksContext,
    initial_value: u32,
}

impl VxWorksFutexBuilder {
    pub fn new(context: VxWorksContext) -> Self {
        Self {
            context,
            initial_value: 0,
        }
    }

    pub fn initial_value(mut self, value: u32) -> Self {
        self.initial_value = value;
        self
    }

    pub fn build(self) -> Result<VxWorksFutex, Error> {
        VxWorksFutex::new(self.context, self.initial_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vxworks_futex_builder() {
        let futex = VxWorksFutexBuilder::new(VxWorksContext::Rtp).initial_value(42).build();

        #[cfg(target_os = "vxworks")]
        {
            assert!(futex.is_ok());
            let futex = futex.unwrap();
            assert_eq!(futex.load(Ordering::Relaxed), 42);
        }

        #[cfg(not(target_os = "vxworks"))]
        assert!(futex.is_err());
    }

    #[test]
    fn test_context_selection() {
        let lkm_builder = VxWorksFutexBuilder::new(VxWorksContext::Lkm);
        let rtp_builder = VxWorksFutexBuilder::new(VxWorksContext::Rtp);

        assert_eq!(lkm_builder.context, VxWorksContext::Lkm);
        assert_eq!(rtp_builder.context, VxWorksContext::Rtp);
    }

    #[test]
    fn test_duration_to_ticks() {
        let duration = Duration::from_millis(100);
        let ticks = VxWorksFutex::duration_to_ticks(duration);

        #[cfg(target_os = "vxworks")]
        assert!(ticks > 0);

        #[cfg(not(target_os = "vxworks"))]
        assert_eq!(ticks, 100);
    }
}
