// Enhanced Bounded WIT Parser with configurable limits
// This is the enhanced implementation for the component module

use wrt_error::{Error, Result};
use wrt_foundation::{MemoryProvider, NoStdProvider};
extern crate alloc;

/// Simple bounded string for no_std environments
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleBoundedString {
    data: [u8; 64], // 64 bytes should be enough for WIT identifiers
    len: usize,
}

impl SimpleBoundedString {
    pub fn new() -> Self {
        Self {
            data: [0; 64],
            len: 0,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        if s.len() > 64 {
            return None;
        }

        let mut result = Self::new();
        let bytes = s.as_bytes();
        result.data[..bytes.len()].copy_from_slice(bytes);
        result.len = bytes.len();
        Some(result)
    }

    pub fn as_str(&self) -> core::result::Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.data[..self.len])
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Bounded WIT name for no_std environments
pub type BoundedWitName = SimpleBoundedString;

/// Simple bounded WIT world definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitWorld {
    /// World name
    pub name: BoundedWitName,
    /// Simple import/export counters for basic functionality
    pub import_count: u32,
    pub export_count: u32,
}

/// Simple bounded WIT interface definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitInterface {
    /// Interface name
    pub name: BoundedWitName,
    /// Simple function counter for basic functionality
    pub function_count: u32,
}

/// Simple bounded WIT function definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitFunction {
    /// Function name
    pub name: BoundedWitName,
    /// Parameter count (simplified)
    pub param_count: u32,
    /// Result count (simplified)
    pub result_count: u32,
}

/// Simple bounded WIT type definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundedWitType {
    /// Primitive types
    Bool,
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    F32,
    F64,
    Char,
    String,

    /// Named type reference
    Named {
        name: BoundedWitName,
    },

    /// Unknown/unsupported type
    Unknown,
}

/// Simple bounded import definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitImport {
    /// Import name
    pub name: BoundedWitName,
    /// Import is a function (simplified)
    pub is_function: bool,
}

/// Simple bounded export definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitExport {
    /// Export name
    pub name: BoundedWitName,
    /// Export is a function (simplified)
    pub is_function: bool,
}

/// WIT parsing limits for platform-aware configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitParsingLimits {
    pub max_input_buffer: usize,
    pub max_worlds: usize,
    pub max_interfaces: usize,
    pub max_functions_per_interface: usize,
    pub max_identifier_length: usize,
    pub max_imports_per_world: usize,
    pub max_exports_per_world: usize,
}

impl Default for WitParsingLimits {
    fn default() -> Self {
        Self {
            max_input_buffer: 8192, // 8KB
            max_worlds: 4,
            max_interfaces: 8,
            max_functions_per_interface: 16,
            max_identifier_length: 64,
            max_imports_per_world: 32,
            max_exports_per_world: 32,
        }
    }
}

impl WitParsingLimits {
    /// Create limits for embedded platforms
    pub fn embedded() -> Self {
        Self {
            max_input_buffer: 2048, // 2KB
            max_worlds: 2,
            max_interfaces: 4,
            max_functions_per_interface: 8,
            max_identifier_length: 32,
            max_imports_per_world: 8,
            max_exports_per_world: 8,
        }
    }

    /// Create limits for QNX platforms
    pub fn qnx() -> Self {
        Self {
            max_input_buffer: 16384, // 16KB
            max_worlds: 8,
            max_interfaces: 16,
            max_functions_per_interface: 32,
            max_identifier_length: 64,
            max_imports_per_world: 64,
            max_exports_per_world: 64,
        }
    }

    /// Create limits for Linux platforms
    pub fn linux() -> Self {
        Self {
            max_input_buffer: 32768, // 32KB
            max_worlds: 16,
            max_interfaces: 32,
            max_functions_per_interface: 64,
            max_identifier_length: 128,
            max_imports_per_world: 128,
            max_exports_per_world: 128,
        }
    }

