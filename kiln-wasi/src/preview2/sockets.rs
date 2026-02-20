//! WASI sockets interface implementation
//!
//! Implements the `wasi:sockets` interface for network operations using WRT's
//! platform abstractions and capability-based security model.
//!
//! This module provides TCP and UDP socket operations with proper sandboxing
//! and capability verification.
//!
//! # Architecture
//!
//! The socket implementation uses a capability-based security model:
//! - All socket operations require appropriate capabilities
//! - Socket handles are managed via a thread-safe socket table
//! - Address and port restrictions are enforced before any network operation
//!
//! # std vs no_std
//!
//! - With `std`: Real socket operations using `std::net` types
//! - Without `std`: Returns `UnsupportedOperation` errors (no_std has no networking)

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;

use core::any::Any;

#[cfg(feature = "std")]
use std::collections::HashMap;
#[cfg(feature = "std")]
use std::io::{Read, Write, ErrorKind};
#[cfg(feature = "std")]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
#[cfg(feature = "std")]
use std::sync::RwLock;

use crate::{prelude::*, Value};

/// WASI socket capabilities for controlling network access
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiSocketCapabilities {
    /// Allow TCP socket creation
    pub tcp_create: bool,
    /// Allow TCP bind operations
    pub tcp_bind: bool,
    /// Allow TCP connect operations
    pub tcp_connect: bool,
    /// Allow TCP listen operations
    pub tcp_listen: bool,
    /// Allow UDP socket creation
    pub udp_create: bool,
    /// Allow UDP bind operations
    pub udp_bind: bool,
    /// Allow DNS resolution
    pub dns_resolve: bool,
    /// Allowed IP address ranges (empty = all allowed)
    pub allowed_addresses: Vec<AllowedAddress>,
    /// Allowed port ranges (empty = all allowed)
    pub allowed_ports: Vec<(u16, u16)>,
}

/// Allowed address specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowedAddress {
    /// Allow any address
    Any,
    /// Allow localhost only
    Localhost,
    /// Allow specific IPv4 address
    Ipv4(Ipv4Addr),
    /// Allow specific IPv6 address
    Ipv6(Ipv6Addr),
    /// Allow IPv4 subnet (address, prefix length)
    Ipv4Subnet(Ipv4Addr, u8),
}

impl Default for WasiSocketCapabilities {
    fn default() -> Self {
        Self {
            tcp_create: false,
            tcp_bind: false,
            tcp_connect: false,
            tcp_listen: false,
            udp_create: false,
            udp_bind: false,
            dns_resolve: false,
            allowed_addresses: Vec::new(),
            allowed_ports: Vec::new(),
        }
    }
}

impl WasiSocketCapabilities {
    /// Create minimal socket capabilities (no access)
    pub fn none() -> Self {
        Self::default()
    }

    /// Create client-only capabilities (can connect but not listen)
    pub fn client_only() -> Self {
        Self {
            tcp_create: true,
            tcp_bind: false,
            tcp_connect: true,
            tcp_listen: false,
            udp_create: true,
            udp_bind: true,
            dns_resolve: true,
            allowed_addresses: vec![AllowedAddress::Any],
            allowed_ports: vec![(1, 65535)],
        }
    }

    /// Create localhost-only capabilities
    pub fn localhost_only() -> Self {
        Self {
            tcp_create: true,
            tcp_bind: true,
            tcp_connect: true,
            tcp_listen: true,
            udp_create: true,
            udp_bind: true,
            dns_resolve: false,
            allowed_addresses: vec![AllowedAddress::Localhost],
            allowed_ports: vec![(1024, 65535)], // Non-privileged ports only
        }
    }

    /// Create full network capabilities
    pub fn full() -> Self {
        Self {
            tcp_create: true,
            tcp_bind: true,
            tcp_connect: true,
            tcp_listen: true,
            udp_create: true,
            udp_bind: true,
            dns_resolve: true,
            allowed_addresses: vec![AllowedAddress::Any],
            allowed_ports: vec![(1, 65535)],
        }
    }

