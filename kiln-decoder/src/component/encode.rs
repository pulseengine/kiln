// Copyright (c) 2025 Ralf Anton Beier
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

// Component encoding requires std for Box and Vec - entire module is std-only
#[cfg(feature = "std")]
mod std_encoding {
    use wrt_error::Result;
    use wrt_format::{binary, component::Component};

    use crate::prelude::*;

    /// Encode a WebAssembly Component Model component into binary format
    pub fn encode_component(component: &Component) -> Result<Vec<u8>> {
        let mut binary = Vec::new();

        // Write magic and version
        binary.extend_from_slice(&binary::COMPONENT_MAGIC); // \0asm
        binary.extend_from_slice(&binary::COMPONENT_VERSION); // Version and layer

        // Encode and write sections
        encode_sections(component, &mut binary)?;

        Ok(binary)
    }

    fn encode_sections(component: &Component, binary: &mut Vec<u8>) -> Result<()> {
        // If the component has a name, add a name section
        if component.name.is_some() {
            // Create a name section with the component name
            let name_section = crate::component::name_section::ComponentNameSection {
                component_name: component.name.clone(),
                sort_names: Vec::new(),
                import_names: Default::default(),
                export_names: Default::default(),
                canonical_names: Default::default(),
                type_names: Default::default(),
            };

            let name_section_bytes =
                crate::component::name_section::generate_component_name_section(&name_section)?;

            // Create the custom section content
            let mut custom_section_content = Vec::new();
            custom_section_content.extend_from_slice(&write_string("name"));
            custom_section_content.extend_from_slice(&name_section_bytes);

            // Add the custom section
            crate::component::utils::add_section(
                binary,
                binary::COMPONENT_CUSTOM_SECTION_ID,
                &custom_section_content,
            );
        }

        // Encode core module section
        if !component.modules.is_empty() {
            let module_data = encode_core_module_section(&component.modules)?;
            crate::component::utils::add_section(
                binary,
                binary::COMPONENT_CORE_MODULE_SECTION_ID,
                &module_data,
            );
        }

        // Encode core instance section
        if !component.core_instances.is_empty() {
            let instance_data = encode_core_instance_section(&component.core_instances)?;
            crate::component::utils::add_section(
                binary,
                binary::COMPONENT_CORE_INSTANCE_SECTION_ID,
                &instance_data,
            );
        }

        // Encode import section
        if !component.imports.is_empty() {
            let import_data = encode_import_section(&component.imports)?;
            crate::component::utils::add_section(
                binary,
                binary::COMPONENT_IMPORT_SECTION_ID,
                &import_data,
            );
        }

        // Encode export section
        if !component.exports.is_empty() {
            let export_data = encode_export_section(&component.exports)?;
            crate::component::utils::add_section(
                binary,
                binary::COMPONENT_EXPORT_SECTION_ID,
                &export_data,
            );
        }

        Ok(())
    }

    fn encode_core_module_section(modules: &[wrt_format::module::Module]) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Write count of modules
        data.extend_from_slice(&write_leb128_u32(modules.len() as u32));

        // Encode each module
        for module in modules {
            // Encode module binary
            let module_binary = module.binary.as_ref().ok_or_else(|| {
                wrt_error::Error::parse_error("Module binary not available for encoding")
            })?;

            // Write module size
            data.extend_from_slice(&write_leb128_u32(module_binary.len() as u32));
            // Write module binary
            data.extend_from_slice(module_binary);
        }

