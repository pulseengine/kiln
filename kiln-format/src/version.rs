//! Version information for wrt-format.
//!
//! This module provides utilities for handling versioning and feature detection
//! in WebAssembly Component Model binaries.

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(feature = "std")]
use std::collections::BTreeMap as HashMap;

/// Current state serialization format version
pub const STATE_VERSION: u32 = 1;

/// Magic bytes that identify WRT state sections
pub const STATE_MAGIC: &[u8; 4] = b"WRT\0";

/// Represents the version of a WebAssembly Component Model binary
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentModelVersion {
    /// Draft version (pre-standardization)
    Draft,
    /// Version 1.0
    V1_0,
    // Future versions can be added here
}

impl Default for ComponentModelVersion {
    fn default() -> Self {
        Self::V1_0
    }
}

/// Feature flags for different Component Model capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub enum ComponentModelFeature {
    /// Default placeholder (first variant serves as Default)
    #[default]
    CoreModule,
    /// Core instance support
    CoreInstance,
    /// Core type support
    CoreType,
    /// Component type support
    ComponentType,
    /// Instance support
    Instance,
    /// Alias support
    Alias,
    /// Canonical function conversion support
    Canon,
    /// Start function support
    Start,
    /// Import support
    Import,
    /// Export support
    Export,
    /// Value support (🪙 Experimental)
    Value,
    /// Resource types support (🪙 Experimental)
    ResourceTypes,
}

/// Status of feature support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeatureStatus {
    /// Feature is not available (default)
    #[default]
    Unavailable,
    /// Feature is available but experimental
    ExperimentalSupported,
    /// Feature is fully supported
    FullySupported,
}

/// Holds information about a Component Model binary version and supported
/// features
#[derive(Debug)]
pub struct VersionInfo {
    /// The detected version
    pub version:           ComponentModelVersion,
    /// Map of features to their support status
    #[cfg(feature = "std")]
    features:              HashMap<ComponentModelFeature, FeatureStatus>,
    #[cfg(not(feature = "std"))]
    features:              crate::HashMap<ComponentModelFeature, FeatureStatus>,
    /// Whether this binary uses any experimental features
    pub uses_experimental: bool,
}

impl Default for VersionInfo {
    fn default() -> Self {
        #[cfg(feature = "std")]
        let features = HashMap::new();

        #[cfg(not(any(feature = "std",)))]
        let features = crate::HashMap::new(wrt_foundation::NoStdProvider::default())
            .expect("Failed to create feature map");

        let mut info = Self {
            version: ComponentModelVersion::default(),
            features,
            uses_experimental: false,
        };

        // Initialize with default feature set for V1.0
        info.initialize_v1_0_features();

        info
    }
}

impl Clone for VersionInfo {
    fn clone(&self) -> Self {
        #[cfg(feature = "std")]
        let features = self.features.clone();

        #[cfg(not(any(feature = "std",)))]
        let features = {
            let new_features = crate::HashMap::new(wrt_foundation::NoStdProvider::default())
                .expect("Failed to create feature map");
            // For now, create a new empty map since BoundedMap doesn't have Clone
            new_features
        };

        Self {
            version: self.version,
            features,
            uses_experimental: self.uses_experimental,
        }
    }
}

impl VersionInfo {
    /// Create a new VersionInfo from the binary version field
    pub fn from_version_bytes(version_bytes: [u8; 4]) -> Self {
        let mut info = VersionInfo::default();

        // First two bytes are the version, next two are the layer
        let version = [version_bytes[0], version_bytes[1]];

        // Detect version
        match version {
            // Version 1.0
            [0x01, 0x00] => {
                info.version = ComponentModelVersion::V1_0;
                info.initialize_v1_0_features();
            },
            // Unknown/future version - default to V1.0 with minimal features
            _ => {
                info.version = ComponentModelVersion::Draft;
                info.initialize_minimal_features();
            },
        }

        info
    }

