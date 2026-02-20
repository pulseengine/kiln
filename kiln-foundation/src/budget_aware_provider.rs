//! CrateId Definitions
//!
//! This module provides the CrateId enum used throughout the WRT memory system.

use crate::{
    memory_coordinator::CrateIdentifier,
    traits::{
        Checksummable,
        FromBytes,
        ToBytes,
    },
    verification::Checksum,
};

/// Crate identifiers for budget tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CrateId {
    #[default]
    Foundation,
    Decoder,
    Runtime,
    Component,
    Host,
    Debug,
    Platform,
    Instructions,
    Format,
    Intercept,
    Sync,
    Math,
    Logging,
    Panic,
    TestRegistry,
    VerificationTool,
    Unknown,
    Wasi,
    WasiComponents,
}

impl CrateId {
    /// Create a CrateId from a thread ID for capability mapping
    pub fn from_thread_id(thread_id: u32) -> Self {
        // Map thread IDs to crate IDs - for now just use Runtime
        // In production, this would maintain a proper mapping
        CrateId::Runtime
    }
}

impl CrateIdentifier for CrateId {
    fn as_index(&self) -> usize {
        match self {
            CrateId::Foundation => 0,
            CrateId::Decoder => 1,
            CrateId::Runtime => 2,
            CrateId::Component => 3,
            CrateId::Host => 4,
            CrateId::Debug => 5,
            CrateId::Platform => 6,
            CrateId::Instructions => 7,
            CrateId::Format => 8,
            CrateId::Intercept => 9,
            CrateId::Sync => 10,
            CrateId::Math => 11,
            CrateId::Logging => 12,
            CrateId::Panic => 13,
            CrateId::TestRegistry => 14,
            CrateId::VerificationTool => 15,
            CrateId::Unknown => 16,
            CrateId::Wasi => 17,
            CrateId::WasiComponents => 18,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            CrateId::Foundation => "foundation",
            CrateId::Decoder => "decoder",
            CrateId::Runtime => "runtime",
            CrateId::Component => "component",
            CrateId::Host => "host",
            CrateId::Debug => "debug",
            CrateId::Platform => "platform",
            CrateId::Instructions => "instructions",
            CrateId::Format => "format",
            CrateId::Intercept => "intercept",
            CrateId::Sync => "sync",
            CrateId::Math => "math",
            CrateId::Logging => "logging",
            CrateId::Panic => "panic",
            CrateId::TestRegistry => "test_registry",
            CrateId::VerificationTool => "verification_tool",
            CrateId::Unknown => "unknown",
            CrateId::Wasi => "wasi",
            CrateId::WasiComponents => "wasi_components",
        }
    }

    fn count() -> usize {
        19
    }
}

impl Checksummable for CrateId {
    fn update_checksum(&self, checksum: &mut Checksum) {
        checksum.update(self.as_index() as u8);
    }
}

impl ToBytes for CrateId {
    fn serialized_size(&self) -> usize {
        1
    }

    fn to_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        &self,
        writer: &mut crate::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(self.as_index() as u8)
    }
}

impl FromBytes for CrateId {
    fn from_bytes_with_provider<'a, PStream: crate::MemoryProvider>(
        reader: &mut crate::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(CrateId::Foundation),
            1 => Ok(CrateId::Decoder),
            2 => Ok(CrateId::Runtime),
            3 => Ok(CrateId::Component),
            4 => Ok(CrateId::Host),
            5 => Ok(CrateId::Debug),
            6 => Ok(CrateId::Platform),
            7 => Ok(CrateId::Instructions),
            8 => Ok(CrateId::Format),
            9 => Ok(CrateId::Intercept),
            10 => Ok(CrateId::Sync),
            11 => Ok(CrateId::Math),
            12 => Ok(CrateId::Logging),
            13 => Ok(CrateId::Panic),
            14 => Ok(CrateId::TestRegistry),
            15 => Ok(CrateId::VerificationTool),
            16 => Ok(CrateId::Unknown),
            17 => Ok(CrateId::Wasi),
            18 => Ok(CrateId::WasiComponents),
            _ => Err(crate::Error::invalid_input("Invalid CrateId index")),
        }
    }
}