    /// Validate limits are reasonable
    pub fn validate(&self) -> Result<()> {
        if self.max_input_buffer == 0 {
            return Err(Error::invalid_input("max_input_buffer cannot be zero"));
        }
        if self.max_worlds == 0 {
            return Err(Error::invalid_input("max_worlds cannot be zero"));
        }
        if self.max_interfaces == 0 {
            return Err(Error::invalid_input("max_interfaces cannot be zero"));
        }
        if self.max_identifier_length < 8 {
            return Err(Error::invalid_input(
                "max_identifier_length must be at least 8",
            ));
        }
        Ok(())
    }
}

/// WIT parse result with metadata
#[derive(Debug, Clone)]
pub struct WitParseResult {
    pub worlds: alloc::vec::Vec<BoundedWitWorld>,
    pub interfaces: alloc::vec::Vec<BoundedWitInterface>,
    pub metadata: WitParseMetadata,
}

#[derive(Debug, Clone)]
pub struct WitParseMetadata {
    pub input_size: usize,
    pub parse_time_us: u64, // Stub timestamp
    pub memory_used: usize,
    pub warnings: alloc::vec::Vec<WitParseWarning>,
}

#[derive(Debug, Clone)]
pub struct WitParseWarning {
    pub message: alloc::string::String,
    pub position: usize,
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningSeverity {
    Info,
    Warning,
    Error,
}

/// Enhanced bounded WIT parser with configurable limits
pub struct BoundedWitParser {
    limits: WitParsingLimits,
    input_buffer: alloc::vec::Vec<u8>, // Dynamic size based on limits
    input_len: usize,
    worlds: alloc::vec::Vec<Option<BoundedWitWorld>>,
    interfaces: alloc::vec::Vec<Option<BoundedWitInterface>>,
    world_count: usize,
    interface_count: usize,
    warnings: alloc::vec::Vec<WitParseWarning>,
    memory_usage: usize,
}

impl BoundedWitParser {
    /// Create a new bounded WIT parser with specified limits
    pub fn new(limits: WitParsingLimits) -> Result<Self> {
        limits.validate()?;

        let input_buffer = alloc::vec![0; limits.max_input_buffer];

        let mut worlds = alloc::vec::Vec::new();
        worlds.resize(limits.max_worlds, None);

        let mut interfaces = alloc::vec::Vec::new();
        interfaces.resize(limits.max_interfaces, None);

        let memory_usage = input_buffer.capacity()
            + worlds.capacity() * core::mem::size_of::<Option<BoundedWitWorld>>()
            + interfaces.capacity() * core::mem::size_of::<Option<BoundedWitInterface>>();

        Ok(Self {
            limits,
            input_buffer,
            input_len: 0,
            worlds,
            interfaces,
            world_count: 0,
            interface_count: 0,
            warnings: alloc::vec::Vec::new(),
            memory_usage,
        })
    }

    /// Create parser with default limits
    pub fn with_default_limits() -> Result<Self> {
        Self::new(WitParsingLimits::default())
    }

    /// Create parser for embedded platforms
    pub fn for_embedded() -> Result<Self> {
        Self::new(WitParsingLimits::embedded())
    }

    /// Create parser for QNX platforms
    pub fn for_qnx() -> Result<Self> {
        Self::new(WitParsingLimits::qnx())
    }

    /// Create parser for Linux platforms
    pub fn for_linux() -> Result<Self> {
        Self::new(WitParsingLimits::linux())
    }

    /// Get the current parsing limits
    pub fn limits(&self) -> &WitParsingLimits {
        &self.limits
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.memory_usage
    }

    /// Get parsing warnings
    pub fn warnings(&self) -> &[WitParseWarning] {
        &self.warnings
    }

    /// Parse WIT source with bounds checking
    pub fn parse_wit(&mut self, wit_source: &[u8]) -> Result<WitParseResult> {
        let start_time = self.get_timestamp(); // Stub implementation

        // Check input size limit
        if wit_source.len() > self.limits.max_input_buffer {
            return Err(Error::WIT_INPUT_TOO_LARGE);
        }

        // Clear previous state
        self.reset_state();

        // Copy input to buffer
        let copy_len = core::cmp::min(wit_source.len(), self.input_buffer.len());
        self.input_buffer[..copy_len].copy_from_slice(&wit_source[..copy_len]);
        self.input_len = copy_len;

        // Perform bounded parsing
        self.bounded_parse()?;

        let end_time = self.get_timestamp();

        // Collect results
        let mut result_worlds = alloc::vec::Vec::new();
        let mut result_interfaces = alloc::vec::Vec::new();

        for world_opt in &self.worlds {
            if let Some(world) = world_opt {
                result_worlds.push(world.clone());
            }
        }

        for interface_opt in &self.interfaces {
            if let Some(interface) = interface_opt {
                result_interfaces.push(interface.clone());
            }
        }

        let metadata = WitParseMetadata {
            input_size: wit_source.len(),
            parse_time_us: end_time.saturating_sub(start_time),
            memory_used: self.memory_usage,
            warnings: self.warnings.clone(),
        };

        Ok(WitParseResult {
            worlds: result_worlds,
            interfaces: result_interfaces,
            metadata,
        })
    }

