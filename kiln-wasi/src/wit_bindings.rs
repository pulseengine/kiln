//! WIT interface bindings for WASI
//!
//! This module provides bindings generated from WASI WIT interface definitions.
//! In a real implementation, these would be auto-generated from WIT files.

use crate::prelude::*;

/// WASI filesystem types from WIT
pub mod filesystem_types {

    /// File descriptor type
    pub type Descriptor = u32;

    /// Directory entry type
    #[derive(Debug, Clone, PartialEq)]
    pub struct DirectoryEntry {
        /// Entry name
        pub name:  String,
        /// Entry type
        pub type_: DescriptorType,
    }

    /// Descriptor types
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DescriptorType {
        /// Regular file
        RegularFile,
        /// Directory
        Directory,
        /// Symbolic link
        SymbolicLink,
        /// Block device
        BlockDevice,
        /// Character device
        CharacterDevice,
        /// FIFO
        Fifo,
        /// Socket
        Socket,
        /// Unknown type
        Unknown,
    }

    /// File metadata
    #[derive(Debug, Clone, PartialEq)]
    pub struct DescriptorStat {
        /// File type
        pub type_: DescriptorType,
        /// File size
        pub size: u64,
        /// Access timestamp
        pub data_access_timestamp: Timestamp,
        /// Modification timestamp
        pub data_modification_timestamp: Timestamp,
        /// Status change timestamp
        pub status_change_timestamp: Timestamp,
    }

    /// Timestamp type
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Timestamp {
        /// Seconds since Unix epoch
        pub seconds:     u64,
        /// Nanoseconds
        pub nanoseconds: u32,
    }
}

/// WASI CLI types from WIT
pub mod cli_types {
    /// Exit code type
    pub type ExitCode = u32;
}

/// WASI clocks types from WIT
pub mod clocks_types {
    

    /// Instant in time
    pub type Instant = u64;

    /// Duration
    pub type Duration = u64;

    /// Wall clock datetime
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Datetime {
        /// Seconds since Unix epoch
        pub seconds:     u64,
        /// Nanoseconds
        pub nanoseconds: u32,
    }
}

/// WASI I/O types from WIT
pub mod io_types {
    

    /// Stream error
    #[derive(Debug, Clone, PartialEq)]
    pub enum StreamError {
        /// Last operation failed
        LastOperationFailed,
        /// Stream closed
        Closed,
    }

    /// Pollable handle
    pub type Pollable = u32;
}

/// WASI random types from WIT
pub mod random_types {
    // No special types for random interface
}

/// WASI sockets types from WIT (Preview3)
#[cfg(feature = "preview3-prep")]
pub mod sockets_types {
    use super::*;

    /// Network handle
    pub type Network = u32;

    /// IP address family
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IpAddressFamily {
        /// IPv4
        Ipv4,
        /// IPv6
        Ipv6,
    }

    /// IP socket address
    #[derive(Debug, Clone, PartialEq)]
    pub enum IpSocketAddress {
        /// IPv4 address
        Ipv4(Ipv4SocketAddress),
        /// IPv6 address
        Ipv6(Ipv6SocketAddress),
    }

    /// IPv4 socket address
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Ipv4SocketAddress {
        /// Port number
        pub port:    u16,
        /// IPv4 address bytes
        pub address: [u8; 4],
    }

    /// IPv6 socket address
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Ipv6SocketAddress {
        /// Port number
        pub port:      u16,
        /// Flow info
        pub flow_info: u32,
        /// IPv6 address bytes
        pub address:   [u8; 16],
        /// Scope ID
        pub scope_id:  u32,
    }

    /// TCP socket handle
    pub type TcpSocket = u32;

    /// UDP socket handle
    pub type UdpSocket = u32;
}

/// Convert WIT types to WRT component values
pub mod conversions {
    use super::{filesystem_types, Result, Error, ErrorCategory, codes};
    use crate::Value;

    /// Convert filesystem descriptor to Value
    #[must_use] 
    pub fn descriptor_to_value(desc: filesystem_types::Descriptor) -> Value {
        Value::U32(desc)
    }

    /// Convert Value to filesystem descriptor
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not a U32.
    pub fn value_to_descriptor(value: &Value) -> Result<filesystem_types::Descriptor> {
        match value {
            Value::U32(desc) => Ok(*desc),
            _ => Err(Error::runtime_execution_error(
                "Cannot convert value to size type",
            )),
        }
    }

    /// Convert timestamp to Value
    #[must_use] 
    pub fn timestamp_to_value(ts: filesystem_types::Timestamp) -> Value {
        Value::Record(vec![
            ("seconds".to_string(), Value::U64(ts.seconds)),
            ("nanoseconds".to_string(), Value::U32(ts.nanoseconds)),
        ])
    }

