//! WASI Dispatcher - Unified entry point for all WASI calls
//!
//! This module provides a centralized dispatcher that routes WASI calls
//! to the appropriate preview2 implementations. This replaces ad-hoc
//! stub implementations scattered throughout the codebase.
//!
//! # Architecture
//!
//! The dispatcher handles core WASM values (raw i32/i64 plus memory pointers)
//! via `dispatch_core`, reached through its `HostImportHandler` implementation.
//!
//! # Usage from kiln-runtime
//!
//! ```ignore
//! use kiln_wasi::WasiDispatcher;
//!
//! let dispatcher = WasiDispatcher::new(capabilities)?;
//! // The dispatcher is installed as the engine's `HostImportHandler`; the
//! // runtime then reaches it through `call_import`, which delegates to
//! // `dispatch_core` with raw core values and the instance's linear memory.
//! engine.set_host_handler(Box::new(dispatcher));
//! ```

// Import tracing utilities for structured logging
#[cfg(feature = "tracing")]
use kiln_foundation::tracing::{trace, warn};

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(all(feature = "wasi-sockets", feature = "std"))]
use crate::Value;
use crate::{
    capabilities::WasiCapabilities,
    host_provider::resource_manager::{WasiResourceManager, WasiResourceType},
    prelude::*,
};
#[cfg(feature = "std")]
use kiln_foundation::MemoryAccessor;

// Global storage for WASI args and env using Mutex for thread-safety
// This allows passing args from kilnd to the engine without threading through all layers
#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(feature = "std")]
static GLOBAL_WASI_ARGS: Mutex<Vec<String>> = Mutex::new(Vec::new());
#[cfg(feature = "std")]
static GLOBAL_WASI_ENV: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Set the global WASI arguments that will be used by all dispatchers
#[cfg(feature = "std")]
pub fn set_global_wasi_args(args: Vec<String>) {
    if let Ok(mut guard) = GLOBAL_WASI_ARGS.lock() {
        *guard = args;
    }
}

/// Get the global WASI arguments
///
/// Falls back to `std::env::args()` if no global args are set
///
/// # Errors
///
/// Returns an error if the global WASI args mutex is poisoned (a thread
/// panicked while holding the lock).
#[cfg(feature = "std")]
pub fn get_global_wasi_args() -> Result<Vec<String>> {
    // Per CLAUDE.md: fail loud and early. A poisoned mutex means a thread panicked
    // while holding the lock - the data may be in an inconsistent state.
    let global_args = GLOBAL_WASI_ARGS
        .lock()
        .map_err(|_| {
            Error::wasi_runtime_error(
                "GLOBAL_WASI_ARGS mutex poisoned - a thread panicked while holding the lock",
            )
        })?
        .clone();
    if global_args.is_empty() {
        // Fall back to actual process args
        Ok(std::env::args().collect())
    } else {
        Ok(global_args)
    }
}

/// Get the global WASI environment variables
///
/// Falls back to `std::env::vars()` if no global env vars are set
///
/// # Errors
///
/// Returns an error if the global WASI env mutex is poisoned (a thread
/// panicked while holding the lock).
#[cfg(feature = "std")]
pub fn get_global_wasi_env() -> Result<Vec<(String, String)>> {
    // Per CLAUDE.md: fail loud and early. A poisoned mutex means a thread panicked
    // while holding the lock - the data may be in an inconsistent state.
    let global_env = GLOBAL_WASI_ENV
        .lock()
        .map_err(|_| {
            Error::wasi_runtime_error(
                "GLOBAL_WASI_ENV mutex poisoned - a thread panicked while holding the lock",
            )
        })?
        .clone();
    if global_env.is_empty() {
        // Fall back to actual process environment
        Ok(std::env::vars().collect())
    } else {
        Ok(global_env)
    }
}

/// Set the global WASI environment variables
#[cfg(feature = "std")]
pub fn set_global_wasi_env(env: Vec<(String, String)>) {
    if let Ok(mut guard) = GLOBAL_WASI_ENV.lock() {
        *guard = env;
    }
}

#[cfg(all(feature = "wasi-sockets", feature = "std"))]
use crate::preview2::sockets::{
    wasi_resolve_addresses, wasi_tcp_accept, wasi_tcp_bind, wasi_tcp_connect, wasi_tcp_create,
    wasi_tcp_listen, wasi_tcp_recv, wasi_tcp_send, wasi_tcp_shutdown, wasi_udp_bind,
    wasi_udp_create, wasi_udp_recv, wasi_udp_send,
};

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::path::PathBuf;

/// Type of file descriptor in the WASI filesystem
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub enum FileDescriptorType {
    /// Standard output stream
    Stdout,
    /// Standard error stream
    Stderr,
    /// Standard input stream
    Stdin,
    /// Pre-opened directory with its path
    PreopenDirectory(PathBuf),
    /// Regular file with its path
    RegularFile(PathBuf),
}

/// A file descriptor entry in the descriptor table
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct FileDescriptorEntry {
    /// Type of this descriptor
    pub fd_type: FileDescriptorType,
    /// Whether this descriptor can be read from
    pub read: bool,
    /// Whether this descriptor can be written to
    pub write: bool,
    /// The file handle `open-at` opened (after sandbox authorization) for this
    /// descriptor. `descriptor.read`/`write`/`sync-data` operate on this handle
    /// only and never reopen by path, so a symlink swapped in after `open-at`
    /// cannot redirect them. `None` for everything `open-at` did not open
    /// (std streams, preopens, stream handles, directories).
    pub file: Option<std::fs::File>,
}

/// A WASI function served by [`WasiDispatcher::dispatch_core`].
///
/// [`SERVED_WASI_IMPORTS`] is the only place an import name is recognised:
/// `dispatch_core` looks the import up there and matches on the variant, and
/// the link-time check ([`HostImportHandler::resolves`]) asks the same table
/// (AD-LINK-001). A variant missing from the table is never constructed and
/// fails the build as dead code; a variant without a dispatch arm fails the
/// match's exhaustiveness. Each variant carries the same cfg gate as its arm.
///
/// [`HostImportHandler::resolves`]: kiln_foundation::HostImportHandler::resolves
// The workspace allows dead code; here an unconstructed variant is exactly
// the table/dispatch disagreement AD-LINK-001 must catch, so deny it.
#[deny(dead_code)]
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WasiFn {
    GetStdout,
    GetStderr,
    GetStdin,
    GetArguments,
    GetEnvironment,
    InitialCwd,
    GetTerminalStdin,
    GetTerminalStdout,
    GetTerminalStderr,
    TerminalResourceDrop,
    CliExit,
    PreviewOneProcExit,
    Poll,
    PollableReady,
    PollableBlock,
    PollableDrop,
    OutputStreamCheckWrite,
    OutputStreamSubscribe,
    #[cfg(feature = "wasi-io")]
    OutputStreamBlockingWriteAndFlush,
    #[cfg(feature = "wasi-io")]
    OutputStreamWrite,
    #[cfg(feature = "wasi-io")]
    OutputStreamBlockingFlush,
    StreamDrop,
    InputStreamRead,
    InputStreamSubscribe,
    ErrorDrop,
    ErrorToDebugString,
    #[cfg(feature = "wasi-clocks")]
    WallClockNow,
    #[cfg(feature = "wasi-clocks")]
    WallClockResolution,
    #[cfg(feature = "wasi-clocks")]
    MonotonicClockNow,
    #[cfg(feature = "wasi-clocks")]
    MonotonicClockResolution,
    #[cfg(feature = "wasi-clocks")]
    MonotonicClockSubscribeDuration,
    #[cfg(feature = "wasi-clocks")]
    MonotonicClockSubscribeInstant,
    #[cfg(feature = "wasi-random")]
    GetRandomBytes,
    #[cfg(feature = "wasi-random")]
    GetRandomU64,
    #[cfg(feature = "wasi-random")]
    InsecureSeed,
    #[cfg(feature = "wasi-random")]
    GetInsecureRandomBytes,
    #[cfg(feature = "wasi-random")]
    GetInsecureRandomU64,
    #[cfg(feature = "nn-preview2")]
    NnLoad,
    #[cfg(feature = "nn-preview2")]
    NnInitExecutionContext,
    #[cfg(feature = "nn-preview2")]
    NnSetInput,
    #[cfg(feature = "nn-preview2")]
    NnCompute,
    #[cfg(feature = "nn-preview2")]
    NnGetOutput,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorGetType,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorOpenAt,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorStat,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorStatAt,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorReadViaStream,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorReadDirectory,
    #[cfg(feature = "wasi-filesystem")]
    ReadDirectoryEntry,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorMetadataHash,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorDrop,
    #[cfg(feature = "wasi-filesystem")]
    DirectoryEntryStreamDrop,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorRead,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorWrite,
    #[cfg(feature = "wasi-filesystem")]
    DescriptorSyncData,
    #[cfg(feature = "wasi-filesystem")]
    CreateDirectoryAt,
    #[cfg(feature = "wasi-filesystem")]
    RemoveDirectoryAt,
    #[cfg(feature = "wasi-filesystem")]
    UnlinkFileAt,
    #[cfg(feature = "wasi-filesystem")]
    RenameAt,
    #[cfg(feature = "wasi-filesystem")]
    LinkAt,
    #[cfg(feature = "wasi-filesystem")]
    SymlinkAt,
    #[cfg(feature = "wasi-filesystem")]
    ReadlinkAt,
    #[cfg(feature = "wasi-filesystem")]
    GetDirectories,
    #[cfg(feature = "wasi-sockets")]
    TcpCreateSocket,
    #[cfg(feature = "wasi-sockets")]
    TcpStartConnect,
    #[cfg(feature = "wasi-sockets")]
    TcpStartBind,
    #[cfg(feature = "wasi-sockets")]
    TcpListen,
    #[cfg(feature = "wasi-sockets")]
    TcpAccept,
    #[cfg(feature = "wasi-sockets")]
    TcpSend,
    #[cfg(feature = "wasi-sockets")]
    TcpReceive,
    #[cfg(feature = "wasi-sockets")]
    TcpShutdown,
    #[cfg(feature = "wasi-sockets")]
    UdpCreateSocket,
    #[cfg(feature = "wasi-sockets")]
    UdpStartBind,
    #[cfg(feature = "wasi-sockets")]
    UdpSend,
    #[cfg(feature = "wasi-sockets")]
    UdpReceive,
    #[cfg(feature = "wasi-sockets")]
    ResolveAddresses,
}

/// Every `(interface, function)` import kiln serves, interface without its
/// `@version`. Shared by dispatch and the link-time import check (SR-65,
/// AD-LINK-001); see [`WasiFn`].
#[cfg(feature = "std")]
const SERVED_WASI_IMPORTS: &[(&str, &str, WasiFn)] = &[
    ("wasi:cli/stdout", "get-stdout", WasiFn::GetStdout),
    ("wasi:cli/stderr", "get-stderr", WasiFn::GetStderr),
    ("wasi:cli/stdin", "get-stdin", WasiFn::GetStdin),
    (
        "wasi:cli/environment",
        "get-arguments",
        WasiFn::GetArguments,
    ),
    (
        "wasi:cli/environment",
        "get-environment",
        WasiFn::GetEnvironment,
    ),
    ("wasi:cli/environment", "initial-cwd", WasiFn::InitialCwd),
    (
        "wasi:cli/terminal-stdin",
        "get-terminal-stdin",
        WasiFn::GetTerminalStdin,
    ),
    (
        "wasi:cli/terminal-stdout",
        "get-terminal-stdout",
        WasiFn::GetTerminalStdout,
    ),
    (
        "wasi:cli/terminal-stderr",
        "get-terminal-stderr",
        WasiFn::GetTerminalStderr,
    ),
    (
        "wasi:cli/terminal-input",
        "[resource-drop]terminal-input",
        WasiFn::TerminalResourceDrop,
    ),
    (
        "wasi:cli/terminal-output",
        "[resource-drop]terminal-output",
        WasiFn::TerminalResourceDrop,
    ),
    ("wasi:cli/exit", "exit", WasiFn::CliExit),
    (
        "wasi_snapshot_preview1",
        "proc_exit",
        WasiFn::PreviewOneProcExit,
    ),
    ("wasi:io/poll", "poll", WasiFn::Poll),
    (
        "wasi:io/poll",
        "[method]pollable.ready",
        WasiFn::PollableReady,
    ),
    (
        "wasi:io/poll",
        "[method]pollable.block",
        WasiFn::PollableBlock,
    ),
    ("wasi:io/poll", "pollable.block", WasiFn::PollableBlock),
    (
        "wasi:io/poll",
        "[resource-drop]pollable",
        WasiFn::PollableDrop,
    ),
    (
        "wasi:io/streams",
        "[method]output-stream.check-write",
        WasiFn::OutputStreamCheckWrite,
    ),
    (
        "wasi:io/streams",
        "output-stream.check-write",
        WasiFn::OutputStreamCheckWrite,
    ),
    (
        "wasi:io/streams",
        "[method]output-stream.subscribe",
        WasiFn::OutputStreamSubscribe,
    ),
    (
        "wasi:io/streams",
        "output-stream.subscribe",
        WasiFn::OutputStreamSubscribe,
    ),
    #[cfg(feature = "wasi-io")]
    (
        "wasi:io/streams",
        "[method]output-stream.blocking-write-and-flush",
        WasiFn::OutputStreamBlockingWriteAndFlush,
    ),
    #[cfg(feature = "wasi-io")]
    (
        "wasi:io/streams",
        "output-stream.blocking-write-and-flush",
        WasiFn::OutputStreamBlockingWriteAndFlush,
    ),
    #[cfg(feature = "wasi-io")]
    (
        "wasi:io/streams",
        "[method]output-stream.write",
        WasiFn::OutputStreamWrite,
    ),
    #[cfg(feature = "wasi-io")]
    (
        "wasi:io/streams",
        "output-stream.write",
        WasiFn::OutputStreamWrite,
    ),
    #[cfg(feature = "wasi-io")]
    (
        "wasi:io/streams",
        "[method]output-stream.blocking-flush",
        WasiFn::OutputStreamBlockingFlush,
    ),
    #[cfg(feature = "wasi-io")]
    (
        "wasi:io/streams",
        "output-stream.blocking-flush",
        WasiFn::OutputStreamBlockingFlush,
    ),
    (
        "wasi:io/streams",
        "[resource-drop]output-stream",
        WasiFn::StreamDrop,
    ),
    (
        "wasi:io/streams",
        "[resource-drop]input-stream",
        WasiFn::StreamDrop,
    ),
    (
        "wasi:io/streams",
        "[method]input-stream.read",
        WasiFn::InputStreamRead,
    ),
    (
        "wasi:io/streams",
        "input-stream.read",
        WasiFn::InputStreamRead,
    ),
    (
        "wasi:io/streams",
        "[method]input-stream.blocking-read",
        WasiFn::InputStreamRead,
    ),
    (
        "wasi:io/streams",
        "input-stream.blocking-read",
        WasiFn::InputStreamRead,
    ),
    (
        "wasi:io/streams",
        "[method]input-stream.subscribe",
        WasiFn::InputStreamSubscribe,
    ),
    (
        "wasi:io/streams",
        "input-stream.subscribe",
        WasiFn::InputStreamSubscribe,
    ),
    ("wasi:io/error", "[resource-drop]error", WasiFn::ErrorDrop),
    (
        "wasi:io/error",
        "[method]error.to-debug-string",
        WasiFn::ErrorToDebugString,
    ),
    #[cfg(feature = "wasi-clocks")]
    ("wasi:clocks/wall-clock", "now", WasiFn::WallClockNow),
    #[cfg(feature = "wasi-clocks")]
    (
        "wasi:clocks/wall-clock",
        "resolution",
        WasiFn::WallClockResolution,
    ),
    #[cfg(feature = "wasi-clocks")]
    (
        "wasi:clocks/monotonic-clock",
        "now",
        WasiFn::MonotonicClockNow,
    ),
    #[cfg(feature = "wasi-clocks")]
    (
        "wasi:clocks/monotonic-clock",
        "resolution",
        WasiFn::MonotonicClockResolution,
    ),
    #[cfg(feature = "wasi-clocks")]
    (
        "wasi:clocks/monotonic-clock",
        "subscribe-duration",
        WasiFn::MonotonicClockSubscribeDuration,
    ),
    #[cfg(feature = "wasi-clocks")]
    (
        "wasi:clocks/monotonic-clock",
        "subscribe-instant",
        WasiFn::MonotonicClockSubscribeInstant,
    ),
    #[cfg(feature = "wasi-random")]
    (
        "wasi:random/random",
        "get-random-bytes",
        WasiFn::GetRandomBytes,
    ),
    #[cfg(feature = "wasi-random")]
    ("wasi:random/random", "get-random-u64", WasiFn::GetRandomU64),
    #[cfg(feature = "wasi-random")]
    (
        "wasi:random/insecure-seed",
        "insecure-seed",
        WasiFn::InsecureSeed,
    ),
    #[cfg(feature = "wasi-random")]
    (
        "wasi:random/insecure",
        "get-insecure-random-bytes",
        WasiFn::GetInsecureRandomBytes,
    ),
    #[cfg(feature = "wasi-random")]
    (
        "wasi:random/insecure",
        "get-insecure-random-u64",
        WasiFn::GetInsecureRandomU64,
    ),
    #[cfg(feature = "nn-preview2")]
    ("wasi:nn/inference", "load", WasiFn::NnLoad),
    #[cfg(feature = "nn-preview2")]
    (
        "wasi:nn/inference",
        "init-execution-context",
        WasiFn::NnInitExecutionContext,
    ),
    #[cfg(feature = "nn-preview2")]
    ("wasi:nn/inference", "set-input", WasiFn::NnSetInput),
    #[cfg(feature = "nn-preview2")]
    ("wasi:nn/inference", "compute", WasiFn::NnCompute),
    #[cfg(feature = "nn-preview2")]
    ("wasi:nn/inference", "get-output", WasiFn::NnGetOutput),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.get-type",
        WasiFn::DescriptorGetType,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.open-at",
        WasiFn::DescriptorOpenAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.stat",
        WasiFn::DescriptorStat,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.stat-at",
        WasiFn::DescriptorStatAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.read-via-stream",
        WasiFn::DescriptorReadViaStream,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.read-directory",
        WasiFn::DescriptorReadDirectory,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]directory-entry-stream.read-directory-entry",
        WasiFn::ReadDirectoryEntry,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.metadata-hash",
        WasiFn::DescriptorMetadataHash,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.metadata-hash-at",
        WasiFn::DescriptorMetadataHash,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[resource-drop]descriptor",
        WasiFn::DescriptorDrop,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[resource-drop]directory-entry-stream",
        WasiFn::DirectoryEntryStreamDrop,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.read",
        WasiFn::DescriptorRead,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.write",
        WasiFn::DescriptorWrite,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.sync-data",
        WasiFn::DescriptorSyncData,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.create-directory-at",
        WasiFn::CreateDirectoryAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.remove-directory-at",
        WasiFn::RemoveDirectoryAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.unlink-file-at",
        WasiFn::UnlinkFileAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.rename-at",
        WasiFn::RenameAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.link-at",
        WasiFn::LinkAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.symlink-at",
        WasiFn::SymlinkAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/types",
        "[method]descriptor.readlink-at",
        WasiFn::ReadlinkAt,
    ),
    #[cfg(feature = "wasi-filesystem")]
    (
        "wasi:filesystem/preopens",
        "get-directories",
        WasiFn::GetDirectories,
    ),
    #[cfg(feature = "wasi-sockets")]
    (
        "wasi:sockets/tcp",
        "create-tcp-socket",
        WasiFn::TcpCreateSocket,
    ),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "start-connect", WasiFn::TcpStartConnect),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "start-bind", WasiFn::TcpStartBind),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "listen", WasiFn::TcpListen),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "accept", WasiFn::TcpAccept),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "send", WasiFn::TcpSend),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "receive", WasiFn::TcpReceive),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/tcp", "shutdown", WasiFn::TcpShutdown),
    #[cfg(feature = "wasi-sockets")]
    (
        "wasi:sockets/udp",
        "create-udp-socket",
        WasiFn::UdpCreateSocket,
    ),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/udp", "start-bind", WasiFn::UdpStartBind),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/udp", "send", WasiFn::UdpSend),
    #[cfg(feature = "wasi-sockets")]
    ("wasi:sockets/udp", "receive", WasiFn::UdpReceive),
    #[cfg(feature = "wasi-sockets")]
    (
        "wasi:sockets/ip-name-lookup",
        "resolve-addresses",
        WasiFn::ResolveAddresses,
    ),
];

/// The served WASI function an import names, if kiln serves it. The import's
/// interface may carry an `@version`, and its function meld's per-instance
/// `$N` suffix (SR-81); the table is keyed without either. The suffix is only
/// stripped here, so an unknown base name stays unresolved.
#[cfg(feature = "std")]
fn served_wasi_fn(interface: &str, function: &str) -> Option<WasiFn> {
    let base_interface = WasiDispatcher::strip_version(interface);
    let (base_function, _instance) = kiln_foundation::split_instance_suffix(function);
    SERVED_WASI_IMPORTS
        .iter()
        .find(|(i, f, _)| *i == base_interface && *f == base_function)
        .map(|(_, _, served)| *served)
}

/// WASI Dispatcher - unified entry point for all WASI function calls
pub struct WasiDispatcher {
    /// WASI capabilities for this dispatcher
    capabilities: WasiCapabilities,
    /// Resource manager for Preview2 resource semantics
    resource_manager: WasiResourceManager,
    /// Command-line arguments
    args: Vec<String>,
    /// Environment variables as (key, value) pairs
    env_vars: Vec<(String, String)>,
    /// Pre-allocated memory for args (`list_ptr`, `string_ptrs`)
    /// Set by the engine after calling `cabi_realloc`
    /// Each string is allocated separately to ensure proper allocator metadata
    args_alloc: Option<(u32, Vec<(u32, u32)>)>,
    /// File descriptor table (maps handle -> descriptor info)
    /// Note: This is separate from `resource_manager` - used for filesystem operations
    #[cfg(feature = "std")]
    fd_table: HashMap<u32, FileDescriptorEntry>,
    /// Pre-opened directories (list of (handle, path) pairs)
    #[cfg(feature = "std")]
    preopens: Vec<(u32, PathBuf)>,
    /// Pre-allocated memory for the get-directories list return (`list_ptr`, `string_ptrs`).
    /// Set by the engine after calling `cabi_realloc`; mirrors `args_alloc`. The
    /// canonical-ABI return area for `list<tuple<descriptor,string>>` is only 8 bytes,
    /// so the entry array and path strings must live in this guest-owned memory.
    #[cfg(feature = "std")]
    preopens_alloc: Option<(u32, Vec<(u32, u32)>)>,
    /// Fresh cabi_realloc'd guest buffer for the NEXT input-stream read (`ptr`, `size`).
    /// Set by the engine per-read (the guest owns and frees the returned `list<u8>`, so a
    /// reused buffer would be use-after-free). Consumed by blocking-read/read. SR-38.
    #[cfg(feature = "std")]
    read_buf_alloc: Option<(u32, u32)>,
    /// Per input-stream read position, so sequential blocking-read calls advance and
    /// terminate at EOF (return stream-error::closed) instead of re-reading from 0. SR-38.
    #[cfg(feature = "std")]
    stream_offsets: HashMap<u32, usize>,
    /// The file handle backing each input-stream created by `read-via-stream`: a
    /// duplicate of the descriptor's kept `open-at` handle, so stream reads never
    /// reopen by path (TOCTOU). Kept apart from `fd_table` so a stream handle is never
    /// mistaken for a file descriptor by `descriptor.read`/`write`.
    #[cfg(feature = "std")]
    stream_files: HashMap<u32, std::fs::File>,
    /// Open directory-entry-streams: a snapshot of (descriptor-type, name) entries plus a
    /// cursor, keyed by the stream handle from read-directory. read-directory-entry walks
    /// the cursor and returns none at the end. SR-39.
    #[cfg(feature = "std")]
    dir_streams: HashMap<u32, (Vec<(u8, String)>, usize)>,
    /// Live `wasi:io/poll` pollable resources, keyed by their guest handle (SR-91).
    #[cfg(feature = "std")]
    pollables: HashMap<u32, Pollable>,
    /// Next pollable handle to issue (handles are never reused).
    #[cfg(feature = "std")]
    next_pollable: u32,
}

/// The kind of a `wasi:io/poll` pollable resource (SR-91).
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pollable {
    /// A monotonic-clock subscription: ready once `Instant::now()` reaches the deadline.
    Deadline(std::time::Instant),
    /// A stream subscription. Output streams (stdout/stderr and file writes) and
    /// file-backed input streams never block in kiln: writes are synchronous and
    /// `check-write` always reports capacity, reads are synchronous. So the
    /// pollable is ready from creation, and `block` returns immediately.
    AlwaysReady,
}

#[cfg(feature = "std")]
impl Pollable {
    /// The deadline to wait for, or `None` if ready now.
    fn pending_until(&self, now: std::time::Instant) -> Option<std::time::Instant> {
        match self {
            Self::Deadline(d) if *d > now => Some(*d),
            Self::Deadline(_) | Self::AlwaysReady => None,
        }
    }
}

impl WasiDispatcher {
    /// Create a new WASI dispatcher with the given capabilities
    ///
    /// # Errors
    ///
    /// Returns an error if the resource manager cannot be initialized.
    pub fn new(capabilities: WasiCapabilities) -> Result<Self> {
        #[cfg(feature = "std")]
        let mut fd_table = HashMap::new();

        // Pre-populate fd_table with stdin/stdout/stderr
        #[cfg(feature = "std")]
        {
            fd_table.insert(
                0,
                FileDescriptorEntry {
                    fd_type: FileDescriptorType::Stdin,
                    read: true,
                    write: false,
                    file: None,
                },
            );
            fd_table.insert(
                1,
                FileDescriptorEntry {
                    fd_type: FileDescriptorType::Stdout,
                    read: false,
                    write: true,
                    file: None,
                },
            );
            fd_table.insert(
                2,
                FileDescriptorEntry {
                    fd_type: FileDescriptorType::Stderr,
                    read: false,
                    write: true,
                    file: None,
                },
            );
        }

        Ok(Self {
            capabilities,
            resource_manager: WasiResourceManager::new()?,
            args: Vec::new(),
            env_vars: Vec::new(),
            args_alloc: None,
            #[cfg(feature = "std")]
            fd_table,
            #[cfg(feature = "std")]
            preopens: Vec::new(),
            #[cfg(feature = "std")]
            preopens_alloc: None,
            #[cfg(feature = "std")]
            read_buf_alloc: None,
            #[cfg(feature = "std")]
            stream_offsets: HashMap::new(),
            #[cfg(feature = "std")]
            stream_files: HashMap::new(),
            #[cfg(feature = "std")]
            dir_streams: HashMap::new(),
            #[cfg(feature = "std")]
            pollables: HashMap::new(),
            #[cfg(feature = "std")]
            next_pollable: 1,
        })
    }

    /// Register a pollable and return its guest handle.
    #[cfg(feature = "std")]
    fn insert_pollable(&mut self, kind: Pollable) -> Result<u32> {
        let handle = self.next_pollable;
        self.next_pollable = handle
            .checked_add(1)
            .ok_or_else(|| Error::runtime_error("pollable handle space exhausted"))?;
        self.pollables.insert(handle, kind);
        Ok(handle)
    }

    /// Look up a live pollable; an unknown handle is an error.
    #[cfg(feature = "std")]
    fn pollable(&self, handle: u32) -> Result<Pollable> {
        self.pollables.get(&handle).copied().ok_or_else(|| {
            Error::wasi_invalid_argument("pollable handle does not name a live pollable")
        })
    }

    /// Sleep until `deadline` has passed (no busy loop).
    #[cfg(feature = "std")]
    fn sleep_until(deadline: std::time::Instant) {
        while let Some(rem) = deadline.checked_duration_since(std::time::Instant::now()) {
            if rem.is_zero() {
                break;
            }
            std::thread::sleep(rem);
        }
    }

    /// Create a dispatcher with default (full) capabilities
    ///
    /// Automatically populates args and environment from global state
    /// (set via `set_global_wasi_args` and `set_global_wasi_env`).
    ///
    /// # Errors
    ///
    /// Returns an error if default capabilities cannot be created or the
    /// resource manager fails to initialize.
    pub fn with_defaults() -> Result<Self> {
        let mut dispatcher = Self::new(WasiCapabilities::system_utility()?)?;
        // Populate from global state
        #[cfg(feature = "std")]
        {
            dispatcher.args = get_global_wasi_args()?;
            dispatcher.env_vars = get_global_wasi_env()?;
        }
        Ok(dispatcher)
    }

    /// Create a dispatcher with arguments
    ///
    /// # Errors
    ///
    /// Returns an error if the resource manager cannot be initialized.
    pub fn with_args(capabilities: WasiCapabilities, args: Vec<String>) -> Result<Self> {
        let mut dispatcher = Self::new(capabilities)?;
        dispatcher.args = args;
        Ok(dispatcher)
    }

    /// Create a dispatcher with arguments and environment variables
    ///
    /// # Errors
    ///
    /// Returns an error if the resource manager cannot be initialized.
    pub fn with_args_and_env(
        capabilities: WasiCapabilities,
        args: Vec<String>,
        env_vars: Vec<(String, String)>,
    ) -> Result<Self> {
        let mut dispatcher = Self::new(capabilities)?;
        dispatcher.args = args;
        dispatcher.env_vars = env_vars;
        Ok(dispatcher)
    }

    /// Add a pre-opened directory to the WASI sandbox
    ///
    /// Returns the file descriptor handle for the directory
    ///
    /// # Errors
    ///
    /// Returns an error if the directory handle cannot be created in the
    /// resource manager.
    #[cfg(feature = "std")]
    pub fn add_preopen(&mut self, path: impl Into<PathBuf>) -> Result<u32> {
        let path = path.into();
        let path_str = path.to_string_lossy();

        // Use resource manager to allocate handle (Preview2 style)
        let handle = self.resource_manager.create_directory_handle(&path_str)?;

        // Also add to fd_table for filesystem operations
        self.fd_table.insert(
            handle,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::PreopenDirectory(path.clone()),
                read: self.capabilities.filesystem.read_access,
                write: self.capabilities.filesystem.write_access,
                file: None,
            },
        );

        // Add to preopens list
        self.preopens.push((handle, path));

