// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

//! Kiln-Synth Bridge — C-ABI interface between Synth-compiled ARM code and Kiln runtime
//!
//! This crate provides `extern "C"` functions that Synth-compiled native ARM code
//! calls to dispatch imported host functions through the Kiln runtime.
//!
//! # ABI Contract
//!
//! Synth-compiled code calls imports via:
//! ```text
//! MOV R0, #<import_index>
//! BL  __meld_dispatch_import
//! ; Result in R0
//! ```
//!
//! The bridge maps import indices to `(module_name, function_name)` pairs using
//! an import table populated during initialization.
//!
//! # Linking
//!
//! This crate builds as both `rlib` (for Rust consumers) and `staticlib` (for
//! linking with Synth-compiled ELF objects via `arm-none-eabi-ld`).

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

use core::sync::atomic::{AtomicPtr, Ordering};

/// Maximum number of imports a single component can declare.
/// This bounds the import descriptor table size for deterministic memory usage.
const MAX_IMPORTS: usize = 64;

/// Import descriptor — maps an import index to its module/function name pair.
///
/// These are stored in a fixed-size table, indexed by the import index that
/// Synth embeds in `MOV R0, #idx` instructions.
#[derive(Debug, Clone)]
pub struct ImportDescriptor {
    /// Module name (e.g., "wasi:cli/stdout")
    pub module: &'static str,
    /// Function name (e.g., "write")
    pub name: &'static str,
}

/// Dispatch function type — called by the bridge to handle an import.
///
/// Takes `(import_index, arg0)` and returns a result value.
/// The arg0 is passed in R1 by Synth's AAPCS-compatible code.
pub type DispatchFn = extern "C" fn(import_index: u32, arg0: u32) -> u32;

/// Bridge state — holds the import table and dispatch function.
///
/// This is initialized once during firmware startup before any Synth-compiled
/// code executes. The bridge is intentionally simple: a flat table of
/// descriptors and a single dispatch function pointer.
struct BridgeState {
    /// Import descriptor table (indexed by import_index from Synth)
    descriptors: [Option<ImportDescriptor>; MAX_IMPORTS],
    /// Number of registered imports
    count: usize,
    /// User-provided dispatch function
    dispatch: Option<DispatchFn>,
}

impl BridgeState {
    const fn new() -> Self {
        Self {
            descriptors: [const { None }; MAX_IMPORTS],
            count: 0,
            dispatch: None,
        }
    }
}

/// Global bridge state.
///
/// # Safety
///
/// Accessed via raw pointers (`&raw const`/`&raw mut`) per Rust 2024 edition rules.
/// The bridge must be fully initialized (via `meld_bridge_init`) before any
/// Synth-compiled code runs. On Cortex-M (single-core, no preemption during
/// init), this is safe.
static mut BRIDGE: BridgeState = BridgeState::new();

/// Pointer to WASM linear memory base (set during initialization).
static MEMORY_BASE: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());

// === Public C-ABI functions (called by Synth-compiled ARM code) ===

/// Dispatch an imported function call.
///
/// Called by Synth-compiled code via `BL __meld_dispatch_import`.
/// R0 contains the import index, R1 contains the first argument.
///
/// # Safety
///
/// This function is called from native ARM code via C ABI. The bridge must
/// be initialized before this is called. If the bridge is not initialized
/// or the import index is out of range, this function traps (UDF).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __meld_dispatch_import(import_index: u32, arg0: u32) -> u32 {
    let ptr = &raw const BRIDGE;
    let dispatch = unsafe { (*ptr).dispatch };
    let count = unsafe { (*ptr).count };

    if let Some(dispatch) = dispatch {
        if (import_index as usize) < count {
            return dispatch(import_index, arg0);
        }
    }

    // No dispatcher or index out of range — trap
    // On ARM Cortex-M, this generates a HardFault
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("udf #0xFE");
    }

    #[cfg(not(target_arch = "arm"))]
    {
        // For testing on host: panic with a descriptive message
        #[cfg(feature = "std")]
        panic!(
            "meld_dispatch_import: unresolved import index {} (bridge has {} imports)",
            import_index, count
        );
        #[cfg(not(feature = "std"))]
        loop {} // Spin trap for no_std non-ARM
    }

    #[allow(unreachable_code)]
    0
}

/// Get the base address of the WASM linear memory.
///
/// Returns a pointer to the start of the linear memory region. This pointer
/// is valid for the lifetime of the runtime. Returns null if memory has not
/// been initialized.
#[unsafe(no_mangle)]
pub extern "C" fn __meld_get_memory_base() -> *mut u8 {
    MEMORY_BASE.load(Ordering::Relaxed)
}

// === Initialization API (called by firmware startup code) ===