    /// Reset parser state
    fn reset_state(&mut self) {
        self.input_len = 0;
        self.world_count = 0;
        self.interface_count = 0;
        self.warnings.clear();

        for world in &mut self.worlds {
            *world = None;
        }

        for interface in &mut self.interfaces {
            *interface = None;
        }
    }

    /// Bounded parsing implementation with comprehensive validation
    fn bounded_parse(&mut self) -> Result<()> {
        let mut position = 0;
        let mut brace_depth = 0;
        let mut in_comment = false;

        while position < self.input_len {
            let byte = self.input_buffer[position];

            // Handle comments
            if !in_comment
                && byte == b'/'
                && position + 1 < self.input_len
                && self.input_buffer[position + 1] == b'/'
            {
                in_comment = true;
                position += 2;
                continue;
            }

            if in_comment && byte == b'\n' {
                in_comment = false;
                position += 1;
                continue;
            }

            if in_comment {
                position += 1;
                continue;
            }

            // Track brace depth for structure validation
            match byte {
                b'{' => brace_depth += 1,
                b'}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    } else {
                        self.add_warning(WitParseWarning {
                            message: "Unmatched closing brace".into(),
                            position,
                            severity: WarningSeverity::Warning,
                        });
                    }
                },
                _ => {},
            }

            // Skip whitespace
            if byte.is_ascii_whitespace() {
                position += 1;
                continue;
            }

            // Try to read a keyword
            if let Some((keyword, new_pos)) = self.read_keyword(position) {
                match keyword.as_str() {
                    Ok("world") => {
                        if let Some((name, final_pos)) = self.read_identifier(new_pos) {
                            if let Err(e) = self.add_world(name) {
                                self.add_warning(WitParseWarning {
                                    message: alloc::format!("Failed to add world: {e}"),
                                    position,
                                    severity: WarningSeverity::Error,
                                });
                            }
                            position = self.skip_to_brace_end(final_pos);
                        } else {
                            self.add_warning(WitParseWarning {
                                message: "Expected world name after 'world' keyword".into(),
                                position: new_pos,
                                severity: WarningSeverity::Error,
                            });
                            position = new_pos;
                        }
                    },
                    Ok("interface") => {
                        if let Some((name, final_pos)) = self.read_identifier(new_pos) {
                            if let Err(e) = self.add_interface(name) {
                                self.add_warning(WitParseWarning {
                                    message: alloc::format!("Failed to add interface: {e}"),
                                    position,
                                    severity: WarningSeverity::Error,
                                });
                            }
                            position = self.skip_to_brace_end(final_pos);
                        } else {
                            self.add_warning(WitParseWarning {
                                message: "Expected interface name after 'interface' keyword".into(),
                                position: new_pos,
                                severity: WarningSeverity::Error,
                            });
                            position = new_pos;
                        }
                    },
                    _ => {
                        position = new_pos;
                    },
                }
            } else {
                position += 1;
            }
        }

        // Validate structure
        if brace_depth != 0 {
            self.add_warning(WitParseWarning {
                message: alloc::format!("Mismatched braces: {brace_depth} unclosed"),
                position: self.input_len,
                severity: WarningSeverity::Error,
            });
        }

