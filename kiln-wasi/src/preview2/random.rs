//! WASI random interface implementation
//!
//! Implements the `wasi:random` interface for random number generation using
//! WRT's platform abstractions and security patterns.

use core::any::Any;

use crate::{
    capabilities::WasiRandomCapabilities,
    prelude::*,
    Value,
};

/// WASI get random bytes operation
///
/// Implements `wasi:random/random.get-random-bytes` for secure random
/// generation
///
/// # Errors
///
/// Returns an error if:
/// - The requested length exceeds 1MB
/// - Secure random generation fails on this platform
pub fn wasi_get_random_bytes(_target: &mut dyn Any, args: &[Value]) -> Result<Vec<Value>> {
    // Extract length from arguments
    let len = extract_length(args)?;

    // Validate length is reasonable
    if len > 1024 * 1024 {
        // 1MB limit
        return Err(Error::wasi_resource_limit(
            "Random bytes request exceeds limit",
        ));
    }

    // Generate secure random bytes using platform abstraction
    let random_bytes = generate_secure_random(len)?;

    // Convert to WASI list<u8>
    let value_bytes: Vec<Value> = random_bytes.into_iter().map(Value::U8).collect();
    Ok(vec![Value::List(value_bytes)])
}

/// WASI get insecure random bytes operation
///
/// Implements `wasi:random/insecure.get-insecure-random-bytes` for fast
/// pseudo-random generation
///
/// # Errors
///
/// Returns an error if:
/// - The requested length exceeds 10MB
/// - Pseudo-random generation fails
pub fn wasi_get_insecure_random_bytes(
    _target: &mut dyn Any,
    args: &[Value],
) -> Result<Vec<Value>> {
    // Extract length from arguments
    let len = extract_length(args)?;

    // Validate length is reasonable
    if len > 10 * 1024 * 1024 {
        // 10MB limit for insecure random
        return Err(Error::wasi_resource_limit(
            "Insecure random bytes request exceeds limit",
        ));
    }

    // Generate pseudo-random bytes using platform abstraction
    let random_bytes = generate_pseudo_random(len)?;

    // Convert to WASI list<u8>
    let value_bytes: Vec<Value> = random_bytes.into_iter().map(Value::U8).collect();
    Ok(vec![Value::List(value_bytes)])
}

/// WASI get random u64 operation
///
/// Implements `wasi:random/random.get-random-u64` for secure u64 generation
///
/// # Errors
///
/// Returns an error if secure random generation fails on this platform.
pub fn wasi_get_random_u64(_target: &mut dyn Any, _args: &[Value]) -> Result<Vec<Value>> {
    // Generate 8 secure random bytes
    let random_bytes = generate_secure_random(8)?;

    // Convert to u64
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&random_bytes);
    let random_u64 = u64::from_le_bytes(bytes);

    Ok(vec![Value::U64(random_u64)])
}

/// WASI get insecure random u64 operation
///
/// Implements `wasi:random/insecure.get-insecure-random-u64` for fast u64
/// generation
///
/// # Errors
///
/// Returns an error if pseudo-random generation fails.
pub fn wasi_get_insecure_random_u64(
    _target: &mut dyn Any,
    _args: &[Value],
) -> Result<Vec<Value>> {
    // Generate 8 pseudo-random bytes
    let random_bytes = generate_pseudo_random(8)?;

    // Convert to u64
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&random_bytes);
    let random_u64 = u64::from_le_bytes(bytes);

    Ok(vec![Value::U64(random_u64)])
}

/// Helper function to extract length from arguments
fn extract_length(args: &[Value]) -> Result<usize> {
    if args.is_empty() {
        return Err(Error::wasi_invalid_fd("Missing length argument"));
    }

    match &args[0] {
        Value::U64(len) => Ok(*len as usize),
        Value::U32(len) => Ok(*len as usize),
        Value::S32(len) => {
            if *len < 0 {
                Err(Error::wasi_invalid_fd("Invalid negative length"))
            } else {
                Ok(*len as usize)
            }
        },
        _ => Err(Error::wasi_invalid_fd("Invalid length type")),
    }
}

/// Generate secure random bytes using platform-specific implementation
///
/// Uses wrt-platform's `PlatformRandom` which provides:
/// - Linux: `/dev/urandom`
/// - macOS: `getentropy()` system call
/// - Windows: `BCryptGenRandom()`
/// - QNX: `/dev/random`
/// - `VxWorks`: `randBytes()`
/// - Others: `/dev/urandom` fallback or error
fn generate_secure_random(len: usize) -> Result<Vec<u8>> {
    #[cfg(feature = "std")]
    {
        use wrt_platform::random::PlatformRandom;

        let mut buffer = vec![0u8; len];
        PlatformRandom::get_secure_bytes(&mut buffer)
            .map_err(|_| Error::wasi_capability_unavailable(
                "Failed to generate secure random bytes"
            ))?;

        Ok(buffer)
    }

    #[cfg(not(feature = "std"))]
    {
        // no_std environments cannot return Vec<u8>
        // Platform-specific implementations would need BoundedVec return type
        let _ = len; // Suppress unused warning
        Err(Error::wasi_capability_unavailable(
            "Secure random requires std feature for Vec allocation",
        ))
    }
}