        Ok(handle)
    }

    /// Register a filesystem path the guest is permitted to access, restricting
    /// access to this subtree in addition to preopen containment (SR-50).
    ///
    /// The path is canonicalized so the allow-list compares against the
    /// OS-resolved real paths the sandbox gate produces (see
    /// `Self::resolve_and_authorize`). A non-empty allow-list is enforced by
    /// every filesystem handler; an empty allow-list imposes no restriction
    /// beyond preopen containment.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be resolved (it must exist) or if the
    /// capability path store is full.
    #[cfg(feature = "std")]
    pub fn add_allowed_path(&mut self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let canonical = std::fs::canonicalize(path.as_ref())
            .map_err(|_| Error::wasi_invalid_argument("Allowed path cannot be resolved"))?;
        self.capabilities.filesystem.add_allowed_path(&canonical.to_string_lossy())
    }

    /// The single sandbox gate every filesystem handler routes through before
    /// touching the host filesystem. Resolves the guest-supplied `guest_path`
    /// against the preopen `base` and authorizes it:
    ///
    /// 1. **Lexical containment** (existence-independent): rejects any `..`
    ///    traversal or absolute path that `Path::join` let replace the base, so
    ///    the check also holds for a not-yet-created target (#392, SR-49).
    /// 2. **Symlink-safe containment** on the OS-resolved real path: canonicalizes
    ///    the preopen and the target and requires the real target to stay inside
    ///    the real preopen, so a symlink inside the preopen that points outside is
    ///    rejected (SR-51). For a not-yet-existing target the parent directory is
    ///    canonicalized and re-checked, then the final component is appended
    ///    literally (never following a symlink out on the final component).
    /// 3. **Allow-list** (only when a non-empty allow-list is configured): the
    ///    resolved absolute path must fall under an allowed path (SR-50).
    ///
    /// The op-specific capability flag (read/write/directory/metadata) is checked
    /// by the caller before this gate. On any failure a `wasi_permission_denied`
    /// error is returned, matching the deny behavior of the `dispatch` path and
    /// wasmtime; the resolved absolute path to use for the host operation is
    /// returned on success.
    ///
    /// # Errors
    ///
    /// Returns `wasi_permission_denied` if the path escapes the preopen (lexically
    /// or via symlink) or is not permitted by a configured allow-list.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn resolve_and_authorize(
        &self,
        base: &std::path::Path,
        guest_path: &str,
    ) -> Result<std::path::PathBuf> {
        let joined = base.join(guest_path);

        // (1) Lexical containment — holds for missing targets too.
        if !is_within_sandbox(base, &joined) {
            return Err(Error::wasi_permission_denied("Path escapes sandbox"));
        }

        // (2) Symlink-safe containment on the OS-resolved real path.
        let real_base = std::fs::canonicalize(base)
            .map_err(|_| Error::wasi_permission_denied("Preopen base cannot be resolved"))?;
        let resolved = match std::fs::symlink_metadata(&joined) {
            // Final component exists (possibly a symlink). Fully resolve it,
            // following symlinks, and require the real path stays inside. A
            // symlink pointing outside resolves outside `real_base` → denied; a
            // dangling symlink fails to canonicalize → denied (fail loud).
            Ok(_) => std::fs::canonicalize(&joined)
                .map_err(|_| Error::wasi_permission_denied("Path escapes sandbox via symlink"))?,
            // Target does not exist at all — resolve the parent (which must exist
            // and stay inside) and append the final component literally, so a
            // create/open-create cannot escape and no symlink is followed on the
            // final component.
            Err(_) => {
                let parent = joined
                    .parent()
                    .ok_or_else(|| Error::wasi_permission_denied("Path has no parent"))?;
                let file_name = joined
                    .file_name()
                    .ok_or_else(|| Error::wasi_permission_denied("Path has no final component"))?;
                let real_parent = std::fs::canonicalize(parent).map_err(|_| {
                    Error::wasi_permission_denied("Parent directory cannot be resolved")
                })?;
                if !real_parent.starts_with(&real_base) {
                    return Err(Error::wasi_permission_denied(
                        "Path escapes sandbox via symlink",
                    ));
                }
                real_parent.join(file_name)
            },
        };
        if !resolved.starts_with(&real_base) {
            return Err(Error::wasi_permission_denied(
                "Path escapes sandbox via symlink",
            ));
        }

        // (3) Allow-list — only enforced when a non-empty allow-list is configured.
        if self.capabilities.filesystem.has_allowed_paths()
            && !self.capabilities.filesystem.is_path_allowed(&resolved.to_string_lossy())
        {
            return Err(Error::wasi_permission_denied("Path not in allow-list"));
        }

        Ok(resolved)
    }

    /// Sandbox gate for calls that act on the directory entry itself (unlink,
    /// remove-directory, rename, readlink, symlink/create targets): same three
    /// checks as [`Self::resolve_and_authorize`] (lexical containment, symlink-safe
    /// containment, allow-list), but a symlink in the FINAL component is never
    /// followed. The parent directory is resolved and must stay inside the real
    /// preopen, and the final component is appended literally, so the host
    /// operation acts on the link itself rather than on whatever it points at
    /// (unlinking `alias` must not delete `alias`'s target).
    ///
    /// # Errors
    ///
    /// Returns `wasi_permission_denied` if the path escapes the preopen (lexically
    /// or via a symlinked parent), names the preopen itself, or is not permitted by
    /// a configured allow-list.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn resolve_and_authorize_nofollow(
        &self,
        base: &std::path::Path,
        guest_path: &str,
    ) -> Result<std::path::PathBuf> {
        let joined = base.join(guest_path);

        // (1) Lexical containment — holds for missing targets too.
        if !is_within_sandbox(base, &joined) {
            return Err(Error::wasi_permission_denied("Path escapes sandbox"));
        }

        // (2) Symlink-safe containment of the parent directory.
        let real_base = std::fs::canonicalize(base)
            .map_err(|_| Error::wasi_permission_denied("Preopen base cannot be resolved"))?;
        let parent = joined
            .parent()
            .ok_or_else(|| Error::wasi_permission_denied("Path has no parent"))?;
        let file_name = joined
            .file_name()
            .ok_or_else(|| Error::wasi_permission_denied("Path has no final component"))?;
        let real_parent = std::fs::canonicalize(parent)
            .map_err(|_| Error::wasi_permission_denied("Parent directory cannot be resolved"))?;
        if !real_parent.starts_with(&real_base) {
            return Err(Error::wasi_permission_denied(
                "Path escapes sandbox via symlink",
            ));
        }
        let resolved = real_parent.join(file_name);
        if resolved == real_base {
            return Err(Error::wasi_permission_denied(
                "Operation on the preopen directory itself is not permitted",
            ));
        }

        // (3) Allow-list — only enforced when a non-empty allow-list is configured.
        if self.capabilities.filesystem.has_allowed_paths()
            && !self.capabilities.filesystem.is_path_allowed(&resolved.to_string_lossy())
        {
            return Err(Error::wasi_permission_denied("Path not in allow-list"));
        }

        Ok(resolved)
    }

    /// SR-96: a guest path ending in `/` or `/.` names a directory. Both
    /// resolvers rebuild the path from its components, which drops the trailing
    /// slash, so a call on `file/` would otherwise act on the regular file
    /// `file`. Callers whose POSIX counterpart reports ENOTDIR for this (open,
    /// stat, unlink, rmdir, rename) check this after the sandbox gate and return
    /// `not-directory`. Only a regular file in the final position is refused; a
    /// symlink keeps the resolver's behaviour, and a missing entry is left to the
    /// host call to report.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn trailing_slash_on_file(guest_path: &str, resolved: &std::path::Path) -> bool {
        (guest_path.ends_with('/') || guest_path.ends_with("/."))
            && matches!(std::fs::symlink_metadata(resolved), Ok(meta) if meta.is_file())
    }

    /// The preopen directory `fd` names, if it is one. Path-based descriptor
    /// calls resolve relative to this directory only.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn preopen_base(&self, fd: u32) -> Option<PathBuf> {
        self.preopens
            .iter()
            .find(|(preopen_fd, _)| *preopen_fd == fd)
            .map(|(_, p)| p.clone())
            .or_else(|| {
                self.fd_table.get(&fd).and_then(|e| match &e.fd_type {
                    FileDescriptorType::PreopenDirectory(p) => Some(p.clone()),
                    _ => None,
                })
            })
    }

    /// Directory-mutating calls need the filesystem write capability.
    ///
    /// # Errors
    ///
    /// Returns `wasi_permission_denied` when write access is not granted.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn require_fs_write(&self) -> Result<()> {
        if self.capabilities.filesystem.write_access {
            Ok(())
        } else {
            Err(Error::wasi_permission_denied(
                "Filesystem write access denied",
            ))
        }
    }

    /// `i32` core argument at `idx`, as the unsigned canonical-ABI value.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn core_u32(
        args: &[kiln_foundation::values::Value],
        idx: usize,
        what: &'static str,
    ) -> Result<u32> {
        match args.get(idx) {
            Some(kiln_foundation::values::Value::I32(v)) => Ok(*v as u32),
            _ => Err(Error::wasi_invalid_argument(what)),
        }
    }

    /// The address `offset` bytes into the return area at `retptr`. A guest-supplied
    /// `retptr` near the top of a u32 address space must fail, not wrap to the start
    /// of memory.
    fn retptr_at(retptr: u32, offset: u32) -> Result<u32> {
        retptr.checked_add(offset).ok_or_else(|| {
            Error::memory_out_of_bounds("Return area offset overflows the address space")
        })
    }

    /// Write a guest-owned `list<u8>`/`string` payload into the engine-supplied
    /// cabi_realloc'd buffer and return its `(ptr, len)` for the return-area header.
    ///
    /// The guest owns and frees the returned payload, so it must never be placed next to
    /// the return area. Fails loud when the engine supplied no buffer or the payload does
    /// not fit (a payload is refused, never truncated). SR-38.
    #[cfg(feature = "std")]
    fn write_owned_return(
        mem: &dyn MemoryAccessor,
        read_buf: Option<(u32, u32)>,
        what: &'static str,
        payload: &[u8],
    ) -> Result<(u32, u32)> {
        let (buf_ptr, buf_size) = read_buf.ok_or_else(|| Error::runtime_error(what))?;
        if payload.len() > buf_size as usize {
            return Err(Error::wasi_resource_limit(
                "returned payload exceeds the cabi_realloc'd buffer",
            ));
        }
        let len = u32::try_from(payload.len())
            .map_err(|_| Error::wasi_resource_limit("returned payload exceeds u32"))?;
        mem.write_bytes(buf_ptr, payload)?;
        Ok((buf_ptr, len))
    }

    /// Generate `len` random bytes into the engine-supplied buffer and write the
    /// `(ptr, len)` list header at `retptr`. A request over 1 MiB is refused. SR-38.
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    fn write_random_list(
        mem: Option<&dyn MemoryAccessor>,
        read_buf: Option<(u32, u32)>,
        what: &'static str,
        len: usize,
        retptr: u32,
    ) -> Result<()> {
        use kiln_platform::random::PlatformRandom;

        /// WASI random requests above this are refused (the engine buffer is capped here).
        const MAX_RANDOM_BYTES: usize = 1024 * 1024;
        if len > MAX_RANDOM_BYTES {
            return Err(Error::wasi_resource_limit(
                "random bytes request exceeds the 1 MiB limit",
            ));
        }
        let mut bytes = vec![0u8; len];
        PlatformRandom::get_secure_bytes(&mut bytes)
            .map_err(|_| Error::wasi_capability_unavailable("Random not available"))?;
        let mem = mem.ok_or_else(|| Error::wasi_invalid_argument("random bytes: no memory"))?;
        let (ptr, len) = Self::write_owned_return(mem, read_buf, what, &bytes)?;
        mem.write_bytes(retptr, &ptr.to_le_bytes())?;
        mem.write_bytes(Self::retptr_at(retptr, 4)?, &len.to_le_bytes())?;
        Ok(())
    }

    /// `filesize` (u64) core argument at `idx`.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn core_u64(
        args: &[kiln_foundation::values::Value],
        idx: usize,
        what: &'static str,
    ) -> Result<u64> {
        match args.get(idx) {
            Some(kiln_foundation::values::Value::I64(v)) => Ok(*v as u64),
            _ => Err(Error::wasi_invalid_argument(what)),
        }
    }

    /// Read a guest `string`/`list<u8>` of `len` bytes at `ptr`. The length is
    /// checked against the guest memory size before anything is allocated.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn read_guest_bytes(mem: &dyn MemoryAccessor, ptr: u32, len: u32) -> Result<Vec<u8>> {
        if len as usize > mem.size() {
            return Err(Error::memory_out_of_bounds(
                "Guest buffer exceeds linear memory",
            ));
        }
        let mut buf = vec![0u8; len as usize];
        mem.read_bytes(ptr, &mut buf)?;
        Ok(buf)
    }

    /// Read a guest path string. Invalid UTF-8 is an error rather than being
    /// lossily rewritten into a different path.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn read_guest_path(mem: &dyn MemoryAccessor, ptr: u32, len: u32) -> Result<String> {
        String::from_utf8(Self::read_guest_bytes(mem, ptr, len)?)
            .map_err(|_| Error::wasi_invalid_encoding("Path is not valid UTF-8"))
    }

    /// Map a host I/O failure to the `wasi:filesystem` `error-code` enum index.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn fs_error_code(err: &std::io::Error) -> i32 {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::PermissionDenied => fs_error::ACCESS,
            ErrorKind::AlreadyExists => fs_error::EXIST,
            ErrorKind::InvalidInput => fs_error::INVALID,
            ErrorKind::IsADirectory => fs_error::IS_DIRECTORY,
            ErrorKind::NotFound => fs_error::NO_ENTRY,
            ErrorKind::NotADirectory => fs_error::NOT_DIRECTORY,
            ErrorKind::DirectoryNotEmpty => fs_error::NOT_EMPTY,
            _ => fs_error::IO,
        }
    }

    /// Flatten a host `SystemTime` into the core values of a canonical
    /// `option<datetime>`: `[i32 0]` for none, `[i32 1, i64 seconds, i32 nanos]` for
    /// some. A time the host cannot provide, or one before the Unix epoch (not
    /// representable as the unsigned `datetime.seconds`), is none, never zero.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn datetime_option(
        time: std::io::Result<std::time::SystemTime>,
    ) -> Vec<kiln_foundation::values::Value> {
        use kiln_foundation::values::Value as CoreValue;
        match time.ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
            Some(d) => vec![
                CoreValue::I32(1),
                CoreValue::I64(d.as_secs() as i64),
                CoreValue::I32(d.subsec_nanos() as i32),
            ],
            None => vec![CoreValue::I32(0)],
        }
    }

    /// The status-change time (`ctime`) as an `option<datetime>`; none when the
    /// host reports a pre-epoch or out-of-range value.
    #[cfg(all(feature = "wasi-filesystem", feature = "std", unix))]
    fn status_change_option(meta: &std::fs::Metadata) -> Vec<kiln_foundation::values::Value> {
        use std::os::unix::fs::MetadataExt;
        let secs = u64::try_from(meta.ctime()).ok();
        let nanos = u32::try_from(meta.ctime_nsec()).ok().filter(|n| *n < 1_000_000_000);
        match secs.zip(nanos) {
            Some((s, n)) => {
                Self::datetime_option(Ok(std::time::UNIX_EPOCH + std::time::Duration::new(s, n)))
            },
            None => Self::datetime_option(Err(std::io::ErrorKind::InvalidData.into())),
        }
    }

    /// Core values of an ok `result<descriptor-stat, error-code>` built from host
    /// metadata, shared by `stat` and `stat-at`: discriminant, type, link-count,
    /// size, then the access, modification and status-change `option<datetime>`.
    #[cfg(all(feature = "wasi-filesystem", feature = "std", unix))]
    fn stat_core_values(meta: &std::fs::Metadata) -> Vec<kiln_foundation::values::Value> {
        use kiln_foundation::values::Value as CoreValue;
        use std::os::unix::fs::MetadataExt;
        let dtype: i32 = if meta.is_dir() { 3 } else { 6 };
        let mut v = vec![
            CoreValue::I32(0),
            CoreValue::I32(dtype),
            CoreValue::I64(meta.nlink() as i64),
            CoreValue::I64(meta.len() as i64),
        ];
        v.extend(Self::datetime_option(meta.accessed()));
        v.extend(Self::datetime_option(meta.modified()));
        v.extend(Self::status_change_option(meta));
        v
    }

    /// There is no stable std API for the link count off unix, and none may be
    /// invented: `stat` and `stat-at` report `unsupported` there.
    /// Non-unix targets (Windows): stable std has no link-count API, so
    /// link-count is reported as 1, a documented limit (SR-94, maintainer
    /// decision 2026-11-12): a hard-linked file shows 1 there. There is no
    /// status-change time (creation time is a different value), so it is none.
    #[cfg(all(feature = "wasi-filesystem", feature = "std", not(unix)))]
    fn stat_core_values(meta: &std::fs::Metadata) -> Vec<kiln_foundation::values::Value> {
        use kiln_foundation::values::Value as CoreValue;
        let dtype: i32 = if meta.is_dir() { 3 } else { 6 };
        let mut v = vec![
            CoreValue::I32(0),
            CoreValue::I32(dtype),
            CoreValue::I64(1),
            CoreValue::I64(meta.len() as i64),
        ];
        v.extend(Self::datetime_option(meta.accessed()));
        v.extend(Self::datetime_option(meta.modified()));
        v.push(CoreValue::I32(0));
        v
    }

    /// Open `path` (already authorized by [`Self::resolve_and_authorize`] against the
    /// preopen `base`) and return the handle the descriptor will keep. This is the one
    /// helper every file open goes through. The file is opened first and containment
    /// is then verified on the opened handle ([`Self::verify_handle_contained`]), so a
    /// path component swapped for a symlink after authorization is caught. The caller
    /// has already checked the path is a regular file, so a FIFO swapped in later is
    /// the only way to block here; the post-open check refuses anything that is not a
    /// regular file. After this the descriptor operates on the handle, never on the
    /// path again.
    ///
    /// The outer `Err` is a sandbox violation; the inner `Err` is a host I/O error.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn open_authorized_file(
        base: &std::path::Path,
        path: &std::path::Path,
        read: bool,
        write: bool,
    ) -> Result<std::result::Result<std::fs::File, std::io::Error>> {
        // Test-only seam: lets a test race a directory swap between authorization
        // and the open.
        #[cfg(test)]
        BEFORE_OPEN_HOOK.with(|h| {
            if let Some(hook) = h.borrow_mut().take() {
                hook();
            }
        });
        let file = match std::fs::OpenOptions::new().read(read || !write).write(write).open(path) {
            Ok(f) => f,
            Err(e) => return Ok(Err(e)),
        };
        Self::verify_handle_contained(base, path, &file)?;
        Ok(Ok(file))
    }

    /// Containment check on the OPENED handle (not on the path, which an attacker can
    /// swap): any directory component swapped for a symlink between authorization and
    /// open makes the handle point outside the preopen, and this refuses it.
    ///
    /// * Linux: the handle's real path is read from `/proc/self/fd/<fd>` and must lie
    ///   under the canonical preopen base.
    /// * Other unix (macOS): `fcntl(F_GETPATH)` would need `libc` (not a dependency of
    ///   this `#![deny(unsafe_code)]` crate), so identity is checked instead: the path
    ///   is re-resolved, must lie under the canonical base, and must name the very
    ///   inode (`dev`, `ino`) the handle refers to. A handle opened through a swapped
    ///   directory refers to an outside inode, so it can never match an inside path.
    /// * Elsewhere: fails loud as unsupported rather than skipping the check.
    ///
    /// A handle that is not a regular file is refused (FIFOs, devices).
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn verify_handle_contained(
        base: &std::path::Path,
        path: &std::path::Path,
        file: &std::fs::File,
    ) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let refused = || Error::wasi_permission_denied("Opened handle escapes the preopen");
            let opened = file
                .metadata()
                .map_err(|_| Error::wasi_permission_denied("Opened handle cannot be inspected"))?;
            if !opened.is_file() {
                return Err(refused());
            }
            let real_base = std::fs::canonicalize(base)
                .map_err(|_| Error::wasi_permission_denied("Preopen base cannot be resolved"))?;
            #[cfg(target_os = "linux")]
            {
                use std::os::fd::AsRawFd as _;
                let real = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
                    .map_err(|_| {
                        Error::wasi_permission_denied("Opened handle cannot be resolved")
                    })?;
                if !real.starts_with(&real_base) {
                    return Err(refused());
                }
            }
            let real = std::fs::canonicalize(path).map_err(|_| refused())?;
            if !real.starts_with(&real_base) {
                return Err(refused());
            }
            let on_disk = std::fs::metadata(&real).map_err(|_| refused())?;
            if on_disk.dev() != opened.dev() || on_disk.ino() != opened.ino() {
                return Err(refused());
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = (base, path, file);
            Err(Error::wasi_unsupported_operation(
                "Handle containment check is not supported on this platform",
            ))
        }
    }

    /// Get the list of pre-opened directories
    #[cfg(feature = "std")]
    pub fn get_preopens(&self) -> &[(u32, PathBuf)] {
        &self.preopens
    }

    /// Set the command-line arguments
    pub fn set_args(&mut self, args: Vec<String>) {
        self.args = args;
    }

    /// Set the environment variables
    pub fn set_env(&mut self, env_vars: Vec<(String, String)>) {
        self.env_vars = env_vars;
    }

    /// Set pre-allocated memory for arguments (from `cabi_realloc`)
    ///
    /// This is set by the engine after calling `cabi_realloc` to allocate
    /// properly owned memory for argument strings.
    ///
    /// IMPORTANT: Each string is allocated SEPARATELY to ensure proper
    /// allocator metadata. This prevents crashes when the component frees
    /// individual strings.
    ///
    /// # Arguments
    /// * `list_ptr` - Pointer to the (ptr, len) array for list elements
    /// * `string_ptrs` - Vector of (ptr, len) for each individually allocated string
    pub fn set_args_alloc(&mut self, list_ptr: u32, string_ptrs: Vec<(u32, u32)>) {
        self.args_alloc = Some((list_ptr, string_ptrs));
    }

    /// Clear pre-allocated memory for arguments
    pub fn clear_args_alloc(&mut self) {
        self.args_alloc = None;
    }

    /// Get pre-allocated memory pointers for arguments
    pub fn get_args_alloc(&self) -> Option<&(u32, Vec<(u32, u32)>)> {
        self.args_alloc.as_ref()
    }

    /// Get the capabilities for this dispatcher
    pub fn capabilities(&self) -> &WasiCapabilities {
        &self.capabilities
    }

    /// Get the command-line arguments
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Strip version suffix from interface name
    /// e.g., "wasi:clocks/wall-clock@0.2.4" -> "wasi:clocks/wall-clock"
    fn strip_version(interface: &str) -> &str {
        if let Some(idx) = interface.find('@') { &interface[..idx] } else { interface }
    }

    /// Dispatch with raw core WASM values and memory access
    ///
    /// This interface is for cases where the canonical ABI lifting hasn't
    /// happened yet - values are raw i32/i64 and complex types are ptr+len
    /// in memory.
    ///
    /// # Arguments
    /// * `interface` - WASI interface name (e.g., "wasi:cli/stdout@0.2.4")
    /// * `function` - Function name (e.g., "get-stdout")
    /// * `args` - Raw values from the WASM stack
    /// * `memory` - Linear memory accessor for reading/writing complex types
    ///   Uses interior mutability for thread-safe writes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The interface/function combination is unknown or unsupported
    /// - Arguments are missing, invalid, or have wrong types
    /// - Memory access fails (when reading/writing complex types)
    /// - The underlying WASI operation fails
    // Large match is intentional - WASI core interface dispatch for all functions.
    #[allow(clippy::too_many_lines)]
    #[cfg(feature = "std")]
    pub fn dispatch_core(
        &mut self,
        interface: &str,
        function: &str,
        args: &[kiln_foundation::values::Value],
        memory: Option<&dyn MemoryAccessor>,
    ) -> Result<Vec<kiln_foundation::values::Value>> {
        use kiln_foundation::values::Value as CoreValue;

        #[cfg(feature = "tracing")]
        let base_interface = Self::strip_version(interface);

        // Per-call trace; emitted only when the `tracing` feature is built in and
        // RUST_LOG enables it. Never written directly to stderr (SR-67).
        #[cfg(feature = "tracing")]
        trace!(interface = %base_interface, function = %function, args = ?args, "WASI call");

        // The served-imports table is the only place an import name is
        // recognised; instantiation refuses imports it does not list (SR-65).
        let Some(served) = served_wasi_fn(interface, function) else {
            #[cfg(feature = "tracing")]
            warn!(interface = %base_interface, function = %function, "unknown WASI function in dispatch_core");

            return Err(Error::runtime_not_implemented("Unknown WASI function"));
        };

        match served {
            // Simple functions that don't need memory
            WasiFn::GetStdout => {
                // Preview2: dynamically allocate output-stream resource
                let handle = self.resource_manager.create_output_stream("stdout")?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            WasiFn::GetStderr => {
                // Preview2: dynamically allocate output-stream resource
                let handle = self.resource_manager.create_output_stream("stderr")?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            WasiFn::GetStdin => {
                // Preview2: dynamically allocate input-stream resource
                let handle = self.resource_manager.create_input_stream("stdin")?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            // Environment functions
            WasiFn::GetArguments => {
                // Canonical ABI for list<string> return:
                // - First argument is the RETURN POINTER where we write (list_ptr, list_len)
                // - We need to allocate space for the list elements and string data
                // - Write element pointers array at one location
                // - Write string data at another location
                // - Write (element_array_ptr, count) to the return pointer

                // The engine allocated this call's lists (cabi_realloc); the guest owns and
                // may free them, so they are consumed here and never handed out twice.
                let args_alloc = self.args_alloc.take();

                // Get args - prefer local args, fall back to global args
                #[cfg(feature = "std")]
                let args_to_use: Vec<String> =
                    if self.args.is_empty() { get_global_wasi_args()? } else { self.args.clone() };
                #[cfg(not(feature = "std"))]
                let args_to_use = self.args.clone();

                // Extract return pointer from args (first argument in canonical ABI lowering)
                let retptr = match args.first() {
                    Some(CoreValue::I32(ptr)) => *ptr as u32,
                    _ => {
                        #[cfg(feature = "tracing")]
                        warn!("get-arguments: no return pointer provided");
                        return Ok(vec![]);
                    },
                };

                #[cfg(feature = "tracing")]
                trace!(
                    arg_count = args_to_use.len(),
                    args = ?args_to_use,
                    retptr = format_args!("0x{:x}", retptr),
                    args_alloc = ?args_alloc,
                    "get-arguments dispatch"
                );

                if let Some(mem) = memory {
                    #[cfg(feature = "tracing")]
                    trace!(memory_size = mem.size(), "memory available");

                    if args_to_use.is_empty() {
                        // Empty list: write (0, 0) to return pointer
                        mem.write_bytes(retptr, &0u32.to_le_bytes())?;
                        mem.write_bytes(Self::retptr_at(retptr, 4)?, &0u32.to_le_bytes())?;
                        return Ok(vec![]);
                    }

                    // Use pre-allocated memory from cabi_realloc - this is required
                    // IMPORTANT: Each string has its own allocation to ensure proper
                    // allocator metadata (prevents crash when component frees strings)
                    let (list_ptr, string_ptrs) =
                        if let Some((alloc_list, ref alloc_strings)) = args_alloc {
                            #[cfg(feature = "tracing")]
                            trace!(
                                list_ptr = format_args!("0x{:x}", alloc_list),
                                num_strings = alloc_strings.len(),
                                "using cabi_realloc memory with SEPARATE string allocations"
                            );
                            (alloc_list, alloc_strings.clone())
                        } else {
                            // NO FALLBACK - cabi_realloc must have been called
                            // Per CLAUDE.md: "FAIL LOUD AND EARLY" - no stack-relative hacks
                            #[cfg(feature = "tracing")]
                            warn!("args_alloc not set - cabi_realloc was not called");
                            return Err(kiln_error::Error::runtime_error(
                                "get-arguments requires cabi_realloc - allocation not performed",
                            ));
                        };

                    // Verify we have the right number of string allocations
                    if string_ptrs.len() != args_to_use.len() {
                        #[cfg(feature = "tracing")]
                        warn!(
                            expected = args_to_use.len(),
                            actual = string_ptrs.len(),
                            "string allocation count mismatch"
                        );
                        return Err(kiln_error::Error::runtime_error(
                            "string allocation count mismatch",
                        ));
                    }

                    // Write each string to its INDIVIDUALLY allocated location
                    // This ensures each string has proper allocator metadata
                    for (i, (arg, (string_ptr, expected_len))) in
                        args_to_use.iter().zip(string_ptrs.iter()).enumerate()
                    {
                        let bytes = arg.as_bytes();
                        let len = bytes.len() as u32;

                        // Verify allocation matches
                        if len != *expected_len {
                            #[cfg(feature = "tracing")]
                            warn!(
                                index = i,
                                expected_len = *expected_len,
                                actual_len = len,
                                "string length mismatch"
                            );
                        }

                        // An allocation that does not fit memory is an error, never an
                        // empty list the guest would take for "no arguments".
                        if (*string_ptr as usize + bytes.len()) > mem.size() {
                            #[cfg(feature = "tracing")]
                            warn!("not enough memory for arg data");
                            return Err(Error::memory_out_of_bounds(
                                "get-arguments: an argument allocation lies past the end of memory",
                            ));
                        }

                        // Write string bytes to its individually allocated location
                        mem.write_bytes(*string_ptr, bytes)?;
                        #[cfg(feature = "tracing")]
                        trace!(
                            index = i,
                            arg = %arg,
                            byte_count = bytes.len(),
                            addr = format_args!("0x{:x}", *string_ptr),
                            "wrote string to separate allocation"
                        );

                        // Write (ptr, len) to list array
                        let offset = list_ptr + (i as u32 * 8);
                        mem.write_bytes(offset, &string_ptr.to_le_bytes())?;
                        mem.write_bytes(offset + 4, &len.to_le_bytes())?;
                        #[cfg(feature = "tracing")]
                        trace!(
                            index = i,
                            ptr = format_args!("0x{:x}", string_ptr),
                            len = len,
                            "list entry written"
                        );
                    }

                    // Write (list_ptr, count) to the return pointer
                    mem.write_bytes(retptr, &list_ptr.to_le_bytes())?;
                    mem.write_bytes(
                        Self::retptr_at(retptr, 4)?,
                        &(args_to_use.len() as u32).to_le_bytes(),
                    )?;

                    #[cfg(feature = "tracing")]
                    trace!(
                        retptr = format_args!("0x{:x}", retptr),
                        list_ptr = format_args!("0x{:x}", list_ptr),
                        count = args_to_use.len(),
                        "wrote to retptr"
                    );

                    // Canonical ABI with return pointer returns void (no return values)
                    Ok(vec![])
                } else {
                    #[cfg(feature = "tracing")]
                    warn!("no memory available for get-arguments");
                    Ok(vec![])
                }
            },

            WasiFn::GetEnvironment => {
                // Similar to get-arguments but for env vars
                // For now, return empty list
                Ok(vec![CoreValue::I32(0), CoreValue::I32(0)])
            },

            // ================================================================
            // wasi:cli/terminal-* - Terminal detection interfaces
            // ================================================================
            // C/C++ runtime needs these to determine buffering behavior
            WasiFn::GetTerminalStdin => {
                // option<terminal-input> lowered: write discriminant 0 (None) at retptr
                if let (Some(mem), Some(CoreValue::I32(retptr))) = (memory, args.first()) {
                    mem.write_bytes(*retptr as u32, &0u32.to_le_bytes())?;
                }
                Ok(vec![])
            },

            WasiFn::GetTerminalStdout => {
                // option<terminal-output> lowered: write discriminant 0 (None) at retptr
                if let (Some(mem), Some(CoreValue::I32(retptr))) = (memory, args.first()) {
                    mem.write_bytes(*retptr as u32, &0u32.to_le_bytes())?;
                }
                Ok(vec![])
            },

            WasiFn::GetTerminalStderr => {
                // option<terminal-output> lowered: write discriminant 0 (None) at retptr
                if let (Some(mem), Some(CoreValue::I32(retptr))) = (memory, args.first()) {
                    mem.write_bytes(*retptr as u32, &0u32.to_le_bytes())?;
                }
                Ok(vec![])
            },

            WasiFn::TerminalResourceDrop => {
                // get-terminal-* above always returns none, so no terminal
                // handle is ever issued; dropping one drops an invalid handle,
                // which traps in the Canonical ABI.
                Err(Error::runtime_trap_error(
                    "Dropped a terminal handle that was never issued",
                ))
            },

            // ================================================================
            // wasi:io/poll - Async I/O polling interfaces
            // ================================================================
            WasiFn::Poll => {
                // poll(in: list<borrow<pollable>>) -> list<u32>   (SR-91)
                // Args: list_ptr, list_len, retptr. Returns the indices (input
                // order) of the ready pollables; when none is ready, waits for the
                // earliest deadline first. The result list lives in a fresh
                // cabi_realloc'd guest buffer (read_buf_alloc, sized `4 * len` by the
                // engine) that the guest owns and frees.
                let read_buf = self.read_buf_alloc.take();
                let mem = memory.ok_or_else(|| {
                    Error::runtime_error("poll requires guest memory to read the pollable list")
                })?;
                let (list_ptr, len, retptr) = match (args.first(), args.get(1), args.get(2)) {
                    (Some(CoreValue::I32(p)), Some(CoreValue::I32(l)), Some(CoreValue::I32(r))) => {
                        (*p as u32, *l as u32, *r as u32)
                    },
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "poll expects (list_ptr, list_len, retptr) as i32",
                        ));
                    },
                };
                // The WIT contract: the list must be non-empty (an empty poll would
                // block forever).
                if len == 0 {
                    return Err(Error::wasi_invalid_argument(
                        "poll requires a non-empty pollable list",
                    ));
                }
                let byte_len =
                    (len as usize).checked_mul(4).filter(|b| *b <= mem.size()).ok_or_else(
                        || Error::wasi_invalid_argument("poll list exceeds guest memory"),
                    )?;
                let mut raw = vec![0u8; byte_len];
                mem.read_bytes(list_ptr, &mut raw)?;
                let mut kinds = Vec::with_capacity(len as usize);
                for chunk in raw.chunks_exact(4) {
                    let handle = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    kinds.push(self.pollable(handle)?);
                }

                let mut now = std::time::Instant::now();
                let mut ready: Vec<u32> = Vec::new();
                for (i, k) in kinds.iter().enumerate() {
                    if k.pending_until(now).is_none() {
                        ready.push(i as u32);
                    }
                }
                if ready.is_empty() {
                    // All are deadlines still in the future: sleep until the earliest.
                    let earliest = kinds.iter().filter_map(|k| k.pending_until(now)).min();
                    if let Some(deadline) = earliest {
                        Self::sleep_until(deadline);
                    }
                    now = std::time::Instant::now();
                    for (i, k) in kinds.iter().enumerate() {
                        if k.pending_until(now).is_none() {
                            ready.push(i as u32);
                        }
                    }
                }
                if ready.is_empty() {
                    return Err(Error::runtime_error(
                        "poll woke with no ready pollable after its earliest deadline",
                    ));
                }

                let (buf_ptr, buf_size) = read_buf.ok_or_else(|| {
                    Error::runtime_error(
                        "poll requires a cabi_realloc'd result buffer (read_buf_alloc not set)",
                    )
                })?;
                let need = ready.len() * 4;
                if need > buf_size as usize {
                    return Err(Error::runtime_error(
                        "poll result buffer is smaller than the ready list",
                    ));
                }
                for (i, idx) in ready.iter().enumerate() {
                    mem.write_bytes(buf_ptr + 4 * i as u32, &idx.to_le_bytes())?;
                }
                mem.write_bytes(retptr, &buf_ptr.to_le_bytes())?;
                mem.write_bytes(
                    Self::retptr_at(retptr, 4)?,
                    &(ready.len() as u32).to_le_bytes(),
                )?;
                Ok(vec![])
            },

            WasiFn::PollableReady => {
                // [method]pollable.ready(self: borrow<pollable>) -> bool   (SR-91)
                let handle = match args.first() {
                    Some(CoreValue::I32(h)) => *h as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "pollable.ready expects a pollable handle",
                        ));
                    },
                };
                let ready =
                    self.pollable(handle)?.pending_until(std::time::Instant::now()).is_none();
                Ok(vec![CoreValue::I32(i32::from(ready))])
            },

            WasiFn::PollableBlock => {
                // [method]pollable.block(self: borrow<pollable>)   (SR-91)
                let handle = match args.first() {
                    Some(CoreValue::I32(h)) => *h as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "pollable.block expects a pollable handle",
                        ));
                    },
                };
                if let Some(deadline) =
                    self.pollable(handle)?.pending_until(std::time::Instant::now())
                {
                    Self::sleep_until(deadline);
                }
                Ok(vec![])
            },

            WasiFn::PollableDrop => {
                // [resource-drop]pollable(self: own<pollable>)   (SR-91)
                let handle = match args.first() {
                    Some(CoreValue::I32(h)) => *h as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "pollable drop expects a pollable handle",
                        ));
                    },
                };
                self.pollables.remove(&handle).map(|_| Vec::new()).ok_or_else(|| {
                    Error::wasi_invalid_argument("dropped a pollable handle that is not live")
                })
            },

            // ================================================================
            // wasi:io/streams - Additional stream operations
            // ================================================================
            WasiFn::OutputStreamCheckWrite => {
                // check-write(self) -> result<u64, stream-error>
                // Returns number of bytes that can be written without blocking
                // Return large value to indicate ready for writes
                // Args: handle, retptr
                if let Some(mem) = memory {
                    let retptr = match args.get(1) {
                        Some(CoreValue::I32(p)) => *p as u32,
                        _ => return Ok(vec![CoreValue::I64(65536)]),
                    };
                    // Write success discriminant (0) and capacity
                    mem.write_bytes(retptr, &0u32.to_le_bytes())?;
                    mem.write_bytes(Self::retptr_at(retptr, 8)?, &65536u64.to_le_bytes())?;
                }
                Ok(vec![])
            },

            WasiFn::OutputStreamSubscribe => {
                // subscribe(self) -> pollable: a real table entry. Output streams
                // never block in kiln (see `Pollable::AlwaysReady`).
                let handle = self.insert_pollable(Pollable::AlwaysReady)?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            // Clock functions
            #[cfg(feature = "wasi-clocks")]
            WasiFn::WallClockNow => {
                use kiln_platform::time::PlatformTime;

                if !self.capabilities.clocks.realtime_access {
                    return Err(Error::wasi_permission_denied("Wall clock access denied"));
                }

                let total_ns = PlatformTime::wall_clock_ns()
                    .map_err(|_| Error::wasi_capability_unavailable("Wall clock not available"))?;

                let seconds = total_ns / 1_000_000_000;
                let nanoseconds = (total_ns % 1_000_000_000) as u32;

                // Per Canonical ABI: datetime record is written to retptr in memory
                // datetime = { seconds: u64, nanoseconds: u32 } = 12 bytes
                // The retptr is passed as the first argument
                if let Some(mem) = memory {
                    let retptr = match args.first() {
                        Some(CoreValue::I32(p)) => *p as u32,
                        _ => {
                            // No retptr provided - return flat values for non-indirected ABI
                            #[cfg(feature = "tracing")]
                            trace!("wall-clock::now - no retptr, returning flat values");
                            return Ok(vec![
                                CoreValue::I64(seconds as i64),
                                CoreValue::I32(nanoseconds as i32),
                            ]);
                        },
                    };

                    // Per Canonical ABI: datetime record layout
                    // seconds: u64 (8 bytes) at offset 0
                    // nanoseconds: u32 (4 bytes) at offset 8
                    // Write seconds (u64, 8 bytes) at retptr
                    mem.write_bytes(retptr, &seconds.to_le_bytes())?;
                    // Write nanoseconds as u32 (4 bytes) at retptr + 8
                    mem.write_bytes(Self::retptr_at(retptr, 8)?, &nanoseconds.to_le_bytes())?;

                    #[cfg(feature = "tracing")]
                    trace!(
                        retptr = format_args!("0x{:x}", retptr),
                        seconds = seconds,
                        nanoseconds = nanoseconds,
                        "wall-clock::now wrote datetime"
                    );

                    // Return nothing - datetime was written to memory
                    Ok(vec![])
                } else {
                    // No memory available - return flat values
                    #[cfg(feature = "tracing")]
                    trace!("wall-clock::now - no memory, returning flat values");
                    Ok(vec![
                        CoreValue::I64(seconds as i64),
                        CoreValue::I32(nanoseconds as i32),
                    ])
                }
            },

            #[cfg(feature = "wasi-clocks")]
            WasiFn::WallClockResolution => {
                // 1 nanosecond resolution
                // Per Canonical ABI: datetime record is written to retptr in memory
                if let Some(mem) = memory {
                    let retptr = match args.first() {
                        Some(CoreValue::I32(p)) => *p as u32,
                        _ => {
                            return Ok(vec![
                                CoreValue::I64(0), // seconds
                                CoreValue::I32(1), // nanoseconds
                            ]);
                        },
                    };

                    // Write seconds (0) at retptr
                    mem.write_bytes(retptr, &0u64.to_le_bytes())?;
                    // Write nanoseconds (1) as i64 at retptr + 8
                    mem.write_bytes(Self::retptr_at(retptr, 8)?, &1u64.to_le_bytes())?;
                    Ok(vec![])
                } else {
                    Ok(vec![
                        CoreValue::I64(0), // seconds
                        CoreValue::I32(1), // nanoseconds
                    ])
                }
            },

            #[cfg(feature = "wasi-clocks")]
            WasiFn::MonotonicClockNow => {
                use kiln_platform::time::PlatformTime;

                if !self.capabilities.clocks.monotonic_access {
                    return Err(Error::wasi_permission_denied(
                        "Monotonic clock access denied",
                    ));
                }

                let ns = PlatformTime::monotonic_ns();
                Ok(vec![CoreValue::I64(ns as i64)])
            },

            // subscribe-duration(when: duration) -> own<pollable>   (SR-91)
            #[cfg(feature = "wasi-clocks")]
            WasiFn::MonotonicClockSubscribeDuration => {
                if !self.capabilities.clocks.monotonic_access {
                    return Err(Error::wasi_permission_denied(
                        "Monotonic clock access denied",
                    ));
                }
                let ns = match args.first() {
                    Some(CoreValue::I64(n)) => *n as u64,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "subscribe-duration expects a u64 duration",
                        ));
                    },
                };
                let deadline = std::time::Instant::now()
                    .checked_add(std::time::Duration::from_nanos(ns))
                    .ok_or_else(|| {
                        Error::wasi_invalid_argument(
                            "subscribe-duration deadline is not representable",
                        )
                    })?;
                let handle = self.insert_pollable(Pollable::Deadline(deadline))?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            // subscribe-instant(when: instant) -> own<pollable>   (SR-91)
            // `when` is an absolute reading of the same monotonic clock `now` returns,
            // so the remaining wait is `when - now` on that clock; a past instant is
            // ready immediately.
            #[cfg(feature = "wasi-clocks")]
            WasiFn::MonotonicClockSubscribeInstant => {
                use kiln_platform::time::PlatformTime;

                if !self.capabilities.clocks.monotonic_access {
                    return Err(Error::wasi_permission_denied(
                        "Monotonic clock access denied",
                    ));
                }
                let when = match args.first() {
                    Some(CoreValue::I64(n)) => *n as u64,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "subscribe-instant expects a u64 instant",
                        ));
                    },
                };
                let remaining = when.saturating_sub(PlatformTime::monotonic_ns());
                let deadline = std::time::Instant::now()
                    .checked_add(std::time::Duration::from_nanos(remaining))
                    .ok_or_else(|| {
                        Error::wasi_invalid_argument(
                            "subscribe-instant deadline is not representable",
                        )
                    })?;
                let handle = self.insert_pollable(Pollable::Deadline(deadline))?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            // I/O functions that need memory access
            #[cfg(feature = "wasi-io")]
            WasiFn::OutputStreamBlockingWriteAndFlush => {
                #[cfg(feature = "tracing")]
                trace!(args = ?args, has_memory = memory.is_some(), "blocking-write-and-flush dispatch");

                if !self.capabilities.io.stdout_access {
                    return Err(Error::wasi_permission_denied("Stream write access denied"));
                }

                // Args: handle (i32), data_ptr (i32), data_len (i32), retptr (i32)
                // The retptr is where we write the result<_, stream-error>
                if args.len() < 3 {
                    return Err(Error::wasi_invalid_argument(
                        "blocking-write-and-flush requires 3+ args",
                    ));
                }

                let handle = match &args[0] {
                    CoreValue::I32(h) => *h as u32,
                    _ => return Err(Error::wasi_invalid_argument("Invalid handle type")),
                };

                let data_ptr = match &args[1] {
                    CoreValue::I32(p) => *p as u32,
                    _ => return Err(Error::wasi_invalid_argument("Invalid ptr type")),
                };

                let data_len = match &args[2] {
                    CoreValue::I32(l) => *l as u32,
                    _ => return Err(Error::wasi_invalid_argument("Invalid len type")),
                };

                // Get retptr if provided (4th argument)
                let retptr = if args.len() >= 4 {
                    match &args[3] {
                        CoreValue::I32(p) => Some(*p as u32),
                        _ => None,
                    }
                } else {
                    None
                };

                // Read data from memory
                let mem = memory.ok_or_else(|| {
                    Error::wasi_capability_unavailable("Memory required for write")
                })?;

                #[cfg(feature = "tracing")]
                trace!(
                    handle = handle,
                    data_ptr = format_args!("0x{:x}", data_ptr),
                    data_len = data_len,
                    retptr = ?retptr,
                    mem_size = mem.size(),
                    "write parameters"
                );

                // Read data from memory using MemoryAccessor
                let mut data = vec![0u8; data_len as usize];
                mem.read_bytes(data_ptr, &mut data)?;

                #[cfg(feature = "tracing")]
                trace!(
                    byte_count = data.len(),
                    preview = %String::from_utf8_lossy(&data[..data.len().min(100)]),
                    "data to write"
                );

                // Preview2: Look up resource by handle - NO FALLBACK
                #[cfg(feature = "std")]
                {
                    use std::io::{self, Write};

                    // Look up the resource in the resource manager
                    let resource = self.resource_manager.get_resource(handle)?;

                    let result = match resource.resource_type() {
                        WasiResourceType::OutputStream { name, .. } => {
                            let name_str = name
                                .as_str()
                                .map_err(|_| Error::wasi_invalid_fd("Invalid stream name"))?;

                            match name_str {
                                "stdout" => {
                                    #[cfg(feature = "tracing")]
                                    trace!("writing to stdout (handle {})", handle);
                                    io::stdout().write_all(&data).and_then(|_| io::stdout().flush())
                                },
                                "stderr" => {
                                    #[cfg(feature = "tracing")]
                                    trace!("writing to stderr (handle {})", handle);
                                    io::stderr().write_all(&data).and_then(|_| io::stderr().flush())
                                },
                                _other => {
                                    #[cfg(feature = "tracing")]
                                    warn!(
                                        handle = handle,
                                        name = _other,
                                        "unknown output stream name"
                                    );
                                    return Err(Error::wasi_invalid_fd("Unknown output stream"));
                                },
                            }
                        },
                        _ => {
                            #[cfg(feature = "tracing")]
                            warn!(handle = handle, "handle is not an output stream");
                            return Err(Error::wasi_invalid_fd("Handle is not an output stream"));
                        },
                    };

                    // Write result to retptr if provided
                    // For result<_, stream-error>:
                    //   discriminant 0 = ok (no payload)
                    //   discriminant 1 = err (followed by error info)
                    // Canonical ABI uses i32 (4 bytes) for discriminant
                    // An out-of-range retptr is an error from the write, never skipped.
                    if let Some(rp) = retptr {
                        match &result {
                            Ok(()) => {
                                // Ok variant: discriminant = 0 (4 bytes, little-endian)
                                mem.write_bytes(rp, &0u32.to_le_bytes())?;
                                #[cfg(feature = "tracing")]
                                trace!(
                                    retptr = format_args!("0x{:x}", rp),
                                    "wrote Ok discriminant (0)"
                                );
                            },
                            Err(_) => {
                                // Err variant: discriminant = 1 (4 bytes, little-endian)
                                // Note: stream-error payload would need more data, but for now just set discriminant
                                mem.write_bytes(rp, &1u32.to_le_bytes())?;
                                #[cfg(feature = "tracing")]
                                trace!(
                                    retptr = format_args!("0x{:x}", rp),
                                    "wrote Err discriminant (1)"
                                );
                            },
                        }
                    }

                    match result {
                        Ok(()) => {
                            #[cfg(feature = "tracing")]
                            trace!("write succeeded");
                            Ok(vec![]) // No stack return for functions with retptr
                        },
                        Err(_e) => {
                            #[cfg(feature = "tracing")]
                            warn!(error = ?_e, "write failed");
                            Ok(vec![]) // No stack return for functions with retptr
                        },
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    // In no_std, we can't write to stdout, but still write Ok to retptr
                    if let Some(rp) = retptr {
                        mem.write_bytes(rp, &0u32.to_le_bytes())?; // Ok discriminant (4 bytes)
                    }
                    Ok(vec![])
                }
            },

            // [method]output-stream.write - non-blocking write (may be partial)
            #[cfg(feature = "wasi-io")]
            WasiFn::OutputStreamWrite => {
                #[cfg(feature = "tracing")]
                trace!(args = ?args, has_memory = memory.is_some(), "output-stream.write dispatch");

                // Args: handle (i32), data_ptr (i32), data_len (i32), retptr (i32)
                // Returns result<_, stream-error> via retptr
                if args.len() < 3 {
                    return Err(Error::wasi_invalid_argument(
                        "output-stream.write requires 3+ args",
                    ));
                }

                let handle = match &args[0] {
                    CoreValue::I32(h) => *h as u32,
                    _ => return Err(Error::wasi_invalid_argument("Invalid handle type")),
                };

                let data_ptr = match &args[1] {
                    CoreValue::I32(p) => *p as u32,
                    _ => return Err(Error::wasi_invalid_argument("Invalid ptr type")),
                };

                let data_len = match &args[2] {
                    CoreValue::I32(l) => *l as u32,
                    _ => return Err(Error::wasi_invalid_argument("Invalid len type")),
                };

                let retptr = if args.len() >= 4 {
                    match &args[3] {
                        CoreValue::I32(p) => Some(*p as u32),
                        _ => None,
                    }
                } else {
                    None
                };

                #[cfg(feature = "std")]
                {
                    let mem = memory
                        .ok_or_else(|| Error::wasi_invalid_argument("write requires memory"))?;

                    // Read data from memory
                    let mut data = vec![0u8; data_len as usize];
                    mem.read_bytes(data_ptr, &mut data)?;

                    // Look up resource and write
                    let resource = self.resource_manager.get_resource(handle)?;
                    // `write` accepts the whole list or fails: it never reports a
                    // partial count.
                    let written = match resource.resource_type() {
                        crate::host_provider::resource_manager::WasiResourceType::OutputStream { name, .. } => {
                            let name_str = name.as_str().unwrap_or("unknown");
                            match name_str {
                                "stdout" => {
                                    use std::io::Write;
                                    std::io::stdout().write_all(&data)
                                }
                                "stderr" => {
                                    use std::io::Write;
                                    std::io::stderr().write_all(&data)
                                }
                                _ => Ok(()), // Treat as sink
                            }
                        }
                        _ => return Err(Error::wasi_invalid_fd("Handle is not an output stream")),
                    };

                    // result<_, stream-error>: 12-byte return area. ok = discriminant 0
                    // and no payload; a failed host write is err(closed) =
                    // discriminant 1, stream-error case 1 at +4.
                    if let Some(rp) = retptr {
                        match written {
                            Ok(()) => mem.write_bytes(rp, &0u32.to_le_bytes())?,
                            Err(_) => {
                                let case_ptr = rp.checked_add(4).ok_or_else(|| {
                                    Error::wasi_invalid_argument(
                                        "output-stream.write: retptr overflows",
                                    )
                                })?;
                                mem.write_bytes(rp, &1u32.to_le_bytes())?;
                                mem.write_bytes(case_ptr, &1u32.to_le_bytes())?;
                            },
                        }
                    }

                    Ok(vec![])
                }

                #[cfg(not(feature = "std"))]
                {
                    // result<_, stream-error>: ok is discriminant 0, no payload.
                    if let Some(rp) = retptr {
                        if let Some(mem) = memory {
                            mem.write_bytes(rp, &0u32.to_le_bytes())?;
                        }
                    }
                    Ok(vec![])
                }
            },

            #[cfg(feature = "wasi-io")]
            WasiFn::OutputStreamBlockingFlush => {
                #[cfg(feature = "std")]
                {
                    use std::io::{self, Write};
                    // Flush operations - intentionally ignore errors
                    drop(io::stdout().flush());
                    drop(io::stderr().flush());
                }
                Ok(vec![CoreValue::I32(0)])
            },

            // Resource drops - properly remove from resource manager
            WasiFn::StreamDrop => {
                // Extract handle from args
                let handle = match args.first() {
                    Some(CoreValue::I32(h)) => *h as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "resource-drop requires handle argument",
                        ));
                    },
                };

                // Only a stream handle is released here. Any other handle (a preopen,
                // a file descriptor, a directory-entry-stream) belongs to another
                // resource type and must keep its slot: freeing it would let a guest
                // release resources it does not hold through a stream drop.
                let is_stream = self.resource_manager.get_resource(handle).is_ok_and(|r| {
                    matches!(
                        r.resource_type(),
                        WasiResourceType::InputStream { .. }
                            | WasiResourceType::OutputStream { .. }
                    )
                }) && !self.dir_streams.contains_key(&handle);
                if is_stream {
                    // Release the file handle duplicated for a file input-stream
                    // (closing it) and its read position.
                    self.stream_files.remove(&handle);
                    self.stream_offsets.remove(&handle);
                    drop(self.resource_manager.remove_resource(handle));
                }
                Ok(vec![])
            },

            // Error resource drop (no resource tracking needed)
            WasiFn::ErrorDrop => Ok(vec![]),

            // CLI exit
            WasiFn::CliExit => {
                let status_ok = match args.first() {
                    Some(CoreValue::I32(0)) => true,
                    _ => false,
                };

                #[cfg(feature = "std")]
                {
                    std::process::exit(i32::from(!status_ok));
                }
                #[cfg(not(feature = "std"))]
                {
                    if status_ok {
                        Ok(vec![])
                    } else {
                        Err(Error::runtime_error("Component exited with failure"))
                    }
                }
            },

            // WASI Preview1 proc_exit - used by some components for exit
            WasiFn::PreviewOneProcExit => {
                let exit_code = match args.first() {
                    Some(CoreValue::I32(code)) => *code,
                    _ => 0,
                };

                #[cfg(feature = "std")]
                {
                    std::process::exit(exit_code);
                }
                #[cfg(not(feature = "std"))]
                {
                    if exit_code == 0 {
                        Ok(vec![])
                    } else {
                        Err(Error::runtime_error(
                            "Component exited with non-zero status",
                        ))
                    }
                }
            },

            // WASI Neural Network (wasi-nn) functions
            // These provide ML inference capabilities for WebAssembly components
            // Note: nn-preview2 feature enables the synchronous sync_bridge module
            #[cfg(all(feature = "nn-preview2", feature = "std"))]
            WasiFn::NnLoad => {
                use crate::nn::nn_load;

                // Args: (data_ptr: i32, data_len: i32, encoding: i32, target: i32)
                let data_ptr = match args.get(0) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_load: missing data_ptr",
                        ));
                    },
                };
                let data_len = match args.get(1) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_load: missing data_len",
                        ));
                    },
                };
                let encoding = match args.get(2) {
                    Some(CoreValue::I32(v)) => *v as u8,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_load: missing encoding",
                        ));
                    },
                };
                let target = match args.get(3) {
                    Some(CoreValue::I32(v)) => *v as u8,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_load: missing target",
                        ));
                    },
                };

                // Read model data from linear memory
                let mem = memory.ok_or_else(|| {
                    kiln_error::Error::runtime_error("nn_load requires memory access")
                })?;

                let mut data = vec![0u8; data_len as usize];
                mem.read_bytes(data_ptr, &mut data)?;

                // Call the actual function
                let graph_id = nn_load(data, encoding, target)?;

                Ok(vec![CoreValue::I32(graph_id as i32)])
            },

            #[cfg(all(feature = "nn-preview2", feature = "std"))]
            WasiFn::NnInitExecutionContext => {
                use crate::nn::nn_init_execution_context;

                // Args: (graph_id: i32)
                let graph_id = match args.get(0) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_init_execution_context: missing graph_id",
                        ));
                    },
                };

                let context_id = nn_init_execution_context(graph_id)?;

                Ok(vec![CoreValue::I32(context_id as i32)])
            },

            #[cfg(all(feature = "nn-preview2", feature = "std"))]
            WasiFn::NnSetInput => {
                use crate::nn::nn_set_input;

                // Args: (context_id, index, tensor_ptr, tensor_len, dims_ptr, dims_count, type)
                let context_id = match args.get(0) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing context_id",
                        ));
                    },
                };
                let index = match args.get(1) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing index",
                        ));
                    },
                };
                let tensor_ptr = match args.get(2) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing tensor_ptr",
                        ));
                    },
                };
                let tensor_len = match args.get(3) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing tensor_len",
                        ));
                    },
                };
                let dims_ptr = match args.get(4) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing dims_ptr",
                        ));
                    },
                };
                let dims_count = match args.get(5) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing dims_count",
                        ));
                    },
                };
                let tensor_type = match args.get(6) {
                    Some(CoreValue::I32(v)) => *v as u8,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_set_input: missing tensor_type",
                        ));
                    },
                };

                let mem = memory.ok_or_else(|| {
                    kiln_error::Error::runtime_error("nn_set_input requires memory access")
                })?;

                // Read tensor data
                let mut tensor_data = vec![0u8; tensor_len as usize];
                mem.read_bytes(tensor_ptr, &mut tensor_data)?;

                // Read dimensions (as u32 array)
                let mut dims_bytes = vec![0u8; (dims_count * 4) as usize];
                mem.read_bytes(dims_ptr, &mut dims_bytes)?;
                let dimensions: Vec<u32> = dims_bytes
                    .chunks(4)
                    .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();

                nn_set_input(context_id, index, tensor_data, dimensions, tensor_type)?;

                Ok(vec![])
            },

            #[cfg(all(feature = "nn-preview2", feature = "std"))]
            WasiFn::NnCompute => {
                use crate::nn::nn_compute;

                // Args: (context_id: i32)
                let context_id = match args.get(0) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_compute: missing context_id",
                        ));
                    },
                };

                nn_compute(context_id)?;

                Ok(vec![])
            },

            #[cfg(all(feature = "nn-preview2", feature = "std"))]
            WasiFn::NnGetOutput => {
                use crate::nn::nn_get_output;

                // Args: (context_id, index, out_ptr, out_capacity)
                let context_id = match args.get(0) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_get_output: missing context_id",
                        ));
                    },
                };
                let index = match args.get(1) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_get_output: missing index",
                        ));
                    },
                };
                let out_ptr = match args.get(2) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_get_output: missing out_ptr",
                        ));
                    },
                };
                let out_capacity = match args.get(3) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(kiln_error::Error::wasi_invalid_argument(
                            "nn_get_output: missing out_capacity",
                        ));
                    },
                };

                let mem = memory.ok_or_else(|| {
                    kiln_error::Error::runtime_error("nn_get_output requires memory access")
                })?;

                let (data, _dimensions, _tensor_type) = nn_get_output(context_id, index)?;

                // Write as much data as fits in the provided buffer
                let write_len = core::cmp::min(data.len(), out_capacity as usize);
                mem.write_bytes(out_ptr, &data[..write_len])?;

                // Return actual data length (caller may need to reallocate)
                Ok(vec![CoreValue::I32(data.len() as i32)])
            },

            // ================================================================
            // wasi:random/* - Random number generation interfaces
            // ================================================================
            #[cfg(feature = "wasi-random")]
            WasiFn::GetRandomBytes => {
                // Check random capability
                if !self.capabilities.random.secure_random {
                    return Err(Error::wasi_permission_denied("Secure random access denied"));
                }

                #[cfg(feature = "tracing")]
                trace!(args_len = args.len(), args = ?args, "get-random-bytes dispatch_core");

                // WASI Preview2: get-random-bytes(len: u64) -> list<u8>
                // Canonical ABI lowering: (len: i64, retptr: i32) or (len_lo: i32, len_hi: i32, retptr: i32)
                // Parse based on argument count and types
                let (len, retptr) = if args.len() >= 2 {
                    match (&args[0], &args[1]) {
                        // If first arg is i64, it's the length directly
                        (CoreValue::I64(n), CoreValue::I32(p)) => (*n as usize, *p as u32),
                        // If first arg is i32, check if second is also i32 (split u64 case)
                        (CoreValue::I32(n), CoreValue::I32(p)) => {
                            // Could be: (len_lo, retptr) with len_hi=0, or (len, retptr) direct
                            // For practical purposes, treat single i32 as the length
                            (*n as usize, *p as u32)
                        },
                        _ => {
                            return Err(Error::wasi_invalid_argument(
                                "get-random-bytes: invalid argument types",
                            ));
                        },
                    }
                } else {
                    return Err(Error::wasi_invalid_argument(
                        "get-random-bytes: missing arguments",
                    ));
                };

                // The list<u8> is owned and freed by the guest: it goes in the fresh
                // cabi_realloc'd buffer the engine allocated (read_buf_alloc), never
                // next to the return area. Requests over 1 MiB are refused. SR-38.
                let read_buf = self.read_buf_alloc.take();
                Self::write_random_list(
                    memory,
                    read_buf,
                    "get-random-bytes requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                    len,
                    retptr,
                )?;

                #[cfg(feature = "tracing")]
                trace!(len = len, retptr = retptr, "get-random-bytes complete");

                Ok(vec![])
            },

            #[cfg(feature = "wasi-random")]
            WasiFn::GetRandomU64 => {
                use kiln_platform::random::PlatformRandom;

                if !self.capabilities.random.secure_random {
                    return Err(Error::wasi_permission_denied("Secure random access denied"));
                }

                // Generate 8 random bytes and convert to u64
                let mut bytes = [0u8; 8];
                PlatformRandom::get_secure_bytes(&mut bytes)
                    .map_err(|_| Error::wasi_capability_unavailable("Random not available"))?;
                let val = u64::from_le_bytes(bytes);

                Ok(vec![CoreValue::I64(val as i64)])
            },

            #[cfg(feature = "wasi-random")]
            WasiFn::InsecureSeed => {
                use kiln_platform::random::PlatformRandom;
                let mut bytes = [0u8; 16];
                PlatformRandom::get_secure_bytes(&mut bytes)
                    .map_err(|_| Error::wasi_capability_unavailable("Random not available"))?;
                let (lo, hi) = bytes.split_at(8);
                let v0 = u64::from_le_bytes(lo.try_into().map_err(|_| {
                    Error::wasi_runtime_error("insecure-seed: low half is not 8 bytes")
                })?);
                let v1 = u64::from_le_bytes(hi.try_into().map_err(|_| {
                    Error::wasi_runtime_error("insecure-seed: high half is not 8 bytes")
                })?);
                Ok(vec![CoreValue::I64(v0 as i64), CoreValue::I64(v1 as i64)])
            },

            // ================================================================
            // wasi:filesystem/types - descriptor operations (core dispatch)
            // ================================================================
            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorGetType => {
                // get-type(self: borrow<descriptor>) -> result<descriptor-type, error-code>
                // Core lowering: (i32 handle, i32 retptr)
                let fd = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("get-type: missing descriptor")),
                };
                let is_dir = self.preopens.iter().any(|(preopen_fd, _)| *preopen_fd == fd)
                    || self.fd_table.get(&fd).is_some_and(|e| {
                        matches!(e.fd_type, FileDescriptorType::PreopenDirectory(_))
                    });
                // descriptor-type enum: 3=directory, 6=regular-file
                let dtype = if is_dir { 3i32 } else { 6i32 };
                // result<ok, _>: discriminant=0, payload=dtype
                Ok(vec![CoreValue::I32(0), CoreValue::I32(dtype)])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorOpenAt => {
                // open-at(self, path-flags, path, open-flags, descriptor-flags) -> result<descriptor, error-code>
                // Core: (i32 fd, i32 path_flags, i32 path_ptr, i32 path_len, i32 open_flags, i32 desc_flags, i32 retptr)
                let dir_fd = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("open-at: missing descriptor")),
                };
                let path_ptr = match args.get(2) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("open-at: missing path ptr")),
                };
                let path_len = match args.get(3) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("open-at: missing path len")),
                };
                let mem = memory.ok_or_else(|| Error::runtime_error("open-at: no memory"))?;
                let mut path_buf = vec![0u8; path_len as usize];
                mem.read_bytes(path_ptr, &mut path_buf)?;
                let path_str = String::from_utf8_lossy(&path_buf).to_string();

                // Resolve base directory from preopens or fd_table
                let base_path = self
                    .preopens
                    .iter()
                    .find(|(fd, _)| *fd == dir_fd)
                    .map(|(_, p)| p.clone())
                    .or_else(|| {
                        self.fd_table.get(&dir_fd).and_then(|e| match &e.fd_type {
                            FileDescriptorType::PreopenDirectory(p) => Some(p.clone()),
                            _ => None,
                        })
                    });

                if let Some(base) = base_path {
                    // Sandbox gate: resolve → contain (symlink-safe) → allow-list.
                    let full_path = self.resolve_and_authorize(&base, &path_str)?;
                    if Self::trailing_slash_on_file(&path_str, &full_path) {
                        return Ok(fs_err(fs_error::NOT_DIRECTORY));
                    }
                    if full_path.exists() {
                        // Granted access = the guest's descriptor-flags (read = bit 0,
                        // write = bit 1) ∩ the filesystem capabilities.
                        let desc_flags =
                            Self::core_u32(args, 5, "open-at: missing descriptor-flags")?;
                        let read_access =
                            desc_flags & 1 != 0 && self.capabilities.filesystem.read_access;
                        let write_access =
                            desc_flags & 2 != 0 && self.capabilities.filesystem.write_access;
                        // Keep the opened handle: read/write/sync-data use it and never
                        // reopen by path (TOCTOU). Directories get no file handle; nothing
                        // but a regular file is ever opened (a FIFO would block the runtime).
                        let file_type = match std::fs::metadata(&full_path) {
                            Ok(m) => m.file_type(),
                            Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                        };
                        let file = if file_type.is_dir() {
                            None
                        } else if file_type.is_file() {
                            match Self::open_authorized_file(
                                &base,
                                &full_path,
                                read_access,
                                write_access,
                            )? {
                                Ok(f) => Some(f),
                                Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                            }
                        } else {
                            return Ok(fs_err(fs_error::NOT_PERMITTED));
                        };
                        let path_for_resource = full_path.to_string_lossy().to_string();
                        let new_handle = self.resource_manager.create_file_descriptor(
                            &path_for_resource,
                            read_access,
                            write_access,
                        )?;
                        self.fd_table.insert(
                            new_handle,
                            FileDescriptorEntry {
                                fd_type: FileDescriptorType::RegularFile(full_path),
                                read: read_access,
                                write: write_access,
                                file,
                            },
                        );
                        // result<ok=descriptor>: discriminant=0, payload=handle
                        Ok(vec![CoreValue::I32(0), CoreValue::I32(new_handle as i32)])
                    } else {
                        Ok(fs_err(fs_error::NO_ENTRY))
                    }
                } else {
                    // error-code::bad-descriptor = 3
                    Ok(vec![CoreValue::I32(1), CoreValue::I32(3)])
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorStat => {
                // stat(self) -> result<descriptor-stat, error-code>
                // Core: (i32 fd, i32 retptr)
                let fd = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("stat: missing descriptor")),
                };
                let file_path = self.fd_table.get(&fd).and_then(|e| match &e.fd_type {
                    FileDescriptorType::RegularFile(p) => Some(p.clone()),
                    FileDescriptorType::PreopenDirectory(p) => Some(p.clone()),
                    _ => None,
                });
                if let Some(path) = file_path {
                    match std::fs::metadata(&path) {
                        Ok(meta) => Ok(Self::stat_core_values(&meta)),
                        Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                    }
                } else {
                    Ok(vec![CoreValue::I32(1), CoreValue::I32(3)]) // bad-descriptor
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorStatAt => {
                // stat-at(self, path-flags, path) -> result<descriptor-stat, error-code>
                let dir_fd = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("stat-at: missing descriptor")),
                };
                let path_ptr = match args.get(2) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("stat-at: missing path ptr")),
                };
                let path_len = match args.get(3) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("stat-at: missing path len")),
                };
                let mem = memory.ok_or_else(|| Error::runtime_error("stat-at: no memory"))?;
                let mut path_buf = vec![0u8; path_len as usize];
                mem.read_bytes(path_ptr, &mut path_buf)?;
                let path_str = String::from_utf8_lossy(&path_buf).to_string();

                let base = self
                    .preopens
                    .iter()
                    .find(|(fd, _)| *fd == dir_fd)
                    .map(|(_, p)| p.clone())
                    .or_else(|| {
                        self.fd_table.get(&dir_fd).and_then(|e| match &e.fd_type {
                            FileDescriptorType::PreopenDirectory(p) => Some(p.clone()),
                            _ => None,
                        })
                    });

                if let Some(base_path) = base {
                    // Sandbox gate: resolve → contain (symlink-safe) → allow-list.
                    let full_path = self.resolve_and_authorize(&base_path, &path_str)?;
                    if Self::trailing_slash_on_file(&path_str, &full_path) {
                        return Ok(fs_err(fs_error::NOT_DIRECTORY));
                    }
                    match std::fs::metadata(&full_path) {
                        Ok(meta) => Ok(Self::stat_core_values(&meta)),
                        Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                    }
                } else {
                    Ok(vec![CoreValue::I32(1), CoreValue::I32(3)])
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorReadViaStream => {
                // read-via-stream(self, offset: u64) -> result<input-stream, error-code>
                let fd = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "read-via-stream: missing descriptor",
                        ));
                    },
                };
                let offset = Self::core_u64(args, 1, "read-via-stream: missing offset")?;
                // The stream reads from a duplicate of the kept open-at handle (never a
                // re-opened path). Only a descriptor open-at opened for reading has one.
                let handle_copy = match self.fd_table.get(&fd) {
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::RegularFile(_),
                        read: true,
                        file: Some(f),
                        ..
                    }) => match f.try_clone() {
                        Ok(c) => c,
                        Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                    },
                    _ => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                let stream_name = format!("file-read:{}", fd);
                let stream_handle = self.resource_manager.create_input_stream(&stream_name)?;
                self.stream_files.insert(stream_handle, handle_copy);
                self.stream_offsets.insert(stream_handle, offset as usize);
                Ok(vec![
                    CoreValue::I32(0),
                    CoreValue::I32(stream_handle as i32),
                ])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorReadDirectory => {
                // read-directory(self) -> result<directory-entry-stream, error-code>
                // Snapshot the directory's entries now; the guest walks them via
                // read-directory-entry. `.`/`..` are excluded (std::fs::read_dir omits them),
                // matching preview2 semantics. SR-39.
                let fd = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "read-directory: missing descriptor",
                        ));
                    },
                };
                let dir_path = self.fd_table.get(&fd).and_then(|e| match &e.fd_type {
                    FileDescriptorType::PreopenDirectory(p)
                    | FileDescriptorType::RegularFile(p) => Some(p.clone()),
                    _ => None,
                });
                let dir_path = match dir_path {
                    Some(p) => p,
                    None => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                let mut entries: Vec<(u8, String)> = Vec::new();
                match std::fs::read_dir(&dir_path) {
                    Ok(rd) => {
                        for ent in rd.flatten() {
                            // descriptor-type enum: unknown=0, block-device=1, character-device=2,
                            // directory=3, fifo=4, symbolic-link=5, regular-file=6, socket=7.
                            let ty = ent.file_type().ok();
                            let tycode: u8 = match ty {
                                Some(t) if t.is_dir() => 3,
                                Some(t) if t.is_symlink() => 5,
                                Some(t) if t.is_file() => 6,
                                _ => 0,
                            };
                            entries.push((tycode, ent.file_name().to_string_lossy().into_owned()));
                        }
                    },
                    Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                }
                let handle = self.resource_manager.create_input_stream(&format!("dir:{}", fd))?;
                self.dir_streams.insert(handle, (entries, 0));
                Ok(vec![CoreValue::I32(0), CoreValue::I32(handle as i32)])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::ReadDirectoryEntry => {
                // read-directory-entry(self) -> result<option<directory-entry>, error-code>
                // directory-entry = record { type: descriptor-type, name: string }. Core values:
                //   ok(some): [I32(0), I32(1), I32(type), I32(name_ptr), I32(name_len)]
                //   ok(none): [I32(0), I32(0)]
                // The name string goes in a fresh cabi_realloc'd buffer (read_buf_alloc). SR-39.
                let stream_handle = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "read-directory-entry: missing stream",
                        ));
                    },
                };
                let read_buf = self.read_buf_alloc.take();
                let next = self
                    .dir_streams
                    .get(&stream_handle)
                    .and_then(|(entries, cursor)| entries.get(*cursor).cloned());
                match next {
                    None => Ok(vec![CoreValue::I32(0), CoreValue::I32(0)]), // ok(none)
                    Some((tycode, name)) => {
                        let (buf_ptr, buf_size) = match read_buf {
                            Some((p, s)) => (p, s as usize),
                            None => {
                                return Err(Error::runtime_error(
                                    "read-directory-entry requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                                ));
                            },
                        };
                        let mem = memory.ok_or_else(|| {
                            Error::wasi_invalid_argument("read-directory-entry: no memory")
                        })?;
                        let name_bytes = name.as_bytes();
                        let n = core::cmp::min(name_bytes.len(), buf_size);
                        mem.write_bytes(buf_ptr, &name_bytes[..n])?;
                        if let Some((_, cursor)) = self.dir_streams.get_mut(&stream_handle) {
                            *cursor += 1;
                        }
                        Ok(vec![
                            CoreValue::I32(0),              // result::ok
                            CoreValue::I32(1),              // option::some
                            CoreValue::I32(tycode as i32),  // descriptor-type
                            CoreValue::I32(buf_ptr as i32), // name ptr
                            CoreValue::I32(n as i32),       // name len
                        ])
                    },
                }
            },

            // ----------------------------------------------------------------
            // SR-72: descriptor read/write/sync-data and directory mutation.
            //
            // Every result is `[I32(0), payload...]` (ok) or `[I32(1), I32(error-code)]`;
            // the caller lowers them to the retptr per the signature table. A path that
            // fails the sandbox gate returns `Err` (same as open-at/stat-at) and the host
            // operation is never performed.
            // ----------------------------------------------------------------
            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorRead => {
                // read(self, length: filesize, offset: filesize)
                //   -> result<tuple<list<u8>, bool>, error-code>
                // Core: (i32 fd, i64 length, i64 offset, i32 retptr)
                //   ok: [I32(0), I32(list ptr), I32(list len), I32(eof)]
                // The list<u8> lives in a fresh cabi_realloc'd guest buffer
                // (read_buf_alloc, SR-38) that the guest owns and frees; never reused.
                let fd = Self::core_u32(args, 0, "read: missing descriptor")?;
                let length = Self::core_u64(args, 1, "read: missing length")?;
                let offset = Self::core_u64(args, 2, "read: missing offset")?;
                let read_buf = self.read_buf_alloc.take();

                if !self.capabilities.filesystem.read_access {
                    return Err(Error::wasi_permission_denied(
                        "Filesystem read access denied",
                    ));
                }
                // Only a file handle open-at opened is readable: never a stream handle,
                // and never a path re-opened here (TOCTOU).
                let (file, readable) = match self.fd_table.get(&fd) {
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::RegularFile(_),
                        read,
                        file: Some(f),
                        ..
                    }) => (f, *read),
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::PreopenDirectory(_),
                        ..
                    }) => {
                        return Ok(fs_err(fs_error::IS_DIRECTORY));
                    },
                    _ => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                if !readable {
                    return Ok(fs_err(fs_error::BAD_DESCRIPTOR));
                }
                // FAIL LOUD: the engine allocated no guest buffer for the returned list.
                let (buf_ptr, buf_size) = read_buf.ok_or_else(|| {
                    Error::runtime_error(
                        "descriptor.read requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                    )
                })?;
                let mem = memory.ok_or_else(|| Error::wasi_invalid_argument("read: no memory"))?;

                use std::io::{Read as _, Seek as _};
                let mut handle: &std::fs::File = file;
                let file_len = match handle.metadata() {
                    Ok(m) => m.len(),
                    Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                };
                if let Err(e) = handle.seek(std::io::SeekFrom::Start(offset)) {
                    return Ok(fs_err(Self::fs_error_code(&e)));
                }
                // Never return more than the guest asked for or than the engine allocated.
                let want = core::cmp::min(length, u64::from(buf_size));
                let mut data = Vec::new();
                if let Err(e) = handle.take(want).read_to_end(&mut data) {
                    return Ok(fs_err(Self::fs_error_code(&e)));
                }
                mem.write_bytes(buf_ptr, &data)?;
                let eof = offset.saturating_add(data.len() as u64) >= file_len;
                Ok(vec![
                    CoreValue::I32(0),
                    CoreValue::I32(buf_ptr as i32),
                    CoreValue::I32(data.len() as i32),
                    CoreValue::I32(i32::from(eof)),
                ])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorWrite => {
                // write(self, buffer: list<u8>, offset: filesize) -> result<filesize, error-code>
                // Core: (i32 fd, i32 buf_ptr, i32 buf_len, i64 offset, i32 retptr)
                //   ok: [I32(0), I64(bytes written)]
                let fd = Self::core_u32(args, 0, "write: missing descriptor")?;
                let buf_ptr = Self::core_u32(args, 1, "write: missing buffer ptr")?;
                let buf_len = Self::core_u32(args, 2, "write: missing buffer len")?;
                let offset = Self::core_u64(args, 3, "write: missing offset")?;

                // Only the handle open-at opened is written; never a re-opened path.
                let (file, writable) = match self.fd_table.get(&fd) {
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::RegularFile(_),
                        write,
                        file: Some(f),
                        ..
                    }) => (f, *write),
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::PreopenDirectory(_),
                        ..
                    }) => {
                        return Ok(fs_err(fs_error::IS_DIRECTORY));
                    },
                    _ => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                // The descriptor must have been opened for writing.
                if !writable {
                    return Ok(fs_err(fs_error::BAD_DESCRIPTOR));
                }
                let mem = memory.ok_or_else(|| Error::wasi_invalid_argument("write: no memory"))?;
                let data = Self::read_guest_bytes(mem, buf_ptr, buf_len)?;

                use std::io::{Seek as _, Write as _};
                let mut handle: &std::fs::File = file;
                // Seeking past the end then writing extends the file with zeros.
                if let Err(e) = handle
                    .seek(std::io::SeekFrom::Start(offset))
                    .and_then(|_| handle.write_all(&data))
                {
                    return Ok(fs_err(Self::fs_error_code(&e)));
                }
                Ok(vec![CoreValue::I32(0), CoreValue::I64(data.len() as i64)])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorSyncData => {
                // sync-data(self) -> result<_, error-code>
                // Core: (i32 fd, i32 retptr). A no-op success when the descriptor is not
                // open for writing (per the WIT).
                let fd = Self::core_u32(args, 0, "sync-data: missing descriptor")?;
                // A file syncs the handle open-at opened (never a re-opened path). A
                // preopen directory has no guest-opened handle; its path is the
                // host-configured root, which the guest cannot replace (the sandbox
                // refuses any operation on the preopen itself).
                let synced = match self.fd_table.get(&fd) {
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::RegularFile(_),
                        write,
                        file: Some(f),
                        ..
                    }) => {
                        if !*write {
                            return Ok(vec![CoreValue::I32(0)]);
                        }
                        f.sync_data()
                    },
                    Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::PreopenDirectory(p),
                        write,
                        ..
                    }) => {
                        if !*write {
                            return Ok(vec![CoreValue::I32(0)]);
                        }
                        std::fs::File::open(p).and_then(|d| d.sync_data())
                    },
                    _ => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                match synced {
                    Ok(()) => Ok(vec![CoreValue::I32(0)]),
                    Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::CreateDirectoryAt | WasiFn::RemoveDirectoryAt | WasiFn::UnlinkFileAt => {
                // f(self, path: string) -> result<_, error-code>
                // Core: (i32 fd, i32 path_ptr, i32 path_len, i32 retptr)
                let fd = Self::core_u32(args, 0, "directory op: missing descriptor")?;
                let path_ptr = Self::core_u32(args, 1, "directory op: missing path ptr")?;
                let path_len = Self::core_u32(args, 2, "directory op: missing path len")?;
                let mem = memory
                    .ok_or_else(|| Error::wasi_invalid_argument("directory op: no memory"))?;
                let guest_path = Self::read_guest_path(mem, path_ptr, path_len)?;
                self.require_fs_write()?;
                let base = match self.preopen_base(fd) {
                    Some(b) => b,
                    None => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                // Sandbox gate; the final component is never followed so unlinking a
                // symlink removes the link, not its target.
                let full_path = self.resolve_and_authorize_nofollow(&base, &guest_path)?;
                // mkdir("file/") is EEXIST on POSIX, so create-directory-at keeps
                // the host's answer; unlink and rmdir report not-directory.
                if function != "[method]descriptor.create-directory-at"
                    && Self::trailing_slash_on_file(&guest_path, &full_path)
                {
                    return Ok(fs_err(fs_error::NOT_DIRECTORY));
                }
                let outcome = match function {
                    "[method]descriptor.create-directory-at" => std::fs::create_dir(&full_path),
                    "[method]descriptor.remove-directory-at" => std::fs::remove_dir(&full_path),
                    _ => match std::fs::symlink_metadata(&full_path) {
                        Ok(meta) if meta.is_dir() => return Ok(fs_err(fs_error::IS_DIRECTORY)),
                        Ok(_) => std::fs::remove_file(&full_path),
                        Err(e) => Err(e),
                    },
                };
                match outcome {
                    Ok(()) => Ok(vec![CoreValue::I32(0)]),
                    Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::RenameAt => {
                // rename-at(self, old-path, new-descriptor: borrow<descriptor>, new-path)
                //   -> result<_, error-code>
                // Core: (i32 fd, i32 old_ptr, i32 old_len, i32 new_fd, i32 new_ptr, i32 new_len, i32 retptr)
                let fd = Self::core_u32(args, 0, "rename-at: missing descriptor")?;
                let old_ptr = Self::core_u32(args, 1, "rename-at: missing old path ptr")?;
                let old_len = Self::core_u32(args, 2, "rename-at: missing old path len")?;
                let new_fd = Self::core_u32(args, 3, "rename-at: missing new descriptor")?;
                let new_ptr = Self::core_u32(args, 4, "rename-at: missing new path ptr")?;
                let new_len = Self::core_u32(args, 5, "rename-at: missing new path len")?;
                let mem =
                    memory.ok_or_else(|| Error::wasi_invalid_argument("rename-at: no memory"))?;
                let old_guest = Self::read_guest_path(mem, old_ptr, old_len)?;
                let new_guest = Self::read_guest_path(mem, new_ptr, new_len)?;
                self.require_fs_write()?;
                let (old_base, new_base) = match (self.preopen_base(fd), self.preopen_base(new_fd))
                {
                    (Some(o), Some(n)) => (o, n),
                    _ => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                // BOTH paths pass the sandbox gate, each against its own descriptor,
                // before anything is renamed.
                let old_full = self.resolve_and_authorize_nofollow(&old_base, &old_guest)?;
                let new_full = self.resolve_and_authorize_nofollow(&new_base, &new_guest)?;
                if Self::trailing_slash_on_file(&old_guest, &old_full)
                    || Self::trailing_slash_on_file(&new_guest, &new_full)
                {
                    return Ok(fs_err(fs_error::NOT_DIRECTORY));
                }
                match std::fs::rename(&old_full, &new_full) {
                    Ok(()) => Ok(vec![CoreValue::I32(0)]),
                    Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::LinkAt => {
                // link-at(self, old-path-flags, old-path, new-descriptor, new-path)
                //   -> result<_, error-code>
                // Core: (i32 fd, i32 flags, i32 old_ptr, i32 old_len, i32 new_fd,
                //        i32 new_ptr, i32 new_len, i32 retptr)
                let fd = Self::core_u32(args, 0, "link-at: missing descriptor")?;
                let flags = Self::core_u32(args, 1, "link-at: missing path-flags")?;
                let old_ptr = Self::core_u32(args, 2, "link-at: missing old path ptr")?;
                let old_len = Self::core_u32(args, 3, "link-at: missing old path len")?;
                let new_fd = Self::core_u32(args, 4, "link-at: missing new descriptor")?;
                let new_ptr = Self::core_u32(args, 5, "link-at: missing new path ptr")?;
                let new_len = Self::core_u32(args, 6, "link-at: missing new path len")?;
                let mem =
                    memory.ok_or_else(|| Error::wasi_invalid_argument("link-at: no memory"))?;
                let old_guest = Self::read_guest_path(mem, old_ptr, old_len)?;
                let new_guest = Self::read_guest_path(mem, new_ptr, new_len)?;
                self.require_fs_write()?;
                let (old_base, new_base) = match (self.preopen_base(fd), self.preopen_base(new_fd))
                {
                    (Some(o), Some(n)) => (o, n),
                    _ => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                // path-flags::symlink-follow is bit 0.
                let old_full = if flags & 1 != 0 {
                    self.resolve_and_authorize(&old_base, &old_guest)?
                } else {
                    self.resolve_and_authorize_nofollow(&old_base, &old_guest)?
                };
                let new_full = self.resolve_and_authorize_nofollow(&new_base, &new_guest)?;
                // The WIT permits hard links to regular files only; a symlink (reached
                // without symlink-follow) or directory is `not-permitted`.
                match std::fs::symlink_metadata(&old_full) {
                    Ok(meta) if meta.is_file() => {},
                    Ok(_) => return Ok(fs_err(fs_error::NOT_PERMITTED)),
                    Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                }
                match std::fs::hard_link(&old_full, &new_full) {
                    Ok(()) => Ok(vec![CoreValue::I32(0)]),
                    Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::SymlinkAt => {
                // symlink-at(self, old-path: string /* link contents */, new-path: string)
                //   -> result<_, error-code>
                // Core: (i32 fd, i32 contents_ptr, i32 contents_len, i32 new_ptr, i32 new_len, i32 retptr)
                let fd = Self::core_u32(args, 0, "symlink-at: missing descriptor")?;
                let contents_ptr = Self::core_u32(args, 1, "symlink-at: missing target ptr")?;
                let contents_len = Self::core_u32(args, 2, "symlink-at: missing target len")?;
                let new_ptr = Self::core_u32(args, 3, "symlink-at: missing new path ptr")?;
                let new_len = Self::core_u32(args, 4, "symlink-at: missing new path len")?;
                let mem =
                    memory.ok_or_else(|| Error::wasi_invalid_argument("symlink-at: no memory"))?;
                let contents = Self::read_guest_path(mem, contents_ptr, contents_len)?;
                let new_guest = Self::read_guest_path(mem, new_ptr, new_len)?;
                self.require_fs_write()?;
                let base = match self.preopen_base(fd) {
                    Some(b) => b,
                    None => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                // The link's own location passes the sandbox gate...
                let new_full = self.resolve_and_authorize_nofollow(&base, &new_guest)?;
                // ...and its contents must be relative with no `..` component
                // (absolute or `..`-bearing targets are `not-permitted`).
                if !symlink_target_is_safe(std::path::Path::new(&contents)) {
                    return Ok(fs_err(fs_error::NOT_PERMITTED));
                }
                #[cfg(unix)]
                {
                    match std::os::unix::fs::symlink(&contents, &new_full) {
                        Ok(()) => Ok(vec![CoreValue::I32(0)]),
                        Err(e) => Ok(fs_err(Self::fs_error_code(&e))),
                    }
                }
                #[cfg(not(unix))]
                {
                    Ok(fs_err(fs_error::UNSUPPORTED))
                }
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::ReadlinkAt => {
                // readlink-at(self, path: string) -> result<string, error-code>
                // Core: (i32 fd, i32 path_ptr, i32 path_len, i32 retptr)
                //   ok: [I32(0), I32(string ptr), I32(string len)]
                // The string lives in a fresh cabi_realloc'd guest buffer (read_buf_alloc).
                let fd = Self::core_u32(args, 0, "readlink-at: missing descriptor")?;
                let path_ptr = Self::core_u32(args, 1, "readlink-at: missing path ptr")?;
                let path_len = Self::core_u32(args, 2, "readlink-at: missing path len")?;
                let read_buf = self.read_buf_alloc.take();
                if !self.capabilities.filesystem.read_access {
                    return Err(Error::wasi_permission_denied(
                        "Filesystem read access denied",
                    ));
                }
                let mem =
                    memory.ok_or_else(|| Error::wasi_invalid_argument("readlink-at: no memory"))?;
                let guest_path = Self::read_guest_path(mem, path_ptr, path_len)?;
                let base = match self.preopen_base(fd) {
                    Some(b) => b,
                    None => return Ok(fs_err(fs_error::BAD_DESCRIPTOR)),
                };
                let full_path = self.resolve_and_authorize_nofollow(&base, &guest_path)?;
                let target = match std::fs::read_link(&full_path) {
                    Ok(t) => t,
                    Err(e) => return Ok(fs_err(Self::fs_error_code(&e))),
                };
                // The WIT: an absolute or rooted target is `not-permitted`.
                if target.has_root() {
                    return Ok(fs_err(fs_error::NOT_PERMITTED));
                }
                let target = match target.to_str() {
                    Some(t) => t,
                    None => return Ok(fs_err(fs_error::ILLEGAL_BYTE_SEQUENCE)),
                };
                // FAIL LOUD: the engine allocated no guest buffer for the returned string.
                let (buf_ptr, buf_size) = read_buf.ok_or_else(|| {
                    Error::runtime_error(
                        "readlink-at requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                    )
                })?;
                // Never truncate a link target into a shorter, different path.
                if target.len() > buf_size as usize {
                    return Ok(fs_err(fs_error::NAME_TOO_LONG));
                }
                mem.write_bytes(buf_ptr, target.as_bytes())?;
                Ok(vec![
                    CoreValue::I32(0),
                    CoreValue::I32(buf_ptr as i32),
                    CoreValue::I32(target.len() as i32),
                ])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorMetadataHash => {
                // Return a simple hash based on file path
                // metadata-hash-value: { upper: u64, lower: u64 }
                Ok(vec![
                    CoreValue::I32(0),
                    CoreValue::I64(0),
                    CoreValue::I64(0),
                ])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DescriptorDrop => {
                // Dropping a descriptor ends its lifetime: remove the file descriptor
                // `open-at` created, which closes the kept File and frees its slot in
                // the resource table.
                //
                // A PREOPEN is never removed. `get-directories` hands out the preopen's
                // fixed handle (it does not mint a fresh own<descriptor> per call), and
                // every later path-based call resolves against it, so removing it would
                // permanently disable the preopen. Per the canonical ABI a drop only
                // ends the guest's use of that handle; here there is no guest-side
                // state to release for it, so the drop is a no-op.
                let handle = Self::core_u32(args, 0, "resource-drop: missing descriptor")?;
                if self.preopen_base(handle).is_none() {
                    if let Some(FileDescriptorEntry {
                        fd_type: FileDescriptorType::RegularFile(_),
                        ..
                    }) = self.fd_table.get(&handle)
                    {
                        self.fd_table.remove(&handle);
                        drop(self.resource_manager.remove_resource(handle));
                    }
                }
                Ok(vec![])
            },

            #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
            WasiFn::DirectoryEntryStreamDrop => {
                // Release the directory snapshot and the stream's resource-table slot.
                // Only a handle that really is a directory-entry-stream frees a slot.
                let handle = Self::core_u32(args, 0, "resource-drop: missing stream")?;
                if self.dir_streams.remove(&handle).is_some() {
                    drop(self.resource_manager.remove_resource(handle));
                }
                Ok(vec![])
            },

            // wasi:io/streams - input stream read operations
            WasiFn::InputStreamRead => {
                // read(self: borrow<input-stream>, len: u64) -> result<list<u8>, stream-error>
                //
                // Returns CORE VALUES; the caller (dispatch_canon_lowered) lowers them to the
                // retptr per the signature Result(ListU8, stream-error):
                //   ok(list<u8>):      [I32(0), I32(ptr), I32(len)]
                //   err(closed):       [I32(1), I32(1)]   (stream-error::closed)
                // The list<u8> bytes live in a fresh cabi_realloc'd buffer (read_buf_alloc)
                // supplied by the engine per call — the guest owns and frees it, so it must
                // NOT be reused. A per-stream offset advances each read and yields closed
                // (the guest's EOF signal) once the file is exhausted.
                let stream_handle = match args.first() {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => return Err(Error::wasi_invalid_argument("read: missing stream handle")),
                };
                let max_len = match args.get(1) {
                    Some(CoreValue::I64(v)) => *v as usize,
                    Some(CoreValue::I32(v)) => *v as usize,
                    _ => 4096,
                };
                // stream-error::closed (result::err discriminant 1, variant 1).
                let closed = || vec![CoreValue::I32(1), CoreValue::I32(1)];

                // Consume the per-call read buffer (fresh each read).
                let read_buf = self.read_buf_alloc.take();

                // Reads go through the handle kept from open-at (never a re-opened path).
                let file = match self.stream_files.get(&stream_handle) {
                    Some(f) => f,
                    None => return Ok(closed()),
                };
                let file_len = match file.metadata() {
                    Ok(m) => m.len() as usize,
                    Err(_) => return Ok(closed()),
                };

                let offset = *self.stream_offsets.get(&stream_handle).unwrap_or(&0);
                if offset >= file_len {
                    return Ok(closed()); // EOF
                }

                let (buf_ptr, buf_size) = match read_buf {
                    Some((p, s)) => (p, s as usize),
                    // FAIL LOUD: data to return but the engine allocated no guest buffer.
                    None => {
                        return Err(Error::runtime_error(
                            "input-stream read requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                        ));
                    },
                };
                let mem = memory.ok_or_else(|| Error::wasi_invalid_argument("read: no memory"))?;
                let avail = file_len - offset;
                let want = core::cmp::min(core::cmp::min(avail, max_len), buf_size);
                use std::io::{Read as _, Seek as _};
                let mut handle: &std::fs::File = file;
                let mut data = Vec::new();
                if handle.seek(std::io::SeekFrom::Start(offset as u64)).is_err()
                    || handle.take(want as u64).read_to_end(&mut data).is_err()
                {
                    return Ok(closed());
                }
                let n = data.len();
                mem.write_bytes(buf_ptr, &data)?;
                self.stream_offsets.insert(stream_handle, offset + n);

                // result::ok(list<u8> { ptr, len }) as core values.
                Ok(vec![
                    CoreValue::I32(0),
                    CoreValue::I32(buf_ptr as i32),
                    CoreValue::I32(n as i32),
                ])
            },

            WasiFn::InputStreamSubscribe => {
                // subscribe(self) -> pollable: a real table entry. Input streams are
                // synchronous in kiln (see `Pollable::AlwaysReady`).
                let handle = self.insert_pollable(Pollable::AlwaysReady)?;
                Ok(vec![CoreValue::I32(handle as i32)])
            },

            // wasi:filesystem/preopens - core dispatch
            #[cfg(feature = "wasi-filesystem")]
            WasiFn::GetDirectories => {
                // Return list<tuple<descriptor, string>> at retptr.
                //
                // The canonical-ABI return area at retptr is only 8 bytes (the list
                // header: ptr + len). The list entries and path strings MUST live in
                // guest-owned memory allocated via cabi_realloc — writing them into
                // memory adjacent to retptr (as an earlier version did) hands the guest
                // a list backed by memory it never allocated, so preview2 libc registers
                // no usable preopen and every file op returns noent. This mirrors the
                // get-arguments path exactly (see preopens_alloc / pre_allocate_wasi_preopens).
                // This call's allocation is consumed whether or not it is used: the guest
                // owns (and may free) the list, so it is never handed out twice.
                let preopens_alloc = self.preopens_alloc.take();
                if let Some(mem) = memory {
                    let retptr = match args.first() {
                        Some(CoreValue::I32(v)) => *v as u32,
                        _ => {
                            return Err(Error::wasi_invalid_argument(
                                "get-directories: missing retptr",
                            ));
                        },
                    };
                    if self.preopens.is_empty() {
                        // No preopens: empty list (ptr=0, len=0)
                        mem.write_bytes(retptr, &0u32.to_le_bytes())?;
                        mem.write_bytes(Self::retptr_at(retptr, 4)?, &0u32.to_le_bytes())?;
                        return Ok(vec![]);
                    }

                    // Use pre-allocated cabi_realloc memory. NO FALLBACK — per FAIL-LOUD,
                    // if the engine did not pre-allocate (no cabi_realloc export) we error
                    // rather than scribble into unowned guest memory.
                    let (list_ptr, string_ptrs) = match preopens_alloc {
                        Some((lp, sp)) => (lp, sp),
                        None => {
                            return Err(Error::runtime_error(
                                "get-directories requires cabi_realloc pre-allocation (preopens_alloc not set)",
                            ));
                        },
                    };
                    if string_ptrs.len() != self.preopens.len() {
                        return Err(Error::runtime_error(
                            "get-directories: preopen string allocation count mismatch",
                        ));
                    }

                    // Each entry is (descriptor: u32, path_ptr: u32, path_len: u32) = 12 bytes.
                    let entry_size = 12u32;
                    for (i, (fd, path)) in self.preopens.iter().enumerate() {
                        let (string_ptr, alloc_len) = string_ptrs[i];
                        let path_str = path.to_string_lossy();
                        let path_bytes = path_str.as_bytes();
                        let path_len = core::cmp::min(path_bytes.len() as u32, alloc_len);
                        // Write the path string into its own cabi_realloc'd buffer.
                        mem.write_bytes(string_ptr, &path_bytes[..path_len as usize])?;
                        // Write the entry (descriptor, path_ptr, path_len) into the list buffer.
                        let entry_offset = list_ptr + (i as u32 * entry_size);
                        mem.write_bytes(entry_offset, &(*fd).to_le_bytes())?;
                        mem.write_bytes(entry_offset + 4, &string_ptr.to_le_bytes())?;
                        mem.write_bytes(entry_offset + 8, &path_len.to_le_bytes())?;
                    }
                    // Write list header (ptr, len) into the return area.
                    mem.write_bytes(retptr, &list_ptr.to_le_bytes())?;
                    mem.write_bytes(
                        Self::retptr_at(retptr, 4)?,
                        &(self.preopens.len() as u32).to_le_bytes(),
                    )?;
                }
                Ok(vec![])
            },

            // ================================================================
            // wasi:cli/environment - initial-cwd (core dispatch)
            // ================================================================
            WasiFn::InitialCwd => {
                // Return the actual current working directory
                // Core lowering: option<string> = (discriminant, ptr, len) via retptr
                #[cfg(feature = "std")]
                {
                    let cwd_opt = std::env::current_dir().ok();
                    // The string is owned and freed by the guest: it goes in the engine's
                    // cabi_realloc'd buffer, never after the return area. SR-38.
                    let read_buf = self.read_buf_alloc.take();
                    if let Some(mem) = memory {
                        let retptr = match args.first() {
                            Some(CoreValue::I32(p)) => *p as u32,
                            _ => return Ok(vec![CoreValue::I32(0)]), // None
                        };
                        if let Some(cwd) = cwd_opt {
                            let cwd_str = cwd.to_string_lossy();
                            let bytes = cwd_str.as_bytes();
                            let (data_ptr, len) = Self::write_owned_return(
                                mem,
                                read_buf,
                                "initial-cwd requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                                bytes,
                            )?;
                            // Write Some discriminant (1), ptr, len
                            mem.write_bytes(retptr, &1u32.to_le_bytes())?;
                            mem.write_bytes(Self::retptr_at(retptr, 4)?, &data_ptr.to_le_bytes())?;
                            mem.write_bytes(Self::retptr_at(retptr, 8)?, &len.to_le_bytes())?;
                        } else {
                            // Write None discriminant (0)
                            mem.write_bytes(retptr, &0u32.to_le_bytes())?;
                        }
                        Ok(vec![])
                    } else {
                        // No memory, return None discriminant
                        Ok(vec![CoreValue::I32(0)])
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    // No cwd in no_std - return None
                    Ok(vec![CoreValue::I32(0)])
                }
            },

            // ================================================================
            // wasi:io/error - error.to-debug-string (core dispatch)
            // ================================================================
            WasiFn::ErrorToDebugString => {
                // Returns a debug string for the error resource
                // Core lowering: string = (ptr, len) via retptr
                // The string is owned and freed by the guest: it goes in the engine's
                // cabi_realloc'd buffer, never after the return area. SR-38.
                let read_buf = self.read_buf_alloc.take();
                if let Some(mem) = memory {
                    let retptr = match args.get(1) {
                        Some(CoreValue::I32(p)) => *p as u32,
                        _ => {
                            // No retptr, return minimal string info
                            return Ok(vec![CoreValue::I32(0), CoreValue::I32(0)]);
                        },
                    };
                    let debug_str = b"error";
                    let (data_ptr, len) = Self::write_owned_return(
                        mem,
                        read_buf,
                        "error.to-debug-string requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                        debug_str,
                    )?;
                    mem.write_bytes(retptr, &data_ptr.to_le_bytes())?;
                    mem.write_bytes(Self::retptr_at(retptr, 4)?, &len.to_le_bytes())?;
                    Ok(vec![])
                } else {
                    Ok(vec![CoreValue::I32(0), CoreValue::I32(0)])
                }
            },

            // ================================================================
            // wasi:clocks/monotonic-clock - resolution (core dispatch)
            // ================================================================
            #[cfg(feature = "wasi-clocks")]
            WasiFn::MonotonicClockResolution => {
                if !self.capabilities.clocks.monotonic_access {
                    return Err(Error::wasi_permission_denied(
                        "Monotonic clock access denied",
                    ));
                }
                // Monotonic clock resolution is 1 nanosecond
                Ok(vec![CoreValue::I64(1)])
            },

            // ================================================================
            // wasi:random/insecure - insecure random (core dispatch)
            // ================================================================
            #[cfg(feature = "wasi-random")]
            WasiFn::GetInsecureRandomBytes => {
                if !self.capabilities.random.pseudo_random {
                    return Err(Error::wasi_permission_denied("Pseudo-random access denied"));
                }

                // Args: len (i64 or i32), retptr (i32)
                let len = match args.first() {
                    Some(CoreValue::I64(v)) => *v as usize,
                    Some(CoreValue::I32(v)) => *v as usize,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "get-insecure-random-bytes: missing length",
                        ));
                    },
                };
                let retptr = match args.get(1) {
                    Some(CoreValue::I32(v)) => *v as u32,
                    _ => {
                        return Err(Error::wasi_invalid_argument(
                            "get-insecure-random-bytes: missing retptr",
                        ));
                    },
                };
                // Guest-owned list<u8>: written into the engine's cabi_realloc'd buffer. SR-38.
                let read_buf = self.read_buf_alloc.take();
                Self::write_random_list(
                    memory,
                    read_buf,
                    "get-insecure-random-bytes requires a cabi_realloc'd buffer (read_buf_alloc not set)",
                    len,
                    retptr,
                )?;
                Ok(vec![])
            },

            #[cfg(feature = "wasi-random")]
            WasiFn::GetInsecureRandomU64 => {
                use kiln_platform::random::PlatformRandom;

                if !self.capabilities.random.pseudo_random {
                    return Err(Error::wasi_permission_denied("Pseudo-random access denied"));
                }

                let mut bytes = [0u8; 8];
                PlatformRandom::get_secure_bytes(&mut bytes)
                    .map_err(|_| Error::wasi_capability_unavailable("Random not available"))?;
                let val = u64::from_le_bytes(bytes);
                Ok(vec![CoreValue::I64(val as i64)])
            },

            // ================================================================
            // wasi:sockets/* - Socket interfaces (core dispatch)
            // ================================================================
            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpCreateSocket => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_create(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpStartConnect => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_connect(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpStartBind => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_bind(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpListen => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_listen(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpAccept => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_accept(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpSend => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_send(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpReceive => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_recv(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::TcpShutdown => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_tcp_shutdown(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::UdpCreateSocket => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_udp_create(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::UdpStartBind => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_udp_bind(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::UdpSend => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_udp_send(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::UdpReceive => {
                let wasi_args = Self::core_to_component_values(args);
                let results = wasi_udp_recv(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },

            #[cfg(all(feature = "wasi-sockets", feature = "std"))]
            WasiFn::ResolveAddresses => {
                let wasi_args = Self::core_to_component_values(args);
                let results =
                    wasi_resolve_addresses(&mut () as &mut dyn core::any::Any, wasi_args)?;
                Ok(Self::component_to_core_values(&results))
            },
        }
    }

    /// Convert core values to component values for delegating to preview2 functions
    #[cfg(all(feature = "wasi-sockets", feature = "std"))]
    fn core_to_component_values(args: &[kiln_foundation::values::Value]) -> Vec<Value> {
        use kiln_foundation::values::Value as CoreValue;

        args.iter()
            .map(|v| match v {
                CoreValue::I32(n) => Value::S32(*n),
                CoreValue::I64(n) => Value::S64(*n),
                CoreValue::F32(f) => Value::F32(f.to_f32()),
                CoreValue::F64(f) => Value::F64(f.to_f64()),
                _ => Value::S32(0),
            })
            .collect()
    }

    /// Convert component values back to core values
    #[cfg(all(feature = "wasi-sockets", feature = "std"))]
    fn component_to_core_values(results: &[Value]) -> Vec<kiln_foundation::values::Value> {
        use kiln_foundation::values::Value as CoreValue;

        results
            .iter()
            .map(|v| match v {
                Value::S32(n) => CoreValue::I32(*n),
                Value::S64(n) => CoreValue::I64(*n),
                Value::U32(n) => CoreValue::I32(*n as i32),
                Value::U64(n) => CoreValue::I64(*n as i64),
                Value::Bool(b) => CoreValue::I32(if *b { 1 } else { 0 }),
                _ => CoreValue::I32(0),
            })
            .collect()
    }
}

/// Implementation of `HostImportHandler` for `WasiDispatcher`
///
/// This allows the engine to call WASI functions through the generic trait
/// without knowing about WASI specifics.
#[cfg(feature = "std")]
impl kiln_foundation::HostImportHandler for WasiDispatcher {
    fn resolves(&self, module: &str, function: &str) -> bool {
        served_wasi_fn(module, function).is_some()
    }

    fn call_import(
        &mut self,
        module: &str,
        function: &str,
        args: &[kiln_foundation::Value],
        memory: Option<&dyn MemoryAccessor>,
    ) -> Result<Vec<kiln_foundation::Value>> {
        // Delegate to dispatch_core with MemoryAccessor
        self.dispatch_core(module, function, args, memory)
    }

    fn set_args_allocation(&mut self, list_ptr: u32, string_ptrs: Vec<(u32, u32)>) {
        self.set_args_alloc(list_ptr, string_ptrs);
    }

    fn get_preopens(&self) -> Vec<(u32, String)> {
        self.preopens
            .iter()
            .map(|(fd, path)| (*fd, path.to_string_lossy().into_owned()))
            .collect()
    }

    fn set_preopens_allocation(&mut self, list_ptr: u32, string_ptrs: Vec<(u32, u32)>) {
        self.preopens_alloc = Some((list_ptr, string_ptrs));
    }

    fn set_read_buffer_allocation(&mut self, ptr: u32, size: u32) {
        self.read_buf_alloc = Some((ptr, size));
    }
}

/// True iff `full` (a guest path already joined onto the preopen `base`) stays
/// inside the sandbox — a **lexical** check that does not touch the filesystem,
/// so it is correct for not-yet-created files. Rejects any `..` traversal and
/// any absolute path that `Path::join` let replace the base. A
/// `canonicalize`-based check silently admits a write to a non-existent
/// `../target` because `canonicalize` fails on the missing file (#392).
fn is_within_sandbox(base: &std::path::Path, full: &std::path::Path) -> bool {
    use std::path::Component;
    !full.components().any(|c| matches!(c, Component::ParentDir)) && full.starts_with(base)
}

/// Test-only: runs once, right before `open_authorized_file` opens the file.
#[cfg(all(test, feature = "wasi-filesystem", feature = "std"))]
thread_local! {
    static BEFORE_OPEN_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

/// `wasi:filesystem/types` `error-code` enum indices (WASI 0.2 declaration order).
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
mod fs_error {
    pub const ACCESS: i32 = 0;
    pub const BAD_DESCRIPTOR: i32 = 3;
    pub const EXIST: i32 = 7;
    pub const ILLEGAL_BYTE_SEQUENCE: i32 = 9;
    pub const INVALID: i32 = 12;
    pub const IO: i32 = 13;
    pub const IS_DIRECTORY: i32 = 14;
    pub const NAME_TOO_LONG: i32 = 18;
    pub const NO_ENTRY: i32 = 20;
    pub const NOT_DIRECTORY: i32 = 24;
    pub const NOT_EMPTY: i32 = 25;
    /// Only returned by symlink-at on hosts without POSIX symlinks.
    #[cfg_attr(unix, allow(dead_code))]
    pub const UNSUPPORTED: i32 = 27;
    pub const NOT_PERMITTED: i32 = 31;
}

/// True iff a symlink with these `contents` may be created: relative, with no
/// `..` component at all. Lexically resolving `..` can disagree with the kernel
/// when a component is itself a symlink, so the simplest safe rule is to refuse
/// every `..` (and every absolute / prefixed target), even one that would stay
/// inside the preopen.
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
fn symlink_target_is_safe(contents: &std::path::Path) -> bool {
    use std::path::Component;
    contents
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Core-value result for `result<_, error-code>` / `result<T, error-code>` errors.
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
fn fs_err(code: i32) -> Vec<kiln_foundation::values::Value> {
    use kiln_foundation::values::Value as CoreValue;
    vec![CoreValue::I32(1), CoreValue::I32(code)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_foundation::memory_init::MemoryInitializer;

    /// AD-LINK-001: the link-time check and dispatch read one table. Every
    /// listed import resolves under any `@version`; an import the table does
    /// not list neither resolves nor dispatches. (That every variant has a
    /// row and an arm is enforced at compile time; see `WasiFn`.)
    // rivet: verifies SR-65
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn link_check_and_dispatch_agree_on_served_imports() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;

        for (interface, function, _) in SERVED_WASI_IMPORTS {
            assert!(
                dispatcher.resolves(interface, function),
                "{interface}::{function}"
            );
            let versioned = format!("{interface}@0.2.6");
            assert!(
                dispatcher.resolves(&versioned, function),
                "{versioned}::{function}"
            );
        }

        for (interface, function) in [
            ("no:such/iface@1.0.0", "missing"),
            ("wasi:cli/stdout@0.2.0", "no-such-function"),
        ] {
            assert!(
                !dispatcher.resolves(interface, function),
                "{interface}::{function}"
            );
            let err = dispatcher
                .dispatch_core(interface, function, &[], None)
                .expect_err("an unlisted import must not dispatch");
            assert_eq!(err.message, "Unknown WASI function");
        }
        Ok(())
    }

    /// SR-81: meld's `$N` instance suffix resolves to the same function as the
    /// unsuffixed name, through the one resolver the link check and dispatch
    /// share (AD-LINK-001); a bad suffix or an unknown base is refused.
    // rivet: verifies SR-81
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn suffixed_imports_resolve_to_the_base_function_for_link_check_and_dispatch() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;

        assert_eq!(
            served_wasi_fn("wasi:cli/stderr@0.2.6", "get-stderr$5"),
            served_wasi_fn("wasi:cli/stderr@0.2.6", "get-stderr")
        );
        assert!(served_wasi_fn("wasi:cli/stderr@0.2.6", "get-stderr$5").is_some());
        assert_eq!(
            served_wasi_fn(
                "wasi:io/streams@0.2.6",
                "[method]output-stream.blocking-write-and-flush$2"
            ),
            served_wasi_fn(
                "wasi:io/streams@0.2.6",
                "[method]output-stream.blocking-write-and-flush"
            )
        );

        // Every served import also resolves, to the same function, with a suffix.
        for (interface, function, served) in SERVED_WASI_IMPORTS {
            let suffixed = format!("{function}$7");
            assert_eq!(
                served_wasi_fn(interface, &suffixed),
                Some(*served),
                "{interface}::{suffixed}"
            );
            assert!(
                dispatcher.resolves(interface, &suffixed),
                "{interface}::{suffixed}"
            );
        }

        // A suffixed name dispatches like the unsuffixed one.
        let handle =
            dispatcher.dispatch_core("wasi:cli/stderr@0.2.6", "get-stderr$5", &[], None)?;
        assert_eq!(handle.len(), 1);

        for function in [
            "get-stderr$",
            "get-stderr$x",
            "get-stderr$5x",
            "get-stderr$+5",
            "not-a-function$3",
        ] {
            assert!(
                !dispatcher.resolves("wasi:cli/stderr@0.2.6", function),
                "{function}"
            );
            let err = dispatcher
                .dispatch_core("wasi:cli/stderr@0.2.6", function, &[], None)
                .expect_err("a refused name must not dispatch");
            assert_eq!(err.message, "Unknown WASI function");
        }
        // The suffix does not move a function to another interface.
        assert!(!dispatcher.resolves("wasi:cli/stdout@0.2.6", "get-stderr$5"));
        Ok(())
    }

    /// kiln never issues a terminal handle (`get-terminal-*` returns `none`),
    /// so it serves the terminal resource drops, and a drop can only name a
    /// handle that was never issued: it traps instead of succeeding silently.
    // rivet: verifies SR-65
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn terminal_resource_drops_resolve_and_trap_on_an_unissued_handle() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;

        for (interface, function) in [
            (
                "wasi:cli/terminal-input@0.2.6",
                "[resource-drop]terminal-input",
            ),
            (
                "wasi:cli/terminal-output@0.2.6",
                "[resource-drop]terminal-output",
            ),
        ] {
            assert!(
                dispatcher.resolves(interface, function),
                "{interface}::{function}"
            );
            let err = dispatcher
                .dispatch_core(
                    interface,
                    function,
                    &[kiln_foundation::values::Value::I32(1)],
                    None,
                )
                .expect_err("dropping a terminal handle kiln never issued must trap");
            assert_eq!(err.category, kiln_error::ErrorCategory::RuntimeTrap);
        }
        Ok(())
    }

    /// Each import name is listed once; a second row for the same name would
    /// never be reached.
    // rivet: verifies SR-65
    #[cfg(feature = "std")]
    #[test]
    fn served_imports_table_lists_each_import_once() {
        let mut seen = std::collections::HashSet::new();
        for (interface, function, _) in SERVED_WASI_IMPORTS {
            assert!(
                seen.insert((interface, function)),
                "duplicate row {interface}::{function}"
            );
        }
    }

    /// The #392 sandbox-escape hazard: a write to a not-yet-existing `../target`
    /// must be rejected. An existence-gated (canonicalize) check would admit it
    /// because canonicalize fails on the missing file.
    // rivet: verifies SR-33
    #[test]
    fn sandbox_rejects_parent_dir_escape_even_when_target_missing() {
        use std::path::Path;
        let base = Path::new("/preopen/sandbox");
        // within the preopen — accepted
        assert!(is_within_sandbox(base, &base.join("inside.txt")));
        assert!(is_within_sandbox(base, &base.join("sub/inside.txt")));
        // `..` escape to a NON-EXISTENT target — must be rejected
        assert!(!is_within_sandbox(base, &base.join("../evil.txt")));
        assert!(!is_within_sandbox(base, &base.join("sub/../../evil.txt")));
        // absolute path that `join` lets replace the base — rejected
        assert!(!is_within_sandbox(base, &base.join("/etc/passwd")));
    }

    /// SR-37 / #405: get-directories must back its `list<tuple<descriptor,string>>`
    /// return with cabi_realloc'd guest memory, NOT memory adjacent to the 8-byte
    /// return area. The list header at retptr must point at the pre-allocated
    /// `list_ptr` (not retptr+8), and each entry (descriptor, path_ptr, path_len)
    /// must reference the pre-allocated string buffer — so a preview2 guest gets a
    /// usable preopen instead of a list backed by memory it never allocated.
    // rivet: verifies SR-37
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_directories_uses_cabi_realloc_allocation_not_retptr_scribble() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;

        // Register a preopen "." (as `--wasi-fs .` would).
        let fd = dispatcher.add_preopen(".")?;

        // Simulate the engine's cabi_realloc pre-allocation: a 12-byte entry buffer
        // at 0x2000 and a 1-byte path-string buffer at 0x3000 — both well away from
        // the 8-byte return area at retptr.
        let retptr = 0x1000u32;
        let list_ptr = 0x2000u32;
        let str_ptr = 0x3000u32;
        dispatcher.set_preopens_allocation(list_ptr, vec![(str_ptr, 1)]);

        let mem = SliceMemory::with_size(0x10000);
        let res = dispatcher.dispatch_core(
            "wasi:filesystem/preopens",
            "get-directories",
            &[kiln_foundation::values::Value::I32(retptr as i32)],
            Some(&mem),
        )?;
        assert!(
            res.is_empty(),
            "get-directories returns void (writes via retptr)"
        );

        // Return area: (list_ptr, len) — MUST point at the allocation, not retptr+8.
        let mut hdr = [0u8; 8];
        mem.read_bytes(retptr, &mut hdr)?;
        let got_ptr = u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]);
        let got_len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]);
        assert_eq!(
            got_ptr, list_ptr,
            "list header must point at cabi_realloc'd list_ptr"
        );
        assert_ne!(
            got_ptr,
            retptr + 8,
            "must NOT scribble into memory adjacent to the return area"
        );
        assert_eq!(got_len, 1, "exactly one preopen");

        // Entry at list_ptr: (descriptor, path_ptr, path_len).
        let mut entry = [0u8; 12];
        mem.read_bytes(list_ptr, &mut entry)?;
        let e_fd = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]);
        let e_pptr = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
        let e_plen = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
        assert_eq!(e_fd, fd, "entry descriptor is the preopen handle");
        assert_eq!(
            e_pptr, str_ptr,
            "entry path_ptr references the allocated string buffer"
        );
        assert_eq!(e_plen, 1, "path \".\" is 1 byte");

        // The path string "." lives in the allocated buffer.
        let mut pbuf = [0u8; 1];
        mem.read_bytes(str_ptr, &mut pbuf)?;
        assert_eq!(
            &pbuf, b".",
            "path bytes written into the cabi_realloc'd buffer"
        );
        Ok(())
    }

    /// SR-37 / #405: FAIL-LOUD — with preopens present but no cabi_realloc
    /// allocation set, get-directories must error rather than scribble into
    /// unowned guest memory (the pre-fix behavior that produced ENOENT).
    // rivet: verifies SR-37
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_directories_fails_loud_without_allocation() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        dispatcher.add_preopen(".")?;
        // NOTE: no set_preopens_allocation call.
        let mem = SliceMemory::with_size(0x10000);
        let res = dispatcher.dispatch_core(
            "wasi:filesystem/preopens",
            "get-directories",
            &[kiln_foundation::values::Value::I32(0x1000)],
            Some(&mem),
        );
        assert!(
            res.is_err(),
            "must fail loud when cabi_realloc pre-allocation is missing"
        );
        Ok(())
    }

    /// SR-102: the allocation the engine made for one `get-directories` call is
    /// consumed by it. The guest owns (and may free) that list, so a second call
    /// without a fresh allocation fails loud instead of rewriting the same memory.
    // rivet: verifies SR-102
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_directories_allocation_is_consumed_by_one_call() -> Result<()> {
        use kiln_foundation::{HostImportHandler, traits::SliceMemory};
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        dispatcher.add_preopen(".")?;
        dispatcher.set_preopens_allocation(0x2000, vec![(0x3000, 1)]);
        let mem = SliceMemory::with_size(0x10000);
        let call = |d: &mut WasiDispatcher| {
            d.dispatch_core(
                "wasi:filesystem/preopens",
                "get-directories",
                &[kiln_foundation::values::Value::I32(0x1000)],
                Some(&mem),
            )
        };
        assert!(call(&mut dispatcher).is_ok());
        assert!(
            call(&mut dispatcher).is_err(),
            "a second call must not reuse the first call's allocation"
        );
        Ok(())
    }

    /// SR-102: the allocation made for one `get-arguments` call is consumed by it.
    // rivet: verifies SR-102
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn get_arguments_allocation_is_consumed_by_one_call() -> Result<()> {
        use kiln_foundation::{HostImportHandler, traits::SliceMemory};
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        dispatcher.set_args(vec!["prog".to_string()]);
        dispatcher.set_args_allocation(0x2000, vec![(0x3000, 4)]);
        let mem = SliceMemory::with_size(0x10000);
        let call = |d: &mut WasiDispatcher| {
            d.dispatch_core(
                "wasi:cli/environment",
                "get-arguments",
                &[kiln_foundation::values::Value::I32(0x1000)],
                Some(&mem),
            )
        };
        assert!(call(&mut dispatcher).is_ok());
        assert!(
            call(&mut dispatcher).is_err(),
            "a second call must not reuse the first call's allocation"
        );
        Ok(())
    }

    /// Fill `[start, start + len)` with a sentinel byte, for asserting that a host call
    /// never writes into guest memory it was not handed.
    #[cfg(feature = "std")]
    fn fill_sentinel(
        mem: &kiln_foundation::traits::SliceMemory,
        start: u32,
        len: usize,
    ) -> Result<()> {
        mem.write_bytes(start, &vec![0xAAu8; len])?;
        Ok(())
    }

    /// SR-38: get-random-bytes writes its list<u8> into the engine-supplied
    /// cabi_realloc'd buffer, never next to the return area.
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_random_bytes_returns_in_owned_buffer_not_after_retptr() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let retptr = 0x1000u32;
        let buf_ptr = 0x4000u32;
        fill_sentinel(&mem, retptr + 8, 64)?;
        fill_sentinel(&mem, buf_ptr, 64)?;

        dispatcher.set_read_buffer_allocation(buf_ptr, 32);
        let res = dispatcher.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[
                kiln_foundation::values::Value::I64(32),
                kiln_foundation::values::Value::I32(retptr as i32),
            ],
            Some(&mem),
        )?;
        assert!(
            res.is_empty(),
            "get-random-bytes returns void (writes via retptr)"
        );

        let mut hdr = [0u8; 8];
        mem.read_bytes(retptr, &mut hdr)?;
        assert_eq!(
            u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]),
            buf_ptr
        );
        assert_eq!(u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]), 32);

        let mut after = [0u8; 64];
        mem.read_bytes(retptr + 8, &mut after)?;
        assert!(
            after.iter().all(|b| *b == 0xAA),
            "must NOT write past the return area"
        );
        // 32 random bytes replaced the sentinel at the buffer (all-0xAA is 2^-256).
        let mut got = [0u8; 32];
        mem.read_bytes(buf_ptr, &mut got)?;
        assert!(
            got.iter().any(|b| *b != 0xAA),
            "random bytes written into the buffer"
        );
        let mut tail = [0u8; 32];
        mem.read_bytes(buf_ptr + 32, &mut tail)?;
        assert!(tail.iter().all(|b| *b == 0xAA), "exactly len bytes written");
        Ok(())
    }

    /// SR-38: FAIL-LOUD — get-random-bytes without an engine-supplied buffer errors.
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_random_bytes_fails_loud_without_buffer() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let res = dispatcher.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[
                kiln_foundation::values::Value::I64(16),
                kiln_foundation::values::Value::I32(0x1000),
            ],
            Some(&mem),
        );
        assert!(res.is_err(), "must fail loud when no buffer was allocated");
        Ok(())
    }

    /// SR-38: get-random-bytes refuses (rather than truncates) a request over 1 MiB,
    /// and a request larger than the allocated buffer.
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_random_bytes_refuses_oversized_requests() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);

        dispatcher.set_read_buffer_allocation(0x4000, 1 << 20);
        let res = dispatcher.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[
                kiln_foundation::values::Value::I64((1 << 20) + 1),
                kiln_foundation::values::Value::I32(0x1000),
            ],
            Some(&mem),
        );
        assert!(res.is_err(), "len > 1 MiB must be refused, not truncated");

        dispatcher.set_read_buffer_allocation(0x4000, 8);
        let res = dispatcher.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[
                kiln_foundation::values::Value::I64(16),
                kiln_foundation::values::Value::I32(0x1000),
            ],
            Some(&mem),
        );
        assert!(res.is_err(), "len > allocated buffer must be refused");
        Ok(())
    }

    /// SR-38: get-insecure-random-bytes writes into the engine-supplied buffer.
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_insecure_random_bytes_returns_in_owned_buffer_not_after_retptr() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let retptr = 0x1000u32;
        let buf_ptr = 0x4000u32;
        fill_sentinel(&mem, retptr + 8, 64)?;
        fill_sentinel(&mem, buf_ptr, 64)?;

        dispatcher.set_read_buffer_allocation(buf_ptr, 32);
        dispatcher.dispatch_core(
            "wasi:random/insecure",
            "get-insecure-random-bytes",
            &[
                kiln_foundation::values::Value::I64(32),
                kiln_foundation::values::Value::I32(retptr as i32),
            ],
            Some(&mem),
        )?;

        let mut hdr = [0u8; 8];
        mem.read_bytes(retptr, &mut hdr)?;
        assert_eq!(
            u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]),
            buf_ptr
        );
        assert_eq!(u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]), 32);
        let mut after = [0u8; 64];
        mem.read_bytes(retptr + 8, &mut after)?;
        assert!(
            after.iter().all(|b| *b == 0xAA),
            "must NOT write past the return area"
        );
        let mut got = [0u8; 32];
        mem.read_bytes(buf_ptr, &mut got)?;
        assert!(
            got.iter().any(|b| *b != 0xAA),
            "random bytes written into the buffer"
        );
        Ok(())
    }

    /// SR-38: FAIL-LOUD — get-insecure-random-bytes without a buffer errors.
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_insecure_random_bytes_fails_loud_without_buffer() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let res = dispatcher.dispatch_core(
            "wasi:random/insecure",
            "get-insecure-random-bytes",
            &[
                kiln_foundation::values::Value::I64(16),
                kiln_foundation::values::Value::I32(0x1000),
            ],
            Some(&mem),
        );
        assert!(res.is_err(), "must fail loud when no buffer was allocated");
        Ok(())
    }

    /// SR-38: initial-cwd writes the path into the engine-supplied buffer and the
    /// option<string> header at retptr; nothing past the 12-byte return area.
    // rivet: verifies SR-38
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn initial_cwd_returns_in_owned_buffer_not_after_retptr() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let retptr = 0x1000u32;
        let buf_ptr = 0x4000u32;
        fill_sentinel(&mem, retptr + 12, 64)?;

        dispatcher.set_read_buffer_allocation(buf_ptr, 4096);
        let res = dispatcher.dispatch_core(
            "wasi:cli/environment",
            "initial-cwd",
            &[kiln_foundation::values::Value::I32(retptr as i32)],
            Some(&mem),
        )?;
        assert!(
            res.is_empty(),
            "initial-cwd returns void (writes via retptr)"
        );

        let mut hdr = [0u8; 12];
        mem.read_bytes(retptr, &mut hdr)?;
        assert_eq!(
            u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]),
            1,
            "some"
        );
        assert_eq!(
            u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]),
            buf_ptr
        );
        let len = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as usize;
        let mut got = vec![0u8; len];
        mem.read_bytes(buf_ptr, &mut got)?;
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            got,
            cwd.to_string_lossy().as_bytes(),
            "cwd bytes in the buffer"
        );

        let mut after = [0u8; 64];
        mem.read_bytes(retptr + 12, &mut after)?;
        assert!(
            after.iter().all(|b| *b == 0xAA),
            "must NOT write past the return area"
        );
        Ok(())
    }

    /// SR-38: FAIL-LOUD — initial-cwd without a buffer errors; a buffer too small for
    /// the path is refused rather than truncated.
    // rivet: verifies SR-38
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn initial_cwd_fails_loud_without_or_with_undersized_buffer() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let args = [kiln_foundation::values::Value::I32(0x1000)];
        let res =
            dispatcher.dispatch_core("wasi:cli/environment", "initial-cwd", &args, Some(&mem));
        assert!(res.is_err(), "must fail loud when no buffer was allocated");

        dispatcher.set_read_buffer_allocation(0x4000, 1);
        let res =
            dispatcher.dispatch_core("wasi:cli/environment", "initial-cwd", &args, Some(&mem));
        assert!(
            res.is_err(),
            "a path longer than the buffer must be refused"
        );
        Ok(())
    }

    /// SR-38: error.to-debug-string writes into the engine-supplied buffer.
    // rivet: verifies SR-38
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn error_to_debug_string_returns_in_owned_buffer_not_after_retptr() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let retptr = 0x1000u32;
        let buf_ptr = 0x4000u32;
        fill_sentinel(&mem, retptr + 8, 64)?;

        dispatcher.set_read_buffer_allocation(buf_ptr, 256);
        dispatcher.dispatch_core(
            "wasi:io/error",
            "[method]error.to-debug-string",
            &[
                kiln_foundation::values::Value::I32(1),
                kiln_foundation::values::Value::I32(retptr as i32),
            ],
            Some(&mem),
        )?;

        let mut hdr = [0u8; 8];
        mem.read_bytes(retptr, &mut hdr)?;
        assert_eq!(
            u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]),
            buf_ptr
        );
        let len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
        assert_eq!(len, 5);
        let mut got = vec![0u8; len];
        mem.read_bytes(buf_ptr, &mut got)?;
        assert_eq!(&got, b"error");
        let mut after = [0u8; 64];
        mem.read_bytes(retptr + 8, &mut after)?;
        assert!(
            after.iter().all(|b| *b == 0xAA),
            "must NOT write past the return area"
        );
        Ok(())
    }

    /// SR-38: FAIL-LOUD — error.to-debug-string without a buffer errors.
    // rivet: verifies SR-38
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn error_to_debug_string_fails_loud_without_buffer() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let res = dispatcher.dispatch_core(
            "wasi:io/error",
            "[method]error.to-debug-string",
            &[
                kiln_foundation::values::Value::I32(1),
                kiln_foundation::values::Value::I32(0x1000),
            ],
            Some(&mem),
        );
        assert!(res.is_err(), "must fail loud when no buffer was allocated");
        Ok(())
    }

    /// SR-38 / #405: input-stream blocking-read returns the file bytes in the
    /// engine-supplied cabi_realloc'd buffer (as core values [ok, ptr, len]),
    /// advances a per-stream offset, and signals stream-error::closed at EOF —
    /// so the guest's read loop terminates instead of re-reading from 0 or being
    /// handed a fixed unowned buffer (the pre-fix bug that corrupted the guest).
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn blocking_read_returns_bytes_in_owned_buffer_and_eofs() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;

        // Backing file with known contents.
        let path = std::env::temp_dir().join("kiln_sr38_blocking_read.txt");
        std::fs::File::create(&path).unwrap().write_all(b"CANARY").unwrap();

        // Register a stream handle backed by that file (as read-via-stream would).
        let stream_handle = 7u32;
        dispatcher
            .stream_files
            .insert(stream_handle, std::fs::File::open(&path).unwrap());

        let mem = SliceMemory::with_size(0x10000);
        let buf_ptr = 0x4000u32;

        // First read: engine supplies a fresh buffer; expect ok(list) = [0, buf_ptr, 6].
        dispatcher.set_read_buffer_allocation(buf_ptr, 64);
        let r1 = dispatcher.dispatch_core(
            "wasi:io/streams",
            "[method]input-stream.blocking-read",
            &[
                kiln_foundation::values::Value::I32(stream_handle as i32),
                kiln_foundation::values::Value::I64(32),
            ],
            Some(&mem),
        )?;
        assert_eq!(r1.len(), 3, "ok(list<u8>) lowers to [disc, ptr, len]");
        assert!(
            matches!(r1[0], kiln_foundation::values::Value::I32(0)),
            "result::ok"
        );
        assert!(
            matches!(r1[1], kiln_foundation::values::Value::I32(p) if p as u32 == buf_ptr),
            "list ptr is the engine-supplied buffer"
        );
        assert!(
            matches!(r1[2], kiln_foundation::values::Value::I32(6)),
            "6 bytes of CANARY"
        );
        let mut got = [0u8; 6];
        mem.read_bytes(buf_ptr, &mut got)?;
        assert_eq!(&got, b"CANARY", "file bytes written into the owned buffer");

        // Second read: offset now at EOF → stream-error::closed = [1, 1].
        dispatcher.set_read_buffer_allocation(0x5000, 64);
        let r2 = dispatcher.dispatch_core(
            "wasi:io/streams",
            "[method]input-stream.blocking-read",
            &[
                kiln_foundation::values::Value::I32(stream_handle as i32),
                kiln_foundation::values::Value::I64(32),
            ],
            Some(&mem),
        )?;
        assert_eq!(r2.len(), 2, "err(stream-error) lowers to [disc, variant]");
        assert!(
            matches!(r2[0], kiln_foundation::values::Value::I32(1)),
            "result::err"
        );
        assert!(
            matches!(r2[1], kiln_foundation::values::Value::I32(1)),
            "stream-error::closed"
        );

        let _ = std::fs::remove_file(&path);
        Ok(())
    }

    /// SR-38 / #405: FAIL-LOUD — with data to return but no engine-supplied buffer,
    /// blocking-read errors rather than writing bytes to a fixed unowned address
    /// (the pre-fix behavior that corrupted the guest allocator → E07DA).
    // rivet: verifies SR-38
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn blocking_read_fails_loud_without_buffer() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let path = std::env::temp_dir().join("kiln_sr38_failloud.txt");
        std::fs::File::create(&path).unwrap().write_all(b"X").unwrap();
        let stream_handle = 8u32;
        dispatcher
            .stream_files
            .insert(stream_handle, std::fs::File::open(&path).unwrap());
        let mem = SliceMemory::with_size(0x1000);
        // NOTE: no set_read_buffer_allocation.
        let res = dispatcher.dispatch_core(
            "wasi:io/streams",
            "[method]input-stream.blocking-read",
            &[
                kiln_foundation::values::Value::I32(stream_handle as i32),
                kiln_foundation::values::Value::I64(32),
            ],
            Some(&mem),
        );
        assert!(
            res.is_err(),
            "must fail loud when the engine supplied no read buffer"
        );
        let _ = std::fs::remove_file(&path);
        Ok(())
    }

    /// SR-39 / #405: read-directory snapshots a directory's entries and
    /// read-directory-entry walks them, returning each name in a cabi_realloc'd
    /// buffer and ok(none) at the end — so a component's std::fs::read_dir works.
    // rivet: verifies SR-39
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn read_directory_enumerates_entries_then_none() -> Result<()> {
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;
        let mut dispatcher = WasiDispatcher::with_defaults()?;

        // Temp directory with two known files.
        let dir = std::env::temp_dir().join("kiln_sr39_dir");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::File::create(dir.join("a.txt")).unwrap().write_all(b"a").unwrap();
        std::fs::File::create(dir.join("b.txt")).unwrap().write_all(b"b").unwrap();

        // Register the directory as an open descriptor.
        let dir_fd = 9u32;
        dispatcher.fd_table.insert(
            dir_fd,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::PreopenDirectory(dir.clone()),
                read: true,
                write: false,
                file: None,
            },
        );

        let mem = SliceMemory::with_size(0x10000);

        // read-directory → ok(stream handle).
        let rd = dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.read-directory",
            &[kiln_foundation::values::Value::I32(dir_fd as i32)],
            Some(&mem),
        )?;
        assert!(
            matches!(rd[0], kiln_foundation::values::Value::I32(0)),
            "read-directory ok"
        );
        let stream = match rd[1] {
            kiln_foundation::values::Value::I32(h) => h as u32,
            _ => panic!("no handle"),
        };

        // Walk entries; each name lands in a fresh buffer.
        let mut names = Vec::new();
        let name_ptr = 0x6000u32;
        for _ in 0..2 {
            dispatcher.set_read_buffer_allocation(name_ptr, 256);
            let e = dispatcher.dispatch_core(
                "wasi:filesystem/types",
                "[method]directory-entry-stream.read-directory-entry",
                &[kiln_foundation::values::Value::I32(stream as i32)],
                Some(&mem),
            )?;
            assert_eq!(
                e.len(),
                5,
                "ok(some(record)) lowers to [ok, some, type, ptr, len]"
            );
            assert!(matches!(e[0], kiln_foundation::values::Value::I32(0)));
            assert!(matches!(e[1], kiln_foundation::values::Value::I32(1)));
            assert!(
                matches!(e[2], kiln_foundation::values::Value::I32(6)),
                "regular-file = 6"
            );
            let len = match e[4] {
                kiln_foundation::values::Value::I32(l) => l as usize,
                _ => 0,
            };
            let mut buf = vec![0u8; len];
            mem.read_bytes(name_ptr, &mut buf)?;
            names.push(String::from_utf8(buf).unwrap());
        }
        names.sort();
        assert_eq!(names, vec!["a.txt".to_string(), "b.txt".to_string()]);

        // Next call → ok(none).
        let end = dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]directory-entry-stream.read-directory-entry",
            &[kiln_foundation::values::Value::I32(stream as i32)],
            Some(&mem),
        )?;
        assert_eq!(end.len(), 2, "ok(none) lowers to [ok, none]");
        assert!(matches!(end[0], kiln_foundation::values::Value::I32(0)));
        assert!(
            matches!(end[1], kiln_foundation::values::Value::I32(0)),
            "none"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_dispatcher_creation() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let caps = WasiCapabilities::minimal()?;
        let dispatcher = WasiDispatcher::new(caps)?;
        // Verify resource manager was created (no hardcoded handles anymore)
        assert_eq!(dispatcher.resource_manager.resource_count(), 0);
        Ok(())
    }

    #[test]
    fn test_strip_version() {
        assert_eq!(
            WasiDispatcher::strip_version("wasi:cli/stdout@0.2.4"),
            "wasi:cli/stdout"
        );
        assert_eq!(
            WasiDispatcher::strip_version("wasi:cli/stdout"),
            "wasi:cli/stdout"
        );
    }

    // ================================================================
    // WASI filesystem sandbox-escape regression tests
    // SR-49 (#437) live-path containment, SR-51 (#419) symlink-safety,
    // SR-50 (#438) allow-list enforcement. These drive the REAL
    // `dispatch_core` path that kilnd uses.
    // ================================================================

    /// A unique temp directory path for an escape-test fixture (not created).
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn sandbox_fixture_dir(tag: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        std::env::temp_dir().join(format!(
            "kiln_fs_sandbox_{}_{}_{}",
            tag,
            std::process::id(),
            nanos
        ))
    }

    /// Invoke the live `dispatch_core` open-at handler with a guest path.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    fn core_open_at(
        dispatcher: &mut WasiDispatcher,
        mem: &kiln_foundation::traits::SliceMemory,
        dir_fd: u32,
        guest_path: &[u8],
    ) -> Result<Vec<kiln_foundation::values::Value>> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::values::Value as CV;
        let path_ptr = 0x1000u32;
        mem.write_bytes(path_ptr, guest_path)?;
        dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.open-at",
            &[
                CV::I32(dir_fd as i32),
                CV::I32(0), // path-flags
                CV::I32(path_ptr as i32),
                CV::I32(guest_path.len() as i32),
                CV::I32(0), // open-flags
                CV::I32(0), // descriptor-flags
            ],
            Some(mem),
        )
    }

    /// SR-49 / #437: the live `dispatch_core` open-at handler must DENY a plain
    /// lexical `../` escape out of the preopen (no symlink needed). Before the
    /// fix the handler only did an `exists()` check, so `open-at("../SECRET.txt")`
    /// returned Ok(descriptor) for a file OUTSIDE the preopen.
    // rivet: verifies SR-49
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn open_at_denies_lexical_parent_escape_on_core_path() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;

        let root = sandbox_fixture_dir("openat_lexical");
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        // SECRET lives OUTSIDE the preopen.
        std::fs::File::create(root.join("SECRET.txt"))
            .unwrap()
            .write_all(b"TOPSECRET")
            .unwrap();

        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let dir_fd = dispatcher.add_preopen(sandbox.clone())?;

        let mem = SliceMemory::with_size(0x10000);
        let res = core_open_at(&mut dispatcher, &mem, dir_fd, b"../SECRET.txt");

        std::fs::remove_dir_all(&root).ok();
        assert!(
            res.is_err(),
            "open-at must DENY a `../` escape out of the preopen (SR-49), got {:?}",
            res
        );
        Ok(())
    }

    /// SR-49 / #437: a legitimate open of a file INSIDE the preopen must still
    /// succeed after the containment gate is added.
    // rivet: verifies SR-49
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn open_at_allows_file_inside_preopen_on_core_path() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;

        let root = sandbox_fixture_dir("openat_inside");
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::fs::File::create(sandbox.join("inside.txt"))
            .unwrap()
            .write_all(b"OK")
            .unwrap();

        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let dir_fd = dispatcher.add_preopen(sandbox.clone())?;

        let mem = SliceMemory::with_size(0x10000);
        let res = core_open_at(&mut dispatcher, &mem, dir_fd, b"inside.txt")?;

        std::fs::remove_dir_all(&root).ok();
        assert!(
            matches!(res.first(), Some(CV::I32(0))),
            "open-at inside the preopen must succeed (result::ok), got {:?}",
            res
        );
        Ok(())
    }

    /// SR-49 / #437: the live `dispatch_core` stat-at handler must DENY a `../`
    /// escape. Before the fix it hit `std::fs::metadata(base.join(guest))` with
    /// no containment check, leaking metadata for files outside the preopen.
    // rivet: verifies SR-49
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn stat_at_denies_lexical_parent_escape_on_core_path() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;

        let root = sandbox_fixture_dir("statat_lexical");
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::fs::File::create(root.join("SECRET.txt"))
            .unwrap()
            .write_all(b"TOPSECRET")
            .unwrap();

        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let dir_fd = dispatcher.add_preopen(sandbox.clone())?;

        let mem = SliceMemory::with_size(0x10000);
        let path_ptr = 0x1000u32;
        let guest = b"../SECRET.txt";
        mem.write_bytes(path_ptr, guest)?;
        let res = dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat-at",
            &[
                CV::I32(dir_fd as i32),
                CV::I32(0),
                CV::I32(path_ptr as i32),
                CV::I32(guest.len() as i32),
            ],
            Some(&mem),
        );

        std::fs::remove_dir_all(&root).ok();
        assert!(
            res.is_err(),
            "stat-at must DENY a `../` escape out of the preopen (SR-49), got {:?}",
            res
        );
        Ok(())
    }

    /// SR-51 / #419: a symlink INSIDE the preopen that points OUTSIDE must not be
    /// followed. Lexical containment alone (the pre-fix `is_within_sandbox`)
    /// admits it because the joined path has no `..`. The gate must reject on the
    /// OS-resolved real path.
    // rivet: verifies SR-51
    #[cfg(all(feature = "wasi-filesystem", feature = "std", unix))]
    #[serial_test::serial]
    #[test]
    fn open_at_denies_symlink_escape_on_core_path() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;

        let root = sandbox_fixture_dir("openat_symlink");
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        let secret = root.join("SECRET.txt");
        std::fs::File::create(&secret).unwrap().write_all(b"TOPSECRET").unwrap();
        // A symlink inside the preopen pointing OUTSIDE it.
        std::os::unix::fs::symlink(&secret, sandbox.join("link")).unwrap();

        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let dir_fd = dispatcher.add_preopen(sandbox.clone())?;

        let mem = SliceMemory::with_size(0x10000);
        let res = core_open_at(&mut dispatcher, &mem, dir_fd, b"link");

        std::fs::remove_dir_all(&root).ok();
        assert!(
            res.is_err(),
            "open-at must DENY a symlink that escapes the preopen (SR-51), got {:?}",
            res
        );
        Ok(())
    }

    /// SR-50 / #438: when a non-empty allow-list is configured, a path that is
    /// inside the preopen but NOT under any allowed path must be DENIED, while a
    /// path under an allowed path is permitted. Before the fix `is_path_allowed`
    /// had zero call sites, so the allow-list was silently unenforced.
    // rivet: verifies SR-50
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn open_at_enforces_nonempty_allowlist_on_core_path() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        use std::io::Write as _;
        MemoryInitializer::ensure_initialized()?;

        let root = sandbox_fixture_dir("openat_allowlist");
        let sandbox = root.join("sandbox");
        let allowed = sandbox.join("allowed");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::File::create(allowed.join("ok.txt")).unwrap().write_all(b"OK").unwrap();
        std::fs::File::create(sandbox.join("blocked.txt"))
            .unwrap()
            .write_all(b"NO")
            .unwrap();

        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let dir_fd = dispatcher.add_preopen(sandbox.clone())?;
        // Restrict to the `allowed/` subtree only (non-empty allow-list).
        dispatcher.add_allowed_path(&allowed)?;

        let mem = SliceMemory::with_size(0x10000);

        // Inside the preopen but outside the allow-list → DENIED.
        let blocked = core_open_at(&mut dispatcher, &mem, dir_fd, b"blocked.txt");
        // Under the allow-list → permitted.
        let ok = core_open_at(&mut dispatcher, &mem, dir_fd, b"allowed/ok.txt")?;

        std::fs::remove_dir_all(&root).ok();
        assert!(
            blocked.is_err(),
            "open-at outside the non-empty allow-list must be DENIED (SR-50), got {:?}",
            blocked
        );
        assert!(
            matches!(ok.first(), Some(CV::I32(0))),
            "open-at under the allow-list must succeed, got {:?}",
            ok
        );
        Ok(())
    }

    /// The existing filesystem arms must report the `wasi:filesystem/types`
    /// `error-code` enum indices from the WIT (declaration order): bad-descriptor=3,
    /// io=13, no-entry=20. They previously used 8 / 17 / 12, which are different codes
    /// (file-too-large / unknown / invalid).
    // rivet: verifies SR-72
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn existing_fs_arms_report_wit_error_code_indices() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        MemoryInitializer::ensure_initialized()?;

        let root = sandbox_fixture_dir("wit_error_codes");
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::fs::write(sandbox.join("gone.txt"), b"x").unwrap();

        let mut dispatcher = WasiDispatcher::with_defaults()?;
        let dir_fd = dispatcher.add_preopen(sandbox.clone())?;
        let mem = SliceMemory::with_size(0x10000);

        let code = |res: Result<Vec<CV>>| match res.as_deref() {
            Ok([CV::I32(1), CV::I32(c)]) => Some(*c),
            _ => None,
        };

        // open-at / stat-at of a missing entry: no-entry (20).
        let open_missing = code(core_open_at(&mut dispatcher, &mem, dir_fd, b"missing.txt"));
        let guest = b"missing.txt";
        mem.write_bytes(0x1000, guest)?;
        let stat_at_missing = code(dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat-at",
            &[
                CV::I32(dir_fd as i32),
                CV::I32(0),
                CV::I32(0x1000),
                CV::I32(guest.len() as i32),
            ],
            Some(&mem),
        ));
        // open-at relative to a descriptor that is not a preopen: bad-descriptor (3).
        let open_bad_fd = code(core_open_at(&mut dispatcher, &mem, 9999, b"x"));
        // stat of a descriptor whose file vanished: no-entry (20), not a catch-all.
        let fd = match core_open_at(&mut dispatcher, &mem, dir_fd, b"gone.txt")?.as_slice() {
            [CV::I32(0), CV::I32(h)] => *h as u32,
            other => panic!("open-at must succeed, got {:?}", other),
        };
        std::fs::remove_file(sandbox.join("gone.txt")).unwrap();
        let stat_vanished = code(dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat",
            &[CV::I32(fd as i32), CV::I32(0x100)],
            Some(&mem),
        ));
        // read-directory on an unknown descriptor: bad-descriptor (3).
        let readdir_bad = code(dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.read-directory",
            &[CV::I32(9999), CV::I32(0x100)],
            Some(&mem),
        ));
        // read-directory on a path that cannot be listed (a file): not-directory (24).
        std::fs::write(sandbox.join("plain.txt"), b"x").unwrap();
        let plain = match core_open_at(&mut dispatcher, &mem, dir_fd, b"plain.txt")?.as_slice() {
            [CV::I32(0), CV::I32(h)] => *h as u32,
            other => panic!("open-at must succeed, got {:?}", other),
        };
        let readdir_file = code(dispatcher.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.read-directory",
            &[CV::I32(plain as i32), CV::I32(0x100)],
            Some(&mem),
        ));
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(open_missing, Some(20), "open-at missing -> no-entry");
        assert_eq!(stat_at_missing, Some(20), "stat-at missing -> no-entry");
        assert_eq!(open_bad_fd, Some(3), "open-at bad dir fd -> bad-descriptor");
        assert_eq!(stat_vanished, Some(20), "stat of vanished file -> no-entry");
        assert_eq!(
            readdir_bad,
            Some(3),
            "read-directory bad fd -> bad-descriptor"
        );
        assert_eq!(
            readdir_file,
            Some(24),
            "read-directory on a file -> not-directory"
        );
        Ok(())
    }

    /// SR-72 / #552: descriptor read/write/sync-data and the directory-mutation
    /// calls the released Go hello world imports, driven through the live
    /// `dispatch_core` path with real tempdir preopens.
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    mod sr72 {
        use super::*;
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;

        const FS: &str = "wasi:filesystem/types";
        // error-code enum indices from wasi:filesystem/types@0.2.x
        const BAD_DESCRIPTOR: i32 = 3;
        const IS_DIRECTORY: i32 = 14;
        const NOT_PERMITTED: i32 = 31;

        fn fixture(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
            let root = sandbox_fixture_dir(tag);
            let sandbox = root.join("sandbox");
            std::fs::create_dir_all(&sandbox).unwrap();
            (root, sandbox)
        }

        /// Write `s` into guest memory at `ptr`, returning the (ptr, len) core pair.
        fn put(mem: &SliceMemory, ptr: u32, s: &[u8]) -> [CV; 2] {
            mem.write_bytes(ptr, s).unwrap();
            [CV::I32(ptr as i32), CV::I32(s.len() as i32)]
        }

        fn call(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            func: &str,
            args: &[CV],
        ) -> Result<Vec<CV>> {
            d.dispatch_core(FS, func, args, Some(mem))
        }

        /// Single-path calls: (self, path ptr, path len, retptr).
        fn path_call(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            func: &str,
            fd: u32,
            path: &[u8],
        ) -> Result<Vec<CV>> {
            let [p, l] = put(mem, 0x1000, path);
            call(d, mem, func, &[CV::I32(fd as i32), p, l, CV::I32(0x100)])
        }

        fn rename(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            fd: u32,
            old: &[u8],
            new_fd: u32,
            new: &[u8],
        ) -> Result<Vec<CV>> {
            let [op, ol] = put(mem, 0x1000, old);
            let [np, nl] = put(mem, 0x2000, new);
            call(
                d,
                mem,
                "[method]descriptor.rename-at",
                &[
                    CV::I32(fd as i32),
                    op,
                    ol,
                    CV::I32(new_fd as i32),
                    np,
                    nl,
                    CV::I32(0x100),
                ],
            )
        }

        fn link(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            fd: u32,
            flags: i32,
            old: &[u8],
            new_fd: u32,
            new: &[u8],
        ) -> Result<Vec<CV>> {
            let [op, ol] = put(mem, 0x1000, old);
            let [np, nl] = put(mem, 0x2000, new);
            call(
                d,
                mem,
                "[method]descriptor.link-at",
                &[
                    CV::I32(fd as i32),
                    CV::I32(flags),
                    op,
                    ol,
                    CV::I32(new_fd as i32),
                    np,
                    nl,
                    CV::I32(0x100),
                ],
            )
        }

        fn symlink(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            fd: u32,
            contents: &[u8],
            new: &[u8],
        ) -> Result<Vec<CV>> {
            let [op, ol] = put(mem, 0x1000, contents);
            let [np, nl] = put(mem, 0x2000, new);
            call(
                d,
                mem,
                "[method]descriptor.symlink-at",
                &[CV::I32(fd as i32), op, ol, np, nl, CV::I32(0x100)],
            )
        }

        /// open-at an existing file inside the preopen, returning its descriptor.
        fn open_file(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            dir: u32,
            name: &[u8],
        ) -> Result<u32> {
            open_file_with(d, mem, dir, name, DESC_READ | DESC_WRITE)
        }

        // descriptor-flags bits
        const DESC_READ: i32 = 1;
        const DESC_WRITE: i32 = 2;

        /// open-at with explicit descriptor-flags.
        fn open_with_flags(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            dir: u32,
            name: &[u8],
            desc_flags: i32,
        ) -> Result<Vec<CV>> {
            let [p, l] = put(mem, 0x1000, name);
            call(
                d,
                mem,
                "[method]descriptor.open-at",
                &[
                    CV::I32(dir as i32),
                    CV::I32(0),
                    p,
                    l,
                    CV::I32(0),
                    CV::I32(desc_flags),
                ],
            )
        }

        fn open_file_with(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            dir: u32,
            name: &[u8],
            desc_flags: i32,
        ) -> Result<u32> {
            let res = open_with_flags(d, mem, dir, name, desc_flags)?;
            match res.as_slice() {
                [CV::I32(0), CV::I32(h)] => Ok(*h as u32),
                other => panic!("open-at must succeed, got {:?}", other),
            }
        }

        /// open-at must refuse a directory component swapped for a symlink to an
        /// outside directory AFTER authorization but BEFORE the open (checked on the
        /// opened handle, not the path).
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn open_at_refuses_intermediate_directory_swapped_for_symlink() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_dirswap");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("f.txt"), b"OUTSIDE-SECRET").unwrap();
            std::fs::create_dir_all(sandbox.join("sub")).unwrap();
            std::fs::write(sandbox.join("sub/f.txt"), b"inside").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);

            let (sb, out) = (sandbox.clone(), outside.clone());
            BEFORE_OPEN_HOOK.with(|h| {
                *h.borrow_mut() = Some(Box::new(move || {
                    std::fs::rename(sb.join("sub"), sb.join("sub.orig")).unwrap();
                    std::os::unix::fs::symlink(&out, sb.join("sub")).unwrap();
                }));
            });
            let res = open_with_flags(&mut d, &mem, dir, b"sub/f.txt", DESC_READ);
            BEFORE_OPEN_HOOK.with(|h| *h.borrow_mut() = None);
            std::fs::remove_dir_all(&root).ok();
            assert!(
                res.is_err(),
                "swapped intermediate directory must be refused, got {:?}",
                res
            );
            Ok(())
        }

        /// Open with write ONLY when the guest's descriptor-flags request it: a
        /// read-only (0444) file opened for reading must succeed and read, and a
        /// write on that descriptor is `bad-descriptor`.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn open_at_opens_read_only_file_for_reading_and_refuses_write() -> Result<()> {
            use std::os::unix::fs::PermissionsExt as _;
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_ro_file");
            let f = sandbox.join("ro.txt");
            std::fs::write(&f, b"readonly").unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o444)).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file_with(&mut d, &mem, dir, b"ro.txt", DESC_READ)?;
            d.set_read_buffer_allocation(0x4000, 64);
            let read = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(64), CV::I64(0), CV::I32(0x100)],
            );
            let [wp, wl] = put(&mem, 0x3000, b"X");
            let write = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(fd as i32), wp, wl, CV::I64(0), CV::I32(0x100)],
            );
            // Requested read+write on a file the OS only lets us read: a host io error
            // from open-at, not a silent downgrade.
            let rw = open_with_flags(&mut d, &mem, dir, b"ro.txt", DESC_READ | DESC_WRITE);
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).ok();
            std::fs::remove_dir_all(&root).ok();
            assert!(is_ok(&read), "got {:?}", read);
            let mut got = [0u8; 8];
            mem.read_bytes(0x4000, &mut got)?;
            assert_eq!(&got, b"readonly");
            assert_eq!(err_code(&write), Some(BAD_DESCRIPTOR));
            // (root bypasses file modes, so only assert the refusal shape when it errors)
            if let Ok(v) = &rw {
                assert!(matches!(v.first(), Some(CV::I32(1)) | Some(CV::I32(0))));
            }
            Ok(())
        }

        /// A write-only open must not be readable (flags ∩ capabilities).
        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn open_at_descriptor_flags_decide_read_and_write() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_flags");
            std::fs::write(sandbox.join("f.txt"), b"abc").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let wo = open_file_with(&mut d, &mem, dir, b"f.txt", DESC_WRITE)?;
            d.set_read_buffer_allocation(0x4000, 16);
            let read = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(wo as i32), CV::I64(3), CV::I64(0), CV::I32(0x100)],
            );
            let [wp, wl] = put(&mem, 0x3000, b"Z");
            let write = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(wo as i32), wp, wl, CV::I64(0), CV::I32(0x100)],
            );
            // Capability off: a requested write is not granted.
            d.capabilities.filesystem.write_access = false;
            let rw = open_file_with(&mut d, &mem, dir, b"f.txt", DESC_READ | DESC_WRITE)?;
            let [wp, wl] = put(&mem, 0x3000, b"Q");
            let denied = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(rw as i32), wp, wl, CV::I64(0), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&read), Some(BAD_DESCRIPTOR));
            assert!(is_ok(&write));
            assert_eq!(err_code(&denied), Some(BAD_DESCRIPTOR));
            Ok(())
        }

        /// open-at on a FIFO must never block the runtime.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn open_at_refuses_fifo_without_blocking() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_fifo");
            let status = std::process::Command::new("mkfifo")
                .arg(sandbox.join("pipe"))
                .status()
                .expect("mkfifo must be available");
            assert!(status.success());
            let (tx, rx) = std::sync::mpsc::channel();
            let sb = sandbox.clone();
            std::thread::spawn(move || {
                let run = || -> Result<Vec<CV>> {
                    let mut d = WasiDispatcher::with_defaults()?;
                    let dir = d.add_preopen(sb)?;
                    let mem = SliceMemory::with_size(0x10000);
                    open_with_flags(&mut d, &mem, dir, b"pipe", DESC_READ)
                };
                tx.send(run().ok()).ok();
            });
            let outcome = rx.recv_timeout(std::time::Duration::from_secs(5));
            std::fs::remove_dir_all(&root).ok();
            match outcome {
                Ok(Some(res)) => {
                    assert_eq!(res.as_slice(), &[CV::I32(1), CV::I32(NOT_PERMITTED)][..])
                },
                Ok(None) => panic!("dispatch failed"),
                Err(_) => panic!("open-at on a FIFO blocked the runtime"),
            }
            Ok(())
        }

        /// SR-38 input-stream read uses the kept handle: a path swapped for a
        /// symlink after open-at/read-via-stream must not redirect it.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn input_stream_read_uses_the_kept_handle_not_the_path() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_stream_swap");
            let outside = root.join("OUTSIDE.txt");
            std::fs::write(&outside, b"OUTSIDE-SECRET").unwrap();
            std::fs::write(sandbox.join("f.txt"), b"original").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file_with(&mut d, &mem, dir, b"f.txt", DESC_READ)?;
            let stream = match call(
                &mut d,
                &mem,
                "[method]descriptor.read-via-stream",
                &[CV::I32(fd as i32), CV::I64(0), CV::I32(0x100)],
            )?
            .as_slice()
            {
                [CV::I32(0), CV::I32(h)] => *h as u32,
                other => panic!("read-via-stream must succeed, got {:?}", other),
            };
            std::fs::rename(sandbox.join("f.txt"), sandbox.join("f.orig")).unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("f.txt")).unwrap();

            d.set_read_buffer_allocation(0x4000, 64);
            let res = d.dispatch_core(
                "wasi:io/streams",
                "[method]input-stream.blocking-read",
                &[CV::I32(stream as i32), CV::I64(64)],
                Some(&mem),
            )?;
            let mut got = [0u8; 8];
            mem.read_bytes(0x4000, &mut got)?;
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(res, vec![CV::I32(0), CV::I32(0x4000), CV::I32(8)]);
            assert_eq!(&got, b"original", "stream read must hit the ORIGINAL inode");
            Ok(())
        }

        fn drop_descriptor(d: &mut WasiDispatcher, mem: &SliceMemory, h: u32) -> Result<Vec<CV>> {
            d.dispatch_core(
                FS,
                "[resource-drop]descriptor",
                &[CV::I32(h as i32)],
                Some(mem),
            )
        }

        fn drop_stream(d: &mut WasiDispatcher, mem: &SliceMemory, h: u32) -> Result<Vec<CV>> {
            d.dispatch_core(
                "wasi:io/streams",
                "[resource-drop]input-stream",
                &[CV::I32(h as i32)],
                Some(mem),
            )
        }

        fn stream_of(d: &mut WasiDispatcher, mem: &SliceMemory, fd: u32) -> Result<Vec<CV>> {
            call(
                d,
                mem,
                "[method]descriptor.read-via-stream",
                &[CV::I32(fd as i32), CV::I64(0), CV::I32(0x100)],
            )
        }

        /// Dropping the input-stream releases the duplicated File and the offset; a
        /// read on the dropped stream sees a closed stream.
        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn input_stream_drop_releases_the_duplicated_handle() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_drop_stream");
            std::fs::write(sandbox.join("f.txt"), b"hello").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            let stream = match stream_of(&mut d, &mem, fd)?.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h as u32,
                other => panic!("read-via-stream must succeed, got {:?}", other),
            };
            // One successful read so a stream offset exists.
            d.set_read_buffer_allocation(0x4000, 2);
            call_stream_read(&mut d, &mem, stream)?;
            assert!(d.stream_files.contains_key(&stream) && d.stream_offsets.contains_key(&stream));

            drop_stream(&mut d, &mem, stream)?;
            let files_gone = !d.stream_files.contains_key(&stream);
            let offsets_gone = !d.stream_offsets.contains_key(&stream);
            d.set_read_buffer_allocation(0x4000, 16);
            let after = call_stream_read(&mut d, &mem, stream)?;
            std::fs::remove_dir_all(&root).ok();
            assert!(
                files_gone,
                "stream_files entry must be removed (File closed)"
            );
            assert!(offsets_gone, "stream_offsets entry must be removed");
            // stream-error::closed
            assert_eq!(after, vec![CV::I32(1), CV::I32(1)]);
            Ok(())
        }

        fn call_stream_read(
            d: &mut WasiDispatcher,
            mem: &SliceMemory,
            stream: u32,
        ) -> Result<Vec<CV>> {
            d.dispatch_core(
                "wasi:io/streams",
                "[method]input-stream.blocking-read",
                &[CV::I32(stream as i32), CV::I64(2)],
                Some(mem),
            )
        }

        /// Dropping a descriptor removes its table entry (closing the kept File); a
        /// read on it is bad-descriptor. A preopen's handle is never removed.
        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn descriptor_drop_releases_the_kept_file_but_never_a_preopen() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_drop_desc");
            std::fs::write(sandbox.join("f.txt"), b"hello").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            assert!(d.fd_table.contains_key(&fd));

            drop_descriptor(&mut d, &mem, fd)?;
            let entry_gone = !d.fd_table.contains_key(&fd);
            d.set_read_buffer_allocation(0x4000, 16);
            let read = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(5), CV::I64(0), CV::I32(0x100)],
            );

            // Dropping the preopen's handle must leave it usable.
            drop_descriptor(&mut d, &mem, dir)?;
            let preopen_kept = d.fd_table.contains_key(&dir) && d.preopen_base(dir).is_some();
            let reopen = open_with_flags(&mut d, &mem, dir, b"f.txt", DESC_READ);
            std::fs::remove_dir_all(&root).ok();
            assert!(entry_gone, "fd_table entry must be removed (File closed)");
            assert_eq!(err_code(&read), Some(BAD_DESCRIPTOR));
            assert!(preopen_kept, "a preopen entry must never be removed");
            assert!(is_ok(&reopen), "got {:?}", reopen);
            Ok(())
        }

        /// Dropping a directory-entry-stream removes its snapshot and frees its
        /// resource slot, so repeated read-directory calls cannot exhaust the table.
        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn directory_entry_stream_drop_releases_state_and_slot() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_drop_dirstream");
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let read_dir = |d: &mut WasiDispatcher| {
                call(
                    d,
                    &mem,
                    "[method]descriptor.read-directory",
                    &[CV::I32(dir as i32), CV::I32(0x100)],
                )
            };
            let stream = match read_dir(&mut d)?.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h as u32,
                other => panic!("read-directory must succeed, got {:?}", other),
            };
            assert!(d.dir_streams.contains_key(&stream));
            let before = d.resource_manager.resource_count();

            d.dispatch_core(
                FS,
                "[resource-drop]directory-entry-stream",
                &[CV::I32(stream as i32)],
                Some(&mem),
            )?;
            let state_gone = !d.dir_streams.contains_key(&stream);
            let after = d.resource_manager.resource_count();

            // Cycle far past the 256-slot cap: only possible if drops free slots.
            let mut all_ok = true;
            for _ in 0..600 {
                match read_dir(&mut d) {
                    Ok(v) => match v.as_slice() {
                        [CV::I32(0), CV::I32(h)] => {
                            d.dispatch_core(
                                FS,
                                "[resource-drop]directory-entry-stream",
                                &[CV::I32(*h)],
                                Some(&mem),
                            )?;
                        },
                        _ => all_ok = false,
                    },
                    Err(_) => all_ok = false,
                }
            }
            std::fs::remove_dir_all(&root).ok();
            assert!(state_gone, "dir_streams entry must be removed");
            assert_eq!(after, before - 1, "the resource slot must be freed");
            assert!(
                all_ok,
                "read-directory/drop cycles must never exhaust the table"
            );
            Ok(())
        }

        /// A drop frees only a handle of the dropped resource's own type: dropping a
        /// preopen descriptor's handle through a stream or directory-entry-stream drop
        /// must not free the preopen's slot (resource type confusion).
        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn stream_drops_never_free_a_handle_of_another_resource_type() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_drop_confusion");
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            assert!(
                d.resource_manager.is_valid_handle(dir),
                "preopen must own a slot"
            );
            let before = d.resource_manager.resource_count();

            d.dispatch_core(
                FS,
                "[resource-drop]directory-entry-stream",
                &[CV::I32(dir as i32)],
                Some(&mem),
            )?;
            d.dispatch_core(
                "wasi:io/streams@0.2.0",
                "[resource-drop]input-stream",
                &[CV::I32(dir as i32)],
                Some(&mem),
            )?;
            d.dispatch_core(
                "wasi:io/streams@0.2.0",
                "[resource-drop]output-stream",
                &[CV::I32(dir as i32)],
                Some(&mem),
            )?;

            let still_valid = d.resource_manager.is_valid_handle(dir);
            let after = d.resource_manager.resource_count();
            std::fs::remove_dir_all(&root).ok();
            assert!(
                still_valid,
                "a stream drop must not free the preopen's slot"
            );
            assert_eq!(
                after, before,
                "no slot may be freed by a drop of another type"
            );
            Ok(())
        }

        /// Handles count toward the resource cap: opening past it is refused, and a
        /// drop frees a slot again.
        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn open_and_stream_handles_are_capped_and_drops_free_slots() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_cap");
            std::fs::write(sandbox.join("f.txt"), b"hello").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);

            // Descriptors: open until refused.
            let mut opened = Vec::new();
            let mut refused = false;
            for _ in 0..1000 {
                match open_with_flags(&mut d, &mem, dir, b"f.txt", DESC_READ) {
                    Ok(v) => match v.as_slice() {
                        [CV::I32(0), CV::I32(h)] => opened.push(*h as u32),
                        other => panic!("unexpected {:?}", other),
                    },
                    Err(_) => {
                        refused = true;
                        break;
                    },
                }
            }
            assert!(refused, "opening past the resource cap must be refused");
            assert!(opened.len() <= 256);

            // Free two slots, then fill them with streams until refused.
            let keep = opened[0];
            drop_descriptor(&mut d, &mem, opened[1])?;
            drop_descriptor(&mut d, &mem, opened[2])?;
            let mut streams = Vec::new();
            let mut stream_refused = false;
            for _ in 0..10 {
                match stream_of(&mut d, &mem, keep) {
                    Ok(v) => match v.as_slice() {
                        [CV::I32(0), CV::I32(h)] => streams.push(*h as u32),
                        other => panic!("unexpected {:?}", other),
                    },
                    Err(_) => {
                        stream_refused = true;
                        break;
                    },
                }
            }
            let stream_files = d.stream_files.len();
            assert_eq!(streams.len(), 2, "exactly the two freed slots are usable");
            assert!(
                stream_refused,
                "stream handles past the cap must be refused"
            );
            assert_eq!(
                stream_files, 2,
                "a refused stream must not leave a File behind"
            );

            // Dropping a stream frees its slot.
            drop_stream(&mut d, &mem, streams[0])?;
            let again = stream_of(&mut d, &mem, keep);
            std::fs::remove_dir_all(&root).ok();
            assert!(is_ok(&again), "got {:?}", again);
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_via_stream_refuses_descriptor_without_a_readable_handle() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_stream_nohandle");
            std::fs::write(sandbox.join("f.txt"), b"abc").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let write_only = open_file_with(&mut d, &mem, dir, b"f.txt", DESC_WRITE)?;
            let unknown = call(
                &mut d,
                &mem,
                "[method]descriptor.read-via-stream",
                &[CV::I32(9999), CV::I64(0), CV::I32(0x100)],
            );
            let wo = call(
                &mut d,
                &mem,
                "[method]descriptor.read-via-stream",
                &[CV::I32(write_only as i32), CV::I64(0), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&unknown), Some(BAD_DESCRIPTOR));
            assert_eq!(err_code(&wo), Some(BAD_DESCRIPTOR));
            Ok(())
        }

        fn is_ok(res: &Result<Vec<CV>>) -> bool {
            matches!(res.as_deref(), Ok([CV::I32(0), ..]))
        }

        fn err_code(res: &Result<Vec<CV>>) -> Option<i32> {
            match res.as_deref() {
                Ok([CV::I32(1), CV::I32(c)]) => Some(*c),
                _ => None,
            }
        }

        // ---------------- read ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_returns_bytes_into_preallocated_buffer_with_eof_flag() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_read");
            std::fs::write(sandbox.join("f.txt"), b"hello world").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;

            // Partial read: 5 bytes at offset 6 -> "world", not yet flagged eof by length alone
            // (offset 6 + 5 == file length, so eof is true).
            d.set_read_buffer_allocation(0x4000, 64);
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(5), CV::I64(6), CV::I32(0x100)],
            );
            assert_eq!(
                res.as_deref().ok(),
                Some(&[CV::I32(0), CV::I32(0x4000), CV::I32(5), CV::I32(1)][..]),
                "read must return ok(list ptr/len, eof) in the pre-allocated buffer"
            );
            let mut got = [0u8; 5];
            mem.read_bytes(0x4000, &mut got)?;
            assert_eq!(&got, b"world");

            // Short read: 3 bytes at offset 0 -> "hel", eof false.
            d.set_read_buffer_allocation(0x5000, 64);
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(3), CV::I64(0), CV::I32(0x100)],
            );
            assert_eq!(
                res.as_deref().ok(),
                Some(&[CV::I32(0), CV::I32(0x5000), CV::I32(3), CV::I32(0)][..])
            );

            // Past end of file: empty list, eof true.
            d.set_read_buffer_allocation(0x6000, 64);
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(8), CV::I64(100), CV::I32(0x100)],
            );
            assert_eq!(
                res.as_deref().ok(),
                Some(&[CV::I32(0), CV::I32(0x6000), CV::I32(0), CV::I32(1)][..])
            );
            std::fs::remove_dir_all(&root).ok();
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_clamps_to_preallocated_buffer_size_and_clears_eof() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_read_clamp");
            std::fs::write(sandbox.join("f.txt"), b"hello world").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;

            // Buffer holds only 4 bytes: never write past what the engine allocated.
            d.set_read_buffer_allocation(0x4000, 4);
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(100), CV::I64(0), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(
                res.as_deref().ok(),
                Some(&[CV::I32(0), CV::I32(0x4000), CV::I32(4), CV::I32(0)][..])
            );
            let mut guard = [0xAAu8; 4];
            mem.read_bytes(0x4004, &mut guard)?;
            assert_eq!(
                guard, [0u8; 4],
                "bytes past the allocation must be untouched"
            );
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_fails_loud_without_preallocated_buffer() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_read_nobuf");
            std::fs::write(sandbox.join("f.txt"), b"hello").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            // NOTE: no set_read_buffer_allocation.
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(5), CV::I64(0), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert!(
                res.is_err(),
                "read without an engine-allocated buffer must fail loud, got {:?}",
                res
            );
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_refuses_unknown_and_directory_descriptors() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_read_bad");
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            d.set_read_buffer_allocation(0x4000, 16);
            let unknown = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(9999), CV::I64(4), CV::I64(0), CV::I32(0x100)],
            );
            d.set_read_buffer_allocation(0x4000, 16);
            let on_dir = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(dir as i32), CV::I64(4), CV::I64(0), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&unknown), Some(BAD_DESCRIPTOR));
            assert_eq!(err_code(&on_dir), Some(IS_DIRECTORY));
            Ok(())
        }

        // ---------------- write ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn write_writes_at_offset_and_returns_count() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_write");
            std::fs::write(sandbox.join("f.txt"), b"abcdef").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            let [bp, bl] = put(&mem, 0x3000, b"XY");
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(fd as i32), bp, bl, CV::I64(1), CV::I32(0x100)],
            );
            let content = std::fs::read(sandbox.join("f.txt")).unwrap();
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(res.as_deref().ok(), Some(&[CV::I32(0), CV::I64(2)][..]));
            assert_eq!(content, b"aXYdef");
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn write_past_end_extends_file_with_zeros() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_write_ext");
            std::fs::write(sandbox.join("f.txt"), b"ab").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            let [bp, bl] = put(&mem, 0x3000, b"Z");
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(fd as i32), bp, bl, CV::I64(4), CV::I32(0x100)],
            );
            let content = std::fs::read(sandbox.join("f.txt")).unwrap();
            std::fs::remove_dir_all(&root).ok();
            assert!(is_ok(&res));
            assert_eq!(content, b"ab\0\0Z");
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn write_refused_when_descriptor_not_opened_for_writing() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_write_ro");
            std::fs::write(sandbox.join("f.txt"), b"abcdef").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            // Demote the descriptor to read-only.
            d.fd_table.get_mut(&fd).unwrap().write = false;
            let [bp, bl] = put(&mem, 0x3000, b"XY");
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(fd as i32), bp, bl, CV::I64(0), CV::I32(0x100)],
            );
            let content = std::fs::read(sandbox.join("f.txt")).unwrap();
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&res), Some(BAD_DESCRIPTOR));
            assert_eq!(
                content, b"abcdef",
                "a refused write must not touch the file"
            );
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn write_refuses_unknown_descriptor_and_out_of_bounds_buffer() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_write_bad");
            std::fs::write(sandbox.join("f.txt"), b"abc").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            let unknown = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[
                    CV::I32(9999),
                    CV::I32(0x3000),
                    CV::I32(1),
                    CV::I64(0),
                    CV::I32(0x100),
                ],
            );
            // A buffer length far beyond guest memory must fail, not allocate/write.
            let oob = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[
                    CV::I32(fd as i32),
                    CV::I32(0x3000),
                    CV::I32(-1),
                    CV::I64(0),
                    CV::I32(0x100),
                ],
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&unknown), Some(BAD_DESCRIPTOR));
            assert!(
                oob.is_err(),
                "oversized buffer must fail loud, got {:?}",
                oob
            );
            Ok(())
        }

        // ---------------- sync-data ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn sync_data_succeeds_on_open_file_and_refuses_unknown_descriptor() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_sync");
            std::fs::write(sandbox.join("f.txt"), b"abc").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            let ok = call(
                &mut d,
                &mem,
                "[method]descriptor.sync-data",
                &[CV::I32(fd as i32), CV::I32(0x100)],
            );
            let bad = call(
                &mut d,
                &mem,
                "[method]descriptor.sync-data",
                &[CV::I32(9999), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(ok.as_deref().ok(), Some(&[CV::I32(0)][..]));
            assert_eq!(err_code(&bad), Some(BAD_DESCRIPTOR));
            Ok(())
        }

        // ---------------- create-directory-at ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn create_directory_at_creates_inside_and_denies_lexical_escape() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_mkdir");
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let ok = path_call(
                &mut d,
                &mem,
                "[method]descriptor.create-directory-at",
                dir,
                b"newdir",
            );
            let esc = path_call(
                &mut d,
                &mem,
                "[method]descriptor.create-directory-at",
                dir,
                b"../evil",
            );
            let abs = path_call(
                &mut d,
                &mem,
                "[method]descriptor.create-directory-at",
                dir,
                b"/tmp/kiln_sr72_abs",
            );
            let inside = sandbox.join("newdir").is_dir();
            let escaped = root.join("evil").exists();
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(ok.as_deref().ok(), Some(&[CV::I32(0)][..]));
            assert!(inside);
            assert!(esc.is_err(), "`../` escape must be denied, got {:?}", esc);
            assert!(abs.is_err(), "absolute path must be denied, got {:?}", abs);
            assert!(
                !escaped,
                "a refused create-directory-at must never be performed"
            );
            assert!(!std::path::Path::new("/tmp/kiln_sr72_abs").exists());
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn create_directory_at_denies_symlink_escape() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_mkdir_sym");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("link")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(
                &mut d,
                &mem,
                "[method]descriptor.create-directory-at",
                dir,
                b"link/sub",
            );
            let escaped = outside.join("sub").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(res.is_err(), "symlink escape must be denied, got {:?}", res);
            assert!(!escaped);
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn create_directory_at_enforces_nonempty_allowlist() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_mkdir_allow");
            let allowed = sandbox.join("allowed");
            std::fs::create_dir_all(&allowed).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            d.add_allowed_path(&allowed)?;
            let mem = SliceMemory::with_size(0x10000);
            let blocked = path_call(
                &mut d,
                &mem,
                "[method]descriptor.create-directory-at",
                dir,
                b"blocked",
            );
            let ok = path_call(
                &mut d,
                &mem,
                "[method]descriptor.create-directory-at",
                dir,
                b"allowed/sub",
            );
            let created = sandbox.join("blocked").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(
                blocked.is_err(),
                "outside allow-list must be denied, got {:?}",
                blocked
            );
            assert!(!created);
            assert!(is_ok(&ok));
            Ok(())
        }

        // ---------------- remove-directory-at ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn remove_directory_at_removes_inside_and_denies_lexical_escape() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_rmdir");
            std::fs::create_dir_all(sandbox.join("gone")).unwrap();
            std::fs::create_dir_all(root.join("victim")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let esc = path_call(
                &mut d,
                &mem,
                "[method]descriptor.remove-directory-at",
                dir,
                b"../victim",
            );
            let ok = path_call(
                &mut d,
                &mem,
                "[method]descriptor.remove-directory-at",
                dir,
                b"gone",
            );
            let gone = !sandbox.join("gone").exists();
            let victim = root.join("victim").is_dir();
            std::fs::remove_dir_all(&root).ok();
            assert!(esc.is_err(), "`../` escape must be denied, got {:?}", esc);
            assert!(
                victim,
                "a refused remove-directory-at must never be performed"
            );
            assert_eq!(ok.as_deref().ok(), Some(&[CV::I32(0)][..]));
            assert!(gone);
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn remove_directory_at_denies_symlink_escape() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_rmdir_sym");
            let outside = root.join("outside");
            std::fs::create_dir_all(outside.join("victim")).unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("link")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(
                &mut d,
                &mem,
                "[method]descriptor.remove-directory-at",
                dir,
                b"link/victim",
            );
            let victim = outside.join("victim").is_dir();
            std::fs::remove_dir_all(&root).ok();
            assert!(res.is_err(), "symlink escape must be denied, got {:?}", res);
            assert!(victim);
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn remove_directory_at_reports_not_empty() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_rmdir_ne");
            std::fs::create_dir_all(sandbox.join("full")).unwrap();
            std::fs::write(sandbox.join("full/x"), b"x").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(
                &mut d,
                &mem,
                "[method]descriptor.remove-directory-at",
                dir,
                b"full",
            );
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&res), Some(25)); // not-empty
            Ok(())
        }

        // ---------------- unlink-file-at ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn unlink_file_at_removes_inside_and_denies_lexical_escape() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_unlink");
            std::fs::write(sandbox.join("gone.txt"), b"x").unwrap();
            std::fs::write(root.join("SECRET.txt"), b"s").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let esc = path_call(
                &mut d,
                &mem,
                "[method]descriptor.unlink-file-at",
                dir,
                b"../SECRET.txt",
            );
            let ok = path_call(
                &mut d,
                &mem,
                "[method]descriptor.unlink-file-at",
                dir,
                b"gone.txt",
            );
            let gone = !sandbox.join("gone.txt").exists();
            let secret = root.join("SECRET.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(esc.is_err(), "`../` escape must be denied, got {:?}", esc);
            assert!(secret, "a refused unlink-file-at must never be performed");
            assert_eq!(ok.as_deref().ok(), Some(&[CV::I32(0)][..]));
            assert!(gone);
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn unlink_file_at_denies_symlink_escape() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_unlink_sym");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("SECRET.txt"), b"s").unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("link")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(
                &mut d,
                &mem,
                "[method]descriptor.unlink-file-at",
                dir,
                b"link/SECRET.txt",
            );
            let secret = outside.join("SECRET.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(res.is_err(), "symlink escape must be denied, got {:?}", res);
            assert!(secret);
            Ok(())
        }

        /// Unlinking a symlink removes the LINK, never the file it points at.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn unlink_file_at_removes_the_symlink_not_its_target() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_unlink_link");
            std::fs::write(sandbox.join("target.txt"), b"t").unwrap();
            std::os::unix::fs::symlink("target.txt", sandbox.join("alias")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(
                &mut d,
                &mem,
                "[method]descriptor.unlink-file-at",
                dir,
                b"alias",
            );
            let link_gone = std::fs::symlink_metadata(sandbox.join("alias")).is_err();
            let target = sandbox.join("target.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(is_ok(&res));
            assert!(link_gone);
            assert!(target, "unlink of a symlink must not delete its target");
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn unlink_file_at_refuses_directory() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_unlink_dir");
            std::fs::create_dir_all(sandbox.join("d")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(&mut d, &mem, "[method]descriptor.unlink-file-at", dir, b"d");
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&res), Some(IS_DIRECTORY));
            Ok(())
        }

        // ---------------- rename-at ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn rename_at_renames_inside_and_checks_both_paths() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_rename");
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            std::fs::write(root.join("SECRET.txt"), b"s").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            // old path escapes
            let old_esc = rename(&mut d, &mem, dir, b"../SECRET.txt", dir, b"stolen.txt");
            // new path escapes
            let new_esc = rename(&mut d, &mem, dir, b"a.txt", dir, b"../a.txt");
            let ok = rename(&mut d, &mem, dir, b"a.txt", dir, b"b.txt");
            let renamed = sandbox.join("b.txt").exists() && !sandbox.join("a.txt").exists();
            let secret = root.join("SECRET.txt").exists();
            let stolen = sandbox.join("stolen.txt").exists();
            let leaked = root.join("a.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(
                old_esc.is_err(),
                "old path escape must be denied, got {:?}",
                old_esc
            );
            assert!(
                new_esc.is_err(),
                "new path escape must be denied, got {:?}",
                new_esc
            );
            assert!(
                secret && !stolen && !leaked,
                "refused renames must never be performed"
            );
            assert_eq!(ok.as_deref().ok(), Some(&[CV::I32(0)][..]));
            assert!(renamed);
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn rename_at_checks_new_path_against_its_own_descriptor() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_rename_two");
            let other = root.join("other");
            std::fs::create_dir_all(&other).unwrap();
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let other_dir = d.add_preopen(other.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            // `..` from the second preopen would land in `sandbox`'s sibling tree: denied.
            let esc = rename(&mut d, &mem, dir, b"a.txt", other_dir, b"../sandbox/x.txt");
            // plain cross-preopen rename is within both preopens: allowed.
            let ok = rename(&mut d, &mem, dir, b"a.txt", other_dir, b"moved.txt");
            let moved = other.join("moved.txt").exists();
            // new descriptor that is not a preopen: bad-descriptor, nothing performed.
            let bad = rename(&mut d, &mem, dir, b"moved.txt", 9999, b"x.txt");
            std::fs::remove_dir_all(&root).ok();
            assert!(esc.is_err(), "got {:?}", esc);
            assert!(is_ok(&ok));
            assert!(moved);
            assert_eq!(err_code(&bad), Some(BAD_DESCRIPTOR));
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn rename_at_denies_symlink_escape_on_either_path() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_rename_sym");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("SECRET.txt"), b"s").unwrap();
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("link")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let old_esc = rename(&mut d, &mem, dir, b"link/SECRET.txt", dir, b"x.txt");
            let new_esc = rename(&mut d, &mem, dir, b"a.txt", dir, b"link/a.txt");
            let secret = outside.join("SECRET.txt").exists();
            let dropped = outside.join("a.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(old_esc.is_err(), "got {:?}", old_esc);
            assert!(new_esc.is_err(), "got {:?}", new_esc);
            assert!(secret && !dropped);
            Ok(())
        }

        // ---------------- link-at ----------------

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn link_at_creates_hard_link_inside_and_checks_both_paths() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_link");
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            std::fs::write(root.join("SECRET.txt"), b"s").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let old_esc = link(&mut d, &mem, dir, 0, b"../SECRET.txt", dir, b"stolen.txt");
            let new_esc = link(&mut d, &mem, dir, 0, b"a.txt", dir, b"../a2.txt");
            let ok = link(&mut d, &mem, dir, 0, b"a.txt", dir, b"b.txt");
            let linked = std::fs::read(sandbox.join("b.txt")).ok();
            let stolen = sandbox.join("stolen.txt").exists();
            let leaked = root.join("a2.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(
                old_esc.is_err(),
                "old path escape must be denied, got {:?}",
                old_esc
            );
            assert!(
                new_esc.is_err(),
                "new path escape must be denied, got {:?}",
                new_esc
            );
            assert!(!stolen && !leaked, "refused links must never be performed");
            assert_eq!(ok.as_deref().ok(), Some(&[CV::I32(0)][..]));
            assert_eq!(linked.as_deref(), Some(&b"a"[..]));
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn link_at_denies_symlink_escape_and_never_links_a_symlink_without_follow() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_link_sym");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::fs::write(outside.join("SECRET.txt"), b"s").unwrap();
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("dirlink")).unwrap();
            std::os::unix::fs::symlink(outside.join("SECRET.txt"), sandbox.join("filelink"))
                .unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            // through a symlinked directory component
            let old_esc = link(&mut d, &mem, dir, 0, b"dirlink/SECRET.txt", dir, b"x.txt");
            let new_esc = link(&mut d, &mem, dir, 0, b"a.txt", dir, b"dirlink/a.txt");
            // final component is a symlink to an outside file, WITHOUT symlink-follow:
            // a symlink is not a file -> not-permitted, never linked.
            let no_follow = link(&mut d, &mem, dir, 0, b"filelink", dir, b"y.txt");
            // WITH symlink-follow it resolves outside the preopen -> denied.
            let follow = link(&mut d, &mem, dir, 1, b"filelink", dir, b"z.txt");
            let made = ["x.txt", "y.txt", "z.txt"].iter().any(|n| sandbox.join(n).exists());
            let dropped = outside.join("a.txt").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(old_esc.is_err(), "got {:?}", old_esc);
            assert!(new_esc.is_err(), "got {:?}", new_esc);
            assert_eq!(err_code(&no_follow), Some(NOT_PERMITTED));
            assert!(follow.is_err(), "got {:?}", follow);
            assert!(!made && !dropped);
            Ok(())
        }

        // ---------------- symlink-at ----------------

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn symlink_at_creates_relative_link_inside() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_symlink");
            std::fs::create_dir_all(sandbox.join("sub")).unwrap();
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let flat = symlink(&mut d, &mem, dir, b"a.txt", b"l1");
            let nested = symlink(&mut d, &mem, dir, b"sub/./x", b"l2");
            let t1 = std::fs::read_link(sandbox.join("l1")).ok();
            let t2 = std::fs::read_link(sandbox.join("l2")).ok();
            std::fs::remove_dir_all(&root).ok();
            assert!(
                is_ok(&flat) && is_ok(&nested),
                "got {:?} / {:?}",
                flat,
                nested
            );
            assert_eq!(t1, Some(std::path::PathBuf::from("a.txt")));
            assert_eq!(t2, Some(std::path::PathBuf::from("sub/./x")));
            Ok(())
        }

        /// Lexical `..` handling can disagree with the kernel when a component is
        /// itself a symlink, so ANY `..` component in the link contents is refused,
        /// even one that stays lexically inside the preopen.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn symlink_at_refuses_any_parent_dir_component_in_target() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_symlink_dotdot");
            std::fs::create_dir_all(sandbox.join("sub")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let inside = symlink(&mut d, &mem, dir, b"sub/../x", b"l1");
            let up_inside = symlink(&mut d, &mem, dir, b"../a.txt", b"sub/l2");
            let made = ["l1", "sub/l2"]
                .iter()
                .any(|n| std::fs::symlink_metadata(sandbox.join(n)).is_ok());
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&inside), Some(NOT_PERMITTED));
            assert_eq!(err_code(&up_inside), Some(NOT_PERMITTED));
            assert!(!made, "a refused symlink-at must never be performed");
            Ok(())
        }

        /// TOCTOU: read/write/sync-data must operate on the handle open-at opened,
        /// never reopen by path. A symlink swapped in afterwards must not redirect them.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn read_write_sync_use_the_opened_handle_not_the_path() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_toctou");
            let outside = root.join("OUTSIDE.txt");
            std::fs::write(&outside, b"OUTSIDE-SECRET").unwrap();
            std::fs::write(sandbox.join("f.txt"), b"original").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;

            // Swap the path for a symlink to a file outside the preopen.
            std::fs::rename(sandbox.join("f.txt"), sandbox.join("f.orig")).unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("f.txt")).unwrap();

            d.set_read_buffer_allocation(0x4000, 64);
            let read = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(64), CV::I64(0), CV::I32(0x100)],
            );
            let [wp, wl] = put(&mem, 0x3000, b"XX");
            let write = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(fd as i32), wp, wl, CV::I64(0), CV::I32(0x100)],
            );
            let sync = call(
                &mut d,
                &mem,
                "[method]descriptor.sync-data",
                &[CV::I32(fd as i32), CV::I32(0x100)],
            );
            let mut got = [0u8; 8];
            mem.read_bytes(0x4000, &mut got)?;
            let outside_now = std::fs::read(&outside).unwrap();
            let orig_now = std::fs::read(sandbox.join("f.orig")).unwrap();
            std::fs::remove_dir_all(&root).ok();

            assert!(is_ok(&read), "got {:?}", read);
            assert_eq!(&got, b"original", "read must hit the ORIGINAL inode");
            assert!(
                is_ok(&write) && is_ok(&sync),
                "got {:?} / {:?}",
                write,
                sync
            );
            assert_eq!(
                outside_now, b"OUTSIDE-SECRET",
                "the outside file must never be touched"
            );
            assert_eq!(
                orig_now, b"XXiginal",
                "write must land in the original inode"
            );
            Ok(())
        }

        /// A symlink already swapped in BEFORE open-at opens the file must not be
        /// followed out of the sandbox either.
        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn open_at_hands_out_no_handle_when_path_is_a_symlink() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_open_sym");
            std::fs::write(root.join("OUTSIDE.txt"), b"S").unwrap();
            std::os::unix::fs::symlink(root.join("OUTSIDE.txt"), sandbox.join("f.txt")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = core_open_at(&mut d, &mem, dir, b"f.txt");
            std::fs::remove_dir_all(&root).ok();
            assert!(
                res.is_err(),
                "symlink escape on open-at must be denied, got {:?}",
                res
            );
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_refuses_stream_handles_and_descriptors_without_an_opened_file() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_read_stream");
            std::fs::write(sandbox.join("f.txt"), b"hello").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            let stream = match call(
                &mut d,
                &mem,
                "[method]descriptor.read-via-stream",
                &[CV::I32(fd as i32), CV::I64(0), CV::I32(0x100)],
            )?
            .as_slice()
            {
                [CV::I32(0), CV::I32(h)] => *h as u32,
                other => panic!("read-via-stream must succeed, got {:?}", other),
            };
            d.set_read_buffer_allocation(0x4000, 16);
            let read = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[
                    CV::I32(stream as i32),
                    CV::I64(5),
                    CV::I64(0),
                    CV::I32(0x100),
                ],
            );
            let [wp, wl] = put(&mem, 0x3000, b"X");
            let write = call(
                &mut d,
                &mem,
                "[method]descriptor.write",
                &[CV::I32(stream as i32), wp, wl, CV::I64(0), CV::I32(0x100)],
            );
            let content = std::fs::read(sandbox.join("f.txt")).unwrap();
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&read), Some(BAD_DESCRIPTOR));
            assert_eq!(err_code(&write), Some(BAD_DESCRIPTOR));
            assert_eq!(content, b"hello");
            Ok(())
        }

        // rivet: verifies SR-72
        #[serial_test::serial]
        #[test]
        fn read_requires_the_read_capability() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_read_cap");
            std::fs::write(sandbox.join("f.txt"), b"hello").unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let fd = open_file(&mut d, &mem, dir, b"f.txt")?;
            d.capabilities.filesystem.read_access = false;
            d.set_read_buffer_allocation(0x4000, 16);
            let res = call(
                &mut d,
                &mem,
                "[method]descriptor.read",
                &[CV::I32(fd as i32), CV::I64(5), CV::I64(0), CV::I32(0x100)],
            );
            std::fs::remove_dir_all(&root).ok();
            assert!(
                res.is_err(),
                "read without read access must be refused, got {:?}",
                res
            );
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn readlink_at_requires_the_read_capability() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_readlink_cap");
            std::os::unix::fs::symlink("a.txt", sandbox.join("l")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            d.capabilities.filesystem.read_access = false;
            d.set_read_buffer_allocation(0x4000, 64);
            let res = path_call(&mut d, &mem, "[method]descriptor.readlink-at", dir, b"l");
            std::fs::remove_dir_all(&root).ok();
            assert!(
                res.is_err(),
                "readlink-at without read access must be refused, got {:?}",
                res
            );
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn symlink_at_refuses_absolute_and_escaping_targets() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_symlink_esc");
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let abs = symlink(&mut d, &mem, dir, b"/etc/passwd", b"l_abs");
            let up = symlink(&mut d, &mem, dir, b"../SECRET.txt", b"l_up");
            let deep = symlink(&mut d, &mem, dir, b"a/../../SECRET.txt", b"l_deep");
            let any = ["l_abs", "l_up", "l_deep"]
                .iter()
                .any(|n| std::fs::symlink_metadata(sandbox.join(n)).is_ok());
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(err_code(&abs), Some(NOT_PERMITTED));
            assert_eq!(err_code(&up), Some(NOT_PERMITTED));
            assert_eq!(err_code(&deep), Some(NOT_PERMITTED));
            assert!(!any, "a refused symlink-at must never be performed");
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn symlink_at_denies_new_path_escape_lexical_and_via_symlink() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_symlink_new");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("dirlink")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let lexical = symlink(&mut d, &mem, dir, b"a.txt", b"../l");
            let via_link = symlink(&mut d, &mem, dir, b"a.txt", b"dirlink/l");
            let made = root.join("l").exists() || outside.join("l").exists();
            std::fs::remove_dir_all(&root).ok();
            assert!(lexical.is_err(), "got {:?}", lexical);
            assert!(via_link.is_err(), "got {:?}", via_link);
            assert!(!made);
            Ok(())
        }

        // ---------------- readlink-at ----------------

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn readlink_at_returns_target_in_preallocated_buffer() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_readlink");
            std::fs::write(sandbox.join("a.txt"), b"a").unwrap();
            std::os::unix::fs::symlink("a.txt", sandbox.join("l")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            d.set_read_buffer_allocation(0x4000, 256);
            let res = path_call(&mut d, &mem, "[method]descriptor.readlink-at", dir, b"l");
            std::fs::remove_dir_all(&root).ok();
            assert_eq!(
                res.as_deref().ok(),
                Some(&[CV::I32(0), CV::I32(0x4000), CV::I32(5)][..])
            );
            let mut got = [0u8; 5];
            mem.read_bytes(0x4000, &mut got)?;
            assert_eq!(&got, b"a.txt");
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn readlink_at_denies_escape_and_refuses_absolute_or_oversized_targets() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_readlink_esc");
            let outside = root.join("outside");
            std::fs::create_dir_all(&outside).unwrap();
            std::os::unix::fs::symlink("/etc/passwd", outside.join("abs_in_outside")).unwrap();
            std::os::unix::fs::symlink("/etc/passwd", sandbox.join("abs")).unwrap();
            std::os::unix::fs::symlink("0123456789", sandbox.join("long")).unwrap();
            std::os::unix::fs::symlink(&outside, sandbox.join("dirlink")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let lexical = path_call(
                &mut d,
                &mem,
                "[method]descriptor.readlink-at",
                dir,
                b"../outside/abs_in_outside",
            );
            let via_link = path_call(
                &mut d,
                &mem,
                "[method]descriptor.readlink-at",
                dir,
                b"dirlink/abs_in_outside",
            );
            d.set_read_buffer_allocation(0x4000, 256);
            let abs = path_call(&mut d, &mem, "[method]descriptor.readlink-at", dir, b"abs");
            // 10-byte target into a 4-byte allocation: error, never truncate.
            d.set_read_buffer_allocation(0x4000, 4);
            let long = path_call(&mut d, &mem, "[method]descriptor.readlink-at", dir, b"long");
            std::fs::remove_dir_all(&root).ok();
            assert!(lexical.is_err(), "got {:?}", lexical);
            assert!(via_link.is_err(), "got {:?}", via_link);
            assert_eq!(err_code(&abs), Some(NOT_PERMITTED));
            assert_eq!(err_code(&long), Some(18)); // name-too-long
            Ok(())
        }

        // rivet: verifies SR-72
        #[cfg(unix)]
        #[serial_test::serial]
        #[test]
        fn readlink_at_fails_loud_without_preallocated_buffer() -> Result<()> {
            MemoryInitializer::ensure_initialized()?;
            let (root, sandbox) = fixture("sr72_readlink_nobuf");
            std::os::unix::fs::symlink("a.txt", sandbox.join("l")).unwrap();
            let mut d = WasiDispatcher::with_defaults()?;
            let dir = d.add_preopen(sandbox.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let res = path_call(&mut d, &mem, "[method]descriptor.readlink-at", dir, b"l");
            std::fs::remove_dir_all(&root).ok();
            assert!(res.is_err(), "got {:?}", res);
            Ok(())
        }
    }

    /// SR-96: a trailing slash (or `/.`) on a guest path must never change which
    /// component is resolved so as to leave the preopen (the wasmtime
    /// trailing-slash sandbox-escape class). `sandbox/link` is a symlink to a
    /// directory OUTSIDE the preopen; every path-taking handler on the single
    /// `dispatch_core` path is driven with `link/`, `link/.`, `dir/../link/` and
    /// `file/` and must be refused or act only inside the preopen.
    #[cfg(all(unix, feature = "wasi-filesystem", feature = "std"))]
    mod sr94 {
        use super::*;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        use std::time::{Duration, UNIX_EPOCH};

        const FS: &str = "wasi:filesystem/types";

        /// A timestamp as the dispatcher reports it: `None` or `Some((seconds, nanos))`.
        type Ts = Option<(i64, i32)>;

        struct Stat {
            link_count: i64,
            size: i64,
            atime: Ts,
            mtime: Ts,
            ctime: Ts,
        }

        /// Parse the flat core values of an ok `descriptor-stat` result:
        /// [0, type, link-count, size, 3 x (disc [, seconds, nanos])].
        fn parse(values: &[CV]) -> Stat {
            let mut it = values.iter();
            assert!(
                matches!(it.next(), Some(CV::I32(0))),
                "ok discriminant: {values:?}"
            );
            assert!(matches!(it.next(), Some(CV::I32(_))), "type: {values:?}");
            let Some(CV::I64(link_count)) = it.next() else {
                panic!("link-count: {values:?}")
            };
            let Some(CV::I64(size)) = it.next() else {
                panic!("size: {values:?}")
            };
            let mut ts = |name: &str| -> Ts {
                match it.next() {
                    Some(CV::I32(0)) => None,
                    Some(CV::I32(1)) => {
                        let Some(CV::I64(s)) = it.next() else {
                            panic!("{name} seconds: {values:?}")
                        };
                        let Some(CV::I32(n)) = it.next() else {
                            panic!("{name} nanos: {values:?}")
                        };
                        Some((*s, *n))
                    },
                    other => panic!("{name} discriminant {other:?}: {values:?}"),
                }
            };
            let atime = ts("access");
            let mtime = ts("modification");
            let ctime = ts("status-change");
            assert!(it.next().is_none(), "trailing values: {values:?}");
            Stat {
                link_count: *link_count,
                size: *size,
                atime,
                mtime,
                ctime,
            }
        }

        struct Fx {
            root: std::path::PathBuf,
            d: WasiDispatcher,
            mem: SliceMemory,
            dir: u32,
        }

        impl Fx {
            fn new(tag: &str) -> Result<Self> {
                MemoryInitializer::ensure_initialized()?;
                let root = sandbox_fixture_dir(tag);
                std::fs::create_dir_all(&root).unwrap();
                std::fs::write(root.join("file"), b"hello").unwrap();
                let mut d = WasiDispatcher::with_defaults()?;
                let dir = d.add_preopen(root.clone())?;
                Ok(Self {
                    root,
                    d,
                    mem: SliceMemory::with_size(0x10000),
                    dir,
                })
            }

            fn put(&self, s: &[u8]) -> [CV; 2] {
                self.mem.write_bytes(0x1000, s).unwrap();
                [CV::I32(0x1000), CV::I32(s.len() as i32)]
            }

            fn call(&mut self, func: &str, args: &[CV]) -> Result<Vec<CV>> {
                self.d.dispatch_core(FS, func, args, Some(&self.mem))
            }

            fn stat_at(&mut self, path: &[u8]) -> Result<Stat> {
                let [p, l] = self.put(path);
                let dir = self.dir as i32;
                let v = self.call(
                    "[method]descriptor.stat-at",
                    &[CV::I32(dir), CV::I32(0), p, l, CV::I32(0x100)],
                )?;
                Ok(parse(&v))
            }

            fn stat(&mut self, path: &[u8]) -> Result<Stat> {
                let [p, l] = self.put(path);
                let dir = self.dir as i32;
                let opened = self.call(
                    "[method]descriptor.open-at",
                    &[CV::I32(dir), CV::I32(0), p, l, CV::I32(0), CV::I32(1)],
                )?;
                let Some([CV::I32(0), CV::I32(h)]) =
                    opened.get(..2).map(|s| [s[0].clone(), s[1].clone()])
                else {
                    panic!("open-at failed: {opened:?}")
                };
                let v = self.call("[method]descriptor.stat", &[CV::I32(h)])?;
                Ok(parse(&v))
            }
        }

        impl Drop for Fx {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.root).ok();
            }
        }

        // rivet: verifies SR-94
        #[test]
        fn stat_and_stat_at_report_the_real_hard_link_count() -> Result<()> {
            let mut fx = Fx::new("sr94-nlink")?;
            assert_eq!(fx.stat_at(b"file")?.link_count, 1);
            assert_eq!(fx.stat(b"file")?.link_count, 1);
            std::fs::hard_link(fx.root.join("file"), fx.root.join("second")).unwrap();
            for path in [&b"file"[..], b"second"] {
                assert_eq!(fx.stat_at(path)?.link_count, 2, "stat-at");
                let s = fx.stat(path)?;
                assert_eq!(s.link_count, 2, "stat");
                assert_eq!(s.size, 5);
            }
            Ok(())
        }

        // rivet: verifies SR-94
        #[test]
        fn stat_and_stat_at_report_the_host_timestamps() -> Result<()> {
            let mut fx = Fx::new("sr94-times")?;
            let mtime = UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789);
            std::fs::File::options()
                .write(true)
                .open(fx.root.join("file"))
                .unwrap()
                .set_modified(mtime)
                .unwrap();
            for s in [fx.stat_at(b"file")?, fx.stat(b"file")?] {
                assert_eq!(s.mtime, Some((1_700_000_000, 123_456_789)));
                // The host provides access and status-change times: some, never a
                // zero-filled none, and a real (post-2020) instant.
                assert!(matches!(s.atime, Some((secs, _)) if secs > 1_600_000_000));
                assert!(matches!(s.ctime, Some((secs, _)) if secs > 1_600_000_000));
            }
            Ok(())
        }

        // rivet: verifies SR-94
        #[test]
        fn a_pre_epoch_timestamp_is_none_not_zero() -> Result<()> {
            let mut fx = Fx::new("sr94-preepoch")?;
            std::fs::File::options()
                .write(true)
                .open(fx.root.join("file"))
                .unwrap()
                .set_modified(UNIX_EPOCH - Duration::from_secs(10))
                .unwrap();
            for s in [fx.stat_at(b"file")?, fx.stat(b"file")?] {
                assert_eq!(s.mtime, None);
            }
            Ok(())
        }
    }

    mod sr96 {
        use super::*;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;

        const FS: &str = "wasi:filesystem/types";
        const NOT_DIRECTORY: i32 = 24;
        const MARKER_BODY: &[u8] = b"OUTSIDE-MARKER";

        /// Tempdir root holding `sandbox/{dir/, file, link -> outside}` and a
        /// sibling `outside/marker`. Removed on drop, even when an assertion fails.
        struct Fx {
            root: std::path::PathBuf,
            sandbox: std::path::PathBuf,
            outside: std::path::PathBuf,
            d: WasiDispatcher,
            mem: SliceMemory,
            dir: u32,
        }

        impl Fx {
            fn new(tag: &str) -> Result<Self> {
                MemoryInitializer::ensure_initialized()?;
                let root = sandbox_fixture_dir(tag);
                let sandbox = root.join("sandbox");
                let outside = root.join("outside");
                std::fs::create_dir_all(sandbox.join("dir")).unwrap();
                std::fs::create_dir_all(&outside).unwrap();
                std::fs::write(sandbox.join("file"), b"inside-file").unwrap();
                std::fs::write(outside.join("marker"), MARKER_BODY).unwrap();
                let abs_outside = std::fs::canonicalize(&outside).unwrap();
                std::os::unix::fs::symlink(&abs_outside, sandbox.join("link")).unwrap();
                let mut d = WasiDispatcher::with_defaults()?;
                let dir = d.add_preopen(sandbox.clone())?;
                Ok(Self {
                    root,
                    sandbox,
                    outside,
                    d,
                    mem: SliceMemory::with_size(0x10000),
                    dir,
                })
            }

            /// Sorted (name, file contents / None for a directory) of `outside/`.
            fn outside_snapshot(&self) -> Vec<(String, Option<Vec<u8>>)> {
                let mut v: Vec<_> = std::fs::read_dir(&self.outside)
                    .unwrap()
                    .map(|e| {
                        let e = e.unwrap();
                        let body = if e.path().is_dir() {
                            None
                        } else {
                            Some(std::fs::read(e.path()).unwrap())
                        };
                        (e.file_name().to_string_lossy().to_string(), body)
                    })
                    .collect();
                v.sort();
                v
            }

            fn put(&self, ptr: u32, s: &[u8]) -> [CV; 2] {
                self.mem.write_bytes(ptr, s).unwrap();
                [CV::I32(ptr as i32), CV::I32(s.len() as i32)]
            }

            fn call(&mut self, func: &str, args: &[CV]) -> Result<Vec<CV>> {
                self.d.dispatch_core(FS, func, args, Some(&self.mem))
            }

            /// open-at / stat-at style: (self, path-flags, path ptr, path len, ...).
            fn open_at(&mut self, path: &[u8]) -> Result<Vec<CV>> {
                let [p, l] = self.put(0x1000, path);
                let dir = self.dir as i32;
                self.call(
                    "[method]descriptor.open-at",
                    &[
                        CV::I32(dir),
                        CV::I32(0),
                        p,
                        l,
                        CV::I32(0),
                        CV::I32(1), // descriptor-flags: read
                    ],
                )
            }

            fn stat_at(&mut self, path: &[u8]) -> Result<Vec<CV>> {
                let [p, l] = self.put(0x1000, path);
                let dir = self.dir as i32;
                self.call(
                    "[method]descriptor.stat-at",
                    &[CV::I32(dir), CV::I32(0), p, l, CV::I32(0x100)],
                )
            }

            /// unlink-file-at / remove-directory-at / create-directory-at.
            fn path_op(&mut self, func: &str, path: &[u8]) -> Result<Vec<CV>> {
                let [p, l] = self.put(0x1000, path);
                let dir = self.dir as i32;
                self.call(func, &[CV::I32(dir), p, l, CV::I32(0x100)])
            }

            fn rename_at(&mut self, old: &[u8], new: &[u8]) -> Result<Vec<CV>> {
                let [op, ol] = self.put(0x1000, old);
                let [np, nl] = self.put(0x2000, new);
                let dir = self.dir as i32;
                self.call(
                    "[method]descriptor.rename-at",
                    &[CV::I32(dir), op, ol, CV::I32(dir), np, nl, CV::I32(0x100)],
                )
            }

            fn symlink_at(&mut self, contents: &[u8], new: &[u8]) -> Result<Vec<CV>> {
                let [op, ol] = self.put(0x1000, contents);
                let [np, nl] = self.put(0x2000, new);
                let dir = self.dir as i32;
                self.call(
                    "[method]descriptor.symlink-at",
                    &[CV::I32(dir), op, ol, np, nl, CV::I32(0x100)],
                )
            }
        }

        impl Drop for Fx {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.root).ok();
            }
        }

        fn err_code(res: &Result<Vec<CV>>) -> Option<i32> {
            match res.as_deref() {
                Ok([CV::I32(1), CV::I32(c)]) => Some(*c),
                _ => None,
            }
        }

        fn succeeded(res: &Result<Vec<CV>>) -> bool {
            matches!(res.as_deref(), Ok([CV::I32(0), ..]))
        }

        /// The outside directory is exactly as the fixture made it.
        fn assert_outside_intact(fx: &Fx, what: &str) {
            assert_eq!(
                fx.outside_snapshot(),
                vec![("marker".to_string(), Some(MARKER_BODY.to_vec()))],
                "{what}: outside/ was created in, removed from or modified"
            );
        }

        /// `file/` on a regular file must report not-directory and leave it intact.
        fn assert_not_directory(fx: &Fx, res: &Result<Vec<CV>>, what: &str) {
            assert_eq!(
                err_code(res),
                Some(NOT_DIRECTORY),
                "{what}: trailing slash on a non-directory must be not-directory, got {res:?}"
            );
            assert_eq!(
                std::fs::read(fx.sandbox.join("file")).unwrap(),
                b"inside-file",
                "{what}: `file` was modified"
            );
            assert_outside_intact(fx, what);
        }

        /// Creating something at `file/`: the name exists, so POSIX `mkdir("file/")`
        /// and `symlink(_, "file/")` fail with EEXIST (not ENOTDIR). The task asked
        /// for not-directory; EEXIST is the platform-correct code for these two
        /// creators and is accepted, but `file` must be left untouched.
        fn assert_exists_not_replaced(fx: &Fx, res: &Result<Vec<CV>>, what: &str) {
            assert_eq!(
                err_code(res),
                Some(fs_error::EXIST),
                "{what}: creating at `file/` must fail with exist, got {res:?}"
            );
            assert_eq!(
                std::fs::read(fx.sandbox.join("file")).unwrap(),
                b"inside-file",
                "{what}: `file` was modified"
            );
            assert_outside_intact(fx, what);
        }

        fn is_file_case(path: &[u8]) -> bool {
            path == b"file/"
        }

        // ---------------- open-at ----------------

        fn check_open_at(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_open")?;
            let res = fx.open_at(path);
            if is_file_case(path) {
                assert_not_directory(&fx, &res, "open-at");
            } else {
                assert!(
                    !succeeded(&res),
                    "open-at {:?} must not hand out a descriptor for the outside dir, got {res:?}",
                    String::from_utf8_lossy(path)
                );
                assert_outside_intact(&fx, "open-at");
            }
            Ok(())
        }

        // ---------------- stat-at ----------------

        fn check_stat_at(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_stat")?;
            let res = fx.stat_at(path);
            if is_file_case(path) {
                assert_not_directory(&fx, &res, "stat-at");
            } else {
                assert!(
                    !succeeded(&res),
                    "stat-at {:?} must not return the outside dir's metadata, got {res:?}",
                    String::from_utf8_lossy(path)
                );
                assert_outside_intact(&fx, "stat-at");
            }
            Ok(())
        }

        // ---------------- unlink-file-at ----------------

        fn check_unlink(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_unlink")?;
            let res = fx.path_op("[method]descriptor.unlink-file-at", path);
            if is_file_case(path) {
                assert_not_directory(&fx, &res, "unlink-file-at");
            } else {
                // Refused, or acted on the `link` entry inside the preopen only.
                assert_outside_intact(&fx, "unlink-file-at");
                assert!(fx.sandbox.join("file").exists(), "unrelated file removed");
                assert!(fx.sandbox.join("dir").is_dir(), "unrelated dir removed");
            }
            Ok(())
        }

        // ---------------- remove-directory-at ----------------

        fn check_rmdir(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_rmdir")?;
            // Make the escape observable: an empty outside dir is removable by rmdir.
            std::fs::remove_file(fx.outside.join("marker")).unwrap();
            let res = fx.path_op("[method]descriptor.remove-directory-at", path);
            assert!(
                fx.outside.is_dir(),
                "remove-directory-at {:?} removed the outside directory, got {res:?}",
                String::from_utf8_lossy(path)
            );
            if is_file_case(path) {
                assert_eq!(
                    err_code(&res),
                    Some(NOT_DIRECTORY),
                    "remove-directory-at `file/` must be not-directory, got {res:?}"
                );
                assert!(fx.sandbox.join("file").exists(), "`file` was removed");
            } else {
                assert!(fx.sandbox.join("file").exists(), "unrelated file removed");
            }
            Ok(())
        }

        // ---------------- create-directory-at ----------------

        fn check_mkdir(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_mkdir")?;
            let res = fx.path_op("[method]descriptor.create-directory-at", path);
            if is_file_case(path) {
                assert_exists_not_replaced(&fx, &res, "create-directory-at");
            } else {
                assert!(
                    !succeeded(&res),
                    "create-directory-at on an existing path must not succeed, got {res:?}"
                );
                assert_outside_intact(&fx, "create-directory-at");
            }
            Ok(())
        }

        // ---------------- rename-at ----------------

        /// `path` as the OLD name; the new name is a fresh `moved`.
        fn check_rename_old(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_rename_old")?;
            let res = fx.rename_at(path, b"moved");
            if is_file_case(path) {
                assert_not_directory(&fx, &res, "rename-at old");
                assert!(!fx.sandbox.join("moved").exists(), "`file/` was renamed");
            } else {
                assert_outside_intact(&fx, "rename-at old");
                assert!(
                    !fx.root.join("moved").exists() && !fx.outside.join("moved").exists(),
                    "rename-at old moved something out of the preopen"
                );
            }
            Ok(())
        }

        /// `path` as the NEW name; the old name is the regular `file`.
        fn check_rename_new(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_rename_new")?;
            let res = fx.rename_at(b"file", path);
            if is_file_case(path) {
                assert_not_directory(&fx, &res, "rename-at new");
            } else {
                assert_outside_intact(&fx, "rename-at new");
                assert!(
                    !fx.root.join("file").exists(),
                    "rename-at new moved `file` out of the preopen"
                );
            }
            Ok(())
        }

        // ---------------- symlink-at ----------------

        fn check_symlink(path: &[u8]) -> Result<()> {
            let mut fx = Fx::new("sr96_symlink")?;
            let res = fx.symlink_at(b"dir", path);
            if is_file_case(path) {
                assert_exists_not_replaced(&fx, &res, "symlink-at");
            } else {
                assert!(
                    !succeeded(&res),
                    "symlink-at over an existing name must not succeed, got {res:?}"
                );
                assert_outside_intact(&fx, "symlink-at");
            }
            Ok(())
        }

        macro_rules! sr96_tests {
            ($check:ident, $link_slash:ident, $link_dot:ident, $dir_dotdot:ident, $file_slash:ident) => {
                // rivet: verifies SR-96
                #[serial_test::serial]
                #[test]
                fn $link_slash() -> Result<()> {
                    $check(b"link/")
                }
                // rivet: verifies SR-96
                #[serial_test::serial]
                #[test]
                fn $link_dot() -> Result<()> {
                    $check(b"link/.")
                }
                // rivet: verifies SR-96
                #[serial_test::serial]
                #[test]
                fn $dir_dotdot() -> Result<()> {
                    $check(b"dir/../link/")
                }
                // rivet: verifies SR-96
                #[serial_test::serial]
                #[test]
                fn $file_slash() -> Result<()> {
                    $check(b"file/")
                }
            };
        }

        sr96_tests!(
            check_open_at,
            sr96_open_at_link_slash,
            sr96_open_at_link_dot,
            sr96_open_at_dir_dotdot_link_slash,
            sr96_open_at_file_slash
        );
        sr96_tests!(
            check_stat_at,
            sr96_stat_at_link_slash,
            sr96_stat_at_link_dot,
            sr96_stat_at_dir_dotdot_link_slash,
            sr96_stat_at_file_slash
        );
        sr96_tests!(
            check_unlink,
            sr96_unlink_file_at_link_slash,
            sr96_unlink_file_at_link_dot,
            sr96_unlink_file_at_dir_dotdot_link_slash,
            sr96_unlink_file_at_file_slash
        );
        sr96_tests!(
            check_rmdir,
            sr96_remove_directory_at_link_slash,
            sr96_remove_directory_at_link_dot,
            sr96_remove_directory_at_dir_dotdot_link_slash,
            sr96_remove_directory_at_file_slash
        );
        sr96_tests!(
            check_mkdir,
            sr96_create_directory_at_link_slash,
            sr96_create_directory_at_link_dot,
            sr96_create_directory_at_dir_dotdot_link_slash,
            sr96_create_directory_at_file_slash
        );
        sr96_tests!(
            check_rename_old,
            sr96_rename_at_old_link_slash,
            sr96_rename_at_old_link_dot,
            sr96_rename_at_old_dir_dotdot_link_slash,
            sr96_rename_at_old_file_slash
        );
        sr96_tests!(
            check_rename_new,
            sr96_rename_at_new_link_slash,
            sr96_rename_at_new_link_dot,
            sr96_rename_at_new_dir_dotdot_link_slash,
            sr96_rename_at_new_file_slash
        );
        sr96_tests!(
            check_symlink,
            sr96_symlink_at_link_slash,
            sr96_symlink_at_link_dot,
            sr96_symlink_at_dir_dotdot_link_slash,
            sr96_symlink_at_file_slash
        );
    }

    /// SR-91 / #625: monotonic-clock `subscribe-duration` / `subscribe-instant`
    /// return real pollables on the monotonic clock; `ready`, `block` and
    /// `wasi:io/poll::poll` honour their deadlines. Driven through the live
    /// `dispatch_core` path.
    #[cfg(all(feature = "wasi-clocks", feature = "std"))]
    mod sr91 {
        use super::*;
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        use std::time::{Duration, Instant};

        const CLOCK: &str = "wasi:clocks/monotonic-clock";
        const POLL: &str = "wasi:io/poll";
        const LIST_PTR: u32 = 0x1000;
        const RET_PTR: u32 = 0x100;
        const RESULT_BUF: u32 = 0x2000;

        struct Fx {
            d: WasiDispatcher,
            mem: SliceMemory,
        }

        impl Fx {
            fn new() -> Result<Self> {
                MemoryInitializer::ensure_initialized()?;
                Ok(Self {
                    d: WasiDispatcher::with_defaults()?,
                    mem: SliceMemory::with_size(0x10000),
                })
            }

            fn call(&mut self, iface: &str, func: &str, args: &[CV]) -> Result<Vec<CV>> {
                self.d.dispatch_core(iface, func, args, Some(&self.mem))
            }

            fn now_ns(&mut self) -> u64 {
                match self.call(CLOCK, "now", &[]).unwrap().as_slice() {
                    [CV::I64(n)] => *n as u64,
                    other => panic!("now returned {:?}", other),
                }
            }

            fn handle(r: Result<Vec<CV>>) -> i32 {
                match r.unwrap().as_slice() {
                    [CV::I32(h)] => *h,
                    other => panic!("expected one handle, got {:?}", other),
                }
            }

            fn subscribe_duration(&mut self, ns: u64) -> i32 {
                let r = self.call(CLOCK, "subscribe-duration", &[CV::I64(ns as i64)]);
                Self::handle(r)
            }

            fn subscribe_instant(&mut self, ns: u64) -> i32 {
                let r = self.call(CLOCK, "subscribe-instant", &[CV::I64(ns as i64)]);
                Self::handle(r)
            }

            fn ready(&mut self, h: i32) -> Result<bool> {
                let r = self.call(POLL, "[method]pollable.ready", &[CV::I32(h)])?;
                match r.as_slice() {
                    [CV::I32(b)] => Ok(*b != 0),
                    other => panic!("ready returned {:?}", other),
                }
            }

            fn block(&mut self, h: i32) -> Result<Vec<CV>> {
                self.call(POLL, "[method]pollable.block", &[CV::I32(h)])
            }

            /// poll over `hs`; returns the indices read back from guest memory.
            fn poll(&mut self, hs: &[i32]) -> Result<Vec<u32>> {
                for (i, h) in hs.iter().enumerate() {
                    self.mem.write_bytes(LIST_PTR + 4 * i as u32, &h.to_le_bytes()).unwrap();
                }
                self.d.set_read_buffer_allocation(RESULT_BUF, 64);
                let r = self.call(
                    POLL,
                    "poll",
                    &[
                        CV::I32(LIST_PTR as i32),
                        CV::I32(hs.len() as i32),
                        CV::I32(RET_PTR as i32),
                    ],
                )?;
                assert!(r.is_empty(), "poll lowers via retptr, got {:?}", r);
                let mut word = [0u8; 4];
                self.mem.read_bytes(RET_PTR, &mut word).unwrap();
                let ptr = u32::from_le_bytes(word);
                self.mem.read_bytes(RET_PTR + 4, &mut word).unwrap();
                let len = u32::from_le_bytes(word);
                assert_eq!(ptr, RESULT_BUF, "result list lives in the guest allocation");
                let mut out = Vec::new();
                for i in 0..len {
                    self.mem.read_bytes(ptr + 4 * i, &mut word).unwrap();
                    out.push(u32::from_le_bytes(word));
                }
                Ok(out)
            }
        }

        #[test]
        fn subscribe_duration_ready_false_then_block_waits_then_ready() -> Result<()> {
            let mut fx = Fx::new()?;
            let p = fx.subscribe_duration(50_000_000);
            assert!(!fx.ready(p)?, "not ready before the deadline");
            let t = Instant::now();
            fx.block(p)?;
            assert!(
                t.elapsed() >= Duration::from_millis(50),
                "block returned after {:?}",
                t.elapsed()
            );
            assert!(fx.ready(p)?, "ready after the deadline");
            Ok(())
        }

        #[test]
        fn subscribe_instant_future_and_past() -> Result<()> {
            let mut fx = Fx::new()?;
            let now = fx.now_ns();
            let p = fx.subscribe_instant(now + 30_000_000);
            assert!(!fx.ready(p)?, "future instant not ready yet");
            let t = Instant::now();
            fx.block(p)?;
            assert!(
                t.elapsed() >= Duration::from_millis(25),
                "block waited only {:?}",
                t.elapsed()
            );
            assert!(fx.ready(p)?);

            let now = fx.now_ns();
            let past = fx.subscribe_instant(now.saturating_sub(1_000_000));
            assert!(fx.ready(past)?, "past instant is ready immediately");
            Ok(())
        }

        #[test]
        fn poll_waits_for_the_earliest_deadline() -> Result<()> {
            let mut fx = Fx::new()?;
            let late = fx.subscribe_duration(200_000_000);
            let early = fx.subscribe_duration(40_000_000);
            let t = Instant::now();
            let ready = fx.poll(&[late, early])?;
            let took = t.elapsed();
            assert_eq!(ready, vec![1]);
            assert!(
                took >= Duration::from_millis(40),
                "poll took only {:?}",
                took
            );
            assert!(took < Duration::from_millis(150), "poll took {:?}", took);
            Ok(())
        }

        #[test]
        fn poll_returns_all_ready_indices_in_input_order() -> Result<()> {
            let mut fx = Fx::new()?;
            let a = fx.subscribe_duration(0);
            let pending = fx.subscribe_duration(5_000_000_000);
            let b = fx.subscribe_duration(0);
            let t = Instant::now();
            assert_eq!(fx.poll(&[a, pending, b])?, vec![0, 2]);
            assert!(t.elapsed() < Duration::from_millis(500), "must not wait");
            Ok(())
        }

        #[test]
        fn poll_without_result_allocation_fails_loud() -> Result<()> {
            let mut fx = Fx::new()?;
            let a = fx.subscribe_duration(0);
            fx.mem.write_bytes(LIST_PTR, &a.to_le_bytes()).unwrap();
            let r = fx.call(
                POLL,
                "poll",
                &[
                    CV::I32(LIST_PTR as i32),
                    CV::I32(1),
                    CV::I32(RET_PTR as i32),
                ],
            );
            assert!(r.is_err());
            Ok(())
        }

        #[test]
        fn unknown_pollable_handle_is_an_error() -> Result<()> {
            let mut fx = Fx::new()?;
            assert!(fx.ready(999).is_err(), "ready");
            assert!(fx.block(999).is_err(), "block");
            assert!(fx.poll(&[999]).is_err(), "poll");
            let drop = fx.call(POLL, "[resource-drop]pollable", &[CV::I32(999)]);
            assert!(drop.is_err(), "drop");
            let live = fx.subscribe_duration(0);
            assert!(fx.poll(&[live, 999]).is_err(), "poll with one unknown");
            Ok(())
        }

        #[test]
        fn dropped_pollable_is_gone() -> Result<()> {
            let mut fx = Fx::new()?;
            let p = fx.subscribe_duration(0);
            fx.call(POLL, "[resource-drop]pollable", &[CV::I32(p)])?;
            assert!(fx.ready(p).is_err());
            let again = fx.call(POLL, "[resource-drop]pollable", &[CV::I32(p)]);
            assert!(again.is_err(), "double drop");
            Ok(())
        }

        #[test]
        fn stream_subscribe_is_a_real_always_ready_pollable() -> Result<()> {
            let mut fx = Fx::new()?;
            let out = fx.call("wasi:cli/stdout", "get-stdout", &[])?;
            let CV::I32(os) = out[0] else {
                panic!("handle")
            };
            let p = Fx::handle(fx.call(
                "wasi:io/streams",
                "[method]output-stream.subscribe",
                &[CV::I32(os)],
            ));
            assert!(fx.ready(p)?);
            assert_eq!(fx.poll(&[p])?, vec![0]);
            Ok(())
        }

        #[test]
        fn empty_poll_list_is_an_error() -> Result<()> {
            let mut fx = Fx::new()?;
            assert!(fx.poll(&[]).is_err());
            Ok(())
        }
    }
}