    /// Convert Value to timestamp
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not a properly structured record.
    pub fn value_to_timestamp(value: &Value) -> Result<filesystem_types::Timestamp> {
        match value {
            Value::Record(fields) => {
                let mut seconds = 0u64;
                let mut nanoseconds = 0u32;

                for (key, val) in fields {
                    match key.as_str() {
                        "seconds" => {
                            if let Value::U64(s) = val {
                                seconds = *s;
                            }
                        },
                        "nanoseconds" => {
                            if let Value::U32(ns) = val {
                                nanoseconds = *ns;
                            }
                        },
                        _ => {},
                    }
                }

                Ok(filesystem_types::Timestamp {
                    seconds,
                    nanoseconds,
                })
            },
            _ => Err(Error::runtime_execution_error(
                "Cannot convert value to size type",
            )),
        }
    }

    /// Convert descriptor type to Value
    #[must_use] 
    pub fn descriptor_type_to_value(dt: filesystem_types::DescriptorType) -> Value {
        Value::U8(match dt {
            filesystem_types::DescriptorType::RegularFile => 0,
            filesystem_types::DescriptorType::Directory => 1,
            filesystem_types::DescriptorType::SymbolicLink => 2,
            filesystem_types::DescriptorType::BlockDevice => 3,
            filesystem_types::DescriptorType::CharacterDevice => 4,
            filesystem_types::DescriptorType::Fifo => 5,
            filesystem_types::DescriptorType::Socket => 6,
            filesystem_types::DescriptorType::Unknown => 7,
        })
    }

    /// Convert Value to descriptor type
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not a valid descriptor type (0-7).
    pub fn value_to_descriptor_type(value: &Value) -> Result<filesystem_types::DescriptorType> {
        match value {
            Value::U8(0) => Ok(filesystem_types::DescriptorType::RegularFile),
            Value::U8(1) => Ok(filesystem_types::DescriptorType::Directory),
            Value::U8(2) => Ok(filesystem_types::DescriptorType::SymbolicLink),
            Value::U8(3) => Ok(filesystem_types::DescriptorType::BlockDevice),
            Value::U8(4) => Ok(filesystem_types::DescriptorType::CharacterDevice),
            Value::U8(5) => Ok(filesystem_types::DescriptorType::Fifo),
            Value::U8(6) => Ok(filesystem_types::DescriptorType::Socket),
            Value::U8(7) => Ok(filesystem_types::DescriptorType::Unknown),
            _ => Err(Error::new(
                ErrorCategory::Parse,
                codes::WASI_INVALID_FD,
                "Expected record type for timestamp",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    #[test]
    fn test_descriptor_conversions() -> Result<()> {
        let desc: filesystem_types::Descriptor = 42;
        let value = conversions::descriptor_to_value(desc);
        assert_eq!(value, Value::U32(42));

        let back = conversions::value_to_descriptor(&value)?;
        assert_eq!(back, desc);

        Ok(())
    }

    #[test]
    fn test_timestamp_conversions() -> Result<()> {
        let ts = filesystem_types::Timestamp {
            seconds:     1234567890,
            nanoseconds: 123456789,
        };

        let value = conversions::timestamp_to_value(ts);
        if let Value::Record(fields) = &value {
            assert_eq!(fields.len(), 2);
        }

        let back = conversions::value_to_timestamp(&value)?;
        assert_eq!(back.seconds, ts.seconds);
        assert_eq!(back.nanoseconds, ts.nanoseconds);

        Ok(())
    }

    #[test]
    fn test_descriptor_type_conversions() -> Result<()> {
        let dt = filesystem_types::DescriptorType::RegularFile;
        let value = conversions::descriptor_type_to_value(dt);
        assert_eq!(value, Value::U8(0));

        let back = conversions::value_to_descriptor_type(&value)?;
        assert_eq!(back, dt);

        // Test all types
        let types = [
            filesystem_types::DescriptorType::RegularFile,
            filesystem_types::DescriptorType::Directory,
            filesystem_types::DescriptorType::SymbolicLink,
            filesystem_types::DescriptorType::BlockDevice,
            filesystem_types::DescriptorType::CharacterDevice,
            filesystem_types::DescriptorType::Fifo,
            filesystem_types::DescriptorType::Socket,
            filesystem_types::DescriptorType::Unknown,
        ];

        for dt in &types {
            let value = conversions::descriptor_type_to_value(*dt);
            let back = conversions::value_to_descriptor_type(&value)?;
            assert_eq!(back, *dt);
        }

        Ok(())
    }
}
