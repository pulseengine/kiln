//! Time utilities for WebAssembly runtime
//!
//! This module provides platform-specific time functionality including
//! wall clock time, monotonic time, and high-resolution timing.

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// Platform time provider
pub struct PlatformTime;

impl PlatformTime {
    /// Get current wall clock time in nanoseconds since Unix epoch
    #[cfg(feature = "std")]
    pub fn wall_clock_ns() -> Result<u64> {
        use std::time::{
            SystemTime,
            UNIX_EPOCH,
        };

        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .map_err(|_| Error::system_io_error("System time before epoch"))
    }

    /// Get monotonic time in nanoseconds
    ///
    /// This clock is guaranteed to be monotonic and is suitable for
    /// measuring elapsed time.
    #[cfg(feature = "std")]
    pub fn monotonic_ns() -> u64 {
        #[cfg(target_os = "linux")]
        {
            Self::linux_monotonic_ns()
        }

        #[cfg(target_os = "macos")]
        {
            Self::macos_monotonic_ns()
        }

        #[cfg(target_os = "windows")]
        {
            Self::windows_monotonic_ns()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Fallback to instant
            use std::time::Instant;
            static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
            let start = START.get_or_init(Instant::now);
            start.elapsed().as_nanos() as u64
        }
    }

    /// Linux implementation using clock_gettime
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn linux_monotonic_ns() -> u64 {
        use std::mem;

        #[repr(C)]
        struct Timespec {
            tv_sec:  i64,
            tv_nsec: i64,
        }

        // SAFETY: Edition 2024 requires unsafe extern blocks
        unsafe extern "C" {
            fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
        }

        const CLOCK_MONOTONIC: i32 = 1;

        unsafe {
            let mut ts = mem::zeroed::<Timespec>();
            if clock_gettime(CLOCK_MONOTONIC, &mut ts) == 0 {
                (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
            } else {
                // Fallback
                0
            }
        }
    }

    /// macOS implementation using mach_absolute_time
    #[cfg(all(feature = "std", target_os = "macos"))]
    fn macos_monotonic_ns() -> u64 {
        // SAFETY: Edition 2024 requires unsafe extern blocks
        unsafe extern "C" {
            fn mach_absolute_time() -> u64;
            fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
        }

        #[repr(C)]
        struct MachTimebaseInfo {
            numer: u32,
            denom: u32,
        }

        unsafe {
            static mut TIMEBASE: MachTimebaseInfo = MachTimebaseInfo { numer: 0, denom: 0 };
            static INIT: std::sync::Once = std::sync::Once::new();

            INIT.call_once(|| {
                mach_timebase_info(&raw mut TIMEBASE);
            });

            let ticks = mach_absolute_time();
            ticks * TIMEBASE.numer as u64 / TIMEBASE.denom as u64
        }
    }

    /// Windows implementation using QueryPerformanceCounter
    #[cfg(all(feature = "std", target_os = "windows"))]
    fn windows_monotonic_ns() -> u64 {
        use std::mem;

        extern "system" {
            fn QueryPerformanceCounter(lpPerformanceCount: *mut i64) -> i32;
            fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
        }

        unsafe {
            static mut FREQUENCY: i64 = 0;
            static INIT: std::sync::Once = std::sync::Once::new();

            INIT.call_once(|| {
                QueryPerformanceFrequency(&mut FREQUENCY);
            });

            let mut counter = mem::zeroed::<i64>();
            if QueryPerformanceCounter(&mut counter) != 0 && FREQUENCY > 0 {
                (counter as u64) * 1_000_000_000 / (FREQUENCY as u64)
            } else {
                0
            }
        }
    }

    /// Get clock resolution in nanoseconds
    pub fn clock_resolution_ns(clock_id: u32) -> u64 {
        match clock_id {
            0 => 1_000_000, // CLOCK_REALTIME: 1ms resolution
            1 => 1,         // CLOCK_MONOTONIC: 1ns resolution (best effort)
            2 => 1_000,     // CLOCK_PROCESS_CPUTIME_ID: 1us resolution
            3 => 1_000,     // CLOCK_THREAD_CPUTIME_ID: 1us resolution
            _ => 1_000_000, // Default: 1ms
        }
    }

    /// Get process CPU time in nanoseconds
    ///
    /// Returns the CPU time consumed by the current process (user + system time).
    /// This is the time the CPU has spent executing this process, not wall-clock time.
    #[cfg(feature = "std")]
    pub fn process_cpu_time_ns() -> Result<u64> {
        #[cfg(target_os = "linux")]
        {
            Self::linux_process_cpu_time_ns()
        }

        #[cfg(target_os = "macos")]
        {
            Self::macos_process_cpu_time_ns()
        }

        #[cfg(target_os = "windows")]
        {
            Self::windows_process_cpu_time_ns()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Unsupported platform - return error
            Err(Error::new(
                ErrorCategory::PlatformRuntime,
                codes::PLATFORM_UNSUPPORTED,
                "Process CPU time not available on this platform",
            ))
        }
    }

    /// Get thread CPU time in nanoseconds
    ///
    /// Returns the CPU time consumed by the current thread (user + system time).
    /// This is the time the CPU has spent executing this thread, not wall-clock time.
    #[cfg(feature = "std")]
    pub fn thread_cpu_time_ns() -> Result<u64> {
        #[cfg(target_os = "linux")]
        {
            Self::linux_thread_cpu_time_ns()
        }

        #[cfg(target_os = "macos")]
        {
            Self::macos_thread_cpu_time_ns()
        }

        #[cfg(target_os = "windows")]
        {
            Self::windows_thread_cpu_time_ns()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Unsupported platform - return error
            Err(Error::new(
                ErrorCategory::PlatformRuntime,
                codes::PLATFORM_UNSUPPORTED,
                "Thread CPU time not available on this platform",
            ))
        }
    }

    /// Linux implementation of process CPU time using clock_gettime(CLOCK_PROCESS_CPUTIME_ID)
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn linux_process_cpu_time_ns() -> Result<u64> {
        use std::mem;

        #[repr(C)]
        struct Timespec {
            tv_sec:  i64,
            tv_nsec: i64,
        }

        // SAFETY: Edition 2024 requires unsafe extern blocks
        unsafe extern "C" {
            fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
        }

        const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;

        unsafe {
            let mut ts = mem::zeroed::<Timespec>();
            if clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut ts) == 0 {
                Ok((ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64))
            } else {
                Err(Error::system_io_error("Failed to get process CPU time"))
            }
        }
    }

