// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Component parsing requires std for Box and complex recursive structures
#[cfg(feature = "std")]
mod std_parsing {
    #[cfg(feature = "tracing")]
    use wrt_foundation::tracing::trace;

    use wrt_error::{Error, Result, kinds};
    use wrt_format::{
        binary,
        component::{
            Alias, Canon, Component, ComponentType, CoreInstance, CoreType, Export, Import,
            Instance, Start, Value,
        },
        module::Module,
    };
    use wrt_foundation::resource;

    use crate::prelude::*;

    // Helper function to convert &[u8] to String
    fn bytes_to_string_safe(bytes: &[u8]) -> Result<String> {
        match core::str::from_utf8(bytes) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => Err(Error::runtime_execution_error("Invalid UTF-8")),
        }
    }

    // Backward compatible wrapper for standard String
    fn bytes_to_string(bytes: &[u8]) -> String {
        match core::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => String::new(),
        }
    }

    // Define a helper function for converting format strings to String
    fn format_to_string(message: &str, value: impl core::fmt::Display) -> String {
        #[cfg(feature = "std")]
        {
            format!("{}: {}", message, value)
        }

        #[cfg(not(feature = "std"))]
        {
            alloc::format!("{}: {}", message, value)
        }

        #[cfg(not(any(feature = "std",)))]
        {
            // Use capability-aware string creation
            let provider = match crate::prelude::create_decoder_provider::<256>() {
                Ok(p) => p,
                Err(_) => return "[allocation_error]".to_string(),
            };
            let mut s = match crate::prelude::String::from_str(message) {
                Ok(s) => s,
                Err(_) => return "[string_error]".to_string(),
            };
            if s.push_str(": ").is_ok() && s.push_str("[value]").is_ok() {
                s.into_string()
            } else {
                "[concat_error]".to_string()
            }
        }
    }

    /// Parse a core module section
    pub fn parse_core_module_section(bytes: &[u8]) -> Result<(Vec<Module>, usize)> {
        // Component Model spec: core module sections can be either:
        // 1. Inline format: section contains one module binary directly
        // 2. Vector format: count followed by (size, module)* pairs

        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;

        #[cfg(feature = "tracing")]
        {
            trace!(
                section_size = bytes.len(),
                count = count,
                "parse_core_module_section"
            );
            if count == 0 && bytes.len() > 8 {
                trace!("count is 0 but section has data - checking if inline format");
                // Check for WASM magic at start
                if bytes.len() >= 8 && &bytes[0..4] == b"\0asm" {
                    trace!("found inline core module (WASM magic at offset 0)");
                }
            }
        }

        let mut modules = Vec::with_capacity(if count == 0 { 1 } else { count as usize });

        // Handle inline format: if count is 0 and we have WASM magic, treat entire section as one module
        if count == 0 && bytes.len() >= 8 && &bytes[0..4] == b"\0asm" {
            #[cfg(feature = "tracing")]
            trace!("parsing inline core module format");

            // The entire section is one module
            let module = binary::parse_binary(bytes)?;
            modules.push(module);
            return Ok((modules, bytes.len()));
        }

        // Handle vector format
        for _ in 0..count {
            // Read a module binary size
            let (module_size, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
            offset += bytes_read;

            if offset + module_size as usize > bytes.len() {
                return Err(Error::parse_error("Module size exceeds section size"));
            }

            // Extract the module binary
            let module_end = offset + module_size as usize;
            let module_bytes = &bytes[offset..module_end];

            // Parse the module binary
            let module = binary::parse_binary(module_bytes)?;
            modules.push(module);

            offset = module_end;
        }

        Ok((modules, offset))
    }

    /// Parse a core instance section
    pub fn parse_core_instance_section(bytes: &[u8]) -> Result<(Vec<CoreInstance>, usize)> {
        // Read a vector of core instances
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        #[cfg(feature = "tracing")]
        trace!(count = count, "parse_core_instance_section");
        let mut instances = Vec::with_capacity(count as usize);

        for idx in 0..count {
            // Parse the instance expression
            #[cfg(feature = "tracing")]
            trace!(idx = idx, total = count, "parsing core instance");
            let (instance_expr, bytes_read) = parse_core_instance_expr(&bytes[offset..])?;
            offset += bytes_read;

            // Create the instance
            instances.push(CoreInstance { instance_expr });
        }

        Ok((instances, offset))
    }

    /// Parse a core instance expression
    fn parse_core_instance_expr(
        bytes: &[u8],
    ) -> Result<(wrt_format::component::CoreInstanceExpr, usize)> {
        if bytes.is_empty() {
            return Err(Error::parse_error(
                "Unexpected end of input while parsing core instance expression",
            ));
        }

        // Read the expression tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Instantiate a module
                let (module_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                #[cfg(feature = "tracing")]
                trace!(
                    tag = 0x00u8,
                    module_idx = module_idx,
                    bytes_read = bytes_read,
                    "core instance: instantiate"
                );
                offset += bytes_read;

                // Read argument vector
                let (args_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut args = Vec::with_capacity(args_count as usize);
                for _ in 0..args_count {
                    // Read name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read sort/kind byte (e.g., instance, module, func, etc.)
                    // According to Component Model spec: modulearg := name:string kind:byte idx:u32
                    // kind: 0x00=Func, 0x01=Table, 0x02=Mem, 0x03=Global, 0x12=Instance
                    if offset >= bytes.len() {
                        return Err(Error::parse_error(
                            "Unexpected end of input while parsing arg kind",
                        ));
                    }
                    let kind_byte = bytes[offset];
                    offset += 1;

                    // Read index in the corresponding component index space
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;

                    #[cfg(feature = "tracing")]
                    trace!(arg_name = %name, kind = kind_byte, idx = idx, "core instance arg");

                    args.push(wrt_format::component::CoreArgReference {
                        name,
                        kind: kind_byte,
                        idx,
                    });
                }

                Ok((
                    wrt_format::component::CoreInstanceExpr::ModuleReference {
                        module_idx,
                        arg_refs: args,
                    },
                    offset,
                ))
            },
            0x01 => {
                // Inline exports
                let (exports_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut exports = Vec::with_capacity(exports_count as usize);
                for _ in 0..exports_count {
                    // Read name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read kind byte
                    if offset >= bytes.len() {
                        return Err(Error::from(kinds::ParseError(
                            "Unexpected end of input while parsing export kind",
                        )));
                    }
                    let kind_byte = bytes[offset];
                    offset += 1;

                    // Convert to CoreSort
                    let sort = match kind_byte {
                        binary::COMPONENT_CORE_SORT_FUNC => {
                            wrt_format::component::CoreSort::Function
                        },
                        binary::COMPONENT_CORE_SORT_TABLE => wrt_format::component::CoreSort::Table,
                        binary::COMPONENT_CORE_SORT_MEMORY => {
                            wrt_format::component::CoreSort::Memory
                        },
                        binary::COMPONENT_CORE_SORT_GLOBAL => {
                            wrt_format::component::CoreSort::Global
                        },
                        binary::COMPONENT_CORE_SORT_TYPE => wrt_format::component::CoreSort::Type,
                        binary::COMPONENT_CORE_SORT_MODULE => {
                            wrt_format::component::CoreSort::Module
                        },
                        binary::COMPONENT_CORE_SORT_INSTANCE => {
                            wrt_format::component::CoreSort::Instance
                        },
                        _ => {
                            return Err(Error::from(kinds::ParseError("Invalid core sort kind")));
                        },
                    };

                    // Read index
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;

                    exports.push(wrt_format::component::CoreInlineExport { name, sort, idx });
                }

                Ok((
                    wrt_format::component::CoreInstanceExpr::InlineExports(exports),
                    offset,
                ))
            },
            _ => Err(Error::parse_error("Invalid core instance expression tag ")),
        }
    }

    /// Parse a core type section
    pub fn parse_core_type_section(bytes: &[u8]) -> Result<(Vec<CoreType>, usize)> {
        // Read a vector of core types
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut types = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Read type definition
            let (definition, bytes_read) = parse_core_type_definition(&bytes[offset..])?;
            offset += bytes_read;

            // Create the type
            types.push(CoreType { definition });
        }

        Ok((types, offset))
    }

    /// Parse a core type definition
    fn parse_core_type_definition(
        bytes: &[u8],
    ) -> Result<(wrt_format::component::CoreTypeDefinition, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing core type definition",
            )));
        }

        // Read the form tag
        let form = bytes[0];
        let mut offset = 1;

        match form {
            0x60 => {
                // Function type

                // Read parameter types
                let (param_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut params = Vec::with_capacity(param_count as usize);
                for _ in 0..param_count {
                    // Read value type
                    if offset >= bytes.len() {
                        return Err(Error::from(kinds::ParseError(
                            "Unexpected end of input while parsing function parameter type",
                        )));
                    }

                    let vtype = match bytes[offset] {
                        binary::I32_TYPE => wrt_format::types::ValueType::I32,
                        binary::I64_TYPE => wrt_format::types::ValueType::I64,
                        binary::F32_TYPE => wrt_format::types::ValueType::F32,
                        binary::F64_TYPE => wrt_format::types::ValueType::F64,
                        binary::V128_TYPE => wrt_format::types::ValueType::ExternRef,
                        binary::FUNCREF_TYPE => wrt_format::types::ValueType::FuncRef,
                        binary::EXTERNREF_TYPE => wrt_format::types::ValueType::ExternRef,
                        _ => {
                            return Err(Error::from(kinds::ParseError("Invalid value type")));
                        },
                    };

                    params.push(vtype);
                    offset += 1;
                }

                // Read result types
                let (result_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut results = Vec::with_capacity(result_count as usize);
                for _ in 0..result_count {
                    // Read value type
                    if offset >= bytes.len() {
                        return Err(Error::from(kinds::ParseError(
                            "Unexpected end of input while parsing function result type",
                        )));
                    }

                    let vtype = match bytes[offset] {
                        binary::I32_TYPE => wrt_format::types::ValueType::I32,
                        binary::I64_TYPE => wrt_format::types::ValueType::I64,
                        binary::F32_TYPE => wrt_format::types::ValueType::F32,
                        binary::F64_TYPE => wrt_format::types::ValueType::F64,
                        binary::V128_TYPE => wrt_format::types::ValueType::ExternRef,
                        binary::FUNCREF_TYPE => wrt_format::types::ValueType::FuncRef,
                        binary::EXTERNREF_TYPE => wrt_format::types::ValueType::ExternRef,
                        _ => {
                            return Err(Error::from(kinds::ParseError("Invalid value type")));
                        },
                    };

                    results.push(vtype);
                    offset += 1;
                }

                Ok((
                    wrt_format::component::CoreTypeDefinition::Function { params, results },
                    offset,
                ))
            },
            0x61 => {
                // Module type

                // Read import vector
                let (import_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut imports = Vec::with_capacity(import_count as usize);
                for _ in 0..import_count {
                    // Read module name
                    let (module_name_bytes, bytes_read) =
                        wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let module_name = bytes_to_string(module_name_bytes);

                    // Read field name
                    let (field_name_bytes, bytes_read) =
                        wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let field_name = bytes_to_string(field_name_bytes);

                    // Read import type
                    let (import_type, bytes_read) = parse_core_extern_type(&bytes[offset..])?;
                    offset += bytes_read;

                    imports.push((module_name, field_name, import_type));
                }

                // Read export vector
                let (export_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut exports = Vec::with_capacity(export_count as usize);
                for _ in 0..export_count {
                    // Read export name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read export type
                    let (export_type, bytes_read) = parse_core_extern_type(&bytes[offset..])?;
                    offset += bytes_read;

                    exports.push((name, export_type));
                }

                Ok((
                    wrt_format::component::CoreTypeDefinition::Module { imports, exports },
                    offset,
                ))
            },
            _ => Err(Error::from(kinds::ParseError("Invalid core type form"))),
        }
    }

    /// Parse a core external type
    fn parse_core_extern_type(
        bytes: &[u8],
    ) -> Result<(wrt_format::component::CoreExternType, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing core external type",
            )));
        }

        // Read the type tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Function type

                // Read type index
                let (_type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Function types are referenced by index, but we need to inline the
                // params/results In a real implementation, this would look up the
                // type in the type section For now, we'll just return an empty
                // function type
                {
                    let provider = crate::prelude::create_decoder_provider::<512>()?;
                    // Use standard Vec for std mode, explicit types for no_std
                    #[cfg(feature = "std")]
                    let empty_params = Vec::new();
                    #[cfg(feature = "std")]
                    let empty_results = Vec::new();

                    #[cfg(not(feature = "std"))]
                    let empty_params = {
                        let provider = crate::prelude::create_decoder_provider::<1024>()?;
                        crate::prelude::DecoderVec::new(provider)?
                    };
                    #[cfg(not(feature = "std"))]
                    let empty_results = {
                        let provider = crate::prelude::create_decoder_provider::<1024>()?;
                        crate::prelude::DecoderVec::new(provider)?
                    };
                    Ok((
                        wrt_format::component::CoreExternType::Function {
                            params: empty_params.to_vec(),
                            results: empty_results.to_vec(),
                        },
                        offset,
                    ))
                }
            },
            0x01 => {
                // Table type

                // Read element type
                if offset >= bytes.len() {
                    return Err(Error::from(kinds::ParseError(
                        "Unexpected end of input while parsing table element type",
                    )));
                }

                let element_type = match bytes[offset] {
                    binary::FUNCREF_TYPE => wrt_format::types::ValueType::FuncRef,
                    binary::EXTERNREF_TYPE => wrt_format::types::ValueType::ExternRef,
                    _ => {
                        return Err(Error::from(kinds::ParseError("Invalid table element type")));
                    },
                };
                offset += 1;

                // Read limits
                if offset >= bytes.len() {
                    return Err(Error::from(kinds::ParseError(
                        "Unexpected end of input while parsing table limits",
                    )));
                }

                let limit_flag = bytes[offset];
                offset += 1;

                // Read minimum size
                let (min, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read maximum size (if present)
                let max = if limit_flag & 0x01 != 0 {
                    let (max, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    Some(max)
                } else {
                    None
                };

                Ok((
                    wrt_format::component::CoreExternType::Table {
                        element_type,
                        min,
                        max,
                    },
                    offset,
                ))
            },
            0x02 => {
                // Memory type

                // Read limits
                if offset >= bytes.len() {
                    return Err(Error::from(kinds::ParseError(
                        "Unexpected end of input while parsing memory limits",
                    )));
                }

                let limit_flag = bytes[offset];
                offset += 1;

                // Check if memory is shared
                let shared = (limit_flag & 0x02) != 0;

                // Read minimum size
                let (min, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read maximum size (if present)
                let max = if limit_flag & 0x01 != 0 {
                    let (max, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    Some(max)
                } else {
                    None
                };

                Ok((
                    wrt_format::component::CoreExternType::Memory { min, max, shared },
                    offset,
                ))
            },
            0x03 => {
                // Global type

                // Read value type
                if offset >= bytes.len() {
                    return Err(Error::from(kinds::ParseError(
                        "Unexpected end of input while parsing global value type",
                    )));
                }

                let value_type = match bytes[offset] {
                    binary::I32_TYPE => wrt_format::types::ValueType::I32,
                    binary::I64_TYPE => wrt_format::types::ValueType::I64,
                    binary::F32_TYPE => wrt_format::types::ValueType::F32,
                    binary::F64_TYPE => wrt_format::types::ValueType::F64,
                    binary::V128_TYPE => wrt_format::types::ValueType::ExternRef,
                    binary::FUNCREF_TYPE => wrt_format::types::ValueType::FuncRef,
                    binary::EXTERNREF_TYPE => wrt_format::types::ValueType::ExternRef,
                    _ => {
                        return Err(Error::from(kinds::ParseError(
                            "Invalid global value type (component import)",
                        )));
                    },
                };
                offset += 1;

                // Read mutability flag
                if offset >= bytes.len() {
                    return Err(Error::from(kinds::ParseError(
                        "Unexpected end of input while parsing global mutability",
                    )));
                }

                let mutable = bytes[offset] != 0;
                offset += 1;

                Ok((
                    wrt_format::component::CoreExternType::Global {
                        value_type,
                        mutable,
                    },
                    offset,
                ))
            },
            _ => Err(Error::from(kinds::ParseError(
                "Invalid core external type tag",
            ))),
        }
    }

    /// Parse a component section
    ///
    /// According to the WebAssembly Component Model binary format, a component
    /// section (0x04) contains exactly ONE nested component binary directly.
    /// The section data IS the component binary (starting with `\x00asm` magic).
    /// Each component section defines one nested component.
    ///
    /// See: https://docs.rs/wasmparser/latest/wasmparser/enum.Payload.html
    pub fn parse_component_section(bytes: &[u8]) -> Result<(Vec<Component>, usize)> {
        // The section data is the complete nested component binary
        // No count prefix, no size prefix - just the raw component bytes

        if bytes.len() < 8 {
            return Err(Error::from(kinds::ParseError(
                "Component section too small for component binary",
            )));
        }

        // Verify component magic
        if &bytes[0..4] != b"\x00asm" {
            return Err(Error::from(kinds::ParseError(
                "Component section does not start with WebAssembly magic",
            )));
        }

        // Parse the component binary using the decoder
        match crate::component::decode_component(bytes) {
            Ok(component) => Ok((vec![component], bytes.len())),
            Err(_e) => Err(Error::from(kinds::ParseError(
                "Failed to parse nested component",
            ))),
        }
    }

    /// Parse an instance section
    pub fn parse_instance_section(bytes: &[u8]) -> Result<(Vec<Instance>, usize)> {
        // Read a vector of instances
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut instances = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Parse the instance expression
            let (instance_expr, bytes_read) = parse_instance_expr(&bytes[offset..])?;
            offset += bytes_read;

            // Create the instance
            instances.push(Instance { instance_expr });
        }

        Ok((instances, offset))
    }

    /// Parse an instance expression
    fn parse_instance_expr(bytes: &[u8]) -> Result<(wrt_format::component::InstanceExpr, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing instance expression",
            )));
        }

        // Read the expression tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Instantiate a component
                let (component_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read argument vector
                let (args_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut args = Vec::with_capacity(args_count as usize);
                for _ in 0..args_count {
                    // Read name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read sort byte
                    if offset >= bytes.len() {
                        return Err(Error::from(kinds::ParseError(
                            "Unexpected end of input while parsing instantiation argument sort",
                        )));
                    }
                    let sort_byte = bytes[offset];
                    offset += 1;

                    // Parse sort
                    let sort = parse_sort(sort_byte)?;

                    // Read index
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;

                    args.push(wrt_format::component::InstantiateArgReference { name, sort, idx });
                }

                Ok((
                    wrt_format::component::InstanceExpr::ComponentReference {
                        component_idx,
                        arg_refs: args,
                    },
                    offset,
                ))
            },
            0x01 => {
                // Inline exports
                let (exports_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut exports = Vec::with_capacity(exports_count as usize);
                for _ in 0..exports_count {
                    // Read name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read sort byte
                    if offset >= bytes.len() {
                        return Err(Error::from(kinds::ParseError(
                            "Unexpected end of input while parsing export sort",
                        )));
                    }
                    let sort_byte = bytes[offset];
                    offset += 1;

                    // Parse sort
                    let sort = parse_sort(sort_byte)?;

                    // Read index
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;

                    exports.push(wrt_format::component::InlineExport { name, sort, idx });
                }

                Ok((
                    wrt_format::component::InstanceExpr::InlineExports(exports),
                    offset,
                ))
            },
            _ => Err(Error::from(kinds::ParseError(
                "Invalid instance expression tag",
            ))),
        }
    }

    /// Parse a sort byte
    fn parse_sort(sort_byte: u8) -> Result<wrt_format::component::Sort> {
        match sort_byte {
            binary::COMPONENT_SORT_CORE => Ok(wrt_format::component::Sort::Core(
                wrt_format::component::CoreSort::Function,
            )),
            binary::COMPONENT_SORT_FUNC => Ok(wrt_format::component::Sort::Function),
            binary::COMPONENT_SORT_MODULE => Ok(wrt_format::component::Sort::Component),
            binary::COMPONENT_SORT_INSTANCE => Ok(wrt_format::component::Sort::Instance),
            binary::COMPONENT_SORT_COMPONENT => Ok(wrt_format::component::Sort::Component),
            binary::COMPONENT_SORT_VALUE => Ok(wrt_format::component::Sort::Value),
            binary::COMPONENT_SORT_TYPE => Ok(wrt_format::component::Sort::Type),
            _ => Err(Error::from(kinds::ParseError("Invalid sort byte"))),
        }
    }

    /// Parse a canon section
    pub fn parse_canon_section(bytes: &[u8]) -> Result<(Vec<Canon>, usize)> {
        // Read a vector of canon operations
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut canons = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Read canon operation
            let (operation, bytes_read) = parse_canon_operation(&bytes[offset..])?;
            offset += bytes_read;

            // Create the canon
            canons.push(Canon { operation });
        }

        Ok((canons, offset))
    }

    /// Parse a canon operation
    fn parse_canon_operation(
        bytes: &[u8],
    ) -> Result<(wrt_format::component::CanonOperation, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing canon operation",
            )));
        }

        // Read the operation tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Lift operation: 0x00 0x00 f opts ft
                // Skip the func sort discriminant (0x00)
                if offset >= bytes.len() || bytes[offset] != 0x00 {
                    return Err(Error::from(kinds::ParseError(
                        "Invalid lift operation - missing func sort discriminant",
                    )));
                }
                offset += 1;

                // Read core function index
                let (func_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read options
                let (options, bytes_read) = parse_lift_options(&bytes[offset..])?;
                offset += bytes_read;

                // Read type index
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::CanonOperation::Lift {
                        func_idx,
                        type_idx,
                        options,
                    },
                    offset,
                ))
            },
            0x01 => {
                // Lower operation: 0x01 0x00 f opts
                // Skip the func sort discriminant (0x00)
                if offset >= bytes.len() || bytes[offset] != 0x00 {
                    return Err(Error::from(kinds::ParseError(
                        "Invalid lower operation - missing func sort discriminant",
                    )));
                }
                offset += 1;

                // Read function index
                let (func_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read options
                let (options, bytes_read) = parse_lower_options(&bytes[offset..])?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::CanonOperation::Lower { func_idx, options },
                    offset,
                ))
            },
            0x02 => {
                // resource.new rt
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::CanonOperation::Resource(
                        wrt_format::component::FormatResourceOperation::New(
                            resource::ResourceNew { type_idx },
                        ),
                    ),
                    offset,
                ))
            },
            0x03 => {
                // resource.drop rt
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::CanonOperation::Resource(
                        wrt_format::component::FormatResourceOperation::Drop(
                            resource::ResourceDrop { type_idx },
                        ),
                    ),
                    offset,
                ))
            },
            0x04 => {
                // resource.rep rt
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::CanonOperation::Resource(
                        wrt_format::component::FormatResourceOperation::Rep(
                            resource::ResourceRep { type_idx },
                        ),
                    ),
                    offset,
                ))
            },
            _ => Err(Error::from(kinds::ParseError(
                "Invalid canon operation tag",
            ))),
        }
    }

    /// Parse lift options
    fn parse_lift_options(bytes: &[u8]) -> Result<(wrt_format::component::LiftOptions, usize)> {
        let mut offset = 0;

        // Read options count (vector length)
        let (count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
        offset += bytes_read;

        let mut memory_idx = None;
        let mut string_encoding = wrt_format::component::StringEncoding::UTF8;
        let mut realloc_func_idx = None;
        let mut post_return_func_idx = None;
        let mut is_async = false;

        // Parse each canonopt
        for _ in 0..count {
            if offset >= bytes.len() {
                return Err(Error::from(kinds::ParseError(
                    "Unexpected end of input while parsing canon options",
                )));
            }

            let opt_tag = bytes[offset];
            offset += 1;

            match opt_tag {
                0x00 => {
                    // UTF8 string encoding
                    string_encoding = wrt_format::component::StringEncoding::UTF8;
                },
                0x01 => {
                    // UTF16 string encoding
                    string_encoding = wrt_format::component::StringEncoding::UTF16;
                },
                0x02 => {
                    // Latin1+UTF16 string encoding
                    string_encoding = wrt_format::component::StringEncoding::Latin1;
                },
                0x03 => {
                    // (memory m)
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    memory_idx = Some(idx);
                },
                0x04 => {
                    // (realloc f)
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    realloc_func_idx = Some(idx);
                },
                0x05 => {
                    // (post-return f)
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    post_return_func_idx = Some(idx);
                },
                0x06 => {
                    // async
                    is_async = true;
                },
                0x07 => {
                    // (callback f) - skip for now
                    let (_, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                },
                _ => {
                    return Err(Error::from(kinds::ParseError("Invalid canon option tag")));
                },
            }
        }

        Ok((
            wrt_format::component::LiftOptions {
                memory_idx,
                string_encoding: Some(string_encoding),
                realloc_func_idx,
                post_return_func_idx,
                is_async,
            },
            offset,
        ))
    }

    /// Parse lower options
    fn parse_lower_options(bytes: &[u8]) -> Result<(wrt_format::component::LowerOptions, usize)> {
        let mut offset = 0;

        // Read options count (vector length)
        let (count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
        offset += bytes_read;

        let mut memory_idx = None;
        let mut string_encoding = wrt_format::component::StringEncoding::UTF8;
        let mut realloc_func_idx = None;
        let mut is_async = false;

        // Parse each canonopt
        for _ in 0..count {
            if offset >= bytes.len() {
                return Err(Error::from(kinds::ParseError(
                    "Unexpected end of input while parsing canon options",
                )));
            }

            let opt_tag = bytes[offset];
            offset += 1;

            match opt_tag {
                0x00 => {
                    // UTF8 string encoding
                    string_encoding = wrt_format::component::StringEncoding::UTF8;
                },
                0x01 => {
                    // UTF16 string encoding
                    string_encoding = wrt_format::component::StringEncoding::UTF16;
                },
                0x02 => {
                    // Latin1+UTF16 string encoding
                    string_encoding = wrt_format::component::StringEncoding::Latin1;
                },
                0x03 => {
                    // (memory m)
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    memory_idx = Some(idx);
                },
                0x04 => {
                    // (realloc f)
                    let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    realloc_func_idx = Some(idx);
                },
                0x05 => {
                    // (post-return f) - skip for now
                    let (_, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                },
                0x06 => {
                    // async
                    is_async = true;
                },
                0x07 => {
                    // (callback f) - skip for now
                    let (_, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                },
                _ => {
                    return Err(Error::from(kinds::ParseError("Invalid canon option tag")));
                },
            }
        }

        Ok((
            wrt_format::component::LowerOptions {
                memory_idx,
                string_encoding: Some(string_encoding),
                realloc_func_idx,
                is_async,
                error_mode: None,
            },
            offset,
        ))
    }

    /// Parse resource operation
    fn parse_resource_operation(
        bytes: &[u8],
    ) -> Result<(resource::ResourceCanonicalOperation, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing resource operation",
            )));
        }

        // Read the tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // New operation
                let (type_idx, type_idx_size) = binary::read_leb128_u32(bytes, offset)?;
                offset += type_idx_size;

                Ok((
                    resource::ResourceCanonicalOperation::New(resource::ResourceNew { type_idx }),
                    offset,
                ))
            },
            0x01 => {
                // Drop operation
                let (type_idx, type_idx_size) = binary::read_leb128_u32(bytes, offset)?;
                offset += type_idx_size;

                Ok((
                    resource::ResourceCanonicalOperation::Drop(resource::ResourceDrop { type_idx }),
                    offset,
                ))
            },
            0x02 => {
                // Rep operation
                let (type_idx, type_idx_size) = binary::read_leb128_u32(bytes, offset)?;
                offset += type_idx_size;

                Ok((
                    resource::ResourceCanonicalOperation::Rep(resource::ResourceRep { type_idx }),
                    offset,
                ))
            },
            _ => Err(Error::from(kinds::ParseError(
                "Invalid resource operation tag",
            ))),
        }
    }

    /// Parse a component type section
    pub fn parse_component_type_section(bytes: &[u8]) -> Result<(Vec<ComponentType>, usize)> {
        // Read a vector of component types
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut types = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Read type definition
            let (definition, bytes_read) = parse_component_type_definition(&bytes[offset..])?;
            offset += bytes_read;

            // Create the type
            types.push(ComponentType { definition });
        }

        Ok((types, offset))
    }

    /// Parse a component type definition
    fn parse_component_type_definition(
        bytes: &[u8],
    ) -> Result<(wrt_format::component::ComponentTypeDefinition, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing component type definition",
            )));
        }

        // Read the form tag (deftype)
        // Per Component Model spec:
        // 0x40 = functype
        // 0x41 = componenttype
        // 0x42 = instancetype
        let form = bytes[0];
        let mut offset = 1;

        match form {
            0x41 => {
                // Component type (0x41)

                // Read import vector
                let (import_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut imports = Vec::with_capacity(import_count as usize);
                for _ in 0..import_count {
                    // Read namespace
                    let (namespace_bytes, bytes_read) =
                        wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let namespace = bytes_to_string(namespace_bytes);

                    // Read name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read type
                    let (extern_type, bytes_read) = parse_extern_type(&bytes[offset..])?;
                    offset += bytes_read;

                    imports.push((namespace, name, extern_type));
                }

                // Read export vector
                let (export_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut exports = Vec::with_capacity(export_count as usize);
                for _ in 0..export_count {
                    // Read name
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read type
                    let (extern_type, bytes_read) = parse_extern_type(&bytes[offset..])?;
                    offset += bytes_read;

                    exports.push((name, extern_type));
                }

                Ok((
                    wrt_format::component::ComponentTypeDefinition::Component { imports, exports },
                    offset,
                ))
            },
            0x42 => {
                // Instance type (0x42)
                // instancetype ::= 0x42 id*:vec(<instancedecl>)
                // instancedecl ::= 0x00 t:<core:type>
                //                | 0x01 t:<type>
                //                | 0x02 a:<alias>
                //                | 0x04 ed:<exportdecl>

                // Read vector of instance declarations
                let (decl_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut exports = Vec::new();
                for _ in 0..decl_count {
                    if offset >= bytes.len() {
                        return Err(Error::parse_error(
                            "Unexpected end of instance declarations",
                        ));
                    }

                    let decl_tag = bytes[offset];
                    offset += 1;

                    match decl_tag {
                        0x00 => {
                            // Core type: 0x00 t:<core:type>
                            // This is an inline core type definition
                            #[cfg(feature = "tracing")]
                            trace!(offset = offset, "instancedecl: parsing inline core type");

                            let (_core_type_def, bytes_read) =
                                parse_core_type_definition(&bytes[offset..])?;
                            offset += bytes_read;
                        },
                        0x01 => {
                            // Type: 0x01 t:<type>
                            // According to spec: type ::= dt:<deftype>
                            // This is an inline type definition (deftype)
                            #[cfg(feature = "tracing")]
                            trace!(offset = offset, "instancedecl: parsing inline type");

                            let (_type_def, bytes_read) =
                                parse_component_type_definition(&bytes[offset..])?;

                            #[cfg(feature = "tracing")]
                            trace!(
                                bytes_read = bytes_read,
                                new_offset = offset + bytes_read,
                                "instancedecl: inline type consumed"
                            );

                            offset += bytes_read;
                        },
                        0x02 => {
                            // Alias - parse complete alias (sort + aliastarget)
                            // instancedecl ::= 0x02 a:<alias>
                            // alias ::= s:<sort> t:<aliastarget>
                            let (_alias, bytes_read) = parse_alias(&bytes[offset..])?;
                            offset += bytes_read;
                        },
                        0x04 => {
                            // Export declaration: name + externdesc
                            let (name_bytes, bytes_read) =
                                wrt_format::binary::read_string(bytes, offset)?;
                            offset += bytes_read;
                            let name = bytes_to_string(name_bytes);

                            let (extern_type, bytes_read) = parse_extern_type(&bytes[offset..])?;
                            offset += bytes_read;

                            exports.push((name, extern_type));
                        },
                        _ => {
                            return Err(Error::parse_error("Invalid instance declaration tag"));
                        },
                    }
                }

                Ok((
                    wrt_format::component::ComponentTypeDefinition::Instance { exports },
                    offset,
                ))
            },
            0x40 => {
                // Function type (0x40)

                // Read parameter vector
                let (param_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut params = Vec::with_capacity(param_count as usize);
                for _ in 0..param_count {
                    // Read parameter name
                    let (name, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;

                    // Read parameter type
                    let (val_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                    offset += bytes_read;

                    params.push((name, val_type));
                }

                // Read result vector
                let (result_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut results = Vec::with_capacity(result_count as usize);
                for _ in 0..result_count {
                    // Read result type
                    let (val_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                    offset += bytes_read;

                    results.push(val_type);
                }

                Ok((
                    wrt_format::component::ComponentTypeDefinition::Function {
                        params: params
                            .into_iter()
                            .map(|(name, ty)| {
                                let name_str = core::str::from_utf8(name)
                                    .unwrap_or("invalid_utf8")
                                    .to_string();
                                (name_str, val_type_to_format_val_type(ty))
                            })
                            .collect(),
                        results: results.into_iter().map(val_type_to_format_val_type).collect(),
                    },
                    offset,
                ))
            },
            0x03 => {
                // Value type

                // Read value type
                let (val_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::ComponentTypeDefinition::Value(
                        val_type_to_format_val_type(val_type),
                    ),
                    offset,
                ))
            },
            0x04 => {
                // Resource type

                // Read representation
                let (representation, bytes_read) = parse_resource_representation(&bytes[offset..])?;
                offset += bytes_read;

                // Read nullable flag
                if offset >= bytes.len() {
                    return Err(Error::from(kinds::ParseError(
                        "Unexpected end of input while parsing resource nullable flag",
                    )));
                }
                let nullable = bytes[offset] != 0;
                offset += 1;

                Ok((
                    wrt_format::component::ComponentTypeDefinition::Resource {
                        representation,
                        nullable,
                    },
                    offset,
                ))
            },
            _ => {
                // For any other form byte, try to parse as defvaltype
                // deftype ::= dvt:<defvaltype> | ft:<functype> | ct:<componenttype> | it:<instancetype> | rt:<resourcetype>
                // primvaltype ranges from 0x64-0x7f
                // defvaltype opcodes are 0x65-0x72

                #[cfg(feature = "tracing")]
                trace!(form = form, "type_def: attempting to parse as defvaltype");

                // Note: parse_val_type expects bytes[0] to be the type tag/opcode
                // The form byte at bytes[0] IS the defvaltype opcode in this case
                // parse_val_type will read it and return the total bytes consumed
                let (val_type, bytes_read) = parse_val_type(bytes)?;

                Ok((
                    wrt_format::component::ComponentTypeDefinition::Value(val_type),
                    bytes_read,
                ))
            },
        }
    }

    /// Parse a resource representation
    fn parse_resource_representation(
        bytes: &[u8],
    ) -> Result<(resource::ResourceRepresentation, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing resource representation",
            )));
        }

        // Read the tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Handle32
                Ok((resource::ResourceRepresentation::Handle32, offset))
            },
            0x01 => {
                // Handle64
                Ok((resource::ResourceRepresentation::Handle64, offset))
            },
            0x02 => {
                // Record representation

                // Read field vector
                let (field_count, bytes_read) = match binary::read_leb128_u32(bytes, offset) {
                    Ok(result) => result,
                    Err(_e) => {
                        return Err(Error::from(kinds::ParseError(
                            "Failed to read field count in resource record representation",
                        )));
                    },
                };
                offset += bytes_read;

                let mut fields = Vec::with_capacity(field_count as usize);
                for _i in 0..field_count {
                    // Read field name
                    let (name_bytes, bytes_read) =
                        match wrt_format::binary::read_string(bytes, offset) {
                            Ok(result) => result,
                            Err(_e) => {
                                return Err(Error::from(kinds::ParseError(
                                    "Failed to read field name in resource record representation",
                                )));
                            },
                        };
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    fields.push(name);
                }

                // Convert Vec to BoundedVec for ResourceRepresentation
                #[cfg(feature = "std")]
                let bounded_fields = {
                    use wrt_foundation::{
                        BoundedString, BoundedVec, NoStdProvider,
                        resource::MAX_RESOURCE_FIELD_NAME_LEN,
                    };
                    let provider = wrt_foundation::safe_managed_alloc!(
                        4096,
                        wrt_foundation::budget_aware_provider::CrateId::Decoder
                    )?;
                    let mut bounded = BoundedVec::new(provider.clone())?;
                    for field in fields {
                        let bounded_string =
                            BoundedString::<MAX_RESOURCE_FIELD_NAME_LEN>::try_from_str(&field)
                                .map_err(|_| {
                                    Error::runtime_execution_error(
                                        "Failed to create bounded string",
                                    )
                                })?;
                        if bounded.push(bounded_string).is_err() {
                            return Err(Error::new(
                                wrt_error::ErrorCategory::Memory,
                                wrt_error::codes::MEMORY_ALLOCATION_FAILED,
                                "Failed to allocate memory for string",
                            ));
                        }
                    }
                    Ok::<
                        BoundedVec<
                            BoundedString<64>,
                            16,
                            wrt_foundation::safe_memory::NoStdProvider<4096>,
                        >,
                        wrt_error::Error,
                    >(bounded)
                }?;

                Ok((
                    #[cfg(feature = "std")]
                    resource::ResourceRepresentation::Record(bounded_fields),
                    #[cfg(not(feature = "std"))]
                    resource::ResourceRepresentation::Record,
                    offset,
                ))
            },
            0x03 => {
                // Aggregate representation

                // Read type indices
                let (index_count, bytes_read) = match binary::read_leb128_u32(bytes, offset) {
                    Ok(result) => result,
                    Err(_e) => {
                        return Err(Error::from(kinds::ParseError(
                            "Failed to read index count in resource aggregate representation",
                        )));
                    },
                };
                offset += bytes_read;

                let mut indices = Vec::with_capacity(index_count as usize);
                for _i in 0..index_count {
                    // Read type index
                    let (idx, bytes_read) = match binary::read_leb128_u32(bytes, offset) {
                        Ok(result) => result,
                        Err(_e) => {
                            return Err(Error::parse_error(
                                "Failed to read type index in resource aggregate representation",
                            ));
                        },
                    };
                    offset += bytes_read;

                    indices.push(idx);
                }

                #[cfg(feature = "std")]
                let repr = {
                    use wrt_foundation::{BoundedVec, NoStdProvider};
                    let provider = wrt_foundation::safe_managed_alloc!(
                        4096,
                        wrt_foundation::budget_aware_provider::CrateId::Decoder
                    )?;
                    let mut bounded_indices = BoundedVec::new(provider)?;
                    for idx in indices {
                        if bounded_indices.push(idx).is_err() {
                            return Err(Error::runtime_execution_error(
                                "Failed to push index to bounded vector",
                            ));
                        }
                    }
                    resource::ResourceRepresentation::Aggregate(bounded_indices)
                };
                #[cfg(not(feature = "std"))]
                let repr = resource::ResourceRepresentation::Record;

                Ok((repr, offset))
            },
            _ => Err(Error::parse_error("Invalid resource representation tag ")),
        }
    }

    /// Parse an external type descriptor (externdesc)
    /// Based on Component Model spec:
    /// externdesc ::= 0x00 0x11 i:<core:typeidx>  => (core module (type i))
    ///              | 0x01 i:<typeidx>             => (func (type i))
    ///              | 0x02 b:<valuebound>          => (value b)
    ///              | 0x03 b:<typebound>           => (type b)
    ///              | 0x04 i:<typeidx>             => (component (type i))
    ///              | 0x05 i:<typeidx>             => (instance (type i))
    fn parse_extern_type(bytes: &[u8]) -> Result<(wrt_format::component::ExternType, usize)> {
        if bytes.is_empty() {
            return Err(Error::parse_error(
                "Unexpected end of input while parsing external type",
            ));
        }

        // Read the tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Core module: 0x00 0x11 i:<core:typeidx>
                if offset >= bytes.len() {
                    return Err(Error::parse_error(
                        "Unexpected end of core module extern type",
                    ));
                }

                let sort_tag = bytes[offset];
                offset += 1;

                if sort_tag != 0x11 {
                    return Err(Error::parse_error("Expected 0x11 (module sort) after 0x00"));
                }

                // Read core type index
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::ExternType::Module { type_idx },
                    offset,
                ))
            },
            0x01 => {
                // Function: 0x01 i:<typeidx>
                // This is a type reference, not an inline function definition
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((wrt_format::component::ExternType::Type(type_idx), offset))
            },
            0x02 => {
                // Value bound: 0x02 b:<valuebound>
                // valuebound ::= 0x00 i:<valueidx> => (eq i)
                //              | 0x01 t:<valtype>  => t
                if offset >= bytes.len() {
                    return Err(Error::parse_error("Unexpected end of value bound"));
                }

                let bound_tag = bytes[offset];
                offset += 1;

                match bound_tag {
                    0x00 => {
                        // Eq bound - value index
                        let (value_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                        offset += bytes_read;
                        // Use a simple type as placeholder for value reference
                        Ok((
                            wrt_format::component::ExternType::Value(
                                wrt_format::component::FormatValType::Bool,
                            ),
                            offset,
                        ))
                    },
                    0x01 => {
                        // Direct value type
                        let (val_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                        offset += bytes_read;
                        Ok((
                            wrt_format::component::ExternType::Value(val_type_to_format_val_type(
                                val_type,
                            )),
                            offset,
                        ))
                    },
                    _ => Err(Error::parse_error("Invalid value bound tag")),
                }
            },
            0x03 => {
                // Type bound: 0x03 b:<typebound>
                // typebound ::= 0x00 i:<typeidx> => (eq i)
                //             | 0x01            => (sub resource)
                if offset >= bytes.len() {
                    return Err(Error::parse_error("Unexpected end of type bound"));
                }

                let bound_tag = bytes[offset];
                offset += 1;

                match bound_tag {
                    0x00 => {
                        // Eq bound - type index
                        let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                        offset += bytes_read;
                        Ok((wrt_format::component::ExternType::Type(type_idx), offset))
                    },
                    0x01 => {
                        // Sub resource bound
                        Ok((wrt_format::component::ExternType::Type(0xFFFFFFFF), offset))
                    },
                    _ => Err(Error::parse_error("Invalid type bound tag")),
                }
            },
            0x04 => {
                // Component: 0x04 i:<typeidx>
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::ExternType::Component { type_idx },
                    offset,
                ))
            },
            0x05 => {
                // Instance: 0x05 i:<typeidx>
                // This is a type reference, not an inline instance definition
                let (type_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((wrt_format::component::ExternType::Type(type_idx), offset))
            },
            _ => Err(Error::parse_error("Invalid external type tag")),
        }
    }

    /// Parse a value type
    /// Parse a valtype according to Component Model spec
    ///
    /// valtype ::= i:<typeidx> => i
    ///           | pvt:<primvaltype> => pvt
    ///
    /// This can be either a type index (non-negative) or a primvaltype (negative SLEB128)
    fn parse_val_type(bytes: &[u8]) -> Result<(wrt_format::component::FormatValType, usize)> {
        if bytes.is_empty() {
            return Err(Error::parse_error(
                "Unexpected end of input while parsing value type",
            ));
        }

        // Read the type tag - could be a type index or primvaltype/defvaltype opcode
        let tag = bytes[0];

        // Check against known opcodes first, then fall back to type index
        let mut offset = 1;

        match tag {
            // primvaltype opcodes (0x64-0x7f)
            0x7F => Ok((wrt_format::component::FormatValType::Bool, offset)),
            0x7E => Ok((wrt_format::component::FormatValType::S8, offset)),
            0x7D => Ok((wrt_format::component::FormatValType::U8, offset)),
            0x7C => Ok((wrt_format::component::FormatValType::S16, offset)),
            0x7B => Ok((wrt_format::component::FormatValType::U16, offset)),
            0x7A => Ok((wrt_format::component::FormatValType::S32, offset)),
            0x79 => Ok((wrt_format::component::FormatValType::U32, offset)),
            0x78 => Ok((wrt_format::component::FormatValType::S64, offset)),
            0x77 => Ok((wrt_format::component::FormatValType::U64, offset)),
            0x76 => Ok((wrt_format::component::FormatValType::F32, offset)),
            0x75 => Ok((wrt_format::component::FormatValType::F64, offset)),
            0x74 => Ok((wrt_format::component::FormatValType::Char, offset)),
            0x73 => Ok((wrt_format::component::FormatValType::String, offset)),
            0x64 => Ok((wrt_format::component::FormatValType::ErrorContext, offset)),

            // defvaltype opcodes (0x65-0x72)
            0x72 => {
                // Record type: 0x72 lt*:vec(<labelvaltype>) => (record (field lt)*)
                // labelvaltype ::= l:<label'> t:<valtype>
                let (field_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut fields = Vec::with_capacity(field_count as usize);
                for _ in 0..field_count {
                    // Read field label (name)
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read field valtype
                    let (field_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                    offset += bytes_read;

                    fields.push((name, field_type));
                }

                Ok((wrt_format::component::FormatValType::Record(fields), offset))
            },
            0x71 => {
                // Variant type: 0x71 case*:vec(<case>) => (variant case+)
                // case ::= l:<label'> t?:<valtype>? 0x00 => (case l t?)
                let (case_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut cases = Vec::with_capacity(case_count as usize);
                for _ in 0..case_count {
                    // Read case label (name)
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);

                    // Read optional type: <T>? ::= 0x00 | 0x01 t:<T>
                    if offset >= bytes.len() {
                        return Err(Error::parse_error("Unexpected end parsing variant case"));
                    }
                    let has_type_flag = bytes[offset];
                    offset += 1;

                    let case_type = if has_type_flag == 0x01 {
                        let (ty, bytes_read) = parse_val_type(&bytes[offset..])?;
                        offset += bytes_read;
                        Some(ty)
                    } else {
                        None
                    };

                    // Read trailing 0x00 byte
                    if offset >= bytes.len() {
                        return Err(Error::parse_error(
                            "Unexpected end parsing variant case trailer",
                        ));
                    }
                    if bytes[offset] != 0x00 {
                        return Err(Error::parse_error("Expected 0x00 trailer in variant case"));
                    }
                    offset += 1;

                    cases.push((name, case_type));
                }

                Ok((wrt_format::component::FormatValType::Variant(cases), offset))
            },
            0x70 => {
                // List type: 0x70 t:<valtype> => (list t)
                let (element_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                offset += bytes_read;
                Ok((
                    wrt_format::component::FormatValType::List(Box::new(element_type)),
                    offset,
                ))
            },
            0x6F => {
                // Tuple type: 0x6f t*:vec(<valtype>) => (tuple t+)
                #[cfg(feature = "tracing")]
                trace!(offset = offset, "parse_val_type: parsing tuple");

                let (elem_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                #[cfg(feature = "tracing")]
                trace!(
                    elem_count = elem_count,
                    offset = offset,
                    "parse_val_type: tuple element count"
                );

                let mut elements = Vec::with_capacity(elem_count as usize);
                for i in 0..elem_count {
                    let (elem_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                    #[cfg(feature = "tracing")]
                    trace!(
                        elem_index = i,
                        bytes_read = bytes_read,
                        new_offset = offset + bytes_read,
                        "parse_val_type: tuple element"
                    );
                    offset += bytes_read;
                    elements.push(elem_type);
                }

                #[cfg(feature = "tracing")]
                trace!(offset = offset, "parse_val_type: tuple complete");

                Ok((
                    wrt_format::component::FormatValType::Tuple(elements),
                    offset,
                ))
            },
            0x6E => {
                // Flags type: 0x6e l*:vec(<label'>) => (flags l+)
                let (flag_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut flags = Vec::with_capacity(flag_count as usize);
                for _ in 0..flag_count {
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);
                    flags.push(name);
                }

                Ok((wrt_format::component::FormatValType::Flags(flags), offset))
            },
            0x6D => {
                // Enum type: 0x6d l*:vec(<label'>) => (enum l+)
                let (case_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut cases = Vec::with_capacity(case_count as usize);
                for _ in 0..case_count {
                    let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let name = bytes_to_string(name_bytes);
                    cases.push(name);
                }

                Ok((wrt_format::component::FormatValType::Enum(cases), offset))
            },
            0x6B => {
                // Option type: 0x6b t:<valtype> => (option t)
                let (inner_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                offset += bytes_read;
                Ok((
                    wrt_format::component::FormatValType::Option(Box::new(inner_type)),
                    offset,
                ))
            },
            0x6A => {
                // Result type: 0x6a t?:<valtype>? u?:<valtype>? => (result t? (error u)?)
                // This is complex - result can have ok type, error type, both, or neither
                // <T>? ::= 0x00 | 0x01 t:<T>

                // Read ok type (optional)
                if offset >= bytes.len() {
                    return Err(Error::parse_error("Unexpected end parsing result ok type"));
                }
                let has_ok_flag = bytes[offset];
                offset += 1;

                let ok_type = if has_ok_flag == 0x01 {
                    let (ty, bytes_read) = parse_val_type(&bytes[offset..])?;
                    offset += bytes_read;
                    Some(Box::new(ty))
                } else {
                    None
                };

                // Read error type (optional)
                if offset >= bytes.len() {
                    return Err(Error::parse_error(
                        "Unexpected end parsing result error type",
                    ));
                }
                let has_err_flag = bytes[offset];
                offset += 1;

                let err_type = if has_err_flag == 0x01 {
                    let (ty, bytes_read) = parse_val_type(&bytes[offset..])?;
                    offset += bytes_read;
                    Some(Box::new(ty))
                } else {
                    None
                };

                // Encode as a Result with combined type
                // FormatValType::Result expects a single Box<FormatValType>
                // We'll use a tuple or variant to represent both ok/error
                // For now, simplify to just the ok type
                let result_type =
                    ok_type.unwrap_or_else(|| Box::new(wrt_format::component::FormatValType::Void));

                Ok((
                    wrt_format::component::FormatValType::Result(result_type),
                    offset,
                ))
            },
            0x69 => {
                // Own type: 0x69 i:<typeidx> => (own i)
                let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;
                Ok((wrt_format::component::FormatValType::Own(idx), offset))
            },
            0x68 => {
                // Borrow type: 0x68 i:<typeidx> => (borrow i)
                let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;
                Ok((wrt_format::component::FormatValType::Borrow(idx), offset))
            },
            0x67 => {
                // Fixed-length list type: 0x67 t:<valtype> len:<u32> => (list t len) 🔧
                let (element_type, bytes_read) = parse_val_type(&bytes[offset..])?;
                offset += bytes_read;

                // Read the length
                let (length, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::FormatValType::FixedList(Box::new(element_type), length),
                    offset,
                ))
            },

            // Note: 0x66 (stream) and 0x65 (future) are part of the async proposal (🔀)
            // We don't have these in FormatValType yet, so skip them for now
            0x66 | 0x65 => {
                #[cfg(feature = "tracing")]
                trace!(
                    tag = tag,
                    "val_type: skipping async type (stream/future not yet implemented)"
                );
                // For now, treat as Void
                Ok((wrt_format::component::FormatValType::Void, offset))
            },

            _ => {
                // Not a known opcode - try to parse as a type index (unsigned LEB128)
                #[cfg(feature = "tracing")]
                trace!(
                    tag = tag,
                    "val_type: not a known opcode, parsing as type index"
                );

                let (idx, bytes_read) = binary::read_leb128_u32(bytes, 0)?;
                Ok((wrt_format::component::FormatValType::Ref(idx), bytes_read))
            },
        }
    }
    /// Parse a start section
    pub fn parse_start_section(bytes: &[u8]) -> Result<(Start, usize)> {
        let mut offset = 0;

        // Read function index
        let (func_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
        offset += bytes_read;

        // Read argument vector
        let (arg_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
        offset += bytes_read;

        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
            // Read argument index
            let (arg_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
            offset += bytes_read;

            args.push(arg_idx);
        }

        // Read results count
        let (results, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
        offset += bytes_read;

        Ok((
            Start {
                func_idx,
                args,
                results,
            },
            offset,
        ))
    }

    /// Parse an import section
    pub fn parse_import_section(bytes: &[u8]) -> Result<(Vec<Import>, usize)> {
        // Read a vector of imports
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut imports = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Read importname which consists of TWO strings: namespace and name
            // Namespace string (can be empty)
            let (namespace_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
            offset += bytes_read;
            let namespace = bytes_to_string(namespace_bytes);

            // Name string (the interface name like "wasi:cli/environment@0.2.0")
            let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
            offset += bytes_read;
            let name = bytes_to_string(name_bytes);

            // Read externdesc (external descriptor)
            let (extern_type, bytes_read) = parse_extern_type(&bytes[offset..])?;
            offset += bytes_read;

            // Create the import
            imports.push(Import {
                name: wrt_format::component::ImportName {
                    namespace,
                    name,
                    nested: Vec::new(),
                    package: None,
                },
                ty: extern_type,
            });
        }

        Ok((imports, offset))
    }

    /// Parse an export section
    pub fn parse_export_section(bytes: &[u8]) -> Result<(Vec<Export>, usize)> {
        // Per Component Model spec:
        // export ::= n:<exportname'> si:<sortidx>
        // Note: exportname' uses same two-string format as importname (namespace + name)
        // sortidx ::= sort:<sort> idx:<u32>

        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut exports = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Read export name - appears to use same two-string format as imports
            // Namespace string (can be empty)
            let (namespace_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
            offset += bytes_read;
            let _namespace = bytes_to_string(namespace_bytes);

            // Name string (the export name like "wasi:cli/run@0.2.0")
            let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
            offset += bytes_read;
            let name = bytes_to_string(name_bytes);

            // Read sortidx: sort byte + index
            if offset >= bytes.len() {
                return Err(Error::parse_error(
                    "Unexpected end of input while parsing export sort",
                ));
            }
            let sort_byte = bytes[offset];
            offset += 1;

            // Parse sort (0x00-0x05)
            let sort = parse_sort(sort_byte)?;

            // Read index
            let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
            offset += bytes_read;

            // Create export with simplified name structure
            let export_name = wrt_format::component::ExportName {
                name,
                is_resource: false,
                semver: None,
                integrity: None,
                nested: Vec::new(),
            };

            exports.push(Export {
                name: export_name,
                sort,
                idx,
                ty: None, // Type information comes from type section via index
            });
        }

        Ok((exports, offset))
    }

    /// Parse a value section
    pub fn parse_value_section(bytes: &[u8]) -> Result<(Vec<Value>, usize)> {
        // Read a vector of values
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut values = Vec::with_capacity(count as usize);

        for _ in 0..count {
            // Read value type
            let (val_type, bytes_read) = parse_val_type(&bytes[offset..])?;
            offset += bytes_read;

            // Read value bytes size
            let (data_size, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
            offset += bytes_read;

            if offset + data_size as usize > bytes.len() {
                return Err(Error::from(kinds::ParseError(
                    "Value data size exceeds section size",
                )));
            }

            // Extract the value data
            let data_end = offset + data_size as usize;
            let data = bytes[offset..data_end].to_vec();
            offset = data_end;

            // Check for expression flag
            let mut expression = None;
            if offset < bytes.len() {
                let has_expr = bytes[offset] != 0;
                offset += 1;

                if has_expr {
                    let (expr, bytes_read) = parse_value_expression(&bytes[offset..])?;
                    offset += bytes_read;
                    expression = Some(expr);
                }
            }

            // Check for name flag
            let mut name = None;
            if offset < bytes.len() {
                let has_name = bytes[offset] != 0;
                offset += 1;

                if has_name {
                    let (value_name_bytes, bytes_read) =
                        wrt_format::binary::read_string(bytes, offset)?;
                    offset += bytes_read;
                    let value_name = bytes_to_string(value_name_bytes);
                    name = Some(value_name);
                }
            }

            // Create the value
            values.push(Value {
                ty: val_type_to_format_val_type(val_type),
                data,
                expression,
                name,
            });
        }

        Ok((values, offset))
    }

    /// Parse a value expression
    fn parse_value_expression(
        bytes: &[u8],
    ) -> Result<(wrt_format::component::ValueExpression, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing value expression",
            )));
        }

        // Read the expression tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Reference to an item
                let kind_byte = bytes[offset];
                offset += 1;

                // Convert to Sort
                let sort = parse_sort(kind_byte)?;

                // Read index
                let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::ValueExpression::ItemRef { sort, idx },
                    offset,
                ))
            },
            0x01 => {
                // Global initialization
                let (global_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::ValueExpression::GlobalInit { global_idx },
                    offset,
                ))
            },
            0x02 => {
                // Function call
                let (func_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read args vector
                let (args_count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                let mut args = Vec::with_capacity(args_count as usize);
                for _ in 0..args_count {
                    let (arg_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                    offset += bytes_read;
                    args.push(arg_idx);
                }

                Ok((
                    wrt_format::component::ValueExpression::FunctionCall { func_idx, args },
                    offset,
                ))
            },
            0x03 => {
                // Constant value
                let (const_value, bytes_read) = parse_const_value(&bytes[offset..])?;
                offset += bytes_read;

                Ok((
                    wrt_format::component::ValueExpression::Const(const_value),
                    offset,
                ))
            },
            _ => Err(Error::from(kinds::ParseError(
                "Invalid value expression tag",
            ))),
        }
    }

    /// Parse a constant value
    fn parse_const_value(bytes: &[u8]) -> Result<(wrt_format::component::ConstValue, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing constant value",
            )));
        }

        // Read the value tag
        let tag = bytes[0];
        let mut offset = 1;

        match tag {
            0x00 => {
                // Boolean value
                let value = bytes[offset] != 0;
                offset += 1;
                Ok((wrt_format::component::ConstValue::Bool(value), offset))
            },
            0x01 => {
                // S8 value
                let value = bytes[offset] as i8;
                offset += 1;
                Ok((wrt_format::component::ConstValue::S8(value), offset))
            },
            0x02 => {
                // U8 value
                let value = bytes[offset];
                offset += 1;
                Ok((wrt_format::component::ConstValue::U8(value), offset))
            },
            0x03 => {
                // S16 value
                let value = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                offset += 2;
                Ok((wrt_format::component::ConstValue::S16(value), offset))
            },
            0x04 => {
                // U16 value
                let value = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                offset += 2;
                Ok((wrt_format::component::ConstValue::U16(value), offset))
            },
            0x05 => {
                // S32 value
                let value = i32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                offset += 4;
                Ok((wrt_format::component::ConstValue::S32(value), offset))
            },
            0x06 => {
                // U32 value
                let value = u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                offset += 4;
                Ok((wrt_format::component::ConstValue::U32(value), offset))
            },
            0x07 => {
                // S64 value
                let value = i64::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                offset += 8;
                Ok((wrt_format::component::ConstValue::S64(value), offset))
            },
            0x08 => {
                // U64 value
                let value = u64::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                offset += 8;
                Ok((wrt_format::component::ConstValue::U64(value), offset))
            },
            0x09 => {
                // F32 value
                let value_bits = u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]);
                let value = f32::from_bits(value_bits);
                offset += 4;
                Ok((wrt_format::component::ConstValue::F32(value), offset))
            },
            0x0A => {
                // F64 value
                let value_bits = u64::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                    bytes[offset + 4],
                    bytes[offset + 5],
                    bytes[offset + 6],
                    bytes[offset + 7],
                ]);
                let value = f64::from_bits(value_bits);
                offset += 8;
                Ok((wrt_format::component::ConstValue::F64(value), offset))
            },
            0x0B => {
                // Char value
                let (value_str, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                offset += bytes_read;

                // Convert bytes to string and validate that it's a single Unicode scalar value
                let value_string = core::str::from_utf8(value_str)
                    .map_err(|_| Error::parse_error("Invalid UTF-8 in char value "))?;
                let mut chars = value_string.chars();
                let first_char = chars.next().ok_or_else(|| {
                    Error::from(kinds::ParseError(
                        "Empty string found when parsing char value",
                    ))
                })?;
                if chars.next().is_some() {
                    return Err(Error::from(kinds::ParseError(
                        "Multiple characters found when parsing char value",
                    )));
                }

                Ok((wrt_format::component::ConstValue::Char(first_char), offset))
            },
            0x0C => {
                // String value
                let (value_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                offset += bytes_read;
                let value = bytes_to_string(value_bytes);
                Ok((wrt_format::component::ConstValue::String(value), offset))
            },
            0x0D => {
                // Null value
                Ok((wrt_format::component::ConstValue::Null, offset))
            },
            _ => Err(Error::from(kinds::ParseError("Invalid constant value tag"))),
        }
    }

    /// Parse a single alias (sort + aliastarget)
    ///
    /// This is the complete alias production: alias ::= s:<sort> t:<aliastarget>
    pub fn parse_alias(bytes: &[u8]) -> Result<(Alias, usize)> {
        let (target, bytes_read) = parse_alias_target(bytes)?;
        Ok((
            Alias {
                target,
                dest_idx: None,
            },
            bytes_read,
        ))
    }

    /// Parse an alias section
    pub fn parse_alias_section(bytes: &[u8]) -> Result<(Vec<Alias>, usize)> {
        // Read a vector of aliases
        let (count, mut offset) = binary::read_leb128_u32(bytes, 0)?;
        let mut aliases = Vec::with_capacity(count as usize);

        #[cfg(feature = "tracing")]
        trace!(
            count = count,
            section_size = bytes.len(),
            "parse_alias_section"
        );

        for i in 0..count {
            #[cfg(feature = "tracing")]
            trace!(alias_index = i, offset = offset, "parsing alias");

            // Parse complete alias (sort + aliastarget)
            let (alias, bytes_read) = parse_alias(&bytes[offset..])?;
            offset += bytes_read;

            aliases.push(alias);
        }

        Ok((aliases, offset))
    }

    /// Parse an alias target
    ///
    /// According to the Component Model spec:
    /// alias ::= s:<sort> t:<aliastarget>
    ///
    /// Where:
    /// - s is the sort byte (func, table, memory, global, type, module, instance, etc.)
    ///   * For COMPONENT_SORT_CORE (0x00), this is followed by a CoreSort byte
    ///   * For other values, this is the complete Sort
    /// - t is the aliastarget discriminant (0x00=export, 0x01=core export, 0x02=outer)
    fn parse_alias_target(bytes: &[u8]) -> Result<(wrt_format::component::AliasTarget, usize)> {
        if bytes.is_empty() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing alias target",
            )));
        }

        // FIRST: Read the sort byte (what kind of thing is being aliased)
        let sort_byte = bytes[0];
        let mut offset = 1;

        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(
            sort_byte = format!("0x{:02x}", sort_byte),
            "Parsing alias"
        );

        // Parse the sort - this may be a CoreSort (if sort_byte == 0x00) or a full Sort
        let parsed_sort: wrt_format::component::Sort;

        if sort_byte == binary::COMPONENT_SORT_CORE {
            // This is a core item - read the CoreSort byte
            if offset >= bytes.len() {
                return Err(Error::from(kinds::ParseError(
                    "Unexpected end of input while parsing CoreSort",
                )));
            }
            let core_sort_byte = bytes[offset];
            offset += 1;

            #[cfg(feature = "tracing")]
            wrt_foundation::tracing::trace!(
                core_sort_byte = format!("0x{:02x}", core_sort_byte),
                "Core sort byte"
            );

            let core_sort = match core_sort_byte {
                binary::COMPONENT_CORE_SORT_FUNC => wrt_format::component::CoreSort::Function,
                binary::COMPONENT_CORE_SORT_TABLE => wrt_format::component::CoreSort::Table,
                binary::COMPONENT_CORE_SORT_MEMORY => wrt_format::component::CoreSort::Memory,
                binary::COMPONENT_CORE_SORT_GLOBAL => wrt_format::component::CoreSort::Global,
                binary::COMPONENT_CORE_SORT_TYPE => wrt_format::component::CoreSort::Type,
                binary::COMPONENT_CORE_SORT_MODULE => wrt_format::component::CoreSort::Module,
                binary::COMPONENT_CORE_SORT_INSTANCE => wrt_format::component::CoreSort::Instance,
                _ => {
                    #[cfg(feature = "tracing")]
                    wrt_foundation::tracing::warn!(
                        core_sort_byte = format!("0x{:02x}", core_sort_byte),
                        "Invalid core sort byte"
                    );
                    return Err(Error::from(kinds::ParseError("Invalid core sort byte")));
                },
            };
            parsed_sort = wrt_format::component::Sort::Core(core_sort);
        } else {
            // This is a component-level sort
            parsed_sort = match sort_byte {
                binary::COMPONENT_SORT_FUNC => wrt_format::component::Sort::Function,
                binary::COMPONENT_SORT_VALUE => wrt_format::component::Sort::Value,
                binary::COMPONENT_SORT_TYPE => wrt_format::component::Sort::Type,
                binary::COMPONENT_SORT_COMPONENT => wrt_format::component::Sort::Component,
                binary::COMPONENT_SORT_INSTANCE => wrt_format::component::Sort::Instance,
                binary::COMPONENT_SORT_MODULE => wrt_format::component::Sort::Component,
                _ => {
                    #[cfg(feature = "tracing")]
                    wrt_foundation::tracing::warn!(
                        sort_byte = format!("0x{:02x}", sort_byte),
                        "Invalid sort byte"
                    );
                    return Err(Error::from(kinds::ParseError("Invalid sort byte")));
                },
            };
        }

        // SECOND: Read the aliastarget tag (where it comes from)
        if offset >= bytes.len() {
            return Err(Error::from(kinds::ParseError(
                "Unexpected end of input while parsing alias target tag",
            )));
        }
        let tag = bytes[offset];
        offset += 1;

        #[cfg(feature = "tracing")]
        wrt_foundation::tracing::trace!(tag = format!("0x{:02x}", tag), "Alias target tag");

        match tag {
            0x00 => {
                // 0x00 = export (component instance export)

                // Read instance index
                let (instance_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read export name
                let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                offset += bytes_read;
                let name = bytes_to_string(name_bytes);

                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::trace!(instance_idx = instance_idx, name = %name, sort = ?parsed_sort, "Component export alias");

                Ok((
                    wrt_format::component::AliasTarget::InstanceExport {
                        instance_idx,
                        name,
                        kind: parsed_sort,
                    },
                    offset,
                ))
            },
            0x01 => {
                // 0x01 = core export (from core instance)

                // For core exports, the sort must be Sort::Core(CoreSort)
                let core_sort = match parsed_sort {
                    wrt_format::component::Sort::Core(cs) => cs,
                    _ => {
                        #[cfg(feature = "tracing")]
                        wrt_foundation::tracing::warn!(parsed_sort = ?parsed_sort, "Core export must have CoreSort");
                        return Err(Error::from(kinds::ParseError(
                            "Core export must have CoreSort",
                        )));
                    },
                };

                // Read instance index
                let (instance_idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read export name
                let (name_bytes, bytes_read) = wrt_format::binary::read_string(bytes, offset)?;
                offset += bytes_read;
                let name = bytes_to_string(name_bytes);

                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::trace!(instance_idx = instance_idx, name = %name, core_sort = ?core_sort, "Core export alias");

                Ok((
                    wrt_format::component::AliasTarget::CoreInstanceExport {
                        instance_idx,
                        name,
                        kind: core_sort,
                    },
                    offset,
                ))
            },
            0x02 => {
                // 0x02 = outer (from enclosing component)

                // Read count (nesting level)
                let (count, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                // Read index
                let (idx, bytes_read) = binary::read_leb128_u32(bytes, offset)?;
                offset += bytes_read;

                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::trace!(count = count, idx = idx, sort = ?parsed_sort, "Outer alias");

                Ok((
                    wrt_format::component::AliasTarget::Outer {
                        count,
                        kind: parsed_sort,
                        idx,
                    },
                    offset,
                ))
            },
            _ => {
                #[cfg(feature = "tracing")]
                wrt_foundation::tracing::warn!(
                    tag = format!("0x{:02x}", tag),
                    "Unsupported alias target tag"
                );
                Err(Error::from(kinds::ParseError("Invalid alias target tag")))
            },
        }
    }

    /// Parse a name from a byte array
    ///
    /// This is a utility function to parse a name field, which is common in
    /// various WebAssembly and Component Model sections.
    #[allow(dead_code)]
    pub fn parse_name(bytes: &[u8]) -> Result<(String, usize)> {
        let (name_bytes, length) = wrt_format::binary::read_string(bytes, 0)?;
        let name_str = core::str::from_utf8(name_bytes)
            .map_err(|_| Error::runtime_execution_error("Invalid UTF-8 in name"))?;
        Ok((name_str.to_string(), length))
    }

    /// Convert ValType to FormatValType for type compatibility
    fn val_type_to_format_val_type(
        val_type: wrt_format::component::FormatValType,
    ) -> wrt_format::component::FormatValType {
        match val_type {
            wrt_format::component::FormatValType::Bool => {
                wrt_format::component::FormatValType::Bool
            },
            wrt_format::component::FormatValType::S8 => wrt_format::component::FormatValType::S8,
            wrt_format::component::FormatValType::U8 => wrt_format::component::FormatValType::U8,
            wrt_format::component::FormatValType::S16 => wrt_format::component::FormatValType::S16,
            wrt_format::component::FormatValType::U16 => wrt_format::component::FormatValType::U16,
            wrt_format::component::FormatValType::S32 => wrt_format::component::FormatValType::S32,
            wrt_format::component::FormatValType::U32 => wrt_format::component::FormatValType::U32,
            wrt_format::component::FormatValType::S64 => wrt_format::component::FormatValType::S64,
            wrt_format::component::FormatValType::U64 => wrt_format::component::FormatValType::U64,
            wrt_format::component::FormatValType::F32 => wrt_format::component::FormatValType::F32,
            wrt_format::component::FormatValType::F64 => wrt_format::component::FormatValType::F64,
            wrt_format::component::FormatValType::Char => {
                wrt_format::component::FormatValType::Char
            },
            wrt_format::component::FormatValType::String => {
                wrt_format::component::FormatValType::String
            },
            wrt_format::component::FormatValType::Ref(idx) => {
                wrt_format::component::FormatValType::Ref(idx)
            },
            wrt_format::component::FormatValType::List(inner) => {
                wrt_format::component::FormatValType::List(Box::new(val_type_to_format_val_type(
                    *inner,
                )))
            },
            wrt_format::component::FormatValType::FixedList(inner, len) => {
                wrt_format::component::FormatValType::FixedList(
                    Box::new(val_type_to_format_val_type(*inner)),
                    len,
                )
            },
            wrt_format::component::FormatValType::Tuple(items) => {
                wrt_format::component::FormatValType::Tuple(
                    items.into_iter().map(val_type_to_format_val_type).collect(),
                )
            },
            wrt_format::component::FormatValType::Option(inner) => {
                wrt_format::component::FormatValType::Option(Box::new(val_type_to_format_val_type(
                    *inner,
                )))
            },
            wrt_format::component::FormatValType::Result(ok) => {
                wrt_format::component::FormatValType::Result(Box::new(val_type_to_format_val_type(
                    *ok,
                )))
            },
            // TODO: Fix FormatValType enum to support ResultErr and ResultBoth variants
            // wrt_format::component::FormatValType::ResultErr(err) => {
            //     wrt_format::component::FormatValType::Result(Box::new(val_type_to_format_val_type(
            //         *err,
            //     )))
            // }
            // wrt_format::component::FormatValType::ResultBoth(ok, _err) => {
            //     wrt_format::component::FormatValType::Result(Box::new(val_type_to_format_val_type(*ok)))
            // }
            wrt_format::component::FormatValType::Record(fields) => {
                wrt_format::component::FormatValType::Record(
                    fields
                        .into_iter()
                        .map(|(name, ty)| (name, val_type_to_format_val_type(ty)))
                        .collect(),
                )
            },
            wrt_format::component::FormatValType::Variant(cases) => {
                wrt_format::component::FormatValType::Variant(
                    cases
                        .into_iter()
                        .map(|(name, ty)| (name, ty.map(val_type_to_format_val_type)))
                        .collect(),
                )
            },
            wrt_format::component::FormatValType::Flags(names) => {
                wrt_format::component::FormatValType::Flags(names)
            },
            wrt_format::component::FormatValType::Enum(names) => {
                wrt_format::component::FormatValType::Enum(names)
            },
            wrt_format::component::FormatValType::Own(idx) => {
                wrt_format::component::FormatValType::Own(idx)
            },
            wrt_format::component::FormatValType::Borrow(idx) => {
                wrt_format::component::FormatValType::Borrow(idx)
            },
            wrt_format::component::FormatValType::Void => {
                wrt_format::component::FormatValType::Void
            },
            wrt_format::component::FormatValType::ErrorContext => {
                wrt_format::component::FormatValType::ErrorContext
            },
        }
    }
} // end std_parsing module

// Re-export std functions when std is available
#[cfg(feature = "std")]
// No_std implementation with bounded alternatives following functional safety guidelines
#[cfg(not(feature = "std"))]
mod no_std_parsing {
    use wrt_error::{Error, ErrorCategory, Result, codes};
    use wrt_foundation::{BoundedVec, NoStdProvider};
    // Define local stub types for no_std parsing
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Component;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct CoreInstance;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct CoreType;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Instance;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Import;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Export;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Start;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Alias;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Canon;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Value;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Module;

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct ComponentType;

    // Implement required traits for all stub types
    macro_rules! impl_stub_traits {
        ($($type:ty),*) => {
            $(
                impl wrt_foundation::traits::ToBytes for $type {
                    fn serialized_size(&self) -> usize { 0 }
                    fn to_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                        &self,
                        _writer: &mut wrt_foundation::traits::WriteStream<'a>,
                        _provider: &PStream,
                    ) -> wrt_error::Result<()> { Ok(()) }
                }

                impl wrt_foundation::traits::FromBytes for $type {
                    fn from_bytes_with_provider<'a, PStream: wrt_foundation::MemoryProvider>(
                        _reader: &mut wrt_foundation::traits::ReadStream<'a>,
                        _provider: &PStream,
                    ) -> wrt_error::Result<Self> { Ok(Self::default()) }
                }

                impl wrt_foundation::traits::Checksummable for $type {
                    fn update_checksum(&self, _checksum: &mut wrt_foundation::verification::Checksum) {}
                }
            )*
        };
    }

    impl_stub_traits!(
        Component,
        CoreInstance,
        CoreType,
        Instance,
        Import,
        Export,
        Start,
        Alias,
        Canon,
        Value,
        Module,
        ComponentType
    );

    // Type aliases for bounded parsing results
    type ParseProvider = NoStdProvider<4096>;
    type ParseVec<T> = BoundedVec<T, 32, ParseProvider>;

    // Helper function to create a provider for parsing
    fn create_parse_provider() -> Result<ParseProvider> {
        crate::prelude::create_decoder_provider::<4096>()
    }

    // Helper function to create an empty ParseVec
    fn create_empty_parse_vec<T>() -> Result<ParseVec<T>>
    where
        T: wrt_foundation::traits::Checksummable
            + wrt_foundation::traits::ToBytes
            + wrt_foundation::traits::FromBytes
            + Default
            + Clone
            + PartialEq
            + Eq,
    {
        let provider = create_parse_provider()?;
        ParseVec::new().map_err(|_| Error::parse_error("Failed to create empty parse vector "))
    }

    /// No_std parse core module section with safety bounds
    ///
    /// # Safety Requirements
    /// - Uses bounded allocation with compile-time limits
    /// - Fails gracefully when limits are exceeded
    /// - No heap allocation or dynamic memory
    pub fn parse_core_module_section(_bytes: &[u8]) -> Result<(ParseVec<Module>, usize)> {
        // Simplified parsing for no_std - only basic validation
        if _bytes.len() < 8 {
            return Err(Error::parse_error("Section data too short"));
        }

        // Return empty parsed result - complex module parsing requires std
        let empty_vec = create_empty_parse_vec::<Module>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    /// No_std parse core instance section with safety bounds
    pub fn parse_core_instance_section(_bytes: &[u8]) -> Result<(ParseVec<CoreInstance>, usize)> {
        // Simplified parsing for no_std
        let empty_vec = create_empty_parse_vec::<CoreInstance>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    /// No_std parse core type section with safety bounds
    pub fn parse_core_type_section(_bytes: &[u8]) -> Result<(ParseVec<CoreType>, usize)> {
        // Simplified parsing for no_std
        let empty_vec = create_empty_parse_vec::<CoreType>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    /// No_std parse component section with safety bounds
    pub fn parse_component_section(_bytes: &[u8]) -> Result<(ParseVec<Component>, usize)> {
        // Simplified parsing for no_std
        let empty_vec = create_empty_parse_vec::<Component>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    /// No_std parse instance section with safety bounds
    pub fn parse_instance_section(_bytes: &[u8]) -> Result<(ParseVec<Instance>, usize)> {
        // Simplified parsing for no_std
        let empty_vec = create_empty_parse_vec::<Instance>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    /// Additional parsing functions required by other modules
    pub fn parse_component_type_section(_bytes: &[u8]) -> Result<(ParseVec<ComponentType>, usize)> {
        let empty_vec = create_empty_parse_vec::<ComponentType>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    pub fn parse_import_section(_bytes: &[u8]) -> Result<(ParseVec<Import>, usize)> {
        let empty_vec = create_empty_parse_vec::<Import>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    pub fn parse_export_section(_bytes: &[u8]) -> Result<(ParseVec<Export>, usize)> {
        let empty_vec = create_empty_parse_vec::<Export>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    pub fn parse_start_section(_bytes: &[u8]) -> Result<(ParseVec<Start>, usize)> {
        let empty_vec = create_empty_parse_vec::<Start>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    pub fn parse_alias_section(_bytes: &[u8]) -> Result<(ParseVec<Alias>, usize)> {
        let empty_vec = create_empty_parse_vec::<Alias>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    pub fn parse_canon_section(_bytes: &[u8]) -> Result<(ParseVec<Canon>, usize)> {
        let empty_vec = create_empty_parse_vec::<Canon>().unwrap_or_default();
        Ok((empty_vec, 0))
    }

    pub fn parse_value_section(_bytes: &[u8]) -> Result<(ParseVec<Value>, usize)> {
        let empty_vec = create_empty_parse_vec::<Value>().unwrap_or_default();
        Ok((empty_vec, 0))
    }
}

// Re-export std functions when std feature is enabled
#[cfg(feature = "std")]
pub use std_parsing::*;