/// Initialize the Meld bridge with a dispatch function.
///
/// Must be called once during firmware initialization, before any
/// Synth-compiled function is invoked.
///
/// # Safety
///
/// Must be called exactly once, before any concurrent access to the bridge.
/// On single-core Cortex-M with interrupts disabled during init, this is safe.
pub unsafe fn meld_bridge_init(dispatch: DispatchFn) {
    let ptr = &raw mut BRIDGE;
    unsafe { (*ptr).dispatch = Some(dispatch) };
}

/// Register an import descriptor at the given index.
///
/// This populates the import table so the dispatch function can look up
/// the module/function name for a given import index.
///
/// # Safety
///
/// Must be called during initialization, before any concurrent access.
pub unsafe fn meld_bridge_register_import(
    index: u32,
    descriptor: ImportDescriptor,
) -> Result<(), &'static str> {
    let ptr = &raw mut BRIDGE;
    let idx = index as usize;
    if idx >= MAX_IMPORTS {
        return Err("import index exceeds MAX_IMPORTS");
    }
    unsafe {
        (*ptr).descriptors[idx] = Some(descriptor);
        if idx >= (*ptr).count {
            (*ptr).count = idx + 1;
        }
    }
    Ok(())
}

/// Set the WASM linear memory base pointer.
///
/// # Safety
///
/// The pointer must remain valid for the lifetime of the runtime.
pub unsafe fn meld_bridge_set_memory(base: *mut u8) {
    MEMORY_BASE.store(base, Ordering::Relaxed);
}

/// Get the import descriptor for a given index (for use by dispatch functions).
///
/// # Safety
///
/// The caller must ensure the bridge has been initialized and no concurrent
/// mutation is occurring.
pub unsafe fn meld_bridge_get_descriptor(index: u32) -> Option<&'static ImportDescriptor> {
    let ptr = &raw const BRIDGE;
    let idx = index as usize;
    if idx < MAX_IMPORTS {
        unsafe { (*ptr).descriptors[idx].as_ref() }
    } else {
        None
    }
}

/// Get the number of registered imports.
///
/// # Safety
///
/// The caller must ensure the bridge has been initialized and no concurrent
/// mutation is occurring.
pub unsafe fn meld_bridge_import_count() -> usize {
    let ptr = &raw const BRIDGE;
    unsafe { (*ptr).count }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn test_dispatch(import_index: u32, arg0: u32) -> u32 {
        import_index + arg0
    }

    #[test]
    fn test_bridge_initialization() {
        unsafe {
            // Reset bridge state for test
            let ptr = &raw mut BRIDGE;
            *ptr = BridgeState::new();

            meld_bridge_init(test_dispatch);

            meld_bridge_register_import(
                0,
                ImportDescriptor {
                    module: "env",
                    name: "log",
                },
            )
            .unwrap();

            meld_bridge_register_import(
                1,
                ImportDescriptor {
                    module: "wasi:cli/stdout",
                    name: "write",
                },
            )
            .unwrap();
        }

        assert_eq!(unsafe { meld_bridge_import_count() }, 2);

        let desc0 = unsafe { meld_bridge_get_descriptor(0) }.unwrap();
        assert_eq!(desc0.module, "env");
        assert_eq!(desc0.name, "log");

        let desc1 = unsafe { meld_bridge_get_descriptor(1) }.unwrap();
        assert_eq!(desc1.module, "wasi:cli/stdout");
        assert_eq!(desc1.name, "write");
    }

    #[test]
    fn test_dispatch_call() {
        unsafe {
            let ptr = &raw mut BRIDGE;
            *ptr = BridgeState::new();
            meld_bridge_init(test_dispatch);
            meld_bridge_register_import(
                0,
                ImportDescriptor {
                    module: "env",
                    name: "log",
                },
            )
            .unwrap();
        }

        // Call through the C-ABI interface
        let result = unsafe { __meld_dispatch_import(0, 42) };
        assert_eq!(result, 42); // 0 + 42
    }

    #[test]
    fn test_memory_base() {
        let mut buffer = [0u8; 256];
        let ptr = buffer.as_mut_ptr();

        unsafe { meld_bridge_set_memory(ptr) };
        assert_eq!(__meld_get_memory_base(), ptr);
    }

    #[test]
    fn test_max_imports_boundary() {
        unsafe {
            let ptr = &raw mut BRIDGE;
            *ptr = BridgeState::new();
        }

        // Should succeed at MAX_IMPORTS - 1
        let result = unsafe {
            meld_bridge_register_import(
                (MAX_IMPORTS - 1) as u32,
                ImportDescriptor {
                    module: "test",
                    name: "last",
                },
            )
        };
        assert!(result.is_ok());

        // Should fail at MAX_IMPORTS
        let result = unsafe {
            meld_bridge_register_import(
                MAX_IMPORTS as u32,
                ImportDescriptor {
                    module: "test",
                    name: "overflow",
                },
            )
        };
        assert!(result.is_err());
    }
}
