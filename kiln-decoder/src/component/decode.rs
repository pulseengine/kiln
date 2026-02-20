// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Component decoding is only available with std feature due to complex
// recursive types
#[cfg(feature = "std")]
mod component_decode {
    use wrt_error::{Error, ErrorCategory, Result, codes};
    use wrt_format::{binary, component::Component};

    use crate::{
        component::parse::{
            parse_alias_section, parse_canon_section, parse_component_section,
            parse_component_type_section, parse_core_instance_section, parse_core_module_section,
            parse_core_type_section, parse_export_section, parse_import_section,
            parse_instance_section, parse_start_section, parse_value_section,
        },
        prelude::*,
    };

    /// Decode a WebAssembly Component Model binary into a structured component
    /// representation
    pub fn decode_component(bytes: &[u8]) -> Result<Component> {
        let mut component = Component::new();
        let mut offset;

        // Check magic and version
        if bytes.len() < 8 {
            return Err(Error::parse_error(
                "Component too small (less than 8 bytes)",
            ));
        }

        // Check magic number
        if bytes[0..4] != binary::COMPONENT_MAGIC {
            return Err(Error::parse_error("Invalid component magic number"));
        }

        offset = 8;

        // Parse sections
        while offset < bytes.len() {
            // Read section ID and size
            if offset + 1 > bytes.len() {
                return Err(Error::parse_error("Unexpected end of component binary"));
            }

            let section_id = bytes[offset];
            offset += 1;

            let (section_size, bytes_read) = match binary::read_leb128_u32(bytes, offset) {
                Ok(result) => result,
                Err(_) => {
                    return Err(Error::parse_error("Invalid section size"));
                },
            };
            offset += bytes_read;

            if offset + section_size as usize > bytes.len() {
                return Err(Error::parse_error("Section size exceeds binary size"));
            }

            // Extract section bytes
            let section_end = offset + section_size as usize;
            let section_bytes = &bytes[offset..section_end];
            offset = section_end;

            // Parse section based on ID
            match section_id {
                binary::COMPONENT_CUSTOM_SECTION_ID => {
                    // Custom section - read name and skip
                    match binary::read_string(section_bytes, 0) {
                        Ok((name, name_offset)) => {
                            // If this is a name section, extract the component name
                            if name == b"name" {
                                if let Ok(name_section) =
                                    crate::component::name_section::parse_component_name_section(
                                        &section_bytes[name_offset..],
                                    )
                                {
                                    // Apply the component name if available
                                    if let Some(component_name) = name_section.component_name {
                                        component.name = Some(component_name);
                                    }
                                }
                            }
                        },
                        Err(_) => {
                            // Continue parsing even if custom section name
                            // can't be read
                        },
                    }
                },
                binary::COMPONENT_CORE_MODULE_SECTION_ID => {
                    // Core module section
                    match parse_core_module_section(section_bytes) {
                        Ok((modules, _)) => {
                            component.modules.extend(modules);
                        },
                        Err(_) => {
                            // Continue parsing other sections even if this one
                            // fails
                        },
                    }
                },
                binary::COMPONENT_CORE_INSTANCE_SECTION_ID => {
                    // Core instance section
                    match parse_core_instance_section(section_bytes) {
                        Ok((instances, _)) => {
                            component.core_instances.extend(instances);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_TYPE_SECTION_ID => {
                    // Type section
                    match parse_core_type_section(section_bytes) {
                        Ok((types, _)) => {
                            component.core_types.extend(types);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }

                    // If this is a component type section, also try to parse it as such
                    match parse_component_type_section(section_bytes) {
                        Ok((types, _)) => {
                            // Add component types to component
                            component.types.extend(types);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_COMPONENT_SECTION_ID => {
                    // Component section
                    match parse_component_section(section_bytes) {
                        Ok((components, _)) => {
                            component.components.extend(components);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_INSTANCE_SECTION_ID => {
                    // Instance section
                    match parse_instance_section(section_bytes) {
                        Ok((instances, _)) => {
                            component.instances.extend(instances);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_CANON_SECTION_ID => {
                    // Canon section
                    match parse_canon_section(section_bytes) {
                        Ok((canons, _)) => {
                            component.canonicals.extend(canons);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_START_SECTION_ID => {
                    // Start section
                    match parse_start_section(section_bytes) {
                        Ok((start, _)) => {
                            component.start = Some(start);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_IMPORT_SECTION_ID => {
                    // Import section
                    match parse_import_section(section_bytes) {
                        Ok((imports, _)) => {
                            component.imports.extend(imports);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_EXPORT_SECTION_ID => {
                    // Export section
                    match parse_export_section(section_bytes) {
                        Ok((exports, _)) => {
                            component.exports.extend(exports);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_VALUE_SECTION_ID => {
                    // Value section
                    match parse_value_section(section_bytes) {
                        Ok((values, _)) => {
                            component.values.extend(values);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                binary::COMPONENT_ALIAS_SECTION_ID => {
                    // Alias section
                    match parse_alias_section(section_bytes) {
                        Ok((aliases, _)) => {
                            component.aliases.extend(aliases);
                        },
                        Err(_) => {
                            // Continue parsing other sections
                        },
                    }
                },
                _ => {
                    // Unknown section - skip
                },
            }
        }

        Ok(component)
    }

    /// Helper function to create a decode error
    pub fn decode_error(_message: &str) -> Error {
        Error::runtime_execution_error("Component decoding error")
    }

    /// Helper function to create a decode error with context
    pub fn decode_error_with_context(_message: &str, _context: &str) -> Error {
        Error::new(
            ErrorCategory::Parse,
            codes::DECODING_ERROR,
            "Component decoding error",
        )
    }

    /// Helper function to create a decode error with position
    pub fn decode_error_with_position(_message: &str, _position: usize) -> Error {
        Error::runtime_execution_error("Component decoding error")
    }

    /// Helper function to create a decode error with type
    pub fn decode_error_with_type(_message: &str, _type_name: &str) -> Error {
        Error::new(
            ErrorCategory::Parse,
            codes::DECODING_ERROR,
            "Component decoding error",
        )
    }

    /// Helper function to create a decode error with value
    pub fn decode_error_with_value(_message: &str, _value: &str) -> Error {
        Error::runtime_execution_error("Component decoding error")
    }

    /// Helper function to create a parse error
    pub fn parse_error(_message: &str) -> Error {
        Error::parse_error("Parse error ")
    }

    /// Helper function to create a parse error with context
    pub fn parse_error_with_context(_message: &str, _context: &str) -> Error {
        Error::parse_error("Parse error ")
    }

    /// Helper function to create a parse error with position
    pub fn parse_error_with_position(_message: &str, _position: usize) -> Error {
        Error::parse_error("Parse error at position ")
    }
} // end of component_decode module

// Re-export public APIs when std feature is enabled
#[cfg(feature = "std")]
pub use component_decode::{
    decode_component, decode_error, decode_error_with_context, decode_error_with_position,
    decode_error_with_type, decode_error_with_value, parse_error, parse_error_with_context,
    parse_error_with_position,
};

// No-std stub implementations
#[cfg(not(feature = "std"))]
pub mod no_std_stubs {
    use wrt_error::{Error, ErrorCategory, Result, codes};

    /// Stub component type for no_std decoding
    #[derive(Debug, Clone)]
    pub struct Component;

    /// Decode component (no_std stub)
    pub fn decode_component(_bytes: &[u8]) -> Result<Component> {
        Err(Error::runtime_execution_error(
            "Component decoding not available in no_std",
        ))
    }

    /// Helper functions (no_std stubs)
    pub fn decode_error(_message: &str) -> Error {
        Error::new(
            ErrorCategory::Parse,
            codes::DECODING_ERROR,
            "Component decoding error",
        )
    }

    pub fn decode_error_with_context(_message: &str, _context: &str) -> Error {
        Error::runtime_execution_error("Component decoding error")
    }

    pub fn decode_error_with_position(_message: &str, _position: usize) -> Error {
        Error::new(
            ErrorCategory::Parse,
            codes::DECODING_ERROR,
            "Component decoding error",
        )
    }

    pub fn decode_error_with_type(_message: &str, _type_name: &str) -> Error {
        Error::runtime_execution_error("Component decoding error")
    }

    pub fn decode_error_with_value(_message: &str, _value: &str) -> Error {
        Error::new(
            ErrorCategory::Parse,
            codes::DECODING_ERROR,
            "Component decoding error",
        )
    }

    pub fn parse_error(_message: &str) -> Error {
        Error::parse_error("Component parse error ")
    }

    pub fn parse_error_with_context(_message: &str, _context: &str) -> Error {
        Error::parse_error("Parse error ")
    }

    pub fn parse_error_with_position(_message: &str, _position: usize) -> Error {
        Error::parse_error("Parse error at position ")
    }
}

#[cfg(not(feature = "std"))]
pub use no_std_stubs::*;