        Ok(())
    }

    /// Read a keyword from the current position
    fn read_keyword(&self, mut position: usize) -> Option<(SimpleBoundedString, usize)> {
        // Skip whitespace
        while position < self.input_len && self.input_buffer[position].is_ascii_whitespace() {
            position += 1;
        }

        let start = position;

        // Read alphabetic characters
        while position < self.input_len && self.input_buffer[position].is_ascii_alphabetic() {
            position += 1;
        }

        if position > start {
            let keyword_bytes = &self.input_buffer[start..position];
            if let Ok(keyword_str) = core::str::from_utf8(keyword_bytes) {
                if let Some(bounded_string) = SimpleBoundedString::from_str(keyword_str) {
                    return Some((bounded_string, position));
                }
            }
        }

        None
    }

    /// Read an identifier from the current position
    fn read_identifier(&self, mut position: usize) -> Option<(SimpleBoundedString, usize)> {
        // Skip whitespace
        while position < self.input_len && self.input_buffer[position].is_ascii_whitespace() {
            position += 1;
        }

        let start = position;

        // Read alphanumeric, hyphens, and underscores
        while position < self.input_len {
            let byte = self.input_buffer[position];
            if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
                position += 1;
            } else {
                break;
            }
        }

        if position > start {
            let id_bytes = &self.input_buffer[start..position];

            // Check identifier length limit
            if id_bytes.len() > self.limits.max_identifier_length {
                return None;
            }

            if let Ok(id_str) = core::str::from_utf8(id_bytes) {
                if let Some(bounded_string) = SimpleBoundedString::from_str(id_str) {
                    return Some((bounded_string, position));
                }
            }
        }

        None
    }

    /// Skip to the end of a brace block
    fn skip_to_brace_end(&self, mut position: usize) -> usize {
        let mut brace_count = 0;
        let mut found_opening = false;

        while position < self.input_len {
            match self.input_buffer[position] {
                b'{' => {
                    brace_count += 1;
                    found_opening = true;
                },
                b'}' => {
                    if brace_count > 0 {
                        brace_count -= 1;
                        if brace_count == 0 && found_opening {
                            return position + 1; // Return position after
                            // closing brace
                        }
                    }
                },
                _ => {},
            }
            position += 1;
        }

        position
    }

    /// Add a world with bounds checking
    fn add_world(&mut self, name: SimpleBoundedString) -> Result<()> {
        if self.world_count >= self.limits.max_worlds {
            return Err(Error::WIT_WORLD_LIMIT_EXCEEDED);
        }

        let world = BoundedWitWorld {
            name,
            import_count: 0,
            export_count: 0,
        };

        self.worlds[self.world_count] = Some(world);
        self.world_count += 1;

        Ok(())
    }

    /// Add an interface with bounds checking
    fn add_interface(&mut self, name: SimpleBoundedString) -> Result<()> {
        if self.interface_count >= self.limits.max_interfaces {
            return Err(Error::WIT_INTERFACE_LIMIT_EXCEEDED);
        }

        let interface = BoundedWitInterface {
            name,
            function_count: 0,
        };

        self.interfaces[self.interface_count] = Some(interface);
        self.interface_count += 1;

        Ok(())
    }

    /// Add a warning to the warnings list
    fn add_warning(&mut self, warning: WitParseWarning) {
        if self.warnings.len() < 100 {
            // Limit warnings to prevent memory bloat
            self.warnings.push(warning);
        }
    }

    /// Get timestamp (stub implementation)
    fn get_timestamp(&self) -> u64 {
        // In a real implementation, this would use platform-specific timing
        0
    }

    /// Get parsed worlds
    pub fn worlds(&self) -> impl Iterator<Item = &BoundedWitWorld> {
        self.worlds.iter().filter_map(|w| w.as_ref())
    }

    /// Get parsed interfaces
    pub fn interfaces(&self) -> impl Iterator<Item = &BoundedWitInterface> {
        self.interfaces.iter().filter_map(|i| i.as_ref())
    }

    /// Get world count
    pub fn world_count(&self) -> usize {
        self.world_count
    }

    /// Get interface count
    pub fn interface_count(&self) -> usize {
        self.interface_count
    }

    /// Validate parsing result
    pub fn validate_result(&self) -> Result<()> {
        if self.world_count == 0 && self.interface_count == 0 {
            return Err(Error::NO_WIT_DEFINITIONS_FOUND);
        }

        // Check for critical errors in warnings
        for warning in &self.warnings {
            if warning.severity == WarningSeverity::Error {
                return Err(Error::wit_parse_error("WIT parse error"));
            }
        }

        Ok(())
    }
}