#[cfg(test)]
mod global_wasi_state_tests {
    use super::*;

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn global_args_round_trip_and_fallback() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        set_global_wasi_args(vec!["prog".to_string(), "--flag".to_string()]);
        let got = get_global_wasi_args()?;
        set_global_wasi_args(Vec::new());
        assert_eq!(got, vec!["prog".to_string(), "--flag".to_string()]);
        let fallback = get_global_wasi_args()?;
        let expected: Vec<String> = std::env::args().collect();
        assert!(!expected.is_empty());
        assert_eq!(fallback, expected);
        Ok(())
    }

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn global_env_round_trip() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let env = vec![
            ("KILN_TEST_ENV_A".to_string(), "1".to_string()),
            ("KILN_TEST_ENV_B".to_string(), "2".to_string()),
        ];
        set_global_wasi_env(env.clone());
        let got = get_global_wasi_env()?;
        set_global_wasi_env(Vec::new());
        assert_eq!(got, env);
        let fallback = get_global_wasi_env()?;
        let expected: Vec<(String, String)> = std::env::vars().collect();
        assert_eq!(fallback, expected);
        Ok(())
    }
}

#[cfg(test)]
mod pollable_deadline_tests {
    use super::*;
    #[test]
    fn deadline_equal_to_now_is_ready_and_future_is_pending() {
        let now = std::time::Instant::now();
        assert_eq!(Pollable::Deadline(now).pending_until(now), None);
        let later = now + std::time::Duration::from_secs(5);
        assert_eq!(Pollable::Deadline(later).pending_until(now), Some(later));
        assert_eq!(Pollable::AlwaysReady.pending_until(now), None);
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
mod create_directory_at_fd_table_preopen_tests {
    use super::*;
    use kiln_foundation::memory_init::MemoryInitializer;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[serial_test::serial]
    #[test]
    fn fd_table_only_preopen_is_a_base() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let root =
            std::env::temp_dir().join(format!("kiln_create_dir_fd_table_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        let _guard = Cleanup(root.clone());
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        d.capabilities.filesystem.write_access = true;
        d.fd_table.insert(
            0x7777,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::PreopenDirectory(sandbox.clone()),
                read: true,
                write: true,
                file: None,
            },
        );
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"newdir")?;
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.create-directory-at",
            &[CV::I32(0x7777), CV::I32(0x1000), CV::I32(6), CV::I32(0x100)],
            Some(&mem),
        )?;
        assert!(matches!(res.as_slice(), [CV::I32(0)]));
        assert!(sandbox.join("newdir").is_dir());
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
mod create_directory_capability_tests {
    use super::*;
    use kiln_foundation::memory_init::MemoryInitializer;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[serial_test::serial]
    #[test]
    fn mkdir_refused_without_write_capability() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let root =
            std::env::temp_dir().join(format!("kiln_create_dir_capability_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        let _guard = Cleanup(root.clone());
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        let dir = d.add_preopen(&sandbox)?;
        d.capabilities.filesystem.write_access = false;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"newdir")?;
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.create-directory-at",
            &[
                CV::I32(dir as i32),
                CV::I32(0x1000),
                CV::I32(6),
                CV::I32(0x100),
            ],
            Some(&mem),
        );
        assert!(res.is_err());
        assert!(!sandbox.join("newdir").exists());
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-random", feature = "std"))]
mod get_random_bytes_tests {
    use super::*;
    use kiln_foundation::memory_init::MemoryInitializer;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    #[serial_test::serial]
    #[test]
    fn random_request_of_4096_bytes_is_served() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        d.capabilities.random.secure_random = true;
        d.read_buf_alloc = Some((0x2000, 4096));
        let mem = SliceMemory::with_size(0x10000);
        let res = d.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[CV::I64(4096), CV::I32(0x100)],
            Some(&mem),
        )?;
        assert!(res.is_empty());
        let mut hdr = [0u8; 8];
        mem.read_bytes(0x100, &mut hdr)?;
        assert_eq!(u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]), 0x2000);
        assert_eq!(u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]), 4096);
        Ok(())
    }

    const MAX: i64 = 1024 * 1024;

    fn run(len: i64) -> Result<(Vec<CV>, SliceMemory)> {
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        d.capabilities.random.secure_random = true;
        d.read_buf_alloc = Some((0x1000, MAX as u32));
        let mem = SliceMemory::with_size(0x110000);
        let res = d.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[CV::I64(len), CV::I32(0x100)],
            Some(&mem),
        )?;
        Ok((res, mem))
    }

    #[serial_test::serial]
    #[test]
    fn exactly_one_mib_is_accepted() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let (res, mem) = run(MAX)?;
        assert!(res.is_empty());
        let mut hdr = [0u8; 8];
        mem.read_bytes(0x100, &mut hdr)?;
        assert_eq!(
            u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]),
            MAX as u32
        );
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn over_one_mib_is_refused() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        assert!(run(MAX + 1).is_err());
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
mod descriptor_write_full_memory_tests {
    use super::*;
    use kiln_foundation::memory_init::MemoryInitializer;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[serial_test::serial]
    #[test]
    fn write_buffer_spanning_whole_memory_is_accepted() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let root =
            std::env::temp_dir().join(format!("kiln_descriptor_write_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sandbox = root.join("sandbox");
        std::fs::create_dir_all(&sandbox).unwrap();
        std::fs::write(sandbox.join("f.txt"), b"x").unwrap();
        let _guard = Cleanup(root.clone());
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        d.capabilities.filesystem.read_access = true;
        d.capabilities.filesystem.write_access = true;
        let dir = d.add_preopen(&sandbox)?;
        let mem = SliceMemory::with_size(4096);
        mem.write_bytes(0x100, b"f.txt")?;
        let opened = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.open-at",
            &[
                CV::I32(dir as i32),
                CV::I32(0),
                CV::I32(0x100),
                CV::I32(5),
                CV::I32(0),
                CV::I32(3),
            ],
            Some(&mem),
        )?;
        let fd = match opened.as_slice() {
            [CV::I32(0), CV::I32(h)] => *h,
            _ => panic!("open-at failed"),
        };
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.write",
            &[
                CV::I32(fd),
                CV::I32(0),
                CV::I32(4096),
                CV::I64(0),
                CV::I32(0x100),
            ],
            Some(&mem),
        )?;
        assert!(matches!(res.as_slice(), [CV::I32(0), CV::I64(4096)]));
        assert_eq!(
            std::fs::metadata(sandbox.join("f.txt")).unwrap().len(),
            4096
        );
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
mod fs_error_code_tests {
    use super::*;
    use kiln_foundation::memory_init::MemoryInitializer;