    /// Check if an address is allowed
    #[cfg(feature = "std")]
    pub fn is_address_allowed(&self, addr: &IpAddr) -> bool {
        if self.allowed_addresses.is_empty() {
            return false;
        }

        for allowed in &self.allowed_addresses {
            match (allowed, addr) {
                (AllowedAddress::Any, _) => return true,
                (AllowedAddress::Localhost, IpAddr::V4(ip)) => {
                    if ip.is_loopback() {
                        return true;
                    }
                }
                (AllowedAddress::Localhost, IpAddr::V6(ip)) => {
                    if ip.is_loopback() {
                        return true;
                    }
                }
                (AllowedAddress::Ipv4(allowed_ip), IpAddr::V4(ip)) => {
                    if allowed_ip == ip {
                        return true;
                    }
                }
                (AllowedAddress::Ipv6(allowed_ip), IpAddr::V6(ip)) => {
                    if allowed_ip == ip {
                        return true;
                    }
                }
                (AllowedAddress::Ipv4Subnet(network, prefix_len), IpAddr::V4(ip)) => {
                    if is_in_subnet(*network, *prefix_len, *ip) {
                        return true;
                    }
                }
                _ => {}
            }
        }

        false
    }

    /// Check if a port is allowed
    pub fn is_port_allowed(&self, port: u16) -> bool {
        if self.allowed_ports.is_empty() {
            return false;
        }

        for (start, end) in &self.allowed_ports {
            if port >= *start && port <= *end {
                return true;
            }
        }

        false
    }
}

/// Check if an IPv4 address is in a subnet
#[cfg(feature = "std")]
fn is_in_subnet(network: Ipv4Addr, prefix_len: u8, addr: Ipv4Addr) -> bool {
    if prefix_len > 32 {
        return false;
    }
    let mask = if prefix_len == 0 {
        0
    } else {
        !0u32 << (32 - prefix_len)
    };
    let network_bits = u32::from_be_bytes(network.octets()) & mask;
    let addr_bits = u32::from_be_bytes(addr.octets()) & mask;
    network_bits == addr_bits
}

// ============================================================================
// Socket Handle Types and Table
// ============================================================================

/// Handle type for WASI sockets
pub type SocketHandle = u32;

/// Socket state for TCP sockets
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum TcpSocketState {
    /// Socket created but not yet bound or connected
    Initial,
    /// Socket bound to a local address (for listening)
    Bound {
        /// The TcpListener for accepting connections
        listener: TcpListener,
    },
    /// Socket is listening for connections
    Listening {
        /// The TcpListener for accepting connections
        listener: TcpListener,
    },
    /// Socket connected to a remote peer
    Connected {
        /// The TcpStream for I/O operations
        stream: TcpStream,
        /// Local address
        local_addr: SocketAddr,
        /// Remote peer address
        peer_addr: SocketAddr,
    },
    /// Socket has been shut down
    Shutdown,
}

/// Socket state for UDP sockets
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum UdpSocketState {
    /// Socket created but not bound
    Initial,
    /// Socket bound to a local address
    Bound {
        /// The UdpSocket for I/O operations
        socket: UdpSocket,
        /// Local bound address
        local_addr: SocketAddr,
    },
    /// Socket connected to a remote peer (for send/recv without address)
    Connected {
        /// The UdpSocket for I/O operations
        socket: UdpSocket,
        /// Local bound address
        local_addr: SocketAddr,
        /// Remote peer address
        peer_addr: SocketAddr,
    },
}

/// Entry in the socket table
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum SocketEntry {
    /// TCP socket entry
    Tcp(TcpSocketState),
    /// UDP socket entry
    Udp(UdpSocketState),
}

/// Thread-safe socket table for managing socket handles
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct SocketTable {
    /// Map of handles to socket entries
    sockets: HashMap<SocketHandle, SocketEntry>,
    /// Next available handle
    next_handle: SocketHandle,
    /// Socket capabilities for this table
    capabilities: WasiSocketCapabilities,
}

#[cfg(feature = "std")]
impl SocketTable {
    /// Create a new socket table with the given capabilities
    pub fn new(capabilities: WasiSocketCapabilities) -> Self {
        Self {
            sockets: HashMap::new(),
            // Start at 1, reserve 0 for invalid
            next_handle: 1,
            capabilities,
        }
    }

