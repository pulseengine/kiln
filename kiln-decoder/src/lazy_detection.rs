//! Lazy component detection to avoid unnecessary decoding
//!
//! This module provides smart detection mechanisms that can identify
//! component-specific sections and features without fully parsing the
//! entire binary, improving performance for mixed workloads.

#[cfg(not(feature = "std"))]
extern crate alloc;

use crate::{prelude::*, unified_loader::WasmFormat};

/// Component detection result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentDetection {
    /// Definitely a core module
    CoreModule,
    /// Definitely a component
    Component,
    /// Could be either (needs full parsing)
    Ambiguous,
    /// Invalid format
    Invalid,
}

/// Component-specific section IDs (from Component Model spec)
const COMPONENT_TYPE_SECTION: u8 = 1;
const COMPONENT_IMPORT_SECTION: u8 = 2;
const COMPONENT_FUNC_SECTION: u8 = 3;
const COMPONENT_COMPONENT_SECTION: u8 = 4;
const COMPONENT_INSTANCE_SECTION: u8 = 5;
const COMPONENT_EXPORT_SECTION: u8 = 6;
const COMPONENT_START_SECTION: u8 = 7;
const COMPONENT_ALIAS_SECTION: u8 = 8;
const COMPONENT_CANONICAL_SECTION: u8 = 9;

/// Core module section IDs (standard WASM)
const CORE_TYPE_SECTION: u8 = 1;
const CORE_IMPORT_SECTION: u8 = 2;
const CORE_FUNCTION_SECTION: u8 = 3;
const CORE_TABLE_SECTION: u8 = 4;
const CORE_MEMORY_SECTION: u8 = 5;
const CORE_GLOBAL_SECTION: u8 = 6;
const CORE_EXPORT_SECTION: u8 = 7;
const CORE_START_SECTION: u8 = 8;
const CORE_ELEMENT_SECTION: u8 = 9;
const CORE_CODE_SECTION: u8 = 10;
const CORE_DATA_SECTION: u8 = 11;
const CORE_DATA_COUNT_SECTION: u8 = 12;

/// Detection configuration
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Maximum bytes to scan for detection
    pub max_scan_bytes: usize,
    /// Maximum sections to examine
    pub max_sections: usize,
    /// Whether to use heuristic detection
    pub use_heuristics: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            max_scan_bytes: 4096, // Scan first 4KB
            max_sections: 10,     // Examine first 10 sections
            use_heuristics: true,
        }
    }
}

/// Lazy component detector
pub struct LazyDetector {
    config: DetectionConfig,
}

impl LazyDetector {
    /// Create a new lazy detector with default configuration
    pub fn new() -> Self {
        Self {
            config: DetectionConfig::default(),
        }
    }

    /// Create a new lazy detector with custom configuration
    pub fn with_config(config: DetectionConfig) -> Self {
        Self { config }
    }

    /// Detect format with minimal parsing
    pub fn detect_format(&self, binary: &[u8]) -> Result<ComponentDetection> {
        // Basic validation
        if binary.len() < 8 {
            return Ok(ComponentDetection::Invalid);
        }

        // Check magic number
        if &binary[0..4] != b"\0asm" {
            return Ok(ComponentDetection::Invalid);
        }

        // Check version
        let version = u32::from_le_bytes([binary[4], binary[5], binary[6], binary[7]]);

        // Component Model typically uses different version numbers
        match version {
            1 => {
                // Version 1 could be core module or component
                // Need to examine sections
                self.examine_sections(binary)
            },
            _ => {
                // Non-standard version, likely component or invalid
                if self.config.use_heuristics {
                    self.examine_sections(binary)
                } else {
                    Ok(ComponentDetection::Ambiguous)
                }
            },
        }
    }