    #[serial_test::serial]
    #[test]
    fn permission_denied_maps_to_access() -> Result<()> {
        MemoryInitializer::ensure_initialized()?;
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(WasiDispatcher::fs_error_code(&err), fs_error::ACCESS);
        assert_eq!(WasiDispatcher::fs_error_code(&err), 0);
        let other = std::io::Error::from(std::io::ErrorKind::Other);
        assert_eq!(WasiDispatcher::fs_error_code(&other), fs_error::IO);
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn invalid_input_maps_to_invalid_error_code() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let e = std::io::Error::from(std::io::ErrorKind::InvalidInput);
        assert_eq!(WasiDispatcher::fs_error_code(&e), fs_error::INVALID);
        assert_eq!(fs_error::INVALID, 12);
        let other = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        assert_eq!(WasiDispatcher::fs_error_code(&other), fs_error::IO);
        Ok(())
    }
}

#[cfg(test)]
mod verify_handle_contained_tests {
    use super::*;

    #[cfg(all(feature = "wasi-filesystem", feature = "std", unix))]
    #[serial_test::serial]
    #[test]
    fn handle_for_other_inode_in_same_dir_is_refused() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "kiln_verify_handle_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.txt"), b"A").unwrap();
        std::fs::write(root.join("b.txt"), b"B").unwrap();
        let handle = std::fs::File::open(root.join("a.txt")).unwrap();
        let good = WasiDispatcher::verify_handle_contained(&root, &root.join("a.txt"), &handle);
        let bad = WasiDispatcher::verify_handle_contained(&root, &root.join("b.txt"), &handle);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(good.is_ok());
        assert!(bad.is_err());
        Ok(())
    }
}