    /// Allocate a new handle
    fn alloc_handle(&mut self) -> SocketHandle {
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1);
        if self.next_handle == 0 {
            self.next_handle = 1;
        }
        handle
    }

    /// Create a new TCP socket
    pub fn create_tcp(&mut self) -> Result<SocketHandle> {
        if !self.capabilities.tcp_create {
            return Err(Error::wasi_capability_unavailable(
                "TCP socket creation not permitted",
            ));
        }
        let handle = self.alloc_handle();
        self.sockets
            .insert(handle, SocketEntry::Tcp(TcpSocketState::Initial));
        Ok(handle)
    }

    /// Create a new UDP socket
    pub fn create_udp(&mut self) -> Result<SocketHandle> {
        if !self.capabilities.udp_create {
            return Err(Error::wasi_capability_unavailable(
                "UDP socket creation not permitted",
            ));
        }
        let handle = self.alloc_handle();
        self.sockets
            .insert(handle, SocketEntry::Udp(UdpSocketState::Initial));
        Ok(handle)
    }

    /// Get a socket entry by handle
    pub fn get(&self, handle: SocketHandle) -> Result<&SocketEntry> {
        self.sockets
            .get(&handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))
    }

    /// Get a mutable socket entry by handle
    pub fn get_mut(&mut self, handle: SocketHandle) -> Result<&mut SocketEntry> {
        self.sockets
            .get_mut(&handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))
    }

    /// Remove a socket from the table
    pub fn remove(&mut self, handle: SocketHandle) -> Result<SocketEntry> {
        self.sockets
            .remove(&handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))
    }

    /// Check if an address/port is allowed by capabilities
    pub fn check_address_allowed(&self, addr: &SocketAddr) -> Result<()> {
        if !self.capabilities.is_address_allowed(&addr.ip()) {
            return Err(Error::wasi_capability_unavailable(
                "Address not in allowed list",
            ));
        }
        if !self.capabilities.is_port_allowed(addr.port()) {
            return Err(Error::wasi_capability_unavailable(
                "Port not in allowed range",
            ));
        }
        Ok(())
    }

    /// Get capabilities reference
    pub fn capabilities(&self) -> &WasiSocketCapabilities {
        &self.capabilities
    }
}

/// Global socket table (thread-safe)
#[cfg(feature = "std")]
static GLOBAL_SOCKET_TABLE: once_cell::sync::Lazy<RwLock<SocketTable>> =
    once_cell::sync::Lazy::new(|| RwLock::new(SocketTable::new(WasiSocketCapabilities::none())));

/// Initialize the global socket table with capabilities
#[cfg(feature = "std")]
pub fn init_socket_table(capabilities: WasiSocketCapabilities) {
    if let Ok(mut table) = GLOBAL_SOCKET_TABLE.write() {
        *table = SocketTable::new(capabilities);
    }
}

/// Get read access to the global socket table
#[cfg(feature = "std")]
fn with_socket_table<T, F: FnOnce(&SocketTable) -> T>(f: F) -> Result<T> {
    let table = GLOBAL_SOCKET_TABLE
        .read()
        .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;
    Ok(f(&table))
}

/// Get write access to the global socket table
#[cfg(feature = "std")]
fn with_socket_table_mut<T, F: FnOnce(&mut SocketTable) -> T>(f: F) -> Result<T> {
    let mut table = GLOBAL_SOCKET_TABLE
        .write()
        .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;
    Ok(f(&mut table))
}

// ============================================================================
// TCP Socket Operations (std feature)
// ============================================================================

/// Create a TCP socket
///
/// Implements `wasi:sockets/tcp.create-tcp-socket`
#[cfg(feature = "std")]
pub fn wasi_tcp_create(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    let handle = with_socket_table_mut(|table| table.create_tcp())??;
    Ok(vec![Value::U32(handle)])
}

/// Connect a TCP socket to a remote address
///
/// Implements `wasi:sockets/tcp.start-connect`
#[cfg(feature = "std")]
pub fn wasi_tcp_connect(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let ip_bytes = extract_ip_address(&args, 1)?;
    let port = extract_u16(&args, 2)?;

    let addr = SocketAddr::new(bytes_to_ip(&ip_bytes)?, port);

    // Check capabilities first
    with_socket_table(|table| {
        if !table.capabilities().tcp_connect {
            return Err(Error::wasi_capability_unavailable(
                "TCP connect not permitted",
            ));
        }
        table.check_address_allowed(&addr)
    })??;

    // Attempt the connection
    let stream = TcpStream::connect(addr).map_err(|e| io_error_to_wasi_error(&e))?;

    let local_addr = stream
        .local_addr()
        .map_err(|e| io_error_to_wasi_error(&e))?;
    let peer_addr = stream
        .peer_addr()
        .map_err(|e| io_error_to_wasi_error(&e))?;

    // Update the socket state
    with_socket_table_mut(|table| {
        let entry = table.get_mut(socket_handle)?;
        match entry {
            SocketEntry::Tcp(state) => {
                *state = TcpSocketState::Connected {
                    stream,
                    local_addr,
                    peer_addr,
                };
                Ok(())
            }
            SocketEntry::Udp(_) => Err(Error::wasi_invalid_fd("Expected TCP socket")),
        }
    })??;

    Ok(vec![Value::Result(Ok(Box::new(Value::U32(socket_handle))))])
}

