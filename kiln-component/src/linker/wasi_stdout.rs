//! WASI stdout implementation for Component Model
//!
//! **DEPRECATED**: This module provides legacy standalone functions for stdout.
//! The proper Preview2 implementation is in `wrt_wasi::WasiDispatcher` which
//! uses dynamic resource allocation via `WasiResourceManager`.
//!
//! These functions are kept for backwards compatibility but should not be used
//! for new code. Use `WasiDispatcher::dispatch()` instead.

use wrt_error::Result;

#[cfg(feature = "std")]
use std::io::Write;

// Tracing imports for structured logging
#[cfg(feature = "tracing")]
use wrt_foundation::tracing::trace;

/// DEPRECATED: Get stdout output stream handle
///
/// This function returns a static handle (1) which is Preview1 semantics.
/// For proper Preview2 resource semantics, use `WasiDispatcher` which
/// allocates handles dynamically via `WasiResourceManager`.
#[deprecated(
    since = "0.2.0",
    note = "Use WasiDispatcher::dispatch(\"wasi:cli/stdout\", \"get-stdout\", ...) instead"
)]
pub fn wasi_get_stdout() -> Result<u32> {
    #[cfg(feature = "tracing")]
    trace!("wasi_get_stdout called (deprecated - use WasiDispatcher)");
    // Legacy: return static handle 1
    Ok(1)
}

/// DEPRECATED: Write bytes to an output stream and flush
///
/// This function uses static handle checking which is Preview1 semantics.
/// For proper Preview2 resource semantics, use `WasiDispatcher` which
/// looks up resources via `WasiResourceManager`.
#[deprecated(
    since = "0.2.0",
    note = "Use WasiDispatcher::dispatch(\"wasi:io/streams\", \"blocking-write-and-flush\", ...) instead"
)]
#[cfg(feature = "std")]
pub fn wasi_blocking_write_and_flush(handle: u32, bytes: &[u8]) -> Result<u64> {
    #[cfg(feature = "tracing")]
    trace!(
        byte_count = bytes.len(),
        "wasi_blocking_write_and_flush called (deprecated)"
    );

    // Legacy: handle 1 = stdout
    if handle == 1 {
        std::io::stdout()
            .write_all(bytes)
            .map_err(|_| wrt_error::Error::runtime_error("Write failed"))?;

        std::io::stdout()
            .flush()
            .map_err(|_| wrt_error::Error::runtime_error("Flush failed"))?;

        Ok(bytes.len() as u64)
    } else {
        Err(wrt_error::Error::runtime_error(
            "Invalid output stream handle",
        ))
    }
}

/// DEPRECATED: Write bytes to an output stream and flush (no_std stub)
#[deprecated(since = "0.2.0", note = "Use WasiDispatcher instead")]
#[cfg(not(feature = "std"))]
pub fn wasi_blocking_write_and_flush(handle: u32, bytes: &[u8]) -> Result<u64> {
    // In no_std, we can't write to stdout - stub that always succeeds for handle 1
    if handle == 1 {
        Ok(bytes.len() as u64)
    } else {
        Err(wrt_error::Error::runtime_error(
            "Invalid output stream handle",
        ))
    }
}

/// DEPRECATED: Write bytes to stdout (convenience function)
#[deprecated(since = "0.2.0", note = "Use WasiDispatcher instead")]
#[allow(deprecated)]
pub fn write_stdout(bytes: &[u8]) -> Result<()> {
    wasi_blocking_write_and_flush(1, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use super::*;

    #[test]
    fn test_get_stdout() {
        #[allow(deprecated)]
        let handle = wasi_get_stdout().unwrap();
        assert_eq!(handle, 1); // Legacy behavior
    }

    #[test]
    fn test_write_stdout() {
        let data = b"Test output\n";
        #[allow(deprecated)]
        let result = wasi_blocking_write_and_flush(1, data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data.len() as u64);
    }
}