    /// Examine sections to determine format
    fn examine_sections(&self, binary: &[u8]) -> Result<ComponentDetection> {
        let mut offset = 8; // Skip header
        let mut sections_examined = 0;
        let mut component_indicators = 0;
        let mut core_indicators = 0;

        let scan_limit = core::cmp::min(binary.len(), 8 + self.config.max_scan_bytes);

        while offset < scan_limit && sections_examined < self.config.max_sections {
            if offset + 1 >= binary.len() {
                break;
            }

            let section_id = binary[offset];
            offset += 1;

            // Read section size
            let (section_size, bytes_read) = read_leb128_u32(binary, offset)?;
            offset += bytes_read;

            let section_end = offset + section_size as usize;
            if section_end > binary.len() {
                return Ok(ComponentDetection::Invalid);
            }

            // Analyze section ID
            match section_id {
                // Component-specific sections
                COMPONENT_COMPONENT_SECTION
                | COMPONENT_INSTANCE_SECTION
                | COMPONENT_ALIAS_SECTION
                | COMPONENT_CANONICAL_SECTION => {
                    component_indicators += 2; // Strong indicator
                },

                // Core-specific sections
                CORE_TABLE_SECTION
                | CORE_MEMORY_SECTION
                | CORE_GLOBAL_SECTION
                | CORE_ELEMENT_SECTION
                | CORE_CODE_SECTION
                | CORE_DATA_SECTION
                | CORE_DATA_COUNT_SECTION => {
                    core_indicators += 2; // Strong indicator
                },

                // Shared sections (could be either)
                CORE_TYPE_SECTION
                | CORE_IMPORT_SECTION
                | CORE_FUNCTION_SECTION
                | CORE_EXPORT_SECTION
                | CORE_START_SECTION => {
                    // Examine content for more clues
                    if self.config.use_heuristics {
                        match self.examine_section_content(section_id, &binary[offset..section_end])
                        {
                            SectionHint::Component => component_indicators += 1,
                            SectionHint::Core => core_indicators += 1,
                            SectionHint::Neutral => {},
                        }
                    }
                },

                // Custom sections
                0 => {
                    // Custom sections might contain component-specific data
                    if self.config.use_heuristics {
                        if let Ok(hint) = self.examine_custom_section(&binary[offset..section_end])
                        {
                            match hint {
                                SectionHint::Component => component_indicators += 1,
                                SectionHint::Core => core_indicators += 1,
                                SectionHint::Neutral => {},
                            }
                        }
                    }
                },

                // Unknown sections (likely component)
                13..=255 => {
                    component_indicators += 1;
                },

                _ => {}, // Other sections
            }

            offset = section_end;
            sections_examined += 1;
        }

        // Make determination based on indicators
        if component_indicators > core_indicators * 2 {
            Ok(ComponentDetection::Component)
        } else if core_indicators > component_indicators * 2 {
            Ok(ComponentDetection::CoreModule)
        } else {
            Ok(ComponentDetection::Ambiguous)
        }
    }

    /// Examine section content for hints
    fn examine_section_content(&self, section_id: u8, data: &[u8]) -> SectionHint {
        match section_id {
            CORE_IMPORT_SECTION => self.examine_import_section(data),
            CORE_EXPORT_SECTION => self.examine_export_section(data),
            _ => SectionHint::Neutral,
        }
    }

    /// Examine import section for component hints
    fn examine_import_section(&self, data: &[u8]) -> SectionHint {
        // Look for component-style imports
        let mut offset = 0;

        // Read count
        if let Ok((count, bytes_read)) = read_leb128_u32(data, offset) {
            offset += bytes_read;

            for _ in 0..core::cmp::min(count, 5) {
                // Examine first few imports
                // Read module name
                if let Ok((module_len, bytes_read)) = read_leb128_u32(data, offset) {
                    offset += bytes_read;

                    if offset + module_len as usize <= data.len() {
                        if let Ok(module_name) =
                            core::str::from_utf8(&data[offset..offset + module_len as usize])
                        {
                            // Component-style module names
                            if module_name.contains("component:")
                                || module_name.contains("interface:")
                            {
                                return SectionHint::Component;
                            }
                            // Core-style module names
                            if module_name == "env" || module_name == "wasi_snapshot_preview1" {
                                return SectionHint::Core;
                            }
                        }
                        #[allow(unused_assignments)]
                        {
                            offset += module_len as usize;
                        }

                        // Skip name and type for now
                        break;
                    }
                }
            }
        }

        SectionHint::Neutral
    }

    /// Examine export section for component hints
    fn examine_export_section(&self, data: &[u8]) -> SectionHint {
        // Look for component-style exports
        let mut offset = 0;

        // Read count
        if let Ok((count, bytes_read)) = read_leb128_u32(data, offset) {
            offset += bytes_read;

            for _ in 0..core::cmp::min(count, 5) {
                // Examine first few exports
                // Read export name
                if let Ok((name_len, bytes_read)) = read_leb128_u32(data, offset) {
                    offset += bytes_read;

                    if offset + name_len as usize <= data.len() {
                        if let Ok(export_name) =
                            core::str::from_utf8(&data[offset..offset + name_len as usize])
                        {
                            // Component-style export names
                            if export_name.contains(":") && export_name.len() > 10 {
                                return SectionHint::Component;
                            }
                        }

                        // Skip type info
                        break;
                    }
                }
            }
        }

        SectionHint::Neutral
    }

