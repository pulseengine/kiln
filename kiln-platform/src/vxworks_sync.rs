use core::{
    fmt::{
        self,
        Debug,
    },
    sync::atomic::{
        AtomicU32,
        Ordering,
    },
    time::Duration,
};

use kiln_error::{
    Error,
    Result,
};

use crate::sync::FutexLike;

#[cfg(target_os = "vxworks")]
unsafe extern "C" {
    // VxWorks semaphore functions (both LKM and RTP)
    fn semBCreate(options: i32, initial_state: i32) -> usize; // SEM_ID
    fn semDelete(sem_id: usize) -> i32;
    fn semTake(sem_id: usize, timeout: i32) -> i32;
    fn semGive(sem_id: usize) -> i32;
    fn semFlush(sem_id: usize) -> i32;

    // POSIX semaphores (RTP context)
    fn sem_init(sem: *mut PosixSem, pshared: i32, value: u32) -> i32;
    fn sem_destroy(sem: *mut PosixSem) -> i32;
    fn sem_wait(sem: *mut PosixSem) -> i32;
    fn sem_timedwait(sem: *mut PosixSem, timeout: *const TimeSpec) -> i32;
    fn sem_post(sem: *mut PosixSem) -> i32;

    // Task/thread functions
    fn taskIdSelf() -> usize;
    fn taskDelay(ticks: i32) -> i32;
    fn sysClkRateGet() -> i32;
}

// VxWorks semaphore options
#[allow(dead_code)]
const SEM_Q_FIFO: i32 = 0x00;
const SEM_Q_PRIORITY: i32 = 0x01;
const SEM_DELETE_SAFE: i32 = 0x04;
const SEM_INVERSION_SAFE: i32 = 0x08;

// Timeout values
const WAIT_FOREVER: i32 = -1;
const NO_WAIT: i32 = 0;

// Error codes
const OK: i32 = 0;

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

impl Debug for VxWorksFutex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VxWorksFutex")
            .field("context", &self.context)
            .field("atomic_value", &self.atomic_value.load(Ordering::Relaxed))
            .field("sem_id", &self.sem_id)
            .field("has_posix_sem", &self.posix_sem.is_some())
            .finish()
    }
}

// Safety: VxWorksFutex uses atomic operations and VxWorks kernel semaphores
// which are thread-safe.
unsafe impl Send for VxWorksFutex {}
unsafe impl Sync for VxWorksFutex {}

impl VxWorksFutex {
    /// Create a new VxWorks futex-like synchronization primitive
    pub fn new(context: VxWorksContext, initial_value: u32) -> Result<Self> {
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
    fn init_vxworks_semaphore(&mut self) -> Result<()> {
        #[cfg(target_os = "vxworks")]
        {
            // Create a binary semaphore with priority queuing and inversion safety
            let options = SEM_Q_PRIORITY | SEM_DELETE_SAFE | SEM_INVERSION_SAFE;
            let sem_id = unsafe { semBCreate(options, 0) }; // Start empty

            if sem_id == 0 {
                return Err(Error::platform_sync_primitive_failed(
                    "Failed to create VxWorks semaphore",
                ));
            }

            self.sem_id = Some(sem_id);
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        Err(Error::platform_sync_primitive_failed(
            "VxWorks semaphore not supported on this platform",
        ))
    }

    /// Initialize POSIX semaphore for RTP context
    fn init_posix_semaphore(&mut self, initial_value: u32) -> Result<()> {
        #[cfg(target_os = "vxworks")]
        {
            let mut posix_sem = PosixSem { _data: [0; 16] };

            let result = unsafe { sem_init(&mut posix_sem, 0, initial_value) };
            if result != 0 {
                return Err(Error::platform_sync_primitive_failed(
                    "Failed to initialize POSIX semaphore",
                ));
            }

            self.posix_sem = Some(posix_sem);
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            let _ = initial_value;
            Err(Error::platform_sync_primitive_failed(
                "POSIX semaphore not supported on this platform",
            ))
        }
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

    /// Load the atomic value
    pub fn load(&self, ordering: Ordering) -> u32 {
        self.atomic_value.load(ordering)
    }

    /// Store to the atomic value
    pub fn store(&self, value: u32, ordering: Ordering) {
        self.atomic_value.store(value, ordering);
    }

    /// Compare-and-exchange on the atomic value
    pub fn compare_exchange_weak(
        &self,
        current: u32,
        new: u32,
        success: Ordering,
        failure: Ordering,
    ) -> core::result::Result<u32, u32> {
        self.atomic_value
            .compare_exchange_weak(current, new, success, failure)
    }
}

impl FutexLike for VxWorksFutex {
    fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<()> {
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
                            return Err(Error::platform_sync_primitive_failed(
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
                                    return Err(Error::platform_sync_primitive_failed(
                                        "POSIX semaphore timed wait failed",
                                    ));
                                }
                            },
                            None => {
                                let result = unsafe { sem_wait(posix_sem as *const _ as *mut _) };
                                if result != 0 {
                                    return Err(Error::platform_sync_primitive_failed(
                                        "POSIX semaphore wait failed",
                                    ));
                                }
                            },
                        }
                    }
                },
            }
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            let _ = timeout;
            Err(Error::platform_sync_primitive_failed(
                "VxWorks futex wait not supported on this platform",
            ))
        }
    }

    fn wake(&self, count: u32) -> Result<()> {
        #[cfg(target_os = "vxworks")]
        {
            match self.context {
                VxWorksContext::Lkm => {
                    if let Some(sem_id) = self.sem_id {
                        if count == u32::MAX {
                            // Wake all waiters
                            let result = unsafe { semFlush(sem_id) };
                            if result != OK {
                                return Err(Error::platform_sync_primitive_failed(
                                    "Failed to flush VxWorks semaphore",
                                ));
                            }
                        } else {
                            // Wake up to `count` waiters
                            for _ in 0..count {
                                let result = unsafe { semGive(sem_id) };
                                if result != OK {
                                    break;
                                }
                            }
                        }
                    }
                },
                VxWorksContext::Rtp => {
                    if let Some(ref posix_sem) = self.posix_sem {
                        let post_count = if count == u32::MAX { 32 } else { count };
                        for _ in 0..post_count {
                            let result = unsafe { sem_post(posix_sem as *const _ as *mut _) };
                            if result != 0 {
                                break;
                            }
                        }
                    }
                },
            }
            Ok(())
        }

        #[cfg(not(target_os = "vxworks"))]
        {
            let _ = count;
            Err(Error::platform_sync_primitive_failed(
                "VxWorks futex wake not supported on this platform",
            ))
        }
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

    pub fn build(self) -> Result<VxWorksFutex> {
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