        Ok(data)
    }

    #[cfg(feature = "std")]
    fn encode_core_instance_section(
        instances: &[wrt_format::component::CoreInstance],
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Write count of instances
        data.extend_from_slice(&write_leb128_u32(instances.len() as u32));

        // Encode each instance
        for instance in instances {
            match &instance.instance_expr {
                wrt_format::component::CoreInstanceExpr::ModuleReference {
                    module_idx,
                    arg_refs,
                } => {
                    // Write instantiate tag
                    data.push(binary::CORE_INSTANCE_INSTANTIATE_TAG);

                    // Write module index
                    data.extend_from_slice(&write_leb128_u32(*module_idx));

                    // Write count of arguments
                    data.extend_from_slice(&write_leb128_u32(arg_refs.len() as u32));

                    // Write each argument
                    for arg in arg_refs {
                        // Write argument name
                        data.extend_from_slice(&write_string(&arg.name));
                        // Write kind byte
                        data.push(arg.kind);
                        // Write index in the corresponding index space
                        data.extend_from_slice(&write_leb128_u32(arg.idx));
                    }
                },
                wrt_format::component::CoreInstanceExpr::InlineExports(exports) => {
                    // Write inline exports tag
                    data.push(binary::CORE_INSTANCE_INLINE_EXPORTS_TAG);

                    // Write count of exports
                    data.extend_from_slice(&write_leb128_u32(exports.len() as u32));

                    // Write each export
                    for export in exports {
                        // Write export name
                        data.extend_from_slice(&write_string(&export.name));
                        // Write sort
                        data.push(export.sort as u8);
                        // Write index
                        data.extend_from_slice(&write_leb128_u32(export.idx));
                    }
                },
            }
        }

        Ok(data)
    }

    fn encode_import_section(imports: &[wrt_format::component::Import]) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Write count of imports
        data.extend_from_slice(&write_leb128_u32(imports.len() as u32));

        // Encode each import
        for import in imports {
            // Write namespace
            data.extend_from_slice(&write_string(&import.name.namespace));

            // Write name
            data.extend_from_slice(&write_string(&import.name.name));

            // Write nested namespaces flag and contents (if any)
            if import.name.nested.is_empty() {
                data.push(0); // No nested namespaces
            } else {
                data.push(1); // Has nested namespaces

                // Write count of nested namespaces
                data.extend_from_slice(&write_leb128_u32(import.name.nested.len() as u32));

                // Write each nested namespace
                for nested in &import.name.nested {
                    data.extend_from_slice(&write_string(nested));
                }
            }

            // Write package flag and contents (if any)
            if let Some(package) = &import.name.package {
                data.push(1); // Has package

                // Write package name
                data.extend_from_slice(&write_string(&package.name));

                // Write version flag and value (if any)
                if let Some(version) = &package.version {
                    data.push(1); // Has version
                    data.extend_from_slice(&write_string(version));
                } else {
                    data.push(0); // No version
                }

                // Write hash flag and value (if any)
                if let Some(hash) = &package.hash {
                    data.push(1); // Has hash
                    data.extend_from_slice(&write_string(hash));
                } else {
                    data.push(0); // No hash
                }
            } else {
                data.push(0); // No package
            }

            // Write import type
            encode_extern_type(&import.ty, &mut data)?;
        }

        Ok(data)
    }

    fn encode_extern_type(
        ty: &wrt_format::component::ExternType,
        data: &mut Vec<u8>,
    ) -> Result<()> {
        match ty {
            wrt_format::component::ExternType::Function { params, results } => {
                // Write function tag
                data.push(binary::EXTERN_TYPE_FUNCTION_TAG);

                // Write parameter count
                data.extend_from_slice(&write_leb128_u32(params.len() as u32));

                // Write each parameter
                for (name, param_ty) in params {
                    // Write parameter name
                    data.extend_from_slice(&write_string(name));
                    // Write parameter type
                    encode_val_type(&format_val_type_to_val_type(param_ty), data)?;
                }

                // Write result count
                data.extend_from_slice(&write_leb128_u32(results.len() as u32));

                // Write each result type
                for result_ty in results {
                    encode_val_type(&format_val_type_to_val_type(result_ty), data)?;
                }
            },
            wrt_format::component::ExternType::Value(val_ty) => {
                // Write value tag
                data.push(binary::EXTERN_TYPE_VALUE_TAG);
                // Write value type
                encode_val_type(&format_val_type_to_val_type(val_ty), data)?;
            },
            wrt_format::component::ExternType::Type(type_idx) => {
                // Write type tag
                data.push(binary::EXTERN_TYPE_TYPE_TAG);
                // Write type index
                data.extend_from_slice(&write_leb128_u32(*type_idx));
            },
            wrt_format::component::ExternType::Instance { exports } => {
                // Write instance tag
                data.push(binary::EXTERN_TYPE_INSTANCE_TAG);

                // Write export count
                data.extend_from_slice(&write_leb128_u32(exports.len() as u32));

                // Write each export
                for (name, export_ty) in exports {
                    // Write export name
                    data.extend_from_slice(&write_string(name));
                    // Write export type
                    encode_extern_type(export_ty, data)?;
                }
            },
            wrt_format::component::ExternType::Module { type_idx } => {
                // Core module: 0x00 0x11 type_idx
                data.push(0x00);
                data.push(0x11);
                data.extend_from_slice(&write_leb128_u32(*type_idx));
            },
            wrt_format::component::ExternType::Component { type_idx } => {
                // Component: 0x04 type_idx
                data.push(0x04);
                data.extend_from_slice(&write_leb128_u32(*type_idx));
            },
        }

        Ok(())
    }

    fn encode_val_type(
        ty: &wrt_format::component::FormatValType,
        data: &mut Vec<u8>,
    ) -> Result<()> {
        match ty {
            wrt_format::component::FormatValType::Bool => {
                data.push(binary::VAL_TYPE_BOOL_TAG);
            },
            wrt_format::component::FormatValType::S8 => {
                data.push(binary::VAL_TYPE_S8_TAG);
            },
            wrt_format::component::FormatValType::U8 => {
                data.push(binary::VAL_TYPE_U8_TAG);
            },
            wrt_format::component::FormatValType::S16 => {
                data.push(binary::VAL_TYPE_S16_TAG);
            },
            wrt_format::component::FormatValType::U16 => {
                data.push(binary::VAL_TYPE_U16_TAG);
            },
            wrt_format::component::FormatValType::S32 => {
                data.push(binary::VAL_TYPE_S32_TAG);
            },
            wrt_format::component::FormatValType::U32 => {
                data.push(binary::VAL_TYPE_U32_TAG);
            },
            wrt_format::component::FormatValType::S64 => {
                data.push(binary::VAL_TYPE_S64_TAG);
            },
            wrt_format::component::FormatValType::U64 => {
                data.push(binary::VAL_TYPE_U64_TAG);
            },
            wrt_format::component::FormatValType::F32 => {
                data.push(binary::VAL_TYPE_F32_TAG);
            },
            wrt_format::component::FormatValType::F64 => {
                data.push(binary::VAL_TYPE_F64_TAG);
            },
            wrt_format::component::FormatValType::Char => {
                data.push(binary::VAL_TYPE_CHAR_TAG);
            },
            wrt_format::component::FormatValType::String => {
                data.push(binary::VAL_TYPE_STRING_TAG);
            },
            wrt_format::component::FormatValType::Ref(type_idx) => {
                data.push(binary::VAL_TYPE_REF_TAG);
                data.extend_from_slice(&write_leb128_u32(*type_idx));
            },
            wrt_format::component::FormatValType::Record(fields) => {
                data.push(binary::VAL_TYPE_RECORD_TAG);
                data.extend_from_slice(&write_leb128_u32(fields.len() as u32));
                for (name, field_ty) in fields {
                    data.extend_from_slice(&write_string(name));
                    encode_val_type(field_ty, data)?;
                }
            },
            wrt_format::component::FormatValType::Variant(cases) => {
                data.push(binary::VAL_TYPE_VARIANT_TAG);
                data.extend_from_slice(&write_leb128_u32(cases.len() as u32));
                for (name, case_ty) in cases {
                    data.extend_from_slice(&write_string(name));
                    match case_ty {
                        Some(ty) => {
                            data.push(1);
                            encode_val_type(ty, data)?;
                        },
                        None => {
                            data.push(0);
                        },
                    }
                }
            },
            wrt_format::component::FormatValType::List(element_ty) => {
                data.push(binary::VAL_TYPE_LIST_TAG);
                encode_val_type(element_ty, data)?;
            },
            wrt_format::component::FormatValType::FixedList(element_ty, length) => {
                data.push(binary::VAL_TYPE_FIXED_LIST_TAG);
                encode_val_type(element_ty, data)?;
                data.extend_from_slice(&write_leb128_u32(*length));
            },
            wrt_format::component::FormatValType::Tuple(types) => {
                data.push(binary::VAL_TYPE_TUPLE_TAG);
                data.extend_from_slice(&write_leb128_u32(types.len() as u32));
                for ty in types {
                    encode_val_type(ty, data)?;
                }
            },
            wrt_format::component::FormatValType::Flags(names) => {
                data.push(binary::VAL_TYPE_FLAGS_TAG);
                data.extend_from_slice(&write_leb128_u32(names.len() as u32));
                for name in names {
                    data.extend_from_slice(&write_string(name));
                }
            },
            wrt_format::component::FormatValType::Enum(names) => {
                data.push(binary::VAL_TYPE_ENUM_TAG);
                data.extend_from_slice(&write_leb128_u32(names.len() as u32));
                for name in names {
                    data.extend_from_slice(&write_string(name));
                }
            },
            wrt_format::component::FormatValType::Option(element_ty) => {
                data.push(binary::VAL_TYPE_OPTION_TAG);
                encode_val_type(element_ty, data)?;
            },
            wrt_format::component::FormatValType::Result(ok_ty) => {
                data.push(binary::VAL_TYPE_RESULT_TAG);
                encode_val_type(ok_ty, data)?;
            },
            // TODO: Fix FormatValType enum to support ResultErr and ResultBoth variants
            // wrt_format::component::FormatValType::ResultErr(err_ty) => {
            //     data.push(binary::VAL_TYPE_RESULT_ERR_TAG);
            //     encode_val_type(err_ty, data)?;
            // }
            // wrt_format::component::FormatValType::ResultBoth(ok_ty, err_ty) => {
            //     data.push(binary::VAL_TYPE_RESULT_BOTH_TAG);
            //     encode_val_type(ok_ty, data)?;
            //     encode_val_type(err_ty, data)?;
            // }
            wrt_format::component::FormatValType::Own(type_idx) => {
                data.push(binary::VAL_TYPE_OWN_TAG);
                data.extend_from_slice(&write_leb128_u32(*type_idx));
            },
            wrt_format::component::FormatValType::Borrow(type_idx) => {
                data.push(binary::VAL_TYPE_BORROW_TAG);
                data.extend_from_slice(&write_leb128_u32(*type_idx));
            },
            wrt_format::component::FormatValType::Void => {
                // There doesn't seem to be a Void tag in the binary constants
                // We'll need to add this or map it to the appropriate value
                return Err(Error::validation_error(
                    "Void type encoding not yet implemented",
                ));
            },
            wrt_format::component::FormatValType::ErrorContext => {
                data.push(binary::VAL_TYPE_ERROR_CONTEXT_TAG);
            },
        }

        Ok(())
    }

    fn sort_to_u8(sort: &wrt_format::component::Sort) -> u8 {
        match sort {
            wrt_format::component::Sort::Core(core_sort) => match core_sort {
                wrt_format::component::CoreSort::Function => binary::COMPONENT_CORE_SORT_FUNC,
                wrt_format::component::CoreSort::Table => binary::COMPONENT_CORE_SORT_TABLE,
                wrt_format::component::CoreSort::Memory => binary::COMPONENT_CORE_SORT_MEMORY,
                wrt_format::component::CoreSort::Global => binary::COMPONENT_CORE_SORT_GLOBAL,
                wrt_format::component::CoreSort::Type => binary::COMPONENT_CORE_SORT_TYPE,
                wrt_format::component::CoreSort::Module => binary::COMPONENT_CORE_SORT_MODULE,
                wrt_format::component::CoreSort::Instance => binary::COMPONENT_CORE_SORT_INSTANCE,
            },
            wrt_format::component::Sort::Function => binary::COMPONENT_SORT_FUNC,
            wrt_format::component::Sort::Value => binary::COMPONENT_SORT_VALUE,
            wrt_format::component::Sort::Type => binary::COMPONENT_SORT_TYPE,
            wrt_format::component::Sort::Component => binary::COMPONENT_SORT_COMPONENT,
            wrt_format::component::Sort::Instance => binary::COMPONENT_SORT_INSTANCE,
        }
    }

    fn encode_export_section(exports: &[wrt_format::component::Export]) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Write count of exports
        data.extend_from_slice(&write_leb128_u32(exports.len() as u32));

        // Encode each export
        for export in exports {
            // Write export basic name
            data.extend_from_slice(&write_string(&export.name.name));

            // Prepare flags
            let mut flags: u8 = 0;
            if export.name.is_resource {
                flags |= 0x01;
            }
            if export.name.semver.is_some() {
                flags |= 0x02;
            }
            if export.name.integrity.is_some() {
                flags |= 0x04;
            }
            if !export.name.nested.is_empty() {
                flags |= 0x08;
            }

            // Write flags
            data.push(flags);

            // Write semver if present
            if let Some(semver) = &export.name.semver {
                data.extend_from_slice(&write_string(semver));
            }

            // Write integrity if present
            if let Some(integrity) = &export.name.integrity {
                data.extend_from_slice(&write_string(integrity));
            }

            // Write nested namespaces if present
            if !export.name.nested.is_empty() {
                // Write count of nested namespaces
                data.extend_from_slice(&write_leb128_u32(export.name.nested.len() as u32));

                // Write each nested namespace
                for nested in &export.name.nested {
                    data.extend_from_slice(&write_string(nested));
                }
            }

            // Write sort byte
            data.push(sort_to_u8(&export.sort));

            // Write index
            data.extend_from_slice(&write_leb128_u32(export.idx));

            // Write type flag and type (if present)
            if let Some(ty) = &export.ty {
                data.push(1); // Has type
                encode_extern_type(ty, &mut data)?;
            } else {
                data.push(0); // No type
            }
        }

        Ok(data)
    }

    /// Convert FormatValType to ValType
    ///
    /// This function properly handles the conversion between different ValType
    /// representations without creating references to temporary values.
    fn format_val_type_to_val_type(
        val_type: &wrt_format::component::FormatValType,
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
                // Clone the value to avoid reference to temporary
                let idx_value = *idx;
                wrt_format::component::FormatValType::Ref(idx_value)
            },
            wrt_format::component::FormatValType::List(inner) => {
                // Create a new boxed value instead of referencing the inner value
                let inner_val_type = format_val_type_to_val_type(inner);
                wrt_format::component::FormatValType::List(Box::new(inner_val_type))
            },
            wrt_format::component::FormatValType::FixedList(inner, len) => {
                // Clone the values to avoid references to temporaries
                let inner_val_type = format_val_type_to_val_type(inner);
                let len_value = *len;
                wrt_format::component::FormatValType::FixedList(Box::new(inner_val_type), len_value)
            },
            wrt_format::component::FormatValType::Record(fields) => {
                // Create new vectors of fields to avoid references to temporaries
                let mut new_fields = Vec::with_capacity(fields.len());
                for (name, field_type) in fields {
                    let new_name = name.clone();
                    let new_field_type = format_val_type_to_val_type(field_type);
                    new_fields.push((new_name, new_field_type));
                }
                wrt_format::component::FormatValType::Record(new_fields)
            },
            wrt_format::component::FormatValType::Variant(cases) => {
                // Create new vectors of cases to avoid references to temporaries
                let mut new_cases = Vec::with_capacity(cases.len());
                for (name, case_type) in cases {
                    let new_name = name.clone();
                    let new_case_type = case_type.as_ref().map(format_val_type_to_val_type);
                    new_cases.push((new_name, new_case_type));
                }
                wrt_format::component::FormatValType::Variant(new_cases)
            },
            wrt_format::component::FormatValType::Tuple(types) => {
                // Create new vectors of types to avoid references to temporaries
                let new_types = types.iter().map(format_val_type_to_val_type).collect();
                wrt_format::component::FormatValType::Tuple(new_types)
            },
            wrt_format::component::FormatValType::Flags(names) => {
                // Clone the names to avoid references to temporaries
                let new_names = names.clone();
                wrt_format::component::FormatValType::Flags(new_names)
            },
            wrt_format::component::FormatValType::Enum(names) => {
                // Clone the names to avoid references to temporaries
                let new_names = names.clone();
                wrt_format::component::FormatValType::Enum(new_names)
            },
            wrt_format::component::FormatValType::Option(inner) => {
                // Create a new boxed value instead of referencing the inner value
                let inner_val_type = format_val_type_to_val_type(inner);
                wrt_format::component::FormatValType::Option(Box::new(inner_val_type))
            },
            wrt_format::component::FormatValType::Result(inner) => {
                // Handle Result with either Ok or Err value
                // We assume inner is not None for this implementation
                // since we're dealing with boxed values
                let inner_val_type = format_val_type_to_val_type(inner);
                wrt_format::component::FormatValType::Result(Box::new(inner_val_type))
            },
            wrt_format::component::FormatValType::Own(resource_idx) => {
                // Clone the resource index to avoid reference to temporary
                let idx_value = *resource_idx;
                wrt_format::component::FormatValType::Own(idx_value)
            },
            wrt_format::component::FormatValType::Borrow(resource_idx) => {
                // Clone the resource index to avoid reference to temporary
                let idx_value = *resource_idx;
                wrt_format::component::FormatValType::Borrow(idx_value)
            },
            wrt_format::component::FormatValType::Void => {
                wrt_format::component::FormatValType::Void
            },
            wrt_format::component::FormatValType::ErrorContext => {
                wrt_format::component::FormatValType::ErrorContext
            },
        }
    }

    #[cfg(test)]
    mod tests {
        use wrt_format::component::{
            Component, CoreInlineExport, CoreInstance, CoreInstanceExpr, CoreSort, Export,
            ExportName, Import, ImportName, Sort,
        };

        use super::*;

        #[test]
        fn test_encode_empty_component() {
            let component = Component::new();
            let binary = encode_component(&component).unwrap();

            // Check magic and version
            assert_eq!(&binary[0..4], binary::COMPONENT_MAGIC);
            assert_eq!(&binary[4..8], binary::COMPONENT_VERSION);

            // Should only have magic and version
            assert_eq!(binary.len(), 8);
        }

        #[test]
        fn test_encode_component_with_name() {
            let mut component = Component::new();
            component.name = Some("test_component".to_string());

            let binary = encode_component(&component).unwrap();

            // Check magic and version
            assert_eq!(&binary[0..4], binary::COMPONENT_MAGIC);
            assert_eq!(&binary[4..8], binary::COMPONENT_VERSION);

            // Should have name section
            assert!(binary.len() > 8);
        }

        #[test]
        fn test_encode_component_with_core_instance() {
            let mut component = Component::new();

            // Add a core instance with inline exports
            let instance = CoreInstance {
                instance_expr: CoreInstanceExpr::InlineExports(vec![CoreInlineExport {
                    name: "test_export".to_string(),
                    sort: CoreSort::Function,
                    idx: 0,
                }]),
            };
            component.core_instances.push(instance);

            let binary = encode_component(&component).unwrap();

            // Check magic and version
            assert_eq!(&binary[0..4], binary::COMPONENT_MAGIC);
            assert_eq!(&binary[4..8], binary::COMPONENT_VERSION);

            // Should have core instance section
            assert!(binary.len() > 8);
        }

        #[test]
        fn test_encode_component_with_imports() {
            let mut component = Component::new();

            // Add an import
            let import = Import {
                name: ImportName::new("test_namespace".to_string(), "test_import".to_string()),
                ty: wrt_format::component::ExternType::Value(
                    wrt_format::component::FormatValType::String,
                ),
            };
            component.imports.push(import);

            let binary = encode_component(&component).unwrap();

            // Check magic and version
            assert_eq!(&binary[0..4], binary::COMPONENT_MAGIC);
            assert_eq!(&binary[4..8], binary::COMPONENT_VERSION);

            // Should have import section
            assert!(binary.len() > 8);
        }

        #[test]
        fn test_encode_component_with_exports() {
            let mut component = Component::new();

            // Add an export
            let export = Export {
                name: ExportName::new("test_export".to_string()),
                sort: Sort::Function,
                idx: 0,
                ty: None,
            };
            component.exports.push(export);

            let binary = encode_component(&component).unwrap();

            // Check magic and version
            assert_eq!(&binary[0..4], binary::COMPONENT_MAGIC);
            assert_eq!(&binary[4..8], binary::COMPONENT_VERSION);

            // Should have export section
            assert!(binary.len() > 8);
        }
    }
} // end std_encoding module