    /// Linux implementation of thread CPU time using clock_gettime(CLOCK_THREAD_CPUTIME_ID)
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn linux_thread_cpu_time_ns() -> Result<u64> {
        use std::mem;

        #[repr(C)]
        struct Timespec {
            tv_sec:  i64,
            tv_nsec: i64,
        }

        // SAFETY: Edition 2024 requires unsafe extern blocks
        unsafe extern "C" {
            fn clock_gettime(clk_id: i32, tp: *mut Timespec) -> i32;
        }

        const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

        unsafe {
            let mut ts = mem::zeroed::<Timespec>();
            if clock_gettime(CLOCK_THREAD_CPUTIME_ID, &mut ts) == 0 {
                Ok((ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64))
            } else {
                Err(Error::system_io_error("Failed to get thread CPU time"))
            }
        }
    }

    /// macOS implementation of process CPU time using getrusage
    #[cfg(all(feature = "std", target_os = "macos"))]
    #[allow(clippy::cast_sign_loss)] // OS time values are always non-negative
    fn macos_process_cpu_time_ns() -> Result<u64> {
        use core::mem;

        #[repr(C)]
        struct Timeval {
            tv_sec:  i64,
            tv_usec: i32,
        }

        #[repr(C)]
        struct Rusage {
            ru_utime:    Timeval,  // user time used
            ru_stime:    Timeval,  // system time used
            _padding:    [i64; 14], // other fields we don't need
        }

        // SAFETY: getrusage is a POSIX system call
        unsafe extern "C" {
            fn getrusage(who: i32, usage: *mut Rusage) -> i32;
        }

        const RUSAGE_SELF: i32 = 0;

        unsafe {
            let mut usage = mem::zeroed::<Rusage>();
            if getrusage(RUSAGE_SELF, &mut usage) == 0 {
                let user_ns = (usage.ru_utime.tv_sec as u64) * 1_000_000_000
                    + (usage.ru_utime.tv_usec as u64) * 1_000;
                let sys_ns = (usage.ru_stime.tv_sec as u64) * 1_000_000_000
                    + (usage.ru_stime.tv_usec as u64) * 1_000;
                Ok(user_ns + sys_ns)
            } else {
                Err(Error::system_io_error("Failed to get process CPU time"))
            }
        }
    }

