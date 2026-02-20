//! Platform-agnostic Inter-Process Communication (IPC) abstractions.
//!
//! This module provides generic traits and implementations for IPC mechanisms
//! that can be specialized for different platforms (QNX message passing,
//! Linux domain sockets, Windows named pipes, etc.).

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{
    boxed::Box,
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use alloc::{
    boxed::Box,
    string::String,
    vec::Vec,
};
use core::{
    fmt::Debug,
    time::Duration,
};

use wrt_error::{
    Error,
    ErrorCategory,
    Result,
};
use wrt_sync::WrtMutex;

/// IPC message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    /// Load a WebAssembly module
    LoadModule,
    /// Execute a function
    Execute,
    /// Query status
    Status,
    /// Shutdown request
    Shutdown,
    /// Custom message type
    Custom(u32),
}

/// IPC message
#[derive(Debug, Clone)]
pub struct Message {
    /// Message type
    pub msg_type:       MessageType,
    /// Message payload
    pub data:           Vec<u8>,
    /// Optional correlation ID for request/response
    pub correlation_id: Option<u64>,
}

/// IPC channel trait for platform-specific implementations
pub trait IpcChannel: Send + Sync {
    /// Create a new server channel
    fn create_server(name: &str) -> Result<Self>
    where
        Self: Sized;

    /// Connect to an existing server
    fn connect(name: &str) -> Result<Self>
    where
        Self: Sized;

    /// Send a message (non-blocking)
    fn send(&self, msg: &Message) -> Result<()>;

    /// Receive a message (blocking)
    fn receive(&self) -> Result<(Message, ClientId)>;

    /// Send and wait for reply (synchronous RPC)
    fn send_receive(&self, msg: &Message, timeout: Duration) -> Result<Message>;

    /// Reply to a client
    fn reply(&self, client: ClientId, msg: &Message) -> Result<()>;

    /// Get channel identifier
    fn id(&self) -> ChannelId;

    /// Close the channel
    fn close(self) -> Result<()>;
}

/// Client identifier for replies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

/// Channel identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub u64);

/// Generic IPC server builder
pub struct IpcServerBuilder {
    name:             String,
    max_message_size: usize,
    max_clients:      usize,
    timeout:          Duration,
}

impl IpcServerBuilder {
    /// Create new IPC server builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:             name.into(),
            max_message_size: 64 * 1024, // 64KB default
            max_clients:      100,
            timeout:          Duration::from_secs(30),
        }
    }

    /// Set maximum message size
    pub fn with_max_message_size(mut self, size: usize) -> Self {
        self.max_message_size = size;
        self
    }

    /// Set maximum number of clients
    pub fn with_max_clients(mut self, max: usize) -> Self {
        self.max_clients = max;
        self
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the IPC server
    pub fn build(self) -> Result<Box<dyn IpcChannel>> {
        create_platform_channel(&self.name)
    }
}

/// Create platform-specific IPC channel
pub fn create_platform_channel(_name: &str) -> Result<Box<dyn IpcChannel>> {
    #[cfg(target_os = "nto")]
    {
        super::qnx_ipc::QnxChannel::create_server(_name)
            .map(|ch| Box::new(ch) as Box<dyn IpcChannel>)
    }

    #[cfg(target_os = "linux")]
    {
        super::linux_ipc::LinuxDomainSocket::create_server(_name)
            .map(|ch| Box::new(ch) as Box<dyn IpcChannel>)
    }

    #[cfg(target_os = "windows")]
    {
        super::windows_ipc::WindowsNamedPipe::create_server(_name)
            .map(|ch| Box::new(ch) as Box<dyn IpcChannel>)
    }

    #[cfg(not(any(target_os = "nto", target_os = "linux", target_os = "windows")))]
    {
        // Generic IPC implementation for platforms without native IPC
        Err(Error::runtime_execution_error(
            "IPC not supported on this platform",
        ))
    }
}

/// Zero-copy buffer for efficient data transfer
pub struct ZeroCopyBuffer {
    /// Shared memory region (platform-specific)
    _inner: Box<dyn SharedMemory>,
}

/// Trait for platform-specific shared memory
pub trait SharedMemory: Send + Sync {
    /// Get buffer size
    fn size(&self) -> usize;

    /// Get read-only view
    fn as_slice(&self) -> &[u8];

    /// Get mutable view
    fn as_mut_slice(&mut self) -> &mut [u8];

    /// Synchronize changes (if needed)
    fn sync(&self) -> Result<()>;
}

/// IPC handler trait for message processing
pub trait IpcHandler: Send + Sync {
    /// Handle incoming message
    fn handle_message(&self, msg: Message, client: ClientId) -> Result<Option<Message>>;

    /// Called when client connects
    fn on_connect(&self, _client: ClientId) -> Result<()> {
        Ok(())
    }

    /// Called when client disconnects
    fn on_disconnect(&self, _client: ClientId) {
        // Default: do nothing
    }
}

/// IPC server that processes messages
pub struct IpcServer {
    channel: Box<dyn IpcChannel>,
    handler: Box<dyn IpcHandler>,
    running: WrtMutex<bool>,
}

impl IpcServer {
    /// Create new IPC server
    pub fn new(channel: Box<dyn IpcChannel>, handler: Box<dyn IpcHandler>) -> Self {
        Self {
            channel,
            handler,
            running: WrtMutex::new(false),
        }
    }

    /// Run the server (blocking)
    pub fn run(&self) -> Result<()> {
        *self.running.lock() = true;

        while *self.running.lock() {
            match self.channel.receive() {
                Ok((msg, client)) => {
                    // Process message
                    match self.handler.handle_message(msg, client) {
                        Ok(Some(reply)) => {
                            let _ = self.channel.reply(client, &reply);
                        },
                        Ok(None) => {
                            // No reply needed
                        },
                        Err(e) => {
                            // Send error reply
                            let error_msg = Message {
                                msg_type:       MessageType::Custom(0xFFFF), // Error type
                                data:           e.to_string().into_bytes(),
                                correlation_id: None,
                            };
                            let _ = self.channel.reply(client, &error_msg);
                        },
                    }
                },
                Err(e) => {
                    if *self.running.lock() {
                        // Only log error if we're still running
                        eprintln!("IPC server error: {e}");
                    }
                },
            }
        }

        Ok(())
    }

    /// Stop the server
    pub fn stop(&self) {
        *self.running.lock() = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message {
            msg_type:       MessageType::LoadModule,
            data:           vec![1, 2, 3, 4],
            correlation_id: Some(12345),
        };

        assert_eq!(msg.msg_type, MessageType::LoadModule);
        assert_eq!(msg.data, vec![1, 2, 3, 4]);
        assert_eq!(msg.correlation_id, Some(12345));
    }

    #[test]
    fn test_ipc_builder() {
        let builder = IpcServerBuilder::new("test_channel")
            .with_max_message_size(1024 * 1024)
            .with_max_clients(50)
            .with_timeout(Duration::from_secs(60));

        assert_eq!(builder.name, "test_channel");
        assert_eq!(builder.max_message_size, 1024 * 1024);
        assert_eq!(builder.max_clients, 50);
        assert_eq!(builder.timeout, Duration::from_secs(60));
    }
}