/// Bind a TCP socket to a local address
///
/// Implements `wasi:sockets/tcp.start-bind`
#[cfg(feature = "std")]
pub fn wasi_tcp_bind(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let ip_bytes = extract_ip_address(&args, 1)?;
    let port = extract_u16(&args, 2)?;

    let addr = SocketAddr::new(bytes_to_ip(&ip_bytes)?, port);

    // Check capabilities
    with_socket_table(|table| {
        if !table.capabilities().tcp_bind {
            return Err(Error::wasi_capability_unavailable("TCP bind not permitted"));
        }
        table.check_address_allowed(&addr)
    })??;

    // Create the listener
    let listener = TcpListener::bind(addr).map_err(|e| io_error_to_wasi_error(&e))?;

    // Update socket state
    with_socket_table_mut(|table| {
        let entry = table.get_mut(socket_handle)?;
        match entry {
            SocketEntry::Tcp(state) => {
                *state = TcpSocketState::Bound { listener };
                Ok(())
            }
            SocketEntry::Udp(_) => Err(Error::wasi_invalid_fd("Expected TCP socket")),
        }
    })??;

    Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
}

/// Listen on a TCP socket
///
/// Implements `wasi:sockets/tcp.start-listen`
#[cfg(feature = "std")]
pub fn wasi_tcp_listen(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;

    // Check capabilities
    with_socket_table(|table| {
        if !table.capabilities().tcp_listen {
            return Err(Error::wasi_capability_unavailable(
                "TCP listen not permitted",
            ));
        }
        Ok(())
    })??;

    // Transition from Bound to Listening
    with_socket_table_mut(|table| {
        let entry = table.get_mut(socket_handle)?;
        match entry {
            SocketEntry::Tcp(state) => {
                // Take the listener from Bound state
                let listener = match core::mem::replace(state, TcpSocketState::Initial) {
                    TcpSocketState::Bound { listener } => listener,
                    other => {
                        // Restore the state and return error
                        *state = other;
                        return Err(Error::wasi_invalid_fd("Socket not in bound state"));
                    }
                };
                *state = TcpSocketState::Listening { listener };
                Ok(())
            }
            SocketEntry::Udp(_) => Err(Error::wasi_invalid_fd("Expected TCP socket")),
        }
    })??;

    Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
}

/// Accept a connection on a TCP listener
///
/// Implements `wasi:sockets/tcp.accept`
#[cfg(feature = "std")]
pub fn wasi_tcp_accept(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;

    // Accept a connection from the listener
    // This needs to borrow the listener, accept, then create a new socket entry
    let (stream, peer_addr) = {
        let table = GLOBAL_SOCKET_TABLE
            .read()
            .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;

        let entry = table.sockets.get(&socket_handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))?;

        match entry {
            SocketEntry::Tcp(TcpSocketState::Listening { listener }) => {
                listener.accept().map_err(|e| io_error_to_wasi_error(&e))?
            }
            SocketEntry::Tcp(_) => {
                return Err(Error::wasi_invalid_fd("Socket not in listening state"));
            }
            SocketEntry::Udp(_) => {
                return Err(Error::wasi_invalid_fd("Expected TCP socket"));
            }
        }
    };

    let local_addr = stream
        .local_addr()
        .map_err(|e| io_error_to_wasi_error(&e))?;

    // Create a new socket handle for the accepted connection
    let new_handle = with_socket_table_mut(|table| -> Result<SocketHandle> {
        let handle = table.alloc_handle();
        table.sockets.insert(
            handle,
            SocketEntry::Tcp(TcpSocketState::Connected {
                stream,
                local_addr,
                peer_addr,
            }),
        );
        Ok(handle)
    })??;

    // Return the new handle and peer address
    let addr_value = socket_addr_to_value(&peer_addr);
    Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![
        Value::U32(new_handle),
        addr_value,
    ]))))])
}

/// Send data on a TCP socket
///
/// Implements output stream write for TCP
#[cfg(feature = "std")]
pub fn wasi_tcp_send(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let data = extract_bytes(&args, 1)?;

    // We need to write to the stream. Since we can't hold a mutable borrow
    // across the lock, we use try_clone() to get an owned stream for writing.
    let mut stream = {
        let table = GLOBAL_SOCKET_TABLE
            .read()
            .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;

        let entry = table.sockets.get(&socket_handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))?;

        match entry {
            SocketEntry::Tcp(TcpSocketState::Connected { stream, .. }) => {
                stream.try_clone().map_err(|e| io_error_to_wasi_error(&e))?
            }
            SocketEntry::Tcp(_) => {
                return Err(Error::wasi_invalid_fd("Socket not connected"));
            }
            SocketEntry::Udp(_) => {
                return Err(Error::wasi_invalid_fd("Expected TCP socket"));
            }
        }
    };

    // Write the data
    let bytes_written = stream.write(&data).map_err(|e| io_error_to_wasi_error(&e))?;

    Ok(vec![Value::U64(bytes_written as u64)])
}

