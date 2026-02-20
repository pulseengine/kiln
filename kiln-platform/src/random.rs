//! Platform-specific cryptographically secure random number generation
//!
//! This module provides a unified interface for accessing platform-specific
//! random number generators suitable for cryptographic use.
//!
//! # Safety
//!
//! This module uses platform-specific APIs that may require unsafe code.
//! All unsafe usage is documented and justified.

#![allow(unsafe_code)] // Required for platform-specific APIs

use wrt_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

/// Platform-specific random number generator
pub struct PlatformRandom;

impl PlatformRandom {
    /// Generate cryptographically secure random bytes
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to fill with random bytes
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    ///
    /// # Platform Support
    ///
    /// - Linux: Uses `/dev/urandom`
    /// - macOS: Uses `getentropy()`
    /// - Windows: Uses `BCryptGenRandom()`
    /// - QNX: Uses `/dev/random`
    /// - VxWorks: Uses `randBytes()`
    /// - Zephyr: Uses `sys_rand_get()`
    /// - Others: Falls back to less secure methods
    #[cfg(feature = "std")]
    pub fn get_secure_bytes(buffer: &mut [u8]) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::linux_random(buffer)
        }

        #[cfg(target_os = "macos")]
        {
            Self::macos_random(buffer)
        }

        #[cfg(target_os = "windows")]
        {
            Self::windows_random(buffer)
        }

        #[cfg(target_os = "nto")]
        {
            Self::qnx_random(buffer)
        }

        #[cfg(target_os = "vxworks")]
        {
            Self::vxworks_random(buffer)
        }

        #[cfg(all(
            not(target_os = "linux"),
            not(target_os = "macos"),
            not(target_os = "windows"),
            not(target_os = "nto"),
            not(target_os = "vxworks")
        ))]
        {
            Self::fallback_random(buffer)
        }
    }

    /// Linux implementation using /dev/urandom
    #[cfg(all(feature = "std", target_os = "linux"))]
    fn linux_random(buffer: &mut [u8]) -> Result<()> {
        use std::{
            fs::File,
            io::Read,
        };

        let mut urandom = File::open("/dev/urandom")
            .map_err(|_| Error::runtime_execution_error("Failed to open /dev/urandom"))?;

        urandom
            .read_exact(buffer)
            .map_err(|_| Error::system_io_error("Failed to read from /dev/urandom"))?;

        Ok(())
    }

    /// macOS implementation using getentropy
    #[cfg(all(feature = "std", target_os = "macos"))]
    fn macos_random(buffer: &mut [u8]) -> Result<()> {
        use std::os::raw::c_void;

        // SAFETY: Edition 2024 requires unsafe extern blocks
        unsafe extern "C" {
            fn getentropy(buf: *mut c_void, buflen: usize) -> i32;
        }

        // getentropy has a maximum of 256 bytes per call
        const MAX_CHUNK: usize = 256;

        for chunk in buffer.chunks_mut(MAX_CHUNK) {
            // Safety: getentropy is guaranteed to be available on macOS 10.12+
            // and we ensure the buffer and length are valid
            let result = unsafe { getentropy(chunk.as_mut_ptr() as *mut c_void, chunk.len()) };

            if result != 0 {
                return Err(Error::system_io_error("getentropy failed"));
            }
        }

        Ok(())
    }

    /// Windows implementation using BCryptGenRandom
    #[cfg(all(feature = "std", target_os = "windows"))]
    fn windows_random(buffer: &mut [u8]) -> Result<()> {
        use std::{
            os::raw::c_void,
            ptr,
        };

        #[link(name = "bcrypt")]
        extern "system" {
            fn BCryptGenRandom(
                hAlgorithm: *mut c_void,
                pbBuffer: *mut u8,
                cbBuffer: u32,
                dwFlags: u32,
            ) -> i32;
        }

        const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;
        const STATUS_SUCCESS: i32 = 0;

        // Safety: BCryptGenRandom is a documented Windows API
        // We pass null for algorithm to use system RNG
        let result = unsafe {
            BCryptGenRandom(
                ptr::null_mut(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };

        if result != STATUS_SUCCESS {
            return Err(Error::system_io_error("ProcessPrng failed"));
        }

        Ok(())
    }

    /// QNX implementation using /dev/random
    #[cfg(all(feature = "std", target_os = "nto"))]
    fn qnx_random(buffer: &mut [u8]) -> Result<()> {
        use std::{
            fs::File,
            io::Read,
        };

        // QNX recommends /dev/random for cryptographic purposes
        let mut random = File::open("/dev/random")
            .map_err(|_| Error::runtime_execution_error("Failed to open /dev/random"))?;

        random
            .read_exact(buffer)
            .map_err(|_| Error::system_io_error("Failed to read from /dev/random"))?;

        Ok(())
    }

    /// VxWorks implementation using randBytes
    #[cfg(all(feature = "std", target_os = "vxworks"))]
    fn vxworks_random(buffer: &mut [u8]) -> Result<()> {
        // SAFETY: Edition 2024 requires unsafe extern blocks
        unsafe extern "C" {
            fn randBytes(pBuf: *mut u8, numBytes: i32) -> i32;
        }

        // Safety: randBytes is a documented VxWorks API
        let result = unsafe { randBytes(buffer.as_mut_ptr(), buffer.len() as i32) };

        if result != 0 {
            return Err(Error::system_io_error("randBytes failed"));
        }

        Ok(())
    }

    /// Fallback implementation for other platforms
    #[cfg(feature = "std")]
    #[allow(dead_code)]
    fn fallback_random(buffer: &mut [u8]) -> Result<()> {
        // Try to use /dev/urandom if available
        use std::{
            fs::File,
            io::Read,
        };

        if let Ok(mut urandom) = File::open("/dev/urandom") {
            if urandom.read_exact(buffer).is_ok() {
                return Ok(());
            }
        }

        // If no secure source is available, return an error
        // We don't want to silently fall back to insecure randomness
        Err(Error::runtime_not_implemented(
            "No secure random source available on this platform",
        ))
    }

    /// No-std implementation with limited entropy
    #[cfg(not(feature = "std"))]
    pub fn get_secure_bytes(buffer: &mut [u8]) -> Result<()> {
        // In no_std environments, we need platform-specific implementations

        #[cfg(feature = "platform-tock")]
        {
            Self::tock_random(buffer)
        }

        #[cfg(not(feature = "platform-tock"))]
        {
            Err(Error::runtime_execution_error(
                "No secure random source available in no_std environment",
            ))
        }
    }

    /// Tock OS random implementation
    #[cfg(all(not(feature = "std"), feature = "platform-tock"))]
    fn tock_random(buffer: &mut [u8]) -> Result<()> {
        // SAFETY: Edition 2024 requires unsafe extern blocks - Tock provides a random syscall
        unsafe extern "C" {
            fn tock_random_bytes(buf: *mut u8, len: usize) -> i32;
        }

        // Safety: tock_random_bytes is provided by Tock kernel
        let result = unsafe { tock_random_bytes(buffer.as_mut_ptr(), buffer.len()) };

        if result != 0 {
            return Err(Error::system_io_error("Tock random syscall failed"));
        }

        Ok(())
    }
}

/// Insecure pseudo-random number generator for testing only
///
/// This should never be used for cryptographic purposes
#[cfg(any(test, feature = "test-utils"))]
pub struct TestRandom {
    seed: u64,
}

#[cfg(any(test, feature = "test-utils"))]
impl TestRandom {
    /// Create a new test random generator
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Generate the next pseudo-random u64
    pub fn next_u64(&mut self) -> u64 {
        // Linear congruential generator
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.seed
    }

    /// Fill a buffer with pseudo-random bytes
    pub fn fill_bytes(&mut self, buffer: &mut [u8]) {
        for chunk in buffer.chunks_mut(8) {
            let value = self.next_u64();
            let bytes = value.to_le_bytes();
            for (i, byte) in chunk.iter_mut().enumerate() {
                if i < bytes.len() {
                    *byte = bytes[i];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_platform_random() {
        let mut buffer1 = [0u8; 32];
        let mut buffer2 = [0u8; 32];

        // Generate two sets of random bytes
        PlatformRandom::get_secure_bytes(&mut buffer1).unwrap();
        PlatformRandom::get_secure_bytes(&mut buffer2).unwrap();

        // They should be different (with overwhelming probability)
        assert_ne!(buffer1, buffer2);

        // They should not be all zeros
        assert_ne!(buffer1, [0u8; 32]);
        assert_ne!(buffer2, [0u8; 32]);
    }

    #[test]
    fn test_test_random() {
        let mut rng1 = TestRandom::new(12345);
        let mut rng2 = TestRandom::new(12345);
        let mut rng3 = TestRandom::new(54321);

        // Same seed should produce same sequence
        assert_eq!(rng1.next_u64(), rng2.next_u64());

        // Different seed should produce different sequence
        assert_ne!(rng1.next_u64(), rng3.next_u64());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_large_buffer() {
        // Test with buffer larger than platform limits (e.g., getentropy's 256 bytes)
        let mut buffer = vec![0u8; 1024];
        assert!(PlatformRandom::get_secure_bytes(&mut buffer).is_ok());
    }
}