// Re-export std functions when std is available
#[cfg(feature = "std")]
pub use std_encoding::*;

// No_std implementation following functional safety guidelines
#[cfg(not(feature = "std"))]
mod no_std_encoding {
    use wrt_error::{Error, ErrorCategory, Result, codes};
    use wrt_foundation::{BoundedVec, safe_memory::NoStdProvider};

    // No_std stub types for components that can't be fully encoded without heap
    // allocation
    #[derive(Debug, Clone)]
    pub struct Component {
        // Simplified component representation for no_std
        pub magic: [u8; 4],
        pub version: [u8; 4],
    }

    /// No_std encode function with bounded output and safety constraints
    ///
    /// # Safety Requirements
    /// - Uses bounded allocation with compile-time limits
    /// - Fails gracefully when limits are exceeded
    /// - No heap allocation or dynamic memory
    pub fn encode_component(
        component: &Component,
    ) -> Result<BoundedVec<u8, 1024, NoStdProvider<2048>>> {
        let provider = wrt_foundation::safe_managed_alloc!(2048, wrt_foundation::CrateId::Decoder)?;
        let mut binary = BoundedVec::new(provider)
            .map_err(|_| Error::memory_error("Failed to create encoding buffer"))?;

        // Write magic and version - these are fixed size and safe
        for &byte in &component.magic {
            binary
                .push(byte)
                .map_err(|_| Error::memory_error("Component encoding buffer overflow"))?;
        }

        for &byte in &component.version {
            binary
                .push(byte)
                .map_err(|_| Error::memory_error("Component encoding buffer overflow"))?;
        }

        Ok(binary)
    }
}