#[cfg(test)]
mod get_preopens_tests {
    use super::*;

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn get_preopens_returns_registered_preopen() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "kiln_get_preopens_{}_{}",
            std::process::id(),
            nanos
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut d = WasiDispatcher::new(WasiCapabilities::minimal()?)?;
        assert!(WasiDispatcher::get_preopens(&d).is_empty());
        let h = d.add_preopen(root.clone())?;
        let list = WasiDispatcher::get_preopens(&d);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, h);
        assert_eq!(list[0].1, root);
        std::fs::remove_dir_all(&root).unwrap();
        Ok(())
    }

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn get_preopens_two_entries_exact() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "kiln_get_preopens_two_{}_{}",
            std::process::id(),
            nanos
        ));
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut d = WasiDispatcher::new(WasiCapabilities::minimal()?)?;
        let ha = d.add_preopen(a.clone())?;
        let hb = d.add_preopen(b.clone())?;
        let list = WasiDispatcher::get_preopens(&d);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], (ha, a.clone()));
        assert_eq!(list[1], (hb, b.clone()));
        std::fs::remove_dir_all(&root).unwrap();
        Ok(())
    }
}

#[cfg(test)]
mod dispatcher_configuration_tests {
    use super::*;

    #[serial_test::serial]
    #[test]
    fn set_args_replaces_args() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        assert!(d.args().is_empty());
        d.set_args(vec![String::from("prog"), String::from("--flag")]);
        assert_eq!(d.args(), &[String::from("prog"), String::from("--flag")]);
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn set_env_replaces_env_vars() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::new(WasiCapabilities::system_utility()?)?;
        assert!(d.env_vars.is_empty());
        d.set_env(vec![(String::from("K"), String::from("V"))]);
        assert_eq!(d.env_vars, vec![(String::from("K"), String::from("V"))]);
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn args_alloc_none_then_exact_then_cleared() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        assert!(d.get_args_alloc().is_none());
        d.set_args_alloc(0x2000, vec![(0x3000, 5), (0x3010, 3)]);
        let got = d.get_args_alloc().expect("alloc set");
        assert_eq!(got.0, 0x2000);
        assert_eq!(got.1, vec![(0x3000u32, 5u32), (0x3010u32, 3u32)]);
        d.clear_args_alloc();
        assert!(d.get_args_alloc().is_none());
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn args_returns_configured_args() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.set_args(vec!["alpha".to_string(), "beta".to_string()]);
        let expected = ["alpha".to_string(), "beta".to_string()];
        assert_eq!(d.args(), &expected[..]);
        d.set_args(Vec::new());
        assert!(d.args().is_empty());
        Ok(())
    }
}

