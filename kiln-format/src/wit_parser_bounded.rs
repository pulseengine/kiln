//! Bounded WIT (WebAssembly Interface Types) parser for no_std environments
//!
//! This module provides basic WIT parsing capabilities using bounded
//! collections, enabling WIT support in pure no_std environments without
//! allocation.

use wrt_error::{
    Error,
    Result,
};
use wrt_foundation::{
    BoundedString,
    MemoryProvider,
    NoStdProvider,
};

use crate::MAX_WASM_STRING_SIZE;

// Debug output was used during development - can be re-enabled if needed
// #[cfg(all(test, feature = "std"))]
// use std::eprintln;

/// Simple bounded string for no_std environments
/// This works around BoundedString issues by using a fixed array
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleBoundedString {
    data: [u8; 64], // 64 bytes should be enough for WIT identifiers
    len:  usize,
}

impl SimpleBoundedString {
    pub fn new() -> Self {
        Self {
            data: [0; 64],
            len:  0,
        }
    }

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

    /// Alias for from_str for API compatibility
    pub fn try_from_str(s: &str) -> Option<Self> {
        Self::from_str(s)
    }
}

/// Bounded WIT name for no_std environments - using simple array-based approach
pub type BoundedWitName = SimpleBoundedString;

/// Simple bounded WIT parser for no_std environments
#[derive(Debug, Clone)]
pub struct BoundedWitParser<
    P: MemoryProvider + Default + Clone + PartialEq + Eq = NoStdProvider<8192>,
> {
    /// Input text being parsed (stored as bytes for processing)
    input_buffer:    [u8; 8192], // 8KB fixed buffer
    input_len:       usize,
    /// Parsed worlds (simplified)
    worlds:          [Option<BoundedWitWorld>; 4], // Maximum 4 worlds
    /// Parsed interfaces (simplified)
    interfaces:      [Option<BoundedWitInterface>; 8], // Maximum 8 interfaces
    /// Number of parsed worlds
    world_count:     usize,
    /// Number of parsed interfaces
    interface_count: usize,
    /// Memory provider
    provider:        P,
}

/// Simple bounded WIT world definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitWorld {
    /// World name
    pub name:         BoundedWitName,
    /// Simple import/export counters for basic functionality
    pub import_count: u32,
    pub export_count: u32,
}

/// Simple bounded WIT interface definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitInterface {
    /// Interface name
    pub name:           BoundedWitName,
    /// Simple function counter for basic functionality
    pub function_count: u32,
}

/// Simple bounded WIT function definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitFunction {
    /// Function name
    pub name:         BoundedWitName,
    /// Parameter count (simplified)
    pub param_count:  u32,
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
    pub name:        BoundedWitName,
    /// Import is a function (simplified)
    pub is_function: bool,
}