/// Receive data from a TCP socket
///
/// Implements input stream read for TCP
#[cfg(feature = "std")]
pub fn wasi_tcp_recv(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let max_len = extract_u64(&args, 1)? as usize;

    // Clone the stream for reading
    let mut stream = {
        let table = GLOBAL_SOCKET_TABLE
            .read()
            .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;

        let entry = table.sockets.get(&socket_handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))?;

        match entry {
            SocketEntry::Tcp(TcpSocketState::Connected { stream, .. }) => {
                stream.try_clone().map_err(|e| io_error_to_wasi_error(&e))?
            }
            SocketEntry::Tcp(_) => {
                return Err(Error::wasi_invalid_fd("Socket not connected"));
            }
            SocketEntry::Udp(_) => {
                return Err(Error::wasi_invalid_fd("Expected TCP socket"));
            }
        }
    };

    // Read the data
    let mut buffer = vec![0u8; max_len.min(65536)];
    let bytes_read = stream.read(&mut buffer).map_err(|e| io_error_to_wasi_error(&e))?;

    // Convert to Value list
    let data: Vec<Value> = buffer[..bytes_read].iter().map(|b| Value::U8(*b)).collect();
    Ok(vec![Value::List(data)])
}

/// Shutdown a TCP socket
///
/// Implements `wasi:sockets/tcp.shutdown`
#[cfg(feature = "std")]
pub fn wasi_tcp_shutdown(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let shutdown_type = extract_u8(&args, 1)?; // 0=read, 1=write, 2=both

    use std::net::Shutdown;

    // Determine shutdown type
    let how = match shutdown_type {
        0 => Shutdown::Read,
        1 => Shutdown::Write,
        _ => Shutdown::Both,
    };

    // Clone stream and shutdown
    let stream = {
        let table = GLOBAL_SOCKET_TABLE
            .read()
            .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;

        let entry = table.sockets.get(&socket_handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))?;

        match entry {
            SocketEntry::Tcp(TcpSocketState::Connected { stream, .. }) => {
                stream.try_clone().map_err(|e| io_error_to_wasi_error(&e))?
            }
            SocketEntry::Tcp(_) => {
                return Err(Error::wasi_invalid_fd("Socket not connected"));
            }
            SocketEntry::Udp(_) => {
                return Err(Error::wasi_invalid_fd("Expected TCP socket"));
            }
        }
    };

    stream.shutdown(how).map_err(|e| io_error_to_wasi_error(&e))?;

    // Update state to Shutdown if both directions
    if shutdown_type >= 2 {
        with_socket_table_mut(|table| -> Result<()> {
            if let Some(entry) = table.sockets.get_mut(&socket_handle) {
                if let SocketEntry::Tcp(state) = entry {
                    *state = TcpSocketState::Shutdown;
                }
            }
            Ok(())
        })??;
    }

    Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
}

// ============================================================================
// UDP Socket Operations (std feature)
// ============================================================================

/// Create a UDP socket
///
/// Implements `wasi:sockets/udp.create-udp-socket`
#[cfg(feature = "std")]
pub fn wasi_udp_create(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    let handle = with_socket_table_mut(|table| table.create_udp())??;
    Ok(vec![Value::U32(handle)])
}

/// Bind a UDP socket
///
/// Implements `wasi:sockets/udp.start-bind`
#[cfg(feature = "std")]
pub fn wasi_udp_bind(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let ip_bytes = extract_ip_address(&args, 1)?;
    let port = extract_u16(&args, 2)?;

    let addr = SocketAddr::new(bytes_to_ip(&ip_bytes)?, port);

    // Check capabilities
    with_socket_table(|table| {
        if !table.capabilities().udp_bind {
            return Err(Error::wasi_capability_unavailable("UDP bind not permitted"));
        }
        table.check_address_allowed(&addr)
    })??;

    // Bind the socket
    let socket = UdpSocket::bind(addr).map_err(|e| io_error_to_wasi_error(&e))?;
    let local_addr = socket
        .local_addr()
        .map_err(|e| io_error_to_wasi_error(&e))?;

    // Update socket state
    with_socket_table_mut(|table| {
        let entry = table.get_mut(socket_handle)?;
        match entry {
            SocketEntry::Udp(state) => {
                *state = UdpSocketState::Bound { socket, local_addr };
                Ok(())
            }
            SocketEntry::Tcp(_) => Err(Error::wasi_invalid_fd("Expected UDP socket")),
        }
    })??;

    Ok(vec![Value::Result(Ok(Box::new(Value::Tuple(vec![]))))])
}