#[cfg(test)]
mod get_arguments_alloc_count_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;
    fn run(allocs: Vec<(u32, u32)>) -> Result<Vec<CV>> {
        let mut d = WasiDispatcher::with_defaults()?;
        d.set_args(vec!["abcd".to_string()]);
        d.set_args_alloc(0x200, allocs);
        let mem = SliceMemory::with_size(0x1000);
        d.dispatch_core(
            "wasi:cli/environment",
            "get-arguments",
            &[CV::I32(0x100)],
            Some(&mem),
        )
    }
    #[serial_test::serial]
    #[test]
    fn matching_alloc_count_succeeds() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let r = run(vec![(0x300, 4)])?;
        assert!(r.is_empty());
        Ok(())
    }
    #[serial_test::serial]
    #[test]
    fn mismatched_alloc_count_is_error() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        assert!(run(vec![(0x300, 4), (0x400, 4)]).is_err());
        assert!(run(vec![]).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod get_arguments_memory_bounds_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;
    fn run(sp: u32) -> Result<(Result<Vec<CV>>, SliceMemory)> {
        let mut d = WasiDispatcher::with_defaults()?;
        d.set_args(vec!["abcd".to_string()]);
        d.set_args_alloc(0x200, vec![(sp, 4)]);
        let mem = SliceMemory::with_size(0x1000);
        mem.write_bytes(0x100, &[0xFF; 8])?;
        let r = d.dispatch_core(
            "wasi:cli/environment",
            "get-arguments",
            &[CV::I32(0x100)],
            Some(&mem),
        );
        Ok((r, mem))
    }
    #[serial_test::serial]
    #[test]
    fn string_ending_exactly_at_memory_end_is_written() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let (r, mem) = run(0xFFC)?;
        assert!(r?.is_empty());
        let mut s = [0u8; 4];
        mem.read_bytes(0xFFC, &mut s)?;
        assert_eq!(&s, b"abcd");
        let mut h = [0u8; 8];
        mem.read_bytes(0x100, &mut h)?;
        assert_eq!(u32::from_le_bytes([h[0], h[1], h[2], h[3]]), 0x200);
        assert_eq!(u32::from_le_bytes([h[4], h[5], h[6], h[7]]), 1);
        Ok(())
    }
    #[serial_test::serial]
    #[test]
    fn string_overflowing_memory_end_is_an_error_and_leaves_header_untouched() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let (r, mem) = run(0xFFD)?;
        let err = r.expect_err("an allocation past the end of memory must be refused");
        assert!(
            err.message.contains("lies past the end of memory"),
            "unexpected message: {}",
            err.message
        );
        let mut h = [0u8; 8];
        mem.read_bytes(0x100, &mut h)?;
        assert_eq!(h, [0xFFu8; 8]);
        Ok(())
    }
}

