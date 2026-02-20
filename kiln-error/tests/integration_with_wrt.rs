// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Integration tests for wrt-error with the main wrt crate.
extern crate alloc;

// Tests the integration of wrt-error with the main wrt crate.
//
// This file tests the error handling functionality in the context of how it
// would be used
// in the main wrt crate.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::unnecessary_literal_unwrap,
    clippy::needless_borrows_for_generic_args,
    clippy::uninlined_format_args
)]
mod tests {
    use wrt_error::{Error, Result};

    // Binary std/no_std choice
    // Binary std/no_std choice
    // The `Display` trait on `wrt_error::Error` handles this internally.

    #[test]
    fn test_result_operations() {
        let result: Result<()> = Ok(());
        assert!(result.is_ok());
        result.unwrap(); // Should not panic

        let error_result: Result<()> = Err(Error::runtime_error("Test error"));
        assert!(error_result.is_err());
        // Binary std/no_std choice
        // Binary std/no_std choice
        // #[cfg(feature = "std")]
        // let _ = std::format!("{}", error_result.as_ref().err().unwrap();
    }

    #[cfg(feature = "std")]
    mod std_alloc_tests {
        use wrt_error::{Error, kinds};
        // Re-import necessary items if not directly available or for clarity
        // use crate::Error; // Assuming Error is pub from lib.rs

        #[test]
        fn test_error_conversion_memory() {
            // let _mem_error = kinds::MemoryAccessOutOfBoundsError { address: 100, length:
            // 32 }; // Removed this line In std mode, plain format! is from
            // std. Provide a static string for Error::memory_error
            let error = Error::memory_error("Test memory access out of bounds: addr 100, len 32");
            assert!(error.is_memory_error());
            // Optionally, assert the message if it needs to be specific and static
            assert_eq!(
                error.message,
                "Test memory access out of bounds: addr 100, len 32"
            );
        }

        #[test]
        fn test_error_conversion_division() {
            let div_error = kinds::division_by_zero_error();
            let error = Error::from(div_error);
            // In std mode, plain format! is from std.
            assert!(format!("{error}").contains("Division by zero"));
        }
    }
}