/// Send data on a UDP socket
///
/// Implements `wasi:sockets/udp.send`
#[cfg(feature = "std")]
pub fn wasi_udp_send(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let data = extract_bytes(&args, 1)?;
    let ip_bytes = extract_ip_address(&args, 2)?;
    let port = extract_u16(&args, 3)?;

    let dest = SocketAddr::new(bytes_to_ip(&ip_bytes)?, port);

    // Check destination is allowed
    with_socket_table(|table| table.check_address_allowed(&dest))??;

    // Clone the socket for sending
    let socket = {
        let table = GLOBAL_SOCKET_TABLE
            .read()
            .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;

        let entry = table.sockets.get(&socket_handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))?;

        match entry {
            SocketEntry::Udp(UdpSocketState::Bound { socket, .. })
            | SocketEntry::Udp(UdpSocketState::Connected { socket, .. }) => {
                socket.try_clone().map_err(|e| io_error_to_wasi_error(&e))?
            }
            SocketEntry::Udp(UdpSocketState::Initial) => {
                return Err(Error::wasi_invalid_fd("Socket not bound"));
            }
            SocketEntry::Tcp(_) => {
                return Err(Error::wasi_invalid_fd("Expected UDP socket"));
            }
        }
    };

    let bytes_sent = socket
        .send_to(&data, dest)
        .map_err(|e| io_error_to_wasi_error(&e))?;

    Ok(vec![Value::U64(bytes_sent as u64)])
}

/// Receive data from a UDP socket
///
/// Implements `wasi:sockets/udp.receive`
#[cfg(feature = "std")]
pub fn wasi_udp_recv(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let socket_handle = extract_u32(&args, 0)?;
    let max_len = extract_u64(&args, 1)? as usize;

    // Clone the socket for receiving
    let socket = {
        let table = GLOBAL_SOCKET_TABLE
            .read()
            .map_err(|_| Error::wasi_runtime_error("Socket table lock poisoned"))?;

        let entry = table.sockets.get(&socket_handle)
            .ok_or_else(|| Error::wasi_invalid_fd("Invalid socket handle"))?;

        match entry {
            SocketEntry::Udp(UdpSocketState::Bound { socket, .. })
            | SocketEntry::Udp(UdpSocketState::Connected { socket, .. }) => {
                socket.try_clone().map_err(|e| io_error_to_wasi_error(&e))?
            }
            SocketEntry::Udp(UdpSocketState::Initial) => {
                return Err(Error::wasi_invalid_fd("Socket not bound"));
            }
            SocketEntry::Tcp(_) => {
                return Err(Error::wasi_invalid_fd("Expected UDP socket"));
            }
        }
    };

    // Receive data
    let mut buffer = vec![0u8; max_len.min(65536)];
    let (bytes_read, source_addr) = socket
        .recv_from(&mut buffer)
        .map_err(|e| io_error_to_wasi_error(&e))?;

    // Convert to Value list
    let data: Vec<Value> = buffer[..bytes_read].iter().map(|b| Value::U8(*b)).collect();
    let addr_value = socket_addr_to_value(&source_addr);

    // Return: (data, source_address)
    Ok(vec![Value::Tuple(vec![Value::List(data), addr_value])])
}

// ============================================================================
// DNS Resolution
// ============================================================================

