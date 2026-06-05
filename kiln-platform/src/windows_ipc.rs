//! Windows-specific IPC implementation using named pipes.
//!
//! Windows named-pipe IPC is not yet implemented. This module exists so the
//! `target_os = "windows"` arm of [`crate::ipc`] resolves and the runtime
//! compiles on Windows; every channel operation returns an explicit
//! not-implemented error rather than silently succeeding. Wiring up real
//! named-pipe transport is tracked separately.

use core::{
    fmt,
    time::Duration,
};
use std::{
    boxed::Box,
    string::String,
};

use kiln_error::{
    codes,
    Error,
    ErrorCategory,
    Result,
};

use crate::ipc::{
    ChannelId,
    ClientId,
    IpcChannel,
    Message,
};

/// Windows named-pipe implementation of IPC channel (not yet implemented).
pub struct WindowsNamedPipe {
    /// Pipe name for this channel
    pipe_name:  String,
    /// Channel ID
    channel_id: ChannelId,
}

impl fmt::Debug for WindowsNamedPipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowsNamedPipe")
            .field("pipe_name", &self.pipe_name)
            .field("channel_id", &self.channel_id)
            .finish()
    }
}

impl WindowsNamedPipe {
    /// Create a new Windows named-pipe channel handle.
    pub fn new(pipe_name: String) -> Self {
        Self {
            pipe_name,
            channel_id: ChannelId(rand::random()),
        }
    }
}

impl IpcChannel for WindowsNamedPipe {
    fn create_server(_name: &str) -> Result<Self>
    where
        Self: Sized,
    {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "Windows named-pipe IPC server not implemented",
        ))
    }

    fn connect(_name: &str) -> Result<Self>
    where
        Self: Sized,
    {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "Windows named-pipe IPC connect not implemented",
        ))
    }

    fn send(&self, _msg: &Message) -> Result<()> {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "Windows named-pipe IPC send not implemented",
        ))
    }

    fn receive(&self) -> Result<(Message, ClientId)> {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "Windows named-pipe IPC receive not implemented",
        ))
    }

    fn send_receive(&self, _msg: &Message, _timeout: Duration) -> Result<Message> {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "Windows named-pipe IPC send_receive not implemented",
        ))
    }

    fn reply(&self, _client: ClientId, _msg: &Message) -> Result<()> {
        Err(Error::new(
            ErrorCategory::System,
            codes::NOT_IMPLEMENTED,
            "Windows named-pipe IPC reply not implemented",
        ))
    }

    fn id(&self) -> ChannelId {
        self.channel_id
    }

    fn close(self) -> Result<()> {
        Ok(())
    }
}

// Simple counter-based ID generation, mirroring linux_ipc.
mod rand {
    use std::sync::atomic::{
        AtomicU64,
        Ordering,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    pub fn random() -> u64 {
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}