/// Generate pseudo-random bytes using fast non-cryptographic implementation
// Result kept for API consistency with secure random functions.
#[allow(clippy::unnecessary_wraps)]
fn generate_pseudo_random(len: usize) -> Result<Vec<u8>> {
    // Use platform time as seed
    use wrt_platform::time::PlatformTime;
    let seed = PlatformTime::monotonic_ns();

    let mut buffer = vec![0u8; len];

    // Xorshift64* algorithm for fast pseudo-random generation
    let mut state = seed;
    if state == 0 {
        state = 0x0BAD_C0FF_EEDE_ADBF_u128 as u64; // Non-zero seed
    }

    for chunk in buffer.chunks_mut(8) {
        // Xorshift64* algorithm
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let value = state.wrapping_mul(0x2545_F491_4F6C_DD1D);

        // Convert to bytes
        let bytes = value.to_le_bytes();
        for (i, byte) in chunk.iter_mut().enumerate() {
            if i < bytes.len() {
                *byte = bytes[i];
            }
        }
    }

    Ok(buffer)
}

/// Validate random generation with capabilities
///
/// Helper function to check if random generation is allowed
///
/// # Errors
///
/// Returns an error if the requested random type (secure or pseudo) is not
/// permitted by the provided capabilities.
pub fn validate_random_capabilities(
    secure: bool,
    capabilities: &WasiRandomCapabilities,
) -> Result<()> {
    if secure && !capabilities.secure_random {
        return Err(Error::wasi_permission_denied("Secure random access denied"));
    }

    if !secure && !capabilities.pseudo_random {
        return Err(Error::wasi_permission_denied("Pseudo-random access denied"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_length() -> Result<()> {
        let args = vec![Value::U64(1024)];
        let len = extract_length(args)?;
        assert_eq!(len, 1024);

        let args = vec![Value::U32(512)];
        let len = extract_length(args)?;
        assert_eq!(len, 512);

        let args = vec![Value::S32(256)];
        let len = extract_length(args)?;
        assert_eq!(len, 256);

        // Test negative length
        let args = vec![Value::S32(-1)];
        let result = extract_length(args);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_wasi_get_random_bytes() -> Result<()> {
        // Test small request
        let args = vec![Value::U64(16)];
        let result = wasi_get_random_bytes(&mut (), args)?;
        assert_eq!(result.len(), 1);

        if let Value::List(bytes) = &result[0] {
            assert_eq!(bytes.len(), 16);
            for byte in bytes {
                assert!(matches!(byte, Value::U8(_)));
            }
        } else {
            panic!("Expected list of bytes");
        }

        // Test large request (should fail)
        let args = vec![Value::U64(2 * 1024 * 1024)]; // 2MB
        let result = wasi_get_random_bytes(&mut (), args);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_wasi_get_insecure_random_bytes() -> Result<()> {
        // Test medium request
        let args = vec![Value::U64(1024)];
        let result = wasi_get_insecure_random_bytes(&mut (), args)?;
        assert_eq!(result.len(), 1);

        if let Value::List(bytes) = &result[0] {
            assert_eq!(bytes.len(), 1024);

            // Check that bytes are not all the same (very unlikely with proper random)
            let first_byte = match &bytes[0] {
                Value::U8(b) => *b,
                _ => panic!("Expected U8"),
            };

            let all_same =
                bytes.iter().all(|b| matches!(b, Value::U8(byte) if *byte == first_byte));
            assert!(!all_same, "Random bytes should not all be the same");
        } else {
            panic!("Expected list of bytes");
        }

        Ok(())
    }

    #[test]
    fn test_wasi_get_random_u64() -> Result<()> {
        let result = wasi_get_random_u64(&mut (), vec![])?;
        assert_eq!(result.len(), 1);

        if let Value::U64(value) = &result[0] {
            // Value should be non-zero (very unlikely to be 0 with proper random)
            assert!(*value != 0, "Random u64 should not be zero");
        } else {
            panic!("Expected u64 value");
        }

        Ok(())
    }

    #[test]
    fn test_pseudo_random_generation() -> Result<()> {
        // Generate two sets of pseudo-random bytes
        let random1 = generate_pseudo_random(32)?;
        let random2 = generate_pseudo_random(32)?;

        // They should be different (unless we hit the same seed timing)
        assert_ne!(
            random1, random2,
            "Pseudo-random should produce different values"
        );

        // Check distribution (very basic test)
        let sum: u32 = random1.iter().map(|&b| b as u32).sum();
        let avg = sum / 32;

        // Average should be somewhere around 128 (middle of 0-255 range)
        assert!(avg > 64 && avg < 192, "Random distribution seems off");

        Ok(())
    }

    #[test]
    fn test_validate_random_capabilities() -> Result<()> {
        let capabilities = WasiRandomCapabilities {
            secure_random: true,
            pseudo_random: false,
        };

        // Should succeed for secure when allowed
        validate_random_capabilities(true, &capabilities)?;

        // Should fail for pseudo when not allowed
        let result = validate_random_capabilities(false, &capabilities);
        assert!(result.is_err());

        Ok(())
    }
}