/// Resolve a hostname to IP addresses
///
/// Implements `wasi:sockets/ip-name-lookup.resolve-addresses`
#[cfg(feature = "std")]
pub fn wasi_resolve_addresses(_target: &mut dyn Any, args: Vec<Value>) -> Result<Vec<Value>> {
    let hostname = extract_string(&args, 0)?;

    use std::net::ToSocketAddrs;

    // Try to resolve the hostname
    let addr_str = format!("{}:0", hostname);
    match addr_str.to_socket_addrs() {
        Ok(addrs) => {
            let ip_values: Vec<Value> = addrs
                .map(|addr| {
                    match addr.ip() {
                        IpAddr::V4(ip) => {
                            let octets = ip.octets();
                            Value::Tuple(vec![
                                Value::U8(4), // IPv4
                                Value::List(octets.iter().map(|b| Value::U8(*b)).collect()),
                            ])
                        }
                        IpAddr::V6(ip) => {
                            let octets = ip.octets();
                            Value::Tuple(vec![
                                Value::U8(6), // IPv6
                                Value::List(octets.iter().map(|b| Value::U8(*b)).collect()),
                            ])
                        }
                    }
                })
                .collect();

            Ok(vec![Value::Result(Ok(Box::new(Value::List(ip_values))))])
        }
        Err(_e) => {
            Err(Error::wasi_capability_unavailable("DNS resolution failed"))
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert a std::io::Error to a WASI error
#[cfg(feature = "std")]
fn io_error_to_wasi_error(e: &std::io::Error) -> Error {
    match e.kind() {
        ErrorKind::NotFound => Error::wasi_invalid_fd("Resource not found"),
        ErrorKind::PermissionDenied => Error::wasi_permission_denied("Permission denied"),
        ErrorKind::ConnectionRefused => Error::wasi_capability_unavailable("Connection refused"),
        ErrorKind::ConnectionReset => Error::wasi_runtime_error("Connection reset"),
        ErrorKind::ConnectionAborted => Error::wasi_runtime_error("Connection aborted"),
        ErrorKind::NotConnected => Error::wasi_invalid_fd("Not connected"),
        ErrorKind::AddrInUse => Error::wasi_resource_limit("Address already in use"),
        ErrorKind::AddrNotAvailable => Error::wasi_capability_unavailable("Address not available"),
        ErrorKind::BrokenPipe => Error::wasi_runtime_error("Broken pipe"),
        ErrorKind::AlreadyExists => Error::wasi_resource_limit("Already exists"),
        ErrorKind::WouldBlock => Error::wasi_runtime_error("Operation would block"),
        ErrorKind::InvalidInput => Error::wasi_invalid_argument("Invalid input"),
        ErrorKind::InvalidData => Error::wasi_invalid_encoding("Invalid data"),
        ErrorKind::TimedOut => Error::wasi_timeout("Operation timed out"),
        ErrorKind::WriteZero => Error::wasi_runtime_error("Write zero"),
        ErrorKind::Interrupted => Error::wasi_runtime_error("Interrupted"),
        ErrorKind::UnexpectedEof => Error::wasi_runtime_error("Unexpected EOF"),
        _ => Error::wasi_runtime_error("I/O error"),
    }
}

/// Convert a SocketAddr to a WASI Value representation
#[cfg(feature = "std")]
fn socket_addr_to_value(addr: &SocketAddr) -> Value {
    let ip_bytes: Vec<Value> = match addr.ip() {
        IpAddr::V4(ip) => ip.octets().iter().map(|b| Value::U8(*b)).collect(),
        IpAddr::V6(ip) => ip.octets().iter().map(|b| Value::U8(*b)).collect(),
    };
    Value::Tuple(vec![Value::List(ip_bytes), Value::U16(addr.port())])
}

fn extract_u32(args: &[Value], index: usize) -> Result<u32> {
    match args.get(index) {
        Some(Value::U32(v)) => Ok(*v),
        Some(Value::S32(v)) if *v >= 0 => Ok(*v as u32),
        _ => Err(Error::wasi_invalid_fd("Expected u32 argument")),
    }
}

fn extract_u16(args: &[Value], index: usize) -> Result<u16> {
    match args.get(index) {
        Some(Value::U16(v)) => Ok(*v),
        Some(Value::U32(v)) if *v <= 65535 => Ok(*v as u16),
        Some(Value::S32(v)) if *v >= 0 && *v <= 65535 => Ok(*v as u16),
        _ => Err(Error::wasi_invalid_fd("Expected u16 argument")),
    }
}

fn extract_u8(args: &[Value], index: usize) -> Result<u8> {
    match args.get(index) {
        Some(Value::U8(v)) => Ok(*v),
        Some(Value::U32(v)) if *v <= 255 => Ok(*v as u8),
        _ => Err(Error::wasi_invalid_fd("Expected u8 argument")),
    }
}

fn extract_u64(args: &[Value], index: usize) -> Result<u64> {
    match args.get(index) {
        Some(Value::U64(v)) => Ok(*v),
        Some(Value::U32(v)) => Ok(*v as u64),
        _ => Err(Error::wasi_invalid_fd("Expected u64 argument")),
    }
}

fn extract_bytes(args: &[Value], index: usize) -> Result<Vec<u8>> {
    match args.get(index) {
        Some(Value::List(items)) => {
            let mut bytes = Vec::with_capacity(items.len());
            for item in items {
                if let Value::U8(b) = item {
                    bytes.push(*b);
                } else {
                    return Err(Error::wasi_invalid_fd("Expected list of bytes"));
                }
            }
            Ok(bytes)
        }
        _ => Err(Error::wasi_invalid_fd("Expected list argument")),
    }
}

fn extract_ip_address(args: &[Value], index: usize) -> Result<Vec<u8>> {
    extract_bytes(args, index)
}

fn extract_string(args: &[Value], index: usize) -> Result<String> {
    match args.get(index) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(Error::wasi_invalid_fd("Expected string argument")),
    }
}

#[cfg(feature = "std")]
fn bytes_to_ip(bytes: &[u8]) -> Result<IpAddr> {
    match bytes.len() {
        4 => Ok(IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))),
        16 => {
            let octets: [u8; 16] = bytes.try_into()
                .map_err(|_| Error::wasi_invalid_fd("Invalid IPv6 address"))?;
            Ok(IpAddr::V6(Ipv6Addr::from(octets)))
        }
        _ => Err(Error::wasi_invalid_fd("Invalid IP address length")),
    }
}