/// Convenience function to parse WIT with platform-specific limits
pub fn parse_wit_with_limits(
    wit_source: &[u8],
    limits: WitParsingLimits,
) -> Result<WitParseResult> {
    let mut parser = BoundedWitParser::new(limits)?;
    parser.parse_wit(wit_source)
}

/// Convenience function to parse WIT for embedded platforms
pub fn parse_wit_embedded(wit_source: &[u8]) -> Result<WitParseResult> {
    parse_wit_with_limits(wit_source, WitParsingLimits::embedded())
}

/// Convenience function to parse WIT for QNX platforms
pub fn parse_wit_qnx(wit_source: &[u8]) -> Result<WitParseResult> {
    parse_wit_with_limits(wit_source, WitParsingLimits::qnx())
}

/// Convenience function to parse WIT for Linux platforms
pub fn parse_wit_linux(wit_source: &[u8]) -> Result<WitParseResult> {
    parse_wit_with_limits(wit_source, WitParsingLimits::linux())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_wit_parser_creation() {
        let limits = WitParsingLimits::default();
        let parser = BoundedWitParser::new(limits);
        assert!(parser.is_ok());

        let parser = parser.unwrap();
        assert_eq!(parser.world_count(), 0);
        assert_eq!(parser.interface_count(), 0);
    }

    #[test]
    fn test_platform_specific_limits() {
        let embedded_limits = WitParsingLimits::embedded();
        assert!(embedded_limits.max_input_buffer < WitParsingLimits::default().max_input_buffer);

        let linux_limits = WitParsingLimits::linux();
        assert!(linux_limits.max_input_buffer > WitParsingLimits::default().max_input_buffer);
    }

    #[test]
    fn test_wit_parsing_with_limits() {
        let wit_source = b"world test-world { }";
        let result = parse_wit_embedded(wit_source);

        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert_eq!(parse_result.worlds.len(), 1);
        assert_eq!(parse_result.worlds[0].name.as_str().unwrap(), "test-world");
    }

    #[test]
    fn test_input_size_limit() {
        let limits = WitParsingLimits {
            max_input_buffer: 10,
            ..WitParsingLimits::default()
        };

        let mut parser = BoundedWitParser::new(limits).unwrap();
        let large_input = b"world very-long-world-name-that-exceeds-limit { }";

        let result = parser.parse_wit(large_input);
        assert!(result.is_err());
    }

    #[test]
    fn test_identifier_length_limit() {
        let limits = WitParsingLimits {
            max_identifier_length: 5,
            ..WitParsingLimits::default()
        };

        let mut parser = BoundedWitParser::new(limits).unwrap();
        let wit_source = b"world verylongname { }";

        let result = parser.parse_wit(wit_source);
        // Should parse but with warnings
        assert!(result.is_ok());

        let parse_result = result.unwrap();
        // The long identifier should be rejected
        assert_eq!(parse_result.worlds.len(), 0);
    }

    #[test]
    fn test_world_limit() {
        let limits = WitParsingLimits {
            max_worlds: 1,
            ..WitParsingLimits::default()
        };

        let mut parser = BoundedWitParser::new(limits).unwrap();
        let wit_source = b"world world1 { } world world2 { }";

        let result = parser.parse_wit(wit_source);
        assert!(result.is_ok());

        let parse_result = result.unwrap();
        assert_eq!(parse_result.worlds.len(), 1); // Only first world should be parsed
        assert!(!parse_result.metadata.warnings.is_empty()); // Should have
        // warnings
    }

    #[test]
    fn test_comment_handling() {
        let wit_source = b"// This is a comment\nworld test { }\n// Another comment";
        let result = parse_wit_embedded(wit_source);

        assert!(result.is_ok());
        let parse_result = result.unwrap();
        assert_eq!(parse_result.worlds.len(), 1);
    }

    #[test]
    fn test_validation() {
        let invalid_limits = WitParsingLimits {
            max_input_buffer: 0,
            ..WitParsingLimits::default()
        };

        let result = BoundedWitParser::new(invalid_limits);
        assert!(result.is_err());
    }
}