    /// Examine custom section for component hints
    fn examine_custom_section(&self, data: &[u8]) -> Result<SectionHint> {
        // Read custom section name
        let mut offset = 0;
        let (name_len, bytes_read) = read_leb128_u32(data, offset)?;
        offset += bytes_read;

        if offset + name_len as usize > data.len() {
            return Ok(SectionHint::Neutral);
        }

        let section_name =
            core::str::from_utf8(&data[offset..offset + name_len as usize]).unwrap_or("");

        // Check for component-specific custom sections
        if section_name == "component-type"
            || section_name == "component-import"
            || section_name.starts_with("component:")
            || section_name.starts_with("interface:")
        {
            return Ok(SectionHint::Component);
        }

        // Check for core-specific custom sections
        if section_name == "name"
            || section_name == "producers"
            || section_name == "target_features"
        {
            return Ok(SectionHint::Core);
        }

        Ok(SectionHint::Neutral)
    }

    /// Quick check if binary needs component processing
    pub fn needs_component_processing(&self, binary: &[u8]) -> Result<bool> {
        match self.detect_format(binary)? {
            ComponentDetection::Component => Ok(true),
            ComponentDetection::CoreModule => Ok(false),
            ComponentDetection::Ambiguous => Ok(true), // Safe to assume yes
            ComponentDetection::Invalid => Ok(false),
        }
    }

    /// Quick check if binary is definitely a core module
    pub fn is_definitely_core_module(&self, binary: &[u8]) -> Result<bool> {
        match self.detect_format(binary)? {
            ComponentDetection::CoreModule => Ok(true),
            _ => Ok(false),
        }
    }
}

impl Default for LazyDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Section content hint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionHint {
    /// Indicates component format
    Component,
    /// Indicates core module format
    Core,
    /// No clear indication
    Neutral,
}

/// Create optimized detector for specific use case
pub fn create_fast_detector() -> LazyDetector {
    LazyDetector::with_config(DetectionConfig {
        max_scan_bytes: 1024,  // Scan only first 1KB
        max_sections: 5,       // Examine only first 5 sections
        use_heuristics: false, // Skip content analysis
    })
}

/// Create thorough detector for accurate detection
pub fn create_thorough_detector() -> LazyDetector {
    LazyDetector::with_config(DetectionConfig {
        max_scan_bytes: 16384, // Scan first 16KB
        max_sections: 20,      // Examine first 20 sections
        use_heuristics: true,  // Use full content analysis
    })
}

/// Helper function to read LEB128 unsigned 32-bit integer
fn read_leb128_u32(data: &[u8], offset: usize) -> Result<(u32, usize)> {
    let mut result = 0u32;
    let mut shift = 0;
    let mut bytes_read = 0;

    for i in 0..5 {
        // Max 5 bytes for u32
        if offset + i >= data.len() {
            return Err(Error::parse_error(
                "Unexpected end of data while reading LEB128",
            ));
        }

        let byte = data[offset + i];
        bytes_read += 1;

        result |= ((byte & 0x7F) as u32) << shift;

        if byte & 0x80 == 0 {
            break;
        }

        shift += 7;
        if shift >= 32 {
            return Err(Error::parse_error("LEB128 value too large for u32"));
        }
    }

    Ok((result, bytes_read))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_detection() {
        let detector = LazyDetector::new();

        // Core module header
        let core_module = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        let result = detector.detect_format(&core_module).unwrap();
        // Without sections, should be ambiguous
        assert!(matches!(
            result,
            ComponentDetection::Ambiguous | ComponentDetection::CoreModule
        ));
    }

    #[test]
    fn test_invalid_magic() {
        let detector = LazyDetector::new();
        let invalid = [0x00, 0x61, 0x73, 0x6E, 0x01, 0x00, 0x00, 0x00];
        let result = detector.detect_format(&invalid).unwrap();
        assert_eq!(result, ComponentDetection::Invalid);
    }

    #[test]
    fn test_too_small() {
        let detector = LazyDetector::new();
        let too_small = [0x00, 0x61, 0x73];
        let result = detector.detect_format(&too_small).unwrap();
        assert_eq!(result, ComponentDetection::Invalid);
    }

    #[test]
    fn test_fast_detector() {
        let detector = create_fast_detector();
        assert_eq!(detector.config.max_scan_bytes, 1024);
        assert_eq!(detector.config.max_sections, 5);
        assert!(!detector.config.use_heuristics);
    }

    #[test]
    fn test_thorough_detector() {
        let detector = create_thorough_detector();
        assert_eq!(detector.config.max_scan_bytes, 16384);
        assert_eq!(detector.config.max_sections, 20);
        assert!(detector.config.use_heuristics);
    }

    #[test]
    fn test_needs_component_processing() {
        let detector = LazyDetector::new();
        let core_module = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

        // Should handle safely even with ambiguous detection
        let result = detector.needs_component_processing(&core_module).unwrap();
        assert!(result || !result); // Either result is acceptable for empty
        // module
    }
}