    /// macOS implementation of thread CPU time using thread_info
    #[cfg(all(feature = "std", target_os = "macos"))]
    #[allow(clippy::cast_sign_loss)] // OS time values are always non-negative
    fn macos_thread_cpu_time_ns() -> Result<u64> {
        #[repr(C)]
        struct ThreadBasicInfo {
            user_time:      TimeValue,
            system_time:    TimeValue,
            cpu_usage:      i32,
            policy:         i32,
            run_state:      i32,
            flags:          i32,
            suspend_count:  i32,
            sleep_time:     i32,
        }

        #[repr(C)]
        struct TimeValue {
            seconds:      i32,
            microseconds: i32,
        }

        // SAFETY: These are macOS system calls
        unsafe extern "C" {
            fn mach_thread_self() -> u32;
            fn thread_info(
                thread: u32,
                flavor: i32,
                thread_info: *mut ThreadBasicInfo,
                thread_info_count: *mut u32,
            ) -> i32;
            fn mach_port_deallocate(task: u32, name: u32) -> i32;
            fn mach_task_self() -> u32;
        }

        const THREAD_BASIC_INFO: i32 = 3;
        const THREAD_BASIC_INFO_COUNT: u32 = 10;

        unsafe {
            let thread = mach_thread_self();
            let mut info: ThreadBasicInfo = core::mem::zeroed();
            let mut count = THREAD_BASIC_INFO_COUNT;

            let result = thread_info(thread, THREAD_BASIC_INFO, &mut info, &mut count);

            // Deallocate the thread port to avoid resource leaks
            mach_port_deallocate(mach_task_self(), thread);

            if result == 0 {
                let user_ns = (info.user_time.seconds as u64) * 1_000_000_000
                    + (info.user_time.microseconds as u64) * 1_000;
                let sys_ns = (info.system_time.seconds as u64) * 1_000_000_000
                    + (info.system_time.microseconds as u64) * 1_000;
                Ok(user_ns + sys_ns)
            } else {
                Err(Error::system_io_error("Failed to get thread CPU time"))
            }
        }
    }

    /// Windows implementation of process CPU time using GetProcessTimes
    #[cfg(all(feature = "std", target_os = "windows"))]
    fn windows_process_cpu_time_ns() -> Result<u64> {
        use std::mem;

        #[repr(C)]
        struct FileTime {
            low:  u32,
            high: u32,
        }

        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn GetProcessTimes(
                process: isize,
                creation: *mut FileTime,
                exit: *mut FileTime,
                kernel: *mut FileTime,
                user: *mut FileTime,
            ) -> i32;
        }

        unsafe {
            let process = GetCurrentProcess();
            let mut creation = mem::zeroed::<FileTime>();
            let mut exit = mem::zeroed::<FileTime>();
            let mut kernel = mem::zeroed::<FileTime>();
            let mut user = mem::zeroed::<FileTime>();

            if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
                // FILETIME is in 100-nanosecond intervals
                let kernel_100ns =
                    ((kernel.high as u64) << 32) | (kernel.low as u64);
                let user_100ns =
                    ((user.high as u64) << 32) | (user.low as u64);
                Ok((kernel_100ns + user_100ns) * 100)
            } else {
                Err(Error::system_io_error("Failed to get process CPU time"))
            }
        }
    }

    /// Windows implementation of thread CPU time using GetThreadTimes
    #[cfg(all(feature = "std", target_os = "windows"))]
    fn windows_thread_cpu_time_ns() -> Result<u64> {
        use std::mem;

        #[repr(C)]
        struct FileTime {
            low:  u32,
            high: u32,
        }

        extern "system" {
            fn GetCurrentThread() -> isize;
            fn GetThreadTimes(
                thread: isize,
                creation: *mut FileTime,
                exit: *mut FileTime,
                kernel: *mut FileTime,
                user: *mut FileTime,
            ) -> i32;
        }

        unsafe {
            let thread = GetCurrentThread();
            let mut creation = mem::zeroed::<FileTime>();
            let mut exit = mem::zeroed::<FileTime>();
            let mut kernel = mem::zeroed::<FileTime>();
            let mut user = mem::zeroed::<FileTime>();

            if GetThreadTimes(thread, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
                // FILETIME is in 100-nanosecond intervals
                let kernel_100ns =
                    ((kernel.high as u64) << 32) | (kernel.low as u64);
                let user_100ns =
                    ((user.high as u64) << 32) | (user.low as u64);
                Ok((kernel_100ns + user_100ns) * 100)
            } else {
                Err(Error::system_io_error("Failed to get thread CPU time"))
            }
        }
    }
}