#[cfg(test)]
mod get_arguments_layout_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    fn rd(mem: &SliceMemory, p: u32) -> u32 {
        let mut b = [0u8; 4];
        mem.read_bytes(p, &mut b).unwrap();
        u32::from_le_bytes(b)
    }

    #[serial_test::serial]
    #[test]
    fn get_arguments_two_entries_layout() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.set_args(vec!["abc".to_string(), "de".to_string()]);
        d.set_args_alloc(0x200, vec![(0x300, 3), (0x400, 2)]);
        let mem = SliceMemory::with_size(0x1000);
        let retptr = 0x100u32;
        let r = d.dispatch_core(
            "wasi:cli/environment",
            "get-arguments",
            &[CV::I32(retptr as i32)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        assert_eq!(rd(&mem, retptr), 0x200);
        assert_eq!(rd(&mem, retptr + 4), 2);
        assert_eq!(rd(&mem, 0x200), 0x300);
        assert_eq!(rd(&mem, 0x204), 3);
        assert_eq!(rd(&mem, 0x208), 0x400);
        assert_eq!(rd(&mem, 0x20C), 2);
        let mut s = [0u8; 3];
        mem.read_bytes(0x300, &mut s)?;
        assert_eq!(&s, b"abc");
        let mut s2 = [0u8; 2];
        mem.read_bytes(0x400, &mut s2)?;
        assert_eq!(&s2, b"de");
        Ok(())
    }
}

#[cfg(test)]
mod poll_return_buffer_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn poll_exact_size_buffer_is_accepted() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = d.dispatch_core(
            "wasi:io/streams",
            "[method]output-stream.subscribe",
            &[CV::I32(0)],
            None,
        )?;
        let handle = match h[0] {
            CV::I32(v) => v,
            _ => panic!("bad handle"),
        };
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x100, &(handle as u32).to_le_bytes())?;
        d.read_buf_alloc = Some((0x2000, 4));
        let res = d.dispatch_core(
            "wasi:io/poll",
            "poll",
            &[CV::I32(0x100), CV::I32(1), CV::I32(0x300)],
            Some(&mem),
        )?;
        assert!(res.is_empty());
        let mut b = [0u8; 4];
        mem.read_bytes(0x300, &mut b)?;
        assert_eq!(u32::from_le_bytes(b), 0x2000);
        mem.read_bytes(0x304, &mut b)?;
        assert_eq!(u32::from_le_bytes(b), 1);
        mem.read_bytes(0x2000, &mut b)?;
        assert_eq!(u32::from_le_bytes(b), 0);
        Ok(())
    }
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn poll_too_small_buffer_is_refused() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = d.dispatch_core(
            "wasi:io/streams",
            "[method]output-stream.subscribe",
            &[CV::I32(0)],
            None,
        )?;
        let handle = match h[0] {
            CV::I32(v) => v,
            _ => panic!("bad handle"),
        };
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x100, &(handle as u32).to_le_bytes())?;
        d.read_buf_alloc = Some((0x2000, 3));
        let res = d.dispatch_core(
            "wasi:io/poll",
            "poll",
            &[CV::I32(0x100), CV::I32(1), CV::I32(0x300)],
            Some(&mem),
        );
        assert!(res.is_err());
        Ok(())
    }
}