    /// Initialize features for version 1.0
    fn initialize_v1_0_features(&mut self) {
        // Standard features in V1.0
        let _ = self.features.insert(
            ComponentModelFeature::CoreModule,
            FeatureStatus::FullySupported,
        );
        let _ = self.features.insert(
            ComponentModelFeature::CoreInstance,
            FeatureStatus::FullySupported,
        );
        let _ = self.features.insert(
            ComponentModelFeature::CoreType,
            FeatureStatus::FullySupported,
        );
        let _ = self.features.insert(
            ComponentModelFeature::ComponentType,
            FeatureStatus::FullySupported,
        );
        let _ = self.features.insert(
            ComponentModelFeature::Instance,
            FeatureStatus::FullySupported,
        );
        let _ = self
            .features
            .insert(ComponentModelFeature::Alias, FeatureStatus::FullySupported);
        let _ = self
            .features
            .insert(ComponentModelFeature::Canon, FeatureStatus::FullySupported);
        let _ = self
            .features
            .insert(ComponentModelFeature::Start, FeatureStatus::FullySupported);
        let _ = self
            .features
            .insert(ComponentModelFeature::Import, FeatureStatus::FullySupported);
        let _ = self
            .features
            .insert(ComponentModelFeature::Export, FeatureStatus::FullySupported);

        // Experimental features
        #[cfg(feature = "component-model-values")]
        let _ = self.features.insert(
            ComponentModelFeature::Value,
            FeatureStatus::ExperimentalSupported,
        );
        #[cfg(not(feature = "component-model-values"))]
        let _ = self.features.insert(ComponentModelFeature::Value, FeatureStatus::Unavailable);

        #[cfg(feature = "component-model-resources")]
        let _ = self.features.insert(
            ComponentModelFeature::ResourceTypes,
            FeatureStatus::ExperimentalSupported,
        );
        #[cfg(not(feature = "component-model-resources"))]
        let _ = self.features.insert(
            ComponentModelFeature::ResourceTypes,
            FeatureStatus::Unavailable,
        );
    }

    /// Initialize minimal feature set (for unknown versions)
    fn initialize_minimal_features(&mut self) {
        // Only include core features
        let _ = self.features.insert(
            ComponentModelFeature::CoreModule,
            FeatureStatus::FullySupported,
        );
        let _ = self.features.insert(
            ComponentModelFeature::CoreInstance,
            FeatureStatus::FullySupported,
        );
        let _ = self.features.insert(
            ComponentModelFeature::CoreType,
            FeatureStatus::FullySupported,
        );

        // Other features are unavailable
        let _ = self.features.insert(
            ComponentModelFeature::ComponentType,
            FeatureStatus::Unavailable,
        );
        let _ = self
            .features
            .insert(ComponentModelFeature::Instance, FeatureStatus::Unavailable);
        let _ = self.features.insert(ComponentModelFeature::Alias, FeatureStatus::Unavailable);
        let _ = self.features.insert(ComponentModelFeature::Canon, FeatureStatus::Unavailable);
        let _ = self.features.insert(ComponentModelFeature::Start, FeatureStatus::Unavailable);
        let _ = self.features.insert(ComponentModelFeature::Import, FeatureStatus::Unavailable);
        let _ = self.features.insert(ComponentModelFeature::Export, FeatureStatus::Unavailable);
        let _ = self.features.insert(ComponentModelFeature::Value, FeatureStatus::Unavailable);
        let _ = self.features.insert(
            ComponentModelFeature::ResourceTypes,
            FeatureStatus::Unavailable,
        );
    }

    /// Check if a feature is available (either experimental or fully supported)
    pub fn is_feature_available(&self, feature: ComponentModelFeature) -> bool {
        #[cfg(feature = "std")]
        {
            match self.features.get(&feature) {
                Some(status) => *status != FeatureStatus::Unavailable,
                None => false,
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            match self.features.get(&feature) {
                Ok(Some(status)) => !matches!(status, FeatureStatus::Unavailable),
                Ok(None) => false,
                Err(_) => false,
            }
        }
    }

    /// Get the status of a feature
    pub fn get_feature_status(&self, feature: ComponentModelFeature) -> FeatureStatus {
        #[cfg(feature = "std")]
        {
            match self.features.get(&feature) {
                Some(status) => *status,
                None => FeatureStatus::Unavailable,
            }
        }
        #[cfg(not(any(feature = "std",)))]
        {
            match self.features.get(&feature) {
                Ok(Some(status)) => status.clone(),
                Ok(None) => FeatureStatus::Unavailable,
                Err(_) => FeatureStatus::Unavailable,
            }
        }
    }

    /// Determine if the binary uses any experimental features
    pub fn detect_experimental_features(&mut self, binary: &[u8]) -> bool {
        // Placeholder for future implementation
        // In a real implementation, this would scan the binary for sections
        // that correspond to experimental features

        let value_section_present = binary
            .windows(1)
            .any(|window| window[0] == crate::binary::COMPONENT_VALUE_SECTION_ID);
        if value_section_present
            && self.get_feature_status(ComponentModelFeature::Value)
                == FeatureStatus::ExperimentalSupported
        {
            self.uses_experimental = true;
        }

        // Add more checks for other experimental features as needed

        self.uses_experimental
    }
}

// Manual trait implementations for no_std compatibility with BoundedMap
use wrt_foundation::traits::{
    Checksummable,
    FromBytes,
    ToBytes,
};

impl Checksummable for ComponentModelFeature {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update(*self as u8);
    }
}

