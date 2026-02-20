//! WRTD Alloc Runtime Main Entry Point
//! 
//! This is the main entry point for the wrtd-alloc binary.
//! SW-REQ-ID: REQ_FUNC_033

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

// Re-export the main module functionality
mod main;

use main::alloc_runtime;

/// Binary std/no_std choice
fn main() -> ! {
    alloc_runtime::main()
}

// Binary std/no_std choice
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In real implementation, would handle panic appropriately
    // - Log to serial/flash for debugging
    // - Reset system
    // - Toggle error LED
    loop {}
}