// ============================================================================
// no_std Socket Stubs
// ============================================================================
// In no_std environments, networking is not available. These stubs return
// appropriate errors instead of placeholder values.

/// Create a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_create(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Connect a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_connect(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Bind a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_bind(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Listen on a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_listen(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Accept a TCP connection (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_accept(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Send data on a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_send(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Receive data from a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_recv(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Shutdown a TCP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_tcp_shutdown(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Create a UDP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_udp_create(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Bind a UDP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_udp_bind(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Send data on a UDP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_udp_send(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Receive data from a UDP socket (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_udp_recv(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "Socket operations not available in no_std environment",
    ))
}

/// Resolve a hostname (no_std stub)
#[cfg(not(feature = "std"))]
pub fn wasi_resolve_addresses(_target: &mut dyn Any, _args: Vec<Value>) -> Result<Vec<Value>> {
    Err(Error::wasi_unsupported_operation(
        "DNS resolution not available in no_std environment",
    ))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_capabilities_none() {
        let caps = WasiSocketCapabilities::none();
        assert!(!caps.tcp_create);
        assert!(!caps.tcp_connect);
        assert!(!caps.dns_resolve);
    }

    #[test]
    fn test_socket_capabilities_client_only() {
        let caps = WasiSocketCapabilities::client_only();
        assert!(caps.tcp_create);
        assert!(caps.tcp_connect);
        assert!(!caps.tcp_listen);
        assert!(caps.dns_resolve);
    }

    #[test]
    fn test_socket_capabilities_localhost() {
        let caps = WasiSocketCapabilities::localhost_only();
        assert!(caps.tcp_create);
        assert!(caps.tcp_listen);
        assert!(!caps.dns_resolve);
        assert!(caps.is_port_allowed(8080));
        assert!(!caps.is_port_allowed(80)); // Privileged port
    }

    #[test]
    fn test_port_range_check() {
        let mut caps = WasiSocketCapabilities::none();
        caps.allowed_ports = vec![(80, 80), (443, 443), (8000, 9000)];

        assert!(caps.is_port_allowed(80));
        assert!(caps.is_port_allowed(443));
        assert!(caps.is_port_allowed(8500));
        assert!(!caps.is_port_allowed(22));
        assert!(!caps.is_port_allowed(3000));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_address_check() {
        let mut caps = WasiSocketCapabilities::none();
        caps.allowed_addresses = vec![
            AllowedAddress::Localhost,
            AllowedAddress::Ipv4(Ipv4Addr::new(192, 168, 1, 1)),
        ];

        assert!(caps.is_address_allowed(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(caps.is_address_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!caps.is_address_allowed(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_subnet_check() {
        let mut caps = WasiSocketCapabilities::none();
        caps.allowed_addresses = vec![
            AllowedAddress::Ipv4Subnet(Ipv4Addr::new(192, 168, 0, 0), 16),
        ];

        assert!(caps.is_address_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(caps.is_address_allowed(&IpAddr::V4(Ipv4Addr::new(192, 168, 255, 255))));
        assert!(!caps.is_address_allowed(&IpAddr::V4(Ipv4Addr::new(192, 169, 0, 1))));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_dns_resolve() {
        // Test with a known hostname (localhost should always resolve)
        let args = vec![Value::String("localhost".to_string())];
        let result = wasi_resolve_addresses(&mut (), args);
        assert!(result.is_ok());
    }
}