impl ToBytes for ComponentModelFeature {
    fn serialized_size(&self) -> usize {
        1 // One byte for the enum value
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

impl FromBytes for ComponentModelFeature {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(ComponentModelFeature::CoreModule),
            1 => Ok(ComponentModelFeature::CoreInstance),
            2 => Ok(ComponentModelFeature::CoreType),
            3 => Ok(ComponentModelFeature::ComponentType),
            4 => Ok(ComponentModelFeature::Instance),
            5 => Ok(ComponentModelFeature::Alias),
            6 => Ok(ComponentModelFeature::Canon),
            7 => Ok(ComponentModelFeature::Start),
            8 => Ok(ComponentModelFeature::Import),
            9 => Ok(ComponentModelFeature::Export),
            10 => Ok(ComponentModelFeature::Value),
            11 => Ok(ComponentModelFeature::ResourceTypes),
            _ => Err(wrt_error::Error::runtime_execution_error(
                "Invalid ComponentModelFeature value",
            )),
        }
    }
}

impl Checksummable for FeatureStatus {
    fn update_checksum(&self, checksum: &mut wrt_foundation::verification::Checksum) {
        checksum.update(*self as u8);
    }
}

impl ToBytes for FeatureStatus {
    fn serialized_size(&self) -> usize {
        1 // One byte for the enum value
    }

    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        &self,
        writer: &mut wrt_foundation::traits::WriteStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<()> {
        writer.write_u8(*self as u8)
    }
}

impl FromBytes for FeatureStatus {
    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
        reader: &mut wrt_foundation::traits::ReadStream<'a>,
        _provider: &PStream,
    ) -> wrt_error::Result<Self> {
        let byte = reader.read_u8()?;
        match byte {
            0 => Ok(FeatureStatus::Unavailable),
            1 => Ok(FeatureStatus::ExperimentalSupported),
            2 => Ok(FeatureStatus::FullySupported),
            _ => Err(wrt_error::Error::new(
                wrt_error::ErrorCategory::Parse,
                wrt_error::codes::INVALID_VALUE,
                "Invalid FeatureStatus value",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_detection() {
        // V1.0
        let v1_bytes = [0x01, 0x00, 0x01, 0x00];
        let v1_info = VersionInfo::from_version_bytes(v1_bytes);
        assert_eq!(v1_info.version, ComponentModelVersion::V1_0);

        // Unknown version
        let unknown_bytes = [0x02, 0x00, 0x01, 0x00];
        let unknown_info = VersionInfo::from_version_bytes(unknown_bytes);
        assert_eq!(unknown_info.version, ComponentModelVersion::Draft);
    }

    #[test]
    fn test_feature_availability() {
        let v1_bytes = [0x01, 0x00, 0x01, 0x00];
        let v1_info = VersionInfo::from_version_bytes(v1_bytes);

        // Core features should be available
        assert!(v1_info.is_feature_available(ComponentModelFeature::CoreModule));
        assert!(v1_info.is_feature_available(ComponentModelFeature::CoreInstance));

        // Experimental features depend on compile-time flags
        #[cfg(feature = "component-model-values")]
        assert!(v1_info.is_feature_available(ComponentModelFeature::Value));
        #[cfg(not(feature = "component-model-values"))]
        assert!(!v1_info.is_feature_available(ComponentModelFeature::Value));
    }
}