/// Simple bounded export definition
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedWitExport {
    /// Export name
    pub name:        BoundedWitName,
    /// Export is a function (simplified)
    pub is_function: bool,
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> BoundedWitParser<P> {
    /// Create a new bounded WIT parser
    pub fn new(provider: P) -> Result<Self> {
        Ok(Self {
            input_buffer: [0; 8192],
            input_len: 0,
            worlds: [None, None, None, None],
            interfaces: [None, None, None, None, None, None, None, None],
            world_count: 0,
            interface_count: 0,
            provider,
        })
    }

    /// Parse WIT text input (simplified)
    pub fn parse(&mut self, input: &str) -> Result<()> {
        // Store input in fixed buffer
        let input_bytes = input.as_bytes();
        let copy_len = core::cmp::min(input_bytes.len(), self.input_buffer.len());
        self.input_buffer[..copy_len].copy_from_slice(&input_bytes[..copy_len]);
        self.input_len = copy_len;

        // Reset parser state
        self.worlds = [None, None, None, None];
        self.interfaces = [None, None, None, None, None, None, None, None];
        self.world_count = 0;
        self.interface_count = 0;

        // Simple parsing - look for "world" and "interface" keywords
        self.simple_parse()?;

        Ok(())
    }

    /// Simple parsing implementation
    fn simple_parse(&mut self) -> Result<()> {
        let mut position = 0;

        // Debug: Print the input we're parsing
        // #[cfg(all(test, feature = "std"))]
        // {
        //     if let Ok(input_str) =
        // core::str::from_utf8(&self.input_buffer[..self.input_len]) {
        //         eprintln!("[DEBUG] Parsing input: '{}'", input_str);
        //         eprintln!("[DEBUG] Input length: {}", self.input_len);
        //     }
        // }

        while position < self.input_len {
            // Skip whitespace
            #[cfg(all(test, feature = "std"))]
            let ws_start = position;
            while position < self.input_len && self.input_buffer[position].is_ascii_whitespace() {
                position += 1;
            }

            #[cfg(all(test, feature = "std"))]
            if position > ws_start {
                eprintln!(
                    "[DEBUG] Skipped {} whitespace chars at position {}",
                    position - ws_start,
                    ws_start
                );
            }

            if position >= self.input_len {
                break;
            }

            // Look for keywords - try to read a word
            let word_start = position;
            if let Some(word) = self.read_word(&mut position) {
                if let Ok(word_str) = word.as_str() {
                    #[cfg(all(test, feature = "std"))]
                    eprintln!(
                        "[DEBUG] Read word '{}' at position {}",
                        word_str, word_start
                    );

                    match word_str {
                        "world" => {
                            #[cfg(all(test, feature = "std"))]
                            eprintln!("[DEBUG] Found 'world' keyword!");

                            // Found world keyword, read the world name
                            if let Some(name) = self.read_word(&mut position) {
                                #[cfg(all(test, feature = "std"))]
                                if let Ok(name_str) = name.as_str() {
                                    eprintln!("[DEBUG] World name: '{}'", name_str);
                                }

                                self.add_world(name)?;
                                // Skip to end of line or next keyword
                                self.skip_to_next_keyword(&mut position);
                            }
                        },
                        "interface" => {
                            #[cfg(all(test, feature = "std"))]
                            eprintln!("[DEBUG] Found 'interface' keyword!");

                            // Found interface keyword, read the interface name
                            if let Some(name) = self.read_word(&mut position) {
                                #[cfg(all(test, feature = "std"))]
                                if let Ok(name_str) = name.as_str() {
                                    eprintln!("[DEBUG] Interface name: '{}'", name_str);
                                }

                                self.add_interface(name)?;
                                // Skip to end of line or next keyword
                                self.skip_to_next_keyword(&mut position);
                            }
                        },
                        _ => {
                            // Not a keyword we care about, continue
                            #[cfg(all(test, feature = "std"))]
                            eprintln!("[DEBUG] Ignoring word: '{}'", word_str);
                        },
                    }
                } else {
                    // Couldn't get string from bounded string, skip
                    #[cfg(all(test, feature = "std"))]
                    eprintln!("[DEBUG] Couldn't convert bounded string to str");
                }
            } else {
                // Couldn't read a word, advance by 1 to avoid infinite loop
                #[cfg(all(test, feature = "std"))]
                eprintln!("[DEBUG] Couldn't read word at position {}", word_start);
                position = word_start + 1;
            }
        }

        #[cfg(all(test, feature = "std"))]
        eprintln!(
            "[DEBUG] Parsing complete. Worlds: {}, Interfaces: {}",
            self.world_count, self.interface_count
        );

        Ok(())
    }

    /// Skip to the next potential keyword location (newline or '}')
    fn skip_to_next_keyword(&self, position: &mut usize) {
        while *position < self.input_len {
            let byte = self.input_buffer[*position];
            if byte == b'\n' || byte == b'}' {
                *position += 1;
                break;
            }
            *position += 1;
        }
    }

    /// Read a word from the input buffer
    fn read_word(&self, position: &mut usize) -> Option<BoundedWitName> {
        #[cfg(all(test, feature = "std"))]
        eprintln!("[DEBUG] read_word called at position {}", *position);

        // Skip whitespace
        #[cfg(all(test, feature = "std"))]
        let ws_start = *position;
        while *position < self.input_len && self.input_buffer[*position].is_ascii_whitespace() {
            *position += 1;
        }

        #[cfg(all(test, feature = "std"))]
        if *position > ws_start {
            eprintln!(
                "[DEBUG] read_word skipped {} whitespace chars",
                *position - ws_start
            );
        }

        if *position >= self.input_len {
            #[cfg(all(test, feature = "std"))]
            eprintln!("[DEBUG] read_word: reached end of input");
            return None;
        }

        let start = *position;

        #[cfg(all(test, feature = "std"))]
        eprintln!(
            "[DEBUG] read_word: starting to read word at position {}",
            start
        );

        // Read alphanumeric characters, hyphens, and underscores
        while *position < self.input_len {
            let byte = self.input_buffer[*position];
            if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
                *position += 1;
            } else {
                break;
            }
        }

        #[cfg(all(test, feature = "std"))]
        eprintln!(
            "[DEBUG] read_word: read from {} to {} (length {})",
            start,
            *position,
            *position - start
        );

        if *position > start {
            // Convert bytes to bounded string (ASCII safe)
            let word_bytes = &self.input_buffer[start..*position];
            if let Ok(word_str) = core::str::from_utf8(word_bytes) {
                #[cfg(all(test, feature = "std"))]
                eprintln!("[DEBUG] read_word: extracted word '{}'", word_str);

                // Use the simple array-based approach
                match SimpleBoundedString::try_from_str(word_str) {
                    Some(bounded_name) => {
                        #[cfg(all(test, feature = "std"))]
                        eprintln!("[DEBUG] read_word: successfully created SimpleBoundedString");
                        Some(bounded_name)
                    },
                    None => {
                        #[cfg(all(test, feature = "std"))]
                        eprintln!(
                            "[DEBUG] read_word: failed to create SimpleBoundedString (too long?)"
                        );
                        None
                    },
                }
            } else {
                #[cfg(all(test, feature = "std"))]
                eprintln!("[DEBUG] read_word: invalid UTF-8 in word bytes");
                None
            }
        } else {
            #[cfg(all(test, feature = "std"))]
            eprintln!("[DEBUG] read_word: no characters read");
            None
        }
    }

    /// Add a world to the parser
    fn add_world(&mut self, name: BoundedWitName) -> Result<()> {
        if self.world_count >= self.worlds.len() {
            // Gracefully handle capacity limit by ignoring additional worlds
            #[cfg(all(test, feature = "std"))]
            eprintln!("[DEBUG] World capacity limit reached, ignoring additional world");
            return Ok(()); // Don't error, just ignore
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

    /// Add an interface to the parser
    fn add_interface(&mut self, name: BoundedWitName) -> Result<()> {
        if self.interface_count >= self.interfaces.len() {
            // Gracefully handle capacity limit by ignoring additional interfaces
            #[cfg(all(test, feature = "std"))]
            eprintln!("[DEBUG] Interface capacity limit reached, ignoring additional interface");
            return Ok(()); // Don't error, just ignore
        }

        let interface = BoundedWitInterface {
            name,
            function_count: 0,
        };

        self.interfaces[self.interface_count] = Some(interface);
        self.interface_count += 1;

        Ok(())
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
}

impl<P: MemoryProvider + Default + Clone + PartialEq + Eq> Default for BoundedWitParser<P> {
    fn default() -> Self {
        Self::new(P::default()).unwrap_or_else(|_| {
            // Fallback to empty parser if creation fails
            Self {
                input_buffer:    [0; 8192],
                input_len:       0,
                worlds:          [None, None, None, None],
                interfaces:      [None, None, None, None, None, None, None, None],
                world_count:     0,
                interface_count: 0,
                provider:        P::default(),
            }
        })
    }
}

/// Feature detection for bounded WIT parsing
pub const HAS_BOUNDED_WIT_PARSING_NO_STD: bool = true;

/// Convenience function to parse WIT text with default provider
pub fn parse_wit_bounded(input: &str) -> Result<BoundedWitParser<NoStdProvider<8192>>> {
    // Use larger memory provider to avoid capacity issues
    let provider = wrt_foundation::safe_managed_alloc!(
        8192,
        wrt_foundation::budget_aware_provider::CrateId::Format
    )?;
    let mut parser = BoundedWitParser::new(provider)?;
    parser.parse(input)?;
    Ok(parser)
}

#[cfg(test)]
mod tests {
    use wrt_foundation::NoStdProvider;

    use super::*;

    type TestProvider = NoStdProvider<8192>;

    #[test]
    fn test_bounded_wit_parser_creation() {
        let provider = wrt_foundation::safe_managed_alloc!(
            8192,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )
        .unwrap();
        let parser = BoundedWitParser::new(provider);
        assert!(parser.is_ok());

        let parser = parser.unwrap();
        assert_eq!(parser.world_count(), 0);
        assert_eq!(parser.interface_count(), 0);
    }

    #[test]
    fn test_simple_wit_parsing() {
        let wit_text = r#"
            world test-world {
                import test-func: func(x: u32) -> string
                export main: func() -> u32
            }
        "#;

        let result = parse_wit_bounded(wit_text);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.world_count(), 1);

        let mut worlds = parser.worlds();
        let world = worlds.next().unwrap();
        assert_eq!(world.name.as_str().unwrap(), "test-world");
    }

    #[test]
    fn test_interface_parsing() {
        let wit_text = r#"
            interface test-interface {
                test-func: func(a: u32, b: string) -> bool
            }
        "#;

        let result = parse_wit_bounded(wit_text);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.interface_count(), 1);

        let mut interfaces = parser.interfaces();
        let interface = interfaces.next().unwrap();
        assert_eq!(interface.name.as_str().unwrap(), "test-interface");
    }

    #[test]
    fn test_multiple_definitions() {
        let wit_text = r#"
            world world1 {}
            interface interface1 {}
            world world2 {}
            interface interface2 {}
        "#;

        let result = parse_wit_bounded(wit_text);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.world_count(), 2);
        assert_eq!(parser.interface_count(), 2);
    }

    #[test]
    fn test_bounded_capacity_limits() {
        // Test that parser respects bounded collection limits
        let provider = wrt_foundation::safe_managed_alloc!(
            8192,
            wrt_foundation::budget_aware_provider::CrateId::Format
        )
        .unwrap();
        let mut parser = BoundedWitParser::new(provider).unwrap();

        // Create input with many worlds (should hit limit)
        let large_input = "world world0 {} world world1 {} world world2 {} world world3 {} world \
                           world4 {} world world5 {}";

        let result = parser.parse(large_input);
        assert!(result.is_ok());

        // Should have parsed up to the limit
        assert!(parser.world_count() <= 4);
    }

    #[test]
    fn test_error_handling() {
        let invalid_wit = "invalid wit syntax {{{";
        let result = parse_wit_bounded(invalid_wit);

        // Should handle gracefully (may parse partially or succeed with no results)
        assert!(result.is_ok());
        let parser = result.unwrap();
        assert_eq!(parser.world_count(), 0);
        assert_eq!(parser.interface_count(), 0);
    }

    #[test]
    fn test_empty_input() {
        let result = parse_wit_bounded("");
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.world_count(), 0);
        assert_eq!(parser.interface_count(), 0);
    }

    #[test]
    fn test_whitespace_handling() {
        let wit_text = "   world   test-world   {}   ";

        let result = parse_wit_bounded(wit_text);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.world_count(), 1);

        let mut worlds = parser.worlds();
        let world = worlds.next().unwrap();
        assert_eq!(world.name.as_str().unwrap(), "test-world");
    }

    #[test]
    fn test_simple_world() {
        // Very simple test case
        let wit_text = "world foo {}";

        let result = parse_wit_bounded(wit_text);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.world_count(), 1);

        let mut worlds = parser.worlds();
        let world = worlds.next().unwrap();
        assert_eq!(world.name.as_str().unwrap(), "foo");
    }

    #[test]
    fn test_simple_interface() {
        // Very simple test case
        let wit_text = "interface bar {}";

        let result = parse_wit_bounded(wit_text);
        assert!(result.is_ok());

        let parser = result.unwrap();
        assert_eq!(parser.interface_count(), 1);

        let mut interfaces = parser.interfaces();
        let interface = interfaces.next().unwrap();
        assert_eq!(interface.name.as_str().unwrap(), "bar");
    }
}