/// No-std time implementation
#[cfg(not(feature = "std"))]
impl PlatformTime {
    /// Get wall clock time (no_std version)
    /// Returns a monotonic counter since real time is not available
    pub fn wall_clock_ns() -> Result<u64> {
        Ok(Self::monotonic_ns())
    }

    /// Get monotonic time (no_std version)
    pub fn monotonic_ns() -> u64 {
        use core::sync::atomic::{
            AtomicU64,
            Ordering,
        };

        static COUNTER: AtomicU64 = AtomicU64::new(1_000_000_000); // Start at 1 second
        COUNTER.fetch_add(1_000_000, Ordering::Relaxed) // Increment by 1ms
    }

    /// Get process CPU time (no_std version)
    ///
    /// TODO: Implement when embedded platform support is available.
    /// In no_std environments, CPU time measurement requires platform-specific
    /// hardware timers or OS-provided APIs that are not universally available.
    /// Returns 0 as a placeholder.
    pub fn process_cpu_time_ns() -> Result<u64> {
        // TODO: Implement for specific embedded platforms (e.g., using DWT cycle counter on ARM Cortex-M)
        Ok(0)
    }

    /// Get thread CPU time (no_std version)
    ///
    /// TODO: Implement when embedded platform support is available.
    /// In no_std environments, thread CPU time measurement requires platform-specific
    /// support. Many embedded systems are single-threaded, making this less relevant.
    /// Returns 0 as a placeholder.
    pub fn thread_cpu_time_ns() -> Result<u64> {
        // TODO: Implement for specific embedded platforms with threading support
        Ok(0)
    }
}

/// Legacy compatibility function
#[cfg(feature = "std")]
pub fn current_time_ns() -> u64 {
    PlatformTime::wall_clock_ns().unwrap_or(0)
}

/// Legacy compatibility function (no_std)
#[cfg(not(feature = "std"))]
pub fn current_time_ns() -> u64 {
    PlatformTime::monotonic_ns()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_wall_clock() {
        let time1 = PlatformTime::wall_clock_ns().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let time2 = PlatformTime::wall_clock_ns().unwrap();

        assert!(time2 > time1);
        // Should have advanced at least 10ms
        assert!(time2 - time1 >= 10_000_000);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_monotonic_clock() {
        let time1 = PlatformTime::monotonic_ns();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let time2 = PlatformTime::monotonic_ns();

        assert!(time2 > time1);
        // Monotonic clock should never go backwards
        assert!(time2 >= time1);
    }

    #[test]
    fn test_clock_resolution() {
        assert_eq!(PlatformTime::clock_resolution_ns(0), 1_000_000);
        assert_eq!(PlatformTime::clock_resolution_ns(1), 1);
        assert_eq!(PlatformTime::clock_resolution_ns(99), 1_000_000);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_process_cpu_time() {
        // Process CPU time should be available and non-negative
        let cpu_time = PlatformTime::process_cpu_time_ns().unwrap();
        // CPU time should be reasonable (less than 24 hours in nanoseconds)
        assert!(cpu_time < 24 * 60 * 60 * 1_000_000_000u64);

        // Do some work to consume CPU time
        let mut sum = 0u64;
        for i in 0..1_000_000 {
            sum = sum.wrapping_add(i);
        }
        // Prevent optimization
        assert!(sum > 0 || sum == 0);

        let cpu_time2 = PlatformTime::process_cpu_time_ns().unwrap();
        // CPU time should have increased
        assert!(cpu_time2 >= cpu_time);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_thread_cpu_time() {
        // Thread CPU time should be available and non-negative
        let cpu_time = PlatformTime::thread_cpu_time_ns().unwrap();
        // CPU time should be reasonable (less than 24 hours in nanoseconds)
        assert!(cpu_time < 24 * 60 * 60 * 1_000_000_000u64);

        // Do some work to consume CPU time
        let mut sum = 0u64;
        for i in 0..1_000_000 {
            sum = sum.wrapping_add(i);
        }
        // Prevent optimization
        assert!(sum > 0 || sum == 0);

        let cpu_time2 = PlatformTime::thread_cpu_time_ns().unwrap();
        // CPU time should have increased
        assert!(cpu_time2 >= cpu_time);
    }
}
