//! WRTD No-Std Runtime Main Entry Point
//! 
//! This is the main entry point for the wrtd-nostd binary.
//! SW-REQ-ID: REQ_FUNC_033

#![no_std]
#![no_main]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Re-export the main module functionality
mod main;

use main::nostd_runtime;

/// Main entry point for nostd runtime mode
#[no_mangle]
fn main() -> ! {
    nostd_runtime::main()
}

// Panic handler for nostd mode
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // In real implementation, would handle panic appropriately
    // - Log to serial/flash for debugging
    // - Reset system
    // - Toggle error LED
    loop {}
}