#[cfg(test)]
mod check_write_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;
    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn check_write_writes_result_at_retptr() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let res = d.dispatch_core(
            "wasi:io/streams",
            "[method]output-stream.check-write",
            &[CV::I32(1), CV::I32(0x100)],
            Some(&mem),
        )?;
        assert!(res.is_empty());
        let mut b = [0u8; 4];
        mem.read_bytes(0x100, &mut b)?;
        assert_eq!(u32::from_le_bytes(b), 0);
        let mut c = [0u8; 8];
        mem.read_bytes(0x108, &mut c)?;
        assert_eq!(u64::from_le_bytes(c), 65536);
        Ok(())
    }
}

#[cfg(test)]
mod wall_clock_now_tests {
    use super::*;
    use kiln_foundation::values::Value as CV;
    #[cfg(all(feature = "std", feature = "wasi-clocks"))]
    #[serial_test::serial]
    #[test]
    fn wall_clock_now_respects_realtime_capability() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.clocks.realtime_access = false;
        let denied = d.dispatch_core("wasi:clocks/wall-clock", "now", &[], None);
        assert!(denied.is_err());
        d.capabilities.clocks.realtime_access = true;
        let ok = d.dispatch_core("wasi:clocks/wall-clock", "now", &[], None)?;
        assert_eq!(ok.len(), 2);
        assert!(matches!(ok[0], CV::I64(_)));
        Ok(())
    }
    #[cfg(all(feature = "std", feature = "wasi-clocks"))]
    #[serial_test::serial]
    #[test]
    fn wall_clock_seconds_is_epoch_seconds() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.clocks.realtime_access = true;
        let res = d.dispatch_core("wasi:clocks/wall-clock", "now", &[], None)?;
        assert_eq!(res.len(), 2);
        let secs = match res[0] {
            CV::I64(s) => s,
            _ => panic!("bad seconds"),
        };
        let nanos = match res[1] {
            CV::I32(n) => n,
            _ => panic!("bad nanos"),
        };
        assert!(
            secs >= 1_600_000_000,
            "seconds must be epoch seconds, got {}",
            secs
        );
        assert!((0..1_000_000_000).contains(&nanos));
        Ok(())
    }
}

#[cfg(test)]
mod wall_clock_now_retptr_layout_tests {
    use super::*;

    #[cfg(all(feature = "wasi-clocks", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn wall_clock_nanos_written_at_retptr_plus_8() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.clocks.realtime_access = true;
        let mem = SliceMemory::with_size(0x10000);
        let retptr = 0x100u32;
        mem.write_bytes(retptr - 16, &[0xAAu8; 48])?;
        d.dispatch_core(
            "wasi:clocks/wall-clock",
            "now",
            &[CV::I32(retptr as i32)],
            Some(&mem),
        )?;
        let mut b = [0u8; 12];
        mem.read_bytes(retptr, &mut b)?;
        let nanos = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        assert!(nanos < 1_000_000_000, "nanos at +8 not written: {}", nanos);
        // the bytes just below the record must be untouched
        let mut below = [0u8; 8];
        mem.read_bytes(retptr - 8, &mut below)?;
        assert_eq!(below, [0xAAu8; 8]);
        Ok(())
    }
}

#[cfg(test)]
mod wall_clock_resolution_tests {
    use super::*;

    #[cfg(all(feature = "wasi-clocks", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn wall_clock_resolution_writes_record_via_retptr() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        let retptr = 0x100u32;
        mem.write_bytes(retptr, &[0xFFu8; 16])?;
        let res = d.dispatch_core(
            "wasi:clocks/wall-clock",
            "resolution",
            &[CV::I32(retptr as i32)],
            Some(&mem),
        )?;
        assert!(res.is_empty());
        let mut b = [0u8; 16];
        mem.read_bytes(retptr, &mut b)?;
        assert_eq!(u64::from_le_bytes(b[0..8].try_into().unwrap()), 0);
        assert_eq!(u64::from_le_bytes(b[8..16].try_into().unwrap()), 1);
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-clocks", feature = "std"))]
mod wall_clock_resolution_versioned_import_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    #[serial_test::serial]
    #[test]
    fn wall_clock_resolution_writes_nanos_at_retptr_plus_8() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x100, &[0xFFu8; 16])?;
        let r = d.dispatch_core(
            "wasi:clocks/wall-clock@0.2.4",
            "resolution",
            &[CV::I32(0x100)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        let mut buf = [0u8; 16];
        mem.read_bytes(0x100, &mut buf)?;
        assert_eq!(&buf[0..8], &0u64.to_le_bytes());
        assert_eq!(&buf[8..16], &1u64.to_le_bytes());
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-io", feature = "std"))]
mod blocking_write_and_flush_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    fn stdout_handle(d: &mut WasiDispatcher) -> Result<i32> {
        let r = d.dispatch_core("wasi:cli/stdout@0.2.4", "get-stdout", &[], None)?;
        match r.first() {
            Some(CV::I32(h)) => Ok(*h),
            _ => panic!("no handle"),
        }
    }

    #[serial_test::serial]
    #[test]
    fn blocking_write_refused_without_stdout_capability() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = stdout_handle(&mut d)?;
        d.capabilities.io.stdout_access = false;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"\n")?;
        mem.write_bytes(0x2000, &[0xFFu8; 4])?;
        let r = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.blocking-write-and-flush",
            &[CV::I32(h), CV::I32(0x1000), CV::I32(1), CV::I32(0x2000)],
            Some(&mem),
        );
        assert!(r.is_err());
        let mut buf = [0u8; 4];
        mem.read_bytes(0x2000, &mut buf)?;
        assert_eq!(buf, [0xFFu8; 4]);
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn blocking_write_allowed_with_stdout_capability() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.io.stdout_access = true;
        let h = stdout_handle(&mut d)?;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"\n")?;
        mem.write_bytes(0x2000, &[0xFFu8; 4])?;
        let r = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.blocking-write-and-flush",
            &[CV::I32(h), CV::I32(0x1000), CV::I32(1), CV::I32(0x2000)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        let mut buf = [0u8; 4];
        mem.read_bytes(0x2000, &mut buf)?;
        assert_eq!(buf, [0u8; 4]);
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn blocking_write_accepts_exactly_three_args() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.io.stdout_access = true;
        let r = d.dispatch_core("wasi:cli/stdout@0.2.4", "get-stdout", &[], None)?;
        let h = match r.first() {
            Some(CV::I32(h)) => *h,
            _ => panic!("no handle"),
        };
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"\n")?;
        let out = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.blocking-write-and-flush",
            &[CV::I32(h), CV::I32(0x1000), CV::I32(1)],
            Some(&mem),
        )?;
        assert!(out.is_empty());
        Ok(())
    }
}

#[cfg(test)]
mod blocking_write_retptr_bounds_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    #[cfg(all(feature = "wasi-io", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn retptr_boundary() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = d.dispatch_core("wasi:cli/stdout@0.2.4", "get-stdout", &[], None)?;
        let handle = match h[0] {
            CV::I32(v) => v,
            _ => panic!("handle"),
        };
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x100, b"x")?;
        mem.write_bytes(0xFFFC, &[0xFF; 4])?;
        // retptr exactly size-4: discriminant fits, must be written
        d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.blocking-write-and-flush",
            &[CV::I32(handle), CV::I32(0x100), CV::I32(1), CV::I32(0xFFFC)],
            Some(&mem),
        )?;
        let mut out = [0xAAu8; 4];
        mem.read_bytes(0xFFFC, &mut out)?;
        assert_eq!(out, [0, 0, 0, 0]);
        // retptr size-2: the discriminant does not fit, so the write is an error
        // and the return area is left untouched.
        mem.write_bytes(0xFFFC, &[0xFF; 4])?;
        let r = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.blocking-write-and-flush",
            &[CV::I32(handle), CV::I32(0x100), CV::I32(1), CV::I32(0xFFFE)],
            Some(&mem),
        );
        assert!(
            r.is_err(),
            "a return area that does not fit in memory must be refused"
        );
        let mut out = [0u8; 4];
        mem.read_bytes(0xFFFC, &mut out)?;
        assert_eq!(out, [0xFF; 4]);
        Ok(())
    }
}

#[cfg(test)]
mod output_stream_write_zero_length_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    #[cfg(all(feature = "wasi-io", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn write_with_exactly_three_args_succeeds() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = d.dispatch_core("wasi:cli/stdout@0.2.4", "get-stdout", &[], None)?;
        let handle = match h[0] {
            CV::I32(v) => v,
            _ => panic!("handle"),
        };
        let mem = SliceMemory::with_size(0x10000);
        let r = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.write",
            &[CV::I32(handle), CV::I32(0x100), CV::I32(0)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        Ok(())
    }
}

#[cfg(test)]
mod output_stream_write_retptr_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    #[cfg(all(feature = "wasi-io", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn write_with_four_args_ok_and_two_args_rejected() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = d.dispatch_core("wasi:cli/stdout@0.2.4", "get-stdout", &[], None)?;
        let handle = match h[0] {
            CV::I32(v) => v,
            _ => panic!("handle"),
        };
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x200, &[0xFF; 4])?;
        let r = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.write",
            &[CV::I32(handle), CV::I32(0x100), CV::I32(0), CV::I32(0x200)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        let mut out = [0xAAu8; 4];
        mem.read_bytes(0x200, &mut out)?;
        assert_eq!(out, [0, 0, 0, 0]);
        let short = d.dispatch_core(
            "wasi:io/streams@0.2.4",
            "[method]output-stream.write",
            &[CV::I32(handle), CV::I32(0x100)],
            Some(&mem),
        );
        assert!(short.is_err());
        Ok(())
    }
}

#[cfg(test)]
mod output_stream_write_stderr_tests {
    use super::*;

    #[cfg(all(feature = "wasi-io", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn write_with_exactly_three_args_is_accepted() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let h = d.dispatch_core("wasi:cli/stderr", "get-stderr", &[], None)?;
        let handle = match h.first() {
            Some(CV::I32(x)) => *x,
            _ => panic!("expected handle"),
        };
        let mem = SliceMemory::with_size(0x1000);
        mem.write_bytes(0x100, b".")?;
        let r = d.dispatch_core(
            "wasi:io/streams",
            "[method]output-stream.write",
            &[CV::I32(handle), CV::I32(0x100), CV::I32(1)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        Ok(())
    }
}

#[cfg(test)]
mod get_random_bytes_i32_length_tests {
    use super::*;
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_random_bytes_accepts_i32_len() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.random.secure_random = true;
        d.read_buf_alloc = Some((0x2000, 64));
        let mem = SliceMemory::with_size(0x10000);
        let res = d.dispatch_core(
            "wasi:random/random",
            "get-random-bytes",
            &[CV::I32(16), CV::I32(0x100)],
            Some(&mem),
        )?;
        assert!(res.is_empty());
        let mut hdr = [0u8; 8];
        mem.read_bytes(0x100, &mut hdr)?;
        assert_eq!(u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]), 0x2000);
        assert_eq!(u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]), 16);
        Ok(())
    }
}

#[cfg(test)]
mod get_random_u64_capability_tests {
    use super::*;
    #[cfg(all(feature = "wasi-random", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_random_u64_requires_secure_random_capability() -> Result<()> {
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.random.secure_random = false;
        let denied = d.dispatch_core("wasi:random/random", "get-random-u64", &[], None);
        assert!(denied.is_err());
        d.capabilities.random.secure_random = true;
        let ok = d.dispatch_core("wasi:random/random", "get-random-u64", &[], None)?;
        assert_eq!(ok.len(), 1);
        assert!(matches!(ok[0], CV::I64(_)));
        Ok(())
    }
}

#[cfg(test)]
mod get_type_preopen_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_type_only_preopen_is_directory() -> Result<()> {
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let fd = d.add_preopen(std::env::temp_dir())?;
        let ty = |d: &mut WasiDispatcher, h: u32| -> Result<i32> {
            let r = d.dispatch_core(
                "wasi:filesystem/types",
                "[method]descriptor.get-type",
                &[CV::I32(h as i32), CV::I32(0x100)],
                None,
            )?;
            match r.get(1) {
                Some(CV::I32(t)) => Ok(*t),
                _ => Err(Error::runtime_error("unexpected result")),
            }
        };
        assert_eq!(ty(&mut d, fd)?, 3);
        // stdout (fd 1) is not a preopen
        if fd != 1 {
            assert_eq!(ty(&mut d, 1)?, 6);
        }
        if fd != 9999 {
            assert_eq!(ty(&mut d, 9999)?, 6);
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(feature = "wasi-filesystem", feature = "std"))]
mod preopen_descriptor_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p =
            std::env::temp_dir().join(format!("kiln_wasi_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[serial_test::serial]
    #[test]
    fn open_at_resolves_preopen_registered_only_in_fd_table() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let root = fixture("open_at_fd_table");
        std::fs::write(root.join("f.txt"), b"hello").unwrap();
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.filesystem.read_access = true;
        let fd = 7777u32;
        d.fd_table.insert(
            fd,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::PreopenDirectory(root.clone()),
                read: true,
                write: false,
                file: None,
            },
        );
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"f.txt")?;
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.open-at",
            &[
                CV::I32(fd as i32),
                CV::I32(0),
                CV::I32(0x1000),
                CV::I32(5),
                CV::I32(0),
                CV::I32(1),
            ],
            Some(&mem),
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
        let res = res?;
        assert_eq!(res.len(), 2);
        assert!(
            matches!(res[0], CV::I32(0)),
            "open-at through an fd_table preopen must succeed"
        );
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn stat_on_preopen_directory_reports_directory() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let root = fixture("stat_preopen");
        let mut d = WasiDispatcher::with_defaults()?;
        let fd = d.add_preopen(root.clone())?;
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat",
            &[CV::I32(fd as i32), CV::I32(0x100)],
            None,
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
        let res = res?;
        assert!(
            res.len() > 2,
            "stat of a preopen must be ok, not an error pair"
        );
        assert!(matches!(res[0], CV::I32(0)));
        assert!(
            matches!(res[1], CV::I32(3)),
            "descriptor-type must be directory (3)"
        );
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn stat_at_uses_the_named_preopen_not_another() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let root = fixture("stat_at_named");
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("a.txt"), b"hello").unwrap();
        let mut d = WasiDispatcher::with_defaults()?;
        let fa = d.add_preopen(a.clone())?;
        let fb = d.add_preopen(b.clone())?;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"a.txt")?;
        let r1 = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat-at",
            &[CV::I32(fa as i32), CV::I32(0), CV::I32(0x1000), CV::I32(5)],
            Some(&mem),
        );
        let r2 = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat-at",
            &[CV::I32(fb as i32), CV::I32(0), CV::I32(0x1000), CV::I32(5)],
            Some(&mem),
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
        let r1 = r1?;
        let r2 = r2?;
        assert!(r1.len() > 3);
        assert!(matches!(r1[0], CV::I32(0)));
        assert!(matches!(r1[1], CV::I32(6)));
        assert!(matches!(r1[3], CV::I64(5)));
        assert_eq!(r2.len(), 2);
        assert!(matches!(r2[0], CV::I32(1)));
        assert!(
            matches!(r2[1], CV::I32(20)),
            "a.txt is not in preopen b: no-entry"
        );
        Ok(())
    }

    #[serial_test::serial]
    #[test]
    fn stat_at_resolves_preopen_registered_only_in_fd_table() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let root = fixture("stat_at_fd_table");
        std::fs::write(root.join("f.txt"), b"hello").unwrap();
        let mut d = WasiDispatcher::with_defaults()?;
        let fd = 7778u32;
        d.fd_table.insert(
            fd,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::PreopenDirectory(root.clone()),
                read: true,
                write: false,
                file: None,
            },
        );
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"f.txt")?;
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.stat-at",
            &[CV::I32(fd as i32), CV::I32(0), CV::I32(0x1000), CV::I32(5)],
            Some(&mem),
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
        let res = res?;
        assert!(res.len() > 3);
        assert!(
            matches!(res[0], CV::I32(0)),
            "stat-at via fd_table preopen must succeed"
        );
        assert!(matches!(res[3], CV::I64(5)));
        Ok(())
    }
}

#[cfg(test)]
mod sync_data_regular_file_tests {
    use super::*;
    #[cfg(all(unix, feature = "wasi-filesystem", feature = "std"))]
    use kiln_foundation::values::Value as CV;

    #[cfg(all(unix, feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn sync_data_on_non_writable_descriptor_is_noop_success() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let fd = 7779u32;
        d.fd_table.insert(
            fd,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::RegularFile(std::path::PathBuf::from("/dev/null")),
                read: false,
                write: false,
                file: Some(std::fs::File::open("/dev/null").unwrap()),
            },
        );
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.sync-data",
            &[CV::I32(fd as i32), CV::I32(0x100)],
            None,
        )?;
        assert_eq!(res.len(), 1);
        assert!(
            matches!(res[0], CV::I32(0)),
            "sync-data on a descriptor not open for writing is a no-op success"
        );
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(target_os = "linux", feature = "wasi-filesystem", feature = "std"))]
mod sync_data_writable_preopen_tests {
    use super::*;
    use kiln_foundation::values::Value as CV;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p =
            std::env::temp_dir().join(format!("kiln_wasi_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[serial_test::serial]
    #[test]
    fn sync_data_on_writable_preopen_is_ok() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let root = fixture("sync_data_rw");
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.filesystem.write_access = true;
        let fd = d.add_preopen(root.clone())?;
        let res = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.sync-data",
            &[CV::I32(fd as i32), CV::I32(0x100)],
            None,
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
        let res = res?;
        assert_eq!(res.len(), 1);
        assert!(matches!(res[0], CV::I32(0)));
        Ok(())
    }
}

#[cfg(test)]
#[cfg(all(unix, feature = "wasi-filesystem", feature = "std"))]
mod readlink_at_tests {
    use super::*;
    use kiln_foundation::traits::SliceMemory;
    use kiln_foundation::values::Value as CV;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p =
            std::env::temp_dir().join(format!("kiln_wasi_{}_{}_{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[serial_test::serial]
    #[test]
    fn readlink_at_buffer_boundary() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let root = fixture("readlink");
        std::fs::write(root.join("target.txt"), b"x").unwrap();
        std::os::unix::fs::symlink("target.txt", root.join("link")).unwrap();
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.filesystem.read_access = true;
        let fd = d.add_preopen(root.clone())?;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, b"link")?;
        let args = [
            CV::I32(fd as i32),
            CV::I32(0x1000),
            CV::I32(4),
            CV::I32(0x100),
        ];
        d.read_buf_alloc = Some((0x2000, 10));
        let exact = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.readlink-at",
            &args,
            Some(&mem),
        );
        d.read_buf_alloc = Some((0x2000, 9));
        let small = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.readlink-at",
            &args,
            Some(&mem),
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&root);
        let exact = exact?;
        let small = small?;
        assert_eq!(exact.len(), 3);
        assert!(
            matches!(exact[0], CV::I32(0)),
            "target exactly filling the buffer must be returned"
        );
        assert!(matches!(exact[1], CV::I32(0x2000)));
        assert!(matches!(exact[2], CV::I32(10)));
        let mut got = [0u8; 10];
        mem.read_bytes(0x2000, &mut got)?;
        assert_eq!(&got, b"target.txt");
        assert_eq!(small.len(), 2);
        assert!(matches!(small[0], CV::I32(1)));
        assert!(matches!(small[1], CV::I32(18)));
        Ok(())
    }
}

#[cfg(test)]
mod get_directories_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn empty_preopen_list_header_len_written_at_retptr_plus_4() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        let mut d = WasiDispatcher::with_defaults()?;
        let mem = SliceMemory::with_size(0x10000);
        mem.write_bytes(0x1000, &[0xFFu8; 8])?;
        let r = d.dispatch_core(
            "wasi:filesystem/preopens",
            "get-directories",
            &[CV::I32(0x1000)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        let mut hdr = [0xAAu8; 8];
        mem.read_bytes(0x1000, &mut hdr)?;
        assert_eq!(hdr, [0u8; 8]);
        Ok(())
    }
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn second_preopen_entry_is_at_list_ptr_plus_12() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        let root =
            std::env::temp_dir().join(format!("kiln_get_directories_{}", std::process::id()));
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let mut d = WasiDispatcher::with_defaults()?;
        let h1 = d.add_preopen(a.clone())?;
        let h2 = d.add_preopen(b.clone())?;
        let la = a.to_string_lossy().len() as u32;
        let lb = b.to_string_lossy().len() as u32;
        d.preopens_alloc = Some((0x2000, vec![(0x3000, la), (0x3400, lb)]));
        let mem = SliceMemory::with_size(0x10000);
        let r = d.dispatch_core(
            "wasi:filesystem/preopens",
            "get-directories",
            &[CV::I32(0x1000)],
            Some(&mem),
        );
        let rd = |addr: u32| {
            let mut x = [0u8; 4];
            mem.read_bytes(addr, &mut x).unwrap();
            u32::from_le_bytes(x)
        };
        let vals = (
            rd(0x1000),
            rd(0x1004),
            rd(0x2000),
            rd(0x2004),
            rd(0x2008),
            rd(0x200C),
            rd(0x2010),
            rd(0x2014),
        );
        let _ = std::fs::remove_dir_all(&root);
        assert!(r?.is_empty());
        assert_eq!(vals.0, 0x2000);
        assert_eq!(vals.1, 2);
        assert_eq!(vals.2, h1);
        assert_eq!(vals.3, 0x3000);
        assert_eq!(vals.4, la);
        assert_eq!(vals.5, h2);
        assert_eq!(vals.6, 0x3400);
        assert_eq!(vals.7, lb);
        Ok(())
    }
}

#[cfg(test)]
mod monotonic_resolution_capability_tests {
    use super::*;
    #[cfg(feature = "wasi-clocks")]
    #[serial_test::serial]
    #[test]
    fn monotonic_resolution_respects_capability() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::values::Value as CV;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.clocks.monotonic_access = true;
        let ok = d.dispatch_core("wasi:clocks/monotonic-clock", "resolution", &[], None)?;
        assert!(matches!(ok.as_slice(), [CV::I64(1)]));
        d.capabilities.clocks.monotonic_access = false;
        let denied = d.dispatch_core("wasi:clocks/monotonic-clock", "resolution", &[], None);
        assert!(denied.is_err());
        Ok(())
    }
}

#[cfg(test)]
mod insecure_random_tests {
    use super::*;
    #[cfg(feature = "wasi-random")]
    #[serial_test::serial]
    #[test]
    fn insecure_random_bytes_accepts_i32_length() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.random.pseudo_random = true;
        d.read_buf_alloc = Some((0x2000, 64));
        let mem = SliceMemory::with_size(0x10000);
        let r = d.dispatch_core(
            "wasi:random/insecure",
            "get-insecure-random-bytes",
            &[CV::I32(16), CV::I32(0x100)],
            Some(&mem),
        )?;
        assert!(r.is_empty());
        let mut hdr = [0u8; 8];
        mem.read_bytes(0x100, &mut hdr)?;
        assert_eq!(u32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]), 0x2000);
        assert_eq!(u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]), 16);
        Ok(())
    }
    #[cfg(feature = "wasi-random")]
    #[serial_test::serial]
    #[test]
    fn insecure_random_u64_respects_capability() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::values::Value as CV;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.random.pseudo_random = true;
        let ok = d.dispatch_core("wasi:random/insecure", "get-insecure-random-u64", &[], None)?;
        assert!(matches!(ok.as_slice(), [CV::I64(_)]));
        d.capabilities.random.pseudo_random = false;
        let denied = d.dispatch_core("wasi:random/insecure", "get-insecure-random-u64", &[], None);
        assert!(denied.is_err());
        Ok(())
    }
}

#[cfg(test)]
mod call_import_tests {
    use super::*;

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn call_import_delegates_to_dispatch_core() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::HostImportHandler;
        use kiln_foundation::values::Value as CoreValue;
        let mut d = WasiDispatcher::with_defaults()?;
        let out = d.call_import("wasi:cli/environment@0.2.4", "get-environment", &[], None)?;
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0], CoreValue::I32(0)));
        assert!(matches!(out[1], CoreValue::I32(0)));
        let unknown = d.call_import("wasi:nothing/here", "nope", &[], None);
        assert!(unknown.is_err());
        Ok(())
    }
}

#[cfg(test)]
mod host_import_handler_tests {
    use super::*;

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn set_args_allocation_records_allocation() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        use kiln_foundation::HostImportHandler;
        let mut d = WasiDispatcher::with_defaults()?;
        d.clear_args_alloc();
        assert!(d.get_args_alloc().is_none());
        d.set_args_allocation(0x2000, vec![(0x3000, 4), (0x3100, 7)]);
        let got = d.get_args_alloc().cloned();
        assert_eq!(
            got,
            Some((0x2000u32, vec![(0x3000u32, 4u32), (0x3100u32, 7u32)]))
        );
        Ok(())
    }

    #[cfg(feature = "std")]
    #[serial_test::serial]
    #[test]
    fn trait_get_preopens_lists_preopens() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let tmp = std::env::temp_dir();
        let fd = d.add_preopen(tmp.clone())?;
        let got = kiln_foundation::HostImportHandler::get_preopens(&d);
        assert_eq!(got, vec![(fd, tmp.to_string_lossy().into_owned())]);
        Ok(())
    }
}

#[cfg(test)]
mod fs_error_code_is_directory_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn is_a_directory_maps_to_error_code_14() -> Result<()> {
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let e = std::io::Error::from(std::io::ErrorKind::IsADirectory);
        assert_eq!(WasiDispatcher::fs_error_code(&e), 14);
        assert_eq!(WasiDispatcher::fs_error_code(&e), fs_error::IS_DIRECTORY);
        Ok(())
    }
}

#[cfg(test)]
mod wall_clock_now_split_tests {
    use super::*;
    #[cfg(feature = "wasi-clocks")]
    #[serial_test::serial]
    #[test]
    fn wall_clock_now_writes_split_seconds_and_nanos() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.capabilities.clocks.realtime_access = true;
        let mem = SliceMemory::with_size(0x1000);
        let now_ns = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        };
        let tol: u128 = 10_000_000;
        for _ in 0..4 {
            let before = now_ns();
            let res = d.dispatch_core(
                "wasi:clocks/wall-clock",
                "now",
                &[CV::I32(0x100)],
                Some(&mem),
            )?;
            let after = now_ns();
            assert!(res.is_empty());
            let mut b = [0u8; 12];
            mem.read_bytes(0x100, &mut b)?;
            let secs = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
            let nanos = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
            assert!(nanos < 1_000_000_000);
            let total = secs as u128 * 1_000_000_000u128 + nanos as u128;
            assert!(total + tol >= before);
            assert!(total <= after + tol);
        }
        Ok(())
    }
}

#[cfg(test)]
mod get_type_fd_table_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn get_type_directory_if_in_either_preopen_list_or_fd_table() -> Result<()> {
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        d.preopens.push((9999u32, std::path::PathBuf::from(".")));
        d.fd_table.insert(
            8888u32,
            FileDescriptorEntry {
                fd_type: FileDescriptorType::PreopenDirectory(std::path::PathBuf::from(".")),
                read: true,
                write: false,
                file: None,
            },
        );
        let f = "[method]descriptor.get-type";
        let a = d.dispatch_core(
            "wasi:filesystem/types",
            f,
            &[CV::I32(9999), CV::I32(0x100)],
            None,
        )?;
        assert!(matches!(a.as_slice(), [CV::I32(0), CV::I32(3)]));
        let b = d.dispatch_core(
            "wasi:filesystem/types",
            f,
            &[CV::I32(8888), CV::I32(0x100)],
            None,
        )?;
        assert!(matches!(b.as_slice(), [CV::I32(0), CV::I32(3)]));
        let c = d.dispatch_core(
            "wasi:filesystem/types",
            f,
            &[CV::I32(7777), CV::I32(0x100)],
            None,
        )?;
        assert!(matches!(c.as_slice(), [CV::I32(0), CV::I32(6)]));
        Ok(())
    }
}

#[cfg(test)]
mod read_directory_entry_type_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn read_directory_reports_subdirectory_as_type_3() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let dir = std::env::temp_dir().join(format!("kiln_read_dir_subdir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let out = (|| -> Result<Vec<CV>> {
            let mut d = WasiDispatcher::with_defaults()?;
            let fd = d.add_preopen(dir.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let r = d.dispatch_core(
                "wasi:filesystem/types",
                "[method]descriptor.read-directory",
                &[CV::I32(fd as i32), CV::I32(0x100)],
                Some(&mem),
            )?;
            let h = match r.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h,
                _ => return Err(Error::runtime_error("read-directory failed")),
            };
            d.read_buf_alloc = Some((0x2000, 64));
            d.dispatch_core(
                "wasi:filesystem/types",
                "[method]directory-entry-stream.read-directory-entry",
                &[CV::I32(h), CV::I32(0x100)],
                Some(&mem),
            )
        })();
        let _ = std::fs::remove_dir_all(&dir);
        let ent = out?;
        assert_eq!(ent.len(), 5);
        assert!(matches!(&ent[0], CV::I32(0)));
        assert!(matches!(&ent[1], CV::I32(1)));
        assert!(matches!(&ent[2], CV::I32(3)));
        Ok(())
    }
    #[cfg(all(feature = "wasi-filesystem", feature = "std", unix))]
    #[serial_test::serial]
    #[test]
    fn read_directory_reports_symlink_as_type_5() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let dir =
            std::env::temp_dir().join(format!("kiln_read_dir_symlink_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink("nonexistent_target", dir.join("lnk")).unwrap();
        let out = (|| -> Result<Vec<CV>> {
            let mut d = WasiDispatcher::with_defaults()?;
            let fd = d.add_preopen(dir.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let r = d.dispatch_core(
                "wasi:filesystem/types",
                "[method]descriptor.read-directory",
                &[CV::I32(fd as i32), CV::I32(0x100)],
                Some(&mem),
            )?;
            let h = match r.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h,
                _ => return Err(Error::runtime_error("read-directory failed")),
            };
            d.read_buf_alloc = Some((0x2000, 64));
            d.dispatch_core(
                "wasi:filesystem/types",
                "[method]directory-entry-stream.read-directory-entry",
                &[CV::I32(h), CV::I32(0x100)],
                Some(&mem),
            )
        })();
        let _ = std::fs::remove_dir_all(&dir);
        let ent = out?;
        assert_eq!(ent.len(), 5);
        assert!(matches!(&ent[2], CV::I32(5)));
        Ok(())
    }
    #[cfg(all(feature = "wasi-filesystem", feature = "std", unix))]
    #[serial_test::serial]
    #[test]
    fn read_directory_reports_socket_entry_as_unknown_0() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let dir = std::env::temp_dir().join(format!("kiln_read_dir_socket_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let listener = match std::os::unix::net::UnixListener::bind(dir.join("s")) {
            Ok(l) => l,
            Err(_) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Ok(());
            },
        };
        let out = (|| -> Result<Vec<CV>> {
            let mut d = WasiDispatcher::with_defaults()?;
            let fd = d.add_preopen(dir.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            let r = d.dispatch_core(
                "wasi:filesystem/types",
                "[method]descriptor.read-directory",
                &[CV::I32(fd as i32), CV::I32(0x100)],
                Some(&mem),
            )?;
            let h = match r.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h,
                _ => return Err(Error::runtime_error("read-directory failed")),
            };
            d.read_buf_alloc = Some((0x2000, 64));
            d.dispatch_core(
                "wasi:filesystem/types",
                "[method]directory-entry-stream.read-directory-entry",
                &[CV::I32(h), CV::I32(0x100)],
                Some(&mem),
            )
        })();
        drop(listener);
        let _ = std::fs::remove_dir_all(&dir);
        let ent = out?;
        assert_eq!(ent.len(), 5);
        assert!(matches!(&ent[2], CV::I32(0)));
        Ok(())
    }
}

#[cfg(test)]
mod descriptor_write_directory_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn write_on_preopen_directory_is_is_directory() -> Result<()> {
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let mut d = WasiDispatcher::with_defaults()?;
        let fd = d.add_preopen(std::env::temp_dir())?;
        let mem = SliceMemory::with_size(0x10000);
        let r = d.dispatch_core(
            "wasi:filesystem/types",
            "[method]descriptor.write",
            &[
                CV::I32(fd as i32),
                CV::I32(0x1000),
                CV::I32(1),
                CV::I64(0),
                CV::I32(0x100),
            ],
            Some(&mem),
        )?;
        assert_eq!(r.len(), 2);
        assert!(matches!(&r[0], CV::I32(1)));
        assert!(matches!(&r[1], CV::I32(14)));
        Ok(())
    }
}

#[cfg(test)]
mod sync_data_write_flag_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn sync_data_on_preopen_honours_write_flag() -> Result<()> {
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let p =
            std::env::temp_dir().join(format!("kiln_sync_data_write_flag_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        let mut d = WasiDispatcher::with_defaults()?;
        let fd = d.add_preopen(p.clone())?;
        std::fs::remove_dir_all(&p).unwrap();
        let f = "[method]descriptor.sync-data";
        d.fd_table.get_mut(&fd).unwrap().write = false;
        let r1 = d.dispatch_core(
            "wasi:filesystem/types",
            f,
            &[CV::I32(fd as i32), CV::I32(0x100)],
            None,
        )?;
        assert_eq!(r1.len(), 1);
        assert!(matches!(&r1[0], CV::I32(0)));
        d.fd_table.get_mut(&fd).unwrap().write = true;
        let r2 = d.dispatch_core(
            "wasi:filesystem/types",
            f,
            &[CV::I32(fd as i32), CV::I32(0x100)],
            None,
        )?;
        assert_eq!(r2.len(), 2);
        assert!(matches!(&r2[0], CV::I32(1)));
        assert!(matches!(&r2[1], CV::I32(20)));
        Ok(())
    }
}

#[cfg(test)]
mod input_stream_read_length_tests {
    use super::*;
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn input_stream_read_honours_i64_length() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let dir =
            std::env::temp_dir().join(format!("kiln_input_stream_read_i64_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let content: Vec<u8> = (0..100u8).collect();
        std::fs::write(dir.join("data.bin"), &content).unwrap();
        let out = (|| -> Result<(Vec<CV>, Vec<u8>)> {
            let fs = "wasi:filesystem/types";
            let mut d = WasiDispatcher::with_defaults()?;
            let fd = d.add_preopen(dir.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            mem.write_bytes(0x1000, b"data.bin")?;
            let o = d.dispatch_core(
                fs,
                "[method]descriptor.open-at",
                &[
                    CV::I32(fd as i32),
                    CV::I32(0),
                    CV::I32(0x1000),
                    CV::I32(8),
                    CV::I32(0),
                    CV::I32(1),
                ],
                Some(&mem),
            )?;
            let h = match o.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h,
                _ => return Err(Error::runtime_error("open-at failed")),
            };
            let s = d.dispatch_core(
                fs,
                "[method]descriptor.read-via-stream",
                &[CV::I32(h), CV::I64(0)],
                Some(&mem),
            )?;
            let sh = match s.as_slice() {
                [CV::I32(0), CV::I32(x)] => *x,
                _ => return Err(Error::runtime_error("read-via-stream failed")),
            };
            d.read_buf_alloc = Some((0x2000, 4096));
            let r = d.dispatch_core(
                "wasi:io/streams",
                "[method]input-stream.read",
                &[CV::I32(sh), CV::I64(10)],
                Some(&mem),
            )?;
            let mut b = vec![0u8; 100];
            mem.read_bytes(0x2000, &mut b)?;
            Ok((r, b))
        })();
        let _ = std::fs::remove_dir_all(&dir);
        let (r, b) = out?;
        assert_eq!(r.len(), 3);
        assert!(matches!(&r[0], CV::I32(0)));
        assert!(matches!(&r[1], CV::I32(0x2000)));
        assert!(matches!(&r[2], CV::I32(10)));
        assert_eq!(&b[..10], &content[..10]);
        Ok(())
    }
    #[cfg(all(feature = "wasi-filesystem", feature = "std"))]
    #[serial_test::serial]
    #[test]
    fn input_stream_read_honours_i32_length() -> Result<()> {
        use kiln_foundation::MemoryAccessor as _;
        use kiln_foundation::traits::SliceMemory;
        use kiln_foundation::values::Value as CV;
        kiln_foundation::memory_init::MemoryInitializer::ensure_initialized()?;
        let dir =
            std::env::temp_dir().join(format!("kiln_input_stream_read_i32_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let content: Vec<u8> = (0..100u8).collect();
        std::fs::write(dir.join("data.bin"), &content).unwrap();
        let out = (|| -> Result<(Vec<CV>, Vec<u8>)> {
            let fs = "wasi:filesystem/types";
            let mut d = WasiDispatcher::with_defaults()?;
            let fd = d.add_preopen(dir.clone())?;
            let mem = SliceMemory::with_size(0x10000);
            mem.write_bytes(0x1000, b"data.bin")?;
            let o = d.dispatch_core(
                fs,
                "[method]descriptor.open-at",
                &[
                    CV::I32(fd as i32),
                    CV::I32(0),
                    CV::I32(0x1000),
                    CV::I32(8),
                    CV::I32(0),
                    CV::I32(1),
                ],
                Some(&mem),
            )?;
            let h = match o.as_slice() {
                [CV::I32(0), CV::I32(h)] => *h,
                _ => return Err(Error::runtime_error("open-at failed")),
            };
            let s = d.dispatch_core(
                fs,
                "[method]descriptor.read-via-stream",
                &[CV::I32(h), CV::I64(0)],
                Some(&mem),
            )?;
            let sh = match s.as_slice() {
                [CV::I32(0), CV::I32(x)] => *x,
                _ => return Err(Error::runtime_error("read-via-stream failed")),
            };
            d.read_buf_alloc = Some((0x2000, 4096));
            let r = d.dispatch_core(
                "wasi:io/streams",
                "[method]input-stream.read",
                &[CV::I32(sh), CV::I32(10)],
                Some(&mem),
            )?;
            let mut b = vec![0u8; 100];
            mem.read_bytes(0x2000, &mut b)?;
            Ok((r, b))
        })();
        let _ = std::fs::remove_dir_all(&dir);
        let (r, b) = out?;
        assert_eq!(r.len(), 3);
        assert!(matches!(&r[0], CV::I32(0)));
        assert!(matches!(&r[1], CV::I32(0x2000)));
        assert!(matches!(&r[2], CV::I32(10)));
        assert_eq!(&b[..10], &content[..10]);
        Ok(())
    }
}
